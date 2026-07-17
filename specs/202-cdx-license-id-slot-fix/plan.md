# Implementation Plan: CDX License Splitter — LicenseRef Escape Hatch

**Branch**: `202-cdx-license-id-slot-fix` | **Date**: 2026-07-17 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/202-cdx-license-id-slot-fix/spec.md`

## Summary

Extend the CDX license splitter's per-token classification at `mikebom-cli/src/generate/cyclonedx/builder.rs::license_entry_for_token` (line 1494) to check each token against the SPDX License List. Tokens on the list emit as `{"license": {"id": <token>, ...}}` (unchanged). Tokens NOT on the list AND NOT already prefixed with `LicenseRef-`/`DocumentRef-` route to `{"license": {"name": "LicenseRef-<sanitized>", ...}}` — matching the SPDX 2.3 emitter's escape-hatch convention landed in m152 for #481.

Membership check via the `spdx` crate's direct `LicenseId::from_name(token) -> Option<&LicenseId>` (no legacy-name normalization side-effects, unlike `SpdxExpression::try_canonical`). Sanitizer reuses the existing `sanitize_to_license_ref_idstring` helper at `rpm_file.rs:778` — plan-phase decision: extract to a shared module (`mikebom-common/src/types/license.rs`) to avoid cross-module leakage from the RPM reader.

Reconnaissance findings (per m199-m201 empirical-verification lesson):
- Bug site pinned at `builder.rs:1494` (verified via grep).
- SPDX-list membership check via `spdx::LicenseId::from_name` avoids the `try_canonical` legacy-name normalization pitfall (`GPL-2.0` → `GPL-2.0-only` shape changes) that could drift existing goldens.
- Sanitizer at `rpm_file.rs:778` (`pub(crate)`) is standalone (no RPM-specific state) — clean extraction candidate.
- Golden regen expected 0 files (verified at implement time; the CDX splitter fires only on compound expressions from OS package readers, and existing goldens contain only canonical SPDX operands).

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–201; no nightly).
**Primary Dependencies**: Existing only — `spdx = "0.10"` (already used by `SpdxExpression::try_canonical` at `mikebom-common/src/types/license.rs`), `serde_json` (already pervasive in the CDX builder). **No new crates.** No subprocess calls. No network access.
**Storage**: N/A — all state in-process per scan; matches every reader milestone since 002.
**Testing**: New integration test scenarios in `mikebom-cli/tests/ipk_license_splitter_m202.rs` that scans a new fixture at `mikebom-cli/tests/fixtures/ipk/license_licenseref_splitter_m202/`. New unit tests in `cyclonedx/builder.rs::tests` (membership check + slot routing) + `mikebom-common/src/types/license.rs::tests` (extracted sanitizer public API).
**Target Platform**: Same as mikebom itself.
**Project Type**: CDX emitter classifier fix + shared-module extraction. ~30 LOC in `builder.rs` (extended `license_entry_for_token`) + ~40 LOC extraction (`sanitize_to_license_ref_idstring` → shared module) + ~50 LOC fixture + ~80 LOC tests. **Roughly 200 LOC total.**
**Performance Goals**: No perf regression beyond FR-006 (`./scripts/pre-pr.sh` wall-clock delta ≤ 5s per SC-006).
**Constraints**: (a) zero new Cargo deps; (b) CDX/SPDX 2.3 sanitizer output MUST be byte-identical for the same input (FR-002 parity); (c) fix MUST NOT reclassify canonical SPDX operands (FR-004 regression guardrail); (d) no new `mikebom:*` annotations — the fix uses the CDX 1.6 spec-blessed `license.name` + `LicenseRef-*` construct.
**Scale/Scope**: 2-3 source files touched (`builder.rs`, `rpm_file.rs` for extraction, `mikebom-common/src/types/license.rs` for the new shared home) + 1 new fixture directory + 1 new integration test file + a handful of new unit tests. Small, focused change.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Pure Rust, Zero C** — ✅ PASS. Pure Rust throughout; no new deps.
- **II. eBPF-Only Observation** — ✅ N/A. User-space emitter bug fix.
- **III. Fail Closed** — ✅ PASS. Membership check is a safe read from a static `spdx::LicenseId` table; no failure mode. Sanitizer returns `Option<String>` — `None` degrades gracefully to omitting the license entry (matches SPDX 2.3 emitter behavior).
- **IV. Type-Driven Correctness** — ✅ PASS. Uses existing `SpdxExpression` newtype + the `spdx::LicenseId` typed handle from the `spdx` crate.
- **V. Specification Compliance** — ✅ PASS. **The fix REMOVES a Constitution Principle V violation** (schema-invalid `license.id` values per CDX 1.6 §5.4.4.1) and routes non-canonical operands to the CDX-spec-blessed `license.name` + `LicenseRef-*` construct (§5.4.4.2). Zero new `mikebom:*` annotations — the wire-format shape is entirely from the CDX 1.6 spec. Standards-native precedence guaranteed.
- **VI. Three-Crate Architecture** — ✅ PASS. All changes stay in `mikebom-cli` + `mikebom-common` (existing crates); the sanitizer extraction lands in `mikebom-common/src/types/license.rs` alongside `SpdxExpression`, keeping the three-crate boundary intact.
- **VII. Test Isolation** — ✅ PASS. New fixture is `tests/fixtures/ipk/...` (in-tree checked, matches existing ipk fixture layout at `tests/fixtures/ipk-files/`).
- **VIII. Completeness** — ✅ PASS. Improves accuracy of an existing emitted license entry; does not omit components.
- **IX. Accuracy** — ✅ PASS. Correcting a schema-invalid emission IS an Accuracy improvement.
- **X. Transparency** — ✅ PASS. Both pre-fix and post-fix wire-format shapes are documented in the CDX 1.6 spec; the fix routes non-canonical operands to the more informative slot (`LicenseRef-*` explicitly signals "not on the SPDX List" vs the pre-fix `license.id` shape that misled consumers into treating the value as SPDX-list-canonical).
- **XI. / XII. Enrichment** — ✅ N/A.
- **Strict Boundary §5 (file-tier)** — ✅ N/A.

**Result**: All principles PASS. No violations. The fix actively RESOLVES a Principle V spec-compliance issue.

**Post-Phase-1 re-check**: N/A — Phase 1 introduces no new entities beyond the existing `license_entry_for_token` extension and the extracted sanitizer's new public location. Constitution gate trivially remains PASS post-design.

## Project Structure

### Documentation (this feature)

```text
specs/202-cdx-license-id-slot-fix/
├── plan.md              # This file
├── spec.md              # Feature specification (input)
├── research.md          # Phase 0 output — 4 mechanical decisions
├── data-model.md        # Phase 1 output — three per-token classification cases
├── quickstart.md        # Phase 1 output — 5 reproducers
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (created by /speckit-tasks)
```

No `contracts/` sub-directory. The fix modifies an existing INTERNAL classifier + uses the CDX 1.6 spec's existing `license.id` / `license.name` shape — no new wire-format contract, no new CLI flag, no new annotation. The CDX 1.6 §5.4.4.1 / §5.4.4.2 spec IS the contract; the fix aligns emission with it.

### Source Code (repository root)

```text
mikebom-cli/src/generate/cyclonedx/
└── builder.rs                                      # MODIFIED — FR-001:
                                                    #   license_entry_for_token (line 1494) gets a
                                                    #   3rd classification branch: token NOT prefixed
                                                    #   AND NOT on SPDX List → LicenseRef-<sanitized>
                                                    #   routed to license.name slot. Membership check
                                                    #   via `spdx::LicenseId::from_name`.

mikebom-common/src/types/license.rs                # MODIFIED — FR-002 sanitizer extraction:
                                                    #   Move `sanitize_to_license_ref_idstring` from
                                                    #   rpm_file.rs into this shared module. Public
                                                    #   API `pub fn sanitize_license_operand_to_ref`
                                                    #   (renamed for clarity). Callers: SPDX 2.3
                                                    #   emitter (via the existing rpm_file.rs path,
                                                    #   now imports from mikebom-common) + new CDX
                                                    #   splitter.

mikebom-cli/src/scan_fs/package_db/
└── rpm_file.rs                                     # MODIFIED — FR-002 sanitizer consumer:
                                                    #   Delete local `sanitize_to_license_ref_idstring`;
                                                    #   replace call sites with the mikebom-common
                                                    #   import. Existing tests either move to the new
                                                    #   home or become thin re-tests via the new API.

mikebom-cli/tests/fixtures/ipk/
└── license_licenseref_splitter_m202/               # NEW — FR-005 fixture:
    └── <synthetic ipk file>                        #   ipk with control containing
                                                    #   `License: GPL-2.0-only & bzip2-1.0.4`
                                                    #   Reproduces the #579 pattern.

mikebom-cli/tests/
└── ipk_license_splitter_m202.rs                    # NEW — FR-005 integration test:
                                                    #   scan_ipk_licenseref_slot_routes_correctly_m202
                                                    #     asserts canonical operand → id slot AND
                                                    #     non-canonical operand → LicenseRef-* name
                                                    #   scan_ipk_parity_between_cdx_and_spdx23_m202
                                                    #     asserts CDX name and SPDX 2.3
                                                    #     hasExtractedLicensingInfos licenseId use
                                                    #     the same sanitized identifier (FR-002)
```

**Structure Decision**: 3 source-file edits + 1 fixture-directory + 1 new integration test file. Sanitizer extraction is additive to `mikebom-common` (existing home for `SpdxExpression`) — natural fit. Zero existing goldens expected to require regen per plan reconnaissance (re-verified at implement time).

## Complexity Tracking

No constitution violations. All principles pass on first check. The fix actively RESOLVES a Principle V compliance issue (schema-invalid `license.id` values).
