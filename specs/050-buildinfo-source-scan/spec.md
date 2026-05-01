# Feature Specification: `mikebom:not-linked` annotation + scope hint for Go source-tree scans

**Feature Branch**: `050-buildinfo-source-scan`
**Created**: 2026-05-01
**Status**: Draft

## Summary

Pre-milestone-050 Go scan behavior:

- `--path <source>` (no binary): emit full `go.sum` closure (63
  components on `apigatewayv2/config`)
- `--path <source>` (binary present): G3 silently DROPS go.sum
  entries not in the binary's BuildInfo (41 components — 22
  silently lost; no recovery path)
- `--image <ref>`: same G3 drop on container layers

The drop behavior throws away meaningful classification data:
those 22 modules really ARE in `go.sum` (the lockfile says they
were fetched), they're just not in the linker's output for THIS
build configuration. A consumer seeing only 41 modules can't tell
whether the missing 22 are "never required" vs. "DCE'd build-tag
alternatives" vs. "test scaffolding."

This milestone inverts G3 from drop → tag. The new behavior:

- `--path <source>` (no binary): unchanged (full go.sum closure)
- `--path <source>` (binary present): emit ALL go.sum entries,
  tag the 22 non-BuildInfo ones with `mikebom:not-linked = true`
- `--image <ref>`: same tag behavior on container layers

Consumers get richer data — both the broad lockfile view AND a
precise "what shipped" filter — on a single SBOM. Existing
`mikebom:dev-dependency` (milestone 049) handles test-only
classification independently; a module can carry BOTH annotations
(common case for testify-style deps).

This milestone also adds a discoverability hint: when a
source-tree scan finds `go.mod` but no binary, log a
`tracing::info` line explaining the workflow.

## User Scenarios & Testing

### User Story 1 - Tag-don't-drop for non-BuildInfo Go entries (Priority: P1)

**As a security analyst consuming an mikebom SBOM**, when a Go
module appears in `go.sum` but the linker did not embed it in
the compiled binary, I want it to STAY in the SBOM with metadata
explaining its status, not be silently dropped. That way I can
choose between (a) "everything the resolver touched" and (b)
"only what shipped" with a single filter, instead of needing two
scans to compare.

**Why this priority**: Direct user-stated requirement (verbatim:
"I just want to make sure we include these dependencies but with
the right metadata showing they're dev dependencies or not
included in the binary"). The current drop behavior actively
loses information.

**Independent Test**: Run `mikebom sbom scan --path
<go-project-with-binary>`. Assert: (a) component count equals
the full go.sum closure, (b) at least one component carries
`mikebom:not-linked = true`, (c) the `(name, version)` tuples
of components without that annotation match the binary's
BuildInfo output (`go version -m`).

**Acceptance Scenarios**:

1. **Given** a Go project with both `go.mod`/`go.sum` AND a
   built binary in the rootfs,
   **When** I run `mikebom sbom scan --path .`,
   **Then** every go.sum entry is emitted as an SBOM component,
   AND entries not in the binary's BuildInfo carry
   `mikebom:not-linked = true`, AND entries in the binary's
   BuildInfo do NOT carry that property.

2. **Given** a Go project with only `go.mod`/`go.sum` (no
   binary),
   **When** I run `mikebom sbom scan --path .`,
   **Then** every go.sum entry is emitted, none carry
   `mikebom:not-linked` (no BuildInfo to compare against), AND
   stderr contains a hint about running `go build` for the
   richer classification.

3. **Given** a container image with both Go binaries and
   go.sum entries (existing `--image` flow),
   **When** I run `mikebom sbom scan --image <tarball>`,
   **Then** non-BuildInfo go.sum entries are tagged
   (not dropped) — same behavior as `--path`.

---

### User Story 2 - Discoverability hint when source-tree scan has no binary (Priority: P2)

**As a developer running `mikebom sbom scan --path .`**, when I
haven't built my Go project yet, I want mikebom to tell me that
the SBOM I just got lacks BuildInfo classification and that
running `go build` would let mikebom annotate non-linked
entries.

**Why this priority**: Closes the discoverability gap that
prompted this milestone.

**Independent Test**: `mikebom sbom scan --path
<go-project-without-binary>` → stderr contains a one-line
`tracing::info` mentioning `go build` and `mikebom:not-linked`.

---

### User Story 3 - README documents the workflow (Priority: P2)

**As a new mikebom user reading the README**, I want the Go
ecosystem section to explain the BuildInfo-vs-go.sum trade-off
and the `mikebom:not-linked` annotation.

**Independent Test**: README's Go-ecosystem subsection contains
the words `mikebom:not-linked` and `go build` and a one-liner
showing the consumer-side filter.

---

### Edge Cases

- **Binary present but BuildInfo unreadable** (`-buildid=` strip):
  G3 sees no analyzed-tier entries from that binary — `linked`
  set is empty — G3 no-ops. Same as having no binary. The hint
  in US2 fires.
- **Multiple binaries with overlapping BuildInfo**: existing G3
  union-of-all-binaries' BuildInfo behavior unchanged.
- **Module in BuildInfo of binary A, not in BuildInfo of binary
  B, with both binaries present**: under union semantics, the
  module is "linked" (not tagged). That matches existing G3.
- **Module already tagged `mikebom:dev-dependency = true`** (G4
  test-only) and also not-linked: gets BOTH annotations. CDX
  emits both as separate `properties[]` entries. Same for SPDX
  2.3 / SPDX 3 annotations.
- **Existing G6 cache-zip filter** (`apply_go_cache_zip_filter`
  in `scan_fs/mod.rs:620`): out of scope. G6 drops cache-zip
  path-resolver entries not in BuildInfo — different code path,
  different semantic. A future milestone may revisit.

## Requirements

### Functional Requirements

- **FR-001**: When `apply_go_linked_filter` (G3) finds at least
  one analyzed-tier `pkg:golang` entry, every source-tier
  `pkg:golang` entry whose `(name, version)` is NOT in the
  analyzed set MUST be annotated with
  `mikebom:not-linked = true` in `extra_annotations`. No source-
  tier entry MUST be removed by G3.
- **FR-002**: When G3's analyzed set is empty (no Go binary
  scanned), G3 MUST be a no-op. No `mikebom:not-linked`
  annotations get applied — there's no BuildInfo signal to
  confirm or deny.
- **FR-003**: The `mikebom:not-linked` annotation MUST appear in
  CDX outputs as a `components[].properties[]` entry, in SPDX
  2.3 outputs as a `packages[].annotations[]` entry, and in
  SPDX 3 outputs as a top-level `annotations[]` entry — via the
  existing generic `extra_annotations` serialization (no new
  format-specific code).
- **FR-004**: Hint emission (US2): when source-tree Go scan
  finds `go.mod` but `go_binary_entries` is empty AND
  `scan_mode == ScanMode::Path`, mikebom MUST log a
  `tracing::info` line naming `mikebom:not-linked` and the
  `go build` workflow.
- **FR-005**: The hint MUST NOT fire on `--image` scans, on
  non-Go projects, or when at least one Go binary's BuildInfo
  was readable.
- **FR-006**: README's Go ecosystem section MUST document the
  `mikebom:not-linked` annotation with a consumer-side filter
  example.
- **FR-007**: Existing milestone-049 default-mode behavior
  (test-only deps tagged + dropped via `mikebom:dev-dependency
  = true` + `--include-dev=false`) MUST continue to work. A
  module test-only AND not-linked carries both annotations.
- **FR-008**: All 27 byte-identity goldens MUST stay
  byte-identical: the `simple-module` Go fixture has no built
  binary so G3 doesn't fire on it.

### Key Entities

- **`PackageDbEntry.extra_annotations: BTreeMap<String,
  serde_json::Value>`** (existing, milestone 048): the bag G3
  inserts `mikebom:not-linked` into. Propagated to
  `ResolvedComponent.extra_annotations` via
  `scan_fs/mod.rs:523`.
- **G3 filter** (`apply_go_linked_filter`): rewritten from
  `entries.retain(linked)` to `for e in entries.iter_mut() { if
  !linked.contains(...) { e.extra_annotations.insert(...) } }`.
- **C-row catalog** (`docs/reference/sbom-format-mapping.md`):
  add a row for `mikebom:not-linked` to document the parity
  contract.

## Success Criteria

### Measurable Outcomes

- **SC-001**: `mikebom sbom scan --path
  ~/Projects/iac/app-code/apigatewayv2/config` (binary in
  rootfs) emits ≥ 60 components AND ≥ 20 of them carry
  `mikebom:not-linked = true`. (Empirically: 65 components,
  24 tagged.)
- **SC-002**: Same path WITHOUT binary present: hint fires
  (stderr contains `mikebom:not-linked` substring), full
  go.sum closure emitted (≥ 50 components per milestone 049).
- **SC-003**: `--image` scans: byte-identical to milestone
  049 EXCEPT for the new annotation appearing on previously-
  dropped entries. Existing image-scan integration tests pass
  with updated drop→tag assertions.
- **SC-004**: All 27 byte-identity goldens unchanged.
- **SC-005**: All 16+ `tests/scan_go.rs` integration tests
  pass (14 existing + 2 new from this milestone).
- **SC-006**: `holistic_parity` 11/11 passes — the
  `mikebom:not-linked` annotation appears identically in CDX,
  SPDX 2.3, and SPDX 3 outputs via existing generic wiring.
- **SC-007**: `pre-pr.sh` clean.
- **SC-008**: 3 CI lanes green.

## Assumptions

- The existing milestone-049 design philosophy ("emit
  everything; tag liberally; drop on opt-in flag") is the
  right pattern; G3 should conform to it. Validated by the
  user's verbatim ask to "include these dependencies".
- Reusing the `extra_annotations` bag (vs. adding a typed
  field on `PackageDbEntry`) keeps the milestone scope tight
  and matches the milestone-048 pattern.
- "Cache-zip not in BuildInfo" (G6, `apply_go_cache_zip_filter`)
  is a separate semantic from "go.sum not in BuildInfo" (G3).
  Updating G6 to tag-don't-drop is out of scope; tracked for
  a future milestone if user demand emerges.
- Building binaries on the user's behalf (auto-`go build`)
  remains out of scope.
