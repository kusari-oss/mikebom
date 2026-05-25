# Feature Specification: Binary Role Classification (Application vs Library) in Emitted SBOMs

**Feature Branch**: `104-binary-role-classification`
**Created**: 2026-05-24
**Status**: Draft
**Input**: User description: "Classify binary components as application or library based on the executable-vs-shared-object header (Mach-O MH_EXECUTE/MH_DYLIB, ELF ET_EXEC/ET_DYN, PE IMAGE_FILE_DLL) and emit the correct format-native component-type across CycloneDX, SPDX 2.3, and SPDX 3. Currently every binary discovered by the binary reader is emitted with CDX type=library, mis-typing executables like /bin/ls."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Executable binaries are correctly typed as applications (Priority: P1)

A security or compliance analyst scans a directory containing executable binaries (e.g., `/bin`, `/usr/bin`, an application bundle, or a build-output directory containing compiled programs) and inspects the emitted SBOM. Every component that corresponds to a runtime executable is identified as an application, not a library. The analyst can run a filter against the SBOM's native component-type field and see exactly the set of executable artifacts present, distinguished from shared libraries and other component kinds.

**Why this priority**: This is the core fix that resolves the reported inversion. Today, executables are mis-typed as libraries in CycloneDX output, which reads as the opposite of the truth — a consumer sees `ls` marked `type: library` and interprets it as "something that gets linked into other components," when in reality `ls` is the application doing the linking. The native component-type field is one of the first things any SBOM consumer reads; getting it wrong undermines every downstream filter, vulnerability scope, license-risk analysis, and deployment-policy check that depends on it.

**Independent Test**: Scan a directory containing at least one known executable binary (e.g., a copy of `/bin/ls`). Inspect the emitted CycloneDX SBOM. Confirm the component representing the executable carries `type: "application"` and that any shared-library components discovered alongside it (e.g., dylibs the executable loads) carry `type: "library"`. Pre-fix output has both as `type: "library"`; post-fix output cleanly separates them.

**Acceptance Scenarios**:

1. **Given** a directory containing a single Mach-O executable, **When** the user runs `mikebom sbom scan --path <dir>`, **Then** the CycloneDX `components[]` entry for that binary has `type: "application"`.
2. **Given** a directory containing both a Mach-O executable and a Mach-O dylib, **When** the user scans with CycloneDX output, **Then** the executable's component has `type: "application"` and the dylib's component has `type: "library"`.
3. **Given** a directory containing an ELF executable (e.g., from a Linux rootfs in `/bin`), **When** the user scans, **Then** the executable's CycloneDX component has `type: "application"`.
4. **Given** a directory containing a Windows `.exe` and a `.dll`, **When** the user scans, **Then** the `.exe` is typed `application` and the `.dll` is typed `library`.

---

### User Story 2 - SPDX 2.3 + SPDX 3 emission carries the same role distinction (Priority: P2)

The same analyst emits SPDX 2.3 and SPDX 3 alongside CycloneDX (for federal-procurement or LF SPDX-validator pipelines that require SPDX, or for downstream consumers that prefer SPDX 3's JSON-LD model). The role classification flows into the SPDX-native fields so the SBOM tells the same story across all three formats: executables are applications, shared libraries are libraries, and a cross-format consumer comparing CDX and SPDX outputs for the same scan sees consistent typing throughout.

**Why this priority**: The reported bug surfaced in CycloneDX, but the underlying mis-classification affects every emission path. SPDX 2.3 currently leaves `primaryPackagePurpose` unset for binary-reader-discovered components (less wrong than the CDX mis-typing, but it loses information that would be useful for the same filtering use cases). SPDX 3 follows SPDX 2.3's pattern via `software_primaryPurpose`. Per Constitution Principle V (standards-native fields take precedence), the native typing slots in each format are the right home for this signal — we should not invent a mikebom-prefixed annotation for it.

**Independent Test**: Run the same scan as Story 1 with `--format spdx-2.3-json,spdx-3-json` (or two separate invocations). Confirm the executable component carries `primaryPackagePurpose: APPLICATION` in SPDX 2.3 and `software_primaryPurpose: application` in SPDX 3, and that the shared-library component carries the corresponding `LIBRARY` / `library` value. Pre-fix SPDX 2.3 leaves the field absent and SPDX 3 mirrors that.

**Acceptance Scenarios**:

1. **Given** a Mach-O executable scanned with `--format spdx-2.3-json`, **When** the SBOM is emitted, **Then** the corresponding `packages[]` entry has `primaryPackagePurpose: "APPLICATION"`.
2. **Given** a Mach-O dylib scanned with `--format spdx-2.3-json`, **When** the SBOM is emitted, **Then** the corresponding `packages[]` entry has `primaryPackagePurpose: "LIBRARY"`.
3. **Given** an ELF executable scanned with `--format spdx-3-json`, **When** the SBOM is emitted, **Then** the corresponding `software_Package` element has `software_primaryPurpose: "application"`.
4. **Given** the same scan emitted in all three formats, **When** a cross-format auditor compares the role classifications, **Then** every binary component carries the equivalent role value in each format (no format diverges on role).

---

### User Story 3 - Ambiguous and edge-case binaries fall back deterministically (Priority: P3)

When the binary reader encounters a file format whose role cannot be unambiguously determined from headers alone (e.g., a position-independent executable that looks like a shared object by ELF type but has an interpreter section; a Mach-O bundle that's neither a pure executable nor a pure dylib; a relocatable object file that's intended for further linking; or a binary whose headers are corrupted), the resulting component is typed deterministically using documented fallback rules. The analyst gets a sensible answer in every case, and the rule chosen is recorded so the classification is auditable.

**Why this priority**: Real-world filesystems contain edge cases — modern Linux distributions ship most executables as ET_DYN PIE binaries (which look like libraries by the raw ELF type but are functionally executables), Mach-O kernel extensions and loadable bundles, ELF `.o` relocatables in build directories, and so on. A naive header read gets these wrong. The fix needs a small set of well-documented disambiguation rules so the role classification is correct in the common case and predictable in the edge case.

**Independent Test**: Scan a directory containing a known PIE executable (e.g., any modern `/usr/bin/*` on a recent Debian/Ubuntu). Confirm the component is typed `application` despite the binary's ELF `e_type` reading `ET_DYN` (the file would naively look like a library). Repeat for a Mach-O bundle and confirm the documented fallback type applies. The classification rule used for each component should be recoverable from the emitted SBOM (as a property/annotation alongside the native type field).

**Acceptance Scenarios**:

1. **Given** an ELF PIE executable (ET_DYN with an interpreter program-header), **When** scanned, **Then** the component is typed `application` (not `library`).
2. **Given** an ELF shared object library (ET_DYN with `DT_SONAME` and no interpreter), **When** scanned, **Then** the component is typed `library`.
3. **Given** a Mach-O bundle (MH_BUNDLE), **When** scanned, **Then** the component is typed using the documented fallback for plugin-style bundles.
4. **Given** an ELF relocatable object file (ET_REL), **When** scanned, **Then** the component is typed `file` (or the format's equivalent "neither application nor library" value).
5. **Given** a binary whose header bytes are unparseable, **When** scanned, **Then** the component falls back to the current default (today's behavior) and the SBOM remains structurally valid.

---

### Edge Cases

- **ELF ET_DYN ambiguity (PIE executables vs shared libraries)**: Both modern executables (PIE-compiled) and shared libraries report ET_DYN. The distinguishing signals are the presence of `PT_INTERP` (program-header table contains an interpreter — typical of executables) and the presence of `DT_SONAME` in the dynamic section (typical of libraries). Documented disambiguation rule: `PT_INTERP` present → executable; otherwise if `DT_SONAME` present → library; otherwise fall back to library (the historic default).
- **Mach-O MH_BUNDLE**: Loadable plugin modules that aren't pure executables or dylibs. Documented as `library` per the spirit of the spec's `library` definition ("a static linkable or dynamically loadable code unit") — but consumers that want finer detail can read the `mikebom:binary-class` annotation.
- **Mach-O MH_OBJECT** (`.o` files in build output): Relocatable object files awaiting further linking; not directly executable and not a deployable library. Typed `file` (CycloneDX), `FILE` (SPDX 2.3), `file` (SPDX 3) — the "structured artifact that isn't an application or library" value.
- **ELF ET_REL** (`.o` files): Same treatment as Mach-O MH_OBJECT.
- **ELF ET_CORE** (core dumps): Out of scope — these aren't software components. If the binary reader picks one up (it shouldn't, but defense-in-depth), classify as `file`.
- **PE without a clear DLL/EXE characteristic**: Should be rare — the `IMAGE_FILE_DLL` bit in `IMAGE_FILE_HEADER.Characteristics` is canonical. Documented disambiguation: bit set → library; bit cleared → application.
- **Stripped binaries**: No effect on classification — the role is determined by the format header, not by debug-symbol presence.
- **Universal/fat Mach-O binaries**: Multiple slices, potentially of different classifications. Documented rule: classification is taken from the first slice (matches existing behavior of identity extractors like `LC_UUID` in milestone 030, and matches the on-disk semantics — the first slice is what `lipo -info` reports first).
- **Components emitted by non-binary readers** (npm packages, Maven jars, deb/rpm packages, Cargo crates, etc.): unaffected by this feature. Those readers continue to emit their components with whatever type they emit today. This feature only changes the type of components originating from the binary reader (`mikebom-cli/src/scan_fs/binary/`).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Mikebom MUST classify each binary-reader-discovered component into one of four roles based on the source file's format header: `Application` (Mach-O `MH_EXECUTE`; ELF `ET_EXEC` OR `ET_DYN` with `PT_INTERP` present; PE without `IMAGE_FILE_DLL` bit), `SharedLibrary` (Mach-O `MH_DYLIB`; ELF `ET_DYN` without `PT_INTERP` and with `DT_SONAME` present, OR `ET_DYN` as the documented fallback when neither marker is present; PE with `IMAGE_FILE_DLL` bit), `Object` (Mach-O `MH_OBJECT`; ELF `ET_REL`), or `Other` (Mach-O `MH_BUNDLE` and other format variants; ELF `ET_CORE` and other unrecognized values).
- **FR-002**: Mikebom MUST emit the classified role through each output format's native component-type field — CycloneDX `components[].type` and `metadata.component.type`, SPDX 2.3 `Package.primaryPackagePurpose`, SPDX 3 `software_Package.software_primaryPurpose` — using the format-equivalent enum value for each role: `Application` → CDX `application` / SPDX 2.3 `APPLICATION` / SPDX 3 `application`; `SharedLibrary` → CDX `library` / SPDX 2.3 `LIBRARY` / SPDX 3 `library`; `Object` → CDX `file` / SPDX 2.3 `FILE` / SPDX 3 `file`; `Other` → CDX `library` (the historic default, since no closer enum value exists) / SPDX 2.3 omitted / SPDX 3 omitted.
- **FR-003**: Mikebom MUST preserve the binary's format-detection signal (the `mikebom:binary-class` annotation that already exists per milestone 050+) alongside the new role classification, so consumers can recover both the format (`elf`/`macho`/`pe`) and the role (`application`/`library`/`file`/`other`) independently.
- **FR-004**: For ambiguous classifications (ELF `ET_DYN` without both `PT_INTERP` and `DT_SONAME`; Mach-O `MH_BUNDLE`), Mikebom MUST emit a non-fatal `tracing::info!` log identifying the component PURL and the fallback rule applied, so operators investigating unexpected role classifications have an audit trail without needing source-level debugging.
- **FR-005**: Mikebom MUST NOT change the emitted type for components originating from non-binary readers (package managers, manifest parsers, lockfile readers — npm, cargo, maven, pip, gem, deb, rpm, apk, etc.). Those components continue to use the type each reader emits today.
- **FR-006**: Mikebom MUST classify universal/fat Mach-O binaries from the first slice's filetype, matching the existing milestone-030 convention for identity metadata extraction from fat binaries.
- **FR-007**: Mikebom MUST keep the SBOM structurally valid (passes existing schema validation against CycloneDX 1.6, SPDX 2.3, and SPDX 3.0.1 schemas) for every classification including the fallback cases.
- **FR-008**: Cross-format role classifications MUST agree component-by-component: for any single component emitted in CycloneDX, SPDX 2.3, and SPDX 3 from the same scan, the role-typing enum values MUST map to equivalent semantics. A cross-format diff tool inspecting all three documents MUST see no role-typing divergence for the same component (this is the same parity invariant the milestone-085 holistic-parity machinery enforces for other cross-format-typed fields).
- **FR-009**: The classification logic MUST be deterministic across runs (same input file → same role classification → byte-identical type-field emission), so existing byte-identity golden fixtures regenerate cleanly without per-run drift.

### Key Entities

- **Binary role**: A four-valued classification (`Application`, `SharedLibrary`, `Object`, `Other`) attached to every binary-reader-discovered component. Derived from the source file's format header. Preserved verbatim through the resolved-component pipeline. Mapped at emission time to each output format's native enum value.
- **Resolved component**: The existing in-process representation of a discovered SBOM component (`mikebom_common::resolution::ResolvedComponent`). Gains one new optional field carrying the binary role (when the component came from the binary reader; absent otherwise). All other fields unchanged.
- **Format-native type field**: The slot in each output format that conveys "what kind of artifact is this component" — CycloneDX `Component.type`, SPDX 2.3 `Package.primaryPackagePurpose`, SPDX 3 `software_Package.software_primaryPurpose`. Each format uses its own enum values; the mapping table is part of this feature's documented behavior.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of executable binaries discovered by a `mikebom sbom scan --path <dir>` invocation, where the input directory contains nothing but binaries (e.g., scanning `/bin` or a `bin/` build-output directory), are emitted with the format-native "application" type in CycloneDX, SPDX 2.3, and SPDX 3 output.
- **SC-002**: For a scan that contains both executables and shared libraries, a consumer running a single-field native-type filter against the emitted SBOM can produce a complete and exclusive partition of components into "applications" vs "libraries" without needing to read any mikebom-prefixed annotation. Pre-fix the same filter returns every binary as "library" (one bucket, useless for the partition); post-fix the filter cleanly separates the two.
- **SC-003**: Cross-format consistency: for any scan emitted in CycloneDX, SPDX 2.3, and SPDX 3, the component-type field for every binary-reader-discovered component carries the role-equivalent value across all three formats (no format omits or disagrees on the role). The existing holistic-parity test suite (`mikebom-cli/tests/holistic_parity.rs`) is extended with a row covering this and passes for every ecosystem fixture that includes binaries.
- **SC-004**: Operator-facing documentation in `docs/reference/sbom-format-mapping.md` adds a new row describing the binary-role mapping table (CDX type ↔ SPDX 2.3 primaryPackagePurpose ↔ SPDX 3 software_primaryPurpose), so a downstream tooling author writing a consumer can build a correct cross-format role filter from the published reference alone.
- **SC-005**: SBOM consumers using the SPDX 2.3 + SPDX 3 conformance harnesses (the `spdx3-validate` validator already used in milestone 078, plus the LF SPDX 2.3 schema validator already used by the regression tests) report zero new validation errors after the fix — the new `primaryPackagePurpose` / `software_primaryPurpose` values emitted are all spec-conformant enum members.

## Assumptions

- The `object` crate (`object = "0.36"`, already a workspace dependency) exposes the format-specific header fields needed for role detection — Mach-O `LC_HEADER.filetype`, ELF `e_type` + `PT_INTERP`, PE `IMAGE_FILE_HEADER.Characteristics`. Initial code spot-check confirms these APIs exist; the plan phase will validate exhaustively before relying on them.
- Operating systems mikebom currently runs on (macOS, Linux, Windows per milestone 100) all surface enough of the binary format to make the classification call. No new platform support is added by this feature.
- The reported "inversion feel" comes from CycloneDX `type: "library"` being applied to executables — not from any literal source/target swap in dep edges (we verified this during diagnosis: relationships are direction-correct, just incomplete or absent). The fix is a typing change, not a graph-direction change.
- Existing component readers (deb, rpm, npm, cargo, etc.) continue to use the per-ecosystem default of `library` for their components even when those packages contain executables. Per-ecosystem role refinement (e.g., a deb package containing `/usr/bin/foo` getting some "applications-only" subtype) is out of scope for this milestone — would require inspecting each package's file list, which is a separate problem.
- Goldens regenerate as part of this milestone. The diffs are scoped to components that originated from the binary reader (file-level scans of executables/dylibs in fixtures). Most existing fixtures are manifest-/lockfile-driven and contain no binary-reader components, so will be byte-identical.
- The CycloneDX `metadata.component.type` field, when the scan's `metadata.component` is itself a binary (image scans, single-binary path scans), follows the same role-classification logic as `components[].type` — applying the rule to the synthetic-root level too keeps the document internally consistent. (For non-binary scan subjects — a manifest-driven cargo workspace, an npm package source tree — the existing `application` default for `metadata.component.type` is unchanged.)

## Dependencies

- Builds on the existing binary-reader infrastructure in `mikebom-cli/src/scan_fs/binary/` (milestones 023, 024, 030, 050, 096, 098, 099). Specifically extends `BinaryScan` (`scan.rs`) and the entry construction at `entry.rs` with the new role field; does not change the file-format parsers themselves.
- Builds on the existing cross-format-type-emission slots in `generate/cyclonedx/builder.rs:577`, `generate/spdx/packages.rs:509-514`, and the SPDX 3 equivalent in `generate/spdx/v3_packages.rs`.
- The component-type-mapping documentation in `docs/reference/sbom-format-mapping.md` is the audit-trail home per Constitution Principle V's documentation requirement for cross-format mappings.
- No new crate dependencies. No new transitive deps. No new CLI flags (the feature is automatic — operators do not opt in; existing scans simply produce correctly-typed components after the fix).
