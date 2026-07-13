# Contract: Helm extraction pipeline (m188)

**Feature**: [../spec.md](../spec.md) · **Plan**: [../plan.md](../plan.md) · **Data model**: [../data-model.md](../data-model.md)

## Pipeline shape

```
--path <dir>  OR  --helm-chart <path>
        │
        ▼
 [Input resolution]
   • dir → chart_dir = <dir>
   • .tgz → extract to tempdir → chart_dir = <tempdir>/<extracted-root>
        │
        ▼
 <chart_dir>/Chart.yaml exists?
    ╱                     ╲
  no                     yes
   │                      │
   ▼                      ▼
 skip helm      [Phase A — chart-level (US1)]
 emission        1. Parse Chart.yaml → ChartMetadata
                 2. Emit HelmComponent::Chart (root)
                 3. If <chart_dir>/Chart.lock exists → parse ChartLock
                    (takes precedence for versions per FR-004)
                 4. For each dep in Chart.yaml (or Chart.lock):
                      Emit HelmComponent::ChartDep
                 5. For each <chart_dir>/charts/*.tgz:
                      Recursively extract + Phase A on the subchart
                      (depth cap = m114's 12)
                      │
                      ▼
                 [Phase B — template-level (US2)]
                 1. Enumerate <chart_dir>/templates/*.yaml + crds/*.yaml
                 2. For each file, line-scan regex for `image:` refs
                    (falls back to line-based extraction if YAML broken
                    by Go-template blocks per FR-006)
                 3. Classify each ref into ImageRefKind (Digested /
                    Tagged / TemplatePlaceholder)
                 4. Dedup — same ref from multiple files → 1
                    HelmComponent::Image + evidence.occurrences[]
                    listing all source paths per FR-009
                 5. Emit HelmComponent::Image per unique ref
                      │
                      ▼
                 [Phase C — rendered extraction (US3, opt-in)]
                 --helm-render set?
                    ╱                     ╲
                  no                     yes
                   │                      │
                   ▼                      ▼
                emit `partial`     [C.1] helm binary on $PATH?
                completeness              │
                annotation               yes / no
                                          │
                            ┌─────────────┼──────────────┐
                            │             │              │
                     yes: shell out    no: WARN     shell-out failure
                     `helm template`   + fallback    (non-zero / timeout)
                     w/ 60s timeout    to Phase B    → WARN + fallback
                     (env override                    to Phase B
                     MIKEBOM_HELM_
                     RENDER_TIMEOUT_
                     SECS)
                            │
                     [C.2] rendered YAML → same
                            regex extraction → replace
                            Phase B's Image emissions
                            → emit `full` completeness annotation
                            │
                            ▼
              [Phase D — emission]
              All HelmComponents → PackageDbEntry via helm.rs's
              build_entry_from_helm_component. Apply Property
              matrix (data-model.md §6).
```

## Per-phase contracts

### Phase A — chart-level (US1)

**Function**: `helm::read_chart_at(chart_dir: &Path) -> Result<Vec<HelmComponent>, HelmParseError>`

**Success signal**: `Ok(components)` — one Chart + N ChartDeps.

**Failure signals**:
- `Err(HelmParseError::ChartYamlRead)` — file doesn't exist or unreadable. Bubbled up; caller decides whether to WARN + skip (auto-detect path) or exit non-zero (explicit `--helm-chart` path).
- `Err(HelmParseError::ChartYamlParse)` — malformed YAML. Same bubble-up.
- `Err(HelmParseError::ChartLockParse)` — Chart.lock present but broken. WARN + Chart.yaml-only fallback (does NOT propagate).
- `Err(HelmParseError::SubchartTarballFailed)` — a specific `charts/*.tgz` failed. WARN + skip that subchart; parent + siblings still emit.

**Version precedence** (FR-004):
1. If `Chart.lock` exists AND parses successfully AND contains an entry matching a Chart.yaml dep by `name`+`repository`, use `Chart.lock`'s `version`. Annotate `mikebom:helm-lock-authoritative = "true"`.
2. Else use `Chart.yaml`'s declared `version`.

### Phase B — template-level unrendered (US2)

**Function**: `helm::extract_image_refs_unrendered(chart_dir: &Path) -> Vec<ImageRef>`

**Extraction regex** (per Assumption in spec.md):
```rust
static IMAGE_REGEX: OnceLock<Regex> = OnceLock::new();
IMAGE_REGEX.get_or_init(|| {
    Regex::new(r#"(?m)^(\s*)image:\s*['"]?([^'"\s{]+(?:\{\{[^}]+\}\}[^'"\s]*)*)['"]?\s*(#.*)?$"#).unwrap()
})
```

The regex has three capture groups:
1. leading whitespace (unused; for future YAML-context inference)
2. the image ref value (may include Go-template blocks)
3. optional trailing comment (unused; for future line-source annotation)

**File targets**: `templates/**/*.yaml`, `templates/**/*.yml`, `crds/*.yaml`, `crds/*.yml`. Recursive under `templates/` (charts often nest `templates/tests/*.yaml` for hooks); flat under `crds/`.

**Success signal**: always returns `Vec<ImageRef>` (empty is valid — no images referenced). Individual template file read failures WARN + skip (do not gate emission).

**Dedup**: keyed on `ImageRef::raw` (exact string match). Same image across multiple templates → single `ImageRef` with `source_paths` accumulating.

### Phase C — rendered extraction (US3, opt-in)

**Function**: `helm::extract_image_refs_rendered(chart_dir: &Path, timeout_secs: u64) -> Result<Vec<ImageRef>, HelmRenderError>`

**Subprocess**:
```rust
std::process::Command::new("helm")
    .args(["template", chart_dir.to_str().unwrap()])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?
```

**Timeout**: enforced via `std::thread::spawn` + `std::sync::mpsc::channel()` — same pattern as `mikebom-cli/src/scan_fs/package_db/golang/go_mod_graph.rs:81-158`. On timeout expire, kill the child process (`Child::kill()`), consume the pipe buffers, return `HelmRenderError::Timeout`.

**Stderr handling** (FR-018): if the subprocess emits stderr AND exits non-zero, the FIRST 20 LINES are captured and logged at WARN level. No full stderr — kubeconfig paths + secret values may leak.

**Success signal**: `Ok(image_refs)` — extracted from the RENDERED stdout via the same Phase B regex. Zero placeholder markers expected (any placeholder in rendered output indicates a chart bug or explicitly-unbound value).

**Failure signals + fall-through**:
- `Err(HelmRenderError::BinaryNotFound)` — `Command::spawn` failed with `ENOENT`. WARN + fallback to Phase B.
- `Err(HelmRenderError::NonZeroExit { code, stderr_head })` — subprocess exited non-zero. WARN with `code + stderr_head` + fallback.
- `Err(HelmRenderError::Timeout)` — subprocess exceeded timeout. WARN + fallback.
- `Err(HelmRenderError::IoError)` — pipe read failed / process wait failed. WARN + fallback.

### Phase D — emission

**Function**: `helm::components_to_package_db_entries(components: Vec<HelmComponent>, extraction_mode: ExtractionMode) -> Vec<PackageDbEntry>`

`ExtractionMode { Unrendered, Rendered }` drives the document-scope `mikebom:image-extraction-completeness` annotation:
- `Unrendered` → `"partial"`
- `Rendered` → `"full"`

Per-component property emission follows data-model.md §6 matrix.

## Overall return shape (top-level `helm::read`)

```rust
pub fn read(
    rootfs: &Path,
    render_mode: HelmRenderMode,
) -> Result<Vec<PackageDbEntry>, HelmParseError> {
    // 1. Detect Chart.yaml at rootfs (auto-detect gate).
    if !rootfs.join("Chart.yaml").is_file() {
        return Ok(Vec::new());  // no-op — not a helm chart; other readers still run
    }

    // 2. Phase A — chart-level.
    let chart_components = read_chart_at(rootfs)?;

    // 3. Phase B or C — template-level.
    let (image_refs, extraction_mode) = match render_mode {
        HelmRenderMode::OptIn => {
            let timeout = resolve_render_timeout();
            match extract_image_refs_rendered(rootfs, timeout) {
                Ok(refs) => (refs, ExtractionMode::Rendered),
                Err(e) => {
                    tracing::warn!(error = %e, "helm-render failed; falling back to unrendered extraction");
                    (extract_image_refs_unrendered(rootfs), ExtractionMode::Unrendered)
                }
            }
        }
        HelmRenderMode::Off => (extract_image_refs_unrendered(rootfs), ExtractionMode::Unrendered),
    };

    // 4. Phase D — emission.
    let mut all_components = chart_components;
    all_components.extend(image_refs.into_iter().map(HelmComponent::Image));
    Ok(components_to_package_db_entries(all_components, extraction_mode))
}
```

## Byte-identity invariant (FR-016 / SC-005)

**Any scan where `Chart.yaml` is NOT present at rootfs AND `--helm-chart` is NOT passed** MUST produce output byte-identical to pre-m188 for the same input. Specifically:
- `helm::read` returns `Ok(vec![])` at the auto-detect gate.
- No `mikebom:image-extraction-completeness` annotation added to the document.
- No new PURL types (`pkg:helm/`, `pkg:generic/<placeholder>`) appear.

Verified via T035-equivalent golden-regen zero-drift check in Phase 6 (see tasks.md).

## Test surface

Data-model.md §7 enumerates 16 unit tests + 16 integration tests. Each per-phase outcome + edge case has ≥1 dedicated test.
