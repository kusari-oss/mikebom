# Research: Binary-tier completion (milestone 130)

## R1. US1 root cause — confirmed during planning

**Decision**: The cargo-auditable reader at `mikebom-cli/src/scan_fs/binary/cargo_auditable.rs`
(milestone 029, 240 LOC) is **functioning correctly**. The bug is at the call site in
`mikebom-cli/src/scan_fs/binary/mod.rs:700` where the cargo-auditable emission block is gated by
`!skip_secondary_evidence`, which becomes `true` for any binary claimed by a package-db reader.

**Rationale**: Code trace performed during planning:

```rust
// mikebom-cli/src/scan_fs/binary/mod.rs:459-463
let skip_file_level = path_claimed
    || rpm_dir_heuristic
    || collapsed_by_python
    || collapsed_by_jdk;
let skip_secondary_evidence = skip_file_level || go_in_linux;

// mikebom-cli/src/scan_fs/binary/mod.rs:694-708
// Same `skip_secondary_evidence` gate as the version-string
// scanner: when the binary is already covered by an
// authoritative package-db entry (dpkg/rpm/etc.), don't
// double-emit `pkg:cargo/<crate>` shadows. Each emitted
// crate carries `parent_purl = file_level_purl` cross-
// linking back to the file-level binary component's identity.
if !skip_secondary_evidence {
    if let Some(ref manifest) = scan.cargo_auditable {
        let entries = cargo_auditable_packages_to_entries(
            manifest,
            &file_level_purl,
            &path,
        );
        out.extend(entries);
    }
}
```

The comment ("don't double-emit shadows") describes the CORRECT intent for the version-string scanner
(which emits `pkg:generic/openssl@1.1.1`-shaped components that DO duplicate package-db claims). But
applying the same gate to cargo-auditable is **conceptually wrong**: the per-crate emissions from
`cargo-auditable` are NOT shadows of the binary's identity — they're a separate tier of truth (the
transitive build closure of crates statically linked into the binary). The Wolfi `pkg:apk/wolfi/uv`
identity says "this is uv version X.Y.Z, installed from apk"; the cargo-auditable per-crate emissions
say "uv internally bundles these 200 Rust crates". Both are correct, neither shadows the other.

The fix is to remove the `skip_secondary_evidence` gate around lines 700-708. All OTHER
`skip_secondary_evidence`-gated blocks (version-string scan at line 502, linkage at line 530,
ELF-note at line 561) STAY gated — those genuinely do produce shadows of package-db claims.

**Audit-image impact prediction**: pre-fix mikebom emits 58 `pkg:cargo` components (all from the
`Cargo.lock` source-tier hit at `/usr/lib64/rustlib/src/rust/library/Cargo.lock`). Post-fix, the
`/usr/bin/uv` (~200 crates) + `/usr/bin/uvx` (~700 crates after dedup across the two binaries) will
emit, lifting the unique count to ~900+. SC-001 (≥900 unique cargo) is achievable from the US1 fix
alone — no other US needed for SC-001.

**Alternatives considered**:

- Add a per-binary-tier carve-out inside the existing gate (e.g. `if !skip_secondary_evidence ||
  cargo_auditable_present_in_claimed_binary`). Rejected — the gate's intent for version-string
  scanning IS correct; the fix is to recognize cargo-auditable as a different tier, not to add a
  carve-out to the wrong abstraction.
- Move the cargo-auditable emission entirely out of the binary-tier loop. Rejected — the file-level
  PURL (used as `parent_purl` for the per-crate emissions' cross-link) is computed in the loop; moving
  the cargo-auditable emission would duplicate the file-level metadata gathering.

## R2. US2 nested-JAR walker design

**Decision**: Recursive `zip::ZipArchive` descent at `mikebom-cli/src/scan_fs/package_db/maven.rs`,
with:

- SHA-256-keyed visited set for cycle detection (mirrors milestone-128 include-chain pattern).
- Depth limit of 8 levels (matches milestone-128 `INCLUDE_DEPTH_LIMIT`).
- Per-archive decompressed-size cap of 1 GB (mitigates zip bombs).
- Extension filter restricted to `.jar` / `.war` / `.ear` (clarification Q2 from milestone 129).
- The recursive helper accepts a byte slice (the parent archive's contents) and a depth counter;
  recurses by extracting each nested entry's bytes into a `Vec<u8>` and re-invoking itself with
  `depth + 1`.

**Rationale**: The `zip::ZipArchive::new(Cursor::new(&[u8]))` pattern works because `Cursor<&[u8]>`
implements both `Read` and `Seek`. The pre-decompression size check (`zip::read::ZipFile::size()`
returns the central-directory declared uncompressed size) is the zip-bomb mitigation per FR-014.
Cycle detection via SHA-256 of the parent archive bytes catches the pathological case where a
fixture archive contains itself.

**Reader integration**: extend the existing milestone-009 reader's per-JAR processing site. For each
top-level JAR currently scanned, AFTER emitting the existing top-level `pkg:maven` component,
recursively walk nested archives starting from the JAR's own ZIP entries. The top-level emission
continues to carry `mikebom:source-mechanism = "maven-jar"`; the nested emissions carry the new
`"maven-jar-nested"` value.

**Alternatives considered**:

- Walk via a tempdir + shell-out to `unzip`. Rejected — violates `--offline` (subprocess); adds
  filesystem cost; tempdir cleanup is complicated.
- Use `walkdir` recursively over the tempdir extraction. Rejected — same issues + materialized
  uncompressed bytes on disk add zip-bomb amplification risk.
- Add a depth annotation to track nesting depth in the output. Rejected — the
  `mikebom:source-files = "<outer>!<inner>!<deeper>..."` URL convention already encodes depth via
  the `!` separator count.

## R3. US3 CLR metadata-table layout (ECMA-335 §II.22)

**Decision**: Hand-roll a minimal metadata-table reader on top of `object::read::pe::PeFile` using
its existing `optional_header().data_directories()[14]` accessor to locate the `COR20_HEADER` (the
`IMAGE_COR20_HEADER` struct at the start of the CLR metadata blob). From the `MetaData` directory
inside that header, locate the `#~` (metadata tables) stream and the `#Strings` (UTF-8 string heap)
stream. Read the `Assembly` table (token `0x20`) row 0 for `Name`, `Culture`, and Version 4-tuple.
Read the `CustomAttribute` table (token `0x0C`) for rows whose `Type` resolves (through the
`MemberRef` table and back through `TypeRef` → `#Strings`) to `AssemblyFileVersionAttribute` or
`AssemblyInformationalVersionAttribute`; each row's `Value` column points into the `#Blob` heap where
the attribute's string argument lives.

**Rationale**: `object` 0.36 gives us PE COFF section + data-directory parsing for free, but does
NOT understand CLR metadata. Our scope is four string fields per managed DLL (Name, Version,
FileVersion, InformationalVersion, Culture) — for that scope, hand-rolling is ~800 LOC of
straight-line code and zero new deps. The format is stable since .NET 2.0 (~2005) and is documented
in ECMA-335 §II.22.

**Implementation skeleton**:

```rust
fn parse_managed_assembly(bytes: &[u8]) -> Option<ManagedPeAssembly> {
    let pe = object::read::pe::PeFile64::parse(bytes).ok()?;
    // Step 1: gate on CLR header present.
    let cor20_rva_size = pe.nt_headers().optional_header.data_directories[14];
    if cor20_rva_size.virtual_address.get(LE) == 0 {
        return None;
    }
    // Step 2: read IMAGE_COR20_HEADER at the COR20 RVA.
    let cor20_bytes = pe.section_by_rva(cor20_rva_size.virtual_address.get(LE))?;
    let cor20_header: IMAGE_COR20_HEADER = read_from_offset(&cor20_bytes, /* offset */)?;
    // Step 3: read MetaData root structure.
    let metadata_root = pe.section_by_rva(cor20_header.metadata_rva)?;
    let signature = u32::from_le_bytes(metadata_root[0..4].try_into().ok()?);
    if signature != 0x424A_5342 { return None; }  // "BSJB"
    // Step 4: walk stream headers, find #~ and #Strings.
    let (tables_stream, strings_heap, blob_heap) = locate_streams(&metadata_root)?;
    // Step 5: parse table headers, find Assembly + CustomAttribute table offsets.
    let table_offsets = parse_table_headers(tables_stream)?;
    // Step 6: read Assembly table row 0 -> Name, Culture, Version.
    let assembly_row = read_assembly_row_0(&tables_stream, &table_offsets, &strings_heap)?;
    // Step 7: walk CustomAttribute table, find FileVersionAttribute + InformationalVersionAttribute.
    let custom_attrs = walk_custom_attributes(&tables_stream, &table_offsets, &blob_heap, &strings_heap);
    Some(ManagedPeAssembly { name: assembly_row.name, ... })
}
```

The `IMAGE_COR20_HEADER` struct + the metadata-root layout are documented inline in the
implementation per ECMA-335 §II.24.2.

**Alternatives considered**:

- Pull in `pelite = "0.10"` (~5 KLOC, ~30 transitive deps). Rejected per Constitution Principle I.
- Pull in `monodis` or `ikdasm` via shell-out. Rejected — `--offline` violation + external runtime.
- Skip CustomAttribute parsing and use only the `Assembly` table's Version 4-tuple as the PURL
  version. Rejected — the milestone-129 clarification Q3 fallback ladder requires the
  InformationalVersion and FileVersion fields to be EMITTED (even if AssemblyVersion wins the PURL
  version when others are absent).

## R4. US3 resource-assembly dedup mechanics

**Decision**: The milestone-105 `SourceMechanism`-keyed dedup pipeline at
`mikebom-cli/src/scan_fs/dedup.rs` already handles cross-mechanism collisions (one component per
canonical PURL). The new dimension introduced by US3's resource-assembly case (multiple culture
variants for the same `(name, version)`) needs an additive merge step in the emission path:

```rust
// In dotnet_pe_clr::read_all, when about to emit a component:
let canonical_purl = build_nuget_purl(&assembly.name, &purl_version);
let entry = accumulator.entry(canonical_purl).or_insert_with(|| ...);
if let Some(culture) = assembly.culture.filter(|c| c != "neutral" && !c.is_empty()) {
    entry.cultures.insert(culture);
}
```

Where `accumulator` is a `BTreeMap<Purl, AccumulatedAssembly>` and `cultures` is a
`BTreeSet<String>`. The final emission flattens cultures into a comma-joined sorted string for the
`mikebom:assembly-cultures` annotation, OR omits the annotation entirely when the set is empty.

**Why this is intra-reader rather than dedup-pipeline-level**: the cultures-set merge is specific
to the PE/CLR reader. The dedup pipeline (milestone 105) operates at the cross-reader level
(merging `.deps.json` emissions with PE/CLR emissions for the same package). The intra-reader merge
happens BEFORE the components reach the dedup pipeline, so the dedup pipeline sees one entry per
`(name, version)` from the PE/CLR reader, with the culture set already merged.

**Cross-reader dedup**: when `.deps.json` (milestone 129) ALSO declares the same package, the
milestone-105 pipeline collapses the two into one component. The surviving component carries
`mikebom:also-detected-via = "dotnet-deps-json,dotnet-assembly-metadata"` (sorted lex per existing
convention). The `assembly-cultures` annotation from the PE side is preserved as-is on the
collapsed component (the `.deps.json` reader doesn't emit a competing cultures annotation, so no
merge conflict).

**Alternatives considered**:

- Emit one component per culture-variant DLL with culture in the source-files annotation. Rejected
  per the 2026-06-18 clarification Q1 (Option B chosen) — inflates component counts ~30× without
  vulnerability-matching value.

## R5. Audit-image cargo unique count baseline — confirmed

**Decision**: Verified during planning that syft's 986 `pkg:cargo` count on the audit image
is the unique-`(name, version)` count from `cargo-auditable-binary-cataloger` (not per-binary
multi-counted). The bash probe `jq -r '[.components[] | select(.purl | startswith("pkg:cargo")) | .purl] | unique | length'` returned 986; the
unfiltered `length` returned 986 as well — syft already dedups at emission time. SC-001 ≥900 is a
~9% bound on the post-US1-fix count.

**Rationale**: Verification was needed because the milestone-129 audit framing used a `total` count
(1,489 for nuget) that turned out to be per-file-multiplied. Cargo's case is simpler — syft's
audit-binary cataloger emits one component per `(name, version)` regardless of how many binaries
reference it.

## R6. PE/CLR fixture provenance

**Decision**: Synthetic hand-crafted minimal-managed-assembly byte arrays (~5 KB each) embedded
via `include_bytes!`. No real Microsoft `.dll` files committed.

**Rationale**: Real Microsoft DLLs would balloon the repo (~20+ MB per `Microsoft.AspNetCore.dll`)
and carry provenance-tracking burden (Microsoft EULAs are permissive for redistribution but require
attribution). Hand-crafted minimal CLR PEs are ~5 KB each and exercise the same code paths
(`IMAGE_COR20_HEADER` presence check + Assembly table row 0 read + CustomAttribute walk). The
real-image acceptance test (FR-018 scenario 1) runs against a public
`mcr.microsoft.com/dotnet/runtime:8.0-alpine` pull — the test pulls the image at test-run time
(network-required, gated behind `MIKEBOM_REQUIRE_DOTNET_RUNTIME=1` env var matching the existing
pattern at `mikebom-cli/tests/oci_registry_smoke.rs`).

**Construction approach**: a `build.rs`-time helper crate (in `mikebom-cli/build/`) that assembles
the minimal PE structure (DOS stub + PE header + sections + `IMAGE_COR20_HEADER` + metadata root +
`#~` stream + `#Strings` heap + `#Blob` heap + Assembly table + CustomAttribute table) per
ECMA-335 §II.22-24. Output is byte-arrays committed under `tests/fixtures/binary_tier_completion/dotnet_pe_clr/`.

**Alternatives considered**:

- Commit real Microsoft DLLs from the dotnet runtime. Rejected per repo bloat + provenance burden.
- Use a fixture-cache sibling repo (milestone 090 pattern). Rejected for this milestone because the
  PE/CLR fixtures are tightly coupled to the reader's parse expectations (any wire-format change
  invalidates the fixture); keeping them in-tree means atomic commits.

## Decisions summary

| ID | Topic | Decision | Status |
|---|---|---|---|
| R1 | US1 root cause | `skip_secondary_evidence` gate at `binary/mod.rs:700` suppresses cargo-auditable for claimed binaries. Fix = remove the gate around that block only. | Confirmed |
| R2 | US2 nested-JAR walker | SHA-256 visited set + 8-level depth + 1 GB cap; `.jar`/`.war`/`.ear` only (Q2) | Decided |
| R3 | US3 CLR metadata-table | Hand-roll on `object`'s PE primitives (~800 LOC, zero new deps) | Decided |
| R4 | US3 resource-assembly dedup | Intra-reader culture-set accumulator + `mikebom:assembly-cultures` annotation | Decided |
| R5 | Audit-image cargo baseline | Syft's 986 is unique-count; SC-001 ≥900 within 9% bound | Confirmed |
| R6 | PE/CLR fixture provenance | Synthetic hand-crafted PEs at build-time; no real Microsoft DLLs committed | Decided |
