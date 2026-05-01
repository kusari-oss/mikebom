---
description: "Task list — milestone 047 SBOM-scope self-description"
---

# Tasks: Self-describe SBOM scope (README explainer + SPDX `comment` parity)

**Input**: spec.md ✅, plan.md ✅, checklists/requirements.md ✅. (No
research.md / data-model.md / contracts/ / quickstart.md — same
4-file tighter template milestones 021/022/023/042/046 use; the
plan resolves the SPDX 3 slot lookup inline in §Phase 0.)

**Tests**: included as inline unit tests for the new comment-text
builder, plus jq-shaped acceptance assertions per FR, plus the
existing byte-identity goldens regen surface (18 SPDX files), plus
`holistic_parity` continuing to pass.

**Organization**: Two user stories. Phase ordering inverts strict
priority (US2 before US1) because US1's README explainer
references US2's just-shipped SPDX comment field per FR-002(c) —
shipping US2 first lets the README cite real behavior. Story
labels still reflect spec priority. See "Dependency graph"
section.

## Format: `[ID] [P?] [Story?] Description`

---

## Phase 1: Setup

- [ ] T001 Confirm clean working tree on branch `047-scope-self-description`. `git status` shows only the un-tracked `specs/047-scope-self-description/` scaffolding from `/speckit.specify`.
- [ ] T002 `./scripts/pre-pr.sh` clean (baseline; should pass since no edits yet).

---

## Phase 2: Foundational

- [ ] T003 Add `pub enum ScopeMode { Artifact, Manifest }` to `mikebom-cli/src/generate/mod.rs`. Place near `OutputConfig` / `ScanArtifacts` definitions. Derive `Debug, Clone, Copy, PartialEq, Eq`.
- [ ] T004 Add `pub scope_mode: ScopeMode` field to `mikebom-cli/src/generate/mod.rs::ScanArtifacts`. Update Default impl + any test fixture builders that construct `ScanArtifacts` directly to set the new field. Default for tests: `ScopeMode::Artifact`.
- [ ] T005 Plumb `scope_mode` through `mikebom-cli/src/cli/scan_cmd.rs` at lines ~743–760: after `effective_include_declared_deps` resolves, set `scope_mode = if effective_include_declared_deps { ScopeMode::Manifest } else { ScopeMode::Artifact }` on the `ScanArtifacts` builder.
- [ ] T006 Create new module `mikebom-cli/src/generate/lifecycle_phases.rs` with two `pub fn`s: `tier_to_phase(tier: &str) -> Option<&'static str>` (extract from `cyclonedx/metadata.rs:37–46`) and `aggregate_phases<'a>(components: impl Iterator<Item = &'a ResolvedComponent>) -> Vec<&'static str>` (extract from `cyclonedx/metadata.rs:76–89`; preserve `BTreeSet` collector for deterministic sort). Register the module in `mikebom-cli/src/generate/mod.rs`.
- [ ] T007 Update `mikebom-cli/src/generate/cyclonedx/metadata.rs`: replace the inlined `tier_to_lifecycle_phase` and per-component aggregation with calls into `lifecycle_phases::tier_to_phase` / `lifecycle_phases::aggregate_phases`. Same algorithm, same sort order — CDX goldens MUST be byte-identical post-extraction.
- [ ] T008 Verify CDX goldens unchanged: `cargo +stable test -p mikebom --test cdx_regression` passes without regen. Any failure here means the extraction altered emission shape — fix before proceeding.

---

## Phase 3: Commit `feat(047/us2-1)` — SPDX 2.3 `creationInfo.comment`

**Goal**: Populate SPDX 2.3 `creationInfo.comment` with a document-level scope hint (scope mode + observed lifecycle phases + pointer to per-component annotations).

**Independent test**: `jq -r '.creationInfo.comment'` on any SPDX 2.3 output returns a non-empty string containing `Scope:` and `mikebom:sbom-tier`.

- [ ] T009 [US2] Edit `mikebom-cli/src/generate/spdx/document.rs:122–134`: add `pub comment: Option<String>` field to the `CreationInfo` struct with `#[serde(skip_serializing_if = "Option::is_none")]`. Default for the field is `None`.
- [ ] T010 [US2] Add a new `pub(super) fn build_scope_comment(scan: &ScanArtifacts<'_>) -> String` helper to `mikebom-cli/src/generate/spdx/document.rs` that:
    - Reads `scan.scope_mode` (`Artifact` → `"artifact (on-disk components only)"`, `Manifest` → `"manifest (declared transitives included)"`).
    - Calls `crate::generate::lifecycle_phases::aggregate_phases(scan.components)` for the deterministic sorted phase list.
    - Returns prose: `"Scope: <mode>. Observed lifecycle phases: <comma-list> (or 'no lifecycle phases observed' when the slice is empty). Per-component scope detail in mikebom:sbom-tier annotations."`
- [ ] T011 [US2] Edit `mikebom-cli/src/generate/spdx/document.rs::build_document` (line 217+): set `creation_info.comment = Some(build_scope_comment(scan))`. Always emit (per spec edge-case decision: predictable presence > conditional emission).
- [ ] T012 [P] [US2] Add inline test `build_scope_comment_emits_artifact_mode_with_phases` in `document.rs::tests`: synthetic `ScanArtifacts` with `scope_mode = Artifact` and 3 components carrying tiers `build`, `deployed`, `analyzed`; assert the returned string contains `Scope: artifact`, `Observed lifecycle phases: build, operations, post-build` (lexicographic phase order), and `mikebom:sbom-tier`.
- [ ] T013 [P] [US2] Add inline test `build_scope_comment_emits_manifest_mode` in `document.rs::tests`: synthetic `ScanArtifacts` with `scope_mode = Manifest` and any tier set; assert `Scope: manifest`.
- [ ] T014 [P] [US2] Add inline test `build_scope_comment_handles_empty_phases` in `document.rs::tests`: synthetic `ScanArtifacts` with all components carrying `sbom_tier = None`; assert the string contains `Scope:` and `no lifecycle phases observed` (per the empty-phases edge-case decision).
- [ ] T015 [US2] Run `cargo +stable test -p mikebom --bin mikebom -- spdx::document` and confirm 3 new tests pass.
- [ ] T016 [US2] Regen 9 SPDX 2.3 goldens: `MIKEBOM_UPDATE_SPDX_GOLDENS=1 cargo +stable test -p mikebom --test spdx_regression`.
- [ ] T017 [US2] Verify goldens regen is "comment-field delta + SHA cascade only" (no other semantic shifts): `git diff main..HEAD -- mikebom-cli/tests/fixtures/golden/spdx-2.3/ | grep -E '^[+-]' | grep -vE '^[+-]{3}|"comment":|"creationInfo":|SPDXRef|documentNamespace'` should be empty.
- [ ] T018 [US2] Verify SC-003 with the regen'd goldens: `jq -r '.creationInfo.comment' mikebom-cli/tests/fixtures/golden/spdx-2.3/maven.spdx.json` returns non-empty containing both `Scope:` and `mikebom:sbom-tier`.
- [ ] T019 [US2] Verify FR-011 / SC-006 (CDX unchanged): `git diff main..HEAD --stat -- mikebom-cli/tests/fixtures/golden/cyclonedx/` empty.
- [ ] T020 [US2] `./scripts/pre-pr.sh` clean.
- [ ] T021 [US2] Commit: `feat(047/us2-1): SPDX 2.3 creationInfo.comment with document-level scope hint + extract lifecycle_phases shared utility`.

---

## Phase 4: Commit `feat(047/us2-2)` — SPDX 3 `SpdxDocument.comment`

**Goal**: Same comment text on the SPDX 3 `SpdxDocument` Element. Per Phase 0 research: comment goes on `SpdxDocument` (not `CreationInfo`); JSON-LD key is plain `comment` (the existing `spdx-context.jsonld` already maps it).

**Independent test**: `jq -r '.["@graph"][] | select(.type == "SpdxDocument") | .comment'` returns non-empty containing `Scope:` and `mikebom:sbom-tier`.

- [ ] T022 [US2] Edit `mikebom-cli/src/generate/spdx/v3_document.rs:113–132`: in the `spdx_document` JSON-LD object construction, add a `"comment": <string>` key. Reuse `super::document::build_scope_comment(scan)` (path TBD — if cross-module privacy requires it, lift `build_scope_comment` to `mikebom-cli/src/generate/spdx/scope_comment.rs` so both `document.rs` and `v3_document.rs` import from a sibling module).
- [ ] T023 [P] [US2] Add inline test `spdx3_document_emits_scope_comment` in `v3_document.rs::tests`: synthetic minimal scan; assert the emitted JSON-LD has a `SpdxDocument` element with non-empty `comment` containing `Scope:` and `mikebom:sbom-tier`.
- [ ] T024 [US2] Run `cargo +stable test -p mikebom --bin mikebom -- spdx::v3_document` and confirm the new test passes.
- [ ] T025 [US2] Regen 9 SPDX 3 goldens: `MIKEBOM_UPDATE_SPDX3_GOLDENS=1 cargo +stable test -p mikebom --test spdx3_regression`.
- [ ] T026 [US2] Verify goldens regen is "comment + SHA cascade only" (the SHA-derived `SPDXID` / `documentNamespace` re-stamps are expected). Inspect the diff for any unexpected non-`comment`-related semantic shifts.
- [ ] T027 [US2] Verify SC-004: `jq -r '.["@graph"][] | select(.type == "SpdxDocument") | .comment' mikebom-cli/tests/fixtures/golden/spdx-3/maven.spdx3.json` returns non-empty containing both `Scope:` and `mikebom:sbom-tier`.
- [ ] T028 [US2] Verify `holistic_parity` still green: `cargo +stable test -p mikebom --test holistic_parity` 11/11 ok. (Document-level metadata is outside the C-row catalog so this should be untouched.)
- [ ] T029 [US2] `./scripts/pre-pr.sh` clean.
- [ ] T030 [US2] Commit: `feat(047/us2-2): SPDX 3 SpdxDocument.comment with document-level scope hint`.

---

## Phase 5: Commit `docs(047/us1)` — README "What kind of SBOM does mikebom emit?" explainer

**Goal**: Add a new top-level README section threading the now-shipped self-describing signals (artifact/manifest scope + 5-value `mikebom:sbom-tier` system + CDX `metadata.lifecycles[]` + SPDX `comment`) into a coherent narrative for operators comparing mikebom to trivy / syft.

**Independent test**: SC-001 (heading match), SC-002 (5 tier values + `--include-declared-deps` named).

- [ ] T031 [US1] Edit `README.md`: insert a new `## What kind of SBOM does mikebom emit?` section between the existing "Why" section and the "Install" section. The section MUST contain (in any reasonable order):
    - **The two-axis framing**: scope-mode (artifact vs manifest, document-level) + per-component lifecycle tier.
    - **The 5 sbom-tier values** with one-line definitions each: `design` (declared but not pinned, e.g., `>=1.0` ranges), `source` (lockfile-pinned, byte-resolvable), `build` (eBPF-traced build event), `deployed` (installed in the runtime image — dpkg, venv, node_modules), `analyzed` (artifact hash on disk).
    - **The CDX self-description trio**: `metadata.lifecycles[]` aggregation (CDX 1.6 native), `compositions[]` per-aggregate completeness, per-component `mikebom:sbom-tier` property.
    - **The SPDX self-description**: `creationInfo.comment` (SPDX 2.3, just shipped) and `SpdxDocument.comment` (SPDX 3, just shipped) — name the slot location for each format. Plus the same per-component `mikebom:sbom-tier` annotation for cross-format symmetry.
    - **One-paragraph mapping to industry / consumer terminology** so readers comparing mikebom to trivy / syft don't assume the tools answer the same question. Mikebom's default for `--image` is artifact (CDX phase: `operations`); trivy's container scan is closer to "build cache + image" hybrid.
    - **Default scope per scan mode**: `--image` → artifact, `--path` → manifest. Name `--include-declared-deps` as the toggle (auto-on for `--path`, auto-off for `--image`; explicit override available).
    - **Pointer to deeper rationale**: link to `docs/design-notes.md` "Scope: artifact vs manifest SBOM" section.
- [ ] T032 [US1] Verify SC-001: `grep -nE '^## .*[Ww]hat kind of SBOM' /Users/mlieberman/Projects/mikebom/README.md` matches one heading line.
- [ ] T033 [US1] Verify SC-002: the new section enumerates all 5 sbom-tier values AND names `--include-declared-deps`. Greppable assertion: each of the strings `design`, `source`, `build`, `deployed`, `analyzed`, `--include-declared-deps` appears within the new section's range.
- [ ] T034 [US1] `./scripts/pre-pr.sh` clean.
- [ ] T035 [US1] Commit: `docs(047/us1): README "What kind of SBOM does mikebom emit?" explainer`.

---

## Phase 6: Commit `chore(047)` — CHANGELOG + spec scaffolding

**Goal**: Land the speckit artifacts and a CHANGELOG entry.

- [ ] T036 Add a CHANGELOG `[Unreleased]` entry under "Added" naming (a) the new SPDX 2.3 `creationInfo.comment` document-level scope hint, (b) the new SPDX 3 `SpdxDocument.comment`, (c) the README explainer. Mention that no CDX output changes (CDX already self-describes via `metadata.lifecycles[]`).
- [ ] T037 Stage `specs/047-scope-self-description/` (spec.md, plan.md, tasks.md, checklists/requirements.md) and any `CLAUDE.md` updates from `update-agent-context.sh`.
- [ ] T038 `./scripts/pre-pr.sh` clean.
- [ ] T039 Commit: `chore(047): CHANGELOG entry + speckit spec/plan/tasks scaffolding`.

---

## Phase 7: Polish & PR

- [ ] T040 Verify SC-005 (SPDX 2.3 goldens regen with comment field present + otherwise byte-identical pre-milestone): grep one regen'd golden for the new comment, confirm via `jq` that all other top-level fields match pre-PR shapes.
- [ ] T041 Verify SC-006 (CDX goldens unchanged): `git diff main..HEAD -- mikebom-cli/tests/fixtures/golden/cyclonedx/` empty.
- [ ] T042 Verify SC-007 (pre-PR + CI green): final `./scripts/pre-pr.sh` clean from a fresh shell.
- [ ] T043 Push branch: `git push -u origin 047-scope-self-description`.
- [ ] T044 Open PR titled `feat(047): SBOM-scope self-description (README explainer + SPDX `comment` parity)`. Body covers: 4-commit summary, scope of audit findings, 8 SC verification commands, out-of-scope reminders.
- [ ] T045 Verify all 3 CI lanes (linux x86_64, linux ebpf, macos-latest) green on the PR.

---

## Dependency graph

```text
T001-T002 (setup, baseline)
   │
   ▼
T003-T008 (foundational: ScopeMode + ScanArtifacts plumbing
           + extract lifecycle_phases utility — required by both
           SPDX 2.3 and SPDX 3 commits)
   │
   ▼
T009-T021  [Commit 1: Phase 3 — US2 SPDX 2.3 comment]
   │
   ▼
T022-T030  [Commit 2: Phase 4 — US2 SPDX 3 comment, reuses Phase 3's helper]
   │
   ▼
T031-T035  [Commit 3: Phase 5 — US1 README explainer, references shipped SPDX behavior]
   │
   ▼
T036-T039  [Commit 4: Phase 6 — CHANGELOG + scaffolding]
   │
   ▼
T040-T045 (verify + push + PR)
```

**Why phase ordering inverts spec priority** (US2 before US1):
US1's README references `creationInfo.comment` and
`SpdxDocument.comment` as concrete shipped behavior per FR-002(c).
Shipping SPDX first lets the README cite real output rather than
forward-reference unshipped fields. Story labels remain `[US1]` /
`[US2]` reflecting spec-time priority; commit ordering reflects
implementation dependency.

## Parallel opportunities

Within each commit, edits land in different files where possible:

| Bucket | Parallel-eligible tasks |
|---|---|
| Foundational | T003 + T004 (mod.rs sequential) → T005 (scan_cmd.rs) || T006 (new file) — T007 depends on T006 |
| Commit 1 | T009 + T010 + T011 (same file → sequential) but T012 / T013 / T014 (test functions in same file) can be added in any order; mark as [P] internally |
| Commit 2 | T022 (single edit) — T023 follows |
| Commit 3 | T031 (single file) |

## Estimated effort

| Phase | Effort | Notes |
|---|---|---|
| Phase 1 (setup) | 5 min | Just baseline check |
| Phase 2 (foundational) | 30 min | New module + plumbing |
| Phase 3 (SPDX 2.3) | 1 hr | Helper + struct field + 9 goldens regen |
| Phase 4 (SPDX 3) | 30 min | Reuses helper; 9 goldens regen |
| Phase 5 (README) | 30 min | New section, 50 lines of Markdown |
| Phase 6 (CHANGELOG + scaffold) | 10 min | Mechanical |
| Phase 7 (verify + PR) | 15 min | Push + CI |
| **Total** | **~3 hr** | One focused session |

## MVP scope

US1 alone (commit 3) without US2 would still be valuable —
documents the existing CDX self-description and the existing
artifact/manifest axis. But the README would have to forward-
reference the SPDX comment ("SPDX equivalent forthcoming") which
is awkward.

US2 alone (commits 1+2) ships the SPDX-side parity but leaves the
documentation gap open — operators still have no project-side
explanation of mikebom's scope choices.

The intended MVP for this milestone is **US2 + US1 together** —
US2 closes the SPDX-side parity gap; US1 documents what the
combined CDX + SPDX self-description means. Shipping both in one
PR is the cleanest UX outcome.

If review bandwidth is tight, the US2 commits (1+2) could land
first as a standalone PR ("SPDX comment parity") with US1
following separately ("docs explainer"). Spec has no hard
coupling that prevents this.
