# Specification Quality Checklist: Extend `mikebom:orphan-reason` vocabulary

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-07-06
**Feature**: [spec.md](../spec.md)

## Content Quality

- [X] No implementation details (FRs describe REQUIRED behavior; code paths cited only as pinpoint locators, matching milestone-163/164/166 pattern)
- [X] Focused on user value (downstream vulnerability scanners can filter honest-signal orphans)
- [X] Written for mikebom maintainer + SBOM consumer audience
- [X] All mandatory sections completed

## Requirement Completeness

- [X] No [NEEDS CLARIFICATION] markers remain
- [X] Requirements are testable and unambiguous
- [X] Success criteria are measurable (count thresholds; jq recipes; exit codes)
- [X] Success criteria are technology-agnostic where possible (SC-005 refers to formats by name, not tools)
- [X] All acceptance scenarios are defined (each user story has 2-3 Given/When/Then)
- [X] Edge cases are identified (5 edge cases including ambiguous classification, cross-ecosystem, file-tier scope-out)
- [X] Scope is clearly bounded (7 explicit out-of-scope items)
- [X] Dependencies and assumptions identified (m061 pre-existing infrastructure; m090 fixture drift expected on Go+npm only)

## Feature Readiness

- [X] All functional requirements have clear acceptance criteria — verified via 8 unit tests + 1 integration test + regenerated goldens
- [X] User scenarios cover primary flows (US1 P1 vocab extension, US2 P2 m061 backward-compat, US3 P3 non-Go/npm byte-identity)
- [X] Feature meets measurable outcomes defined in Success Criteria
- [X] No implementation details leak into specification

## Notes

- Empirically-grounded — root cause + vocab codes derived from milestone-165 `analyze.py` classifications.
- KEY DISCOVERY at spec time: C45 `mikebom:orphan-reason` already EXISTS (milestone 061) — this milestone EXTENDS the vocabulary rather than introducing a new annotation. Reuses existing parity-catalog row + wire format. Zero new parity-catalog rows.
- Milestone 167 vocabulary is OPEN-ENUM per existing C45 semantics; adding future codes doesn't break wire format.
- 4 new/refined codes: `stale-go-sum-entry` (Go), `dead-lockfile-entry` (npm), `hoisted-unused` (npm), + preserved `unresolved-indirect-require` (Go, from m061).
- File-tier + staging-repo + other-orphan classes explicitly OUT OF SCOPE for m167 (deferred to future milestones per empirical volume).
