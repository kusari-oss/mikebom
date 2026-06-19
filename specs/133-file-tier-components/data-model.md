# Data Model: File-tier component emission

**Date**: 2026-06-19
**Branch**: `133-file-tier-components`

Five entities; no persistent state; per-scan derivation.

## Entity: `FileInventoryMode` (US3)

**Source location**: New enum in `mikebom-cli/src/cli/scan_cmd.rs` (clap
`ValueEnum` derive).
**Driven by**: FR-015.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum FileInventoryMode {
    /// No file-tier components emitted. Pre-milestone-133 behavior.
    Off,
    /// Default — emit file-tier components for files surviving content-shape
    /// allowlist AND failing hybrid dedupe (FR-011: path OR hash coverage).
    Orphan,
    /// Emit per-unique-hash file-tier component for every file passing the
    /// content-shape allowlist, regardless of dedupe coverage. Document-level
    /// `mikebom:file-inventory-mode = "full"` annotation set.
    Full,
}

// CLI surface (added to ScanArgs):
//   #[arg(long, value_enum, default_value_t = FileInventoryMode::Orphan)]
//   pub file_inventory: FileInventoryMode,
//
//   #[arg(long, default_value_t = 100 * 1024 * 1024)]
//   pub file_inventory_size_limit: u64,
```

**Validation**: clap value-parser handles enum at parse time; invalid value =
non-zero exit. Default `Orphan` is the behavior-change-flip per Q1
clarification.

## Entity: `ContentShape` classifier (US1)

**Source location**: `mikebom-cli/src/scan_fs/file_tier/content_shape.rs` (new
file).
**Driven by**: FR-005 (post-tightening at plan time per FR-022).

### Shape

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentShape {
    ElfBinary,
    PeBinary,
    MachoBinary,
    SharedLib,
    JavaOrArchive,    // .jar, .war, .ear, generic .zip
    OsPackage,        // .deb, .rpm, .apk
    CompressedArchive, // .tar.gz, .tgz, .tar.xz, .tar.bz2
    LoneManifest,     // Cargo.toml/package.json/pom.xml etc. NO adjacent lockfile
    ExecScript,       // chmod+x + #! magic
}

/// Apply the FR-005 content-shape allowlist + path-prefix exclusion + adjacent-
/// lockfile check. Returns `Some(ContentShape)` when the file qualifies for
/// file-tier emission; `None` when it should be skipped.
pub(crate) fn classify(
    rel_path: &Path,
    abs_path: &Path,
    rootfs_root: &Path,
    exclusion_globs: &globset::GlobSet,
) -> Option<ContentShape>;
```

### Path-prefix exclusion list (FR-005 post-tightening)

Built once per scan from a `const` slice of glob patterns:

```rust
const ORPHAN_PATH_EXCLUSIONS: &[&str] = &[
    "**/dotnet/packs/**",
    "**/dotnet/shared/**",
    "**/dotnet/sdk/**",
    "**/dotnet/store/**",
    "**/usr/share/dotnet/**",
    "**/node_modules/**",
    "**/lib/python*/site-packages/**",
    "**/.cargo/registry/**",
    "**/ruby/gems/**",
    "**/jvm/openjdk*/lib/**",
];
```

Glob building uses `globset::GlobSet` per the milestone-113 / -118 convention.

### Lone-manifest adjacent-lockfile check

Per FR-005 second bullet:

- `package.json` → check sibling for any of `package-lock.json`, `yarn.lock`,
  `pnpm-lock.yaml`; lockfile present → NOT lone.
- `Cargo.toml` → walk parents (bounded to 8 levels by `scan_fs::walk`
  convention) looking for `Cargo.lock`; found → NOT lone.
- `pom.xml` → check sibling for `target/` directory; present → NOT lone (real
  build, not vendored source-tree signal).
- `requirements.txt`, `Gemfile`, `go.mod` apply analogous rules (`requirements-
  freeze.txt` / `pyproject.toml` lockfile; `Gemfile.lock`; `go.sum`).

### State transitions

None. Per-file pure-function classification.

## Entity: Hybrid dedupe set (US1)

**Source location**: `mikebom-cli/src/scan_fs/file_tier/dedupe.rs` (new file).
**Driven by**: FR-011 (CORRECTED — Q3 hybrid path-OR-hash).

### Shape

```rust
pub(crate) struct DedupeIndex {
    /// FR-011 (a): paths claimed by ANY package-tier component's
    /// `mikebom:component-paths` property. Populated from the
    /// already-resolved component vector.
    claimed_paths: HashSet<PathBuf>,
    /// FR-011 (b): SHA-256 hashes of any binary-tier component
    /// (milestone-104 binary readers populate per-file hashes via
    /// existing `evidence.occurrences`).
    claimed_hashes: HashSet<Sha256Hex>,
}

impl DedupeIndex {
    pub(crate) fn build(components: &[ResolvedComponent]) -> Self;

    /// FR-011 dedupe check. Returns `true` when the file is COVERED (skip
    /// file-tier emission); `false` when orphan (emit).
    pub(crate) fn is_covered(&self, rel_path: &Path, hash: &Sha256Hex) -> bool;
}
```

### Validation

- `claimed_paths` populated AFTER all milestone-001 package-DB + milestone-104
  binary-tier readers complete. Built from `evidence.identity[]` or
  `mikebom:component-paths` properties on each component.
- `claimed_hashes` populated from `hashes[]` field on binary-tier components.
  Source confidence is high — binary-tier readers always carry the per-file
  hash since milestone 038.
- Path normalization: leading `/` stripped to match the no-leading-`/`
  convention from FR-007 / FR-012.

### State transitions

Build once per scan; immutable thereafter.

## Entity: `FileTierComponent` (US1, US3)

**Source location**: New module in `mikebom-cli/src/scan_fs/file_tier/mod.rs`
mapping a `(SHA-256, Vec<PathBuf>, ContentShape, u64-size)` into a
`ResolvedComponent`.
**Driven by**: FR-006, FR-007, FR-008, FR-009, FR-002.

### Shape

```rust
pub(crate) struct FileTierEntry {
    pub sha256: Sha256Hex,
    pub paths: Vec<PathBuf>,         // sorted lex-ascending per FR-007
    pub shape: ContentShape,
    pub size_bytes: u64,
}

impl From<FileTierEntry> for ResolvedComponent {
    /// CDX 1.6 emission: type = "file"; no PURL (FR-009).
    /// SPDX 2.3 emission: Package with filesAnalyzed: false; component-tier
    ///   annotation; checksums[] with SHA-256.
    /// SPDX 3 emission: software_File element with hash; component-tier
    ///   annotation kept for cross-format symmetry.
    fn from(e: FileTierEntry) -> Self;
}
```

### Validation

- FR-006: per-unique-hash dedupe at `HashMap<Sha256Hex, FileTierEntry>` build
  time; subsequent path encounters append to the existing entry's `paths`.
- FR-007: paths sorted before emission. `mikebom:file-paths` property emitted
  as JSON-encoded array.
- FR-008: SHA-256 mandatory. Streaming hash via `sha2::Sha256` over a chunked
  read (8 KB buffer) to avoid loading huge files into memory.
- FR-009: NO PURL. Identity-via-`bom-ref` only. `name` = basename of `paths[0]`.
- FR-002: `mikebom:component-tier = "file"` annotation on every emitted
  component.
- FR-010: skip files where `size_bytes > config.size_limit`; increment
  document-level skip counter.

### Emission targets

| Format | Identity | Hashes | Paths-as-property |
|---|---|---|---|
| CDX 1.6 | `components[].type = "file"`, `bom-ref` UUID, `name = basename` | `hashes[].alg = "SHA-256"` | `properties[name="mikebom:file-paths", value=<JSON-encoded array>]` |
| SPDX 2.3 | `Package` with `filesAnalyzed: false`, `SPDXID = "SPDXRef-File-<hash-prefix>"`, `name = basename` | `checksums[].algorithm = "SHA256"` | `annotations[].comment = <MikebomAnnotationCommentV1 envelope>` |
| SPDX 3 | `software_File` element, `spdxId = "SPDXRef-File-<hash-prefix>"`, `name = basename` | `verifiedUsing[].Hash{algorithm: "sha256", hashValue: <hex>}` | `Annotation` element with `annotationType: OTHER` + envelope |

## Entity: Trivy-style path/layer property pair on package-tier components (US2)

**Source location**: extension to every package-DB reader (apk, dpkg, rpm,
nuget/pe_clr, cargo, maven, npm/walk, gem, pypi, ...) that emits a component
from a rootfs path.
**Driven by**: FR-012, FR-013, FR-014.

### Shape

```rust
// In `PackageDbEntry` (mikebom_common::resolution):
pub struct PackageDbEntry {
    // ... existing fields ...
    /// FR-012: relative rootfs path the reader identified the package from.
    /// None for non-rootfs scans (lockfile-only, network metadata).
    pub source_path: Option<PathBuf>,
    /// FR-013: OCI layer digest containing source_path. Only populated for
    /// image scans (--image flow); None for --path scans.
    pub source_layer_digest: Option<String>,
}
```

### Emission

| Format | Property |
|---|---|
| CDX 1.6 | `components[].properties[name="mikebom:component-path", value="usr/bin/curl"]` + `components[].properties[name="mikebom:layer-digest", value="sha256:..."]` |
| SPDX 2.3 | `packages[].annotations[].comment` carrying `MikebomAnnotationCommentV1` envelope for each |
| SPDX 3 | `Annotation` element with the same envelope |

When a single package is identified from multiple paths (rare; e.g. apk
package whose files span layers), use the PLURAL `mikebom:component-paths`
property carrying a JSON-encoded sorted array — same shape as FR-007's
file-tier paths-as-property.

### Validation

- FR-012 required for every component with `source_path.is_some()`.
- FR-013 required only when both `source_path.is_some()` AND scan kind is
  `--image` AND the source path falls within an identified OCI layer per the
  milestone-130 `docker_image::ExtractedImage` layer mapping.
- Backwards compat: existing component dedupe (by PURL) unchanged.

## Entity: Document-level `mikebom:file-inventory-mode` annotation (US3)

**Source location**: emission in `mikebom-cli/src/generate/cyclonedx/metadata.rs`
+ `spdx/document.rs` + `spdx/v3_document.rs`.
**Driven by**: FR-017.

### Shape

Single document-level annotation emitted whenever `--file-inventory != orphan`:

- `--file-inventory=off`: `mikebom:file-inventory-mode = "off"` — explicit
  signal that file-tier is disabled even though it could have been on.
- `--file-inventory=orphan`: ABSENT (the default; absence = default mode).
- `--file-inventory=full`: `mikebom:file-inventory-mode = "full"` — signal that
  duplicates with package-tier may be present per the Strict Boundary §5
  override carve-out.

### Validation

Emitted at document level (not per-component) so consumers can detect the
override at parse time without walking every component.

## Entity relationships

```text
ResolvedComponent (existing)
  ├── (package-tier) -- US2 -- mikebom:component-path / mikebom:layer-digest
  ├── (binary-tier)  -- (unchanged; hashes populated since milestone 038)
  └── (file-tier, NEW)
        ├── name = basename(paths[0])
        ├── no PURL (FR-009)
        ├── hashes[].SHA-256 (FR-008)
        ├── mikebom:component-tier = "file" (FR-002)
        └── mikebom:file-paths = JSON sorted array (FR-007)

SbomDocument (existing)
  ├── components: Vec<ResolvedComponent>
  ├── document-level annotations
  │     ├── mikebom:file-inventory-mode (FR-017, conditional on non-default)
  │     ├── mikebom:file-inventory-skipped-oversize (FR-010 count)
  │     ├── mikebom:file-inventory-skipped-special-files (edge case count)
  │     ├── mikebom:file-inventory-unreadable (edge case count)
  │     └── mikebom:file-paths-truncated (edge case flag)
  └── (everything else unchanged)
```

No new persistent state. No state machines. Pure per-scan derivation.

## Notes for tasks.md

- US1 + US2 can be implemented in parallel — they touch different code paths
  (US1 adds the new `file_tier/` module; US2 extends existing per-reader
  emission). Recommended ordering: US2 FIRST so US1's path-coverage dedupe
  has real `mikebom:component-paths` to subtract.
- US3 builds directly on US1's `file_tier/` infrastructure; only the dedupe
  check differs (skipped in full mode).
- US4 is independent — Constitution amendment + reference doc + C-rows can
  land before, with, or after US1-US3 code.
