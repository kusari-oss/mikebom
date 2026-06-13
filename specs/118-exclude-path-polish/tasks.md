# Tasks: Milestone 113 `--exclude-path` polish bundle

**Input**: Design documents from `/specs/118-exclude-path-polish/`
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/tracing-summary.md ✓, contracts/perf-bench.md ✓, quickstart.md ✓

**Tests**: Per spec FR-001 through FR-008 + FR-011, integration tests + opt-in perf benchmark are the principal validation mechanism. Eight test tasks below cover this.

**Single-PR scope**: ~1 day of focused work per issue #343's body. No PR-split required. Cut-point if needed: FR-011's perf benchmark (T012) is the spec's documented deferral candidate.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

Single-project layout (mikebom workspace at repo root). Affected paths:
- `mikebom-cli/src/scan_fs/package_db/exclude_path.rs` (ExclusionSet counter + accessors)
- `mikebom-cli/src/scan_fs/walk.rs` (increment site at line 227)
- `mikebom-cli/src/cli/scan_cmd.rs` (tracing::info! extension at line 1750-1754)
- `mikebom-cli/tests/exclude_path_integration.rs` (6 new integration tests appended)
- `mikebom-cli/tests/exclude_path_help_text.rs` (NEW — FR-008 discoverability test)
- `mikebom-cli/tests/exclude_path_perf.rs` (NEW — FR-011 opt-in perf benchmark)
- `docs/ecosystems.md` (consolidated cross-cutting section + per-ecosystem pointers)

No new files in production source paths beyond the existing `ExclusionSet` extension. The walker-audit gate (milestone 115/117) does NOT apply because all test additions live under `tests/` (outside `scan_fs/`).

---

## Phase 1: Setup

No setup tasks. The shared test helpers (`run_scan`, `component_names`, `envelope_property`, `write_cargo_project`) already exist in `exclude_path_integration.rs` from milestone 113 — reused unchanged by US1+US2 tasks.

---

## Phase 2: Foundational

No foundational tasks. US1 + US2 tests assert on SBOM content (no dependency on US3's tracing instrumentation). US3 tasks can land in any order relative to US1/US2.

---

## Phase 3: User Story 1 — Per-ecosystem regression coverage (Priority: P1) 🎯 MVP

**Goal**: Every ecosystem covered by the milestone-113 walker plumbing has at least one integration test asserting `--exclude-path` works for that ecosystem. Future ecosystem-reader changes can't silently break exclusion without CI catching it.

**Independent Test**: Run `cargo +stable test --test exclude_path_integration` and verify the four new tests pass alongside the five existing milestone-113 tests (9 total).

### Implementation for User Story 1

- [X] T001 [US1] Append integration test `golang_source_fixture_suppressed_via_exclude_path` to `mikebom-cli/tests/exclude_path_integration.rs` per FR-001. Synthesize via `tempfile::tempdir()` per Decision 3: a `<tmp>/real-app/` directory with a real `go.mod` + `main.go` (package main; module github.com/example/real-app); plus a `<tmp>/tests/fixtures/fixture-app/` directory with a fixture `go.mod` + `main.go` (module github.com/example/fixture-app). Scan with `run_scan(<tmp>, &[])` → assert `component_names` contains both `github.com/example/real-app` AND `github.com/example/fixture-app`. Scan again with `run_scan(<tmp>, &["tests/fixtures"])` → assert `component_names` contains `github.com/example/real-app` but NOT `github.com/example/fixture-app`. Fixture paths chosen to NOT match the Go-tool unconditional skip shapes (no `testdata/`, no `_`-prefix) so the test exercises `--exclude-path` exclusively. Add a small `write_go_module(dir, module_path)` helper near the existing `write_cargo_project` if it doesn't already exist.

- [X] T002 [US1] Append integration test `go_binary_fixture_suppressed_via_exclude_path` to `mikebom-cli/tests/exclude_path_integration.rs` per FR-002. The fixture pre-builds a tiny Go binary at test setup via `std::process::Command::new("go").arg("build").arg("-o").arg(&binary_path).arg(&source_path).status()`. If the host has no `go` toolchain (`Command::new("go").arg("version").output().is_err()`), the test gracefully skips with `eprintln!("skipping: go toolchain not available")` + `return` — matches the milestone-053 / milestone-091 Go-shell-out conditional-skip pattern, preserves Decision 3's "no new fixture directories" rule, and keeps the test hermetic across hosts. Place the binary at `<tmp>/tests/fixtures/go-binary/bin/foo`. Scan with `run_scan(<tmp>, &[])` → assert `component_names` contains `pkg:generic/foo` (the milestone-096 binary-tier classification PURL). Scan with `run_scan(<tmp>, &["tests/fixtures"])` → assert `component_names` does NOT contain `pkg:generic/foo`.

- [X] T003 [US1] Append integration test `dependency_edges_referencing_suppressed_components_dropped` to `mikebom-cli/tests/exclude_path_integration.rs` per FR-003. Synthesize a multi-ecosystem fixture where ecosystem A's component depends on ecosystem B's component: e.g., a Cargo project at `<tmp>/main-app/Cargo.toml` declaring a path-dependency on `<tmp>/tests/fixtures/dep-lib/Cargo.toml`. Scan with no flag → assert the emitted SBOM contains a `dependencies` entry from `main-app` → `dep-lib` (or a `DEPENDS_ON` relationship per the CDX 1.6 emission shape). Scan with `run_scan(<tmp>, &["tests/fixtures"])` → assert the emitted SBOM contains `main-app` but the dependency edge pointing AT `dep-lib` is absent (no dangling `DEPENDS_ON` reference to a missing component). Reuses `component_names` + a new small helper `find_dependency_edges(sbom, from_purl)` if needed.

- [X] T004 [US1] Append integration test `scan_root_excluded_yields_only_metadata_component` to `mikebom-cli/tests/exclude_path_integration.rs` per FR-004. Synthesize a fixture with one Cargo project at `<tmp>/some-app/`. Resolve `<tmp>` to an absolute path. Scan with `run_scan(<tmp>, &[<tmp-absolute-path>])` → assert the emitted SBOM contains ONLY `metadata.component` (mikebom's own self-description) and the `components[]` array is empty (no project components emitted). Verifies every per-ecosystem walker correctly short-circuits when the scan root itself is excluded.

**Checkpoint**: After T001-T004, US1's per-ecosystem regression coverage is in place. Run `cargo test --test exclude_path_integration` and confirm 9/9 pass (5 existing + 4 new).

---

## Phase 4: User Story 2 — Complex pattern combinations + cross-platform separators (Priority: P2)

**Goal**: `--exclude-path` correctly handles two distinct patterns in one scan AND backslash-separated literal entries normalize to the forward-slash form. Both edge cases have integration tests pinning the behavior.

**Independent Test**: Run `cargo +stable test --test exclude_path_integration` and verify the two new US2 tests pass.

### Implementation for User Story 2

- [X] T005 [US2] Append integration test `multiple_pattern_entries_combine_by_union` to `mikebom-cli/tests/exclude_path_integration.rs` per FR-005. Synthesize a fixture with both a `<tmp>/services/payment/testdata/cargo/fixture-cargo/` subtree AND a `<tmp>/services/payment/_archive/cargo/legacy-app/` subtree. Scan with `run_scan(<tmp>, &["**/testdata", "**/_archive"])` → assert components from BOTH subtree shapes are suppressed in the same scan (neither `fixture-cargo` nor `legacy-app` appears in `component_names`). Verify the existing milestone-113 `mikebom:exclude-path` envelope annotation lists both pattern entries.

- [X] T006 [US2] Append integration test `cross_platform_separator_normalization` to `mikebom-cli/tests/exclude_path_integration.rs` per FR-006. Synthesize a fixture with `<tmp>/real-app/` + `<tmp>/tests/fixtures/fixture-app/`. Scan with `run_scan(<tmp>, &["tests\\fixtures"])` (backslash-separated literal). Assert `component_names` contains `real-app` but NOT `fixture-app` — the backslash form normalizes to the same suppression as forward-slash. The test runs on any host (Linux/macOS/Windows) because the normalization happens at parse time in `ExclusionSet::push_entry()`; if the test FAILS on PR-open (i.e., the parser doesn't currently normalize), that's a real bug — fix it in `exclude_path.rs::push_entry()` as part of this PR per spec Assumption #3.

**Checkpoint**: After T005-T006, US2 edge-case coverage is in place. Run `cargo test --test exclude_path_integration` and confirm 11/11 pass (5 existing + 4 US1 + 2 US2).

---

## Phase 5: User Story 3 — Discoverability + observability + perf assurance (Priority: P3)

**Goal**: New operators can discover `--exclude-path` from CLI help + docs cross-links; operators using the flag see a scan-end tracing summary; maintainers can verify ≤1.10× perf overhead via opt-in benchmark.

**Independent Test**: (a) `mikebom sbom scan --help` stdout contains `--exclude-path` and the user-guide pointer (T010); (b) `docs/ecosystems.md` has the consolidated section + per-ecosystem pointers (T011); (c) `cargo test --test exclude_path_perf -- --ignored` passes the budget assertion on Linux (T012); (d) running mikebom with `--exclude-path` emits the new stderr summary fields (T007+T008+T009 combined).

### Implementation for User Story 3

- [X] T007 [US3] Extend `ExclusionSet` at `mikebom-cli/src/scan_fs/package_db/exclude_path.rs:108-228` per data-model.md § Entity 1 + Entity 2. Add `pub(crate) suppressed_dirs: std::sync::atomic::AtomicUsize` field (initialized to 0 in the constructor at `exclude_path.rs:130-150`). Add two accessor methods: `pub(crate) fn count_literals(&self) -> usize` (wraps `self.literal_paths.len()`) and `pub(crate) fn count_patterns(&self) -> usize` (filters `self.entries` for `Pattern` variant per the existing entry enum). Verify the existing `is_empty()` method's short-circuit logic still applies (`literal_paths.is_empty() && pattern_set.is_none()`). The AtomicUsize uses `Relaxed` ordering per Decision 1 / data-model invariant 4.

- [X] T008 [US3] Extend `walk.rs` at line 224-235 per data-model.md § Entity 1 lifecycle step 2. After the existing `if cfg.exclude_set.matches(&rel_str)` check at line 227 returns `true`, BEFORE the `tracing::debug!` skip-event emission at lines 228-232, increment the counter via `cfg.exclude_set.suppressed_dirs.fetch_add(1, std::sync::atomic::Ordering::Relaxed)`. Keep the existing `tracing::debug!` line unchanged. The counter increment is free in the no-flag path because the parent `if cfg.exclude_set.is_empty() { return }` short-circuit at line 224 prevents reaching this code at all.

- [X] T009 [US3] Extend the scan-end `tracing::info!` at `mikebom-cli/src/cli/scan_cmd.rs:1750-1754` per contracts/tracing-summary.md. Wrap the existing single `tracing::info!` call in an `if !exclude_set.is_empty() { ... } else { ... }` branch. The empty-arm preserves the existing two-field shape byte-identically. The non-empty arm extends with four new fields: `excluded_entries = exclude_set.entries().len()`, `excluded_literals = exclude_set.count_literals()`, `excluded_patterns = exclude_set.count_patterns()`, `suppressed_dirs = exclude_set.suppressed_dirs.load(std::sync::atomic::Ordering::Relaxed)`. The message string ("scan complete") stays unchanged in both arms.

- [X] T010 [US3] Create new integration test file `mikebom-cli/tests/exclude_path_help_text.rs` per FR-008 / contracts/perf-bench.md is the wrong reference — this test's setup is in plan.md § "Source Code" project structure. Single test function `help_text_documents_exclude_path`: invokes `std::process::Command::new(env!("CARGO_BIN_EXE_mikebom")).arg("sbom").arg("scan").arg("--help").output()`, asserts `output.status.success()`, parses `String::from_utf8(output.stdout)`, asserts the substring `--exclude-path` is present AND the substring `cli-reference` (or `cli-reference.md`) is present. The test does NOT assert exact wording — operators can rewrite the description without breaking the test per FR-008. The pointer is already in the clap doc-comment at `main.rs:140`; the test simply asserts the existing behavior.

- [X] T011 [US3] Edit `docs/ecosystems.md` per FR-007 / Decision 6 (research.md). Add a new `## Directory exclusion (--exclude-path)` cross-cutting section between the `## Coverage matrix` (currently around line 19) and the per-ecosystem sections (starting `## apk` around line 22). Section content: a 2-3 paragraph operator-facing explanation covering literal vs pattern matching, the `MIKEBOM_EXCLUDE_PATH` env var, built-in skip-list precedence, and a deep-link to `docs/user-guide/cli-reference.md#--exclude-path-path_or_pattern` for the troubleshooting matrix. Then in each ecosystem section (cargo, maven, gem, pip, npm, gradle, nuget, yocto, golang — survey the file at edit time to confirm the full ecosystem list), add the identical-wording pointer line in the "Known limitations" sub-section (or as a leading bullet if no Known limitations exists): `**Path exclusion**: see [Directory exclusion (--exclude-path)](#directory-exclusion---exclude-path).` per Decision 6. Verify presence with a one-shot `grep -c '#directory-exclusion---exclude-path' docs/ecosystems.md` returning the per-ecosystem-section count + 1 (the consolidated section's own slug).

- [X] T012 [US3] Create new opt-in perf benchmark file `mikebom-cli/tests/exclude_path_perf.rs` per FR-011 / contracts/perf-bench.md. Single test function `exclude_path_does_not_exceed_1_10x_baseline` with `#[test] #[ignore = "wall-clock perf test — opt in via `cargo test -- --ignored`; runs on Linux CI lane only per milestone-094"]`. Resolve fixture via `let fixture = PathBuf::from(env!("MIKEBOM_FIXTURES_DIR")).join("kusari-cli");`. Implement `time_scan(fixture, exclude_paths)` per contracts/perf-bench.md § "Per-sample timing". Implement `median(samples)` per contracts/perf-bench.md § "Median computation". Collect 5 baseline samples + 5 with-flag samples (`--exclude-path '**/testdata'`). On Linux: assert `excluded_median <= baseline_median.mul_f64(1.10)` with a descriptive failure message. On macOS: print measurements to stderr (per contracts/perf-bench.md § "macOS skip pattern") and skip the strict assertion via `cfg!(target_os = "macos")` early-return.

**Checkpoint**: After T007-T012, US3's full instrumentation + docs + perf assurance are in place. Verify locally: (a) run mikebom with `--exclude-path` and confirm the new stderr fields appear (`excluded_entries=N`, `excluded_literals=N`, `excluded_patterns=N`, `suppressed_dirs=N`); (b) run mikebom without `--exclude-path` and confirm stderr is byte-identical to pre-118 (the two-field "scan complete" line); (c) run `cargo test --test exclude_path_help_text` and confirm pass; (d) read `docs/ecosystems.md` and confirm the consolidated section + per-ecosystem pointers are present; (e) run `cargo test --test exclude_path_perf -- --ignored` and confirm pass on Linux.

---

## Phase 6: Polish

- [X] T013 Run `MIKEBOM_SKIP_DOCKER_INTEGRATION=1 ./scripts/pre-pr.sh` from the repo root. Verify clippy `--workspace --all-targets -D warnings` passes clean AND `cargo +stable test --workspace` passes clean (every suite `ok. N passed; 0 failed`). Per CLAUDE.md this is MANDATORY before opening any PR.

- [X] T014 Update `specs/118-exclude-path-polish/tasks.md` (this file) marking T001–T013 as `[X]` completed.

- [X] T015 Commit per CLAUDE.md commit protocol. Commit title: `feat(scan_fs): --exclude-path polish — per-ecosystem tests + tracing summary + perf bench (milestone 118, closes #343)`. Commit body summarizes: (a) 4 new US1 integration tests for golang source + go binary + dep-edge + scan-root edge cases; (b) 2 new US2 integration tests for multi-pattern union + cross-platform separator; (c) ExclusionSet AtomicUsize counter + accessor helpers; (d) walk.rs increment site; (e) scan_cmd.rs tracing::info! extension (preserves byte-identity when --exclude-path unused); (f) help-text discoverability test; (g) consolidated cross-cutting section in docs/ecosystems.md + per-ecosystem pointers; (h) opt-in perf benchmark on kusari-cli fixture with ≤1.10× budget. NO `--no-verify`.

- [X] T016 Open PR. Title: `feat(scan_fs): --exclude-path polish — per-ecosystem tests + tracing summary + perf bench (milestone 118, closes #343)`. Body includes: (1) issue #343 link; (2) `## Summary` listing the 8 changes (4 US1 tests + 2 US2 tests + counter machinery + help-text + docs + perf bench); (3) `## Test plan` listing the spec's 11 acceptance scenarios as manually-verified-on-this-PR checklist items; (4) `cargo +stable test --test exclude_path_perf -- --ignored` output snippet showing the Linux baseline + excluded measurements + ratio (proof the budget holds in practice).

---

## Dependencies & Execution Order

```text
                Phase 3 (US1, P1)                Phase 4 (US2, P2)              Phase 5 (US3, P3)
                ─────────────────                ─────────────────              ─────────────────
                T001 → T002 → T003 → T004        T005 → T006                    T007 → T008 → T009
                (all extend                       (all extend                     (production code chain
                exclude_path_integration.rs)     exclude_path_integration.rs)   on shared mod path)

                                                                                  T010 [P] (help-text test, new file)
                                                                                  T011 [P] (docs/ecosystems.md, independent)
                                                                                  T012 [P] (perf bench, new file)
                                                                                  
                                                          ↓
                                                  Phase 6 (Polish)
                                                  ───────────────
                                                  T013 → T014 → T015 → T016
```

**Sequential within each US phase**: T001-T004 (same file), T005-T006 (same file), T007 → T008 → T009 (production code chain: counter exists before increment exists before read).
**Parallel branches**: T010, T011, T012 (independent files: new test file, docs edit, new perf-bench file). All three can land concurrently after T007 + T008 + T009 are in (T010 doesn't need them; T011 doesn't need them; T012 doesn't need them — but in practice they ship in the same PR).

## Parallel Opportunities

Within US3's Phase 5, three tasks operate on different files with no shared state:

```text
# After T007 + T008 + T009 (the production-code chain), three independent tasks:
T010 [US3] — new file mikebom-cli/tests/exclude_path_help_text.rs
T011 [US3] — edit docs/ecosystems.md
T012 [US3] — new file mikebom-cli/tests/exclude_path_perf.rs
```

No other parallel opportunities exist — within US1 + US2 the tests live in the same file (sequential); within US3 production-code path the counter + increment + read are sequential by data flow.

## Independent Test Criteria

Per the spec's three user stories:

- **US1 (P1) — MVP**: Confirmed by T001-T004 all passing in `cargo test --test exclude_path_integration` (4 new tests alongside the 5 existing milestone-113 tests).
- **US2 (P2)**: Confirmed by T005-T006 passing in the same test suite.
- **US3 (P3)**: Confirmed by (a) T010 passing; (b) T011's docs grep returning ≥10 hits (one consolidated section + ≥9 per-ecosystem pointers); (c) T012 passing on Linux with `--ignored`; (d) manual stderr inspection showing the new fields when `--exclude-path` is used and the two-field byte-identity when it isn't.

## Implementation Strategy

**Single-PR ship**: T001 → T016 in one PR. The whole feature is one milestone-113 polish bundle; ~450 LoC production + tests + ~80 LoC docs estimate from plan.md § Scale.

**MVP scope**: T001-T004 + T013-T016. If review feedback pushes back on diff size mid-implementation, T012 (perf benchmark) is the spec's documented cut-point per the Assumptions section — defer it to a follow-up issue without blocking the rest.

**Format validation**: All 16 tasks above use the required checklist format — `- [ ]` checkbox + sequential ID (T001…T016) + optional [P] marker + [US1]/[US2]/[US3] label for user-story tasks (Setup + Foundational + Polish have no story label) + description with exact file path(s).
