---
description: "Task list for milestone 160 — Go transitive-edge coverage"
---

# Tasks: Go transitive-edge coverage investigation + gap surface

**Input**: Design documents from `/specs/160-go-transitive-coverage/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/annotations.md, quickstart.md

**Tests**: INCLUDED. SC-008 requires ≥10 unit tests; SC-009 requires a new integration test; SC-001 requires a gated audit test. All 3 test surfaces are load-bearing SC evidence and MUST land alongside the implementation.

**Organization**: Tasks are grouped by the 3 user stories from spec.md (US1 P1 edge coverage, US2 P2 doc-scope signal, US3 P3 byte-identity guard). US1 is the load-bearing MVP.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Different files, no dependencies on incomplete tasks — parallelizable.
- **[Story]**: US1 / US2 / US3 for user-story tasks. Setup, Foundational, Polish have NO story label.

## Path Conventions

Single-project workspace layout per plan.md §Project Structure. All paths absolute-relative to repo root `/Users/mlieberman/Projects/mikebom/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: New Rust types + wire-string serializers used by both US1 and US2.

- [X] T001 [P] Extend `ResolutionStep` enum with `as_wire_str(&self) -> &'static str` method returning the 5-code kebab-case wire vocab (`go-mod-graph`, `module-cache`, `proxy-fetch`, `go-sum-fallback`, `unresolved`) per data-model.md E1 in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T002 [P] Add NEW `UnresolvedReasonClass` enum with 7 variants + `as_wire_str()` + `From<&StepError>` impl per data-model.md E2 in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T003 [P] Add NEW `GoTransitiveCoverage` enum with 3 variants (Complete / Partial(String) / Unknown(String)) + `value_wire_str()` + `reason()` accessors per data-model.md E3 in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T004 [P] Add small `StepError::is_forbidden(&self) -> bool` helper (single-line body: `self.class == ErrorClass::Http4xx && self.detail.contains("403")`) per data-model.md E2 in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T005 Extend `LadderSummary` struct with new field `go_mod_graph_degraded: bool` + new `total_modules(&self) -> usize` helper per data-model.md E4 in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`

**Checkpoint**: All 5 types + serializers compile. `cargo build -p mikebom-cli` succeeds. No behavior change yet.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core `compute_coverage()` logic + `ScanDiagnostics` field wiring + parity-catalog macro registration. All user stories depend on this.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T006 Implement `compute_coverage(summary: &LadderSummary, ctx: &WorkspaceContext) -> GoTransitiveCoverage` per research.md R4 priority-ladder (offline → goproxy-off → go_mod_graph_degraded → missing_count>0 → Complete) in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T007 Wire `go_mod_graph_degraded` field population in `go_mod_graph.rs` — set to true when the subprocess spawn fails OR when the parser rejects the output — in `mikebom-cli/src/scan_fs/package_db/golang/go_mod_graph.rs`
- [X] T008 Wire `compute_coverage()` invocation inside `GraphResolver::resolve()` post-ladder, store result on a new `resolve_output.coverage: GoTransitiveCoverage` return field (or extend the existing `Result<ModuleGraphMap>` return to include it via a wrapper struct — pick the pattern matching the existing shape) in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T009 Add `go_transitive_coverage: Option<GoTransitiveCoverage>` field to `ScanDiagnostics` struct, sibling to the existing `go_graph_completeness` per data-model.md E5 in `mikebom-cli/src/scan_fs/mod.rs`
- [X] T010 Populate `ScanDiagnostics.go_transitive_coverage` from the resolver output at the same call site where `go_graph_completeness` is already populated in `mikebom-cli/src/scan_fs/mod.rs`
- [X] T011 [P] Register C108 (`mikebom:go-transitive-source`, component) + C109 (`mikebom:go-transitive-unresolved-reason`, component) + C110 (`mikebom:go-transitive-coverage`, document) + C111 (`mikebom:go-transitive-coverage-reason`, document) macro invocations per contracts/annotations.md §Parity catalog integration in `mikebom-cli/src/parity/extractors/cdx.rs`
- [X] T012 [P] Register C108/C109/C110/C111 `spdx23_anno!` invocations per contracts/annotations.md §Parity catalog integration in `mikebom-cli/src/parity/extractors/spdx2.rs`
- [X] T013 [P] Register C108/C109/C110/C111 `spdx3_anno!` invocations per contracts/annotations.md §Parity catalog integration in `mikebom-cli/src/parity/extractors/spdx3.rs`
- [X] T014 Register 4 `ParityExtractor` entries (C108/C109/C110/C111 with `Directionality::SymmetricEqual`, `order_sensitive: false`) adjacent to the existing C104/C105/C106/C107 block per contracts/annotations.md §Parity catalog integration in `mikebom-cli/src/parity/extractors/mod.rs`

**Checkpoint**: Types wired end-to-end. `cargo +stable clippy --workspace --all-targets -- -D warnings` clean. Parity registration in place but not yet exercised (annotations not yet emitted — happens in US1/US2).

---

## Phase 3: User Story 1 - test-podman ≥90% edge coverage + per-component annotations (Priority: P1) 🎯 MVP

**Goal**: Fix the FR-006 root causes so `test-podman` online-mode edge coverage rises from 52.2% to ≥90% vs `go mod graph`. Emit per-component `mikebom:go-transitive-source` (universal per Q2) + conditional `mikebom:go-transitive-unresolved-reason`.

**Independent Test**: Scan `test-podman` in online mode. Assert (a) `|mikebom_edges ∩ go_mod_graph_edges| / |go_mod_graph_edges| ≥ 0.90` (SC-001); (b) the 2 cross-platform missing edges from `containernetworking/plugins@v1.9.1` are present (SC-002 spot-checks: `alexflint/go-filemutex`, `buger/jsonparser`); (c) every Go component carries `mikebom:go-transitive-source` (SC-004).

### FR-006 investigation + fixes for User Story 1

- [ ] T015 [US1] Instrument milestone-055's `proxy_fetch.rs::parse_module_mod` with `tracing::debug!` lines counting parsed vs kept requires per module. Land the instrumentation ONLY (no fix yet); use it to record the pre-fix baseline against `test-podman` in `mikebom-cli/src/scan_fs/package_db/golang/proxy_fetch.rs`
- [ ] T016 [US1] Empirical investigation — run the T015-instrumented binary against `$MIKEBOM_FIXTURES_DIR/transitive_parity/golang/test-podman`, capture per-module `parsed vs kept` counts, cross-reference with `go mod graph` output, identify which of FR-006a (parser-drops `// indirect`), FR-006b (go.sum fallback not triggering on proxy-fetch failure), FR-006c (offline mode per-fetch warn spam) apply for the 5 SC-002 spot-check edges. Document findings inline as code comments in the fix commits, no separate deliverable file needed. Investigation-only task — no code fix here.
- [ ] T017 [US1] Fix FR-006a: if T016 found `// indirect` deps being over-filtered by `parse_module_mod`, correct the filter to preserve `// indirect` lines while still dropping lines that are actually not requires (empty lines, comments, blocks). Reference: contrast the current filter against `parse_go_mod` at `legacy.rs` which handles the same file format elsewhere. Fix in `mikebom-cli/src/scan_fs/package_db/golang/proxy_fetch.rs`
- [ ] T018 [US1] Fix FR-006b: if T016 found the milestone-091 go.sum fallback not exercised on proxy-fetch-failure paths (only on step-3-unavailable paths), extend `GraphResolver::resolve()` to call step 5 (go.sum fallback) BOTH on `StepResult::Unavailable` AND on `StepResult::Failed(_)`. Preserve the existing behavior when step 3 succeeds. Fix in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [ ] T019 [US1] Fix FR-006c: when `WorkspaceContext.offline == true`, `GraphResolver::resolve()` MUST skip step 3 (proxy-fetch) entirely at ladder-start rather than per-fetch. Emit ONE info-level `"go transitive edges: offline mode, skipping proxy fetch"` log line before the ladder, not N warn logs during the ladder. Fix in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [ ] T020 [US1] Remove the T015 debug-instrumentation (`tracing::debug!` lines added in T015). Investigation is complete; leave only the fix-side logging in `mikebom-cli/src/scan_fs/package_db/golang/proxy_fetch.rs`

### Per-component annotation emission for User Story 1

- [X] T021 [US1] Extend the resolver's per-module bookkeeping to track `unresolved_reasons: HashMap<ModuleId, UnresolvedReasonClass>` so the emitter can populate FR-003 C109 conditionally. Populate the map inside `GraphResolver::resolve()` on `StepResult::Failed(step_error)` outcomes via `UnresolvedReasonClass::from(&step_error)` in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T022 [US1] Emit C108 `mikebom:go-transitive-source` universally on every Go `PackageDbEntry.extra_annotations` at the existing insertion site near `legacy.rs:1723` per quickstart.md §3 in `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs`
- [X] T023 [US1] Emit C109 `mikebom:go-transitive-unresolved-reason` conditionally (iff C108 value == `"unresolved"`) at the same site as T022, using the T021 `unresolved_reasons` map lookup, in `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs`

### Tests for User Story 1

- [X] T024 [P] [US1] Unit test: `compute_coverage()` returns `Complete` when `summary.missing_count == 0` AND no unknown-triggers apply per SC-008 (a) in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T025 [P] [US1] Unit test: `compute_coverage()` returns `Partial("proxy-fetch-degraded: N of M modules unresolved")` when `summary.missing_count > 0` and no unknown-triggers apply per SC-008 (b) in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T026 [P] [US1] Unit test: `compute_coverage()` returns `Unknown("offline-mode: ...")` when `ctx.offline == true` regardless of counts per SC-008 (c) in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T027 [P] [US1] Unit test: `compute_coverage()` returns `Unknown("goproxy-off-in-chain: ...")` when `ctx.goproxy.contains_off()` per SC-008 (d) in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T028 [P] [US1] Unit test: `ResolutionStep::as_wire_str()` returns exactly the 5 expected kebab-case strings per contracts/annotations.md §C108 vocabulary table in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T029 [P] [US1] Unit test: `UnresolvedReasonClass::as_wire_str()` returns exactly the 7 expected kebab-case strings per contracts/annotations.md §C109 vocabulary table in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T030 [P] [US1] Unit test: `UnresolvedReasonClass::from(&StepError)` maps `ErrorClass::Http404 → ProxyFetchNotFound`, `ErrorClass::Timeout → ProxyFetchTimeout`, `ErrorClass::Http4xx` with `403` in detail → `ProxyFetchForbidden`, `ErrorClass::Parse → UnknownError` per data-model.md E2 impl in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T031 [P] [US1] Unit test: milestone-091 go.sum fallback triggers on `StepResult::Failed(_)` (T018 fix regression guard) per SC-008 (f) in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T032 [P] [US1] Unit test: FR-006c offline-mode early-skip fires ONE info log, zero warn logs (T019 fix regression guard) per SC-008 (g) in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`
- [X] T032a [US1] Unit test: `parse_module_mod` preserves `// indirect` requires (T017 FR-006a regression guard) per SC-008 (e) — synthesizes a fixture `.mod` blob mirroring `containernetworking/plugins@v1.9.1`'s shape (mix of direct + `// indirect` requires) and asserts the parsed require count == the raw count; also asserts the 2 cross-platform requires (`alexflint/go-filemutex@v1.3.0`, `buger/jsonparser@v1.1.1`) appear in the output; in `mikebom-cli/src/scan_fs/package_db/golang/proxy_fetch.rs`
- [X] T032b [US1] Unit test: per-component `mikebom:go-transitive-source = "go-mod-graph"` emitted on a module resolved via step 1 per SC-008 (h) — synthesizes a `ModuleGraphMap` with one `ModuleGraphEntry` whose `source: ResolutionStep::GoModGraph`; runs the T022 emission path; asserts the resulting `PackageDbEntry.extra_annotations["mikebom:go-transitive-source"] == "go-mod-graph"`; in `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs`
- [X] T032c [US1] Unit test: per-component `mikebom:go-transitive-source = "proxy-fetch"` emitted on a module resolved via step 3 per SC-008 (i); same shape as T032b but `source: ResolutionStep::Proxy`; asserts value == `"proxy-fetch"`; in `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs`
- [X] T032d [US1] Unit test: per-component `mikebom:go-transitive-source = "unresolved"` AND `mikebom:go-transitive-unresolved-reason` present per SC-008 (j); synthesizes `source: ResolutionStep::None` + `unresolved_reasons.get(module_id) = Some(UnresolvedReasonClass::ProxyFetchNotFound)`; asserts BOTH annotations present with correct values; also asserts C109 is NOT present when source is any other variant (negative case); in `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs`
- [ ] T033 [US1] Add SC-001 + SC-002 audit test at `mikebom-cli/tests/go_transitive_coverage_audit.rs` per research.md R5 — gated behind `MIKEBOM_TRANSITIVE_COVERAGE_AUDIT=1` env var; shells out to `go mod graph` on the milestone-090 `test-podman` fixture; parses edges into `HashSet<(String, String)>`; scans the fixture via `env!("CARGO_BIN_EXE_mikebom")`; extracts mikebom's edges from CDX `dependencies[].dependsOn[]`. TWO assertions: (a) SC-001 aggregate — intersection ratio `|mikebom ∩ go_mod_graph| / |go_mod_graph| ≥ 0.90` with a 20-sample diagnostic failure message; (b) SC-002 spot-check — for each of the 5 `containernetworking/plugins@v1.9.1` outbound edges (`alexflint/go-filemutex@v1.3.0`, `buger/jsonparser@v1.1.1`, `Microsoft/hcsshim@v0.13.0`, `coreos/go-iptables@v0.8.0`, `containerd/cgroups/v3@v3.0.3`), assert either the edge is present in mikebom's output OR the target component carries `mikebom:go-build-tag-filtered` OR `mikebom:go-transitive-unresolved-reason` naming why it's absent

**Checkpoint**: US1 is fully functional. Running `MIKEBOM_TRANSITIVE_COVERAGE_AUDIT=1 cargo test --test go_transitive_coverage_audit` on `test-podman` fixture emits ≥90% edge coverage AND explicitly verifies the 5 SC-002 spot-check edges from `containernetworking/plugins@v1.9.1`. Every Go component has `mikebom:go-transitive-source`. SC-008 unit-test floor covered by T024–T032d (13 unit tests: SC-008 sub-items a–j + 3 wire-string / mapping tests).

---

## Phase 4: User Story 2 - Document-scope go-transitive-coverage signal (Priority: P2)

**Goal**: Emit `mikebom:go-transitive-coverage` doc-scope annotation with `complete` / `partial` / `unknown` values + conditional `mikebom:go-transitive-coverage-reason` when != complete. Universal presence on scans with ≥1 Go component.

**Independent Test**: For every emitted SBOM containing ≥1 Go component, assert `mikebom:go-transitive-coverage` is present at document scope exactly once (SC-005). Value ∈ 3-code vocab. If value ∈ `{partial, unknown}`, reason is present with a documented code.

### Document-scope emission for User Story 2

- [X] T034 [US2] Emit C110 `mikebom:go-transitive-coverage` at document scope in the CLI's SBOM assembly path, guarded by `if go_component_count > 0 && diagnostics.go_transitive_coverage.is_some()`, per quickstart.md §4 in `mikebom-cli/src/cli/scan_cmd.rs`
- [X] T035 [US2] Emit C111 `mikebom:go-transitive-coverage-reason` at document scope conditionally (iff `coverage.reason().is_some()`) at the same call site as T034 in `mikebom-cli/src/cli/scan_cmd.rs`
- [X] T036 [US2] Wire the FR-010 info-level tracing log `"go transitive edges resolution summary"` with fields `total_modules`, `go_mod_graph_count`, `cache_count`, `proxy_count`, `gosum_count`, `unresolved_count` — emitted at scan-emission time. Reuse `LadderSummary::fmt` output or expand it as needed in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`

### Tests for User Story 2

- [X] T037 [P] [US2] Unit test: doc-scope emission code emits C110 iff `go_component_count > 0` — synthesizes a `ScanResult` with 1 Go component + a `GoTransitiveCoverage::Complete` diagnostics; asserts CDX output contains `mikebom:go-transitive-coverage` in `metadata.properties[]` per SC-005 in `mikebom-cli/src/cli/scan_cmd.rs`
- [X] T038 [P] [US2] Unit test: doc-scope emission code emits C111 iff coverage is Partial or Unknown — synthesizes coverage `Partial("proxy-fetch-degraded: 45 of 300 modules unresolved")`; asserts C111 property with the exact reason string in `mikebom-cli/src/cli/scan_cmd.rs`
- [X] T039 [US2] Integration test at `mikebom-cli/tests/go_transitive_coverage.rs` per SC-009 — synthesizes a 3-module Go workspace where 2 modules fetch-succeed via mock proxy (`wiremock`) and 1 module fetch-fails (404), invokes the release binary, asserts (a) per-component C108 values match ladder attribution (2× `proxy-fetch`, 1× `unresolved`), (b) per-component C109 present on the failing module with value `proxy-fetch-not-found`, (c) doc-scope C110 == `partial`, (d) doc-scope C111 == `proxy-fetch-degraded: 1 of 3 modules unresolved`

**Checkpoint**: US2 is fully functional. Doc-scope annotations emitted on every Go-containing scan. Integration test T039 passes.

---

## Phase 5: User Story 3 - Non-Go scans byte-identical to pre-160 (Priority: P3)

**Goal**: Regression guard. Verify the milestone-090 non-Go goldens (10 ecosystems × 3 formats = 30 files) are byte-identical to pre-160. The `golang` fixture is exempt.

**Independent Test**: `git diff <pre-160-sha> HEAD -- mikebom-cli/tests/fixtures/goldens/{apk,bazel,cargo,cmake,deb,gem,maven,npm,pip,rpm}/*.{cdx.json,spdx.json,spdx3.jsonld}` produces zero output.

### Golden regeneration + verification for User Story 3

- [X] T040 [US3] Regenerate the milestone-090 `golang` fixture goldens with the milestone-160 code: `MIKEBOM_UPDATE_GOLDENS=golang cargo +stable test --workspace --no-fail-fast`. Manually inspect the diff to confirm ONLY the new `mikebom:go-transitive-*` annotations changed (no edge topology changes on the fixture, which uses source-tier resolution not requiring proxy fetches)
- [X] T041 [US3] Verify SC-003 dual-side byte-identity: run the loop from quickstart.md §6 checking every non-golang fixture golden. Any non-zero diff on the 30 non-golang goldens indicates a regression (likely from an over-emission bug — C108 leaking to a non-Go component). Fix if any diff surfaces, then re-verify

**Checkpoint**: 30 non-Go goldens byte-identical to pre-160. `golang` golden changes limited to the new annotations.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, CHANGELOG, pre-PR gate, issue closure.

- [X] T042 [P] Add `CHANGELOG.md` entry per SC-010 documenting: (a) motivation (issue #494 + milestone-157/158/159 Round-2 audit), (b) FR-006 fix summary (parser-drop / go.sum fallback / offline early-skip), (c) new annotation vocab table (C108/C109/C110/C111), (d) empirical impact — pre/post SC-001 numbers on test-podman, (e) consumer jq recipe from contracts/annotations.md §Consumer jq recipes, (f) Q1-Q4 clarification bullets
- [X] T043 [P] Update `docs/reference/sbom-format-mapping.md` with the 4 new mikebom-prefixed annotations (C108/C109 per-component + C110/C111 document-scope) — per-format wire shape per contracts/annotations.md
- [X] T044 [P] Update `docs/reference/component-tiers.md` (or the appropriate consumer-facing doc) with the FR-006c offline-mode behavior change (single info log at ladder-start instead of per-fetch warns)
- [X] T045 Run `./scripts/pre-pr.sh` — both `cargo +stable clippy --workspace --all-targets -- -D warnings` and `cargo +stable test --workspace --no-fail-fast` MUST pass clean (SC-007). Any failure blocks PR opening
- [ ] T046 Verify `MIKEBOM_TRANSITIVE_COVERAGE_AUDIT=1 cargo test --test go_transitive_coverage_audit` passes with SC-001 ≥ 0.90 on the `test-podman` fixture. If empirically below target but above pre-160 baseline (52.2%), revise SC-001 inline per Assumptions §7 and document the revised floor in CHANGELOG
- [ ] T047 Include `closes #494` in the impl PR body per SC-012 so merging the PR auto-closes the tracking issue

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No prerequisites. Types + serializers land first.
- **Phase 2 (Foundational)**: Depends on Phase 1. Wires `compute_coverage()` + parity catalog. Blocks US1/US2/US3.
- **Phase 3 (US1)**: Depends on Phase 2. FR-006 investigation + fixes + per-component emission + 10 SC-008 unit tests + SC-001 audit test.
- **Phase 4 (US2)**: Depends on Phase 2 (also on US1's T021 `unresolved_reasons` bookkeeping for shared state). Doc-scope emission + integration test.
- **Phase 5 (US3)**: Depends on Phase 3+4 completion (golden regeneration MUST reflect final emission behavior).
- **Phase 6 (Polish)**: Depends on Phases 1-5 completion.

### Within Each User Story

- **US1**: T015 (instrumentation) → T016 (investigation) → T017-T019 (fixes) → T020 (remove instrumentation) → T021 (bookkeeping) → T022-T023 (emission) → T024-T033 (tests, most parallel).
- **US2**: T034-T036 (emission) → T037-T039 (tests, T037-T038 parallel, T039 sequential).
- **US3**: T040 (regen) → T041 (verify).

### Parallel Opportunities

- Phase 1 T001-T004 all `[P]` — same file so NOT actually parallelizable per checklist format (should be sequential); T005 depends on T001-T004 landing first to avoid merge churn on `graph_resolver.rs`. **Correction**: T001-T005 land as a single sequenced commit.
- Phase 2 T011-T013 `[P]` — different files (`cdx.rs`, `spdx2.rs`, `spdx3.rs`) — genuinely parallelizable.
- Phase 3 T024-T032 `[P]` — all unit tests can be authored in parallel; T033 is a separate file so also parallelizable.
- Phase 4 T037-T038 `[P]` — same file but non-conflicting test-fn additions.
- Phase 6 T042-T044 `[P]` — different files.

### File-conflict warnings

Per Task Generation Rules, `[P]` requires different files. Tasks flagged as `[P]` but touching the SAME file lose `[P]` status. Corrected:

- **T001-T004** all touch `graph_resolver.rs` → NOT actually `[P]`; sequence as T001 → T002 → T003 → T004 → T005.
- **T024-T032** all touch `graph_resolver.rs` `#[cfg(test)]` module → NOT actually `[P]`; sequence one after another (they're additive test-fn additions but Rust's `cargo test` orderability requires no compile conflict).
- **T037-T038** both touch `scan_cmd.rs` `#[cfg(test)]` → sequence sequentially.

**Final `[P]` set** (genuine parallel opportunities): T011-T013 (parity extractors in 3 different files), T033 vs T034 (different files), T042-T044 (3 different doc files).

---

## Parallel Example: Phase 2 parity registration

```bash
# T011 + T012 + T013 all edit DIFFERENT files:
Task: "Register C108/C109/C110/C111 cdx_anno! invocations in mikebom-cli/src/parity/extractors/cdx.rs"
Task: "Register C108/C109/C110/C111 spdx23_anno! invocations in mikebom-cli/src/parity/extractors/spdx2.rs"
Task: "Register C108/C109/C110/C111 spdx3_anno! invocations in mikebom-cli/src/parity/extractors/spdx3.rs"

# T014 depends on T011-T013 completing (mod.rs registration references the extractor fns defined by T011-T013).
```

## Parallel Example: Phase 6 docs

```bash
Task: "Add CHANGELOG.md entry per SC-010"
Task: "Update docs/reference/sbom-format-mapping.md with 4 new annotations"
Task: "Update docs/reference/component-tiers.md with FR-006c behavior change"
```

---

## Implementation Strategy

### MVP scope

**US1 alone is the MVP.** Delivers the observable-bug fix (52.2% → ≥90% edge coverage on `test-podman`) + universal per-component annotations. US2's doc-scope signal is a P2 enhancement; US3 is a regression guard.

Ship order:

1. Phase 1 (Setup) — 1 sitting.
2. Phase 2 (Foundational) — 1 sitting. Parity registration is quick given the milestone-127 macro pattern.
3. Phase 3 (US1) — **the heavy lift**. T016 empirical investigation is the load-bearing task; it may take 3-5 investigation loops. T017-T019 fixes depend on T016 findings.
4. **STOP + VALIDATE**: Run T033 audit test. Iterate on T016-T019 if SC-001 < 0.90.
5. Phase 4 (US2) — 1 sitting once US1 stabilizes.
6. Phase 5 (US3) — 1 sitting. Should be trivially green if US1 + US2 don't leak emission to non-Go components.
7. Phase 6 (Polish) — 1 sitting. Pre-PR gate + issue closure.

### Empirical revision escape hatch

Per spec.md Assumptions §7, if T016 investigation reveals FR-006 root causes are more complex than anticipated (e.g., a third parser bug outside FR-006a/b/c), SC-001 target ≥ 90% may be revised inline to a demonstrated floor above 52.2%. CHANGELOG entry (T042) MUST document the revised floor + rationale.

### Parallel team strategy

With 2-3 contributors:

- Contributor A: Phases 1 → 2 → 3 (US1) — the load-bearing sequential path.
- Contributor B: Phase 4 (US2) — starts after Phase 2 lands; parallelizable with US1's tests-only section (T024-T033).
- Contributor C: Phase 5 (US3) — starts after Phases 3+4 land; also handles Phase 6 polish tasks in parallel.

---

## Notes

- All test tasks are load-bearing SC evidence (SC-008 requires ≥10 unit tests; SC-009 requires the integration test; SC-001 requires the audit test). Skipping tests fails the milestone acceptance.
- The FR-006 investigation (T016) is deliberately investigation-first — the exact root causes are not fully knowable at spec time. Documenting findings inline in T017-T019 commit messages is the deliverable, not a separate spec update.
- Preserve milestone-055 fetch-concurrency (16-way, per `graph_resolver.rs:344`) unchanged per FR-007. No perf-tuning tasks in this milestone.
- SC-003 dual-side byte-identity is a REGRESSION GUARD, not new emission. Failing SC-003 during Phase 5 verification indicates an emission-leak bug that needs fixing in US1/US2 before proceeding.
- Constitution Principle IV (`no .unwrap()` in production): all new code follows the milestone-055/091 pattern with `anyhow::Result` + `?` propagation. Test modules using `.unwrap()` MUST be guarded with `#[cfg_attr(test, allow(clippy::unwrap_used))]` per the crate-root convention.
- No new Cargo dependencies (spec Assumption §6). Reuse `wiremock` (already dev-dep from milestone 055) for T039's mock proxy.
- Per Q4 (closed-but-extensible reason vocab): if T016 uncovers a new failure class not covered by the current 5 codes for C111 or 7 codes for C109, discuss vocab extension at PR review; don't unilaterally add codes.
- **Task-ID reference reconciliation**: spec.md, plan.md, research.md, and quickstart.md reference "T014–T016 empirical investigation" as a shorthand for the FR-006 discovery loop. During tasks decomposition this materialized as T015 (instrumentation) → T016 (investigation) → T017–T019 (fixes). Both ID ranges refer to the same body of work; the spec's `T014–T016` label predates the final tasks.md numbering and was not renumbered because the spec is written from the reader's perspective (investigation-first FR-006 shape) rather than the tasks.md executor's perspective (individual atomic steps).
- **T016 acceptance criterion**: T016 is an investigation-only task. Deliverable is the FR-006a/b/c root-cause classification decision, documented inline in the T017–T019 fix-commit messages. Task is "done" when T017, T018, and T019 have been landed with commit messages naming which of FR-006a/b/c their fix addresses.
