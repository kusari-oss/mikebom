# Quickstart: m171 Manual Verification

**Feature**: 171-fix-auto-tag-perms
**Date**: 2026-07-06

Three verification paths — one per user story. Each is executable without external infra beyond a GitHub account with repo access.

## Path A — Reproduce the pre-fix bug (before merging m171)

This confirms the bug is still present. Skip if you already ran it during the v0.1.0-alpha.54 release.

```bash
# Look at the auto-tag workflow run for v0.1.0-alpha.54:
gh run view 28826487562 --repo kusari-oss/mikebom --log-failed 2>&1 | grep -A 3 "denied to github-actions"
```

**Expected**: prints the 403 permission error. Confirms the bug exists pre-m171.

## Path B — Verify the fix works end-to-end (US1 P1)

**After** the `RELEASE_TAG_TOKEN` secret is provisioned per `contracts/secret-provisioning.md` AND the m171 YAML change is merged:

**Step 1** — synthesize a scratch release-shaped PR that DOESN'T actually publish a real release. Two variants:

**Variant 1 (safest)**: use a private scratch fork/branch not on main.

**Variant 2 (real-world)**: the very next legitimate `release: bump workspace to v0.1.0-alpha.55` PR is the smoke test. Merge it. If the workflow fires and pushes the tag, US1 P1 is verified against a live release.

**Step 2** — verify the workflow ran:

```bash
gh run list --repo kusari-oss/mikebom --workflow auto-tag-release.yml --limit 1
```

**Expected**: most recent run's `conclusion` is `success`.

**Step 3** — verify the tag appeared:

```bash
git ls-remote --tags origin | grep v0.1.0-alpha.55
```

**Expected**: prints exactly one line with a SHA matching the release PR's merge commit.

**Step 4** — verify `release.yml` fired:

```bash
gh run list --repo kusari-oss/mikebom --workflow release.yml --limit 1
```

**Expected**: the most recent run was triggered by the tag push (event: `push`, headBranch: `v0.1.0-alpha.55`).

If all four steps pass, US1 P1 SC-001 through SC-004 are satisfied.

## Path C — Verify the retroactive audit table (US2 P2)

Run the audit `gh` query and classify each of the last 10 release-flavored runs:

```bash
gh run list --repo kusari-oss/mikebom --workflow auto-tag-release.yml --limit 100 \
    --json databaseId,status,conclusion,event,headBranch,displayTitle \
  | jq -r '.[]
    | select(.displayTitle | test("release: bump workspace"))
    | [.databaseId, .conclusion, .displayTitle] | @tsv'
```

**Expected shape** (pre-m171, based on research §R3):

```
28826487562  failure   release: bump workspace to v0.1.0-alpha.54
27910300383  success   release: bump workspace to v0.1.0-alpha.51 + regen 34 byte-identity goldens
27888400699  success   release: bump workspace to v0.1.0-alpha.50 + regen 34 byte-identity goldens
27874335968  success   release: bump workspace to v0.1.0-alpha.49 + regen 34 byte-identity goldens
27644750532  success   release: bump workspace to v0.1.0-alpha.48 + regen 33 byte-identity goldens
```

Copy the table into the m171 PR body as evidence for SC-005.

## Path D — Verify failure-loud path (US3 P2)

Introduce a synthesized failure (locally, on a scratch branch — do NOT merge to main):

**Step 1** — on a scratch branch, edit `auto-tag-release.yml` to inject a bogus MERGE_SHA:

```yaml
env:
  # SYNTHESIZED FAILURE — DO NOT COMMIT
  MERGE_SHA: this-sha-does-not-exist-abc123
```

**Step 2** — push the scratch branch and open a scratch PR titled `release: bump workspace to vtest-synthesized-failure` targeting `main`.

**Step 3** — merge the scratch PR.

**Step 4** — watch the auto-tag workflow fire and fail:

```bash
gh run watch <workflow-run-id> --repo kusari-oss/mikebom
```

**Expected**: workflow ends in `failure` status; the merged PR's page shows a red X in its post-merge status widget.

**Step 5** — revert the merged scratch PR immediately (close/revert) so main isn't corrupted.

If the failure surfaced clearly to a maintainer glancing at the PR page, US3 P2 SC-004 is satisfied.

## Path E — Verify pre-PR gate stays green

```bash
./scripts/pre-pr.sh
```

**Expected**: `>>> all pre-PR checks passed.` — since m171 changes only YAML + Markdown, the pre-PR gate is a lightweight smoke check.

## Path F — Verify docs section is complete (FR-007)

Confirm `docs/releases.md` exists and contains the four sections named in `research.md` §R2:

```bash
grep -E "^#" docs/releases.md
```

**Expected** (at minimum):

```
# Releases
## Overview
## The release PR
## The auto-tag mechanism
## Failure playbook
```

Absence of any of these section headings is a docs incompleteness — fix before merging.
