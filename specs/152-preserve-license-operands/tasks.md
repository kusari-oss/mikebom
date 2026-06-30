---
description: "Task list for milestone 152 — preserve license operands (closes issue #481)"
---

# Tasks: Preserve license operands — milestone 152

**Input**: Design documents from `/specs/152-preserve-license-operands/`
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/helper-api.md ✓, quickstart.md ✓

**Tests**: Tests are part of the implementation per the milestone-478 / #475 convention (inline `#[cfg(test)] mod tests` in `rpm_file.rs`). Spec SC-006 enumerates ≥8 new unit tests; data-model.md §7 lists the canonical 12. No separate test-only phase.

**Organization**: Tasks are grouped by user story. The implementation is concentrated in ONE Rust file (`mikebom-cli/src/scan_fs/package_db/rpm_file.rs`) plus one CHANGELOG entry, so `[P]` markers indicate "no semantic dependency" rather than physical-parallel file edits — actual authoring order is serial.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: No semantic dependency on other tasks in the same phase
- **[Story]**: Which user story this task belongs to (US1, US2)
- File paths are exact; the primary deliverable is `mikebom-cli/src/scan_fs/package_db/rpm_file.rs` unless noted

## Path Conventions

- **Rust deliverable**: `mikebom-cli/src/scan_fs/package_db/rpm_file.rs` (the SINGLE Rust file touched per FR-009 + SC-007)
- **CHANGELOG**: `CHANGELOG.md` (one new bullet under `[Unreleased]` / `### Fixed` per SC-008)
- **Authoring artifacts** (not shipped publicly): `specs/152-preserve-license-operands/*.md`
- **Untouched references** (READ-ONLY this milestone): `docs/reference/sbom-format-mapping.md`, `mikebom-common/src/types/license.rs`, every other reader under `mikebom-cli/src/scan_fs/package_db/`, every format emitter under `mikebom-cli/src/generate/`

---

## Phase 1: Setup

**Purpose**: Verify the baseline environment so any later test break is traceable to this milestone's edits.

- [X] T001 Verify pre-PR baseline on `main` is green by running `./scripts/pre-pr.sh` from the repo root before any edits; record the documented `sbomqs_parity::sbomqs_spdx_score_meets_or_beats_cdx_across_ecosystems` env-only failure as the ONLY pre-existing failure permitted in SC-005.
- [X] T002 [P] Open `mikebom-cli/src/scan_fs/package_db/rpm_file.rs` and locate (a) the integration site at lines 469–476 (the `normalized_license` → `try_canonical` → `Vec<SpdxExpression>` pipeline), (b) the existing `normalize_bitbake_license_operators` helper at line 603, and (c) the `#[cfg(test)] mod tests` block starting near line 1060. Record the exact line numbers in working memory for the US1 + US2 phases.
- [X] T003 [P] Confirm `mikebom-common::types::license::SpdxExpression::try_canonical` signature + behavior at `mikebom-common/src/types/license.rs:135` (returns `Result<Self, LicenseError>`; rejects unknown operands; calls `spdx::Expression::parse` internally). Confirm `spdx::license_id`, `spdx::exception_id`, and `spdx::imprecise_license_id` are available at the workspace's pinned `spdx = "0.10"` version per research.md §R1.

---

## Phase 2: Foundational

**Purpose**: Add the internal type + tokenizer scaffolding so US1/US2 implementation can build on a settled foundation.

**⚠️ CRITICAL**: T004 + T005 must complete before US1's main helper (T009) is wired in, because the main helper consumes both the `Token` enum and the `tokenize` function.

- [X] T004 Add the `Token<'a>` enum (`Operand` / `And` / `Or` / `With` / `LParen` / `RParen` / `Whitespace`) to `mikebom-cli/src/scan_fs/package_db/rpm_file.rs` as a private (module-local) enum, near the existing `normalize_bitbake_license_operators` helper at line 603 area. Field shapes per data-model.md §1. Include the doc comment listing the operand-classification cases (LicenseRef-/DocumentRef-prefixed, bare SPDX id, imprecise synonym, genuinely unknown).
- [X] T005 Add the `tokenize(raw: &str) -> Vec<Token<'_>>` helper to `mikebom-cli/src/scan_fs/package_db/rpm_file.rs`, immediately after the `Token` enum. Implementation per research.md §R2: single-pass char walk emitting borrowed-slice operands + standalone parens + whitespace-run-collapsed `Token::Whitespace`; second pass re-classifies operand tokens equal to `"AND"` / `"OR"` / `"WITH"` (case-sensitive) as the corresponding operator variants. ~30 LOC.
- [X] T006 [P] Add 2 small unit tests for `tokenize` in `rpm_file.rs`'s test module — `tokenize_simple_compound` (e.g., `MIT OR Apache-2.0` → expected 5-token Vec) and `tokenize_with_parens_and_whitespace` (e.g., `(MIT OR Apache-2.0) AND PD` → expected 11-token Vec). Used to lock the tokenizer's behavior before US1 builds on it.

**Checkpoint**: Phase 2 complete — `Token` enum + `tokenize` helper + locking unit tests are in place. US1 can build the main helper on top.

---

## Phase 3: User Story 1 — Compliance auditor sees recoverable license info (Priority: P1) 🎯 MVP

**Goal**: After this US ships, scanning the issue-#481 testbed emits non-NOASSERTION `licenseDeclared` for the 5 affected packages (`busybox` × 4 + `liblzma5`) using the SPDX 2.3 `LicenseRef-<sanitized>` escape hatch per FR-001 through FR-005 + FR-013.

**Independent Test**: Per quickstart.md Scenario 1 — manual operator-cadence verification against the issue-#481 testbed at the maintainer's local `yocto-test/` repo. Plus inline unit tests #1, #2, #3, #9, #10 (per data-model.md §7) cover the 5 affected packages' synthetic forms.

### Implementation for User Story 1

- [X] T007 [US1] Add the `sanitize_to_license_ref_idstring(s: &str) -> Option<String>` helper to `mikebom-cli/src/scan_fs/package_db/rpm_file.rs`, immediately after the `tokenize` function. Implementation per research.md §R5: replace each char outside `[a-zA-Z0-9-.]` with `-`, collapse consecutive `-` to single, strip leading/trailing `-`; return `None` when the result is empty. Include the worked-examples table from data-model.md §2 in the doc comment per FR-002.
- [X] T008 [P] [US1] Add unit test #8 `sanitization_worked_examples` to `rpm_file.rs`'s test module covering all 8 (raw, expected) pairs from data-model.md §2's worked-examples table: `GPLv2+` → `GPLv2`, `My License v2` → `My-License-v2`, `(custom)` → `custom`, `LGPL-2.1+` → `LGPL-2.1`, `bzip2-1.0.4` → `bzip2-1.0.4` (unchanged), `PD` → `PD` (unchanged), `!@#$` → `None`, `""` → `None`, `"---"` → `None`. Each assertion uses `assert_eq!(sanitize_to_license_ref_idstring(input), expected)`.
- [X] T009 [US1] Add the main `preserve_known_operands_with_license_ref(raw: &str) -> Option<String>` helper to `mikebom-cli/src/scan_fs/package_db/rpm_file.rs`, immediately after `sanitize_to_license_ref_idstring`. Implementation per research.md §R3 + §R4 + data-model.md §3: tokenize the input; walk tokens with one-token-lookahead state for WITH-exception detection; for each operand classify via R4 step ladder (LicenseRef-/DocumentRef- prefix → bare SPDX `license_id` → `imprecise_license_id` → wrap as `LicenseRef-<sanitize(s)>`); rebuild the string; return `None` on empty input / unrecognized WITH-exception / sanitization-empty cases per FR-008 + FR-013.
- [X] T010 [US1] Wire the `.or_else(|| ...)` fallback into the existing license-processing pipeline at `mikebom-cli/src/scan_fs/package_db/rpm_file.rs:472–476` per data-model.md §5 + contracts/helper-api.md Contract 4. The diff: `try_canonical(l).ok()` becomes `try_canonical(l).ok().or_else(|| preserve_known_operands_with_license_ref(l).and_then(|wrapped| SpdxExpression::try_canonical(&wrapped).ok()))`. Net delta: 4 lines → 9 lines. The first-pass behavior is byte-identical.
- [X] T011 [P] [US1] Add unit test #1 `preserve_busybox_compound` to `rpm_file.rs`'s test module — assert input `"GPLv2 AND bzip2-1.0.4"` (post-milestone-478 normalization shape) feeds through `preserve_known_operands_with_license_ref` + `SpdxExpression::try_canonical` and yields `"GPL-2.0-only AND LicenseRef-bzip2-1.0.4"` (the SC-001 reference output for busybox-family packages). The test has TWO assertions: (a) call `preserve_known_operands_with_license_ref("GPLv2 AND bzip2-1.0.4")` directly, then feed result through `SpdxExpression::try_canonical`, assert canonical output; (b) for the FR-007 pipeline-ordering verification, manually chain `let normalized = normalize_bitbake_license_operators("GPLv2 & bzip2-1.0.4"); assert!(SpdxExpression::try_canonical(&normalized).is_err()); let wrapped = preserve_known_operands_with_license_ref(&normalized).unwrap(); let final = SpdxExpression::try_canonical(&wrapped).unwrap();` inside the test body — DO NOT set up a real RPM-header fixture; the test scope is the helper functions composed manually within the test, not the end-to-end RPM-reader path. (Per analysis remediation A1.)
- [X] T012 [P] [US1] Add unit test #2 `preserve_liblzma5_single_unknown` to `rpm_file.rs`'s test module — assert input `"PD"` feeds through and yields `"LicenseRef-PD"` per FR-004 (single-operand unrecognized → wrapped, not NOASSERTION).
- [X] T013 [P] [US1] Add unit test #3 `preserve_or_operator` to `rpm_file.rs`'s test module — assert input `"GPLv2 OR bzip2-1.0.4"` yields `"GPL-2.0-only OR LicenseRef-bzip2-1.0.4"` (US1 scenario 3 — OR-operator path).
- [X] T014 [P] [US1] Add unit test #9 `with_clause_known_exception_preserved` to `rpm_file.rs`'s test module — assert input `"GPL-2.0-or-later WITH Classpath-exception-2.0"` yields the unchanged canonical form (first-pass `try_canonical` succeeds; LicenseRef fallback never fires per FR-013 happy path).
- [X] T015 [P] [US1] Add unit test #10 `with_clause_unknown_exception_collapses_to_noassertion` to `rpm_file.rs`'s test module — assert input `"GPL-2.0-only WITH UnknownExc AND MIT"` causes `preserve_known_operands_with_license_ref` to return `None` (per FR-013 + Clarifications Q2 — whole compound → NOASSERTION). Verify via `assert!(preserve_known_operands_with_license_ref(input).is_none())`.
- [X] T015a [P] [US1] Add unit test #13 `with_clause_unknown_license_wrapped` to `rpm_file.rs`'s test module — assert input `"UnknownLicense WITH Classpath-exception-2.0"` (unrecognized LEFT side of WITH, recognized exception on RIGHT) yields `"LicenseRef-UnknownLicense WITH Classpath-exception-2.0"` per FR-013 first clause. Verifies the symmetric path to T015 — the LEFT-side license gets wrapped, the recognized exception passes through unchanged. (Per analysis remediation C1 — closes the FR-013 first-clause test gap.)

**Checkpoint**: §3 produces the SC-001 fix. The 5 affected packages now emit non-NOASSERTION licenseDeclared in the issue-#481 testbed.

---

## Phase 4: User Story 2 — Idempotency + happy-path safeguards (Priority: P2)

**Goal**: After this US ships, mikebom is provably safe to ship — the new code path does NOT alter the byte output for any already-canonicalizable expression (SC-002), and feeding milestone-152 output back as input is idempotent (SC-003 + FR-006).

**Independent Test**: Unit tests #4, #5, #6, #7, #11, #12 (per data-model.md §7) cover happy-path no-op, empty input, opaque garbage, idempotency, parens preservation, imprecise-synonym canonicalization. Plus the existing milestone-090 golden test infrastructure provides automated byte-identity regression coverage for happy-path expressions.

### Implementation for User Story 2

- [X] T016 [P] [US2] Add unit test #4 `happy_path_unchanged_for_fully_recognized` to `rpm_file.rs`'s test module — assert input `"GPLv2 AND LGPLv2.1+"` produces `"GPL-2.0-only AND LGPL-2.1-or-later"` AND that the fallback helper `preserve_known_operands_with_license_ref` is NEVER called (first-pass `try_canonical` succeeds via `imprecise_license_id` lookup). Method: verify the end-to-end pipeline output AND confirm via a separate `assert_eq!(preserve_known_operands_with_license_ref("GPL-2.0-only AND LGPL-2.1-or-later"), Some("GPL-2.0-only AND LGPL-2.1-or-later".to_string()))` that the helper itself is a no-op on already-recognized input (FR-003).
- [X] T017 [P] [US2] Add unit test #5 `empty_input_remains_noassertion` to `rpm_file.rs`'s test module — assert that empty (`""`) and whitespace-only (`"   "`) inputs cause `preserve_known_operands_with_license_ref` to return `None`. Verify via `assert!(preserve_known_operands_with_license_ref("").is_none())` and `assert!(preserve_known_operands_with_license_ref("   ").is_none())` per FR-008 + US1 scenario 5.
- [X] T018 [P] [US2] Add unit test #6 `opaque_garbage_remains_noassertion` to `rpm_file.rs`'s test module — assert input `"!@#$"` (a single token whose sanitization strips to empty) causes `preserve_known_operands_with_license_ref` to return `None` per FR-008 + US1 scenario 6.
- [X] T019 [P] [US2] Add unit test #7 `idempotent_on_already_wrapped_input` to `rpm_file.rs`'s test module — assert that feeding milestone-152-shaped output (`"GPL-2.0-only AND LicenseRef-bzip2-1.0.4"`) back into `preserve_known_operands_with_license_ref` yields the same string unchanged (no double-wrapping). Implementation: `let first = preserve_known_operands_with_license_ref("GPLv2 AND bzip2-1.0.4").unwrap(); let second = preserve_known_operands_with_license_ref(&first).unwrap(); assert_eq!(first, second);` per SC-003 + FR-006 + contracts/helper-api.md Contract 5.
- [X] T020 [P] [US2] Add unit test #11 `parens_preserved_through_fallback` to `rpm_file.rs`'s test module — assert input `"(GPLv2 OR LGPLv2.1+) AND PD"` produces `"(GPL-2.0-only OR LGPL-2.1-or-later) AND LicenseRef-PD"` after full pipeline. Verifies the edge case from spec Edge Cases (parenthesized sub-expressions preserved through the LicenseRef-wrapping pass).
- [X] T020a [P] [US2] Add unit test #14 `mixed_precedence_preserved` to `rpm_file.rs`'s test module — assert input `"MIT OR PD AND GPLv2"` (mixed AND/OR with implicit SPDX precedence — `AND` binds tighter than `OR`) yields `"MIT OR LicenseRef-PD AND GPL-2.0-only"` (or whatever canonical form `try_canonical` produces — minor parens-insertion is acceptable as long as the operator structure preserves the SPDX precedence rule). Verifies FR-005 operator-precedence-preservation for IMPLICIT precedence cases (T020 covers EXPLICIT parens). (Per analysis remediation C2 — closes the FR-005 implicit-precedence test gap.)

**Checkpoint**: §4 produces the SC-002 + SC-003 safeguards. The fallback path is idempotent + the happy path is provably untouched.

---

## Phase 5: Polish & cross-cutting

**Purpose**: Add the remaining test for imprecise-synonym handling, add the CHANGELOG entry, run the full audit suite, finalize PR description.

- [X] T021 [P] Add unit test #12 `imprecise_synonym_canonicalized_not_wrapped` to `rpm_file.rs`'s test module — assert input `"GPLv2"` alone (no operator) feeds through the full pipeline and yields `"GPL-2.0-only"` (first-pass `try_canonical` handles the imprecise synonym via the spdx crate's `imprecise_license_id` table; LicenseRef fallback never fires). Verifies research.md §R4 step 3 — bare imprecise synonyms are passed through, not wrapped.
- [X] T022 [P] Add the milestone-152 CHANGELOG.md entry under `## [Unreleased]` / `### Fixed` per research.md §R9 + SC-008 — single bullet documenting the LicenseRef escape-hatch behavior + the replace+collapse+strip sanitization rule + 3 worked examples (`GPLv2+` → `LicenseRef-GPLv2`, `bzip2-1.0.4` → `LicenseRef-bzip2-1.0.4`, `My License v2` → `LicenseRef-My-License-v2`) + the issue #481 reference + the 5/35 Yocto package impact statement. **Precheck (per analysis remediation A3)**: before adding the bullet, run `head -50 CHANGELOG.md` to confirm the `## [Unreleased]` and `### Fixed` headers exist. If either is missing, create the section per Keep-a-Changelog convention before inserting the bullet — don't force the bullet into an incompatible structure.
- [X] T023 Run quickstart.md Scenarios 6 + 7 (SC-007 wire-format guard + CHANGELOG presence check) and confirm: (a) `git diff main --name-only -- docs/ mikebom-common/ mikebom-ebpf/ mikebom-cli/src/generate/` returns empty; (b) `git diff main --name-only -- mikebom-cli/src/scan_fs/` returns ONLY `mikebom-cli/src/scan_fs/package_db/rpm_file.rs`; (c) `sed -n '/^## \[Unreleased\]/,/^## \[v/p' CHANGELOG.md | grep -A1 "LicenseRef"` returns the entry from T022.
- [X] T024 Run `./scripts/pre-pr.sh` per SC-005. Confirm: (a) clippy is clean (no new warnings/errors); (b) every new unit test added in Phase 2/3/4/5 passes; (c) the documented `sbomqs_parity::sbomqs_spdx_score_meets_or_beats_cdx_across_ecosystems` env-only failure remains the only acceptable test failure. Capture the test-result count line for the PR description.
- [X] T025 Run quickstart.md Scenario 4 (SC-006 unit-test count audit) via the `grep -cE` recipe and confirm ≥8 new tests in `rpm_file.rs` matching the milestone-152-added function-name patterns (`preserve_*` / `sanitize_*` / `with_clause_*` / `happy_path_unchanged*` / `empty_input*` / `opaque_garbage*` / `idempotent_on_already_wrapped*` / `imprecise_synonym*` / `parens_preserved*` / `tokenize_*`). Final count from data-model.md §7 + T006: 14 total (2 tokenizer + 12 helper tests) — comfortably above the SC-006 floor.
- [X] T026 Run quickstart.md Scenario 2 (SC-002 automated happy-path regression) — `cargo +stable test --workspace` MUST show every milestone-090 golden test passing. Any failure in `mikebom-cli/tests/transitive_parity_*` is a SC-002 regression (the new code path altered byte output for a happy-path expression); investigate before merge.
- [X] T027 Draft the PR description at `specs/152-preserve-license-operands/pr-description.md` with sections: Summary, Closes #481, Changes (the single Rust file + CHANGELOG), Verification (SC-001 manual instructions for the maintainer + SC-002 / SC-003 / SC-005 / SC-006 / SC-007 / SC-008 automated results from T023–T026), Constitution check (cite plan.md POST-DESIGN re-check), Reviewer instructions (point at quickstart.md Scenario 1 for the Yocto-testbed verification + Scenario 4 for the spot-check format pass). The SC-001 verification stays manual operator-cadence per Assumption 3 + research.md §R8.

**Final checkpoint**: Milestone 152 is shippable. Mark all tasks in this file complete in the PR.

---

## Dependencies & Execution Order

### Phase dependencies

```text
Phase 1 (Setup)
  └─> Phase 2 (Foundational)
        └─> Phase 3 (US1) ┐
                          ├─> Phase 5 (Polish)
            Phase 4 (US2) ┘  (US2 can run in parallel with US1 after Phase 2)
```

### Within-phase parallelism

- **Phase 1**: T001 sequential (gates everything); T002 + T003 [P] in parallel after T001.
- **Phase 2**: T004 → T005 sequential (T005's `tokenize` uses `Token`); T006 [P] after T005.
- **Phase 3 (US1)**: T007 → T009 sequential (`preserve_known_operands_with_license_ref` uses `sanitize_to_license_ref_idstring`); T008 [P] after T007; T010 sequential after T009; T011–T015 [P] in parallel after T010.
- **Phase 4 (US2)**: T016–T020 all [P] after Phase 2's helpers exist (specifically after T009).
- **Phase 5 (Polish)**: T021 + T022 + T023 + T024 + T025 + T026 [P] in parallel; T027 sequential at the end.

### Cross-US independence

US1 (the LicenseRef fallback fix) and US2 (idempotency + happy-path safeguards) touch the SAME file (`rpm_file.rs`) but DIFFERENT regions:
- US1 adds the new helpers + wires the call site + adds tests #1, #2, #3, #9, #10.
- US2 adds tests #4, #5, #6, #7, #11 to the same test module.

Authoring order is serial because both write to the same file; reviewers can verify each US independently by reading the test names + the diff hunks. US2's tests can run after US1's helpers land — there's an implicit dependency on US1's T009 (the main helper must exist before US2 tests can call it).

## Implementation strategy

### MVP scope

The MVP is **US1 alone** (the 5-package fix). Shipping US1 closes issue #481. US2 (idempotency safeguards) is the regression-guard layer — important but not blocking issue closure. In practice, this milestone is small enough (~150 LOC + ~14 tests) that both USs ship together; the prioritization exists so a future bisect can split US1's fix from US2's safeguards if needed.

A degenerate "US1-only" milestone (skipping US2) would still:
- Satisfy SC-001 (the 5-package fix).
- Satisfy SC-005 (pre-PR gate).
- Satisfy SC-006 (US1 contributes 5 of the 8+ required tests).
- Risk SC-002 (no automated idempotency assertion) + SC-003 (same).

Shipping both together is the cheap-and-right call.

### Per-task time estimate

- Phase 1 (T001–T003): ~10 min (pre-PR baseline ~6 min compile + ~3 min test; T002 + T003 are read-only)
- Phase 2 (T004–T006): ~30 min (enum + 30-LOC tokenizer + 2 tests)
- Phase 3 (US1, T007–T015): ~75 min (sanitize helper + 30-LOC main helper + call-site wiring + 5 tests)
- Phase 4 (US2, T016–T020): ~25 min (5 tests, all variations on already-built helpers)
- Phase 5 (Polish, T021–T027): ~45 min (1 test + CHANGELOG + diff checks + pre-PR + PR description)

**Total**: ~3 hours focused authoring + audit, well under a single sitting. Materially smaller than milestone 151 because the code surface is constrained to 1 file + 1 CHANGELOG bullet.
