# Contract: `split-manifest.json` wire schema

**Feature**: 215-sbom-auto-split
**Kind**: Operator-facing JSON artifact schema
**Consumers**: downstream tools that need to reason about a split-scan output as a whole (route each sub-SBOM to its owner service, dedup shared deps client-side, cross-reference to a portfolio SBOM registry, etc.).

## Schema URL + version

Every emitted manifest carries:

```json
"$schema": "https://waybill.dev/schema/split-manifest/v1.json"
```

`v1` is pinned. Schema changes that break existing v1 consumers bump to `v2`. Both versions coexist during any deprecation window; per-manifest `$schema` value tells consumers which shape to parse.

## Full example

```json
{
  "$schema": "https://waybill.dev/schema/split-manifest/v1.json",
  "waybill_version": "0.1.0-alpha.66",
  "scan_root": "/Users/mike/Projects/monorepo-example",
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
        "cyclonedx-json": "libsafe.cargo.cdx.json",
        "spdx-2.3-json": "libsafe.cargo.spdx.json",
        "spdx-3-json": "libsafe.cargo.spdx3.json"
      }
    },
    {
      "subproject_id": "frontend.npm",
      "root_purl": "pkg:npm/frontend@1.0.0",
      "source_dir": "apps/frontend",
      "component_count": 342,
      "shared_deps_count": 5,
      "files": {
        "cyclonedx-json": "frontend.npm.cdx.json",
        "spdx-2.3-json": "frontend.npm.spdx.json",
        "spdx-3-json": "frontend.npm.spdx3.json"
      }
    }
  ]
}
```

## Field contract

### Top-level

| Field | Type | Required | Description |
|---|---|---|---|
| `$schema` | string | yes | Stable schema-version URL. Consumers key on this to select parser version. Values: `https://waybill.dev/schema/split-manifest/v1.json` (only v1 shipped). |
| `waybill_version` | string | yes | Which Waybill release emitted this manifest. Aids reproducibility + debugging. Value from `env!("CARGO_PKG_VERSION")` at compile time. |
| `scan_root` | string | yes | The `--path` argument passed to the scan. Absolute path OR repo-relative (whichever the operator specified). Under `WAYBILL_FIXED_TIMESTAMP` reproducibility mode, path normalized to `<WORKSPACE>` placeholder if it matches the CWD prefix (matches golden-test normalization). |
| `generated_at` | string | yes | RFC 3339 timestamp of scan start. Under `WAYBILL_FIXED_TIMESTAMP`, equals the fixed value. |
| `total_unique_components` | integer (u64) | yes | Distinct-PURL count across ALL sub-SBOMs. Equals the pre-feature single-SBOM component count when `--split` were absent. Diagnostic for SC-004 sanity. |
| `shared_dep_count` | integer (u64) | yes | Distinct-PURL count of components appearing in ≥ 2 sub-SBOMs. Diagnostic; downstream tools MAY use for client-side dedup. |
| `entries` | array | yes | One entry per detected subproject. Sorted lexicographically by `subproject_id` for deterministic output. |

### `entries[]` element

| Field | Type | Required | Description |
|---|---|---|---|
| `subproject_id` | string | yes | Stable operator-visible primary key. Format: `<slug>.<ecosystem>`. Matches the sub-SBOM filename prefix. Deterministic function of the subproject's PURL. |
| `root_purl` | string | yes | Full PURL of the subproject's root component. Format per PURL spec (pkg:cargo/name@version, pkg:npm/name@version, pkg:pypi/name@version, etc.). |
| `source_dir` | string | yes | Subproject source directory relative to `scan_root`. Empty string if the subproject IS the scan root (rare). |
| `component_count` | integer (u64) | yes | Number of components in this sub-SBOM. Includes root. |
| `shared_deps_count` | integer (u64) | yes | Number of THIS sub-SBOM's components that also appear in ≥ 1 sibling sub-SBOM. Distinct from the top-level `shared_dep_count` (which is a repository-wide distinct count). |
| `files` | object (map) | yes | Map of format-name → relative-filename. Keys drawn from `{"cyclonedx-json", "spdx-2.3-json", "spdx-3-json"}`. At least one entry required. Filenames relative to the manifest's containing directory. |

## Determinism contract

Under `WAYBILL_FIXED_TIMESTAMP`, TWO successive scans of the SAME input tree MUST produce byte-identical `split-manifest.json`:

- `entries` sorted lexicographically by `subproject_id`.
- `files` map keys sorted alphabetically (`BTreeMap` serialization).
- Numeric fields exact.
- No fresh UUIDs or randomness in the manifest itself.
- `generated_at` matches the fixed timestamp.

The manifest is deterministic per-scan-input. Two different monorepos produce different manifests; two runs on the same monorepo produce identical manifests.

## Naming contract for sub-SBOM filenames

Per R3 in research.md:
- `<slug>.<ecosystem>.<format-ext>.json`
- `<slug>` = PURL name field with `/` → `-`, `@` → `at-`, other unsafe chars stripped.
- `<ecosystem>` = PURL type (`cargo`, `npm`, `pypi`, `maven`, `go`, `gem`, `swift`, `generic`).
- `<format-ext>` = `cdx` | `spdx` | `spdx3`.
- Collision fallback: append `-<8-hex-chars-of-sha256(source_dir)>` before the ecosystem segment.

The manifest's `files` map values MUST be filesystem-safe on Linux, macOS, and Windows. Chars stripped: `\/:*?"<>|`.

## Validation

Downstream consumers can validate a manifest via JSON schema at the `$schema` URL. Waybill ships a validation test at `waybill-cli/tests/split_manifest_schema.rs` that:
1. Emits a manifest from a known fixture.
2. Loads the schema (pinned in-repo at `waybill-cli/contracts/split-manifest-v1.schema.json`).
3. Validates the emitted manifest passes the schema.
4. Ensures no drift between the code's emit shape and the schema.

## Compatibility with existing operator surface

- Manifest does NOT overlap with any existing Waybill emit format. Distinct from CDX / SPDX SBOMs.
- Manifest is NOT a required prerequisite for consuming sub-SBOMs — each sub-SBOM is self-contained and independently valid. Manifest is a convenience for tools that need to reason about the emitted set as a whole.
- Absence of a manifest on disk (e.g., operator deleted it) does not invalidate the sub-SBOMs. Manifest is purely additive metadata.

## Contract stability going forward

- **Field additions** to v1 are non-breaking if optional. New fields MUST be `#[serde(default)]` on the Rust side and MUST be documented in `docs/user-guide/split-manifest.md` at addition time.
- **Field removals** or renames are breaking → v2 bump.
- **Sort order** of `entries[]` is stable at lex-by-`subproject_id`. Changing sort order would break byte-identical determinism tests.
- **Schema URL** stability: `v1` remains resolvable indefinitely. Consumers pinned to v1 continue to work.
