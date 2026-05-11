# Feature Specification: Deflake wall-clock perf tests (architectural fix, not threshold-bump)

**Feature Branch**: `094-deflake-perf-tests`
**Created**: 2026-05-11
**Status**: Draft
**Input**: User description: "yes let's fix flaky perf-tests"

## Background

Two wall-clock perf tests in the workspace inherit-recur the same flakiness pattern:

- `mikebom-cli/tests/triple_format_perf.rs::triple_format_is_at_least_25_percent_faster_than_three_sequential_scans` (milestone 011 SC-007)
- `mikebom-cli/tests/dual_format_perf.rs` (milestone 010 SC-009, presumed similar)

Both assert that an N-format combined scan completes in **≥25% less wall-clock time** than N sequential single-format scans, against a ~2-second synthetic fixture. They run on every PR in `cargo +stable test --workspace`, on macOS-latest + linux-x86_64 + (linux-x86_64 --features ebpf-tracing) lanes.

**History of the treadmill**:
- milestone 011 (origin): median-of-3, 30% spec target, 25% CI gate
- milestone 045 (first deflake): bumped to median-of-5 after two macOS CI fails (14.4%, 19.9%). The fix's own doc-comment notes "local distribution sits around 50%"
- 2026-05-10 (PR #194's CI): triple_format failed again at 22.9% on macOS-latest. The user merged despite the failure because the test gates the merge button on every PR and the failure isn't a real regression — just CI runner noise.

Each "fix" cycle bumps a parameter (sample count up, threshold down) without addressing the architectural root cause: **wall-clock perf assertions don't belong in a per-PR CI lane that blocks merges**. Median-of-N is a noise filter; it doesn't reduce the underlying variance from CI-runner thermal throttling, concurrent-test CPU contention, page-cache jitter, or macOS-latest scheduler quirks. With ~2 seconds of total work per measurement and ~10s of total test runtime, a single 200ms outlier on one of the 5 triple-format samples can dominate the median.

The user's frustration is explicit and the cause is structural. The fix is to **stop blocking PRs on wall-clock perf assertions** and instead:
1. Run a deterministic structural-correctness check on every PR (asserts single-pass dispatch, not wall-clock — fast, byte-stable, catches real regressions in the single-invocation amortization logic).
2. Move the wall-clock perf tests to a dedicated opt-in/scheduled CI lane that can retry, run on dedicated runners, and not gate per-PR merges.

This is the standard pattern in mature Rust projects (rust-lang/rust's nightly perf tracker, cargo's `criterion`-based benchmarks gated behind `--release` + `bench` profile, tokio's perf-CI lane). It preserves the regression-catch surface while removing the merge-blocking flakiness.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Contributor opens a PR; CI does not fail on wall-clock noise (Priority: P1)

A contributor (any contributor — maintainer or external) pushes a PR with a code change unrelated to performance. CI runs `cargo +stable clippy --workspace --all-targets -- -D warnings` and `cargo +stable test --workspace` across the 3 lanes. The two perf tests are skipped by default; no failure due to CI-runner timing noise occurs.

**Why this priority**: The user-visible pain point is "I have to merge anyway despite perf-test failures". P1 because every other quality improvement in this milestone is downstream of CI no longer false-failing.

**Independent Test**: open a no-op PR (e.g., README typo fix). All three CI lanes pass cleanly. `cargo +stable test --workspace` locally also passes cleanly without the perf tests running.

**Acceptance Scenarios**:

1. **Given** a contributor pushes any PR, **When** the default CI lanes execute `cargo +stable test --workspace`, **Then** the triple_format_perf and dual_format_perf tests are NOT executed in the default test run (they are skipped or filtered out).
2. **Given** the pre-PR gate (`./scripts/pre-pr.sh`), **When** a contributor runs it locally, **Then** the perf tests are also skipped by default (matches CI), and the gate completes with the same `0 failed` semantics.
3. **Given** 5 consecutive PRs are opened over a single day, **When** all CI runs complete, **Then** zero merge-blocking failures occur from the two perf tests, even on macOS-latest (the noisiest lane).

---

### User Story 2 — Maintainer can still verify perf hasn't regressed (Priority: P1)

A maintainer wants to confirm that a refactor or new feature hasn't regressed the single-pass amortization win that SC-007 / SC-009 originally promised. They have a way to run the wall-clock perf check on demand — locally, via a labeled CI run, or via a scheduled lane — and get the same signal as before (or better).

**Why this priority**: Co-equal to US1. Removing the perf test from the default lane CAN'T mean losing the regression-catch surface. If we lose the signal, we'd accumulate silent regressions and the original SC-007 / SC-009 spec gets gutted. P1 because both US1 (no false failures) and US2 (still catch real regressions) are necessary for the architectural fix to land.

**Independent Test**: a maintainer adds a deliberately-regressing change (e.g., disable the single-pass dispatch in `serialize_all`, forcing 3 full scans even in triple-format mode) on a test branch. They run the perf test on the dedicated lane / on-demand mechanism. The test fails with a clear message indicating ≥25% reduction is no longer met.

**Acceptance Scenarios**:

1. **Given** a maintainer wants to validate perf locally, **When** they invoke the documented opt-in command (e.g., `cargo test --workspace -- --ignored` or `MIKEBOM_RUN_PERF_TESTS=1 ./scripts/pre-pr.sh`), **Then** both perf tests run and produce the same wall-clock measurements with the same CI gate semantics as today.
2. **Given** a deliberate regression that disables single-pass dispatch, **When** the perf test runs (on any platform), **Then** the test FAILS with a clear assertion message identifying the regression (reduction <25%).
3. **Given** a maintainer wants periodic regression detection without per-PR overhead, **When** the scheduled / labeled CI lane runs, **Then** it executes both perf tests with built-in retry (e.g., up to 3 attempts) and reports overall pass/fail.

---

### User Story 3 — A structural-correctness test catches single-pass dispatch regressions deterministically (Priority: P2)

The original spec intent of SC-007 / SC-009 is "a single invocation produces all N formats by amortizing the scan + deep-hash + layer-walk work". The wall-clock test is an *indirect proxy* for that intent — it's measuring "did the work happen once vs N times" by measuring how long it took. A structural-correctness test can measure the same intent **directly and deterministically**, without wall-clock semantics.

**Why this priority**: This is the layered defense — it runs on every PR (replacing the wall-clock perf test in the default lane) and catches any change that breaks single-pass dispatch in a binary pass/fail way. P2 because US1 + US2 together would still work without US3 (perf tests opt-in lane catches regressions periodically), but US3 closes the per-PR window where a regression could ship.

**Independent Test**: a maintainer adds a deliberate regression (same as US2's regression scenario). The new structural-correctness test (running in the default lane) fails immediately on the regression PR, with a clear message about the single-pass dispatch invariant being violated.

**Acceptance Scenarios**:

1. **Given** the default CI lane (`cargo +stable test --workspace`), **When** it runs against current main, **Then** a new structural-correctness test passes deterministically (no flakiness signal in the assertion — it's pass/fail on the structural invariant, not wall-clock).
2. **Given** a change that breaks single-pass dispatch (e.g., the serializer is called 3 times instead of 1 for triple-format output), **When** the default CI lane runs, **Then** the structural-correctness test FAILS with an assertion message identifying which invariant broke (e.g., "expected 1 scan-pipeline invocation per triple-format request; observed 3").
3. **Given** the new structural test runs back-to-back 100 times locally (`cargo test --workspace -- --test-threads=1 ${{ test_name }}`), **When** all runs complete, **Then** all 100 pass with no flakiness. The test is deterministic by construction.

---

### Edge Cases

- **The opt-in lane needs perf-grade hardware**: macOS-latest GitHub Actions runners are noisy (shared with thousands of other repos). On the opt-in lane, retry-on-fail (≥3 attempts) absorbs most noise; running on a dedicated runner or self-hosted runner is a future option.
- **A real perf regression sneaks in via a PR without the opt-in lane firing**: the scheduled nightly run on `main` catches it within ~24h. The opt-in label provides a synchronous gate for performance-touching PRs.
- **The structural-correctness test passes but the wall-clock perf actually regresses by some small amount** (e.g., the per-invocation fixed overhead doubles): structural test doesn't catch this. The opt-in / scheduled wall-clock lane is the safety net for these cases. Documented tradeoff.
- **A contributor disables an opt-in via env var locally and the PR breaks the perf gate**: the pre-PR gate (`./scripts/pre-pr.sh`) should NOT auto-enable the perf lane (matches CI default). Contributors opting in to the perf lane MUST be deliberate.
- **dual_format_perf vs triple_format_perf divergence**: both tests have the same root cause; both should be moved together to the opt-in lane. No partial fix.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Both `triple_format_perf.rs` and `dual_format_perf.rs`'s wall-clock perf tests MUST be skipped by default in `cargo +stable test --workspace`. The standard mechanism is `#[ignore]` on the test functions.
- **FR-002**: The pre-PR gate (`./scripts/pre-pr.sh`) MUST also skip these tests by default. No env var opt-in required to match CI behavior; the gate's `cargo test --workspace` invocation skips ignored tests automatically.
- **FR-003**: A documented opt-in mechanism MUST exist for maintainers to run the wall-clock perf tests on demand. Specifically: `cargo +stable test --workspace -- --ignored` MUST run both perf tests with the same assertions as today (median-of-5, 25% CI gate).
- **FR-004**: A dedicated CI lane MUST run both perf tests on a schedule (nightly on `main`) AND on PR opt-in (via a `perf` label OR via a manually-triggered workflow_dispatch). The dedicated lane MUST run `cargo +stable test --workspace -- --ignored --test-threads=1` with built-in 3-attempt retry on failure of each perf test. The lane MUST NOT be required for PR merge.
- **FR-005**: A NEW structural-correctness test MUST be added to the default test suite that asserts the single-pass amortization invariant **without using wall-clock timing**. It MUST run on every PR via the default `cargo +stable test --workspace` invocation. The exact invariant assertion shape (e.g., counting the number of scan-pipeline invocations, checking for a single output-dispatch trace event, comparing emitted SBOM byte-equivalence between triple-format and concatenated-single-formats outputs) is an implementation choice deferred to the planning phase — but the test MUST fail deterministically when single-pass dispatch is broken.
- **FR-006**: `CONTRIBUTING.md` MUST be updated to document the split: default gate is correctness + the new structural test; perf wall-clock tests are opt-in via `-- --ignored`. The PR template (`.github/pull_request_template.md`) MUST add a note that perf-touching PRs should opt in to the perf lane.
- **FR-007**: The two perf tests' source files MUST retain their existing assertions, sample sizes, and threshold values. This milestone moves them out of the default lane; it does NOT weaken the assertions. Future tuning (e.g., sample-size bumps, threshold adjustments) is out of scope.
- **FR-008**: No production code change. `mikebom-cli/src/` MUST NOT be modified. The only edits are to: the two perf-test source files (adding `#[ignore]`), CI workflow files (adding the dedicated lane), one new test file (the structural-correctness check), CONTRIBUTING.md, and `.github/pull_request_template.md`.
- **FR-009**: Zero byte-identity golden regen. `git status mikebom-cli/tests/fixtures/golden/` MUST be empty after the change lands.
- **FR-010**: No new Cargo dependencies. `Cargo.lock` MUST be unchanged.

### Key Entities

- **`triple_format_perf.rs` test function**: the existing milestone-011 perf test. State change: gains `#[ignore]` attribute. Behavior preserved when explicitly run.
- **`dual_format_perf.rs` test function**: same pattern, milestone 010.
- **Structural-correctness test** (NEW, file path TBD at planning): deterministic pass/fail test of the single-pass invariant.
- **Dedicated perf CI lane** (NEW, in `.github/workflows/`): scheduled + label-gated workflow. Runs both perf tests with retry.
- **`CONTRIBUTING.md`** + **`.github/pull_request_template.md`**: documentation updates.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Over the next 10 consecutive PRs (any author, any size), zero perf-test failures occur in the default CI lanes (`Lint + test (linux-x86_64)`, `Lint + test (macos-latest)`, `Lint + test (linux-x86_64, --features ebpf-tracing)`). Measured by querying `gh run list` for the workflow runs post-merge and confirming zero failures attributable to `triple_format_perf` or `dual_format_perf`.
- **SC-002**: A maintainer can opt into the wall-clock perf test on demand and get a result within 60 seconds of invocation (matching today's test runtime for triple_format_perf — median-of-5 = ~10s per test × 2 tests = ~20s plus per-invocation overhead).
- **SC-003**: A deliberate single-pass-dispatch regression (e.g., setting a hypothetical `force_per_format_scan: true` config) is caught BY the new structural-correctness test in the default lane on the PR that introduces it — within the same CI run, no scheduled-lane delay.
- **SC-004**: The dedicated perf CI lane catches a deliberate amortization regression (e.g., adding a 500ms `sleep` to each scan invocation that wouldn't affect single-pass count but would affect per-invocation wall-clock) within 24 hours of the regression landing on `main`, via the scheduled nightly run.
- **SC-005**: Pre-PR gate runs clean post-changes: `./scripts/pre-pr.sh` reports `>>> all pre-PR checks passed.` with zero clippy warnings and zero test failures across the workspace. The two perf tests are skipped (not failed, not unexpectedly run).
- **SC-006**: The new structural-correctness test runs deterministically: 100 consecutive `cargo test ${{ structural_test_name }}` invocations locally produce 100 passes, zero flakes. Verified once during implementation as a `for i in $(seq 1 100); do ... done` loop.
- **SC-007**: Diff scope audit: 100% of changed files match the allowlist {`mikebom-cli/tests/triple_format_perf.rs`, `mikebom-cli/tests/dual_format_perf.rs`, `mikebom-cli/tests/<new_structural_test>.rs`, `.github/workflows/*.yml`, `.github/pull_request_template.md`, `CONTRIBUTING.md`}. Zero changes under `mikebom-cli/src/`, `mikebom-common/src/`, `xtask/src/`, `mikebom-cli/tests/fixtures/golden/`, `Cargo.lock`.

## Assumptions

- Both perf tests have the same root-cause flakiness pattern (wall-clock variance on shared CI runners). dual_format_perf may not have had as many observed failures as triple_format_perf, but moving both together is the right call — splitting them would leave a known flakiness pattern in place for one of them.
- The dedicated perf CI lane uses GitHub Actions (matches the existing CI infrastructure). A future improvement could be self-hosted runners with dedicated CPU pinning, but that's out of scope here.
- Retry-on-failure logic in the dedicated lane is implemented via the standard GitHub Actions retry pattern (a step-level `continue-on-error` + a retry-action like `nick-fields/retry@v3`, OR via `cargo test`'s built-in retry mechanisms if available, OR via a small wrapper script). Implementation choice deferred to planning.
- A `perf` label on a PR triggers the opt-in lane via a `pull_request: types: [labeled]` workflow trigger. Standard GitHub Actions pattern.
- The "structural-correctness test" is implementable. Concrete approaches (any one suffices):
   - Add a `MIKEBOM_PERF_INSTRUMENT=1` env var that causes the scan pipeline to emit a counter on stderr or a sidecar file (e.g., "scan_pipeline_invocations: 1"), then the test parses that.
   - Use the existing `tracing` infrastructure to emit a span event per scan invocation, capture stderr, count spans.
   - Compare emitted SBOMs across triple-format and 3-single-format invocations for byte-equivalence (after normalization) AND assert that triple-format's total subprocess CPU time is less than the sum of the 3 single-format invocations (proxy via `resource::getrusage` or similar; less wall-clock-sensitive).
- The opt-in mechanism's exact UX (`-- --ignored` vs `--features perf-tests` vs env var) is a planning-phase choice. Any approach that satisfies FR-003 is acceptable.

## Dependencies

- The existing perf tests at `mikebom-cli/tests/{dual,triple}_format_perf.rs` (milestones 010 + 011) — this milestone moves them, doesn't replace them.
- `./scripts/pre-pr.sh` — must continue to exit clean post-change (FR-002 + SC-005).
- The existing CI workflows at `.github/workflows/ci.yml` — must continue to pass after the perf tests stop running in them.
- The new dedicated perf workflow will be added at `.github/workflows/perf.yml` (or equivalent name).

## Out of Scope

- **Replacing GitHub Actions runners with self-hosted dedicated perf hardware**: longer-term improvement; can run on the same ubuntu-latest / macos-latest until perf-lane signal quality demands more.
- **Tuning the assertion thresholds (25% gate, 30% spec target)**: FR-007 explicitly forbids weakening assertions in this milestone. Adjusting these is a separate decision, ideally driven by observed perf-lane signal over multiple weeks.
- **Adding `criterion`-based microbenchmarks**: would shift mikebom toward bench-mark-as-test pattern. Scope creep; can be a future milestone if the team wants finer-grained perf signal.
- **Removing the wall-clock perf tests entirely**: explicitly NOT proposed. The user wants the regression-catch surface preserved, just not blocking per-PR.
- **Investigating WHY macOS-latest runners are noisier than linux**: documented downstream; out of scope here. Just need the opt-in lane to absorb the noise via retry.
- **Threat-model documentation** (OSPS-SA-03.02 — left open from milestone-094 compliance work): tracked separately.
- **Branch-protection settings** (OSPS-AC-03.01/.02, OSPS-QA-03.01, OSPS-QA-07.01): operator-side GitHub Settings; tracked separately.
- **`generate_threat_model` MCP-tool retry**: separate effort.
