# Feature Specification: Binary-tier completion — closing the three milestone-129 follow-ups

**Feature Branch**: `130-binary-tier-completion`
**Created**: 2026-06-18
**Status**: Draft
**Input**: User description: "merged, let's followup and see what we can do to fix the other stuff."

## Context

Milestone 129 shipped the `.deps.json` reader (US1A) and intentionally deferred three follow-ups
surfaced during /speckit-implement when the planning-phase assumptions met reality:

1. **`cargo_auditable.rs` returns zero components on the audit image** (PR #370 follow-up). mikebom already
   has a milestone-029 binary reader for `.dep-v0` ELF sections (240 LOC, ZLIB-decompresses + JSON-parses),
   wired into `scan_fs::binary::scan::scan_file` at `scan.rs:216` for ELF and `scan.rs:469` for Mach-O.
   The reader emits zero components on `/usr/bin/uv` and `/usr/bin/uvx` in the remediation-planner audit
   image. mikebom's 58 reported `pkg:cargo` components all come from a `Cargo.lock` source-tier hit at
   `/usr/lib64/rustlib/src/rust/library/Cargo.lock`, NOT from the binary reader. The cause is not yet
   diagnosed — three plausible hypotheses (binary not visited by scan loop, section discovery returns
   `None`, ZLIB decompression silently fails).
2. **Maven nested-JAR recursion is unimplemented.** mikebom's existing milestone-009 maven JAR reader
   handles top-level `.jar` files only; it does not descend into nested archives. Spring Boot uber JARs
   and similar fat-JAR shapes carry their dependency JARs inside `BOOT-INF/lib/` — invisible to mikebom
   pre-130.
3. **PE/CLR managed-assembly metadata reader is unimplemented.** On .NET images that ship the SDK or
   runtime store, ~451 unique NuGet packages live in `.dll` files' CLR metadata tables WITHOUT a
   neighboring `.deps.json` declaration (e.g. `Microsoft.AspNetCore.dll` from the reference assemblies
   pack). Milestone 129's US1A `.deps.json` reader cannot see these.

This feature closes the three follow-ups in priority order by coverage impact, with a debugging-first
posture on US1 (where the unknown is largest) so the milestone can scope-adjust quickly if the cargo
fix proves bigger than expected.

## Clarifications

### Session 2026-06-18

- Q: For US3, when the same `(AssemblyName, AssemblyVersion)` is detected across multiple culture-variant resource DLLs (e.g. `de/...`, `fr/...`, `ja/...`), should each emit a separate component or dedup to one with a culture-set annotation? → A: Dedup to one component per `(name, version)`. The set of detected cultures lists in a `mikebom:assembly-cultures` annotation (plural) so the audit trail is preserved without inflating component counts ~30× per package.
- Q: For US2 SC-003 verification, real Spring Boot uber JAR (committed binary) vs synthetic in-memory ZIP (test-build-time helper)? → A: Synthetic in-memory ZIP built at test-build time via a small builder helper. Determinism trumps fidelity; the shape mimicked (top-level JAR → `BOOT-INF/lib/<inner>.jar` → `META-INF/maven/.../pom.properties`) is exactly what the milestone-009 reader's parser expects. Matches the milestone-128 `recipe_body.rs` test-builder pattern and the milestone-090 fixture-cache "stay-set" rule.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Cargo dependency enumeration on cargo-auditable binaries works again (Priority: P1)

A platform engineer scans a container image that ships Rust binaries built with `cargo auditable` (Astral's
`uv` and `uvx`, `rustup`, `cargo-binstall`, plus a growing tail of CNCF tooling). The image carries no
source manifests. The engineer expects mikebom to enumerate every crate listed in the `.dep-v0` ELF
section of every audit-enabled binary in the rootfs. Pre-milestone-130, mikebom's existing reader silently
fails on real-world binaries — the engineer sees a handful of crates from an unrelated `Cargo.lock` but
none from the binaries themselves.

**Why this priority**: The unknown-but-bounded debugging task with potentially the largest coverage payoff
(~928 unique cargo packages on the audit image, vs ~451 for US3 and ~300 for US2). If the cause is a
ten-line fix, this single PR closes a 24% slice of mikebom's total package-coverage gap relative to syft
on the audit image. Even if the cause turns out to be deeper, the diagnostic work is bounded: there are
three candidate failure points in ~240 LOC of existing code, each independently verifiable.

**Independent Test**: Run `mikebom sbom scan --image <ref>` against an image carrying `uv`. The emitted
SBOM MUST contain `pkg:cargo/<crate>@<version>` components matching what the upstream `rust-audit-info`
reference tool extracts from the same binary, with `mikebom:source-mechanism = "cargo-auditable-binary"`
on each. Verifiable on the milestone-129 audit image (`remediation-planner:latest`) — expected
component-count delta is ~900 additional `pkg:cargo` components.

**Acceptance Scenarios**:

1. **Given** the `remediation-planner:latest` image (which ships `/usr/bin/uv` carrying a cargo-auditable
   manifest with ~200 crates), **When** the engineer runs `mikebom sbom scan --image
   767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner:latest --output cyclonedx-json=/tmp/out.cdx.json --offline`,
   **Then** the emitted SBOM contains AT LEAST 200 components matching `pkg:cargo/<name>@<version>`
   whose `mikebom:source-mechanism` annotation equals `"cargo-auditable-binary"`.
2. **Given** a synthetic ELF fixture with a `.dep-v0` section carrying 5 crate entries, **When** the
   engineer runs `mikebom sbom scan --path <dir>`, **Then** the emitted SBOM contains 5 `pkg:cargo`
   components with the matching `(name, version)` pairs.
3. **Given** a synthetic ELF fixture without a `.dep-v0` section, **When** the engineer runs the scan,
   **Then** the scan does NOT fail and zero `pkg:cargo` components attributed to that binary are emitted.
4. **Given** the cause of the existing failure is diagnosed and documented in the PR, **When** a regression
   test fixture is added covering the failure mode, **Then** the regression test fails against
   alpha.48 + milestone-129 and passes on milestone-130.

---

### User Story 2 — Maven dependencies inside fat JARs are enumerated (Priority: P2)

A platform engineer scans a container image carrying a Java application packaged as a Spring Boot uber
JAR / shaded JAR / WAR file. The application's runtime classpath includes dozens of transitive
dependencies whose `META-INF/maven/.../pom.properties` entries live INSIDE the application JAR, not as
separate top-level JAR files in the image rootfs. The engineer expects mikebom to descend into the
nested archives and enumerate every embedded dependency.

**Why this priority**: Bounded implementation work (~300-400 LOC extending the existing milestone-009
reader). Modest absolute coverage gain (~300 unique maven packages on the audit image) but high per-image
impact for enterprise Java images (every Spring Boot / Quarkus / Micronaut microservice image follows this
shape). The scope is well-understood: depth-limited recursive `zip::ZipArchive` descent with cycle
detection and a per-archive decompressed-size cap.

**Independent Test**: Run `mikebom sbom scan --path <dir>` against a directory containing a single
Spring Boot uber JAR (e.g. one built from `spring-projects/spring-petclinic`). The emitted SBOM MUST
contain a `pkg:maven/<group>/<artifact>@<version>` component for every nested JAR's
`META-INF/maven/.../pom.properties` entry, with depth bounded at 8 levels.

**Acceptance Scenarios**:

1. **Given** a Spring Boot uber JAR carrying 50 nested dependency JARs in its `BOOT-INF/lib/` directory,
   **When** the engineer runs the scan, **Then** the emitted SBOM contains 50 `pkg:maven/.../...@<version>`
   components whose coordinates match each nested JAR's `META-INF/maven/.../pom.properties`, each
   carrying `mikebom:source-mechanism = "maven-jar-nested"`.
2. **Given** a fat JAR with deeply-nested archives (an EAR file containing WARs containing JARs containing
   JARs, three or more levels deep), **When** the engineer runs the scan, **Then** the walker descends to
   a depth limit of 8 levels and stops gracefully without infinite recursion.
3. **Given** a malformed JAR (corrupt central directory) inside an otherwise-well-formed parent JAR,
   **When** the engineer runs the scan, **Then** the scan does NOT fail; the malformed nested archive
   surfaces via a `warn`-level log; processing continues on sibling entries.
4. **Given** a synthetic "zip bomb" nested archive declaring an uncompressed size of 2 GB, **When** the
   engineer runs the scan, **Then** the walker skips the entry with a `warn` log and continues; the
   scan does not exhaust memory.

---

### User Story 3 — PE/CLR managed-assembly metadata enumerated when `.deps.json` is absent (Priority: P3)

A platform engineer scans a .NET container image where some assemblies are present WITHOUT a neighboring
`.deps.json` declaration — for example, reference assemblies under
`/usr/share/dotnet/packs/Microsoft.AspNetCore.App.Ref/<ver>/ref/net8.0/`, MSBuild tasks DLLs
(`DotNetWatchTasks.dll`), or CLI host extensions. mikebom's milestone-129 `.deps.json` reader cannot
see these. The engineer expects mikebom to extract `(AssemblyName, AssemblyVersion)` from each managed
PE assembly's CLR metadata and emit a `pkg:nuget/<name>@<version>` component.

**Why this priority**: Largest single new-code item (~800-1000 LOC for an ECMA-335 §II.22 hand-roll on
top of `object` 0.36's PE primitives). Closes ~451 unique NuGet packages on the audit image (the
delta between mikebom's milestone-129 184 unique and syft's 635 unique). Naturally last in priority because
the implementation work bounds the timeline more than the other two stories.

**Independent Test**: Run `mikebom sbom scan --image <ref>` against a Microsoft-published .NET runtime
image (e.g. `mcr.microsoft.com/dotnet/runtime:8.0-alpine`). For every managed `.dll` in the image that
has a non-zero `IMAGE_DIRECTORY_ENTRY_COM_DESCRIPTOR` data directory entry, the emitted SBOM MUST contain
a `pkg:nuget/<assembly-name>@<version>` component sourced from PE metadata with
`mikebom:source-mechanism = "dotnet-assembly-metadata"`.

**Acceptance Scenarios**:

1. **Given** an image with `/usr/share/dotnet/packs/Microsoft.AspNetCore.App.Ref/8.0.27/ref/net8.0/Microsoft.AspNetCore.dll`
   (a managed reference assembly with no neighboring `.deps.json`), **When** the engineer runs the scan,
   **Then** the emitted SBOM contains a `pkg:nuget/Microsoft.AspNetCore@8.0.2726.23008` component with
   the version extracted from the `AssemblyInformationalVersionAttribute → AssemblyFileVersionAttribute
   → AssemblyVersion` fallback ladder.
2. **Given** the same image's `Microsoft.AspNetCore.App.Runtime.wolfi.20230201-x64` package whose `.deps.json`
   ALSO declares `Microsoft.AspNetCore@8.0.2726.23008`, **When** the engineer runs the scan, **Then** the
   emitted SBOM contains exactly ONE `Microsoft.AspNetCore` component (the dedup pipeline collapses the
   collision) with `mikebom:also-detected-via` listing both `dotnet-deps-json` AND `dotnet-assembly-metadata`.
3. **Given** an image with a native `.dll` (a Win32 DLL with no CLR header — `DataDirectory[14]` is
   zeroed), **When** the engineer runs the scan, **Then** the scan does NOT emit a `pkg:nuget` component
   for the file and does NOT emit a `parse-failure` annotation (silent skip).
4. **Given** an image with a managed `.dll` whose CLR metadata tables are corrupt or truncated, **When**
   the engineer runs the scan, **Then** the scan does NOT fail; the file surfaces via a `warn`-level
   log naming the path; processing continues on sibling files.

---

### Edge Cases

- **cargo-auditable + Mach-O**: mikebom's existing `scan.rs:469` Mach-O path already handles
  `__DATA,.dep-v0` segment-prefixed sections via `object::section_by_name_bytes`. The US1 debug work
  MUST NOT regress Mach-O extraction.
- **cargo-auditable + PE (Windows binaries inside Linux containers)**: rare but possible. The existing
  reader's `section_by_name_bytes(b".dep-v0")` should match PE section table entries too. US1 debug
  MUST verify PE path still works post-fix.
- **Nested JAR cycle**: detected via SHA-256 visited set, mirroring the milestone-128 include-chain
  convention. The walker returns silently when re-encountering a hash; no log noise.
- **Nested JAR depth limit**: 8 levels matching milestone-128 `INCLUDE_DEPTH_LIMIT`. Empirically the
  deepest real-world nesting is 4 (EAR > WAR > JAR > nested-test-JAR); the 8-level bound tolerates
  pathological inputs.
- **CLR assembly with no `Assembly` table row 0**: an obfuscated or stripped managed assembly that
  has the CLR header but no readable metadata. Skipped silently — the absence of an Assembly identity
  is indistinguishable from a native DLL at the action level.
- **CLR assembly with `Culture` other than "neutral"**: e.g. localized resource assemblies
  (`<lang>/Microsoft.AspNetCore.resources.dll`). Per FR-024 + the 2026-06-18 clarification, every
  culture variant for the same `(name, version)` merges into a single component via the milestone-105
  dedup pipeline; the union of detected non-"neutral" cultures emits as the
  `mikebom:assembly-cultures` annotation.
- **Dedup collision priority**: `.deps.json` declarations take precedence over PE-derived components
  because the `.deps.json` carries higher-fidelity version data including pre-release suffixes. The
  PE component still emits; the dedup pipeline keeps the higher-fidelity source-mechanism field.
- **Maven `.jar` files inside .NET runtime store**: rare but observed in some images. The nested-JAR
  walker MUST NOT descend into `.jar` files inside `.dll` files or vice versa — extension-based
  dispatch keeps this clean.

## Requirements *(mandatory)*

### Functional Requirements

#### Cross-cutting (all three stories)

- **FR-001**: Every component emitted by a milestone-130 reader MUST carry `mikebom:sbom-tier = "image"`
  (consistent with milestone 129 US1A).
- **FR-002**: Every component MUST carry a `mikebom:source-mechanism` annotation naming the specific
  extractor: `cargo-auditable-binary`, `maven-jar-nested`, `dotnet-assembly-metadata`.
- **FR-003**: Collisions with existing readers (source-tier or other image-tier) flow through the
  milestone-105 dedup pipeline; the surviving component carries `mikebom:also-detected-via` listing all
  source mechanisms.
- **FR-004**: Every new reader MUST respect `--offline` (no network, no subprocess) and `--exclude-path`
  (routes via milestone-114 `safe_walk`).
- **FR-005**: Every new reader MUST handle malformed input gracefully — a single failure surfaces via
  a `warn`-level log line; the scan continues on sibling files. No silent omission of failures from logs.
- **FR-006**: SC-005 byte-identity preservation — for images where the new readers find no applicable
  inputs (a pure-Go image, a pure-Python image, etc.), the emitted SBOM MUST be byte-identical to the
  pre-130 output across the 33 committed alpha.48 goldens.

#### US1 — cargo-auditable debugging

- **FR-007**: Investigation MUST produce a written diagnosis of why the existing milestone-029 reader
  returns zero components on `/usr/bin/uv` and `/usr/bin/uvx` in the remediation-planner audit image.
  The diagnosis MUST identify which of these four candidate checkpoints fires AND which one fails:
  (a) the binary scan loop visits the file; (b) `section_by_name_bytes(b".dep-v0")` returns `Some`;
  (c) ZLIB decompression succeeds; (d) the post-scan emission gate at
  `mikebom-cli/src/scan_fs/binary/mod.rs:700` (`if !skip_secondary_evidence { ... }`) lets the per-crate
  emissions through. The diagnosis MUST be documented in `specs/130-binary-tier-completion/research.md §R1`
  and referenced from the PR description. **Note**: per planning-phase code trace, the cause is the (d)
  post-scan emission gate — (a), (b), (c) all succeed; the cargo-auditable manifest is decoded
  correctly but suppressed downstream. The PR MUST link to research.md §R1 as the diagnosis artifact.
- **FR-008**: A regression-test fixture MUST be added that fails against alpha.48 + milestone-129 and
  passes on milestone-130, covering the diagnosed failure mode.
- **FR-009**: Post-fix, the emitted SBOM for the remediation-planner audit image MUST contain at least
  900 unique `pkg:cargo/<name>@<version>` components carrying
  `mikebom:source-mechanism = "cargo-auditable-binary"` (within ~9% of syft's 986 audit baseline).
- **FR-010**: The fix MUST NOT regress the existing 240-LOC `parse_dep_v0` reader's unit tests at
  `mikebom-cli/src/scan_fs/binary/cargo_auditable.rs` or the Mach-O path at `scan.rs:469`.

#### US2 — Maven nested-JAR recursion

- **FR-011**: The existing milestone-009 maven JAR reader at `mikebom-cli/src/scan_fs/package_db/maven.rs`
  MUST be extended with a recursive nested-archive walker that descends into entries with `.jar`,
  `.war`, or `.ear` extensions inside the outer archive's ZIP central directory.
- **FR-012**: Descent depth MUST be capped at 8 levels (matching the milestone-128 `INCLUDE_DEPTH_LIMIT`
  convention).
- **FR-013**: Cycle detection MUST be implemented via SHA-256-keyed visited set on each archive's bytes;
  re-encountered archives are silently skipped to break the cycle.
- **FR-014**: Per-archive decompressed-size cap MUST be 1 GB. Archives declaring a higher uncompressed
  size are skipped with a `warn` log; never extracted into memory.
- **FR-015**: For each nested archive's `META-INF/maven/<group>/<artifact>/pom.properties`, emit a
  `pkg:maven/<group>/<artifact>@<version>` component with `mikebom:source-mechanism = "maven-jar-nested"`.
  Top-level JAR emissions retain the existing `"maven-jar"` source-mechanism.
- **FR-016**: The `mikebom:source-files` annotation on nested-emitted components MUST use the JAR-URL
  convention: `<outer-path>!<inner-path>!<deeper-path>...`.
- **FR-017**: `.zip` entries inside JARs MUST NOT be descended into. The milestone-129 clarification Q2
  rationale (false-positive risk on maven-assembly-plugin distribution archives) carries forward verbatim.

#### US3 — PE/CLR managed-assembly metadata

- **FR-018**: A new reader at `mikebom-cli/src/scan_fs/binary/dotnet_pe_clr.rs` MUST walk the rootfs
  for `*.dll` files, parse each as a PE via `object::read::pe::PeFile`, and gate further processing
  on `is_managed_assembly()`: the file's `IMAGE_OPTIONAL_HEADER.DataDirectory[14]` (the
  `IMAGE_DIRECTORY_ENTRY_COM_DESCRIPTOR` entry, holding the `IMAGE_COR20_HEADER`) MUST have non-zero
  `VirtualAddress` and `Size`.
- **FR-019**: For managed assemblies, the reader MUST extract `AssemblyName`, `AssemblyVersion`
  (4-tuple), `AssemblyFileVersion` (if present), `AssemblyInformationalVersion` (if present), and
  `Culture` (if present and not "neutral") per ECMA-335 §II.22.
- **FR-020**: The emitted `pkg:nuget` component's version field MUST follow the milestone-129 clarification
  Q3 fallback ladder: `AssemblyInformationalVersion → AssemblyFileVersion → AssemblyVersion`.
- **FR-021**: All three extracted version strings MUST be surfaced as separate annotations
  (`mikebom:assembly-version-informational`, `mikebom:assembly-version-file`,
  `mikebom:assembly-version-runtime`) when present. Absent fields produce no annotation entry.
- **FR-022**: Non-managed `.dll` files (Win32 native DLLs, native MSVCRT-style DLLs) MUST be silently
  skipped with no log entry. The `is_managed_assembly()` gate is the discriminator.
- **FR-023**: Corrupt or truncated CLR metadata tables MUST emit a single `warn` log + skip; the scan
  does NOT abort.
- **FR-024**: When multiple culture-variant resource DLLs share the same `(AssemblyName,
  AssemblyVersion)` (e.g. `de/...resources.dll`, `fr/...resources.dll`, `ja/...resources.dll` — a typical
  .NET runtime image carries 30+ cultures per package), the existing milestone-105 dedup pipeline MUST
  collapse them into a SINGLE component per `(name, version)`. The set of detected non-"neutral"
  cultures (sorted, comma-joined) MUST surface as a `mikebom:assembly-cultures` annotation on the
  surviving component — preserving the audit trail without inflating component counts ~30× per package
  (resolved 2026-06-18 clarification). Assemblies with only the "neutral" culture omit the annotation.

#### Catalog / parity bookkeeping

- **FR-025**: Any new `mikebom:*` annotation key MUST be catalogued in
  `docs/reference/sbom-format-mapping.md` with full Principle V audit narrative per the milestone-128
  convention. New keys this milestone introduces: `mikebom:assembly-version-informational`,
  `mikebom:assembly-version-file`, `mikebom:assembly-version-runtime`, `mikebom:assembly-cultures`.
- **FR-026**: Each catalogued key MUST be registered as a `ParityExtractor` slice entry with matching
  `cdx_anno!` / `spdx23_anno!` / `spdx3_anno!` macros, emitting `SymmetricEqual` across all three formats.

### Key Entities

- **`.dep-v0` ELF section** (US1): Zlib-compressed JSON payload at section name `.dep-v0`. Schema
  identical to milestone 029's existing `CargoAuditableManifest` struct at
  `mikebom-cli/src/scan_fs/binary/cargo_auditable.rs:60-90`. No new struct introduced.
- **Nested archive** (US2): A JAR / WAR / EAR file embedded as a ZIP central-directory entry inside
  another archive. Same shape as milestone-129's deferred `NestedArchiveWalker` design in
  `specs/129-binary-tier-readers/data-model.md`.
- **Managed PE assembly** (US3): A `.dll` file with both a PE COFF header AND a CLR `IMAGE_COR20_HEADER`
  data directory entry. Carries metadata tables including `Assembly` (token 0x20) for
  `AssemblyName + Version 4-tuple`, and `CustomAttribute` (token 0x0C) for `AssemblyFileVersionAttribute`
  + `AssemblyInformationalVersionAttribute`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For the audit image `767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner:latest`,
  the post-130 emitted SBOM's unique `pkg:cargo` component count MUST be ≥900 (vs syft's 986 unique
  baseline; milestone 129 left this at 58).
- **SC-002**: For the same audit image, the unique `pkg:nuget` component count from
  `mikebom:source-mechanism = "dotnet-assembly-metadata"` (US3) MUST be ≥400 (the gap between
  milestone-129's 184 unique and syft's 635 unique).
- **SC-003**: For a synthetic Spring Boot-shaped uber JAR built at test-build time via a small
  `tests/common/maven_jar_builder.rs` helper (per the 2026-06-18 clarification — no committed binary
  JAR; the builder emits a top-level JAR carrying N nested JARs in `BOOT-INF/lib/` each with a planted
  `META-INF/maven/<group>/<artifact>/pom.properties`), the post-130 emitted SBOM MUST contain at
  least one `pkg:maven` component per nested JAR's `pom.properties`, with the
  `mikebom:source-mechanism = "maven-jar-nested"` annotation distinguishing them from top-level
  emissions.
- **SC-004**: The sbom-comparison weighted score on the audit image MUST be ≥4.5 (alpha.48 was 3.3;
  milestone-129 didn't move the weighted score because completeness improvements were partial).
- **SC-005**: The 33 committed alpha.48 goldens MUST regen with zero `.cdx.json` / `.spdx.json` churn.
- **SC-006**: Total scan time growth on the audit image MUST be under 30% relative to milestone 129
  (the PE/CLR walker is the largest new cost; per-`.dll` parse latency MUST be under 20 ms).
- **SC-007**: Each user story is independently shippable — a partial milestone delivering only US1
  (cargo debug) closes the largest single coverage gap and can ship as PR #N+1 if US2/US3 slip.

## Assumptions

- The cargo-auditable debug investigation will pinpoint a fixable cause inside the existing 240-LOC
  reader OR a single missing call site in the scan dispatch. If the cause is structural (e.g. mikebom's
  binary-scan dispatcher doesn't visit `/usr/bin/uv` at all), the fix scope expands but the investigation
  is the gating step regardless.
- The Spring Boot uber JAR test fixture will be a synthetic in-memory ZIP construction (mirroring
  milestone-129's deferred US3 fixture plan) — no committed binary JAR. Reduces fixture-cache footprint
  + makes the test deterministic on every machine.
- The PE/CLR metadata-table reader is a bounded ECMA-335 hand-roll (~800-1000 LOC) leveraging
  `object` 0.36's PE primitives. We do NOT pull in the `pelite` crate (~5 KLOC, ~30 transitive deps);
  Constitution Principle I (zero new C deps) and the historical "zero new Cargo deps" posture argue
  against it.
- Mach-O cargo-auditable extraction (already at `scan.rs:469`) is presumed working — milestone-130 US1
  focuses on the ELF path that demonstrably fails on the audit image. Mach-O fixture coverage stays
  unchanged.
- The milestone-129 US1A `.deps.json` reader's dedup integration (via milestone-105's source-mechanism
  enum) extends seamlessly to milestone-130's new mechanisms. No dedup pipeline changes anticipated.

## Out of Scope

- File-type component inventory (syft's 27,004 `syft:file` entries on the audit image). mikebom's design
  choice continues to emit `library` and `application` components only.
- `cargo-auditable` v1 wire format (currently v0; v1 not yet ratified at time of writing). Add when
  upstream ratifies.
- WIX MSI installer reading (used by some Windows .NET runtime installers; irrelevant on Linux containers).
- EAR-file deployment descriptor (`application.xml`) module-level metadata. The nested-archive walker
  enumerates JAR/WAR/EAR content; it does not parse Java EE deployment descriptors.
- WebAssembly binaries (`.wasm`) carrying their own dependency metadata.
- Reading native (non-managed) `.dll` files for embedded `VERSIONINFO` resource blocks. The managed-assembly
  reader operates only on CLR-tagged DLLs.

## Dependencies

- Existing milestone-029 `cargo_auditable.rs` reader (US1 debug target).
- Existing milestone-009 maven JAR reader (US2 extension target).
- Existing milestone-105 `SourceMechanism`-based dedup pipeline (collision handling for cross-mechanism
  duplicates).
- Existing milestone-114 `safe_walk` helper (rootfs traversal for all three readers).
- Existing milestone-113 `--exclude-path` flag (honored by all three readers).
- Existing milestone-097 `mikebom:cpe-candidates` annotation channel (reused for new components).
- Existing milestone-128 parity catalog C-row system in `docs/reference/sbom-format-mapping.md` (4 new
  C-rows for the new annotation keys).
- The `object` crate (workspace dep; PE COFF + CLR header parsing via `optional_header().data_directories()[14]`).
- The `zip` crate (workspace dep; nested archive descent).
- The `flate2` crate (workspace dep; reused by US1 if the debug fix touches decompression).
- The `serde_json` crate (workspace dep).
- The milestone-129 audit corpus (`/Users/mlieberman/Projects/sbom-comparison/sbom-comparison` tool +
  the syft baseline at `~/Downloads/remediation-planner-syft-image-sbom.json` + the
  `remediation-planner:latest` ECR image) for end-to-end SC verification.
