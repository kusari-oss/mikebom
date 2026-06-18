# Data Model: Binary-tier completion (milestone 130)

Three entities. None persist beyond a single scan (matches every milestone since 002).

## Entity 1: `CargoAuditableEmissionDecision` (US1)

NOT a new Rust struct — instead, a **decision table** documenting the behavioral surface the US1
gate-removal exposes. Future readers of `mikebom-cli/src/scan_fs/binary/mod.rs` need this table to
understand WHY the cargo-auditable block at line 700 is unique among the `skip_secondary_evidence`-gated
emissions.

| Binary state | `skip_secondary_evidence` | Pre-130 cargo-auditable emission | Post-130 cargo-auditable emission |
|---|---|---|---|
| Unclaimed, no rpm-dir, not python/jdk collapsed | `false` | EMIT | EMIT (unchanged) |
| Path claimed by apk/dpkg/rpm reader | `true` | suppress | **EMIT (FIX)** |
| Inside rpm-managed directory | `true` | suppress | **EMIT (FIX)** |
| Collapsed by python umbrella | `true` | suppress | **EMIT (FIX)** |
| Collapsed by jdk umbrella | `true` | suppress | **EMIT (FIX)** |
| Go binary in linux container | `true` | suppress | **EMIT (FIX)** — note: rare in practice; Go binaries don't carry cargo-auditable manifests |

**Validation per FR-008**: the regression test fixture (a synthetic ELF with both an
`apk-style-claim-path` AND a valid `.dep-v0` section) MUST fail against alpha.48 + milestone 129
(zero `pkg:cargo` emitted from the binary) and pass on milestone 130 (≥1 `pkg:cargo` emitted from
the binary, in addition to the `pkg:apk/...` package-db entry that owns the binary path).

**Side-effect surface**: the `parent_purl` cross-link on each emitted `pkg:cargo` component
continues to reference the file-level binary's PURL (computed at line 466-468 of `mod.rs` even when
`skip_file_level` is true — see the in-file comment for why). The change is purely additive at the
output level — no existing components are dropped, no PURLs change, no annotations change.

## Entity 2: `NestedArchiveWalker` (US2)

Direct port of the milestone-129 deferred US3 design at
`specs/129-binary-tier-readers/data-model.md:Entity 4`. Lives at
`mikebom-cli/src/scan_fs/package_db/maven.rs`.

```rust
pub(crate) struct NestedArchiveWalker {
    /// Visited-set keyed on SHA-256 of each archive's bytes.
    /// Cycle protection — milestone-128 convention.
    visited: HashSet<[u8; 32]>,
    /// Current recursion depth; bounded at 8 per FR-012.
    depth: u8,
    /// Per-archive decompressed-size cap; 1 GB per FR-014.
    size_cap: u64,
    /// Emitter for newly-discovered nested components.
    out: Vec<PackageDbEntry>,
    /// Path of the OUTERMOST archive (for parse-failure annotations
    /// and `mikebom:source-files` URL construction).
    outer_path: PathBuf,
    /// Stack of nested-entry names traversed (for the `!`-separated
    /// `mikebom:source-files` URL convention).
    nested_stack: Vec<String>,
}

impl NestedArchiveWalker {
    pub fn walk(&mut self, archive_bytes: &[u8]) {
        let sha = sha256_of(archive_bytes);
        if !self.visited.insert(sha) {
            return; // cycle detected; milestone-128 pattern
        }
        if self.depth >= 8 {
            tracing::warn!(
                outer = %self.outer_path.display(),
                "nested-archive depth limit (8) reached; further nesting skipped"
            );
            return;
        }
        // Open via `zip::ZipArchive::new(Cursor::new(archive_bytes))`,
        // iterate entries:
        //   - For META-INF/maven/<group>/<artifact>/pom.properties:
        //       emit a PackageDbEntry with mikebom:source-mechanism =
        //       "maven-jar-nested" and mikebom:source-files =
        //       "<outer_path>!<nested_stack joined by !>".
        //   - For entries with .jar/.war/.ear suffix AND
        //       uncompressed_size <= self.size_cap:
        //         self.nested_stack.push(entry_name);
        //         self.depth += 1;
        //         self.walk(&inner_bytes);  // recursive descent
        //         self.depth -= 1;
        //         self.nested_stack.pop();
        //   - For .zip entries: SKIP (clarification Q2).
        //   - For entries declaring uncompressed_size > self.size_cap:
        //         tracing::warn! + SKIP.
    }
}
```

**Output**: each emitted nested-JAR component carries:

- `mikebom:sbom-tier = "image"` (FR-001)
- `mikebom:source-mechanism = "maven-jar-nested"` (FR-002 + FR-015)
- `mikebom:source-files = "<outer-path>!<nested-path>!<deeper-path>..."` (FR-016)
- `mikebom:cpe-candidates = "<derived>"` (existing milestone-097 channel reuse)
- Existing milestone-009 fields (`license`, `evidence`, etc.) unchanged for nested entries.

**Integration**: the existing milestone-009 reader's per-JAR processing site at
`package_db/maven.rs::read_one_jar` is extended to invoke `NestedArchiveWalker::walk` after the
top-level JAR's processing completes. The top-level JAR continues to emit with
`mikebom:source-mechanism = "maven-jar"`.

## Entity 3: `ManagedPeAssembly` (US3)

Port of the milestone-129 deferred US3 design at `specs/129-binary-tier-readers/data-model.md:Entity 2`,
plus the 2026-06-18 culture-set addition.

```rust
#[derive(Debug, Clone)]
pub(crate) struct ManagedPeAssembly {
    pub assembly_name: String,                          // Assembly table row 0, Name column (#Strings ref)
    pub assembly_version: Version4Tuple,                // (Major, Minor, Build, Revision) from Assembly table row 0
    pub assembly_file_version: Option<String>,          // CustomAttribute: AssemblyFileVersionAttribute (#Blob ref)
    pub assembly_informational_version: Option<String>, // CustomAttribute: AssemblyInformationalVersionAttribute
    pub culture: Option<String>,                        // Assembly table row 0, Culture column; "neutral" or
                                                        // empty maps to None.
    pub source_path: PathBuf,                           // Where the .dll lives in the rootfs
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Version4Tuple {
    pub major: u16,
    pub minor: u16,
    pub build: u16,
    pub revision: u16,
}

impl ManagedPeAssembly {
    /// Per FR-020 + clarification Q3 from milestone 129: pick PURL
    /// version via fallback ladder.
    pub fn purl_version(&self) -> String {
        if let Some(v) = self.assembly_informational_version.as_ref() {
            return v.clone();
        }
        if let Some(v) = self.assembly_file_version.as_ref() {
            return v.clone();
        }
        format!("{}.{}.{}.{}",
            self.assembly_version.major,
            self.assembly_version.minor,
            self.assembly_version.build,
            self.assembly_version.revision,
        )
    }
}

/// Intra-reader accumulator for the resource-assembly culture-set
/// merge per the 2026-06-18 clarification + R4.
pub(crate) struct AssemblyAccumulator {
    /// Keyed on canonical PURL (one entry per unique (name, version)).
    components: BTreeMap<Purl, AccumulatedAssembly>,
}

pub(crate) struct AccumulatedAssembly {
    pub representative: ManagedPeAssembly,  // First DLL contributing this (name, version)
    pub cultures: BTreeSet<String>,         // Union of all non-empty, non-"neutral" cultures
    pub source_paths: BTreeSet<PathBuf>,    // All DLL paths contributing this (name, version)
}
```

**Validation rules** (per FR-018..024):

- `is_managed_assembly()` returns `true` iff `DataDirectory[14].VirtualAddress != 0 && Size != 0`.
- `assembly_name` MUST be a valid UTF-8 string from `#Strings` heap; if not, the assembly is SKIPPED
  with a `warn` log.
- The `purl_version()` fallback ladder ensures every emitted component carries a non-empty version.
- Per the 2026-06-18 clarification: multiple DLLs with the same `(name, version)` collapse into ONE
  component. Their cultures (when non-"neutral" and non-empty) merge into
  `AccumulatedAssembly.cultures`; their paths merge into `source_paths`.
- The final emission flattens `cultures` into a comma-joined sorted string for the
  `mikebom:assembly-cultures` annotation. Empty `cultures` set → annotation OMITTED entirely
  (the FR-024 default for assemblies with only the "neutral" culture).

**Output**: each emitted component carries:

- `mikebom:sbom-tier = "image"` (FR-001)
- `mikebom:source-mechanism = "dotnet-assembly-metadata"` (FR-002)
- `mikebom:source-files = <BTreeSet flattened comma-joined>` (one path per culture variant)
- `mikebom:assembly-version-informational = "<value>"` (FR-021, if present on the representative)
- `mikebom:assembly-version-file = "<value>"` (FR-021, if present)
- `mikebom:assembly-version-runtime = "<4-tuple>"` (FR-021, always present)
- `mikebom:assembly-cultures = "<comma-joined sorted>"` (FR-024, if non-empty after merge)
- `mikebom:cpe-candidates = "<derived>"` (existing milestone-097 channel reuse)

## Cross-entity dedup pipeline (milestone 105 reuse)

When the same `(purl-type, name, version)` is detected by multiple readers (e.g. `.deps.json` AND
PE/CLR for the same NuGet package), the existing milestone-105 dedup pipeline at
`mikebom-cli/src/scan_fs/dedup.rs` merges them into ONE `ResolvedComponent` with a
`mikebom:also-detected-via` annotation listing all source-mechanism variants in sorted order. No
new code is needed in dedup.rs for milestone 130 — the new source-mechanism string values
(`maven-jar-nested`, `dotnet-assembly-metadata`) extend the existing enum purely additively.

When the dedup pipeline merges a PE-derived component with a `.deps.json`-derived component, the
`mikebom:assembly-cultures` annotation from the PE side flows through to the surviving component
unchanged. The `.deps.json` reader doesn't emit a competing cultures annotation.

## State transitions

None — three entities are constructed once per file/section parse and consumed by the
`PackageDbEntry` → `ResolvedComponent` conversion. The lifecycle is:

```text
File on disk
  → entity (parsed)
    → Vec<PackageDbEntry> (via per-reader accumulator merge for US3)
      → ResolvedComponent (via existing milestone-105 dedup)
        → CDX/SPDX2.3/SPDX3 emission (via existing format builders)
          → SBOM bytes on disk
```

The three entities have no mutable state after construction (except the `NestedArchiveWalker` and
`AssemblyAccumulator`, both of which are local-to-call and discarded after the per-reader pass
completes).
