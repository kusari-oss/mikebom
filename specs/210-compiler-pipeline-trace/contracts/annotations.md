# Contract: `mikebom:*` annotations (C130..C134)

**Date**: 2026-07-19
**Purpose**: Lock the wire shape + emission rules for the 5 new `mikebom:*` annotations across all three SBOM formats. Reviewers cite this doc when auditing emitter code.

## A-1: C130 — `mikebom:source-read-set` (per-component)

**Purpose**: FR-006 signal — enumerates the source files that contributed to a specific SBOM component.

**Wire shape**:

```json
{
  "invocation_ids": [<u64>, ...],
  "read_set": [
    { "path": "<abs-path>", "sha256": "<64-hex>", "kind": "file" },
    { "path": "<stdin>", "kind": { "stdin_input": { "bytes_read": <u64> } } }
  ]
}
```

**Presence rules**: emitted when `ScanArtifacts.attestation.predicate.compiler_pipeline.is_some()` AND the component's file-path (from m133 evidence OR `hashes[]` file-anchor) intersects at least one compiler-invocation's write-set. The `read_set` is the TRANSITIVE union of the matching invocation's read-set PLUS the read-sets of all ancestor invocations in the DAG (per Clarifications Q1). Trace-noise entries (per FR-016) are excluded before emission.

**Ordering**: `read_set[]` sorted by `path` (byte-order lex). `invocation_ids[]` sorted ascending.

## A-2: C131 — `mikebom:read-set-source` (per-component)

**Purpose**: FR-015 + FR-017 signal — indicates how the read-set for this component was produced (or why it's omitted).

**Wire shape**: string enum with the following values:
- `"traced"` — read-set captured via eBPF observation.
- `"cache-hit"` — **RESERVED**. Emitted only when a future milestone adds compiler-cache-server tracing (sccache/ccache/mold). MVP does NOT emit this value.
- `"trace-attach-late"` — trace attached mid-build; read-set may be partial.
- `"unknown"` — component didn't map to any compiler-invocation write-set. In MVP, this includes cache-served components (per FR-015 revised: MVP cannot distinguish cache-hit from other unmapped cases without cache-server tracing).

**Presence rules**: emitted on EVERY component the compiler-pipeline data covers. When value is `"traced"`, C130 is also present. When `"unknown"` (or the future `"cache-hit"`), C130 is OMITTED. When `"trace-attach-late"`, C130 is present but marked `partial: true`.

## A-3: C132 — `mikebom:compiler-pipeline-completeness` (document-scope)

**Purpose**: FR-008 signal — surfaces trace integrity for the compiler-pipeline data.

**Wire shape**:

```json
{ "state": "complete" }
```

OR

```json
{ "state": "degraded", "dropped": <u64>, "affected_component_count": <usize> }
```

OR

```json
{ "state": "partial", "reason": "attach_late" }
```

**Presence rules**: emitted UNCONDITIONALLY at document scope when `compiler_pipeline.is_some()`. Value `"complete"` is a positive signal; `"degraded"` + `"partial"` are the transparency signals per Principle X.

## A-4: C133 — `mikebom:secrets-read-filtered` (document-scope)

**Purpose**: FR-016a signal — surfaces that the trace filtered secret-adjacent paths (auditable evidence of "the build touched secrets" without leaking which secrets).

**Wire shape**: string containing the u64 count of filtered secret-adjacent reads.

**Presence rules**: emitted only when `CompilerPipelineData.secrets_read_filtered > 0`. Zero-count case is silent (no annotation).

## A-5: C134 — `mikebom:trace-attach-late` (per-component)

**Purpose**: FR-017 signal — marks components whose compiler-invocation started before mikebom attached.

**Wire shape**: literal string `"true"`.

**Presence rules**: emitted per-component when the component's invocation was captured as attach-late. Complements C131 = `"trace-attach-late"`.

## A-6: Format-specific emission carriers (byte-identity envelope)

Per Principle V, wire shapes are byte-identical across formats; only the carrier field differs.

| Format | Doc-scope carrier | Per-component carrier |
|---|---|---|
| CDX 1.6 | `metadata.properties[]` (`name`, `value`) | `components[].properties[]` (`name`, `value`) |
| SPDX 2.3 | doc-scope `annotations[]` (`annotator: "Tool: mikebom"`, `annotationType: "OTHER"`, `comment: <envelope-string>`) | `packages[].annotations[]` (same shape) |
| SPDX 3.0.1 | `Annotation` element with `subject: <document-IRI>`, `statement: <envelope-string>` | `Annotation` element with `subject: <package-element-IRI>`, `statement: <envelope-string>` |

The envelope-string uses the m071 `MikebomAnnotationCommentV1` shape: `{"schema": "MikebomAnnotationCommentV1", "field": "<C-row-annotation-name>", "value": <shape-per-A1..A5>}`.

## A-7: `docs/reference/sbom-format-mapping.md` update

Five new catalog-row sections appended after the m208 C129 section:

- **C130** rationale: no format has a "list of source files that contributed to this binary" field. CDX `components[].evidence.identity[]` is about identity discovery evidence, not source-input attribution. SPDX 2.3 `Package.originator` is who published, not what was compiled. SPDX 3 has `SoftwareArtifact.originatedBy` — same semantic (publisher, not source files). `mikebom:*` bridging is justified per Principle V.
- **C131** rationale: no format has an "explanation of how this component's source attribution was obtained" field. Downstream policy consumers need this to distinguish traced from cache-hit; `mikebom:*` is warranted.
- **C132** rationale: no format has a "compiler-pipeline observation was complete or degraded" signal. Related standards-native fields (SPDX `Package.filesAnalyzed`) address a different concern (has the license/copyright scan been done, not has the pipeline observation been complete).
- **C133** rationale: no format has a "secret paths were observed and filtered" signal.
- **C134** rationale: no format has a "trace attached mid-build" signal.

Milestone-071 parity infrastructure gains 5 new catalog rows in numerically-sorted order (appended AFTER existing C129).
