# Phase 1: Data Model — Deeper Yocto / OpenEmbedded SBOM coverage

## Entity: `RecipeMetadata`

**Location**: `mikebom-cli/src/scan_fs/package_db/yocto/recipe_body.rs` (NEW).

**Lifetime**: Parsed once per `.bb` file (with recursive `.inc` merging), consumed by the component-builder at `recipe.rs`, discarded after the component is emitted.

```rust
/// All fields the body-parser extracts from one recipe (after merging
/// transitively-included `.inc` files per FR-004 last-write-in-source-order).
#[derive(Debug, Clone, Default)]
pub struct RecipeMetadata {
    /// `<name>` segment of `<name>_<version>.bb`. Filename-derived.
    pub recipe_name: String,
    /// `<version>` segment of `<name>_<version>.bb`. Filename-derived.
    /// Overridden by FR-018 (SRCREV-derived) when literal `"git"` or
    /// when contains `AUTOINC`.
    pub recipe_version: String,
    /// `LICENSE = "..."` field, canonicalized via `SpdxExpression::try_canonical`.
    /// `None` when LICENSE was absent OR set to `"CLOSED"` (use `license_closed`
    /// flag for the discriminator); `Some(...)` for every other case.
    pub license: Option<SpdxExpression>,
    /// `LICENSE = "CLOSED"` discriminator. Drives FR-012's
    /// `mikebom:yocto-license-closed` annotation.
    pub license_closed: bool,
    /// `SRC_URI = "..."` field, split on whitespace and per-URI cleaned.
    /// Each entry preserves its scheme + qualifiers (e.g., `git://...;branch=...`).
    pub src_uris: Vec<String>,
    /// `SRCREV = "..."` field (single-value).
    pub srcrev: Option<String>,
    /// `SRCREV_machine:<arch> = "..."` field set. Key is the arch literal,
    /// value is the SRCREV. Drives FR-003's per-machine annotation.
    pub srcrev_by_machine: std::collections::BTreeMap<String, String>,
    /// `HOMEPAGE = "..."` field.
    pub homepage: Option<String>,
    /// `SUMMARY = "..."` field.
    pub summary: Option<String>,
    /// `DESCRIPTION = "..."` field. Only emitted in the `mikebom:yocto-description`
    /// annotation when it materially differs from `summary` per FR-010.
    pub description: Option<String>,
    /// `DEPENDS = "..."` field, split on whitespace. Each entry is a recipe-name
    /// (later resolved to a PURL by the cross-recipe linker pass).
    pub depends: Vec<String>,
    /// `RDEPENDS_<pkg> = "..."` field set. Key is the `<pkg>` suffix
    /// (typically `${PN}`); value is the dep-name list.
    pub rdepends: std::collections::BTreeMap<String, Vec<String>>,
    /// `BBCLASSEXTEND = "..."` field, split on whitespace. e.g.,
    /// `["native", "nativesdk"]`. Drives FR's `mikebom:yocto-class-extend`.
    pub class_extend: Vec<String>,
    /// Unresolved `${VAR}` references the parser saw in IDENTITY fields
    /// (LICENSE, SRC_URI, SRCREV, HOMEPAGE, SUMMARY). Drives FR-005's
    /// `mikebom:yocto-unexpanded-vars` annotation.
    pub unexpanded_vars: Vec<String>,
    /// Did the parser apply ≥1 override-syntax merge (FR-016)? Drives
    /// the `mikebom:yocto-overrides-merged` annotation.
    pub overrides_merged: bool,
    /// Filesystem path of the `.bb` file (for provenance + nearest-ancestor
    /// layer attribution per FR-006).
    pub source_path: std::path::PathBuf,
    /// Filesystem paths of every `.inc` file merged into this recipe.
    /// Drives the `mikebom:source-files` evidence array.
    pub include_paths: Vec<std::path::PathBuf>,
}
```

**Validation rules**:

- `recipe_name` and `recipe_version` MUST be non-empty (filename-derived; FR-005).
- `license` if present MUST round-trip through `SpdxExpression::try_canonical`.
- `src_uris` entries that are git URLs MUST be normalized to `https://...` for the vcs external reference (FR-002).
- `srcrev` if present MUST match `^[0-9a-f]{6,40}$` (hex SHA prefix or full SHA).
- `depends` and `rdepends` entries MUST be valid BitBake recipe names (`[a-zA-Z0-9_\-\+]+`).

## Entity: `LayerConf`

**Location**: `mikebom-cli/src/scan_fs/package_db/yocto/layer_conf.rs` (NEW).

**Lifetime**: Parsed once per `conf/layer.conf` file, cached by directory for the duration of the scan.

```rust
/// Parsed `conf/layer.conf` shape.
#[derive(Debug, Clone)]
pub struct LayerConf {
    /// `BBFILE_COLLECTIONS += "<name>"` value. Required field — when
    /// absent, the layer.conf is malformed and we skip it with a warn.
    pub collection: String,
    /// `LAYERVERSION_<collection> = "<version>"` value.
    /// `None` when absent (some layers omit it).
    pub version: Option<String>,
    /// `LAYERSERIES_COMPAT_<collection> = "<series>"` value (e.g.,
    /// `"scarthgap"` or `"kirkstone honister"`). Split on whitespace.
    pub series_compat: Vec<String>,
    /// Filesystem path of the `conf/layer.conf` file.
    pub source_path: std::path::PathBuf,
}
```

**Validation rules**:

- `collection` MUST be non-empty. Malformed layer.conf → skip + warn.
- `version` is `String` (BitBake uses semantic-ish but not strictly semver — `"1"`, `"7"`, `"2"` are all common).
- Multiple `BBFILE_COLLECTIONS` declarations in one file produce multiple `LayerConf` entries (the rare two-layer-in-one-file case per the spec edge cases).

## Entity: `BbAppendIndex`

**Location**: `mikebom-cli/src/scan_fs/package_db/yocto/bbappend.rs` (NEW).

**Lifetime**: Built once per scan after all `.bb` files are discovered; consumed during component-building to add `mikebom:bbappend-applied` annotations.

```rust
/// Maps `(recipe-name, version-glob)` → list of `.bbappend` file paths.
#[derive(Debug, Clone, Default)]
pub struct BbAppendIndex {
    /// Key: (recipe-name, version-glob-string). The glob string is
    /// the version segment from the `.bbappend` filename, with `%`
    /// preserved as the wildcard. e.g., `u-boot_%.bbappend` → key
    /// `("u-boot", "%")`; `u-boot_2024.07.bbappend` → key
    /// `("u-boot", "2024.07")`.
    /// Value: lex-sorted, deduped list of `.bbappend` paths matching
    /// that key.
    pub by_recipe: std::collections::BTreeMap<(String, String), Vec<std::path::PathBuf>>,
    /// `.bbappend` files that had no matching recipe in the scan.
    /// Surfaced via the `tracing::warn!` log per US4 AC#3; no phantom
    /// components are emitted.
    pub orphans: Vec<std::path::PathBuf>,
}

impl BbAppendIndex {
    /// For a recipe `(name, version)`, return the lex-sorted list of
    /// appends modifying it. Matches both exact-version appends
    /// (`name_<version>.bbappend`) and version-glob appends
    /// (`name_%.bbappend`).
    pub fn appends_for(&self, name: &str, version: &str) -> Vec<&std::path::PathBuf> { ... }
}
```

## Entity: `CpeNameMap`

**Location**: `mikebom-cli/src/scan_fs/package_db/yocto/cpe_name_map.rs` (NEW).

**Lifetime**: Compile-time `&'static [(&str, &str)]` slice. Looked up O(1) per recipe via a `phf`-style or linear search (the table is small enough for linear).

```rust
/// Recipe-name → CPE-product-name mapping. Sourced from
/// openembedded-core's `meta/conf/distro/include/cve-extra-exclusions.inc`.
/// Stable across Yocto releases; refresh requires a minor mikebom update.
pub const CPE_NAME_MAP: &[(&str, &str)] = &[
    ("linux-kernel", "linux_kernel"),
    ("nss", "network_security_services"),
    ("nspr", "netscape_portable_runtime"),
    ("dropbear", "dropbear_ssh"),
    ("zstd", "zstandard"),
    // ... ~50 more entries, lex-sorted by recipe-name
];

/// Look up the CPE product name for a recipe. Returns the recipe name
/// unchanged when no mapping exists (the common case).
pub fn cpe_product_for_recipe(recipe_name: &str) -> &str { ... }
```

## Relationships

```text
.bb file ──parsed by──> RecipeMetadata ──┐
.inc files ──merged into──┐               │
                          │               ├──> PackageDbEntry ──> ResolvedComponent ──> SBOM
.bbappend ──indexed by──> BbAppendIndex ──┤
                          │               │
conf/layer.conf ──parsed by──> LayerConf ─┘
                          │
                          └──> nearest-ancestor lookup populates RecipeMetadata.layer fields

CpeNameMap (static) ──used by──> milestone-097 CPE-candidates synthesizer
```

## State transitions

None. The data flow is acyclic: read files → parse → build index → emit components → done. No persistent state between scans.

## Validation rules summary

| Rule | Source | Validation |
|---|---|---|
| `RecipeMetadata.license` round-trips through `SpdxExpression::try_canonical` | FR-001 | Unit test in `recipe_body.rs` |
| `RecipeMetadata.srcrev` matches hex SHA pattern | FR-003 | Unit test |
| Last-write-in-source-order on `.bb` vs `.inc` field conflicts | FR-004 + Q1 | Integration test using `include_chain/` fixture asserting LICENSE on the `.bb` wins over the `.inc`'s |
| Nearest-ancestor `conf/layer.conf` heuristic | FR-006 + Q2 | Integration test using `multi_layer_polyglot/` fixture asserting recipes in nested layers attribute to the right layer |
| BbAppendIndex matches version-glob appends | FR-008 | Unit test for the `_%.bbappend` glob expansion |
| CPE-name normalization applies for `linux-kernel` → `linux_kernel` | FR-017 | Unit test asserting the mapping table fires |
| Version derived from SRCREV when PV is `"git"` or contains `AUTOINC` | FR-018 | Integration test using `autoinc_version/` fixture asserting the emitted PURL version is 12-char hex |
| One component per recipe with multi-CPE candidates in array (NOT fan-out) | FR-019 | Integration test using `multi_cpe_curl/` fixture asserting exactly 1 `curl` component with ≥3 CPE candidates in the array |
| Orphan `.bbappend` → warn + no phantom component | US4 AC#3 | Integration test using `orphan_bbappend/` fixture asserting the emitted SBOM contains NO component named after the orphan |
| Mixed-scan cross-source dedup via milestone-105 also-detected-via | FR-014a + Q3 | Integration test scanning meta-layer tree + synthetic opkg DB; assert recipe-reader's license flows onto opkg-DB-discovered component |
