# Contract: Three new `mikebom:*` annotation shapes

**Feature**: 119-supplement-cdx
**Date**: 2026-06-13
**Consumed by**: consumers reading emitted SBOMs; parity-extractors; downstream tools
**Spec mapping**: FR-008, FR-009, FR-011, FR-012; Constitution Principle V audit per research.md § Decision 8

This contract defines the value shapes + emission gating + Principle V audit conclusions for the three new annotation keys this feature introduces. All three integrate via the existing `extra_annotations` channel + `metadata.properties[]` document-scope channel.

## Annotation 1 — `mikebom:source-tier = "declared"` (new value on existing C5 key)

**Scope**: per-component.
**CDX 1.6 carrier**: `components[].properties[]` entry with `name = "mikebom:source-tier"`, `value = "declared"`.
**SPDX 2.3 carrier**: `Package.annotations[]` entry wrapped in `MikebomAnnotationCommentV1` envelope, `field = "mikebom:source-tier"`, `value = "declared"`.
**SPDX 3.0.1 carrier**: `Annotation` graph-element targeting the component, same envelope shape as SPDX 2.3.

### Permitted values (post-this-feature)

| Value | Meaning | Source |
|---|---|---|
| `installed` | Pre-existing — scanner discovered via OS package DB or installed-metadata lookup | Pre-118 |
| `analyzed` | Pre-existing — scanner discovered via filesystem walker or binary analysis | Pre-118 |
| `source` | Pre-existing — scanner discovered via source-tree manifest reader | Pre-118 |
| `declared` | NEW — operator declared via `--supplement-cdx` supplement file | This feature |

The four values partition the source-tier space. Future values are reviewer-policed.

### Emission gating

- Present on EVERY component the supplement contributed (collision or solo per merge-pipeline.md).
- Absent on every component the scanner discovered without supplement intervention (scanner-only entries keep their existing `installed`/`analyzed`/`source` value).
- For collision entries (PURL matches both supplement and scanner): the value stays at the scanner's pre-existing source-tier; the `declared` annotation is NOT added (the component is fundamentally a scanner discovery; the supplement provided enrichment, not the original observation).

### Principle V audit (research.md § Decision 8 — C65 extension)

No native field expresses "this component was operator-declared, not scanner-observed" in CDX 1.6 / SPDX 2.3 / SPDX 3.0.1. Documented in `docs/reference/sbom-format-mapping.md` as a value-extension of the existing C5 row.

## Annotation 2 — `mikebom:supplement-cdx` (envelope-level provenance, new C66)

**Scope**: document-level (`metadata.properties[]` in CDX; `creationInfo` annotation in SPDX 2.3; envelope `Annotation` in SPDX 3).
**Cardinality**: exactly one per scan (one `--supplement-cdx` per v0.1).
**CDX 1.6 carrier**: `metadata.properties[]` entry with `name = "mikebom:supplement-cdx"`, `value = "<path>@sha256:<hex>"`.
**SPDX 2.3 carrier**: `creationInfo.annotations[]` entry, annotator = `Tool: mikebom-<version>`, annotationType = `OTHER`, comment = `mikebom:supplement-cdx=<path>@sha256:<hex>`.
**SPDX 3.0.1 carrier**: `Annotation` element on the `SpdxDocument`, annotationType = `other`, statement = same comment shape.

### Value shape

```text
<path>@sha256:<hex>
```

Where:
- `<path>` is the verbatim string the operator passed to `--supplement-cdx` (Decision 6 — NOT absolutized; matches operator's command-line input).
- `<hex>` is the lowercase hex of SHA-256 over the supplement file's raw bytes (64 hex characters).

Examples:
```text
supplement.cdx.json@sha256:5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8
/etc/mikebom/supplement.cdx.json@sha256:6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b
```

### Emission gating

- Present iff `--supplement-cdx <PATH>` was supplied on the command line.
- Absent (preserving byte-identity with pre-feature mikebom) when the flag was not supplied.
- Per Decision 3 — even when the supplement file is empty (zero components/services/dependencies), the annotation is still emitted; consumers see "a supplement was supplied" regardless of whether it had any effect.

### Principle V audit (research.md § Decision 8 — C66 new row)

NO native field expressing "this SBOM merged in a supplement file with hash X" exists in any of the three formats. Per Principle X (Transparency), operators reading the emitted SBOM MUST be able to detect supplement use; the annotation is the structured machine-parseable mechanism. New row added to `docs/reference/sbom-format-mapping.md`.

## Annotation 3 — `mikebom:assertion-conflict` (per-component conflict record, new C67)

**Scope**: per-component, REPEATABLE (one per conflicted field).
**CDX 1.6 carrier**: `components[].properties[]` entry with `name = "mikebom:assertion-conflict"`, `value = "<JSON-encoded object>"`.
**SPDX 2.3 carrier**: `Package.annotations[]` entry wrapped in `MikebomAnnotationCommentV1` envelope, `value = <JSON object>`.
**SPDX 3.0.1 carrier**: `Annotation` graph-element targeting the component, same envelope shape.

### Value shape (JSON object)

```json
{
  "field": "<conflicted-field-name>",
  "scanner_value": <JSON value>,
  "supplement_value": <JSON value>,
  "winner": "scanner" | "supplement",
  "justification": "developer-metadata-override" | "bytes-evident-detection-preserved"
}
```

Where:
- `field` is the conflicted CDX/SPDX field name (e.g., `"licenses"`, `"supplier"`, `"hashes"`).
- `scanner_value` and `supplement_value` are the values each side asserted, preserved verbatim as JSON.
- `winner` ∈ `{"scanner", "supplement"}` — derived from the FR-006 / FR-007 partition (research.md § Decision 3).
- `justification` ∈ the minimal 2-value enum committed in spec clarification Q3 (research.md § Decision 5).

### Cardinality + storage shape

A single component MAY have MULTIPLE conflicts (one per conflicted field). Because the in-process storage channel is `extra_annotations: BTreeMap<String, serde_json::Value>` (one value per string key), multiple conflicts on one component are stored as a **JSON ARRAY of conflict-record objects** under the single key `mikebom:assertion-conflict`:

```rust
extra_annotations.insert(
    "mikebom:assertion-conflict".to_string(),
    serde_json::Value::Array(vec![record_obj_1, record_obj_2, ...]),
);
```

The CDX emission shape is therefore ONE `properties[]` entry whose `value` is a JSON-encoded string of the array:

```json
{
  "name": "mikebom:assertion-conflict",
  "value": "[{\"field\":\"licenses\",...},{\"field\":\"hashes\",...}]"
}
```

Consumers `JSON.parse()` the value to enumerate. SPDX 2.3 / SPDX 3 wrap the same array in the existing `MikebomAnnotationCommentV1` envelope.

### Emission gating

- Present iff the supplement's PURL collided with a scanner-discovered component AND at least one field disagreed between the two.
- Absent when: (a) no supplement file was supplied; (b) supplement was supplied but no PURL collisions; (c) PURL collision but every field matched exactly.
- A collision with N conflicted fields produces N annotations on the resulting component.

### Worked example

Scanner observed:
```json
{ "purl": "pkg:cargo/opaque-lib@1.0.0", "licenses": [], "hashes": [{"alg": "SHA-256", "content": "deadbeef..."}] }
```

Supplement declared:
```json
{ "purl": "pkg:cargo/opaque-lib@1.0.0", "licenses": [{"license": {"id": "Apache-2.0"}}], "hashes": [{"alg": "SHA-256", "content": "cafebabe..."}] }
```

Merged emission carries ONE `mikebom:assertion-conflict` annotation whose value is a JSON-encoded array of two records:

```json
{
  "name": "mikebom:assertion-conflict",
  "value": "[{\"field\":\"licenses\",\"scanner_value\":[],\"supplement_value\":[{\"license\":{\"id\":\"Apache-2.0\"}}],\"winner\":\"supplement\",\"justification\":\"developer-metadata-override\"},{\"field\":\"hashes\",\"scanner_value\":[{\"alg\":\"SHA-256\",\"content\":\"deadbeef...\"}],\"supplement_value\":[{\"alg\":\"SHA-256\",\"content\":\"cafebabe...\"}],\"winner\":\"scanner\",\"justification\":\"bytes-evident-detection-preserved\"}]"
}
```

The component's `licenses[]` field carries `[{"license": {"id": "Apache-2.0"}}]` (developer won). The component's `hashes[]` field carries `[{"alg": "SHA-256", "content": "deadbeef..."}]` (scanner won). Consumers reading the SBOM see operator-authoritative licensing AND scanner-authoritative bytes-evidence simultaneously.

### Principle V audit (research.md § Decision 8 — C67 new row)

NO native field expresses "scanner observed X but operator declared Y; here's who won and why" in any of the three formats. CDX `evidence.identity` carries identification confidence + method, not conflict resolution. SPDX 2.3 `Annotation` is free-form prose, not structured records. SPDX 3.0.1's evidence-profile model doesn't cover conflict resolution in stable 3.0.1. Per Principle X (Transparency), every conflict resolution decision MUST be auditable; this annotation provides the structured machine-parseable record. New row added to `docs/reference/sbom-format-mapping.md`.

## Cross-link

Combined behavior of all three annotations:

1. **Consumer enumerates supplement-declared components**: grep `name = "mikebom:source-tier"` AND `value = "declared"` over `components[].properties[]`.
2. **Consumer enumerates supplement-induced conflicts**: grep `name = "mikebom:assertion-conflict"` over `components[].properties[]`.
3. **Consumer verifies supplement-file provenance**: read `mikebom:supplement-cdx` from `metadata.properties[]`, parse path + sha256, verify against operator's source-control supplement file.

The three annotations are designed to compose: a consumer can fully reconstruct "what did the operator declare; what did the scanner find; where did they conflict; what file fed the merge" without any out-of-band metadata.
