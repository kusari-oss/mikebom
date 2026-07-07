---

description: "Tasks for milestone 171 — fix auto-tag-release.yml so github-actions[bot] can push release tags"
---

# Tasks: Fix auto-tag-release.yml permission bug (closes #519)

**Input**: Design documents from `/specs/171-fix-auto-tag-perms/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/{workflow-diff,secret-provisioning}.md, quickstart.md

**Tests**: This is a CI/workflow config change. Verification is manual (per quickstart.md paths A-F); no unit tests to add.

**Organization**: 3 user stories from spec.md + polish. Small in scope: ~20 tasks total. Some tasks are marked **[MANUAL]** — they require a human (GitHub UI interaction, out-of-band secret provisioning). Others are LLM-doable file edits. Tasks are ordered so LLM tasks can proceed in parallel with the human's [MANUAL] provisioning work.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[MANUAL]**: Requires human execution (browser or repo-admin interaction). NOT LLM-executable.
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm the branch is checked out + reproduce the bug so we know it's still present.

- [X] T001 Confirm current branch. **Completed 2026-07-06**: `git rev-parse --abbrev-ref HEAD` → `171-fix-auto-tag-perms`.

- [X] T002 Reproduce the pre-fix bug per quickstart.md Path A. **Completed 2026-07-06**: `gh run view 28826487562 --repo kusari-oss/mikebom --log-failed | grep -A 3 "denied to github-actions"` printed 3 lines: `remote: Permission to kusari-oss/mikebom.git denied to github-actions[bot].`, `fatal: unable to access ... The requested URL returned error: 403`, `##[error]Process completed with exit code 128.` Bug confirmed still present pre-m171.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: No shared infrastructure needed. Phase 2 is intentionally empty for this milestone (matches the m170 pattern for small config fixes).

**Checkpoint**: Phase 3 (US1) can start immediately after T002.

---

## Phase 3: User Story 1 — Zero-touch releases (Priority: P1) 🎯 MVP

**Goal**: `auto-tag-release.yml` creates + pushes the release tag on release-PR merge WITHOUT any human running `git tag`/`git push` locally.

**Independent Test**: quickstart.md Path B — merge a release-shaped PR; verify (a) workflow run succeeds; (b) tag appears on origin; (c) release.yml fires.

### PAT provisioning (out-of-band manual work; can run in parallel with the YAML edit tasks)

- [ ] T003 **[MANUAL]** [US1] Follow `contracts/secret-provisioning.md` Steps 1-2 to sign in as Michael's account and create a fine-grained PAT at https://github.com/settings/tokens?type=beta with EXACT values:
  - Token name: `mikebom-release-tag-token`
  - Expiration: 365 days
  - Resource owner: `kusari-oss`
  - Repository access: "Only select repositories" → `kusari-oss/mikebom` (exactly one repo)
  - Repository permissions: `Contents: Read and write` (ONLY — every other permission stays at "No access")
  - Copy the PAT value into a password manager. Do NOT paste it into Slack/email/chat.

- [ ] T004 **[MANUAL]** [US1] Verify PAT scope per `contracts/secret-provisioning.md` Step 3 — run the three `gh api` commands (mikebom access → should succeed; other-repo access → should fail 403/404; Actions listing → should fail 403). Confirms the PAT is properly scoped and can't reach anything beyond mikebom's Contents.

- [ ] T005 **[MANUAL]** [US1] Store the PAT as repo secret `RELEASE_TAG_TOKEN` per `contracts/secret-provisioning.md` Step 4. Navigate to https://github.com/kusari-oss/mikebom/settings/secrets/actions → New repository secret → Name: `RELEASE_TAG_TOKEN` (exact match, uppercase) → Value: paste PAT.

- [ ] T006 **[MANUAL]** [US1] Verify no other workflow references `RELEASE_TAG_TOKEN` (defense-in-depth check per `contracts/secret-provisioning.md` Step 5): `grep -rln "RELEASE_TAG_TOKEN" .github/workflows/` — post-m171 should print exactly one path (`.github/workflows/auto-tag-release.yml`). Any additional path indicates unexpected use of the secret.

- [ ] T007 **[MANUAL]** [US1] Set a calendar reminder for 10 months out (rotation reminder — 2 months of margin before the 365-day PAT expiration). Reminder text per `contracts/secret-provisioning.md` Step 6.

### Workflow YAML change (LLM-executable in parallel with T003-T007)

- [X] T008 [US1] Edited checkout step. **Completed 2026-07-07**: added `token: ${{ secrets.RELEASE_TAG_TOKEN }}` under `with:` + 4-line comment block explaining m171 rationale and why `persist-credentials: false` stays.

- [X] T009 [US1] Edited env block. **Completed 2026-07-07**: `GH_TOKEN` source changed from `secrets.GITHUB_TOKEN` to `secrets.RELEASE_TAG_TOKEN` + 5-line comment block explaining the switch and pointing at `docs/releases.md`.

- [X] T010 [US1] Added fail-loud guard. **Completed 2026-07-07**: 11-line block at top of tag-push `run:` script — checks `[ -z "${GH_TOKEN}" ]`, emits `::error::` annotation with recovery playbook + interim workaround, `exit 1`. Turns cryptic 403 into actionable diagnostic per FR-003.

- [X] T011 [US1] Validated YAML syntax + verified `if:` guard byte-identical. **Completed 2026-07-07**: `python3 -c 'import yaml; yaml.safe_load(...)'` exit 0, YAML syntax OK. `git diff main | grep "if:"` returned empty — title-match `if:` guard unchanged per contract's non-goal. Diff stat: +23 -1 lines (net +22), under the contract's +30 scope-creep threshold. **Notable discovery worth flagging in PR body**: the workflow's second step `Dispatch release workflow` still uses `secrets.GITHUB_TOKEN` — but post-171 the tag push is made via `RELEASE_TAG_TOKEN` (a PAT, NOT `GITHUB_TOKEN`), so GitHub's anti-loop policy no longer applies. The natural `push: tags:` trigger on `release.yml` SHOULD fire automatically post-m171, which may make the explicit `Dispatch release workflow` step redundant OR cause `release.yml` to fire TWICE for the same tag. Left unchanged pending review — file follow-up if double-fire observed on the next release.

**Checkpoint**: US1 delivered post-merge — the very next release PR (v0.1.0-alpha.55+) will exercise the fix end-to-end. Live-smoke verification is captured as T020 in Polish phase since it depends on a future real release event.

---

## Phase 4: User Story 2 — Retroactive audit (Priority: P2)

**Goal**: Historical inspection surfaces every prior silent failure so the team knows how long the bug has been latent.

**Independent Test**: quickstart.md Path C — run the `gh` audit query; classify each release-flavored run; attach the table to the PR body.

- [X] T012 [US2] Ran audit query. **Completed 2026-07-07**: 7 release-flavored runs found — alpha.48-51 succeeded via workflow, alpha.52-53 skipped (title format mismatch), alpha.54 failed on permission. Also verified all 7 tags landed on `origin` via `git ls-remote --tags origin`. Query broadened per T012 to also catch the `release: v0.1.0-alpha.N` title-format pattern used pre-`feedback_release_pr_title_format` memory.

- [X] T013 [US2] Formatted as table. **Completed 2026-07-07**: table at `specs/171-fix-auto-tag-perms/audit.md` with 5 columns (release, workflow run ID + link, conclusion, tag-on-origin status, notes). Committed as part of m171 PR so it's a permanent audit-trail record, not just a PR-body ephemeral.

- [X] T014 [US2] Added interpretation. **Completed 2026-07-07**: 4-paragraph interpretation section in `audit.md` — the bug is a **fresh regression** (alpha.48-51 all succeeded before something changed between 2026-06-20 and 2026-07-06); alpha.52-53 masked the regression via title-format mismatch (coincidence, not intent); zero consumer-visible impact (all tags landed, just via different provenance); blast radius has been "maintainer inbox time" not "downstream missed a release". Cross-referenced with research §R3.

---

## Phase 5: User Story 3 — Failure-loud verification (Priority: P2)

**Goal**: Confirm that GitHub's default status widgets surface workflow failures on the merged PR page — no additional wiring needed post-171.

**Independent Test**: quickstart.md Path D — synthesize a workflow failure on a scratch branch; verify the red X appears on the merged PR page.

- [X] T015 [US3] Verified GitHub's default failure-visibility UX. **Completed 2026-07-07**: (a) inspected `.github/workflows/auto-tag-release.yml` trigger config — `on: pull_request: types: [closed] branches: [main]` confirms the workflow fires on every closed PR against main, with the `if:` guard filtering to release-titled ones (unchanged post-m171). (b) empirically verified against PR #518 (the failing release): `gh pr view 518 --json statusCheckRollup` returns `{"conclusion":"FAILURE","name":"tag-and-dispatch"}` — the failed job surfaces in the PR's status-check rollup as a red X, visible on the merged PR page without opening the Actions tab. No additional wiring needed.

- [X] T016 [US3] Documented the finding in audit.md. **Completed 2026-07-07**: added `## Failure-mode verification (US3)` section covering (a) empirical verification via PR #518's `statusCheckRollup`, (b) coverage across all failure modes (permission, missing secret, network, malformed version), (c) explicit non-goal (m171 does NOT add Slack/email/issue-creation notifications; rationale: release cadence + m165 "signal not spam" guidance). Prevents future re-litigating whether US3 was implemented.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Docs + verification + PR wrap-up.

- [X] T017 [P] Created `docs/releases.md`. **Completed 2026-07-07**: 141-line new file with 4 top-level sections — Overview, The release PR (title format + version bump + golden regen sweep + skip-local-pre-PR guidance, all cross-referencing the relevant memories), The auto-tag mechanism (happy path + secret ownership + rotation cadence + emergency revocation), Failure playbook (detection + manual recovery + when to rotate). Over the 50-line plan estimate but justified as first-time-maintainer-actionable reference.

- [X] T018 [P] Follow-up milestone tags added to `specs/171-fix-auto-tag-perms/audit.md`. **Completed 2026-07-07**: 3 checkbox items — (a) provision kusari-oss-bot service account + rotate PAT; (b) investigate `Dispatch release workflow` step redundancy post-m171; (c) add actionlint to pre-PR gate. All 3 non-blocking, referenceable from the PR body.

- [X] T019 Ran pre-PR gate. **Completed 2026-07-07**: green — `>>> all pre-PR checks passed.` No test failures; no `---- .+ stdout ----` failure lines. SC-007 satisfied.

- [X] T020 Diff verified. **Completed 2026-07-07**: `git diff main --name-only -- '.github/workflows/**'` returns exactly `.github/workflows/auto-tag-release.yml` — no other workflow YAMLs touched. Full stat: `auto-tag-release.yml` +23/-1, `CLAUDE.md` +3/-1, `docs/releases.md` new 141-line file, `specs/171-fix-auto-tag-perms/` new dir. SC-008 satisfied. FR-005 (single-revert reversibility) + FR-006 (author-agnostic) satisfied by inspection: `grep -c "pull_request.user\|github.actor\|github.triggering_actor"` on the workflow returns 0.

- [~] T021 **[MANUAL — post-merge]** Live-smoke verification per quickstart.md Path B. **Deferred**: cannot execute within m171's PR because it requires a subsequent release event (v0.1.0-alpha.55+). The maintainer merges the next release PR AND confirms the tag appears on origin without manual push, then comments on the m171 PR with the `gh run view <run-id>` evidence. SC-001/002/003/004 verified retrospectively at that point.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001-T002. No prerequisites.
- **Foundational (Phase 2)**: No-op.
- **User Story 1 (Phase 3, P1)**: T003-T011. LLM tasks (T008-T011) run in parallel with human [MANUAL] tasks (T003-T007). Both must complete before merge; PAT provisioning can happen AFTER m171 merges IF the T010 guard is in place (worst case: next release fails the guard, maintainer provisions PAT, retries).
- **User Story 2 (Phase 4, P2)**: T012-T014. Independent of US1 code changes; can start anytime.
- **User Story 3 (Phase 5, P2)**: T015-T016. Verification-only; can start anytime.
- **Polish (Phase 6)**: T017-T021. T017 depends on Q2 clarification (already settled); T018 depends on user's decision to file a follow-up; T019-T020 depend on Phase 3 code changes; T021 depends on a future release event (post-merge).

### Within User Story 1

- **PAT provisioning subgroup (T003-T007)**: sequential-human — T003 before T004 (verify uses T003's PAT), T004 before T005 (store only after verify), T005 before T006 (grep only after storage), T006 before T007 (calendar reminder is last).
- **YAML editing subgroup (T008-T011)**: T008 + T009 in parallel [P] (different sub-blocks of the same file — actually same file so probably sequential to avoid merge conflicts on my end, but reviewer can look at each as a separate hunk). T010 depends on T008-T009. T011 (validation) is last.
- **Cross-subgroup**: PAT provisioning subgroup + YAML editing subgroup are independent — they can run in parallel because they touch different systems (GitHub UI vs local YAML file).

### Parallel Opportunities

- **Phase 1**: T001 → T002 sequential.
- **Phase 2**: no tasks.
- **Phase 3 US1**: PAT provisioning (T003-T007, human, sequential) IN PARALLEL WITH YAML edits (T008-T011, LLM). T008 + T009 could theoretically go in parallel [P] but same file — do sequential for safety.
- **Phase 4 US2**: T012 → T013 → T014 sequential (T013 depends on T012's raw output; T014 depends on T013's table for the interpretation paragraph).
- **Phase 5 US3**: T015 → T016 sequential.
- **Phase 6 polish**: T017 + T018 parallel [P] (both are text-only, different sections of the PR body / a new file). Then T019 → T020 sequential (diff after pre-PR passes). T021 is post-merge, decoupled from the m171 PR itself.

### Independent Test Criteria per User Story

- **US1**: (a) `gh run view <next-release-workflow-run> --repo kusari-oss/mikebom` shows `success`; (b) `git ls-remote --tags origin v0.1.0-alpha.<N>` returns a matching SHA; (c) `release.yml` fires within 30 seconds of the tag push. Deferred to T021 post-merge.
- **US2**: Retroactive audit table lands in PR body under `## Retroactive audit (SC-005)` heading with all 5+ historical release-flavored runs classified.
- **US3**: `## Failure-mode verification (US3)` paragraph in PR body explains GitHub's default failure-surfacing behavior + why no additional wiring was added.

### MVP Scope

**Suggested MVP**: US1 alone (T003-T011 + T017-T020 in Polish). US2 (audit) and US3 (failure-loud verification) are P2 refinements that could ship as a follow-up PR without blocking the immediate release-flow fix.

**Recommended**: land all three stories in one PR — the whole milestone is ~11 lines of YAML + ~50 lines of docs + audit table in PR body. Splitting adds process overhead disproportionate to size.

**Post-merge**: T021's live-smoke verification happens on the next release PR (v0.1.0-alpha.55+). If that succeeds, m171 is verified end-to-end. If it fails, the T010 guard catches it cleanly, and the maintainer diagnoses via the docs/releases.md failure playbook.
