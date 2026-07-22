# `split-manifest.json` — operator's guide

**Milestone 215.** Sibling artifact emitted alongside sub-SBOMs when
`waybill sbom scan --split` runs. Describes the split so downstream tooling
can reason about the emitted file set as a whole.

## When it's written

Whenever `--split` triggers the fan-out (≥ 2 workspace boundaries detected),
Waybill writes `<output-dir>/split-manifest.json` LAST — after every sub-SBOM
file has landed successfully. Its presence signals a successful split.

On the zero-boundary fallback path (0 or 1 boundaries), NO manifest is
written — nothing meaningful to describe.

## Schema

Pinned at [`waybill-cli/contracts/split-manifest-v1.schema.json`][schema]
(JSON Schema draft 2020-12). Consumers key on the top-level `$schema` URL
to select a parser version; future breaking schema changes bump to v2.

[schema]: https://github.com/kusari-oss/waybill/blob/main/waybill-cli/contracts/split-manifest-v1.schema.json

## Full example

```json
{
  "$schema": "https://waybill.dev/schema/split-manifest/v1.json",
  "waybill_version": "0.1.0-alpha.66",
  "scan_root": "/Users/mike/Projects/monorepo",
  "generated_at": "2026-07-22T14:00:00Z",
  "total_unique_components": 1247,
  "shared_dep_count": 42,
  "entries": [
    {
      "subproject_id": "libsafe.cargo",
      "root_purl": "pkg:cargo/libsafe@0.1.0",
      "source_dir": "crates/libsafe",
      "component_count": 87,
      "shared_deps_count": 12,
      "files": {
        "cyclonedx-json": "libsafe.cargo.cdx.json"
      }
    }
  ]
}
```

## Field reference

### Top-level

| Field | Type | Description |
|---|---|---|
| `$schema` | string | Schema-version URL. Consumers key on this to select parser version. |
| `waybill_version` | string | Which Waybill release emitted this manifest (from `CARGO_PKG_VERSION`). |
| `scan_root` | string | The `--path` argument the operator passed. Under `WAYBILL_FIXED_TIMESTAMP`, path normalizes to `<WORKSPACE>` for byte-identical output. |
| `generated_at` | RFC 3339 string | Scan-start timestamp. Under `WAYBILL_FIXED_TIMESTAMP` equals the fixed value. |
| `total_unique_components` | integer | Distinct-PURL count across ALL sub-SBOMs (equals the pre-feature single-SBOM component count). Diagnostic for SC-004. |
| `shared_dep_count` | integer | Distinct-PURL count of components appearing in ≥ 2 sub-SBOMs. Diagnostic — downstream tools MAY use for client-side dedup. |
| `entries[]` | array | One entry per detected subproject. Sorted lex by `subproject_id`. |

### `entries[]` element

| Field | Type | Description |
|---|---|---|
| `subproject_id` | string | Stable primary key of the form `<slug>.<ecosystem>`. Matches the sub-SBOM filename prefix. Deterministic function of the subproject's PURL. |
| `root_purl` | string | Full PURL of the subproject's root (e.g., `pkg:cargo/libsafe@0.1.0`). |
| `source_dir` | string | Subproject source directory relative to `scan_root`. Empty when the subproject IS the scan root. |
| `component_count` | integer | Number of components in this sub-SBOM (includes the root). |
| `shared_deps_count` | integer | Number of THIS sub-SBOM's components that also appear in ≥ 1 sibling. |
| `files{}` | object | Map of format-name → relative-filename. Keys: `cyclonedx-json` / `spdx-2.3-json` / `spdx-3-json` / `spdx-3-json-experimental`. At least one entry required. |

## Common jq recipes

### List every subproject + its emitted files

```bash
jq '.entries[] | {subproject_id, files}' ./sboms/split-manifest.json
```

### Find the CDX SBOM for a specific subproject

```bash
jq -r '.entries[]
       | select(.subproject_id == "libsafe.cargo")
       | .files["cyclonedx-json"]' \
    ./sboms/split-manifest.json
```

### Total emitted vs distinct component count

```bash
# Sum of every sub-SBOM's component_count (counts duplicates once per sub).
jq '[.entries[].component_count] | add' ./sboms/split-manifest.json

# Distinct component count repo-wide (no double-counting shared deps).
jq '.total_unique_components' ./sboms/split-manifest.json
```

### Highest-fan-in subprojects (most-shared)

```bash
jq '.entries
    | sort_by(-.shared_deps_count)
    | .[0:5]
    | map({subproject_id, shared_deps_count, component_count})' \
    ./sboms/split-manifest.json
```

## CI integration example

Route each sub-SBOM to a per-service vuln scanner:

```bash
#!/usr/bin/env bash
set -euo pipefail

waybill sbom scan --path . --split --output-dir ./sboms/ --format cyclonedx-json

jq -c '.entries[]' ./sboms/split-manifest.json | while read -r entry; do
    subproject_id=$(echo "$entry" | jq -r '.subproject_id')
    sbom_file=$(echo "$entry" | jq -r '.files["cyclonedx-json"]')

    echo "Scanning $subproject_id..."
    trivy sbom "./sboms/$sbom_file" \
        --format json \
        > "./scan-results/$subproject_id.json"
done
```

## Reproducibility

Under `WAYBILL_FIXED_TIMESTAMP` two successive scans of the same input tree
produce byte-identical `split-manifest.json`:

- `entries[]` sort order is lexicographic by `subproject_id` (deterministic).
- `files` map keys sorted alphabetically (`BTreeMap` serialization).
- `generated_at` matches the fixed timestamp.
- No fresh UUIDs or randomness in the manifest itself.

```bash
export WAYBILL_FIXED_TIMESTAMP="2026-01-01T00:00:00Z"
waybill sbom scan --path . --split --output-dir ./run-1/
waybill sbom scan --path . --split --output-dir ./run-2/
diff -r ./run-1/ ./run-2/   # → empty (byte-identical)
```

## Version-compatibility

- Schema is v1. All fields required at v1 will remain present in v1.
- **Field additions** to v1 are non-breaking if optional; MUST be
  `#[serde(default)]` on the Rust side.
- **Field removals** or renames are breaking → v2 bump.
- **Sort order** of `entries[]` is stable at lex-by-`subproject_id`.
- **Schema URL** `https://waybill.dev/schema/split-manifest/v1.json` remains
  resolvable indefinitely; consumers pinned to v1 continue to work.

## Cross-references

- CLI flag reference: [`--split`](cli-reference.md#--split) +
  [`--output-dir`](cli-reference.md#--output-dir-dir)
- Merge/split/edit interop guide (post-merge support lands):
  [waybill#627](https://github.com/kusari-oss/waybill/issues/627)
