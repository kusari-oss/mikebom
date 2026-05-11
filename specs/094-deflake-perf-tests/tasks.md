---
description: "Task list for milestone 094 — deflake wall-clock perf tests"
---

# Tasks: Deflake wall-clock perf tests (architectural fix, not threshold-bump)

**Input**: Design documents from `/Users/mlieberman/Projects/mikebom/specs/094-deflake-perf-tests/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. This milestone IS a test-infrastructure refactor — adding `#[ignore]` to two existing perf tests, creating a new deterministic structural-correctness test, and verifying determinism via a 100-iteration local loop. No production code is written; verification is via existing test machinery + content-shape greps.

**Organization**: Tasks grouped by user story. US1 (P1) and US2 (P1) are co-equal in priority (eliminate false PR fails AND preserve regression-catch surface). US3 (P2) adds the deterministic sibling in the default lane. Stories are file-level independent (each owns a distinct file set) and can be implemented in any order or in parallel after Setup.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: User story this task belongs to (US1–US3)
- File paths are workspace-relative.

## Path Conventions

Two existing test files modified (`mikebom-cli/tests/{triple,dual}_format_perf.rs`), one new test file (`triple_format_structural.rs`), one new workflow (`.github/workflows/perf.yml`), two docs files (`CONTRIBUTING.md`, `.github/pull_request_template.md`). Zero source-code changes (FR-008).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify branch state + confirm the structural-signal hook still exists.

- [X] T001 Confirm working branch is `094-deflake-perf-tests`. Run `git status` + `git log -1 --oneline`; verify the branch was created by `/speckit.specify` and main is at post-PR-#200 state or later.
- [X] T002 Confirm baseline pre-PR gate passes. Run `./scripts/pre-pr.sh` once on the unchanged tree; expect `>>> all pre-PR checks passed.` Isolates any post-edit failure as introduced by milestone 094.
- [X] T003 Confirm the structural-signal hook exists. Run `grep -n '"scan starting"' mikebom-cli/src/cli/scan_cmd.rs` → expect at least 1 match at approximately line 1413. If the line has been renamed/removed in main since the plan was written, STOP and update `research.md §1` + `data-model.md` to use the new signal hook.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: No shared infrastructure required — each user story is file-level independent.

(No tasks in this phase.)

**Checkpoint**: US1, US2, US3 can begin in any order or in parallel.

---

## Phase 3: User Story 1 — Skip wall-clock perf tests in default lane (Priority: P1)

**Goal**: Add `#[ignore]` to both perf tests so `cargo +stable test --workspace` and `./scripts/pre-pr.sh` no longer execute them by default. Existing assertions / sample counts / threshold values stay untouched (FR-007); only the default-lane execution is removed.

**Independent Test**: `cargo +stable test -p mikebom --test triple_format_perf` reports `0 passed; 0 failed; 1 ignored`. Same for `--test dual_format_perf`. `cargo +stable test -p mikebom --test triple_format_perf -- --ignored` runs the test (succeeds when wall-clock allows, but doesn't gate PR merges).

### Implementation for User Story 1

- [X] T004 [P] [US1] Add `#[ignore = "wall-clock perf test — opt in via 'cargo test -- --ignored', runs in dedicated perf.yml lane"]` immediately above `#[test]` on `fn triple_format_is_at_least_25_percent_faster_than_three_sequential_scans` in `mikebom-cli/tests/triple_format_perf.rs` (the fn is at ~line 199). Body unchanged (FR-007).
- [X] T005 [P] [US1] Add the same `#[ignore = "..."]` attribute to the dual_format_perf test fn in `mikebom-cli/tests/dual_format_perf.rs`. The exact fn name is one of `dual_format_is_at_least_30_percent_faster_than_two_sequential_scans` or similar — verify via `grep -n "^fn dual" mikebom-cli/tests/dual_format_perf.rs` before applying. Body unchanged.
- [X] T006 [US1] Verify Contracts 1 + 2 from `contracts/deflake-contracts.md`, including the opt-in path that FR-003 requires. Run:
    ```bash
    # Default-skip behavior (Contracts 1 + 2):
    cargo +stable test -p mikebom --test triple_format_perf -- --list | grep "triple_format_is_at_least" | grep -q ignored
    cargo +stable test -p mikebom --test triple_format_perf 2>&1 | grep -E "test result: ok.*0 passed.*0 failed.*1 ignored"
    cargo +stable test -p mikebom --test dual_format_perf -- --list | grep -i dual | grep -q ignored
    cargo +stable test -p mikebom --test dual_format_perf 2>&1 | grep -E "test result: ok.*0 passed.*0 failed.*1 ignored"
    cargo +stable test -p mikebom --test holistic_parity -- --list 2>&1 | grep -E "dual.*ignored"

    # Local opt-in behavior (FR-003): the test actually runs when explicitly requested.
    # Acceptance: 1 test runs (not "1 ignored"). Pass/fail of the assertion itself is
    # NOT gated here — wall-clock noise is allowed; this only proves the opt-in
    # mechanism executes the test body.
    cargo +stable test -p mikebom --test triple_format_perf -- --ignored 2>&1 | grep -E "test result:.*1 (passed|failed); 0 (failed|passed); 0 ignored"
    ```
    The five `#[ignore]`-coverage greps MUST succeed. The opt-in verification MUST show the test was executed (status pass-or-fail, NOT "ignored"). Closes the FR-003 local-opt-in coverage gap surfaced by `/speckit.analyze` finding C1.

**Checkpoint**: US1 complete. Default CI lanes + pre-PR gate no longer execute the wall-clock perf tests.

---

## Phase 4: User Story 2 — Add dedicated opt-in/scheduled perf CI lane (Priority: P1)

**Goal**: Preserve the wall-clock regression-catch surface via a new `.github/workflows/perf.yml` workflow that runs both perf tests with 3-attempt retry. Triggered by `pull_request` (labeled `perf`), `workflow_dispatch`, and `schedule` (06:00 UTC daily). NOT required for PR merge.

**Independent Test**: YAML parses cleanly via `python3 -c "import yaml; yaml.safe_load(...)"`. Required triggers + retry SHA + `perf`-label gate all present per Contract 4. Post-merge: adding the `perf` label to a test PR triggers the workflow within ~1 minute.

### Implementation for User Story 2

- [X] T007 [P] [US2] Create `.github/workflows/perf.yml` per the full body in `data-model.md §perf.yml`. **Before writing, resolve the actual `dual_format_perf` test fn name** via `grep -n "^fn dual" mikebom-cli/tests/dual_format_perf.rs` and substitute it into the retry-step `command:` field (data-model.md's body uses the assumed name `dual_format_is_at_least_30_percent_faster_than_two_sequential_scans`; correct it if the actual name differs). Key invariants: (a) three triggers (`pull_request: types: [labeled]`, `workflow_dispatch:`, `schedule: cron: '0 6 * * *'`); (b) job-level `if: github.event_name != 'pull_request' || github.event.label.name == 'perf'` so labeled-PR firings gate on the `perf` label specifically; (c) `runs-on: ${{ matrix.runner }}` with `matrix: runner: [ubuntu-latest, macos-latest]`; (d) every action SHA-pinned per the milestone-094 / PR #200 convention (use the exact SHAs from `data-model.md` — they match the same actions/SHAs already in `release.yml` for actions/checkout, dtolnay/rust-toolchain stable, Swatinem/rust-cache; plus `nick-fields/retry@ce71cc2ab81d554ebbe88c79ab5975992d79ba08 # v3.0.2`); (e) one retry-wrapped step per perf test (`triple_format` + `dual_format`) with `max_attempts: 3, timeout_minutes: 5`.
- [X] T008 [US2] Verify Contract 4 from `contracts/deflake-contracts.md`. Run:
    ```bash
    test -f .github/workflows/perf.yml && python3 -c "import yaml; yaml.safe_load(open('.github/workflows/perf.yml'))" && echo "YAML OK"
    grep -E "pull_request:|workflow_dispatch:|schedule:" .github/workflows/perf.yml | wc -l   # expect >= 3
    grep -E "nick-fields/retry@[0-9a-f]{40}" .github/workflows/perf.yml | wc -l               # expect >= 2
    grep -q "github.event.label.name == 'perf'" .github/workflows/perf.yml                    # expect exit 0
    ```
    All four MUST succeed.
- [X] T009 [US2] Verify the workflow's actions match the SHA-pinning convention from PR #200. Run `grep -E "uses: [^@]+@(v[0-9]|nightly|stable|latest|main|master)$" .github/workflows/perf.yml` → expect empty output. Kusari Inspector will fail the PR if any tag-pinned action lands.

**Checkpoint**: US2 complete. Wall-clock regression-catch surface preserved on the opt-in/scheduled lane. PR merges no longer gated on wall-clock perf.

---

## Phase 5: User Story 3 — Deterministic structural-correctness sibling test in default lane (Priority: P2)

**Goal**: Add `triple_format_structural.rs` with 3 deterministic test fns that catch single-pass-dispatch regressions binary pass/fail using the existing `"scan starting"` log signal — no wall-clock semantics, no thresholds, no flakiness.

**Independent Test**: 100 consecutive `cargo test --test triple_format_structural` invocations all pass with no flakes (SC-006). Deliberately breaking single-pass dispatch (e.g., a hypothetical refactor that calls scan-pipeline 3 times for triple-format) causes the structural test to fail immediately.

### Implementation for User Story 3

- [X] T010 [P] [US3] Create `mikebom-cli/tests/triple_format_structural.rs` with the three test fns per `data-model.md §triple_format_structural.rs`:
    - `triple_format_invokes_scan_pipeline_exactly_once` — single `mikebom sbom scan --format cdx,spdx,spdx3` invocation; capture stderr; assert `stderr.matches("scan starting").count() == 1`.
    - `three_sequential_invocations_emit_three_pipeline_starts` — three separate single-format invocations; assert total `"scan starting"` count across all stderrs == 3.
    - `triple_format_outputs_byte_match_three_sequential` — run triple-format once + three single-format runs; compare normalized output for each format using `common::normalize::{normalize_cdx_for_golden, normalize_spdx23_for_golden, normalize_spdx3_for_golden}`; assert byte-equality per format.
    
    Include `mod common;` + `use common::normalize::*;` (provides `apply_fake_home_env`, `normalize_cdx_for_golden`, `normalize_spdx23_for_golden`, `normalize_spdx3_for_golden`) + `use common::{bin, workspace_root};` at the top. Duplicate `build_synthetic_image` + `build_benchmark_fixture` + `ImageFile` from `triple_format_perf.rs` (~40 lines; acceptable per FR-008 as test-code duplication). **Do NOT** use local `bin()` / `apply_fake_home_env()` copies — the `dual_format_perf.rs` local-helper pattern exists only because that file is also included as a submodule of `holistic_parity.rs`; `triple_format_structural.rs` is a standalone test target with no such inclusion conflict, so the standard `common::` imports work cleanly.
- [X] T011 [US3] Run the 3 new tests and verify they pass:
    ```bash
    cargo +stable test -p mikebom --test triple_format_structural 2>&1 | tail -5
    # Expected: "test result: ok. 3 passed; 0 failed; 0 ignored"
    ```
- [X] T012 [US3] Run the 100-iteration determinism check per Contract 3 / SC-006:
    ```bash
    for i in $(seq 1 100); do
      cargo +stable test -p mikebom --test triple_format_structural --quiet >/dev/null 2>&1 \
        || { echo "FAIL on iteration $i"; exit 1; }
    done && echo "100/100 deterministic passes — SC-006 satisfied"
    ```
    This takes ~10–20 min locally. If even ONE iteration fails, investigate the assertion that fired (likely culprits: stderr buffering race, normalize helper missing a volatile field). Do NOT mark T010 complete until 100/100 passes — a flaky structural test is worse than the wall-clock test it replaces.
- [X] T013 [US3] Verify Contract 3 listing assertions. Run:
    ```bash
    cargo +stable test -p mikebom --test triple_format_structural -- --list | grep -cE "triple_format_invokes_scan_pipeline_exactly_once|three_sequential_invocations_emit_three_pipeline_starts|triple_format_outputs_byte_match_three_sequential"
    # Expected: 3
    ```

**Checkpoint**: US3 complete. Default lane gains a deterministic, fast (~4s), binary-pass/fail signal for single-pass dispatch.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Documentation updates (FR-006), diff-scope audit (FR-008/009/010 / SC-007), final pre-PR gate (FR-002 / SC-005).

- [X] T014 [P] Update `.github/pull_request_template.md` per `data-model.md §pull_request_template.md`. Add ONE new checklist item to the existing "Pre-PR checklist" section (after the SPDX-3-validator line):
    ```markdown
    - [ ] If this PR touches the scan pipeline, output dispatch, or per-format emission, I added the `perf` label to trigger the dedicated perf benchmarking lane (`.github/workflows/perf.yml`).
    ```
- [X] T015 [P] Update `CONTRIBUTING.md` per `data-model.md §CONTRIBUTING.md`. Add a new `### Performance benchmarks (opt-in)` subsection under "Pre-PR gate (MANDATORY)". Body covers: rationale for the split, the three perf.yml triggers, the local opt-in command (`cargo +stable test --workspace -- --ignored --test-threads=1`), and a one-line pointer at the structural-correctness sibling test for what DOES run on every PR.
- [X] T016 Verify Contract 7 — diff scope guard. Run, in order:
    ```bash
    git diff --name-only main | grep -vE '^(mikebom-cli/tests/(triple_format_perf|dual_format_perf|triple_format_structural)\.rs|\.github/(workflows/perf|pull_request_template)\.(yml|md)|CONTRIBUTING\.md|specs/094-deflake-perf-tests/.+)$' | grep -v '^$' | wc -l
    # Expected: 0

    git diff --name-only main | grep -E '^(mikebom-cli/src|mikebom-common/src|xtask/src)/' | wc -l
    # Expected: 0 (FR-008)

    git diff --name-only main | grep -E '^mikebom-cli/tests/fixtures/golden/' | wc -l
    # Expected: 0 (FR-009)

    git diff --name-only main | grep -E '^Cargo\.(lock|toml)$' | wc -l
    # Expected: 0 (FR-010)
    ```
    If ANY check returns non-zero, STOP and investigate scope creep before T017.
- [X] T017 Run the mandatory pre-PR gate per Contract 8. Run `./scripts/pre-pr.sh`. Expect: `>>> all pre-PR checks passed.` with zero clippy warnings; the test summary should show the 2 wall-clock perf tests as `ignored` and the 3 new structural tests as `passed`. This is the CLAUDE.md mandatory gate; failure here blocks PR.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies. Start immediately.
- **Foundational (Phase 2)**: None — file-level independence between user stories.
- **US1 (Phase 3)**: Independent. Touches `triple_format_perf.rs` + `dual_format_perf.rs`.
- **US2 (Phase 4)**: Independent. Touches `.github/workflows/perf.yml` only.
- **US3 (Phase 5)**: Independent. Touches new file `mikebom-cli/tests/triple_format_structural.rs` only.
- **Polish (Phase 6)**: Depends on US1+US2+US3 being complete (verifies aggregate diff scope + runs the gate).

### User Story Dependencies

- **US1 (P1)**: Independent. Required for US2's opt-in mechanism to make sense (`-- --ignored` filters `#[ignore]`-marked tests).
- **US2 (P1)**: Independent at FILE level (own workflow). At RUNTIME, perf.yml needs the perf tests to have `#[ignore]` so `-- --ignored` actually selects them. But the workflow can be MERGED before US1 is complete; it just won't run anything useful until US1 lands. In practice, ship them together.
- **US3 (P2)**: Independent at FILE + RUNTIME level. The structural test only needs the existing `"scan starting"` log line.

### Within Each User Story

- US1: T004 + T005 can run in parallel (different files); T006 verifies both, runs after.
- US2: T007 standalone; T008 + T009 verify after.
- US3: T010 standalone; T011 verifies functional correctness; T012 verifies determinism (100-iter loop, the long-pole task); T013 verifies listing.

### Parallel Opportunities

- T004 + T005 + T007 + T010 (all on different files) — primary parallel batch.
- T014 + T015 (different docs files) — parallel within Polish.
- T011 + T013 (different verifications of the same file) — sequential but cheap.
- T012 (100-iter loop) is the longest task; let it run in background while writing T014/T015.

---

## Parallel Example: Phase 3–5 (US1 + US2 + US3 implementation)

```bash
# All three stories touch different files; fan out:
Task: "Add #[ignore] to triple_format_perf.rs (T004)"
Task: "Add #[ignore] to dual_format_perf.rs (T005)"
Task: "Create .github/workflows/perf.yml (T007)"
Task: "Create mikebom-cli/tests/triple_format_structural.rs (T010)"
```

Verification tasks (T006, T008, T009, T011, T012, T013) run after their corresponding implementation tasks complete.

---

## Implementation Strategy

### MVP First (US1 + US2 — both P1)

The user's core ask is "stop CI from false-failing". The MVP is US1 + US2:

1. Phase 1: Setup (T001–T003)
2. Phase 3: US1 (T004–T006) — `#[ignore]` applied
3. Phase 4: US2 (T007–T009) — perf.yml created
4. Phase 6 partial: T016 + T017 — diff scope + pre-PR gate
5. **STOP and VALIDATE**: open a test PR; confirm no perf-test failures on the 3 default lanes.

US3 + docs can land in the same PR or as a follow-up. US3 strengthens the regression-catch surface in the default lane; without it, US1+US2 still solve the immediate flaky-test pain but lose the per-PR regression signal for single-pass dispatch.

### Incremental Delivery (recommended)

1. Setup → US1 → US2 → US3 → Polish → ship as a single PR.
2. The whole milestone is ~17 tasks, ~90 min including the 10–20 min 100-iter loop. Single PR is appropriate.

### Single-Developer Strategy

This is a small test-infrastructure milestone; one developer does it sequentially:

1. T001–T003 (setup, ~5 min)
2. T004–T006 (US1 ignore + verify, ~10 min)
3. T007–T009 (US2 perf.yml + verify, ~15 min)
4. T010 (US3 new structural test file write, ~20 min — the longest write)
5. T011 + T013 (US3 quick verifications, ~3 min)
6. T012 (US3 100-iter determinism loop, ~10–20 min wall-clock; can run in background)
7. T014 + T015 (Polish docs, ~10 min)
8. T016 + T017 (Polish audit + pre-PR gate, ~5 min)

Total: ~80 min including the determinism loop's wall-clock.

---

## Notes

- [P] markers = different files OR different verifications of independent files.
- [Story] label maps task to specific user story for traceability.
- US1 and US2 are co-equal P1 because the user's frustration is dual-rooted: (a) CI blocks merges on noise, (b) we don't want to lose the regression-catch surface that the wall-clock test ostensibly provides.
- US3 is P2 because US1+US2 alone solve the immediate pain. US3 is the architectural completion: deterministic signal in the default lane that catches single-pass dispatch breakage immediately, complementing the periodic perf.yml lane.
- The 100-iteration determinism check (T012) is non-negotiable. A flaky NEW structural test would defeat the milestone's purpose. If T012 fails, fix the test before merging.
- Commit boundary suggestion: one commit per user-story phase (3 commits) + one polish commit, OR squash to a single PR-level commit at merge time.
- Avoid: tuning median-N or threshold values in this milestone (FR-007). Those are separate decisions for future milestones once we have perf-lane signal over multiple weeks.
