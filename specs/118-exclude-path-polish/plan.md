# Implementation Plan: Milestone 113 `--exclude-path` polish bundle

**Branch**: `118-exclude-path-polish` | **Date**: 2026-06-13 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/118-exclude-path-polish/spec.md`

## Summary

Three coordinated changes shipped in one PR:

**Production-code additions (minimal)** — `ExclusionSet` (`mikebom-cli/src/scan_fs/package_db/exclude_path.rs:108-228`) gains a `pub(crate)` `AtomicUsize` `suppressed_dirs` counter incremented by `safe_walk` (`walk.rs:224-235`) on every exclusion hit, plus two cheap accessor helpers (`count_literals()`, `count_patterns()`). The scan-end `tracing::info!` line at `scan_cmd.rs:1750-1754` extends to emit the FR-010 summary fields (`excluded_entries=N`, `suppressed_dirs=M`) when the set is non-empty — preserved-byte-identity when empty per FR-010's emission gating.

**Test additions** — extend `mikebom-cli/tests/exclude_path_integration.rs` with the four US1 + two US2 integration tests (six total), reusing the existing `run_scan` / `component_names` / `envelope_property` helpers. Add `mikebom-cli/tests/exclude_path_perf.rs` for FR-011's opt-in `#[ignore]`d perf benchmark following the milestone-094 `dual_format_perf.rs` pattern (`std::time::Instant`, median-of-5, ≤1.10× budget assertion). Add the help-text test for FR-008. All synthetic fixtures vendored in-tree via `tempfile` per FR-012 (no new vendored fixture dirs; the spec's "vendored in-tree" wording is satisfied by `tempfile`-synthesized per-test scaffolds, matching how every existing milestone-113 test works).

**Docs additions** — add ONE consolidated `## Directory exclusion (--exclude-path)` cross-cutting section in `docs/ecosystems.md` after the coverage matrix and before per-ecosystem sections; add a short pointer line to each ecosystem section. CLI-reference and help-text already document the flag (milestone 113 T029 + T028); FR-008's pointer is already present in the clap doc-comment at `main.rs:140`. The help-text test simply asserts the existing behavior.

**Technical approach**: greenfield additions only. The walker-audit gate (milestone 115/117) doesn't apply because all test additions live in `mikebom-cli/tests/` (outside `scan_fs/`). The `AtomicUsize` counter is the smallest viable instrumentation that doesn't churn `safe_walk`'s `&self` borrow chain; alternatives (thread-local register, summary-struct return type from `safe_walk`) were rejected for outsized refactor cost. The single-PR scope is sized for ~1 day per the issue body; the perf benchmark (FR-011) is the documented cut-point if review feedback pushes back on diff size.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–117; no nightly required for this user-space-only feature).
**Primary Dependencies**: Existing only — `std::sync::atomic::AtomicUsize` (std), `tracing`, `tempfile` (test-only; already a dev-dep), `serde_json` (test parsing), `globset` (workspace; already used by milestone 113's `ExclusionSet`). **Zero new Cargo dependencies.**
**Storage**: N/A — all state in-process per scan; the `AtomicUsize` counter is reset implicitly on each new `ExclusionSet` instance (one per scan).
**Testing**: `cargo +stable test --workspace` (existing harness) covers the new integration + help-text tests. The perf benchmark is `#[ignore]`d and runs only via `cargo +stable test --test exclude_path_perf -- --ignored` per the milestone-094 convention. The kusari-cli fixture is already cached locally via milestone 090 (`MIKEBOM_FIXTURES_DIR` env var, pinned via `tests/fixtures.rev`).
**Target Platform**: Linux x86_64 + macOS aarch64 + Windows x86_64 (default test suite); perf benchmark runs on Linux only per milestone-094's macOS thermal-noise exemption.
**Project Type**: Single-project Rust CLI (`mikebom-cli/`).
**Performance Goals**: FR-011's ≤1.10× overhead budget on the kusari-cli fixture (per spec clarification Q1). The `AtomicUsize` increment per exclusion hit costs single-digit nanoseconds; in the no-flag scan path the `is_empty()` short-circuit at `walk.rs:224` skips the counter entirely.
**Constraints**: Backwards-compatible — when `--exclude-path` has zero entries, output is byte-identical to pre-118 mikebom per FR-010's emission gating + the `is_empty()` short-circuit. The walker-audit gate (milestone 115/117) is preserved; this feature adds no `fn walk*` functions and adds no allow-list entries.
**Scale/Scope**: 1 production-code module extended (`exclude_path.rs` — ~20 LoC for the counter + accessors), 1 `safe_walk` call-site update (~3 LoC), 1 scan_cmd `tracing::info!` extension (~5 LoC), 6 new integration tests in the existing `exclude_path_integration.rs` (~200 LoC), 1 new perf-bench file (~120 LoC), 1 new help-text test (~30 LoC), 1 docs section + ~10 short pointer lines (~80 LoC). Diff size estimate: ~450 LoC production + tests + ~80 LoC docs.

## Constitution Check

| Principle | Status | Notes |
|---|---|---|
| I. Pure Rust, Zero C | ✓ | std-only feature; no new C deps. |
| II. eBPF-Only Observation | N/A | `sbom scan` path; trace path is untouched. |
| III. Fail Closed | ✓ | No operator-bypass paths. The `--exclude-path` flag IS an operator override; this feature adds observability + tests, no relaxation of any existing fail-closed behavior. |
| IV. Type-Driven Correctness | ✓ | `AtomicUsize` is std; no new domain types needed; zero new `.unwrap()` in production code. |
| V. Specification Compliance | ✓ | No SBOM emission change. The `tracing::info!` extension is stderr-only (not SBOM content); the existing milestone-113 `mikebom:exclude-path` envelope annotation is unchanged. |
| VI. Three-Crate Architecture | ✓ | Lives entirely in `mikebom-cli/`; no new crates. |
| VII. Test Isolation | ✓ | Per-test `tempfile::tempdir()` isolation matches the existing milestone-113 pattern. Perf benchmark uses the milestone-090 fixture cache (read-only). |
| VIII. Completeness | ✓ | No discovery layer change. |
| IX. Accuracy | ✓ | No accuracy change. |
| X. Transparency | ✓ | New `tracing::info!` summary surfaces operator-visible exclusion stats (FR-010 / SC-005), strengthening Principle X. |
| XI. Enrichment | N/A | |
| XII. External Data Source Enrichment | N/A | |
| Strict Boundary 1 (no lockfile-based discovery) | N/A | |
| Strict Boundary 2 (no MITM) | N/A | |
| Strict Boundary 3 (no C code) | ✓ | |
| Strict Boundary 4 (no `.unwrap()` in production) | ✓ | Counter increments use `fetch_add(1, Relaxed)`; reads use `load(Relaxed)`; both infallible. The `tracing::info!` extension uses the existing infallible field-emission pattern. |

**Result**: Constitution Check PASSES. No violations.

## Project Structure

### Documentation (this feature)

```text
specs/118-exclude-path-polish/
├── plan.md              # This file
├── research.md          # Phase 0 — 6 implementation decisions: counter mechanism (AtomicUsize vs thread-local vs return-struct), test-file consolidation, fixture vending (tempfile vs in-tree dirs), tracing-summary wording, perf-bench median-of-N, docs cross-link anchor placement
├── data-model.md        # Phase 1 — ExclusionSet counter extension + tracing summary contract + perf-bench measurement protocol
├── quickstart.md        # Phase 1 — "how an operator reads the new tracing summary" + "how a contributor runs the opt-in perf benchmark" runbook
├── contracts/
│   ├── tracing-summary.md  # The scan-end `tracing::info!` summary contract: field names, emission gating, byte-identity preservation
│   └── perf-bench.md       # The opt-in perf-bench contract: invocation, fixture, budget, median sampling protocol
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   ├── scan_fs/
│   │   ├── package_db/
│   │   │   └── exclude_path.rs                  # EXTENDED — AtomicUsize counter + accessor helpers (~20 LoC)
│   │   └── walk.rs                              # EXTENDED — increment counter at line 227 exclusion-hit site (~3 LoC)
│   └── cli/
│       └── scan_cmd.rs                          # EXTENDED — tracing::info! at line 1750-1754 gains excluded_entries + suppressed_dirs fields (~5 LoC)
├── tests/
│   ├── exclude_path_integration.rs              # EXTENDED — 6 new tests (golang source, go binary, dep-edge, scan-root, multi-pattern, separator) reusing existing helpers (~200 LoC)
│   ├── exclude_path_perf.rs                     # NEW — opt-in `#[ignore]`d perf benchmark using kusari-cli fixture (~120 LoC)
│   └── exclude_path_help_text.rs                # NEW — FR-008 help-text discoverability test (~30 LoC)
docs/
├── ecosystems.md                                # EXTENDED — one consolidated "## Directory exclusion" cross-cutting section + per-ecosystem pointer lines (~80 LoC)
└── user-guide/
    └── cli-reference.md                         # UNCHANGED — milestone 113 T029 already established the comprehensive `### --exclude-path` section
mikebom-common/                                    # UNCHANGED
mikebom-ebpf/                                      # UNCHANGED
```

**Structure Decision**: Single-project layout. Per spec FR-012, all new integration-test fixtures are synthesized in-process via `tempfile::tempdir()` — same pattern as the five existing milestone-113 tests at `exclude_path_integration.rs:145-285`. The "vendored in-tree" wording in FR-012 is satisfied by this pattern (per-test scaffolds materialized at runtime under `${TMPDIR}`, never persisted to the source tree). The perf-bench fixture (kusari-cli) lives in the existing milestone-090 cache; no new vendored fixture directories are created.

## Complexity Tracking

No constitution violations. The one design decision worth noting is the **counter-mechanism choice** (Decision 1 in research.md): `AtomicUsize` on `ExclusionSet` vs. thread-local register vs. summary-struct return-type from `safe_walk`. The `AtomicUsize` path is chosen because (a) the existing `ExclusionSet` is already threaded by `&` through every walker; (b) `fetch_add(1, Relaxed)` on `&self` doesn't churn the borrow chain; (c) reading `.load(Relaxed)` post-scan is one line. The alternatives would force ~20 `safe_walk` call-site updates (return-struct path) OR add a process-global static (thread-local path) that interacts awkwardly with concurrent scans.
