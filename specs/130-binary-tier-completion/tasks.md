---

description: "Task list for milestone 130 — Binary-tier completion (cargo-auditable fix + maven nested-JAR + PE/CLR managed-assembly)"

---

# Tasks: Binary-tier completion (milestone 130)

**Input**: Design documents from `/specs/130-binary-tier-completion/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/{annotation-schema.md,reader-behavior.md}, quickstart.md

**Tests**: REQUIRED. The spec's user story acceptance scenarios are expressed in Given/When/Then
form; SC-001..006 are unit/integration-test verifiable. Test tasks are included alongside
implementation tasks.

**Organization**: Tasks are grouped by user story (US1 P1, US2 P2, US3 P3) so each can be shipped
independently per SC-007. The recommended strategy is **three sequential PRs**: US1 (5-LOC fix, fastest
win) → US2 (~400 LOC) → US3 (~1000 LOC). Spec/plan support a single bundled PR if all three fit one
push, but the natural seam between phases makes the split obvious.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to ([US1], [US2], [US3])

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify the workspace is on `130-binary-tier-completion` branch and the milestone-129
work is merged. No new files in this phase — milestone 130 reuses existing infrastructure
throughout.

- [ ] T001 Run `git rev-parse --abbrev-ref HEAD` and confirm `130-binary-tier-completion`. Run `git log -1 --oneline main` and confirm the milestone-129 US1A `.deps.json` reader commit (`feat(scan_fs/nuget): .deps.json reader`) is present. If on a fresh setup, run `git checkout main && git pull && git checkout -b 130-binary-tier-completion`.
- [ ] T002 [P] Run `cargo +stable build -p mikebom` to confirm the workspace baseline builds clean before any milestone-130 edits.
- [ ] T003 [P] Run `cargo +stable test -p mikebom --bin mikebom nuget::deps_json 2>&1 | tail -5` to confirm milestone 129 US1A's 10 unit tests still pass. Baseline.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: None. Milestone 130 has zero foundational tasks because all three user stories
operate on existing infrastructure (milestone-105 dedup, milestone-114 safe_walk, milestone-097
CPE annotation channel, milestone-009 maven reader, milestone-029 cargo-auditable reader). Each
user story is fully independent and parallelizable from this point.

**Checkpoint**: User story implementation can begin in parallel.

---

## Phase 3: User Story 1 — Cargo dependency enumeration on cargo-auditable binaries works again (Priority: P1) 🎯 MVP

**Goal**: Remove the `skip_secondary_evidence` gate around the cargo-auditable emission block at
`mikebom-cli/src/scan_fs/binary/mod.rs:700`. On the audit image, this lifts unique `pkg:cargo`
coverage from 58 to ≥900 components.

**Independent Test**: Run `mikebom sbom scan --image 767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner:latest --output cyclonedx-json=/tmp/out.cdx.json --offline` and confirm
`jq -r '.components[].purl' /tmp/out.cdx.json | grep -c '^pkg:cargo'` returns ≥900.

### Implementation

- [ ] T004 [US1] Open `mikebom-cli/src/scan_fs/binary/mod.rs` at lines 700-708. Replace the `if !skip_secondary_evidence { if let Some(ref manifest) = scan.cargo_auditable { ... } }` block with an UNGATED `if let Some(ref manifest) = scan.cargo_auditable { ... }` block, preserving the inner `cargo_auditable_packages_to_entries` call. Add an inline comment per `contracts/reader-behavior.md` Reader 1 explaining the gate removal: cargo-auditable per-crate emissions are NOT shadows of the file-level binary identity.
- [ ] T005 [US1] Verify all OTHER `skip_secondary_evidence`-gated blocks at the same file (version-string scan ~line 502, linkage ~line 530, ELF-note ~line 561) are UNCHANGED. Run `grep -n "skip_secondary_evidence" mikebom-cli/src/scan_fs/binary/mod.rs` and confirm the only change is the cargo-auditable block at line 700.

### Regression test

- [ ] T006 [US1] Create `mikebom-cli/tests/fixtures/binary_tier_completion/cargo_auditable_regression/` directory. Build a synthetic ELF fixture (`claimed_binary_with_dep_v0.elf`) at test-build time via a helper using the `object` crate's `object::write` API. The fixture is a minimal valid ELF64 with: (a) ELF magic + valid PE header structure; (b) a `.dep-v0` section carrying a zlib-compressed JSON payload `{"packages":[{"name":"foo","version":"1.0.0","source":"crates-io","root":false},{"name":"bar","version":"2.0.0","source":"crates-io","root":false}]}`; (c) file size > 1 KB to pass the binary-tier size envelope.
- [ ] T007 [US1] Create `mikebom-cli/tests/binary_tier_completion_us1_cargo_auditable.rs` integration test. Test 1: scan a directory containing ONLY the synthetic ELF AND a synthetic apk database fixture that "claims" the ELF's path (so `path_claimed = true` in the binary-tier dispatcher). Assert the emitted SBOM contains 2 `pkg:cargo` components (`foo@1.0.0`, `bar@2.0.0`), each carrying `mikebom:source-mechanism = "cargo-auditable-binary"`. This test FAILS against alpha.48 + milestone-129 (0 emitted) and PASSES against milestone-130 (2 emitted) per FR-008.
- [ ] T008 [US1] Add a second test to `binary_tier_completion_us1_cargo_auditable.rs`: scan the same synthetic ELF WITHOUT a path-claim (clean directory). Assert the emitted SBOM still contains the 2 `pkg:cargo` components — verifying that the gate-removal hasn't regressed the unclaimed-binary case.

### Verification

- [ ] T009 [US1] Run `cargo +stable test -p mikebom --bin mikebom cargo_auditable` and confirm the existing 240-LOC `parse_dep_v0` unit tests at `mikebom-cli/src/scan_fs/binary/cargo_auditable.rs` (lines 1393-1450) still pass — verifying FR-010 no-regression on the reader-level tests.
- [ ] T010 [US1] Run `cargo +stable test -p mikebom --test binary_tier_completion_us1_cargo_auditable` and confirm both integration test scenarios pass.
- [ ] T011 [US1] End-to-end audit-image check: run `mikebom sbom scan --image 767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner:latest --output cyclonedx-json=/tmp/us1.cdx.json --offline` and assert `jq -r '.components[].purl' /tmp/us1.cdx.json | grep -c '^pkg:cargo'` returns ≥900 (SC-001). Document the actual count in the PR description.

**Checkpoint**: US1 fully functional and independently shippable. SC-001 verifiable.

---

## Phase 4: User Story 2 — Maven dependencies inside fat JARs are enumerated (Priority: P2)

**Goal**: Extend the existing milestone-009 maven reader with depth-bounded recursive nested-archive
descent per FR-011..017.

**Independent Test**: Build a Spring Boot uber JAR shape via the new
`tests/common/maven_jar_builder.rs` helper; scan via `mikebom sbom scan --path <dir>`; assert one
`pkg:maven` component per nested JAR's `pom.properties`.

### Test infrastructure

- [ ] T012 [P] [US2] Create `mikebom-cli/tests/common/maven_jar_builder.rs` — a synthetic ZIP builder helper per the 2026-06-18 clarification Q2. Functions: `build_jar_with_pom_properties(group, artifact, version) -> Vec<u8>` (produces a minimal JAR carrying a `META-INF/maven/<group>/<artifact>/pom.properties` entry); `build_uber_jar(nested_jars: Vec<Vec<u8>>) -> Vec<u8>` (produces a Spring Boot-shaped outer JAR with each inner JAR placed at `BOOT-INF/lib/<name>.jar`).
- [ ] T013 [P] [US2] Add zip-bomb test helper to `tests/common/maven_jar_builder.rs`: `build_zip_bomb_archive(uncompressed_size_declared: u64) -> Vec<u8>` constructs a JAR with a forged central-directory entry declaring an uncompressed_size > 1 GB (the actual deflate payload stays minimal — this is for testing the size-cap check, not actually decompressing 1 GB).

### Implementation

- [ ] T014 [US2] In `mikebom-cli/src/scan_fs/package_db/maven.rs`, add the `NestedArchiveWalker` struct per data-model.md Entity 2. Fields: `visited: HashSet<[u8; 32]>`, `depth: u8`, `size_cap: u64`, `out: Vec<PackageDbEntry>`, `outer_path: PathBuf`, `nested_stack: Vec<String>`. Default `size_cap = 1 << 30` (1 GB).
- [ ] T015 [US2] Implement `NestedArchiveWalker::walk(&mut self, archive_bytes: &[u8])` per data-model.md Entity 2: SHA-256 of `archive_bytes` via existing `sha2::Sha256` helper; cycle-check via `visited.insert`; gate on `depth < 8`; open `zip::ZipArchive::new(Cursor::new(archive_bytes))`; iterate entries.
- [ ] T016 [US2] In `walk`'s entry-iteration loop, add the `META-INF/maven/<group>/<artifact>/pom.properties` matcher. For each match: parse the `name=` and `version=` lines, construct `pkg:maven/<group>/<artifact>@<version>` via the existing `mikebom_common::types::purl::Purl::new` helper, emit a `PackageDbEntry` with `mikebom:source-mechanism = "maven-jar-nested"` and `mikebom:source-files = "<outer_path>!<nested_stack joined by !>"` per FR-016.
- [ ] T017 [US2] In `walk`'s entry-iteration loop, add the `.jar` / `.war` / `.ear` extension filter (case-insensitive). For each matching entry: check the entry's `size()` (uncompressed declared) ≤ 1 GB; if exceeded, emit `tracing::warn!` per FR-014 + SKIP; if within cap, extract entry bytes into a `Vec<u8>` via `zip::read::ZipFile::read_to_end`, push entry name to `self.nested_stack`, increment `self.depth`, recursively call `self.walk(&inner_bytes)`, decrement `self.depth`, pop `self.nested_stack`.
- [ ] T018 [US2] Per FR-017 + clarification Q2 (carried from milestone 129): `.zip` entries MUST NOT trigger recursion. Add an explicit check that excludes `.zip` from the extension filter; ensure the test in T020 covers this.
- [ ] T019 [US2] Wire `NestedArchiveWalker::walk` into the existing milestone-009 reader's per-JAR processing site. Find the function in `package_db/maven.rs` that opens each top-level JAR; AFTER it emits the existing top-level component (with `mikebom:source-mechanism = "maven-jar"`), instantiate a `NestedArchiveWalker` and call `walker.walk(&jar_bytes)`. Drain `walker.out` into the reader's output `Vec<PackageDbEntry>`.

### Unit tests

- [ ] T020 [US2] Add unit tests inside `package_db/maven.rs` for: SHA-256 cycle detection (an archive containing itself returns after one descent); depth limit (an 8-level-deep nest triggers the warn); 1 GB size cap (a forged-size entry skips with warn); extension filter (a `.zip` entry inside a JAR does NOT recurse); `!`-separator path construction (`outer.jar!BOOT-INF/lib/inner.jar!META-INF/maven/foo/bar/pom.properties`).

### Integration test

- [ ] T021 [US2] Create `mikebom-cli/tests/binary_tier_completion_us2_maven_nested_jar.rs` integration test using the `tests/common/maven_jar_builder.rs` helper. Test 1: scan a synthetic uber JAR with 5 nested JARs each declaring distinct `(group, artifact, version)` — assert the emitted SBOM contains 5 `pkg:maven` components with `mikebom:source-mechanism = "maven-jar-nested"` + 1 top-level component with `"maven-jar"`.
- [ ] T022 [US2] Add test 2 to the integration test: scan an EAR > WAR > JAR > JAR 4-level-deep nest. Assert all four levels' `pom.properties` emit. Verify the `mikebom:source-files` annotation on the deepest component contains exactly four `!` separators.
- [ ] T023 [US2] Add test 3: scan a malformed JAR (corrupt central directory) inside an otherwise-valid uber JAR. Assert the scan does NOT fail; the malformed inner archive surfaces via a `warn`-level log (verifiable via stderr capture); processing continues — verifying FR-005.
- [ ] T024 [US2] Add test 4: scan an uber JAR carrying a forged-size zip-bomb entry. Assert the entry is skipped with a `warn` log; the scan does not exhaust memory (the actual deflate payload is tiny, so this is a logic-path check not a stress test).

### Verification

- [ ] T025 [US2] Run `cargo +stable test -p mikebom --test binary_tier_completion_us2_maven_nested_jar` and confirm all 4 tests pass.
- [ ] T026 [US2] Run `cargo +stable test -p mikebom --bin mikebom maven` to verify all 4 new unit tests + existing milestone-009 unit tests pass — verifying no regression on top-level JAR enumeration.

**Checkpoint**: US2 fully functional. SC-003 verifiable.

---

## Phase 5: User Story 3 — PE/CLR managed-assembly metadata enumerated when `.deps.json` is absent (Priority: P3)

**Goal**: New reader at `mikebom-cli/src/scan_fs/binary/dotnet_pe_clr.rs` parsing ECMA-335 §II.22
metadata tables from managed PE assemblies. Closes ~451 unique NuGet packages on the audit image.

**Independent Test**: Scan a dotnet runtime image; assert ≥400 components with
`mikebom:source-mechanism = "dotnet-assembly-metadata"`.

### Test infrastructure

- [ ] T027 [P] [US3] Create `mikebom-cli/tests/fixtures/binary_tier_completion/dotnet_pe_clr/builder.rs` — a test-time helper that constructs synthetic minimal-managed-PE fixtures per research R6. Functions: `build_managed_pe(name: &str, version: Version4Tuple, culture: Option<&str>, file_version: Option<&str>, informational_version: Option<&str>) -> Vec<u8>` produces ~5 KB PE bytes carrying DOS stub + PE header + sections + `IMAGE_COR20_HEADER` + metadata root + `#~` tables stream + `#Strings` heap + Assembly table row 0 + optional CustomAttribute rows. The helper is invoked at test-time from integration tests via the `tests/common/` import pattern (not at build.rs time — keeps fixture byte-emission code out of the production build pipeline per C2 resolution).
- [ ] T028 [P] [US3] Invoke the helper from each integration test in T048..T052 to materialize fixtures into a `tempfile::TempDir`. Per the plan's project structure (sub-dir layout, NOT flat names), emit: `valid_clr_no_culture.dll` (name=Foo.Bar, version=1.2.3.4, culture=null); `valid_clr_with_info_version.dll` (informational_version="1.2.3-rc.1"); `valid_clr_with_file_version.dll` (file_version="1.2.3.5"); the multi-culture set as `valid_clr_with_cultures/Foo.Bar.dll` (neutral) + `valid_clr_with_cultures/de/Foo.Bar.resources.dll` (culture="de") + `valid_clr_with_cultures/fr/Foo.Bar.resources.dll` (culture="fr") — sub-dir layout matches real-world .NET resource-DLL paths; `native_pe_no_clr.dll` (DOS+PE header without COR20 directory).
- [ ] T029 [P] [US3] Add a `corrupt_clr_metadata.dll` fixture: synthetic PE with a valid COR20 header but a truncated `#Strings` heap (so the Assembly table row's Name lookup falls off the end).

### Module skeleton

- [ ] T030 [US3] Create `mikebom-cli/src/scan_fs/binary/dotnet_pe_clr.rs`. Declare the module in `mikebom-cli/src/scan_fs/binary/mod.rs`. Set up the file's preamble: module docstring referencing ECMA-335 §II.22, the 2026-06-18 clarification Q1 on resource-assembly dedup, milestone-129 clarification Q3 on version-ladder.
- [ ] T031 [US3] Implement `ManagedPeAssembly`, `Version4Tuple`, `AssemblyAccumulator`, `AccumulatedAssembly` structs per data-model.md Entity 3. Include the `purl_version()` ladder method.

### CLR header detection

- [ ] T032 [US3] Implement `is_managed_assembly(pe_bytes: &[u8]) -> bool` per FR-018 + research R3: parse via `object::read::pe::PeFile64` (or `PeFile32` per `Magic`); read `nt_headers().optional_header.data_directories[14]`; return `data_directories[14].virtual_address.get(LE) != 0 && data_directories[14].size.get(LE) != 0`. Returns `false` cheaply for native DLLs.

### Metadata-table parser

- [ ] T033 [US3] Implement `read_cor20_header(pe: &PeFile, cor20_rva: u32) -> Option<Cor20Header>` per research R3 skeleton. The `IMAGE_COR20_HEADER` struct fields are documented in ECMA-335 §II.25.3.3.
- [ ] T034 [US3] Implement `read_metadata_root(pe: &PeFile, metadata_rva: u32) -> Option<MetadataRoot>` reading the "BSJB" signature + version + stream-header count + stream-header array, locating the `#~` (or `#-`) tables stream + `#Strings` heap + `#Blob` heap stream offsets.
- [ ] T035 [US3] Implement `parse_table_headers(tables_stream: &[u8]) -> Option<TableHeaders>` reading the table-row counts header per ECMA-335 §II.24.2.6.
- [ ] T036 [US3] Implement `read_assembly_table_row_0(tables_stream: &[u8], headers: &TableHeaders, strings_heap: &[u8]) -> Option<AssemblyRow>` reading the Assembly table (token 0x20) row 0's fields: HashAlgId (u32), MajorVersion (u16), MinorVersion (u16), BuildNumber (u16), RevisionNumber (u16), Flags (u32), PublicKey (#Blob index), Name (#Strings index), Culture (#Strings index). Resolve Name and Culture via the strings heap.
- [ ] T037 [US3] Implement `walk_custom_attributes(tables_stream: &[u8], headers: &TableHeaders, blob_heap: &[u8], strings_heap: &[u8]) -> Vec<(String, String)>` reading the CustomAttribute table (token 0x0C). For each row: resolve the `Type` column through `MemberRef` → `TypeRef` → check the resolved type name against `"AssemblyFileVersionAttribute"` and `"AssemblyInformationalVersionAttribute"`. For matching rows, extract the attribute's string argument from the `Value` column's `#Blob` heap reference (prolog `01 00` + UTF-8 length-prefixed string).

### Reader entrypoint

- [ ] T038 [US3] Implement `read(rootfs: &Path, exclude_set: &ExclusionSet) -> Vec<PackageDbEntry>` per `contracts/reader-behavior.md` Reader 3: walks via `safe_walk` for `*.dll` files; for each, `std::fs::read` the bytes, call `is_managed_assembly()`, gate on `true`; parse the metadata; emit through the `AssemblyAccumulator` (intra-reader culture-set merge per FR-024 + clarification Q1).
- [ ] T039 [US3] Implement the `AssemblyAccumulator::flatten() -> Vec<PackageDbEntry>` method that walks the accumulator's BTreeMap, emitting one entry per `(name, version)` key. For each: build the canonical `pkg:nuget` PURL via `nuget::build_nuget_purl`; populate annotations: `mikebom:sbom-tier = "image"`, `mikebom:source-mechanism = "dotnet-assembly-metadata"`, `mikebom:source-files = <BTreeSet flattened comma-joined>`, `mikebom:assembly-version-informational/file/runtime` (when present per FR-021), `mikebom:assembly-cultures = "<comma-joined sorted>"` (when the cultures set is non-empty per FR-024).
- [ ] T040 [US3] Wire `dotnet_pe_clr::read` into the binary-tier dispatcher at `mikebom-cli/src/scan_fs/binary/mod.rs`. Find the existing `discover_binaries` loop — AFTER it processes each binary, ADD the `dotnet_pe_clr::read` call at the appropriate point (likely in a separate post-loop walk, since `discover_binaries` filters by ELF/Mach-O/PE magic which already includes PE).

### Catalogue + parity

- [ ] T041 [US3] Add 4 new C-rows (C92..C95 — final numbers may shift; grep `docs/reference/sbom-format-mapping.md` for the highest current C-NN to pin) per `contracts/annotation-schema.md`. Each row gets the full Principle V audit narrative.
- [ ] T042 [P] [US3] Register C92..C95 as `cdx_anno!` entries in `mikebom-cli/src/parity/extractors/cdx.rs`. Place immediately after the highest existing C-NN entry. Each is component-scope, SymmetricEqual.
- [ ] T043 [P] [US3] Register C92..C95 as `spdx23_anno!` entries in `mikebom-cli/src/parity/extractors/spdx2.rs`.
- [ ] T044 [P] [US3] Register C92..C95 as `spdx3_anno!` entries in `mikebom-cli/src/parity/extractors/spdx3.rs`.
- [ ] T045 [US3] Register C92..C95 as `ParityExtractor` slice entries in `mikebom-cli/src/parity/extractors/mod.rs` with matching `use` statements. Verify `extractors_table_is_sorted_by_row_id` + `every_catalog_row_has_an_extractor` shape tests pass.

### Unit tests

- [ ] T046 [US3] Add unit tests inside `dotnet_pe_clr.rs` for: `is_managed_assembly()` returns `true` for `valid_clr_no_culture.dll` and `false` for `native_pe_no_clr.dll`; the Assembly table row 0 parser extracts the expected (Name, Version4Tuple, Culture) from `valid_clr_no_culture.dll`; the CustomAttribute walker extracts the expected informational/file version strings from `valid_clr_with_info_version.dll`; the `purl_version()` ladder picks Informational → File → Version in the correct precedence order.
- [ ] T047 [US3] Add unit tests for `AssemblyAccumulator`: a single non-"neutral" culture variant accumulates into the cultures set; the "neutral" culture is skipped; two same-(name, version) DLLs with different cultures collapse to ONE accumulated entry.

### Integration test

- [ ] T048 [US3] Create `mikebom-cli/tests/binary_tier_completion_us3_dotnet_pe_clr.rs`. Test 1: scan a directory containing only `valid_clr_no_culture.dll`. Assert the emitted SBOM contains 1 `pkg:nuget/Foo.Bar@1.2.3.4` component with `mikebom:source-mechanism = "dotnet-assembly-metadata"` and NO `mikebom:assembly-cultures` annotation.
- [ ] T049 [US3] Add test 2: scan a directory with `Foo.Bar.dll` (neutral) + `de/Foo.Bar.resources.dll` (culture=de) + `fr/Foo.Bar.resources.dll` (culture=fr). Assert ONE component emitted with `mikebom:assembly-cultures = "de,fr"` (sorted lex) per the 2026-06-18 clarification Q1.
- [ ] T050 [US3] Add test 3 (cross-reader dedup with milestone 129): scan a directory with both a `.deps.json` declaring `Foo.Bar/1.2.3.4` AND the synthetic `valid_clr_no_culture.dll` (same name + version). Assert ONE component with `mikebom:also-detected-via` containing both `dotnet-deps-json` AND `dotnet-assembly-metadata` source mechanisms (sorted lex).
- [ ] T051 [US3] Add test 4: scan `native_pe_no_clr.dll`. Assert NO `pkg:nuget` component emitted and NO log entry (silent skip per FR-022).
- [ ] T052 [US3] Add test 5: scan `corrupt_clr_metadata.dll`. Assert NO `pkg:nuget` component emitted; assert the scan does NOT fail; assert stderr contains a `warn` line naming the file (FR-023).

### Verification

- [ ] T053 [US3] Run `cargo +stable test -p mikebom --bin mikebom dotnet_pe_clr` and confirm unit tests pass.
- [ ] T054 [US3] Run `cargo +stable test -p mikebom --test binary_tier_completion_us3_dotnet_pe_clr` and confirm all 5 integration test scenarios pass.

**Checkpoint**: US3 fully functional. SC-002 verifiable.

---

## Phase 6: Polish & Cross-Cutting Verification

**Purpose**: End-to-end SC verification, CHANGELOG, pre-PR gate, PR.

- [ ] T055 Run end-to-end SC-001 verification per quickstart Scenario 1. Confirm cargo unique count ≥900 on audit image.
- [ ] T056 Run end-to-end SC-002 verification per quickstart Scenario 3. Confirm nuget component count from `mikebom:source-mechanism = "dotnet-assembly-metadata"` is ≥400 on the audit image (or on `mcr.microsoft.com/dotnet/runtime:8.0-alpine` as a fallback).
- [ ] T057 Run end-to-end SC-004 verification: `/Users/mlieberman/Projects/sbom-comparison/sbom-comparison --format summary /tmp/rp-130.cdx.json ~/Downloads/remediation-planner-syft-image-sbom.json` and assert weighted score ≥4.5.
- [ ] T058 Run SC-005 byte-identity verification: `./scripts/regen-goldens.sh && git status --short mikebom-cli/tests/fixtures/` produces zero `.cdx.json` / `.spdx.json` churn.
- [ ] T059 Run SC-006 performance verification: time the audit-image scan pre vs post; assert wall-clock growth <30%.
- [ ] T060 Update `CHANGELOG.md` `[Unreleased]` section with the milestone-130 entry. Lead with the US1 fix's coverage gain (58 → ~900 cargo packages), then break down by US2/US3 if those land in the same PR; call out the 4 new mikebom:* annotation keys (C92..C95).
- [ ] T061 Run the pre-PR gate: `./scripts/pre-pr.sh` and confirm `>>> all pre-PR checks passed.` Fix any clippy lints surfaced.
- [ ] T062 Commit + push the `130-binary-tier-completion` branch.
- [ ] T063 Open PR via `gh pr create` with the summary referencing the audit image's SC-001/002 deltas + the milestone-130 PR template.
- [ ] T064 Create `mikebom-cli/tests/offline_mode_audit_ecosystem_130.rs` mirroring the milestone-107/108 offline-audit pattern: grep the milestone-130 surface area (`scan_fs/binary/dotnet_pe_clr.rs`, the `package_db/maven.rs` nested-walk additions, and the `scan_fs/binary/mod.rs` cargo-auditable gate-removal site) for `reqwest::`, `std::process::Command`, `tokio::net::` — assert zero occurrences. Per FR-004 verification.

---

## Dependency Graph

```text
Phase 1 (Setup) ───→ Phase 2 (Foundational — empty) ───┬──→ Phase 3 (US1, P1) ──┐
                                                       │                         ├──→ Phase 6 (Polish)
                                                       ├──→ Phase 4 (US2, P2) ──┤
                                                       │                         │
                                                       └──→ Phase 5 (US3, P3) ──┘
```

Phases 3, 4, 5 are **independent**. The milestone-105 dedup pipeline handles interactions emergently;
test fixtures don't overlap.

## Parallel Execution Opportunities

**Within Phase 1**: T002 + T003 in parallel.

**Within Phase 3 (US1)**: T004 + T006 in parallel (different files). T007 + T008 sequential
(same test file). T009/T010/T011 sequential after T004 + T006 complete.

**Within Phase 4 (US2)**: T012 + T013 in parallel. T014–T019 sequential (same file). T020–T024
sequential. T025 + T026 in parallel.

**Within Phase 5 (US3)**: T027 + T028 + T029 in parallel (fixture builders). T030–T040 sequential
(implementation chain). T041 sequential. T042 + T043 + T044 in parallel (three extractor files).
T045 sequential. T046–T052 mostly sequential within test files. T053 + T054 in parallel.

**Across Phases 3/4/5**: after Phase 1+2 checkpoint, all three user stories can be developed and
shipped in any order — including parallel branches that rebase onto a shared milestone-130 root.

## Implementation Strategy

**Recommended cadence: three sequential PRs**.

- **PR 1: US1 only** — 5-LOC code change + ~3 test files. Ships in <1 day. Closes SC-001 (cargo
  coverage) standalone. Maximally derisks the milestone.
- **PR 2: US2 only** — ~400 LOC + ~5 test files. Bounded recursive walker. Ships in ~1-2 days.
  Closes SC-003.
- **PR 3: US3 only** — ~800-1000 LOC + ~10 test files + 4 C-row catalogue entries. The bulk of the
  milestone. Ships in ~3-5 days. Closes SC-002.

Alternatively, bundle all three in one PR if confidence is high after US1 lands locally — the
spec/plan support this. The split is the **safer default** given US3's ECMA-335 hand-roll
complexity.

**MVP scope: US1 alone.** The cargo coverage gain is the largest single SC-shifting delivery in
the milestone. Even if US2 + US3 slip indefinitely, US1 alone justifies the milestone.

**Risk callouts**:

- US1 is genuinely 5 LOC + tests, but the **regression-test fixture** (T006) requires synthesizing
  an ELF with both a `.dep-v0` section AND a path-claim. The `object::write` API supports the ELF
  construction; the path-claim simulation requires either a synthetic apk-database fixture OR an
  in-test injection into the `claimed_paths` set. Either approach is ~50 LOC of test code.
- US3 T032..T037 (ECMA-335 metadata-table parser) is the largest single-task complexity in the
  milestone. Time-box at 2 days for the full table-parser stack; if it slips, the fallback is to
  add `pelite = "0.10"` as a dependency (burning the "zero new Cargo deps" constraint). Decision
  point: document in tasks.md as a comment if the fallback is exercised.
- US3 T040 (wire `dotnet_pe_clr::read` into binary-tier dispatcher) may need adjustment if the
  existing `discover_binaries` loop doesn't have a clean post-loop hook — review `binary/mod.rs`'s
  current structure during implementation.
