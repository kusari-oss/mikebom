---
description: "Task list for milestone 082 — Documentation refresh and audit"
---

# Tasks: Documentation refresh and audit

**Input**: Design documents from `/specs/082-docs-refresh/`
**Prerequisites**: plan.md, spec.md (with /speckit.clarify Q1 + Q2 integrated), research.md (with the 22-file audit table + style decisions + architecture spot-check claims), data-model.md, contracts/docs-style.md, quickstart.md

**Tests**: Spec references SC-001 through SC-011. Verification is via `scripts/verify-docs-currency.sh` (the SC-001 verifier) + manual cross-reference click-through (SC-003) + reviewer cover-to-cover review (SC-006). No test code is added; production tests must continue passing per FR-009 + SC-009.

**Organization**: Five user stories. US1 (P1) CLI reference completeness; US2 (P1) cross-reference graph; US3 (P1) quickstart + configuration + installation refresh; US4 (P1) README accuracy; US5 (P2) architecture-doc targeted spot-check. All ship in one PR.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: parallelizable (different files, no incomplete-task dependencies)
- **[Story]**: US1 / US2 / US3 / US4 / US5 (user-story phase tasks only)
- File paths are absolute or repository-relative

## Path Conventions

Pure documentation refresh. 22 in-scope markdown files across `docs/architecture/`, `docs/reference/`, `docs/user-guide/`, plus `docs/{index,design-notes,ecosystems}.md` and `README.md`. ONE new shell script at `scripts/verify-docs-currency.sh`. NO production code, NO test code, NO byte-identity goldens touched.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Finalize style-convention decisions; create the SC-001 verifier script.

- [ ] T001 Finalize the three style conventions from research §2 + contracts/docs-style.md. The three decisions are already pinned (milestone-reference handling: omitted in user-guide + README, parenthetical `(milestone N)` elsewhere; code-block fence: language tag REQUIRED on every fenced block; inter-doc link: relative paths, NO leading `./`). Commit them as durable contract by reviewing contracts/docs-style.md once more for any ambiguities; spot-check the contract's example links resolve correctly. No file edits; this is a sanity-check task.

- [ ] T002 Create `scripts/verify-docs-currency.sh` per research §7. ~50 LOC bash script that diffs `mikebom <subcommand> --help` flag set against `docs/user-guide/cli-reference.md`. Exit 0 when in sync; exit 1 with missing-flag list otherwise. Coverage MUST include all 8 subcommands documented in T004 (per analyze F1 fix): `mikebom sbom scan`, `mikebom sbom verify`, `mikebom sbom enrich`, `mikebom trace run`, `mikebom trace capture`, `mikebom policy init`, `mikebom verify-binding`, `mikebom trace-binding`. The script's `for sub in ...` loop iterates over all 8. Make executable (`chmod +x`). Spot-test that the script runs (likely produces a long missing-flag list pre-fix; that's expected — the script's value is gating against future drift, not pre-fix audit).

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Verify the per-file audit baseline + no in-scope files have been added since /speckit.plan setup.

- [ ] T003 Verify `find docs -name "*.md" \! -path "*binding-fixtures*" \! -path "*examples*" \! -path "*research/*"` outputs match research §1's 22-file list. If new files have appeared since /speckit.plan setup (e.g., from a parallel branch merge), classify them per research §1's rubric and append rows to the audit table. Update the audit-record reference in milestone PR description.

---

## Phase 3: User Story 1 — CLI reference completeness (Priority: P1)

**Goal**: `docs/user-guide/cli-reference.md` documents every operator-facing CLI flag at alpha.22 — including the 11 flags from milestones 073–081 currently missing.

**Independent Test**: `bash scripts/verify-docs-currency.sh` exits 0; manual review confirms every flag listed by `mikebom sbom scan --help` and `mikebom trace run --help` has an entry in `cli-reference.md` with description + example + cross-reference.

### Implementation for User Story 1

- [ ] T004 [US1] Comprehensive rebuild of `docs/user-guide/cli-reference.md` per research §3 + contracts/docs-style.md. Apply the per-subcommand layout (Quick reference table at top of each section + per-flag detail blocks below). Subcommand coverage (alpha.22 set): `mikebom sbom scan`, `mikebom sbom verify`, `mikebom sbom enrich`, `mikebom trace run`, `mikebom trace capture` (experimental banner), `mikebom policy init`, `mikebom verify-binding`, `mikebom trace-binding`. Per-flag detail block per the contract: name + value placeholder, type, default, repeatable-or-not, valid value vocab, one-paragraph description, ≥1 example invocation, cross-reference link to deep-dive doc. **The 11 missing flags from milestones 073–081 land here**: `--root-name`, `--root-version`, `--scan-target-name`, `--sbom-type`, `--component-id`, `--creator`, `--annotator`, `--annotation-comment`, `--metadata-comment`, `--metadata-file`, `--strip-id-credentials`, `--component-id-allowed-schemes`. Per FR-011: `--include-dev` gets the deprecation block format (since milestone 052/part-3, replacement: `--exclude-scope`). Apply Q1 style pass (omit milestone references in user-guide; tag every code block; relative inter-doc links no `./`). Cross-references go to: `--sbom-type` → `../reference/sbom-types.md`; `--component-id` / `--creator` / `--annotator` etc. → `../reference/identifiers.md`; `--strip-id-credentials` → `../reference/identifiers.md`. Estimated diff: +200–300 lines net.

- [ ] T005 [US1] Run `bash scripts/verify-docs-currency.sh`; assert exit 0; confirm every subcommand's flag set is in sync. If exit 1, add the missing flags + re-run. Lock SC-001.

---

## Phase 4: User Story 2 — Reference-doc cross-reference graph (Priority: P1)

**Goal**: Every `docs/reference/*.md` page has a "See also" section per FR-003 + contracts/docs-style.md. The cross-reference graph is connected: starting from any reference doc, ≤3 clicks reach any other reference doc per SC-003.

**Independent Test**: Click through every "See also" link from any starting reference doc; confirm reachability of all 5 reference docs within ≤3 clicks (manual smoke).

### Implementation for User Story 2

- [ ] T006 [P] [US2] Add "See also" section to `docs/reference/identifiers.md` per the contract: 4 links to {cross-tier-binding, sbom-types, sbom-format-mapping, conformance-harness-guide}. Apply Q1 aggressive style pass: tag every fenced block, normalize inter-doc links (no leading `./`), normalize milestone references to parenthetical `(milestone N)` form (this is a reference doc, not user-guide). Estimated diff: +10 lines for "See also" + ~30–50 lines style normalization spread across the file's 1146 lines.

- [ ] T007 [P] [US2] Add "See also" section to `docs/reference/cross-tier-binding.md` per the contract: 3 links to {identifiers, sbom-types, conformance-harness-guide}. Apply Q1 style pass. Estimated diff: +10 lines for "See also" + ~20–30 lines style normalization across the file's 1032 lines.

- [ ] T008 [P] [US2] Add "See also" section to `docs/reference/sbom-format-mapping.md` per the contract: 3 links to {identifiers, sbom-types, conformance-harness-guide}. **Critical preservation**: Section I (the Principle V audit-record entries from milestones 080 + 081) MUST stay byte-identical — durable historical record per the spec's edge case. Apply Q1 style pass to other sections only. Estimated diff: +10 lines for "See also" + minimal style normalization (file is short — 140 lines).

- [ ] T009 [P] [US2] Add "See also" section to `docs/reference/conformance-harness-guide.md` per the contract: 3 links to {sbom-format-mapping, identifiers, sbom-types}. Apply Q1 style pass. Cross-reference the milestone-078 SPDX 3 SHACL conformance work where appropriate (per research §1 audit's note). Estimated diff: +15 lines.

- [ ] T010 [US2] `docs/reference/sbom-types.md` already has a "See also" section from milestone 081 — verify links match the contract's per-doc graph plan ({identifiers, sbom-format-mapping, cross-tier-binding}). Apply Q1 style pass for any inconsistencies. If the existing "See also" already covers the 3 expected neighbors, this task is a no-op for content + small for style.

**Checkpoint**: US2 passes. Run a manual click-through smoke: starting from `identifiers.md` → click any "See also" link → click again → confirm reachability of all 5 reference docs within ≤3 hops. Document the longest-path observed in the milestone PR description.

---

## Phase 5: User Story 3 — Quickstart + Configuration + Installation refresh (Priority: P1)

**Goal**: User-guide docs reflect current operator surface at alpha.22. Quickstart recipes run clean against the published binary; configuration covers every operator-visible env var; installation reflects modern install workflow.

**Independent Test**: A new operator follows the docs top-to-bottom against the alpha.22 binary; every documented command exits zero and produces output matching documented snippets (per SC-004).

### Implementation for User Story 3

- [ ] T011 [US3] Major refresh of `docs/user-guide/quickstart.md` per FR-004. Add or update recipes covering: source-tier scan (basic `mikebom sbom scan --path .`); image scan (`--image registry.example.com/img:tag`); build-tier trace (`mikebom trace run --command -- ...`); `--sbom-type` operator override (e.g., `--sbom-type build`); `--metadata-file` sidecar JSON input; `--component-id` user-defined identifier injection; `--root-name` overriding root component name. Each recipe MUST run clean against the alpha.22 binary; documented output snippets MUST match alpha.22's actual output shape. Apply Q1 style pass: omit milestone references entirely; tag every code block; relative inter-doc links no `./`. Cross-references at the end of each recipe pointing to the relevant deep-dive `docs/reference/<topic>.md`. Recipe count stays ≤10 per FR-004. Estimated diff: net +60–100 lines (some old recipes removed, new ones added).

- [ ] T012 [US3] Refresh `docs/user-guide/configuration.md` per FR-005. Document every operator-visible environment variable mikebom reads at runtime: `MIKEBOM_REQUIRE_SPDX3_VALIDATOR` (CI strict-mode for the milestone-078 conformance gate); `MIKEBOM_UPDATE_CDX_GOLDENS` / `MIKEBOM_UPDATE_SPDX_GOLDENS` / `MIKEBOM_UPDATE_SPDX3_GOLDENS` (test-side regen mechanism); `MIKEBOM_PREPR_EBPF` (local pre-PR opt-in for eBPF feature gate); `MIKEBOM_NO_DEPRECATION_NOTICE` (suppresses deprecation warnings, e.g., in CI). Plus any others surfaced by `grep -roh 'MIKEBOM_[A-Z_]*' mikebom-cli/src/ | sort -u` per SC-005's verification approach. Each entry: name, purpose, accepted values, default, example use case. Apply Q1 style pass. Estimated diff: +60–100 lines (the file is currently 69 lines).

- [ ] T013 [US3] Refresh `docs/user-guide/installation.md` per US3 + research §1 (materially-stale classification). Replace pre-alpha.6 install instructions with version-agnostic current workflow (per analyze F3 fix — do NOT hard-code a specific alpha version, since the doc would go immediately stale on every release): (a) **Pre-built binaries** — link to the GitHub Releases page (`https://github.com/kusari-sandbox/mikebom/releases`) and document the discovery pattern: `gh release list -R kusari-sandbox/mikebom --limit 1` to find the latest tag, then `gh release download <tag> -p "mikebom-<tag>-<arch>-<os>.tar.gz"`. Operators clicking through find the current version themselves; doc stays current automatically. (b) **Source builds** — `cargo install --path mikebom-cli` (link to README's build instructions). (c) **Lima VM workflow** for macOS users wanting eBPF locally (per memory + per existing project setup). Apply Q1 style pass. Estimated diff: significant — ~50–100 lines net change as the file's 72-line current size doesn't reflect current state.

---

## Phase 6: User Story 4 — README accuracy (Priority: P1)

**Goal**: Project README accurately describes mikebom at alpha.22. No stale claims; recent-milestone surface visible; "what mikebom emits" + "getting started" sections current.

**Independent Test**: Reviewer cover-to-cover review of README against alpha.22 binary; verifies every behavioral claim matches actual output per SC-006.

### Implementation for User Story 4

- [ ] T014 [US4] Cover-to-cover review + refresh of `README.md` per FR-006 + SC-006. Sections to verify: (a) "what mikebom does" — confirm scope description matches alpha.22 (likely current per the audit); (b) "what mikebom emits" — confirm CDX 1.6 + SPDX 2.3 + SPDX 3.0.1 are listed + each format's distinguishing features (lifecycles, software_sbomType, etc.) are mentioned; (c) "stability" — confirm `sbom scan` / `sbom verify` / `policy init` / `sbom enrich` are still labeled stable and `trace capture` / `trace run` are still labeled experimental; (d) "getting started" — confirm the install + first-scan recipe works against alpha.22 (matches `quickstart.md` Recipe 1 for consistency per the avoid-triplicate-updates rule); (e) cross-references to recent-milestone surface — every milestone 072–081 feature has at least ONE README mention with a link to the deep-dive `docs/reference/<topic>.md` per FR-006. Apply Q1 style pass: omit milestone references in README (operators don't care that `--sbom-type` came from milestone 081; they care what it does); tag every code block; relative inter-doc links. Recipe count stays ≤10 per FR-006. Estimated diff: net +30–80 lines depending on how much of the existing 625 lines is re-organized.

---

## Phase 7: User Story 5 — Architecture docs targeted spot-check (Priority: P2)

**Goal**: 9 architecture docs receive the targeted Q2 spot-check (5–10 testable claims per doc verified against alpha.22 source code). Stale claims fixed inline if ≤30 lines per claim; larger rewrites filed as follow-up issues.

**Independent Test**: Reviewer reads the milestone PR's research.md §6 spot-check appendix; cross-checks per-claim verification status against current source code; ≥90% per-claim accuracy.

### Implementation for User Story 5

**T015 split note (per analyze F2 fix)**: T015 is split into 9 per-doc tasks (T015a–T015i) so partial completion is recoverable if a worker times out partway. Each task targets ONE architecture doc with the same per-doc spot-check + Q1 style pass. The 9 tasks are logically parallel (different files) but realistic single-developer pacing keeps them sequential within the milestone — per-doc completion lets a timeout-recovery worker resume at the next undone doc rather than restart the batch.

- [ ] T015a [P] [US5] `docs/architecture/overview.md`: Q2 spot-check (5 claims per research §6 — five-stage-pipeline, per-stage output type, no-fallback Principle III, native-Rust Principle I, eBPF-discovery Principle II). Verify each against alpha.22 source via grep + read (~1–5min per claim). Record status in research.md §6 as `verified` / `fixed inline` / `filed as follow-up #N`. Inline fixes ≤30 lines per claim; file issue for larger rewrites. Apply Q1 style pass: tag code blocks, normalize milestone references to `(milestone N)` parenthetical, relative inter-doc links no `./`.

- [ ] T015b [P] [US5] `docs/architecture/scanning.md`: Q2 spot-check (5 claims per research §6 — recursive walk, per-ecosystem package DB modules, dep-scope per milestone 052, sbom-tier auto-detect per milestone 047, eBPF observes builds-not-runtime). Same verification + status recording + inline-fix-or-file rules as T015a. Apply Q1 style pass.

- [ ] T015c [P] [US5] `docs/architecture/generation.md`: Q2 spot-check (5 claims per research §6 — CDX builder path, SPDX 2.3 builder path, SPDX 3 builder path, shared lifecycle_phases.rs aggregation, per-format schema validation). Same rules as T015a. Apply Q1 style pass.

- [ ] T015d [P] [US5] `docs/architecture/attestations.md`: Q2 spot-check (3 claims per research §6 — in-toto witness-v0.1 emission, milestone-072 cross-tier binding metadata, milestone-076 build-tier subjects). Same rules as T015a. Apply Q1 style pass.

- [ ] T015e [P] [US5] `docs/architecture/enrichment.md`: Q2 spot-check (3 claims per research §6 — Strict Boundary 1 enrichment-not-discovery, ClearlyDefined opt-out via --offline, deps.dev enrichment-only). Same rules as T015a. Apply Q1 style pass.

- [ ] T015f [P] [US5] `docs/architecture/licenses.md`: Q2 spot-check (3 claims per research §6 — SPDX-listed canonical IRI, `spdx` crate canonicalization, milestone-078 simplelicensing_LicenseExpression). Same rules as T015a. Apply Q1 style pass.

- [ ] T015g [P] [US5] `docs/architecture/purls-and-cpes.md`: Q2 spot-check (3 claims per research §6 — per-component PURL emission, CPE emission via content-shape detection, four-layer identity model from milestones 072–077). Same rules as T015a. Apply Q1 style pass.

- [ ] T015h [P] [US5] `docs/architecture/resolution.md`: Q2 spot-check (3 claims per research §6 — lockfile-based dep-tree edges per Principle XII, current `--exclude-scope` behavior post-052, polyglot per-ecosystem aggregation). Same rules as T015a. Apply Q1 style pass.

- [ ] T015i [P] [US5] `docs/architecture/signing.md`: Q2 spot-check (2 claims per research §6 — DSSE-signed envelopes when signing enabled, operator-supplied keys not mikebom-managed). Same rules as T015a. Apply Q1 style pass.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T016 [P] Q1 aggressive style pass on the remaining in-scope files: `docs/index.md`, `docs/design-notes.md`, `docs/ecosystems.md`. Each gets: code-block tagging on any untagged fenced blocks; milestone references normalized to `(milestone N)` parenthetical; inter-doc links normalized to no-leading-`./` relative-path form. `docs/index.md` ALSO gets navigation update per FR-010 — add entries for any new reference docs added since the file was last touched (per the audit, the file is materially stale and predates the milestone-072+ reference docs additions). Estimated diff: ~80–120 lines net change concentrated in index.md.

- [ ] T017 Run the standard pre-PR gate per CLAUDE.md: (a) `cargo +stable clippy --workspace --all-targets -- -D warnings` (zero warnings); (b) `cargo +stable test --workspace` (every target reports `0 failed`). The milestone is docs-only, so production code + test code MUST be unchanged from main; the gate's job here is to prove no accidental code touches snuck in. Capture the per-target "ok. N passed; 0 failed" lines for the PR description. Critically: `cdx_regression`, `spdx_regression`, `spdx3_regression` MUST pass without their `MIKEBOM_UPDATE_*_GOLDENS` env vars (no goldens regenerated by docs work).

- [ ] T018 Manual verification + smoke. (a) Run `bash scripts/verify-docs-currency.sh`; assert exit 0 (re-confirms T005). (b) Click-through SC-003 verification: starting from `docs/reference/identifiers.md`, click "See also" links to confirm all 5 reference docs reachable in ≤3 clicks; record the longest-path observed. (c) Quickstart recipe smoke: pick at least 3 of the documented quickstart recipes; run them against the alpha.22 binary; confirm output matches documented snippets (SC-004). (d) README cover-to-cover read: verify behavioral claims against alpha.22 output (SC-006). (e) Architecture doc spot-check appendix verification: pick 1 random claim per architecture doc; confirm the recorded status (verified / fixed inline / filed as follow-up) is accurate.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: T001 (style decisions) + T002 (verifier script) — both have no in-milestone dependencies; can run in parallel.
- **Phase 2 (Foundational)**: T003 (audit baseline verify) — depends on T001 (style decisions) for the audit's classification rubric.
- **Phase 3 (US1)**: T004 (cli-reference rebuild) depends on T001 (style decisions) + T002 (verifier script for SC-001 gating). T005 (verifier run) depends on T004.
- **Phase 4 (US2)**: T006-T010 (reference doc "See also" sections) — depend on T001 (style decisions); independent of each other (different files); 4 of 5 marked [P].
- **Phase 5 (US3)**: T011 + T012 + T013 — independent of each other (different files); each depends on T001 (style decisions). All are major-refresh-scope.
- **Phase 6 (US4)**: T014 (README) — depends on T011 (quickstart) for consistency on the install + first-scan recipe per the avoid-triplicate-updates rule.
- **Phase 7 (US5)**: T015a–T015i (9 per-doc spot-check + style tasks per analyze F2 fix) — each depends on T001 (style decisions); independent of all other US-phase tasks AND independent of each other (different files; all marked [P]). Recoverable by-doc if a worker times out partway.
- **Phase 8 (Polish)**: T016 (remaining files style pass) — independent of all other tasks; can run in parallel. T017 (pre-PR gate) — runs after Phases 1-7 complete. T018 (manual verification) — runs after T017.

### Parallel Opportunities

- **T001 + T002** [parallel] — sanity check + new shell script; no overlap.
- **T006 + T007 + T008 + T009** [parallel] — four different reference doc files.
- **T011 + T012 + T013** — three different user-guide files; logically parallel but each is a substantial rewrite, so realistic parallelism is limited by reviewer bandwidth.
- **T015** — internally batched across 9 architecture docs; could split per-doc if multiple developers, but realistic single-developer pacing keeps it as one task.
- **T015a–T015i** [parallel] — 9 different architecture-doc files; all independently parallelizable.
- **T016** [P] — different files (index, design-notes, ecosystems); parallel with T015a–T015i in particular.

### Within Each User Story

- **US1**: T004 (rebuild) → T005 (verify) — sequential.
- **US2**: T006-T010 — 4 marked [P]; T010 is small and depends on the existing milestone-081 "See also" being already-correct.
- **US3**: T011 + T012 + T013 — independent files, logically parallel.
- **US4**: T014 — single task; depends on T011 for cross-doc consistency.
- **US5**: T015a–T015i — 9 per-doc tasks (one per architecture doc) for timeout-recoverability per analyze F2 fix.

---

## Parallel Example: Phase 4 (US2 cross-reference graph)

```bash
# Sequential: style decisions
Task: "T001 Finalize style conventions"

# Parallel: 4 different reference doc files
Task: "T006 [P] [US2] identifiers.md See also + style"
Task: "T007 [P] [US2] cross-tier-binding.md See also + style"
Task: "T008 [P] [US2] sbom-format-mapping.md See also + style (preserve Section I)"
Task: "T009 [P] [US2] conformance-harness-guide.md See also + style"

# Sequential: builds on the existing milestone-081 See also
Task: "T010 [US2] sbom-types.md style verify"
```

---

## Implementation Strategy

### MVP First (Phases 1-3 = US1)

1. Phase 1 setup (T001 + T002).
2. Phase 2 foundational (T003 audit baseline verify).
3. Phase 3 US1 (T004 cli-reference rebuild + T005 verifier).
4. **STOP and VALIDATE**: at this checkpoint, the dominant operator-facing pain point (CLI reference missing 11 flags) is closed. `verify-docs-currency.sh` passes; operators can find every flag in one canonical place. The other 4 user stories can ship later if scope is tight.
5. Continue to Phases 4-8.

### Incremental Delivery

Single PR. Splitting US1-US4 by user-story would create transient states where some files cross-reference others that haven't yet been refreshed. The cross-reference graph (US2) explicitly depends on the reference docs being current AND the user-guide docs they link to existing in their final form. Single-PR delivery preserves consistency.

### Parallel Team Strategy

Single developer + reviewer fits the milestone. Two-way parallelism: developer-A handles US1 + US3 + US4 (the user-facing major refreshes); developer-B handles US2 + US5 (the smaller spot-check + cross-reference work). T016 polish task is independent.

---

## Notes

- [P] = different files, no incomplete-task dependencies.
- All five user stories ship in this PR; no follow-up issues filed UNLESS T015's architecture spot-check surfaces a claim requiring >30-line rewrite (in which case the issue is filed inline during T015 execution).
- Per CLAUDE.md: pre-PR gate REQUIRES both `cargo +stable clippy --workspace --all-targets -- -D warnings` clean AND `cargo +stable test --workspace` clean. Cite both in the PR description.
- This is a docs-only milestone. Production Rust code, test code, and byte-identity goldens MUST stay byte-identical to main pre-merge. T017 verifies via the existing regression test targets passing without their `MIKEBOM_UPDATE_*_GOLDENS` env vars.
- The new `scripts/verify-docs-currency.sh` is a one-off helper; not added to CI in this milestone (out of scope per spec). Future milestones may integrate it into the pre-PR gate or CI workflow.
- Style conventions per `contracts/docs-style.md` are the durable artifact future milestones reference. Future milestones touching docs reference this contract rather than re-litigating choices.
- Per FR-009 + SC-009: NO production code, NO test code, NO byte-identity goldens touched. The single new file is `scripts/verify-docs-currency.sh` (a shell script, not Rust).
- Total estimated tasks: 26 (was 18 pre-analyze; T015 split into T015a–T015i for timeout-recoverability per analyze F2 fix, adding 8 net tasks). Total estimated effort: 1.5–2.5 person-days for a single developer; the 9 architecture-doc tasks are individually small (~15–20 min each) and independently parallelizable.
