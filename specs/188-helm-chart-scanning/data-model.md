# Data Model: Helm chart scanning (m188)

**Feature**: [spec.md](./spec.md) · **Plan**: [plan.md](./plan.md) · **Research**: [research.md](./research.md)

## 1. Core types

### 1.1 `ChartMetadata`

**File**: `mikebom-cli/src/scan_fs/package_db/helm.rs` (private)

Deserialized from `Chart.yaml` via `serde_yaml`.

```rust
#[derive(Debug, Clone, Deserialize)]
struct ChartMetadata {
    /// Chart name (`name:` field). REQUIRED.
    name: String,
    /// SemVer version (`version:` field). REQUIRED.
    version: String,
    /// Chart type (`type: application | library`). Defaults to
    /// "application" when omitted (Helm convention).
    #[serde(default = "default_chart_type")]
    r#type: String,
    /// Application version (`appVersion:` — the version of the app
    /// packaged by the chart, distinct from `version:` which is the
    /// chart's own version). Optional.
    #[serde(default, rename = "appVersion")]
    app_version: Option<String>,
    /// Dependencies declared in Chart.yaml. Each is emitted as its own
    /// component per FR-003.
    #[serde(default)]
    dependencies: Vec<ChartDep>,
    /// Description, keywords, home, sources, maintainers — captured for
    /// optional annotation emission but not gating.
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    home: Option<String>,
}

fn default_chart_type() -> String { "application".to_string() }
```

### 1.2 `ChartDep`

Deserialized from `Chart.yaml`'s `dependencies[]` array OR `Chart.lock`'s locked-resolution list.

```rust
#[derive(Debug, Clone, Deserialize)]
struct ChartDep {
    /// Dep chart name. REQUIRED.
    name: String,
    /// Dep chart version (SemVer or SemVer range in Chart.yaml; exact
    /// pinned version in Chart.lock). REQUIRED.
    version: String,
    /// Dep repository — URL (`https://charts.bitnami.com/bitnami`) OR
    /// `@`-prefixed alias (`@bitnami`). REQUIRED in modern charts;
    /// legacy charts may omit. When omitted, fall back to
    /// `pkg:generic/<name>@<version>` with a WARN log naming the
    /// ambiguity per FR-003.
    #[serde(default)]
    repository: Option<String>,
    /// Optional dependency alias (`alias:` field). When present, takes
    /// precedence over `repository` for the PURL `<namespace>` segment.
    #[serde(default)]
    alias: Option<String>,
    /// Conditional-inclusion expression (`condition:` field). Present
    /// but not evaluated in m188 — captured as annotation only.
    #[serde(default)]
    condition: Option<String>,
}
```

### 1.3 `ChartLock`

Deserialized from `Chart.lock`. Structurally similar to `ChartMetadata` but scoped to locked resolutions.

```rust
#[derive(Debug, Clone, Deserialize)]
struct ChartLock {
    /// Locked dependency list. `Chart.lock` schema wraps this in a
    /// `dependencies:` top-level key.
    #[serde(default)]
    dependencies: Vec<ChartDep>,
    /// SHA-256 digest of the `dependencies:` block, used by Helm for
    /// integrity checks. Captured but not verified in m188.
    #[serde(default)]
    digest: Option<String>,
    /// ISO-8601 timestamp of lock creation. Captured but not gating.
    #[serde(default)]
    generated: Option<String>,
}
```

### 1.4 `ImageRef`

The extracted image reference from a template file, before PURL construction.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ImageRef {
    /// The exact string extracted from the template (post-regex, pre-
    /// PURL normalization). E.g., `nginx:1.27.0`, `nginx@sha256:...`,
    /// `{{ .Values.image.repository }}:{{ .Values.image.tag }}`.
    raw: String,
    /// Classification driven by the ref's shape — determines PURL type
    /// per research.md Decision 2.
    kind: ImageRefKind,
    /// List of `templates/*.yaml` OR `crds/*.yaml` files the ref
    /// appeared in (unique — populated during dedup at emission time
    /// per FR-009). Relative to the chart directory root.
    source_paths: Vec<String>,
}
```

### 1.5 `ImageRefKind`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ImageRefKind {
    /// Ref contains a `sha256:<hex>` digest — emitted as `pkg:oci/...`.
    Digested {
        image: String,   // e.g., "nginx" or "ghcr.io/foo/bar"
        digest: String,  // e.g., "sha256:abc123..."
    },
    /// Ref contains a `:<tag>` version marker — emitted as
    /// `pkg:docker/...`. Docker Hub's `library/` prefix added for
    /// unqualified refs (e.g., `nginx:1.27.0` → `docker/library/nginx`).
    Tagged {
        image: String,   // e.g., "nginx", "library/nginx", "ghcr.io/foo/bar"
        tag: String,     // e.g., "1.27.0", "latest"
    },
    /// Ref contains one or more `{{ ... }}` blocks — emitted as
    /// `pkg:generic/<placeholder-slug>` with `mikebom:image-ref-unresolved
    /// = "true"` property.
    TemplatePlaceholder {
        /// URL-safe slug with `{{ ... }}` blocks replaced by
        /// `__PLACEHOLDER_N__` tokens per research.md Decision 2.
        slug: String,
    },
}
```

### 1.6 `HelmRenderMode`

Controls the US3 dispatch.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelmRenderMode {
    /// Default — no `helm` binary invocation, unrendered US2 extraction.
    Off,
    /// Operator passed `--helm-render`. Attempt to shell out to `helm
    /// template`; fall back to US2 on failure per FR-012.
    OptIn,
}
```

### 1.7 `HelmParseError`

```rust
#[derive(Debug, thiserror::Error)]
enum HelmParseError {
    /// `Chart.yaml` couldn't be read from disk.
    #[error("failed to read Chart.yaml at {path}: {source}")]
    ChartYamlRead { path: String, source: std::io::Error },
    /// `Chart.yaml` deserialization failed (malformed YAML, missing
    /// required fields).
    #[error("failed to parse Chart.yaml at {path}: {source}")]
    ChartYamlParse { path: String, source: serde_yaml::Error },
    /// `Chart.lock` present but couldn't be parsed. Falls through to
    /// Chart.yaml-only emission with a WARN log.
    #[error("failed to parse Chart.lock at {path}: {source}")]
    ChartLockParse { path: String, source: serde_yaml::Error },
    /// A `charts/<subchart>.tgz` tarball couldn't be extracted or
    /// contains no valid `Chart.yaml`. Falls through — parent chart
    /// still emits, only the specific subchart is skipped with WARN.
    #[error("failed to process subchart tarball at {path}: {reason}")]
    SubchartTarballFailed { path: String, reason: String },
    /// The `--helm-chart <path>.tgz` tarball input couldn't be
    /// extracted or contains no Chart.yaml at the extracted root.
    /// FATAL — mikebom exits non-zero.
    #[error("--helm-chart tarball {path} extraction failed: {reason}")]
    HelmChartTarballInvalid { path: String, reason: String },
}
```

## 2. New CLI flags on `ScanArgs`

**File**: `mikebom-cli/src/cli/scan_cmd.rs`

```rust
/// Milestone 188 (#455) — Helm chart tarball input.
///
/// When `<path>` ends in `.tgz`, mikebom extracts the tarball to a
/// tempdir + runs the scan pipeline against the extracted contents.
/// When `<path>` is a directory, behavior is identical to `--path`
/// (convenience alias per Clarifications Q1). The `.tgz` MUST
/// contain a `Chart.yaml` at the top-level extracted directory;
/// otherwise mikebom exits non-zero per FR-017.
///
/// Composes freely with all other package-DB readers — the chart's
/// contents are scanned by every applicable reader (npm if
/// `package.json` present, etc.) alongside the helm extraction.
#[arg(long = "helm-chart", value_name = "PATH_OR_TGZ")]
pub helm_chart: Option<PathBuf>,

/// Milestone 188 (#455) — opt-in Helm template rendering.
///
/// When set, mikebom shells out to `helm template <chart-dir>` before
/// image-ref extraction, resolving every `{{ .Values.image.tag }}`
/// placeholder to its concrete value. Requires the `helm` binary on
/// `$PATH`. On failure (missing binary, non-zero exit, timeout),
/// mikebom emits a WARN log and falls back to the default
/// unrendered extraction — the scan does NOT abort.
///
/// Timeout: 60 seconds by default; override via
/// `MIKEBOM_HELM_RENDER_TIMEOUT_SECS=<n>` env var.
///
/// Default (flag omitted): NO helm-binary invocation. Zero
/// external-tool calls per FR-013.
#[arg(long = "helm-render", default_value_t = false)]
pub helm_render: bool,
```

## 3. Extraction pipeline — `HelmComponent`

The intermediate type returned by helm.rs before conversion to `PackageDbEntry` (mikebom's canonical component representation).

```rust
#[derive(Debug)]
enum HelmComponent {
    /// The chart itself — Chart.yaml top-level entry.
    Chart {
        name: String,
        version: String,
        r#type: String,        // "application" or "library"
        app_version: Option<String>,
        source_file: String,   // "<chart-root>/Chart.yaml"
    },
    /// A declared chart dep (from Chart.yaml) or locked chart dep
    /// (from Chart.lock — takes precedence per FR-004).
    ChartDep {
        name: String,
        version: String,
        repo: String,          // URL host or `@`-prefixed alias
        source_kind: ChartDepSource,
        alias: Option<String>,
        condition: Option<String>,
    },
    /// Image reference extracted from a template file.
    Image {
        image_ref: ImageRef,
    },
}

#[derive(Debug, Clone, Copy)]
enum ChartDepSource {
    ChartYaml,           // FR-003
    ChartLock,           // FR-004 (authoritative when both present)
    ChartsTarball,       // FR-005 (recursive from charts/*.tgz)
}
```

## 4. Dispatch matrix (`read_all` → `helm::read` wire-up)

**File**: `mikebom-cli/src/scan_fs/package_db/mod.rs` (near the existing dpkg/npm/cargo callsites)

```rust
// Milestone 188 (#455) — Helm chart reader. Runs alongside all other
// package-DB readers (composability preserved per Clarifications Q1).
// Auto-detects <rootfs>/Chart.yaml presence per research.md Decision 1.
let helm_entries = helm::read(
    rootfs,
    helm_render_mode,  // HelmRenderMode threaded from ScanArgs
).unwrap_or_else(|e| {
    tracing::warn!(error = %e, "helm reader failed; continuing without helm components");
    Vec::new()
});
entries.extend(helm_entries);
```

## 5. PURL construction (per component kind)

| `HelmComponent` variant | Emitted PURL | Emitted `mikebom:evidence-kind` |
|---|---|---|
| `Chart { name, version, r#type: "application", .. }` | `pkg:helm/local/<name>@<version>` (repo=`local` marker) | `helm-chart-yaml` |
| `Chart { name, version, r#type: "library", .. }` | Same as application; `?type=library` qualifier | `helm-chart-yaml` |
| `ChartDep { source_kind: ChartYaml, name, version, repo, alias, .. }` | `pkg:helm/<alias-or-repo-host>/<name>@<version>` | `helm-chart-yaml` |
| `ChartDep { source_kind: ChartLock, .. }` | Same as above + `mikebom:helm-lock-authoritative = "true"` property | `helm-chart-lock` |
| `ChartDep { source_kind: ChartsTarball, .. }` | `pkg:helm/<repo>/<name>@<version>` + `mikebom:source-mechanism = "helm-charts-tgz"` property | `helm-chart-yaml` (from inside the tarball) |
| `Image { ImageRef { kind: Digested { image, digest }, .. } }` | `pkg:oci/<image>@<digest>` | `helm-template-image-ref` |
| `Image { ImageRef { kind: Tagged { image, tag }, .. } }` | `pkg:docker/<image>@<tag>` (with `library/` prefix if unqualified) | `helm-template-image-ref` |
| `Image { ImageRef { kind: TemplatePlaceholder { slug }, .. } }` | `pkg:generic/<slug>` + `mikebom:image-ref-unresolved = "true"` + `mikebom:image-ref-raw = "<original>"` | `helm-template-image-ref` |

## 6. Property matrix (`mikebom:*` annotations)

| Property | Emitted on | Values | Source of truth |
|---|---|---|---|
| `mikebom:evidence-kind` | every helm component | `"helm-chart-yaml"`, `"helm-chart-lock"`, `"helm-template-image-ref"` | plan.md §3 |
| `mikebom:helm-lock-authoritative` | ChartDep from Chart.lock | `"true"` | FR-004 |
| `mikebom:source-mechanism` | ChartDep from charts/*.tgz | `"helm-charts-tgz"` | FR-005 |
| `mikebom:image-ref-unresolved` | Image with TemplatePlaceholder kind | `"true"` | FR-008 |
| `mikebom:image-ref-raw` | Image with TemplatePlaceholder kind | verbatim raw string | FR-008 |
| `mikebom:image-extraction-completeness` | document-scope (top-level metadata for CDX; document-scope `Annotation` for SPDX 2.3 + SPDX 3) | `"partial"` or `"full"` | FR-015, plumbed via `ScanDiagnostics.helm_extraction_mode` per T023 + T023a/b/c |

## 7. Test contract

### 7.1 Unit tests (colocated with `helm.rs`)

- `chart_yaml_parses_minimal_shape` — happy path Chart.yaml
- `chart_yaml_parses_full_shape` — includes dependencies, appVersion, keywords, maintainers
- `chart_lock_takes_precedence_over_chart_yaml` — FR-004 verification
- `chart_dep_with_url_repo_produces_correct_purl` — `https://charts.bitnami.com/bitnami` → `pkg:helm/charts.bitnami.com/nginx@13.0.0`
- `chart_dep_with_alias_repo_uses_alias` — `@bitnami` → `pkg:helm/@bitnami/nginx@13.0.0`
- `chart_dep_with_no_repo_falls_back_to_generic` — WARN + `pkg:generic/<name>@<version>`
- `image_ref_regex_extracts_tagged` — `image: nginx:1.27.0` → Tagged
- `image_ref_regex_extracts_digested` — `image: nginx@sha256:...` → Digested
- `image_ref_regex_extracts_placeholder` — `image: "{{ .Values.image }}"` → TemplatePlaceholder
- `image_ref_regex_extracts_mixed` — `image: "reg.io/{{ .Values.name }}:v1.2.3"` → TemplatePlaceholder (any placeholder taints)
- `image_ref_regex_handles_quoted_and_unquoted` — both `image: nginx:1.27.0` and `image: "nginx:1.27.0"` extract
- `image_ref_regex_handles_comments` — `image: nginx:1.27.0  # deploy target` extracts `nginx:1.27.0`
- `library_prefix_added_for_dockerhub_unqualified` — `nginx:1.27.0` → `pkg:docker/library/nginx@1.27.0`
- `library_prefix_not_added_for_registry_prefixed` — `ghcr.io/foo/bar:v1` → `pkg:docker/ghcr.io/foo/bar@v1`
- `dedup_collapses_same_ref_from_multiple_templates` — same image in `deployment.yaml` + `job.yaml` → 1 component + `evidence.occurrences[]` with 2 paths
- `helm_render_mode_off_never_invokes_helm` — FR-013 verification (mock via subprocess-tracing test)

### 7.2 Integration tests (`mikebom-cli/tests/helm_reader.rs`)

- `us1_chart_yaml_only_produces_expected_components` — synthesize `Chart.yaml` with 3 deps, scan, assert 4 helm components
- `us1_chart_lock_overrides_chart_yaml_versions` — declare `1.0.0` in Chart.yaml + lock to `1.0.5` in Chart.lock; assert emitted PURLs use `@1.0.5`
- `us1_charts_tgz_subchart_deps_emit_recursively` — synthesize a Chart with a `charts/subchart-1.0.0.tgz` that itself has 2 deps; assert 4 components emit (parent chart + subchart + 2 subsubdeps)
- `us1_helm_chart_flag_with_tarball_extracts_and_scans` — synthesize a `.tgz` chart tarball; invoke via `--helm-chart <path>.tgz`; assert identical output to `--path <extracted-dir>`
- `us1_helm_chart_flag_with_invalid_tarball_exits_nonzero` — tarball without Chart.yaml at root → non-zero exit + actionable stderr per FR-017
- `us1_composability_with_npm_reader` — a chart directory containing `package.json` produces BOTH helm components AND npm components (composability preserved per Clarifications Q1)
- `us2_templated_image_ref_emits_unresolved_property` — synthesize template with `image: "{{ .Values.image }}"`; assert emitted component has `mikebom:image-ref-unresolved = "true"` + `mikebom:image-ref-raw = "{{ .Values.image }}"`
- `us2_concrete_image_refs_emit_normal_purl` — template with `image: nginx:1.27.0` → `pkg:docker/library/nginx@1.27.0` + NO unresolved property
- `us2_crds_yaml_scanned_alongside_templates` — CRD file with `image:` → extracted per FR-010
- `us2_yaml_parse_break_falls_back_to_regex` — template that opens `{{ if .Values.enabled }}` block spanning multiple YAML docs; assert image extracted + WARN log
- `us2_document_scope_completeness_partial_annotation_present` — assert `mikebom:image-extraction-completeness = "partial"` on doc scope
- `us3_helm_render_success_produces_no_unresolved_markers` — REQUIRES helm binary; gate behind `MIKEBOM_HELM_INTEGRATION=1` env-var; assert 100% resolved refs
- `us3_helm_render_missing_binary_falls_back` — helm binary absent → WARN + US2 fallback + scan succeeds
- `us3_helm_render_timeout_falls_back` — stub `helm` binary that sleeps 65s (default timeout 60s); assert timeout WARN + US2 fallback + scan succeeds
- `us3_helm_render_env_var_timeout_override` — `MIKEBOM_HELM_RENDER_TIMEOUT_SECS=5`; assert 5s timeout applied
- `default_scan_without_chart_yaml_is_byte_identical` — scan a non-Helm directory; assert output byte-identical to pre-m188 (FR-016 gate)

## 8. Backward compatibility

- **Pre-m188 golden fixtures** — none contain Helm charts, so byte-identity is preserved on every existing fixture (FR-016 / SC-005).
- **CLI additions** — `--helm-chart` + `--helm-render` are both NEW flags. Pre-m188 invocations continue to work identically.
- **`read_all` dispatcher signature** — extended with a new `helm_render_mode: HelmRenderMode` parameter; call sites updated in `scan_cmd.rs`. Since `read_all` is `pub`-scoped inside the crate but not part of the public workspace API, this is a source-compatible change.
- **New properties + PURL types** — additive on helm-scanned targets only. Consumers ignoring unknown `mikebom:*` properties see no behavior change.

## 9. Constitution alignment recap

- Two new `mikebom:*` properties (`mikebom:image-ref-unresolved`, `mikebom:image-extraction-completeness`) — required per Principle V native-field audit result documented in `docs/reference/sbom-format-mapping.md` §Milestone 188 addendum.
- All other properties reuse existing `mikebom:evidence-kind`, `mikebom:source-mechanism`, `mikebom:helm-lock-authoritative` axes established by prior milestones.
