# Retroactive audit: `auto-tag-release.yml` run history

**Feature**: 171-fix-auto-tag-perms (closes #519)
**Date**: 2026-07-07
**Query**: last 100 runs of `auto-tag-release.yml` on `kusari-oss/mikebom`, filtered to release-flavored PR titles.

## Audit table (SC-005)

| Release | Workflow run ID | Workflow conclusion | Tag on `origin`? | Notes |
|---|---|---|---|---|
| v0.1.0-alpha.54 | [28826487562](https://github.com/kusari-oss/mikebom/actions/runs/28826487562) | **failure** (permission 403) | yes (manual push) | Workflow fired (title matched); tag-push step failed with 403; recovered by manual `git tag -a && git push origin`. |
| v0.1.0-alpha.53 | [28809141679](https://github.com/kusari-oss/mikebom/actions/runs/28809141679) | skipped (title-format mismatch) | yes (manual push) | PR title was `release: v0.1.0-alpha.53` (missing `bump workspace to`) — workflow's `if:` guard didn't match. Manual tag push. |
| v0.1.0-alpha.52 | [28339119789](https://github.com/kusari-oss/mikebom/actions/runs/28339119789) | skipped (title-format mismatch) | yes (lightweight tag) | Same title-format issue as alpha.53. Manual tag. |
| v0.1.0-alpha.51 | [27910300383](https://github.com/kusari-oss/mikebom/actions/runs/27910300383) | **success** | yes (workflow-pushed) | Workflow's happy path. Annotated tag. |
| v0.1.0-alpha.50 | [27888400699](https://github.com/kusari-oss/mikebom/actions/runs/27888400699) | **success** | yes (workflow-pushed) | Workflow's happy path. |
| v0.1.0-alpha.49 | [27874335968](https://github.com/kusari-oss/mikebom/actions/runs/27874335968) | **success** | yes (workflow-pushed) | Workflow's happy path. |
| v0.1.0-alpha.48 | [27644750532](https://github.com/kusari-oss/mikebom/actions/runs/27644750532) | **success** | yes (workflow-pushed) | Workflow's happy path. |

## Interpretation

**The permission bug is a fresh regression, not a long-standing gap.** Releases alpha.48-51 all succeeded via `auto-tag-release.yml`'s happy path, which means the workflow's YAML + the `github-actions[bot]` token's permission model were fully working through at least 2026-06-20 (approximate date of alpha.51 based on run-ID sequencing).

Between alpha.51 and alpha.54, something at the org / repo permissions layer changed. Candidates per research.md §R3:
- Kusari org admin toggled Settings → Actions → General → Workflow permissions to read-only.
- GitHub rolled out the default-token-read-only policy to Kusari org via its 2024 phased rollout.
- A branch/tag protection rule change on `main` narrowed scope for `github-actions[bot]`.

**Why alpha.52 and alpha.53 didn't hit the bug**: coincidence. Both used a non-matching PR title format (`release: v0.1.0-alpha.<N>` instead of `release: bump workspace to v0.1.0-alpha.<N>`) so the workflow's title-match `if:` guard skipped both runs entirely — masking the permission regression until alpha.54's PR used the correct title format.

**Consumer-visible impact of the audit's findings**:
- All 7 releases in the audit window (alpha.48-54) DID land tags on `origin`. No release was silently dropped.
- Tags for alpha.52-54 were pushed with slightly different provenance than alpha.48-51 (manual vs workflow). GitHub's release artifacts + `git ls-remote` behavior are indistinguishable — downstream consumers don't observe any difference.
- The bug's blast radius has been "the maintainer's inbox" (5-10 min per release of manual `git tag && git push` work), not "downstream consumers missed a release".

Post-m171 the workflow's happy path is restored, and future releases don't require the manual recovery.

## Failure-mode verification (US3)

**GitHub's default UX already surfaces workflow failures on the merged PR page.** m171 adds no notification/webhook wiring for this — none is needed. Verified empirically against PR #518 (the release that exposed the bug):

```bash
gh pr view 518 --repo kusari-oss/mikebom --json statusCheckRollup \
  --jq '.statusCheckRollup[] | select(.name == "tag-and-dispatch") | {name, conclusion}'
# → {"conclusion":"FAILURE","name":"tag-and-dispatch"}
```

The failed `tag-and-dispatch` job appears in the PR's status-check rollup with `conclusion: FAILURE`, which GitHub renders as a red X in the PR's post-merge status widget. A maintainer glancing at the merged PR page sees the failure immediately without opening the Actions tab.

**What this means for future failure modes**:
- Permission errors (the m171-fixed case): surface as red X on merged PR.
- Missing `RELEASE_TAG_TOKEN` secret (m171 T010's new guard): surfaces as red X + the `::error::` annotation appears inline in the workflow log.
- Network / rate-limit errors: surface as red X.
- Malformed version-string in Cargo.toml: surfaces as red X.

**Explicit non-goal**: m171 does NOT add Slack, email, or issue-creation notifications for workflow failures. Rationale — releases are infrequent enough (2-3/week) that a maintainer merging a release PR is present at the moment of merge and can react to a red X immediately. Adding notifications would (a) add configuration surface + secret storage complexity, (b) potentially spam the team on transient failures that self-resolve on retry, (c) conflict with the m165 audit guidance ("failures should signal, not spam"). If a future scenario justifies notifications, that's a separate follow-up milestone.

## Follow-up work (for the m171 PR body)

- [ ] **File follow-up milestone**: provision proper `kusari-oss-bot` service account + rotate `RELEASE_TAG_TOKEN` to it + update `docs/releases.md` § auto-tag mechanism to reflect the new ownership. Rationale: m171 interim uses Michael's personal PAT per Q2 clarification; the service-account migration reduces bus-factor risk. Not blocking — the interim posture works; the follow-up hardens ownership.
- [ ] **Investigate `Dispatch release workflow` step redundancy**: post-m171 the tag push happens via a fine-grained PAT (not `github-actions[bot]`'s `GITHUB_TOKEN`), so GitHub's anti-loop policy no longer applies — the natural `push: tags:` trigger on `release.yml` should fire. That may make the explicit `Dispatch release workflow` step redundant OR cause `release.yml` to fire twice. Observe the next real release to determine which; file follow-up if double-fire.
- [ ] **Add `actionlint` to the pre-PR gate**: research §R4 identified this as a valuable prevention against future workflow-YAML regressions but scoped it out of m171. Small tooling milestone.

## Reference

- Root cause + fix approach documented in `specs/171-fix-auto-tag-perms/` (spec, plan, research).
- Manual recovery playbook: `docs/releases.md` § auto-tag mechanism (created by m171 T017).
- Memory `feedback_release_pr_title_format` — the title-format convention that ensures the workflow's `if:` guard matches.
