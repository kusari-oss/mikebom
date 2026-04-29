---
description: "Task list — milestone 037 dpkg status.d/ reader"
---

# Tasks: dpkg `status.d/` reader

**Input**: spec.md (✅), plan.md (✅), checklists/requirements.md (✅).

**Tests**: 5 new inline tests in `dpkg.rs` per FR-005; the existing
distroless smoke test is renamed + assertion flipped.

**Organization**: Single user story (US1, P1). Two atomic commits.

## Path conventions

- Touches `mikebom-cli/src/scan_fs/package_db/dpkg.rs` (additive).
- Touches `mikebom-cli/tests/oci_registry_smoke.rs` (rename test +
  flip assertion).
- Touches `CHANGELOG.md`.
- Does NOT touch parity/, generate/, resolve/, attestation/, any
  other CLI command, or any other ecosystem reader.

---

## Phase 1: Setup + baseline

- [X] T001 Recon done: `dpkg.rs::read` at line 37; `parse_stanza` at line 135 is reusable as-is. 604-line file budget allows ~200 additions before SC-005's 800-LOC ceiling.
- [ ] T002 Snapshot baseline.

---

## Phase 2: Commit 1 — `037/status-d-reader`

**Goal**: `read()` covers both sources; new tests in place; smoke test renamed.

- [ ] T003 [US1] Add `const DPKG_STATUS_D_DIR: &str = "var/lib/dpkg/status.d";` near line 21 of dpkg.rs.
- [ ] T004 [US1] Add `fn read_status_d_dir(rootfs, namespace, distro_version) -> Vec<PackageDbEntry>`. Walks `<rootfs>/var/lib/dpkg/status.d/`, skips directories and any file with an extension (`.md5sums`, `.conffiles`, etc.), reads each file and feeds through `parse`. IO errors per-file log at tracing::debug and skip; never propagate.
- [ ] T005 [US1] Edit `dpkg.rs::read`: after the existing `status` file read, also call `read_status_d_dir` and concatenate. If both sources produce entries with the same `(package, version)` purl, the status.d/ entry wins (FR-003). Implement via dedup on `entry.purl` after concat — keep the LATER one (which is status.d/, since we append it second).

      Wait, dedup-keeping-later is awkward with Vec. Cleaner: build a HashMap<Purl, PackageDbEntry> and overwrite from status.d/. Or simpler: since collisions are pathological, use `Vec::dedup_by_key` + sort. Even simpler: build a HashSet of seen purls from status results, then only append status.d/ entries whose purl isn't in the set.

      Actually FR-003 says status.d/ wins. Cleanest: process status.d/ FIRST into a HashMap<Purl, _>, then process status entries — only insert if not already present. That way status.d/ wins.

      Final approach: build a HashMap keyed on `entry.purl`, processing status.d/ first then status, with `entry(...).or_insert(_)` semantics so status.d/ takes precedence.

- [ ] T006 [US1] Inline test `parses_status_d_only_layout`: tempdir with `var/lib/dpkg/status.d/foo` + `bar` (each one stanza, install ok installed). Assert `read()` returns 2 entries with names `foo` + `bar`.
- [ ] T007 [US1] Inline test `parses_mixed_status_and_status_d`: tempdir with `var/lib/dpkg/status` containing `pkg-a` AND `var/lib/dpkg/status.d/pkg-b`. Assert 2 entries, one from each source.
- [ ] T008 [US1] Inline test `status_d_filters_non_installed`: tempdir with `var/lib/dpkg/status.d/keep` (install ok installed) + `var/lib/dpkg/status.d/drop` (deinstall ok config-files). Assert only `keep` survives.
- [ ] T009 [US1] Inline test `status_d_skips_companion_files`: tempdir with `var/lib/dpkg/status.d/keep` + `var/lib/dpkg/status.d/keep.md5sums` (which is NOT a valid stanza). Assert 1 entry, no error from the companion file.
- [ ] T010 [US1] Inline test `status_d_empty_dir_returns_empty`: tempdir with empty `var/lib/dpkg/status.d/` directory. Assert 0 entries, no error.
- [ ] T011 [US1] Edit `mikebom-cli/tests/oci_registry_smoke.rs`: rename `pulls_distroless_static_and_emits_well_formed_sbom_with_zero_components` → `pulls_distroless_static_and_emits_dpkg_status_d_components`. Update assertion: components.len() >= 4 (was == 0); the set of component names contains `{"base-files", "media-types", "netbase", "tzdata"}`.
- [ ] T012 [US1] Verify: `cargo +stable test -p mikebom --bin mikebom scan_fs::package_db::dpkg` includes the 5 new tests and they pass.
- [ ] T013 [US1] `./scripts/pre-pr.sh` clean.
- [ ] T014 [US1] Commit: `feat(037/status-d-reader): scan /var/lib/dpkg/status.d/ for distroless / chainguard / Bazel-built minimal images`.

---

## Phase 3: Commit 2 — `037/changelog`

- [ ] T015 [US1] Edit `CHANGELOG.md`: unreleased entry — distroless / chainguard support; closes #64.
- [ ] T016 [US1] `./scripts/pre-pr.sh` clean.
- [ ] T017 [US1] Commit: `docs(037): CHANGELOG entry for distroless / chainguard dpkg coverage`.

---

## Phase 4: Verification + PR

- [ ] T018 SC-001: `./scripts/pre-pr.sh` clean.
- [ ] T019 SC-003: `MIKEBOM_UPDATE_*_GOLDENS=1 ./scripts/pre-pr.sh` produces zero diff.
- [ ] T020 SC-005: `wc -l mikebom-cli/src/scan_fs/package_db/dpkg.rs` ≤ 800.
- [ ] T021 Push branch; observe all 3 CI lanes green.
- [ ] T022 Open PR closing #64.
