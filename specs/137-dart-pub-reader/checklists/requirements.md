# Specification Quality Checklist: Dart/Flutter pub ecosystem reader

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-06-22
**Feature**: [spec.md](../spec.md)

## Content Quality

- [X] No implementation details (languages, frameworks, APIs) — spec talks about PURL shape, source-discriminator handling, SDK pseudo-deps; the `Dependencies and Constraints` section references mikebom milestones (architectural deps) which is permitted per the template
- [X] Focused on user value and business needs — every user story names the operator outcome (Flutter app dev-machine inventory, source-discriminator distinction, library publisher design-tier scan)
- [X] Written for non-technical stakeholders — terms like `pubspec.lock`, "hosted vs path vs git vs sdk", "SDK pseudo-dep" are explained inline where they appear
- [X] All mandatory sections completed — User Scenarios, Requirements, Success Criteria all present and substantive

## Requirement Completeness

- [X] No [NEEDS CLARIFICATION] markers remain — informed defaults documented in Assumptions for: modern Dart 2.0+ scope, lockfile-first design, no live `dart` invocation, `pkg:generic/` placeholder for non-pub sources, license deferral (parallels milestone-135/136 deferrals)
- [X] Requirements are testable and unambiguous — every FR-001..FR-011 has explicit MUST/MUST NOT semantics and a corresponding SC- gate or acceptance scenario
- [X] Success criteria are measurable — SC-001..SC-007 are all concretely verifiable (component count match, source-type evidence presence, tier annotation, byte-identity preservation, exit-code-0 on corrupted-input fixture, jq-able PURL filter, dev-scope filterability)
- [X] Success criteria are technology-agnostic — SC-006 explicitly requires the standard PURL filter to work without Dart-specific consumer code
- [X] All acceptance scenarios are defined — US1×3, US2×3, US3×2
- [X] Edge cases are identified — 8 entries (mixed source kinds, self-hosted registry, workspace projects, malformed git deps, pre-2.0 format, malformed lockfile, dev vs regular, empty lockfile)
- [X] Scope is clearly bounded — explicit Out-of-Scope section listing 7 deferred concerns (live `dart` invocation, pre-2.0 format, package_config.json, constraint resolution, registry auth, future format migrations, license, package_graph.json)
- [X] Dependencies and assumptions identified — both sections present and concrete

## Feature Readiness

- [X] All functional requirements have clear acceptance criteria — FR-001/FR-006 → US1.3 + SC-004; FR-002/FR-003/FR-004 → US1.1+US1.2 + SC-001; FR-003 source-discrimination clauses → US2.1/2/3 + SC-002; FR-005 → US3 + SC-003; FR-007 → SC-005; FR-008 → SC-007; FR-009 → Edge Case (workspaces); FR-010 → Assumption; FR-011 → US2.3 + SC-002
- [X] User scenarios cover primary flows — P1 Flutter app baseline, P2 source-discriminator distinction, P3 design-tier library scan; each independently testable per the spec template's MVP-slice discipline
- [X] Feature meets measurable outcomes defined in Success Criteria — SC-001..SC-007 each tie back to a user story and a functional requirement
- [X] No implementation details leak into specification — references to mikebom milestones (002, 122) in Dependencies / Assumptions refer to mikebom architectural milestones (template permits this); `serde_yaml` mentioned ONCE in Assumptions as a workspace-dep posture statement, not as an implementation prescription

## Notes

- The `pub` PURL type IS purl-spec-blessed (unlike `brew` in milestone 136 and `yocto` in milestone 128). No follow-up purl-spec extension issue needed.
- Path-deps and SDK-deps deliberately use `pkg:generic/` placeholders rather than `pkg:pub/` — these don't have pub.dev provenance and emitting them under the pub namespace would be incorrect identity assertion. The `mikebom:source-type` evidence provides the discriminator downstream consumers need.
- License extraction is deferred (parallels milestone-135 FR-012 URL, milestone-136 FR-011 license deferrals). Tracked as a cross-reader follow-up — extracting license info from `~/.pub-cache/hosted/pub.dev/<pkg>-<ver>/pubspec.yaml` requires walking the pub-cache, which is separate concern from the lockfile parse.
- US2 has different code path semantics for each source type but doesn't require a separate reader module — all four (hosted/path/git/sdk) discriminate within the same `parse_lockfile_entry` function.
- US3 (design-tier mode without lockfile) is an additive code path layered on top of US1's lockfile parser; the constraint-string preservation pattern matches the existing milestone-122 Kotlin DSL design-tier emission.
