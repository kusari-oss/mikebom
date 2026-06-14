# Contract: `--supplement-cdx` file format

**Feature**: 119-supplement-cdx
**Date**: 2026-06-13
**Consumed by**: operators authoring supplement files; CMake-SBOM-Builder / custom-script generators
**Spec mapping**: FR-001, FR-002, FR-010, FR-014

This contract defines the file format `mikebom sbom scan --supplement-cdx <PATH>` accepts.

## File envelope

The supplement file MUST be a CDX 1.6 JSON document (CDX 1.4 and 1.5 are also accepted for backwards-compatibility per Decision 1). At minimum:

```json
{
  "bomFormat": "CycloneDX",
  "specVersion": "1.6",
  "components": [],
  "services": [],
  "dependencies": []
}
```

### Required envelope keys

| Key | Required? | Accepted values | Notes |
|---|---|---|---|
| `bomFormat` | yes | `"CycloneDX"` | Exact-match string |
| `specVersion` | yes | `"1.4"` / `"1.5"` / `"1.6"` | Per Decision 1; mikebom reads the same field set across all three |
| `components` | optional (default `[]`) | array of component entries | See § "Component entries" below |
| `services` | optional (default `[]`) | array of service entries | See § "Service entries" below |
| `dependencies` | optional (default `[]`) | array of dependency edges | See § "Dependency edges" below |

### Ignored envelope keys

Per FR-014 and Decision 1, mikebom IGNORES the following CDX 1.6 fields if present in the supplement:

- `metadata.component` — supplement's `metadata.component` is IGNORED per FR-014 + spec clarification Q1. `--scan-as` always wins.
- `metadata.timestamp`, `metadata.lifecycles`, `metadata.tools`, `metadata.authors`, `metadata.supplier`, `metadata.licenses`, `metadata.properties` — mikebom's own metadata generation owns these fields in the emitted SBOM.
- `serialNumber`, `version` — mikebom generates these for the emitted SBOM.
- `compositions`, `formulation`, `vulnerabilities`, `annotations`, `signature` — out of scope for v0.1 merge semantics.

Operators MAY include any of these fields in the supplement file for compatibility with their own tooling; mikebom silently ignores them at merge time. Future v2 milestones may extend the consumed subset.

## Component entries

Each entry in `components[]` describes one operator-declared component.

### Required fields

| Field | Type | Notes |
|---|---|---|
| `purl` | string (canonical PURL) | The join key for FR-010 merge against scanner output |

### Honored optional fields (developer can declare these)

| Field | Type | When wins per FR-006/FR-007 |
|---|---|---|
| `bom-ref` | string | Re-anchored at merge time; advisory |
| `name` | string | Developer wins on conflict |
| `version` | string | Scanner wins on conflict (bytes-derived) |
| `supplier.name` | string | Developer wins |
| `licenses[]` | CDX license array | Developer wins |
| `copyright` | string | Developer wins |
| `description` | string | Developer wins |
| `externalReferences[]` | CDX externalRef array | Developer wins on `website`/`documentation`/`distribution`; scanner wins on others |
| `hashes[]` | CDX hash array | Scanner wins on conflict (bytes-derived) |
| `cpe` | string (CPE 2.3 URI) | Scanner wins on conflict |

### Ignored fields (silently dropped at parse time)

`evidence`, `properties` (with mikebom-prefix), `pedigree`, `evidence.identity`, `evidence.callstack`, `evidence.occurrences`, `evidence.confidence`, `swid`, `signature`. These are scanner-domain fields; declarations don't apply.

### Component validation rules

1. `purl` MUST be present and MUST parse as a canonical PURL. Invalid PURLs cause non-zero exit per FR-002.
2. `purl` MUST be unique within the `components[]` AND `services[]` arrays of the same supplement file (per spec edge case 5).
3. Any field NOT in the "honored" list above is silently dropped at parse time. Operators who need a field that's not in the list should file an issue to expand the list — declaring an "unknown" field doesn't surface in the emitted SBOM.

## Service entries

Each entry in `services[]` describes one operator-declared service (SaaS dep, internal microservice, etc.).

### Required fields

| Field | Type | Notes |
|---|---|---|
| `name` | string | Required by CDX 1.6 services schema |

### Honored optional fields

| Field | Type | Notes |
|---|---|---|
| `bom-ref` | string | Used by dependencies block for cross-referencing |
| `provider.name` | string | Developer-declared service provider |
| `endpoints[]` | array of strings | URI / URL endpoints |
| `description` | string | Free-form |
| `licenses[]` | CDX license array | Operator-declared license on the service |
| `externalReferences[]` | CDX externalRef array | Documentation / API spec links |

### Service emission to non-CDX formats

Per Decision 4:
- **SPDX 2.3**: services project onto `packages[]` carrying `mikebom:component-role = "saas-service"` annotation (C40 pattern from milestone 049).
- **SPDX 3.0.1**: services project as `Bundle` + `Relationship[contains]` where supported; fall back to SPDX 2.3 projection pattern.

The CDX 1.6 path is the lossless native path. Operators emitting SPDX as their primary format are not penalized; services appear, just via a projection.

## Dependency edges

Each entry in `dependencies[]` declares one edge from a referencing component to one or more dependees.

### Required fields

| Field | Type | Notes |
|---|---|---|
| `ref` | string (bom-ref OR canonical PURL) | The source side of the edge |
| `dependsOn` | array of strings (bom-refs OR PURLs) | The target side(s) of the edge |

### Re-anchoring semantics

At merge time, mikebom re-anchors `ref` / `dependsOn` strings as follows:
- If the string matches a supplement-internal `bom-ref` of a supplement component/service that has a canonical PURL, the canonical PURL is used in the emitted edge.
- If the string matches a scanner-discovered component's canonical PURL, the scanner's bom-ref is used.
- If the string matches a supplement component/service that has no scanner-side counterpart, the supplement's bom-ref is preserved (operator owns identity).

Dangling `dependsOn` references (no match anywhere) cause non-zero exit per spec edge case 6.

## Validation summary

The hand-rolled structural validator at `mikebom-cli/src/supplement/parser.rs` enforces:

1. File reads as valid JSON.
2. Top-level `bomFormat == "CycloneDX"` AND `specVersion ∈ {"1.4", "1.5", "1.6"}`.
3. `components[]` / `services[]` / `dependencies[]` are arrays (when present).
4. Each component entry has a parsable `purl` (FR-010 join key).
5. Each service entry has a `name`.
6. Each dependency entry has a `ref` AND a `dependsOn` array.
7. PURL uniqueness within `components[]` ∪ `services[]`.
8. No dangling `dependsOn` references (enforced at merge time after the scanner side is also known).

Failures at any step → non-zero exit BEFORE any walker begins per FR-002. The error message names the supplement path and the specific failure.

## Worked example

Operator authors `supplement.cdx.json` for a Rust project that depends on Stripe + a vendored `liberror`:

```json
{
  "bomFormat": "CycloneDX",
  "specVersion": "1.6",
  "components": [
    {
      "type": "library",
      "bom-ref": "liberror-1.2.3",
      "purl": "pkg:generic/liberror@1.2.3",
      "name": "liberror",
      "version": "1.2.3",
      "supplier": { "name": "Acme Open Source Foundation" },
      "licenses": [ { "license": { "id": "MIT" } } ],
      "copyright": "© 2026 Acme"
    }
  ],
  "services": [
    {
      "bom-ref": "stripe-saas",
      "name": "Stripe",
      "provider": { "name": "Stripe, Inc." },
      "endpoints": [ "https://api.stripe.com" ]
    }
  ],
  "dependencies": [
    {
      "ref": "pkg:cargo/my-app@1.0.0",
      "dependsOn": [ "liberror-1.2.3", "stripe-saas" ]
    }
  ]
}
```

Operator invokes:
```bash
mikebom sbom scan --path . --supplement-cdx supplement.cdx.json --output sbom.cdx.json
```

Result: the emitted CDX 1.6 SBOM contains every scanner-discovered component (the operator's `my-app` Cargo project + its transitive deps) PLUS the `liberror` component as `mikebom:source-tier = "declared"` PLUS the Stripe service under `services[]`. The `dependencies[]` block carries an edge from `my-app` to both `liberror` and the Stripe service.
