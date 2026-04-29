# Feature Specification: Package-DB Follow-Ons (Trifecta)

**Feature Branch**: `040-pkg-db-followups`
**Created**: 2026-04-29
**Status**: Draft
**Input**: User description: "let's do the quick housekeeping first and then look at the apk sha-1 cross-ref and then look at rpm deep hashing"

## Clarifications

### Session 2026-04-29

- Q: Should US3 also include the rpm FILEDIGESTS cross-reference (mirroring deb's `md5` and apk's `sha1` cross-refs added in this milestone)? → A: Defer per current spec — US3 ships with `sha256` only; FILEDIGESTS lands in a separate follow-on milestone. Asymmetry across deb+apk vs rpm is documented and easy to close later.

## User Scenarios & Testing *(mandatory)*

Three sequenced follow-on items after milestones 037 / 038 / 039
closed out the deb and apk per-file evidence work. Each is
independently testable and deliverable; ordering reflects effort
and risk (smallest / safest first).

### User Story 1 - Housekeeping: stale OCI comment (Priority: P1) 🎯 MVP

A maintainer reading mikebom's source for the first time
encounters comments that name long-shipped features as
"deferred." Specifically, `host_oci_arch` in
`mikebom-cli/src/scan_fs/oci_pull/mod.rs:215` directs the user to
"`--image-platform linux/<arch>` deferred to milestone 031.y" — a
flag that shipped two milestones ago (PR #72). Stale comments
erode trust in the rest of the codebase's commentary. Removing
them costs ten minutes; not removing them costs a future
maintainer's afternoon when they assume `--image-platform`
doesn't exist and start re-implementing it.

**Why this priority**: pure housekeeping. No behavioral risk.
Smallest possible MVP deliverable; sets the right expectation
that this milestone is incremental, not architectural.

**Independent Test**: `grep -rn 'deferred to milestone 031'
mikebom-cli/src/` returns zero results post-merge (today: at
least one match in `oci_pull/mod.rs:215`).

**Acceptance Scenarios**:

1. **Given** the post-039 codebase, **When** a contributor greps
   for "deferred to milestone 031" anywhere under `mikebom-cli/src/`,
   **Then** zero matches appear (today: one stale match).
2. **Given** an unmapped host architecture (e.g. someone running
   mikebom on a `s390x`-yet-unmapped variant), **When** the
   error message renders, **Then** it points the user at the
   shipped `--image-platform` flag rather than at a deferred
   milestone identifier.
3. **Given** the rest of the codebase, **When** the contributor
   greps for any other "deferred to milestone" markers,
   **Then** every remaining occurrence still points at a
   genuinely deferred follow-on (no false positives left from
   this cleanup).

---

### User Story 2 - Apk Z:-line SHA-1 cross-reference (Priority: P2)

An SBOM consumer integrating mikebom output into a downstream
tool (e.g. apk-tooling that already trusts apk-provided
checksums) wants to cross-reference each apk per-file occurrence
against the upstream-published checksum, the way they already
can for dpkg components (whose `additionalContext` carries
`md5`). Today milestone 039 ships SHA-256 only on the apk side —
the apk-provided per-file SHA-1 from each `Z:` line in
`/lib/apk/db/installed` is parsed and discarded.

**Why this priority**: closes a documented out-of-scope item from
milestone 039's spec. Small extension (~1-2 hr); strictly additive
to the on-the-wire SBOM. Lower priority than housekeeping
because it requires touching production code and re-running the
27-fixture goldens (zero diff expected since goldens use
`--no-deep-hash`, but verify).

**Independent Test**: After implementation, run `mikebom sbom
scan --image alpine:3.19 --output alpine.cdx.json` and inspect a
non-empty `evidence.occurrences[]` entry's `additionalContext`
JSON-string: it now contains both `sha256` (mikebom-computed)
AND `sha1` (apk-provided), where today only `sha256` appears.

**Acceptance Scenarios**:

1. **Given** an apk-based image scan, **When** the user inspects
   any per-file occurrence's `additionalContext`, **Then** it
   contains both `sha256` (mikebom-computed) and `sha1` (apk-
   provided, parsed from the `Z:` line in the package's stanza).
2. **Given** an apk package whose stanza is malformed or missing
   `Z:` lines for some files (rare; possibly very old apk dbs),
   **When** the user runs a scan, **Then** occurrences for files
   without an apk-provided checksum carry `sha256` only — no
   error, no missing-field warning. The scan completes.
3. **Given** a deb-based image scan, **When** the user runs it
   after this story ships, **Then** the deb output is unchanged
   (the apk cross-ref work touches only the apk path).

---

### User Story 3 - rpm per-file deep-hashing (Priority: P3)

An SBOM consumer scanning an rpm-based image (`fedora:40`,
`almalinux:9`, `centos`-stream variants, similar) expects each
rpm component to carry the same `evidence.occurrences[]` block
that deb and apk components already carry. Today rpm components
get zero per-file evidence — `file_hashes.rs` has paths for
dpkg (037 / 038) and apk (039), but not rpm.

**Why this priority**: completes the OS-package per-file-evidence
trilogy. Larger than the other two stories (~½ day) because it
involves a different metadata source (the `rpm` crate's
HeaderBlob `BASENAMES` / `DIRNAMES` / `DIRINDEXES` triple, which
must be combined to reconstruct each file's path) and a
different stanza format. P3 because the deb and apk gaps
shipped first as separate milestones; rpm completing the
trilogy is the same shape of work.

**Independent Test**: Run `mikebom sbom scan --image fedora:40
--output fedora.cdx.json` (or another rpm-based image) and
verify the SBOM has populated `evidence.occurrences[]` for
every rpm component — total file occurrences across components
> 0 (today: 0).

**Acceptance Scenarios**:

1. **Given** an rpm-based image, **When** the user runs an SBOM
   scan with default flags, **Then** every rpm component in the
   output carries a non-empty per-file evidence block with file
   paths and content hashes.
2. **Given** an rpm-based image, **When** the user passes the
   existing `--no-deep-hash` flag, **Then** each rpm component
   carries a package-level identity hash but the per-file
   evidence block is empty — matching the existing fast-path
   behavior for deb and apk.
3. **Given** an rpm package whose `BASENAMES` / `DIRNAMES` /
   `DIRINDEXES` triple is empty (a metadata-only or virtual
   package), **When** the user runs a scan, **Then** the
   component still appears in the SBOM with package-level
   identity but with an empty per-file evidence block. No error.
4. **Given** a deb-based image or apk-based image, **When** the
   user runs a scan after this story ships, **Then** the
   deb/apk output is unchanged (zero golden drift across all
   non-rpm goldens).

---

### Edge Cases

- **Empty cleanup search results post-US1**: someone might add a
  new "deferred to milestone 031" comment between this milestone
  starting and shipping. The acceptance test runs at merge time
  against the actual tree, so any new occurrences need to be
  resolved before the milestone closes.
- **Apk packages with Z: but no R:** (rare; effectively malformed
  metadata): the `Z:` cross-ref is keyed by position relative to
  the preceding `R:`. Without a corresponding `R:`, the `Z:` is
  orphan and silently dropped — same posture as `read_file_lists`
  drops orphan `R:` lines without a preceding `F:`.
- **Rpm packages built without per-file checksums**: rpm has
  always carried per-file SHA-256 / SHA-1 in the FILEDIGESTS tag,
  but very old rpm packages may have empty FILEDIGESTS. mikebom
  computes SHA-256 from the on-disk content regardless; the
  rpm-provided digest is ignored for now (out-of-scope
  cross-ref).
- **Mixed-ecosystem images**: a hypothetical image with both deb
  and rpm metadata (rare; some Bazel pipelines for cross-
  ecosystem builds) would surface both. Each ecosystem's
  reader handles its own per-file evidence; no interaction.

## Requirements *(mandatory)*

### Functional Requirements

#### US1 — Housekeeping

- **FR-001**: Update the error message in
  `mikebom-cli/src/scan_fs/oci_pull/mod.rs::host_oci_arch` to
  point users at the shipped `--image-platform` flag instead of
  the long-shipped milestone-031.y identifier. Wording removes
  the "deferred" framing entirely; replaces it with a positive
  pointer at the existing CLI surface.

- **FR-002**: After the cleanup, no comment or string literal in
  `mikebom-cli/src/` MUST contain the substring "deferred to
  milestone 031" (covering 031.x, 031.y, 031.z — all of which
  shipped). Spec docs under `specs/` are exempt — those record
  history.

#### US2 — Apk SHA-1 cross-reference

- **FR-003**: When mikebom scans an apk-based image, each per-file
  `evidence.occurrences[]` entry's `additionalContext` JSON-
  string MUST carry both `sha256` (mikebom-computed) and `sha1`
  (the apk-provided checksum extracted from the package's `Z:`
  line). When the `Z:` line is missing for a particular file
  (rare), `sha1` is omitted from `additionalContext` for that
  occurrence — no other change.

- **FR-004**: The apk file-list extractor MUST be extended to
  return both the path AND the apk-provided SHA-1 per file (when
  available). The downstream hash-and-emit pipeline MUST thread
  the SHA-1 alongside the path into the per-file occurrence.

- **FR-005**: Deb-based image scans MUST be byte-identical to
  milestone 039's output. The `additionalContext` of any
  dpkg-sourced occurrence is unchanged.

#### US3 — Rpm per-file deep-hashing

- **FR-006**: When mikebom scans an rpm-based image, each emitted
  rpm component MUST carry a per-file evidence block populated
  from the package's HeaderBlob file-list (the
  `BASENAMES` / `DIRNAMES` / `DIRINDEXES` triple — the canonical
  rpm representation of file ownership).

- **FR-007**: The rpm per-file evidence block MUST follow the
  same shape that dpkg and apk components produce — file path
  inside the rootfs, SHA-256 of the file content, optionally a
  cross-reference checksum (rpm's FILEDIGESTS — out of scope
  for this milestone, defer if needed).

- **FR-008**: When the user passes `--no-deep-hash`, the per-file
  evidence block MUST be empty for rpm components — matching the
  established fast-path behavior. The package-level identity
  hash MUST still resolve.

- **FR-009**: Rpm packages whose HeaderBlob has empty file lists
  (metadata-only / virtual packages) MUST still emerge as
  components with package-level identity; their per-file evidence
  block is empty. The scan MUST NOT fail.

#### Cross-cutting

- **FR-010**: No new top-level Cargo dependencies. The work
  reuses `sha2` (existing), the `rpm` crate (already a direct
  dep from milestone 004 for rpm metadata reading), `flate2` /
  `xz2` for compressed-payload handling if needed (already in
  the dep tree), and stdlib primitives.

- **FR-011**: All existing byte-identity goldens MUST regen with
  zero diff. (The 27-fixture goldens use `--no-deep-hash` so
  they're naturally insulated from the new deep-hash path.)

### Key Entities

- **`additionalContext` JSON-string**: today carries `sha256` +
  optionally `md5` (dpkg) on per-file occurrences. After US2 it
  also carries `sha1` (apk-provided) on apk occurrences. After
  US3 it carries `sha256` only on rpm occurrences (FILEDIGESTS
  cross-ref deferred).
- **Rpm HeaderBlob file triple**: `(BASENAMES, DIRNAMES,
  DIRINDEXES)`. Used to reconstruct each file's full path:
  `DIRNAMES[DIRINDEXES[i]] + BASENAMES[i]`. The canonical rpm
  encoding; the `rpm` crate exposes accessors.

## Success Criteria *(mandatory)*

### Measurable Outcomes

#### US1

- **SC-001**: Post-merge `grep -rn 'deferred to milestone 031'
  mikebom-cli/src/` returns zero matches (today: ≥1).

- **SC-002**: Post-merge `grep -rn 'deferred to milestone'
  mikebom-cli/src/` returns only matches that point at genuinely
  deferred items (manual audit at PR review time).

#### US2

- **SC-003**: A scan of `alpine:3.19` produces an SBOM where
  every populated apk occurrence's `additionalContext` carries
  both `sha256` and `sha1` — measurable as 100% of populated
  occurrences having both keys.

- **SC-004**: A scan of `cgr.dev/chainguard/static:latest`
  shows the same `sha1` cross-ref population for apk
  occurrences.

#### US3

- **SC-005**: A scan of an rpm-based image (`fedora:40` or
  similar) produces an SBOM whose rpm components each carry a
  non-empty per-file evidence block — measurable as
  `total file occurrences across all components > 0` (today: 0).

- **SC-006**: For at least one well-understood rpm package, the
  per-file evidence count matches what `rpm -ql <pkg>` reports —
  measurable as a hand-verified count parity for at least one
  package on a real `fedora:40` scan.

#### Cross-cutting

- **SC-007**: Existing byte-identity goldens regen with zero
  diff (FR-011).
- **SC-008**: All 3 CI lanes green.

## Assumptions

- The `rpm` crate (already a direct dep) exposes BASENAMES,
  DIRNAMES, DIRINDEXES via accessors on its parsed HeaderBlob /
  Header type. Verified at planning time; if the API differs,
  the implementation either uses lower-level tags directly or
  parses the header bytes manually (the format is documented in
  the rpm spec).
- Apk's `Z:` line carries a `Q1`-prefixed base64-encoded SHA-1
  (per the apk source / community documentation). The
  implementation parses this opaquely as bytes, not relying on
  the prefix being constant — and re-encodes as a 40-hex-char
  string on the wire (SHA-1 output is 20 bytes → 40 hex chars).
- The three user stories are independent enough that they can
  ship as one PR (recommended — small, contained, all
  package-db-related) or as three sequenced PRs (acceptable but
  more overhead).
- "rpm-based image" includes `fedora:*`, `almalinux:*`,
  `rockylinux:*`, `centos:stream*`, `redhat/*`, and similar.
  Mikebom's existing rpm reader already handles them all
  uniformly via `/var/lib/rpm/*.db` (or the legacy Berkeley DB
  variant gated by `--include-legacy-rpmdb`).

## Out of scope

- Rpm FILEDIGESTS cross-ref (the rpm-provided per-file SHA-256 /
  SHA-1 / MD5). Could be added in a follow-on if real demand
  surfaces — would mirror the apk-side `sha1` cross-ref this
  milestone adds.
- Container layer attribution (separate architectural milestone;
  pre-existing deferred item from the milestone-023 roadmap).
- Schema-level `hashes` array on `FileOccurrence` (would unify
  cross-ref carriers across ecosystems but requires a schema
  extension across all three downstream emitters; defer until
  there's concrete external demand).
- Maven sidecar Debian / Alpine variants (separate concern;
  pre-existing deferred item).
