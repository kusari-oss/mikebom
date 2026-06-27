# Data Model — milestone 144

Phase 1 output. Identifies the new in-memory types introduced by this milestone and how they relate to existing structs.

## New types

### `RpmReaderConfig` (private to `rpm_file.rs`)

Per-scan configuration bundle threaded from `ScanArgs` through `read_all` to the standalone-`.rpm` reader. Constructed once per scan; immutable thereafter.

| Field | Type | Origin | Default | Validation |
|---|---|---|---|---|
| `cap_bytes` | `u64` | `ScanArgs.max_rpm_bytes` OR `DEFAULT_RPM_FILE_BYTES` (512 MiB) | `512 * 1024 * 1024` | `> 0`, clap-side rejection (R6) |
| `distro_override` | `Option<String>` | `ScanArgs.rpm_distro` | `None` | non-empty + lowercased by clap value_parser (R6 + R8) |

Lifecycle: created in `package_db::mod::read_all` once per scan, borrowed (`&RpmReaderConfig`) into every reader call. Dies when the scan completes. **No persistence, no cache.**

### `SkipReason` (private to `rpm_file.rs`)

Internal enum encoding why a `.rpm` file was skipped during parse. Used by the WARN-emission helper to keep WARN text testable without `tracing` subscriber plumbing (R4 + R10).

| Variant | Carries | WARN-text contains "malformed"? | Structured `reason=` |
|---|---|---|---|
| `StatFailed(std::io::Error)` | the io error | YES | `stat-failed` |
| `TruncatedLead { size: u64 }` | observed size | YES | `truncated-lead` |
| `SizeCapExceeded { size: u64, cap: u64 }` | observed size + active cap | **NO** (new wording: "skipping oversized .rpm file") | `size-cap-exceeded` |
| `ParseFailed { reason: &'static str, error: String }` | classifier output + display | YES | `<classify_rpm_error output>` |

`impl Display for SkipReason` produces the human-readable WARN-text prefix (NOT the structured field). Tests assert on `format!("{}", reason)` for substring presence/absence.

### `DistroId` (internal helper, optionally a newtype)

**Decision deferred to implementation**: may or may not be a separate `#[derive(Debug, Clone)] struct DistroId(String)` newtype. The argument for a newtype is Constitution Principle IV (Type-Driven Correctness) — `String` should not flow across function boundaries for spec-bounded values. The argument against is YAGNI — this string flows through exactly one helper call (`resolve_rpm_vendor_slug`) and is then folded into a `Purl::new()` validated `String`.

**Recommendation for implementation**: skip the newtype for now; if the value flows across >2 function boundaries in a future milestone, promote it then. This matches the project's existing handling of `os-release ID=` (no `OsReleaseId` newtype in `os_release.rs`).

## Modified types

### `ScanArgs` (in `mikebom-cli/src/cli/scan_cmd.rs:65`)

Two new fields:

```rust
/// Override the auto-detected distro for RPM PURL namespaces. When set,
/// overrides /etc/os-release `ID=` AND per-RPM RPMTAG_VENDOR/PACKAGER
/// metadata across the entire scan. Typical Yocto use: --rpm-distro poky.
#[arg(long, value_name = "ID", value_parser = parse_non_empty_lowercase_distro_id)]
pub rpm_distro: Option<String>,

/// Per-file size cap for standalone .rpm files (bytes). Default 512 MB
/// (accommodates Yocto debug RPMs up to ~400 MB with margin). Files
/// exceeding the cap are skipped with a structured WARN log.
#[arg(long, value_name = "BYTES", value_parser = parse_nonzero_u64)]
pub max_rpm_bytes: Option<u64>,
```

Both `Option<T>` so absence means "use the reader default" (the const `DEFAULT_RPM_FILE_BYTES` lives in `rpm_file.rs`, not in `cli/scan_cmd.rs` — keeps the default close to its enforcement site).

### `resolve_rpm_vendor_slug` (signature change in `rpm_file.rs`)

**Before** (line 96):

```rust
pub fn resolve_rpm_vendor_slug(
    header_vendor: Option<&str>,
    os_release_id: Option<&str>,
) -> (String, VendorSource);
```

**After**:

```rust
pub fn resolve_rpm_vendor_slug(
    cli_override: Option<&str>,        // NEW — highest precedence
    os_release_id: Option<&str>,       // unchanged position, but precedence raised above header
    header_vendor: Option<&str>,       // unchanged value, but precedence lowered below os_release
) -> (String, VendorSource);
```

Note the argument-order change is **intentional** — it mirrors the new strict precedence order specified in the spec's Key Entities section. The new ladder body is:

1. If `cli_override` is `Some(s)` with `!s.is_empty()` → `(s.to_string(), VendorSource::CliOverride)` (new variant; see below).
2. Else if `os_release_id` is `Some(id)` and `rpm_vendor_from_id(id)` returns non-empty → `(slug, VendorSource::OsRelease)`.
3. Else if `header_vendor` matches `VENDOR_HEADER_MAP` prefix → `(slug, VendorSource::Header)`.
4. Else → `(String::new(), VendorSource::Fallback)` — the `String::new()` is the key behavioral change from today's `("rpm".to_string(), VendorSource::Fallback)`.

### `VendorSource` (enum extension)

Add one variant: `VendorSource::CliOverride`. Update `as_str()`:

```rust
match self {
    VendorSource::CliOverride => "cli-override",
    VendorSource::Header => "header",
    VendorSource::OsRelease => "os-release",
    VendorSource::Fallback => "fallback",
}
```

### `read` (signature change in `rpm_file.rs`)

**Before** (line 121):

```rust
pub fn read(rootfs: &Path, distro_version: Option<&str>) -> Vec<PackageDbEntry>;
```

**After**:

```rust
pub fn read(
    rootfs: &Path,
    distro_version: Option<&str>,
    config: &RpmReaderConfig,
) -> Vec<PackageDbEntry>;
```

### `parse_rpm_file` (signature change in `rpm_file.rs`)

**Before** (line 200):

```rust
fn parse_rpm_file(
    path: &Path,
    os_release_id: Option<&str>,
    distro_version: Option<&str>,
) -> Option<PackageDbEntry>;
```

**After**:

```rust
fn parse_rpm_file(
    path: &Path,
    os_release_id: Option<&str>,
    distro_version: Option<&str>,
    config: &RpmReaderConfig,
) -> Option<PackageDbEntry>;
```

### PURL constructor (lines 335–343)

**Before**:

```rust
let purl_str = format!(
    "pkg:rpm/{}/{}@{}?arch={}{}{}",
    percent_encode_purl_segment(&vendor_slug),
    mikebom_common::types::purl::encode_purl_segment(&name),
    ...
);
```

**After** (R5 — conditional namespace omission):

```rust
let purl_str = if vendor_slug.is_empty() {
    format!(
        "pkg:rpm/{}@{}?arch={}{}{}",
        mikebom_common::types::purl::encode_purl_segment(&name),
        mikebom_common::types::purl::encode_purl_segment(&version_tok),
        percent_encode_purl_qualifier(&arch),
        epoch_seg,
        distro_seg,
    )
} else {
    format!(
        "pkg:rpm/{}/{}@{}?arch={}{}{}",
        percent_encode_purl_segment(&vendor_slug),
        mikebom_common::types::purl::encode_purl_segment(&name),
        mikebom_common::types::purl::encode_purl_segment(&version_tok),
        percent_encode_purl_qualifier(&arch),
        epoch_seg,
        distro_seg,
    )
};
```

### Const renames / additions

- `const MAX_RPM_FILE_BYTES: u64 = 200 * 1024 * 1024` (line 37) → renamed to `const DEFAULT_RPM_FILE_BYTES: u64 = 512 * 1024 * 1024`. The "MAX_" prefix is misleading once the cap is operator-overridable.
- `const MAX_RPMDB_BYTES: u64 = 200 * 1024 * 1024` (in `rpm.rs:39`) → raised to `512 * 1024 * 1024`. The const is NOT renamed (rpmdb cap stays "MAX_" because rpmdb has no operator override per FR-008).

## Relationships

```text
ScanArgs (clap)
    │
    │  --rpm-distro / --max-rpm-bytes
    ▼
read_all (package_db/mod.rs)
    │
    │  builds RpmReaderConfig { cap_bytes, distro_override } once per scan
    ▼
rpm_file::read(rootfs, distro_version, &config)
    │
    │  passes config into parse_rpm_file
    ▼
parse_rpm_file(path, os_release_id, distro_version, &config)
    │
    │  size check uses config.cap_bytes
    │  vendor resolution uses config.distro_override.as_deref()
    ▼
resolve_rpm_vendor_slug(cli_override, os_release_id, header_vendor) → (vendor_slug, source)
    │
    ▼
PURL constructor — branches on vendor_slug.is_empty()
    │
    ▼
Purl::new(&purl_str) → validation → PackageDbEntry
```

## State transitions

None — everything is constructed once per scan and read-only thereafter.

## Validation rules (consolidated from spec FRs)

| Input | Rule | Source |
|---|---|---|
| `--rpm-distro <ID>` | Non-empty, lowercased | FR-003 + R6 + R8 |
| `--max-rpm-bytes <BYTES>` | `> 0`, parseable as u64 | FR-005 + R6 |
| Per-RPM file size | `MIN_RPM_FILE_BYTES ≤ size ≤ config.cap_bytes` | FR-004 + existing FR-007/FR-011 |
| Resolved `vendor_slug` | Either empty (omit namespace) OR non-empty + percent-encoded | FR-001 + R5 |

## Out of model

- Per-subtree distro selection — explicitly out of scope (spec Out of Scope §1).
- Operator override of `MAX_RPMDB_BYTES` — explicitly out of scope (spec Out of Scope §2 + FR-008).
- `mikebom:*` properties for size-cap / distro metadata — explicitly out of scope (spec Out of Scope §5).
