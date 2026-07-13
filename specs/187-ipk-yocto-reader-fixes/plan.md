# Implementation Plan: ipk reader — modern ar-format extraction + filename-fallback arch fix

**Branch**: `187-ipk-yocto-reader-fixes` | **Date**: 2026-07-12 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/187-ipk-yocto-reader-fixes/spec.md`

## Summary

Two coupled bug fixes in `mikebom-cli/src/scan_fs/package_db/ipk_file.rs`:

**US1 (#543)** — The current ipk reader's format-detection logic treats the ar-format (`!<arch>\n` magic) as a "legacy pre-2015" failure mode and routes 100% of modern Yocto ipks to the filename-only fallback path. This loses `License:`, `Depends:`, `Recommends:`, `Section:`, `Priority:`, `Maintainer:`, `Homepage:` metadata for every Yocto-scanned component. **Fix**: promote ar-format to the PRIMARY parse path. Add a hand-rolled ~100-line ar parser (zero new deps per Constitution I). Reuse the existing `extract_control_file` + `build_entry_from_control` downstream helpers. Preserve the `gzip(tar)` path as a secondary fallback for pre-2015 ipks.

**US2 (#542)** — The filename-fallback's `rsplit_once('_')` arch parser regresses for arches whose names contain `_` (`qemux86_64`, `powerpc_e500v2`, `mips_i6400`), emitting `?arch=64` + a corrupted `version` string with `_qemux86` glued on. Affects 12% (552/4587) of components in a stock `core-image-minimal` Yocto scan. **Fix**: consult the parent-directory name in the filename fallback. Per Clarifications Q1, the gate is: parent-dir wins IFF the filename (with `.ipk` stripped) ends with `_<parent-dir-name>` — byte-for-byte suffix match. This correctly identifies the Yocto convention without misfiring on loose-file layouts.

**Technical approach**: All changes confined to `ipk_file.rs`. Add a `parse_ar_archive(bytes: &[u8]) -> Result<Vec<ArMember>, ArError>` helper (~100 lines) that returns the ar-container members. Wire it as the FIRST format-detection branch in `parse_ipk_file`; on member-list success, extract the `control.tar[.gz]` member via the existing `extract_control_file` code. Extend `filename_fallback_entry` + `parse_ipk_filename` with the FR-010 parent-dir suffix-match rule. Extend `collect_claimed_paths` (line 204) to also process ar-format ipks. Rename `IpkParseError::LegacyArFormat` variant to `LegacyGzipOnlyFallbackFailed` (or drop it entirely if no external caller depends on it).

Zero new Cargo dependencies. Existing `flate2` + `tar` crates cover the inner tar / gzip extraction; ar-format parsing is stdlib-only (byte-slice indexing + ASCII decimal parsing).

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–186; no nightly required for this user-space-only bug fix).

**Primary Dependencies**: Existing only — `flate2` (workspace; inner gzip decompression), `tar = 0.4` (workspace; inner tar walk), `mikebom_common::types::license::SpdxExpression` (already used by the ipk `extract_control_file` path for m152/m185 license normalization), `tracing` (WARN logs), `anyhow`/`thiserror` (error propagation), `serde_json` (extra_annotations). **Zero new Cargo dependencies.** The ar-format parser is hand-rolled in ~100 lines of pure Rust (BSD ar format: 8-byte magic + 60-byte fixed-size headers per member; opkg-produced ipks use short member names, so no GNU ar long-name-table handling required).

**Storage**: N/A — all state in-process per scan. The ar-format member table lives in a `Vec<ArMember>` on the stack for the duration of one ipk parse; dropped at return. Matches every milestone since 002.

**Testing**: `cargo +stable test --workspace` (unit + integration tests), `cargo +stable clippy --workspace --all-targets -- -D warnings` (lint). New integration test file `mikebom-cli/tests/ipk_yocto_reader_fixes.rs` fabricates synthetic ar-format + gzip-tar-format ipks at test time (using stdlib byte-vector construction + the `tar` + `flate2` crates) so the tests are self-contained and don't require the split-fixtures repo (matches the m185 test pattern at `mikebom-cli/tests/ipk_reader_regression.rs`).

**Target Platform**: Linux + macOS user-space (unchanged from prior milestones). ar-format parsing is byte-level and platform-agnostic.

**Project Type**: CLI (Rust binary + shared common crate). Existing three-crate architecture: `mikebom-cli`, `mikebom-common`, `xtask`.

**Performance Goals**: The ar-format detection adds one 8-byte magic-check per ipk file (constant-time). For a 4587-component Yocto scan, the added overhead is negligible (<50ms total). The primary perf win comes from ELIMINATING the filename-fallback path for the 4587 Yocto ipks — those files were previously fully read + rejected at magic-byte check BEFORE going into fallback; post-m187 they're detected at byte-8 and go straight into the ar parser. Net perf is likely IMPROVED, not degraded.

**Constraints**: FR-014 + SC-005 byte-identity guard on existing pre-2015 `gzip(tar)`-format ipks. The `gzip(tar)` code path MUST NOT regress — its unit tests + integration tests + existing regression fixtures MUST produce byte-identical output. FR-015 / SC-007 zero-new-deps gate (`cargo tree --workspace | wc -l` identical pre/post).

**Scale/Scope**: 2 user stories (both P1), 1 code file touched (`ipk_file.rs`), 1 new integration test file. Estimated ~18-22 tasks across 6 phases.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

**Principle I (Pure Rust, Zero C)** — PASS. Zero new Cargo dependencies. ar-format parser is hand-rolled pure Rust (~100 lines: byte-slice indexing + ASCII decimal parsing).

**Principle II (eBPF-Only Observation)** — N/A. m187 is user-space package-DB reader work; `mikebom-ebpf` untouched.

**Principle III (Fail Closed)** — PASS. Every parse-error class (ar-malformed, gzip-tar-malformed, control-missing, control-oversize) surfaces a specific WARN log with the underlying reason. Fall-through to the filename-fallback path is preserved for all shapes. Zero silent drops per FR-006/FR-007.

**Principle IV (Type-Driven Correctness)** — PASS. `IpkParseError` variants distinguish structural failure classes (ArMalformed vs GzipTarMalformed vs ControlMissing vs ControlOversize) so dispatch logic is compile-time-typed. A new `ArMember` struct newtype wraps `(name: String, data: Vec<u8>)` for the ar-format helper's return type.

**Principle V (Specification Compliance + Native-first)** — PASS. `License:` field flows through the existing `SpdxExpression::try_canonical` path (native SPDX). `Architecture:` becomes the PURL `?arch=` qualifier (native PURL). Two new `mikebom:*` properties (`mikebom:source-mechanism = "ipk-file-archive-extraction"`, `mikebom:arch-source = "control-file|parent-directory|filename-heuristic"`) — documented per spec.md §Assumptions. No native CDX / SPDX 2.3 / SPDX 3 construct exists for "which code path emitted this qualifier" — this is diagnostic metadata the standards don't model. Per Constitution V's native-first requirement, a `mikebom:*` property is the only option; documented in `docs/reference/sbom-format-mapping.md` as part of Phase 6 polish.

**Principle VI (Three-Crate Architecture)** — PASS. All changes confined to `mikebom-cli/src/scan_fs/package_db/ipk_file.rs` (production) + `mikebom-cli/tests/ipk_yocto_reader_fixes.rs` (test). Zero changes to `mikebom-common`, `mikebom-ebpf`, or `xtask`.

**Principle VII (Test Isolation)** — PASS. New unit tests colocated with `ipk_file.rs` (ar parser + suffix-match gate). Integration tests via `mikebom-cli/tests/ipk_yocto_reader_fixes.rs` — no root/CAP_BPF requirement (byte-slice + tempdir manipulation only).

**Principle VIII (Completeness)** — PASS. m187 CLOSES a 100% completeness gap on Yocto ipk metadata (License, Depends, Recommends, Section, Priority, Maintainer, Homepage — all currently NOASSERTION or empty; post-m187 all extracted from the control file). Directly addresses the `unattributed content also counts toward completeness` clarification added in Constitution v1.5.0.

**Principle IX (Accuracy)** — PASS. m187 REMOVES a 12% false-positive rate on Yocto qemux86_64 arches (552/4587 components emitting `?arch=64` with corrupted `version` strings). Correct arch values improve vulnerability matching + PURL-based DB lookups.

**Principle X (Transparency)** — PASS. `mikebom:source-mechanism` (per-component) + `mikebom:arch-source` (per-component) properties are explicit transparency signals — SBOM consumers can distinguish extraction-path components from fallback-path components + identify low-confidence arch emissions at parse time.

**Principle XI (Enrichment)** — N/A. No external-data enrichment for m187.

**Principle XII (External Data Source Enrichment)** — N/A. Same as XI.

**Result**: All 12 principles PASS. No violations to justify. No Complexity Tracking table needed.

## Project Structure

### Documentation (this feature)

```text
specs/187-ipk-yocto-reader-fixes/
├── plan.md                    # This file
├── research.md                # Phase 0 output (4 decisions — ar-format shape, member handling, suffix-match wire, variant rename scope)
├── data-model.md              # Phase 1 output (ArMember struct, IpkParseError variant updates, suffix-match algorithm)
├── quickstart.md              # Phase 1 output (operator scan example + contributor extension guide)
├── contracts/
│   └── ipk-parse-pipeline.md  # Format detection → member extraction → control-file parse → filename-fallback contract
├── checklists/
│   └── requirements.md        # 16/16 PASS from /speckit-specify
├── spec.md                    # Feature specification
└── tasks.md                   # Phase 2 output (/speckit-tasks — NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   └── scan_fs/
│       └── package_db/
│           └── ipk_file.rs                    # US1 + US2 — ALL production changes in this one file:
│                                              #   • Add `parse_ar_archive` + `ArMember` (US1 primary path)
│                                              #   • Refactor `parse_ipk_file` dispatch (ar → gzip-tar → filename)
│                                              #   • Extend `collect_claimed_paths` for ar-format ipks (US1 T028 forward)
│                                              #   • Rename `IpkParseError::LegacyArFormat` → `LegacyGzipTarFallbackFailed` (or drop)
│                                              #   • Add `parent_dir_arch_suffix_match` helper (US2)
│                                              #   • Extend `filename_fallback_entry` with parent-dir consultation (US2)
│                                              #   • Add `mikebom:arch-source` property emission (US2 FR-013)
│                                              #   • Update `mikebom:source-mechanism` value on ar-path success (US1 FR-007)
│                                              #   • New `#[cfg(test)] mod tests` cases for ar parser + suffix-match
└── tests/
    └── ipk_yocto_reader_fixes.rs              # NEW — US1 + US2 integration tests (synthetic ar + gzip-tar fixtures)
```

**Structure Decision**: Single-file scope inside `mikebom-cli/src/scan_fs/package_db/`. One new integration test file. Follows the m185 minimal-touch precedent (which also fixed ipk_file.rs bugs in-place without cross-file refactor). No new modules — the ar parser is a private helper within `ipk_file.rs` at ~100 LOC, well under the "extract to sibling file" threshold used elsewhere in the codebase.

## Complexity Tracking

*No violations to justify — all 12 constitution principles PASS.*
