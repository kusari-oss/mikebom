# Feature Specification: Extend `mikebom:orphan-reason` vocabulary per milestone 165 audit classifications

**Feature Branch**: `167-orphan-reason-expand`
**Created**: 2026-07-06
**Status**: Draft
**Input**: Milestone-165 audit follow-on (#2 top-3 recommendation). Milestone 061 introduced the C45 `mikebom:orphan-reason` per-component annotation with a single reason code (`unresolved-indirect-require`), Go-only. Milestone 165's `analyze.py` classified real-world orphans on Kubernetes + ArgoCD + podman-desktop into 6 named buckets — but only ONE (`unresolved-indirect-require` for Go) is currently emitted as a first-class SBOM signal. Downstream consumers (vulnerability scanners, license auditors) can't automatically distinguish "honest signal" orphans (stale-go-sum, hoisted-unused, dead-lockfile) from real bugs.

## Motivation

Empirical evidence from milestones 158/164/165 audit rounds:

| Bucket | Ecosystem | Count across 3 targets | Current emitted reason |
|---|---|---|---|
| `stale-go-sum-entry` | Go | 46 (25 K8s + 21 ArgoCD) | `unresolved-indirect-require` (imprecise) |
| `dead-lockfile-entry` | npm | 13 (12 podman-desktop + 1 ArgoCD) | (none emitted) |
| `unresolved-go-module` | Go | 3 (1 K8s + 2 ArgoCD) | `unresolved-indirect-require` (correct — but coarse) |
| `hoisted-unused` | npm | 2 (ArgoCD) | (none emitted) |
| `other-orphan` | Both | 18 (13 K8s + 5 ArgoCD) | (none emitted) |
| `file-tier-unattributed` | File-tier | 0 observed | N/A |

**64 orphans across 3 real-world targets** could carry a per-component reason annotation that consumers could use to filter honest-signal orphans from true bugs. Milestone 167 formalizes m165's external classification as first-class SBOM emission. Per clarification Q1 (2026-07-06), orphan is defined as BFS-unreachable from `metadata.component.purl` — matching the classifier that produced these 64 count above; the `other-orphan` bucket re-classifies into the 4-code vocabulary under this unified definition.

Milestone-165 audit surfaced this as **#2 top-3 follow-on recommendation** with medium scope (~25-30 tasks).

**This is a Constitution Principle X (Transparency) mapping**: current per-component orphan classification is buried in mikebom's audit tooling; making it visible in emitted SBOMs lets downstream consumers act on it automatically.

## Clarifications

### Session 2026-07-06

- Q: How does milestone 167 define "orphan" for emission scope? → A: **BFS-unreachable from `metadata.component.purl`** (Option B). Matches milestone-165 audit's classification methodology; strict superset of milestone-061's zero-incoming definition (every m061-classified orphan remains an orphan; new emissions ADD on multi-version cluster non-leaves like `foo@2.0.0` where a parent-orphan cycles back). No milestone-061 backward-compat break.

## Distinction from milestone 061 (introduction)

- **Milestone 061** (#119): introduced C45 `mikebom:orphan-reason` with a single reason code `unresolved-indirect-require`. Go-only emission at `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs:2091`. Comment at legacy.rs:1432 explicitly plans `proxy-fetch-failed` and `private-module` refinements but never implemented them.
- **Milestone 158** (#128): introduced doc-scope `mikebom:graph-completeness-reason` with an 8-code closed enum for DOCUMENT-level orphan-explanation vocabulary.
- **Milestone 167** (this): EXTENDS the m061 per-component vocabulary from 1 code (Go-only) to 5+ codes across Go + npm + file-tier ecosystems, aligned with m165's audit-tool classifications. Reuses the existing C45 parity-catalog row + wire format. Zero new parity-catalog rows.

## User Scenarios & Testing

### User Story 1 — Downstream vulnerability scanner filters orphaned honest-signal components (Priority: P1)

A vulnerability scanner processes mikebom's emitted SBOM and encounters components with no incoming `dependsOn` edges. Pre-167, the scanner sees a single `mikebom:orphan-reason=unresolved-indirect-require` on some Go orphans and NOTHING on npm orphans — it can't distinguish "the maintainer removed this from package.json but pnpm-lock retained the entry" (honest signal) from "mikebom failed to detect a real edge" (bug). Post-167, each orphan carries a specific reason code (`stale-go-sum-entry`, `dead-lockfile-entry`, `hoisted-unused`, etc.) so the scanner can auto-filter honest-signal orphans and focus on true detection gaps.

**Why this priority**: Constitution Principle X (Transparency) mapping. Milestone 165 identified this as the #2 top-3 follow-on. Concrete downstream ROI: reduces vulnerability-scanner false-positive noise on real polyglot codebases.

**Independent Test**: Regenerate the milestone-165 audit's Kubernetes + ArgoCD SBOMs post-167. Assert:
- Every Go orphan carries `mikebom:orphan-reason` per its m165 audit classification.
- Every npm orphan carries `mikebom:orphan-reason` per its m165 audit classification.
- Post-167 audit report's failure-mode counts remain unchanged (the CLASSIFICATION doesn't change; it just gets emitted as a first-class SBOM signal now).

**Acceptance Scenarios**:

1. **Given** a mikebom scan against a Go monorepo with multi-version co-existence (e.g., `k8s.io/api` v0.30.0 reachable + v0.28.0 orphan), **When** the SBOM is emitted, **Then** the orphan component MUST carry `mikebom:orphan-reason=stale-go-sum-entry` (NOT `unresolved-indirect-require`).

2. **Given** a mikebom scan against a pnpm-monorepo with `dead-lockfile-entry` cases (e.g., milestone-164 podman-desktop pattern where a package version has no live consumer), **When** the SBOM is emitted, **Then** each such orphan MUST carry `mikebom:orphan-reason=dead-lockfile-entry`.

3. **Given** an npm scan where a package is emitted but no importer resolves through it (`hoisted-unused` pattern), **When** the SBOM is emitted, **Then** the orphan MUST carry `mikebom:orphan-reason=hoisted-unused`.

---

### User Story 2 — Milestone-061 semantic preserved (Priority: P2)

Users who currently consume `mikebom:orphan-reason=unresolved-indirect-require` on Go components see this reason code CONTINUE to fire — but only on the specific "no incoming AND no same-name sibling" case that milestone 061's classifier originally targeted. Cases that pre-167 would have been classified as `unresolved-indirect-require` but are actually multi-version co-existence get the more-specific `stale-go-sum-entry` code instead.

**Why this priority**: Backward-compat regression guard. Consumers may already have logic keyed on the m061 vocabulary. Milestone 167 REFINES rather than replaces.

**Independent Test**: For every existing golden fixture that emitted `mikebom:orphan-reason=unresolved-indirect-require` pre-167, verify the same components either (a) continue to carry that reason (the case is still "no incoming, no sibling") OR (b) legitimately get refined to a more specific reason. NO Go component that was NON-orphan pre-167 should acquire an orphan-reason post-167.

**Acceptance Scenarios**:

1. **Given** a Go component that pre-167 carried `mikebom:orphan-reason=unresolved-indirect-require` AND has no same-name sibling in the SBOM, **When** the SBOM is regenerated post-167, **Then** it MUST still carry `mikebom:orphan-reason=unresolved-indirect-require`.

2. **Given** a Go component that pre-167 carried `mikebom:orphan-reason=unresolved-indirect-require` BUT has a same-name reachable sibling (m165's `stale-go-sum-entry` refinement case), **When** post-167 SBOM emitted, **Then** it MUST carry `mikebom:orphan-reason=stale-go-sum-entry` (refined) and MUST NOT carry `unresolved-indirect-require`.

3. **Given** a non-orphan Go component (has incoming edges), **When** post-167 SBOM emitted, **Then** it MUST NOT carry ANY `mikebom:orphan-reason` value.

---

### User Story 3 — Non-orphan components + non-Go/npm ecosystems byte-identical (Priority: P3)

Users scanning repos WITHOUT orphaned Go or npm components see byte-identical SBOM output vs pre-167.

**Why this priority**: Regression guard. Mirrors milestones 158/159/160/161/162/163/164/165/166 dual-side byte-identity precedent.

**Independent Test**: Regenerate all milestone-090 goldens with milestone-167 code. Diff against pre-167. Zero diff bytes on ecosystems where no orphan-reason changes occur.

**Acceptance Scenarios**:

1. **Given** the milestone-090 cargo fixture (Rust, no Go/npm orphans), **When** mikebom scans, **Then** emitted CDX / SPDX 2.3 / SPDX 3 SBOMs MUST be byte-identical to pre-167.

### Edge Cases

- **Orphan with ambiguous classification** (could match multiple reasons): pick the MOST-SPECIFIC reason per a documented priority ordering. Documented in FR-005.

- **Same-name-different-version ambiguity**: `stale-go-sum-entry` requires "same-name reachable sibling exists in this SBOM's `components[]`". If the sibling is present but ALSO orphan → not `stale-go-sum-entry` (falls to next specificity).

- **Component with pre-existing `mikebom:orphan-reason` from user supplement or debug scaffolding**: leave user-supplied values untouched. Milestone 167's emitter runs AFTER user supplements per existing supplement-precedence rules.

- **Cross-ecosystem orphan** (a `pkg:golang/*` component depending on a `pkg:npm/*` component that is BFS-unreachable from `metadata.component.purl`): under the clarified BFS-unreachability definition (2026-07-06), reachability includes cross-ecosystem edges. Whether the npm component is orphan depends solely on whether ANY path from `metadata.component.purl` reaches it. Cross-ecosystem edges (like milestone-165's `pkg:golang/argoproj/argo-cd/v3 → pkg:npm/argo-cd-ui@1.0.0`) are traversed.

- **Component with type `pkg:generic/*` or file-tier**: `file-tier-unattributed` is out of scope for milestone 167 (file-tier orphan-reason emission is a separate future milestone if empirical evidence surfaces it as high-ROI).

- **File-tier components (`pkg:generic/*` from milestone 133)**: OUT OF SCOPE for this milestone. Milestone 165's `analyze.py` category `file-tier-unattributed` had 0 observed occurrences.

## Requirements

### Functional Requirements

- **FR-001**: mikebom MUST emit `mikebom:orphan-reason` per-component annotation on EVERY component that is **orphan** (BFS-unreachable from `metadata.component.purl` following `dependsOn` edges — per clarification 2026-07-06) in the emitted SBOM. Applies to Go (`pkg:golang/*`) + npm (`pkg:npm/*`) ecosystems. Other ecosystems are unchanged (no orphan-reason emission pre-167 or post-167). This definition is a strict superset of milestone-061's zero-incoming criterion: every component m061 already annotates continues to receive an annotation; multi-version cluster non-leaves (like `foo@2.0.0` where a parent-orphan `parent-b` cycles back) are newly captured.

- **FR-002**: The reason-code vocabulary MUST include at least 4 codes:
  - `unresolved-indirect-require` (existing from m061; Go-only; preserved semantic — "no incoming, no same-name sibling")
  - `stale-go-sum-entry` (NEW; Go-only; "no incoming, same-name reachable sibling exists" — multi-version case)
  - `dead-lockfile-entry` (NEW; npm-only; "no incoming, same-name reachable sibling exists" — analogous to m164 podman-desktop pattern)
  - `hoisted-unused` (NEW; npm-only; "no incoming, no same-name reachable sibling" — pnpm/yarn hoisted-package artifact)

- **FR-003**: The reason-code MUST be single-valued (one string per component). No multi-value / union / array shape.

- **FR-004**: The wire shape MUST remain byte-identical to milestone-061's C45 wire shape (parity-catalog C45; single-string value per CDX `properties[]` entry / SPDX 2.3 annotation / SPDX 3 Annotation). Milestone 167 EXTENDS the vocabulary; it does NOT change the wire format.

- **FR-005**: When a component matches multiple reason codes (e.g., a Go component is both "no incoming" AND "has same-name reachable sibling"), the classifier MUST pick the MOST-SPECIFIC reason per this documented priority ordering (from most-specific to least-specific):
  1. `stale-go-sum-entry` (Go multi-version case)
  2. `dead-lockfile-entry` (npm multi-version case)
  3. `hoisted-unused` (npm no-incoming, no-sibling)
  4. `unresolved-indirect-require` (Go no-incoming, no-sibling — preserves m061 semantic)

- **FR-006**: Non-orphan components (any component reachable from `metadata.component.purl` via BFS along `dependsOn` edges — per FR-001 clarification 2026-07-06) MUST NOT carry `mikebom:orphan-reason`. Three-state semantics: absent on non-orphans, present-with-value on orphans.

- **FR-007**: `mikebom:orphan-reason` annotation MUST be emitted in ALL THREE output formats: CDX 1.6 (`properties[]`), SPDX 2.3 (`annotations[]`), SPDX 3.0.1 (`Annotation` element). Uses the existing parity-catalog C45 infrastructure (extractors + `MikebomAnnotationCommentV1` envelope). No new parity-catalog rows.

- **FR-008**: A per-scan info-level tracing log MUST fire summarizing per-ecosystem orphan-reason counts. Field naming: `orphan_reason_stale_go_sum_entry=<N>`, `orphan_reason_dead_lockfile_entry=<N>`, `orphan_reason_hoisted_unused=<N>`, `orphan_reason_unresolved_indirect_require=<N>`. Grep-friendly per the milestone-157-onwards observability convention. Zero counters indicate a healthy scan.

- **FR-009**: Standards-native precedence per Constitution Principle V. The existing C45 parity-catalog row's justification (documented in `docs/reference/sbom-format-mapping.md` per milestone 061) applies — no CDX 1.6 / SPDX 2.3 / SPDX 3.0.1 native field carries "why this component has no incoming edge" semantics. The `mikebom:orphan-reason` annotation is the correct finer-info carrier.

- **FR-010**: No new Cargo dependencies. No new parity-catalog rows (extends existing C45). No new CLI flags. Reuses existing per-ecosystem annotation-emission infrastructure. FR-008's log line is the ONLY new observable output beyond the extended vocabulary values.

### Key Entities

- **`mikebom:orphan-reason` annotation** (C45 — pre-existing): per-component annotation carrying a single-string reason code. Wire format unchanged from m061.

- **Reason code vocabulary** (extended by milestone 167): 4-code open enum after milestone 167 lands. Documented in `docs/reference/sbom-format-mapping.md` C45 row + updated CHANGELOG entry.

- **Classifier priority order** (FR-005): deterministic tie-break when a component matches multiple codes.

- **Ecosystem partition** (Go vs npm): the classifier logic runs PER-ECOSYSTEM. Go-side logic (in `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs` per m061 precedent) emits `stale-go-sum-entry` OR preserves `unresolved-indirect-require`. NEW npm-side classifier emits `dead-lockfile-entry` OR `hoisted-unused`.

## Success Criteria

### Measurable Outcomes

- **SC-001 (K8s Go orphans carry refined reason codes)**: Regenerating the milestone-165 audit's Kubernetes SPDX 3 SBOM post-167 MUST show: (a) at least 20 components carry `mikebom:orphan-reason=stale-go-sum-entry` (matching m165 audit's classification of 25 stale-go-sum-entry orphans on K8s minus classifier-refinement margin); (b) at most 15 components carry `mikebom:orphan-reason=unresolved-indirect-require` (upper bound includes m165 audit's 1 unresolved-go-module baseline + the 13 "other-orphan" bucket that will re-classify as `unresolved-indirect-require` under BFS-unreachable definition per clarification Q1). Total Go orphans emitted MUST be ≥ 35 (matches m165 audit's 39 Go orphans minus small margin for classifier variance).

- **SC-002 (ArgoCD Go + npm orphans carry reason codes)**: Regenerating the milestone-165 audit's ArgoCD SBOM post-167 MUST show at least 21 stale-go-sum-entry + at least 1 dead-lockfile-entry + at least 2 hoisted-unused per m165's audit classification (allowing classifier-refinement margin). Total orphans emitted MUST be ≥ 28 (matches m165 audit's 31 ArgoCD orphans minus small margin). Under the clarified BFS-unreachable definition (Q1), any "other-orphan" bucket members re-classify into one of the 4 codes based on same-name-sibling presence.

- **SC-003 (pre-167 orphan reason semantic preserved for the `unresolved-indirect-require` case)**: For every milestone-090 fixture golden that pre-167 emitted `mikebom:orphan-reason=unresolved-indirect-require` on a component, if that component in the post-167 SBOM has NO same-name reachable sibling, it MUST continue to carry `unresolved-indirect-require`. This is the milestone-061 backward-compat guard.

- **SC-004 (three-state semantics preserved)**: Non-orphan components (any component with ≥1 incoming edge in the emitted SBOM) MUST NOT carry `mikebom:orphan-reason`. Absence-on-non-orphans is preserved.

- **SC-005 (dual-side byte-identity for ecosystems without Go/npm orphans)**: For every milestone-090 fixture that has NO Go or npm components (e.g., cargo, apk, deb, rpm, gem, maven, cmake, bazel, pip, rpm), CDX / SPDX 2.3 / SPDX 3 goldens MUST be byte-identical to pre-167. Zero diff bytes.

- **SC-006 (golden diff limited to orphan-reason additions)**: For milestone-090 fixtures WITH Go or npm orphans, goldens MAY change. Diff MUST be limited to: (a) NEW `mikebom:orphan-reason` values on npm orphan components; (b) REFINED reason codes on Go orphan components (from `unresolved-indirect-require` to `stale-go-sum-entry` where applicable). NO other content changes.

- **SC-007 (pre-PR gate)**: `cargo +stable clippy --workspace --all-targets -- -D warnings` and `cargo +stable test --workspace --no-fail-fast` MUST both pass with zero errors.

- **SC-008 (unit test coverage)**: The classifier logic MUST have at least 8 unit tests covering: (a) Go `stale-go-sum-entry` (same-name sibling exists); (b) Go `unresolved-indirect-require` (no sibling); (c) npm `dead-lockfile-entry` (same-name sibling exists); (d) npm `hoisted-unused` (no sibling); (e) FR-005 priority-order edge case where component matches multiple criteria; (f) FR-006 non-orphan → no annotation; (g) FR-003 single-value shape; (h) FR-002 vocabulary completeness — all 4 codes exercised in at least one test each.

- **SC-009 (integration test)**: A new integration test at `mikebom-cli/tests/orphan_reason_expand.rs` MUST synthesize a multi-ecosystem scan producing at least one orphan of each new reason code and assert (a) each orphan carries the correct reason code per FR-005 priority; (b) non-orphans carry NO reason code; (c) FR-008 tracing log fires with correct per-ecosystem counts.

- **SC-010 (CHANGELOG entry)**: `CHANGELOG.md` MUST document: (a) new vocabulary entries with concrete meaning; (b) FR-005 priority order; (c) empirical impact — pre/post orphan-reason emission counts on the milestone-165 audit targets; (d) consumer jq recipe for filtering honest-signal orphans; (e) note that C45 wire shape is unchanged (byte-identity for existing consumers).

- **SC-011 (empirical closure)**: The impl commit MUST reference this milestone (`implements milestone 167 — audit-surfaced fix from milestone 165`) and include re-measured results: post-167 SBOMs for Kubernetes + ArgoCD show the expected orphan-reason distribution matching m165's audit classification.

## Assumptions

- **The classification logic runs AT EMISSION TIME**: not at scan time. The classifier reads the assembled component + edge graph (after all package-db readers have run + `scan_fs::mod.rs`'s graph resolution has completed) and adds the orphan-reason annotation before serialization.

- **milestone-090 fixtures MAY drift on Go + npm sides**: Fixtures with Go or npm orphans will have their goldens updated with new `mikebom:orphan-reason` annotations. Non-Go/npm fixtures are byte-identical (SC-005).

- **Vocabulary is OPEN-ENUM**: matches the existing C45 catalog entry semantics ("open-enum string per parity/extractors/mod.rs:258 comment"). Future milestones may add new codes without breaking C45 wire format.

- **No production code changes to milestone-158's document-scope `graph-completeness-reason`**: milestone 167 is per-component only. The doc-scope enum vocabulary is unchanged.

- **The `analyze.py` script in milestone-165 will be updated in a POST-167 audit re-run**: to prefer the emitted `mikebom:orphan-reason` values over external classification. Milestone 167 does NOT change `analyze.py`; that's follow-on work.

- **No new Cargo dependencies**: following milestone-158/159/160/161/162/163/164/165/166 precedent. Reuses existing annotation infrastructure.

## Out of Scope

- **File-tier orphan classification** (`file-tier-unattributed`) — 0 observed occurrences in the m165 audit; deferred to a future milestone if empirical evidence surfaces.

- **`staging-repo-artifact` reason code** — Kubernetes-specific pattern (staging/src/ directory). Adding as a first-class code would require a substring-match classifier that's brittle. Deferred to a future milestone if empirical evidence surfaces.

- **`other-orphan` reason code** — the m165 audit's catch-all bucket. Post-167, components that don't match any specific reason code would NOT get an annotation. Consumers filter on the SPECIFIC codes; absent annotation on an orphan is the "unclassified" signal.

- **Retroactive vocabulary migration of pre-167 SBOMs** — this is a scan-time fix; no consumer-side migration tooling is added.

- **Root-cause investigation of milestone-165 audit's `unresolved-go-module` class** — 3 orphans across 2 targets; too small a sample to warrant its own follow-on milestone.

- **Cross-ecosystem orphan reason** (e.g., a `pkg:golang/*` component with only `pkg:npm/*` incoming edges via milestone-165's cross-ecosystem edge detection) — deferred; today's orphan definition is "has ANY incoming edge across the whole SBOM" which correctly handles this case.

- **Automated CI-gating on orphan-reason distribution** — the FR-008 log fires as observability; adding a CI test that FAILS when `orphan_reason_X > N` is a policy decision for a future milestone.

- **Consumer-facing SBOM Consumer Guide update** — the milestone-150/151 consumer guide may benefit from a note about the new vocab codes and how to filter honest-signal orphans, but that's a docs milestone, not part of 167.
