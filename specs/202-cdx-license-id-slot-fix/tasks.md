---

description: "Task list for m202 — CDX License Splitter LicenseRef Escape Hatch"
---

# Tasks: CDX License Splitter — LicenseRef Escape Hatch

**Input**: Design documents from `/specs/202-cdx-license-id-slot-fix/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, quickstart.md (no `contracts/` — internal classifier fix, CDX 1.6 spec §5.4.4.1/§5.4.4.2 IS the wire contract)

**Tests**: Included — every FR requires an executable regression assertion (m194-m201 precedent).

**Organization**: Two P1 stories — US1 (correct slot routing via new SPDX-list check + LicenseRef wrap) and US2 (regression guard for canonical operands + existing LicenseRef tokens). Foundational phase does the sanitizer extraction so BOTH stories can consume from the shared module.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Different file / no dependency on incomplete task
- **[Story]**: US1 (slot routing fix) — US2 is a validation phase, no story label needed

## Path Conventions

- Rust workspace: `mikebom-cli/src/`, `mikebom-cli/tests/`, `mikebom-cli/tests/fixtures/`, `mikebom-common/src/`
- Absolute paths in every task per plan.md structure.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Baseline pins + reconnaissance re-verification. No code changes.

- [X] T001 Verify pre-m202 baseline pre-PR is green by running `./scripts/pre-pr.sh` on branch `202-cdx-license-id-slot-fix` HEAD (post-checkout, pre-implementation) and capture wall-clock time to `/tmp/m202-prepr-baseline.txt` for SC-006 delta measurement later.
- [X] T002 [P] Golden-drift baseline: run `find /Users/mlieberman/Projects/mikebom/mikebom-cli/tests/fixtures/ -name '*.cdx.json' -o -name '*.spdx.json' | xargs grep -lE '"id":\s*"[a-z0-9._-]+-[0-9]+"' | head -20` — record the hit list to `/tmp/m202-golden-audit.txt`. Expected: possibly 0 hits (all cargo/npm/pip goldens use canonical SPDX identifiers). Non-empty hit list means the fix might reclassify existing golden entries — investigate before Phase 3.
- [X] T003 [P] Fixture-shape reconnaissance: `ls /Users/mlieberman/Projects/mikebom/mikebom-cli/tests/fixtures/ipk-files/ && ls /Users/mlieberman/Projects/mikebom/mikebom-cli/tests/fixtures/ipk/ 2>&1` — enumerate existing ipk fixture layouts. Pin the T007 fixture location convention (`fixtures/ipk/license_licenseref_splitter_m202/` per plan). Confirm whether `.ipk` files are committed as binary or generated at test time. Also run `ls /Users/mlieberman/Projects/mikebom/mikebom-cli/tests/rpm_file*.rs 2>&1` — if the file exists, T019 uses the `--test rpm_file...` form; if empty, T019 uses the `--bin mikebom scan_fs::package_db::rpm_file` form. Record the pinned path to `/tmp/m202-rpm-test-path.txt` for T019 to consume.
- [X] T003a [P] Verify the exact `spdx` crate v0.10 public API for SPDX-List membership check before T011's code snippet lands. Run `cargo doc -p spdx --open` (or grep the vendored source: `find ~/.cargo/registry/src -type d -name 'spdx-0.10*' | head -1 | xargs -I{} grep -rnE 'pub fn (license_id|from_name)' {}/src/`) and confirm whether the actual API is `spdx::license_id(&str) -> Option<&LicenseId>` (crate-level) OR `spdx::LicenseId::from_name(&str) -> Option<&LicenseId>` (method) OR both. Also verify the exception-id counterpart: `spdx::exception_id(&str)` vs `spdx::ExceptionId::from_name(&str)`. Update T011's code snippet with the confirmed names before starting Phase 3. Prevents a compile-error mid-implementation.

---

## Phase 2: Foundational — Sanitizer Extraction (Blocking Prerequisites)

**Purpose**: Extract `sanitize_to_license_ref_idstring` from `rpm_file.rs` into a shared `mikebom-common` module. This is a prerequisite for BOTH US1 (new CDX consumer) and US2 (existing SPDX 2.3 consumer must not break).

**⚠️ CRITICAL**: T004 → T005 → T006 must complete + all pre-existing SPDX 2.3 tests must pass byte-identically BEFORE US1 code starts. If any pre-existing test breaks post-T004/T005, halt — extraction has drifted the sanitizer's behavior.

- [X] T004 Add `pub fn sanitize_license_operand_to_ref(s: &str) -> Option<String>` to `mikebom-common/src/types/license.rs` per research R2 + data-model E2. Body byte-identical to `sanitize_to_license_ref_idstring` at `mikebom-cli/src/scan_fs/package_db/rpm_file.rs:778`. Same filter (alphanumeric + `-` + `.`), same `LicenseRef-` prefix, same `None`-on-empty behavior. Copy the existing doc comment. Add module-level `pub use` if the crate's public API surface uses re-exports.
- [X] T005 Modify `mikebom-cli/src/scan_fs/package_db/rpm_file.rs`: DELETE the local `sanitize_to_license_ref_idstring` at line 778 (and its `pub(crate)` visibility). Replace every call site with `mikebom_common::types::license::sanitize_license_operand_to_ref(...)`. Grep-verify zero remaining references to the old name in the crate.
- [X] T006 [P] Move the existing `sanitize_to_license_ref_idstring` unit tests from `mikebom-cli/src/scan_fs/package_db/rpm_file.rs::tests` to `mikebom-common/src/types/license.rs::tests`. Tests: idempotent-guard, alphanumeric-filter, empty-input-returns-None, hyphen-preserved, dot-preserved. Update assertion sites to use the new function name. Keep test-content assertions byte-identical to guarantee behavioral parity.

**Checkpoint**: Foundation ready — sanitizer lives in `mikebom-common`, `rpm_file.rs` imports from it, existing SPDX 2.3 emitter behavior byte-identical. Run `cargo +stable test --manifest-path mikebom-cli/Cargo.toml --test spdx_regression 2>&1 | tail -3` to confirm — expected: `ok. N passed; 0 failed`.

---

## Phase 3: User Story 1 — CDX Splitter Slot Routing Fix (Priority: P1) 🎯 MVP

**Goal**: Extend `license_entry_for_token` at `builder.rs:1494` to check each token against the SPDX License List via `spdx::LicenseId::from_name`. Route non-canonical, non-prefixed operands through the extracted sanitizer to `license.name = "LicenseRef-<sanitized>"`. Closes #579.

**Independent Test**: Synthetic ipk fixture with `License: GPL-2.0-only & bzip2-1.0.4` → scan → assert one `license.id: "GPL-2.0-only"` entry AND one `license.name: "LicenseRef-bzip2-1.0.4"` entry, no `license.id: "bzip2-1.0.4"`.

### Tests for User Story 1

- [X] T007 [P] [US1] Create the m202 fixture at `mikebom-cli/tests/fixtures/ipk/license_licenseref_splitter_m202/`. Author a synthetic `.ipk` binary (ar-format archive containing `debian-binary`, `control.tar.gz` with a `control` file carrying `License: GPL-2.0-only & bzip2-1.0.4`, and `data.tar.gz` with a stub `usr/bin/test` empty file). Commit the resulting `.ipk` alongside a small `README.md` or `regenerate.sh` script documenting reproducibility. Fixture path per plan structure decision.
- [X] T008 [P] [US1] Create integration test file `mikebom-cli/tests/ipk_license_splitter_m202.rs` with a `scan_path` helper following the `scan_npm.rs::scan_path` pattern (shells out to `env!("CARGO_BIN_EXE_mikebom")` with `--offline sbom scan --path <fixture> --format cyclonedx-json --output <tempfile>`).
- [X] T009 [US1] Add integration test `scan_ipk_licenseref_slot_routes_correctly_m202` to `mikebom-cli/tests/ipk_license_splitter_m202.rs`: scan the T007 fixture, assert (a) exactly one component emitted (the synthetic `test` package), (b) `licenses[]` contains an entry with `.license.id == "GPL-2.0-only"`, (c) `licenses[]` contains an entry with `.license.name == "LicenseRef-bzip2-1.0.4"`, (d) NO `licenses[]` entry has `.license.id == "bzip2-1.0.4"` (SC-001 explicit guard).
- [X] T010 [US1] Add integration test `scan_ipk_parity_between_cdx_and_spdx23_m202` to the same file: scan the T007 fixture TWICE (once with `--format cyclonedx-json`, once with `--format spdx-2.3-json`). Extract the `LicenseRef-*` value from each: from CDX via `jq '.components[0].licenses[] | .license.name | select(startswith("LicenseRef-"))'`, from SPDX 2.3 via `jq '.hasExtractedLicensingInfos[]?.licenseId | select(startswith("LicenseRef-"))'`. Assert both values are string-equal (FR-002 parity guarantee).

### Implementation for User Story 1

- [X] T011 [US1] Modify `mikebom-cli/src/generate/cyclonedx/builder.rs::license_entry_for_token` (line 1494 per plan) to add the SPDX-list-membership check as a middle branch between the existing `LicenseRef-`/`DocumentRef-` prefix check and the fallthrough `license.id` slot. Per research R1 + data-model E1:
  ```rust
  fn license_entry_for_token(token: &str, acknowledgement: &str) -> serde_json::Value {
      // Branch 1 (unchanged): already-prefixed tokens route to name slot verbatim.
      if token.starts_with("LicenseRef-") || token.starts_with("DocumentRef-") {
          return json!({
              "license": { "name": token, "acknowledgement": acknowledgement }
          });
      }
      // Branch 2 (Milestone 202 FR-001, closes #579): SPDX-list-canonical
      // identifiers (including exceptions from WITH clauses) route to the
      // spec-canonical `license.id` slot per CDX 1.6 §5.4.4.1.
      let is_spdx_list_id = spdx::license_id(token).is_some()
          || spdx::exception_id(token).is_some();
      if is_spdx_list_id {
          return json!({
              "license": { "id": token, "acknowledgement": acknowledgement }
          });
      }
      // Branch 3 (Milestone 202 FR-001): non-canonical operand → LicenseRef-*
      // escape hatch per CDX 1.6 §5.4.4.2 + parity with SPDX 2.3 emitter
      // per FR-002 (uses the same sanitizer from mikebom-common).
      let sanitized = mikebom_common::types::license::sanitize_license_operand_to_ref(token);
      match sanitized {
          Some(ref_id) => json!({
              "license": { "name": ref_id, "acknowledgement": acknowledgement }
          }),
          // Defensive fallback for all-stripped inputs (empty after
          // sanitization) — matches SPDX 2.3 emitter's None-degrade
          // behavior. Emits nothing usable; caller-side filter drops
          // this via the existing empty-license-entry filter.
          None => serde_json::Value::Null,
      }
  }
  ```
  Adjust caller (`license_entries_from_expression` or equivalent) to filter out `Null` values from the resulting array. Verify the exact spdx-crate API for `license_id` / `exception_id` via `cargo doc --open` on the `spdx` crate if the function names differ from the plan's assumption.
- [X] T012 [P] [US1] Add unit test `license_entry_for_token_routes_canonical_to_id_slot_m202` in `mikebom-cli/src/generate/cyclonedx/builder.rs::tests`: assert `license_entry_for_token("MIT", "declared")` returns `{"license": {"id": "MIT", "acknowledgement": "declared"}}`. Cross-check with `Apache-2.0`, `GPL-3.0-only`, `GPL-2.0-only-with-classpath-exception` (all canonical SPDX-list identifiers).
- [X] T013 [P] [US1] Add unit test `license_entry_for_token_routes_non_canonical_to_licenseref_name_slot_m202` in the same tests module: assert `license_entry_for_token("bzip2-1.0.4", "declared")` returns `{"license": {"name": "LicenseRef-bzip2-1.0.4", "acknowledgement": "declared"}}`. Cross-check with `custom-license`, `made-up-name-2.0`.
- [X] T014 [P] [US1] Add unit test `license_entry_for_token_preserves_pre_formed_licenseref_verbatim_m202` in the same tests module: assert `license_entry_for_token("LicenseRef-user-supplied", "declared")` returns `{"license": {"name": "LicenseRef-user-supplied", ...}}` — NO double-prefixing (per data-model E1 Branch 1 preservation).
- [X] T015 [P] [US1] Add unit test `license_entry_for_token_uses_shared_sanitizer_m202` in the same tests module: verify that the CDX splitter's sanitizer output matches the SPDX 2.3 side by calling `mikebom_common::types::license::sanitize_license_operand_to_ref("bzip2 with spaces & specials!")` and asserting the same output the CDX Branch 3 would produce for that input. FR-002 structural parity.

**Checkpoint**: US1 fully functional. Vaultwarden-style verification via T024 in Phase 6 (manual quickstart Reproducer 2 against hand-built ipk).

---

## Phase 4: User Story 2 — Regression Guard for Canonical + Existing-LicenseRef Cases (Priority: P1)

**Goal**: Verify that no existing CDX golden's license entries drift + all existing test suites pass byte-identically.

**Independent Test**: Every existing CDX regression test + SPDX regression test + public-corpus golden test passes without modification.

### Validation Tasks for User Story 2

- [X] T016 [P] [US2] Run `cargo +stable test --manifest-path mikebom-cli/Cargo.toml --test cdx_regression --no-fail-fast 2>&1 | tail -3`. Expected: `ok. N passed; 0 failed`. FR-004 verification: all existing per-ecosystem CDX goldens (cargo, npm, pip, maven, gem, etc.) unchanged.
- [X] T017 [P] [US2] Run `cargo +stable test --manifest-path mikebom-cli/Cargo.toml --test spdx_regression --no-fail-fast 2>&1 | tail -3`. Expected: `ok. N passed; 0 failed`. FR-004 verification: SPDX 2.3 emitter behavior unchanged post-sanitizer-extraction (T005 regression guard).
- [X] T018 [P] [US2] Run `cargo +stable test --manifest-path mikebom-cli/Cargo.toml --test spdx3_regression --no-fail-fast 2>&1 | tail -3`. Expected: `ok. N passed; 0 failed`. SPDX 3 emitter behavior unchanged.
- [X] T019 [P] [US2] Run the RPM reader's tests to verify T005 (rpm_file.rs → mikebom-common import migration) preserves license-normalization behavior byte-identically. Use the path pinned at T003 (`/tmp/m202-rpm-test-path.txt`): if `mikebom-cli/tests/rpm_file*.rs` exists, run `cargo +stable test --manifest-path mikebom-cli/Cargo.toml --test <that-file> --no-fail-fast 2>&1 | tail -3`; else run `cargo +stable test --manifest-path mikebom-cli/Cargo.toml --bin mikebom scan_fs::package_db::rpm_file --no-fail-fast 2>&1 | tail -3`. Expected: `ok. N passed; 0 failed`. Also covers the extracted sanitizer's tests at their new home per T006.
- [X] T020 [US2] Grep post-fix: `git diff --stat mikebom-cli/tests/fixtures/`. Expected: ONLY the new `ipk/license_licenseref_splitter_m202/` fixture (per T007). If ANY existing golden JSON drifts, investigate immediately — that would be an unexpected FR-004 violation. Document outcome in PR body.

**Checkpoint**: Both US1 (new-behavior guarantee) and US2 (no-regression guarantee) validated.

---

## Phase 5: Cross-Cutting Golden Drift Re-Verification

**Purpose**: Follow the m199-m201 empirical-verification lesson. Research R3 predicted 0 golden drifts, but that's an unverified claim until implement time. Explicitly re-audit post-implementation.

- [X] T021 [P] Re-run T002 audit post-implementation: `git diff --stat mikebom-cli/tests/fixtures/`. The ONLY changes should be the new `ipk/license_licenseref_splitter_m202/` fixture files (T007). Any existing golden JSON in the diff means unexpected drift.
- [X] T022 If T021 reveals drift on `mikebom-cli/tests/fixtures/public_corpus/*/{cdx,spdx-2.3,spdx-3}.json`, plan a follow-up regen PR via `gh workflow run public-corpus.yml --field branch=main --field regen_goldens=true` after m202 merges (m196/m199/m200/m201 pattern). If drift is on any OTHER non-public-corpus golden, HALT — that would be a genuine FR-004 regression requiring code investigation.

---

## Phase 6: Polish & Verification

- [X] T023 Run `./scripts/pre-pr.sh` post-implementation. Capture wall-clock time; compute delta vs T001 baseline; MUST be ≤ 5 seconds per SC-006. Enumerate every `^---- .+ stdout ----` line if any test binary fails (per feedback_prepr_gate_bails_on_first_failure memory).
- [X] T024 [P] Manually execute quickstart.md Reproducer 2 (hand-built ipk end-to-end) against a scratch `/tmp/m202-repro/` directory. Confirm `jq '.components[] | .licenses' /tmp/m202-out.cdx.json` returns `[{"license": {"id": "GPL-2.0-only", ...}}, {"license": {"name": "LicenseRef-bzip2-1.0.4", ...}}]` post-fix (was `[{"license": {"id": "GPL-2.0-only", ...}}, {"license": {"id": "bzip2-1.0.4", ...}}]` pre-fix). SC-001/SC-002/SC-003 verified.
- [X] T025 [P] Manually execute quickstart.md Reproducer 4 (FR-002 CDX/SPDX 2.3 parity script) against the same `/tmp/m202-ipks/` directory. Confirm the `PARITY ✓` outcome — CDX `license.name` string-equal to SPDX 2.3 `hasExtractedLicensingInfos[].licenseId` for the LicenseRef entry.
- [X] T026 Draft PR body with `Closes #579` per SC-007. Include: (a) 1-paragraph summary of the 3-branch classifier + sanitizer extraction, (b) research R3 empirical-verification outcome (T021 result), (c) code-diff LOC + files touched (~200 LOC, 3 source + 1 fixture + 1 test), (d) test coverage summary (2 new integration tests + 4 new unit tests), (e) hand-built ipk before/after jq output showing the slot rotation.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies. T001 sequential; T002 + T003 parallel.
- **Phase 2 (Foundational sanitizer extraction)**: Depends on Phase 1. T004 → T005 → T006 sequential (each layers on the previous). Post-T006, verify SPDX 2.3 tests still pass before proceeding.
- **Phase 3 (US1)**: Depends on Phase 2 completion. Fixture (T007) + integration-test-file scaffold (T008) parallel; T009 + T010 sequential in same file; code edit (T011) after tests (TDD); T012-T015 unit tests parallel after T011.
- **Phase 4 (US2)**: Depends on Phase 3 T011 completion.
- **Phase 5 (Golden Drift Re-Verification)**: Depends on Phase 4 completion.
- **Phase 6 (Polish)**: Depends on Phase 5 completion.

### Within US1

- Fixture (T007) [P] + integration-test-file scaffold (T008) [P] → integration tests (T009 → T010 sequential same file) → code edit (T011) → unit tests (T012 + T013 + T014 + T015 parallel).

### Within US2

- T016 + T017 + T018 + T019 all parallel (4 independent test binaries).
- T020 sequential (needs post-code filesystem state).

### Parallel Opportunities

- **Phase 1**: T002 + T003 parallel.
- **Phase 2**: T004 → T005 → T006 sequential; T006 may run parallel with the rpm_file.rs post-T005 test-suite verification.
- **Phase 3**: T007 + T008 parallel (different files). T012 + T013 + T014 + T015 parallel unit tests.
- **Phase 4**: T016-T019 all parallel (4 independent test-binary invocations).
- **Phase 6**: T024 + T025 parallel (independent verification steps).

---

## Parallel Example: Phase 4 Regression Guard

```bash
# Kick off all 4 US2 regression checks in parallel:
Task: "cargo test cdx_regression"
Task: "cargo test spdx_regression"
Task: "cargo test spdx3_regression"
Task: "cargo test rpm_file::tests"
```

---

## Implementation Strategy

### MVP First (US1 Only)

1. Phase 1 (Setup) → baselines captured.
2. Phase 2 (Foundational sanitizer extraction) → shared sanitizer available.
3. Phase 3 (US1) → fixture + tests + code edit + unit tests.
4. STOP + VALIDATE: T009/T010 pass, T012-T015 pass, quickstart Reproducer 2 shows correct slot routing.
5. Optional stopping point — US1 alone closes #579. US2 is a regression guarantee, not a separate deliverable.

### Full-Bundle Delivery (Preferred)

1. Phases 1 → 2 → 3 → 4 → 5 → 6 in order.
2. Single PR closes #579 with US1 + US2 validation in one merge.

---

## Notes

- [P] tasks = different files, no cross-dependency on incomplete task.
- Every FR has ≥1 executable test: FR-001 via T011 code + T012/T013/T014 unit tests; FR-002 via T010 integration + T015 unit; FR-003 via T014 unit; FR-004 via T016-T020 regression guard; FR-005 via T007 fixture + T009 integration; FR-006 via T023 wall-clock; FR-007 no explicit test (no new annotation to filter; the fix uses spec-native constructs).
- Empirical R3 claim (0 pre-existing goldens require regen) is re-verified at implement time via T020 + T021.
- Zero new Cargo dependencies.
- Zero new user-facing `mikebom:*` annotations (the fix uses spec-blessed `license.name` + `LicenseRef-*` construct).
- Total ~200 LOC across 3 source files + 1 fixture + 1 test file (per plan.md scope estimate).
