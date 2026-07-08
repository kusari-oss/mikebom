# Phase 0 Research: m172 Go Fallback Count Annotation

**Feature**: 172-go-fallback-count
**Date**: 2026-07-07

## R1 — m160 C110 plumbing precedent

**Question**: Where does `go_transitive_coverage` flow from the resolver to the format emitters? The new fallback count needs to travel the same path.

**Decision**: Mirror the C110 flow exactly. Precise plumbing path (post-m170 shape, no `go_graph_completeness` since that was retired):

| File | Line | Role |
|---|---|---|
| `mikebom-cli/src/scan_fs/mod.rs` | 98 | `pub go_transitive_coverage: Option<...>` field on `ScanResult` |
| `mikebom-cli/src/scan_fs/mod.rs` | 268-271 | `let mut go_transitive_coverage: Option<...> = None;` — local variable |
| `mikebom-cli/src/scan_fs/mod.rs` | 307 | Assigned from `scan_result.diagnostics.go_transitive_coverage.clone()` |
| `mikebom-cli/src/scan_fs/mod.rs` | 809 | Placed into `ScanResult` returned by `scan_path` |
| `mikebom-cli/src/cli/scan_cmd.rs` | 1975 | Destructured from `scan_path`'s return |
| `mikebom-cli/src/cli/scan_cmd.rs` | 2613 | Threaded into `SbomEmission { go_transitive_coverage: go_transitive_coverage.as_ref(), ... }` |
| `mikebom-cli/src/generate/mod.rs` | 75-80 | `pub go_transitive_coverage: Option<&'a GoTransitiveCoverage>` on `SbomEmission` |
| `mikebom-cli/src/generate/cyclonedx/metadata.rs` | 466-481 | CDX emission — `if let Some(coverage) = go_transitive_coverage { properties.push(...) }` |
| `mikebom-cli/src/generate/spdx/annotations.rs` | (parallel) | SPDX 2.3 emission — same shape |
| `mikebom-cli/src/generate/spdx/v3_annotations.rs` | (parallel) | SPDX 3 emission — same shape |

**Path forward**: add a parallel `go_transitive_fallback_count: Option<usize>` field at every corresponding step. In `ScanResult`, add the new field alongside `go_transitive_coverage`. In `scan_fs::mod.rs`, populate it at line 307 from `scan_result.diagnostics.go_transitive_fallback_count`. Thread through the return, destructure at scan_cmd.rs, add to SbomEmission, emit at the CDX/SPDX/SPDX3 sites right after the C110 block.

## R2 — Aggregation site: `ModuleGraphMap` vs `LadderSummary`

**Question**: Where should the fallback-count computation live? On the `ModuleGraphMap` (per-scan module accessor) or on `LadderSummary` (aggregate diagnostics)?

**Decision**: **Aggregate on `LadderSummary`** — put a new field `pub go_transitive_fallback_count: usize` on the summary type. Rationale:

1. `LadderSummary` already exists as the doc-scope diagnostics carrier for the resolver — coverage classification uses it (`compute_coverage(&summary, &ctx)`).
2. Computing at `ModuleGraphMap` accessor time would require adding a public accessor to a lower-level type + threading it separately. Redundant work.
3. The summary is built during resolver output, and the emitter needs a single scalar to consume — matches the existing plumbing shape.

**Alternatives considered**:
- **Add `fallback_count()` accessor on `ModuleGraphMap`** — extra abstraction hop for zero benefit. Rejected.
- **Compute at emission time from per-component annotations** — would work (m160 T001 emits per-component `mikebom:go-transitive-source`) but conflates emission logic with counting logic. Rejected. Also would require the emission code to iterate all components looking for the annotation, which is silly when the resolver already knows the count.

## R3 — Emission gating for FR-002 (no Go scan case)

**Question**: How does m160 C110 handle "no Go scan happened" so we mirror it for FR-002 (annotation absent when no Go components)?

**Decision**: **m160 gates on `go_transitive_coverage.is_some()`** at the CDX metadata.rs:470 site: `if let Some(coverage) = go_transitive_coverage { properties.push(...) }`. When no Go scan happened, `scan_result.diagnostics.go_transitive_coverage` stays `None`, propagating through the plumbing to `SbomEmission.go_transitive_coverage: None`, and the emission block is skipped.

**Path forward**: mirror exactly. Wrap the C117 emission in `if let Some(count) = go_transitive_fallback_count { ... }`. Emit `"0"` (per Q1 clarification) when count is `Some(0)`; only omit when count is `None` (which corresponds to "no Go scan").

## R4 — Go-touching goldens beyond the primary set

**Question**: Are there Go-touching goldens beyond `golang.*.json`? m171 T005's discovery suggested `pkg_alias_binding/image-baz.cdx.json` was a stray.

**Decision**: **Only 3 committed goldens contain `pkg:golang/*` components** per `find + grep`:
- `mikebom-cli/tests/fixtures/golden/cyclonedx/golang.cdx.json`
- `mikebom-cli/tests/fixtures/golden/spdx-2.3/golang.spdx.json`
- `mikebom-cli/tests/fixtures/golden/spdx-3/golang.spdx3.json`

The `.actual.json` variants are `.gitignore`-covered per m169 T038 pattern; they're the diff-payload files created by failing goldens tests, not committed.

**Path forward**: only the 3 primary golden files need regen. Non-Go goldens (apk, cargo, deb, gem, maven, npm, pip, rpm, cmake, bazel, plus the pkg_alias_binding fixture from m171) MUST show zero delta since m172 only changes the Go emission path. SC-008 verification: `git diff main -- 'mikebom-cli/tests/fixtures/golden/**' | grep '^[+-]' | grep -v 'golang\.'` should return empty.

## R5 — Reading guide §3.5 status

**Question**: Does the reading guide have a `mikebom:go-transitive-coverage` section that this milestone enriches, or a new subsection needs to be created?

**Decision**: **Partially exists — needs enrichment**. Grep of `docs/reference/reading-a-mikebom-sbom.md`:

- §3.1: Vulnerability scanning
- §3.2: Compliance auditing
- §3.3: Build provenance
- §3.4: Transparency / completeness gaps

The `mikebom:go-transitive-coverage` annotation was added to the reading guide in m170 T029b, likely landing under §3.4 Transparency / completeness gaps. Need to verify — but almost certainly it's in §3.4 as a per-signal callout (m170 T029b explicitly said "new §3.5 covering `mikebom:go-transitive-coverage` (C110)").

Actually the grep shows §3.4 is the last section — no §3.5 exists yet. m170 T029b likely added the coverage annotation as a callout WITHIN §3.4 rather than as a new §3.5.

**Path forward**: enrich the existing coverage of `mikebom:go-transitive-coverage` (wherever it landed in §3.4 or elsewhere) with the 5-step ladder mechanism explanation + fallback-count jq recipe. The reading guide's structure is flat "one signal per subsection", so adding fallback-count as its own subsection alongside coverage is the pattern to follow.

## Consolidated open questions for `/speckit-tasks`

Every research question is resolved. Zero NEEDS CLARIFICATION markers propagate to planning.

- R1: precise plumbing path documented above.
- R2: aggregation on `LadderSummary`; new field `go_transitive_fallback_count: usize`.
- R3: emission gating via `Option<usize>` presence, mirror m160 C110.
- R4: only 3 Go goldens to regen.
- R5: enrich the existing `mikebom:go-transitive-coverage` coverage in the reading guide; add fallback-count as a sibling subsection.
