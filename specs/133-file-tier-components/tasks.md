---

description: "Task list for milestone 133: file-tier components for unattributed content + trivy-style path/layer properties + opt-in full mode + constitution amendment"
---

# Tasks: File-tier component emission for unattributed content

**Input**: Design documents from `/specs/133-file-tier-components/`
**Prerequisites**: spec.md (loaded), plan.md (loaded), research.md (loaded — FR-022 projection done), data-model.md (loaded), contracts/component-tiers.md (loaded), quickstart.md (loaded)

**Tests**: Per spec, each user story has integration test obligations. Test tasks are part of each story's implementation phase, not a separate `tests/` phase.

**Critical sequencing note** (from `data-model.md §Notes for tasks.md`): **US2 ships BEFORE US1** even though both are P1. US1's FR-011 hybrid dedupe reads `mikebom:component-paths` annotations US2 emits; without US2 already merged, US1's path-coverage dedupe has nothing to subtract → over-emission. The FR-022 projection's 3 276 upper-bound (vs the ~245 post-tightening estimate) demonstrates the gap. US4 (docs) lands LAST so the Constitution amendment + reference doc reflect what actually shipped.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no incomplete-dependency)
- **[Story]**: Which user story (US1, US2, US3, US4)
- Include exact file paths

## Path Conventions

Single-project Rust workspace:
- Production code: `mikebom-cli/src/`
- Integration tests: `mikebom-cli/tests/`
- Spec/plan artifacts: `specs/133-file-tier-components/`
- Documentation: `docs/reference/`
- Constitution: `.specify/memory/constitution.md`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify the milestone-132 pinned audit baseline + the FR-022 projection tool are accessible. Both are needed for SC verification at story-completion time.

- [ ] T001 Verify the pinned audit baseline is locally accessible: `docker images --digests 767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner | grep sha256:4e7b05811ce4885d8a7183819b4e0e209662784fe24b7553ceea3d149e3c719c`. If absent, follow `specs/133-file-tier-components/quickstart.md §Step 0` to refresh ECR creds + re-pull + re-tag as `rp-132-us3:pinned`.
- [ ] T002 Verify the FR-022 projection tool at `/tmp/mb-133-projection/project.sh` + extracted rootfs at `/tmp/mb-133-projection/rootfs/` + known-hash list at `/tmp/mb-133-projection/known-hashes.txt`. If absent, re-run the planning-time extraction per `specs/133-file-tier-components/research.md §Orphan projection §Projection methodology` to re-build all three. T001 + T002 are both prerequisites for the SC-001 verification step in Phase 7.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: None at the code level. Each user story extends existing milestone-001 / -104 / -130 / -131 / -132 surfaces directly; no shared types or shared services need to be built first. Phase 2 is intentionally empty so all four user stories can start in priority order after Phase 1.

**Checkpoint**: T001 + T002 complete → US2 (Phase 3) starts immediately.

---

## Phase 3: User Story 2 — Path coverage + layer-digest for package-tier components (Priority: P1, IMPLEMENTATION MVP — split into 3 PRs)

**Goal** (corrected 2026-06-20 per spec-correction history in spec.md §US2): fix the 3 defects on existing `mikebom:source-files` (PR US2.1), add `mikebom:layer-digest` for image scans (PR US2.2), populate CDX-native `evidence.occurrences[].location` for language-ecosystem readers (PR US2.3).

**Independent Test**: scan the pinned audit image; assert every component's `mikebom:source-files` is (a) JSON-encoded array, (b) no `/private/var/folders/` or similar tempdir prefix, (c) no leading `/`; assert every image-scan component carries `mikebom:layer-digest`; assert every language-ecosystem component has `evidence.occurrences[].location` non-empty (vs the 0 % coverage today).

**Why US2 ships first**: US1's FR-011 hybrid dedupe reads `mikebom:component-paths` to know which paths are package-tier-claimed. The corrected US2 sources path coverage from the CDX-native `evidence.occurrences[].location` field (FR-014) plus the corrected `mikebom:source-files` (FR-012). Both are populated before US1's walker runs.

### PR US2.1 — Bug fixes on `mikebom:source-files`

Targets FR-012 defects (A, B, C). Smallest, most-immediate value.

- [ ] T003 [US2.1] Read `mikebom-cli/src/generate/cyclonedx/builder.rs:754-761` (the `mikebom:source-files` emission site) and `mikebom-cli/src/scan_fs/sbom_path.rs` (path normalization helpers). Identify where the rootfs/tempdir prefix originates — the milestone-130 `scan_fs::docker_image::extract` flow creates a tempdir and the resolver records absolute paths into `ResolutionEvidence.source_file_paths` without stripping the tempdir. Write up the data-flow trace in the commit message so future contributors see the architecture.
- [ ] T004 [US2.1] Thread the rootfs-root `PathBuf` from `scan_cmd::scan` through to the CDX `BuilderConfig` (verify struct location at task start; likely `mikebom-cli/src/generate/cyclonedx/builder.rs`). When `BuilderConfig.rootfs_root.is_some()`, strip the prefix from every value emitted via `mikebom:source-files`. Non-image scans (no rootfs tempdir) leave the field unchanged.
- [ ] T005 [US2.1] Change `mikebom:source-files` emission shape from comma-separated string (current: `"path1, path2"`) to JSON-encoded array (`"[\"path1\",\"path2\"]"`). Defect B fix. Update the parallel emission site in `mikebom-cli/src/generate/cyclonedx/metadata.rs:587-590` to match.
- [ ] T006 [US2.1] Strip leading `/` from every emitted path. Defect C fix. After T004's prefix-stripping, this should be straightforward — `Path::strip_prefix` then `Path::to_string_lossy` won't carry leading `/`.
- [ ] T007 [US2.1] Update SPDX 2.3 emission (`mikebom-cli/src/generate/spdx/document.rs` — verify location) to match the corrected shape; the `MikebomAnnotationCommentV1` envelope carries the JSON array string. Same for SPDX 3 (`spdx/v3_document.rs`).
- [ ] T008 [US2.1] Identify the existing C-row for `mikebom:source-files` in `docs/reference/sbom-format-mapping.md` (grep for `mikebom:source-files`); update the row's "CycloneDX 1.6 location" cell to document the corrected JSON-array shape + rootfs-relative + no-leading-`/` rules.
- [ ] T009 [P] [US2.1] Create `mikebom-cli/tests/source_files_normalization.rs` with three cases: (1) image scan produces no tempdir prefix in `mikebom:source-files` (assert no `/private/var/folders/` or `/tmp/mikebom-image-` substring); (2) value parses as JSON array; (3) no leading `/` on any element.
- [ ] T010 [US2.1] Regenerate the image-scan byte-identity goldens — declared churn per FR-004 / SC-005. The change is intentional and a one-time event; review the diff to confirm it's pure-shape-change (no semantic content loss).
- [ ] T011 [US2.1] Pre-PR gate + open PR. Smallest of the three US2 PRs; ships first.

### PR US2.2 — `mikebom:layer-digest` for image scans (FR-013)

Adds the one genuinely missing parity-bridge.

- [ ] T012 [US2.2] Extend `mikebom::scan_fs::docker_image::ExtractedImage` to expose a path → layer-digest map. The existing extraction tracks per-layer tarball entries; build a HashMap once at extraction completion: every rootfs-relative path mapped to the digest of the last layer that wrote it (later layers shadow earlier ones, per OCI overlay semantics).
- [ ] T013 [US2.2] Thread the path → layer-digest map through to CDX emission (alongside the rootfs-root from PR US2.1's T004). For every component with a `mikebom:source-files` path, look up the matching layer digest and emit `properties[name="mikebom:layer-digest", value=<sha256:...>]`. When the component has multiple source paths and they fall in different layers, emit the digest of the LAST layer (matches overlay precedence).
- [ ] T014 [US2.2] Same emission in SPDX 2.3 + SPDX 3 (annotation envelope shape).
- [ ] T015 [US2.2] Add new C-row to `docs/reference/sbom-format-mapping.md` (next available C-number — verify at task start; should be C88 unless milestone-132 added more after C87): `mikebom:layer-digest`. Inline Principle V audit clause per FR-002.1. Add extractor wiring in `parity/extractors/{cdx,spdx2,spdx3,mod}.rs` per the milestone-132 PR #380 C87 pattern.
- [ ] T016 [P] [US2.2] Create `mikebom-cli/tests/package_tier_layer_digest.rs` with three cases: (1) image scan emits `mikebom:layer-digest` on every component whose source path lives in an OCI layer; (2) non-image scan emits nothing; (3) overlay precedence — when the same path is written by two layers, the LATER layer's digest is emitted.
- [ ] T017 [US2.2] Pre-PR gate + open PR.

### PR US2.3 — Populate `evidence.occurrences[].location` for language-ecosystem readers (FR-014)

The path-coverage closing PR. Largest scope (per-reader work) but no new annotations.

- [ ] T018 [US2.3] Identify the complete list of language-ecosystem package-DB readers. Grep `mikebom-cli/src/scan_fs/package_db/` for `PackageDbEntry::new`-style construction or `PackageDbEntry { ... }` literals. Expected list (verify before coding): `cargo.rs`, `nuget/pe_clr.rs`, `nuget/deps_json.rs`, `maven.rs`, `npm/walk.rs`, `gem.rs`, `pypi/*.rs`, plus `binary/entry.rs` for the cargo-auditable / binary-tier path emission. Record the actual list in the commit message.
- [ ] T019 [P] [US2.3] In each reader file from T018, populate the `occurrences: Vec<FileOccurrence>` vector when constructing the resolved component (today these readers leave `occurrences: vec![]`). Per-reader population details:
  - **cargo Cargo.lock reader**: a single occurrence carrying the Cargo.lock path (relative to rootfs, no leading `/`).
  - **cargo-auditable / binary reader**: occurrence at the binary path.
  - **npm walker**: occurrence at the resolved `package.json` path.
  - **nuget PE/CLR reader**: occurrence at the `.dll` path.
  - **nuget `.deps.json` reader**: occurrence at the `.deps.json` path.
  - **maven walker**: occurrence at the POM / JAR path.
  - **pypi readers**: occurrence at the METADATA / requirements path.
  - **gem reader**: occurrence at the `.gemspec` path.
  Reuse the existing `FileOccurrence` struct shape (location + additionalContext per the milestone-038 SHA pattern). Mark `[P]` because each file is independent.
- [ ] T020 [US2.3] Verify the CDX evidence emission at `mikebom-cli/src/generate/cyclonedx/evidence.rs` handles the newly-populated `occurrences` correctly (it should — the apk path already exercises it). Verify SPDX 2.3 / SPDX 3 evidence emission similarly.
- [ ] T021 [P] [US2.3] Create `mikebom-cli/tests/language_ecosystem_path_coverage.rs` asserting that for every emitted component with a `pkg:cargo|npm|nuget|maven|pypi|gem` PURL on a synthetic fixture image, `evidence.occurrences[]` is non-empty AND `evidence.occurrences[0].location` is a relative, no-leading-`/` path.
- [ ] T022 [US2.3] Regenerate image-scan byte-identity goldens (declared churn per FR-004 / SC-005); review diff for pure additive `evidence.occurrences` entries on language-ecosystem fixture components.
- [ ] T023 [US2.3] SC verification: re-scan the milestone-132 pinned audit baseline; assert `evidence.occurrences` coverage rises from the current 177 / 2 926 (6 %) to ≥95 % for the components in deps.dev-indexed ecosystems. Capture the new percentage in the PR description.
- [ ] T024 [US2.3] Pre-PR gate + open PR.

**Checkpoint**: US2 fully functional across 3 PRs. `mikebom:source-files` fixed; `mikebom:layer-digest` shipped; `evidence.occurrences[]` coverage closed. Ready for US1 to follow.

> **Task renumbering note (2026-06-20)**: Phase 3's US2 expansion from 11 tasks (T003–T013) to 22 tasks (T003–T024) means downstream phases' task IDs (T014–T042) are now out of sequence. Rather than renumber all 28 downstream tasks (high churn, low value), references to "T014" through "T042" in the original ordering are PRESERVED VERBATIM below; cross-references between phases (e.g. "T020 wires file-tier emission into the scan pipeline") still point to the correct task by its original ID. Implementation order: complete Phase 3 (T003-T024), then Phase 4 (still T014-T028 in numbering but executes after T024).

---

## Phase 4: User Story 1 — Orphan file-tier emission (Priority: P1, BEHAVIOR-CHANGE-FLIP)

**Goal**: emit file-tier components for files surviving the FR-005 content-shape allowlist (plus path-prefix exclusion + adjacent-lockfile check) AND failing the FR-011 hybrid dedupe (path coverage from US2's `mikebom:component-paths` OR hash coverage from binary-tier per-file hashes). Default behavior changes — `--file-inventory=orphan` is the new default per Q1 clarification.

**Independent Test**: scan an image with a known unattributed binary (synthetic fixture: a `curl-static` binary at `/usr/local/bin/curl-vendored` not in any package DB and not matching binary-tier symbol fingerprints); assert the emitted SBOM contains a file-tier component for that binary with the correct SHA-256, `mikebom:component-tier = "file"` annotation, no PURL, and the package-tier component count is unchanged from the pre-US1 scan. Then re-run the FR-022 projection — expect 180-440 orphan components per SC-001.

### Implementation

- [ ] T014 [US1] Create the new directory `mikebom-cli/src/scan_fs/file_tier/` with `mod.rs` (entry point), `content_shape.rs`, `dedupe.rs`, `walker.rs`. Wire `mod.rs` into the parent `scan_fs/mod.rs` via a `mod file_tier;` declaration. Each file starts with a top-doc-comment citing the milestone-133 spec FR-numbers it implements.
- [ ] T015 [P] [US1] In `mikebom-cli/src/scan_fs/file_tier/content_shape.rs`, implement `ContentShape` enum per `data-model.md §Entity: ContentShape classifier` (variants: ElfBinary, PeBinary, MachoBinary, SharedLib, JavaOrArchive, OsPackage, CompressedArchive, LoneManifest, ExecScript). Implement `pub(crate) fn classify(rel: &Path, abs: &Path, root: &Path, exclusions: &globset::GlobSet) -> Option<ContentShape>` applying: (a) magic-number probe (first 4 bytes) for ELF/PE/Mach-O; (b) extension+magic for shared libs / archives / OS packages / compressed archives; (c) lone-manifest check WITH adjacent-lockfile rule per FR-005 second bullet; (d) executable-script check (#! magic + exec bit). Apply the FR-005 EXCLUSION list (source code / docs / configs) before any classification. Apply the path-prefix exclusion `globset::GlobSet` (built once per scan from `ORPHAN_PATH_EXCLUSIONS` const slice in `data-model.md`).
- [ ] T016 [P] [US1] In `mikebom-cli/src/scan_fs/file_tier/dedupe.rs`, implement `DedupeIndex` per `data-model.md §Entity: Hybrid dedupe set`. `build(components: &[ResolvedComponent]) -> Self` walks every component, collects (a) `mikebom:component-paths` claimed paths from US2's properties into `claimed_paths: HashSet<PathBuf>`, (b) SHA-256 hashes from binary-tier components' `hashes[]` into `claimed_hashes: HashSet<String>`. `is_covered(rel_path: &Path, hash: &str) -> bool` returns true when EITHER set contains the value. Path normalization: strip leading `/` to match the no-leading-`/` convention from FR-007.
- [ ] T017 [US1] In `mikebom-cli/src/scan_fs/file_tier/walker.rs`, implement the rootfs walker using `scan_fs::walk::safe_walk` (NOT `walkdir` per milestone-114). For each file: (a) skip if size exceeds `--file-inventory-size-limit` (default 100 MB per FR-010); skip-counter increments; (b) skip if special file (device, socket, sparse, FIFO); skip-counter increments; (c) skip if unreadable; skip-counter increments + log warn per Principle X; (d) call `ContentShape::classify`; if `None`, skip; (e) compute SHA-256 via streaming hash with 8 KB chunk buffer; (f) call `DedupeIndex::is_covered` (in orphan mode); skip if covered; (g) build `FileTierEntry`.
- [ ] T018 [US1] In `mikebom-cli/src/scan_fs/file_tier/mod.rs`, implement the entry point that drives the walker, accumulates per-SHA-256 `HashMap<String, FileTierEntry>`, sorts paths within each entry per FR-007, and converts to `Vec<ResolvedComponent>` via the `FileTierEntry → ResolvedComponent` mapping documented in `data-model.md`. Emit `mikebom:component-tier = "file"` annotation on every emitted component. Emit `mikebom:file-paths` property carrying JSON-encoded sorted array. Set `name = basename(paths[0])`. No PURL (FR-009). SHA-256 in `hashes[]`. Cap paths at 100 entries with `mikebom:file-paths-truncated = true` flag per edge case bullet.
- [ ] T019 [US1] Extend the SBOM emission paths to handle the new file-tier components: (a) CDX `cyclonedx/components.rs` already routes by `type` so `type = "file"` flows through; verify and add any missing path; (b) SPDX 2.3 `spdx/document.rs` emits as `Package` with `filesAnalyzed: false` + the component-tier annotation per `data-model.md §Entity: FileTierComponent §Emission targets`; (c) SPDX 3 `spdx/v3_document.rs` emits as `software_File` element (FR-001 + research.md §SPDX 3 element type decision).
- [ ] T020 [US1] Wire file-tier emission into the scan pipeline at the appropriate point in `mikebom-cli/src/cli/scan_cmd.rs` AFTER all package-tier and binary-tier readers complete AND AFTER any enrichment passes (deps.dev, ClearlyDefined). The dedupe index reads from the already-resolved component vector so it MUST run last. Gate on `args.file_inventory != FileInventoryMode::Off`. Build the `globset::GlobSet` from `ORPHAN_PATH_EXCLUSIONS` once at this site and pass to the walker.
- [ ] T021 [US1] Emit document-level skip-counter annotations per Principle X when any skip-counter is >0: `mikebom:file-inventory-skipped-oversize`, `mikebom:file-inventory-skipped-special-files`, `mikebom:file-inventory-unreadable`. Plus the `mikebom:file-paths-truncated` per-component flag (set inside T018 already).
- [ ] T022 [US1] Add new C-rows to `docs/reference/sbom-format-mapping.md`: C91 `mikebom:component-tier`, C92 `mikebom:file-paths`, C93 `mikebom:file-paths-truncated`, C94 `mikebom:file-inventory-skipped-oversize`, C95 `mikebom:file-inventory-skipped-special-files`, C96 `mikebom:file-inventory-unreadable`. Each row carries the Principle V audit clause inline. Wire extractors into `parity/extractors/{cdx,spdx2,spdx3,mod}.rs` per the T010 pattern.

### Tests for US1

- [ ] T023 [P] [US1] Create `mikebom-cli/tests/file_tier_orphan.rs` with five cases: (1) unattributed `curl-static` binary at `/opt/custom-tool` emits file-tier component with correct SHA-256, no PURL, `mikebom:component-tier = "file"` annotation; (2) `/usr/bin/curl` covered by an apk package emits NO file-tier component (FR-011 path coverage); (3) `/app/src/main.rs` source file emits NO file-tier component (FR-005 exclusion); (4) same content at two paths emits ONE component with both paths in `mikebom:file-paths` sorted array (FR-006 + FR-007); (5) lone `Cargo.toml` (no `Cargo.lock` in parent chain) emits a file-tier component; same `Cargo.toml` with `Cargo.lock` in parent emits NONE (FR-005 adjacent-lockfile check).
- [ ] T024 [P] [US1] Add a synthetic fixture rootfs tree under `mikebom-cli/tests/fixtures/file_tier_orphan/` containing: a 2-byte ELF magic file, a Cargo.toml WITH Cargo.lock alongside (NO orphan emit), a Cargo.toml WITHOUT Cargo.lock (orphan emit), a `dotnet/packs/Foo/1.0/Foo.dll` PE file (excluded by path-prefix list), and a source file `app/src/main.rs` (excluded by content-shape). T023 references this fixture.
- [ ] T025 [US1] Run `cargo +stable test --workspace --test file_tier_orphan`; all 5 cases pass.
- [ ] T026 [US1] Re-run the FR-022 projection from `/tmp/mb-133-projection/project.sh` AFTER updating it to apply the milestone-133 FR-005 path-prefix exclusion list (the projection's current script approximates this via `is_package_dir`; tighten to use the actual exclusion list). Result MUST land in 180-440 band per SC-001. Capture the exact count in the PR description's measured-vs-target table.
- [ ] T027 [US1] Production-mode SC verification: build release binary, scan the pinned audit image with `--file-inventory=orphan` (the new default; can also be omitted), count file-tier components via `jq '[.components[] | select((.properties // [])[]? | select(.name == "mikebom:component-tier" and .value == "file"))] | length'`. Result MUST match T026's projection within ±10 % per the FR-022 SC-001 contract.
- [ ] T028 [US1] SC-002 zero-duplicate gate verification: per quickstart §Step 1c, extract file-tier hash set + package/binary-tier hash set; assert `comm -12` returns 0 entries.

**Checkpoint**: US1 fully functional + SC-001 + SC-002 met on the pinned baseline. Mostly-ready PR; can land here as the second behavior-change PR after US2.

---

## Phase 5: User Story 3 — Opt-in full file inventory mode (Priority: P2)

**Goal**: `--file-inventory=full` flag emits per-unique-hash file-tier component for every file passing the content-shape allowlist (no path-prefix exclusion; no hybrid dedupe). Document-level `mikebom:file-inventory-mode = "full"` annotation. Targets sbom-comparison Completeness ≥4★ (SC-003).

**Independent Test**: scan the audit image twice — once default (orphan), once `--file-inventory=full`. Assert (a) full-count ≥ 10× orphan-count (SC-007); (b) sbom-comparison `licenses.starsA` reports `completeness.starsA ≥ 4` (SC-003); (c) full SBOM carries document-level `mikebom:file-inventory-mode = "full"` annotation.

### Implementation

- [ ] T029 [US3] Add `FileInventoryMode` enum per `data-model.md §Entity: FileInventoryMode` to `mikebom-cli/src/cli/scan_cmd.rs`. Use clap's `ValueEnum` derive. Default value `Orphan`. Add `--file-inventory <off|orphan|full>` flag + `--file-inventory-size-limit <bytes>` (default 100 * 1024 * 1024) to `ScanArgs`.
- [ ] T030 [US3] Wire `args.file_inventory` through to the file-tier emission site from T020. `Off` → skip entirely. `Orphan` → existing US1 path. `Full` → skip the `DedupeIndex::is_covered` check (still apply content-shape allowlist; still apply size limit; still build per-unique-hash dedup at the entry-accumulation layer). Path-prefix exclusion list is also SKIPPED in full mode (the whole point is per-file completeness; not even `dotnet/packs/` files are excluded).
- [ ] T031 [US3] Emit document-level `mikebom:file-inventory-mode` annotation: `"off"` when explicitly off, `"full"` when full, ABSENT for default `orphan` (absence = default mode). Wire into `cyclonedx/metadata.rs` + `spdx/document.rs` + `spdx/v3_document.rs`.
- [ ] T032 [US3] Add new C-row C97 `mikebom:file-inventory-mode` to `docs/reference/sbom-format-mapping.md`. Document-level annotation row (not per-component). Add extractor.

### Tests for US3

- [ ] T033 [P] [US3] Create `mikebom-cli/tests/file_tier_full.rs` with three cases: (1) `--file-inventory=full` scan emits document-level annotation; default scan does not; (2) full-mode emits a file-tier component for `/etc/ssl/openssl.cnf` even though apk's `openssl` package covers it (proves the Strict Boundary §5 override); (3) full-mode component count is at least 10× orphan-mode count on the synthetic fixture from T024.
- [ ] T034 [US3] Run `cargo +stable test --workspace --test file_tier_full`; all 3 cases pass.
- [ ] T035 [US3] SC-003 verification per quickstart §Step 2: build release, scan pinned image with `--file-inventory=full`, run sbom-comparison against the existing syft baseline at `/tmp/syft-rp-132-baseline.cdx.json`, assert `.completeness.starsA >= 4`.
- [ ] T036 [US3] SC-007 verification per quickstart §Step 6: orphan-count vs full-count ratio ≥ 10. Use the cached `/tmp/mb-rp-133-orphan.cdx.json` from T027 + the new `/tmp/mb-rp-133-full.cdx.json` from T035.

**Checkpoint**: US3 fully functional + SC-003 + SC-007 met. Can land as the third PR after US1.

---

## Phase 6: User Story 4 — Constitution amendment + reference doc (Priority: P3)

**Goal**: durability layer for future contributors. Constitution amendment codifies the design space; reference doc lives at `docs/reference/component-tiers.md` and is cited from PR reviews going forward.

**Independent Test**: a new contributor reading `docs/reference/component-tiers.md` plus the amended Constitution can correctly answer "given file X at path Y, would it emit in default mode? what about full mode? what tier?" without reading any source code.

### Implementation

- [ ] T037 [P] [US4] Amend `.specify/memory/constitution.md` per FR-019: (a) NEW Strict Boundary §5 with the exact text from `spec.md §FR-019 (a)`; (b) §VIII Completeness clarification per `spec.md §FR-019 (b)`; (c) MINOR version bump 1.4.0 → 1.5.0 (the doc-head version line + the bottom `**Version**` line); (d) add a SYNC IMPACT REPORT block at the very top of the file matching the existing 1.3.1 → 1.4.0 example's format. Cite milestone-133 PR # (when known) + the milestone-132 audit-baseline measurement that surfaced the gap.
- [ ] T038 [P] [US4] Copy `specs/133-file-tier-components/contracts/component-tiers.md` to `docs/reference/component-tiers.md`. Strip the contract preamble ("Driven by", "Target file", etc.) so what lands is just the user-facing reference. Cross-link from the existing `docs/reference/sbom-format-mapping.md` introduction (one-sentence pointer "see component-tiers.md for the tier model").
- [ ] T039 [US4] Verify per `spec.md §SC-006`: (a) `grep -c '^**Version**: 1.5.0' .specify/memory/constitution.md` returns 1; (b) `grep -c 'Strict Boundary 5' .specify/memory/constitution.md` (or equivalent §5 anchor) returns at least 1; (c) `test -f docs/reference/component-tiers.md`; (d) `docs/reference/component-tiers.md` contains at least one worked example per tier per format (package-tier × CDX, package-tier × SPDX 2.3, package-tier × SPDX 3, file-tier × CDX, file-tier × SPDX 2.3, file-tier × SPDX 3, binary-tier × CDX, binary-tier × SPDX 2.3, binary-tier × SPDX 3 — 9 examples minimum). Add examples if missing.

**Checkpoint**: US4 docs landed. The reference doc is the source of truth for future PR reviews touching file-tier emission. Can land in parallel with US1 / US2 / US3 (no code dependencies) — or batch into the same PR for review efficiency.

---

## Phase 7: Polish & Cross-Cutting

**Purpose**: pre-PR gate, full SC verification across all stories, CHANGELOG.

- [ ] T040 Run `./scripts/pre-pr.sh` for the entire merge train. Per CLAUDE.md, both `cargo +stable clippy --workspace --all-targets` (zero errors) AND `cargo +stable test --workspace` (every per-target `N passed; 0 failed`) MUST pass. Paste per-target `N passed; 0 failed` lines verbatim into the PR description — don't grep.
- [ ] T040.5 SC-004 hyperfine verification: build the release binary (`cargo build --release --bin mikebom`); run `hyperfine --warmup 1 --runs 3 --export-json /tmp/mb-rp-133-timing.json "./target/release/mikebom sbom scan --image rp-132-us3:pinned --offline --output /tmp/discard.cdx.json --root-name foo --file-inventory=off" "./target/release/mikebom sbom scan --image rp-132-us3:pinned --offline --output /tmp/discard.cdx.json --root-name foo --file-inventory=orphan" "./target/release/mikebom sbom scan --image rp-132-us3:pinned --offline --output /tmp/discard.cdx.json --root-name foo --file-inventory=full"`; compute `orphan_median / off_median` (assert <1.50 per SC-004 orphan budget) and `full_median / off_median` (assert <4.00 per SC-004 full budget); record both ratios in the PR description's measured-vs-target table.
- [ ] T041 Run the full `specs/133-file-tier-components/quickstart.md` end-to-end against the pinned baseline. Populate the measured-vs-target table per quickstart §Step 8. Every SC (001 - 007) MUST be marked MET or have an explicit honest-accounting deferral.
- [ ] T042 [P] Append `[Unreleased]` CHANGELOG entries to `/Users/mlieberman/Projects/mikebom/CHANGELOG.md` — one entry per user-story PR (US2 first, US1 second, US3 third, US4 fourth). Each entry cites: the SC outcomes from T041's verification, the pinned digest for SC traceability, and the FR-022 measured projection count + post-tightening figure for US1's CHANGELOG. The US1 entry MUST explicitly call out the default behavior flip (`--file-inventory` from off to orphan) per Q1 clarification — consumers reading the release notes MUST see this.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: T001 + T002 — verifies the pinned baseline + projection tool are reachable. Both must complete before any SC-claim PR opens. ~5-10 minutes total.
- **Phase 2 (Foundational)**: empty.
- **Phase 3 (US2)**: starts after Phase 1. Independent of US3, US4.
- **Phase 4 (US1)**: **starts after US2 merges** (the FR-011 hybrid dedupe reads US2's `mikebom:component-paths`). Independent of US3, US4 once US2 is in.
- **Phase 5 (US3)**: starts after US1 merges (full mode reuses US1's `file_tier/` infrastructure). Independent of US4.
- **Phase 6 (US4)**: pure documentation — starts after Phase 1. **Independent of US1, US2, US3** at the file-level. Can land as its own PR at any time after Phase 1, OR batch into the US3 PR for review efficiency.
- **Phase 7 (Polish)**: depends on US1 + US2 + US3 + US4 complete.

### User Story Dependencies

- **US2 (P1)**: depends only on Phase 1.
- **US1 (P1)**: depends on US2 being already-merged. See "Critical sequencing note" at top.
- **US3 (P2)**: depends on US1 being already-merged.
- **US4 (P3)**: independent of all code work. Lands anytime after Phase 1.

### Within Each User Story

- **US2**: T003 → T004 → T005 (parallelizable per reader) → T006/T007/T008 (parallelizable across formats) → T009 → T010 → T011 → T012 → T013.
- **US1**: T014 → T015/T016 (parallel) → T017 → T018 → T019 → T020 → T021 → T022 → T023/T024 (parallel) → T025 → T026 → T027 → T028.
- **US3**: T029 → T030 → T031 → T032 → T033 → T034 → T035 → T036.
- **US4**: T037/T038 (parallel — different files) → T039.

### Parallel Opportunities

- After Phase 1: US2 + US4 can launch in parallel by 2 developers.
- After US2 merges: US1 + US4 (if not done yet) can run in parallel.
- After US1 merges: US3 + any leftover US4 in parallel.
- Within US2: T005 per-reader edits all parallelizable. T006 + T007 + T008 all parallelizable.
- Within US1: T015 + T016 parallel. T023 + T024 parallel.
- Within US4: T037 + T038 parallel.
- T042 CHANGELOG entries can be batched into the same final PR or split per-PR.

---

## Parallel Example: US2 + US4 launched together after Phase 1

```bash
# Two developers, two parallel paths:
Task: "T003 Extend PackageDbEntry struct"
Task: "T037 Amend constitution.md with Strict Boundary §5"
Task: "T038 Copy contracts/component-tiers.md to docs/reference/"
```

---

## Implementation Strategy

### MVP First (US2)

**Suggested MVP** (overriding the spec's "US1 is MVP" wording per the data-model.md note + this tasks.md "Critical sequencing"): land US2 ALONE as the MVP PR. Smallest behavioral change (pure additive properties on package-tier components); no scorecard movement on its own; closes no SC; but it's the prerequisite for US1's SC-001 / SC-002 to make sense.

Sequence:

1. T001 + T002 (Setup).
2. T003 → T013 (US2 implementation + tests + goldens).
3. **STOP and VALIDATE**: every package-tier component carries `mikebom:component-path`; image scans also carry `mikebom:layer-digest`; existing goldens regenerated with the new properties.
4. T040 (pre-PR gate) → T042 partial (CHANGELOG US2 entry).
5. Open MVP PR; land.
6. Sync main; start US1.

### Incremental Delivery

After US2 lands, next increments in priority order:

- **US1 PR**: orphan emission + default-flip behavior change. SC-001 + SC-002 + SC-005 met. **CHANGELOG callout for default flip MANDATORY** per Q1 clarification.
- **US3 PR**: full mode + Completeness 4★ lift. SC-003 + SC-007 met.
- **US4 PR**: Constitution amendment + reference doc. SC-006 met.

Each lands as its own PR with its own measured-vs-target table per quickstart §Step 8.

### Parallel Team Strategy

With 4 developers post-Phase 1:

- Dev A: US2 (T003 → T013)
- Dev B: US4 docs prep (T037, T038)
- Dev C: US1 design review + start scaffolding T014 (waits to commit until US2 merges)
- Dev D: SC-006 verification harness prep (T039) + sbom-comparison test corpus

---

## Notes

- [P] = different files, no dependencies on incomplete tasks.
- [Story] label maps every user-story task back to spec.md user stories US1..US4.
- Each user story is independently completable; SC-001..SC-007 verification is end-to-end in T041.
- Pre-PR gate (T040) is MANDATORY per CLAUDE.md; cite per-target `N passed; 0 failed` lines verbatim, don't grep.
- The FR-022 projection tool re-run (T026) is the SC-001 verification gate. It MUST land in the 180-440 band post-tightening. If it doesn't, the FR-005 path-prefix exclusion list needs tightening AT THAT POINT (in-PR plan-correction per the milestone-132 pattern), not after merge.
- Commit after each task or logical group; "Commits should be small enough to bisect through" per project convention.
- Avoid: vague tasks, same-file conflicts not marked sequential, declaring SCs MET without re-measuring against the pinned digest (the exact pattern milestones 131 / 132 trained us to prevent).
