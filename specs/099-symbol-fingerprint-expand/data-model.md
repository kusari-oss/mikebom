# Data Model — milestone 099

Single-file delta to `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs`. The `FINGERPRINTS` const table grows from 3 → 7 entries; a new comment block documents 4 deliberate omissions; 5 new unit tests verify the additions.

## File inventory

| File | State | Owner FRs |
|------|-------|-----------|
| `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs` | EXTEND existing | FR-001, FR-002, FR-003, FR-004, FR-007, FR-010 |

That's it. No other production code touched, no new files.

## `symbol_fingerprint.rs` — extension shape

**New const-table entries** (appended to the existing `FINGERPRINTS` array, per research §1 + §2):

```rust
const FINGERPRINTS: &[SymbolFingerprint] = &[
    // ... existing milestone-096 entries: openssl, zlib, libcurl ...

    SymbolFingerprint {
        library_name: "sqlite",
        symbols: &[
            "sqlite3_open",
            "sqlite3_close",
            "sqlite3_exec",
            "sqlite3_prepare_v2",
            "sqlite3_step",
            "sqlite3_finalize",
            "sqlite3_bind_int",
            "sqlite3_column_text",
            "sqlite3_errmsg",
            "sqlite3_libversion",
        ],
        required_symbol_count: 8,
    },
    SymbolFingerprint {
        library_name: "pcre",
        symbols: &[
            "pcre_compile",
            "pcre_exec",
            "pcre_free",
            "pcre_study",
            "pcre_get_substring",
            "pcre_version",
            "pcre_fullinfo",
            "pcre_compile2",
            "pcre_dfa_exec",
            "pcre_jit_exec",
        ],
        required_symbol_count: 8,
    },
    SymbolFingerprint {
        library_name: "pcre2",
        // 8-bit width variant (`libpcre2-8`) — dominant in practice.
        // 16-bit / 32-bit variants are deferred per research §3.
        symbols: &[
            "pcre2_compile_8",
            "pcre2_match_8",
            "pcre2_match_data_create_8",
            "pcre2_substring_get_byname_8",
            "pcre2_substring_get_bynumber_8",
            "pcre2_get_ovector_pointer_8",
            "pcre2_code_free_8",
            "pcre2_match_data_free_8",
            "pcre2_compile_context_create_8",
            "pcre2_set_compile_extra_options_8",
        ],
        required_symbol_count: 8,
    },
    SymbolFingerprint {
        library_name: "gnutls",
        symbols: &[
            "gnutls_init",
            "gnutls_deinit",
            "gnutls_handshake",
            "gnutls_record_send",
            "gnutls_record_recv",
            "gnutls_global_init",
            "gnutls_global_deinit",
            "gnutls_set_default_priority",
            "gnutls_credentials_set",
            "gnutls_session_set_ptr",
        ],
        required_symbol_count: 8,
    },
];
```

**New comment block** (immediately above the `const FINGERPRINTS` declaration, per research §6):

```rust
// Documented omissions per milestone-099 spec §1 + FR-004. These
// libraries are intentionally NOT in the fingerprint table; future
// maintainers should consult the per-library rationale before adding
// them:
//
//   - boringssl: drop-in OpenSSL ABI replacement; symbol overlap with
//     openssl prevents reliable disambiguation at the symbol level.
//     Operators wanting fork identification rely on the version-string
//     scanner's `BoringSSL ` anchor (milestone 026).
//   - libressl: same reasoning — OpenBSD's OpenSSL fork shares ABI.
//     Version-string scanner's `LibreSSL ` anchor handles disambiguation.
//   - llvm: API surface too broad (hundreds of public-API entry points
//     across libLLVMCore / libLLVMAnalysis / ...); no stable 10-symbol
//     slice. Different mikebom releases would pick different slices.
//     Defer until a versioned compiler-libs strategy emerges.
//   - openjdk: the launcher binary doesn't statically link JDK APIs
//     (those live in libjvm.so loaded via JNI). Symbol fingerprinting
//     at the launcher level yields no signal. Defer indefinitely.
```

**New unit tests** (5 — appended to the existing `symbol_fingerprint::tests` module per research §5):

```rust
#[test]
fn sqlite_full_set_matches() {
    let s = syms(&[
        "sqlite3_open", "sqlite3_close", "sqlite3_exec",
        "sqlite3_prepare_v2", "sqlite3_step", "sqlite3_finalize",
        "sqlite3_bind_int", "sqlite3_column_text", "sqlite3_errmsg",
        "sqlite3_libversion",
    ]);
    let hits = scan(&s);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].library, "sqlite");
    assert_eq!(hits[0].matched_count, 10);
    assert_eq!(hits[0].total_count, 10);
}

#[test]
fn pcre_full_set_matches() {
    let s = syms(&[
        "pcre_compile", "pcre_exec", "pcre_free", "pcre_study",
        "pcre_get_substring", "pcre_version", "pcre_fullinfo",
        "pcre_compile2", "pcre_dfa_exec", "pcre_jit_exec",
    ]);
    let hits = scan(&s);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].library, "pcre");
    assert_eq!(hits[0].matched_count, 10);
}

#[test]
fn pcre2_full_set_matches() {
    let s = syms(&[
        "pcre2_compile_8", "pcre2_match_8", "pcre2_match_data_create_8",
        "pcre2_substring_get_byname_8", "pcre2_substring_get_bynumber_8",
        "pcre2_get_ovector_pointer_8", "pcre2_code_free_8",
        "pcre2_match_data_free_8", "pcre2_compile_context_create_8",
        "pcre2_set_compile_extra_options_8",
    ]);
    let hits = scan(&s);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].library, "pcre2");
    assert_eq!(hits[0].matched_count, 10);
}

#[test]
fn gnutls_full_set_matches() {
    let s = syms(&[
        "gnutls_init", "gnutls_deinit", "gnutls_handshake",
        "gnutls_record_send", "gnutls_record_recv", "gnutls_global_init",
        "gnutls_global_deinit", "gnutls_set_default_priority",
        "gnutls_credentials_set", "gnutls_session_set_ptr",
    ]);
    let hits = scan(&s);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].library, "gnutls");
    assert_eq!(hits[0].matched_count, 10);
}

#[test]
fn sqlite_seven_of_ten_below_threshold() {
    // 7 of 10 < threshold 8 → no match.
    let s = syms(&[
        "sqlite3_open", "sqlite3_close", "sqlite3_exec",
        "sqlite3_prepare_v2", "sqlite3_step", "sqlite3_finalize",
        "sqlite3_bind_int",
    ]);
    assert!(scan(&s).is_empty());
}
```

**No changes** to:
- `SymbolFingerprint` struct definition (existing fields are sufficient).
- `scan()` function signature or body (the existing loop iterates `FINGERPRINTS` and applies the threshold uniformly — growing the table doesn't change the loop).
- `pub use` exports.
- The `syms()` test helper (existing helper handles arbitrary string slices).

## Compatibility

- **No `Cargo.lock` change** — pure in-source const-table addition.
- **Goldens regen forecast** — at most ≤1 spurious match across the 9 existing ecosystem fixtures (per milestone-096 SC-007 bound). Realistically zero: existing fixtures don't include real-world C-library-statically-linked binaries; the 4 new fingerprints are distinctive enough that even fortuitous symbol-name coincidence is improbable at 8/10 strength.
- **Backward compatibility** — 100% additive. Existing 3-library fingerprint set continues to emit unchanged. The new 4 entries only fire on binaries that actually statically link the corresponding library.
- **Mikebom-self regression**: the existing `binary_id_enrich.rs::mikebom_itself_does_not_emit_spurious_symbol_fingerprints` integration test currently checks for spurious openssl/zlib/libcurl matches. It should be extended to also check the 4 new libraries — addressed in tasks.md (Phase 2).

## No JSON / no YAML schema additions

Zero new fields in any output schema. The new fingerprints emit through the existing milestone-096 `mikebom:evidence-kind = symbol-fingerprint` + `mikebom:fingerprint-symbols-matched = N/10` annotation pipeline.

## No new parity-catalog rows

The 4 new libraries emit through the same existing parity-catalog rows that openssl/zlib/libcurl already use (the milestone-096 emission path). No Constitution V audit needed; no new `mikebom:*` properties introduced.
