# Data Model — Developer-asserted source-of-truth supplement (v0.1)

**Feature**: 119-supplement-cdx
**Date**: 2026-06-13

This feature introduces THREE new in-process entities + extends ONE existing field channel + adds THREE new annotation property contracts. No persisted entities — the supplement file is read once, parsed in-process, and consumed at merge time; the resulting state flows into the SBOM payload and dies with the scan process.

## Entity 1 — `Supplement` (loaded supplement file representation)

**Location**: New struct at `mikebom-cli/src/supplement/parser.rs`.

**Definition**:

```rust
pub(crate) struct Supplement {
    /// SHA-256 of the raw supplement file bytes (lowercase hex).
    /// Used by FR-012 `mikebom:supplement-cdx` provenance annotation.
    pub(crate) source_sha256: String,
    /// Verbatim path string the operator passed to `--supplement-cdx`.
    /// Preserved as-is — not normalized — so the `mikebom:supplement-cdx`
    /// provenance annotation matches what the operator typed (Decision 6).
    pub(crate) source_path: String,
    /// Parsed components (one entry per supplement-declared component).
    /// PURL-keyed at merge time per FR-010 (Decision 2).
    pub(crate) components: Vec<SupplementComponent>,
    /// Parsed services (CDX-native section that scanner doesn't populate).
    /// Flow straight through to the emitted SBOM's `services[]` via the
    /// new `build_services()` function (per plan.md § Source Code).
    pub(crate) services: Vec<SupplementService>,
    /// Parsed dependency edges; references are bom-refs OR purls.
    /// Re-anchored to scanner-side bom-refs at merge time per FR-005.
    pub(crate) dependencies: Vec<SupplementDependency>,
}
```

**Per-component entry**:

```rust
pub(crate) struct SupplementComponent {
    pub(crate) purl: mikebom_common::types::purl::Purl,
    pub(crate) bom_ref: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) supplier: Option<String>,
    pub(crate) licenses: Option<Vec<serde_json::Value>>,
    pub(crate) copyright: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) external_references: Option<Vec<serde_json::Value>>,
    pub(crate) hashes: Option<Vec<serde_json::Value>>,
    pub(crate) cpes: Option<Vec<String>>,
    /// Any additional fields we ignore for v0.1 but want to preserve
    /// for "stamp-as-declared annotation" purposes — stored as the
    /// original JSON value blob.
    pub(crate) raw_json: serde_json::Value,
}
```

**Lifecycle**:

1. **Construction** (`parser::load(path)`): reads bytes from disk, computes sha256, parses JSON, validates structural keys (Decision 1), constructs `Supplement` instance. Failures (I/O / JSON parse / structural validation) bubble up as `SupplementError` → non-zero exit per FR-002.
2. **Consumption** (`merge::merge(scanner_components, supplement)`): the `Supplement` is moved into `merge()`; its components are PURL-keyed against the scanner-discovered set; matches are routed through `conflict::resolve_component()`; misses become standalone `ResolvedComponent` entries with `mikebom:source-tier = declared`. The `services` and `dependencies` vecs are returned to the caller for downstream emission.
3. **Destruction**: implicit at end of merge step. The `source_sha256` + `source_path` are extracted by the caller and stamped onto the document-scope `mikebom:supplement-cdx` annotation per FR-012 before the `Supplement` drops.

**Invariants**:

1. **Source path preserved verbatim**: the `source_path` field is NEVER absolutized or canonicalized post-parse. The annotation must match what the operator typed.
2. **PURL uniqueness within file**: per spec edge case 5, the supplement MUST contain no duplicate PURLs across `components[]` AND `services[]`. The parser enforces this at construction time.
3. **No dangling references**: per spec edge case 6, every `dependencies[].dependsOn[]` entry MUST reference a bom-ref or PURL that exists somewhere (supplement or scanner output). The merge step verifies this and fails closed if violated.
4. **Constructed only via `parser::load()`**: there is no public constructor; consumers must go through the validating loader.

## Entity 2 — `ConflictRecord` (conflict-resolution audit trail)

**Location**: New struct at `mikebom-cli/src/supplement/conflict.rs`.

**Definition**:

```rust
pub(crate) struct ConflictRecord {
    /// Which CDX/SPDX field had a divergent value.
    pub(crate) field: ConflictField,
    /// JSON-encoded value from the scanner side.
    pub(crate) scanner_value: serde_json::Value,
    /// JSON-encoded value from the supplement side.
    pub(crate) supplement_value: serde_json::Value,
    /// Who won per the FR-006 / FR-007 partition.
    pub(crate) winner: ConflictWinner,
}

pub(crate) enum ConflictField {
    Hashes,
    Cpe,
    Purl,
    Version,
    BinaryRole,
    Licenses,
    ConcludedLicenses,
    Supplier,
    Copyright,
    Name,
    Description,
    ExternalReferences,
    /// Any field not in either authoritative set; scanner wins by default
    /// per FR-015 safety property + Decision 3 catch-all.
    Other(String),
}

pub(crate) enum ConflictWinner {
    Scanner,
    Supplement,
}
```

**Emission**:

For each `ConflictRecord` produced during merge, a `mikebom:assertion-conflict` annotation is stamped on the resulting `ResolvedComponent`'s `extra_annotations` BTreeMap with the JSON object shape from research.md § Decision 5:

```json
{
  "field": "licenses",
  "scanner_value": [],
  "supplement_value": [{"license": {"id": "Apache-2.0"}}],
  "winner": "supplement",
  "justification": "developer-metadata-override"
}
```

A single component may emit multiple `mikebom:assertion-conflict` annotations (one per conflicted field). Per Decision 5, the annotation key NAME stays the same across all conflicts on one component; consumers enumerate via array iteration over `properties[]`.

**Invariants**:

1. **`winner` derives mechanically from `field`** per the FR-006/FR-007 partition:
   - `Hashes | Cpe | Purl | Version | BinaryRole | Other(_)` → `Scanner`
   - `Licenses | ConcludedLicenses | Supplier | Copyright | Name | Description | ExternalReferences` → `Supplement`
   - No `winner` field on `ConflictRecord` could in principle drift from the partition; the field is derived, not stored independently.
2. **JSON values are preserved verbatim**: `scanner_value` and `supplement_value` are exactly what each side had pre-merge. Consumers can audit "what did the scanner observe" by inspecting the annotation.

## Entity 3 — `MergeOutcome` (merge step's structured return)

**Location**: New struct at `mikebom-cli/src/supplement/merge.rs`.

**Definition**:

```rust
pub(crate) struct MergeOutcome {
    /// The augmented component set: scanner-discovered + supplement-declared,
    /// with conflicts resolved per FR-006/FR-007.
    pub(crate) components: Vec<mikebom_common::resolution::ResolvedComponent>,
    /// New services array (CDX-native; no scanner-side equivalent).
    pub(crate) services: Vec<SupplementService>,
    /// Merged dependency edges (scanner + supplement, with supplement-side
    /// bom-refs re-anchored to scanner-side bom-refs where PURLs collide).
    pub(crate) dependencies: Vec<mikebom_common::resolution::RelationshipEdge>,
    /// Document-scope provenance for the FR-012 annotation.
    pub(crate) supplement_provenance: SupplementProvenance,
    /// Conflict records (already stamped onto components' extra_annotations).
    /// Returned to the caller for any test-time inspection.
    pub(crate) conflicts: Vec<ComponentConflicts>,
}

pub(crate) struct SupplementProvenance {
    pub(crate) source_path: String,
    pub(crate) source_sha256: String,
}

pub(crate) struct ComponentConflicts {
    pub(crate) component_purl: mikebom_common::types::purl::Purl,
    pub(crate) records: Vec<ConflictRecord>,
}
```

**Lifecycle**:

1. **Construction** (`merge::merge(scanner_components, supplement)`): returns a `MergeOutcome` after fully resolving all conflicts. Pure function — no side effects.
2. **Consumption** by the CDX/SPDX builders at `generate/cyclonedx/builder.rs:355`:
   - `components` feeds `build_components()` as the augmented input
   - `services` feeds the new `build_services()` function
   - `dependencies` feeds the existing `build_dependencies()` augmented input
   - `supplement_provenance` is read by `metadata.rs` to emit the `mikebom:supplement-cdx` document-scope property per FR-012
3. **Destruction**: implicit when the builders complete and the `MergeOutcome` drops.

**Invariants**:

1. **`components.len() >= scanner_components.len()`** at all times. The merge never reduces the component set (FR-015 safety property — supplement assertions cannot suppress scanner detection).
2. **Every supplement-only component carries `mikebom:source-tier = "declared"`**: PURLs from the supplement that didn't collide with scanner output are appended to `components` with the new source-tier value (per FR-011).
3. **Every scanner-supplement collision produces a single merged entry**: there is never a duplicate of the same canonical PURL in the output. The merge is PURL-exact-collapsing.

## Entity 4 — `extra_annotations` channel extension (no new entity, new keys)

**Location**: Existing field on `ResolvedComponent` at `mikebom-common/src/resolution.rs:217-218`.

**Definition** (pre-existing):

```rust
pub extra_annotations: BTreeMap<String, serde_json::Value>,
```

**New keys this feature stamps**:

| Key | Scope | Value shape | Emission gating |
|---|---|---|---|
| `mikebom:source-tier` | per-component | `"declared"` (new value on existing C5 key) | Only on supplement-declared components |
| `mikebom:assertion-conflict` | per-component (repeatable) | `{ field, scanner_value, supplement_value, winner, justification }` JSON object | One per conflict; same component may have many |
| `mikebom:supplement-cdx` | document-scope (`metadata.properties[]`) | `"<path>@sha256:<hex>"` | Only when `--supplement-cdx` is in effect |

**Emission path**: existing serialization at `generate/cyclonedx/builder.rs:965-973` (per-component properties) and `generate/cyclonedx/metadata.rs:106-160` (document-scope properties) automatically renders these into the emitted CDX output. SPDX 2.3 path uses the existing `MikebomAnnotationCommentV1` envelope (same as milestone 116/118). SPDX 3.0.1 path uses graph-element annotations.

## Validation rules summary

| Rule | Source | Where enforced |
|---|---|---|
| Supplement parse-failure → non-zero exit | FR-002 / Constitution III | `parser::load()` returns `Err` → propagates to `scan_cmd.rs` |
| Bytes-derived field set: scanner wins | FR-006 / Decision 3 | `conflict::SCANNER_AUTHORITATIVE_FIELDS` constant + `resolve_component()` partition logic |
| Metadata field set: developer wins | FR-007 / Decision 3 | `conflict::DEVELOPER_AUTHORITATIVE_FIELDS` constant + `resolve_component()` partition logic |
| Catch-all default: scanner wins | FR-015 / Decision 3 | `resolve_component()` default branch |
| PURL exact-match for merge | FR-010 / clarification Q2 | `merge::build_purl_index()` + `HashMap::get()` |
| Supplement-declared components stamped `mikebom:source-tier = declared` | FR-011 | `merge::merge()` post-loop for non-collision entries |
| `mikebom:assertion-conflict` annotation on every conflict | FR-008 + FR-009 | `conflict::resolve_component()` returns `Vec<ConflictRecord>` → stamped onto `extra_annotations` |
| Document-scope `mikebom:supplement-cdx` provenance | FR-012 | `metadata.rs` reads `MergeOutcome.supplement_provenance` |
| `--scan-as` always wins over supplement `metadata.component` | FR-014 / clarification Q1 | `parser::load()` discards supplement's `metadata.component` field at parse time; never propagated downstream |
| Supplement byte-identity preserved when flag absent | FR-013 / SC-006 | scan_cmd.rs short-circuits the merge step when `--supplement-cdx` is `None` |
| No suppression of scanner-detected components | FR-015 | `merge::merge()` post-condition assertion `merged.components.len() >= scanner.len()` |

No state transitions (no lifecycle FSM); the supplement is load-once, merge-once, emit-once-per-format within a single scan.
