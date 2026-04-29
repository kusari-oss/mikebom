---
description: "Task list — milestone 040 package-db follow-ons (housekeeping + apk SHA-1 cross-ref + rpm per-file deep-hash)"
---

# Tasks: Package-DB Follow-Ons (Trifecta)

**Input**: Design documents from `/specs/040-pkg-db-followups/`
**Prerequisites**: spec.md ✅, plan.md ✅, research.md ✅, data-model.md ✅, quickstart.md ✅, checklists/requirements.md ✅

**Tests**: included — inline coverage for each new helper, plus a smoke-test assertion update for US2.

**Organization**: Three independent user stories sequenced by priority. Each is independently testable and each commit's `./scripts/pre-pr.sh` is clean.

## Format: `[ID] [P?] [Story?] Description`

---

## Phase 1: Setup

- [ ] T001 Snapshot pre-implementation baseline by running `./scripts/pre-pr.sh` from the repo root and confirming clean before any changes
- [ ] T002 Confirm milestone 039's apk smoke test still passes via `MIKEBOM_OCI_NETWORK_TESTS=1 cargo +stable test --manifest-path /Users/mlieberman/Projects/mikebom/Cargo.toml -p mikebom --test oci_registry_smoke pulls_alpine_3_19_and_emits_apk_components` (post-039 baseline healthy on this branch)

---

## Phase 2: Foundational

**Purpose**: None for this milestone. Each user story extends an established pattern from prior milestones (037 / 038 / 039); no shared infrastructure to land first.

---

## Phase 3: User Story 1 - Stale OCI comment cleanup (Priority: P1) 🎯 MVP

**Goal**: Remove the lingering "deferred to milestone 031.y" string in `oci_pull/mod.rs` that names the long-shipped `--image-platform` flag as deferred. After this commit, `grep -rn 'deferred to milestone 031' mikebom-cli/src/` returns zero matches.

**Independent Test**: `grep -rn 'deferred to milestone 031' mikebom-cli/src/` returns zero matches (today: 1 in `oci_pull/mod.rs:215`). Manually trigger an unmapped-arch error path and confirm the message points at the shipped `--image-platform` flag.

### Implementation for User Story 1

- [ ] T003 [US1] Edit `mikebom-cli/src/scan_fs/oci_pull/mod.rs::host_oci_arch` (around line 215) to rewrite the bail message: drop the "deferred to milestone 031.y" framing and instead point users at the shipped `--image-platform <linux/arch>` flag. Wording emphasizes the flag's existence rather than naming any deferred milestone.
- [ ] T004 [US1] Run `grep -rn 'deferred to milestone 031' mikebom-cli/src/` and confirm zero matches (SC-001).
- [ ] T005 [US1] Run `grep -rn 'deferred to milestone' mikebom-cli/src/` and audit each remaining match — every one MUST point at a genuinely deferred follow-on (no false positives left from this cleanup).
- [ ] T006 [US1] `./scripts/pre-pr.sh` clean.
- [ ] T007 [US1] Commit: `fix(040/us1): drop stale "deferred to milestone 031.y" framing; point at shipped --image-platform`.

**Checkpoint**: At this point, US1 is fully delivered. The remaining stories may proceed independently.

---

## Phase 4: User Story 2 - Apk Z:-line SHA-1 cross-reference (Priority: P2)

**Goal**: Each apk per-file occurrence's `additionalContext` JSON-string carries both `sha256` (mikebom-computed) and `sha1` (apk-provided from the `Z:` line). Mirrors deb's `md5` cross-ref shape from milestone 037.

**Independent Test**: `mikebom sbom scan --image alpine:3.19 --output /tmp/alpine.cdx.json` followed by `jq '.components[0].evidence.occurrences[0].additionalContext'` shows a JSON-string containing both `sha256` and `sha1` keys (today: only `sha256`).

### Commit `feat(040/us2-extract-z-lines)`

- [ ] T008 [US2] Add a new struct `ApkFileEntry { path: String, sha1: Option<String> }` to `mikebom-cli/src/scan_fs/package_db/apk.rs`. The optional `sha1` is the 40-hex-char lowercase form of the apk-provided SHA-1.
- [ ] T009 [US2] Change the return type of `apk::read_file_lists` from `HashMap<String, Vec<String>>` to `HashMap<String, Vec<ApkFileEntry>>`. Within the existing F:/R: walk, also track `Z:` lines that follow each `R:`. Decode each `Z:` value per research.md R2: strip the `Q1` prefix (skip on any other prefix), base64-decode the trailing 28-char payload (using the workspace `base64` dep), hex-encode the resulting 20 bytes as a 40-char lowercase string. Store as the `sha1` field; `None` when missing or malformed.
- [ ] T010 [US2] Mark the new struct + return-type change with whatever `#[allow(dead_code)]` is needed pending the next commit's wire-up. (The existing single caller in `scan_fs/mod.rs` will need its signature updated alongside.)
- [ ] T011 [P] [US2] Add inline test `read_file_lists_extracts_z_line_sha1` in `apk.rs::tests`: synthetic stanza with `R:foo` followed by `Z:Q1<base64-encoded-known-sha1>`; assert the resulting entry has `sha1 == Some("<expected 40-hex>")`.
- [ ] T012 [P] [US2] Add inline test `read_file_lists_handles_missing_z_line` in `apk.rs::tests`: synthetic stanza with `R:foo` followed by `R:bar` (no `Z:` between); assert `foo` and `bar` both have `sha1 == None`.
- [ ] T013 [P] [US2] Add inline test `read_file_lists_rejects_non_q1_prefix` in `apk.rs::tests`: synthetic stanza with `R:foo` followed by `Z:Q2<base64-data>` (hypothetical future scheme); assert `foo` has `sha1 == None` (defensive: only Q1 is currently a known apk scheme).
- [ ] T014 [US2] Run `cargo +stable test -p mikebom --bin mikebom scan_fs::package_db::apk` and confirm the 3 new tests pass alongside the existing 18.
- [ ] T015 [US2] `./scripts/pre-pr.sh` clean.
- [ ] T016 [US2] Commit: `feat(040/us2-extract-z-lines): apk Z:-line SHA-1 extraction in read_file_lists`.

### Commit `feat(040/us2-thread-sha1-into-evidence)`

- [ ] T017 [US2] Add an `apk_sha1: Option<String>` field to `mikebom_common::resolution::FileOccurrence` (in the `mikebom-common` crate). Default-`None` via `#[serde(default, skip_serializing_if = "Option::is_none")]` so existing serialized fixtures still round-trip identically.
- [ ] T018 [US2] Update `file_hashes.rs::hash_apk_package_files`'s signature from `&[String]` to `&[ApkFileEntry]`. Inside the per-file loop, copy the `sha1` from the entry onto the resulting `FileOccurrence.apk_sha1`. Update the existing `hash_apk_package_files_*` inline tests to construct `ApkFileEntry`s.
- [ ] T019 [US2] Edit `mikebom-cli/src/generate/cyclonedx/evidence.rs`: when serializing per-occurrence `additionalContext`, if `o.apk_sha1` is `Some`, include it as `"sha1": "<value>"` alongside the existing `"sha256"` key. Existing `"md5"` (dpkg) emission unchanged.
- [ ] T020 [US2] Verify the SPDX 2.3 and SPDX 3 emitters (under `mikebom-cli/src/generate/spdx*/`) consume `additionalContext` via the same shape — if any of them reach into the field directly rather than going through CycloneDX-shaped helpers, mirror the apk-sha1 extension there too. Otherwise no edit needed.
- [ ] T021 [US2] Update the call site in `mikebom-cli/src/scan_fs/mod.rs`'s deep-hash dispatcher: the apk file-list map now yields `Vec<ApkFileEntry>` not `Vec<String>`. Pass slices through `hash_apk_package_files` unchanged.
- [ ] T022 [US2] Edit the existing milestone-039 alpine smoke test in `mikebom-cli/tests/oci_registry_smoke.rs` (`pulls_alpine_3_19_and_emits_apk_components`): in the per-occurrence `additionalContext` parse loop, also count occurrences whose ctx contains a `sha1` key. Assert `sha1_seen > 0` AND that the recovered SHA-1 strings match the format expected (40-hex-char lowercase).
- [ ] T023 [US2] Run `MIKEBOM_OCI_NETWORK_TESTS=1 cargo +stable test --manifest-path /Users/mlieberman/Projects/mikebom/Cargo.toml -p mikebom --test oci_registry_smoke pulls_alpine_3_19_and_emits_apk_components` and confirm passing.
- [ ] T024 [US2] Run goldens regen: `MIKEBOM_UPDATE_*_GOLDENS=1 cargo +stable test -p mikebom --test '*'`. Confirm `git status --short` shows zero diffs under `mikebom-cli/tests/fixtures/27/` (the goldens use `--no-deep-hash` so apk-sha1 doesn't surface there).
- [ ] T025 [US2] `./scripts/pre-pr.sh` clean.
- [ ] T026 [US2] Commit: `feat(040/us2-thread-sha1-into-evidence): apk SHA-1 cross-ref in additionalContext alongside sha256`.

**Checkpoint**: At this point, US1 + US2 both work independently. Apk components carry the cross-ref the deb side has carried since milestone 037.

---

## Phase 5: User Story 3 - Rpm per-file deep-hashing (Priority: P3)

**Goal**: Each rpm component carries a populated `evidence.occurrences[]` block with file paths and SHA-256 hashes. Closes the OS-package per-file-evidence trilogy (deb 037+038, apk 039, rpm 040).

**Independent Test**: `mikebom sbom scan --image fedora:40 --output /tmp/fedora.cdx.json` followed by `jq '[.components[].evidence.occurrences | length // 0] | add'` returns a positive integer (today: 0).

### Commit `feat(040/us3-rpm-deep-hash)`

- [ ] T027 [US3] Add `pub fn read_file_lists(rootfs: &Path) -> std::collections::HashMap<String, Vec<String>>` to `mikebom-cli/src/scan_fs/package_db/rpm.rs`. Reuses the existing `iter_rpmdb` visitor pattern: each visited `(PackageDbEntry, Vec<PathBuf>)` contributes one map entry keyed on `entry.name`, with the path-list converted from `Vec<PathBuf>` to `Vec<String>` (the sha-256 loop wants `String`).
- [ ] T028 [US3] Add `pub fn hash_rpm_package_files(rootfs: &Path, files: &[String]) -> (Vec<FileOccurrence>, Option<ContentHash>)` to `mikebom-cli/src/scan_fs/package_db/file_hashes.rs`. Mirrors `hash_apk_package_files` exactly (no cross-ref): walks each rootfs-relative path, opens, SHA-256s, accumulates occurrences with `apk_sha1: None` and `md5_legacy: None`. Computes Merkle root.
- [ ] T029 [US3] Add `pub fn hash_rpm_db_only(rootfs: &Path, pkg_name: &str) -> Option<ContentHash>` to `file_hashes.rs`. Per research.md R5, this is the SHA-256 of the named package's HeaderBlob bytes. Implementation: iterate the rpmdb via a small helper that exposes the blob bytes; SHA-256 the matching one.
- [ ] T030 [US3] Edit `mikebom-cli/src/scan_fs/mod.rs`: build the rpm file-list map ONCE per scan via `rpm::read_file_lists(root)` before the per-entry loop (parallel to the `apk_file_lists` variable from milestone 039). Add `let is_rpm = entry.source_path.contains("lib/rpm/");` parallel to `is_dpkg` / `is_apk`. Add an `else if is_rpm` branch in the deep-hash dispatcher that mirrors the apk branch (deep mode → `hash_rpm_package_files`; fast mode → `hash_rpm_db_only`).
- [ ] T031 [P] [US3] Add inline test `hash_rpm_package_files_round_trips_files` in `file_hashes.rs::tests`: synthetic rootfs with 2 files; call with their relative paths; assert 2 occurrences with valid 64-hex SHA-256, absolute-prefixed `location`, and `md5_legacy`/`apk_sha1` both `None`.
- [ ] T032 [P] [US3] Add inline test `hash_rpm_package_files_skips_absent_files` in `file_hashes.rs::tests`: file-list claims 2 files, only 1 exists on disk; assert 1 occurrence.
- [ ] T033 [P] [US3] Add inline test `hash_rpm_package_files_returns_empty_for_empty_input` in `file_hashes.rs::tests`: empty file-list; assert empty Vec + None root (FR-009 metadata-only-package handling).
- [ ] T034 [P] [US3] Add inline test `read_file_lists_extracts_per_package_paths` in `rpm.rs::tests`: use the existing test-fixture infrastructure (`rpm::tests::build_test_header` is already used for fixture data per the file we surveyed earlier); construct a synthetic rpmdb with 2 packages and verify `read_file_lists` returns the right per-package path map.
- [ ] T035 [US3] Run `cargo +stable test -p mikebom --bin mikebom scan_fs::package_db::rpm scan_fs::package_db::file_hashes` and confirm the 4 new tests pass.
- [ ] T036 [US3] Run gated rpm smoke verification (no smoke-test code addition required; just confirm a real rpm-image scan produces non-zero occurrences):
      ```bash
      mikebom sbom scan --image fedora:40 --output /tmp/fedora-040.cdx.json
      jq '[.components[].evidence.occurrences | length // 0] | add' /tmp/fedora-040.cdx.json
      # expected: > 0 (was 0 pre-040)
      ```
- [ ] T037 [US3] Run goldens regen: `MIKEBOM_UPDATE_*_GOLDENS=1 cargo +stable test -p mikebom --test '*'`. Confirm zero diff (the existing 27 goldens use `--no-deep-hash` so they're insulated).
- [ ] T038 [US3] `./scripts/pre-pr.sh` clean.
- [ ] T039 [US3] Commit: `feat(040/us3-rpm-deep-hash): rpm per-file evidence — completes the OS-package per-file-evidence trilogy`.

**Checkpoint**: All three user stories now ship together. The OS-package per-file-evidence work is complete across deb / apk / rpm.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T040 Update `docs/user-guide/cli-reference.md`: drop the apk `Z:`-line caveat from the milestone-039 paragraph (now closed); add a one-paragraph note that rpm components also produce per-file evidence (parallel to the deb / apk paragraphs).
- [ ] T041 Add `CHANGELOG.md` Unreleased entry summarizing milestone 040: US1 (stale-comment cleanup), US2 (apk SHA-1 cross-ref), US3 (rpm per-file deep-hash). Include pre/post numbers for US3 (alpine + chainguard already validated; quote the fedora:40 numbers from T036).
- [ ] T042 `./scripts/pre-pr.sh` clean.
- [ ] T043 Commit: `docs(040): user-guide rpm parity paragraph + CHANGELOG`.

---

## Phase 7: PR + verification

- [ ] T044 Push the branch.
- [ ] T045 Open PR titled `feat(040): package-db follow-ons — comment cleanup, apk SHA-1 cross-ref, rpm per-file deep-hash`. PR body lists pre/post numbers (US3: fedora:40 occurrence count), zero non-fixture-golden drift, no new top-level deps.
- [ ] T046 Verify all 3 CI lanes green (Linux default + Linux ebpf + macOS).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies; run first.
- **Foundational (Phase 2)**: Empty.
- **US1 (Phase 3)**: Depends on Setup. Independent of US2/US3.
- **US2 (Phase 4)**: Depends on Setup. Independent of US1/US3 — touches different files.
- **US3 (Phase 5)**: Depends on Setup. Independent of US1/US2 — touches different files.
- **Polish (Phase 6)**: Depends on completing whichever US's the maintainer wants in the current PR.
- **PR (Phase 7)**: Depends on Polish.

### Within US2

- T008 (struct) precedes T009 (signature change).
- T009 precedes T010 (allow-dead-code holds for the wire-up gap).
- T011–T013 are parallel inline tests against the new struct; can be authored simultaneously.
- T017 (mikebom-common field) precedes T018 (file_hashes update) precedes T021 (call-site update).
- T019 (CDX emitter) and T020 (SPDX emitters) are ordered logically but touch different files; can be authored simultaneously.

### Within US3

- T027 (rpm::read_file_lists) precedes T028 / T029.
- T028 + T029 are parallel — different functions in same file; the `[P]` marker isn't applied because by mikebom convention same-file edits are sequential, but they can be reviewed independently.
- T031–T034 are parallel inline tests (different test functions across two files).

### Parallel Opportunities

- **US1 + US2 + US3** can be developed independently. Recommended sequencing per spec: US1 → US2 → US3 (matches user's stated order).
- T011–T013 within US2: parallel test additions.
- T031–T034 within US3: parallel test additions across two files.

---

## Implementation Strategy

### MVP First (US1 only)

1. Phase 1 (T001–T002).
2. Phase 3 (T003–T007).
3. **STOP & VALIDATE**: confirm the grep-cleanup is complete; manually trigger an unmapped-arch error and confirm the new message reads well.
4. Optionally ship US1 standalone as a tiny PR. Given the trivial size, more likely just continue to US2.

### Recommended (single PR sequencing all three stories)

1. Phase 1 → US1 → US2 → US3 → Polish → PR.
2. Single PR with 5 commits visible in the review (one per US plus Polish), each individually reviewable.

### Stop conditions

- US3 fedora:40 scan produces 0 occurrences → debug: confirm the rpmdb is at one of the documented paths; confirm `is_rpm` substring matches `entry.source_path`. Most likely cause is a path-detection mismatch.
- Goldens regen produces non-zero diff → likely the apk SHA-1 has leaked into a fixture-test path. Inspect the diff; non-`additionalContext` field changes indicate a deeper issue.

---

## Notes

- `[P]` tasks = different files OR different test functions, no dependencies on incomplete tasks.
- `[Story]` label maps task to specific user story for traceability.
- Each commit's `./scripts/pre-pr.sh` MUST be clean (Constitution Pre-PR Verification gate).
- US1 is genuinely tiny — keep its commit small.
- US2 splits across two commits because the in-flight state (changing apk's API but not yet wiring it through) requires a `#[allow(dead_code)]`; clean separation makes review easier.
- US3 lands as a single commit since the helper + wire-up + tests are tightly coupled.
