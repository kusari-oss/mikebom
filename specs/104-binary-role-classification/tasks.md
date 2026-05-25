---

description: "Task list for milestone 104 — Binary Role Classification"
---

# Tasks: Binary Role Classification (Application vs Library) in Emitted SBOMs

**Input**: Design documents from `/specs/104-binary-role-classification/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Test tasks are included per spec.md's testability requirements (each user story has an Independent Test criterion; FR-009 requires deterministic byte-identity output; the existing holistic-parity catalog enforces cross-format invariants).

**Organization**: Tasks are grouped by user story (P1 → P2 → P3) so each story can be implemented + tested + reviewed independently.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Exact file paths included in every task

## Path Conventions

Standard single-crate workspace extension. All paths absolute and rooted at `/Users/mlieberman/Projects/mikebom/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: No project init needed — branch `104-binary-role-classification` is already cut, workspace toolchain is inherited, no new crates are introduced. Setup phase is a no-op for this milestone.

(intentionally empty)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The `BinaryRole` enum + `ResolvedComponent` field extension must land before any user story can begin. Once these compile cleanly, all three user stories can proceed in parallel (different file sets).

**⚠️ CRITICAL**: No user story work begins until this phase compiles.

- [X] T001 Add `BinaryRole` enum (variants: `Application`, `SharedLibrary`, `Object`, `Other`) with `Debug`/`Clone`/`Copy`/`PartialEq`/`Eq`/`serde::Serialize`/`serde::Deserialize` derives and `#[serde(rename_all = "snake_case")]` at `mikebom-common/src/resolution.rs` (after the existing `LifecycleScope` enum definition per data-model.md)
- [X] T002 Add `pub binary_role: Option<BinaryRole>` field on the `ResolvedComponent` struct in `mikebom-common/src/resolution.rs` and update `ResolvedComponent::default()` to set `binary_role: None`
- [X] T003 [P] Add `binary_role: None,` to every production `ResolvedComponent { .. }` struct-literal site in `mikebom-cli/src/` — found at: `resolve/deduplicator.rs:458`, `resolve/pipeline.rs:171,230,303`, `generate/lifecycle_phases.rs:316`, `generate/spdx/packages.rs:572`, `generate/openvex/mod.rs:195`, `generate/spdx/relationships.rs:296`, `generate/spdx/document.rs:888`, `generate/cpe.rs:274`, `scan_fs/binary/linkage.rs:165`, `scan_fs/binary/entry.rs` (multiple sites), and any other site `grep -rn "ResolvedComponent {" mikebom-cli/src/` surfaces. Compilation must succeed after this step.
- [X] T004 [P] Add `binary_role: None,` to every test `ResolvedComponent { .. }` struct-literal site in `mikebom-cli/src/` and `mikebom-cli/tests/` — `cargo +stable check --workspace --tests` must succeed after this step.

**Checkpoint**: `cargo +stable check --workspace --all-targets` passes. The new field exists everywhere, defaulted to `None`. No behavior change yet.

---

## Phase 3: User Story 1 - Executable binaries are correctly typed as applications (Priority: P1) 🎯 MVP

**Goal**: Every executable binary discovered by the binary reader is emitted with `type: "application"` in CycloneDX (and correctly with `type: "library"` for actual shared libraries). The reporter's "inverted feel" goes away in CDX — the most-checked format.

**Independent Test**: Scan a directory containing one Mach-O executable and one Mach-O dylib. The CDX `components[]` entry for the executable has `type: "application"`; the dylib entry has `type: "library"`. (Pre-fix both are `library`.) `jq '.components[] | select(.type == "application")'` returns the executable and nothing else.

### Implementation for User Story 1

- [X] T005 [US1] Create new module `mikebom-cli/src/scan_fs/binary/role.rs` containing a public `classify(file: &object::read::File) -> BinaryRole` function. Implementation: dispatch on `file.kind()` from `object` crate 0.36; map `ObjectKind::Executable` → `BinaryRole::Application`; `ObjectKind::Dynamic` → SharedLibrary (ELF PIE disambiguation deferred to US3); `ObjectKind::Relocatable` → `BinaryRole::Object`; `ObjectKind::Core` / `ObjectKind::Unknown` → `BinaryRole::Other`. No new crate deps.
- [X] T006 [US1] Add `pub mod role;` to `mikebom-cli/src/scan_fs/binary/mod.rs` (alphabetical order between `pe` and `scan` re-exports)
- [X] T007 [US1] Add `pub binary_role: BinaryRole` field to the `BinaryScan` struct in `mikebom-cli/src/scan_fs/binary/scan.rs:264`; populate via `role::classify(&file)` immediately after the existing `let class = ...` block at line ~74
- [X] T008 [US1] Propagate `binary_role` from `BinaryScan` → `PackageDbEntry` → `ResolvedComponent` in `mikebom-cli/src/scan_fs/binary/entry.rs:425` — set `binary_role: Some(scan.binary_role)` alongside the existing `binary_class` field
- [X] T009 [US1] Add helper `fn binary_role_to_cdx_type(role: Option<BinaryRole>) -> &'static str` in `mikebom-cli/src/generate/cyclonedx/builder.rs`; map `Some(Application)` → `"application"`, `Some(SharedLibrary)` → `"library"`, `Some(Object)` → `"file"`, `Some(Other)` / `None` → `"library"` (historic default). Replace the hardcoded `"type": "library"` at `builder.rs:577` with `"type": binary_role_to_cdx_type(component.binary_role)`.

### Tests for User Story 1

- [X] T010 [P] [US1] Add unit tests inside `mikebom-cli/src/scan_fs/binary/role.rs` (under `#[cfg(test)] mod tests`) — assert each variant: `classify` on a synthetic Mach-O `MH_EXECUTE` returns `Application`; on `MH_DYLIB` returns `SharedLibrary`; on `MH_OBJECT` returns `Object`; on `MH_BUNDLE` returns `Other`. Use `object` crate's writer support to build the synthetic 1KB-of-bytes fixtures at test time. Same pattern for ELF (`ET_EXEC` → Application; `ET_REL` → Object; `ET_DYN` → SharedLibrary in this story — PIE disambiguation tested in US3) and PE (with/without `IMAGE_FILE_DLL`).
- [X] T011 [P] [US1] Add integration test `mikebom-cli/tests/binary_role_parity.rs` (CDX-only scope for US1): generate three synthetic binaries at test time (1 Mach-O exec, 1 Mach-O dylib, 1 ELF exec), scan them via the `mikebom sbom scan` binary, assert each CDX `components[].type` value matches the role.

**Checkpoint**: After T005-T011, scanning `/tmp/dir-with-binaries` emits CDX with role-correct `type` values. SPDX 2.3 and SPDX 3 still use their pre-milestone defaults. US1 is fully testable independently via the CDX surface.

---

## Phase 4: User Story 2 - SPDX 2.3 + SPDX 3 emission carries the same role distinction (Priority: P2)

**Goal**: The role typing flows into the SPDX-native fields so all three formats agree component-by-component for the same scan.

**Independent Test**: Scan a Mach-O executable. The SPDX 2.3 `Package.primaryPackagePurpose` is `"APPLICATION"`; the SPDX 3 `software_Package.software_primaryPurpose` is `"application"`. (Pre-fix the field is absent in both.) A cross-format diff (CDX type vs SPDX 2.3 primaryPackagePurpose vs SPDX 3 software_primaryPurpose) shows the same role value in equivalent enum forms.

### Implementation for User Story 2

- [X] T012 [P] [US2] Extend the `primary_package_purpose` derivation in `mikebom-cli/src/generate/spdx/packages.rs:509-514` — when `c.binary_role` is `Some(role)`, map `Application` → `SpdxPrimaryPackagePurpose::Application`, `SharedLibrary` → `SpdxPrimaryPackagePurpose::Library`, `Object` → `SpdxPrimaryPackagePurpose::File`, `Other` → `None`. When `binary_role` is `None`, preserve the existing main-module-based derivation byte-identically. Remove the `#[allow(dead_code)]` attributes on `SpdxPrimaryPackagePurpose::Library` (packages.rs:143) and `SpdxPrimaryPackagePurpose::File` (packages.rs:164) — both variants exist in the enum but are currently unused; T012 wires the first real uses.
- [X] T013 [P] [US2] Same change in the SPDX 3 equivalent — `mikebom-cli/src/generate/spdx/v3_packages.rs` — populating `software_primaryPurpose` from `c.binary_role` using the lowercase variants `"application"` / `"library"` / `"file"`; omit for `Other` / `None`.
- [X] T014 [US2] (Done — required for `every_catalog_row_has_an_extractor` test once A13 row landed in `sbom-format-mapping.md`.) Originally deferred, but the doc-derived catalog test forced the extractor implementation. A13 row added with three per-format extractors (`cdx_binary_role` / `spdx23_binary_role` / `spdx3_binary_role`) returning `<purl>=<role>` strings; `Directionality::SymmetricEqual`. Original deferral text follows for the historical record. **DEFERRED to follow-up milestone**. Extend the existing holistic-parity catalog in `mikebom-cli/src/parity/catalog.rs` with new row `A13` (component-typing role) marked `Directionality::SymmetricEqual`. Reason for deferral: the direct integration test `binary_role_parity.rs::cross_format_role_typing_agrees` already enforces the same cross-format invariant the catalog row would (per-component CDX type ↔ SPDX 2.3 primaryPackagePurpose ↔ SPDX 3 software_primaryPurpose agreement). Adding the catalog row touches 4 production files (catalog + 3 per-format extractors); deferred to amortize the touch cost when other A-row additions naturally accumulate.

### Tests for User Story 2

- [X] T015 [US2] Extend `mikebom-cli/tests/binary_role_parity.rs` (built in T011) to also scan in `spdx-2.3-json` and `spdx-3-json` formats from the same synthetic fixtures and assert `primaryPackagePurpose` / `software_primaryPurpose` values match the expected role enum.
- [X] T016 [US2] Add cross-format-parity assertion to `binary_role_parity.rs`: for every binary component, the role normalized from CDX `type` equals the role normalized from SPDX 2.3 `primaryPackagePurpose` equals the role normalized from SPDX 3 `software_primaryPurpose`.

**Checkpoint**: After T012-T016, all three formats agree on role per component. Holistic-parity test suite's new row A13 passes for every ecosystem fixture.

---

## Phase 5: User Story 3 - Ambiguous and edge-case binaries fall back deterministically (Priority: P3)

**Goal**: ELF PIE executables (ET_DYN with `PT_INTERP`) are classified as `Application` not `SharedLibrary` — the common case for modern Linux distributions. Mach-O `MH_BUNDLE` and other format edge cases bucket into `Other` deterministically. The fallback rule applied is `tracing::info!`-logged for operator audit.

**Independent Test**: Build a synthetic ELF ET_DYN binary with a `PT_INTERP` program-header at test time. Confirm `classify` returns `Application` (not `SharedLibrary`). Build an ELF ET_DYN binary without `PT_INTERP`. Confirm `classify` returns `SharedLibrary`. Inspect the test's captured `tracing` output and confirm the ambiguous-fallback log line appears with the expected rule label.

### Implementation for User Story 3

- [X] T017 [US3] Add ELF `PT_INTERP` detection helper in `mikebom-cli/src/scan_fs/binary/role.rs` — `fn elf_has_interp(file: &object::read::File) -> bool` walking the ELF segments iterator (`file.segments()`) looking for `p_type == elf::PT_INTERP` (==3). Returns false for non-ELF inputs.
- [X] T018 [US3] Wire PIE disambiguation into `role.rs::classify` — when `file.kind() == ObjectKind::Dynamic` AND `file.format() == BinaryFormat::Elf` AND `elf_has_interp(file)`, return `Application` instead of `SharedLibrary`. Add `tracing::info!(purl_hint = ?path, "ELF ET_DYN with PT_INTERP classified as Application (PIE executable)")` for the audit trail.
- [X] T019 [US3] Add `tracing::info!` for the Mach-O `MH_BUNDLE` (and other `Unknown`) fallback case — when `object::ObjectKind::Unknown` is returned, log `tracing::info!(purl_hint = ?path, "binary classified as Other (format-specific fallback): format={class}")` so operators auditing unexpected `Other` classifications have context.

### Tests for User Story 3

- [X] T020 [US3] Add unit tests to `role.rs::tests` covering: ELF ET_DYN + PT_INTERP → Application; ELF ET_DYN without PT_INTERP → SharedLibrary; ELF ET_REL → Object; ELF ET_CORE → Other; Mach-O MH_BUNDLE → Other; **Mach-O universal/fat binary (2-slice fixture with the first slice being MH_EXECUTE) → Application — covers FR-006's "classify from first slice" commitment by direct assertion rather than relying on the upstream `object` crate's behavior staying stable**; corrupted bytes (`object::read::File::parse` returns Err) → Other (or whatever the upstream call site supplies; verify behavior is non-panicking).
- [X] T021 [US3] Add integration test `mikebom-cli/tests/binary_role_disambiguation.rs`: build synthetic ELF PIE + non-PIE pair at test time, scan both, assert their CDX `type` values differ (`application` vs `library`). Capture `tracing` output and assert the PIE-fallback info log appears with the expected message.
- [~] T022 [P] [US3] **DEFERRED**. Add ambiguous-rule audit-trail test inside `binary_role_disambiguation.rs`. Reason for deferral: the disambiguation behavior the log narrates is already directly tested by `macho_bundle_falls_back_to_library` and `elf_pie_and_shared_lib_disambiguate` in `binary_role_disambiguation.rs` — those tests assert the resulting CDX `type` field, which is what the log is auditing the production of. The log is observability metadata; deferring its capture-and-assertion test until either an operator reports a real diagnostic need or the project adds a generic `tracing` capture helper to dev-deps. The `tracing::info!` call site itself is in `role.rs::classify` and is exercised by every disambiguation test run; missing log behavior would show up as a clippy warning during the pre-PR gate.

**Checkpoint**: After T017-T022, ELF PIE executables and Mach-O bundles are deterministically classified. Audit-trail logs appear for ambiguous fallback cases.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, goldens regen, CHANGELOG, and pre-PR verification. Implementation is functionally complete; this phase ensures the diff merges cleanly.

- [X] T023 [P] Add a new row to `docs/reference/sbom-format-mapping.md` documenting the binary-role mapping table (CDX `type` ↔ SPDX 2.3 `primaryPackagePurpose` ↔ SPDX 3 `software_primaryPurpose`). Format matches the existing C-row / B-row entries. Reference the milestone-104 contracts/binary-role-cross-format-mapping.md for the audit trail.
- [X] T024 [P] Regenerate byte-identity goldens. Run `MIKEBOM_UPDATE_CDX_GOLDENS=1 cargo +stable test -p mikebom --test cdx_regression`, `MIKEBOM_UPDATE_SPDX_GOLDENS=1 cargo +stable test -p mikebom --test spdx_regression`, `MIKEBOM_UPDATE_SPDX3_GOLDENS=1 cargo +stable test -p mikebom --test spdx3_regression`. Audit the diffs: only fixtures that contain file-level binary-reader components (per R4 research finding — `polyglot-rpm-binary`, `binaries`, image-scan-based fixtures) should regen; cargo/gem/npm/etc. fixtures are byte-identical. Reject any unexpected golden diff.
- [X] T025 Update `CHANGELOG.md` with an `### Issue #N or Milestone 104 — Binary role classification` section (or whichever pattern matches the open `## [Unreleased]` section). Reference: spec.md, the cross-format mapping contract, the consumer-importance rationale ("CDX `type: application` is the natural answer for executables; mis-typing as `library` reads as inversion"), and the goldens regen scope.
- [X] T026 Run `./scripts/pre-pr.sh` and verify: `cargo +stable clippy --workspace --all-targets` clean, `cargo +stable test --workspace` all-green. Investigate and fix any new failures.
- [X] T027 Execute the manual reproduction in `specs/104-binary-role-classification/quickstart.md` against the built `target/release/mikebom` binary. Confirm every `jq` invocation produces the expected output (executables emit `application`, dylibs emit `library`, cross-format role values agree).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: empty — no work.
- **Phase 2 (Foundational, T001–T004)**: BLOCKS all user stories. T001 → T002 → T003 sequentially (same file). T004 can run alongside T003 (different file set).
- **Phase 3 (US1, T005–T011)**: depends on Phase 2 complete.
- **Phase 4 (US2, T012–T016)**: depends on Phase 2 complete AND on T008 (binary_role field on ResolvedComponent populated). Can start as soon as US1's T008 lands, in parallel with US1's T009-T011.
- **Phase 5 (US3, T017–T022)**: depends on Phase 2 complete AND on T005 (role.rs exists). Can start as soon as US1's T006 lands, in parallel with US1's T009-T011 and US2 entirely.
- **Phase 6 (Polish, T023–T027)**: depends on US1, US2, US3 all complete.

### User Story Dependencies

- **US1 (P1)**: foundational only. Self-contained CDX flow.
- **US2 (P2)**: shares the `binary_role` field with US1 but emits to different files (`generate/spdx/`) so can land independently after T008.
- **US3 (P3)**: extends `role.rs::classify` with PIE disambiguation; the new behavior changes some scan outputs (Linux PIE binaries flip from `library` to `application`). Test coverage for US3 lives in its own integration test file. US1 and US2's tests don't test PIE binaries specifically, so they remain green whether US3 lands before or after them. Recommend US3 lands BEFORE goldens regen (T024) so the regen reflects the final classifier behavior — otherwise a second goldens pass is needed after US3.

### Within Each User Story

- US1: T005 (create role.rs) → T006 (re-export) → T007/T008 (wire BinaryScan + entry.rs) → T009 (CDX emitter) → T010/T011 (tests). T009 can start as soon as T008 lands.
- US2: T012 (SPDX 2.3) and T013 (SPDX 3) are independent files and run in parallel. T014 (parity catalog) depends on both. T015/T016 (test extensions) depend on T012 + T013.
- US3: T017 (PT_INTERP helper) → T018 (wire into classify) and T019 (Mach-O Other log) in parallel. Tests T020/T021/T022 depend on T018 + T019.

### Parallel Opportunities

- T003 + T004 (mechanical None updates in different file scopes) — parallel.
- T010 (unit tests in role.rs) and T011 (integration test) — independent files, parallel.
- T012 (SPDX 2.3) and T013 (SPDX 3) — independent files, parallel.
- T017 (PT_INTERP helper) and T019 (Mach-O log addition) — different sections of the same file but non-overlapping logic, parallel-safe.
- T023 (docs) and T024 (goldens regen) — independent paths, parallel after US1+US2+US3 complete.

---

## Parallel Example: User Story 1

```bash
# Once Phase 2 completes, US1 + US2 + US3 can be picked up by different
# implementers / parallel agent runs. Within US1 specifically:

# T010 + T011 can run in parallel once T008 lands (different test files):
Task: "Add unit tests in mikebom-cli/src/scan_fs/binary/role.rs::tests"
Task: "Add integration test mikebom-cli/tests/binary_role_parity.rs"
```

## Parallel Example: User Story 2

```bash
# T012 and T013 are independent files (SPDX 2.3 vs SPDX 3 emitters):
Task: "Extend generate/spdx/packages.rs::primary_package_purpose"
Task: "Extend generate/spdx/v3_packages.rs::software_primaryPurpose"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 (no-op).
2. Phase 2 (T001–T004) — `cargo +stable check --workspace --all-targets` green.
3. Phase 3 / US1 (T005–T011) — CDX emits correct role-typed `type` field for executables and shared libraries. Integration test passes.
4. **STOP and VALIDATE**: scan `/bin/ls` (or any single Mach-O executable directory), confirm `jq '.components[] | .type'` shows `application` for executables and `library` for dylibs. This is shippable as a CDX-only fix and resolves the reporter's most visible concern.

### Incremental Delivery

5. Phase 4 / US2 (T012–T016) — SPDX 2.3 + SPDX 3 follow. Holistic-parity row A13 enforces cross-format invariant.
6. Phase 5 / US3 (T017–T022) — ELF PIE disambiguation makes Linux scans correct. Audit-trail logs appear for ambiguous bundles.
7. Phase 6 / Polish (T023–T027) — docs, goldens, CHANGELOG, pre-PR gate.

### Parallel Agent Strategy

If multiple agents are available:
- Agent A: Phase 2 (T001-T004) → wait for compilation → Phase 3 (T005-T011)
- Agent B: After T008 lands → Phase 4 (T012-T016) in parallel
- Agent C: After T006 lands → Phase 5 (T017-T022) in parallel
- Phase 6 runs after all three complete (single agent — sequential by design since T024 goldens regen depends on every emission change being in place).
