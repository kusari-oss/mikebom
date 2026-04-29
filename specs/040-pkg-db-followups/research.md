# Phase 0 Research: Package-DB Follow-Ons (Trifecta)

**Feature**: 040-pkg-db-followups

## R1 — Stale OCI comment locations

**Question**: Which file/line carries the stale "deferred to
milestone 031.y" string for US1?

**Decision**: `mikebom-cli/src/scan_fs/oci_pull/mod.rs:215` —
inside the `host_oci_arch` function's error message. Verified
via `grep -rn 'deferred to milestone 031' mikebom-cli/src/`
which returns exactly one match. The grep regex from spec
SC-001 is the post-merge verification.

**Action**: rewrite the bail message to point at the shipped
`--image-platform` flag instead of the stale milestone
identifier.

## R2 — apk Z:-line wire format

**Question**: How does apk encode per-file checksums on the `Z:`
line, and how does mikebom decode them for `additionalContext`?

**Decision**: per the apk source (`src/apk-tools/src/database.c`)
and the open-spec at <https://wiki.alpinelinux.org/wiki/APKBUILD/format>,
apk `Z:` lines carry:

```
Z:Q1<base64-encoded-bytes>
```

where `Q1` is a single-byte version prefix (literally the ASCII
characters "Q1") and the trailing bytes are base64-encoded
SHA-1 (20 bytes → 28 base64 chars, including 1 `=` padding).
Implementation:

1. Strip the `Q1` prefix.
2. Base64-decode the tail (using the workspace's existing
   `base64` dep).
3. Hex-encode the resulting 20 bytes → 40-char lowercase string.
4. Carry as `sha1` in the per-file `additionalContext` JSON-string.

**Edge cases**:
- `Z:Q1` followed by an empty value → drop (no SHA-1 available).
- Other `Q<digit>` prefixes — defensive: reject non-`Q1` values
  silently; mikebom emits the file with `sha256` only.
- Multi-line `Z:` continuations (don't exist in apk's format) —
  not handled.

## R3 — rpm crate BASENAMES / DIRNAMES / DIRINDEXES API

**Question**: How does mikebom's existing `rpm` crate access
expose per-package file paths?

**Decision**: mikebom's own `rpm.rs::iter_rpmdb` already
extracts these. The visitor signature is:

```rust
fn iter_rpmdb<F>(rootfs: &Path, distro_version: Option<&str>, mut visitor: F)
where
    F: FnMut(PackageDbEntry, Vec<PathBuf>),
```

The `Vec<PathBuf>` argument is the concrete file list
reconstructed from the `(BASENAMES, DIRNAMES, DIRINDEXES)`
triple via mikebom's `header_blob.rs` decoder. `collect_claimed_paths`
already consumes it.

**Action**: add a sibling `rpm::read_file_lists(rootfs) ->
HashMap<String, Vec<String>>` that uses the same helper and
returns the per-package map keyed on `entry.name`. ~25 LOC.

For the fast-path (`hash_rpm_db_only`), the rpm db is a SQLite
file (or BDB legacy). The simplest stable per-package digest is
the SHA-256 of the package's HeaderBlob bytes — mikebom already
has this byte stream available during `iter_rpmdb` (one row per
package; the blob is the second SQLite column). Extending
`iter_rpmdb` to expose the blob bytes to a different visitor
shape would let `hash_rpm_db_only` SHA-256 them without
re-reading the db.

**Alternatives considered**:

- *Re-parse the HeaderBlob inside `hash_rpm_package_files`*:
  rejected. Re-parsing is wasted work — mikebom already does it
  during the main scan path. Threading the file list through the
  existing visitor is cleaner.
- *Use the rpm crate's `rpm::Package` directly instead of going
  through `iter_rpmdb`*: rejected. The crate's high-level API
  expects standalone `.rpm` package files, not the rpmdb format
  mikebom reads. Mikebom's `iter_rpmdb` is the right entry point.

## R4 — `is_rpm` source-path detector

**Question**: What pattern in `entry.source_path` reliably
identifies an rpm component?

**Decision**: substring match on `var/lib/rpm/` or
`usr/lib/sysimage/rpm/`. Both possibilities are enumerated in
`rpm.rs::RPMDB_SQLITE_CANDIDATES` and `RPMDB_BDB_CANDIDATES`:

```
var/lib/rpm/rpmdb.sqlite
var/lib/rpm/Packages
usr/lib/sysimage/rpm/rpmdb.sqlite
…
```

Combining via a substring check on `"rpm/"` would be
ambiguous (matches dpkg's `var/lib/rpm` … but dpkg doesn't have
that path). Use the more specific `"lib/rpm/"` substring; it
matches all rpm rootfs layouts and excludes anything else.

**Action**: in `scan_fs/mod.rs`, add `let is_rpm =
entry.source_path.contains("lib/rpm/");` parallel to
`is_dpkg` and `is_apk`.

## R5 — Fast-path identity for rpm

**Question**: What does `hash_rpm_db_only` SHA-256 to produce a
package-level identity hash analogous to dpkg's `.md5sums`
content or apk's stanza bytes?

**Decision**: SHA-256 of the per-package HeaderBlob bytes
(opaque). The HeaderBlob is the canonical per-package serialized
form and is what `iter_rpmdb` already extracts. Two packages
with identical metadata produce identical HeaderBlob bytes →
identical hash → useful as a stable per-package identity claim.

**Implementation note**: extend `iter_rpmdb` to also pass the
HeaderBlob byte slice to its visitor, OR add a parallel
`iter_rpmdb_blobs` helper that yields just `(name, blob_bytes)`
pairs for the fast-path lookup. Choose whichever produces less
churn at implementation time.

## Open questions resolved at clarification

Q1 (FILEDIGESTS scope): deferred per session 2026-04-29.
Affects only US3 spec — no implementation impact.
