# Reader behavior contract — milestone 130

Per-reader I/O / side-effect / observability contract. Tests at
`mikebom-cli/tests/binary_tier_completion_us{1,2,3}*.rs` exercise these contracts end-to-end via
`mikebom sbom scan`.

## Reader 1: cargo-auditable gate-removal fix (US1)

**Module path**: `mikebom-cli/src/scan_fs/binary/mod.rs` (modification, not new file).

### Behavioral change

Lines 700-708 currently read:

```rust
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

Post-130 this becomes:

```rust
// Milestone 130 US1: cargo-auditable per-crate emissions are NOT
// shadows of the file-level binary identity — they're the
// transitive build closure of crates statically linked into the
// binary, which is a separate tier of truth from the package-db
// claim (apk/dpkg/rpm). Therefore the `skip_secondary_evidence`
// gate (which correctly suppresses version-string-scan shadows
// that DO duplicate package-db claims) MUST NOT apply here.
//
// This was a bug pre-130: claimed binaries (Wolfi /usr/bin/uv,
// Debian /usr/bin/rustup, etc.) had their .dep-v0 manifests
// silently suppressed.
if let Some(ref manifest) = scan.cargo_auditable {
    let entries = cargo_auditable_packages_to_entries(
        manifest,
        &file_level_purl,
        &path,
    );
    out.extend(entries);
}
```

### Logs

- No new log entries — the `cargo_auditable_packages_to_entries` function is unchanged. The pre-130
  behavior would silently skip; the post-130 behavior emits.

### Invariants

- All other `skip_secondary_evidence`-gated blocks (version-string scan at line ~502, linkage at
  line ~530, ELF-note at line ~561) STAY gated — those genuinely produce shadows of package-db claims.
- The `parent_purl` cross-link on each emitted `pkg:cargo` component continues to reference the
  file-level binary's PURL (computed at line ~466 even when `skip_file_level` is true).
- Mach-O path at `scan.rs:469` is unchanged.

### Test surface

- New integration test `binary_tier_completion_us1_cargo_auditable.rs` exercises a synthetic ELF
  fixture carrying both a `.dep-v0` section AND a path-claim by an apk-style fixture. The test asserts:
  - Pre-fix (alpha.48 + milestone 129): 0 `pkg:cargo` components emitted.
  - Post-fix: ≥1 `pkg:cargo` component emitted, with `mikebom:source-mechanism = "cargo-auditable-binary"`.
- Existing `mikebom-cli/src/scan_fs/binary/cargo_auditable.rs` unit tests at lines 1393-1450 are
  unchanged and continue to pass.

---

## Reader 2: Maven nested-JAR recursion (US2)

**Module path**: `mikebom-cli/src/scan_fs/package_db/maven.rs` (extension, not new file).

### Input

- The existing top-level reader at `read_one_jar` already opens each JAR via
  `zip::ZipArchive::new` and iterates entries.
- The new path: for each entry whose name ends in `.jar` / `.war` / `.ear`, extract the entry's
  bytes into a `Vec<u8>` and invoke the new `NestedArchiveWalker::walk(&inner_bytes)` helper.

### Output

- Additional `Vec<PackageDbEntry>` entries (appended to the existing top-level emissions) — one per
  nested `pom.properties` entry, with `mikebom:source-mechanism = "maven-jar-nested"` and the
  `!`-separated `mikebom:source-files` URL per FR-016.

### Side effects

- None. The walker materializes nested-archive bytes in memory but releases them as recursion unwinds.

### Logs

- `debug`-level: per nested archive successfully descended; logs outer path, inner path, depth.
- `warn`-level: per archive exceeding the 1 GB decompressed-size cap (FR-014). Names outer + inner
  path.
- `warn`-level: per nested-archive depth limit (8) being reached (FR-012). Names outer path.
- (Silent) per archive cycle detected (FR-013) — the SHA-256 visited-set returns immediately without
  logging. Cycle cases are exceedingly rare in practice; a log would only fire on pathological
  inputs.

### Invariants

- Same as Reader 1 (offline, exclude-path-aware, non-aborting).
- 8-level depth limit (FR-012).
- SHA-256-keyed visited set (FR-013).
- 1 GB per-archive size cap (FR-014).
- Only `.jar` / `.war` / `.ear` extensions descended (FR-017, clarification Q2). `.zip` NOT descended.
- Existing top-level reader behavior unchanged — only the recursive path is added.

---

## Reader 3: PE/CLR managed-assembly metadata (US3)

**Module path**: `mikebom-cli/src/scan_fs/binary/dotnet_pe_clr.rs` (NEW).

### Input

- Scan rootfs.
- The reader walks via `safe_walk` (milestone 114) for paths matching `*.dll` extension.
- Each matching file is opened via `object::read::pe::PeFile64::parse` (or `PeFile32::parse` per
  `IMAGE_OPTIONAL_HEADER.Magic`).
- `is_managed_assembly()` check (`DataDirectory[14]` non-zero) GATEs all subsequent metadata parsing.

### Output

- `Vec<PackageDbEntry>`, one entry per UNIQUE `(name, version)` post-culture-set-merge per the
  2026-06-18 clarification Q1.

### Side effects

- None.

### Logs

- `debug`-level: per managed assembly successfully parsed; logs `name`, `version`, `path`, `culture`.
- `debug`-level (no entry per skip): when a `.dll` is detected to be a native (non-CLR) DLL, the
  reader silently returns early.
- `warn`-level: per parse failure inside a CLR-tagged `.dll` (corrupt metadata table, missing
  `#Strings` heap, etc.). Names the file path. Scan does NOT abort.

### Invariants

- Same as Reader 1.
- `is_managed_assembly()` MUST return `bool` cheaply (~1 µs per `.dll`) — gated read of a single
  data-directory entry.
- Per the milestone-129 clarification Q3 + R3 fallback ladder: `assembly_informational_version →
  assembly_file_version → assembly_version`. ALL three are emitted as separate annotations when
  present (FR-021).
- Per the 2026-06-18 clarification Q1 + R4: when multiple DLLs share the same `(name, version)`,
  collapse via the intra-reader `AssemblyAccumulator` to a single component; the union of
  non-"neutral" non-empty cultures emits as `mikebom:assembly-cultures`.
- Cross-reader dedup (with milestone 129's `.deps.json` reader) is handled by the existing
  milestone-105 pipeline; the surviving component carries `mikebom:also-detected-via` with both
  source mechanisms.

### Test surface

- New integration test `binary_tier_completion_us3_dotnet_pe_clr.rs` covering:
  - Single-culture managed DLL → one component, NO `mikebom:assembly-cultures` annotation.
  - Multi-culture set (`Foo.Bar.dll` + `de/Foo.Bar.resources.dll` + `fr/Foo.Bar.resources.dll`) →
    one component, `mikebom:assembly-cultures = "de,fr"`.
  - Native non-managed DLL → silent skip (no log entry, no component).
  - Cross-reader dedup with `.deps.json` declaring the same package → one component with both source
    mechanisms in `mikebom:also-detected-via`.
  - Corrupt CLR metadata → warn-level log + scan continues.

---

## Cross-reader integration via the scan-orchestrator

All three reader paths plug into existing dispatchers:

- US1 is a fix to an existing call site at `binary/mod.rs:700`. No dispatch change.
- US2 extends the existing milestone-009 maven reader's per-JAR processing site. No dispatch change.
- US3 adds a new `binary::dotnet_pe_clr::read_all` entry to the binary-tier dispatcher at
  `binary/mod.rs` (same shape as the existing `binary::dotnet_pe::read_all` would have been; not
  yet implemented).

The milestone-105 dedup pipeline at `scan_fs/dedup.rs` handles cross-mechanism collisions
emergently — no dedup-pipeline changes for milestone 130.
