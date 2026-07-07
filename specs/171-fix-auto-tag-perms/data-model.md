# Phase 1 Data Model: m171 Auto-Tag Permission Fix

**Feature**: 171-fix-auto-tag-perms
**Date**: 2026-07-06

Three entities are affected. Each documented as a before/after pair, plus one net-new entity (`RELEASE_TAG_TOKEN` secret) and one implicit entity (the service account).

## Entity 1 — `.github/workflows/auto-tag-release.yml`

**Type**: GitHub Actions workflow YAML.

### Pre-171 shape (concrete excerpts)

Checkout step:

```yaml
- uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7.0.0
  with:
    ref: ${{ github.event.pull_request.merge_commit_sha }}
    fetch-depth: 0
    persist-credentials: false
```

Tag-push step env block:

```yaml
env:
  GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  MERGE_SHA: ${{ github.event.pull_request.merge_commit_sha }}
```

The `secrets.GITHUB_TOKEN` is the default workflow token, silently narrowed to read-only by the repo-level workflow-permissions setting per research §R3.

### Post-171 shape

Checkout step:

```yaml
- uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7.0.0
  with:
    ref: ${{ github.event.pull_request.merge_commit_sha }}
    fetch-depth: 0
    persist-credentials: false
    # Milestone 171: use the fine-grained PAT so the push step
    # authenticates as the service account, not github-actions[bot].
    token: ${{ secrets.RELEASE_TAG_TOKEN }}
```

Tag-push step env block:

```yaml
env:
  # Milestone 171: switched from secrets.GITHUB_TOKEN (silently
  # narrowed to read-only by the org-level workflow-permissions
  # policy per research §R3) to a fine-grained PAT stored as
  # secrets.RELEASE_TAG_TOKEN, scoped to contents:write on this repo
  # only. See docs/releases.md § auto-tag mechanism for rotation.
  GH_TOKEN: ${{ secrets.RELEASE_TAG_TOKEN }}
  MERGE_SHA: ${{ github.event.pull_request.merge_commit_sha }}
```

Plus a graceful fallback at the top of the tag-push step's `run:` block:

```bash
if [ -z "${GH_TOKEN}" ]; then
  echo "::error::RELEASE_TAG_TOKEN secret is missing or empty."
  echo "Recovery: see docs/releases.md § auto-tag mechanism."
  echo "Interim workaround: manually tag via:"
  echo "  git tag -a v0.1.0-alpha.<N> -m 'Release v...' <merge-sha>"
  echo "  git push origin v0.1.0-alpha.<N>"
  exit 1
fi
```

**Deltas**:
- Checkout step: **added** `token: ${{ secrets.RELEASE_TAG_TOKEN }}` (defense in depth — persist-credentials is off, but future-proofs against a `persist-credentials: true` reintroduction).
- Tag-push env block: **changed** `GH_TOKEN` source from `secrets.GITHUB_TOKEN` to `secrets.RELEASE_TAG_TOKEN`.
- Tag-push run block: **added** an empty-token guard at the top.
- **NOT changed**: the workflow's `if:` guard, permissions block, workflow-level env, or any other step.

**Blast radius**: 5 lines changed in the YAML (excluding the comments), plus 6 lines added for the guard. Total ~11 lines.

## Entity 2 — `RELEASE_TAG_TOKEN` repo secret

**Type**: GitHub repo secret, stored under Settings → Secrets and variables → Actions.

**Provisioning** (out-of-band, manual, one-time by a repo admin):

1. Service account (see Entity 3) creates a fine-grained PAT at https://github.com/settings/tokens?type=beta with:
   - **Token name**: `mikebom-release-tag-token`
   - **Expiration**: 1 year (GitHub's fine-grained PAT max)
   - **Resource owner**: `kusari-oss`
   - **Repository access**: "Only select repositories" → `kusari-oss/mikebom` (exactly one repo)
   - **Repository permissions**: `Contents: Read and write` (only; every other permission left at the default "No access")
2. Copy the PAT value.
3. In the mikebom repo: Settings → Secrets and variables → Actions → New repository secret. Name: `RELEASE_TAG_TOKEN`. Value: the PAT.
4. Verify the secret appears in the secrets list. Verify no other workflows reference it (grep — research §R5 already confirmed zero cross-workflow use).

**Rotation policy**:
- **Frequency**: annual (before GitHub's 1-year expiration).
- **Reminder**: create a calendar reminder for 10 months out, giving 2 months of margin.
- **Recovery**: if the PAT expires unrenewed, the guard in Entity 1's Post-171 shape catches it — the workflow fails cleanly with a maintainer-actionable error, and the human maintainer manually tags per the memory `feedback_release_pr_title_format` until the PAT is renewed.

**Access control**:
- Only repo admins can create/read/delete the secret.
- The secret's value is not readable after creation — GitHub masks it in workflow logs and in the UI.
- The service account owning the PAT has admin trust equivalent to any human admin on this repo.

## Entity 3 — Service account

**Type**: GitHub user account, ideally owned by the `kusari-oss` org.

**Status**: **UNKNOWN — planning-phase discovery elevated to a plan-review question** per research §R1. Either:

- Option A: A `kusari-oss-bot` (or similarly-named) service account already exists as a private org member. If so: use it for the PAT.
- Option B: No service account exists. Interim: Michael provisions the PAT under his personal account with the fine-grained scope; long-term: file a follow-up milestone to provision a proper service account and rotate the PAT to it.

**Requirements the account must meet**:
- Must be added to the `kusari-oss/mikebom` repo as an admin or maintainer (needs write access to push tags).
- Should be added to a mikebom-owner team so future rotations don't need re-inviting the account per-time.
- Must have 2FA enabled (GitHub org policy compliance).

**Documentation obligation** (per FR-007): whichever account owns the PAT is documented by name in `docs/releases.md` § auto-tag mechanism, so the NEXT maintainer knows who to contact when the PAT rotates or a permission audit fires.

## Entity 4 — `docs/releases.md` (new file)

**Type**: Markdown documentation, new file at repo root's `docs/` subtree per research §R2.

**Sections** (~50 lines total):

1. **Overview** — mikebom's release cadence (2-3/week), semantic versioning + `alpha.N` suffix, what triggers a release (a maintainer decision to bump the workspace version).
2. **The release PR** — title format `release: bump workspace to v` per memory `feedback_release_pr_title_format`; the golden-regen sweep per memory `feedback_release_bump_regen_goldens`; the version-string cascade into SPDX IDs; skip local pre-PR per memory `feedback_release_bump_prepr_slow`.
3. **The auto-tag mechanism** — how `auto-tag-release.yml` works post-171; the `RELEASE_TAG_TOKEN` secret dependency; the service-account ownership + rotation cadence; who to contact for PAT renewal.
4. **Failure playbook** — how to detect that the workflow failed (red status check on the merged PR); manual recovery via `git tag -a v<N> -m 'Release v...' <merge-sha> && git push origin v<N>`; when to rotate the PAT (annual + on-suspicion-of-compromise); watch item on issue #519's audit follow-up.

## Cross-entity invariants (post-171)

1. **Zero-touch releases**: For any merged PR whose title starts with `release: bump workspace to v`, the tag `v<version>` MUST appear on `origin` within 30 seconds of the merge event without any human running `git tag` or `git push`.
2. **Loud failures**: If Entity 1's tag-push step fails for any reason (missing/expired PAT, network error, permission narrowing), the workflow run MUST end in `failure` state — no silent-success paths.
3. **Documented ownership**: Entity 4 MUST name (a) the service account that owns the PAT, and (b) the rotation cadence. First-time maintainers running a release MUST be able to find that info without asking anyone.
4. **Minimum blast radius**: Entity 2's PAT scope MUST be `contents: write` on this repo only. No expansion to `metadata`, no expansion to other repos.

## State transitions

None on the workflow-run side. The PAT itself has a lifecycle (`create → in-use → approaching-expiration → rotate → revoke-old`) tracked in Entity 4's docs section but no state machine in mikebom code.
