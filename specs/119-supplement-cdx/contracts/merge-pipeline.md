# Contract: Merge pipeline

**Feature**: 119-supplement-cdx
**Date**: 2026-06-13
**Consumed by**: the CDX/SPDX builders at `generate/cyclonedx/builder.rs:355` and equivalent SPDX entry points; future-PR contributors extending the merge semantics
**Spec mapping**: FR-003, FR-004, FR-005, FR-006, FR-007, FR-010, FR-011, FR-013, FR-015

This contract defines the externally observable behavior of the `supplement::merge()` step.

## Pipeline position

```text
clap --supplement-cdx <PATH>
        │
        ▼
parser::load(path) ──► Supplement struct (in-memory)
        │
        ▼
scanner discovery (filesystem walkers) ──► Vec<ResolvedComponent>
        │
        ▼
dedup pipeline (milestone 105 SourceMechanism) ──► deduped Vec<ResolvedComponent>
        │
        ▼
enrichment (license / CPE / etc.) ──► enriched Vec<ResolvedComponent>
        │
        ▼
supplement::merge(scanner_components, supplement)  ◄── NEW STAGE (this feature)
        │
        ▼
MergeOutcome { components, services, dependencies, supplement_provenance, conflicts }
        │
        ▼
generate::cyclonedx::builder::{build_components, build_services, build_dependencies}
        │
        ▼
emit CDX/SPDX 2.3/SPDX 3 output
```

Merge runs ONCE per scan, immediately before the CDX builder's `build_components()` call at `mikebom-cli/src/generate/cyclonedx/builder.rs:355`. The output `MergeOutcome` feeds all three format builders identically.

## Function signature

```rust
pub(crate) fn merge(
    scanner_components: Vec<mikebom_common::resolution::ResolvedComponent>,
    scanner_dependencies: Vec<mikebom_common::resolution::RelationshipEdge>,
    supplement: Supplement,
) -> Result<MergeOutcome, SupplementError>;
```

Returns `MergeOutcome` (see data-model.md § Entity 3) on success. Returns `Err(SupplementError)` on (a) dangling `dependsOn` references in the supplement's `dependencies[]` block (per spec edge case 6); (b) any safety-property violation that the merge detects.

## Pre/post invariants

### Pre-conditions

1. **`scanner_components` has been deduped**: milestone 105's `SourceMechanism` dedup has run; no two entries in `scanner_components` share a canonical PURL.
2. **`supplement` has been structurally validated**: the parser has rejected malformed files; the components/services arrays carry the keys mikebom consumes.
3. **`scanner_components` is mutable**: merge MAY add to it (supplement-only entries become new `ResolvedComponent`s) but MUST NOT remove from it (FR-015).

### Post-conditions

1. **No component removed**: `merge_outcome.components.len() >= scanner_components.len()` (FR-015 safety property). Verified by an assertion in `merge::merge()` and a regression test in `supplement_cdx_integration.rs`.
2. **Every supplement-only entry stamped `mikebom:source-tier = "declared"`**: PURLs in the supplement that don't collide with scanner output become new components with this annotation (FR-011).
3. **PURL uniqueness preserved**: no two entries in `merge_outcome.components` share a canonical PURL. The merge collapses collisions into one entry.
4. **Every conflicted field annotated**: per FR-008, every field-level disagreement between scanner and supplement produces one `mikebom:assertion-conflict` annotation on the resulting component.
5. **Services emit only from the supplement**: `merge_outcome.services` is derived entirely from `supplement.services`; scanner produces no services in v0.1.
6. **Dependency edges re-anchored**: supplement's `dependencies[]` references are resolved to scanner-side bom-refs where PURLs collide; otherwise preserved as the supplement's bom-refs. Dangling references → `Err`.
7. **Document-scope provenance preserved**: `merge_outcome.supplement_provenance` carries the supplement's source path + sha256 for the FR-012 `mikebom:supplement-cdx` annotation.

## Conflict resolution algorithm

For each supplement component whose PURL matches a scanner component (the COLLISION case):

```text
for field in {licenses, supplier, copyright, name, description, externalReferences, ...}:
    if scanner_side[field] == supplement_side[field]:
        emit as-is (no conflict)
    else:
        if field in SCANNER_AUTHORITATIVE_FIELDS:
            winner = scanner
            emit scanner's value as primary
            emit supplement's value as `mikebom:declared-{field}` annotation
        elif field in DEVELOPER_AUTHORITATIVE_FIELDS:
            winner = supplement
            emit supplement's value as primary
            emit scanner's value as `mikebom:scanner-discovered-{field}` annotation
        else:
            # FR-015 safety default — scanner wins for unknown fields
            winner = scanner
            emit scanner's value as primary
            emit supplement's value as `mikebom:declared-{field}` annotation
        record ConflictRecord(field, scanner_value, supplement_value, winner)

for each ConflictRecord:
    stamp `mikebom:assertion-conflict` annotation with the shape from contracts/annotation-shape.md
```

For each supplement component whose PURL does NOT match any scanner component (the SOLO case):

```text
construct new ResolvedComponent with all supplement-declared fields
stamp `mikebom:source-tier = "declared"` annotation
append to merge_outcome.components
```

For each scanner component whose PURL does NOT match any supplement entry (the SCANNER-ONLY case):

```text
emit unchanged — supplement makes no claim about this component
```

## Field-set membership tables

### `SCANNER_AUTHORITATIVE_FIELDS` (scanner wins; FR-006)

| Field | CDX 1.6 source | Why scanner wins |
|---|---|---|
| `hashes` | `components[].hashes[]` | Bytes-derived — scanner read the file |
| `cpe` | `components[].cpe` | Bytes-derived — scanner extracted via fingerprint or vendor mapping |
| canonical `purl` | `components[].purl` | THIS IS the merge join key; cannot differ within a pair |
| `version` | `components[].version` | Bytes-derived when scanner detected via embedded version string |
| `binary_role` | `components[].properties[].mikebom:binary-role` | Milestone-104 fingerprint-derived classification |

### `DEVELOPER_AUTHORITATIVE_FIELDS` (developer wins; FR-007)

| Field | CDX 1.6 source | Why developer wins |
|---|---|---|
| `licenses` | `components[].licenses[]` | Operator-known authoritative license info; scanner heuristic |
| `concluded_licenses` | `mikebom:concluded-licenses` annotation | Operator's curation overlay (ClearlyDefined-style) |
| `supplier` | `components[].supplier` | Operator-known commercial/upstream context |
| `copyright` | `components[].copyright` | Operator-known authoritative copyright text |
| `name` (display) | `components[].name` | Operator's display name (PURL identity unchanged per scanner) |
| `description` | `components[].description` | Operator-domain free-form prose |
| `externalReferences` (ALL types) | `components[].externalReferences[]` (full array) | Operator-supplied URLs are operator-domain regardless of reference type (`website` / `documentation` / `distribution` / `vcs` / `mailing-list` / `issue-tracker` / etc.) |

### Catch-all default

Any field NOT in either set → SCANNER WINS (FR-015 safety default). This is the conservative choice; future PRs adding fields to either set are reviewer-policed.

## Error semantics

| Error class | Cause | Behavior |
|---|---|---|
| `SupplementError::Io(path, source)` | Supplement file unreadable | Non-zero exit BEFORE any walker; error names path and io error |
| `SupplementError::ParseJson(path, msg)` | File parses as invalid JSON | Non-zero exit; error names path and serde_json error |
| `SupplementError::ValidationFailed(path, reason)` | Structural validation fails | Non-zero exit; error names path and the specific failure (e.g., "components[3] missing required key `purl`") |
| `SupplementError::DuplicatePurl(purl)` | Multiple supplement entries share canonical PURL | Non-zero exit at parse time |
| `SupplementError::DanglingDependsOn(ref_str)` | A `dependsOn` references neither supplement nor scanner | Non-zero exit at merge time |

Per Constitution Principle III and FR-002, no partial SBOM is ever emitted on these errors. The scan exits before walker initialization.

## Backwards compatibility

When `--supplement-cdx` is NOT supplied:

- The parser is never invoked.
- The merge step is never invoked.
- The CDX builder runs identical code paths to pre-118 mikebom.
- The emitted SBOM is byte-identical to pre-118 (modulo random `serialNumber` and timestamp fields).

When `--supplement-cdx` IS supplied with an EMPTY supplement (zero components/services/dependencies):

- The parser succeeds.
- The merge step runs but is a no-op (no PURLs to merge; no services to add; no dependencies to re-anchor).
- The `mikebom:supplement-cdx` document-scope annotation is still emitted per FR-012 (so consumers know a supplement was supplied even if it had no effect).
- All component-level annotations are absent (no `mikebom:source-tier = declared`, no `mikebom:assertion-conflict`).
- The emitted SBOM components/services/dependencies match the no-flag case byte-for-byte except for the metadata.properties addition.
