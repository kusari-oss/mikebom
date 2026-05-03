# Data Model: Cargo source-tree main-module component

## Entities

### CargoMainModuleEntry (new conceptual entity, no new Rust type)

A `PackageDbEntry` (existing struct in `mikebom-cli/src/scan_fs/package_db/mod.rs`) constrained to represent a synthetic main-module emitted by the cargo source-tree reader.

| Field | Value | Source | FR |
|-------|-------|--------|-----|
| `purl` | `pkg:cargo/<name>@<resolved-version>` | `[package].name` + `[package].version` (or workspace-inherited) | FR-001 |
| `name` | `[package].name` verbatim | `Cargo.toml` | FR-001 |
| `version` | resolved version (literal / workspace-inherited / `0.0.0-unknown`) | manifest resolver | FR-001 |
| `source` | `Some("path+file://<absolute-cargo-toml-dir>")` | filesystem walker | (existing convention) |
| `lifecycle_scope` | `None` | n/a (Runtime by default) | (out of scope) |
| `sbom_tier` | `Some("source")` | constant | FR-006 |
| `extra_annotations` | `BTreeMap` containing `mikebom:component-role: "main-module"` | constant | FR-004 |
| `parent_purl` | `None` | constant (top-level) | FR-001a |
| `depends` | `Vec<String>` of direct-dep PURLs from `[dependencies]` / `[dev-dependencies]` / `[build-dependencies]`, scope-filtered | manifest tables, post existing scope filter | FR-007 |
| `licenses` | `vec![]` (empty) | constant | FR-005 |
| `hashes` | `vec![]` (empty) | constant — main-module is a synthetic component, no file hashes | (n/a) |

### WorkspaceContext (new private helper struct)

In-scan-built map populated by an upfront pass over discovered `Cargo.toml` files. Resolves `version.workspace = true` to the workspace root's `[workspace.package].version`.

```rust
struct WorkspaceContext {
    /// Maps absolute path of a workspace-root Cargo.toml dir → that workspace's
    /// [workspace.package].version (if declared).
    versions: HashMap<PathBuf, String>,
}
```

Construction: scan all `Cargo.toml` files; for each one with `[workspace.package].version` set, insert `(parent_dir, version)`. Lookup is by walking up from a member's `Cargo.toml` until a key matches, stopping at the scan root.

If lookup fails (no enclosing workspace root within the scan boundary, OR workspace root has `[workspace]` but no `[workspace.package].version`), the resolver returns the literal `0.0.0-unknown` placeholder per FR-001 + Assumption A2.

### DroppedDuplicate (new private helper struct)

```rust
struct DroppedDuplicate {
    purl: Purl,
    kept_path: PathBuf,
    dropped_path: PathBuf,
}
```

Returned in batch from `dedup_main_modules_by_purl(&mut Vec<PackageDbEntry>)` so the caller emits one consolidated `tracing::warn!` per scan rather than per-collision (cleaner operator output for repos with many collisions).

## Relationships

### Direct-dep edges from main-module to dep targets

Each cargo main-module emits direct-dep edges into the existing relationship graph:

```text
Relationship {
    from: <cargo-main-module-purl>,           // e.g. pkg:cargo/mikebom@0.1.0-alpha.11
    to: <dep-target-purl>,                    // e.g. pkg:cargo/serde@1.0.193
    relationship_type: DependsOn,
    provenance: {
        source: "<absolute-Cargo.toml-path>",
        data_type: "cargo-manifest-direct-dep",
    },
}
```

These replace pre-064 edges that used the synthetic `DocumentRoot-*` placeholder as `from` (FR-007). Existing edge-emission machinery in `scan_fs/mod.rs` picks them up unchanged once the `from` PURL is correct.

### DESCRIBES relationship (document → main-module)

Emitted by the existing SPDX 2.3 `build_document::root_id` algorithm, case 1 (single top-level) or case 3 (multiple top-levels → super-root DESCRIBES each), which already supports multi-target DESCRIBES from the milestone-053 polyglot work. No code change required to the document-builder for the cargo case.

### Path-dep edges between workspace members (FR-011)

When member A's `Cargo.toml` declares `b = { path = "../b" }`, the existing edge-emission emits a `DependsOn` edge from A's direct-dep set to whatever component `b`'s manifest resolves to. With this milestone, that target IS member B's main-module component (same `pkg:cargo/b@<version>` PURL). No special handling — both endpoints exist as real components.

## State transitions

None — main-module emission is read-only and deterministic. There are no lifecycles, no mutations after emission.

## Validation rules

| Rule | Source | Failure mode |
|------|--------|--------------|
| `[package].name` and `[package].version` MUST both be present in any `Cargo.toml` to emit a main-module | FR-001 | If either missing, skip emission (no main-module for that file); not an error. |
| `version.workspace = true` MUST resolve to a `[workspace.package].version` in an enclosing workspace root | FR-001 + A2 | Falls back to literal `0.0.0-unknown` if resolution fails. Not an error; deterministic. |
| Same-PURL emissions MUST be deduplicated to one entry | FR-001 + Q1 | First-discovered (alphabetical walker order) wins; `tracing::warn!` emitted listing dropped paths. |
| The main-module's `licenses` field MUST be empty for milestone 064 | FR-005 | License detection deferred to issue #103; no attempt to read `[package].license` in this milestone. |
| Workspace-only `Cargo.toml` (no `[package]`) MUST NOT emit a main-module | FR-002 | The reader checks for `[package]` presence before constructing the entry. |

## Reuses from milestone 053

- `SpdxPrimaryPackagePurpose::Application` enum (in `mikebom-cli/src/generate/spdx/packages.rs`) — set on cargo main-module SPDX 2.3 packages identically to Go's path.
- The CDX `metadata.component` swap at `metadata.rs:156-200` — generalized from "filter by Go PURL prefix" to "filter by C40 role tag" (a 1-2 line change).
- The `components[]` exclusion at `builder.rs` — same generalization.
- The polyglot super-root pattern from FR-008 — already supports N-children DESCRIBES, so cargo workspace members slot in as additional describable elements without structural change.

## Does NOT introduce

- No new public Rust type
- No new crate dependency
- No new CLI flag
- No new SBOM annotation key (C40 row already exists)
- No new SPDX `primaryPackagePurpose` enum value (`Application` already wired)
- No subprocess calls
