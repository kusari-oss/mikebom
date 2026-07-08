# Contract: C118 + C119 wire shapes across CDX/SPDX 2.3/SPDX 3

**Feature**: 173-warm-go-cache
**Date**: 2026-07-08

Authoritative wire-shape reference. Deviations in emitted output are grounds for review comment.

## C118 — `mikebom:go-cache-warming-mode`

### CDX 1.6

**Where**: `.metadata.properties[]` — document-scope property.

```json
{ "name": "mikebom:go-cache-warming-mode", "value": "off" }
```

Placement: alphabetically before `mikebom:go-transitive-*` cluster (which starts with `coverage`). Consequence — the emitted metadata.properties[] Go-related block reads:
```
mikebom:go-cache-warming-failed         (C119, conditional)
mikebom:go-cache-warming-mode           (C118, unconditional)
mikebom:go-transitive-coverage          (C110)
mikebom:go-transitive-coverage-reason   (C111, conditional)
mikebom:go-transitive-fallback-count    (C117)
mikebom:go-workspace-mode               (C112, conditional)
```

**Emitted iff**: `SbomEmission.go_cache_warming.is_some()`.

**Value regex**: `^(off|per-workspace|offline-inhibited)$`.

### SPDX 2.3

**Where**: document-scope `annotations[]` on the `SpdxDocument`.

```json
{
  "annotationType": "OTHER",
  "annotator": "Tool: mikebom-0.1.0-alpha.NN",
  "annotationDate": "1970-01-01T00:00:00Z",
  "comment": "{\"schema\":\"mikebom-annotation/v1\",\"field\":\"mikebom:go-cache-warming-mode\",\"value\":\"off\"}"
}
```

Uses the standing `MikebomAnnotationCommentV1` envelope shape m080/m160/m172 established.

### SPDX 3.0.1

**Where**: `@graph[]` typed `Annotation` element targeting the SpdxDocument root IRI.

```json
{
  "type": "Annotation",
  "spdxId": "urn:mikebom:annotation:<content-hash>",
  "subject": "<SpdxDocument-root-IRI>",
  "statement": "{\"schema\":\"mikebom-annotation/v1\",\"field\":\"mikebom:go-cache-warming-mode\",\"value\":\"off\"}",
  "annotationType": "other",
  "creationInfo": "_:creationInfo0"
}
```

## C119 — `mikebom:go-cache-warming-failed`

### CDX 1.6

**Where**: `.metadata.properties[]` — document-scope property.

```json
{
  "name": "mikebom:go-cache-warming-failed",
  "value": "[{\"reason\":\"parse-error\",\"workspace\":\"cmd/bar\"},{\"reason\":\"timeout\",\"workspace\":\"cmd/foo\"}]"
}
```

**Value shape rules**:
- JSON-encoded array of record objects.
- Records sorted alphabetically by `workspace` (byte-identity across regenerations).
- Record fields serialized alphabetically (`reason` before `workspace`) via `serde_json::to_string(&sorted_vec)` with `WorkspaceFailure` derives `Serialize` with default field ordering matching struct declaration → we declare `workspace` first but serde's `#[serde(rename_all = "...")]` doesn't change order; explicitly use `#[derive(Serialize)]` with struct field order `reason, workspace` to get alphabetized output.
- Workspace paths RELATIVE to the scan root (not absolute; needed for byte-identity across environments and for portability of the emitted SBOM).
- Reason class values regex: `^(go-binary-absent|spawn-failed|timeout|subcommand-failed|parse-error|budget-exhausted)$`.

**Emitted iff**: `SbomEmission.go_cache_warming.map(|r| !r.failures.is_empty()).unwrap_or(false)`.

### SPDX 2.3

Same envelope as C118 with `field = "mikebom:go-cache-warming-failed"` and the same JSON-encoded array in `value`.

### SPDX 3.0.1

Same typed `Annotation` graph element shape as C118 with the same array-string in `statement`.

## Wire-shape parity guarantee

C118 and C119 are declared `Directionality::SymmetricEqual` in the parity catalog. The three formats MUST emit the same field name and the same (string-encoded) value across all three outputs — enforced by the m071 parity test suite.

## Non-goals

- No compact numeric summary in C119 (like `"3 workspaces failed"`). The array-of-records form is authoritative and machine-readable.
- No per-workspace timing data in C119. If needed, add in a future milestone under a different annotation.
- No CDX-native `component.evidence.identity[].confidence` mapping for C119. See Constitution Principle V audit in the plan.md Constitution Check row — evidence-model is per-component, C119 is a doc-scope aggregate; the semantic mismatch is the reason we keep `mikebom:*` here.

## Consumer jq recipes

### Recipe 1 — has this SBOM had cache warming?

```jq
.metadata.properties[]?
| select(.name == "mikebom:go-cache-warming-mode")
| .value
```

Returns `"off"` / `"per-workspace"` / `"offline-inhibited"` / empty.

### Recipe 2 — did any workspace fail warming?

```jq
.metadata.properties[]?
| select(.name == "mikebom:go-cache-warming-failed")
| .value
| fromjson
| map(.workspace)
```

Returns an array of workspace paths that failed, or empty output if no C119 was emitted.

### Recipe 3 — full Go-signals dashboard (post-173)

```jq
{
  fallback_count:     (.metadata.properties[]? | select(.name == "mikebom:go-transitive-fallback-count") | .value // "not-applicable"),
  cache_warming:      (.metadata.properties[]? | select(.name == "mikebom:go-cache-warming-mode") | .value // "not-applicable"),
  warming_failures:   ([.metadata.properties[]? | select(.name == "mikebom:go-cache-warming-failed") | .value | fromjson | .[]] // []),
  transitive_verdict: (.metadata.properties[]? | select(.name == "mikebom:go-transitive-coverage") | .value // "not-applicable")
}
```

Post-173 this composes m160 + m172 + m173 signals into one readable panel.
