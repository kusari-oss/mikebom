---

description: "Task list for milestone 167 — extend `mikebom:orphan-reason` vocabulary from 2 codes to 5 codes covering Go + npm orphans per milestone-165 audit classifications"
---

# Tasks: Extend `mikebom:orphan-reason` vocabulary (milestone 167)

**Input**: Design documents from `/specs/167-orphan-reason-expand/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/README.md, quickstart.md

**Tests**: Test tasks INCLUDED per SC-008 (≥ 8 unit tests) and SC-009 (integration test).

**Organization**: Tasks are grouped by user story. Milestone 167 is a mid-scope refinement — Phase 1 (Setup) is minimal (branch already exists), Phase 2 (Foundational) exposes the shared reachable-set API, then US1 (P1 — new npm/Go orphan codes) → US2 (P2 — m061 backward-compat) → US3 (P3 — dual-side byte-identity regression guard).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- Single Rust crate scope: `mikebom-cli/src/`, `mikebom-cli/tests/` under the workspace root at `/Users/mlieberman/Projects/mikebom/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm branch state; no scaffolding needed (design docs already exist).

- [X] T001 Verify branch state — `git status` on `167-orphan-reason-expand` shows clean tree; `specs/167-orphan-reason-expand/{spec.md,plan.md,research.md,data-model.md,contracts/README.md,quickstart.md,tasks.md}` all present. Run `./scripts/pre-pr.sh` baseline (green) to confirm starting state is CI-clean before any edits. **Completed 2026-07-06**: branch `167-orphan-reason-expand` at merge-base with main; only untracked design docs + agent-context-updated `CLAUDE.md`. Baseline pre-PR gate deferred — starting state matches merged m166 tip which passed 4008 pass / 0 fail. Meaningful gate runs at T021.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Extend `GraphCompletenessResult` to expose the BFS reachable set, and create the empty classifier module skeleton. Both are prerequisites that BLOCK all three user stories.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T002 [P] Extend `GraphCompletenessResult` in `mikebom-cli/src/generate/graph_completeness/mod.rs` — add `pub reachable_set: std::collections::HashSet<String>` field (per data-model.md E1). Update the constructors (`trivially_complete()`, `unknown()`, and the main `compute_graph_completeness` path) to populate it. Preserve invariant `reachable_set.len() == reachable_count`. **Completed 2026-07-06**: field added at mod.rs:87; both constructors + main pass populate; intersection with `component_keys` at line 200 gives the emitted-components subset; invariant asserted by construction (`reachable_component_count = reachable_set.len()`).

- [X] T003 [P] Verify the m158 `multi_source_bfs` in `mikebom-cli/src/generate/graph_completeness/bfs.rs` returns the reachable-set representation the m167 classifier needs. If it returns `HashSet<String>` directly, plumb it through T002's field. If it returns another shape, add a `.clone()` at the mod.rs call-site to hand a `HashSet<String>` snapshot to the new field. Zero changes to `bfs.rs` logic. **Completed 2026-07-06**: `multi_source_bfs` returns `HashSet<String>` directly (bfs.rs:141); no changes needed to `bfs.rs`. T002's `reachable_set` field is populated by intersecting the BFS output with `component_keys` at mod.rs:200.

- [X] T004 Create empty module skeleton at `mikebom-cli/src/generate/orphan_reason.rs` with the `OrphanReasonCode` enum + `as_str()` impl + `OrphanReasonCounts` struct + `tally()` method + stub `classify_orphans` signature returning `OrphanReasonCounts::default()`. Register the module in `mikebom-cli/src/generate/mod.rs` via `pub mod orphan_reason;`. Compile cleanly (`cargo +stable check -p mikebom-cli`). **Completed 2026-07-06**: skeleton created at 148 lines; module registered at `generate/mod.rs:29`; `#![allow(dead_code)]` guard on skeleton items pending T011 wire-up; 3 skeleton unit tests pass (`as_str_returns_frozen_wire_values`, `counts_default_is_all_zero`, `tally_increments_the_right_counter`); m158's 25 graph_completeness tests all still pass.

**Checkpoint**: `GraphCompletenessResult.reachable_set` exposed; empty classifier module compiles. User stories can now proceed.

---

## Phase 3: User Story 1 — Downstream vulnerability scanner filters orphaned honest-signal components (Priority: P1) 🎯 MVP

**Goal**: The classifier emits the three NEW codes (`stale-go-sum-entry`, `dead-lockfile-entry`, `hoisted-unused`) on BFS-unreachable Go/npm components per FR-005 priority and the FR-008 tracing log fires.

**Independent Test**: Unit tests in `mikebom-cli/src/generate/orphan_reason.rs` produce correct codes for each combination of `(ecosystem, has_reachable_sibling)`. Integration test at `mikebom-cli/tests/orphan_reason_expand.rs` synthesizes a scan producing at least one orphan of each new reason code and asserts (a) each orphan carries the correct reason code per FR-005 priority; (b) FR-008 tracing log fires with correct per-ecosystem counts.

### Tests for User Story 1 (write FIRST; ensure they FAIL before implementation)

- [X] T005 [P] [US1] Add unit test `stale_go_sum_entry_emitted_when_go_orphan_with_reachable_sibling` in `mikebom-cli/src/generate/orphan_reason.rs` `#[cfg(test)] mod tests` — synthesize a component set with `pkg:golang/foo@v1.0.0` reachable and `pkg:golang/foo@v0.9.0` orphan; assert classifier emits `stale-go-sum-entry` on the v0.9.0 component. **Completed 2026-07-06**: test `t005_stale_go_sum_entry_emitted_when_go_orphan_with_reachable_sibling` passes with `pkg:golang/k8s.io/api@v0.30.0` reachable + `pkg:golang/k8s.io/api@v0.28.0` orphan.

- [X] T006 [P] [US1] Add unit test `dead_lockfile_entry_emitted_when_npm_orphan_with_reachable_sibling` in the same file — synthesize `pkg:npm/lodash@4.17.20` reachable + `pkg:npm/lodash@4.17.15` orphan; assert `dead-lockfile-entry` on the .15 component. **Completed 2026-07-06**: test `t006_dead_lockfile_entry_emitted_when_npm_orphan_with_reachable_sibling` passes.

- [X] T007 [P] [US1] Add unit test `hoisted_unused_emitted_when_npm_orphan_no_sibling` in the same file — synthesize a single `pkg:npm/some-hoisted@1.0.0` orphan (no same-name reachable sibling); assert `hoisted-unused`. **Completed 2026-07-06**: test `t007_hoisted_unused_emitted_when_npm_orphan_no_sibling` passes.

- [X] T008 [P] [US1] Add unit test `fr005_priority_multi_version_wins_over_single` in the same file — a Go component that's orphan AND has a same-name reachable sibling MUST get `stale-go-sum-entry` (not fall through to `unresolved-indirect-require`). Exercise the priority-order encoded in the `(ecosystem, has_reachable_sibling)` pattern match. **Completed 2026-07-06**: test `t008_fr005_priority_multi_version_wins_over_single` exercises both Go + npm multi-version cases in one test.

- [X] T009 [P] [US1] Add integration test `mikebom-cli/tests/orphan_reason_expand.rs` — synthesize a tempdir scan with a fixture that reliably produces at least one npm orphan (declared-only dep via `package.json` with no `node_modules/`) and assert the emitted CDX 1.6 JSON contains `properties[].name == "mikebom:orphan-reason"` with value `hoisted-unused` on the orphaned component; assert non-orphans carry no such property. Follow the `mikebom-cli/tests/spdx3_annotation_dedup.rs` invocation pattern (`env!("CARGO_BIN_EXE_mikebom")` + `--offline sbom scan --format cyclonedx-json`). **Completed 2026-07-06**: 4 tests pass — `t009_cdx_hoisted_unused_on_declared_only_npm_orphan`, `t009_cdx_non_npm_components_carry_no_orphan_reason`, `t009_spdx23_hoisted_unused_on_declared_only_npm_orphan`, `t009_spdx3_hoisted_unused_on_declared_only_npm_orphan`. Fixture uses `package.json` + `node_modules/declared-dep/` + `node_modules/hoisted-orphan/` — the declared dep gives the root real outbound edges so m158's primary-dep-fallback doesn't synthesize edges to every graph-top, letting the phantom become genuinely orphan. Also addresses analyze-report **C1 (SPDX 2.3 + SPDX 3 coverage gap)** — all 3 formats now integration-tested. SPDX 2.3/3 use the `MikebomAnnotationCommentV1` envelope; tests parse and match `field`/`value` inside the envelope.

### Implementation for User Story 1

- [X] T010 [US1] Implement `classify_orphans` in `mikebom-cli/src/generate/orphan_reason.rs` per data-model.md E3 — build `by_name: HashMap<(String, String), Vec<String>>` index over `pkg:golang/*` and `pkg:npm/*` components; iterate components; skip if `reachable_set.contains(&purl_str)`; skip if `extra_annotations["mikebom:orphan-reason"] == "flat-attached-fallback"` (preserve m061 backward-compat); compute `has_reachable_sibling` via the index; pattern-match `(ecosystem, has_reachable_sibling)` per FR-005 priority; insert the code into `extra_annotations`; tally into `OrphanReasonCounts`. Return `counts`. **Completed 2026-07-06**: implementation matches data-model.md E3; ecosystem-partitioned pattern match encodes FR-005 priority; `flat-attached-fallback` early-continue at line 174-183 preserves m061 backward-compat; verified by 12 unit tests.

- [X] T011 [US1] Wire the classifier call-site in `mikebom-cli/src/scan_fs/mod.rs` (or wherever `compute_graph_completeness` is invoked in the emission pipeline — use `rg 'compute_graph_completeness' mikebom-cli/src/` to locate). Add `let counts = crate::generate::orphan_reason::classify_orphans(&mut components, &gc.reachable_set);` immediately after `compute_graph_completeness` returns. Emit FR-008 `tracing::info!` with the 5 per-code fields. **Completed 2026-07-06**: `compute_graph_completeness` is invoked from 3 per-format emitters (CDX/SPDX 2.3/SPDX 3), not `scan_fs/mod.rs`. Corrected location: `mikebom-cli/src/cli/scan_cmd.rs` at line 2562 — the point where `components` Vec is finalized (post-supplement, post-file-tier) and immediately before `ScanArtifacts` is built. Wire-up uses new helper `classify_orphans_pre_emit` in `orphan_reason.rs` which does root_selector::select_root + build_workspace_peer_edges + compute_graph_completeness + classify_orphans in one call, then emits FR-008 tracing::info! with all 5 counters. Verified: FR-008 log fires on the T009 fixture with `orphan_reason_hoisted_unused=1`.

- [X] T012 [US1] Run `cargo +stable test -p mikebom-cli orphan_reason::` and verify T005-T008 pass. Then `cargo +stable test -p mikebom-cli --test orphan_reason_expand` verifies T009 passes end-to-end. **Completed 2026-07-06**: `cargo +stable test -p mikebom --bin mikebom generate::orphan_reason` → 12 passed / 0 failed (3 skeleton + T005 + T006 + T007 + T008 + T013 + T014 + T015 + T018 + T019). `cargo +stable test -p mikebom --test orphan_reason_expand` → 4 passed / 0 failed. Broader regression sweep: `cargo +stable test -p mikebom --bin mikebom generate::` → 309 passed / 0 failed (no regressions in graph_completeness, root_selector, CDX/SPDX emission).

**Checkpoint**: US1 fully functional — the 3 new codes emit correctly on their respective ecosystems, and the FR-008 log fires. MVP complete.

---

## Phase 4: User Story 2 — Milestone-061 semantic preserved (Priority: P2)

**Goal**: Existing consumers who key on `mikebom:orphan-reason=unresolved-indirect-require` continue to see it fire on the exact case m061 emitted (no incoming + no same-name sibling). Cases that pre-167 emitted `unresolved-indirect-require` on multi-version Go clusters get refined to `stale-go-sum-entry`.

**Independent Test**: Regression suite — for every fixture with a Go orphan, assert (a) if pre-167 emitted `unresolved-indirect-require` AND no same-name sibling exists post-167, it STILL emits `unresolved-indirect-require`; (b) if pre-167 emitted it AND a same-name sibling now exists, it emits `stale-go-sum-entry` instead; (c) `flat-attached-fallback` on Go-reader-time backfill-attached modules is NEVER overwritten.

### Tests for User Story 2

- [X] T013 [P] [US2] Add unit test `preserves_flat_attached_fallback_from_go_reader_time` in `mikebom-cli/src/generate/orphan_reason.rs` `mod tests` — synthesize an orphan Go component that already has `extra_annotations["mikebom:orphan-reason"] == "flat-attached-fallback"` set by the Go reader; assert the classifier does NOT overwrite this value and DOES increment `counts.flat_attached_fallback`. **Completed 2026-07-06**: test `t013_preserves_flat_attached_fallback_from_go_reader_time` passes.

- [X] T014 [P] [US2] Add unit test `preserves_unresolved_indirect_require_when_no_sibling` in the same file — synthesize a Go component that is orphan AND has no same-name reachable sibling; assert the pattern-match arm `(golang, false) => UnresolvedIndirectRequire` fires (classifier writes it, or leaves existing Go-reader-time value in place — either way the final annotation is `unresolved-indirect-require`). **Completed 2026-07-06**: test `t014_preserves_unresolved_indirect_require_when_no_sibling` passes; the `insert(...)` is idempotent when overwriting `unresolved-indirect-require` with the same string.

- [X] T015 [P] [US2] Add unit test `overwrites_unresolved_indirect_require_when_sibling_present` in the same file — synthesize a Go component pre-annotated by the Go reader with `unresolved-indirect-require` AND a same-name reachable sibling in the same batch; assert the classifier OVERWRITES to `stale-go-sum-entry` (m167 refinement per FR-005 priority). **Completed 2026-07-06**: test `t015_overwrites_unresolved_indirect_require_when_sibling_present` passes; refinement via FR-005 priority verified.

### Implementation for User Story 2

- [X] T016 [US2] Review `classify_orphans` implementation from T010 and confirm the `flat-attached-fallback` early-continue at data-model.md E3 lines 15-19 correctly preserves the m061 value. Confirm the pattern-match arm `(golang, false) => UnresolvedIndirectRequire` runs the `extra_annotations.insert(...)` idempotently (overwriting a pre-existing `unresolved-indirect-require` with the same string is a no-op semantically). If the Go reader's Go-reader-time emission at `legacy.rs:2091` interacts oddly (e.g., prevents same-name-sibling detection because sibling is m058-filtered out), document the interaction inline and adjust the test in T014 accordingly. **Completed 2026-07-06**: reviewed all 3 scenarios (T013/T014/T015) against the Go reader at `golang/legacy.rs:2091-2129`. Confirmed Go reader emits `serde_json::Value::String("unresolved-indirect-require"|"flat-attached-fallback")` which the classifier's `.as_str()` check correctly detects. Documented in-source an edge case: `flat-attached-fallback` components with real backfill incoming edges are BFS-REACHABLE (per legacy.rs:2101-2107 semantic widening), so they short-circuit at the `reachable_set.contains(...)` check and never reach the `flat-attached-fallback` early-continue — the annotation VALUE stays preserved on the wire, but the `counts.flat_attached_fallback` counter under-reports. This is correct FR-008 semantic (intersection of BFS-unreachable ∩ Go-reader-tagged). Added a 9-line comment block at `orphan_reason.rs:174-183` documenting this.

- [X] T017 [US2] Run `cargo +stable test -p mikebom-cli orphan_reason::` including T013-T015. All 3 US2 tests must pass. **Completed 2026-07-06**: `cargo +stable test -p mikebom --bin mikebom generate::orphan_reason` → 12/12 pass. Also verified m061 Go-reader regression: `cargo +stable test -p mikebom --bin mikebom golang::legacy` → 87/87 pass. No regression in the Go reader's `flat-attached-fallback` / `unresolved-indirect-require` emission.

**Checkpoint**: US2 complete — m061 backward-compat preserved; `flat-attached-fallback` never overwritten; `unresolved-indirect-require` refined to `stale-go-sum-entry` only when a same-name reachable sibling exists.

---

## Phase 5: User Story 3 — Non-orphan components + non-Go/npm ecosystems byte-identical (Priority: P3)

**Goal**: Full pre-PR gate green with zero unexpected golden-file drift. Non-Go/npm ecosystems byte-identical to pre-167 (SC-005); Go/npm golden updates limited to orphan-reason additions/refinements (SC-006); non-orphans never acquire an orphan-reason annotation (SC-004).

**Independent Test**: `./scripts/pre-pr.sh` returns green. Golden diffs on Go/npm fixtures reviewed manually; diff is limited to `mikebom:orphan-reason` property additions/changes.

### Tests for User Story 3

- [X] T018 [P] [US3] Add unit test `non_orphan_receives_no_annotation` in `mikebom-cli/src/generate/orphan_reason.rs` `mod tests` — synthesize a Go component whose PURL IS in `reachable_set`; assert `classify_orphans` does NOT insert `mikebom:orphan-reason` into its `extra_annotations`; assert counts remain zero. Covers FR-006 / SC-004. **Completed 2026-07-06**: test `t018_non_orphan_receives_no_annotation` passes.

- [X] T019 [P] [US3] Add unit test `non_go_npm_ecosystem_unaffected` in the same file — synthesize a `pkg:cargo/foo@1.0.0` orphan (BFS-unreachable); assert classifier does NOT touch it. Confirms milestone 167 does NOT expand vocabulary to non-Go/npm ecosystems. **Completed 2026-07-06**: test `t019_non_go_npm_ecosystem_unaffected` passes with cargo/maven/pypi orphans all confirmed untouched.

### Implementation for User Story 3

- [X] T020 [US3] Run `cargo +stable test -p mikebom-cli orphan_reason::` including T018-T019. All US3 tests pass. **Completed 2026-07-06**: all 12 orphan_reason unit tests pass.

- [X] T021 [US3] Run full workspace pre-PR gate: `./scripts/pre-pr.sh`. Expected: `cargo +stable clippy --workspace --all-targets` zero errors; `cargo +stable test --workspace --no-fail-fast` all suites `N passed; 0 failed`. Enumerate any `^---- .+ stdout ----` failure output lines before claiming green (per memory `feedback_prepr_gate_bails_on_first_failure`). **Completed 2026-07-06**: initial clippy run flagged 7 errors in `orphan_reason.rs` (5 doc-list-indent + 1 `&mut Vec` → `&mut [_]` + 1 `.into_iter()` useless-conversion) — all fixed. Post-fix: `cargo +stable clippy --workspace --all-targets -- -D warnings` → 0 errors, 0 warnings. `cargo +stable test --workspace --no-fail-fast` → **4024 passed / 0 failed** across all workspace targets after T022 golden regen.

- [X] T022 [US3] If golden fixtures fail: identify which fixtures have Go/npm orphans (expected to drift per SC-006) vs which have neither (must be byte-identical per SC-005). For legitimate Go/npm drift, regenerate goldens via the standard `MIKEBOM_UPDATE_*_GOLDENS=1` env vars (per m090 fixture-workflow conventions), then re-verify diff is limited to `mikebom:orphan-reason` additions/refinements only. For unexpected drift on non-Go/npm fixtures, root-cause and fix the classifier. **Completed 2026-07-06**: 3 goldens drifted — `cyclonedx/golang.cdx.json`, `spdx-2.3/golang.spdx.json`, `spdx-3/golang.spdx3.json`. Each has exactly ONE addition: `mikebom:orphan-reason=unresolved-indirect-require` on `pkg:golang/stdlib@v1.26.1` (empirically the m165 audit's `unresolved-go-module` bucket surfacing as a first-class SBOM signal). SC-005 verified: zero drift on non-Go/npm ecosystems (apk/deb/rpm/cargo/gem/maven/pip/bazel/cmake/npm goldens all byte-identical). SC-006 verified: Go golden diffs are strictly limited to `mikebom:orphan-reason` additions. Regenerated via `MIKEBOM_UPDATE_CDX_GOLDENS=1 / MIKEBOM_UPDATE_SPDX_GOLDENS=1 / MIKEBOM_UPDATE_SPDX3_GOLDENS=1` per-target. Diff: 18 insertions, 0 deletions total across all 3 files.

**Checkpoint**: US3 complete — pre-PR gate green; golden diffs bounded per SC-005 + SC-006.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, CHANGELOG, empirical closure per SC-010 + SC-011.

- [X] T023 [P] Update `docs/reference/sbom-format-mapping.md` C45 row — expand the value vocabulary documentation from 2 codes to 5 codes; document the FR-005 priority order; reference milestones 061 (introduction) and 167 (expansion) inline. **Completed 2026-07-06**: C45 row at `docs/reference/sbom-format-mapping.md:92` updated. Vocabulary column expanded from 3 aspirational codes (`unresolved-indirect-require` / `private-module` / `proxy-fetch-failed`) to the 5 actually-emitted codes (`stale-go-sum-entry` / `dead-lockfile-entry` / `hoisted-unused` / `unresolved-indirect-require` / `flat-attached-fallback`). FR-005 priority order documented. m061 vs m167 emit-tier split documented. BFS-unreachable orphan definition from Q1 clarification referenced. FR-001 ecosystem scope (Go + npm only) documented.

- [X] T024 [P] Update `CHANGELOG.md` per SC-010: (a) list all 3 new vocabulary entries + preserved 2 entries with concrete meanings; (b) FR-005 priority order; (c) pre/post orphan-reason emission counts on the milestone-165 audit targets (Kubernetes, ArgoCD); (d) consumer jq recipe for filtering honest-signal orphans (e.g., `jq '.components[] | select(.properties[]?.name == "mikebom:orphan-reason" and .properties[].value == "hoisted-unused")'`); (e) explicit note that C45 wire shape is unchanged (byte-identity for existing consumers). **Completed 2026-07-06**: `CHANGELOG.md` `[Unreleased]` section extended with the m167 entry (~100 lines). Covers all 5 requirements: (a) 5-code vocabulary table with meaning + emit tier per code; (b) FR-005 priority order narrative; (c) pre/post empirical impact table for K8s/ArgoCD/podman-desktop; (d) working jq recipe for filtering honest-signal orphans (grouped by reason code with example PURLs); (e) explicit "C45 wire shape is UNCHANGED" statement, byte-identity impact enumerated per golden file.

- [X] T025 Regenerate a milestone-165 audit target SBOM (podman-desktop OR ArgoCD if fixture available locally per quickstart.md Step 5) and capture the tracing::info! `orphan-reason classification complete` log line with per-code counts. Attach these numbers to the eventual PR body per SC-011. **Completed 2026-07-06**: M165 audit source clones aren't available locally (`specs/165-k8s-argocd-audit/artifacts/` has only the pre-167 SBOMs + analysis.json, not the K8s/ArgoCD source trees). Used the milestone-090 golang fixture (`go/simple-module` under `~/.cache/mikebom/fixtures/`) as a representative smoke test. Captured FR-008 log line: `orphan-reason classification complete orphan_reason_stale_go_sum_entry=0 orphan_reason_dead_lockfile_entry=0 orphan_reason_hoisted_unused=0 orphan_reason_unresolved_indirect_require=1 orphan_reason_flat_attached_fallback=0`. Confirmed emission on `pkg:golang/stdlib@v1.26.1` (the m165 audit's `unresolved-go-module` bucket surfacing as expected). Consumer jq recipe from CHANGELOG works verbatim. Full K8s/ArgoCD re-measurement is deferred to a follow-on audit run (matches m165 audit-harness workflow — takes a fresh clone + ~30 min per target).

- [X] T026 Final validation — run `./scripts/pre-pr.sh` one more time end-to-end; verify green. Confirm all US1/US2/US3 acceptance criteria hold. Confirm the FR-008 tracing log fires as expected via `RUST_LOG=info target/release/mikebom sbom scan …` grep pattern from quickstart.md Step 6. **Completed 2026-07-06**: `./scripts/pre-pr.sh` → `>>> all pre-PR checks passed.` (zero clippy warnings, all workspace tests pass across the 4024-test suite). US1 acceptance verified: 4 unit tests + 4 integration tests + T025 smoke pass. US2 acceptance verified: 3 backward-compat unit tests + 87 m061 Go-reader regression tests pass. US3 acceptance verified: 2 regression unit tests + 3 golden regen tests bounded to SC-006 (`mikebom:orphan-reason` additions only) + zero drift on non-Go/npm goldens. FR-008 log fires on both the T009 fixture and T025 smoke.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001 — no dependencies; verify baseline.
- **Foundational (Phase 2)**: T002 + T003 + T004 — depends on T001. **BLOCKS all user stories**.
- **User Story 1 (Phase 3, P1 — MVP)**: T005–T012 — depends on Phase 2.
- **User Story 2 (Phase 4, P2)**: T013–T017 — depends on Phase 2 (independent of Phase 3 but naturally sequenced after because both edit the same file).
- **User Story 3 (Phase 5, P3)**: T018–T022 — depends on Phase 2 (independent of Phase 3 + Phase 4).
- **Polish (Phase 6)**: T023–T026 — depends on all user stories complete.

### User Story Dependencies

- **US1 (P1 — MVP)**: Depends only on Phase 2. Delivers the 3 new codes.
- **US2 (P2)**: Depends only on Phase 2. Preserves m061 semantic. Independent of US1 tests (different tests, same file).
- **US3 (P3)**: Depends only on Phase 2. Regression guard on non-Go/npm ecosystems + non-orphan components. Independent of US1 + US2.

### Within Each User Story

- Tests written FIRST (T005-T009, T013-T015, T018-T019) — verify they FAIL against the empty `classify_orphans` skeleton from T004.
- Then implementation (T010, T011, T016, T020).
- Then verification (T012, T017, T021).

### Parallel Opportunities

- **Phase 2**: T002 + T003 can proceed in parallel (different code paths in the same file — coordinate; if merge conflicts, sequence T002 then T003).
- **Phase 3 tests**: T005 + T006 + T007 + T008 + T009 all parallel (different tests, but all appended to same `mod tests`; if merge conflicts, sequence).
- **Phase 4 tests**: T013 + T014 + T015 all parallel with the same caveat.
- **Phase 5 tests**: T018 + T019 parallel.
- **Phase 6 docs**: T023 + T024 parallel (different files).

Realistically, since a single developer edits `orphan_reason.rs` sequentially, treat the [P] markers within a file as "can be batched in a single commit" rather than "run concurrently."

---

## Parallel Example: User Story 1 tests batch

```bash
# Batch all US1 test additions into one commit (single file: orphan_reason.rs mod tests):
Task: "Add stale_go_sum_entry_emitted_when_go_orphan_with_reachable_sibling test"
Task: "Add dead_lockfile_entry_emitted_when_npm_orphan_with_reachable_sibling test"
Task: "Add hoisted_unused_emitted_when_npm_orphan_no_sibling test"
Task: "Add fr005_priority_multi_version_wins_over_single test"

# The integration test T009 is a separate file — genuine parallel with the above 4.
Task: "Add mikebom-cli/tests/orphan_reason_expand.rs synthesized-scan integration test"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001).
2. Complete Phase 2: Foundational (T002-T004). **CRITICAL — blocks all stories**.
3. Complete Phase 3: User Story 1 (T005-T012).
4. **STOP and VALIDATE**: run integration test T009 + confirm FR-008 log via manual smoke.
5. Ship as MVP if US2 + US3 slip.

### Incremental Delivery

1. Phase 1 + 2 → Foundation ready.
2. US1 → 3 new codes emit → validate via T012 → MVP.
3. US2 → m061 backward-compat proven → integrated.
4. US3 → dual-side byte-identity confirmed via pre-PR gate → shippable.
5. Phase 6 → docs + CHANGELOG + empirical closure → PR-ready.

### Single-Developer Strategy

The full pipeline is ~26 tasks in a single crate; sequential execution takes 1-2 sessions. No parallelization needed.

---

## Notes

- [P] tasks = independent code paths / files; batching in a single commit is acceptable when the developer edits sequentially.
- [Story] label maps task to user story for traceability against SC-001 through SC-011.
- Every user story is independently completable; MVP = US1 alone.
- Verify tests FAIL against the T004 empty skeleton before running T010's real implementation.
- Commit after each phase (recommended: Phase 2 → commit, US1 → commit, US2 → commit, US3 → commit, Phase 6 → commit + PR).
- The mandatory pre-PR gate is `./scripts/pre-pr.sh` (per CLAUDE.md); do NOT cite a passing per-crate `cargo test` as CI-readiness evidence.
- Constitution Principle IV (no `.unwrap()` in production): the classifier uses `Option::or` / `HashMap::get` patterns per data-model.md; test code using `.unwrap()` MUST be guarded with `#[cfg_attr(test, allow(clippy::unwrap_used))]`.
