---
description: "Task list — milestone 039 apk per-file deep-hash"
---

# Tasks: Per-File Evidence for apk Components

**Input**: spec.md ✅, plan.md ✅, checklists/requirements.md ✅.

**Tests**: included — inline coverage for the new helpers, plus a
smoke-test assertion update.

**Organization**: Single user story (US1, P1).

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

- [ ] T001 Snapshot baseline by running `./scripts/pre-pr.sh` from the repo root and confirming clean before any changes
- [ ] T002 Confirm milestone 038's distroless smoke test still passes via `MIKEBOM_OCI_NETWORK_TESTS=1 cargo +stable test --manifest-path /Users/mlieberman/Projects/mikebom/Cargo.toml -p mikebom --test oci_registry_smoke pulls_distroless_static_and_emits_dpkg_status_d_components`

---

## Phase 2: Commit `feat(039/extract-file-lists)`

- [ ] T003 [US1] Add `pub fn read_file_lists(rootfs: &Path) -> std::collections::HashMap<String, Vec<String>>` to `mikebom-cli/src/scan_fs/package_db/apk.rs`. Walks `<rootfs>/lib/apk/db/installed` once, tracking the current package via `P:` and current directory via `F:`, and accumulating each `R:` line as a rootfs-relative path under the current package. Blank-line-separated stanzas reset state.
- [ ] T004 [US1] Add inline test `read_file_lists_extracts_per_package_paths` in `apk.rs::tests`: synthetic installed-db with 2 packages, each owning 2 files; assert the returned map has 2 entries with the right paths.
- [ ] T005 [US1] Add inline test `read_file_lists_handles_empty_dir_for_root_level_files` in `apk.rs::tests`: stanza with `F:` (empty value) followed by `R:` lines; assert the resulting paths have no leading directory.
- [ ] T006 [US1] Add inline test `read_file_lists_returns_empty_when_db_absent` in `apk.rs::tests`: tempdir with no `/lib/apk/db/installed`; assert empty map, no error.
- [ ] T007 [US1] Mark the new fn with `#[allow(dead_code)]` since callers come in commit 3.
- [ ] T008 [US1] Run `cargo +stable test -p mikebom --bin mikebom scan_fs::package_db::apk` and confirm new tests pass alongside existing.
- [ ] T009 [US1] `./scripts/pre-pr.sh` clean.
- [ ] T010 [US1] Commit: `feat(039/extract-file-lists): apk per-package file-list extraction from installed-db F:/R: stanzas`.

---

## Phase 3: Commit `feat(039/hash-and-wire)`

- [ ] T011 [US1] Add `pub fn hash_apk_package_files(rootfs: &Path, files: &[String]) -> (Vec<FileOccurrence>, Option<ContentHash>)` in `file_hashes.rs`. Mirrors `hash_package_files` shape: walks each rootfs-relative path, opens, SHA-256s the content (cap inherited via `MAX_PER_FILE_BYTES`), accumulates occurrences. Computes Merkle root over occurrences. Skips absent / oversized files silently. No `md5_legacy` field — apk's analogous `Z:` cross-ref is out of scope.
- [ ] T012 [US1] Add `pub fn hash_apk_db_only(rootfs: &Path, pkg_name: &str) -> Option<ContentHash>` for the `--no-deep-hash` fast path. SHA-256s the bytes of the package's stanza extracted from the installed-db (we can't hash a per-package companion file because apk doesn't ship one). Returns `None` if the package isn't found.
- [ ] T013 [US1] Inline test `hash_apk_package_files_round_trips_files` in `file_hashes.rs::tests`: synthetic rootfs with 2 files; pass them as the file-list; assert 2 occurrences with valid 64-hex SHA-256.
- [ ] T014 [US1] Inline test `hash_apk_package_files_skips_absent_files` in `file_hashes.rs::tests`: file-list claims 2 files, only 1 exists on disk; assert 1 occurrence.
- [ ] T015 [US1] Inline test `hash_apk_package_files_returns_empty_for_empty_input` in `file_hashes.rs::tests`: empty file-list; assert empty Vec + None root.
- [ ] T016 [US1] Edit `mikebom-cli/src/scan_fs/mod.rs` (line 370 area): add an `is_apk` detector mirroring `is_dpkg`. In the deep-hash dispatcher, add an apk branch:
  - Deep mode: look up the file list from the per-scan map, call `hash_apk_package_files`.
  - Fast mode: call `hash_apk_db_only`.
  - The per-scan apk file-list map is built once via `apk::read_file_lists` before the per-entry loop.
- [ ] T017 [US1] Remove `#[allow(dead_code)]` from `apk::read_file_lists` now that it's wired up.
- [ ] T018 [US1] Edit `mikebom-cli/tests/oci_registry_smoke.rs`'s `pulls_alpine_3_19_and_emits_apk_components`: add a milestone-039 assertion that total per-component occurrences > 0 AND at least one occurrence carries a 64-hex SHA-256 in the established CDX evidence shape.
- [ ] T019 [US1] Run `MIKEBOM_OCI_NETWORK_TESTS=1 cargo +stable test --manifest-path /Users/mlieberman/Projects/mikebom/Cargo.toml -p mikebom --test oci_registry_smoke pulls_alpine_3_19_and_emits_apk_components` and confirm passing.
- [ ] T020 [US1] Run goldens regen: `MIKEBOM_UPDATE_*_GOLDENS=1 cargo +stable test -p mikebom --test '*'`. Existing alpine-fixture goldens MAY change additively (new `evidence.occurrences[]`); verify the change is additive only via `git diff`. Other goldens (deb-fixture, non-apk-fixture) must show zero diff.
- [ ] T021 [US1] `./scripts/pre-pr.sh` clean.
- [ ] T022 [US1] Commit: `feat(039/hash-and-wire): per-file evidence for apk components — apk reaches deb parity`.

---

## Phase 4: Commit `docs(039)`

- [ ] T023 Update `docs/user-guide/cli-reference.md` Behaviour notes paragraph: apk components now produce per-file evidence in the same shape as deb components (remove the "tracked as #75" qualifier).
- [ ] T024 Add CHANGELOG Unreleased entry: milestone 039 closes #75 — apk per-file evidence at deb parity for alpine + chainguard apko + Wolfi.
- [ ] T025 `./scripts/pre-pr.sh` clean.
- [ ] T026 Commit: `docs(039): apk per-file evidence — user-guide + CHANGELOG`.

---

## Phase 5: PR + verification

- [ ] T027 Push branch.
- [ ] T028 Open PR closing #75. PR body lists pre/post numbers (alpine + chainguard) + zero-non-apk-golden-drift confirmation.
- [ ] T029 Verify all 3 CI lanes green (Linux default + Linux ebpf + macOS).
