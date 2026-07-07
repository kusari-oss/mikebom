# Feature Specification: Fix auto-tag-release.yml permission bug (closes #519)

**Feature Branch**: `171-fix-auto-tag-perms`
**Created**: 2026-07-06
**Status**: Draft
**Input**: User description: "Fix auto-tag-release.yml so github-actions[bot] can push release tags without falling back to manual git push"

## Background

Every release of mikebom follows the same intended pipeline:

1. A maintainer opens a "release: bump workspace to v0.1.0-alpha.N" PR that bumps `workspace.package.version` in `Cargo.toml` and regenerates the goldens.
2. The PR merges into `main`.
3. `.github/workflows/auto-tag-release.yml` fires, extracts the version, creates an annotated `v0.1.0-alpha.N` tag on the merge commit, and pushes it to `origin`.
4. `release.yml` fires on the tag push, builds artifacts (Linux/macOS/Windows binaries + Docker images), and publishes the GitHub release.

The v0.1.0-alpha.54 release (PR #518, merge commit `0163ff1`) exposed a break in step 3. `auto-tag-release.yml` reached the tag-push step and failed with:

```
remote: Permission to kusari-oss/mikebom.git denied to github-actions[bot].
fatal: unable to access 'https://github.com/kusari-oss/mikebom.git/': The requested URL returned error: 403
##[error]Process completed with exit code 128.
```

**Reproduction**: run history at https://github.com/kusari-oss/mikebom/actions/runs/28826487562 — the run itself is deterministic (any release PR merge triggers it).

**Diagnosis**: the workflow YAML already declares the necessary permissions at file scope:

```yaml
permissions:
  contents: write   # to push the new tag
  actions: write    # to dispatch release.yml
```

But GitHub Actions issues the workflow token as the INTERSECTION of the workflow-level declaration AND the repository's default token permission (Settings → Actions → General → Workflow permissions). When the repo default caps at "Read repository contents and packages permissions", the workflow's `contents: write` is silently narrowed to read-only. The push then fails with 403 despite the workflow YAML looking correct.

**Recovery cost**: for v0.1.0-alpha.54 the maintainer had to manually run `git tag -a v0.1.0-alpha.54 -m 'Release v0.1.0-alpha.54' 0163ff1 && git push origin v0.1.0-alpha.54`. This is documented in the memory `feedback_release_pr_title_format` as the workaround. Every release since the bug appeared has silently required this manual step or gone un-tagged.

**Watch item raised in issue #519**: it's unclear whether previous releases (v0.1.0-alpha.52, .53) hit the same failure and were quietly worked around, or whether this is a fresh regression from an org-level permissions change. Historical run inspection is part of this feature's research phase.

## Clarifications

### Session 2026-07-06

- Q: Which token mechanism should the fix use? → A: **Option B — fine-grained PAT** (Q1 clarification). Create a new PAT scoped to `contents: write` on `kusari-oss/mikebom` ONLY, store it as repo secret `RELEASE_TAG_TOKEN`, and use it in the auto-tag workflow's checkout + push steps. This scopes the write capability to a single workflow (rejected: repo-wide toggle, which would grant write to every workflow by default) and skips the GitHub App setup cost (rejected: 5-10 min extra install/config work for equivalent posture). Rotation: yearly (GitHub max fine-grained PAT lifetime).

- Q2 (post-plan §R1): Where does the new PAT live — on an existing `kusari-oss` service account, or on Michael's personal account? → A: **Plan for a new PAT** on Michael's personal account as INTERIM. Rationale: research §R1 found no confirmed service account in the org's public members list (`gh api orgs/kusari-oss/members` returned only human accounts). Provisioning a proper service account is a separate concern (org-admin coordination, 2FA setup, ownership documentation) that would gate m171 unnecessarily. **Follow-up milestone**: file after m171 merges to (a) provision a proper `kusari-oss` service account, (b) rotate `RELEASE_TAG_TOKEN` from Michael's PAT to the service account's PAT, (c) update `docs/releases.md` to reflect the new ownership. Bus-factor watch item: while the PAT lives on Michael's account, the docs and the follow-up-milestone tag are the mitigation — a future maintainer knows to file the migration if Michael becomes unavailable.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Release PR merge auto-tags without manual intervention (Priority: P1)

A maintainer merges a release PR whose title starts with `release: bump workspace to v`. The `auto-tag-release.yml` workflow fires, extracts the new version, creates the annotated tag pointing at the merge commit, and pushes it to `origin` — all without the maintainer running any git command locally. `release.yml` then fires on the tag push and publishes the release artifacts.

**Why this priority**: This is the P1 defect. Without this, every future release requires a manual `git push origin v<X>` after the merge; if a maintainer forgets, the release never publishes. That's a workflow-breaking bug for an operation that runs 2-3 times per week.

**Independent Test**: Open a synthetic release-shaped PR (title starting with `release: bump workspace to v0.1.0-alpha.<test-tag>`) targeting a scratch branch. Merge it. Verify (a) `auto-tag-release.yml` run completes with `success` status; (b) `git ls-remote --tags origin v0.1.0-alpha.<test-tag>` returns a matching hash; (c) `release.yml` fires within 30 seconds of the tag push.

**Acceptance Scenarios**:

1. **Given** a PR titled `release: bump workspace to v0.1.0-alpha.<N>` targeting `main`, **When** the maintainer merges the PR, **Then** the auto-tag workflow creates and pushes tag `v0.1.0-alpha.<N>` to `origin` within 30 seconds of the merge event.
2. **Given** the tag push succeeds, **When** `release.yml` fires on the tag event, **Then** it runs to completion with the same artifacts and behavior it produces today for manually-pushed tags.
3. **Given** a PR with a non-matching title (e.g., `impl(170): dedup ...`), **When** it merges, **Then** the auto-tag workflow does NOT fire (or fires and correctly skips per the existing `if:` guard) — no regression on non-release PRs.

---

### User Story 2 — Historical inspection surfaces every prior silent failure (Priority: P2)

A maintainer running the retroactive audit on `auto-tag-release.yml`'s run history can confirm which prior releases (v0.1.0-alpha.51 → v0.1.0-alpha.53) also failed the tag-push step and required manual recovery, so the team has a complete picture of how long the bug has been latent.

**Why this priority**: Refinement — closes the "watch item" from issue #519. Not blocking on the fix (the fix works either way); but knowing whether this is a fresh regression vs a long-standing gap changes how the team communicates with downstream consumers who may have missed tag pushes.

**Independent Test**: Manually inspect the `auto-tag-release.yml` workflow's run history for every merged release PR since the workflow was introduced. Classify each as `success`, `failure-with-manual-recovery`, or `failure-silent-drop`. Attach the classification table to the PR body.

**Acceptance Scenarios**:

1. **Given** a browse of `gh run list --repo kusari-oss/mikebom --workflow auto-tag-release.yml --limit 50`, **When** the auditor filters to `event == 'pull_request'` events matching release titles, **Then** the auditor produces a per-release-PR verdict (success / manual-recovery / silent-drop).
2. **Given** any release-PR run that failed silently (no tag on `origin`), **When** the auditor confirms via `git ls-remote --tags origin`, **Then** the auditor either pushes the missing tag manually (if the release should have publish artifacts backfilled) or documents why the omission is acceptable.

---

### User Story 3 — Failure mode is loud, not silent (Priority: P2)

If the auto-tag workflow fails for ANY reason (permission, network, malformed version string), the maintainer who just merged the release PR receives a signal immediately — not a discovered-a-week-later "we forgot to publish the release" surprise.

**Why this priority**: Refinement — defense in depth. Even after the P1 permission fix, the workflow can fail for other reasons (rate limits, remote outages, someone renaming the default branch). A failure that closes silently is the WORST failure mode because releases are infrequent enough that maintainers don't notice for days.

**Independent Test**: Introduce a synthesized failure in the workflow (e.g., set `MERGE_SHA` to a bogus value); verify the workflow's failure surfaces via (a) a red status check on the merged PR's history, and (b) an explicit `gh run watch` output that the maintainer can see. No auto-created issue is required per the m165 audit guidance ("failures should signal, not spam"); but the maintainer's PR-merge dashboard MUST show the red X.

**Acceptance Scenarios**:

1. **Given** a synthesized failure in the auto-tag workflow, **When** the maintainer looks at the merged PR page, **Then** the auto-tag workflow run appears in the PR's timeline with `Failure` status.
2. **Given** the same synthesized failure, **When** GitHub's post-merge status widget renders on the PR page, **Then** the widget shows the failure prominently (not buried in a collapsed "checks" section).

---

### Edge Cases

- **Concurrent release PRs**: two release PRs merge within seconds of each other (unlikely but possible). Post-fix behavior: both trigger the workflow; both extract distinct versions from their respective `Cargo.toml` snapshots; both create + push distinct tags. No cross-interference expected because each workflow invocation is scoped to its own `merge_commit_sha`.
- **Retriggered workflow runs**: if a maintainer clicks "Re-run all jobs" on a previous auto-tag-release.yml run, the tag-existence pre-check should skip the create+push step (already implemented in the workflow's `git ls-remote --tags origin ...` pre-check). Post-fix this pre-check MUST continue to work — a re-run of a successful tag push is a no-op, not a failure.
- **Workflow YAML edits in a release PR**: a release PR that BOTH bumps the version AND edits `auto-tag-release.yml` itself creates a chicken-and-egg. The auto-tag workflow that fires after merge is the NEW edited version — if the edit introduced a bug, tag push may fail. Mitigation: the workflow YAML edit is separately auditable in the release PR's diff; a maintainer reviewing the release PR must eyeball any workflow YAML diff before merging.
- **Empty `Cargo.toml` change**: a PR titled `release: bump workspace to v...` but with no actual `[workspace.package].version` change. The workflow's version-extract step should fail-early with a clear error rather than silently pushing a stale-version tag or a `v` tag with an empty suffix.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When a merged pull request's title matches `startsWith('release: bump workspace to v')`, `auto-tag-release.yml` MUST successfully create and push an annotated tag `v<version>` (where `<version>` is extracted from `Cargo.toml`'s `[workspace.package].version` on the merge commit) to `origin` — without ANY manual maintainer intervention.
- **FR-002**: The successful tag push MUST fire `release.yml` with the standard `push: tags: 'v*-alpha.*'` trigger, matching current behavior for manually-pushed tags.
- **FR-003**: When the auto-tag workflow fails for any reason (permission, network, malformed version, etc.), the failure MUST surface as a red status check visible on the merged PR's page — no silent success + missing tag combination.
- **FR-004**: The fix MUST NOT expand the auto-tag workflow token's blast radius beyond what's strictly necessary. Per the Q1 clarification, the token is a fine-grained PAT (repo secret `RELEASE_TAG_TOKEN`) scoped to `contents: write` on `kusari-oss/mikebom` ONLY — no other repos in the org, no other permissions on this repo. The `refs/tags/*`-vs-blanket-`refs/*` sub-scoping goal is a `SHOULD` because GitHub's fine-grained PAT model treats `contents: write` as a single scope covering both branch pushes and tag pushes; if a tag-only scoping mechanism becomes available in the future, migrate. Meanwhile, mitigation for the branch-write risk: (a) the workflow's push step targets only `refs/tags/<tag>`, so the token is USED only for tag push in this repo's workflows; (b) branch-protection rules on `main` (already present) prevent even a leaked PAT from force-pushing to `main` without additional review; (c) the token's audit trail in GitHub's org security log shows every use.
- **FR-005**: The fix MUST be reversible by a single revert of the fix PR. If a downstream security review determines the chosen fix mechanism has an unacceptable posture, reverting must restore the pre-171 state (broken auto-tag but everything else intact).
- **FR-006**: The fix MUST work for future release PRs regardless of who authors the release PR (Michael, a fellow maintainer, or automation). Specifically: the fix MUST NOT depend on the PR author being a specific person (e.g., must not rely on a personally-owned PAT).
- **FR-007**: The fix MUST be documented in `docs/releases.md` (or the closest equivalent — the actual doc location is a planning-phase discovery) so a future maintainer running a release for the first time understands what the auto-tag workflow does + how to diagnose failures.
- **FR-008**: The fix MUST include a smoke test — a synthetic release-flavored PR that exercises the auto-tag workflow end-to-end against a scratch branch WITHOUT publishing a real release. This proves the fix in CI and defends against future regressions.

### Key Entities

- **auto-tag-release.yml workflow** (`.github/workflows/auto-tag-release.yml`): the GitHub Actions workflow that responds to release-PR merges. Pre-171 shape: uses the default `github-actions[bot]` token which is silently narrowed to read-only. Post-171 shape (per Q1 clarification): checkout step uses `secrets.RELEASE_TAG_TOKEN`, push step uses the same via an explicit `https://x-access-token:${TOKEN}@github.com/...` URL. Token is a fine-grained PAT owned by a bot/service account with `contents: write` scope on `kusari-oss/mikebom` ONLY.

- **`RELEASE_TAG_TOKEN` repo secret**: new secret (created in Settings → Secrets and variables → Actions) holding the fine-grained PAT. Owned by a service account (not a personal account). Lifecycle: create once at m171 rollout, rotate yearly per GitHub's max PAT lifetime. Documented in the "How auto-tag-release.yml works" doc section produced by FR-007.

- **Service account** (the identity that owns `RELEASE_TAG_TOKEN`): a dedicated GitHub account used for automation. Not tied to any specific human maintainer. Documented separately so future maintainers know which account to check when the PAT rotates or a permission audit fires.
- **Repo-level workflow-permissions setting** (`repo Settings → Actions → General → Workflow permissions`): controls the default maximum permission of workflow tokens. Governs whether workflow YAML `permissions: contents: write` is honored or silently narrowed.
- **Release tag** (`v0.1.0-alpha.N`): the annotated Git tag that both triggers `release.yml` AND becomes the human-facing GitHub Releases entry. Must be pushed exactly once per release; must point at the merge commit of the release PR.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The next release PR (v0.1.0-alpha.55 or later) merges cleanly and the `v0.1.0-alpha.55` tag appears on `origin` without any maintainer running `git tag` or `git push` locally.
- **SC-002**: `gh run view <auto-tag-release-run-id> --repo kusari-oss/mikebom` for that same release returns `success` status on both the checkout step AND the tag-push step.
- **SC-003**: `release.yml` fires within 30 seconds of the tag push and completes with the same artifact set it produces today for manually-pushed tags.
- **SC-004**: Zero permissions-related error strings (`403`, `denied to github-actions[bot]`, `The requested URL returned error`) appear in the auto-tag workflow run's log.
- **SC-005**: A retroactive audit of the last N release-PR merges classifies each as `auto-tagged-cleanly` / `auto-tagged-with-manual-recovery` / `silently-dropped`. The audit table lands in the fix PR's body so the team knows the historical impact.
- **SC-006**: `docs/releases.md` (or the closest equivalent) has a new "How auto-tag-release.yml works" section that a first-time maintainer can follow to diagnose failures.
- **SC-007**: Pre-PR gate (`./scripts/pre-pr.sh`) passes green — this is a workflow/CI feature not a Rust code change, so the pre-PR gate is a lightweight smoke.
- **SC-008**: No regression: workflows other than `auto-tag-release.yml` continue to behave identically. Specifically: `ci.yml`, `release.yml`, `realistic-projects.yml`, and the various security-scan workflows produce byte-identical run outputs for the same PR/commit before and after the fix.

## Assumptions

- The repo's current "Workflow permissions" setting caps at "Read repository contents and packages permissions" — the standard GitHub default for repos created after the 2022-01 policy change. Planning phase will confirm via repo Settings inspection or via a GitHub API call.
- Recovery cost for the manual workaround (~5 minutes: `git tag` + `git push`) has been paid on at least the v0.1.0-alpha.54 release. The retroactive audit under US2 may reveal it was paid on earlier releases too, or that earlier releases actually succeeded (which would tell us this is a NEW regression rather than a long-standing gap).
- Every release PR title conforms to the `release: bump workspace to v` pattern per the pre-existing memory `feedback_release_pr_title_format`. Post-171, that convention continues to gate whether auto-tag fires.
- No downstream consumer relies on the specific behavior of "auto-tag fires but silently no-ops". Post-fix behavior of "auto-tag fires and successfully pushes" is strictly better for every consumer.
- The fix is a workflow/CI-config change only. No Rust source touched. `mikebom-cli`, `mikebom-common`, and `mikebom-ebpf` are untouched.
- The `contents: write` permission on `refs/tags/*` specifically (FR-004's `SHOULD` sub-goal) is NOT achievable with GitHub Actions' current fine-grained PAT model per Q1 clarification's discovery — the model treats `contents: write` as a single scope covering both branch pushes and tag pushes. Mitigations documented in FR-004.

- A **service account** with a mikebom-scoped fine-grained PAT is the org-standard automation pattern per Q1. Planning phase will confirm that a service account either already exists (e.g., a `kusari-oss-bot` account already used elsewhere in the org) or needs to be provisioned as part of this milestone. If the service account is new, the associated `docs/releases.md` guidance MUST cover the "who owns this account and where's the recovery secret" question for the next maintainer.
