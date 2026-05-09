---
description: "Tasks: Verify-and-close cargo proc-macro outgoing dep edges (closes #173)"
---

# Tasks: Verify-and-close cargo proc-macro outgoing dep edges

**Input**: Design documents from `/specs/088-cargo-procmacro-edges/`
**Prerequisites**: spec.md ✅, plan.md ✅, research.md ✅, data-model.md ✅, contracts/procmacro-edge-pin.md ✅, quickstart.md ✅

**Organization**: Tiny verify-and-close milestone. No code changes (FR-004). Phase 1 captures pre-pin evidence (`Cargo.lock` ground truth + post-087 SBOM check). Phase 2 covers US1 (regression-test pin) — the MVP. Phase 3 covers US2 (audit doc + issue closure). Phase 4 = polish.

## Path Conventions

Repository-relative paths from `/Users/mlieberman/Projects/mikebom/`:
- Cargo regression test: `mikebom-cli/tests/transitive_parity_cargo.rs`
- Audit fixture: `mikebom-cli/tests/fixtures/transitive_parity/cargo/`
- Milestone-083 audit research doc: `specs/083-transitive-correctness/research.md`

---

## Phase 1: Setup (pre-pin evidence)

- [X] T001 [P] Capture the post-087 ground truth: build release binary, scan `mikebom-cli/tests/fixtures/transitive_parity/cargo`, save to `/tmp/check-088.spdx.json`. Run the jq query from quickstart Recipe 1 to confirm `clap_derive@4.5.18` emits exactly 4 outgoing edges (heck, proc-macro2, quote, syn) with versions per the workspace lockfile. Verifies VR-088-001 + VR-088-002. **Result**: 4 edges confirmed (heck@0.5.0, proc-macro2@1.0.86, quote@1.0.36, syn@2.0.70).
- [X] T002 [P] Read `mikebom-cli/tests/fixtures/transitive_parity/cargo/Cargo.lock` lines 510-518 to confirm `clap_derive@4.5.18`'s declared dependencies are exactly `["heck 0.5.0", "proc-macro2", "quote", "syn"]`. Confirms the 4 representative edges to be pinned match the lockfile (the `heck 0.5.0` form will be normalized via milestone-087's dual-key insert).

## Phase 2: US1 — Regression-test pin (Priority: P1) — MVP

**Goal**: Pin the 4 `clap_derive →` outgoing edges in milestone-083's regression test so future cargo-reader changes that drop proc-macro outgoing edges fail loudly.

**Independent Test**: `cargo +stable test -p mikebom --test transitive_parity_cargo` passes with the bumped `EXPECTED_REPRESENTATIVE_EDGES` array. Length post-add MUST be 8 (4 milestone-087 baseline + 4 milestone-088 additions).

### Implementation for User Story 1

- [X] T003 [US1] Edit `mikebom-cli/tests/transitive_parity_cargo.rs::EXPECTED_REPRESENTATIVE_EDGES` per quickstart Recipe 2: append the 4 milestone-088 entries (`("pkg:cargo/clap_derive", "pkg:cargo/heck")`, `→ proc-macro2`, `→ quote`, `→ syn`) after the existing 4 milestone-087 entries. Update the surrounding doc-comment to add a "Closed by milestone 088" subsection mirroring the existing "Closed by milestone 087" subsection. Verifies VR-088-003 + VR-088-004.
- [X] T004 [US1] Run `cargo +stable test -p mikebom --test transitive_parity_cargo` and confirm all 4 tests pass. Confirms VR-088-001 (each new edge actually emits in the post-087 SBOM). **Result**: 4 tests pass.

## Phase 3: US2 — Audit doc + issue closure (Priority: P2)

**Goal**: Mark gap #2 closed in `specs/083-transitive-correctness/research.md §8 — Ecosystem: cargo`, mirroring the gap #1 closure pattern. Close GitHub issue #173 via `Closes #173` in the PR body.

**Independent Test**: Read `research.md`; confirm zero open mikebom-side gaps remain in §8 — Ecosystem: cargo. Both #172 and #173 marked "Closed by milestone 087" in the "Filed follow-up issues" footer. Post-merge, `gh issue view 173` shows state = closed.

### Implementation for User Story 2

- [X] T005 [US2] Edit `specs/083-transitive-correctness/research.md §8 — Ecosystem: cargo` per quickstart Recipe 3, change 1: strikethrough gap #2's original observation, append the "Closed by milestone 087" annotation block (root-cause one-liner + reference to `transitive_parity_cargo.rs::EXPECTED_REPRESENTATIVE_EDGES` as the lock-down test). Mirrors gap #1's existing closure annotation. Verifies VR-088-005. **Also**: updated the "Tiebreaker resolution" + "Follow-up disposition" lines that referenced gap #2 as open.
- [X] T006 [US2] Edit `specs/083-transitive-correctness/research.md` "Filed follow-up issues" footer per quickstart Recipe 3, change 2: append `. **Closed by milestone 087.**` to the `#173 — cargo: proc-macro crates emit zero outgoing edges (clap_derive case)` row. Mirrors how #172 is annotated. Verifies VR-088-006.
- [X] T007 [US2] Open the PR with `Closes #173` in the body so GitHub auto-closes issue #173 on merge. Verifies VR-088-007. **Result**: PR #182 opened with `Closes #173` in body.
- [X] T008 [US2] Post-merge: run `gh issue view 173` and confirm state is `closed`. Verifies VR-088-008. **Result**: state=CLOSED, closedAt=2026-05-09T12:17:21Z, closedByPullRequestsReferences=#182, stateReason=COMPLETED.

## Phase 4: Polish

- [X] T009 Run `git status --short mikebom-cli/tests/fixtures/golden/` and confirm zero modified golden files. This milestone is verify-only; ANY golden regen indicates scope creep. Verifies the per-format scope contract. **Result**: zero modified golden files. Also verified zero modified files in `mikebom-cli/src/scan_fs/` (FR-004 implicit verification).
- [X] T010 Run `./scripts/pre-pr.sh`: zero clippy warnings + every test suite reports `0 failed`. Verifies SC-005 + the standard CLAUDE.md mandatory gate. **Result**: clean.
- [X] T011 Update CLAUDE.md "Recent Changes" if the speckit infrastructure didn't auto-update it (verify with `grep "088-cargo-procmacro-edges" CLAUDE.md`). **Result**: speckit-plan auto-added the milestone 088 entry; no manual edit required.

---

## Dependencies & Execution Order

- T001 + T002 (Phase 1 evidence) — both `[P]`, can run in parallel.
- T003 (US1 implementation) — sequential after T001+T002 confirm the ground truth.
- T004 (US1 verification) — sequential after T003.
- T005 + T006 (US2 doc edits) — sequential to each other (same file), can run in parallel with T004 since they touch different files.
- T007 (US2 PR open) — depends on T003+T005+T006 being committed.
- T008 (US2 post-merge verify) — depends on T007 being merged by maintainer.
- T009 → T010 → T011 (Polish) — sequential, run before T007 (PR open).

## Parallel Opportunities

- T001 + T002 (Phase 1 evidence) — independent jq query vs file read.
- T005 + T006 are sequential to each other (same file), but parallel with T004 (test run).

## Notes

- **No code changes** to `mikebom-cli/src/scan_fs/package_db/cargo.rs` or `mikebom-cli/src/scan_fs/mod.rs` (FR-004). The cargo reader is correct as of milestone 087.
- **Zero golden regenerations** (FR-006). T009 verifies this explicitly.
- **No new Cargo dependencies** (FR-005, SC-005).
- **PR diff target**: ~25 LOC across 2 files (`transitive_parity_cargo.rs` + `research.md`) + 1 GitHub issue closure comment.
- **Suggested MVP scope**: US1 only (the regression-test pin). US2 is documentation + issue closure, ships in same PR for atomicity but technically isolatable.
