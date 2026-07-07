# Implementation Plan: Fix auto-tag-release.yml permission bug

**Branch**: `171-fix-auto-tag-perms` | **Date**: 2026-07-06 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/171-fix-auto-tag-perms/spec.md`

## Summary

**Primary requirement**: `auto-tag-release.yml` must successfully create + push the release tag on release-PR merge without maintainer intervention (US1 P1). Per spec Q1 clarification: use a fine-grained PAT stored as repo secret `RELEASE_TAG_TOKEN`, owned by a service account, scoped to `contents: write` on `kusari-oss/mikebom` ONLY.

**Technical approach**: Three coordinated changes:

1. **Provision `RELEASE_TAG_TOKEN` secret** (out-of-band, manual — repo admin action). A service account (existing or new — planning research resolves this) creates a fine-grained PAT with:
   - Repository access: only `kusari-oss/mikebom`
   - Permissions: `contents: write` (only)
   - Expiration: maximum (1 year for fine-grained PATs)
   
   The PAT is stored as repo secret `RELEASE_TAG_TOKEN` under Settings → Secrets and variables → Actions.

2. **Modify `.github/workflows/auto-tag-release.yml`** to consume the secret:
   - `actions/checkout` step: pass `token: ${{ secrets.RELEASE_TAG_TOKEN }}` so the persisted credential (if any) has the right scope. Even though the workflow's current `persist-credentials: false` skips this, the explicit token argument is defense-in-depth per the memory `feedback_sha_pin_before_dependabot`-style hygiene.
   - `Extract version + create + push tag` step: change the `GH_TOKEN` env from `secrets.GITHUB_TOKEN` to `secrets.RELEASE_TAG_TOKEN`. The push URL construction (`https://x-access-token:${GH_TOKEN}@github.com/${REPO}.git`) stays; only the token source changes.
   - Add a graceful fallback: if `RELEASE_TAG_TOKEN` is missing (e.g., not yet provisioned or accidentally deleted), the step exits with a clear, actionable error naming the recovery playbook: "PAT missing — see docs/releases.md § auto-tag mechanism".

3. **Add documentation** (per FR-007) at `docs/releases.md` (or the closest equivalent — planning research locates the actual doc). New section: "How auto-tag-release.yml works" covering the workflow's happy path, the `RELEASE_TAG_TOKEN` dependency, the recovery playbook for token expiration/rotation, and the manual `git tag && git push` fallback for pre-171 releases per memory `feedback_release_pr_title_format`.

**Additional US2 (P2) work**: Retroactive audit of `auto-tag-release.yml` run history — surfaces prior releases that silently failed or required manual recovery. Attaches the audit table to the m171 PR body per FR-005.

**US3 (P2) work**: The failure-loud guarantee. Post-fix analysis confirms that GitHub Actions' default status widgets already surface workflow failures on the merged PR page — no additional wiring required. US3 becomes a verification-only task set (like m170's US2).

**Blast radius**: 1 YAML file edited (`auto-tag-release.yml`, ~5 lines changed), 1 docs file edited (`docs/releases.md`, +1 section), 0 Rust source files touched, 0 goldens changed. Plus 2 out-of-band manual actions: (a) provision `RELEASE_TAG_TOKEN` secret in repo settings, (b) confirm/create the service account.

## Technical Context

**Language/Version**: N/A — this is a GitHub Actions workflow YAML + Markdown docs change. No Rust code touched.

**Primary Dependencies**: Existing GitHub Actions (`actions/checkout@9c091bb...`), the standard GitHub Actions token model, `gh` CLI (already used in the workflow). **No new external dependencies.**

**Storage**: N/A — no persistent state on mikebom's side. The `RELEASE_TAG_TOKEN` secret lives in GitHub's repo-secrets store, managed by GitHub, not by mikebom code.

**Testing**: Two verification layers:
- **Static**: workflow YAML lint via `actionlint` (already in the pre-PR gate — planning research confirms).
- **Live smoke** (FR-008): a synthetic release-shaped PR against a scratch branch, exercising the auto-tag workflow end-to-end without publishing a real release. Details in the quickstart.md.

**Target Platform**: GitHub Actions runners (`ubuntu-latest`) — same as existing workflow.

**Project Type**: workflow-config-change (a subset of "cli" — mikebom is a cli, this milestone changes CI-side workflows only).

**Performance Goals**: N/A — the workflow already completes in ~15 seconds; the fix adds zero new steps.

**Constraints**: 
- SC-005 requires attaching the retroactive audit table to the PR body — this is a manual maintainer action, not a build step, so the plan tracks it as a task rather than an automated check.
- SC-008 requires no regression on other workflows — planning research confirms which workflows share state with `auto-tag-release.yml` (probably none, but need to verify).

**Scale/Scope**: Tiny. 1 YAML file, 1 docs file, ~5 lines of workflow YAML changed, 1 new docs section (~50 lines).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Pure Rust, Zero C**: ✅ No Rust code touched. Zero new dependencies. The change is entirely in GitHub Actions YAML + Markdown.
- **II. eBPF-Only Observation**: ✅ `mikebom-ebpf` untouched.
- **III. Fail Closed**: ✅ Post-171 the workflow fails closed on missing `RELEASE_TAG_TOKEN` with a clear diagnostic — MORE fail-closed than pre-171's silent 403 was.
- **IV. Type-Driven Correctness**: N/A — no Rust types touched.
- **V. Specification Compliance**: N/A — no SBOM emission code touched.
- **VI. Three-Crate Architecture**: ✅ No crate structure changes.
- **VII. Test Isolation**: N/A — no test isolation concerns.
- **VIII. Completeness**: N/A — no SBOM completeness code touched.
- **IX. Accuracy**: N/A — no SBOM output touched.
- **X. Transparency**: ✅ **Improved**. Post-171 the workflow's failure paths are documented and load-bearing signals (the workflow-fail status check) surface to maintainers. Pre-171 was worse: the 403 failure surfaced only in workflow logs a maintainer had to dig for.
- **XI. Enrichment**: N/A.
- **XII. External Data Source Enrichment**: N/A.

**Strict Boundaries check**: 
- No new subprocess calls (the workflow already shells out to `git`).
- No new network access (uses the existing GitHub API).
- No new filesystem writes.
- No new `mikebom:*` annotations.
- No new Cargo dependencies.

**Verdict**: All principles pass. Zero violations.

## Project Structure

### Documentation (this feature)

```text
specs/171-fix-auto-tag-perms/
├── plan.md              # This file
├── research.md          # Phase 0 output — historical audit + doc-location discovery + service-account status
├── data-model.md        # Phase 1 output — 3 entities (workflow, secret, service account)
├── quickstart.md        # Phase 1 output — manual verification recipe + smoke-test procedure
├── contracts/           # Phase 1 output — YAML shape contract for the workflow diff + secret-provisioning runbook
├── checklists/          # Requirements checklist (spec-phase output)
└── tasks.md             # Phase 2 output (/speckit.tasks — NOT created by /speckit.plan)
```

### Source Code (repository root)

Files touched by this feature:

```text
.github/workflows/
└── auto-tag-release.yml         # 5-line change: token source + graceful-fallback error message

docs/
└── releases.md                  # (or closest equivalent — see research §R2) + 1 new section
```

Files NOT touched:
- Any Rust source in `mikebom-cli/`, `mikebom-common/`, `mikebom-ebpf/`.
- Any golden fixtures.
- Any other workflow YAML.
- `Cargo.toml` / `Cargo.lock`.

**Structure Decision**: Workflow + docs-only change, no crate work. Constitution VI's three-crate architecture is untouched.

## Complexity Tracking

No Constitution violations; no complexity to track. This is a small config + docs change.

## Phase 0 — Outline & Research

Research questions this feature raises:

1. **Does a `kusari-oss` org-owned service account already exist?** — if yes, use it; if no, provision one. The service account's identity affects secret ownership + rotation responsibility.
2. **Where does the "release process" documentation actually live?** — spec assumes `docs/releases.md` but the actual file may be `docs/RELEASES.md`, `CONTRIBUTING.md#releases`, or `docs/reference/release-process.md`. Need to grep + decide.
3. **What's the audit history of `auto-tag-release.yml`?** — retroactive US2 audit. `gh run list --repo kusari-oss/mikebom --workflow auto-tag-release.yml --limit 50`. Classify each run.
4. **Does the pre-PR gate include workflow-YAML linting?** — if `actionlint` isn't in the pre-PR gate, add a note that the m171 change was linted manually.
5. **Which other workflows share tokens or state with `auto-tag-release.yml`?** — need to confirm there's no cross-workflow coupling that would create SC-008 regression risk.

`research.md` will consolidate.

## Phase 1 — Design & Contracts

Design outputs for this feature:

- **data-model.md** — three affected entities per spec Key Entities:
  1. `auto-tag-release.yml` workflow (before/after YAML diff shape).
  2. `RELEASE_TAG_TOKEN` repo secret (lifecycle + rotation policy).
  3. Service account identity (name, ownership, audit trail).

- **contracts/** — thin folder:
  1. `workflow-diff.md` — the exact YAML diff that will land + a reviewer checklist. This is the "contract" between the m171 PR and the reviewer.
  2. `secret-provisioning.md` — the exact steps the maintainer follows to create the fine-grained PAT + store it as the secret. Written so a first-time maintainer can execute it without asking anyone.

- **quickstart.md** — three-part manual verification:
  1. Pre-fix reproduction (already documented in issue #519 — reference it).
  2. Post-fix live-smoke path (FR-008): open a scratch release-shaped PR, merge, verify tag appears.
  3. Post-fix failure-loud verification (US3): synthesize a workflow failure (e.g., invalid MERGE_SHA), verify the red status check surfaces on the merged PR.

- Agent context update via `.specify/scripts/bash/update-agent-context.sh claude` — appends the m171 CI-only-change note to `CLAUDE.md`'s tech list.

Post-design Constitution re-check: no drift from Phase 0 verdict. All principles remain green.
