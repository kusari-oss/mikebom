# Feature Specification: Surface BuildInfo-vs-go.sum scope to source-tree Go users

**Feature Branch**: `050-buildinfo-source-scan`
**Created**: 2026-05-01
**Status**: Draft

## Summary

**The behavior already exists.** When `mikebom sbom scan --path
<go-project>` finds both a Go binary and a `go.mod` in the same
rootfs, the existing G3 filter (`apply_go_linked_filter` in
`mikebom-cli/src/scan_fs/package_db/mod.rs:458`) intersects the
source-tier go.sum entries with the binary's BuildInfo — emitting
exactly the linker-DCE'd "what shipped" set. No code change needed
to make G3 fire on source-tree scans; the current implementation is
rootfs-agnostic.

**What's missing is discoverability.** The user's
`apigatewayv2/config` audit emitted 63 components from a source-only
scan because the binary hadn't been built yet. After
`go build .` and re-running mikebom against the same path, the SBOM
correctly drops to 41 components (40 deps + 1 main) via existing
G3. The user shouldn't have to discover this by accident.

This milestone delivers two things:

1. **Diagnostic at scan time**: when mikebom's Go reader finds
   `go.mod` somewhere under `--path` but the Go-binary walker
   finds zero binaries in the same rootfs, log a `tracing::info`
   hint explaining the SBOM scope (full go.sum closure) and how
   to tighten it (`go build` then re-scan).
2. **Doc updates**: README's Go-scanning explainer + `cli-reference.md`
   `sbom scan --path` notes call out the BuildInfo-intersection
   workflow explicitly.

No behavior change. No new flag. No new annotation. No goldens churn.

## User Scenarios & Testing

### User Story 1 - Source-tree scan with no binary surfaces a hint (Priority: P1)

**As a developer running `mikebom sbom scan --path .` on a Go
project I haven't built yet**, I want mikebom to tell me that the
SBOM I just got is the full `go.sum` closure (which includes
DCE'd build-tag-alternatives and test scaffolding), and that
running `go build` first will give me a tighter "what actually
ships in the binary" SBOM via mikebom's existing BuildInfo
intersection.

**Why this priority**: This is the entire delivery. The G3 logic
that closes the 63-vs-40 gap already exists and works correctly;
users just don't know to fire it.

**Independent Test**: `mikebom sbom scan --path
~/Projects/iac/app-code/apigatewayv2/config` (with the binary NOT
present) → stderr contains a one-line hint mentioning `go build`.
Same scan with the binary present → no hint emitted (G3 already
fired and the SBOM is BuildInfo-tight).

**Acceptance Scenarios**:

1. **Given** a Go project at `--path` with `go.mod` + `go.sum`
   but no built binary,
   **When** I run `mikebom sbom scan --path .`,
   **Then** stderr contains a `tracing::info` log line naming
   the gap and suggesting `go build` for a tighter SBOM.

2. **Given** a Go project at `--path` with both `go.mod`/`go.sum`
   AND a built binary in the rootfs,
   **When** I run `mikebom sbom scan --path .`,
   **Then** no hint is emitted (G3 fires; SBOM is already the
   BuildInfo intersection).

3. **Given** a non-Go project at `--path`,
   **When** I run `mikebom sbom scan --path .`,
   **Then** no Go-related hint is emitted (the diagnostic is
   gated on Go reader having parsed at least one go.mod).

---

### User Story 2 - README + CLI reference document the workflow (Priority: P1)

**As a new mikebom user reading the README**, I want the Go
ecosystem section to explicitly explain that scanning a directory
containing a built binary gives a tighter SBOM than scanning a
source-only tree, and to point at the exact one-liner workflow
(`go build && mikebom sbom scan --path .`).

**Why this priority**: Same motivation as US1 — this is the docs
half of the discoverability gap.

**Independent Test**: README's Go section (or `docs/cli-reference.md`)
contains a paragraph naming BuildInfo, `go build`, and the
expected component-count tightening. Search the docs for "BuildInfo"
or "go build" → at least one hit in user-facing docs.

**Acceptance Scenarios**:

1. **Given** a user reading mikebom's README,
   **When** they look up Go-scanning behavior,
   **Then** they see the source-vs-binary trade-off named with
   numbers (e.g., "63 from go.sum vs ~40 from BuildInfo on
   typical projects") and the recommended workflow.

---

### Edge Cases

- **Binary present but BuildInfo unreadable** (`-ldflags="-w -s"`
  + `-buildid=` strip): `go_binary::read` already emits a
  `mikebom:buildinfo-status` diagnostic. The new hint should
  fire IFF `go_binary` returned ZERO entries with valid BuildInfo
  — same condition as G3 no-op'ing.
- **Multiple Go projects under `--path`**, some built, some not:
  G3 already fires per-rootfs against the union of all binaries'
  BuildInfo. Hint fires when zero binaries are found anywhere.
- **`--image` scans**: hint MUST NOT fire (it's only meaningful
  for source-tree scans where the user can run `go build`).

## Requirements

### Functional Requirements

- **FR-001**: When `mikebom sbom scan --path <root>` parses at
  least one `go.mod` (i.e., `go_signals.main_modules` is
  non-empty) AND `go_binary::read` returns zero analyzed-tier
  entries from the same rootfs, mikebom MUST emit a single
  `tracing::info` log line naming the gap.
- **FR-002**: The hint message MUST include (a) a count of the
  source-tier Go entries that would be filtered out by G3 if
  a binary were present, computed from `go.sum` closure size
  vs. zero (use `go.sum` size as the upper bound),
  (b) a literal `go build` suggestion, (c) a pointer to the
  README section explaining the BuildInfo workflow.
- **FR-003**: The hint MUST NOT fire on `--image` scans (where
  the user has no opportunity to run `go build`).
- **FR-004**: The hint MUST NOT fire on non-Go scans (no
  `go.mod` parsed).
- **FR-005**: The hint MUST NOT fire when at least one Go
  binary's BuildInfo was successfully read (G3 already did
  the right thing).
- **FR-006**: README's Go ecosystem section MUST name the
  BuildInfo workflow with concrete numbers and a one-line
  command example.
- **FR-007**: Existing behavior MUST be byte-identical for
  every milestone-049 case: same default-mode SBOM, same
  `--include-dev` SBOM, same 27 byte-identity goldens, same
  11/11 holistic_parity.

### Key Entities

- **`go_signals.main_modules`**: existing `HashSet<String>` —
  empty IFF no go.mod was parsed.
- **`go_binary_entries.len()`**: existing — zero IFF no
  BuildInfo-readable Go binary was found in the rootfs.
- **scan-mode flag**: existing `crate::scan_fs::ScanMode` enum
  carries Image vs. Path discrimination — used to gate the
  hint to source-tree scans only.

## Success Criteria

### Measurable Outcomes

- **SC-001**: `mikebom sbom scan --path
  ~/Projects/iac/app-code/apigatewayv2/config` (binary absent)
  → stderr contains the new hint.
- **SC-002**: Same path with binary present → no hint, SBOM
  components ≤ 41 (BuildInfo intersection still fires).
- **SC-003**: All 27 byte-identity goldens unchanged (the
  `simple-module` Go fixture has no go.mod main-module activity
  that triggers the new code path's `tracing::info` — and even
  if it did, goldens don't capture stderr).
- **SC-004**: All 14 `tests/scan_go.rs` integration tests pass
  unchanged.
- **SC-005**: `holistic_parity` 11/11 passes.
- **SC-006**: At least one new integration test asserts the
  hint fires (or doesn't) based on binary presence: synthetic
  Go project, scan, capture stderr, assert hint substring
  present/absent.
- **SC-007**: `pre-pr.sh` clean.
- **SC-008**: CI lanes green (3 lanes).

## Assumptions

- Users running `mikebom sbom scan --path` on a Go project who
  haven't built the binary yet would WANT to know they're getting
  a wider SBOM than what would ship. Validated by the user
  surfacing exactly this question in the conversation that
  produced this milestone.
- A `tracing::info` log line is a sufficient surface area —
  mikebom already emits scan-progress info to stderr, users
  see them, and the existing alpha-stage UX has set this
  expectation. Not adding stdout output (would risk breaking
  pipelines that consume `--output -`).
- README + `cli-reference.md` are the right docs to update;
  no new docs file needed.
- The hint is a one-shot emission per scan (not per-go.mod
  found) — multi-project rootfs with no binaries gets one
  hint, not N.
