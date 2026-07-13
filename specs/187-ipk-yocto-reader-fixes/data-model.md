# Data Model: ipk reader — ar-format extraction + filename-fallback arch fix (m187)

**Feature**: [spec.md](./spec.md) · **Plan**: [plan.md](./plan.md) · **Research**: [research.md](./research.md)

## 1. New type — `ArMember`

**File**: `mikebom-cli/src/scan_fs/package_db/ipk_file.rs` (private helper)

```rust
/// One entry from an ar-format archive (BSD ar spec) — a member name +
/// its raw data body. m187 US1 (#543) — the primary parse type returned
/// by [`parse_ar_archive`].
///
/// Members are returned in the order they appear in the archive. Caller
/// scans for named members (`control.tar.gz`, `data.tar.gz`,
/// `debian-binary`) without assuming any specific ordering.
#[derive(Debug)]
struct ArMember {
    /// Member name, decoded from the 16-byte name field. Trailing slash
    /// (BSD ar convention for null-padding) is stripped. Never empty
    /// (the parser rejects empty names).
    name: String,
    /// Raw member data (unpadded). Length matches the header's decimal
    /// `size` field.
    data: Vec<u8>,
}
```

## 2. Enum update — `IpkParseError`

**File**: `mikebom-cli/src/scan_fs/package_db/ipk_file.rs` (existing enum, extend + rename)

Old:
```rust
pub(crate) enum IpkParseError {
    OuterMalformed(String),
    ControlMissing,
    #[allow(dead_code)]
    FilenameNonConforming,
    LegacyArFormat,           // ← RENAME per FR-016 / research Decision 4
    ControlOversize { actual: u64, cap: u64 },
}
```

New:
```rust
pub(crate) enum IpkParseError {
    OuterMalformed(String),
    ControlMissing,
    #[allow(dead_code)]
    FilenameNonConforming,
    /// m187 US1 — the ar-format primary parser detected valid magic but
    /// failed to enumerate members. Reasons include: truncated header,
    /// non-ASCII size field, size overrun body. Distinct from
    /// [`OuterMalformed`] which covers the pre-2015 `gzip(tar)` fallback
    /// path's failures.
    ArMalformed(String),
    /// m187 — the pre-2015 `gzip(tar)` outer-envelope fallback path was
    /// tried (after ar-format probe failed at magic-check) AND ALSO
    /// failed. Renamed from `LegacyArFormat` per FR-016.
    LegacyGzipTarFallbackFailed(String),
    ControlOversize { actual: u64, cap: u64 },
}
```

Display impls updated in parallel — each variant has a concrete error message including any embedded string reason.

## 3. Format-detection dispatch (updated `parse_ipk_file`)

**File**: `mikebom-cli/src/scan_fs/package_db/ipk_file.rs::parse_ipk_file`

**Flow (post-m187)**:
```
                    read file bytes (existing)
                         │
                         ▼
                 first 8 bytes == `!<arch>\n`?
                    /                    \
                  yes                     no
                   │                       │
                   ▼                       ▼
        parse_ar_archive()          old gzip(tar) parser
                   │                       │
      ┌────────────┼────────────┐          │
      │            │            │          │
      ▼            ▼            ▼          ▼
   Ok(members) Err(ArMalf)  (n/a)     Ok(...)  Err(OuterMalf)
      │            │                    │            │
      │            │                    ▼            ▼
      │            └───────────────► success   Err(IpkParseError::LegacyGzipTarFallbackFailed)
      │                                             │
      ▼                                             ▼
scan for control.tar[.gz] / data.tar[.gz]      caller falls
      │                                        through to
      ├── found → extract_control_file() (reused)   filename-fallback
      │            │                                (existing behavior)
      │            └── build_entry_from_control()
      │                (reused; source-mechanism = "ipk-file-archive-extraction")
      └── missing → Err(ControlMissing)
```

**Key changes**:
- Line 316 (old): `if bytes[..8] == "!<arch>\n" { return Err(LegacyArFormat); }` → REMOVED
- New: ar magic → `parse_ar_archive(bytes)` → on success, extract control member + reuse `extract_control_file` + `build_entry_from_control`.
- Old `gzip(tar)` path preserved as a secondary branch for the NON-ar case.
- The `mikebom:source-mechanism` value on ar-path success is `"ipk-file-archive-extraction"` (new). The gzip-tar path preserves `"ipk-file"` (existing value at line 479) — unchanged.

## 4. Suffix-match helper (US2 FR-010)

**File**: `mikebom-cli/src/scan_fs/package_db/ipk_file.rs` (new private helper)

```rust
/// m187 US2 (#542) — parent-directory arch-source disambiguation.
///
/// Returns `Some(prefix)` IFF `filename_no_ext` ends with
/// `_<parent_dir_name>` — i.e., the filename's last `_`-delimited
/// segment equals the parent-dir name byte-for-byte case-sensitive.
/// The returned `prefix` is everything BEFORE the matched `_<parent-dir-name>`
/// suffix (feeds the existing `<name>_<version>` LEFT-split logic).
///
/// Returns `None` when the suffix doesn't match. Caller then falls
/// through to the existing rsplit-based filename heuristic.
///
/// Rationale per Clarifications Q1: the Yocto convention emits
/// filenames with the arch as the last `_`-delimited segment AND
/// places each ipk in an `<arch>/` directory. This suffix-match
/// gate cleanly identifies parent-dir-as-arch without misfiring on
/// loose-file layouts.
fn parent_dir_arch_match<'a>(
    filename_no_ext: &'a str,
    parent_dir_name: &str,
) -> Option<&'a str> {
    let suffix = format!("_{parent_dir_name}");
    filename_no_ext.strip_suffix(&suffix)
}
```

## 5. Updated `parse_ipk_filename` (US2 FR-011)

**File**: `mikebom-cli/src/scan_fs/package_db/ipk_file.rs::parse_ipk_filename`

**Signature change**:
```rust
// Old: fn parse_ipk_filename(filename: &str) -> Option<(String, String, String)>
// New:
fn parse_ipk_filename(
    filename: &str,
    parent_dir_name: Option<&str>,  // ← NEW parameter
) -> Option<ParsedFilename>;

/// Return type carries the arch-source signal for FR-013 diagnostic
/// property emission.
struct ParsedFilename {
    name: String,
    version: String,
    arch: String,
    arch_source: ArchSource,
}

enum ArchSource {
    /// Parent-directory name matched the filename's arch suffix per
    /// FR-010 suffix-match gate.
    ParentDirectory,
    /// Fallback rsplit-based heuristic (no parent-dir agreement).
    FilenameHeuristic,
}
```

**Behavior**:
1. Strip `.ipk` suffix → `stem`.
2. If `parent_dir_name` is `Some` AND `parent_dir_arch_match(stem, parent)` returns `Some(prefix)`:
   - `arch = parent_dir_name.to_string()`
   - `arch_source = ParentDirectory`
   - Split `prefix` on first `_` for `<name>_<version>` → `(name, version)`
3. Else:
   - Existing rsplit-based logic runs on `stem` → `(name, version, arch)`
   - `arch_source = FilenameHeuristic`

## 6. Updated `filename_fallback_entry` (US2 FR-013)

**File**: `mikebom-cli/src/scan_fs/package_db/ipk_file.rs::filename_fallback_entry`

**Changes**:
- Extract parent-dir name from `path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str())`.
- Pass parent-dir name to `parse_ipk_filename`.
- Emit `mikebom:arch-source` property based on the returned `ArchSource` variant:
  - `ParentDirectory` → `mikebom:arch-source = "parent-directory"`
  - `FilenameHeuristic` → `mikebom:arch-source = "filename-heuristic"`
- The existing `mikebom:source-mechanism = "ipk-file-filename-fallback"` property is unchanged.

## 7. `mikebom:arch-source` property matrix

| Path taken | `mikebom:source-mechanism` | `mikebom:arch-source` |
|---|---|---|
| ar-format archive extraction (US1) | `ipk-file-archive-extraction` | `control-file` |
| pre-2015 `gzip(tar)` archive extraction | `ipk-file` (existing) | *(not emitted — FR-014 / SC-005 byte-identity)* |
| Filename fallback + parent-dir suffix match | `ipk-file-filename-fallback` | `parent-directory` |
| Filename fallback + no parent-dir match | `ipk-file-filename-fallback` | `filename-heuristic` |

**F9 analyze-report remediation note**: The gzip-tar (legacy) row deliberately omits `mikebom:arch-source` because emitting a new property on that code path would violate the FR-014 / SC-005 byte-identity guarantee for pre-m187 golden fixtures. The trade-off is that consumers grep-searching for `mikebom:arch-source` will not see it on legacy-format ipks — but the `mikebom:source-mechanism = "ipk-file"` value is a sufficient signal that the extraction path was authoritative (not the filename fallback).

## 8. `collect_claimed_paths` update

**File**: `mikebom-cli/src/scan_fs/package_db/ipk_file.rs::collect_claimed_paths` (line 204)

**Change**: Add a mirror ar-format branch alongside the existing gzip-tar one. Structure:

```rust
pub fn collect_claimed_paths(...) {
    for ipk_path in discover_ipk_files(rootfs) {
        let Ok(bytes) = std::fs::read(&ipk_path) else { continue };

        // m187 — ar-format primary path (was previously skipped via
        // `if bytes[..8] == "!<arch>\n" { continue; }`).
        if bytes.len() >= 8 && &bytes[..8] == b"!<arch>\n" {
            if let Ok(members) = parse_ar_archive(&bytes) {
                if let Some(data_member) = members.iter().find(|m|
                    m.name == "data.tar.gz" || m.name == "data.tar"
                ) {
                    // Same inner-tar walk as the existing gzip-tar branch,
                    // but operate on data_member.data instead of the
                    // `entry` stream. Populates `claimed` + `claimed_inodes`.
                    walk_data_tar(&data_member.data, ipk_path, rootfs,
                                  claimed, claimed_inodes);
                }
            }
            continue;
        }

        // Fall through to the existing gzip-tar path (unchanged).
        ...
    }
}
```

The `walk_data_tar` helper is a small extraction of the inner-tar-walking loop from the existing code — refactored out for reuse across both the ar and gzip-tar branches.

## 9. Test contract

### 9.1 Unit tests (colocated with `ipk_file.rs`)

- `parse_ar_archive_extracts_three_members` — happy path with `debian-binary` + `control.tar.gz` + `data.tar.gz`.
- `parse_ar_archive_tolerates_missing_debian_binary` — 2-member archive (control + data); succeeds.
- `parse_ar_archive_handles_uncompressed_inner_tar` — `control.tar` (no .gz) + `data.tar` — succeeds; downstream still extracts the control file.
- `parse_ar_archive_rejects_truncated_header` — 8-byte magic + 20 bytes = truncated first header; returns `ArError::TruncatedHeader`.
- `parse_ar_archive_rejects_non_ascii_size` — malformed header with non-decimal size field.
- `parent_dir_arch_match_matches_yocto_convention` — `filename="foo_1.0-r0_qemux86_64"`, parent=`"qemux86_64"` → `Some("foo_1.0-r0")`.
- `parent_dir_arch_match_rejects_loose_layout` — `filename="foo_1.0_all"`, parent=`"downloads"` → `None`.
- `parent_dir_arch_match_handles_multi_underscore_arch` — `filename="foo_1.0_powerpc_e500v2"`, parent=`"powerpc_e500v2"` → `Some("foo_1.0")`.
- `parse_ipk_filename_uses_parent_dir_when_suffix_matches` — full round-trip with parent-dir signal.
- `parse_ipk_filename_falls_back_to_rsplit_when_no_parent_match` — pre-m187 behavior preserved for loose-file layouts.

### 9.2 Integration tests (`mikebom-cli/tests/ipk_yocto_reader_fixes.rs`)

- `us1_ar_format_extracts_control_metadata` — synthesize a modern ar-format ipk with `License: GPL-2.0-only & bzip2-1.0.4`, `Depends: libc6 (>= 2.39), update-alternatives-opkg`, `Recommends: busybox-udhcpc`, scan it, assert emitted component has `licenses[]` populated + `mikebom:source-mechanism = "ipk-file-archive-extraction"` + depends edges.
- `us1_ar_format_tolerates_missing_debian_binary` — ar archive with only `control.tar.gz` + `data.tar.gz`; scan succeeds with license extracted + WARN log flagging missing `debian-binary`.
- `us1_pre_2015_gzip_tar_still_works` — synthesize a gzip(tar)-wrapped ipk (the OLD format); scan succeeds with existing `mikebom:source-mechanism = "ipk-file"` (byte-identical to pre-m187).
- `us1_malformed_ar_falls_through_to_filename` — ar archive with truncated header; scan produces a filename-fallback component + WARN log naming `ar-format:` reason (NOT the old "legacy ar-format" language).
- `us2_qemux86_64_arch_extracted_from_parent_dir` — malformed ar body inside `<tempdir>/qemux86_64/kernel-1.0-r0_qemux86_64.ipk`; scan produces `?arch=qemux86_64`, `version=1.0-r0`, `mikebom:arch-source = "parent-directory"`.
- `us2_powerpc_e500v2_arch_extracted_from_parent_dir` — same as above with `powerpc_e500v2` arch; verifies multi-underscore arches are handled.
- `us2_no_parent_dir_match_falls_back_to_filename_heuristic` — malformed ar body inside `<tempdir>/downloads/foo_1.0_all.ipk`; scan produces `?arch=all` via filename rsplit, `mikebom:arch-source = "filename-heuristic"`.
- `us2_arch_source_control_file_when_ar_succeeds` — well-formed ar with `Architecture: qemux86_64` in control file inside a directory of a DIFFERENT name; scan produces `?arch=qemux86_64` from control file, `mikebom:arch-source = "control-file"` (US1 wins over US2 per FR-005).
- `regression_us1_us2_combined_yocto_scan` — synthesize 3 ipks: (a) ar-format + qemux86_64 parent, (b) ar-format + core2-64 parent, (c) legacy gzip-tar + core2-64 parent; scan all three; assert (a) + (b) go archive-extraction, (c) goes gzip-tar path; all three have correct arch + `mikebom:source-mechanism` values.

## 10. Backward compatibility

- **Pre-m187 golden fixtures**: no existing fixture uses the new properties (`ipk-file-archive-extraction`, `arch-source`), so byte-identity for gzip-tar-format ipks is preserved. Verified via T031 golden-regen zero-drift check.
- **Existing `IpkParseError::LegacyArFormat` matchers**: variant is renamed to `LegacyGzipTarFallbackFailed` — compiler forces every match arm to update, preventing runtime "wrong branch fires" outcomes.
- **`parse_ipk_filename` signature change** (added `parent_dir_name` parameter): only one caller (`filename_fallback_entry` in the same file). Updated in-place.
- **Existing `ipk-file` `source-mechanism` value**: preserved on the pre-2015 gzip-tar path. Post-m187 output for legacy ipks matches pre-m187 output byte-for-byte.
