---
description: "Task list for m187 ipk reader — modern ar-format extraction + filename-fallback arch fix"
---

# Tasks: ipk reader — ar-format extraction + filename-fallback arch fix (m187)

**Input**: Design documents from `/specs/187-ipk-yocto-reader-fixes/`
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/ ✓, quickstart.md ✓

**Tests**: Included — data-model.md §9 enumerates 10 unit tests + 9 integration tests. TDD ordering within each phase (unit tests colocated in `ipk_file.rs`, integration tests in `mikebom-cli/tests/ipk_yocto_reader_fixes.rs`).

**Organization**: Two P1 user stories, both bug-fix follow-ups to m185. US1 (#543 archive-extraction) must land before US2 (#542 arch-source) can be independently tested end-to-end — the two share the same `ipk_file.rs` file and both edits touch adjacent code, so sequential development is the practical shape even though the stories are conceptually independent.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story (US1, US2)
- Include exact file paths in descriptions

## Path Conventions

- Rust workspace root at repository root; all production changes in `mikebom-cli/src/scan_fs/package_db/ipk_file.rs`; new integration test file at `mikebom-cli/tests/ipk_yocto_reader_fixes.rs`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: SC-007 zero-new-deps baseline anchoring.

- [X] T001 Capture pre-m187 `cargo tree --workspace | wc -l` line count baseline — 1136 lines persisted at `specs/187-ipk-yocto-reader-fixes/artifacts/cargo-tree-pre.txt`.

**Checkpoint**: Baseline captured.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The `ArMember` type + `IpkParseError` enum update + `parse_ar_archive` STUB that both US1 and US2 need to exist before their tests can compile. Type-driven correctness gate per Principle IV.

**⚠️ CRITICAL**: US1 and US2 both block on Phase 2 completion.

- [X] T002 Renamed `IpkParseError::LegacyArFormat` → `LegacyGzipTarFallbackFailed(String)`; 4 callsites updated (Display impl, parse_ipk_file dispatch, test variant list, existing single ref).
- [X] T003 Added `IpkParseError::ArMalformed(String)` variant with Display impl.
- [X] T004 Added `ArMember` struct + `ArError` enum (TruncatedHeader / NonAsciiSizeField / SizeOverrunsBody) + `parse_ar_archive` STUB, all under the m187 banner.
- [X] T005 Added `ArchSource` enum (ParentDirectory / FilenameHeuristic) + `ParsedFilename` struct. `ArchSource::as_wire_str()` helper for property emission.
- [X] T006 Updated `parse_ipk_filename` signature to `(filename, parent_dir_name: Option<&str>) -> Option<ParsedFilename>` — MERGED with T018's parent-dir handling since the return-type change is atomic. Includes parent-dir suffix-match path AND legacy rsplit fallback. `filename_fallback_entry` updated to consult parent-dir + emit `mikebom:arch-source` property. 6 existing tests migrated to struct-accessor assertions.
- [X] T007 `cargo +stable build --workspace --all-targets` — clean (only dead-code warnings for unwired stubs cleared in Phase 3).

**Checkpoint**: US1 and US2 tests can now compile against the new types. Type-driven correctness gate closed.

---

## Phase 3: User Story 1 — ar-format primary path with control-file extraction (Priority: P1) 🎯 MVP

**Goal**: Operators scanning Yocto build output get full `License:` / `Depends:` / `Recommends:` / `Section:` / `Maintainer:` metadata extracted from every modern ar-format ipk. Closes #543.

**Independent Test**: spec.md §User Story 1 Acceptance 1-5. Verifiable via `mikebom-cli/tests/ipk_yocto_reader_fixes.rs::us1_*` (4-5 tests) with synthetic ar-format ipks fabricated at test time.

### Tests for User Story 1 (write FIRST, ensure they FAIL before implementation) ⚠️

- [X] T008 [P] [US1] 5 ar-parser unit tests added + passing (7 including edge cases). Uses `ar_header` + `ar_archive` fixture helpers.

### Implementation for User Story 1

- [X] T009 [US1] `parse_ar_archive` fully implemented — BSD ar 60-byte-header state machine, ~65 lines. Zero unsafe, zero external deps.
- [X] T010 [US1] `parse_ipk_file` refactored — ar-format PRIMARY branch (calls `parse_ar_archive` → `parse_ipk_from_ar_members`); legacy gzip-tar as SECONDARY branch (wraps errors in `LegacyGzipTarFallbackFailed`).
- [X] T011 [US1] `build_entry_from_control` extended with `source_mechanism_value: &str` + `emit_arch_source: bool` params. ar path emits `arch-source = "control-file"`; legacy path does NOT emit `arch-source` (FR-014 / SC-005 byte-identity).
- [X] T012 [US1] Added `extract_control_file_from_bytes(bytes, gzipped: bool)` + `extract_control_from_plain_tar` for uncompressed `control.tar` support (edge case). Legacy `extract_control_from_gzipped_tar` preserved.
- [X] T013 [US1] `walk_data_tar_file_list(data_bytes, gzipped)` helper extracted; called from `parse_ipk_from_ar_members` + `collect_claimed_paths` ar branch.
- [X] T014 [US1] `collect_claimed_paths` ar-format branch added — parses ar members, walks `data.tar[.gz]` file-list, populates claim set. Pre-m187 short-circuit removed.
- [X] T015 [US1] 5 US1 integration tests added to `mikebom-cli/tests/ipk_yocto_reader_fixes.rs`, all passing:
  - `us1_ar_format_extracts_control_metadata` (with stub libc6 ipk to allow Depends edge resolution).
  - `us1_ar_format_tolerates_missing_debian_binary`.
  - `us1_pre_2015_gzip_tar_still_works` (asserts NO `arch-source` on legacy path).
  - `us1_malformed_ar_falls_through_to_filename` (asserts pre-m187 "legacy ar-format" string GONE).
  - `us1_sc004_invariant_extraction_implies_license` (3-ipk fixture, invariant across MIT / Apache-2.0 / empty).

**Checkpoint**: US1 fully functional. Ship as MVP. `mikebom sbom scan --path <yocto-ipk-dir>` produces license + deps metadata for the first time.

---

## Phase 4: User Story 2 — Parent-directory arch source for filename fallback (Priority: P1)

**Goal**: Operators scanning Yocto qemux86_64 (or any multi-underscore-arch) build get correct `?arch=` PURL qualifiers + correct `version` strings under the filename-fallback path. Closes #542.

**Independent Test**: spec.md §User Story 2 Acceptance 1-5. Verifiable via `mikebom-cli/tests/ipk_yocto_reader_fixes.rs::us2_*` (4-5 tests). The T015 test file already exists; T020 appends US2 tests to it.

### Tests for User Story 2 (write FIRST) ⚠️

- [X] T016 [P] [US2] 6 US2 unit tests added + passing (parent_dir_arch_match Yocto convention, loose layout, multi-underscore, kernel-module round-trip, rsplit fallback).

### Implementation for User Story 2

- [X] T017 [US2] `parent_dir_arch_match` helper implemented (merged with T006).
- [X] T018 [US2] `parse_ipk_filename` fully updated (merged with T006).
- [X] T019 [US2] `filename_fallback_entry` fully updated (merged with T006) — consults parent dir + emits `mikebom:arch-source`.
- [X] T020 [US2] 5 US2 integration tests added + passing (qemux86_64 parent-dir, powerpc_e500v2 multi-underscore, no-match filename heuristic, control-file authoritative, 3-ipk combined regression).

**Checkpoint**: US2 fully functional. Yocto qemux86_64 (and any multi-underscore-arch) scans now emit correct PURLs even when the archive-extraction path fails.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: SC-007 verification, pre-PR gate, docs, PR.

- [ ] T021 [P] Update `docs/reference/sbom-format-mapping.md` (or the equivalent m152/m185 documentation location) to document the two new `mikebom:*` properties: `mikebom:source-mechanism = "ipk-file-archive-extraction"` (new value; joins existing `"ipk-file"` + `"ipk-file-filename-fallback"`) and `mikebom:arch-source` (new property; three values: `control-file`, `parent-directory`, `filename-heuristic`). Include a sentence justifying each per Constitution Principle V — no native CDX / SPDX 2.3 / SPDX 3 construct exists for "which code path emitted this qualifier".
- [ ] T022 Verify SC-007 zero-new-deps gate: capture post-m187 `cargo tree --workspace | wc -l` line count; diff against `specs/187-ipk-yocto-reader-fixes/artifacts/cargo-tree-pre.txt` from T001 — MUST be identical. If differs, root-cause and remove transitive additions before opening the PR.
- [ ] T023 Run `./scripts/pre-pr.sh` — both `cargo +stable clippy --workspace --all-targets -- -D warnings` and `cargo +stable test --workspace --no-fail-fast` MUST report `0 failed` on every target per CLAUDE.md. Capture the per-target `N passed; 0 failed` enumeration for the PR description per memory `feedback_prepr_gate_full_output`.
- [ ] T024 Verify FR-014 / SC-005 byte-identity for the existing m169-era ipk-reader integration test. Specifically run `cargo +stable test -p mikebom --test ipk_reader` and confirm every test passes with zero drift on the gzip-tar-format code path. This suite includes the m185 fixture additions (filename-fallback, malformed-body salvage, installed-DB dedup) that must continue to work identically post-m187.
- [ ] T025 Manual smoke test — the quickstart.md Example 3 (legacy gzip-tar) is the highest-signal to verify locally. Fabricate one pre-2015-style ipk via `tar czf inner.tar.gz control.tar.gz data.tar.gz debian-binary` and scan it; confirm `mikebom:source-mechanism = "ipk-file"` (unchanged) and license/depends extracted normally.
- [ ] T026 Open PR from `187-ipk-yocto-reader-fixes` → `main` with PR title `impl(187): ipk reader — ar-format extraction + filename-fallback arch fix (#542 + #543)` and body linking to spec.md + plan.md + tasks.md. Close #542 + #543 in the PR body.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001 only. No dependencies.
- **Foundational (Phase 2)**: Depends on Setup. T002 (rename) MUST come first — it triggers compile errors that T003–T007 will collectively resolve. **BLOCKS US1 + US2.**
- **US1 (Phase 3)**: Depends on Foundational.
- **US2 (Phase 4)**: Depends on Foundational + shares `ipk_file.rs` with US1; sequential in practice. Some US2 unit tests (T016) can be authored in parallel with US1 impl (T009–T014) if desired.
- **Polish (Phase 5)**: Depends on US1 + US2 completion.

### User Story Dependencies

- **US1 (P1)**: Foundational only.
- **US2 (P1)**: Foundational + US1's `ParsedFilename`/`ArchSource` types (from T005) + `parent_dir_arch_match` helper (added in T017). If US1 is fully staffed out sequentially first, US2 slots in cleanly.

### Within Each User Story

- **US1**: Tests FIRST (T008), then impl (T009 → T010 → T011 → T012 → T013 → T014), then integration tests (T015).
- **US2**: Tests FIRST (T016), then impl (T017 → T018 → T019), then integration tests (T020).

### Parallel Opportunities

- **Phase 2**: T004 || T005 (different types; no dependencies on each other). T006 depends on T005.
- **Phase 3 US1 tests**: T008 (single task; the 5 unit tests are all in one file/block — one contributor authoring).
- **Phase 4 US2 tests**: T016 (same shape as T008).
- **Phase 5 Polish**: T021 (docs) can run in parallel with T022 (cargo tree) and T024 (regression run); T023 pre-PR must be last.

---

## Parallel Example: Foundational Phase 2

```bash
# Sequential (compile errors cascade):
Task: "T002 rename LegacyArFormat → LegacyGzipTarFallbackFailed"

# Then parallel:
Task: "T004 add ArMember + ArError + parse_ar_archive STUB"
Task: "T005 add ArchSource enum + ParsedFilename struct"

# Then sequential (T003 depends on T002 compile-clean):
Task: "T003 add ArMalformed variant"
Task: "T006 update parse_ipk_filename signature"

# Finally:
Task: "T007 cargo build --all-targets validation"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001) — 2 minutes.
2. Complete Phase 2: Foundational (T002–T007) — 45 minutes. **CRITICAL: blocks all US work.**
3. Complete Phase 3: US1 (T008–T015) — 3-4 hours.
4. **STOP and VALIDATE**: `cargo +stable test --workspace` — verify US1 tests pass, m185 regression pass, no drift.
5. Demoable: `mikebom sbom scan --path <yocto-ipk-dir>` produces license + deps for the first time.

### Incremental Delivery

1. Setup + Foundational → foundation ready.
2. Add US1 → validate license/deps extraction end-to-end. **Ship-able as a standalone fix for #543.**
3. Add US2 → validate qemux86_64 arch correctness. **Closes #542 too.**
4. Polish (Phase 5) → SC-007 gate + pre-PR + PR.

### Sequential Team Strategy

One developer:
- Day 1 morning: Phase 1 + Phase 2 (setup + scaffolding).
- Day 1 afternoon: Phase 3 US1 (write ar tests → implement parser → wire into dispatch → integration tests).
- Day 2 morning: Phase 4 US2 (write suffix-match tests → implement → wire into fallback → integration tests).
- Day 2 afternoon: Phase 5 (docs + pre-PR + PR).

---

## Notes

- All 26 tasks strictly follow the `- [ ] T### [P?] [Story?] Description with file path` format.
- Each task has a concrete file path and specific instruction.
- Tests before implementation per data-model.md §9 test-contract commitment.
- Commit after each task or logical group. Avoid amending; use new commits per CLAUDE.md.
- The rename in T002 will trigger compile errors at every callsite of `IpkParseError::LegacyArFormat`. Do NOT `--allow` these; the compiler is the safety net that ensures every dispatch site is updated correctly (research Decision 4).
- The `#[cfg(test)] mod tests { #[cfg_attr(test, allow(clippy::unwrap_used))] }` guard is REQUIRED in `ipk_file.rs` — the crate root denies `clippy::unwrap_used` per Constitution Principle IV; test code must opt out explicitly.
- Do NOT skip T024 (regression suite check) — FR-014 / SC-005 byte-identity is the guarantee that pre-2015 ipk scans continue to work identically. Any drift here is a shippable-blocker regression.
- Do NOT skip T022 (cargo-tree zero-drift verification) — SC-007 zero-new-deps is a Constitution Principle I gate.
