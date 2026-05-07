---
description: "Task list for milestone 078 — SPDX 3.0.1 conformance pass"
---

# Tasks: SPDX 3.0.1 conformance pass

**Input**: Design documents from `/specs/078-spdx3-conformance/`
**Prerequisites**: plan.md, spec.md (with /speckit.clarify integration), research.md (with real validator output captured during research §1), data-model.md, contracts/spdx3-conformance.md, quickstart.md

**Tests**: Spec references SC-001 through SC-008 plus the 10-test integration matrix in contracts/spdx3-conformance.md. Test tasks are included.

**Organization**: Three user stories. US1 (P1) is the user-reported `createdBy` hotfix; US2 (P1) is the broader validator-driven audit (covers the second SHACL violation surfaced in research §1: `dataLicense`); US3 (P2) is the CI gate. All three ship in one PR per the spec assumptions section.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: parallelizable (different files, no incomplete-task dependencies)
- **[Story]**: US1 / US2 / US3 (user-story phase tasks only)
- File paths are absolute or repository-relative

## Path Conventions

Single workspace; bulk of milestone work is in `mikebom-cli/src/generate/spdx/v3_document.rs` plus a new integration test file, a new shell helper, and a small CI workflow update. No new modules, no new crates. Python tooling lives in `.venv/spdx3-validate/` (gitignored) and is installed by the helper script.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Capture pre-implementation findings; prepare local environment for validator-driven work.

- [ ] T001 Audit three explicit deliverables and capture each in this PR's commit message or a checked-in scratchpad. (a) **Validator binary location + version**: confirm `spdx3-validate` PyPI version pin per research §2 (`0.0.5` as of 2026-05-06; check whether a newer stable version has shipped since and decide whether to bump). Note any environmental gotchas (Python version requirements, OS-specific packaging issues). (b) **Exact graph-construction sites in `v3_document.rs`**: confirm `cyclonedx/builder.rs:163-228`-equivalent sites in v3_document.rs at lines 109-131 (CreationInfo + Tool construction) and lines 374-390ish (SpdxDocument construction with dataLicense). T002 and T003 plug into these. (c) **SPDX 3 model class definitions** for the `simplelicensing_License` element to be added in T003: verify the exact concrete-subclass name (`simplelicensing_License` vs `simplelicensing_LicenseExpression`) and the exact property names (`simplelicensing_simpleLicensingText` vs `simplelicensing_licenseExpression`) by inspecting the SPDX 3 model docs at https://spdx.github.io/spdx-3-model/model/SimpleLicensing/ or the JSON-LD context. Document the chosen names so T003 has unambiguous targets.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Ship the production-side conformance fixes. After this phase, the SPDX 3 emission code path is conformant; story phases verify via tests + CI.

**⚠️ CRITICAL**: All three user-story tracks depend on this phase.

- [ ] T002 Fix the user-reported `createdBy` violation in `mikebom-cli/src/generate/spdx/v3_document.rs` (around lines 109-131). Specifically: (a) construct a new `Organization` element with `type: "Organization"`, `spdxId: format!("{doc_iri}/agent/mikebom-contributors")` per research §4, `creationInfo: CREATION_INFO_ID`, `name: "mikebom contributors"` (matching the CDX `metadata.tools[0].publisher` value at `cyclonedx/metadata.rs:323`); (b) push the Organization element into the graph BEFORE the existing CreationInfo push so the `@graph` order stays deterministic; (c) update the CreationInfo construction so `createdBy: [org_iri]` (was `[tool_iri]`); (d) add a new field `createdUsing: [tool_iri]` referencing the existing Tool element. Per data-model.md "CreationInfo element (MODIFIED)". The Tool element's existing identity (lines 126-131) is preserved verbatim — only the slot referencing it on CreationInfo moves.

- [ ] T003 Fix the `dataLicense` violation surfaced by Phase 0 research §1 (Violation B). In the same `v3_document.rs` file, around the SpdxDocument construction site: (a) construct a new `simplelicensing_License` element (or whichever exact concrete subclass T001(c) confirmed) with `spdxId: "https://spdx.org/licenses/CC0-1.0"` (stable IRI for the SPDX-listed license; not document-scoped per research §6 rationale), `creationInfo: CREATION_INFO_ID`, `simplelicensing_simpleLicensingText: "CC0-1.0"`, `name: "Creative Commons Zero v1.0 Universal"` (or the exact properties from T001(c)); (b) push the License element into the graph; (c) the existing `dataLicense: "https://spdx.org/licenses/CC0-1.0"` field on SpdxDocument is unchanged in VALUE — it now resolves (within `@graph`) to the License element, satisfying the SHACL ClassConstraint. Per data-model.md "simplelicensing_License element (NEW)".

- [ ] T004 [P] Create `scripts/install-spdx3-validate.sh` per research §2: bash script that creates `.venv/spdx3-validate/` if absent, runs `pip install spdx3-validate==0.0.5` (or whichever version T001(a) confirmed), prints the binary path on stdout. Idempotent: re-running is a no-op when the venv already has the pinned version. Add `.venv/spdx3-validate/` to `.gitignore` (verify the existing `.gitignore` doesn't already exclude `.venv/` patterns — most Rust projects do). Make the script executable (`chmod +x`).

---

## Phase 3: User Story 1 — Fix the `createdBy` type-mismatch (Priority: P1)

**Goal**: post-fix emission has `CreationInfo.createdBy[]` referencing a `type: Organization` element (not `Tool`); `CreationInfo.createdUsing[]` references the existing Tool element; both are present and distinct in `@graph`.

**Independent Test**: emit a fresh SPDX 3 SBOM via `mikebom sbom scan --path . --output out.spdx3.json`; assert `createdBy[0]` element has `type: Organization`; assert `createdUsing[0]` element has `type: Tool`; assert the JPEWdev validator reports zero `Core/createdBy` violations.

### Tests for User Story 1

- [ ] T005 [US1] Create `mikebom-cli/tests/spdx3_conformance.rs` with: (a) test-module-level helper `run_validator(fixture_path: &Path) -> ValidationResult` that shells out to `.venv/spdx3-validate/bin/spdx3-validate -j <path>`, captures stdout+stderr, parses the stderr text for `"Violation of type"` markers, returns a struct holding exit code + violation list. Skip behavior: when the binary is absent AND `MIKEBOM_REQUIRE_SPDX3_VALIDATOR` is unset, return a `Skipped` variant + print a clear "validator not found; run scripts/install-spdx3-validate.sh" message; when the env var IS set, return `Err` so the test fails. (b) Test `created_by_references_organization_post_fix` — emits a fresh SPDX 3 SBOM via `mikebom sbom scan` against a tempdir; parses the JSON-LD `@graph`; finds the CreationInfo element; resolves the `createdBy[0]` IRI to an element in `@graph`; asserts that element's `type == "Organization"` and `name == "mikebom contributors"`. (c) Test `created_using_references_tool_post_fix` — same fixture; asserts `createdUsing[0]` IRI resolves to an element with `type == "Tool"`. (d) Tests guarded with `#[cfg_attr(test, allow(clippy::unwrap_used))]` per CLAUDE.md.

**Checkpoint**: US1 passes. The user-reported bug is verified-fixed at the wire-format level by direct JSON-LD assertions; further verification by the validator happens in US2.

---

## Phase 4: User Story 2 — Pass the JPEWdev validator against every fixture (Priority: P1)

**Goal**: every existing SPDX 3 golden fixture (9 files) AND every fresh-emission scan target (≥3) passes `spdx3-validate` with zero violations. Includes the second SHACL violation (`dataLicense`) that research §1 surfaced.

**Independent Test**: loop the validator over every `*.spdx3.json` under `mikebom-cli/tests/fixtures/golden/spdx-3/` and assert exit 0 + zero violation markers in stderr for each.

### Tests for User Story 2

- [ ] T006 [US2] Add to `mikebom-cli/tests/spdx3_conformance.rs`: test `data_license_references_simplelicensing_license_post_fix` — emit fresh SPDX 3 SBOM; resolve `SpdxDocument.dataLicense` IRI to an element in `@graph`; assert that element's `type` is the chosen License subclass per T001(c) (e.g., `simplelicensing_License`); assert the element has the expected `name` and license-text fields; assert the JPEWdev validator reports zero `Core/dataLicense` violations against the same SBOM. Verifies T003's production fix.

- [ ] T007 [US2] Add to `mikebom-cli/tests/spdx3_conformance.rs`: test `every_existing_golden_passes_validator` — loop over all 9 SPDX 3 golden fixtures (`apk`, `cargo`, `deb`, `gem`, `golang`, `maven`, `npm`, `pip`, `rpm`); call `run_validator(fixture_path)` for each; assert validator exit 0 + zero violations for every fixture. The test runs AFTER T009 regenerates the goldens; assertion fails until then — that's the expected TDD ordering.

- [ ] T008 [US2] Add to `mikebom-cli/tests/spdx3_conformance.rs`: three tests for fresh emissions per FR-003: `fresh_source_tier_emission_passes` (mikebom sbom scan against a tempdir), `fresh_image_tier_emission_passes` (mikebom sbom scan --image against a synthetic docker-save tarball — pattern from `triple_format_perf.rs`'s synthetic-image helper), `fresh_synthetic_build_tier_emission_passes` (construct a synthetic `ScanArtifacts` with `GenerationContext::BuildTimeTrace` and pass to per-format builders directly per the milestone-077 pattern, then validate the emitted JSON). Each test calls `run_validator(emitted_path)` and asserts zero violations.

### Implementation for User Story 2

- [ ] T009 [US2] Regenerate all 9 SPDX 3 golden fixtures under `mikebom-cli/tests/fixtures/golden/spdx-3/`. Use the established project regen mechanism (verified against `mikebom-cli/tests/spdx3_regression.rs:109`): `MIKEBOM_UPDATE_SPDX3_GOLDENS=1 cargo test --test spdx3_regression`. Verify the per-file diff includes: (a) one new `Organization` element, (b) one new `simplelicensing_License` element (or chosen concrete subclass), (c) updated `CreationInfo.createdBy` reference, (d) new `CreationInfo.createdUsing` field. The Tool element's content is unchanged. SpdxDocument's `dataLicense` field's VALUE is unchanged (still `https://spdx.org/licenses/CC0-1.0`); what's new is the License element it now resolves to. Confirm CDX 1.6 + SPDX 2.3 goldens stay byte-identical (no incidental regen) by running `cargo test --test cdx_regression` and `cargo test --test spdx_regression` without their respective `MIKEBOM_UPDATE_*_GOLDENS` env vars and asserting all-pass.

**Checkpoint**: US2 passes. Validator clean on all 9 goldens + 3 fresh emissions. The two SHACL violations from Phase 0 §1 are both fixed and verified.

---

## Phase 5: User Story 3 — CI gate (Priority: P2)

**Goal**: a PR that introduces a SPDX 3 conformance violation fails CI before it can merge. The pre-PR gate catches violations locally when the validator is installed; gracefully skips when not installed (preserves local-dev experience for developers without Python configured).

**Independent Test**: open a deliberate-regression PR (or in a scratch branch, revert T002's changes); run CI; verify the conformance check fails with the validator's stderr captured in the workflow log.

### Implementation for User Story 3

- [ ] T010 [US3] Update `.github/workflows/ci.yml`: in the `Lint + test (linux-x86_64)` job, add a step BEFORE `cargo test` that runs `bash scripts/install-spdx3-validate.sh` and a step that sets `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1` in the workflow env. The `Lint + test (macos-latest)` job intentionally does NOT install the validator (per research §5 + plan project-structure decision — Linux is the authoritative gate). Linux+ebpf-tracing lane: same install + env var as the regular Linux lane (consistent gating across the Linux matrix).

- [ ] T011 [US3] Add to `mikebom-cli/tests/spdx3_conformance.rs`: three tests for the CI gate behavior: `validator_absence_graceful_skip_local` (set up with no `.venv/spdx3-validate/` and `MIKEBOM_REQUIRE_SPDX3_VALIDATOR` unset; assert the helper returns the `Skipped` variant + the test PASSES), `validator_absence_hard_fail_ci` (set up with `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1` and the validator binary absent; assert the helper returns `Err` and the test FAILS), `validator_pinned_version_check` (run `.venv/spdx3-validate/bin/spdx3-validate --version`; parse output; assert the version output **contains** the pinned version string from research §2 — substring match, not equality, since validator-side output formatting is not under our control). **Concurrency mitigation (env var toggling)**: `cargo test` runs tests in parallel by default, so the two env-var-toggling tests (`validator_absence_graceful_skip_local` and `validator_absence_hard_fail_ci`) MUST serialize their env-var manipulation. Use a process-wide `static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(())` at the test-module top level and have both tests acquire it as their first action (`let _g = ENV_LOCK.lock().expect("env lock poisoned");`). Avoid adding the `serial_test` crate — it's not in workspace dev-deps and a local mutex is sufficient for two tests. The `validator_pinned_version_check` test does NOT toggle env vars and does NOT need the lock.

**Checkpoint**: US3 passes. CI integration in place. Local pre-PR gate's graceful-skip preserved for non-Python-equipped dev environments.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T012 [P] Update `docs/reference/identifiers.md` (or a new `docs/reference/spdx3-conformance.md` if the existing doc gets too crowded — prefer the existing doc to keep navigation simple): add a brief note in the SPDX 3 wire-mapping section that `CreationInfo.createdBy` references an Organization Agent and `CreationInfo.createdUsing` references the Tool, with a one-line "this corrects the alpha.16-alpha.18 emission shape per milestone 078." Add a recipe linking to `scripts/install-spdx3-validate.sh` for operators who want to verify mikebom-emitted SPDX 3 conformance themselves. Optional: add a brief reference to the SHACL validation rules at https://github.com/spdx/spdx-3-model/blob/develop/serialization/jsonld/validation.md for advanced operators who want to understand the underlying spec. **Property-name backfill (after T001(c) + T003 land)**: T001(c) confirms the exact `simplelicensing_*` concrete subclass + property names against the SPDX 3 model docs. After T003 implements those names, propagate the confirmed values back into the spec artifacts so future readers aren't reading drafts: update `specs/078-spdx3-conformance/data-model.md` ("simplelicensing_License element (NEW)" section), `specs/078-spdx3-conformance/contracts/spdx3-conformance.md` ("simplelicensing_License element (NEW)" section), and `specs/078-spdx3-conformance/quickstart.md` (Recipe 1 jq examples) to use the confirmed names verbatim. Replace the implementation-caveat language ("the exact `simplelicensing_*` property names need verification at implementation time") with the concrete confirmed names + a note "verified against SPDX 3 model docs during T001(c) audit; see commit \<sha\>."

- [ ] T013 Run pre-PR gate per CLAUDE.md: (a) install validator first via `bash scripts/install-spdx3-validate.sh` so the new conformance integration test runs to actual validation (not graceful-skip); (b) export `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1` so the test fails on absent binary; (c) run `./scripts/pre-pr.sh`. Both `cargo +stable clippy --workspace --all-targets -- -D warnings` (zero warnings) AND `cargo +stable test --workspace` (every target reports `0 failed`) must pass. The new `spdx3_conformance` test target must report all-green specifically. Verify the existing milestone-073/074/075/076/077 byte-identity goldens (CDX 1.6 + SPDX 2.3) are unchanged.

- [ ] T014 Manually validate quickstart.md recipes 1-5 end-to-end against a real local build of milestone 078. Specifically: Recipe 1 (jq queries surface the new Organization + License elements as documented); Recipe 2 (validator installs cleanly + runs zero-error); Recipe 3 (fresh emission validates clean); Recipe 4 (Java SPDX library cross-check — at minimum verify the previous "Incompatible type" error message is no longer producible against post-fix output, even if running the actual Java library is out of scope for this manual check); Recipe 5 (graceful-skip on a Python-less dev environment — emulate by deleting `.venv/spdx3-validate/` and running `cargo test --test spdx3_conformance` without `MIKEBOM_REQUIRE_SPDX3_VALIDATOR`). **(d) Deliberate-regression smoke per SC-004**: in a scratch commit, temporarily revert T002's `createdBy` fix (change `createdBy: [org_iri]` back to `createdBy: [tool_iri]` and remove the new Organization element); run `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1 cargo test --test spdx3_conformance`; verify the test fails with the validator's stderr captured in the failure output and the failure message clearly identifies `Core/createdBy` as the violation site. Restore the fix (`git restore` the scratch revert) before opening the PR. Document the captured stderr snippet in the PR description as evidence that the CI gate works as designed.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: T001 has no dependencies; survey task. Should land before T002/T003 begin so T001(c) can confirm the exact `simplelicensing_*` property names T003 uses.
- **Phase 2 (Foundational)**: T002 → T003 sequential (same file, contiguous edits to v3_document.rs). T004 [P] independent (different file). T002 needs T001(b)'s confirmed line numbers; T003 needs T001(c)'s confirmed property names.
- **Phase 3 (US1)**: T005 depends on T002 (production fix in place) + T004 (validator binary available — or graceful-skip handles the case).
- **Phase 4 (US2)**: T006 depends on T003 + T005's helper functions; T007 depends on T009 (golden regen) — assertion fails on un-regenerated goldens; T008 depends on T002 + T003 (production fixes in place); T009 depends on T002 + T003 (production code emits the new shape that gets captured into goldens).
- **Phase 5 (US3)**: T010 + T011 depend on T004 (helper script exists) and T005 (the helper-function infrastructure for the validator-shell-out).
- **Phase 6 (Polish)**: T012 [P] (docs) parallel with everything else; T013 depends on Phases 1-5 complete; T014 depends on T013 (need a clean build to smoke-test).

### Parallel Opportunities

- T004 [P] (install script) parallel with T002/T003 in Phase 2 — different files.
- T012 [P] (docs) parallel with T013/T014 in Phase 6.

### Within Each User Story

- US1 / US2 / US3 share Phase 2 production code. Test surface splits by US.
- The test file `mikebom-cli/tests/spdx3_conformance.rs` is shared across US1/US2/US3 — sequential within file but tests are independent functions.

---

## Parallel Example: Phase 2 (Foundational)

```bash
# Sequential — same file (v3_document.rs):
Task: "T002 Fix createdBy/createdUsing in v3_document.rs"
Task: "T003 Fix dataLicense in v3_document.rs"

# Parallel with the above — different file:
Task: "T004 [P] Create scripts/install-spdx3-validate.sh"
```

---

## Implementation Strategy

### MVP First (Phases 1-3 = US1)

1. Phase 1 setup (T001).
2. Phase 2 foundational (T002, T003, T004) — production fixes for both violations + helper script.
3. Phase 3 US1 (T005) — wire-format assertion tests for the createdBy/createdUsing fix.
4. **STOP and VALIDATE**: at this checkpoint, the user-reported bug is verified-fixed at the wire-format level. The dataLicense fix is also in place but un-tested-by-validator until US2. Both production violations are corrected; the question is just whether the assertions hold.
5. Continue to Phases 4-6.

### Incremental Delivery

Single PR. The Phase 0 §1 validator run shows the fix list is exactly 2 violations — small enough that splitting US1 from US2 would create a transient state where the createdBy fix ships but dataLicense doesn't, leaving CI red until the second fix lands. Better to ship together.

### Parallel Team Strategy

Single developer + reviewer fits the milestone comfortably. Three-way parallelism (T002 dev A, T003 dev B, T004 dev C) is possible but overkill — the milestone is small enough for one person.

---

## Notes

- [P] = different files, no incomplete-task dependencies.
- All three user stories share Phase 2 wiring.
- Per CLAUDE.md: pre-PR gate REQUIRES both `cargo +stable clippy --workspace --all-targets -- -D warnings` clean AND `cargo +stable test --workspace` clean. Cite both in the PR description.
- Tests in `spdx3_conformance.rs` MUST guard their `mod tests` items with `#[cfg_attr(test, allow(clippy::unwrap_used))]` per CLAUDE.md.
- Total estimated tasks: 14. Total estimated effort: 2-3 person-days.
