# Implementation Plan: Bazel + CMake source-tree readers (milestone 102 PR-B)

**Branch**: `103-bazel-cmake-impl` | **Date**: 2026-05-14 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/103-bazel-cmake-impl/spec.md`

## Summary

Replace the two stub readers shipped in milestone-102 PR-A (`mikebom-cli/src/scan_fs/package_db/bazel.rs` and `cmake.rs` — both currently `Vec::new()` returners) with regex-based parsers per the design recorded in [`specs/102-cpp-bazel-cmake-readers/`](../102-cpp-bazel-cmake-readers/). The reader entry-point signatures, package-db dispatch, `--include-vendored` CLI flag, env var fallback, and `pub mod` declarations are all already in place from PR-A — this milestone is **pure body replacement plus tests + fixtures + goldens**.

The same `regex` workspace dep used by vcpkg/conan in PR-A drives both new parsers. Test fixtures live in-repo at `mikebom-cli/tests/fixtures/{bazel,cmake}/` (small, synthetic, matching the PR-A pattern). Goldens regen extends the existing `cdx_regression.rs` / `spdx_regression.rs` / `spdx3_regression.rs` test arrays by 2 ecosystems each (6 new byte-identity goldens). Diff scope: 2 modified production files + 4 new integration tests + 2 fixture dirs + 6 goldens + 1 doc file.

The biggest implementation risk is CMake's heuristic regex parser — CMake is Turing-complete, so SC-002 explicitly caps coverage at ≥90%. The plan acknowledges this and budgets `if(BUILD_TESTING)` block detection as a stretch goal (covered by Edge Cases, not a hard FR).

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–102; no nightly required).
**Primary Dependencies**: Existing only — `regex = "1"` (workspace; already used by vcpkg/conan in PR-A), `tracing`, `anyhow`, std. **Zero new Cargo dependencies.**
**Storage**: N/A — all parsing is in-process; reader returns `Vec<PackageDbEntry>` to the existing `read_all` dispatcher.
**Testing**: Standard `cargo test` integration tests — `tests/scan_bazel.rs`, `tests/scan_cmake.rs`, `tests/scan_cmake_vendored.rs`, `tests/scan_cmake_findpackage_negative.rs`. Plus 6 byte-identity goldens via the existing `cdx_regression` / `spdx_regression` / `spdx3_regression` suites.
**Target Platform**: Cross-platform (no `#[cfg(unix)]` gates per FR-011). Verified Windows-compatible via the milestone-100 path-normalization chokepoint already in place.
**Project Type**: Single-crate workspace extension. Net addition: 2 reader bodies (replacing stubs) + 4 integration-test files + 2 fixture directories + 6 goldens.
**Performance Goals**: <100ms per manifest-file parse on average hardware (CMakeLists.txt rarely exceeds 100KB; WORKSPACE.bazel can hit 1-5MB on the long tail). Aggregate impact <500ms on a typical C/C++ source tree.
**Constraints**: Diff scope per SC-007 — ≤2 modified production files, ≤4 NEW integration tests, ≤2 new fixture dirs, 6 new goldens, ≤1 modified doc file. Zero new Cargo deps.
**Scale/Scope**: ~280-line bazel.rs + ~350-line cmake.rs + ~150-line × 4 integration tests = ~1200 LOC net. Plus 6 generated goldens (~15 KB).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Constitution version: **1.4.0**.

All 12 principles' status carries over from milestone-102 plan.md's Constitution Check table — no design change, same alignment:

| Principle | Compliance |
|---|---|
| I. Pure Rust, Zero C | ✅ PASS (no new crates; regex/tracing/anyhow already in tree) |
| II. eBPF-Only Observation | ✅ PASS (source-tree enrichment, not runtime discovery) |
| III. Fail Closed | ✅ PASS (per-file skip-with-warn matches maven/golang/PR-A precedent) |
| IV. Type-Driven Correctness | ✅ PASS (Purl newtype, LifecycleScope enum, thiserror/anyhow) |
| V. Specification Compliance | ✅ PASS (3 `mikebom:*` properties audited in 102's plan.md: download-url, vendored, bazel-archive-name) |
| VI. Three-Crate Architecture | ✅ PASS (touches only `mikebom-cli/src/scan_fs/package_db/`) |
| VII. Test Isolation | ✅ PASS (standard cargo test; no eBPF privileges) |
| VIII. Completeness | ✅ PASS (parse errors `tracing::warn!` per FR-013) |
| IX. Accuracy | ✅ PASS (`find_package` NOT emitted per FR-007; vendored opt-in default-OFF per FR-008) |
| X. Transparency | ✅ PASS (`mikebom:source-files` per FR-010; warn-logging on parse failures) |
| XI. Enrichment | ✅ PASS |
| XII. External Data Source Enrichment | ✅ PASS (local manifest files only) |

**Result**: All gates PASS. No complexity-tracking entries required.

## Project Structure

### Documentation (this feature)

```text
specs/103-bazel-cmake-impl/
├── plan.md                                  # This file
├── spec.md                                  # Inherits from 102 via cross-reference
├── research.md                              # Phase 0: regex strategies + parsing edge cases
├── data-model.md                            # Phase 1: file-by-file shapes
├── contracts/
│   └── reader-contracts.md                  # Phase 1: 8 behavioral contracts
├── quickstart.md                            # Phase 1: 5 maintainer recipes
├── checklists/
│   └── requirements.md                      # Spec quality checklist (12/12 PASS)
└── tasks.md                                 # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
mikebom-cli/
├── Cargo.toml                               # unchanged — no new deps
├── src/scan_fs/package_db/
│   ├── bazel.rs                             # MODIFY: replace PR-A stub with regex parser
│   ├── cmake.rs                             # MODIFY: replace PR-A stub with regex parser
│   └── (no other reader changes)
└── tests/
    ├── scan_bazel.rs                        # NEW: US1 integration test (4 tests)
    ├── scan_cmake.rs                        # NEW: US2 integration test (4 tests)
    ├── scan_cmake_vendored.rs               # NEW: US3 opt-in vendored test (3 tests)
    ├── scan_cmake_findpackage_negative.rs   # NEW: FR-007 negative-emission test (1 test)
    ├── cdx_regression.rs                    # MODIFY: +2 ecosystem test fns (bazel, cmake)
    ├── spdx_regression.rs                   # MODIFY: +2
    ├── spdx3_regression.rs                  # MODIFY: +2
    └── fixtures/
        ├── bazel/                           # NEW
        │   ├── MODULE.bazel
        │   └── WORKSPACE.bazel
        ├── cmake/                           # NEW
        │   ├── CMakeLists.txt
        │   ├── cmake/third_party.cmake
        │   └── third_party/foo/version.txt
        └── golden/
            ├── cyclonedx/{bazel,cmake}.cdx.json       # NEW (2)
            ├── spdx-2.3/{bazel,cmake}.spdx.json       # NEW (2)
            └── spdx-3/{bazel,cmake}.spdx3.json        # NEW (2)

docs/user-guide/
└── cli-reference.md                         # MODIFY: --include-vendored flag docs per FR-014
```

**Structure Decision**: The 2 readers replace existing stubs at exact `pub fn read(...)` signature locations:
- `bazel.rs::read(scan_root: &Path) -> Vec<PackageDbEntry>` — same as PR-A stub.
- `cmake.rs::read(scan_root: &Path, include_vendored: bool) -> Vec<PackageDbEntry>` — same as PR-A stub (the `include_vendored` flag was wired through PR-A's dispatch in `read_all`).

PR-A's wiring at `scan_fs/package_db/mod.rs:807-808` already calls both stubs unconditionally — no dispatch changes needed. The `MIKEBOM_INCLUDE_VENDORED` env var → `include_vendored` parameter plumbing is also unchanged.

`scan_cmake_findpackage_negative.rs` is a separate test file specifically for the FR-007 negative-emission contract (per milestone-102 spec C2 remediation that added the dedicated assertion). Could be folded into `scan_cmake.rs` if that test surface gets too granular at implementation time; this plan keeps it separate for clarity.

## Existing-Architecture Context

PR-A's foundational architecture is reused verbatim. The Phase-2 work from milestone 102 is already in main:

| Slot | Where | Reused as-is in milestone 103 |
|---|---|---|
| Reader entry point | `bazel.rs::read(scan_root)` + `cmake.rs::read(scan_root, include_vendored)` (stubs in main) | Bodies replaced; signatures unchanged |
| Package-db dispatch | `package_db/mod.rs::read_all` lines 807-808 (already calls both stubs) | No change needed |
| CLI flag | `cli/scan_cmd.rs::ScanArgs::include_vendored` (PR-A) | Already wired; just consumed by cmake.rs body |
| Env-var fallback | `MIKEBOM_INCLUDE_VENDORED=1` read directly by `read_all` and passed as `include_vendored: bool` to `cmake::read` (PR-A) | Already plumbed |
| `PackageDbEntry` field shape | `scan_fs/package_db/mod.rs:41-208` (24 fields) | Same construction pattern as vcpkg.rs/conan.rs from PR-A |
| `LifecycleScope` enum | `mikebom_common/src/resolution.rs:349-376` | Used for `dev_dependency = True` mapping (FR-002) |
| PURL construction | `mikebom_common/src/types/purl.rs::encode_purl_segment` + `Purl::new` | Same pattern as PR-A readers |
| `extra_annotations` for `mikebom:*` props | `BTreeMap<String, serde_json::Value>` | Same as PR-A's `mikebom:download-url` / `mikebom:vendored` / `mikebom:bazel-archive-name` |

The plan is **purely additive replacement** — no architectural decisions need re-litigation.

## Complexity Tracking

No constitution violations. The milestone is body-replacement-of-stubs + tests + goldens; zero design risk.

**The single implementation risk is the CMake regex parser's coverage ceiling**, captured by SC-002's 90% floor:
- `FetchContent_Declare` and `ExternalProject_Add` arguments are sometimes assembled via CMake variables (`${GTEST_VERSION}`) — the regex won't expand these.
- Some projects wrap fetch calls in macros (`set_fetch_content(...)`) — invisible to a regex that anchors on the literal call names.
- Spot-check baseline (LLVM, gRPC, Envoy, RocksDB CMakeLists.txt files in milestone-102's research): ≥90% of declared deps are literal-argument calls.

Mitigation: document the heuristic ceiling explicitly in cli-reference.md (per FR-014), so operators know to supplement via vcpkg/Conan manifests for non-literal cases.
