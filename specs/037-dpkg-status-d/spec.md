---
description: "dpkg reader extension: support /var/lib/dpkg/status.d/ per-package metadata layout for distroless / chainguard / Bazel-built minimal images"
status: spec
milestone: 037
closes: "#64"
---

# Spec: dpkg `status.d/` reader (distroless coverage)

## Background

mikebom's dpkg reader currently checks only the legacy single-file
location `/var/lib/dpkg/status` (`mikebom-cli/src/scan_fs/package_db/dpkg.rs:21`).
When that file is absent, `read()` returns `Ok(Vec::new())` —
producing zero deb components for the entire image.

Modern minimal-image builds (rules-distroless, chainguard's apko,
some Bazel pipelines) ship package metadata as one stanza per file
at `/var/lib/dpkg/status.d/<pkgname>` instead of concatenating into
the single `status` file. The grammar is identical — each
`status.d/<pkgname>` file is exactly one stanza in the same
RFC-822-style format the existing `parse_stanza` already handles.

Surfaced during milestone 031 smoke testing on
`gcr.io/distroless/static-debian12:latest` — mikebom reported 0
components when 4 (`base-files`, `media-types`, `netbase`,
`tzdata`) are actually documented in `status.d/`. Both syft and
trivy pick these up correctly, so this is a documented coverage
gap mikebom should close.

## User story (US1, P1)

**As an SBOM consumer scanning a distroless / chainguard /
Bazel-built minimal container image**, I want mikebom to report
the deb packages whose metadata lives in
`/var/lib/dpkg/status.d/<pkgname>` files, so my SBOMs reflect what
the image actually carries — matching syft / trivy output for the
same image.

**Why P1**: this is correctness, not polish. distroless and similar
minimal images are widely deployed (rules-distroless ships them as
the recommended Bazel container base; major cloud vendors document
them as best practice). Reporting 0 components for an image with
4 packages is a real data-quality bug.

### Independent test

After implementation:
- `mikebom sbom scan --image gcr.io/distroless/static-debian12:latest --output distroless.cdx.json`
  produces a CDX with components for at least the 4 documented
  packages (`base-files`, `media-types`, `netbase`, `tzdata`).
- Each is a valid `pkg:deb/debian/<name>@<version>?distro=debian-12&arch=<arch>` PURL.
- The existing `pulls_distroless_static_and_emits_well_formed_sbom_with_zero_components`
  smoke-test assertion is updated: it now asserts `>= 4` components,
  not `== 0`. (The test name itself becomes stale — rename
  appropriately.)
- A new fixture-driven test in `dpkg.rs`'s inline tests covers a
  synthetic `status.d/`-shaped rootfs and asserts the same 4
  components emerge.

## Acceptance scenarios

**Scenario 1: status.d/-only image → 4 components**
```
Given: a rootfs with /var/lib/dpkg/status.d/base-files,
       /var/lib/dpkg/status.d/media-types, etc., and no
       /var/lib/dpkg/status file
When:  mikebom sbom scan --path <rootfs>
Then:  CDX contains 4 deb components, one per status.d/<pkg> file,
       with valid PURLs.
```

**Scenario 2: legacy status-only image → unchanged**
```
Given: a rootfs with /var/lib/dpkg/status (no status.d/ dir)
When:  mikebom sbom scan --path <rootfs>
Then:  output is byte-identical to current behavior. (The
       byte-identity goldens in tests/fixtures/27 cover this.)
```

**Scenario 3: both present → both contribute, deduped by package name**
```
Given: a rootfs with /var/lib/dpkg/status containing pkg-a and
       /var/lib/dpkg/status.d/pkg-b
When:  mikebom sbom scan --path <rootfs>
Then:  CDX contains 2 components (pkg-a from status, pkg-b from
       status.d/). If both sources mention the same package
       (rare in practice — a real dpkg image has one or the
       other, not both), the status.d/ stanza wins (per-file
       metadata is the modern convention; status is legacy).
```

**Scenario 4: status.d/ with non-stanza files → tolerated**
```
Given: /var/lib/dpkg/status.d/ contains *.md5sums companion files
       (e.g. base-files.md5sums) alongside the per-package stanzas
When:  mikebom reads the directory
Then:  files whose name ends in `.md5sums` (or any extension other
       than the bare package name) are skipped. Files that don't
       parse as a valid stanza are tolerated (not crashed) and
       logged at tracing::debug.
```

**Scenario 5: status.d/ entry without `Status: install ok installed`**
```
Given: a status.d/<pkg> file whose Status field is something
       other than "install ok installed" (e.g. "deinstall ok
       config-files")
When:  mikebom reads the directory
Then:  the entry is filtered out exactly as the legacy reader
       filters out non-installed entries from /var/lib/dpkg/status.
       Reuse parse_stanza's existing Status-filter logic.
```

## Edge cases

- **Empty `status.d/` directory.** Treat as no entries; no error.
  Same as missing-`status`-file: `read()` continues with whatever
  the other source(s) yielded.
- **Non-UTF-8 filenames in `status.d/`.** Skip with a debug log;
  Linux package names are restricted to ASCII anyway.
- **Symlink to a file outside the rootfs.** The dpkg reader uses
  `fs::read_to_string` which follows symlinks. Inside a rootfs
  scanned by `--path` this is generally safe; for `--image` flows,
  the OCI extraction has already constrained content to the
  tempdir. No additional path-traversal handling needed beyond
  what's already there.
- **`status.d/<pkg>.md5sums` files.** These are the per-package
  installed-file checksums (mirroring `info/<pkg>.md5sums` in
  the legacy layout). Skip by extension; downstream consumers
  read them via the existing `file_hashes.rs` reader if/when
  per-file hashing becomes relevant for status.d/ images.
  **Out of scope for this milestone**: full deep-hashing in
  status.d/ images. The component-level metadata is the gap.
- **Multiple stanzas in a single status.d/ file.** Real-world
  files are one-stanza-per-file. We tolerate multiple by reusing
  `split_stanzas` + `parse_stanza` — but log a tracing::debug
  if it happens, since it's anomalous.

## Functional requirements

- **FR-001**: `mikebom-cli/src/scan_fs/package_db/dpkg.rs::read`
  gains a second source. After checking the legacy `status` file,
  it also walks `var/lib/dpkg/status.d/` if present. Both sources'
  results are concatenated into the returned `Vec<PackageDbEntry>`.

- **FR-002**: A new private helper `read_status_d_dir(rootfs,
  namespace, distro_version) -> Vec<PackageDbEntry>` walks
  `<rootfs>/var/lib/dpkg/status.d/`. For each file whose name has
  no extension (or whose name doesn't match `*.md5sums` /
  `*.conffiles` / similar companion patterns), reads the bytes,
  parses via the existing `parse` helper, and accumulates entries.
  IO errors on individual files log at `tracing::debug` and skip
  that file (consistent with the `collect_claimed_paths` "tolerant
  of malformed inputs" posture).

- **FR-003**: When BOTH sources contribute an entry for the same
  `(package, version)`, dedup by source-path: the status.d/-sourced
  entry wins. Rationale: minimal-image builds that ship a
  `status.d/` directory generally don't also ship a meaningful
  monolithic `status`, but if both exist, `status.d/` is the
  modern source-of-truth. (In practice this happens basically
  never; the dedup is defensive against pathological images.)

- **FR-004**: The existing milestone-031 smoke test
  `pulls_distroless_static_and_emits_well_formed_sbom_with_zero_components`
  is renamed and its assertion updated:
  - New name: `pulls_distroless_static_and_emits_dpkg_status_d_components`.
  - New assertion: `components.len() >= 4` (not `== 0`), AND the
    set of component `name`s contains at least
    `{"base-files", "media-types", "netbase", "tzdata"}`.

- **FR-005**: Inline tests in `dpkg.rs`:
  - Synthetic `status.d/`-only fixture (one tempdir with
    `var/lib/dpkg/status.d/foo` and `bar` files); assert 2
    components emerge.
  - Synthetic mixed fixture (status + status.d/) → both contribute.
  - Status filter: a `status.d/<pkg>` with `Status: deinstall ok
    config-files` is filtered out.
  - Companion-file rejection: `status.d/foo.md5sums` is skipped.
  - Empty status.d/ directory: no entries, no error.

- **FR-006**: NO change to PURL grammar, output format, parity
  catalog, or any other downstream surface. The same `parse_stanza`
  produces the same `PackageDbEntry`s; the change is purely in
  HOW we discover the stanzas.

## Success criteria

- **SC-001**: `./scripts/pre-pr.sh` clean.
- **SC-002**: `git diff main..HEAD -- mikebom-cli/src/parity/
  mikebom-cli/src/generate/ mikebom-cli/src/resolve/` empty.
- **SC-003**: 27-golden regen produces ZERO diff. The fixtures all
  use either the legacy `status` file or no dpkg metadata at all
  (none uses `status.d/`); behaviour on those is unchanged.
- **SC-004**: New inline tests in `dpkg.rs` cover the 5 cases
  enumerated in FR-005.
- **SC-005**: `wc -l mikebom-cli/src/scan_fs/package_db/dpkg.rs`
  ≤ 800 (today: 604; budget: ≤ ~200 LOC additions inclusive of
  tests).
- **SC-006**: `git diff main..HEAD -- mikebom-cli/Cargo.toml ...
  | grep -E '^\+[a-z][a-z0-9_-]+ = '` empty (no new top-level
  deps).
- **SC-007**: All 3 CI lanes green.

## Clarifications

- **Why not parse `*.md5sums` here too?** Component-level metadata
  is the user-visible gap (an image reporting 0 components is the
  obvious bug). Per-file hashing in status.d/ images is a separate
  concern: the rootfs has been merged-without-ownership-tracking,
  so even with `.md5sums` we can't reliably attribute hashes back
  to packages. Defer until a real user need surfaces.
- **Why dedup with status.d/ winning?** Defensive only. In every
  real-world image either `status` exists (full dpkg-managed) OR
  `status.d/` exists (Bazel/distroless), never both with
  overlapping packages. The win-rule prevents pathological
  degenerate cases from producing duplicate components.

## Out of scope

- **Per-file deep-hashing in status.d/ images** — see Clarifications.
- **chainguard's apko `lib/apk/db/installed.d/` variant.** Issue
  #64 lists this as "if it exists analogously, investigate
  separately." Out of scope here; track in a follow-on if surfaced.
- **rpm `Packages.db` / `rpmdb.sqlite` variants.** Different reader
  entirely.
- **PURL semantic changes.** None.
