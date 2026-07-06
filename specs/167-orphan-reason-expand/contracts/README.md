# Contracts — milestone 167

**Feature**: [spec.md](../spec.md) | **Plan**: [plan.md](../plan.md) | **Data model**: [data-model.md](../data-model.md)

Milestone 167 does NOT introduce new external contracts. It extends the value vocabulary of the existing `mikebom:orphan-reason` C45 annotation from 2 codes to 5. The wire-format shape is unchanged across CDX 1.6, SPDX 2.3, and SPDX 3.0.1.

## C45 vocabulary — post-167

The C45 parity-catalog row for `mikebom:orphan-reason` (at `mikebom-cli/src/parity/extractors/`) documents the following authoritative value set after this milestone lands:

| Code | Meaning | Emit tier | Ecosystem | Milestone |
|------|---------|-----------|-----------|-----------|
| `stale-go-sum-entry` | Go module BFS-unreachable from `metadata.component.purl`; a same-name sibling with a different version IS reachable. Indicates `go.sum` retains an entry a module upgrade orphaned. | Emit-time (m167 `orphan_reason.rs`) | `pkg:golang/*` | 167 (NEW) |
| `dead-lockfile-entry` | npm package BFS-unreachable from `metadata.component.purl`; a same-name sibling with a different version IS reachable. Indicates lockfile retains an entry a package upgrade orphaned. | Emit-time (m167 `orphan_reason.rs`) | `pkg:npm/*` | 167 (NEW) |
| `hoisted-unused` | npm package BFS-unreachable from `metadata.component.purl`; NO same-name reachable sibling exists. Indicates hoisted-but-declared-only dependency (pnpm phantom scenario surviving m164). | Emit-time (m167 `orphan_reason.rs`) | `pkg:npm/*` | 167 (NEW) |
| `unresolved-indirect-require` | Go module referenced by `// indirect` in `go.mod` but not resolvable via graph fallback. | Go-reader-time (m061 `legacy.rs:2091`) — preserved. Emit-time classifier does not overwrite. | `pkg:golang/*` | 061 (preserved) |
| `flat-attached-fallback` | Go module attached at flat-fallback layer during transitive resolution. Technically BFS-reachable via the fallback edge; documented for observability. | Go-reader-time (m061 `legacy.rs:2118`) — preserved. Emit-time classifier explicitly skips overwrite (data-model.md E3). | `pkg:golang/*` | 061 (preserved) |

**Frozen guarantee**: once a code appears in this table, its `as_str()` value is never renamed. SBOM consumers key on the literal string values.

## Wire-format examples

The annotation shape is unchanged from milestone 061; only the value set expands.

### CycloneDX 1.6

```json
{
  "type": "library",
  "bom-ref": "pkg:npm/lodash@4.17.20",
  "purl": "pkg:npm/lodash@4.17.20",
  "name": "lodash",
  "version": "4.17.20",
  "properties": [
    { "name": "mikebom:orphan-reason", "value": "dead-lockfile-entry" }
  ]
}
```

### SPDX 2.3

```json
{
  "SPDXID": "SPDXRef-Package-npm-lodash-4-17-20",
  "name": "lodash",
  "versionInfo": "4.17.20",
  "annotations": [
    {
      "annotator": "Tool: mikebom",
      "annotationType": "OTHER",
      "annotationDate": "2026-07-06T00:00:00Z",
      "comment": "mikebom:orphan-reason=dead-lockfile-entry"
    }
  ]
}
```

### SPDX 3.0.1

```json
{
  "spdxId": "https://mikebom.kusari.dev/annotation/...",
  "type": "Annotation",
  "creationInfo": "_:creationInfo0",
  "subject": "https://mikebom.kusari.dev/Package/npm/lodash-4-17-20",
  "annotationType": "other",
  "statement": "mikebom:orphan-reason=dead-lockfile-entry"
}
```

## Validation

- **CDX**: property with `name == "mikebom:orphan-reason"` and `value` matching one of the 5 vocabulary codes.
- **SPDX 2.3**: annotation with `comment` matching regex `^mikebom:orphan-reason=(stale-go-sum-entry|dead-lockfile-entry|hoisted-unused|unresolved-indirect-require|flat-attached-fallback)$`.
- **SPDX 3.0.1**: `Annotation` element with `statement` matching the same regex.

## Non-goals

- Milestone 167 does NOT expand the vocabulary to non-Go/npm ecosystems (Q3 clarification: FR-006 forbids orphan-reason on non-Go/npm components).
- Milestone 167 does NOT introduce a general-purpose `other-orphan` fallback code (research R8).
- Milestone 167 does NOT change the C45 emit-tier for `unresolved-indirect-require` or `flat-attached-fallback` — those remain Go-reader-time emissions to preserve m061 backward-compatibility (research R3).
