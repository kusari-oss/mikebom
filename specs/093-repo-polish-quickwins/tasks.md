---
description: "Task list for milestone 093 — repo polish quick-wins cleanup"
---

# Tasks: Repo polish — quick-wins cleanup pass

**Input**: Design documents from `/Users/mlieberman/Projects/mikebom/specs/093-repo-polish-quickwins/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Not applicable — docs/metadata-only milestone. No new Rust tests added; verification is via the existing pre-PR gate + content-shape greps documented in `contracts/repo-surface-contracts.md`.

**Organization**: Tasks grouped by user story. US1 (P1) is the MVP increment (fix the broken install URL). US2–US3 (P2) and US4–US5 (P3) follow in priority order. Because each user story touches a different file, US1–US5 are mutually independent and can be implemented in any order after Setup.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: User story this task belongs to (US1–US5)
- File paths are workspace-relative.

## Path Conventions

Repo-root files (`README.md`, `SECURITY.md`, `CONTRIBUTING.md`), `.github/` files (`ISSUE_TEMPLATE/*.yml`, `pull_request_template.md`), and one workspace-member `Cargo.toml` metadata addition. Zero source-code changes.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify branch state before touching files.

- [X] T001 Confirm working branch is `093-repo-polish-quickwins`. Run `git status` and `git log -1 --oneline`; verify the branch was created by `/speckit.specify` and main is at `426517d` (alpha.30 release commit) or later.
- [X] T002 Confirm baseline pre-PR gate passes. Run `./scripts/pre-pr.sh` once on the unchanged tree; expect `>>> all pre-PR checks passed.` This isolates any post-edit failure as introduced by milestone 093 specifically.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: No shared infrastructure required — each user story is file-level independent. This phase exists only to confirm the `.github/` parent directory layout (it already contains `workflows/`; we'll add `ISSUE_TEMPLATE/` as a sibling).

- [X] T003 Confirm `.github/` exists and currently contains only `workflows/`. Run `ls .github/` and verify output is `workflows/`. The Phase 6 issue/PR-template tasks will add new files alongside it without modifying `workflows/`.

**Checkpoint**: Foundation ready (trivial — no setup actually needed). US1–US5 can now begin in any order.

---

## Phase 3: User Story 1 — README install commands actually work (Priority: P1) 🎯 MVP

**Goal**: Replace the broken `mlieberman85/mikebom.git` URL at README:238 with the canonical `kusari-sandbox/mikebom.git`. 100% of visitors who copy-paste the install instructions hit a working URL.

**Independent Test**: `grep -rn "mlieberman85" --include='*.md' --include='*.toml' --include='*.yml' .` returns zero matches. Cloning the canonical URL succeeds in a fresh shell.

### Implementation for User Story 1

- [X] T004 [US1] Fix the broken git URL in `README.md` (~line 238). Replace `https://github.com/mlieberman85/mikebom.git` with `https://github.com/kusari-sandbox/mikebom.git`. Use `sed -i.bak 's|mlieberman85/mikebom|kusari-sandbox/mikebom|g' README.md && rm README.md.bak` or an Edit tool invocation. Verify via `grep -rn "mlieberman85" --include='*.md' --include='*.toml' --include='*.yml' .` → expect zero matches (`exit 1` from grep is OK).
- [X] T005 [US1] Verify the canonical URL resolves. Run `git ls-remote https://github.com/kusari-sandbox/mikebom.git HEAD 2>&1 | head -1` and confirm output starts with a 40-char hex SHA (means the repo exists at that URL).

**Checkpoint**: US1 is complete. The MVP win lands here — fresh visitors can now copy-paste the install instructions without hitting a 404.

---

## Phase 4: User Story 2 — SECURITY.md exists and renders on GitHub Security tab (Priority: P2)

**Goal**: Create a `SECURITY.md` at the repo root with the four mandatory sections (per Contract 4 + data-model §SECURITY.md) so the GitHub Security-tab integration surfaces it and researchers know how to disclose vulnerabilities.

**Independent Test**: `test -f SECURITY.md && grep -cE '^## (Reporting a vulnerability|What to expect|Supported versions|Scope)' SECURITY.md` returns `4`. Post-merge: visiting `https://github.com/kusari-sandbox/mikebom/security/policy` renders the file.

### Implementation for User Story 2

- [X] T006 [P] [US2] Create `SECURITY.md` at the repo root with the four sections per `quickstart.md` Recipe 3 body (reporting channel via GHSA link, response-time expectation, supported-versions policy, in/out-of-scope classification). Use the exact body from quickstart Recipe 3 as the starting point; tweak prose for readability without dropping any of the four section headings.
- [X] T007 [US2] Verify Contract 4. Run `grep -c '^## ' SECURITY.md` → expect exactly 4 H2 headings. Run `grep -c "https://github.com/kusari-sandbox/mikebom/security/advisories/new" SECURITY.md` → expect ≥ 1 (the GHSA private-disclosure link is present).

**Checkpoint**: US2 complete. GitHub Security tab will auto-detect the file post-merge.

---

## Phase 5: User Story 3 — CONTRIBUTING.md onboards external contributors (Priority: P2)

**Goal**: Create a `CONTRIBUTING.md` at the repo root with the five mandatory sections (per Contract 5 + data-model §CONTRIBUTING.md) so new contributors know the speckit lifecycle, pre-PR gate, project principles, and where the planning artifacts live.

**Independent Test**: `grep -c '^## ' CONTRIBUTING.md` returns ≥ 5. The four critical-content greps from Contract 5 (`pre-pr.sh`, `constitution.md`, `MIKEBOM_REQUIRE_SPDX3_VALIDATOR`, speckit-lifecycle link) all succeed.

### Implementation for User Story 3

- [X] T008 [P] [US3] Create `CONTRIBUTING.md` at the repo root with the five mandatory sections per `quickstart.md` Recipe 4 body (Welcome, Workflow overview (speckit lifecycle), Local development setup, Pre-PR gate MANDATORY, Project principles + where to find them with all 12 constitution-principle one-liners). Use the exact body from quickstart Recipe 4 as the starting point.
- [X] T009 [US3] Verify Contract 5. Run, in order: `grep -c '^## ' CONTRIBUTING.md` (≥ 5), `grep -q 'pre-pr.sh' CONTRIBUTING.md`, `grep -q 'constitution.md' CONTRIBUTING.md`, `grep -q 'MIKEBOM_REQUIRE_SPDX3_VALIDATOR' CONTRIBUTING.md`, `grep -q 'speckit' CONTRIBUTING.md`. All five MUST succeed.

**Checkpoint**: US3 complete. New contributors have an explicit onboarding doc.

---

## Phase 6: User Story 4 — Bug reports + PRs use structured templates (Priority: P3)

**Goal**: Create YAML issue forms (`bug_report.yml` + `feature_request.yml`) and a Markdown PR-template under `.github/`. GitHub's issue-chooser UI presents the templates; PR-form auto-fills with the checklist.

**Independent Test**: All three files exist; YAML files parse cleanly (`python3 -c "import yaml; yaml.safe_load(open('...'))"`); PR template has ≥ 3 checkbox items; the three required-content greps from Contract 7 succeed.

### Implementation for User Story 4

- [X] T010 [P] [US4] Create `.github/ISSUE_TEMPLATE/bug_report.yml` per data-model §bug_report.yml (6-field YAML form: title, OS dropdown with the 6 platform options, mikebom-version input, command-output textarea, expected-vs-actual textarea, additional-context textarea). Top-level fields: `name: Bug report`, `description: Report a bug or unexpected behavior`, `title: "[BUG] "`, `labels: ['bug']`. Body field types: `markdown` (greeting), `dropdown`, `input`, `textarea` per the data-model spec.
- [X] T011 [P] [US4] Create `.github/ISSUE_TEMPLATE/feature_request.yml` per data-model §feature_request.yml (5-field YAML form: title, use-case textarea, proposed-solution textarea, alternatives-considered textarea optional, additional-context textarea optional). Top-level fields: `name: Feature request`, `description: Suggest a new feature or improvement`, `title: "[FEATURE] "`, `labels: ['enhancement']`.
- [X] T012 [P] [US4] Create `.github/ISSUE_TEMPLATE/config.yml` (optional but recommended). Body: `blank_issues_enabled: false` so all reports route through one of the structured templates. Skip `contact_links` (no discussions board active yet).
- [X] T013 [P] [US4] Create `.github/pull_request_template.md` with the 5-checkbox body per data-model §pull_request_template.md. MUST include: `pre-pr.sh`, `speckit lifecycle`, `goldens`, `mikebom:* property audit`, `MIKEBOM_REQUIRE_SPDX3_VALIDATOR for release-bump PRs`.
- [X] T014 [US4] Verify Contracts 6 + 7. Run: `test -f .github/ISSUE_TEMPLATE/bug_report.yml && test -f .github/ISSUE_TEMPLATE/feature_request.yml && test -f .github/pull_request_template.md`. Then validate YAML parses: `python3 -c "import yaml; yaml.safe_load(open('.github/ISSUE_TEMPLATE/bug_report.yml'))"` and the same for `feature_request.yml`. Then PR template content: `grep -c '^- \[ \]' .github/pull_request_template.md` (≥ 3), and the three required-content greps (`pre-pr.sh`, `speckit`, `goldens`).

**Checkpoint**: US4 complete. Post-merge, `https://github.com/kusari-sandbox/mikebom/issues/new/choose` will offer the template chooser.

---

## Phase 7: User Story 5 — cargo binstall works (Priority: P3)

**Goal**: Add the `[package.metadata.binstall]` table to `mikebom-cli/Cargo.toml` (per research.md §1's resolution) AND document the `cargo binstall --git ... mikebom` invocation in the README's `## Install` section.

**Independent Test**: `cargo +stable build -p mikebom` succeeds (metadata addition doesn't break parse). Contract 3 grep finds the `[package.metadata.binstall]` block with `pkg-url` + `pkg-fmt` lines. Contract 2 visual inspection confirms the new README subsection appears.

### Implementation for User Story 5

- [X] T015 [P] [US5] Append a `[package.metadata.binstall]` block to `mikebom-cli/Cargo.toml` per research.md §1's resolution. Exact content:
    ```toml
    [package.metadata.binstall]
    pkg-url = "{ repo }/releases/download/v{ version }/{ name }-v{ version }-{ target }{ archive-suffix }"
    pkg-fmt = "tgz"
    ```
    Place near the bottom of the file (after any existing `[package.metadata.*]` tables, or after `[dependencies]` if none).
- [X] T016 [P] [US5] Add a new `### Via cargo binstall (Rust toolchain users)` subsection to `README.md`'s `## Install` section per data-model §`README.md` Edit 2 + quickstart Recipe 2 body. Place AFTER the existing tarball-download paragraph and BEFORE "Or build from source". Body MUST include: one-line preamble, the exact `cargo binstall --git https://github.com/kusari-sandbox/mikebom mikebom` command in a fenced bash block, and a note that bare `cargo binstall mikebom` works after crates.io publication.
- [X] T017 [US5] Verify Contracts 2 + 3. Run `grep -A2 "\[package.metadata.binstall\]" mikebom-cli/Cargo.toml` → expect to see the table + pkg-url + pkg-fmt lines. Run `grep -A4 "cargo binstall" README.md` → expect to see the new subsection. Run `cargo +stable build -p mikebom 2>&1 | tail -3` → expect a successful build (cargo treats unknown metadata as opaque; no parse error).

**Checkpoint**: US5 complete. Rust-toolchain users have a one-command install path.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Diff-scope audit + final pre-PR gate to confirm zero scope creep and CI-readiness.

- [X] T018 Verify Contract 8 — diff scope guard. Run, in order:
    ```bash
    # Only allowed paths:
    git diff --name-only main \
      | grep -vE '^(README\.md|SECURITY\.md|CONTRIBUTING\.md|\.github/.+\.(md|yml)|mikebom-cli/Cargo\.toml)$' \
      | grep -v '^$' \
      | wc -l
    # Expected: 0

    # No source-tree changes:
    git diff --name-only main | grep -E '^(mikebom-cli/src|mikebom-common/src|xtask/src)/' | wc -l
    # Expected: 0

    # No golden regen:
    git diff --name-only main | grep -E '^mikebom-cli/tests/fixtures/golden/' | wc -l
    # Expected: 0

    # No Cargo.lock churn (FR-009):
    git diff --name-only main | grep -E '^Cargo\.lock$' | wc -l
    # Expected: 0
    ```
    If ANY check returns non-zero, STOP and investigate scope creep before proceeding to T019.
- [X] T019 Run the mandatory pre-PR gate per Contract 9. Run `./scripts/pre-pr.sh`. Expect: `>>> all pre-PR checks passed.` with zero clippy warnings and every test suite reporting `0 failed`. This is the CLAUDE.md mandatory gate; failure here blocks PR.
- [X] T020 Manual content-quality review pass. Re-read all five new files end-to-end (`SECURITY.md`, `CONTRIBUTING.md`, both YAML issue forms, the PR template) against the spec's User Stories acceptance scenarios. Specifically verify: a hypothetical visitor reading `SECURITY.md` knows where to report and what to expect; a hypothetical contributor reading `CONTRIBUTING.md` can answer the four SC-003 quiz questions. Fix any prose ambiguities.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: Depends on Setup. Trivial — only confirms `.github/` layout.
- **US1 / US2 / US3 / US4 / US5 (Phases 3–7)**: All depend on Foundational. **Mutually independent** because each touches a different file. Can ship in any order or in parallel.
- **Polish (Phase 8)**: Depends on US1–US5 being complete (verifies aggregate diff scope + runs the gate).

### User Story Dependencies

- **US1 (P1)**: Independent. Touches `README.md` only.
- **US2 (P2)**: Independent. Touches `SECURITY.md` only.
- **US3 (P2)**: Independent. Touches `CONTRIBUTING.md` only.
- **US4 (P3)**: Independent. Touches `.github/ISSUE_TEMPLATE/*.yml` + `.github/pull_request_template.md`.
- **US5 (P3)**: Independent. Touches `mikebom-cli/Cargo.toml` + `README.md` (different section than US1's edit — appends a new subsection after the existing Install body).

Note: US1 and US5 both touch `README.md` but at different sections. Sequential commits (US1 first, then US5) avoid any merge edge case.

### Parallel Opportunities

- US2, US3, US4 internal tasks (T006, T008, T010–T013) — all touch different files; ideal for parallel agent execution.
- US5's two implementation tasks (T015 in `mikebom-cli/Cargo.toml`, T016 in `README.md`) — different files; parallel.
- Phase-3 through Phase-7 tasks can interleave freely. Polish (Phase 8) is the only strict gate.

---

## Parallel Example: Phase 4–7 (Stories US2–US5)

```bash
# All four user stories touch different files; fan out:
Task: "Create SECURITY.md at repo root (T006)"
Task: "Create CONTRIBUTING.md at repo root (T008)"
Task: "Create .github/ISSUE_TEMPLATE/bug_report.yml (T010)"
Task: "Create .github/ISSUE_TEMPLATE/feature_request.yml (T011)"
Task: "Create .github/ISSUE_TEMPLATE/config.yml (T012)"
Task: "Create .github/pull_request_template.md (T013)"
Task: "Add binstall metadata to mikebom-cli/Cargo.toml (T015)"
Task: "Add cargo binstall README subsection (T016)"
```

The four `Verify` tasks (T007, T009, T014, T017) run after their corresponding implementation tasks complete.

---

## Implementation Strategy

### MVP First (US1 only)

1. Phase 1: Setup (T001–T002)
2. Phase 2: Foundational (T003 — trivial)
3. Phase 3: US1 (T004–T005) — URL fix
4. **STOP and VALIDATE**: confirm the README clone command resolves; this single change is shippable as its own one-line PR if needed.

### Incremental Delivery

1. Setup + Foundational → ready
2. US1 → fixes the immediate broken link → could ship alone
3. US2 + US3 → add SECURITY.md + CONTRIBUTING.md → community-health docs ready
4. US4 → issue + PR templates → structured contributor flow
5. US5 → cargo binstall → Rust-toolchain quick-install path
6. Polish → diff-scope audit + pre-PR gate → ready for PR

### Single-Developer Strategy (this milestone)

This is a small docs-only milestone; one developer does it in a single pass:

1. T001–T003 (setup, ~3 min)
2. T004–T005 (US1, ~5 min — Edit + grep verification)
3. T006–T007 (US2, ~10 min — write SECURITY.md using quickstart Recipe 3 body)
4. T008–T009 (US3, ~15 min — write CONTRIBUTING.md using quickstart Recipe 4 body)
5. T010–T014 (US4, ~20 min — write three template files; the YAML schema is the longest part)
6. T015–T017 (US5, ~10 min — Cargo.toml table + README subsection)
7. T018–T020 (Polish, ~5 min — diff audit + pre-PR gate + content review)

Total: ~70 minutes including the pre-PR gate. Target ≤500 lines of diff across all changes.

---

## Notes

- [P] markers = different files OR different sections within the same file with no implicit dependency.
- [Story] label maps task to specific user story for traceability.
- The five user stories are file-level independent — each story owns its file set and produces no cross-story merge edges (US1 + US5 both touch README.md but at different sections; sequential commits in that order keep history clean).
- Verify tests fail pre-fix and pass post-fix per TDD discipline does NOT apply here — no code change, no Rust tests added; verification is content-shape greps + pre-PR gate.
- The mandatory pre-PR gate (Contract 9 / SC-006) is the only automated CI signal. Pass = milestone is ready for PR.
- Commit boundary suggestion: one commit per user-story phase (4–5 commits total) for clean atomic history, OR squash to a single commit at PR time.
- Avoid: touching any path outside the Contract 8 allowlist (`README.md`, `SECURITY.md`, `CONTRIBUTING.md`, `.github/**.{md,yml}`, `mikebom-cli/Cargo.toml`). FR-007 / FR-008 / FR-009 are hard scope guardrails.
