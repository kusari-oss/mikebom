# Specification Quality Checklist: Developer-asserted source-of-truth supplement (v0.1)

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-06-13
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

- The "users" of this feature are: (US1) the developer authoring the supplement file; (US2) the developer relying on the safety property that bytes-evident detection can't be suppressed; (US3) the consumer reading the emitted SBOM and distinguishing scanner-observed from developer-declared. All three are operator-facing stakeholders.
- The spec deliberately commits to **CDX 1.6 only** for the supplement format (Assumption #1). SPDX supplement input is deferred. The CDX-as-input choice is the issue body's Option A; deferring SPDX is consistent with the issue's recommended starting point.
- The hard/soft split (FR-006 vs FR-007) is the trust-calibration centerpiece. The initial fixed field sets are deliberately CONSERVATIVE:
  - Bytes-derived (scanner wins): hashes, cpe, canonical purl, embedded version strings, binary-fingerprint identity
  - Metadata (developer wins): licenses, supplier, copyright, display name, description, externalReferences
  - Fields NOT in either set fall back to "scanner wins" as a safe default per the safety property in FR-015.
- FR-015's safety property — "developer cannot suppress scanner detection of bytes-evident content" — is the spec's hardest commitment. It's worded broadly enough to cover any future mechanism a developer might try (confidence=0, explicit removal, contradicting fact). The implementation MUST verify no edge case in CDX 1.6's `components[]` semantics allows removal-via-assertion.
- FR-014 explicitly defers `metadata.component` override semantics to clarify Q3 (manifest vs `--scan-as` precedence). v0.1 ignores supplement's `metadata.component` and the milestone-110 SelfIdentity resolver continues to own scan-target identity.
- The six open questions from the issue body are tracked in the spec's Assumptions section last bullet and routed to `/speckit-clarify` for resolution. Clarify will likely use 3-4 questions to lock in the highest-impact defaults.
- The single-PR scope (~400 LoC per the issue body's estimate) is realistic. If review feedback pushes back on diff size, FR-009's justification enum is the natural cut-point (a minimal 2-value enum can defer the full enum design to a follow-up).
- Items marked incomplete require spec updates before `/speckit.clarify` or `/speckit.plan`.
