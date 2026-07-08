# Phase 1 Data Model: m172 Go Fallback Count Annotation

**Feature**: 172-go-fallback-count
**Date**: 2026-07-07

Three entities gain new state. Two new pieces of code emit new annotations. One new parity catalog row wires the annotation across the format-parity gate.

## Entity 1 — `LadderSummary` (field ALREADY EXISTS)

**Location**: `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs:366-378`

### Pre-172 shape

Already tracks per-resolver-run diagnostics — INCLUDING the counter this milestone would have added:

```rust
pub struct LadderSummary {
    pub graph_count: usize,
    pub cache_count: usize,
    pub proxy_count: usize,
    pub gosum_fallback_count: usize,  // ← EXISTS since milestone 091
    pub missing_count: usize,
    // ...other fields
}
```

**Discovered during m172 analyze phase**: `LadderSummary.gosum_fallback_count: usize` (line 371) is populated at line 916 whenever a module resolves via `ResolutionStep::GoSumFallback` (`map.summary_mut().gosum_fallback_count += 1`). This is exactly what m172 needs to expose.

### Post-172 shape

**No schema change**. m172 reads the existing `gosum_fallback_count` field. Naming reconciliation: keep the internal `gosum_fallback_count` name (renaming ripples through 10+ callsites for zero benefit); expose it externally via the standards-facing `mikebom:go-transitive-fallback-count` annotation name.

**Populated by**: unchanged — milestone-091's step-5 handler at `graph_resolver.rs:916` already fires the increment for every step-5 resolution.

## Entity 2 — `ScanResult` / `SbomEmission` plumbing

**Location**: `mikebom-cli/src/scan_fs/mod.rs` (ScanResult) + `mikebom-cli/src/generate/mod.rs` (SbomEmission).

### Pre-172 shape

Both structs carry `go_transitive_coverage: Option<GoTransitiveCoverage>` for the milestone-160 C110 emission.

### Post-172 shape

Add one new field to each:

**`ScanResult`** (`scan_fs/mod.rs`):

```rust
pub struct ScanResult {
    // ...existing fields (go_transitive_coverage, go_workspace_mode, etc.)

    /// Milestone 172: doc-scope count of Go modules resolved via
    /// step-5 (go.sum flat fallback) instead of steps 1-3. `None`
    /// iff no Go scan happened (annotation absent per FR-002);
    /// `Some(0)` on healthy Go scans (annotation present with value "0"
    /// per Q1 clarification); `Some(N > 0)` on degraded scans.
    pub go_transitive_fallback_count: Option<usize>,
}
```

**`SbomEmission`** (`generate/mod.rs`):

```rust
pub struct SbomEmission<'a> {
    // ...existing fields (go_transitive_coverage: Option<&'a GoTransitiveCoverage>, etc.)

    /// Milestone 172: mirror of ScanResult.go_transitive_fallback_count.
    pub go_transitive_fallback_count: Option<usize>,
}
```

**Plumbing pattern**: identical to m160's `go_transitive_coverage` per research §R1. Source of the value: read `scan_result.diagnostics.gosum_fallback_count` (bare `usize`) and wrap in `Some(...)` when a Go scan happened; leave as `None` otherwise.

## Entity 3 — CDX 1.6 metadata `properties[]` shape

**Location**: emitted CDX SBOM, `.metadata.properties[]`.

### Pre-172 shape (Go scan example)

```json
{
  "name": "mikebom:go-transitive-coverage",
  "value": "unknown"
},
{
  "name": "mikebom:go-transitive-coverage-reason",
  "value": "offline-mode: transitive edges from proxy fetches unavailable"
}
```

### Post-172 shape (Go scan, degraded, 73 fallbacks)

```json
{
  "name": "mikebom:go-transitive-coverage",
  "value": "unknown"
},
{
  "name": "mikebom:go-transitive-coverage-reason",
  "value": "offline-mode: transitive edges from proxy fetches unavailable"
},
{
  "name": "mikebom:go-transitive-fallback-count",
  "value": "73"
}
```

### Post-172 shape (Go scan, healthy — value is `"0"` per Q1)

```json
{
  "name": "mikebom:go-transitive-coverage",
  "value": "complete"
},
{
  "name": "mikebom:go-transitive-fallback-count",
  "value": "0"
}
```

### Post-172 shape (non-Go scan)

Neither `mikebom:go-transitive-coverage` NOR `mikebom:go-transitive-fallback-count` present. Both are Go-gated per m160 (C110) and m172 (C117) emission gating.

**Delta per Go golden**: +4 lines (the new 4-line JSON entry).

## Entity 4 — SPDX 2.3 annotations envelope

**Location**: emitted SPDX 2.3 SBOM, document-scope `annotations[]` array — envelope shape per m080.

Same JSON shape wrapped in the mikebom annotation-comment envelope:

```json
{
  "annotationType": "OTHER",
  "annotator": "Tool: mikebom-0.1.0-alpha.NN",
  "comment": "{\"schema\":\"mikebom-annotation/v1\",\"field\":\"mikebom:go-transitive-fallback-count\",\"value\":\"73\"}",
  "annotationDate": "..."
}
```

**Delta per Go golden**: +5 lines (annotation-envelope object plus the field-value payload).

## Entity 5 — SPDX 3.0.1 typed Annotation element

**Location**: emitted SPDX 3.0.1 SBOM, `@graph[]` typed Annotation element targeting the SpdxDocument root IRI.

```json
{
  "type": "Annotation",
  "spdxId": "urn:mikebom:annotation:...",
  "subject": "urn:spdx:document-root",
  "statement": "{\"schema\":\"mikebom-annotation/v1\",\"field\":\"mikebom:go-transitive-fallback-count\",\"value\":\"73\"}",
  "annotationType": "other",
  "creationInfo": "_:creationInfo0"
}
```

**Delta per Go golden**: ~7 lines (a full graph-element object) — larger than CDX/SPDX 2.3 due to SPDX 3's typed-graph representation.

## Entity 6 — Parity catalog row C117

**Location**: `docs/reference/sbom-format-mapping.md` Section C + `mikebom-cli/src/parity/extractors/mod.rs` EXTRACTORS table + per-format extractor helpers.

### Docs row (in `sbom-format-mapping.md`)

New row after C116 (dep-alternative-alternates):

```
| C117 | `mikebom:go-transitive-fallback-count` | document-scope property/annotation — stringified non-negative integer. Emitted iff the scan included ≥1 Go component AND the Go transitive resolver ran. Value = count of Go modules whose final resolution step was m091's `GoSumFallback` (step 5 of the ladder). Per Q1 (spec.md m172): value `"0"` MUST be emitted explicitly on healthy scans; consumers distinguish "no Go in scan" (annotation absent) from "Go was scanned cleanly" (annotation `"0"`). Companion to milestone-160's `mikebom:go-transitive-coverage` (C110) and per-component `mikebom:go-transitive-source` (C108) — this doc-scope aggregate lets consumers threshold on "N modules degraded" without walking every component's annotations. Milestone 172. | Document-level `Annotation` on `SPDXRef-DOCUMENT`, `MikebomAnnotationCommentV1` envelope: `{"type":"mikebom:go-transitive-fallback-count","value":"73"}`. | Document-scope `Annotation` element on the SpdxDocument root IRI; same envelope shape. | **KEEP-NO-NATIVE**. CDX 1.6 / SPDX 2.3 / SPDX 3.0.1 have no native "count of transitive resolutions that degraded to flat fallback" field. Rejected alternatives: (1) CDX `component.evidence.identity[].confidence` (per-component evidence-model, not a doc-scope aggregate); (2) SPDX `Package.filesAnalyzed` (unrelated: file-analysis flag, not resolution-quality). Standards-native precedence per Constitution Principle V: milestone-172 FR-004 codifies migration if either standard adopts a resolution-quality vocabulary. |
```

### EXTRACTORS row (in `parity/extractors/mod.rs`)

New row placed alphabetically after C116 (the dep-alternative-alternates row added in m169):

```rust
// Milestone 172 (closes #TBD): C117 document-scope
// `mikebom:go-transitive-fallback-count` annotation. Non-negative
// integer count of Go modules whose final resolution step was
// `ResolutionStep::GoSumFallback` (step 5 of the m091 ladder).
// Companion to C108 (per-component) and C110 (doc-scope coverage
// verdict). Emitted with value "0" on healthy Go scans per Q1
// clarification; absent when no Go scan happened.
ParityExtractor { row_id: "C117", label: "mikebom:go-transitive-fallback-count", cdx: c117_cdx, spdx23: c117_spdx23, spdx3: c117_spdx3, directional: Directionality::SymmetricEqual, order_sensitive: false },
```

Plus the three extractor helpers in `cdx.rs`, `spdx2.rs`, `spdx3.rs`, each using the standard `cdx_anno!` / `spdx23_anno!` / `spdx3_anno!` macro pattern with `document` scope.

## Cross-entity invariants (post-172)

1. **Presence gate**: `mikebom:go-transitive-fallback-count` is present iff `mikebom:go-transitive-coverage` is present. Both share the "≥1 Go component + Go resolver ran" emission condition. Enforced by the plumbing shape: same `Option` wrapping, same emission-gating point.
2. **Count sum invariant** (SC-005): `[.metadata.properties[] | select(.name == "mikebom:go-transitive-fallback-count") | .value | tonumber] == [.components[]?.properties[]? | select(.name == "mikebom:go-transitive-source" and .value == "go-sum-fallback")] | length` — the doc-scope count equals the sum of per-component tags. Structurally guaranteed since both derive from the same `ResolutionStep` values.
3. **Value shape**: stringified non-negative integer with no leading zeros (i.e., matches regex `^0$|^[1-9][0-9]*$`). Matches the m134 `mikebom:purl-collisions-detected` value convention.

## State transitions

None. This is a stateless emission-code addition — the value is computed once per scan and emitted verbatim into three format outputs. No lifecycle changes.
