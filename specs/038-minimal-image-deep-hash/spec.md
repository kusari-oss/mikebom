# Feature Specification: Per-File Evidence for Minimal-Image Scans

**Feature Branch**: `038-minimal-image-deep-hash`
**Created**: 2026-04-28
**Status**: Draft
**Input**: User description: "let's continue on these other image types"

## User Scenarios & Testing *(mandatory)*

Minimal-image container distributions (distroless, chainguard, Bazel-
built) are widely deployed as the security-conscious alternative to
full Debian / Ubuntu / Alpine base layers. Milestone 037 closed the
"zero packages" gap for distroless / chainguard / Bazel-built deb
images — mikebom now reports the per-image package list. What's
still missing is **per-file evidence**: the rootfs file paths and
content hashes that prove which files belong to which package.

For full-fat images (`debian:bookworm-slim`, `ubuntu:24.04`,
`alpine:3.19`), mikebom emits an `evidence.occurrences[]` block per
component listing every installed file path + SHA-256. SBOM
consumers use this to verify "did anything tamper with this
package's files between build and deployment?" and to correlate
binary findings back to a source package.

For minimal-image scans today, that block is empty — even though
the package metadata directories *do* contain enough information to
populate it. This feature closes that coverage gap so SBOMs of
minimal images carry the same evidence quality as SBOMs of full-fat
images.

### User Story 1 - Per-file evidence for distroless / Bazel-built deb images (Priority: P1)

An SBOM consumer scans a distroless deb image
(`gcr.io/distroless/static-debian12:latest`,
`gcr.io/distroless/cc:latest`, etc.) and expects each deb component
in the output to carry the same per-file evidence block they get for
`debian:bookworm-slim` — file paths inside the package and per-file
content hashes — so they can verify integrity, correlate binary
findings to packages, and run downstream tooling that depends on
file-level evidence.

**Why this priority**: this is the concrete, observable gap in
mikebom's minimal-image coverage. Milestone 037 made the package
list correct; this milestone makes the evidence quality match.
Without it, SBOM consumers can't tell *which* files distroless's
`base-files` package actually shipped, only that the package was
present. That breaks workflows that currently work for full-fat
images.

**Independent Test**: Run `mikebom sbom scan --image
gcr.io/distroless/static-debian12:latest --output distroless.cdx.json`
and inspect the resulting SBOM. Each deb component's evidence
section should contain a populated occurrences list with per-file
paths and SHA-256 hashes — the same shape that `mikebom sbom scan
--image debian:bookworm-slim` produces today, just for the
minimal-image variant. Verify that the file count and paths align
with what `dpkg-query -L <pkg>` reports on the equivalent full-fat
package.

**Acceptance Scenarios**:

1. **Given** a distroless deb image with the per-package metadata
   layout, **When** the user runs an SBOM scan with default flags,
   **Then** every deb component in the output carries a non-empty
   per-file evidence block with file paths and content hashes.
2. **Given** a distroless deb image, **When** the user passes the
   "fast hash" flag (the existing `--no-deep-hash`), **Then** each
   component carries the package-level identity hash but the per-
   file evidence block is empty — matching the existing fast-path
   behavior for full-fat images.
3. **Given** a distroless deb image whose per-package metadata is
   incomplete (a hypothetical broken image with stanza files but no
   companion file-listing data), **When** the user runs a scan,
   **Then** the scan still completes successfully with package-level
   metadata; the per-file evidence block is empty for components
   that lack file-listing data; no errors are reported.
4. **Given** a full-fat image with the legacy single-file metadata
   layout, **When** the user runs a scan, **Then** the resulting SBOM
   is byte-identical to what milestone 037 produced — no regression
   on full-fat coverage.

---

### User Story 2 - Confirm or close minimal-image apk coverage (Priority: P2)

An SBOM consumer scanning a minimal apk-based image (notably
chainguard's apko-built images, which are the security-conscious
counterpart to distroless and increasingly common in production)
gets the same coverage as for a full-fat `alpine:3.19` scan — both
component metadata and per-file evidence.

**Why this priority**: chainguard apko was named as a possible
non-standard layout in the issue that drove milestone 037
(out-of-scope there pending investigation). Before this milestone
ships, the project needs to either confirm that the existing apk
reader covers the apko layout (in which case the work is
verification + smoke test) or implement the missing variant. P2
because the deb gap is the larger known issue; this one might turn
out to be a no-op.

**Independent Test**: Run `mikebom sbom scan --image
cgr.dev/chainguard/static:latest` (or another apko-built image)
and verify the output shape matches what an equivalent full-fat
alpine scan produces — non-empty component list AND per-file
evidence for each. If the existing apk reader already produces
this output, the test is the smoke test. If it doesn't, the test
becomes the acceptance test for the new variant reader.

**Acceptance Scenarios**:

1. **Given** a chainguard apko-built image with the standard apk
   metadata layout, **When** the user runs an SBOM scan, **Then**
   the SBOM contains the expected apk components with full per-file
   evidence — confirming the existing reader covers this case.
2. **Given** an apko-built image with a non-standard metadata
   layout (if one exists), **When** the user runs an SBOM scan,
   **Then** the SBOM still surfaces the components and per-file
   evidence — confirming this milestone closed the variant gap.
3. **Given** the existing apk reader covers all observed apko
   layouts, **When** the milestone work concludes, **Then** the
   investigation is documented (in the spec or in a follow-on note)
   and no new code paths are added.

---

### Edge Cases

- **Files listed in metadata but absent from the rootfs**: the image
  build process may have removed configuration files or runtime-
  created paths. The evidence block should silently skip absent
  files — same posture as the existing full-fat reader. No error.
- **Files listed in metadata but oversized**: a per-file hash cap
  already exists in the full-fat path. Reuse the same cap;
  oversized files are skipped with a debug log.
- **Symlinks listed in metadata**: follow the symlink and hash the
  target only if the target is inside the rootfs. Out-of-rootfs
  symlinks are skipped (and rare for OS-managed packages).
- **Directory entries in metadata**: dpkg's metadata records
  ownership of directories too. These have no content to hash and
  should be skipped, matching existing behavior.
- **Non-UTF-8 file paths in metadata**: skip with a debug log; OS
  package files are virtually always UTF-8 paths.
- **Same package name appears in both legacy and per-package
  metadata sources**: milestone 037's dedup rule (per-package source
  wins) carries through. The per-file evidence comes from the per-
  package metadata in the dedup-winner case.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When mikebom scans a deb image whose package metadata
  lives in the per-package layout (rather than the legacy
  single-file layout), each emitted deb component MUST carry a
  per-file evidence block populated from the per-package file-
  listing data.

- **FR-002**: The per-file evidence block MUST contain the same
  fields, in the same shape, that the existing legacy-layout reader
  produces — file path inside the rootfs and SHA-256 of the file
  content. Downstream emitters (CycloneDX, SPDX 2.3, SPDX 3) MUST
  serialize them identically regardless of which metadata layout
  produced them.

- **FR-003**: When the user passes the existing fast-hash flag
  (`--no-deep-hash`), the per-file evidence block MUST be empty for
  per-package-layout components — matching the behavior already
  defined for full-fat-image components.

- **FR-004**: Components whose per-package metadata is incomplete
  (a stanza file with no companion file-listing data) MUST still
  appear in the SBOM with package-level identity, but with an empty
  per-file evidence block. The scan MUST NOT fail.

- **FR-005**: Existing full-fat-image scans (legacy single-file
  metadata) MUST be byte-identical to milestone 037's output. The
  byte-identity goldens MUST regen with zero diff.

- **FR-006**: Before this milestone is considered complete, mikebom's
  coverage of chainguard apko-built images MUST be either confirmed
  (the existing apk reader already handles them) or extended (a new
  apk variant reader matches the layout). The outcome MUST be
  documented so a future maintainer understands the coverage promise.

- **FR-007**: If FR-006's investigation discovers a new apk metadata
  variant requiring code, that variant MUST receive the same
  per-file evidence treatment as the existing apk reader. If no new
  variant is required, FR-007 is a no-op.

### Key Entities

- **Per-package metadata directory**: the directory inside a
  minimal-image rootfs that holds one metadata stanza per package
  plus that package's companion file-listing data. The companion
  file-listing format already mirrors what the existing legacy
  reader consumes; no new format is introduced.
- **Per-file evidence block**: the standard SBOM evidence-occurrence
  surface — list of file paths inside the rootfs, each with its
  SHA-256 content hash. Already a first-class concept in
  CycloneDX / SPDX output emitted by mikebom for full-fat images.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A scan of `gcr.io/distroless/static-debian12:latest`
  produces an SBOM whose deb components each carry a non-empty
  per-file evidence block — measurable as `total file occurrences
  across all components > 0` (today: 0).

- **SC-002**: For at least one well-understood distroless variant
  (e.g. `static-debian12`), the per-file evidence count for each
  package matches what the equivalent `dpkg-query -L <pkg>` lists on
  a full-fat debian:12 system — measurable as a hand-verified count
  parity for the 4 known packages (`base-files`, `media-types`,
  `netbase`, `tzdata`).

- **SC-003**: A scan of `debian:bookworm-slim` (legacy layout)
  produces an SBOM that is byte-identical to milestone 037's output —
  measurable as the existing byte-identity goldens regenning with
  zero diff.

- **SC-004**: For chainguard apko-built images, mikebom's coverage
  is either confirmed-works (smoke test green out of the box) or
  extended (new variant reader added with passing tests).
  Measurable as: every observed apko-built image SBOM has a non-
  empty component list AND non-empty per-file evidence for each
  component.

- **SC-005**: The fast-hash path (`--no-deep-hash`) produces the
  same package-level component identities as the deep-hash path,
  with empty per-file evidence — measurable as set equality on
  component identity hashes across the two flag values, AND
  occurrences count = 0 under fast-hash for per-package-layout
  components.

## Assumptions

- The per-package metadata layout already documented in milestone
  037 (per-package stanza files plus companion file-listing files)
  is the only deb minimal-image variant in scope. If a third deb
  layout exists (e.g. some Bazel pipelines emitting an entirely
  different shape), it is out of scope for this milestone.
- The existing per-file hash cap and the existing absent-file /
  oversized-file / symlink edge-case behavior carry through
  unchanged for the per-package layout.
- The existing fast-hash semantics (component-level identity with
  empty per-file evidence) is the right behavior for the per-package
  layout too. No user-visible flag changes.
- chainguard apko's apk metadata layout is either standard-apk-
  compatible (in which case this milestone is a verification +
  documentation exercise for that path) or differs in a small,
  well-defined way (in which case the milestone implements the
  variant).
- "These other image types" in the user request refers to the
  minimal-image family (distroless deb, Bazel-built deb, chainguard
  apko apk). Other image categories — non-Linux containers, OCI
  artifacts that aren't filesystem images, etc. — are not in scope.

## Out of scope

- Hypothetical third-form deb layouts beyond the legacy single-file
  layout and the per-package directory layout already known.
- Per-file evidence for image categories outside the deb / apk
  ecosystems (rpm minimal-image variants, if any, are a separate
  investigation).
- Any change to the SBOM output schema or the fast-hash flag
  semantics — this milestone is a coverage extension, not a
  format / UX change.
- Manifest caching or registry-side optimizations — orthogonal.
- The earlier deferred items "rpm file-list extraction from
  HeaderBlob" and "Maven sidecar Debian / Alpine layouts" —
  separate concerns, not part of "these other image types."
