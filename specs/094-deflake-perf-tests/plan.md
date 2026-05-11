# Implementation Plan: Deflake wall-clock perf tests (architectural fix, not threshold-bump)

**Branch**: `094-deflake-perf-tests` | **Date**: 2026-05-11 | **Spec**: [spec.md](spec.md)

## Summary

Two wall-clock perf tests (`triple_format_perf.rs`, `dual_format_perf.rs`) inherit-recur the same flakiness pattern: median-of-N filter doesn't absorb shared-CI-runner thermal/scheduler noise, so they keep false-failing on macOS-latest at ~14–22% measured-reduction vs a 25% gate. The "fix" treadmill (median-3 → median-5 → another fail at 22.9%) tightens parameters without fixing the structural problem.

**Approach**: stop blocking PRs on wall-clock perf entirely. Three changes land together:

1. **Mark both perf tests `#[ignore]`** so they skip in `cargo +stable test --workspace`. Existing assertions / sample-counts / threshold values stay (FR-007); only the default-lane execution is removed.
2. **Add a NEW deterministic structural-correctness test** in the default lane that uses the existing `tracing::info!(... "scan starting")` log line at `scan_cmd.rs:1413` as a side-channel signal. Counts log occurrences in `mikebom sbom scan` stderr to assert "scan pipeline runs exactly ONCE for triple-format, exactly THREE TIMES for 3-sequential". Plus a byte-equivalence check between triple-format outputs and 3-single-format outputs (after normalization). Zero wall-clock semantics; pass/fail is binary.
3. **Add a dedicated opt-in CI lane** at `.github/workflows/perf.yml` triggered by (a) `pull_request` with `perf` label, (b) `workflow_dispatch` (manual), (c) `schedule` cron nightly on main. Runs both perf tests with 3-attempt retry on each test (via `nick-fields/retry@v3`, SHA-pinned per milestone-094 conventions). NOT required for PR merge.

**Constitution-friendly**: zero production code change (FR-008), zero golden regen (FR-009), zero new Cargo deps (FR-010), no new `mikebom:*` properties.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited; no nightly required for this user-space test-infrastructure work).
**Primary Dependencies**: existing only — `std::process::Command` for subprocess invocation, `tempfile`, `tar` (already used by current perf tests), `serde_json` (for byte-equivalence comparisons). The new GitHub Action `nick-fields/retry@v3` is permitted under FR-010 because it's a workflow YAML reference, not a Cargo dependency.
**Storage**: N/A — test infrastructure only; no caches, no persistence.
**Testing**: `./scripts/pre-pr.sh` (mandatory pre-PR gate, must skip perf tests now) + the new opt-in `cargo +stable test --workspace -- --ignored --test-threads=1` (matches the perf.yml workflow invocation).
**Target Platform**: same as workspace — Linux x86_64, macOS aarch64, Linux aarch64. The new perf CI lane runs on `ubuntu-latest` + `macos-latest` (matching existing default-lane platforms) so per-platform perf characteristics stay visible.
**Project Type**: Rust workspace, test-infrastructure refactor. Not a software feature.
**Performance Goals**: N/A from a user perspective. The structural test should run in <5s (one mikebom invocation per format-pair, plus 3 sequential = 4 invocations × ~1s synthetic-fixture = ~4s).
**Constraints**: FR-007 (preserve existing assertion semantics — no median-N tuning, no threshold change in this milestone), FR-008 (no production code change), FR-009 (zero golden regen), FR-010 (zero new Cargo deps). The structural signal uses `tracing::info!` output already emitted by production code — no instrumentation hook added.
**Scale/Scope**: 2 existing test files modified (add `#[ignore]`); 1 NEW test file (`triple_format_structural.rs` — structural-correctness check + 3-single-format byte-equivalence); 1 NEW workflow file (`.github/workflows/perf.yml`); 2 docs files updated (CONTRIBUTING.md + PR template). Total diff target: <600 lines.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

- **I. Pure Rust, Zero C**: ✅ no FFI, no C deps, no toolchain change.
- **II. eBPF-Only Observation**: N/A — no scan-discovery behavior changes; eBPF lane continues unchanged.
- **III. Fail Closed**: ✅ no runtime behavior changes; only test execution is moved between lanes.
- **IV. Type-Driven Correctness / no `.unwrap()` in production**: ✅ no Rust production source touched. Test code may use `.unwrap()` per existing convention.
- **V. Specification Compliance / standards-native precedence**: ✅ no `mikebom:*` properties involved; no SBOM emission affected.
- **VI. Three-Crate Architecture**: ✅ no crate boundaries touched.
- **VII. Test Isolation**: ✅ this milestone refines test-isolation discipline. The default lane gets MORE deterministic; wall-clock-dependent tests move to opt-in. Aligned with Principle VII's spirit.
- **VIII. Completeness**, **IX. Accuracy**, **X. Transparency**, **XI. Enrichment**, **XII. External Data Source Enrichment**: N/A — no SBOM output touched.

**No violations.** No Complexity Tracking entry needed.

## Project Structure

### Documentation (this feature)

```text
specs/094-deflake-perf-tests/
├── plan.md                          # This file
├── research.md                      # Phase 0: log-signal verification, opt-in mechanism, retry pattern
├── data-model.md                    # Phase 1: per-deliverable shape (test bodies, workflow shape)
├── contracts/
│   └── deflake-contracts.md         # Phase 1: assertion shapes + workflow trigger conditions
├── quickstart.md                    # Phase 1: maintainer recipes (apply, verify, ship)
├── checklists/
│   └── requirements.md              # 16/16 pass — already complete
└── spec.md                          # Feature spec
```

### Source Code (repository root)

```text
mikebom/
├── mikebom-cli/tests/
│   ├── triple_format_perf.rs                # MODIFIED: add #[ignore] to single test fn
│   ├── dual_format_perf.rs                  # MODIFIED: add #[ignore] to single test fn
│   │                                        # (Note: dual_format_perf is also included as
│   │                                        # `mod dual_format_perf;` from holistic_parity.rs;
│   │                                        # #[ignore] applies in both inclusion contexts.)
│   └── triple_format_structural.rs          # NEW: deterministic structural-correctness test
├── .github/workflows/
│   └── perf.yml                             # NEW: opt-in/scheduled perf lane with retry
├── .github/pull_request_template.md         # MODIFIED: add perf-touching-PR note (FR-006)
└── CONTRIBUTING.md                          # MODIFIED: document the default-vs-opt-in split (FR-006)
```

**Structure Decision**: 5 file changes total (2 modified test files + 1 new test + 1 new workflow + 2 docs). All under repo root or `.github/`/`mikebom-cli/tests/`. Zero source-tree changes (`mikebom-cli/src/`, `mikebom-common/src/`, `xtask/src/` all untouched per FR-008).

## Complexity Tracking

> Not applicable — no Constitution gate violations.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|--------------------------------------|
| (none)    | (none)     | (none)                               |

## Phase Plan

### Phase 0 — Research (`research.md`)

Three decision points resolved:

1. **Structural-signal mechanism** — confirmed via grep of `scan_cmd.rs:1413`: `tracing::info!(root = %..., "scan starting")` fires exactly once per scan-pipeline invocation. Stderr-grep `"scan starting"` count = number of pipeline invocations. Zero production code change needed.
2. **Opt-in UX** — picked `#[ignore]` + `cargo test --workspace -- --ignored` as the opt-in mechanism. Standard Rust convention; works with `cargo test`'s built-in filtering; no env var dance needed. The pre-PR gate inherits the default-skip behavior because cargo's default test invocation skips ignored tests.
3. **Retry mechanism** — picked `nick-fields/retry@v3` (SHA-pinned) for the perf.yml workflow. 3 attempts per perf test; the workflow_dispatch / scheduled trigger surfaces the final result.

### Phase 1 — Design (`data-model.md`, `contracts/`, `quickstart.md`)

- **data-model.md** — per-file shape: existing perf tests (1-line `#[ignore]` addition each), new structural test (3 test fns: triple-format-fires-once, sequential-fires-three-times, output-byte-equivalence), perf.yml workflow (3 trigger types × 2 jobs × retry-wrapped steps).
- **contracts/deflake-contracts.md** — concrete invariants per test + workflow contract for the perf lane.
- **quickstart.md** — maintainer recipes: apply the ignore attrs; verify the structural test passes 100× locally; verify the perf lane runs on `perf` label.

Re-evaluate Constitution Check post-design: still no violations expected (pure test infrastructure).

### Phase 2 — Tasks

Out-of-scope for `/speckit.plan`; will be generated by `/speckit.tasks`.

## Agent Context Update

The agent-context update script will be re-run after Phase 1; this milestone adds no new technology surface (one new GitHub Action workflow dep, no Cargo changes), so the agent context delta should be empty or trivial.
