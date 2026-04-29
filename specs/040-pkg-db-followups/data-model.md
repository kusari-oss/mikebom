# Phase 1 Data Model: Package-DB Follow-Ons

**Feature**: 040-pkg-db-followups

This milestone introduces no new public types and no SBOM-schema
changes. All shape extensions are internal to the package-db
readers and the per-file evidence pipeline.

## Existing types (unchanged)

- `mikebom_common::resolution::FileOccurrence` —
  `{ location: String, sha256: String, md5_legacy: Option<String> }`.
  Identical shape across deb/apk/rpm. The `additionalContext`
  on-the-wire is a JSON-string serialized from these fields plus
  any ecosystem-specific cross-refs.
- `PackageDbEntry`, `Purl`, `ContentHash`, etc. — all unchanged.

## Type signature changes

### `apk::read_file_lists`

**Before** (milestone 039):

```rust
pub fn read_file_lists(rootfs: &Path) -> HashMap<String, Vec<String>>
```

**After** (milestone 040 US2):

```rust
pub fn read_file_lists(rootfs: &Path)
    -> HashMap<String, Vec<ApkFileEntry>>;

#[derive(Clone, Debug)]
pub struct ApkFileEntry {
    pub path: String,
    pub sha1: Option<String>,  // 40-hex; None if Z: line absent
}
```

The map's value-vec changes from `String` to `ApkFileEntry`.
Callers in `scan_fs/mod.rs` need a small mechanical update.

### `file_hashes.rs::hash_apk_package_files`

**Before**:

```rust
pub fn hash_apk_package_files(rootfs: &Path, files: &[String])
    -> (Vec<FileOccurrence>, Option<ContentHash>)
```

**After**:

```rust
pub fn hash_apk_package_files(rootfs: &Path, files: &[ApkFileEntry])
    -> (Vec<FileOccurrence>, Option<ContentHash>)
```

The `additionalContext` JSON-string emitter (in
`generate/cyclonedx/evidence.rs`) gains a new path: when the
occurrence's md5_legacy is None AND a sha1 is available, emit
`additionalContext = "{\"sha256\":\"…\",\"sha1\":\"…\"}"`.
For dpkg occurrences with md5_legacy populated, the existing
shape `{"sha256":"…","md5":"…"}` is unchanged.

The `FileOccurrence` struct itself doesn't grow — the sha1 lives
on a per-occurrence basis only inside the
`additionalContext` JSON-string at emission time.

### Rpm functions (new)

```rust
// in rpm.rs (US3)
pub fn read_file_lists(rootfs: &Path) -> HashMap<String, Vec<String>>;

// in file_hashes.rs (US3)
pub fn hash_rpm_package_files(rootfs: &Path, files: &[String])
    -> (Vec<FileOccurrence>, Option<ContentHash>);
pub fn hash_rpm_db_only(rootfs: &Path, pkg_name: &str)
    -> Option<ContentHash>;
```

The rpm helpers mirror the apk pre-040 shape (no per-file
checksum cross-ref); the FILEDIGESTS extension is deferred per
the Q1 clarification.

### Carrying SHA-1 through evidence emission

The path is:

1. `apk::read_file_lists` returns `Vec<ApkFileEntry>`.
2. `file_hashes::hash_apk_package_files` walks the entries,
   computes SHA-256 from the rootfs file content, and produces
   `FileOccurrence`s. Each occurrence's `md5_legacy` field stays
   `None` (apk doesn't ship MD5).
3. To carry the SHA-1, we need either:
   - **Option A**: extend `FileOccurrence` with a new
     `apk_sha1: Option<String>` field. Cleanest typing but
     requires touching `mikebom-common`.
   - **Option B**: serialize the SHA-1 INSIDE the
     `additionalContext` JSON-string at the emission site.
     `FileOccurrence` stays unchanged; the emitter knows about
     the SHA-1 because it's threaded through alongside the
     occurrence in a parallel `Vec<Option<String>>`.

The clearer choice is **Option A** — a typed field that survives
all downstream serializers without special-casing in each one.
Mikebom-common is a tiny crate and adding one optional field
is a low-risk schema extension. The dpkg `md5_legacy` field
already exists there; adding `apk_sha1` is the same shape.

## Lookup precedence (rpm deep-hash mode)

For an rpm package `<pkg>`:

```text
file-list source (only one possibility for rpm):
  1. rpm.rs::read_file_lists exposes the BASENAMES/DIRNAMES/
     DIRINDEXES decoded triple; one map entry per package.
  2. → empty file list (metadata-only package)

fast-path (hash_rpm_db_only):
  1. SHA-256 of the package's HeaderBlob bytes — opaque
     per-package identity hash.
  2. → None if the named package is absent from the rpmdb.
```
