# Feature Specification: Bind binary-tier C/C++ components to source-tier PURLs via cmake build-directory observation

**Feature Branch**: `109-binary-source-purl-binding`
**Created**: 2026-06-02
**Status**: Draft
**Input**: User description: "Bind binary-tier C/C++ components to their source-tier declarations via cmake build-directory observation, so the binary SBOM uses the same upstream PURLs as the source SBOM."

## Clarifications

### Session 2026-06-02

- Q: Which name-comparison strategy joins a cmake declaration to a fingerprint match? → A: case-insensitive equality on the cmake declaration's `name` parameter compared against the fingerprint record's `library` field, with the build-artifact path's `_deps/<name>-build/` existence as the corroborating signal. Non-standard cmake names (`zlib_static`, `ZLIB::ZLIB`) get no binding (correct: their setup isn't following the convention; silently guessing would be worse than emitting both PURLs).
- Q: How broad should the build-directory observer's path-pattern coverage be in this milestone? → A: `FetchContent_Declare`'s `_deps/<name>-build/` layout ONLY. `ExternalProject_Add` is deferred to a follow-on milestone — its source-tier components still emit unchanged via the existing cmake reader, they just don't participate in cross-tier attribution yet. FetchContent is the cmake-recommended path (3.11+, ubiquitous since ~2018); ExternalProject's per-project path variance needs separate research.

## User Scenarios & Testing

### User Story 1 — Cross-tier PURL equality after a project-root scan (Priority: P1) 🎯 MVP

An SBOM consumer scans a cmake project (source + built binary in one tree). With `--fingerprints-corpus` opted in, the emitted SBOM has ONE entry per third-party library: a source-tier PURL like `pkg:github/madler/zlib@v1.3.1` (from `FetchContent_Declare`) that ALSO covers what would otherwise be a separate binary-tier `pkg:generic/zlib`. The two ends of the build pipeline produce one identifier per real-world library.

**Why this priority**: this is the visible gap consumers hit when diffing source + binary SBOMs of the same C/C++ project today. Without this attribution, the cmake-demo (and every analogous real project) emits the same library under two non-joining PURLs — the phantom-mismatch problem.

**Independent Test**: scan the `mikebom-cmake-demo` project root with `mikebom sbom scan --path . --fingerprints-corpus`; assert that the emitted SBOM contains exactly ONE zlib component whose PURL is `pkg:github/madler/zlib@v1.3.1`; assert that NO `pkg:generic/zlib` component exists for the same target binary. Without this milestone, two zlib components are emitted with non-joining PURLs.

**Acceptance Scenarios**:

1. **Given** a cmake project with `FetchContent_Declare(zlib GIT_REPOSITORY ... GIT_TAG v1.3.1)` and a built binary that statically links zlib, **When** the operator runs `mikebom sbom scan --path . --fingerprints-corpus`, **Then** the emitted SBOM contains exactly one zlib component with PURL `pkg:github/madler/zlib@v1.3.1` carrying both the cmake-reader evidence AND the symbol-fingerprint corroboration.
2. **Given** a cmake project with a second `FetchContent_Declare(openssl ...)` declaration that produces `_deps/openssl-build/` after build, **When** the operator runs the same scan, **Then** the openssl component carries the source-tier PURL the cmake reader emitted rather than a separate `pkg:generic/openssl` from the binary matcher. (Note: per the Phase-2 clarification, `ExternalProject_Add` is out of scope for this milestone — its source-tier components still emit unchanged via the existing cmake reader; they just don't participate in cross-tier attribution. A follow-on milestone covers ExternalProject's distinct path layout.)
3. **Given** a cmake project where `FetchContent_Declare(zlib ...)` was declared but the operator never built (`_deps/zlib-build/` doesn't exist), **When** the scan runs, **Then** the source-tier `pkg:github/madler/zlib@v1.3.1` is emitted unchanged AND no binary-tier zlib entry appears (nothing was actually linked).

---

### User Story 2 — Consumer joins source + binary SBOMs by PURL equality (Priority: P1)

A vulnerability-triage analyst receives both the source-tier SBOM (emitted from a CI scan of the source tree) and the binary-tier SBOM (emitted from a runtime scan of the built artifact). They want to diff the two to confirm "everything I declared as a dep actually shipped in the binary." Today the diff is noisy because the same libraries appear under different PURLs across the two SBOMs.

**Why this priority**: this is the consumer-facing outcome that makes the milestone visible. Without it, mikebom emits two SBOMs that don't equality-join, and consumers have to write their own PURL-alignment fixup logic.

**Independent Test**: given a source.cdx.json (from `mikebom sbom scan --path src/`) and a binary.cdx.json (from a separate post-#306 scan of the build/), a `jq` one-liner can compute the set difference of `pkg:` PURLs between the two and surface ONLY genuinely-missing-from-binary deps (declared-but-not-linked) without phantom mismatches caused by PURL form drift.

**Acceptance Scenarios**:

1. **Given** a source SBOM with `pkg:github/madler/zlib@v1.3.1` and a binary SBOM (post-milestone) with the same PURL, **When** the consumer runs `jq` to compute the set difference, **Then** the result is empty (no phantom mismatch).
2. **Given** a source SBOM declaring zlib + libcurl, and a binary SBOM where only zlib's symbols matched (libcurl is declared but never linked), **When** the consumer computes the set difference, **Then** libcurl appears in the source-but-not-binary set (legitimate signal) and zlib does NOT appear in either side's difference (correctly joined).

---

### User Story 3 — Binary-only scans preserve pre-109 behavior (Priority: P2)

An operator scans only a stripped binary (no source tree, no cmake build directory) with `--fingerprints-corpus`. The fingerprint matcher still fires, but with no cmake declarations to bind to, the milestone-108 behavior is preserved: `pkg:generic/<library>` is emitted unchanged.

**Why this priority**: the milestone-108 contract (operators can run `--fingerprints-corpus` against ANY binary, anywhere) must not regress. Single-binary scans are a legitimate use case (forensics, third-party-binary triage) that this milestone deliberately does not change.

**Independent Test**: copy a built C/C++ binary to an otherwise-empty tempdir; run `mikebom sbom scan --path <tempdir> --fingerprints-corpus`; assert the SBOM contains `pkg:generic/<library>` for matched fingerprints (byte-identical to alpha.44 behavior).

**Acceptance Scenarios**:

1. **Given** a tempdir containing only a built crc-demo binary (no source, no cmake build dir), **When** the operator scans it with `--fingerprints-corpus`, **Then** the SBOM contains `pkg:generic/zlib` exactly as alpha.44 produced.
2. **Given** a scan where `--fingerprints-corpus` is OFF (default), **When** the same project root from US1 is scanned, **Then** the SBOM is byte-identical (modulo timestamps) to what alpha.44 would have produced — the new attribution is gated behind the same opt-in as milestone 108.

---

### User Story 4 — Attribution is transparent and auditable (Priority: P2)

A maintainer reviewing an emitted SBOM wants to understand WHY a binary component carries `pkg:github/madler/zlib@v1.3.1` rather than `pkg:generic/zlib`. The component carries an annotation explaining the binding: source-tier declaration produced the PURL, fingerprint matcher confirmed the binary contains it.

**Why this priority**: operator trust in non-obvious attribution depends on transparency. A binary component carrying a versioned PURL without explanation looks like the matcher is "guessing" the version (which it isn't — the version came from the source declaration).

**Independent Test**: inspect any binary-tier component in a US1-scenario SBOM; assert it carries an annotation indicating the source-tier mechanism that drove the attribution (e.g., `cmake-fetchcontent-git` from the milestone-105 source-mechanism enum).

**Acceptance Scenarios**:

1. **Given** the US1 scan output, **When** the consumer inspects the zlib component's properties, **Then** the existing `mikebom:source-mechanism` annotation (closed-enum from milestone 105) carries the cmake-derived value (`cmake-fetchcontent-git`, `cmake-fetchcontent-url`, or `cmake-externalproject`) rather than a binary-only value.
2. **Given** the same component, **When** the consumer inspects the `mikebom:evidence-kind` annotations, **Then** BOTH source-tier evidence (`cmake-fetchcontent-git`-style) and binary-tier evidence (`symbol-fingerprint` from milestone 099 + 108) are surfaced — the attribution is multi-evidence, not single-evidence.

---

### User Story 5 — Forward-compat for non-cmake build systems (Priority: P3)

The attribution mechanism is designed to extend to Bazel (`bazel-out/<config>/`), Meson (`subprojects/`), and other build systems in subsequent milestones without architectural rework. The cmake-first design uses a generic "build-tree dep declaration → built-artifact path" mapping that other readers can plug into.

**Why this priority**: not a release-blocker, but the cmake-first scope decision shouldn't paint the milestone into a corner. Future Bazel/Meson readers should reach for the same attribution mechanism rather than building parallel infrastructure.

**Independent Test**: review the architectural notes in the design phase — the attribution layer accepts pluggable "build-tree declaration sources" without cmake-specific assumptions baked in.

**Acceptance Scenarios**:

1. **Given** the implementation, **When** a Bazel reader is hypothetically added in a follow-on milestone, **Then** the attribution layer accepts it without modification (the cmake-specific path-observation logic is isolated; the shared attribution layer consumes any build-system observer that produces "source-tier declaration → built-artifact path" mappings).

---

### Edge Cases

- **Operator scans only `build/`** (no source tree): no cmake source declarations to bind against → fall back to milestone-108 generic PURL behavior; no error.
- **Operator runs `cmake -B build/` but never builds** (`_deps/<dep>-src/` exists but no compiled artifacts): no binary fingerprint matches → source-tier PURL emits unchanged.
- **Source declares `FetchContent_Declare(zlib ...)` but never links it** (declared-but-unused): no binary match → source-side PURL emits unchanged; no spurious binary component.
- **Two cmake projects under one scan root** (workspace with subprojects, each with its own `build/`): each project's source declarations bind only to fingerprint matches in binaries originating from THAT project's build dir.
- **`ExternalProject_Add` declarations (any flag combination)**: per the Phase-2 clarification, out of scope for this milestone. Source-tier components from the cmake reader still emit; no cross-tier attribution fires. Follow-on milestone covers ExternalProject's `<name>-prefix/` default + `BINARY_DIR` override.
- **`FetchContent_Declare(SOURCE_DIR ...)` with operator-overridden source directory** (pre-fetched zlib living outside `_deps/`): attribution follows the BUILD dir, not the source dir; if `_deps/<dep>-build/` exists, the binding fires regardless of where the source code is.
- **Multiple source declarations could match one fingerprint** (rare — e.g., a vcpkg.json AND a FetchContent declaration for the same library): emit BOTH source-tier components per the milestone-105 `mikebom:also-detected-via` dedup pattern; mark the cmake-build-dir-observed one as authoritative for the binary attribution.
- **A stray static archive named `libz.a` exists in `build/` outside any `_deps/zlib-build/` directory**: do NOT cross-attribute (the well-formed path is the only signal that participates; reduces false-positive risk).
- **Cmake reader emits a `pkg:github/madler/zlib@v1.3.1` AND the binary fingerprint matches a DIFFERENT library at `_deps/zlib-build/`** (highly unlikely but possible if the operator hand-edited cmake's output): no semantic check is performed beyond library-name match; the attribution fires, with `mikebom:also-detected-via` surfacing any disagreement.

## Requirements

### Functional Requirements

- **FR-001**: When mikebom scans a tree that contains BOTH a cmake `FetchContent_Declare` source declaration (parsed by the existing milestone-102/103 cmake reader) AND a built binary whose symbol fingerprint matches a library referenced by that declaration, the binary-tier component MUST be emitted under the source-tier PURL rather than the milestone-108 generic PURL. The JOIN KEY between the two sides is: **case-insensitive equality on the cmake declaration's `name` parameter compared against the fingerprint record's `library` field, with `_deps/<name>-build/` existence required as a corroborating signal**. Non-standard cmake names (e.g., `zlib_static`, `ZLIB::ZLIB`, vendor-renamed targets) deliberately get no binding — silently aliasing would be worse than emitting both PURLs separately. **`ExternalProject_Add` declarations do NOT participate in this milestone** (deferred to a follow-on; ExternalProject's per-project path variance is distinct from FetchContent's stable `_deps/` convention).
- **FR-002**: The cross-tier attribution MUST be opt-in via the existing `--fingerprints-corpus` flag (or `MIKEBOM_FINGERPRINTS_CORPUS=1`). When the opt-in is not set, NO new attribution behavior fires (SC-003 byte-identity preservation extends to this milestone).
- **FR-003**: When a fingerprint match occurs but NO source-tier cmake declaration covers the matched library (e.g., scanning only a binary, or scanning a build tree where the matched library was never declared by cmake), the matcher MUST fall back to the milestone-108 generic PURL behavior with no error.
- **FR-004**: Each cross-attributed binary component MUST carry a transparency annotation indicating the source-tier mechanism that drove the attribution. The annotation MUST reuse the milestone-105 `mikebom:source-mechanism` closed enum — for this milestone the relevant values are `cmake-fetchcontent-git` and `cmake-fetchcontent-url`. (`cmake-externalproject` exists in the enum but does not participate in attribution this milestone per the Phase-2 clarification.)
- **FR-005**: The cross-attribution MUST be emitted symmetrically across CycloneDX 1.6, SPDX 2.3, and SPDX 3.0.1 output formats per the milestone-013 cross-format parity contract.
- **FR-006**: Per milestone-105 dedup-pipeline conventions, when multiple source-tier readers declare the same library covered by one binary fingerprint match, mikebom MUST emit ONE component (no silent dedup loss) AND populate `mikebom:also-detected-via` listing the additional source mechanisms that corroborated the dep.
- **FR-007**: Source declarations from non-cmake readers (deb, rpm, apk, etc.) MUST NOT participate in this milestone's attribution. Image-tier cross-tier binding is covered by milestone 072's `--bind-to-source` mechanism; this milestone is scoped to single-scan-tree cmake → binary attribution.
- **FR-008**: mikebom MUST identify a cmake build directory deterministically: presence of `CMakeCache.txt` at the directory root AND a `_deps/` subdirectory containing at least one entry. Directories matching only one of these conditions MUST NOT be treated as cmake build directories.
- **FR-009**: When the cmake reader's declaration produces a versioned PURL (e.g., `pkg:github/madler/zlib@v1.3.1`), the attributed binary component MUST emit the EXACT source-tier PURL verbatim — no version-shape transformation, no re-formatting, no version stripping.
- **FR-010**: Stray static archives in the build directory that don't follow the cmake `_deps/<dep>-build/<libname>.a` convention MUST NOT trigger cross-attribution. Only artifacts at predictable cmake-convention paths participate.
- **FR-011**: The walker that maps cmake source declarations to build artifacts MUST be additive — failure of the build-dir observation (e.g., `_deps/` exists but is empty) MUST NOT block the source-tier scan from emitting its components normally. Per Constitution Principle III (Fail Closed): cross-attribution failure surfaces as a warning, not a scan abort.
- **FR-012**: The architecture MUST accommodate Bazel and Meson observers as follow-on milestones without rework. The cmake-specific observer logic lives in a discrete module; the shared attribution layer accepts pluggable "build-tree declaration → built-artifact path" mappings from any observer.

### Key Entities

- **CmakeBuildDirObservation**: a per-cmake-project record produced at scan time joining (cmake source-tier declaration, expected build-artifact path under `_deps/<dep>-build/`, presence-of-built-artifact flag). Computed once per cmake project found in the scan root; consumed by the binary scanner's fingerprint matcher when deciding attribution.
- **SourceTierBinding**: a per-binary-component attribution record produced when a fingerprint match correlates with a `CmakeBuildDirObservation`. Carries the source-tier PURL, the source-tier mechanism enum value (for the `mikebom:source-mechanism` annotation), and the matched fingerprint record (for evidence transparency).
- **AttributionFallback**: the milestone-108 generic-PURL emission path, preserved for the cases where no cross-attribution applies (no cmake source declaration, no build directory, single-binary scans).

## Success Criteria

### Measurable Outcomes

- **SC-001**: After scanning the `mikebom-cmake-demo` project root with `mikebom sbom scan --path . --fingerprints-corpus`, the emitted SBOM contains EXACTLY ONE zlib component, whose PURL is `pkg:github/madler/zlib@v1.3.1`. Zero components have PURL `pkg:generic/zlib` for the same binary target.
- **SC-002**: For ANY cmake project scanned with `--fingerprints-corpus` from its root (containing source + built artifacts), source-tier and binary-tier components for libraries declared via `FetchContent_Declare` OR `ExternalProject_Add` MUST equality-join by PURL with 100% recall (no phantom mismatches caused by version-shape drift).
- **SC-003**: For scans WITHOUT `--fingerprints-corpus`, the emitted SBOM is byte-identical (modulo timestamps) to what alpha.44 produced for the same scan target. The 33 byte-identity goldens MUST pass byte-identically. This is the SC-003 inherited from milestone 108.
- **SC-004**: For scans of a SINGLE BINARY (no source tree, no cmake build dir) with `--fingerprints-corpus`, the emitted SBOM is byte-identical (modulo timestamps) to what alpha.44 produced for the same single-binary input. The milestone-108 single-binary behavior is preserved.
- **SC-005**: For scans of a cmake project where the build directory is absent (operator never built), source-tier components emit unchanged AND zero binary-tier components are spuriously created. Declared-but-unbuilt deps do NOT trigger false attribution.
- **SC-006**: Every cross-attributed binary component carries the existing `mikebom:source-mechanism` annotation per FR-004 with a value from the closed milestone-105 enum. Components without the annotation prove the attribution did NOT fire (either the milestone-108 fallback path OR a non-cmake-derived component).

## Assumptions

- mikebom's existing milestone-102/103 cmake reader correctly parses `FetchContent_Declare` (GIT_REPOSITORY + URL forms). The attribution layer relies on the reader's parsed declarations for FetchContent specifically (ExternalProject deferred per Phase-2 clarification) without re-parsing CMakeLists.txt.
- mikebom's existing milestone-099 + 108 symbol-fingerprint matcher correctly emits `pkg:generic/<library>` for matched libraries on ELF + Mach-O + PE binaries (Mach-O added in alpha.44 PR #305, PE in alpha.45+ PR #309).
- Cmake's `FetchContent_Declare` writes to `<build-dir>/_deps/<dep>-{src,build,subbuild}/` per the documented convention. The observer ONLY handles this layout in this milestone. `ExternalProject_Add`'s `<build-dir>/<name>-prefix/` default (and its `BINARY_DIR` / `INSTALL_DIR` overrides) is deferred to a follow-on milestone per the Phase-2 clarification.
- Operators scan project ROOTS (containing both source + build dirs) for cross-attribution to fire. Single-binary scans and source-only scans fall back to existing behavior.
- The fingerprint corpus content remains unchanged — this milestone is purely an attribution layer above the existing matcher.
- The architecture assumes ONE cmake project per scan root for the MVP. Multi-project workspaces (each with its own `CMakeCache.txt` + `_deps/`) are handled as if each subproject were a separate scan, with per-project attribution scoping.
- Operators who run incremental builds (cmake builds where `_deps/` already exists from a previous build) get correct attribution as long as cmake hasn't re-pinned to a different version in CMakeLists.txt since the last build.
- No new Cargo dependencies are required. The walker uses existing std + the milestone-102/103 cmake reader's parsed output.
- No network access is added by this milestone. The attribution is computed entirely from local filesystem state + the existing cmake reader's output. Per FR-011 / Constitution Principle X (Transparency), no enrichment or external lookups participate.
