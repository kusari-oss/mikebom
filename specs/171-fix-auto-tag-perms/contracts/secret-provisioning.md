# Contract: `RELEASE_TAG_TOKEN` secret provisioning runbook

**Feature**: 171-fix-auto-tag-perms
**Date**: 2026-07-06

Step-by-step runbook for creating the `RELEASE_TAG_TOKEN` secret. Written so a first-time maintainer can execute it without asking anyone.

## Prerequisites

- Repo-admin access on `kusari-oss/mikebom`.
- Access to a service account (or an interim personal-account fallback per data-model.md Entity 3).
- A password manager to store the PAT value until it lands in the repo secret (never paste into Slack, email, or chat).

## Step 1 — Sign in as the service account

Sign in to GitHub as the service account. If no service account exists yet, sign in as your personal account and treat this as an interim provisioning until the org creates a proper service account.

## Step 2 — Create the fine-grained PAT

1. Navigate to https://github.com/settings/tokens?type=beta.
2. Click "Generate new token" → "Fine-grained personal access token".
3. Fill in:

| Field | Value |
|---|---|
| Token name | `mikebom-release-tag-token` |
| Description | `Grants auto-tag-release.yml the contents:write scope needed to push v0.1.0-alpha.N tags on release-PR merge. Rotate annually. See docs/releases.md § auto-tag mechanism.` |
| Expiration | Custom → 365 days from today |
| Resource owner | `kusari-oss` |
| Repository access | "Only select repositories" → select `kusari-oss/mikebom` (exactly one repo — do NOT select any others) |

4. Under **Repository permissions**:
   - `Contents`: change from "No access" to "Read and write".
   - Leave EVERY OTHER permission at "No access". Do not grant Metadata, Actions, Pull requests, Issues, Deployments, Secrets, etc.

5. Under **Account permissions**: leave everything at default (no changes).

6. Click "Generate token".

7. **Copy the token value into a password manager immediately** — GitHub only shows it once.

## Step 3 — Verify the PAT's scope

Before storing it in the repo secret, verify the PAT works and has ONLY the intended scope:

```bash
# Should succeed (Contents:read is included in Contents:write):
gh api repos/kusari-oss/mikebom --header "Authorization: Bearer <PAT>" | jq '.name'
# → "mikebom"

# Should fail with 403 (proves the PAT can't access other repos):
gh api repos/kusari-oss/some-other-repo --header "Authorization: Bearer <PAT>" 2>&1 | head -3
# → error like "Not Found" or "Not accessible by fine-grained personal access tokens"

# Should fail with 403 (proves the PAT can't read Actions):
gh api repos/kusari-oss/mikebom/actions/workflows --header "Authorization: Bearer <PAT>" 2>&1 | head -3
# → error indicating the PAT lacks the Actions scope
```

## Step 4 — Store as repo secret

1. Navigate to `https://github.com/kusari-oss/mikebom/settings/secrets/actions`.
2. Click "New repository secret".
3. Fill in:
   - **Name**: `RELEASE_TAG_TOKEN` (exact match, uppercase — the workflow references this exact string)
   - **Secret**: paste the PAT value.
4. Click "Add secret".
5. Verify the secret appears in the list. GitHub masks the value; you cannot re-read it after saving.

## Step 5 — Verify no other workflow references the secret

Belt-and-braces check (research §R5 already confirmed zero use, but future contributors may add references):

```bash
grep -rln "RELEASE_TAG_TOKEN" .github/workflows/
# Post-m171 should print exactly one path:
#   .github/workflows/auto-tag-release.yml
```

If other paths appear, investigate — the secret's blast radius has expanded beyond spec.

## Step 6 — Record rotation reminder

Set a calendar reminder for 10 months from today (2 months of margin before the PAT's 365-day expiration). Reminder text:

> Rotate `RELEASE_TAG_TOKEN` on `kusari-oss/mikebom`. Runbook: `specs/171-fix-auto-tag-perms/contracts/secret-provisioning.md`. Recovery playbook if you miss it: `docs/releases.md` § auto-tag mechanism.

## Step 7 — Document the service account identity

In `docs/releases.md` § auto-tag mechanism, record:

- The GitHub username that owns the PAT.
- The rotation cadence (annual).
- The last rotation date (today).
- The next rotation date (365 days from today, minus 2 months of margin).

Rationale: when the next maintainer investigates a permission failure or a rotation reminder fires, they need to know which account to sign in as without asking the previous maintainer.

## Rotation runbook (annual)

At rotation time:

1. Sign in as the service account.
2. Repeat Steps 2-3 above (generate a new PAT with the same scope).
3. Update the `RELEASE_TAG_TOKEN` secret with the new value (Step 4 — "New repository secret" flow will let you overwrite the existing entry, or delete + recreate).
4. Verify the workflow works via the quickstart's live-smoke path.
5. Revoke the OLD PAT at https://github.com/settings/tokens — don't leave it lingering.
6. Update the last-rotation date in `docs/releases.md`.

## Emergency revocation

If the PAT is suspected compromised:

1. Immediately: https://github.com/settings/tokens → find the token → Revoke.
2. `auto-tag-release.yml` will start failing for the next release (empty-token guard catches it — Entity 1 Post-171 shape).
3. Follow the manual `git tag && git push` recovery playbook from `docs/releases.md`.
4. Provision a NEW PAT following Steps 2-4.
5. Post-incident: audit `gh api /user/tokens/xxx/log` (or the org security log) for any writes made by the compromised PAT.
