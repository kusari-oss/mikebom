---

description: "Tasks for milestone 172 — add doc-scope mikebom:go-transitive-fallback-count annotation"
---

# Tasks: Go-transitive fallback attachment count doc-scope annotation

**Input**: Design documents from `/specs/172-go-fallback-count/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/{annotation-wire-shape,emission-gating}.md, quickstart.md

**Tests**: 1 new integration test at `mikebom-cli/tests/go_fallback_count.rs` covering the 3 emission-gating scenarios from FR-006 + the SC-005 count-sum invariant.

**Organization**: 2 user stories from spec.md (US1 P1 + US2 P2) + setup + foundational + polish. Small feature — ~22 tasks total. All LLM-executable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm branch state + verify current pre-fix output shape (no C117 annotation).

- [X] T001 Confirm current branch is `172-go-fallback-count` via `git rev-parse --abbrev-ref HEAD`. If not, `git checkout 172-go-fallback-count`.

- [X] T002 Verify pre-172 state — annotation `mikebom:go-transitive-fallback-count` MUST NOT already be emitted. Verified via source-grep (`grep -rn "go-transitive-fallback-count" mikebom-cli/src/ mikebom-common/src/` → 0 hits) + golden-grep (`grep -rl "go-transitive-fallback-count" mikebom-cli/tests/fixtures/golden/` → 0 hits). Stronger than a runtime jq check — proves the string is not emitted anywhere in source OR embedded in any golden. Baseline confirmed.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Add the `LadderSummary.go_transitive_fallback_count` field so subsequent US1 tasks have data to thread through the pipeline. Blocks Phase 3.

- [X] T003 **NO-OP — field already exists**. Discovery during analyze phase: `LadderSummary.gosum_fallback_count: usize` (line 371 of `graph_resolver.rs`) is the same field this milestone would have added. It's already populated at line 916 whenever a module resolves via `ResolutionStep::GoSumFallback` (`map.summary_mut().gosum_fallback_count += 1`). m172 exposes this existing field; no schema change needed. Naming reconciliation: keep the existing `gosum_fallback_count: usize` name inside `LadderSummary` (renaming it would ripple through 10+ callsites for zero benefit); expose it externally via the new `mikebom:go-transitive-fallback-count` annotation name (which is the standards-facing name).

- [X] T004 **NO-OP — population already happens**. Line 916 (`map.summary_mut().gosum_fallback_count += 1`) fires for every module that lands on step 5 per m091's step-5 handler. No additional population needed.

- [X] T005 Add unit test in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`'s existing `#[cfg(test)] mod tests` block — `ladder_summary_gosum_fallback_count_readable`: verifies the pre-existing `gosum_fallback_count` counter is a public field readable from external callers. Constructs `let s = LadderSummary { gosum_fallback_count: 5, ..Default::default() };`, asserts `s.gosum_fallback_count == 5`. This is a "sanity gate" that the field name doesn't get renamed later. Fast — 3-line test. **Verified**: `cargo test -p mikebom --bin mikebom -- ladder_summary_gosum_fallback_count_readable` → `1 passed; 0 failed`.

**Checkpoint**: Phase 3 (US1) can start once T003-T005 are green.

---

## Phase 3: User Story 1 — Doc-scope annotation emitted (Priority: P1) 🎯 MVP

**Goal**: Every SBOM produced from a Go-touching scan has `mikebom:go-transitive-fallback-count` in its doc-scope properties/annotations across CDX 1.6, SPDX 2.3, and SPDX 3.

**Independent Test**: quickstart.md Path A → value=`"0"`; Path B → value>0; Path C → absent.

- [X] T006 [US1] Add `pub go_transitive_fallback_count: Option<usize>` field to `ScanResult` in `mikebom-cli/src/scan_fs/mod.rs` (per research §R1's line 98 — sibling of `go_transitive_coverage`). Include doc comment matching the spec's Entity 2 shape.

- [X] T007 [US1] Populate the new `ScanResult` field: in `scan_fs::mod.rs` around line 268-271 declare `let mut go_transitive_fallback_count: Option<usize> = None;`. Around line 307 (where `go_transitive_coverage` is assigned from `scan_result.diagnostics`), also assign `go_transitive_fallback_count = Some(scan_result.diagnostics.gosum_fallback_count);` — note the **source field is `gosum_fallback_count`** (m091 name at `graph_resolver.rs:371`), NOT `go_transitive_fallback_count` (the exported annotation name). **Type-wrapping note**: per data-model.md Entity 1, `LadderSummary.gosum_fallback_count` is a bare `usize` — always populated when the summary was built. `ScanResult.go_transitive_fallback_count` per Entity 2 is `Option<usize>` — `None` means "no Go scan happened". The `Some(...)` wrap converts the always-present `usize` on the summary into the presence-signaling `Option` on the ScanResult. Do NOT use `.clone()` on the `usize` — `usize` is `Copy`; use direct value. At line 809 add the field to the `ScanResult` returned by `scan_path`.

- [X] T008 [US1] Add `pub go_transitive_fallback_count: Option<usize>` field to `SbomEmission` in `mikebom-cli/src/generate/mod.rs` (per research §R1's line 75-80 — sibling of `go_transitive_coverage`). Add doc comment.

- [X] T009 [US1] Wire `SbomEmission.go_transitive_fallback_count` at every construction site (enumerated via `grep -rn "go_transitive_coverage:" mikebom-cli/src/generate/ mikebom-cli/src/cli/`). **3 real construction sites — value derives from scan/artifacts**:
  - `mikebom-cli/src/cli/scan_cmd.rs:2613` — primary emission call; use `scan_result.go_transitive_fallback_count` (Option value forwarded)
  - `mikebom-cli/src/generate/spdx/v3_document.rs:99` — use `scan.go_transitive_fallback_count`
  - `mikebom-cli/src/generate/spdx/document.rs:462 + :490` — use `artifacts.go_transitive_fallback_count` at BOTH sites (mirror `go_transitive_coverage` pattern)
  
  **6 test-harness stub sites — `None`** (mirroring the m170 T011-T015 audit precedent — sites that construct `SbomEmission` in `#[cfg(test)] mod tests` blocks to exercise a single emitter in isolation):
  - `mikebom-cli/src/generate/cyclonedx/builder.rs:134`
  - `mikebom-cli/src/generate/spdx/document.rs:1165`
  - `mikebom-cli/src/generate/spdx/mod.rs:388`
  - `mikebom-cli/src/generate/spdx/packages.rs:724`
  - `mikebom-cli/src/generate/spdx/relationships.rs:345`
  - `mikebom-cli/src/generate/openvex/mod.rs:246`
  
  Preserve m170's plumbing pattern verbatim. If the compiler flags missing fields at any of these 9 sites, add the field with the mirrored value from the enumeration above.

- [X] T010 [US1] Emit C117 in CDX at `mikebom-cli/src/generate/cyclonedx/metadata.rs`. Immediately after the existing C110/C111 emission block (~line 481), add a new emission block per contracts/annotation-wire-shape.md:
  ```rust
  if let Some(count) = go_transitive_fallback_count {
      properties.push(json!({
          "name": "mikebom:go-transitive-fallback-count",
          "value": count.to_string(),
      }));
  }
  ```
  Match indentation + comment style of the C110 block.

- [X] T011 [US1] Emit C117 in SPDX 2.3 at `mikebom-cli/src/generate/spdx/annotations.rs`. Add block next to the existing C110 emission, same shape as T010 but using the SPDX-2.3 `push` helper (the `MikebomAnnotationCommentV1` envelope-wrapping helper already used by other m160 annotations).

- [X] T012 [US1] Emit C117 in SPDX 3 at `mikebom-cli/src/generate/spdx/v3_annotations.rs`. Add block next to the existing C110 emission, using the SPDX-3 `push` helper (creates typed `Annotation` graph elements).

- [X] T013 [US1] Add C117 parity extractor row + helpers. In `mikebom-cli/src/parity/extractors/mod.rs`, add `ParityExtractor { row_id: "C117", label: "mikebom:go-transitive-fallback-count", cdx: c117_cdx, spdx23: c117_spdx23, spdx3: c117_spdx3, directional: Directionality::SymmetricEqual, order_sensitive: false }` in alphabetical position after C116. Also add `c117_cdx`/`c117_spdx23`/`c117_spdx3` to the three import lists.

- [X] T014 [US1] Add extractor helper `c117_cdx` in `mikebom-cli/src/parity/extractors/cdx.rs` using the `cdx_anno!` macro with `document` scope: `cdx_anno!(c117_cdx, "mikebom:go-transitive-fallback-count", document);`. Place alphabetically after `c116_cdx`.

- [X] T015 [US1] Add extractor helper `c117_spdx23` in `mikebom-cli/src/parity/extractors/spdx2.rs` using the `spdx23_anno!` macro: `spdx23_anno!(c117_spdx23, "mikebom:go-transitive-fallback-count", document);`. Place alphabetically after `c116_spdx23`.

- [X] T016 [US1] Add extractor helper `c117_spdx3` in `mikebom-cli/src/parity/extractors/spdx3.rs` using the `spdx3_anno!` macro: `spdx3_anno!(c117_spdx3, "mikebom:go-transitive-fallback-count", document);`. Place alphabetically after `c116_spdx3`.

- [X] T017 [US1] Regenerate the 3 Go goldens per research §R4: `MIKEBOM_UPDATE_CDX_GOLDENS=1 cargo +stable test -p mikebom --test cdx_regression 2>&1 | tail -3 && MIKEBOM_UPDATE_SPDX_GOLDENS=1 cargo +stable test -p mikebom --test spdx_regression 2>&1 | tail -3 && MIKEBOM_UPDATE_SPDX3_GOLDENS=1 cargo +stable test -p mikebom --test spdx3_regression 2>&1 | tail -3`. Expected diff per Go golden: 4 lines added (CDX), ~5 lines added (SPDX 2.3), ~7 lines added (SPDX 3). Non-Go goldens (~30 files) MUST show zero delta per SC-008. Verify: `git diff main --stat -- 'mikebom-cli/tests/fixtures/golden/**' | grep -v golang | grep -v CLAUDE.md` should return empty.

- [X] T018 [US1] Add integration test at `mikebom-cli/tests/go_fallback_count.rs` covering FR-006's 3 scenarios: (a) `t018_healthy_go_scan_emits_zero` — scan a synthesized Go fixture with warm cache; assert `mikebom:go-transitive-fallback-count == "0"`. (b) `t018_degraded_go_scan_emits_positive` — scan same Go fixture with `env -i GOPROXY=off GOMODCACHE=/tmp/nonexistent`; assert value > 0. (c) `t018_non_go_scan_omits_annotation` — scan a pure npm/Rust fixture; assert annotation absent. Also (d) `t018_sc005_count_sum_invariant` — for the degraded scenario, assert the doc-scope count equals the sum of components tagged `mikebom:go-transitive-source == "go-sum-fallback"`. Follow the pattern in `mikebom-cli/tests/ipk_reader.rs` (m169) for scan invocation via `Command::new(bin())`.

**Checkpoint**: US1 delivered — every Go-touching SBOM emits C117 across all 3 formats; SC-001/002/003/004/005 all satisfied.

---

## Phase 4: User Story 2 — Docs enrichment (Priority: P2)

**Goal**: Reading guide explains the 5-step ladder mechanism + provides jq recipe for detecting flat-fallback contamination. Prevents future confusion.

**Independent Test**: A future reader opens `docs/reference/reading-a-mikebom-sbom.md` and can explain the fallback mechanism + use the jq recipe from the doc alone.

- [X] T019 [P] [US2] Add row C117 to `docs/reference/sbom-format-mapping.md`. Insert after C116 (dep-alternative-alternates from m169). Use the exact row content from data-model.md Entity 6 — includes the Constitution Principle V "KEEP-NO-NATIVE" audit + companion-to-C108/C110 explanation.

- [X] T020 [P] [US2] Enrich `docs/reference/reading-a-mikebom-sbom.md` where `mikebom:go-transitive-coverage` is currently documented (added by m170 T029b under §3.4 Transparency / completeness gaps per research §R5). Add a new subsection alongside or immediately after the existing coverage callout — either as its own `#### mikebom:go-transitive-fallback-count` heading or as an enrichment to the existing coverage section — explaining:
  1. **The 5-step ladder**: `go mod graph` (step 1) → `$GOMODCACHE` walk (step 2) → `$GOPROXY` fetch (step 3) → go.sum flat fallback (step 5) → unresolved (step 6). Note that step 4 is retired numbering.
  2. **Why step 5 changes graph shape**: it emits flat root→transitive edges, losing parent-child topology.
  3. **The new C117 annotation**: what it counts, when it's emitted (per Q1's emit-0-explicit rule).
  4. **The jq recipe** from quickstart.md Path B assertion. Copy the recipe verbatim.
  5. **Cross-reference to C108** per-component `mikebom:go-transitive-source` for granular per-module attribution.
  Target length: ~40-60 lines added.

**Checkpoint**: US2 delivered — reading guide covers the mechanism + the diagnostic recipe.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Verify pre-PR gate + diff scope + walker audit + no non-Go regressions.

- [X] T021 Run walker-audit CI-gate locally per memory `feedback_walker_audit_local_check`. m172 touches no walker code — the audit should PASS with zero drift. Command: absolute-path `/usr/bin/sed` variant per m171 T033 precedent. Expected: PASS.

- [X] T022 Run `./scripts/pre-pr.sh` per SC-007. Verify green — `>>> all pre-PR checks passed.` Enumerate any `^---- .+ stdout ----` failure lines before claiming green per memory `feedback_prepr_gate_bails_on_first_failure`. This exercises the m071 parity gate (T013-T016 correctness), the golden regression checks, and the new T018 integration test.

- [X] T023 Diff the working tree against `main` per SC-008. Expected paths changed:
  - `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs` (T003-T005)
  - `mikebom-cli/src/scan_fs/mod.rs` (T006-T007)
  - `mikebom-cli/src/generate/mod.rs` (T008)
  - `mikebom-cli/src/generate/cyclonedx/metadata.rs` (T010)
  - `mikebom-cli/src/generate/spdx/annotations.rs` (T011)
  - `mikebom-cli/src/generate/spdx/v3_annotations.rs` (T012)
  - `mikebom-cli/src/generate/spdx/document.rs` + `v3_document.rs` + test stubs (T009 wiring)
  - `mikebom-cli/src/cli/scan_cmd.rs` (T009)
  - `mikebom-cli/src/parity/extractors/{cdx,mod,spdx2,spdx3}.rs` (T013-T016)
  - `mikebom-cli/tests/fixtures/golden/{cyclonedx,spdx-2.3,spdx-3}/golang.*` (T017 — exactly 3 files)
  - `mikebom-cli/tests/go_fallback_count.rs` (T018)
  - `docs/reference/sbom-format-mapping.md` (T019)
  - `docs/reference/reading-a-mikebom-sbom.md` (T020)
  - `CLAUDE.md` (auto-updated)
  - `specs/172-go-fallback-count/**` (new)
  Verify SC-008 explicitly: `git diff main --stat -- 'mikebom-cli/tests/fixtures/golden/**' | grep -v golang` MUST return empty (no non-Go goldens changed).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001-T002. No prerequisites.
- **Foundational (Phase 2)**: T003-T005 — depends on Phase 1. **BLOCKS Phase 3**. All subsequent US1 tasks need `LadderSummary.go_transitive_fallback_count` to exist and be populated.
- **User Story 1 (Phase 3, P1)**: T006-T018 — depends on Phase 2 complete.
- **User Story 2 (Phase 4, P2)**: T019-T020 — depends on Phase 3 T013 (parity row exists) for T019's C117 doc consistency; T020 is fully independent of T019.
- **Polish (Phase 5)**: T021-T023 — depends on Phases 3 + 4 complete.

### Within User Story 1

Order matters because plumbing depends on struct fields existing:

1. **T006** → **T007** sequential (ScanResult field + populate)
2. **T008** → **T009** sequential (SbomEmission field + populate at call-sites; T009 may struct-literal-error until T008 lands)
3. **T010** + **T011** + **T012** parallel [P] (three different files, all format emitters — depend on T008 + T009)
4. **T013** → **T014** + **T015** + **T016** parallel [P] (mod.rs row must exist to reference c117_* helpers; the three helper files independent)
5. **T017** (golden regen) depends on T010-T012 (emission code produces the annotation)
6. **T018** (integration test) depends on T017 (goldens don't fight the test)

### Within User Story 2

T019 + T020 fully parallel [P] (different files, both docs).

### Parallel Opportunities

- **Phase 1**: T001 → T002 sequential.
- **Phase 2**: T003 → T004 sequential; T005 [P] runs after T004.
- **Phase 3 US1**: T010 + T011 + T012 parallel [P] after T008-T009 land. T014 + T015 + T016 parallel [P] after T013 lands.
- **Phase 4 US2**: T019 + T020 parallel [P].
- **Phase 5 polish**: T021 [P] with T023 [P] (different concerns); T022 sequential last.

### Independent Test Criteria per User Story

- **US1**: quickstart.md Path A → value `"0"` on healthy Go scan; Path B → value > 0 on degraded scan; Path C → annotation absent on non-Go scan; SC-005 cross-verify (doc-count equals per-component count).
- **US2**: `grep -E "step 5|fallback-count|5-step ladder" docs/reference/reading-a-mikebom-sbom.md` returns matches; visual inspection of the new subsection.

### MVP Scope

**Suggested MVP**: US1 alone (T003-T018 + T021-T023). US2 is doc enrichment that ships alongside for discoverability but could be a follow-up PR.

**Recommended**: land both stories in one PR. Total change is ~30 lines source + ~50 lines docs + 3 golden updates + 1 integration test. Splitting adds process overhead disproportionate to size.
