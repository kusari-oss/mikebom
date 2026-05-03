# Specification Quality Checklist: Cargo source-tree main-module component

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-05-02
**Feature**: [spec.md](../spec.md)

## Content Quality

- [X] No implementation details (languages, frameworks, APIs) — *Spec describes manifest fields (`Cargo.toml`'s `[package]`, `[workspace]`), PURL shapes, and SBOM constructs (CDX `metadata.component`, SPDX `documentDescribes`). These are user-facing data contracts, not implementation. Rust/cargo crate names are referenced as the SUBJECT of scanning, not as implementation choices for mikebom itself.*
- [X] Focused on user value and business needs — *Each user story leads with consumer value: vuln-intersection accuracy, sbomqs scoring, GUAC ingest, dogfood verifiability.*
- [X] Written for non-technical stakeholders — *User stories use plain language; technical edge cases are in their own section.*
- [X] All mandatory sections completed — *User Scenarios & Testing, Requirements, Success Criteria all present and populated.*

## Requirement Completeness

- [X] No [NEEDS CLARIFICATION] markers remain — *Issue #104 itself answers the workspace-handling question; all other points have defensible defaults documented in Assumptions.*
- [X] Requirements are testable and unambiguous — *FR-001 through FR-011 each name a specific observable behavior (PURL emitted, placement slot, edge origin, etc.) verifiable from the SBOM output.*
- [X] Success criteria are measurable — *SC-001 (100% emission), SC-002 (exactly 4 main-modules for mikebom), SC-003 (≤1pp sbomqs delta), SC-004 (byte-identity across 3 hosts), SC-005 (placeholder removed) — all binary or numeric.*
- [X] Success criteria are technology-agnostic — *SC criteria describe SBOM-content properties, not Rust/cargo internals.*
- [X] All acceptance scenarios are defined — *Each user story has 3-4 Given/When/Then scenarios.*
- [X] Edge cases are identified — *10 edge cases enumerated: workspace-only, root-with-workspace, version inheritance, hyphen/underscore, renamed deps, path deps, excluded crates, missing Cargo.lock, pre-release versions, build metadata.*
- [X] Scope is clearly bounded — *Cargo only; npm/pip/maven/gem/Go-binary explicitly out of scope per A8. License detection deferred to #103 follow-up per FR-005 / A3.*
- [X] Dependencies and assumptions identified — *9 assumptions (A1-A9) cover manifest authority, workspace inheritance, license deferral, root+workspace combination, excluded crates, binary-path non-interaction, scope-filter preservation, ecosystem isolation, super-root additivity.*

## Feature Readiness

- [X] All functional requirements have clear acceptance criteria — *FR-001 through FR-011 each map to user-story acceptance scenarios or success criteria.*
- [X] User scenarios cover primary flows — *US1 (project identification, P1) is the primary; US2 (excludable for sbomqs) and US3 (document root) are P2/P3 polish layers.*
- [X] Feature meets measurable outcomes defined in Success Criteria — *SC-001 through SC-005 are achievable by the FRs as stated.*
- [X] No implementation details leak into specification — *No mention of specific Rust crates (e.g., `toml` crate), file paths in `mikebom-cli/src/`, or function names from milestone 053's implementation.*

## Notes

All items pass on first iteration. Spec is ready for `/speckit-plan`.

The spec deliberately mirrors milestone 053's structure (FR-001 through FR-010 numbered identically where the semantics align) so the implementer can carry over the Go playbook with minimal cognitive load. The substantive differences are:

- No version-resolution ladder (manifest authoritative — A1)
- `version.workspace = true` inheritance is the cargo-specific quirk (FR-001 + A2)
- Workspace member handling is explicit (FR-002 + US1 AS#2)
- No binary-path dedup (A6)

Cargo workspaces with `[package]+[workspace]` mixed roots and excluded-but-present crates (the mikebom workspace itself exercises BOTH edge cases) are explicitly handled and exercised by SC-002.
