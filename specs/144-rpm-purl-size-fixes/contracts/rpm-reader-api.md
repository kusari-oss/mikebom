# Contract — `rpm_file.rs` reader API

Phase 1 output. Defines the Rust-internal API contract changes for the standalone-`.rpm`-file reader. Crate-internal but stable across this milestone's PR.

## `RpmReaderConfig` (new — `pub` within `rpm_file` module)

```rust
#[derive(Clone, Debug)]
pub struct RpmReaderConfig {
    /// Per-file size cap, in bytes. Files larger than this are skipped.
    /// Default: `DEFAULT_RPM_FILE_BYTES` (512 MiB).
    pub cap_bytes: u64,

    /// Operator-supplied distro identifier (from `--rpm-distro`). When
    /// `Some(s)`, overrides ALL other distro sources for vendor-slug
    /// resolution. Value MUST be non-empty and lowercased by the caller
    /// (clap value_parser enforces this).
    pub distro_override: Option<String>,
}

impl Default for RpmReaderConfig {
    fn default() -> Self {
        Self {
            cap_bytes: DEFAULT_RPM_FILE_BYTES,
            distro_override: None,
        }
    }
}
```

**Invariants**:
- `cap_bytes > 0` (clap-enforced upstream; constructor does NOT re-validate to avoid double-check overhead).
- `distro_override.as_ref().map_or(true, |s| !s.is_empty() && s == s.to_lowercase())` (clap-enforced upstream).
- `Default::default()` yields the same behavior as `--rpm-distro` and `--max-rpm-bytes` BOTH absent.

## `read` (signature change)

```rust
pub fn read(
    rootfs: &Path,
    distro_version: Option<&str>,
    config: &RpmReaderConfig,
) -> Vec<PackageDbEntry>;
```

**Behavioral contract**:
- Walks `rootfs` (single-file invocation also supported per existing semantics).
- For each `.rpm` candidate (extension + lead-magic match), invokes `parse_rpm_file(path, os_release_id, distro_version, config)`.
- Returns `Vec<PackageDbEntry>` containing only the successfully-parsed entries; skipped files contribute zero entries and one WARN log line each.

**Caller migration** (single caller in `package_db/mod.rs:1454`):

```diff
- out.extend(rpm_file::read(rootfs, distro_version.as_deref()));
+ let rpm_config = build_rpm_reader_config(scan_args);
+ out.extend(rpm_file::read(rootfs, distro_version.as_deref(), &rpm_config));
```

Where `build_rpm_reader_config(scan_args: &ScanArgs) -> RpmReaderConfig` is a small private helper in `mod.rs` that maps the `ScanArgs` Options to the config struct.

## `parse_rpm_file` (signature change, function private)

```rust
fn parse_rpm_file(
    path: &Path,
    os_release_id: Option<&str>,
    distro_version: Option<&str>,
    config: &RpmReaderConfig,
) -> Option<PackageDbEntry>;
```

**Behavioral changes vs today**:
1. Size check uses `config.cap_bytes` instead of the const `MAX_RPM_FILE_BYTES`.
2. WARN emission for the size-cap path uses the new `SkipReason::SizeCapExceeded { size, cap }` variant whose `Display` impl does NOT contain "malformed".
3. Vendor resolution calls the new 3-arg `resolve_rpm_vendor_slug(config.distro_override.as_deref(), os_release_id, vendor_header.as_deref())`.

## `resolve_rpm_vendor_slug` (signature change, function `pub`)

```rust
pub fn resolve_rpm_vendor_slug(
    cli_override: Option<&str>,
    os_release_id: Option<&str>,
    header_vendor: Option<&str>,
) -> (String, VendorSource);
```

**Precedence ladder** (strict; later sources consulted only if all earlier sources empty/absent):

1. `cli_override` → `(s.to_string(), VendorSource::CliOverride)` when `Some(s)` and `!s.is_empty()`.
2. `os_release_id` → `(rpm_vendor_from_id(id), VendorSource::OsRelease)` when `Some(id)` and `!id.is_empty()` and `rpm_vendor_from_id` returns non-empty.
3. `header_vendor` → `(slug, VendorSource::Header)` when `Some(v)` and v matches a `VENDOR_HEADER_MAP` prefix.
4. Else → `(String::new(), VendorSource::Fallback)`.

**Examples**:

```rust
assert_eq!(
    resolve_rpm_vendor_slug(Some("poky"), Some("fedora"), Some("CentOS")),
    ("poky".to_string(), VendorSource::CliOverride)
);

assert_eq!(
    resolve_rpm_vendor_slug(None, Some("fedora"), Some("CentOS")),
    ("fedora".to_string(), VendorSource::OsRelease)
);

assert_eq!(
    resolve_rpm_vendor_slug(None, None, Some("Red Hat, Inc.")),
    ("redhat".to_string(), VendorSource::Header)
);

assert_eq!(
    resolve_rpm_vendor_slug(None, None, None),
    (String::new(), VendorSource::Fallback)  // <-- KEY CHANGE: empty, not "rpm"
);
```

## `VendorSource` (variant addition)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VendorSource {
    CliOverride,  // NEW
    Header,
    OsRelease,
    Fallback,
}

impl VendorSource {
    pub fn as_str(self) -> &'static str {
        match self {
            VendorSource::CliOverride => "cli-override",  // NEW
            VendorSource::Header => "header",
            VendorSource::OsRelease => "os-release",
            VendorSource::Fallback => "fallback",
        }
    }
}
```

The `#[allow(dead_code)]` on `as_str` can be removed if the property-bag plumbing referenced in the existing line-66 comment is now wired up (verify during implementation; if not, leave the allow in place).

## `SkipReason` (new internal enum)

```rust
#[derive(Debug)]
enum SkipReason {
    StatFailed(std::io::Error),
    TruncatedLead { size: u64 },
    SizeCapExceeded { size: u64, cap: u64 },
    ParseFailed { reason: &'static str, error: String },
}

impl SkipReason {
    /// Stable structured-field value for the `reason="..."` log field.
    /// MUST NOT change across this milestone (FR-006 — log-parsing tools depend on it).
    fn structured_reason(&self) -> &'static str {
        match self {
            Self::StatFailed(_) => "stat-failed",
            Self::TruncatedLead { .. } => "truncated-lead",
            Self::SizeCapExceeded { .. } => "size-cap-exceeded",
            Self::ParseFailed { reason, .. } => reason,
        }
    }

    /// Human-readable WARN-message prefix. The "malformed" wording is
    /// reserved for genuinely-malformed files (FR-007); oversized files
    /// use "oversized" wording (FR-006).
    fn warn_prefix(&self) -> &'static str {
        match self {
            Self::SizeCapExceeded { .. } => "skipping oversized .rpm file",
            Self::StatFailed(_)
            | Self::TruncatedLead { .. }
            | Self::ParseFailed { .. } => "skipping malformed .rpm file",
        }
    }
}
```

**Invariant under this milestone**: `structured_reason()` MUST yield exactly the same strings the current code emits today (verify by grepping `reason = ` in `rpm_file.rs` lines 211, 221, 230, 243). This keeps log-parsing tools backward compatible.

## Const renames

| Old | New | Reason |
|---|---|---|
| `const MAX_RPM_FILE_BYTES: u64 = 200 * 1024 * 1024;` | `const DEFAULT_RPM_FILE_BYTES: u64 = 512 * 1024 * 1024;` | Const is no longer the maximum (operator can raise it); rename clarifies its role as the default. |
| `const MAX_RPMDB_BYTES: u64 = 200 * 1024 * 1024;` (in `rpm.rs:39`) | `const MAX_RPMDB_BYTES: u64 = 512 * 1024 * 1024;` | Value only; const name unchanged because rpmdb cap remains compile-time fixed (no operator override per FR-008). |

## Test surface contract

The implementation MUST add unit tests covering:

| Test | Asserts |
|---|---|
| `resolve_rpm_vendor_slug_cli_overrides_everything` | SC-012: CLI overrides os-release + per-RPM vendor |
| `resolve_rpm_vendor_slug_os_release_overrides_header` | SC-011: os-release overrides per-RPM RPMTAG_VENDOR |
| `resolve_rpm_vendor_slug_header_wins_when_no_cli_no_os_release` | Preserves existing behavior when neither override present |
| `resolve_rpm_vendor_slug_fallback_is_empty_not_rpm` | SC-002: fallback yields empty string, not `"rpm"` |
| `purl_omits_namespace_when_vendor_slug_empty` | SC-002: PURL constructor emits `pkg:rpm/<name>@<ver>` without double slash |
| `size_cap_exceeded_skips_file_without_malformed_in_warn` | SC-007 + FR-006: WARN does NOT contain "malformed", DOES contain `reason="size-cap-exceeded"` |
| `size_cap_at_boundary_includes_file` | Edge case: size == cap is included (strict `>`) |
| `default_rpm_file_bytes_is_512_mib` | FR-004: const guard against accidental revert |
| `max_rpmdb_bytes_is_512_mib` | FR-008 sibling raise guard in `rpm.rs` |

Plus the integration test described in research §R9 (`mikebom-cli/tests/rpm_file_yocto_regression.rs`).
