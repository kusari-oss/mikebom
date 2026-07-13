# Quickstart: Helm chart scanning (m188)

**Feature**: [spec.md](./spec.md) · **Plan**: [plan.md](./plan.md) · **Contracts**: [cli-flags.md](./contracts/cli-flags.md) · [extraction-pipeline.md](./contracts/extraction-pipeline.md)

## Operator worked examples

### Example 1 — Scan a Helm chart directory (auto-detect)

**Use case**: You have a chart at `~/charts/my-app/` containing `Chart.yaml` + `templates/` + `charts/*.tgz`. You want mikebom to emit chart deps + image refs.

```bash
mikebom sbom scan --path ~/charts/my-app --format cyclonedx-json --output ~/my-app.cdx.json
```

**Expected outcomes**:
- Chart.yaml auto-detected — one `pkg:helm/local/my-app@<version>` component (the chart itself).
- Each `dependencies[]` entry emits `pkg:helm/<repo>/<name>@<version>`.
- `charts/*.tgz` subcharts recursively parsed — their own deps also emit.
- Template images extracted from `templates/**/*.yaml` — tagged refs as `pkg:docker/library/<name>@<tag>` (Docker Hub unqualified) or `pkg:docker/<registry>/<name>@<tag>` (qualified).
- Templated image refs (`{{ .Values.image.tag }}`) emit as `pkg:generic/<placeholder-slug>` with `mikebom:image-ref-unresolved = "true"`.
- Document-scope annotation: `mikebom:image-extraction-completeness = "partial"` (unrendered mode default).
- Zero external-binary calls.
- Other package-DB readers (npm / cargo / etc.) run alongside if their manifests are present.

### Example 2 — Scan a chart tarball (`.tgz`)

**Use case**: You've downloaded a chart via `helm pull ...` and have `my-app-1.2.3.tgz` locally.

```bash
mikebom sbom scan --helm-chart ~/downloads/my-app-1.2.3.tgz \
    --format cyclonedx-json --output ~/my-app.cdx.json
```

**Expected outcomes**:
- mikebom extracts the `.tgz` to a tempdir (auto-cleanup on exit).
- The extracted directory contains `my-app/Chart.yaml`, `my-app/templates/`, etc.
- Same emission as Example 1.
- Tempdir cleaned up after emission (no disk residue).

### Example 3 — Rendered extraction with `--helm-render`

**Use case**: You have `helm` on `$PATH` and want maximum-fidelity image-ref extraction (all `{{ .Values.image.tag }}` placeholders resolved).

```bash
mikebom sbom scan --path ~/charts/my-app --helm-render \
    --format cyclonedx-json --output ~/my-app.cdx.json
```

**Expected outcomes**:
- mikebom shells out to `helm template ~/charts/my-app` (60s timeout by default).
- Rendered YAML fed into the same image-ref extractor.
- Zero `mikebom:image-ref-unresolved = "true"` properties on emitted components (all resolved).
- Document-scope annotation: `mikebom:image-extraction-completeness = "full"`.
- If `helm` is missing OR the shell-out fails, mikebom WARN-logs the reason and falls back to unrendered extraction. The scan still succeeds.

### Example 4 — Compose with other package-DB readers

**Use case**: Your chart directory ALSO contains a `package.json` (some charts vendor a small nodejs helper). You want BOTH helm and npm components emitted.

```bash
mikebom sbom scan --path ~/charts/my-app \
    --format cyclonedx-json --output ~/my-app.cdx.json
```

**Expected outcomes**:
- Auto-detects Chart.yaml AND `package.json`. Both readers run.
- Emitted SBOM contains helm components + npm components (composability preserved per Clarifications Q1).
- Each set of components carries the reader-appropriate `mikebom:evidence-kind`.

### Example 5 — Override the `--helm-render` timeout

**Use case**: Your umbrella chart with 15 subcharts takes ~90 seconds to render.

```bash
MIKEBOM_HELM_RENDER_TIMEOUT_SECS=180 \
mikebom sbom scan --path ~/charts/mega-chart --helm-render \
    --format cyclonedx-json --output ~/mega-chart.cdx.json
```

**Expected outcomes**:
- Timeout raised to 180 seconds for this invocation.
- If `helm template` completes within 180s, `full` completeness annotation emitted.
- If it exceeds 180s, timeout WARN + fallback to unrendered extraction.

## Developer worked example (contributor flow)

### Adding a new image-field extraction path

The default `helm.rs` regex catches the standard Kubernetes `image: <ref>` field. If a future milestone needs to also extract non-standard image fields (`spec.image`, `containerImage:`, etc.):

1. Extend the regex constant in `helm.rs`:
   ```rust
   static IMAGE_REGEX: OnceLock<Regex> = OnceLock::new();
   IMAGE_REGEX.get_or_init(|| {
       Regex::new(r#"(?m)^(\s*)(?:image|containerImage|spec\.image):\s*..."#).unwrap()
   })
   ```
2. Add unit tests to `helm.rs::tests::image_ref_regex_extracts_<new-field-name>`.
3. Add integration coverage in `mikebom-cli/tests/helm_reader.rs`.

### Adding a new `Chart.yaml` field to emission

Currently the reader captures `name`, `version`, `type`, `appVersion`, and `dependencies`. To also emit `keywords[]` as a `mikebom:helm-keywords` annotation:

1. Extend `ChartMetadata` to include the new field (already deserialized).
2. In `helm::build_entry_from_control_metadata` (or equivalent), add the annotation to `extra_annotations`.
3. Update `docs/reference/sbom-format-mapping.md` §Milestone 188 addendum with a native-field audit for the new property per Constitution Principle V.
4. Add a unit test.

### Running the m188 tests locally

```bash
# Unit tests only (fast; ~1s):
cargo +stable test -p mikebom --bin mikebom scan_fs::package_db::helm

# Integration tests (~10s):
cargo +stable test -p mikebom --test helm_reader

# Integration tests including US3 (REQUIRES helm binary):
MIKEBOM_HELM_INTEGRATION=1 cargo +stable test -p mikebom --test helm_reader

# Full pre-PR gate:
./scripts/pre-pr.sh
```

### Verification checklist for merge

Before opening a PR:
- [ ] `cargo +stable clippy --workspace --all-targets -- -D warnings` — zero warnings.
- [ ] `cargo +stable test --workspace --no-fail-fast` — every suite passes with `0 failed`.
- [ ] `./scripts/pre-pr.sh` runs to green.
- [ ] Zero drift in `cargo tree --workspace` output (SC-007 zero-new-deps gate).
- [ ] Existing golden fixtures produce byte-identical output (FR-016 / SC-005; no fixture contains Chart.yaml, so this is preserved by construction; still verify explicitly).
- [ ] Walker-audit CI gate passes (m115 — helm.rs's file-enumeration should use `safe_walk`, not raw `walkdir`, per m114 convention).

## FAQ

**Q: Does m188 verify GPG signatures on `.tgz` chart provenance files?**
A: No — signature verification is deferred per spec.md §Deferred. m188 processes the tarball's YAML content without cryptographic verification.

**Q: My chart uses `--set image.tag=v2.0.0`-style overrides. Does mikebom respect those?**
A: Only via `--helm-render` (US3) which passes them through to `helm template`. In default unrendered mode, mikebom sees the raw `{{ .Values.image.tag }}` placeholders and emits them as `TemplatePlaceholder` refs. If you want to bake `--set` overrides into the scan, use `--helm-render` and configure a `values.yaml` OR use `helm template --set image.tag=v2.0.0 ...` pre-rendering + `mikebom sbom scan --path <rendered-output-dir>` on the rendered result.

**Q: Can I disable auto-detection if I don't want helm components?**
A: There's no explicit disable flag in m188. If auto-detection is causing noise (unlikely — helm charts are structurally distinctive), you can either (a) scan a subdirectory that doesn't include `Chart.yaml`, or (b) file an issue requesting a `--no-helm` opt-out for a follow-up milestone.

**Q: What happens if my chart has NO `dependencies[]` entries but does have `templates/`?**
A: One `HelmComponent::Chart` component (for the chart itself) + N `HelmComponent::Image` components (one per unique image ref). No `HelmComponent::ChartDep` emissions.

**Q: What if a subchart tarball is corrupted?**
A: WARN log naming the corrupted tarball; parent chart + sibling subcharts still emit normally. Zero silent drops per Constitution Principle III.

**Q: Does `--helm-render` need a `values.yaml`?**
A: mikebom passes `helm template <chart-dir>` verbatim. If `<chart-dir>/values.yaml` exists, `helm template` uses it automatically (Helm convention). If not, `helm template` uses the chart's built-in defaults.

**Q: Can I pass `--helm-render` when scanning a chart tarball via `--helm-chart <path>.tgz`?**
A: Yes — the tempdir extraction happens BEFORE the `--helm-render` shell-out, so `helm template <tempdir>` sees a normal chart directory. Both work together.
