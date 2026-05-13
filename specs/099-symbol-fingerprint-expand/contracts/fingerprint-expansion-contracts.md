# Contract — milestone 099 symbol-fingerprint expansion

Six behavioral contracts. Each specifies (a) the invariant the const-table addition holds, (b) a verification recipe.

## Contract 1 — Per-library full-set match (FR-001, FR-002, SC-001/SC-002/SC-003)

**Path**: `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs::FINGERPRINTS`.

**Invariant**: when a binary's `.dynsym` table contains all 10 symbols from any of the new library fingerprints (sqlite, pcre, pcre2, gnutls), `scan()` returns exactly one `SymbolFingerprintMatch { library: <slug>, matched_count: 10, total_count: 10 }` for that library — no spurious matches for other libraries.

**Verification**:
```bash
cargo +stable test -p mikebom --bin mikebom \
    --no-fail-fast sqlite_full_set_matches \
    pcre_full_set_matches \
    pcre2_full_set_matches \
    gnutls_full_set_matches 2>&1 | grep "test result:"
# Expected: ok. 4 passed.
```

## Contract 2 — Threshold enforcement (FR-007, SC-001 case 3)

**Path**: same as Contract 1.

**Invariant**: when a binary's `.dynsym` contains exactly 7 of a library's 10 fingerprint symbols (below the 8/10 threshold), `scan()` returns an empty Vec — no false claim of identification.

**Verification**:
```bash
cargo +stable test -p mikebom --bin mikebom \
    --no-fail-fast sqlite_seven_of_ten_below_threshold 2>&1 | grep "test result:"
# Expected: ok. 1 passed.
```

(One under-threshold test is sufficient — the threshold logic in `scan()` is shared across all libraries; if it works for SQLite at 7/10, it works for all 4 new libraries.)

## Contract 3 — PCRE 8.x vs PCRE 10.x disambiguation (US2)

**Path**: same as Contract 1.

**Invariant**: a binary exporting `pcre_*` symbols matches the `pcre` fingerprint (NOT `pcre2`); a binary exporting `pcre2_*_8` symbols matches the `pcre2` fingerprint (NOT `pcre`). The two prefix sets are disjoint at the symbol-name level, so disambiguation is automatic.

**Verification**: the existing `pcre_full_set_matches` and `pcre2_full_set_matches` tests verify each library matches its own fingerprint. To verify they DON'T cross-match, the test suite implicitly relies on the deterministic table iteration order in `scan()` — each fingerprint is independently scored, and the threshold check fires once per fingerprint.

```bash
# Spot-check: a PCRE 8.x binary's symbol set should NOT trigger
# `pcre2` because the threshold for pcre2 (8/10 of pcre2_*_8 symbols)
# is unmet with 0/10 pcre2 symbols present.
cargo +stable test -p mikebom --bin mikebom \
    --no-fail-fast pcre_full_set_matches 2>&1 | grep "test result:"
# Expected: ok. 1 passed.
# The matched library MUST be "pcre" (assertion in the test body).
```

## Contract 4 — Composite-evidence merge correctness (FR-008, SC-004)

**Path**: `mikebom-cli/src/scan_fs/binary/mod.rs::read()` — the milestone-096 Q1 merge loop. No code changes in this milestone — the existing merge keys on lowercase library name, and the v1 + v2 fingerprint slugs (sqlite, pcre, pcre2, gnutls) match the values from `CuratedLibrary::slug()`.

**Verification** (existing-code behavior):
```bash
# Confirm the lowercase slug naming match:
grep -nE '=> "(sqlite|pcre|pcre2|gnutls)"' \
    mikebom-cli/src/scan_fs/binary/version_strings.rs
# Expected: 4 matches (one per new library — already in CuratedLibrary::slug()).

# Confirm the milestone-096 composite-merge code path is untouched:
git diff --name-only main | grep -E 'binary/mod\.rs|binary/entry\.rs'
# Expected: empty (no changes to those files in this milestone).
```

## Contract 5 — Documented-omission rationale present (FR-004)

**Path**: `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs` (just above the `const FINGERPRINTS` declaration).

**Invariant**: a `//`-comment block explicitly names the 4 deliberate omissions (boringssl, libressl, llvm, openjdk) with per-library rationale.

**Verification**:
```bash
grep -nE '//\s+(- boringssl|- libressl|- llvm|- openjdk):' \
    mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs
# Expected: 4 matches.
```

## Contract 6 — Diff scope guardrails (FR-005, FR-006, FR-009)

**Verification**:
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
# Expected: empty (no goldens regenerated — milestone-096 SC-007 bound
# inherits to milestone 099 because no existing fixtures statically link
# sqlite / pcre / pcre2 / gnutls)

# Diff scope allowlist:
git diff --name-only main | sort
# Expected only:
#   CLAUDE.md                                              (auto-updated)
#   mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs
#   specs/099-symbol-fingerprint-expand/...
```

## Contract 7 — Pre-PR gate clean (SC-005)

**Verification**:
```bash
MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1 ./scripts/pre-pr.sh
# Expected: prints `>>> all pre-PR checks passed.`; exit 0.
# Clippy: zero warnings.
# Test suite: every target `0 failed`.
# Existing milestone-096 + binary_id_enrich tests continue to pass —
# mikebom uses rustls so the new 4 fingerprints stay quiet on
# mikebom-self (SC-006 mikebom-self regression guard).
```
