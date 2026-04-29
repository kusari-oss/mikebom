# Feature Specification: Per-File Evidence for apk Components

**Feature Branch**: `039-apk-deep-hash`
**Created**: 2026-04-29
**Status**: Draft
**Input**: User description: "apk per-file deep-hashing for alpine and chainguard apko (mirror dpkg file_hashes.rs)"

## User Scenarios & Testing *(mandatory)*

Milestone 038 surfaced an unintentional asymmetry: deb-based image
scans produce per-file `evidence.occurrences[]` blocks (paths +
SHA-256 + MD5 cross-ref), while apk-based image scans — both
full-fat `alpine:3.19` and minimal chainguard apko / Wolfi — emit
zero per-file evidence. The asymmetry is a coverage gap, not a
design choice: mikebom's `file_hashes.rs` was implemented for the
dpkg layout and never extended to apk.

This milestone closes the gap. After it ships, an SBOM consumer
scanning any apk-based image gets the same evidence quality they
already get for any deb-based image: per-file paths, content
hashes, and a component-level Merkle root.

### User Story 1 - Per-file evidence for any apk-based image (Priority: P1)

An SBOM consumer scans an apk-based image (alpine, chainguard
apko, Wolfi-derived, etc.) and expects each apk component in the
output to carry a populated `evidence.occurrences[]` block with
per-file paths and SHA-256 hashes — at the same evidence quality
already produced for deb components.

**Why this priority**: this is a single-story milestone closing a
specific symmetry gap discovered during milestone 038. There's no
broader UX or schema change; just bringing the apk path up to
parity with the deb path.

**Independent Test**: Run `mikebom sbom scan --image alpine:3.19
--output alpine.cdx.json` and verify the SBOM has populated per-
component `evidence.occurrences[]` for every apk component —
specifically that the total file-occurrence count across all
components is non-zero (today: 0). Each occurrence carries a 64-
hex SHA-256 in the established CycloneDX evidence shape.

**Acceptance Scenarios**:

1. **Given** an apk-based image (alpine, chainguard apko, or
   Wolfi-derived), **When** the user runs an SBOM scan with default
   flags, **Then** every apk component in the output carries a
   non-empty per-file evidence block with file paths and content
   hashes.
2. **Given** an apk-based image, **When** the user passes the
   existing `--no-deep-hash` flag, **Then** each component carries
   a package-level identity hash but the per-file evidence block
   is empty — matching the existing fast-path behavior for deb
   components.
3. **Given** an apk-based image where some files listed in the
   installed-db are absent from the rootfs (rare; would indicate a
   broken image), **When** the user runs a scan, **Then** the scan
   completes successfully, the absent files are silently skipped,
   and the remaining files are hashed normally.
4. **Given** a deb-based image (legacy single-file dpkg or
   minimal-image status.d/), **When** the user runs a scan,
   **Then** the resulting SBOM is byte-identical to milestone
   038's output — no regression on dpkg coverage.

---

### Edge Cases

- **Files listed in the apk db but absent from the rootfs**: the
  image build process may have removed config files or runtime-
  created paths. The evidence block silently skips absent files,
  matching the dpkg posture.
- **Files listed in the apk db but oversized**: same per-file hash
  cap as the dpkg path applies. Oversized files are skipped with
  a debug log.
- **Symlinks listed in the apk db**: follow the symlink and hash
  the target only if the target is inside the rootfs; out-of-
  rootfs symlinks are skipped (matches dpkg posture).
- **Directory entries in the apk db**: apk's `F:` lines name
  directories; these are not hashable content and are skipped.
  Only `R:` lines (regular files) produce occurrences.
- **Stanza fields in arbitrary order**: apk's `F:`/`R:` interleave
  with metadata fields and reset on blank-line stanza boundaries.
  The file-list extractor MUST track stanza boundaries correctly
  so each package's file list is associated with the right
  package name.
- **Wolfi / apko-built images**: same standard apk DB layout as
  alpine. No variant code required.
- **`--no-package-db`**: when set, the apk reader is skipped
  entirely. The new per-file evidence path is also skipped (no
  metadata = no path list to hash). Same as the dpkg path's
  behavior.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When mikebom scans an apk-based image, each emitted
  apk component MUST carry a per-file evidence block populated
  from the apk installed-db's `R:` (regular file) entries under
  the component's stanza.

- **FR-002**: The per-file evidence block MUST carry the same
  fields, in the same shape, that the existing deb-component
  evidence block carries — file path inside the rootfs and SHA-256
  of the file content. Downstream emitters (CycloneDX, SPDX 2.3,
  SPDX 3) MUST serialize them identically regardless of whether
  the component is deb or apk.

- **FR-003**: When the user passes `--no-deep-hash`, the per-file
  evidence block MUST be empty for apk components — matching the
  existing fast-path behavior for deb components. The package-
  level identity hash MUST still resolve.

- **FR-004**: Components with no `R:` entries in their stanza
  (rare; would indicate a content-less metadata-only package)
  MUST still appear in the SBOM with package-level identity, but
  with an empty per-file evidence block. The scan MUST NOT fail.

- **FR-005**: Existing deb-image scans (legacy dpkg or status.d/)
  MUST be byte-identical to milestone 038's output. The
  byte-identity goldens MUST regen with zero diff.

- **FR-006**: No new top-level Cargo dependencies. The work reuses
  `sha2` (already in the dep tree for the dpkg path) and stdlib
  primitives. The `no_c_dependencies_in_tree` regression test
  MUST continue to pass.

### Key Entities

- **apk file-list map**: a per-scan map keyed on package name
  whose values are the rootfs-relative file paths the package's
  stanza claims. Built once per scan by walking the apk
  installed-db; consumed by the per-component hashing loop.
- **Per-file evidence block**: identical to the existing deb-side
  shape — one `FileOccurrence` per file, carrying the rootfs path
  + SHA-256.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A scan of `alpine:3.19` produces an SBOM whose apk
  components each carry a non-empty per-file evidence block —
  measurable as `total file occurrences across all components > 0`
  (today: 0).

- **SC-002**: For at least one well-understood apk image
  (`alpine:3.19`), the per-file evidence count for each package
  matches what `apk info -L <pkg>` reports on a live alpine
  system — measurable as a hand-verified count parity for at
  least one package.

- **SC-003**: A scan of any deb-based image (e.g. `debian:bookworm-
  slim`, `gcr.io/distroless/static-debian12:latest`) produces an
  SBOM byte-identical to milestone 038's output — measurable as
  the existing 27 byte-identity goldens regenning with zero diff.

- **SC-004**: Scanning a chainguard apko / Wolfi image produces
  per-file evidence in the same shape — measurable as `mikebom
  sbom scan --image cgr.dev/chainguard/static:latest` having a
  non-zero per-component occurrences count, where pre-039 it was
  zero.

- **SC-005**: The fast-hash path (`--no-deep-hash`) produces the
  same package-level component identities as the deep-hash path,
  with empty per-file evidence — measurable as set equality on
  component identity hashes across the two flag values, AND
  occurrences count = 0 under fast-hash for all apk components.

## Assumptions

- The apk installed-db format used by alpine and apko/wolfi
  is the same: a single `/lib/apk/db/installed` file with
  blank-line-separated stanzas, `F:` for directories, `R:` for
  regular file basenames within the current `F:` directory.
  Milestone 038's recon confirmed this for chainguard apko.
- Per-file SHA-1 cross-reference (the `Z:` checksum line that
  follows each `R:`) is OUT OF SCOPE for this milestone. The
  dpkg path stores an MD5 cross-ref; the apk path emits SHA-256
  only for now. A `Z:`-cross-ref pass can be added in a
  follow-on if real demand surfaces.
- The existing per-file hash cap and edge-case behavior (absent /
  oversized / symlink) carry through unchanged from the dpkg path.

## Out of scope

- Apk-provided per-file SHA-1 cross-reference (`Z:` lines).
- rpm per-file deep-hashing (separate concern; pre-existing
  deferred item).
- Any change to SBOM output schema or the fast-hash flag
  semantics.
- Manifest caching / registry-side optimizations — orthogonal.
