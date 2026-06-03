# Implementation Plan: Bind binary-tier C/C++ components to source-tier PURLs via cmake build-directory observation

**Branch**: `109-binary-source-purl-binding` | **Date**: 2026-06-02 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/109-binary-source-purl-binding/spec.md`

## Summary

mikebom currently emits the same C/C++ library under two non-joining PURLs when the same project is scanned end-to-end: source-tier (`pkg:github/madler/zlib@v1.3.1` from `FetchContent_Declare`) and binary-tier (`pkg:generic/zlib` from the symbol-fingerprint matcher). This milestone introduces a build-directory observation layer that joins the two when a project root is scanned with `--fingerprints-corpus`: walk the build tree for cmake's documented `_deps/<name>-build/` layout, and when a fingerprint match's library name (case-insensitive) corresponds to a cmake declaration that produced one of those build dirs, attribute the binary match to the source-tier PURL. Scope is `FetchContent_Declare` only this milestone (per Phase-2 clarification); `ExternalProject_Add` deferred. Non-opt-in scans, single-binary scans, and source-only scans preserve milestone-108 behavior byte-identically. No new Cargo deps; uses the existing cmake reader's parsed declarations + std::fs walking.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–108; no nightly required for this user-space-only attribution layer).
**Primary Dependencies**: existing only — `std::fs::canonicalize` / `std::fs::read_dir` (build-dir walking), the milestone-102/103 cmake reader's existing parsed-declaration output (`PackageDbEntry` instances tagged with `mikebom:source-mechanism = cmake-fetchcontent-{git,url}`), and the milestone-099/108 fingerprint matcher's existing `SymbolFingerprintMatch` records. The milestone-105 dedup pipeline (`SourceMechanism` enum + `mikebom:also-detected-via` collision handling) merges the cmake source-tier component with the post-attribution binary-tier component into ONE final component via shared PURL.
**Storage**: N/A — all attribution state is in-process for the duration of a single scan; mirrors every milestone since 002.
**Testing**: `cargo +stable test --workspace`; unit tests for the path-observer pure functions in `mikebom-cli/src/scan_fs/binary/source_binding/`; integration tests against synthetic cmake build-dir fixtures (no real cmake invocation needed at test time — the observer's input is filesystem state, easily reproducible with `std::fs::create_dir_all`).
**Target Platform**: cross-platform Unix + Windows. The fingerprint matcher works across ELF + Mach-O + PE as of PR #305 (Mach-O) + PR #309 (PE); the build-dir observer is pure filesystem walking and platform-agnostic.
**Project Type**: Rust workspace CLI — `mikebom-cli` only; no `mikebom-common` or `xtask` touched.
**Performance Goals**: walker overhead ≤10ms per cmake build directory at typical project scale (≤100 `_deps/` entries). Negligible compared to the binary-walker's existing per-binary cost. Refined in Phase 0 research.
**Constraints**: zero new Cargo dependencies; no subprocess calls (no `cmake --build` invocation); no network access; offline-mode-compatible by construction (the attribution layer reads only the local filesystem, never makes new network calls — FR-014 audit naturally extends to cover this milestone's new files); 33 byte-identity goldens unchanged.
**Scale/Scope**: tens of cmake-declared deps per typical project (mikebom-cmake-demo has 1; large C++ projects have 5-50); one or two cmake projects per scan root (multi-project workspaces handled per-project per the spec's edge case).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

All twelve principles + four strict boundaries evaluated; gates PASS without violations.

- **I. Pure Rust, Zero C** ✅ — zero new Cargo dependencies. Pure stdlib walker.
- **II. eBPF-Only Observation** ✅ — N/A. User-space only; `mikebom-ebpf` untouched.
- **III. Fail Closed** ✅ — FR-011 explicitly: walker failure (e.g., `_deps/` exists but is empty, permission denied on a subdirectory) MUST surface as a `tracing::warn!` and let the source-tier scan emit unchanged. Scan does NOT abort. Honors the principle's "warn, don't block" carve-out for ambient-quality-improvement layers.
- **IV. Type-Driven Correctness** ✅ — new entities (`CmakeBuildDirObservation`, `SourceTierBinding`) are explicit Rust structs; the join is computed via `BTreeMap<(library_name_lowercased, build_dir_path), SourceTierBinding>` (compiler-enforced uniqueness on the lookup key).
- **V. Specification Compliance** ✅ — no CDX 1.6 / SPDX 2.3 / SPDX 3.0.1 emission changes; the attribution rewrites the binary-tier matcher's output BEFORE the dedup pipeline, so emission flows through the same existing per-format paths. Reuses the milestone-105 `mikebom:source-mechanism` enum + `mikebom:also-detected-via` annotation per FR-004 + FR-006 — no new annotation keys, no new C-row needed in `sbom-format-mapping.md` (the source-mechanism field already documented at C55; the cross-attribution just causes more components to carry it).
- **VI. Three-Crate Architecture** ✅ — changes confined to `mikebom-cli`. No `mikebom-common` type bumps. No `xtask` changes.
- **VII. Test Isolation** ✅ — per-test `tempfile::TempDir`s for both the source-tree cmake fixture AND the build-dir fixture. No shared state. No env-var mutations beyond existing `MIKEBOM_FINGERPRINTS_*` patterns already covered by `fingerprints::test_env_lock()`.
- **VIII. Completeness** ✅ — covers FetchContent's full default layout (`_deps/<name>-{src,build,subbuild}/`) per the cmake documented convention. `ExternalProject_Add` deferred per the Phase-2 clarification (documented limitation, not a coverage gap).
- **IX. Accuracy** ✅ — conservative join key (case-insensitive `name` equality + `_deps/<name>-build/` existence) deliberately produces zero false-positive bindings rather than aliasing non-standard names. False negatives are explicit and recoverable (operator sees `pkg:generic/zlib` instead of the source-tier PURL; the existing milestone-108 path still works).
- **X. Transparency** ✅ — the post-attribution component carries the cmake-derived `mikebom:source-mechanism` value AND the `mikebom:fingerprint-corpus-sha` value from milestone 108. Consumers can see exactly which evidence supports each component.
- **XI. Enrichment** ✅ — no enrichment layer changes. The attribution computes from local filesystem state only.
- **XII. External Data Source Enrichment** ✅ — N/A. No external data sources participate. The corpus reference data (milestone 108) is already covered.

**Strict Boundaries**:

- Pure Rust: ✅ no C/C++ deps added.
- eBPF-Only Observation: ✅ not in scope; this is user-space attribution.
- Fail Closed: ✅ documented and enforced via FR-011.
- Type-Driven: ✅ all new state explicit.

**Result**: PASS. No Complexity Tracking entries needed.

## Project Structure

### Documentation (this feature)

```text
specs/109-binary-source-purl-binding/
├── plan.md              # This file
├── research.md          # Phase 0 output — walker design + perf budget + annotation-merge semantics
├── data-model.md        # Phase 1 output — CmakeBuildDirObservation + SourceTierBinding shapes
├── quickstart.md        # Phase 1 output — operator + consumer recipes (mirrors m108 quickstart shape)
├── contracts/
│   ├── attribution-rules.md       # Join-key contract + per-record attribution outcomes
│   ├── walker-protocol.md         # Build-dir discovery walk algorithm + path patterns
│   └── annotation-emission.md     # How attribution interacts with m105 dedup pipeline + m108 corpus-sha
├── checklists/
│   └── requirements.md            # Already exists from /speckit-specify
└── tasks.md             # Phase 2 output (/speckit-tasks command — NOT created here)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   └── scan_fs/
│       ├── binary/
│       │   ├── mod.rs                       # Modified — orchestrate the attribution step in the existing per-binary loop
│       │   ├── scan.rs                      # Unchanged — symbol-extract paths already cover ELF + Mach-O + PE
│       │   ├── symbol_fingerprint.rs        # Modified — accept an optional `BuildAttributionRegistry` ref to rewrite match PURLs
│       │   ├── entry.rs                     # Modified — symbol_match_to_entry consumes the attributed PURL when present
│       │   ├── fingerprints/                # Unchanged — corpus loader from m108
│       │   └── source_binding/              # NEW — cmake build-dir observer + attribution registry
│       │       ├── mod.rs                   # Public surface: CmakeBuildDirObservation, SourceTierBinding, build_attribution_registry()
│       │       ├── cmake_observer.rs        # Walks _deps/<name>-build/ paths; joins with cmake reader's parsed declarations
│       │       └── registry.rs              # In-memory BTreeMap<library_name_lc, SourceTierBinding>; lookup helpers
│       └── package_db/
│           └── cmake.rs                     # Unchanged — already emits FetchContent declarations with source-mechanism tag
│
└── tests/
    ├── binary_source_binding_cmake.rs       # NEW — end-to-end integration: synthetic source + build-dir fixtures
    └── binary_source_binding_regression.rs  # NEW — SC-003/SC-004 byte-identity regression (no-opt-in + single-binary)
```

**Structure Decision**: NEW sub-module `mikebom-cli/src/scan_fs/binary/source_binding/` houses the cmake-build-dir observer + the attribution registry. The matcher path (`symbol_fingerprint.rs::scan_with_corpus`) gains an optional `Option<&BuildAttributionRegistry>` parameter; when `Some(_)`, the match's `target_purl` field is rewritten to the source-tier PURL before the dedup pipeline runs. This keeps the cmake-specific path-observation logic isolated (per FR-012's forward-compat constraint) — a future Bazel observer would land alongside `cmake_observer.rs` in the same sub-module and feed the same `BuildAttributionRegistry`.

## Complexity Tracking

> Constitution Check passed. No violations to justify.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| _(none)_  | _(none)_   | _(none)_                            |
