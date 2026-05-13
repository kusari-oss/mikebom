# Research — milestone 099 symbol-fingerprint table expansion

Phase 0 investigation. Six decision points, all resolved with reference-doc citations + audit-of-existing-code outcomes.

## §1 — Library coverage selection (FR-001 / FR-004)

**Decision**: add 4 libraries — sqlite, pcre, pcre2, gnutls. Document 4 deliberate omissions — boringssl, libressl, llvm, openjdk.

**Selection criteria**: a library is a good symbol-fingerprint target when (a) the version-string scanner already covers it (so composite-evidence merge per milestone-096 Q1 has somewhere to land), (b) its public-API symbol set is distinctive (high-uniqueness prefix avoids collision), (c) its API has been stable across major versions (no per-version fingerprint divergence), (d) it is genuinely statically linked in the wild (not just a system-wide shared library).

| Library | Add? | Rationale |
|---------|------|-----------|
| openssl | ✓ existing | milestone-096 v1 set |
| zlib | ✓ existing | milestone-096 v1 set |
| libcurl | ✓ existing | milestone-096 v1 set |
| sqlite | **+ ADD** | `sqlite3_*` prefix → near-zero collision risk. Most-statically-linked C library in CLI tooling. API stable since SQLite 3.0 (2004). |
| pcre | **+ ADD** | PCRE 8.x; `pcre_*` prefix distinctive. Common statically-linked dependency in regex-heavy CLIs (`grep -P`, perl-derived tools). API frozen as of PCRE 8.45 (final 8.x release, 2021). |
| pcre2 | **+ ADD** | PCRE 10.x; `pcre2_*_8` prefix (8-bit width — dominant variant). Successor to PCRE 8.x; separate NVD vendor:product (`pcre:pcre2`). 16-bit/32-bit width variants deferred to a future milestone (see §3). |
| gnutls | **+ ADD** | `gnutls_*` prefix. Mozilla NSS / OpenSSL alternative; common on Debian-derived distros. API stable since GnuTLS 3.0 (2011). |
| boringssl | ✗ OMIT | Drop-in OpenSSL ABI replacement; symbols overlap with OpenSSL almost completely. Operators wanting fork-specific identification rely on the version-string scanner where the anchor `"BoringSSL "` is distinctive. Symbol fingerprinting cannot reliably distinguish forks. |
| libressl | ✗ OMIT | Same reasoning as BoringSSL — LibreSSL is OpenBSD's OpenSSL fork with overlapping symbol set. Version-string scanner's `"LibreSSL "` anchor handles disambiguation. |
| llvm | ✗ OMIT | API surface has hundreds of public-API entry points spread across `LLVM*`-prefixed libraries (`libLLVMCore`, `libLLVMAnalysis`, ...). Picking any stable 10-symbol slice is arbitrary; different mikebom releases would pick different slices. Skip. |
| openjdk | ✗ OMIT | OpenJDK's binary is the `java` launcher, a small wrapper that loads `libjvm.so` via JNI. The launcher's `.dynsym` doesn't carry JDK API symbols (those live in the JVM shared lib loaded at runtime). Symbol fingerprinting at the launcher level doesn't yield OpenJDK identification. Defer indefinitely. |

**Result**: 7-library coverage at v1 (3 existing + 4 added). 4 documented omissions with in-source rationale.

## §2 — Symbol list curation per library (FR-002)

**Decision**: 10 high-distinctiveness public-API symbols per new library, all matching the library's canonical prefix.

| Library | Prefix | Symbols (10 each) | Source-of-truth header |
|---------|--------|-------------------|------------------------|
| sqlite | `sqlite3_*` | `sqlite3_open`, `sqlite3_close`, `sqlite3_exec`, `sqlite3_prepare_v2`, `sqlite3_step`, `sqlite3_finalize`, `sqlite3_bind_int`, `sqlite3_column_text`, `sqlite3_errmsg`, `sqlite3_libversion` | `sqlite3.h` |
| pcre | `pcre_*` | `pcre_compile`, `pcre_exec`, `pcre_free`, `pcre_study`, `pcre_get_substring`, `pcre_version`, `pcre_fullinfo`, `pcre_compile2`, `pcre_dfa_exec`, `pcre_jit_exec` | `pcre.h` (PCRE 8.45) |
| pcre2 | `pcre2_*_8` | `pcre2_compile_8`, `pcre2_match_8`, `pcre2_match_data_create_8`, `pcre2_substring_get_byname_8`, `pcre2_substring_get_bynumber_8`, `pcre2_get_ovector_pointer_8`, `pcre2_code_free_8`, `pcre2_match_data_free_8`, `pcre2_compile_context_create_8`, `pcre2_set_compile_extra_options_8` | `pcre2.h` (PCRE2 10.42) |
| gnutls | `gnutls_*` | `gnutls_init`, `gnutls_deinit`, `gnutls_handshake`, `gnutls_record_send`, `gnutls_record_recv`, `gnutls_global_init`, `gnutls_global_deinit`, `gnutls_set_default_priority`, `gnutls_credentials_set`, `gnutls_session_set_ptr` | `gnutls/gnutls.h` (GnuTLS 3.8) |

**Rationale**:
- All symbols are public-API functions documented in each library's primary header (not internal/private symbols).
- Prefix distinctiveness: `sqlite3_*`, `pcre_*` vs `pcre2_*_8`, `gnutls_*` — collision probability with unrelated libraries is vanishingly small at the 8-of-10 threshold.
- Symbol stability: each set is dominated by functions that have shipped in every major version of the library for the past 10+ years.

**Alternatives considered**:
- **Fewer symbols per library** (e.g., 5): rejected — would lower the 8/10 → 4/5 threshold and increase false-positive risk on small custom libraries that happen to define one or two name-collision symbols. The 10-symbol corpus matches milestone-096's existing pattern for consistency.
- **Larger symbol corpus per library** (e.g., 20): rejected — diminishing returns. The 10 chosen per library are the most-public-API of each library; adding more risks pulling in less-stable functions (deprecated, internal-leaked-public, etc.).

## §3 — PCRE2 width variant scope (Edge Case)

**Decision**: cover the 8-bit width (`pcre2_*_8`) only in v1. The 16-bit (`pcre2_*_16`) and 32-bit (`pcre2_*_32`) variants are deferred to a future milestone if signal emerges.

**Rationale**:
- PCRE2 ships in 3 width variants for `char` (8-bit), `wchar_t`/UTF-16 (16-bit), and UTF-32 (32-bit) string types. Each is a separate `libpcre2-{8,16,32}.{a,so}` artifact.
- The 8-bit width is dominant in practice — `grep`, `php`'s preg, Apache `mod_rewrite`, and the vast majority of regex consumers use `char *` strings.
- 16/32-bit variants are common in some specific contexts (Qt's QRegularExpression, ICU-aware applications) but a small minority of binaries.
- All three variants are the SAME NVD CPE (`pcre:pcre2`); operators with vulnerability matching don't need width-disambiguation. The version-string scanner already covers all 3 widths uniformly under the `pcre2` slug.
- Future extension: a 16/32-bit binary would currently produce no fingerprint match (the symbols don't match the 8-bit set). That's an honest "we don't know" — fixable by adding `pcre2_compile_16` / `pcre2_compile_32` variant tables in a follow-up milestone if signal emerges.

**Implementation note**: the library_name for all PCRE2 width variants stays `pcre2` (single slug). A future milestone adding 16/32-bit variants would add separate fingerprint table rows (e.g., `pcre2-16`, `pcre2-32`) or extend the existing `pcre2` row's `symbols` list to include all 30 symbols across widths — implementation-time choice.

## §4 — Composite-evidence merge correctness (FR-008)

**Decision**: no code changes required. The milestone-096 `binary/mod.rs::read()` composite-evidence merge keys on lowercase library name; the v1 starter set's slug naming convention is preserved by FR-003.

**Verified at planning time**:
```
$ grep -nE '=> "sqlite"|=> "pcre"|=> "gnutls"' mikebom-cli/src/scan_fs/binary/version_strings.rs
66:            CuratedLibrary::Sqlite => "sqlite",
68:            CuratedLibrary::Pcre => "pcre",
69:            CuratedLibrary::Pcre2 => "pcre2",
70:            CuratedLibrary::GnuTls => "gnutls",
```

All 4 new fingerprint slugs (`sqlite`, `pcre`, `pcre2`, `gnutls`) already exist in `CuratedLibrary::slug()` from milestone 026. The milestone-096 Q1 merge uses these exact strings as the HashMap key. Composite-evidence behavior:
- Binary statically links SQLite, version string retained: version-string scanner emits `pkg:generic/sqlite@3.45.1` first; fingerprint scanner emits a `SymbolFingerprintMatch { library: "sqlite", ... }`. The merge in `binary/mod.rs::read()` collapses to ONE `PackageDbEntry` with the version-pinned PURL + the fingerprint annotation.
- Binary statically links SQLite, version string stripped: fingerprint scanner alone emits `pkg:generic/sqlite` (no version). ONE component, symbol-fingerprint-only.

## §5 — Test approach (FR-010)

**Decision**: unit tests with synthetic symbol lists, following milestone-096's pattern in `symbol_fingerprint::tests`. No toolchain-dependent fixture binaries.

**Rationale**: milestone 096 established the unit-test-with-synthetic-symbol-set pattern for `symbol_fingerprint::tests::openssl_full_set_matches` etc. — synthesizing the exact symbol set the fingerprint expects, calling `scan()`, asserting the match. This avoids the build-an-sqlite-static-binary-in-CI complexity that the milestone-096 spec explicitly rejected. Integration verification continues via the existing `binary_id_enrich.rs::mikebom_itself_does_not_emit_spurious_symbol_fingerprints` test (mikebom uses rustls — should NOT trip any new fingerprint).

**Tests planned**:
1. `sqlite_full_set_matches` — synthesizes all 10 `sqlite3_*` symbols; asserts one match `{library: "sqlite", matched_count: 10, total_count: 10}`.
2. `pcre_full_set_matches` — synthesizes all 10 `pcre_*` symbols; asserts `{library: "pcre", matched_count: 10, total_count: 10}`.
3. `pcre2_full_set_matches` — synthesizes all 10 `pcre2_*_8` symbols; asserts `{library: "pcre2", matched_count: 10, total_count: 10}`.
4. `gnutls_full_set_matches` — synthesizes all 10 `gnutls_*` symbols; asserts `{library: "gnutls", matched_count: 10, total_count: 10}`.
5. `sqlite_seven_of_ten_below_threshold` — synthesizes only 7 of 10 SQLite symbols; asserts NO match (under-threshold sentinel).

The existing 3 milestone-096 tests (`openssl_full_set_matches`, `openssl_eight_of_ten_just_matches`, `openssl_seven_of_ten_below_threshold`) continue to pass unchanged.

## §6 — Documented-omission comment block (FR-004)

**Decision**: add a `//`-comment block immediately above the `FINGERPRINTS` const definition (after the existing struct documentation but before the const itself) listing the 4 deliberate omissions with per-library rationale.

**Format**:
```rust
// Documented omissions (per milestone-099 §1 + spec.md FR-004):
//   - boringssl: drop-in OpenSSL ABI replacement; symbol overlap with
//     openssl prevents reliable disambiguation. Use version-string
//     scanner's `BoringSSL ` anchor for fork-specific identification.
//   - libressl: same as boringssl — OpenBSD's OpenSSL fork shares ABI.
//   - llvm: API surface too broad (hundreds of public-API entry points
//     across libLLVMCore / libLLVMAnalysis / ...); no stable 10-symbol
//     slice. Defer until a versioned compiler-libs strategy emerges.
//   - openjdk: launcher binary doesn't statically link JDK APIs (those
//     live in libjvm.so loaded via JNI). Symbol fingerprinting at the
//     launcher level yields no signal. Defer indefinitely.
const FINGERPRINTS: &[SymbolFingerprint] = &[ ... ];
```

**Rationale**: future maintainers see the decision tree without re-litigating each library. Constitution X transparency at the source-code level.

## Coverage map

| Spec section | Resolution |
|--------------|------------|
| FR-001 (4 new libraries) | §1 + §2 → sqlite, pcre, pcre2, gnutls locked with 10 symbols each |
| FR-002 (canonical prefixes) | §2 → `sqlite3_*` / `pcre_*` / `pcre2_*_8` / `gnutls_*` |
| FR-003 (slug consistency with CuratedLibrary) | §4 → all 4 slugs already exist; composite-evidence works automatically |
| FR-004 (documented omissions) | §1 + §6 → 4 rationales in-source comment block |
| FR-005 (no new Cargo deps) | by-construction — pure const-table addition |
| FR-006 (production code scope) | §6 → single-file delta to `symbol_fingerprint.rs` |
| FR-007 (8/10 threshold preserved) | §2 → uniform across all 7 libraries |
| FR-008 (composite-evidence merge unchanged) | §4 → no code changes needed |
| FR-009 (golden regen ≤1-spurious bound) | inherits from milestone-096 SC-007 |
| FR-010 (unit tests per new library) | §5 → 4 happy-path tests + 1 under-threshold guard |
| SC-001/SC-002/SC-003 (per-library emission) | §5 → unit tests validate via synthetic symbol sets |
| SC-004 (composite-evidence emits ONE component) | §4 → milestone-096 Q1 merge handles this |
| SC-005 (pre-PR gate clean) | by-construction |
| SC-006 (4 new tests pass + mikebom-self regression) | §5 |
| SC-007 (≤1-spurious golden regen) | §3 (PCRE2 8-bit only) + §2 (distinctive prefixes) bounds spurious-match probability |
| SC-008 (zero new deps) | by-construction |
| Constitution V audit | N/A — no new properties / parity-catalog rows |
| Constitution X transparency | §6 → omissions documented in-source |

All open spec questions resolved. Ready for Phase 1 (data-model + contracts + quickstart).
