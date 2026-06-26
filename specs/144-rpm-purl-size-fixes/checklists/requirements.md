# Specification Quality Checklist: RPM reader — fix double-`rpm` PURL namespace + raise size cap

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-06-26
**Feature**: [spec.md](../spec.md)

## Content Quality

- [X] No implementation details (languages, frameworks, APIs)
  - Spec names file paths (`rpm_file.rs`, `rpm.rs`, `os_release.rs`) and a constant (`MAX_RPM_FILE_BYTES`) as anchors for the change — these are scope-bounding references, not implementation prescriptions. Spec does not specify language constructs, control flow, or function signatures.
- [X] Focused on user value and business needs
  - US1 framed around security engineer + downstream vulnerability scanners; US2 around Yocto release engineer + missing debug surface; US3/US4 around operator overrides for edge-case corpora.
- [X] Written for non-technical stakeholders
  - Each user story has a plain-language journey, an explicit "Why this priority" paragraph, and Given/When/Then scenarios that name observable behaviors rather than internal mechanics.
- [X] All mandatory sections completed
  - User Scenarios & Testing, Requirements, Success Criteria, Assumptions all present.

## Requirement Completeness

- [X] No [NEEDS CLARIFICATION] markers remain
  - Zero markers. All design decisions documented in Assumptions.
- [X] Requirements are testable and unambiguous
  - FR-001..FR-011 each name a specific input/output behavior with a measurable assertion. Example: FR-001 specifies "MUST NOT emit PURLs with the literal string `rpm` as the namespace segment under any code path" — directly testable via grep on output.
- [X] Success criteria are measurable
  - SC-001 names exact component count (4587 vs 4584). SC-002 names exact substring assertion. SC-003..SC-007 name specific unit-test assertions. SC-008 names the pre-PR-gate exit code. SC-009 names byte-identity invariance across CI lanes. SC-010 names `--help` output content.
- [X] Success criteria are technology-agnostic (no implementation details)
  - SC-001..SC-010 describe observable outcomes (component counts, substring presence, help text, log absence/presence). The only "technology" references are file formats (CDX, SPDX 2.3, SPDX 3) which are spec-bounded artifact formats, not implementation choices.
- [X] All acceptance scenarios are defined
  - US1 has 4 scenarios; US2 has 3; US3 has 3; US4 has 3. Each is Given/When/Then-shaped.
- [X] Edge cases are identified
  - 8 distinct edge cases covered: conflicting RPMTAG_VENDOR, non-`[a-z0-9-]` ID, empty ID, scan root = `/`, very-large `--max-rpm-bytes`, zero/negative cap, unreadable os-release, size-at-cap boundary.
- [X] Scope is clearly bounded
  - Out of Scope section lists 7 explicit exclusions (per-subtree distro selection, rpmdb cap override, exemption lists, rpmdb-path PURL changes, `mikebom:*` annotations, live `rpm` subprocess, malformed-warning text changes).
- [X] Dependencies and assumptions identified
  - Assumptions section names 9 explicit assumptions including the os-release source choice, no-translation policy, 512MB rationale, scan-wide semantics, rpmdb cap symmetry, low-risk warning-text change, no new deps, Yocto build-output context, and milestone scope bounding.

## Feature Readiness

- [X] All functional requirements have clear acceptance criteria
  - FR-001 ↔ US1 scenarios 1, 3 + SC-002. FR-002 ↔ US1 scenario 2 + SC-005. FR-003 ↔ US4 scenarios + SC-004. FR-004 ↔ US2 scenario 1 + SC-001. FR-005 ↔ US3 scenarios + US2 scenario 3. FR-006 ↔ US2 scenario 2 + SC-007. FR-007 ↔ existing-behavior preservation (no regression test required beyond existing suite). FR-008 ↔ implicit consistency invariant. FR-009 ↔ SC-009. FR-010 ↔ Independent Test in US1. FR-011 ↔ SC-010.
- [X] User scenarios cover primary flows
  - US1 + US2 (both P1) are the two reported defects from yocto-test, both with full Given/When/Then. US3 + US4 are convenience operator-facing knobs with their own scenarios.
- [X] Feature meets measurable outcomes defined in Success Criteria
  - Every SC is achievable purely via the FR set (no SC requires work outside the listed FRs).
- [X] No implementation details leak into specification
  - File paths referenced are scope anchors, not implementation prescriptions. No function signatures, no Rust syntax, no library API calls in the spec body.

## Notes

- All checklist items pass on first iteration.
- The spec deliberately names two file paths (`rpm_file.rs`, `rpm.rs`) and one existing helper (`os_release.rs`) because they bound the scope of the change — a planner reading this spec needs to know that the rpmdb path is in scope (FR-008) but not the rpmdb-path PURL construction (Out of Scope §4). Without those anchors, the planner could over-scope or under-scope.
- Ready for `/speckit-clarify` (if any further questions arise) or directly for `/speckit-plan`.
