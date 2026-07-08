# Contract: `mikebom:go-transitive-fallback-count` wire shape

**Feature**: 172-go-fallback-count
**Date**: 2026-07-07

The exact JSON shape of the C117 annotation across all 3 emission formats. Reviewers treat this as the authoritative reference; deviations in the emitted output are grounds for review comment.

## CDX 1.6

**Where**: `.metadata.properties[]` — document-scope property.

**Shape**:

```json
{
  "name": "mikebom:go-transitive-fallback-count",
  "value": "N"
}
```

Where `N` is a stringified non-negative integer. Placement: alphabetically after the existing `mikebom:go-transitive-coverage-reason` (which appears immediately after `mikebom:go-transitive-coverage` per m160 T035's implementation).

**Emitted iff**: `SbomEmission.go_transitive_fallback_count.is_some()`.

## SPDX 2.3

**Where**: document-scope `annotations[]` on the `SpdxDocument`.

**Shape** (`MikebomAnnotationCommentV1` envelope):

```json
{
  "annotationType": "OTHER",
  "annotator": "Tool: mikebom-0.1.0-alpha.NN",
  "annotationDate": "1970-01-01T00:00:00Z",
  "comment": "{\"schema\":\"mikebom-annotation/v1\",\"field\":\"mikebom:go-transitive-fallback-count\",\"value\":\"N\"}"
}
```

Placement: sibling of the C110/C111 annotations at document scope. Annotation-date driven by `MIKEBOM_FIXED_TIMESTAMP` for goldens.

**Emitted iff**: same condition as CDX.

## SPDX 3.0.1

**Where**: `@graph[]` typed `Annotation` element targeting the SpdxDocument root IRI.

**Shape**:

```json
{
  "type": "Annotation",
  "spdxId": "urn:mikebom:annotation:<content-hash>",
  "subject": "<SpdxDocument-root-IRI>",
  "statement": "{\"schema\":\"mikebom-annotation/v1\",\"field\":\"mikebom:go-transitive-fallback-count\",\"value\":\"N\"}",
  "annotationType": "other",
  "creationInfo": "_:creationInfo0"
}
```

**Emitted iff**: same condition as CDX + SPDX 2.3.

## Consumer jq recipes (post-172)

### Recipe 1 — check if scan hit any Go transitive fallbacks

```jq
.metadata.properties[]? | select(.name == "mikebom:go-transitive-fallback-count") | .value | tonumber
```

**Returns**:
- Number `0` — scan had Go components + resolved them cleanly (no fallback fires)
- Positive number — degraded scan; N modules landed on step-5 fallback
- Nothing (jq empty) — either no Go components in scan, OR pre-m172 SBOM

### Recipe 2 — differentiate "no Go in scan" from "not m172-produced"

Rather than gating on a version-string regex (which drifts every release), use the **presence of the m160 companion signal** as the m172-capability probe. Any post-m160 mikebom emits `mikebom:go-transitive-coverage` for every Go scan; m172 additionally emits `mikebom:go-transitive-fallback-count`. Together they tell you three states:

```jq
{
  covered: (.metadata.properties[]? | select(.name == "mikebom:go-transitive-coverage") | .value // "no-go-signal"),
  fallback: (.metadata.properties[]? | select(.name == "mikebom:go-transitive-fallback-count") | .value // "not-emitted")
}
```

**Interpretation**:
- `{covered: "no-go-signal", fallback: "not-emitted"}` — either no Go components in scan OR pre-m160 SBOM (very old mikebom)
- `{covered: "<value>", fallback: "not-emitted"}` — Go scan, m160-capable emitter, but pre-m172 (no fallback signal)
- `{covered: "<value>", fallback: "<count>"}` — Go scan, m172-capable emitter, fallback count = the value

No version-string parsing needed; the semantic gates itself.

### Recipe 3 — full Go health check (all m160+m172 signals)

```jq
{
  transitive_coverage: (.metadata.properties[]? | select(.name == "mikebom:go-transitive-coverage") | .value),
  fallback_count: (.metadata.properties[]? | select(.name == "mikebom:go-transitive-fallback-count") | .value // "0"),
  graph_completeness: (.metadata.properties[]? | select(.name == "mikebom:graph-completeness") | .value)
}
```

## Non-goals

- No per-format wire divergence beyond the standard envelope-vs-property distinction that already exists for m160 C110/C111.
- No emission ordering guarantee vs C110 — implementations MAY emit C117 before or after C110/C111 as long as SPDX/CDX field-order-canonicalization tests aren't broken.
- No compact numeric form ("K" for thousands, etc.) — plain integer only.
- No sign or leading-zero variants — value is `^0$|^[1-9][0-9]*$`.
