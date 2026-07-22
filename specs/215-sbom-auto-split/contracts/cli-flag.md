# Contract: `--split` CLI flag

**Feature**: 215-sbom-auto-split
**Kind**: CLI-surface contract (`waybill sbom scan --split`)
**Consumers**: operators + CI scripts invoking `waybill sbom scan`.

## Flag definition

```rust
#[arg(long, help = "Emit one SBOM per detected workspace member instead of one combined SBOM. \
    Requires --output-dir; incompatible with --output.")]
pub split: bool,
```

- **Name**: `--split` (bare word; canonical per Clarification 2026-07-22 Q3).
- **Type**: boolean flag (no value; presence = enabled).
- **Default**: `false` (opt-in only; pre-feature behavior unchanged).
- **Short form**: NONE. No `-s` alias — `-s` risks colliding with other flags in a growing CLI surface.

## Interaction matrix

| `--split` | `--output <file>`  | `--output-dir <dir>` | Behavior |
|:---------:|:------------------:|:--------------------:|----------|
| unset     | unset              | unset                | Pre-feature default: emit ONE SBOM to stdout (single-file) |
| unset     | set                | unset                | Pre-feature: emit ONE SBOM to `<file>` |
| unset     | unset              | set                  | Pre-feature: emit ONE SBOM to `<dir>/<autogen-name>.<format>.json` |
| unset     | set                | set                  | Pre-feature: `--output-dir` takes precedence (existing behavior) |
| set       | unset              | unset                | Auto-generate default `<output-dir>=./waybill-split-<YYYYMMDDTHHMMSS>/`; emit N × M sub-SBOMs + 1 manifest |
| set       | unset              | set                  | Emit N × M sub-SBOMs + 1 manifest into `<dir>` |
| set       | set                | unset                | **HARD ERROR at CLI parse** — see below |
| set       | set                | set                  | **HARD ERROR at CLI parse** — see below |

## Error message on `--split` + `--output`

```
error: `--split` is incompatible with `--output <file>` — a single file
       cannot hold N sub-SBOMs. Use `--output-dir <dir>` instead;
       Waybill will emit N sub-SBOM files + a split-manifest.json into <dir>.

       Example:
         waybill sbom scan --path . --split --output-dir ./sboms/
```

Emitted at CLI-parse time (before any scanning). Exit code 2 (clap's standard usage-error exit).

## Help-text contract

`waybill sbom scan --help` MUST include the `--split` flag with the description shown above. Longer description in `--help-long` or a `docs/user-guide/cli-reference.md#split` reference explains:
- One SBOM per detected workspace member (Cargo workspaces, npm workspaces, Go workspaces, Maven multi-module, gradle sub-projects, pyproject dirs, gem, etc.)
- Reuses the same workspace-detection layer used to identify main-module components in single-SBOM mode
- Shared transitive deps duplicate across sub-SBOMs (each SBOM is self-contained)
- A `split-manifest.json` sibling file describes the split
- Zero-boundary case (single-package project): falls back to ONE SBOM + WARN log

## Format-multiplication rule

If `--format` is passed multiple times:

```
waybill sbom scan --path . --split \
    --format cyclonedx-json --format spdx-2.3-json --format spdx-3-json
```

Split-mode emits N × M files (N subprojects × M formats). Manifest lists all under each `entries[].files{}` map keyed by format name (`cyclonedx-json`, `spdx-2.3-json`, `spdx-3-json`).

Single `--format` value → N × 1 = N sub-SBOM files.

## Deterministic-output-name-generation rule (default `--output-dir`)

When `--split` is set with neither `--output` nor `--output-dir`, the CLI generates a default output dir:

```
./waybill-split-<YYYYMMDDTHHMMSS>/
```

The timestamp is the trace start time in UTC. Under `WAYBILL_FIXED_TIMESTAMP`, the timestamp portion becomes the fixed value (RFC 3339 → compact form; e.g., `2026-07-22T14:00:00Z` → `20260722T140000Z`). Directory is created if missing.

## Contract stability

- Flag name (`--split`) is a public-CLI-surface contract. Renaming would be a breaking change; requires a MAJOR version bump per the constitution's amendment procedure.
- Interaction matrix rules are stable. Changing an "unset+unset" cell (e.g., adding a new default output shape) is a breaking change.
- The HARD ERROR on `--split + --output` is intentional; softening to a warning (with silent data loss) is not permitted.
