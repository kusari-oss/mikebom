# Data Model: SBOM auto-split

**Feature**: 215-sbom-auto-split
**Date**: 2026-07-22

## E1 — `SubprojectRoot` (in-memory, boundary enumeration output)

**Location**: `waybill-cli/src/generate/split.rs`
**Purpose**: Represents one detected workspace-member that becomes the axis for one sub-SBOM.

```rust
#[derive(Debug, Clone)]
pub(crate) struct SubprojectRoot {
    /// Canonical PURL identifying the subproject. Drives filename slug
    /// (R3), manifest `subproject_id` field, and serves as the BFS seed
    /// for dep-graph projection (R2).
    pub purl: Purl,
    /// Component's `bom-ref` — the graph-node identity used by
    /// `Relationship.from` / `Relationship.to` edges. Distinct from
    /// PURL (bomref can be namespaced; PURL is the wire identity).
    pub bomref: String,
    /// Source directory relative to scan_root — recorded for manifest
    /// `source_dir` field + collision-disambiguation (R3 falls back to
    /// hash-of-relpath when two subprojects share a slug).
    pub source_dir: PathBuf,
    /// Ecosystem name (`cargo`, `npm`, `pypi`, `maven`, `go`, `gem`,
    /// `swift`, `generic`, etc.) — derived from `purl.ecosystem()`.
    /// Appears in filename per `<slug>.<ecosystem>.<format>.json`.
    pub ecosystem: String,
}
```

**Invariants**:
- `purl.name` non-empty. Enumeration filters out any main-module with a placeholder-empty PURL (m127 emits synthetic PURLs occasionally; those are NOT split axes).
- `source_dir` relative to `scan_root` (not absolute — makes manifest cross-host stable).
- `ecosystem` non-empty. Falls back to `"generic"` when the PURL type is unknown.

**Enumeration source (R1)**: iterate `resolved_components: Vec<ResolvedComponent>`, filter to those with `extra_annotations["waybill:is-workspace-root"] == true`, project into `SubprojectRoot` structs. Result is a `Vec<SubprojectRoot>` sorted lexicographically by `subproject_id` for stable emit order.

## E2 — `SplitProjection` (in-memory, BFS result per subproject)

**Location**: `waybill-cli/src/generate/split.rs`
**Purpose**: The scope-narrowed component + relationship set for one subproject. Fed to the existing emit pipeline unchanged.

```rust
#[derive(Debug)]
pub(crate) struct SplitProjection {
    pub root: SubprojectRoot,
    /// Reachable component set from `root.bomref` over dep-edge relationships (R2).
    /// Includes root itself. Order preserved from source `resolved_components`.
    pub components: Vec<ResolvedComponent>,
    /// Relationships where BOTH endpoints are in `components` (per BFS).
    pub relationships: Vec<Relationship>,
    /// Diagnostic count: how many `components` also appear in ≥1 other
    /// subproject's projection. Populated after all projections computed
    /// (needs cross-subproject visibility). Fed into manifest per R4.
    pub shared_deps_count: usize,
}
```

**Invariants**:
- `components[0]` is the root (BFS seed). Downstream emit code relies on this ordering to place the root at `metadata.component` / `describes` position.
- Every `Relationship` in `relationships` has `from` and `to` both present in `components` (self-contained per FR-007).
- `shared_deps_count` computed AFTER all N projections done (one linear pass over the union).

## E3 — `SplitManifest` (on-wire, split-manifest.json)

**Location**: `waybill-cli/src/generate/split_manifest.rs`
**Purpose**: Operator-facing artifact describing the split. Serialized as JSON per R4 schema.

```rust
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SplitManifest {
    #[serde(rename = "$schema")]
    pub schema_url: String,          // "https://waybill.dev/schema/split-manifest/v1.json"
    pub waybill_version: String,     // env!("CARGO_PKG_VERSION")
    pub scan_root: String,           // absolute or repo-relative
    pub generated_at: String,        // RFC 3339 timestamp
    pub total_unique_components: u64,
    pub shared_dep_count: u64,
    pub entries: Vec<SplitEntry>,    // sorted by subproject_id for determinism
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SplitEntry {
    pub subproject_id: String,       // "<slug>.<ecosystem>" — filename prefix
    pub root_purl: String,           // full PURL string
    pub source_dir: String,          // relative to scan_root
    pub component_count: u64,
    pub shared_deps_count: u64,
    pub files: BTreeMap<String, String>,  // format-name → relative-filename
}
```

**Invariants**:
- `entries` sorted lexicographically by `subproject_id` (deterministic output).
- `files` uses `BTreeMap` (stable key order for byte-identical output across runs).
- Under `WAYBILL_FIXED_TIMESTAMP`, `generated_at` equals the fixed value.
- `schema_url` is a stable v1 pin; future breaking schema changes bump the version segment.

## E4 — Sub-SBOM (on-wire, per-subproject SBOM files)

**Location**: emitted by existing `waybill-cli/src/generate/{cyclonedx,spdx,spdx3}/` code paths — no new module. Split just narrows the input set.

**Semantics** (per FR-004 + FR-015):
- `metadata.component` (CDX 1.6) / `describes` (SPDX 2.3) / root Element (SPDX 3) identifies the subproject's `SubprojectRoot.purl`. NOT the whole-repo root.
- `components[]` (CDX) / `packages[]` (SPDX) contains exactly `SplitProjection.components`.
- `dependencies[]` / `relationships[]` contains exactly `SplitProjection.relationships`.
- Every annotation, evidence block, license expression, hash algorithm that the pre-feature single-SBOM would emit for THAT SUBSET is preserved.
- `serialNumber` is deterministic per subproject under `WAYBILL_FIXED_TIMESTAMP` per R5.

**Invariants**:
- Each sub-SBOM independently validates against its format's JSON schema (SC-006 gate via existing schema-validation tests + `spdx3-validate` CI).
- Under `WAYBILL_FIXED_TIMESTAMP`, byte-identical across two runs of the same scan (SC-008).

## E5 — `ScanArgs.split` (CLI flag)

**Location**: `waybill-cli/src/cli/scan_cmd.rs`

```rust
/// Milestone 215 — emit one SBOM per detected workspace member instead
/// of one combined SBOM. Requires --output-dir; incompatible with
/// --output. See docs/user-guide/cli-reference.md#split.
#[arg(long)]
pub split: bool,
```

**Interaction rules** (per R7):
- `--split` set + `--output-dir` set → emit N × M sub-SBOMs + 1 manifest into `--output-dir`.
- `--split` set + `--output-dir` unset + `--output` unset → default to `--output-dir=./waybill-split-<timestamp>/`.
- `--split` set + `--output` set → **HARD ERROR** at CLI-parse time with fix-suggestion.
- `--split` unset → pre-feature single-SBOM behavior, entirely unchanged.

## Cross-entity flow (per-scan lifecycle)

```
[Scan resolves → Vec<ResolvedComponent> + Vec<Relationship>]
             │
             │  R1: enumerate workspace roots via
             │      waybill:is-workspace-root annotation
             ▼
[Vec<SubprojectRoot>]
             │
             │  R2: for each root, BFS project the reachable
             │      subset of components + relationships
             ▼
[Vec<SplitProjection>]  ← shared_deps_count populated after all N done
             │
             │  R6: for each projection × each output format,
             │      run existing emit pipeline against the subset
             ▼
[N × M sub-SBOM files on disk]
             │
             │  Manifest emission: aggregate SplitProjection metadata
             │      + file-list into SplitManifest struct, serialize as JSON
             ▼
[split-manifest.json on disk]
             │
             │  R8: if N == 0 (zero-boundary fallback), skip split logic;
             │      emit ONE pre-feature-shape SBOM + WARN log; no manifest.
             │
             │  R7: validate --output-dir vs --output at CLI-parse time.
             ▼
[Exit 0]
```
