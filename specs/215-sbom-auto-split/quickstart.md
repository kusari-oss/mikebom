# Quickstart: SBOM auto-split (`--split` flag)

**Feature**: 215-sbom-auto-split
**Date**: 2026-07-22

End-to-end recipe for the new `--split` flag on `waybill sbom scan`.

## The 30-second happy path

```bash
# Cargo workspace with 4 members — emits 4 sub-SBOMs + 1 manifest
waybill sbom scan --path ./my-monorepo --split --output-dir ./sboms/

ls ./sboms/
# libsafe.cargo.cdx.json
# libvuln.cargo.cdx.json
# safe-only.cargo.cdx.json
# split-manifest.json
# vuln-included.cargo.cdx.json
```

The manifest describes what was emitted:

```bash
jq '.entries[] | {subproject_id, root_purl, component_count}' ./sboms/split-manifest.json
```

## Multi-format emission

Pass `--format` multiple times to emit each subproject in all requested formats:

```bash
waybill sbom scan --path ./my-monorepo --split \
    --output-dir ./sboms/ \
    --format cyclonedx-json \
    --format spdx-2.3-json \
    --format spdx-3-json

ls ./sboms/ | head -8
# libsafe.cargo.cdx.json
# libsafe.cargo.spdx.json
# libsafe.cargo.spdx3.json
# libvuln.cargo.cdx.json
# libvuln.cargo.spdx.json
# libvuln.cargo.spdx3.json
# safe-only.cargo.cdx.json
# ...
```

For N subprojects × M formats, you get N × M files + 1 manifest.

## Heterogeneous project (multi-ecosystem)

Given a repo with:
```
./
├── apps/frontend/package.json    (npm)
├── services/api/pyproject.toml   (pypi)
└── mobile/Package.swift          (swift)
```

Split scan produces one SBOM per detected ecosystem-root:

```bash
waybill sbom scan --path . --split --output-dir ./sboms/

# ./sboms/frontend.npm.cdx.json     (rooted at pkg:npm/frontend@x.y.z)
# ./sboms/api.pypi.cdx.json         (rooted at pkg:pypi/api@x.y.z)
# ./sboms/mobile.swift.cdx.json     (rooted at pkg:swift/mobile@x.y.z)
# ./sboms/split-manifest.json
```

## Inspecting the manifest

The manifest is JSON with a stable schema (v1 pinned):

```bash
cat ./sboms/split-manifest.json | jq .
```

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
    },
    ...
  ]
}
```

Common queries:

```bash
# List every subproject_id + its emitted files
jq '.entries[] | {subproject_id, files}' ./sboms/split-manifest.json

# Find the CycloneDX SBOM for a specific subproject
jq -r '.entries[] | select(.subproject_id == "libsafe.cargo") | .files["cyclonedx-json"]' \
    ./sboms/split-manifest.json

# Total component count across all sub-SBOMs (with duplication)
jq '[.entries[].component_count] | add' ./sboms/split-manifest.json

# Number of distinct components repo-wide
jq '.total_unique_components' ./sboms/split-manifest.json
```

## Reproducible split under `WAYBILL_FIXED_TIMESTAMP`

For CI reproducibility (byte-identical output across two runs on unchanged input):

```bash
export WAYBILL_FIXED_TIMESTAMP="2026-01-01T00:00:00Z"
waybill sbom scan --path . --split --output-dir ./sboms-run-1/
waybill sbom scan --path . --split --output-dir ./sboms-run-2/

diff -r ./sboms-run-1/ ./sboms-run-2/   # → empty (identical)
```

- All sub-SBOM serial numbers become deterministic hashes of `(subproject_purl + fixed_ts)`
- Manifest `generated_at` matches the fixed value
- `entries[]` sort order is deterministic (lex by `subproject_id`)

## Zero-boundary fallback

On a single-package project (no workspace members detected):

```bash
waybill sbom scan --path ./single-pkg --split --output-dir ./out/

# WARN waybill::generate::split: no workspace boundaries detected in scan_root=./single-pkg;
#      emitting single SBOM per --split fallback contract (FR-009).

ls ./out/
# root.generic.cdx.json     # OR whatever the single main-module PURL slug is
# (no split-manifest.json — nothing to describe)
```

Exit code 0 — `--split` on a single-package project is a benign no-op, not an error.

## Common CI recipe

Route each sub-SBOM to a per-service vuln scanner:

```bash
#!/bin/bash
set -euo pipefail

waybill sbom scan --path . --split --output-dir ./sboms/ --format cyclonedx-json

for entry_path in $(jq -r '.entries[] | @base64' ./sboms/split-manifest.json); do
    entry=$(echo "$entry_path" | base64 -d)
    subproject_id=$(echo "$entry" | jq -r '.subproject_id')
    sbom_file=$(echo "$entry" | jq -r '.files["cyclonedx-json"]')

    # Route to service-specific scan target
    echo "Scanning $subproject_id..."
    trivy sbom "./sboms/$sbom_file" --format json > "./scan-results/$subproject_id.json"
done
```

## Error cases

### `--split` + `--output <file>` — HARD ERROR

```bash
waybill sbom scan --path . --split --output single.cdx.json
```

```
error: `--split` is incompatible with `--output <file>` — a single file
       cannot hold N sub-SBOMs. Use `--output-dir <dir>` instead;
       Waybill will emit N sub-SBOM files + a split-manifest.json into <dir>.

       Example:
         waybill sbom scan --path . --split --output-dir ./sboms/
```

Exit code 2 (clap standard usage-error exit).

### Emit failure mid-split

If sub-SBOM 3 of 4 fails to emit (disk full, permission denied, etc.), the WHOLE invocation exits non-zero. No manifest is written; already-written sub-SBOMs 1 and 2 remain on disk but are unusable without a manifest to describe the set. Operator recovery: delete the partial output-dir, fix underlying issue, re-run.

## Verification checklist

After a split scan, sanity checks the operator can run:

```bash
# 1. Manifest exists and validates
[ -f ./sboms/split-manifest.json ] && echo "OK: manifest present"

# 2. Every file listed in manifest exists on disk
jq -r '.entries[].files[]' ./sboms/split-manifest.json | \
    while read -r f; do [ -f "./sboms/$f" ] || echo "MISSING: $f"; done

# 3. Every emitted sub-SBOM is valid JSON
find ./sboms/ -name '*.json' -exec jq empty {} \; || echo "invalid JSON found"

# 4. Component-count sanity: manifest total ≥ every entry count
total=$(jq '.total_unique_components' ./sboms/split-manifest.json)
max_entry=$(jq '[.entries[].component_count] | max' ./sboms/split-manifest.json)
[ "$total" -ge "$max_entry" ] && echo "OK: total >= max_entry"
```

## Rollback / opt-out

`--split` is strictly opt-in. Existing single-SBOM invocations are byte-identical to pre-feature behavior (SC-007). To opt out:

```bash
# Just don't pass --split. Same command, single SBOM.
waybill sbom scan --path . --output-dir ./out/
```

If a downstream consumer needs a unified whole-repo SBOM from an already-split output, they can either (a) re-scan without `--split`, or (b) wait for the SBOM merge feature (tracked at [waybill#627](https://github.com/kusari-oss/waybill/issues/627)) which will document the re-merge recipe.
