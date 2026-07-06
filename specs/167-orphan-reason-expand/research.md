# Research: milestone 167 — extend `mikebom:orphan-reason` vocabulary

**Date**: 2026-07-06
**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

Phase 0 research. Empirically-grounded from milestones 158/164/165; documents the design decisions rather than exploring unknowns.

## R1 — Pre-existing vocabulary discovered during spec authoring

**Decision**: The C45 vocabulary already has **2 codes** in production, not 1 as milestone-165 audit assumed:

- `unresolved-indirect-require` — Go component with zero incoming edges AND no backfill applied (`legacy.rs:2091`).
- `flat-attached-fallback` — Go component with incoming edge from main-module via synthesized backfill (m053-era work; `legacy.rs:2118`). Semantic: "incoming edge attribution unknown/synthesized" — NOT strictly orphan.

Milestone 167 preserves BOTH existing codes AND adds 3 new codes. Total post-167 vocabulary = **5 codes**.

**Rationale**: `flat-attached-fallback` is a legitimate m061-era vocabulary entry with clear semantics. Removing it would be a backward-compat break for consumers keyed on it. Preserving it means FR-006's "non-orphan → no annotation" rule has an exception for `flat-attached-fallback` (which is BFS-reachable via synthesized fallback but still carries the annotation).

## R2 — Q1 clarification impact: BFS-unreachable definition

**Decision**: Per Q1 clarification (2026-07-06), orphan = BFS-unreachable from `metadata.component.purl`. This is a strict superset of the pre-167 zero-incoming definition:

- Every component m061 already annotates (zero incoming) continues to be annotated (BFS-unreachable via same criterion — if nothing points at it, root can't reach it).
- Multi-version cluster non-leaves (like `foo@2.0.0` where a parent-orphan `parent-b@1.0.0` cycles back — parent-b has 1 incoming from foo@2.0.0 but is itself unreachable) are NEWLY captured.

**Concrete new coverage**: milestone-164 podman-desktop's 12 residual orphans (which included the multi-version vitest@4.1.8 cluster) all become BFS-unreachable and receive `dead-lockfile-entry` annotations under m167.

**Rationale**: Matches milestone-165 audit's `analyze.py` classifier directly. Delivers m165's #2 recommendation cleanly.

## R3 — Emit-tier choice: Go-reader-time vs emit-time

**Decision**: Two-tier emission.

- **Existing Go-reader-time** (`legacy.rs:2091,2118`): UNCHANGED. Continues emitting `unresolved-indirect-require` + `flat-attached-fallback` per m061 semantics.
- **New emit-time classifier** (`generate/orphan_reason.rs`): runs at emission time AFTER `compute_graph_completeness`. Iterates every `pkg:golang/*` and `pkg:npm/*` component. Applies FR-005 priority per BFS-unreachability + same-name-sibling check.

**Overwrite semantics**: When the emit-time classifier picks a MORE-SPECIFIC code than the Go-reader-time value (e.g., `stale-go-sum-entry` vs `unresolved-indirect-require`), the emit-time value OVERWRITES. When the emit-time classifier can't produce a more specific value (Go component: BFS-unreachable but no same-name sibling → would produce `unresolved-indirect-require`, same as what the Go reader already emitted), the classifier leaves the Go-reader-time value in place.

**Priority order per FR-005** (most-specific to least-specific):
1. `stale-go-sum-entry` (Go, BFS-unreachable, same-name sibling reachable)
2. `dead-lockfile-entry` (npm, BFS-unreachable, same-name sibling reachable)
3. `hoisted-unused` (npm, BFS-unreachable, no same-name reachable sibling)
4. `unresolved-indirect-require` (Go, BFS-unreachable, no same-name reachable sibling) — falls back to Go-reader-time value
5. `flat-attached-fallback` (Go, backfill-attached, technically BFS-reachable) — NEVER overwritten by m167 classifier

**Rationale**: Two-tier preserves m061 backward-compat + m053 backfill semantics + adds the 3 new codes without emission-time conflicts. Emit-time classifier is additive.

**Alternatives considered**:

- **A. Move ALL emission to emit-time**: rejected — breaks m061 backward-compat if the timing shift produces different results; also breaks `flat-attached-fallback` which is emitted BEFORE the assembled graph exists.
- **B. Two-tier (chosen)**: minimal disturbance to existing code.
- **C. Add npm-reader-time emitter parallel to the Go one**: rejected — npm reader would need to compute reachability locally, which requires knowing the full graph (not available at reader time).

## R4 — Reusing milestone-158's BFS infrastructure

**Decision**: Extend `GraphCompletenessResult` (at `graph_completeness/mod.rs:75`) to expose a `pub reachable_set: HashSet<String>` field. Milestone-167 classifier reads this field to determine which components are BFS-unreachable.

**Rationale**: milestone-158's `multi_source_bfs` in `bfs.rs:141` already computes the reachable set; it's currently used only for the pass/fail decision in `compute_graph_completeness` and then discarded. Exposing it via the result struct is a 3-line change (add field + populate in `mod.rs:265`).

**Alternatives considered**:

- **A. Add a new BFS pass just for m167**: rejected — duplicates work; BFS is O(V+E) but non-trivial on large scans.
- **B. Extend GraphCompletenessResult (chosen)**: reuses existing computation; zero extra runtime cost.
- **C. Move BFS out of graph_completeness and into a shared utility**: rejected as over-scoped for m167. Could be considered later if a third consumer needs it.

## R5 — Same-name-reachable-sibling check

**Decision**: For a candidate orphan `pkg:npm/foo@2.0.0`, "same-name reachable sibling" is defined as: exists `pkg:npm/foo@<any-other-version>` in `components[]` AND that PURL is in the reachable_set.

**Algorithm**:

```rust
fn has_reachable_same_name_sibling(
    orphan_purl: &Purl,
    all_components: &[ResolvedComponent],
    reachable_set: &HashSet<String>,
) -> bool {
    let orphan_name = orphan_purl.name();
    let orphan_ecosystem = orphan_purl.ecosystem();
    for c in all_components {
        if c.purl.as_str() == orphan_purl.as_str() { continue; } // skip self
        if c.purl.ecosystem() != orphan_ecosystem { continue; }
        if c.purl.name() != orphan_name { continue; }
        if reachable_set.contains(c.purl.as_str()) { return true; }
    }
    false
}
```

**Rationale**: Matches milestone-165 audit's `analyze.py` classifier logic exactly.

**Complexity**: O(N) per orphan × O(orphan_count) orphans = O(N × K) where K = orphan count. On podman-desktop: 2748 × 12 = ~33K comparisons. Sub-millisecond.

## R6 — FR-008 tracing log field naming

**Decision**: Fire ONE per-scan info-level log line with 4 fields:
- `orphan_reason_stale_go_sum_entry=<N>`
- `orphan_reason_dead_lockfile_entry=<N>`
- `orphan_reason_hoisted_unused=<N>`
- `orphan_reason_unresolved_indirect_require=<N>` (delta from Go-reader-time; how many the emit-time classifier LEFT AS-IS)

Fire unconditionally per scan (matches milestone-166 posture). Zero counters indicate a healthy scan.

**Rationale**: Grep-friendly per m157-onwards convention. Snake_case field names match Rust log convention.

**Alternatives considered**:

- **A. One log line with all 4 counts (chosen)**: consolidated, easy to parse.
- **B. Log at WARN when total > 0**: rejected — orphans are HONEST SIGNAL, not warnings.
- **C. Log per-code per-component (verbose)**: rejected — noisy on scans with 40+ orphans.

## R7 — Integration test scope

**Decision**: SC-009 integration test at `mikebom-cli/tests/orphan_reason_expand.rs` synthesizes a multi-ecosystem monorepo:
- 2 Go modules: `foo@1.0.0` (reachable via root) + `foo@2.0.0` (orphan, same-name sibling → `stale-go-sum-entry`)
- 2 npm packages: `bar@1.0.0` (reachable) + `bar@2.0.0` (orphan, same-name sibling → `dead-lockfile-entry`)
- 1 npm package: `unrelated@1.0.0` (BFS-unreachable, no same-name sibling → `hoisted-unused`)

Asserts:
- (a) Each orphan carries the expected reason code per FR-005 priority
- (b) Reachable components carry NO orphan-reason
- (c) FR-008 log field values match expected counts

**Rationale**: Comprehensive coverage of all 3 new codes + FR-005 priority + FR-006 three-state semantics in a single test.

## R8 — Root-cause emitter investigation for the "other-orphan" bucket

**Decision**: Milestone 167 does NOT introduce an `other-orphan` reason code. Per the Q1 clarification (BFS-unreachable definition), every orphan classifies into one of the 4 codes based on ecosystem + same-name-sibling presence. milestone-165's `other-orphan` bucket was the audit tool's catch-all when its `analyze.py` classifier's rules didn't match; under m167's cleaner 4-code partition, those cases fall into `unresolved-indirect-require` (Go, no sibling) or `hoisted-unused` (npm, no sibling).

**Rationale**: 4-code closed vocabulary is easier for consumers than an open enum with a catch-all. Milestone 165 audit's `analyze.py` will be updated post-167 to prefer the emitted values.

## Open items (none blocking)

All research questions resolved. Ready for Phase 1.
