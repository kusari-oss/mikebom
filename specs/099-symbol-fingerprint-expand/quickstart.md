# Quickstart — milestone 099 maintainer recipes

Three recipes for landing the symbol-fingerprint table expansion. Total estimated implementation time: ~30 min single-developer.

## Recipe 1 — Add the documented-omission comment block + 4 new fingerprint rows (FR-001, FR-002, FR-004)

Open `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs`. Above the existing `const FINGERPRINTS: &[SymbolFingerprint] = &[` line (around line 39 per current code), insert the comment block from `data-model.md §symbol_fingerprint.rs — extension shape` documenting the 4 omissions.

Then append the 4 new rows (sqlite, pcre, pcre2, gnutls) to the `FINGERPRINTS` array — full symbol lists per `data-model.md`.

Compile-check: `cargo +stable check -p mikebom`.

## Recipe 2 — Add 5 new unit tests (FR-010)

Append to the existing `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs::tests` module. 5 tests per `data-model.md §symbol_fingerprint.rs — extension`:
- `sqlite_full_set_matches`
- `pcre_full_set_matches`
- `pcre2_full_set_matches`
- `gnutls_full_set_matches`
- `sqlite_seven_of_ten_below_threshold`

Run:

```bash
cargo +stable test -p mikebom --bin mikebom \
    --no-fail-fast sqlite_ pcre_ pcre2_ gnutls_ 2>&1 | grep "test result:"
# Expected: ok. 5 passed (+ any matching existing test names; verify the 5 new ones report ok).
```

## Recipe 3 — Extend mikebom-self regression test for the 4 new libraries (SC-006)

The existing `mikebom-cli/tests/binary_id_enrich.rs::mikebom_itself_does_not_emit_spurious_symbol_fingerprints` test currently checks for `pkg:generic/openssl`, `pkg:generic/zlib`, `pkg:generic/libcurl`. Extend the assertion to also forbid `pkg:generic/sqlite`, `pkg:generic/pcre`, `pkg:generic/pcre2`, `pkg:generic/gnutls`.

Specifically, the existing test's `matches!` arm:
```rust
matches!(purl, "pkg:generic/openssl" | "pkg:generic/zlib" | "pkg:generic/libcurl")
```
becomes:
```rust
matches!(purl,
    "pkg:generic/openssl" | "pkg:generic/zlib" | "pkg:generic/libcurl"
        | "pkg:generic/sqlite" | "pkg:generic/pcre" | "pkg:generic/pcre2"
        | "pkg:generic/gnutls"
)
```

Run:

```bash
cargo +stable test -p mikebom --test binary_id_enrich \
    mikebom_itself_does_not_emit_spurious_symbol_fingerprints 2>&1 | tail -5
# Expected: ok. 1 passed (mikebom uses rustls → no sqlite/pcre/gnutls
# symbols exported → the assertion still passes).
```

## Recipe 4 — Run pre-PR gate + verify diff scope (Contract 6, Contract 7)

```bash
MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1 ./scripts/pre-pr.sh
# Expected: `>>> all pre-PR checks passed.`

# Diff scope (Contract 6):
git diff --name-only main | sort
# Expected:
#   CLAUDE.md                                              (auto-updated)
#   mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs
#   mikebom-cli/tests/binary_id_enrich.rs                  (mikebom-self guard)
#   specs/099-symbol-fingerprint-expand/...

# No Cargo.* changes (FR-005):
git diff --name-only main | grep -E '^Cargo\.(lock|toml)$' && echo "DEP CHURN" || echo "clean"
# Expected: clean

# Goldens regen scope (FR-009 / SC-007):
git diff --stat mikebom-cli/tests/fixtures/golden/ | tail -1
# Expected: empty
```

## When in doubt

- **A new library is added to the milestone-026 version-string scanner without a fingerprint here**: the binary-id-enrich pipeline continues to work — version-string emission handles that library. The fingerprint scanner stays quiet. Add a fingerprint row in a follow-up milestone when both signals are valuable.
- **A library's public-API prefix evolves** (e.g., a hypothetical SQLite 4.x that switches from `sqlite3_*` to `sqlite4_*`): the existing fingerprint stops matching. Detection of the new major version is a separate milestone's concern — the v1 fingerprint targets long-stable APIs (SQLite 3.x has shipped since 2004 with stable symbol names).
- **A 16-bit / 32-bit PCRE2 fingerprint becomes interesting**: per research §3, extend the `pcre2` row's symbol list OR add separate `pcre2-16` / `pcre2-32` rows. The existing slug naming convention can accommodate either approach.
- **The mikebom-self regression test starts failing on a future toolchain that statically links one of these libraries**: that's a real signal — mikebom now depends on the matched library. Update the assertion list AND document the dependency change.
- **A binary trips two fingerprints because a custom library happens to share 8+ symbol names with a tracked library**: vanishingly unlikely at the chosen prefix-distinctiveness levels. If it ever happens, narrow the offending fingerprint by replacing its lowest-distinctiveness symbol with a more-distinctive alternative.
