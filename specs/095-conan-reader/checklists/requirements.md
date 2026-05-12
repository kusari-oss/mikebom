# Specification Quality Checklist: Conan source-side reader

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-05-12
**Feature**: [spec.md](../spec.md)

## Content Quality

- [X] No implementation details (languages, frameworks, APIs) — describes WHAT (Conan v2 manifest + lockfile reader emitting `pkg:conan/...` PURLs with dep-graph relationships) and WHY (first source-side C/C++ ecosystem reader; close the source-side gap that the OS-package readers + binary scanner can't cover). Internal implementation choices (INI parser shape, scan_fs integration point) are explicitly deferred to the planning phase.
- [X] Focused on user value and business needs — explicitly frames each gap by operator pain point ("scans a Conan-managed C/C++ source repo and sees declared dependencies", "distinguish direct vs transitive deps"). The first 2 paragraphs walk the reader through WHY Conan over alternatives (CMake, vcpkg, Meson, Bazel) with concrete reasoning.
- [X] Written for non-technical stakeholders — Background explains the "9 existing ecosystems + binary linkage covers deployed; source-side C/C++ is the gap" framing without prescribing parser internals; rationale references industry-standard adoption (Bloomberg, JFrog) without prescribing tools.
- [X] All mandatory sections completed — User Scenarios & Testing (2 stories), Requirements (12 FRs), Success Criteria (7 SCs), Assumptions, Dependencies, Out of Scope.

## Requirement Completeness

- [X] No [NEEDS CLARIFICATION] markers remain — all open choices (PURL qualifier shape for `user/channel`, hand-rolled vs crate INI parser, Conan version-range handling without lockfile, fixture location) are explicitly captured in Assumptions with reasonable-default rationale or marked for Phase 0 research.
- [X] Requirements are testable and unambiguous — FR-001 names the exact PURL shape and component-metadata fields; FR-002/003 specify lockfile-priority semantics; FR-004 references milestone-052's standards-native scope convention; FR-005 names the packageurl-spec; FR-006/007/008 enumerate graceful-fallthrough behaviors; FR-009/010 specify integration; FR-011/012 are scope guards.
- [X] Success criteria are measurable — SC-001 (component-count matches lockfile/manifest), SC-002 (≥4 DEPENDS_ON edges in 5-component fixture), SC-003 (zero regressions), SC-004 (PURL round-trip validation), SC-005 (pre-PR gate clean), SC-006 (7 test scenarios enumerated by letter), SC-007 (milestone-083 audit-harness extension or documented deferral).
- [X] Success criteria are technology-agnostic — outcomes framed as operator-visible behaviors + standards-native conformance metrics (PURL spec, SPDX/CDX dep-graph correctness). The few tool references (cargo test, gh CLI) are project-conventional, not novel.
- [X] All acceptance scenarios are defined — 2 user stories, US1 with 4 Given/When/Then scenarios + US2 with 2.
- [X] Edge cases are identified — 6 edge cases covering `[tool_requires]` build-scope, version-range manifest-only handling, lockfile schema mismatch, `.py`-manifest fallthrough, `user/channel` qualifiers, manifest-vs-lockfile conflict resolution.
- [X] Scope is clearly bounded — 13-item Out of Scope section explicitly deferring `.py` parsing, Conan v1, Conan Center HTTP, every other C/C++ build system (CMake / vcpkg / Meson / Bazel / autoconf), DWARF, eBPF, Conan Center enrichment, Conan profile/options propagation, recipe-revision suffix.
- [X] Dependencies and assumptions identified — both sections populated; dependencies named on milestones 003/052/083/090 + the external packageurl-spec.

## Feature Readiness

- [X] All functional requirements have clear acceptance criteria — FR-001 ↔ US1 AS#1 + SC-001; FR-002 ↔ US1 AS#2 + SC-001; FR-003 ↔ US2 + SC-002; FR-004 ↔ Edge Case `[tool_requires]` + SC-006(d); FR-005 ↔ SC-004; FR-006 ↔ US1 AS#4 + Edge Case `.py`-only + SC-006(f); FR-007 ↔ Edge Case version-range + SC-006(e); FR-008 ↔ US1 AS#3 + SC-006(b); FR-009 (integration), FR-010 (HTTP-fallback scope guard), FR-011 (no new Cargo deps), FR-012 (golden-regen scope) all map to scope-guard SCs.
- [X] User scenarios cover primary flows — US1 (P1, "operator gets components") + US2 (P2, "operator gets relationships"). Both have independent test criteria.
- [X] Feature meets measurable outcomes defined in Success Criteria — every FR maps to ≥1 SC; SC-006 enumerates 7 test scenarios that collectively exercise every FR's behavior.
- [X] No implementation details leak into specification — INI-parser shape, scan_fs integration point, exact code-module name, and PURL-qualifier wire format are all explicitly deferred to the planning phase per the Assumptions section.

## Notes

All 16 checklist items pass. Spec is ready for `/speckit.plan`.

Scope shape (for planning's information): one new ecosystem reader (~milestone 003 sized — small surface area), 1 new fixture set in the sibling fixture repo, 3 new byte-identity goldens (CDX/SPDX2.3/SPDX3 × one fixture), and a small extension to the milestone-083 transitive_parity audit harness. Estimated 1 dev-day plus the fixture-repo PR (which is the only cross-repo coordination needed).
