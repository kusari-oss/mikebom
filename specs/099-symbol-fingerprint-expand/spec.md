# Feature Specification: Symbol-fingerprint table expansion to 7 libraries

**Feature Branch**: `099-symbol-fingerprint-expand`
**Created**: 2026-05-13
**Status**: Draft
**Input**: User description: "milestone 099 — expand the symbol-fingerprint scanner from the milestone-096 v1 starter set (openssl/zlib/libcurl) to match the broader version-string coverage. Add sqlite, pcre, pcre2, gnutls fingerprints — 10 symbols each, 8/10 match threshold, ELF only. Document why boringssl/libressl/llvm/openjdk are deliberately excluded. Zero new Cargo deps."

## Background

Milestone 096 introduced the symbol-fingerprint scanner with a v1 starter set of 3 libraries: openssl, zlib, libcurl — each with 10 well-known public-API symbols and an 80% match threshold. This is the same scanner that emits `pkg:generic/<lib>` components (no version) on ELF binaries where the embedded-version-string scanner misses (e.g., the version string is stripped, never emitted, or hidden inside compressed data).

The version-string scanner covers a broader 11-library set: openssl, boringssl, zlib, sqlite, curl, pcre, pcre2, gnutls, libressl, llvm, openjdk. The symmetry gap between the two scanners means a binary that statically links **sqlite** but has its version string stripped emits no identification at all — even though sqlite's public-API symbols (`sqlite3_open`, `sqlite3_prepare_v2`, etc.) are highly distinctive and easily fingerprintable.

This milestone closes the gap for the four libraries that are both **fingerprintable** and **valuable to identify**: sqlite, pcre (PCRE 8.x), pcre2 (PCRE 10.x), gnutls. The remaining four — boringssl, libressl, llvm, openjdk — are documented as deliberate omissions with rationale (BoringSSL/LibreSSL share OpenSSL's symbol set making per-library disambiguation unreliable at the symbol level; LLVM has too broad an API surface for a useful 10-symbol fingerprint; OpenJDK isn't a C library so symbol fingerprinting doesn't apply to its compiled launcher binary).

**Scope framing**: this is a *pure data-table expansion*. The scanner code path is unchanged from milestone 096 — `symbol_fingerprint::scan(symbol_names: &[String])` already iterates the `FINGERPRINTS` table, applies the 8/10 threshold, and emits `SymbolFingerprintMatch` records. The only code change is appending 4 new entries to the const table + 4 new unit tests verifying each fingerprint matches its target library's expected symbol set. Zero new Cargo dependencies. Zero changes outside `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs`. Zero changes to the existing composite-evidence merge or PURL emission shape.

**What this is NOT**: this is not a new identification technique, not a new evidence channel, not a new property. The CDX `evidence.identity[].methods[].technique` and the `mikebom:evidence-kind = symbol-fingerprint` annotation remain unchanged. Operators consuming SBOMs continue to use the same query patterns (`components[?(@.purl=="pkg:generic/sqlite")]`); they just get more hits when binaries actually link these libraries.

Out of scope: PE / Mach-O symbol fingerprinting (milestone 096 deferred those; they remain deferred — `.dynsym` is ELF-specific, and PE Export Directory + Mach-O `LC_DYSYMTAB` parsing involves enough new code that it deserves its own milestone). Confidence-tier tuning (still 0.4 per milestone 096 FR-004). Symbol-fingerprint-only CPE candidate emission (milestone 097 FR-004 explicitly suppresses these to avoid wildcard-version false-positive floods; that policy is preserved).

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Operator scans a stripped binary that statically links SQLite and sees a SQLite component (Priority: P1)

An operator has a stripped ELF binary that statically links SQLite (typical pattern for self-contained CLI tools — `fossil`, `litecli`, `sqlite-utils`-frozen, every embedded-database CLI). Today: mikebom emits no SQLite signal because the version string `"3.45.1 ..."` was stripped from `.rodata`. With this milestone: mikebom matches the 10-symbol SQLite fingerprint (8 of 10 symbols required, all `sqlite3_*`-prefixed and highly distinctive), emits `pkg:generic/sqlite` with `mikebom:evidence-kind = symbol-fingerprint` and `mikebom:fingerprint-symbols-matched = 9/10` (or however many matched).

**Why this priority**: SQLite is one of the most-statically-linked C libraries in the wild. Stripped CLI binaries often retain their `.dynsym` (needed for dynamic linker fixups against libc) so the `sqlite3_*` symbols are present even when the version string has been removed. This single library closes a meaningful unknown-binary identification gap.

**Independent Test**: build a small C test program that statically links SQLite, strip the version string but keep `.dynsym`, scan with mikebom. Expect `pkg:generic/sqlite` component with `mikebom:evidence-kind = symbol-fingerprint` and `mikebom:fingerprint-symbols-matched ≥ 8/10`. Toolchain-graceful-skip if `sqlite-dev` or `cc` unavailable.

**Acceptance Scenarios**:

1. **Given** an ELF binary whose `.dynsym` table contains `sqlite3_open`, `sqlite3_close`, `sqlite3_exec`, `sqlite3_prepare_v2`, `sqlite3_step`, `sqlite3_finalize`, `sqlite3_bind_int`, `sqlite3_column_text`, `sqlite3_errmsg`, `sqlite3_libversion` (all 10), **When** mikebom scans the file, **Then** the SBOM contains `pkg:generic/sqlite` with `mikebom:fingerprint-symbols-matched = "10/10"`.
2. **Given** an ELF binary whose `.dynsym` contains 8 of the 10 SQLite fingerprint symbols (any 8), **When** mikebom scans it, **Then** the SBOM still emits `pkg:generic/sqlite` (8/10 matches the threshold) with `mikebom:fingerprint-symbols-matched = "8/10"`.
3. **Given** an ELF binary whose `.dynsym` contains 7 of the 10 SQLite fingerprint symbols (below threshold), **When** mikebom scans it, **Then** the SBOM emits NO `pkg:generic/sqlite` component from the symbol-fingerprint path (under-threshold means we don't claim identification).
4. **Given** an ELF binary that ALSO emits a version-string match for SQLite (i.e., `.rodata` retained the `"SQLite version 3.45.1"` literal AND `.dynsym` matched the fingerprint), **When** mikebom scans it, **Then** the composite-evidence merge per milestone-096 Q1 produces ONE component (`pkg:generic/sqlite@3.45.1`) with the symbol-fingerprint recorded as a `mikebom:fingerprint-symbols-matched` annotation on the same component — not two separate components.

---

### User Story 2 — Operator scans a binary that exports PCRE 8.x or PCRE 10.x symbols (Priority: P2)

An operator has an ELF binary that statically links PCRE — either the legacy PCRE 8.x library (`pcre_*` prefix) or the modern PCRE 10.x library (`pcre2_*_8` prefix for the 8-bit width). The two libraries are independent — they have separate NVD CPE namespaces (`pcre:pcre` for 8.x, `pcre:pcre2` for 10.x) — and binaries link one or the other, not both. The milestone-096 / earlier version-string scanner already disambiguates them by anchor (`PCRE ` vs `PCRE2 `); this milestone extends the same disambiguation to the symbol-fingerprint scanner.

**Why this priority**: P2 because PCRE is less universally statically linked than SQLite (most apps use libc regex), but the symbol-fingerprint distinguishes 8.x from 10.x with high confidence — the prefix difference (`pcre_compile` vs `pcre2_compile_8`) is the disambiguator.

**Independent Test**: build a C test program that links `libpcre.a` (PCRE 8.x) — confirm `pkg:generic/pcre` (NOT `pcre2`) emits. Repeat with `libpcre2-8.a` (PCRE 10.x) — confirm `pkg:generic/pcre2` emits.

**Acceptance Scenarios**:

1. **Given** an ELF binary whose `.dynsym` exports the PCRE 8.x public-API symbols (`pcre_compile`, `pcre_exec`, `pcre_free`, etc. — 8 of 10), **When** mikebom scans it, **Then** the SBOM emits `pkg:generic/pcre` (NOT `pkg:generic/pcre2`).
2. **Given** an ELF binary whose `.dynsym` exports the PCRE 10.x public-API symbols (`pcre2_compile_8`, `pcre2_match_8`, etc. — 8 of 10), **When** mikebom scans it, **Then** the SBOM emits `pkg:generic/pcre2` (NOT `pkg:generic/pcre`).
3. **Given** an unusual binary that statically links BOTH PCRE 8.x and PCRE 10.x (rare but legal — e.g., a polyglot library bundling both versions), **When** mikebom scans it, **Then** the SBOM emits BOTH `pkg:generic/pcre` AND `pkg:generic/pcre2` as separate components.

---

### User Story 3 — Operator scans a binary that statically links GnuTLS (Priority: P2)

An operator has an ELF binary that statically links GnuTLS (Mozilla's NSS-alternative TLS implementation, common in GNU-aligned distros). The fingerprint set targets GnuTLS's stable public API: `gnutls_init`, `gnutls_deinit`, `gnutls_handshake`, etc. — all `gnutls_*`-prefixed for high distinctiveness.

**Why this priority**: P2 because GnuTLS is less universal than OpenSSL but is the dominant alternative on debian-derived systems where OpenSSL licensing concerns drove adoption. Statically-linked GnuTLS is rarer than statically-linked OpenSSL but worth covering for parity.

**Independent Test**: build a C test program that links `libgnutls.a`, scan with mikebom. Expect `pkg:generic/gnutls` component with `mikebom:fingerprint-symbols-matched ≥ 8/10`.

**Acceptance Scenarios**:

1. **Given** an ELF binary whose `.dynsym` exports the GnuTLS public-API symbols (`gnutls_init`, `gnutls_deinit`, `gnutls_handshake`, etc. — 8 of 10), **When** mikebom scans it, **Then** the SBOM emits `pkg:generic/gnutls`.
2. **Given** an ELF binary that exports both GnuTLS AND OpenSSL symbols (some hybrid TLS-stack binaries link both as fallback options), **When** mikebom scans it, **Then** the SBOM emits BOTH `pkg:generic/openssl` AND `pkg:generic/gnutls` as separate components — independent fingerprints, independent emissions.

---

### Edge Cases

- **Library name collision** (e.g., a custom library happens to define a function called `pcre_compile`): the 8-of-10 match threshold makes accidental collisions extremely unlikely — a custom library would need to also define `pcre_exec`, `pcre_free`, `pcre_study`, `pcre_version`, `pcre_fullinfo`, `pcre_compile2`, and one more of `pcre_get_substring` / `pcre_dfa_exec` / `pcre_jit_exec`. The combined probability is vanishingly small. Documented; no special-case handling.
- **PCRE 8.x and 10.x co-resident**: the `pcre_*` and `pcre2_*_8` prefixes are disjoint at the symbol-name level. A binary linking both libraries gets two separate fingerprint matches; the SBOM emits both components per US2 acceptance scenario 3.
- **Version-string + symbol-fingerprint composite for SQLite/PCRE/PCRE2/GnuTLS**: the milestone-096 Q1 composite-evidence merge per `binary/mod.rs::read()` keys on lowercase library name. The version-string scanner uses slugs `sqlite`, `pcre`, `pcre2`, `gnutls` (from `CuratedLibrary::slug()`); this milestone uses the same slugs in the fingerprint table. Composite-merge works automatically; one component per library per binary.
- **OpenSSL fork at symbol level**: BoringSSL and LibreSSL both expose the OpenSSL ABI by design (drop-in replacement). A binary statically linking BoringSSL or LibreSSL will trip the OpenSSL fingerprint and emit `pkg:generic/openssl`. This is the documented behavior — operators wanting fork-specific identification rely on the version-string scanner where the anchor `"BoringSSL "` or `"LibreSSL "` is distinctive. Symbol-fingerprinting cannot distinguish the forks reliably.
- **Stripped `.dynsym` (rare; `strip --strip-all` keeps it, but `strip --discard-all` removes it)**: the scanner reads `.dynsym` via `object::read::File::dynamic_symbols()`; binaries with no `.dynsym` produce an empty symbol set and no fingerprint matches fire. Same behavior as milestone 096 — pre-existing edge case.
- **Symbols present but versioned via GNU symbol versioning** (e.g., `sqlite3_open@@GLIBC_2.2.5`-style decoration): the `object` crate strips version suffixes before exposing the symbol name to the iterator. Verified by milestone-096's existing tests on real binaries. No change needed.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The `FINGERPRINTS` const table in `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs` MUST grow from 3 entries (openssl, zlib, libcurl) to **7 entries** by adding 4 new libraries: sqlite, pcre, pcre2, gnutls. Each new entry MUST follow the same shape: `library_name: &'static str`, `symbols: &'static [&'static str]` (10 entries), `required_symbol_count: usize` (= 8 = 80% threshold per milestone-096 FR-004).
- **FR-002**: The 4 new symbol sets MUST consist of public-API functions documented in each library's headers (no internal/private symbols) and use the library-canonical prefix for distinctiveness: `sqlite3_*` (SQLite), `pcre_*` (PCRE 8.x), `pcre2_*_8` (PCRE 10.x), `gnutls_*` (GnuTLS).
- **FR-003**: Library slugs in the `FINGERPRINTS` table MUST match the slugs produced by `version_strings::CuratedLibrary::slug()` so the milestone-096 Q1 composite-evidence merge correctly collapses version-string + symbol-fingerprint matches for the same library on the same binary into ONE component. Required slugs: `sqlite`, `pcre`, `pcre2`, `gnutls`.
- **FR-004**: Documented-omission allowlist MUST grow from 1 entry (`boringssl` per milestone 097) to **5 entries**: `boringssl`, `libressl`, `llvm`, `openjdk`, plus a placeholder slot for future additions. The list lives in-source as a `//` comment block above the `FINGERPRINTS` table with per-library rationale (BoringSSL/LibreSSL share OpenSSL's symbol set → unreliable disambiguation; LLVM has too broad an API surface for a useful 10-symbol fingerprint; OpenJDK isn't a C library so symbol fingerprinting doesn't apply).
- **FR-005**: No new Cargo dependencies. Pure const-table addition + unit tests using the existing `String::new(...)` test harness from milestone 096.
- **FR-006**: Production code changes confined to `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs`. No changes to `binary/mod.rs`, no changes to `entry.rs`, no changes to `generate/*`, no changes to the parity catalog (no new `mikebom:*` properties).
- **FR-007**: The 8-of-10 match threshold per milestone-096 FR-004 MUST apply uniformly to all 7 libraries. No per-library threshold tuning in v1.
- **FR-008**: Composite-evidence merge with version-string matches per milestone-096 Q1 MUST work for all 4 new libraries without code changes — the merge keys on lowercase library name, and the slug naming convention is preserved per FR-003.
- **FR-009**: Goldens MAY regenerate if any existing fixture contains a binary triggering one of the 4 new fingerprints. Per milestone-096 SC-007, the ≤1-spurious-match bound across the 9 existing ecosystem fixtures continues to apply — at most 1 spurious match across all 7 libraries.
- **FR-010**: 4 new unit tests in `symbol_fingerprint::tests`: one per new library. Each test synthesizes the library's full 10-symbol set, calls `scan()`, asserts a single match with the right library name and `matched_count == 10`. Plus an under-threshold test verifying a 7-of-10 symbol set produces NO match.

### Key Entities

- **Symbol fingerprint entry**: a `(library_name, symbols[10], required_symbol_count)` triple in the `FINGERPRINTS` const table. Library name is lowercase ASCII matching `CuratedLibrary::slug()`. Symbols are exact-string public-API function names from the library's headers. Threshold is uniformly 8 (= 80% of 10).
- **Documented-omission rationale**: in-source `//` comment per omitted library explaining why no fingerprint exists. Helps future maintainers understand the decision tree.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An operator scanning a fixture that statically links SQLite (and has its version string stripped) sees `pkg:generic/sqlite` in the emitted SBOM with `mikebom:evidence-kind = symbol-fingerprint`.
- **SC-002**: An operator scanning a fixture that statically links PCRE 8.x sees `pkg:generic/pcre` (not `pcre2`); an operator scanning a fixture that statically links PCRE 10.x sees `pkg:generic/pcre2` (not `pcre`).
- **SC-003**: An operator scanning a fixture that statically links GnuTLS sees `pkg:generic/gnutls` in the emitted SBOM.
- **SC-004**: The composite-evidence merge from milestone-096 Q1 produces ONE component (not two) when both version-string and symbol-fingerprint matches fire for any of the 4 new libraries on the same binary.
- **SC-005**: `./scripts/pre-pr.sh` clean post-implementation — zero clippy warnings, every test target reports `0 failed`. `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1` opt-in also passes (no new emission paths; conformance unchanged).
- **SC-006**: 4 new unit tests pass; the existing 3 milestone-096 tests continue to pass; no regression in the existing `binary_id_enrich.rs::mikebom_itself_does_not_emit_spurious_symbol_fingerprints` test (mikebom itself uses rustls so it should NOT trip any of the 4 new fingerprints either).
- **SC-007**: Existing-ecosystem golden regen scope bounded — at most 1 component per existing golden gains a new symbol-fingerprint emission across the 9 ecosystem fixtures (matches milestone-096 SC-007 ≤1-spurious-bound semantics).
- **SC-008**: Zero new Cargo dependencies (FR-005).

## Assumptions

- The 4 chosen libraries (sqlite, pcre, pcre2, gnutls) cover the high-value gap between the milestone-096 symbol-fingerprint set and the broader milestone-026 version-string set. Operators with niche static-linking patterns (boost, libpng, freetype, etc.) require both scanners to support those libraries first — out of scope for this milestone.
- The 8-of-10 threshold is correctly calibrated per milestone-096 FR-004. No threshold tuning in this milestone; tune at a separate milestone if false-positive data emerges.
- Symbol names in each library's public API are stable across versions — `sqlite3_open` has shipped since SQLite 3.0.0 in 2004; `gnutls_init` since GnuTLS 1.0 in 2004. Selecting 10 long-stable symbols per library bounds the regression risk from upstream API changes.
- The mikebom binary itself (built with rustls, not OpenSSL/GnuTLS/etc.) does NOT export any of these 7 fingerprint sets at 8/10 strength. Verified at implementation time; the SC-007 spurious-bound test guards regressions.

## Dependencies

- **Milestone 096** (binary-id enrichment) — provides the `symbol_fingerprint.rs` module + `scan()` function + composite-evidence merge this milestone extends. Direct dependency on the FINGERPRINTS table shape.
- **Milestone 026** (version-string easy-4 cohort) — defines the `CuratedLibrary::slug()` values (`sqlite`, `pcre`, `pcre2`, `gnutls`) this milestone matches for composite-evidence merge.

## Out of Scope

- **PE Export Directory + Mach-O `LC_DYSYMTAB` symbol fingerprinting**. ELF only in v1; the milestone-096 Out-of-Scope clause for PE/Mach-O remains in force.
- **CPE candidate emission for symbol-fingerprint-only components**. Milestone-097 FR-004 explicitly suppresses these to avoid wildcard-version false-positive floods. Preserved.
- **Confidence-tier tuning** (e.g., raising SQLite's confidence above 0.4 because its prefix is distinctive). Out of scope; the heuristic-tier is uniform across all symbol-fingerprint emissions per milestone-096 conventions.
- **Additional libraries beyond the v1 starter set** (libpng, freetype, boost, libssh2, nghttp2, etc.). These require version-string-scanner support first; out of scope until that milestone lands.
- **BoringSSL / LibreSSL fork-specific fingerprinting**. The forks share the OpenSSL ABI by design; symbol fingerprinting cannot reliably distinguish them. Operators wanting fork identity rely on the milestone-026 version-string scanner where the anchors `"BoringSSL "` / `"LibreSSL "` are distinctive.
- **LLVM fingerprinting**. LLVM is a compiler infrastructure project with hundreds of public-API entry points; selecting any stable 10-symbol subset is unreliable (different mikebom releases would pick different 10-symbol slices). Skip.
- **OpenJDK fingerprinting**. OpenJDK's "binary" is the `java` launcher, which doesn't statically link the JDK as a C library — the JDK lives in `libjvm.so` loaded via JNI. Symbol fingerprinting on the launcher doesn't yield OpenJDK identification. Defer until milestone-026's OpenJDK version-string extraction is the sole signal.
- **Per-library symbol-fingerprint-count metric** (e.g., `mikebom:fingerprint-strength = high|medium|low` based on prefix distinctiveness). Constitution X transparency is satisfied by the existing `mikebom:fingerprint-symbols-matched = N/10` annotation; finer-grained signals are operator policy, not data.
- **Symbol-fingerprint scanning at runtime** via eBPF observation. Different signal channel; orthogonal to static binary inspection.
