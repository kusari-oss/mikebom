# Implementation Plan: Symbol-fingerprint table expansion to 7 libraries

**Branch**: `099-symbol-fingerprint-expand` | **Date**: 2026-05-13 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/Users/mlieberman/Projects/mikebom/specs/099-symbol-fingerprint-expand/spec.md`

## Summary

Append 4 new entries to the existing `FINGERPRINTS` const table in `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs` — `sqlite`, `pcre`, `pcre2`, `gnutls` — each with 10 public-API symbols and the 8/10 match threshold inherited from milestone-096 FR-004. Add a documented-omission `//` comment block above the table explaining why BoringSSL, LibreSSL, LLVM, and OpenJDK are intentionally skipped. Add 4 new unit tests (one per new library) plus an under-threshold negative test. Zero new files, zero new Cargo dependencies, zero code changes outside the one module.

The composite-evidence merge from milestone-096 Q1 works automatically because the library slugs (`sqlite`, `pcre`, `pcre2`, `gnutls`) match the values already produced by `version_strings::CuratedLibrary::slug()` — verified at planning time via `grep -nE '=> "sqlite"|=> "pcre"|=> "gnutls"' mikebom-cli/src/scan_fs/binary/version_strings.rs` which confirmed all 4 slugs exist in the milestone-026 starter set.

Net delta: ~40 lines of new const-table entries + ~10 lines of comment-block omissions + ~60 lines of new unit tests. Estimated implementation time: ~30 min single-developer.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–098; no nightly required).
**Primary Dependencies**: Existing only — `std::collections::HashSet` (already imported in milestone-096's `symbol_fingerprint.rs`). **No new Cargo deps.**
**Storage**: N/A — pure read-only inference per scan; no caches, no persistence.
**Testing**: `cargo +stable test` workspace. 4 new unit tests in `symbol_fingerprint::tests` + 1 under-threshold negative test (per FR-010). The milestone-096 `binary_id_enrich.rs::mikebom_itself_does_not_emit_spurious_symbol_fingerprints` integration test continues to pass (mikebom uses rustls, not sqlite/pcre/gnutls — so all 4 new fingerprints stay quiet on mikebom-self).
**Target Platform**: Host-agnostic. ELF parsing via the `object` crate is platform-independent — Mach-O and Windows hosts can still scan ELF binaries via mikebom.
**Project Type**: Rust CLI workspace (`mikebom-cli` binary + `mikebom-common` lib + `xtask`).
**Performance Goals**: ≤1µs additional per-binary scan time. The fingerprint loop is a HashSet membership check on each fingerprint's symbol list; growing the table from 3 → 7 rows is linear in the table size + O(10) per row. Sub-microsecond per binary.
**Constraints**: Zero new Cargo deps (FR-005). Production code confined to `mikebom-cli/src/scan_fs/binary/symbol_fingerprint.rs` (FR-006). 8/10 match threshold uniformly across all 7 libraries (FR-007).
**Scale/Scope**: Library coverage 3 → 7. Documented-omission list 1 → 5 entries. Codebase delta: ~110 lines (table + comments + tests). Single-file diff.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Rationale |
|-----------|--------|-----------|
| **I. Pure Rust, Zero C** | ✅ PASS | All new code is Rust. No new Cargo deps. |
| **II. eBPF-Only Observation** | ✅ N/A | Enrichment milestone, not discovery; no observation path touched. |
| **IV. Test Discipline** | ✅ PASS | 4 new unit tests + 1 under-threshold guard + existing mikebom-self spurious-match regression test. Pre-PR gate per SC-005. |
| **V. Specification Compliance** | ✅ N/A | No new emission paths. Existing `mikebom:evidence-kind = symbol-fingerprint` annotation unchanged. No new parity-catalog rows. The new fingerprints flow through the existing milestone-096 emission infrastructure. |
| **X. Transparency** | ✅ PASS | The `mikebom:fingerprint-symbols-matched = N/10` annotation (milestone 096) continues to surface match-strength; under-threshold matches silently skip emission (no false-positive claims). Documented-omission rationale in-source for future maintainer visibility. |
| **XII. External Data Source Enrichment** | ✅ N/A | No external API calls; all parsing is in-source against the binary bytes. |

**No CRITICAL violations.** No new properties; no new parity-catalog rows; no new emission paths.

## Project Structure

### Documentation (this feature)

```text
specs/099-symbol-fingerprint-expand/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
├── checklists/
│   └── requirements.md  # Already exists
├── spec.md              # Already exists
└── tasks.md             # Phase 2 output (NOT created here)
```

### Source Code (repository root)

```text
mikebom-cli/src/scan_fs/binary/
└── symbol_fingerprint.rs   # EXTEND — append 4 rows to FINGERPRINTS table,
                             #         add OMITTED-rationale comment block,
                             #         add 4 + 1 unit tests in tests module
```

**Structure Decision**: single-file delta. No new files, no new modules, no changes to `binary/mod.rs`, no changes to `entry.rs`, no changes to `generate/*`, no changes to parity catalog or docs.

## Complexity Tracking

No constitution violations. Table empty.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| — | — | — |
