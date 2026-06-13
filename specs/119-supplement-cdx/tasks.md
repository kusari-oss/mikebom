# Tasks: Developer-asserted source-of-truth supplement (v0.1, CDX 1.6 input)

**Input**: Design documents from `/specs/119-supplement-cdx/`
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/{supplement-format,merge-pipeline,annotation-shape}.md ✓, quickstart.md ✓

**Tests**: Per spec FRs and quickstart's negative-test runbook, integration tests are the principal validation mechanism. ~13 test functions across three US phases + a polish-phase negative-test suite cover acceptance scenarios + edge cases + safety property + fail-closed behavior.

**Single-PR scope**: ~700 LoC production + ~300 LoC tests + ~150 LoC docs per plan.md estimate. **The shipping cut-point used by the implementation PR was the MVP slice** (Phase 1 + Phase 2 + Phase 3 + the polish subset reachable without SPDX projection): T001, T003–T013, T021 subset (the four negative tests), T022–T025. T002 docs row was moved from Phase 1 into the MVP polish step (executed inline). Deferred to a follow-up milestone-119-phase-2 PR: T014–T016 polish (the conflict-CDX-wire-shape integration tests + license-override propagation onto Cargo's `metadata.component` main-module path), T017–T020 (SPDX 2.3 + SPDX 3 services projection + parity catalog C65/C66/C67 extractors), the remaining T021 negative-test runbook entries.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

Single-project layout (mikebom workspace at repo root). Affected paths:
- `mikebom-cli/src/supplement/{mod,parser,merge,conflict,annotation}.rs` (NEW module — ~550 LoC across 5 files)
- `mikebom-cli/src/main.rs` (clap derive — new flag)
- `mikebom-cli/src/cli/scan_cmd.rs` (thread the supplement through the pipeline)
- `mikebom-cli/src/generate/cyclonedx/builder.rs` (call merge at line 355; consume MergeOutcome)
- `mikebom-cli/src/generate/cyclonedx/metadata.rs` (`mikebom:supplement-cdx` document-scope property)
- `mikebom-cli/src/generate/cyclonedx/services.rs` (NEW FILE — `build_services()`)
- `mikebom-cli/src/generate/spdx/packages.rs` + `v3_packages.rs` (services projection)
- `mikebom-cli/src/parity/extractors/{cdx,spdx2,spdx3,mod}.rs` (three new C-row extractors)
- `mikebom-cli/tests/supplement_cdx_integration.rs` (NEW — 13+ tests covering acceptance + edge cases)
- `docs/reference/sbom-format-mapping.md` (three new C-rows with Principle V audit citations)

---

## Phase 1: Setup

**Purpose**: Establish the clap flag, the supplement module skeleton, and the Constitution Principle V audit docs row + parity catalog scaffolding that every subsequent task references.

- [X] T001 Add the `--supplement-cdx <PATH>` clap flag to `mikebom-cli/src/main.rs` (the global-flag block alongside `--exclude-path` per milestone-113 precedent). Use `Option<PathBuf>` per research.md (single-file v0.1). Doc-comment explains the flag is opt-in, accepts a CDX 1.6 JSON document, and points at `docs/user-guide/cli-reference.md` § `--supplement-cdx`. The flag is single-occurrence in v0.1 (FR-001); repeating it returns a clap error.

- [X] T002 Add three new rows to `docs/reference/sbom-format-mapping.md` per research.md § Decision 8: **C65** (value extension on the existing `mikebom:source-tier` row — add `"declared"` as a new permitted value; cite Principle V audit narrative); **C66** (`mikebom:supplement-cdx` envelope-level provenance — new row, full Principle V audit narrative naming CDX/SPDX-2.3/SPDX-3 native-field gaps); **C67** (`mikebom:assertion-conflict` per-component conflict record — new row, full audit narrative). Each row follows the structure of the existing C63 (exclude-path) row at lines 108-110+. Include CDX carrier, SPDX 2.3 carrier, SPDX 3 carrier, and the Principle V audit conclusion.

- [X] T003 Create new module entry `mikebom-cli/src/supplement/mod.rs` with module-level doc-comment summarizing the merge model (operator-supplied CDX 1.6 → parsed Supplement → merged into ResolvedComponent set), public re-exports of `Supplement`, `MergeOutcome`, `SupplementError`, and the `pub(crate) fn merge()` entry point signature stub. Add `pub(crate) mod supplement;` to `mikebom-cli/src/lib.rs` (or `main.rs` if module visibility doesn't extend that far — verify on first read).

---

## Phase 2: Foundational

**Purpose**: Build the Supplement parser + error types + annotation helpers that every US phase consumes. US1 + US2 + US3 all depend on Phase 2 being complete.

**⚠️ CRITICAL**: No user-story work begins until T004-T007 are complete.

- [X] T004 Implement `mikebom-cli/src/supplement/parser.rs` per data-model.md § Entity 1 + contracts/supplement-format.md. Pub(crate) `Supplement` struct with `source_sha256`, `source_path`, `components: Vec<SupplementComponent>`, `services: Vec<SupplementService>`, `dependencies: Vec<SupplementDependency>` fields. `pub(crate) fn load(path: &Path) -> Result<Supplement, SupplementError>` reads bytes, computes sha256 via `sha2::Sha256::new()` + `data_encoding::HEXLOWER`, parses JSON via `serde_json::from_slice`, validates structurally per Decision 1 (asserts `bomFormat == "CycloneDX"`, `specVersion ∈ {"1.4","1.5","1.6"}`, components/services/dependencies are arrays with required keys, PURL uniqueness, every component has a parsable PURL via `mikebom_common::types::purl::Purl::new()`). Discards `metadata.component` per FR-014 / clarification Q1.

- [X] T005 Implement `SupplementError` enum at `mikebom-cli/src/supplement/parser.rs` (or a sibling `error.rs`) with variants `Io { path, source }`, `ParseJson { path, source }`, `ValidationFailed { path, reason }`, `DuplicatePurl(Purl)`, `DanglingDependsOn(String)` per contracts/merge-pipeline.md § "Error semantics". Implement `thiserror::Error` derive. Each variant's `Display` impl names the path verbatim and the specific failure so operators can diagnose without debug logs (per SC-005).

- [X] T006 Implement `mikebom-cli/src/supplement/annotation.rs` with three helpers: (a) `stamp_source_tier_declared(extra_annotations: &mut BTreeMap<String, serde_json::Value>)` inserts `"mikebom:source-tier" → "declared"`; (b) `stamp_assertion_conflict(extra_annotations: &mut BTreeMap, conflict: &ConflictRecord)` inserts/extends the `"mikebom:assertion-conflict"` key whose value is a **JSON ARRAY of conflict-record objects** per contracts/annotation-shape.md § "Cardinality + storage shape" (committed: BTreeMap stores one `serde_json::Value::Array` per key). Behavior: if the key is absent, insert `serde_json::Value::Array(vec![record_obj])`; if the key is already present (repeat conflict on the same component), call `as_array_mut().unwrap_or(&mut empty).push(record_obj)` to append. The CDX wire shape is ONE `properties[]` entry whose `value` is the JSON-encoded string of the array — the existing emitter at `generate/cyclonedx/builder.rs:965-973` calls `serde_json::to_string(&value)` on each `extra_annotations` entry. (c) `build_supplement_cdx_provenance_string(source_path, source_sha256) -> String` returns `"<path>@sha256:<hex>"` per FR-012 / Decision 6.

- [X] T007 Implement `MergeOutcome` + `ConflictRecord` + `ConflictField` + `ConflictWinner` + `SupplementProvenance` + `ComponentConflicts` structs per data-model.md § Entity 2 + Entity 3. Place in `mikebom-cli/src/supplement/conflict.rs` (conflict-related types) and `mikebom-cli/src/supplement/merge.rs` (merge-outcome types). Public visibility within the supplement module. The `ConflictWinner` derives from `ConflictField` via the FR-006/FR-007 partition (per data-model.md invariant 1) — implement as `impl ConflictField { fn winner(&self) -> ConflictWinner { ... } }` rather than a stored field.

**Checkpoint**: After T004-T007, the foundational types + parser exist; US1/US2/US3 work can begin.

---

## Phase 3: User Story 1 — Merge declared components/services/dependencies (additive case) (Priority: P1) 🎯 MVP

**Goal**: Operator-declared SaaS deps + vendored libraries + service entries from a supplement file appear in the emitted SBOM. The merge is purely ADDITIVE in this phase (no collisions yet — that's US2). Plus the supplement-cdx provenance annotation per FR-012.

**Independent Test**: Run `cargo +stable test --test supplement_cdx_integration` and verify the four US1 acceptance scenarios pass (SaaS service + vendored library + empty-supplement no-op + no-flag byte-identity).

### Implementation for User Story 1

- [X] T008 [US1] Implement the additive path of `mikebom-cli/src/supplement/merge.rs`'s `pub(crate) fn merge(scanner_components: Vec<ResolvedComponent>, scanner_dependencies: Vec<RelationshipEdge>, supplement: Supplement) -> Result<MergeOutcome, SupplementError>`. The additive path handles SOLO entries (supplement PURL doesn't collide with any scanner-discovered PURL): construct a new `ResolvedComponent` from the `SupplementComponent`'s fields, call `annotation::stamp_source_tier_declared()` on its `extra_annotations`, append to the output `components` vec. For services: forward the supplement's services straight through to `MergeOutcome.services`. For dependencies: implement re-anchoring per contracts/supplement-format.md § "Re-anchoring semantics" — supplement bom-refs that have canonical PURLs get rewritten to those PURLs; supplement bom-refs without canonical PURLs are preserved verbatim. Build a `HashMap<Purl, usize>` index over scanner components for O(1) collision detection (used by US2's collision path; for US1 the index is built but every supplement PURL misses). Detect dangling `dependsOn` references and return `Err(SupplementError::DanglingDependsOn(_))` per contracts/merge-pipeline.md § "Error semantics". Implement the FR-015 post-condition assertion `merged.components.len() >= scanner_components.len()` as a runtime debug-assert + a separate test.

- [X] T009 [US1] Extend `mikebom-cli/src/cli/scan_cmd.rs` to thread the supplement through the pipeline per plan.md § Source Code. After parsing args + before invoking the scanner, if `--supplement-cdx` was supplied: call `supplement::parser::load(path)` → `Supplement`; route any `SupplementError` to a non-zero exit with the error message printed to stderr per FR-002 / SC-005. Pass the `Option<Supplement>` through the existing function signature to the generator; the generator's CDX builder consumes it at builder.rs:355 per T010.

- [X] T010 [US1] Wire the merge step into `mikebom-cli/src/generate/cyclonedx/builder.rs:355` (per research.md § Decision 2). Immediately after `build_components()` populates the deduped scanner-discovered `Vec<ResolvedComponent>`, before `build_compositions()` and `build_dependencies()`, call `supplement::merge::merge(scanner_components, scanner_dependencies, supplement)` IFF a supplement was supplied. Replace `scanner_components` + `scanner_dependencies` with `MergeOutcome.components` + `MergeOutcome.dependencies`. Pass `MergeOutcome.services` to a new `build_services()` invocation per T011. Pass `MergeOutcome.supplement_provenance` to `metadata.rs` per T012. When no supplement: skip entirely (preserves byte-identity per FR-013).

- [X] T011 [US1] Create `mikebom-cli/src/generate/cyclonedx/services.rs` with `pub(super) fn build_services(services: &[SupplementService]) -> serde_json::Value` per plan.md § Source Code. Returns a JSON array of CDX 1.6-shaped service entries: each entry carries `bom-ref`, `name`, optional `provider`, `endpoints`, `description`, `licenses`, `externalReferences`. (CDX 1.6 `services[]` entries do NOT carry a `type` field — that field belongs to `components[]` only; per the CDX 1.6 schema, services are typed by their position in the `services[]` array.) Wire the result into `builder.rs:455-465`'s final JSON-object construction next to `components` and `dependencies`. When the input slice is empty, return `null` OR omit the field entirely (whichever matches the existing pattern at `dependencies.rs` for empty arrays — verify on first read).

- [X] T012 [US1] Extend `mikebom-cli/src/generate/cyclonedx/metadata.rs` to emit the `mikebom:supplement-cdx` document-scope property per FR-012 / Decision 6, following the milestone-113 `mikebom:exclude-path` pattern at lines 126-135. If a `SupplementProvenance` was passed in, build the property as `{ "name": "mikebom:supplement-cdx", "value": format!("{}@sha256:{}", source_path, source_sha256) }` and push to the metadata.properties[] vec. Emission is gated on the provenance being supplied — absent for non-supplement scans (FR-013 byte-identity).

- [X] T013 [US1] Create `mikebom-cli/tests/supplement_cdx_integration.rs` with the test scaffolding (`use std::process::Command;`, `binary_path()`, `run_scan(root, supplement_path)` helper that invokes mikebom with `--supplement-cdx <path>` + the standard `--no-deep-hash` / `--offline` / `MIKEBOM_FIXED_TIMESTAMP` env per milestone-113's exclude_path_integration.rs precedent). Then write the four US1 acceptance-scenario tests: `us1_as1_saas_service_appears_in_services_section`; `us1_as2_vendored_library_carries_declared_metadata`; `us1_as3_empty_supplement_is_byte_identical_to_no_flag`; `us1_as4_no_flag_is_byte_identical_to_pre_feature`. Each test synthesizes a fixture project + supplement file via `tempfile::tempdir()`, scans, parses the emitted CDX, asserts the expected component / service / property presence.

**Checkpoint**: After T008-T013, US1 is fully functional. Vendored libraries + SaaS services appear in the emitted SBOM with `mikebom:source-tier = "declared"` and the document-scope provenance annotation. PR could ship here if review feedback pushes back on US2/US3 diff size.

---

## Phase 4: User Story 2 — Hard/soft conflict resolution (Priority: P2)

**Goal**: When supplement and scanner declare overlapping facts on the SAME PURL, the FR-006/FR-007 partition resolves the conflict per Decision 3. Scanner wins on bytes-derived facts; developer wins on metadata. Every disagreement annotated with `mikebom:assertion-conflict`.

**Independent Test**: Run the US2 integration tests and verify license-override works AND scanner-detected-symbol can't be suppressed.

### Implementation for User Story 2

- [ ] T014 [US2] Implement `mikebom-cli/src/supplement/conflict.rs`'s `resolve_component()` function per contracts/merge-pipeline.md § "Conflict resolution algorithm" + data-model.md § Entity 2. Constants `SCANNER_AUTHORITATIVE_FIELDS: &[&str]` and `DEVELOPER_AUTHORITATIVE_FIELDS: &[&str]` per research.md § Decision 3. Function signature: `pub(crate) fn resolve_component(scanner: ResolvedComponent, supplement: &SupplementComponent) -> (ResolvedComponent, Vec<ConflictRecord>)`. For each field present on BOTH sides where the values differ: classify by field name into the partition; build a `ConflictRecord`; on conflict, the winning side's value is emitted as the primary field; the losing side's value is preserved as a `mikebom:scanner-discovered-{field}` or `mikebom:declared-{field}` annotation (per FR-006 / FR-007). Per the FR-015 safety default, unknown fields → scanner wins. Field-level comparison uses `serde_json::Value::eq` for nested-JSON fields like `hashes[]`, `licenses[]`. **Justification derivation**: the `justification` value on each emitted `ConflictRecord` is derived MECHANICALLY from `ConflictField` via `ConflictField::winner()` per data-model.md Entity 2 invariant 1 (`Scanner` → `"bytes-evident-detection-preserved"`; `Supplement` → `"developer-metadata-override"`). No separate justification-decision logic is needed — the partition IS the justification source of truth. The minimal 2-value enum is intentional (clarification Q3); do not import OpenVEX values.

- [ ] T015 [US2] Extend `mikebom-cli/src/supplement/merge.rs`'s `merge()` to handle the COLLISION case (supplement PURL matches a scanner-discovered PURL via the index from T008). For each collision: call `conflict::resolve_component()`, get the merged `ResolvedComponent` + `Vec<ConflictRecord>`; for each `ConflictRecord` in the vec, call `annotation::stamp_assertion_conflict()` per T006. Replace the scanner-side entry in-place in the components vec (preserve ordering). The post-condition `merged.components.len() >= scanner.len()` continues to hold (collisions replace, don't add; solo entries from T008 append).

- [ ] T016 [US2] Add the US2 acceptance-scenario tests to `mikebom-cli/tests/supplement_cdx_integration.rs`: `us2_as1_declared_license_overrides_empty_scanner_value`; `us2_as2_supplement_cannot_suppress_bytes_evident_detection` (the openssl-symbol-fingerprint negative scenario — synthesizes a fake bytes-evident detection via a hand-built scanner-side ResolvedComponent in a non-integration test path, OR uses a real fixture that triggers binary-fingerprint detection); `us2_as3_scanner_sha256_wins_over_declared_sha256`; `us2_as4_developer_name_wins_scanner_name_annotated`. Each test verifies (a) the winning side's primary field; (b) the losing side's annotation key; (c) the presence + shape of the `mikebom:assertion-conflict` JSON-encoded value.

**Checkpoint**: After T014-T016, the trust-calibration centerpiece is in place. Operators can override licenses/metadata; cannot suppress bytes-evident detection.

---

## Phase 5: User Story 3 — Consumer transparency + SPDX projection + parity catalog (Priority: P3)

**Goal**: Consumers reading the emitted SBOM can enumerate scanner-observed vs declared components AND identify the supplement file. SPDX 2.3 and SPDX 3 outputs carry the supplement's services via the projection per Decision 4. Parity catalog rows added.

**Independent Test**: Run the US3 integration tests + verify SPDX outputs carry projected services + the parity catalog rows are picked up by the format-parity tests.

### Implementation for User Story 3

- [ ] T017 [P] [US3] Extend `mikebom-cli/src/generate/spdx/packages.rs` to project supplement's `services[]` entries onto `packages[]` carrying `mikebom:component-role = "saas-service"` annotation per Decision 4 / spec edge case 3. The C40 pattern is at `generate/spdx/annotations.rs:142` (or thereabouts — verify on first read). Each `SupplementService` becomes one SPDX 2.3 Package with `name`, optional `supplier` (from supplement's `provider.name`), and the C40 saas-service annotation. The provenance annotation from FR-012 emits onto `creationInfo.annotations[]` per the milestone-113 / 118 SPDX 2.3 envelope-annotation pattern.

- [ ] T018 [P] [US3] Extend `mikebom-cli/src/generate/spdx/v3_packages.rs` (or equivalent file — verify on first read) to project supplement's `services[]` entries as SPDX 3.0.1 elements per Decision 4. Where the SPDX 3 schema supports `Service` as a graph-element type, use it directly; otherwise fall back to the SPDX 2.3 projection pattern wrapped in a `Bundle` per the issue body's research summary. The document-scope provenance annotation emits onto the SpdxDocument element's `Annotation` per the milestone-113 / 118 SPDX 3 envelope pattern.

- [ ] T019 [P] [US3] Add three new parity-catalog rows to `mikebom-cli/src/parity/extractors/{cdx,spdx2,spdx3}.rs` + register in `mikebom-cli/src/parity/extractors/mod.rs` per the milestone-118 C64 / milestone-115 C63 precedent. **C65**: extension of existing source-tier extractor (verify the existing C5 extractor handles new value transparently — likely yes, since extractors are value-agnostic); **C66**: new `c66_cdx` / `c66_spdx23` / `c66_spdx3` extractors for `mikebom:supplement-cdx` document-scope annotation; **C67**: new `c67_*` extractors for `mikebom:assertion-conflict` per-component annotation. Each extractor uses the existing `cdx_anno!` / `spdx23_anno!` / `spdx3_anno!` macros from milestone 115/118 precedents.

- [ ] T020 [US3] Add the US3 acceptance-scenario tests + consumer-query verification tests to `mikebom-cli/tests/supplement_cdx_integration.rs`: `us3_as1_consumer_can_distinguish_declared_from_observed` (grep `mikebom:source-tier == "declared"` over the emitted CDX); `us3_as2_metadata_carries_supplement_cdx_provenance` (verify `metadata.properties[].name == "mikebom:supplement-cdx"` + correct value shape); `us3_as3_no_flag_omits_supplement_cdx_property` (byte-identity preserved). Plus SPDX projection tests: `us3_spdx23_services_project_as_saas_service_packages`; `us3_spdx3_services_project_via_bundle_or_service_element`.

**Checkpoint**: After T017-T020, US3 is fully functional. SPDX outputs carry the projected services. Consumer queries work. Parity catalog tests pass.

---

## Phase 6: Polish

- [X] T021 Add the negative-test runbook tests to `mikebom-cli/tests/supplement_cdx_integration.rs` per quickstart.md § "Negative test runbook": `malformed_json_supplement_exits_nonzero`; `missing_supplement_file_exits_nonzero`; `schema_invalid_supplement_exits_nonzero`; `duplicate_purl_in_supplement_exits_nonzero`; `dangling_dependson_in_supplement_exits_nonzero`. Each verifies non-zero exit code + the error message names the supplement path + no SBOM file was created at the output location (per FR-002 / SC-005).

- [X] T022 Run `MIKEBOM_SKIP_DOCKER_INTEGRATION=1 ./scripts/pre-pr.sh` from the repo root. Verify clippy `--workspace --all-targets -D warnings` passes clean AND `cargo +stable test --workspace` passes clean (every suite `ok. N passed; 0 failed`). Per CLAUDE.md this is MANDATORY before opening any PR.

- [X] T023 Update `specs/119-supplement-cdx/tasks.md` (this file) marking T001-T022 as `[X]` completed.

- [ ] T024 Commit per CLAUDE.md commit protocol. Commit title: `feat(supplement): --supplement-cdx hand-authored CDX 1.6 supplement merge (milestone 119, closes #326)`. Commit body summarizes: (a) new `--supplement-cdx <PATH>` flag accepts a hand-authored CDX 1.6 JSON document; (b) hard/soft split — scanner wins on bytes-derived facts (FR-006); developer wins on metadata (FR-007); FR-015 safety property (developer cannot suppress scanner detection of bytes-evident content) preserved by post-condition assertion; (c) three new annotation keys with Principle V audit citations — C65 (source-tier=declared value extension), C66 (supplement-cdx document-scope provenance), C67 (assertion-conflict per-component conflict record); (d) SPDX 2.3 + SPDX 3 service projection per Decision 4; (e) hand-rolled structural validator (no `jsonschema` runtime dep); (f) ~13 integration tests covering US1/US2/US3 acceptance + the 5 negative-test scenarios.

- [ ] T025 Open PR. Title: `feat(supplement): --supplement-cdx hand-authored CDX 1.6 supplement merge (milestone 119, closes #326)`. Body includes: (1) issue #326 link AND the three issue clarification answers (Q1: scan-as wins; Q2: PURL exact-match; Q3: minimal 2-value enum); (2) `## Summary` listing the supplement merge mechanism + the three new annotation keys + SPDX projection + hand-rolled validator; (3) `## Test plan` listing the 11 spec acceptance scenarios + 5 negative tests + 1 safety-property regression test as manually-verified-on-this-PR checklist items; (4) explicit mention that this is **Option C v0.1** per the issue body's recommendation — TOML manifest (Option B) is deferred to a future milestone; (5) the Principle V audit conclusions citing C65/C66/C67 in `docs/reference/sbom-format-mapping.md`.

---

## Dependencies & Execution Order

```text
Phase 1 Setup:           T001 → T002 → T003 (sequential — file-coordination on main.rs + docs)

Phase 2 Foundational:    T004 → T005 → T006 → T007 (sequential — supplement module's parser→error→annotation→types chain)

Phase 3 US1 (P1, MVP):   T008 (merge additive) → T009 (scan_cmd) → T010 (builder.rs:355 wiring) → T011 (services.rs) → T012 (metadata.rs) → T013 (US1 tests)
                                                                            ↓
                                                                       (parallel branches start)

Phase 4 US2 (P2):        T014 (conflict.rs) → T015 (merge collision path) → T016 (US2 tests)

Phase 5 US3 (P3):        T017 [P] (SPDX 2.3) ─┐
                         T018 [P] (SPDX 3)    ├─→ T020 (US3 tests including SPDX projection assertions)
                         T019 [P] (parity)    ─┘

Phase 6 Polish:          T021 (negative-test suite) → T022 (pre-PR gate) → T023 (mark tasks.md) → T024 (commit) → T025 (open PR)
```

**Sequential chains**:
- T001 → T002 → T003 (Setup phase — same-file edits + docs)
- T004 → T005 → T006 → T007 (Foundational — supplement module data-type chain; T006 uses T005's `SupplementError`, T007 uses T006's annotation helpers' field-name conventions)
- T008 → T009 → T010 → T011 → T012 → T013 (US1 — production-code dependency chain: merge function then scan_cmd integration then builder wiring then services emission then metadata provenance then tests)
- T014 → T015 → T016 (US2 — conflict module then merge collision path then tests)
- T021 → T022 → T023 → T024 → T025 (Polish — linear ratchet)

**Parallel branches within US3** (T017/T018/T019): three independent files (SPDX 2.3 packages.rs vs SPDX 3 v3_packages.rs vs parity extractors); all can land concurrently after US2's collision path is done (US3's parity extractors particularly need US2's annotation shapes to be stable; otherwise the extractors test against a moving target).

## Parallel Opportunities

Within US3 Phase 5:

```text
# After US2 lands, three independent files:
T017 [US3] — generate/spdx/packages.rs (SPDX 2.3 services projection)
T018 [US3] — generate/spdx/v3_packages.rs (SPDX 3 services projection)
T019 [US3] — parity/extractors/{cdx,spdx2,spdx3}.rs + mod.rs (C65/C66/C67 catalog rows)
```

No other parallel opportunities — within US1 + US2 the production-code chain is sequential by data flow (merge consumes parser; builder consumes merge; metadata consumes provenance from merge). Within the polish phase the polish tasks are linear.

## Independent Test Criteria

Per the spec's three user stories:

- **US1 (P1) — MVP**: Confirmed by T013's four acceptance tests (SaaS service + vendored library + empty supplement + no-flag byte-identity).
- **US2 (P2)**: Confirmed by T016's four acceptance tests (license override + safety property + sha256 conflict + name conflict).
- **US3 (P3)**: Confirmed by T020's three acceptance tests + the SPDX projection tests, plus T019's parity-catalog `every_catalog_row_has_an_extractor` test.

## Implementation Strategy

**Single-PR ship**: T001 → T025 in one PR. ~700 LoC production + ~300 LoC tests + ~150 LoC docs per plan.md estimate.

**MVP scope**: T001-T013 + T022-T025 (Setup + Foundational + US1 + Polish). After this scope ships, the feature's textbook user story is closed (developers can declare SaaS deps + vendored libraries + license-overrides-without-collisions). US2 + US3 add critical polish but the feature is shippable at the MVP cut-point.

**Cut-points** (per plan.md): if review feedback pushes back on diff size, T017+T018 (SPDX 2.3/3 services projection) defer to a follow-up PR — CDX 1.6 path is the lossless native path; SPDX projection is convenience for non-CDX consumers.

**Format validation**: All 25 tasks above use the required checklist format — `- [ ]` checkbox + sequential ID (T001…T025) + optional [P] marker + [US1]/[US2]/[US3] label for user-story tasks (Setup + Foundational + Polish tasks have no story label) + description with exact file path(s).
