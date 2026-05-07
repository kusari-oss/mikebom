# Specification Quality Checklist: SPDX 3.0.1 conformance pass

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-05-06
**Feature**: [spec.md](../spec.md)

## Content Quality

- [X] No implementation details (languages, frameworks, APIs)
- [X] Focused on user value and business needs
- [X] Written for non-technical stakeholders
- [X] All mandatory sections completed

## Requirement Completeness

- [X] No [NEEDS CLARIFICATION] markers remain
- [X] Requirements are testable and unambiguous
- [X] Success criteria are measurable
- [X] Success criteria are technology-agnostic (no implementation details)
- [X] All acceptance scenarios are defined
- [X] Edge cases are identified
- [X] Scope is clearly bounded
- [X] Dependencies and assumptions identified

## Feature Readiness

- [X] All functional requirements have clear acceptance criteria
- [X] User scenarios cover primary flows
- [X] Feature meets measurable outcomes defined in Success Criteria
- [X] No implementation details leak into specification

## Notes

- Three user stories: US1 (P1) is the user-reported `createdBy` hotfix; US2 (P1) is the broader validator-driven audit; US3 (P2) is the CI gate that prevents regression. All three ship in one PR per the spec assumptions section.
- Bounded scope: only SPDX 3.0.1 emission. CDX 1.6 + SPDX 2.3 stay byte-identical (FR-007, SC-006). Broader SPDX 3 features (Build profile, AI/Dataset profile) are explicitly out — separate milestones if demand emerges.
- The exact list of conformance fixes beyond the user-reported `createdBy` bug is not enumerated in the spec on purpose — the validator will surface them at research/plan/implement time. Visual inspection of fixtures suggests likely candidates (`dataLicense` license-expression shape, `externalIdentifierType` controlled vocabulary, `software_*` property naming, `@id` vs `spdxId` consistency, blank-node CreationInfo identifier) — these are documented in the Edge Cases section.
- All decisions inherit project conventions where applicable: pre-PR gate (CLAUDE.md), `#[cfg_attr(test, allow(clippy::unwrap_used))]` guard for tests with `.unwrap()`, no new `mikebom:*` annotations (Constitution Principle V — all changes are at the standards-native field level inside SPDX 3 emission).
- Validator version pinning per FR-008 is the documented policy for handling validator updates that might produce false positives.
- All items pass on first iteration; spec is ready for `/speckit.clarify` or `/speckit.plan`.
- Recommend `/speckit.clarify` for one likely-worth-pinning decision: the choice between `SoftwareAgent` and `Organization` for the `createdBy` Agent element (both are valid Agent subclasses per SPDX 3 spec; the choice has minor downstream-tool ergonomic implications). Phase 0 research can absorb this if `/speckit.clarify` is skipped.
- **Post-`/speckit.clarify` integration (2026-05-06)**: applied one clarification — the `createdBy` Agent class is `Organization` with `name: "mikebom contributors"` (matching the publisher identity already in CDX `metadata.tools[0].publisher`). This is the conventional SBOM-tooling reading: `createdBy` for the human/legal entity responsible (Person or Organization), `createdUsing` for the Tool. `SoftwareAgent` is reserved per the SPDX 3 spec for autonomous software agents (AI assistants etc.), not for SBOM-generation tool runtimes. Note: the initial recommendation was `SoftwareAgent` and was course-corrected to `Organization` after the user pointed out the conventional reading; the spec now consistently uses `Organization` in FR-001, SC-003, US1 acceptance scenarios, and the Overview deliverables list.
