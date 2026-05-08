# Implementation Plan: Maven SPDX dep-edge emission

**Branch**: `085-maven-spdx-dep-edges` | **Date**: 2026-05-08 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/085-maven-spdx-dep-edges/spec.md`

## Summary

One extra `name_to_purl` insert per maven entry in `mikebom-cli/src/scan_fs/mod.rs:373-379` so maven main-module's `"groupId:artifactId"` depend names resolve to their target PURLs. Side effects: SPDX 2.3 + SPDX 3 maven goldens regenerate (3 new `DEPENDS_ON` / `software_dependsOn` edges per maven main-module). CDX maven golden stays byte-identical (the same 3 edges already came through the primary-dep fallback at `dependencies.rs:78-91`; post-085 they come through real `Relationship` entries instead). The `KNOWN_PARITY_GAPS` allowlist entry from milestone 084 is removed.

Net code change: ~6 LOC in `scan_fs/mod.rs` + 1 line removed from `holistic_parity.rs` + maven SPDX 2.3 + SPDX 3 goldens regenerated.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain).
**Primary Dependencies**: existing only — no new crates.
**Storage**: N/A.
**Testing**: existing `cdx_regression` / `spdx_regression` / `spdx3_regression` / `holistic_parity` / `cdx_ref_closure_invariant` test suites.
**Target Platform**: Linux + macOS (same as rest of mikebom).
**Project Type**: CLI extension (existing `mikebom-cli` crate).
**Performance Goals**: emission stays within milestone-082 baseline (one extra HashMap insert per maven entry; negligible).
**Constraints**: maven SPDX 2.3 + SPDX 3 goldens regenerate with diffs containing only the new `DEPENDS_ON` / `software_dependsOn` edges; CDX maven golden byte-identical; other ecosystems' goldens (CDX/SPDX/SPDX 3) all byte-identical.
**Scale/Scope**: ~6 LOC in `scan_fs/mod.rs` + 1 LOC removed from `holistic_parity.rs` + 2 SPDX goldens regenerate.

## Constitution Check

| Principle | Compliance |
|---|---|
| **I. Pure Rust, Zero C** | ✅ pass |
| **II. eBPF-Only Observation** | N/A — emission, not discovery |
| **III. Fail Closed** | N/A |
| **IV. Type-Driven Correctness** | ✅ pass — no new untyped boundaries; no `.unwrap()` in production |
| **V. Specification Compliance** | ✅✅ — closes a per-format vocabulary divergence; standards-native dep-edge emission (SPDX 2.3 `DEPENDS_ON` + SPDX 3 `software_dependsOn`); no `mikebom:*` introduced |
| **VI. Three-Crate Architecture** | ✅ pass |
| **VII. Test Isolation** | ✅ pass |
| **VIII. Completeness** | N/A — identifier resolution, not dep discovery |
| **IX. Accuracy** | ✅ improves accuracy — closes dep-edge gap surfaced by milestone 084 |
| **X. Transparency** | N/A |
| **XI. Enrichment** | N/A |
| **XII. External Data Source Enrichment** | N/A |

Strict Boundaries: all preserved. Pre-PR gate per CLAUDE.md applies. **Decision: PASS.**

## Project Structure

```text
mikebom-cli/
├── src/
│   └── scan_fs/
│       └── mod.rs                       # MODIFY — name_to_purl dual-key insert for maven (~6 LOC at lines 373-379)
├── tests/
│   ├── holistic_parity.rs               # MODIFY — remove KNOWN_PARITY_GAPS entry for ("maven", "B1") (~5 LOC removed; allowlist becomes empty const &[])
│   └── fixtures/golden/
│       ├── spdx-2.3/maven.spdx.json     # REGEN — 3 new DEPENDS_ON
│       └── spdx-3/maven.spdx3.json      # REGEN — 3 new software_dependsOn
```

## Complexity Tracking

No violations. Section intentionally empty.
