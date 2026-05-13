---
description: "Task list for milestone 099 — symbol-fingerprint table expansion to 7 libraries"
---

# Tasks: Symbol-fingerprint table expansion

**Input**: Design documents from `/Users/mlieberman/Projects/mikebom/specs/099-symbol-fingerprint-expand/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/fingerprint-expansion-contracts.md, quickstart.md

**Tests**: Included. 5 new unit tests per data-model.md (4 happy-path + 1 under-threshold) + 1 extension to the existing milestone-096 mikebom-self regression test.

**Organization**: Three user stories each add 1-2 library fingerprints to the same file (`symbol_fingerprint.rs`). The single-file convergence means US1 → US2 → US3 land sequentially within the file; tests within each user story remain parallel-safe. Documented-omission comment block is the only foundational task (Phase 2).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files OR different test functions in the same file)
- **[Story]**: User story this task belongs to (US1–US3)
- File paths are workspace-relative.

## Path Conventions

Production code under `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs` (extends milestone-096's FINGERPRINTS const table). One regression-test extension in `mikebom-cli/tests/binary_id_enrich.rs`. Zero changes outside these two files (FR-006).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify environment + confirm preconditions before touching production code.

- [X] T001 Confirm working branch is `099-symbol-fingerprint-expand`. Run `git status` + `git log -1 --oneline`; verify branch was created by `/speckit-specify` and main is at post-PR-#206 (alpha.32 release) or later.
- [X] T002 Confirm baseline pre-PR gate passes. Run `./scripts/pre-pr.sh` once on the unchanged tree; expect `>>> all pre-PR checks passed.` Isolates any post-edit failure as introduced by milestone 099.
- [X] T003 Audit existing `symbol_fingerprint.rs` + verify slug consistency per research §4. Run:
    ```bash
    grep -nE 'CuratedLibrary::Sqlite|CuratedLibrary::Pcre|CuratedLibrary::GnuTls|=> "sqlite"|=> "pcre"|=> "gnutls"' \
        mikebom-cli/src/scan_fs/binary/version_strings.rs | head -6
    grep -nE 'const FINGERPRINTS|struct SymbolFingerprint' \
        mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs | head -3
    grep -nE 'matches!\(purl,' mikebom-cli/tests/binary_id_enrich.rs | head -3
    ```
    Expected: `=> "sqlite"`, `=> "pcre"`, `=> "pcre2"`, `=> "gnutls"` all present in `CuratedLibrary::slug()`; `const FINGERPRINTS` defined in `symbol_fingerprint.rs`; existing `matches!(purl, ...)` lookup in `binary_id_enrich.rs::mikebom_itself_does_not_emit_spurious_symbol_fingerprints` for openssl/zlib/libcurl.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Add the documented-omission `//` comment block above the `FINGERPRINTS` array. This is a single one-shot edit that explains the deliberate exclusions (boringssl, libressl, llvm, openjdk) for all three user stories' future maintainers. Lands once; benefits all three USs.

- [X] T004 Add documented-omission comment block immediately above `const FINGERPRINTS: &[SymbolFingerprint] = &[` in `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs` per `data-model.md §symbol_fingerprint.rs — extension shape` + `research.md §6`. The block names each of the 4 omissions (boringssl, libressl, llvm, openjdk) with per-library rationale. Format: `//` comments, no doc-comment markers (the docs already exist on the `SymbolFingerprint` struct above).

**Checkpoint**: After T004, the omission rationale lives in-source for future maintainer visibility. The FINGERPRINTS table itself is unchanged; US1/US2/US3 each add their respective rows in the user-story phases that follow.

---

## Phase 3: User Story 1 — SQLite fingerprint (Priority: P1) 🎯 MVP

**Goal**: Add the `sqlite` row to `FINGERPRINTS`. After landing US1, an ELF binary that statically links SQLite (with `.dynsym` retained but version string stripped) emits `pkg:generic/sqlite` with `mikebom:evidence-kind = symbol-fingerprint`.

**Independent Test**: synthesize a symbol set containing all 10 SQLite fingerprint symbols; call `symbol_fingerprint::scan()`; expect one match with `library = "sqlite"`, `matched_count = 10`, `total_count = 10`. Plus the under-threshold test (7/10) verifies the 8-of-10 gate.

### Implementation for User Story 1

- [X] T005 [US1] Append the SQLite row to `FINGERPRINTS` in `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs` per `data-model.md §symbol_fingerprint.rs — extension shape`. 10 symbols: `sqlite3_open`, `sqlite3_close`, `sqlite3_exec`, `sqlite3_prepare_v2`, `sqlite3_step`, `sqlite3_finalize`, `sqlite3_bind_int`, `sqlite3_column_text`, `sqlite3_errmsg`, `sqlite3_libversion`. `required_symbol_count: 8`.

### Tests for User Story 1

- [X] T006 [P] [US1] Add unit test `sqlite_full_set_matches` to `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs::tests` per `data-model.md`. Synthesizes all 10 SQLite symbols, calls `scan()`, asserts one match with `library == "sqlite"` and `matched_count == 10`.
- [X] T007 [P] [US1] Add unit test `sqlite_seven_of_ten_below_threshold` to `symbol_fingerprint::tests` per `data-model.md`. Synthesizes only 7 of 10 SQLite symbols, asserts `scan()` returns empty Vec (under-threshold sentinel — Contract 2).
- [X] T008 [US1] Verify Contracts 1 + 2 for SQLite from `contracts/fingerprint-expansion-contracts.md`. Run:
    ```bash
    cargo +stable test -p mikebom --bin mikebom \
        --no-fail-fast sqlite_full_set_matches \
        sqlite_seven_of_ten_below_threshold 2>&1 | grep "test result:"
    # Expected: ok. 2 passed.
    ```

**Checkpoint**: US1 complete. MVP — SQLite identification now works on stripped binaries.

---

## Phase 4: User Story 2 — PCRE 8.x + PCRE 10.x fingerprints (Priority: P2)

**Goal**: Add the `pcre` and `pcre2` rows to `FINGERPRINTS`. After landing US2, an ELF binary statically linking PCRE 8.x emits `pkg:generic/pcre`; a binary linking PCRE 10.x emits `pkg:generic/pcre2`. The two libraries are disambiguated by their distinct symbol prefixes (`pcre_*` vs `pcre2_*_8`).

**Independent Test**: synthesize the PCRE 8.x symbol set → match on `pcre` (not `pcre2`). Synthesize the PCRE 10.x set → match on `pcre2` (not `pcre`). Both libraries' fingerprints are independent at the symbol-prefix level.

### Implementation for User Story 2

- [X] T009 [US2] Append the PCRE 8.x + PCRE 10.x rows to `FINGERPRINTS` in `symbol_fingerprint.rs` per `data-model.md`. PCRE 8.x symbols: `pcre_compile`, `pcre_exec`, `pcre_free`, `pcre_study`, `pcre_get_substring`, `pcre_version`, `pcre_fullinfo`, `pcre_compile2`, `pcre_dfa_exec`, `pcre_jit_exec`. PCRE 10.x symbols (8-bit width only per research §3): `pcre2_compile_8`, `pcre2_match_8`, `pcre2_match_data_create_8`, `pcre2_substring_get_byname_8`, `pcre2_substring_get_bynumber_8`, `pcre2_get_ovector_pointer_8`, `pcre2_code_free_8`, `pcre2_match_data_free_8`, `pcre2_compile_context_create_8`, `pcre2_set_compile_extra_options_8`. Both with `required_symbol_count: 8`. Include an inline `//` comment on the pcre2 row noting "8-bit width variant only; 16/32-bit deferred per research §3".

### Tests for User Story 2

- [X] T010 [P] [US2] Add unit test `pcre_full_set_matches` to `symbol_fingerprint::tests` per `data-model.md`. Synthesizes all 10 PCRE 8.x symbols, asserts match on `library == "pcre"` with `matched_count == 10`.
- [X] T011 [P] [US2] Add unit test `pcre2_full_set_matches` to `symbol_fingerprint::tests` per `data-model.md`. Synthesizes all 10 PCRE 10.x symbols, asserts match on `library == "pcre2"` with `matched_count == 10`.
- [X] T012 [US2] Verify Contracts 1 + 3 for PCRE from `contracts/fingerprint-expansion-contracts.md`. Run:
    ```bash
    cargo +stable test -p mikebom --bin mikebom \
        --no-fail-fast pcre_full_set_matches \
        pcre2_full_set_matches 2>&1 | grep "test result:"
    # Expected: ok. 2 passed.

    # Implicit cross-match check: `pcre_full_set_matches` asserts
    # `library == "pcre"` not "pcre2" — the test body verifies the
    # disambiguation per Contract 3.
    ```

**Checkpoint**: US2 complete. PCRE 8.x and PCRE 10.x both identifiable on stripped binaries; the prefix-distinctiveness disambiguates the two libraries automatically.

---

## Phase 5: User Story 3 — GnuTLS fingerprint (Priority: P2)

**Goal**: Add the `gnutls` row to `FINGERPRINTS`. After landing US3, an ELF binary statically linking GnuTLS emits `pkg:generic/gnutls`.

**Independent Test**: synthesize the GnuTLS symbol set → match on `gnutls` with `matched_count == 10`.

### Implementation for User Story 3

- [X] T013 [US3] Append the GnuTLS row to `FINGERPRINTS` in `symbol_fingerprint.rs` per `data-model.md`. 10 symbols: `gnutls_init`, `gnutls_deinit`, `gnutls_handshake`, `gnutls_record_send`, `gnutls_record_recv`, `gnutls_global_init`, `gnutls_global_deinit`, `gnutls_set_default_priority`, `gnutls_credentials_set`, `gnutls_session_set_ptr`. `required_symbol_count: 8`.

### Tests for User Story 3

- [X] T014 [P] [US3] Add unit test `gnutls_full_set_matches` to `symbol_fingerprint::tests` per `data-model.md`. Synthesizes all 10 GnuTLS symbols, asserts match on `library == "gnutls"` with `matched_count == 10`.
- [X] T015 [US3] Verify Contract 1 for GnuTLS from `contracts/fingerprint-expansion-contracts.md`. Run:
    ```bash
    cargo +stable test -p mikebom --bin mikebom \
        --no-fail-fast gnutls_full_set_matches 2>&1 | grep "test result:"
    # Expected: ok. 1 passed.
    ```

**Checkpoint**: US3 complete. The 4 new libraries (sqlite/pcre/pcre2/gnutls) all emit identification from symbol-fingerprint scanning on stripped binaries.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Extend the existing milestone-096 mikebom-self regression test to guard against future spurious matches on the 4 new libraries; diff-scope audit; pre-PR gate.

- [X] T016 [P] Extend the assertion list in `mikebom-cli/tests/binary_id_enrich.rs::mikebom_itself_does_not_emit_spurious_symbol_fingerprints` per `quickstart.md` Recipe 3. The existing `matches!(purl, ...)` arm currently checks for `pkg:generic/openssl`, `pkg:generic/zlib`, `pkg:generic/libcurl`. Add `pkg:generic/sqlite`, `pkg:generic/pcre`, `pkg:generic/pcre2`, `pkg:generic/gnutls` to the same arm. Mikebom uses rustls (not OpenSSL/SQLite/PCRE/GnuTLS) so the extended assertion should continue to pass — closes SC-006's regression guard.
- [X] T017 Verify Contract 6 — diff scope guardrails. Run:
    ```bash
    # No new Cargo deps (FR-005 / SC-008):
    git diff --name-only main | grep -E '^Cargo\.(lock|toml)$' | wc -l
    # Expected: 0

    # Production code outside symbol_fingerprint.rs:
    git diff --name-only main | grep -E '^mikebom-cli/src/' \
      | grep -vE '^mikebom-cli/src/scan_fs/binary/symbol_fingerprint\.rs$' \
      | wc -l
    # Expected: 0

    # Golden regen scope (FR-009 / SC-007):
    git diff --stat mikebom-cli/tests/fixtures/golden/ | tail -1
    # Expected: empty (no existing fixture binaries statically link sqlite/pcre/gnutls)

    # Diff scope allowlist:
    git diff --name-only main | sort
    # Expected:
    #   CLAUDE.md                                                  (auto-updated)
    #   mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs
    #   mikebom-cli/tests/binary_id_enrich.rs                      (regression test)
    #   specs/099-symbol-fingerprint-expand/...
    ```
- [X] T018 Run the mandatory pre-PR gate per Contract 7. Run `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1 ./scripts/pre-pr.sh`. Expect: `>>> all pre-PR checks passed.` with zero clippy warnings and zero test failures across the workspace. The 5 new symbol_fingerprint tests pass; the existing 3 milestone-096 tests continue to pass; the extended `binary_id_enrich.rs` mikebom-self test passes (mikebom doesn't link any of the 7 tracked libraries).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies. Start immediately.
- **Foundational (Phase 2)**: T004 adds the omission comment block — non-blocking for the 3 USs (USs can land their rows before or after T004; landing T004 first is just cleaner for code-review).
- **US1 (Phase 3, P1, MVP)**: Independent at file level. Adds 1 row + 2 tests.
- **US2 (Phase 4, P2)**: Independent at file level. Adds 2 rows + 2 tests. Soft-depends on US1 having appended its row first only for diff-locality (same file region).
- **US3 (Phase 5, P2)**: Independent at file level. Adds 1 row + 1 test.
- **Polish (Phase 6)**: Depends on US1+US2+US3 implementation tasks complete (so all 4 new libraries are in the table when T016 extends the regression test).

### User Story Dependencies

- **US1 (P1)**: Independent at file level. T005 appends one row.
- **US2 (P2)**: Independent at file level. T009 appends two rows.
- **US3 (P2)**: Independent at file level. T013 appends one row.

(The 3 user stories share the same const-table region in `symbol_fingerprint.rs`. Diff-wise they land in sequence within the file; behavior-wise each is independently testable.)

### Within Each User Story

- US1: T005 (impl) → T006-T007 (tests, parallel-safe) → T008 (verify).
- US2: T009 (impl, 2 rows) → T010-T011 (tests, parallel-safe) → T012 (verify).
- US3: T013 (impl) → T014 (test) → T015 (verify).

### Parallel Opportunities

- T006 / T007 / T010 / T011 / T014 — 5 parallel-safe test additions across all three stories (different test functions, same module file — Rust allows multiple functions in one file without ordering constraints).
- T016 (regression test) is parallel-safe with T017 (diff audit) once Phase 5 is complete.

---

## Parallel Example: Phase 3-5 unit tests

```bash
# After T005, T009, T013 land (the 4 const-table rows), all 5 unit
# tests can be added in any order:
Task: "Add unit test sqlite_full_set_matches (T006)"
Task: "Add unit test sqlite_seven_of_ten_below_threshold (T007)"
Task: "Add unit test pcre_full_set_matches (T010)"
Task: "Add unit test pcre2_full_set_matches (T011)"
Task: "Add unit test gnutls_full_set_matches (T014)"
```

---

## Implementation Strategy

### MVP First (US1 only)

The user's headline ask is "expand symbol-fingerprint coverage". SQLite is the most-statically-linked C library in CLI tooling — covering it alone moves the needle measurably. MVP path:

1. Phase 1: Setup (T001-T003)
2. Phase 2: Foundational (T004 omission comment block)
3. Phase 3: US1 (T005-T008) — SQLite row + 2 tests + verify
4. Phase 6 partial: T016 (regression test extension for sqlite only) + T018 (pre-PR gate)
5. **STOP and VALIDATE**: scan a statically-linked SQLite binary if available; confirm `pkg:generic/sqlite` appears.

US2 + US3 layer on after MVP-validation. The full milestone delivers all 4 libraries in a single PR.

### Incremental Delivery (recommended)

Single PR shipping all three stories — the surface (~110 lines new code) is small enough that splitting adds noise. Total estimated time: ~30 min single-developer.

### Single-Developer Strategy

1. T001-T003 (setup, ~3 min)
2. T004 (omission comment block, ~2 min)
3. T005-T008 (US1 sqlite, ~7 min — row + 2 tests + verify)
4. T009-T012 (US2 pcre + pcre2, ~10 min — 2 rows + 2 tests + verify)
5. T013-T015 (US3 gnutls, ~5 min — row + 1 test + verify)
6. T016-T018 (Polish, ~5 min — regression-test extension + diff audit + pre-PR gate)

Total: ~32 minutes single-developer focus.

---

## Notes

- [P] markers = different test functions OR different files with no shared edit-dependency.
- [Story] label maps task to user story for traceability.
- All 3 USs converge on the same `symbol_fingerprint.rs` file but at distinct row-positions in the `FINGERPRINTS` table; each US's row addition is an independent diff hunk.
- T004 (omission comment block) is foundational because it documents WHY libraries aren't in the table — it doesn't add functionality. Could land in US1 if a strict per-US phase is preferred; treating it as foundational separates the documentation work from the per-library code-add work for cleaner code review.
- Pre-PR gate (T018) MUST run with `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1` per CLAUDE.md SBOM-spec-touching-changes rule. Even though this milestone touches no emission paths, the workspace-wide test gate is the safety net.
- Commit boundary suggestion: single commit (US1+US2+US3+Polish in one PR). Surface is small enough that splitting adds noise.
- Avoid: tuning the 8/10 threshold per library (FR-007 says uniform across all 7), expanding the table beyond the 4 listed libraries (per spec out-of-scope), or adding PE/Mach-O symbol fingerprinting (per spec out-of-scope).
