---
description: "Task list — milestone 030 Mach-O codesign metadata"
---

# Tasks: Mach-O codesign metadata — Tighter Spec

**Input**: Design documents from `/specs/030-macho-codesign-metadata/`
**Prerequisites**: spec.md (✅), plan.md (✅), checklists/requirements.md (✅)

**Tests**: 8+ new inline parser tests in `macho.rs::tests` + 1
integration assertion bumped on `tests/scan_binary.rs` macOS lane +
holistic_parity continuing to pass + sbom_format_mapping_coverage
continuing to pass.

**Organization**: Single user story (US1, P1). Three atomic commits.

## Path Conventions

- Touches `mikebom-cli/src/scan_fs/binary/{macho,entry,scan}.rs`,
  `mikebom-cli/src/parity/extractors/{cdx,spdx2,spdx3,mod}.rs`
  (additive), `docs/reference/sbom-format-mapping.md` (additive),
  `mikebom-cli/tests/scan_binary.rs` (additive).
- Does NOT touch `mikebom-common/`, `mikebom-cli/src/cli/`,
  `mikebom-cli/src/resolve/`, `mikebom-cli/src/generate/`,
  `mikebom-cli/src/scan_fs/binary/{elf,pe,version_strings,cargo_auditable,linkage,packer,predicates,jdk_collapse,python_collapse}.rs`,
  or any `mikebom-cli/src/scan_fs/package_db/` file.

---

## Phase 1: Setup + baseline

- [X] T001 Recon done. Confirmed:
      - `LC_CODE_SIGNATURE = 0x1D` per
        `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/object-0.36.7/src/macho.rs:727`.
      - SuperBlob format documented at the Apple opensource
        Security project (cs_blobs.h). Big-endian framing.
      - CodeDirectory blob magic `0xfade0c02`; identifier at
        `identOffset`, flags as a u32 bitfield, team_id at
        `teamOffset` when CD version ≥ 0x20200.
      - Existing `binary/macho.rs::for_each_load_command` helper
        (milestone 024) already walks load commands; new parsers
        reuse it.
      - macOS CI lane already exists; `/bin/ls` is Apple-signed
        with team ID `EQHXZ8M8AV` and hardened-runtime flag.
- [ ] T002 Snapshot baseline: `./scripts/pre-pr.sh 2>&1 | tee /tmp/baseline-030.txt | grep -cE '^test [a-z_:]+ \.\.\. ok' > /tmp/baseline-030-count.txt`.

---

## Phase 2: Commit 1 — `030/parsers`

**Goal**: 3 public parsers + private SuperBlob/CodeDirectory
helper + flag-bit decoder + 8+ inline tests; dead-code allowed
for this commit only.

- [ ] T003 [US1] Add SuperBlob + CodeDirectory magic constants
      at the top of `binary/macho.rs`:
      ```rust
      const CSMAGIC_EMBEDDED_SIGNATURE: u32 = 0xfade0cc0;
      const CSMAGIC_CODEDIRECTORY:      u32 = 0xfade0c02;
      // Other magics referenced for skip purposes:
      const CSMAGIC_REQUIREMENTS:       u32 = 0xfade0c01;
      const CSMAGIC_BLOBWRAPPER:        u32 = 0xfade0b01;
      const CSMAGIC_EMBEDDED_ENTITLEMENTS: u32 = 0xfade7171;
      ```
- [ ] T004 [US1] Add private helper `parse_codesign_codedirectory(bytes:
      &[u8]) -> Option<&[u8]>` that walks: load commands → find
      LC_CODE_SIGNATURE (0x1D) → read LinkeditDataCommand
      (cmd/cmdsize/dataoff/datasize) → bounds-check dataoff +
      datasize against `bytes.len()` → read 12 bytes at dataoff
      (BE u32 magic, BE u32 length, BE u32 count) → verify magic
      == CSMAGIC_EMBEDDED_SIGNATURE → walk count × 8 bytes of
      (type: BE u32, offset: BE u32) → for each entry whose blob
      magic at `dataoff + entry_offset` is CSMAGIC_CODEDIRECTORY
      → return a slice into the blob bytes. Returns None on any
      malformed input.
- [ ] T005 [US1] Add `pub fn parse_codesign_identifier(bytes:
      &[u8]) -> Option<String>`. Calls
      `parse_codesign_codedirectory`; reads the CD `identOffset`
      field (BE u32 at offset 20 within the CD blob) → reads
      a NUL-terminated UTF-8 string at that offset within the
      blob → returns the string. Returns None on any failure.
- [ ] T006 [US1] Add `pub fn parse_codesign_flags(bytes: &[u8])
      -> Vec<String>`. Calls `parse_codesign_codedirectory`;
      reads the CD `flags` field (BE u32 at offset 12 within
      the CD blob) → calls a private `decode_codesign_flags(u32)
      -> Vec<String>` decoder → returns the resulting Vec
      (alphabetically sorted for determinism). Returns empty Vec
      on failure.
- [ ] T007 [US1] Add `pub fn parse_codesign_team_id(bytes:
      &[u8]) -> Option<String>`. Calls
      `parse_codesign_codedirectory`; reads the CD `version`
      field (BE u32 at offset 8 within the CD blob); if version
      < 0x20200, returns None (older CDs don't carry teamOffset).
      Else reads the CD `teamOffset` field (BE u32 at offset 48
      within the CD blob — derived from Apple's CodeDirectory
      v2.0.x layout) → reads NUL-terminated UTF-8 string at
      that offset within the blob → returns the string. Returns
      None on any failure (including teamOffset == 0).
- [ ] T008 [US1] Add private `fn decode_codesign_flags(value: u32)
      -> Vec<String>`. Maps the canonical Apple bit names:
      ```
      0x00000001 → "host"           (CS_VALID-ish; rarely emit)
      0x00000002 → "adhoc"
      0x00000004 → "get-task-allow"
      0x00000008 → "installer"
      0x00000010 → "force-hard"
      0x00000020 → "force-kill"
      0x00000040 → "force-expiration"
      0x00000080 → "restrict"
      0x00000100 → "enforcement"
      0x00000200 → "library-validation"
      0x00010000 → "hardened-runtime"  (Apple calls it RUNTIME)
      0x00020000 → "linker-signed"
      ```
      Unrecognized bits → emit `format!("unknown-0x{:x}", bit)`.
      Returns alphabetically sorted Vec for determinism.
- [ ] T009 [US1] Add inline tests in `#[cfg(test)] mod tests`:
      - `parse_codesign_identifier_from_synthetic_superblob`
      - `parse_codesign_flags_decodes_hardened_runtime` (0x10000
        → `["hardened-runtime"]`)
      - `parse_codesign_flags_handles_multi_flag_bitfield`
        (0x10100 → `["hardened-runtime", "library-validation"]`
        sorted)
      - `parse_codesign_flags_emits_unknown_for_unrecognized_bits`
        (0x80000000 → `["unknown-0x80000000"]`)
      - `parse_codesign_team_id_skips_when_cd_version_too_old`
        (0x20100 → None)
      - `parse_codesign_team_id_extracts_when_cd_version_supports_it`
        (0x20400 + populated teamOffset → expected string)
      - `parse_codesign_returns_none_for_no_lc_code_signature`
      - `parse_codesign_returns_none_for_malformed_superblob_magic`
      Build synthetic Mach-O bytes with hand-crafted SuperBlobs
      via fixture-builder helpers (mirror the existing
      `build_minimal_macho` style from milestone 024).
- [ ] T010 [US1] Add `#[allow(dead_code)]` on the 3 public
      parsers + helpers (lifted in commit 2).
- [ ] T011 [US1] Verify: `cargo +stable test -p mikebom --bin
      mikebom scan_fs::binary::macho` includes the new tests +
      they pass. `./scripts/pre-pr.sh` clean.
- [ ] T012 [US1] Commit: `feat(030/parsers): add Mach-O codesign identifier, flags, and team-ID readers`.

---

## Phase 3: Commit 2 — `030/wire-up-bag`

**Goal**: BinaryScan gains 3 fields; scan.rs populates them on
both Mach-O paths; entry.rs translates to bag entries; macOS-lane
integration test bumped.

- [ ] T013 [US1] Edit `binary/entry.rs::BinaryScan`: add
      `pub macho_codesign_identifier: Option<String>`,
      `pub macho_codesign_flags: Vec<String>`,
      `pub macho_codesign_team_id: Option<String>` after the
      existing macho_min_os field. Doc comments naming
      LC_CODE_SIGNATURE + the fat-binary first-slice convention.
- [ ] T014 [US1] Update the 4 BinaryScan struct-literal sites:
      - `scan.rs` non-fat path: when `class == "macho"`, call
        the 3 FR-001 parsers and populate the fields.
      - `scan.rs` fat-Mach-O path: same parsers against the
        first slice's bytes (per 024 convention).
      - `entry.rs::tests::fake_binary_scan`: defaults
        (None / Vec::new() / None).
- [ ] T015 [US1] Edit `entry.rs::build_macho_identity_annotations`:
      extend to emit the 3 new bag keys with skip-on-empty:
      - `mikebom:macho-codesign-identifier` ← Value::String(id)
        if Some
      - `mikebom:macho-codesign-flags` ← serde_json::json!(vec)
        if non-empty
      - `mikebom:macho-codesign-team-id` ← Value::String(team)
        if Some
- [ ] T016 [US1] Remove `#[allow(dead_code)]` from the 3 parsers
      and helper(s) in macho.rs.
- [ ] T017 [US1] Edit `mikebom-cli/tests/scan_binary.rs::scan_system_binary_emits_file_level_and_linkage`:
      under the existing `class == "macho"` branch, add:
      - `mikebom:macho-codesign-identifier` is Some + non-empty.
      - `mikebom:macho-codesign-team-id` matches the regex
        `^[A-Z0-9]{10}$` (10-char Apple Team ID).
      - `mikebom:macho-codesign-flags` is parseable as a JSON
        array AND contains at least `"hardened-runtime"` (Apple
        has shipped HR on every system binary since macOS 10.14).
- [ ] T018 [US1] Verify: `cargo +stable test -p mikebom --test
      scan_binary` green (locally on macOS or via the CI
      macOS lane). `./scripts/pre-pr.sh` clean.
- [ ] T019 [US1] Commit: `feat(030/wire-up-bag): populate Mach-O codesign metadata into the extra_annotations bag`.

---

## Phase 4: Commit 3 — `030/parity-rows`

**Goal**: 3 new catalog rows + per-format extractors + EXTRACTORS rows.

- [ ] T020 [US1] Edit `docs/reference/sbom-format-mapping.md`:
      add 3 C-section rows (C37/C38/C39 — next available after
      milestone 029's C36). Each `Present` × 3 formats ×
      `SymmetricEqual`. Justification: from Mach-O
      LC_CODE_SIGNATURE → SuperBlob → CodeDirectory; identifier
      / flags / team ID. Cross-link to specs/030.
- [ ] T021 [US1] Edit `mikebom-cli/src/parity/extractors/cdx.rs`:
      add 3 `cdx_anno!` invocations after the C36 block:
      ```rust
      cdx_anno!(c37_cdx, "mikebom:macho-codesign-identifier", component);
      cdx_anno!(c38_cdx, "mikebom:macho-codesign-flags",      component);
      cdx_anno!(c39_cdx, "mikebom:macho-codesign-team-id",    component);
      ```
- [ ] T022 [US1] Edit `mikebom-cli/src/parity/extractors/spdx2.rs`:
      add 3 mirror `spdx23_anno!` invocations.
- [ ] T023 [US1] Edit `mikebom-cli/src/parity/extractors/spdx3.rs`:
      add 3 mirror `spdx3_anno!` invocations.
- [ ] T024 [US1] Edit `mikebom-cli/src/parity/extractors/mod.rs::EXTRACTORS`:
      add 3 new `ParityExtractor` rows + 9 fn imports.
- [ ] T025 [US1] Verify: `cargo +stable test -p mikebom --test
      holistic_parity` green. `cargo +stable test -p mikebom
      --test sbom_format_mapping_coverage` green.
- [ ] T026 [US1] `./scripts/pre-pr.sh` clean.
- [ ] T027 [US1] Commit: `feat(030/parity-rows): wire Mach-O codesign metadata into the holistic-parity matrix`.

---

## Phase 5: Verification

- [ ] T028 SC-001 verification: 4 standard gates green.
- [ ] T029 SC-002 verification (macOS CI lane): `/bin/ls` scan
      emits `mikebom:macho-codesign-identifier = "com.apple.ls"`
      AND `mikebom:macho-codesign-team-id = "EQHXZ8M8AV"` AND
      `mikebom:macho-codesign-flags` contains
      `"hardened-runtime"`.
- [ ] T030 SC-003 verification: `git diff main..HEAD --
      mikebom-cli/src/scan_fs/binary/{elf,pe,version_strings,cargo_auditable,linkage,packer,predicates,jdk_collapse,python_collapse}.rs`
      empty.
- [ ] T031 SC-004 verification: `wc -l mikebom-cli/src/scan_fs/binary/macho.rs`
      ≤ 700.
- [ ] T032 SC-005 verification: `git diff main..HEAD --
      mikebom-common/ mikebom-cli/src/cli/ mikebom-cli/src/resolve/
      mikebom-cli/src/generate/ mikebom-cli/src/scan_fs/package_db/`
      empty. **6th amortization-proof consumer.**
- [ ] T033 SC-007 verification: 27-golden regen
      (`MIKEBOM_UPDATE_*_GOLDENS=1`) produces zero diff.
- [ ] T034 SC-008 verification: `cargo tree -p mikebom` shows
      no new crate additions.
- [ ] T035 Push branch; observe all 3 CI lanes green (SC-006).
      The macOS lane is the SC-002 anchor.
- [ ] T036 Author the PR description: 3-commit summary,
      6th-consumer bag-amortization attestation, Apple Team ID
      anchoring on macOS lane, byte-identity attestation, scope-
      deferral note (entitlements XML + CMS PKCS#7).

---

## Dependency graph

```text
T001-T002 (recon + baseline, recon done)
   │
   ↓
T003-T012 [Commit 1: parsers + helpers + 8+ tests]
   │
   ↓
T013-T019 [Commit 2: wire-up-bag + macOS-lane integration test]
   │
   ↓
T020-T027 [Commit 3: parity-rows]
   │
   ↓
T028-T036 (verification + PR)
```

## Estimated effort

| Phase | Effort | Notes |
|---|---|---|
| Phase 1 (baseline) | 5 min | T001 done; just snapshot |
| Phase 2 (parsers) | 3 hr | byte-level SuperBlob walk; fixture-builder for synthetic SuperBlobs |
| Phase 3 (wire-up + macOS test) | 2 hr | 4 BinaryScan literal sites + bag emission + macOS-lane anchoring |
| Phase 4 (parity rows) | 30 min | mechanical |
| Phase 5 (verify + PR) | 1 hr | golden regen + CI watch |
| **Total** | **~6-7 hr** | comparable to milestone 024. |
