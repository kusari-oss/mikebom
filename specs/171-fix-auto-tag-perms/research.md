# Phase 0 Research: m171 Auto-Tag Permission Fix

**Feature**: 171-fix-auto-tag-perms
**Date**: 2026-07-06

## R1 — Does a `kusari-oss` service account already exist?

**Question**: Which service account owns automation for the `kusari-oss` org, and does it have a fine-grained PAT model set up?

**Decision**: **UNKNOWN — planning-phase discovery elevated to a planning question for the human user**. The `gh api orgs/kusari-oss/members` query returned three human accounts (`mlieberman85`, `pxp928`, `trmiller`); no bot/service account is visible via the public-members endpoint. Two possibilities:

- Option A: A service account exists but is a private org member — `gh api orgs/kusari-oss/members?filter=private` (as an org admin) would show it.
- Option B: No service account exists; m171 must provision one.

**Path forward per Q1 clarification (fine-grained PAT + service account)**:
- If option A holds, use the existing service account. The mikebom PR body notes the account name so the next maintainer can rotate it.
- If option B holds, either (i) the human maintainer (Michael) owns a PAT AS AN INTERIM using a bot-like GitHub account under his control, and the org files a follow-up milestone to provision a proper service account; OR (ii) the org creates the service account before m171 merges.

**Recommendation for the m171 PR**: Ask the human maintainer at plan-review time which state (A or B) applies. The workflow YAML change is the same either way — only the token source (which PAT the secret holds) differs.

**Data**: `gh api orgs/kusari-oss/members --jq '.[].login'` → `mlieberman85`, `pxp928`, `trmiller`. Not conclusive.

## R2 — Where does the "release process" documentation live?

**Question**: Spec's FR-007 requires a new "How auto-tag-release.yml works" doc section. Where does the existing release-process doc live?

**Decision**: **No dedicated release-process doc currently exists in the mikebom repo**. Grep across `docs/` and `CONTRIBUTING.md` for keywords `release process` / `how to release` / `releasing` returned zero matches.

**Path forward**: Create `docs/releases.md` as a new file, per m171 FR-007. Structure:

1. Overview (1 paragraph — mikebom's release cadence + versioning scheme).
2. The release PR (1 section — title format `release: bump workspace to v...`, the golden-regen sweep per `feedback_release_bump_regen_goldens`, the version-string cascade into SPDX IDs).
3. The auto-tag mechanism (1 section — how `auto-tag-release.yml` works, the `RELEASE_TAG_TOKEN` secret dependency, service-account ownership).
4. Failure playbook (1 section — how to detect the workflow failed, how to manually recover via `git tag && git push`, when to rotate the PAT).

Estimated length: ~50 lines. Small enough to co-locate with the m171 PR without splitting into its own PR.

**Alternatives considered**:
- Add the section to `CONTRIBUTING.md` — rejected, that file is about contributor onboarding not release mechanics.
- Add the section to `docs/index.md` — rejected, index is a navigation hub not authoritative content.
- Create `docs/reference/release-process.md` under the existing reference subtree — considered, but the reference subtree is currently ecosystem/format documentation for consumers. Release process is maintainer-facing, so a top-level `docs/releases.md` is clearer.

**Data**: `ls docs/` shows `architecture`, `audits`, `DEPENDENCIES.md`, `design-notes.md`, `ecosystems.md`, `examples`, `index.md`, `reference`, `research`, `SAST-POLICY.md`. No release doc.

## R3 — Audit history of `auto-tag-release.yml`

**Question**: How often has the workflow succeeded, failed, or been silently skipped in the past 100 runs? Is this a fresh regression or a long-standing gap?

**Decision**: **Fresh regression**. The last 100 runs of `auto-tag-release.yml`:

| Release | Workflow run | Conclusion | Note |
|---|---|---|---|
| v0.1.0-alpha.48 | 27644750532 | **success** | tag pushed by workflow |
| v0.1.0-alpha.49 | 27874335968 | **success** | tag pushed by workflow |
| v0.1.0-alpha.50 | 27888400699 | **success** | tag pushed by workflow |
| v0.1.0-alpha.51 | 27910300383 | **success** | tag pushed by workflow |
| v0.1.0-alpha.52 | (n/a) | title-mismatch skip | PR title was `release: v0.1.0-alpha.52` (missing `bump workspace to`) — workflow's `if:` guard didn't match. Tag pushed manually per memory. |
| v0.1.0-alpha.53 | (n/a) | title-mismatch skip | Same title format. Manually tagged. |
| **v0.1.0-alpha.54** | **28826487562** | **FAILURE (permission)** | Workflow fired (title matched post-`feedback_release_pr_title_format`); tag-push step 403'd. |

**Interpretation**: The workflow was working correctly through alpha.51 (~2026-06-20 based on run-ID sequencing). Then two releases (alpha.52, .53) accidentally used a non-matching title format (spec-writer / maintainer error) — the memory `feedback_release_pr_title_format` was added specifically to prevent this. Then alpha.54 correctly matched the title but hit a NEW permission error that alpha.51 did not experience.

**Root-cause hypothesis**: Something at the org level changed between alpha.51 and alpha.54 (roughly 2026-06-20 to 2026-07-06) that narrowed `github-actions[bot]`'s effective permissions. Possibilities:
- Kusari org admin toggled Settings → Actions → General → Workflow permissions to read-only.
- Kusari org enrolled in GitHub's default-token-read-only policy (rolled out to enterprise orgs mid-2024).
- Branch/tag protection rule change on `main` that scopes to `github-actions[bot]`.

**Impact of the finding**: The m171 fix is not just a nice-to-have — it's a NEW regression. If the m171 fix DOESN'T land, every future release requires the manual `git tag && git push` recovery from `feedback_release_pr_title_format`'s docs.

**Alternatives considered**:
- Roll back the org-level change that broke it. **Rejected**: even if a maintainer with org-admin access identifies the change, the read-only-workflow-tokens policy is GitHub's recommended posture; rolling back would grant broad `contents: write` to every workflow in the repo (equivalent to Option A of the Q1 clarification, which was rejected).
- Retire `auto-tag-release.yml` entirely. **Rejected** — Q1 clarification's Option D. The maintainer chose the auto-tag path.

## R4 — Does the pre-PR gate include workflow-YAML linting?

**Question**: If `actionlint` isn't in the pre-PR gate, does the m171 workflow change need manual verification?

**Decision**: **`actionlint` is NOT in the pre-PR gate or `ci.yml`**. Grep across `scripts/pre-pr.sh` and `.github/workflows/ci.yml` returned zero matches for `actionlint`.

**Path forward**: 
- **Manual YAML syntax check** in a m171 task: run `python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/auto-tag-release.yml"))'` or `yq . .github/workflows/auto-tag-release.yml` before commit to catch obvious syntax errors. `python3` and `yq` are both preinstalled on macOS with Homebrew and on Linux dev boxes.
- **Optional follow-up milestone** (not m171-scope): add `actionlint` to `scripts/pre-pr.sh` and `ci.yml` so future workflow changes get automatic linting. File as a low-priority tooling issue.

**Alternatives considered**:
- Add `actionlint` in the m171 PR. **Rejected**: scope creep. m171 fixes the auto-tag bug; adding a workflow lint gate is a separate concern with its own review considerations (tool provenance, version pinning, cache configuration).
- Skip syntax validation entirely. **Rejected**: even a 5-line YAML change can typo an indent and break the workflow silently.

## R5 — Cross-workflow coupling

**Question**: Which other workflows share tokens or state with `auto-tag-release.yml`? Does the m171 change risk breaking any other workflow?

**Decision**: **Zero cross-workflow coupling**. Grep for `auto-tag-release` and `RELEASE_TAG_TOKEN` across `.github/workflows/` shows:

- `auto-tag-release.yml` — the file we're changing.
- `release.yml` — mentions `auto-tag-release` in a comment explaining the tag-push flow, but has no direct dependency (`release.yml` fires on the `push: tags:` event; whether the tag comes from auto-tag or manual push is irrelevant).

No other workflow references either identifier. The m171 fix is fully isolated. SC-008 (no regression on other workflows) is trivially satisfied since we're not touching any other workflow file.

**Confidence**: high — grep across the entire `.github/workflows/` directory covered every workflow present.

## Consolidated open questions for `/speckit-tasks`

- R1 elevates one plan-review question: **which service account owns `RELEASE_TAG_TOKEN`?** Michael answers at review time. Tasks are written to be posture-agnostic (either service account works; the tasks don't care about the account's name, just that a PAT gets provisioned).
- R2 confirms `docs/releases.md` is the new-file location (no ambiguity remaining).
- R3 confirms the fix is a fresh regression, sizing the "how urgent" answer: high.
- R4 identifies a follow-up milestone (add `actionlint` to CI) but keeps it out of m171 scope.
- R5 confirms zero blast-radius beyond the target workflow.
