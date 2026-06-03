# Contract: attribution-rules — join key + per-record outcomes

## Join key (FR-001 per the Phase-1 clarification)

A `SymbolFingerprintMatch` (post-milestone-108 matcher output) is JOINED to a `CmakeBuildDirObservation` when ALL three conditions hold:

1. **Library-name equality (case-insensitive)**: `match.library.to_ascii_lowercase() == observation.library_name.to_ascii_lowercase()`. The fingerprint corpus records carry `library` values like `zlib`, `openssl`, `libcurl` (lowercase by convention); the cmake declaration's name comes from the first positional arg of `FetchContent_Declare(<name> ...)` (operator-controlled casing). Lowercasing both sides handles `FetchContent_Declare(Zlib ...)` or `FetchContent_Declare(ZLIB ...)` without aliasing non-conventional names like `ZLIB::ZLIB`.

2. **Build-dir existence**: `observation.build_artifact_dir.is_dir()` was true at observation time. This is the corroborating signal — we don't bind to a declaration whose build dir doesn't exist (declared-but-unbuilt case).

3. **Scope ancestry**: the binary being scanned has `observation.cmake_project_build_root` as a path-ancestor. Per R4 scoping: a binary at `subprojects/A/build/bin/foo` is attributable via project A's observations only; project B's observations don't apply.

When all three conditions hold, the matcher rewrites `match.target_purl` from `pkg:generic/<library>` to `observation.source_tier_purl`.

## Non-conditions (deliberately excluded from the join key)

- **NOT** the static-archive filename (`libz.a`). Only the directory's existence is checked. We don't open `_deps/<name>-build/lib<name>.a` to confirm — the directory itself is the signal.
- **NOT** any cmake target name (`ZLIB::ZLIB`, `zlibstatic`). Per the Phase-1 clarification, only the declaration's name parameter participates.
- **NOT** any version-string comparison. The fingerprint matcher doesn't know the version; the cmake declaration carries it. The join is on identity, not version.
- **NOT** any symbol-overlap check against the source-tier dep. The fingerprint match's correctness is the matcher's responsibility; this layer just routes the match to the right PURL.

## Per-record outcomes

For each `SymbolFingerprintMatch` produced by `scan_with_corpus`:

| Cmake observation exists? | Build-dir exists? | Binary under project scope? | Outcome |
|---|---|---|---|
| No matching declaration | — | — | **Fallback** — keep `pkg:generic/<library>`; behavior identical to milestone 108. |
| Yes | No (`_deps/<name>-build/` absent) | — | **Fallback** — keep `pkg:generic/<library>`. The declaration's source-tier component still emits separately (declared-but-unbuilt). |
| Yes | Yes | No (binary lives outside this project's build root) | **Fallback** for THIS match — but a different project's observation may still bind, evaluated independently. |
| Yes | Yes | Yes | **Attribute** — rewrite `match.target_purl` to `observation.source_tier_purl`. |

## Post-rewrite component shape

After the matcher returns and `symbol_match_to_entry` builds a `PackageDbEntry`, the entry has:

- `purl` = the attributed source-tier PURL (e.g., `pkg:github/madler/zlib@v1.3.1`).
- `evidence_kind` = `"symbol-fingerprint"` (unchanged from milestone 108 — preserves the matcher's evidence claim).
- `extra_annotations` carries:
  - `mikebom:fingerprint-symbols-matched` = `"<count>/<total>"` (unchanged from milestone 108).
  - `mikebom:fingerprint-corpus-sha` = 12-hex or `bundled` sentinel (unchanged from milestone 108).
  - (After dedup merge with the cmake source-tier component): `mikebom:source-mechanism` = the cmake-derived value (`cmake-fetchcontent-git` / `cmake-fetchcontent-url`), inherited from the source-tier entry per R3.

## Dedup-pipeline interaction (milestone 105 reuse)

After the matcher returns, the per-binary loop's `by_library` HashMap (already in `binary/mod.rs:~448`) keys components by library name. The cmake source-tier component (`pkg:github/madler/zlib@v1.3.1`) and the rewritten binary-tier component (also `pkg:github/madler/zlib@v1.3.1`) collide on PURL inside the dedup pipeline. The pipeline merges them per the existing milestone-105 rules:

- `source-mechanism` becomes the cmake-derived value (the source-tier entry was registered first and wins on PURL collision per the existing pipeline ordering).
- `evidence-kind` annotations on the merged component include both source-tier (`cmake-fetchcontent-git`) and binary-tier (`symbol-fingerprint`) entries.
- `also-detected-via` populated when additional source-tier readers also declared the same library (e.g., a vcpkg.json entry corroborated the cmake one).

If the dedup pipeline doesn't already exhibit this merge behavior on PURL-collision (TBD; verify in Phase 2 implementation), an additive PR may be required. The expected behavior aligns with the milestone-105 spec; the implementation just needs to confirm the existing pipeline handles this case correctly.

## Multi-observation collision (rare)

When the registry returns multiple observations for the same library (e.g., two cmake projects in a workspace both declare `zlib`), the lookup picks the observation whose `cmake_project_build_root` is the CLOSEST path-ancestor of the binary's path. If two project roots are siblings of the binary (same depth), the lookup picks deterministically by lexical sort of `cmake_project_build_root` and emits `tracing::warn!("multiple cmake observations match {library} for {binary}; picking {chosen}")`.

## Failure handling

- **Build dir disappeared between observation and lookup**: the directory's existence was checked once at observation time; if a concurrent process removed it before the matcher fires, the rewrite still uses the observation's `source_tier_purl` (the cached value). The directory-existence check is a corroboration filter at OBSERVATION time, not a per-match filter.
- **Library name not in fingerprint corpus**: the matcher only emits matches for corpus libraries; non-corpus libraries never reach this layer.
- **Cmake declaration parsed by the reader but produces a malformed PURL** (e.g., `FetchContent_Declare(...)` with a URL the cmake reader couldn't turn into a valid PURL): the source-tier `PackageDbEntry` doesn't exist; no observation gets created for that declaration; the matcher's match falls back. No error.
