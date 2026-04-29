# Phase 1 Data Model: Minimal-Image Per-File Evidence

**Feature**: 038-minimal-image-deep-hash

This milestone introduces no new public types and no new persisted
data shapes. All changes are internal to the deb-deep-hash code
path. This document records the function-signature changes and
the lookup-precedence model so a future maintainer can navigate
the new behavior without re-deriving it from code.

## Existing types (unchanged)

- `mikebom_common::resolution::FileOccurrence` —
  `{ location: String, sha256: String, md5_legacy: Option<String> }`.
  Identical shape across legacy and status.d/ paths.
- `mikebom_common::types::hash::ContentHash` — Merkle root over
  occurrences. Identical across paths.
- `mikebom_common::types::purl::Purl` — package identity. Already
  layout-agnostic.

## Function signatures (modified)

### `read_info_file(rootfs, pkg_name, arch, ext) -> Option<String>`

**Before** (milestone 037 era):

Lookup chain:
1. `<rootfs>/var/lib/dpkg/info/<pkg>.<ext>`
2. `<rootfs>/var/lib/dpkg/info/<pkg>:<arch>.<ext>` (if `arch` is
   `Some`)
3. → `None`

**After** (milestone 038):

Lookup chain:
1. `<rootfs>/var/lib/dpkg/info/<pkg>.<ext>`
2. `<rootfs>/var/lib/dpkg/info/<pkg>:<arch>.<ext>` (if `arch` is
   `Some`)
3. `<rootfs>/var/lib/dpkg/status.d/<pkg>.<ext>` *(new)*
4. → `None`

The signature is unchanged; only the lookup chain extends. All
existing call sites work without modification.

### `read_info_file_bytes(rootfs, pkg_name, arch, ext) -> Option<Vec<u8>>`

Same change as `read_info_file` — adds the status.d/ fallback as
the third (final) lookup step.

### `hash_package_files(rootfs, pkg_name, arch) -> (Vec<FileOccurrence>, Option<ContentHash>)`

**Behavior change**: when `read_info_file(.., "list")` returns
`None` (no `info/<pkg>.list` and no
`status.d/<pkg>.list` — the latter doesn't exist in practice),
fall back to deriving the path list from
`read_info_file(.., "md5sums")` (which now finds
`status.d/<pkg>.md5sums`).

The path-derivation is a one-line transformation: each line is
`<32-hex-md5>\s+<relative-path>`; take the second whitespace-
delimited field and trim. Empty / malformed lines are silently
skipped (same posture as the existing
`read_md5sums` parser).

The Merkle-root computation is unchanged.

### `hash_md5sums_only(rootfs, pkg_name, arch) -> Option<ContentHash>`

**No behavior change** beyond the lookup chain extension. The
function calls `read_info_file_bytes(.., "md5sums")` which now
finds `status.d/<pkg>.md5sums`, hashes the raw bytes, and returns
`ContentHash::sha256`. Spec FR-003 satisfied by this transparent
path extension.

## Lookup precedence (deep-hash mode)

For a package `<pkg>` with optional `<arch>`:

```text
list path source (in priority order):
  1. var/lib/dpkg/info/<pkg>.list
  2. var/lib/dpkg/info/<pkg>:<arch>.list
  3. var/lib/dpkg/status.d/<pkg>.list             [does not exist in practice]
  4. <derive paths from md5sums>:
     a. var/lib/dpkg/info/<pkg>.md5sums
     b. var/lib/dpkg/info/<pkg>:<arch>.md5sums
     c. var/lib/dpkg/status.d/<pkg>.md5sums       [the distroless source]
  5. → empty occurrences

md5 lookup source (for evidence.md5_legacy):
  1. var/lib/dpkg/info/<pkg>.md5sums
  2. var/lib/dpkg/info/<pkg>:<arch>.md5sums
  3. var/lib/dpkg/status.d/<pkg>.md5sums
  4. → empty map (md5_legacy = None on every occurrence)
```

The synthesize-from-md5sums step (#4 above) is the only
behaviorally novel logic in this milestone. Everything else is
path-extension-driven.

## US2 — apko apk variant data model

**Resolved at implementation time per R2.** If the recon step
discovers a non-standard apko layout, this section will be
populated with the corresponding lookup chain for the apk reader.
If the recon shows apko uses the standard layout, this section
remains empty (no data-model change for the apk path).
