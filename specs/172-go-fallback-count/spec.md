# Feature Specification: Go-transitive fallback attachment count doc-scope annotation

**Feature Branch**: `172-go-fallback-count`
**Created**: 2026-07-07
**Status**: Draft
**Input**: User description: "Add doc-scope `mikebom:go-transitive-fallback-count` annotation exposing how many Go modules resolved via the m091 step-5 flat fallback instead of proper transitive-topology recovery (steps 1-3 of the ladder). Would have let today's guac reporter see '73 modules got fallback-attached' immediately instead of chasing counts."

## Background

Mikebom's Go transitive-edge resolution uses a 5-step ladder (see `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs:65-83`):

1. `go mod graph` subprocess (needs `go` binary on PATH)
2. `$GOMODCACHE` walk (milestone 053)
3. HTTP fetch from `$GOPROXY`
4. (retired numbering)
5. **go.sum-driven flat fallback** (milestone 091): parses go.sum directly and emits **flat root → transitive edges** covering the deduped (module, version) closure — no parent-child topology between transitives, because go.sum doesn't encode that.
6. Unresolved — no edges produced.

When step 5 activates for a module, that module's incoming edges from its real intermediate parents are lost. Downstream: the module appears as a "direct" of root instead of a transitive child of Y where Y actually depends on X. Consumers who read the emitted graph for topology see a distorted picture — flattened where reality would have hierarchy.

This is intentional design (m091 chose "flat fallback" over "drop the module entirely") but it makes the emitted SBOM's shape **environment-dependent**: the same source scanned in a healthy environment (all modules resolved via steps 1-3) produces a hierarchical graph, while the same source scanned in a degraded environment (some modules fall through to step 5) produces a partially-flattened graph.

**Recent incident** (2026-07-07 guac investigation): a downstream consumer regenerated their guac SBOM fixture and observed +73 direct deps and -1438 edges vs the previous version's fixture. The delta appeared to be a mikebom behavior change but was actually 100% environmental — 73 modules fell through to step 5 in the new scan's environment. There was no doc-scope signal in the emitted SBOM identifying this — the consumer had to work through direct-dep and edge counts, then compare per-component annotations, to diagnose. The pre-existing `mikebom:go-transitive-coverage` (C110, m160) reports whether the ladder measured cleanly (`complete`/`partial`/`unknown`) but does not specifically expose HOW MANY modules landed on the flat-fallback path.

**This milestone adds a doc-scope integer signal** that closes the diagnostic gap: `mikebom:go-transitive-fallback-count = "N"` where N is the number of Go modules resolved via step 5. When N is 0, the emitted graph faithfully represents Go transitive topology. When N > 0, the shape is degraded by exactly N flattened attachments — consumers can threshold, filter, or annotate accordingly.

## Clarifications

### Session 2026-07-07

- Q: Emit `mikebom:go-transitive-fallback-count = "0"` explicitly on healthy Go scans, or omit the annotation entirely when N=0? → A: **Option A — emit `"0"` explicitly**. Matches m134's `mikebom:purl-collisions-detected` and m158's universal `mikebom:graph-completeness` emit-always-when-applicable precedent. Rationale: consumers reading the annotation for affirmative "scan was clean" verdict can rely on presence-with-value; distinguishes "no Go components in scan" (annotation absent) from "Go was scanned and fell back cleanly" (annotation present with value `"0"`). Rejected: omit-when-zero (Option B — smaller default output but forces consumer correlation with `mikebom:go-transitive-coverage` to affirmatively determine "scan was clean"); flag-gated emission (Option C — unnecessary CLI surface expansion).

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Consumer sees the fallback-count in the SBOM (Priority: P1)

A consumer opens a mikebom-emitted SBOM containing Go components. In the document-scope properties/annotations, they find `mikebom:go-transitive-fallback-count = "N"`. When N = 0, they know the Go graph shape is fully hierarchical. When N > 0, they know exactly how many modules landed on the flat-fallback path and that the graph's shape is partially degraded.

**Why this priority**: This is the core defect: today the signal is invisible. Every consumer confused by graph-shape variance (like the guac investigation reporter) can't self-diagnose from the SBOM alone; they have to trace through mikebom's source. This closes the gap.

**Independent Test**: Emit an SBOM for a Go project with a healthy scan (module cache warm, GOPROXY working); assert the annotation is present with value `"0"`. Emit the same SBOM in a degraded environment; assert the annotation reports a positive integer matching the count of Go modules that resolved via step 5.

**Acceptance Scenarios**:

1. **Given** a scan target with Go components AND a healthy environment (all Go modules resolve via steps 1-3), **When** the operator emits CDX 1.6 output, **Then** the document metadata's `properties[]` array contains an entry `{name: "mikebom:go-transitive-fallback-count", value: "0"}`.
2. **Given** a scan target with Go components AND a degraded environment where 73 modules fall through to step 5, **When** the operator emits any format, **Then** the annotation carries `value: "73"`.
3. **Given** a scan target with NO Go components (pure npm project, pure Rust project, etc.), **When** the operator emits any format, **Then** the `mikebom:go-transitive-fallback-count` annotation is ABSENT from the document metadata (matching the m160 C110 emission-gating convention — Go-specific signals emit only when Go is present).
4. **Given** a scan of a Go project where `mikebom:go-transitive-coverage` is `unknown` (offline mode, GOPROXY off, etc.), **When** the operator emits any format, **Then** the fallback-count annotation is still present and reports the actual count from the step-5 pass (Coverage=unknown does not mean the fallback-count is unknown; step 5 either ran and produced a number, or it didn't run and the number is 0).

---

### User Story 2 — Docs enrich the reading guide with the diagnostic recipe (Priority: P2)

A future consumer hitting Go graph-shape variance opens `docs/reference/reading-a-mikebom-sbom.md`'s `mikebom:go-transitive-coverage` section and finds the mechanism explained in plain English, with a callout to the new `mikebom:go-transitive-fallback-count` annotation. They can self-diagnose in minutes instead of hours.

**Why this priority**: Refinement. Solving the mechanism-explanation gap prevents future consumers from repeating the guac reporter's confusion path. Not P1 because the P1 annotation alone solves the immediate signaling gap; docs improve the discoverability.

**Independent Test**: A tech writer unfamiliar with the mikebom Go ladder opens the reading guide's `mikebom:go-transitive-coverage` section post-m172 and can explain the fallback mechanism + how to use `mikebom:go-transitive-fallback-count` from the doc alone, without opening mikebom source.

**Acceptance Scenarios**:

1. **Given** the post-m172 reading guide, **When** a reader opens `docs/reference/reading-a-mikebom-sbom.md`'s `mikebom:go-transitive-coverage` section, **Then** it explicitly names all 5 ladder steps + explains that step 5 produces flat root → transitive attachments losing parent-child topology.
2. **Given** the same section, **When** the reader looks for how to detect this, **Then** it points at the new `mikebom:go-transitive-fallback-count` annotation with a jq-recipe example.

---

### Edge Cases

- **Non-Go scans**: annotation MUST be absent (matches m160 C110 emission-gating).
- **Go scan where step 5 never fired**: annotation MUST emit with value `"0"` explicitly (not absent). Rationale: consumers looking for the signal need to distinguish "step 5 didn't fire" (healthy scan, value=0) from "no Go components at all" (annotation absent). Being explicit about 0 is more informative.
- **Go scan with multiple workspaces / `go.work`**: fallback-count aggregates across all workspace members. A single doc-scope value covers the whole scan.
- **Go scan where step 5 fires but the module already had OTHER edges** (via step 1-3 succeeding for a peer): only modules whose FINAL resolution step is step 5 count. If step 1 succeeded, that module doesn't count regardless of whether step 5 could have handled it.
- **Trace-mode scans** (`mikebom trace`): if trace-mode ever exercises the Go resolver (currently it does not), the annotation applies. Out of scope for this milestone if trace-mode doesn't touch the ladder.
- **`go.work` workspace members**: fallback-count treats each workspace member's step-5 modules as contributing to the total. Cross-workspace aggregation is per doc-scope, not per member.
- **Very large fallback counts**: the annotation's value is a plain integer string; no cap. If N > 10000 (unusual), the annotation is still valid.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When a scan includes ≥1 Go component AND the Go transitive resolver ran, mikebom MUST emit a document-scope annotation `mikebom:go-transitive-fallback-count` in CDX 1.6, SPDX 2.3, and SPDX 3.0.1 output formats. The value is a stringified non-negative integer.
- **FR-002**: When a scan has no Go components OR the Go transitive resolver did not run at all (e.g., no Go files in scan target), the annotation MUST NOT be emitted (matches the m160 C110 emission-gating precedent).
- **FR-003**: The value MUST equal the count of Go modules whose FINAL resolution step (per the m091 `ResolutionStep` enum) was `GoSumFallback` (step 5). Modules resolved via `GoModGraph` (step 1), `GoModCache` (step 2), `Proxy` (step 3), or `None` (step 6 — unresolved) do NOT count.
- **FR-004**: The annotation MUST appear in the parity catalog at a new row (e.g., C117) with the standard 4 fields (label, CDX location, SPDX 2.3 location, SPDX 3 location, justification). Constitution Principle V's standards-native audit MUST cite: CDX 1.6 / SPDX 2.3 / SPDX 3.0.1 have no native "how many transitive fetches degraded to a flat fallback" field; the `mikebom:*` annotation is the finer-info carve-out.
- **FR-005**: The value MUST be observable in `docs/reference/sbom-format-mapping.md`'s parity catalog as a symmetric-equal entry (same string value across all 3 emission formats).
- **FR-006**: A per-scan integration test (in the existing `mikebom-cli/tests/` layout) MUST verify: (a) annotation present + value = 0 on a healthy Go fixture; (b) annotation present + value > 0 on a Go fixture with `--offline` OR `GOPROXY=off` forcing step-5 fallback; (c) annotation absent on a pure-npm or pure-Rust fixture.
- **FR-007**: The reading guide at `docs/reference/reading-a-mikebom-sbom.md` MUST gain a new subsection under `mikebom:go-transitive-coverage` (§3.5) explaining the 5-step ladder, the step-5 fallback semantics, and the new fallback-count annotation. Include one jq-recipe example demonstrating "detect if the SBOM has flat-fallback contamination".
- **FR-008**: Pre-PR gate (`./scripts/pre-pr.sh`) MUST pass, including the m071 parity gate + the new integration test.
- **FR-009**: No golden byte-identity regression on non-Go ecosystem fixtures. Golden diffs on Go ecosystem fixtures are expected (adds 1 annotation line per Go SBOM) and MUST be regenerated per the standard `MIKEBOM_UPDATE_*_GOLDENS=1` workflow.

### Key Entities

- **`mikebom:go-transitive-fallback-count` (new document-scope annotation)**: a non-negative integer as a string. Present iff the scan includes Go components AND the Go transitive resolver ran. Value = count of Go modules whose final resolution step was `GoSumFallback` (m091 step 5).
- **`ResolutionStep::GoSumFallback` (existing enum variant, m091)**: the enum discriminant that marks step-5 resolution. Already exists in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`. No new type introduced; only a count aggregated at emission time.
- **`ModuleGraphMap.entries` (existing per-module map)**: the resolver's output. Each entry has a `source: ResolutionStep` field. This is what m172's count aggregates over.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For any emitted SBOM (CDX 1.6, SPDX 2.3, SPDX 3.0.1) of a Go project, `jq '.metadata.properties[]? | select(.name == "mikebom:go-transitive-fallback-count") | .value | tonumber'` returns a non-negative integer.
- **SC-002**: For any emitted SBOM of a non-Go project, `jq '.metadata.properties[]? | select(.name == "mikebom:go-transitive-fallback-count")'` returns nothing.
- **SC-003**: For a healthy scan of a stable Go fixture (module cache warm, GOPROXY working, `go` on PATH), the emitted `mikebom:go-transitive-fallback-count` is exactly `"0"`.
- **SC-004**: For a scan of the same Go fixture with `--offline` set (forcing step-5 fallback for modules that need proxy fetch), the emitted `mikebom:go-transitive-fallback-count` is a positive integer matching the actual count observed in the resolver's diagnostics (measurable via the existing `mikebom:go-transitive-source` per-component annotations from m160 T001).
- **SC-005**: The per-component `mikebom:go-transitive-source` annotations (m160 T001) and the doc-scope `mikebom:go-transitive-fallback-count` annotation MUST agree: `jq '[.components[]?.properties[]? | select(.name == "mikebom:go-transitive-source" and .value == "go-sum-fallback")] | length'` equals the value of `mikebom:go-transitive-fallback-count`. Guarantee: the doc-scope count is the aggregate of the per-component tags.
- **SC-006**: `docs/reference/reading-a-mikebom-sbom.md`'s post-m172 rendering has a subsection explaining the 5-step ladder + jq recipe. Verified by grep for keywords like `step 5`, `go-transitive-fallback-count`.
- **SC-007**: Pre-PR gate passes green (`>>> all pre-PR checks passed.`).
- **SC-008**: No regression: for every non-Go ecosystem golden in the m090 sibling-repo test suite (apk, cargo, deb, gem, maven, npm, pip, rpm, cmake, bazel), byte-identity is preserved.
- **SC-009**: A future SBOM consumer diagnosing an unexpected Go graph shape can locate the fallback-count annotation and infer the mechanism from the reading guide in under 5 minutes.

## Assumptions

- The m091 step-5 fallback is the ONLY source of flat root → transitive attachments that this signal cares about. The m061 `flat-attached-fallback` orphan-reason (per-component, C45) handles the SEPARATE case of Go components that had no incoming edge even after the backfill; that's already covered by C45 and is NOT what this signal exposes.
- The doc-scope count is computed at emission time from the per-component `mikebom:go-transitive-source` annotations (which m160 T001 already emits). No new resolver logic — this is an aggregation, not a computation.
- The annotation shape is a stringified integer (matching m134 `mikebom:purl-collisions-detected` and other numeric-value conventions in the catalog). Not a JSON number.
- The naming convention `mikebom:go-transitive-fallback-count` mirrors `mikebom:go-transitive-coverage` and `mikebom:go-transitive-coverage-reason` from m160 — the `go-transitive-*` namespace is the established home for m055/m091/m160 signals.
- No CLI flag is added. The annotation is emitted unconditionally when the emission-gating conditions are met, matching how m160's C110 works. Consumers who don't want the annotation can filter it out post-emission.
- The audit trail for reproducibility should NOT include the actual PURLs of the step-5-resolved modules at doc scope. Per-component `mikebom:go-transitive-source` already provides that granularity. Aggregating to a doc-scope integer is the minimum-information carrier; consumers wanting the per-module list drill into `.components[]?`.
- The value is a plain integer, not a range or bucketed enum. Rationale: even large counts are legitimate signals (a scan of a giant Go project might have 500 modules fall through step 5); consumers can do their own bucketing.
- Constitution Principle V standards-native audit: CDX 1.6 has no native equivalent (its evidence model tracks per-component confidence, not doc-scope resolution-quality counts). SPDX 2.3 has no native equivalent. SPDX 3.0.1's evidence profile is not stable in 3.0.1. The `mikebom:*` annotation is justified as a parity-bridging carve-out; documented in the sbom-format-mapping.md new row.
