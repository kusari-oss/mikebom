# Contract: `--helm-chart` + `--helm-render` CLI flags (m188)

**Feature**: [../spec.md](../spec.md) · **Plan**: [../plan.md](../plan.md) · **Data model**: [../data-model.md](../data-model.md)

## Flag surface

Two new flags on `mikebom sbom scan`. Zero conflict with existing flags.

### `--helm-chart <PATH_OR_TGZ>`

```
--helm-chart <PATH_OR_TGZ>          PathBuf, optional
                                    Default: not set
```

Clap attribute (per data-model.md §2):

```rust
#[arg(long = "helm-chart", value_name = "PATH_OR_TGZ")]
pub helm_chart: Option<PathBuf>,
```

**Help text**:
```
--helm-chart <PATH_OR_TGZ>
    Helm chart tarball or directory to scan. When PATH_OR_TGZ ends
    in `.tgz`, mikebom extracts the tarball to a tempdir and runs
    the scan pipeline against the extracted contents. When it is a
    directory, behavior is identical to `--path <PATH_OR_TGZ>` —
    Chart.yaml is auto-detected regardless.

    The `.tgz` MUST contain a `Chart.yaml` at the top-level extracted
    directory; otherwise mikebom exits non-zero. Directory inputs
    without Chart.yaml don't cause an error — other package-DB readers
    (npm / cargo / etc.) still run.

    Composes freely with all other package-DB readers — the chart's
    contents are scanned by every applicable reader alongside the
    helm extraction.
```

### `--helm-render`

```
--helm-render                       bool flag, optional
                                    Default: false
```

Clap attribute (per data-model.md §2):

```rust
#[arg(long = "helm-render", default_value_t = false)]
pub helm_render: bool,
```

**Help text**:
```
--helm-render
    Opt-in Helm template rendering. When set, mikebom shells out to
    `helm template <chart-dir>` before extracting container image
    references, resolving every `{{ .Values.image.tag }}` placeholder
    to a concrete value. Requires the `helm` binary on `$PATH`.

    On failure (missing binary, non-zero exit, timeout), mikebom emits
    a WARN log and falls back to the default unrendered extraction —
    the scan does NOT abort. The emitted SBOM's document-scope
    `mikebom:image-extraction-completeness` annotation surfaces
    whether extraction was "partial" (fallback) or "full" (helm
    succeeded).

    Timeout: 60 seconds by default; override via
    `MIKEBOM_HELM_RENDER_TIMEOUT_SECS=<n>` env var.

    Default (flag omitted): NO helm binary invocation. Zero external-
    tool calls.
```

## `--helm-chart` × input-type × `--path` composability matrix

The `--helm-chart <path>` flag composes with `--path <path>` per Clarifications Q1:

| `--helm-chart` value | `--path` value | Behavior |
|---|---|---|
| Not set | `<chart-dir>` | Auto-detect Chart.yaml → emit helm components alongside other readers (composability preserved) |
| Not set | `<non-chart-dir>` | No helm components emitted; other package-DB readers run normally |
| `<chart-dir>` | Not set | Identical to `--path <chart-dir>` — auto-detect Chart.yaml + composability |
| `<chart-dir>` | `<other-dir>` | **REJECTED** — clap `conflicts_with` between `--helm-chart` and `--path` (both are top-level scan targets; one at a time) |
| `<chart-dir>` | `<chart-dir>` | **REJECTED** — same as above |
| `<path>.tgz` | Not set | Extract tarball to tempdir → scan extracted dir (helm components emit; other readers scan too) |
| `<path>.tgz` (no Chart.yaml at root) | Not set | **NON-ZERO EXIT** with actionable error per FR-017 |
| `<nonexistent-path>` | Not set | **NON-ZERO EXIT** with actionable error per FR-017 |

**Rejection error message** (FR-017 + Clarifications Q1):
```
Error: --helm-chart and --path are mutually exclusive; specify exactly one scan target.
```

## `--helm-render` × availability matrix

| `--helm-render` set? | `helm` binary on `$PATH`? | Behavior |
|---|---|---|
| No (default) | Any | US2 unrendered extraction only. Zero external-tool calls. |
| Yes | Yes | US3 rendered extraction. Emits `mikebom:image-extraction-completeness = "full"`. |
| Yes | No | WARN log naming missing binary + install hint; fall back to US2. Emits `partial`. |
| Yes | Yes, but `helm template` exits non-zero | WARN log with exit code + first 20 lines of stderr; fall back to US2. Emits `partial`. |
| Yes | Yes, but `helm template` exceeds 60s | WARN log naming timeout + env-var override hint; fall back to US2. Emits `partial`. |

## Backward compatibility guard (FR-016 / SC-005)

- Any invocation WITHOUT `--helm-chart` AND scanning a directory that does NOT contain `Chart.yaml` MUST produce output byte-identical to pre-m188 for the same input.
- Verified by:
  - Integration test `default_scan_without_chart_yaml_is_byte_identical`.
  - Existing golden fixtures — zero drift expected (no fixture contains Chart.yaml).

## Composition with existing flags

- **`--offline`**: doesn't affect helm reader (no network activity in the default flow; `--helm-render` shells out to a LOCAL binary, not network).
- **`--path`**: mutually exclusive with `--helm-chart` per matrix above.
- **`--image` / `--image-src`**: composes freely — helm charts inside an OCI image tarball are auto-detected and scanned.
- **`--exclude-path`**: composes freely — exclusion patterns apply to helm reader's walker paths just like every other reader (m113 gate).
- **`--sbom-source` (m186)**: composes freely — an image with published SBOM referrer AND a chart directory scan target both take their respective m186 / m188 paths without conflict.

## Error message templates (FR-017)

| Scenario | stderr message | Exit code |
|---|---|---|
| `--helm-chart <path>.tgz` where `<path>` doesn't exist | `Error: --helm-chart tarball <path> not found` | 1 |
| `--helm-chart <path>.tgz` where tarball extraction fails | `Error: --helm-chart tarball <path> could not be extracted: <underlying-error>` | 1 |
| `--helm-chart <path>.tgz` where tarball has no `Chart.yaml` at top-level | `Error: --helm-chart tarball <path> extracted successfully but no Chart.yaml found at top-level directory (expected <chart-name>/Chart.yaml)` | 1 |
| `--helm-chart <path>` (directory) where `<path>` doesn't exist | `Error: --helm-chart path <path> not found` | 1 |
| `--helm-chart` AND `--path` both set | `Error: --helm-chart and --path are mutually exclusive; specify exactly one scan target.` | 1 |
