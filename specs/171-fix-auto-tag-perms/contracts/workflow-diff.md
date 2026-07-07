# Contract: `.github/workflows/auto-tag-release.yml` post-m171 diff

**Feature**: 171-fix-auto-tag-perms
**Date**: 2026-07-06

This is the exact YAML change that will land. Reviewer treats this as the authoritative shape — deviations are grounds for review comments.

## Diff (unified, indicative)

```diff
       - uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7.0.0
         with:
           ref: ${{ github.event.pull_request.merge_commit_sha }}
           fetch-depth: 0
-          persist-credentials: false
+          persist-credentials: false
+          # Milestone 171 (closes #519): use the fine-grained PAT so
+          # the push step authenticates as the service account, not
+          # github-actions[bot]. persist-credentials stays false —
+          # the push URL is built explicitly below.
+          token: ${{ secrets.RELEASE_TAG_TOKEN }}

       - name: Extract version + create + push tag
         id: tag
         env:
-          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
+          # Milestone 171 (closes #519): switched from
+          # secrets.GITHUB_TOKEN (silently narrowed to read-only by
+          # the org-level workflow-permissions policy — see
+          # docs/releases.md § auto-tag mechanism) to a fine-grained
+          # PAT scoped to contents:write on this repo only.
+          GH_TOKEN: ${{ secrets.RELEASE_TAG_TOKEN }}
           MERGE_SHA: ${{ github.event.pull_request.merge_commit_sha }}
           REPO: kusari-oss/mikebom
         run: |
           set -euo pipefail
+          # Milestone 171: fail-loud on missing PAT so the maintainer
+          # gets an actionable diagnostic instead of a 403 stack trace.
+          if [ -z "${GH_TOKEN}" ]; then
+            echo "::error::RELEASE_TAG_TOKEN secret is missing or empty."
+            echo "Recovery: see docs/releases.md § auto-tag mechanism."
+            echo "Interim workaround: manually tag via"
+            echo "  git tag -a v0.1.0-alpha.<N> -m 'Release v...' <merge-sha>"
+            echo "  git push origin v0.1.0-alpha.<N>"
+            exit 1
+          fi
           # (existing shell body — version extract, tag create, push — unchanged)
```

Not shown: the full shell body of the tag-create-and-push step stays byte-identical. Only the env source + the leading guard change.

## Line-count budget

- **Additions**: ~14 lines (2 comment blocks + `token:` + guard).
- **Removals**: 1 line (the old `GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}` — replaced by 2 lines of comment + 1 line of new env).
- **Net**: +13 lines.

If the actual diff meaningfully exceeds this budget (say, +30 lines), the reviewer flags it as suspicious scope creep.

## Reviewer checklist

Before approving the m171 PR, the reviewer confirms:

- [ ] The `RELEASE_TAG_TOKEN` secret is provisioned in the repo (Settings → Secrets and variables → Actions shows an entry named exactly `RELEASE_TAG_TOKEN`).
- [ ] The secret's owning PAT is scoped to `contents: write` on `kusari-oss/mikebom` ONLY (verified out-of-band via the service account's PAT settings page).
- [ ] The workflow YAML change matches the diff shape above — no other steps modified, no other permissions widened, no other secrets referenced.
- [ ] The `docs/releases.md` new section names (a) which service account owns the PAT, (b) the annual rotation cadence, (c) the manual-recovery playbook.
- [ ] The workflow's existing `if:` guard (`startsWith(github.event.pull_request.title, 'release: bump workspace to v')`) is unchanged.
- [ ] `python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/auto-tag-release.yml"))'` (or `yq . <path>`) exits 0 — the YAML is well-formed.
- [ ] Pre-PR gate `./scripts/pre-pr.sh` is green (lightweight for a CI-only change).

## Non-goals for m171

- Do NOT change the workflow's title-match `if:` guard.
- Do NOT change the workflow's `permissions:` block at file scope (leaving `contents: write` + `actions: write` in place is fine — the fine-grained PAT is what actually authenticates the push; the file-level `permissions:` becomes a documentation hint about intent).
- Do NOT add `actionlint` to CI (research §R4 defers this to a follow-up milestone).
- Do NOT touch `release.yml` (research §R5 confirmed zero cross-workflow coupling).
