# Specification Quality Checklist: Ruby built-in gem edges (milestone 162)

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-07-04
**Feature**: [spec.md](../spec.md)

## Content Quality

- [X] No implementation details (languages, frameworks, APIs) — spec cites `mikebom-cli/src/scan_fs/package_db/gem.rs` as the containing module for the FR-001 allowlist, but names emission behavior + annotation names, not fix mechanics. FR-002/003/004/005 prescribe wire-level behavior rather than exact code changes.
- [X] Focused on user value and business needs — the SBOM consumer (vulnerability scanner, supply-chain auditor) sees the previously-silently-dropped `bundler-audit → bundler` edge. The 0.4% baseline is small numerically but the SILENCE is the load-bearing failure mode.
- [X] Written for non-technical stakeholders — outcomes phrased as "consumer sees the edge" not "fix the graph resolver's dangling-target-drop pass." The Motivation section names the concrete audit shape + the specific dropped edge.
- [X] All mandatory sections completed — User Scenarios + Requirements + Success Criteria + Assumptions + Out of Scope all present.

## Requirement Completeness

- [X] No [NEEDS CLARIFICATION] markers remain — the fix shape is well-known (allowlist + synthetic emission). Two candidate clarifications identified for /speckit-clarify but both are LOW-impact and have reasonable defaults.
- [X] Requirements are testable and unambiguous — each FR names a concrete emitted-artifact behavior; each SC names a verification method (edge presence in dependencies[], annotation presence, versionless PURL check, byte-diff on non-Ruby fixtures, integration test).
- [X] Success criteria are measurable — SC-001 = 100% edge match (from 99.20% pre-162); SC-002 = 1 specific edge present; SC-004 = versionless PURL invariant on synthetic components; SC-006 = 248 non-regression on EXACT-MATCH edges; SC-009 = ≥ 10 unit tests; SC-010 = new integration test; SC-013 = closes #496.
- [X] Success criteria are technology-agnostic — outcomes phrased in SBOM-consumer terms (edge presence, annotation presence, PURL shape). CDX / SPDX 2.3 / SPDX 3 named as inputs to the format-parity check, not as implementation choice.
- [X] All acceptance scenarios are defined — US1 has 4 Given/When/Then scenarios; US2 has 3; US3 has 2. Total: 9 acceptance scenarios.
- [X] Edge cases are identified — 6 edge cases enumerated: no-built-in-references case, multi-source dedup, real-gem-name-collision, allowlist evolution across Ruby versions, no version probe, requirement string parsing.
- [X] Scope is clearly bounded — Out of Scope enumerates 5 explicit exclusions (issue #498 named, Python stdlib, ruby --version probe, runtime version detection, cross-language annotation extension, version-range resolution).
- [X] Dependencies and assumptions identified — 8 Assumptions covering Gemfile.lock GEM/specs ground-truth authority, test-rails empirical benchmark, static allowlist maintainability, versionless PURL spec compliance, no ruby probe, no new Cargo deps, milestone-090 gem fixture may change, SC-001 target achievable (no empirical investigation needed).

## Feature Readiness

- [X] All functional requirements have clear acceptance criteria — FR-001..FR-011 each map to a US1/US2/US3 acceptance scenario, an SC, or both. Zero orphaned FRs.
- [X] User scenarios cover primary flows — US1 (P1 edge fix) is the primary bug fix; US2 (P2 distinguishable synthetic components) is the transparency mechanism; US3 (P3 non-Ruby byte-identity) is the regression guard.
- [X] Feature meets measurable outcomes defined in Success Criteria — SC-001 (edge match %), SC-002 (specific edge present), SC-003 (byte-identity), SC-004 (versionless PURL invariant), SC-005 (no false-positive CVE), SC-006 (no regression on 248 EXACT-MATCH edges), SC-007 (non-Ruby byte-identity), SC-008 (pre-PR gate), SC-009 (unit test floor ≥ 10), SC-010 (integration test), SC-011 (CHANGELOG), SC-012 (parity catalog registration), SC-013 (closes #496).
- [X] No implementation details leak into specification — `mikebom-cli/src/scan_fs/package_db/gem.rs` reference in FR-001 is a code anchor for the allowlist's file location; describes WHERE the semantic lives, not HOW it's implemented.

## Notes

- All 16 checklist items pass on first authoring pass.
- Unlike milestones 160 + 161, this milestone is NOT investigation-heavy — the fix shape is fully known at spec time (allowlist + synthetic emission). The plan phase can jump directly to implementation without a T014-T016 empirical investigation loop.
- The 1 concrete missing edge from milestone-157's audit is the load-bearing SC-002 evidence. Post-162, that edge MUST appear either as a real edge (Option A: synthetic component) or as an annotation (Option B: source-side unresolved-dep-name).
- Building on the milestone-158/159/160/161 vocabulary pattern: 2 per-component annotations (`mikebom:synthetic-built-in`, `mikebom:built-in-requirement`) — both bare-string values matching the milestone-159 C106/C107 shape.
- SC-003 dual-side byte-identity guard verified achievable pre-authoring: 10 non-`gem` milestone-090 fixtures × 3 formats = 30 goldens byte-identical; the `gem` fixture goldens MAY change if its Gemfile.lock references built-in gems.
- Ready for `/speckit-clarify` OR `/speckit-plan`. Two candidate clarifications identified but neither blocking:
  - Q1 candidate: Emit synthetic components (Option A) vs edge-source unresolved-dep-name annotation (Option B)? Recommended default: **Option A** — synthetic component with versionless PURL. Consumers get both the PURL (for vulnerability lookup) AND the transparency annotation (for auditability). Matches Constitution Principle IX (Accuracy — no fake versions) + Principle X (Transparency — synthetic-built-in annotation).
  - Q2 candidate: Should the FR-001 allowlist support future extensibility (e.g., other Ruby-toolchain-managed gems added in Ruby 3.5+), OR should it be pinned to a specific Ruby version's `Gem::default_gems` output? Recommended default: **pin to Ruby 3.4** with an FR-006 maintenance note. Every ~1-year Ruby release cycle needs a milestone-N+X revision to update the list. Matches the milestone-158/160 closed-but-extensible vocab governance precedent.
- Both are LOW-impact questions — could be resolved at `/speckit-clarify` or deferred to plan-time.
