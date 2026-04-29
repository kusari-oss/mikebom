# Implementation Plan: Per-File Evidence for apk Components

**Branch**: `039-apk-deep-hash` | **Date**: 2026-04-29 | **Spec**: [spec.md](spec.md)

## Summary

Mirror milestone 038's deb deep-hash path for apk. The apk
installed-db already encodes the per-file paths inline via `F:`
(directory) and `R:` (regular file basename) lines under each
package's stanza, so no companion-file or layout discovery is
needed — just a one-pass extraction during the same db read,
and a `hash_apk_package_files` hashing loop that mirrors the
existing dpkg one.

**Technical approach**: extract a per-package file-list map once
in `apk.rs`, pass it into a new `hash_apk_package_files` in
`file_hashes.rs`, and add an `is_apk` branch to the existing
deep-hash dispatcher in `scan_fs/mod.rs`. ~250 LOC total.

## Technical Context

**Language**: Rust stable (workspace toolchain unchanged from 038).
**Primary Dependencies**: existing only — `sha2`, stdlib `std::fs` /
`std::io`, `tempfile` (dev). No new top-level deps.
**Project Type**: CLI / library (Rust three-crate workspace —
Constitution Principle VI preserved).
**Performance**: per-file SHA-256 is IO-bound; cap inherited from
dpkg path (256 MB / file). Apk packages are typically smaller than
deb packages so deep-hash cost should be slightly lower.
**Constraints**: Constitution Principles I, IV, VI (all already
satisfied; no new surface).

## Constitution Check

| Principle | Status |
|---|---|
| I. Pure Rust, Zero C | ✅ no new deps |
| IV. Type-Driven Correctness | ✅ Option/Result throughout; reuses existing newtypes |
| V. Specification Compliance | ✅ no SBOM schema changes; reuses CycloneDX evidence shape |
| VI. Three-Crate Architecture | ✅ touches `mikebom-cli` only |
| VIII. Completeness | ✅ closes a known false-negative |
| IX. Accuracy | ✅ file content hashes are observed-bytes truth |

**No constitution violations.**

## Touched files

| File | Change | LOC |
|---|---|---|
| `mikebom-cli/src/scan_fs/package_db/apk.rs` | + `read_file_lists(rootfs) -> HashMap<String, Vec<String>>`; inline tests | +90 |
| `mikebom-cli/src/scan_fs/package_db/file_hashes.rs` | + `hash_apk_package_files(rootfs, &[String])`; + `hash_apk_db_only(rootfs, pkg)`; inline tests | +120 |
| `mikebom-cli/src/scan_fs/mod.rs` | + `is_apk` branch parallel to `is_dpkg` | +30 |
| `mikebom-cli/tests/oci_registry_smoke.rs` | extend alpine smoke to assert per-file evidence > 0 | +15 |
| `docs/user-guide/cli-reference.md` | minimal-image-coverage paragraph adjusted | +5 |
| `CHANGELOG.md` | unreleased entry | +5 |

Total ~265 LOC across 6 files.

## Phasing

Three commits.

1. **`spec(039)`** — spec set (this file + spec.md + checklists).
2. **`feat(039/extract-file-lists)`** — `apk.rs::read_file_lists`
   + inline tests. Behind `#[allow(dead_code)]` until commit 3.
3. **`feat(039/hash-and-wire)`** — `hash_apk_package_files` in
   `file_hashes.rs` + `is_apk` branch in `scan_fs/mod.rs` + smoke
   test extension. Removes the dead-code allow.
4. **`docs(039)`** — user-guide + CHANGELOG.

## Estimated effort

| Phase | Effort |
|---|---|
| Spec set | 30 min |
| Commit 2 (file-list extract) | 1.5 hr |
| Commit 3 (hash + wire) | 2 hr |
| Commit 4 (docs) | 30 min |
| Verify + PR | 30 min |
| **Total** | **~5 hr** |

## Risks

- **R1**: apk's `R:` line format edge cases. Most are basenames
  under the current `F:`-declared directory, but some packages
  (e.g. baselayout-data) carry empty `F:` for root-level files.
  The existing `collect_claimed_paths` handles this correctly
  (line 81 — `if dir.is_empty() ...`); reuse that logic.
- **R2**: golden drift. Existing 27 byte-identity goldens cover
  alpine fixtures. If any of those alpine-fixture goldens contain
  apk components, this milestone WILL change them (adding per-file
  evidence). Need to regen and verify the change is additive only.

## What this milestone does NOT do

- Does not add the apk-provided SHA-1 cross-reference (Z: lines).
- Does not change PURL, schema, or fast-hash flag semantics.
- Does not touch rpm or any other ecosystem.
