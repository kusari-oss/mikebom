# Implementation Plan: RPM reader — fix double-`rpm` PURL namespace + raise size cap

**Branch**: `144-rpm-purl-size-fixes` | **Date**: 2026-06-26 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/144-rpm-purl-size-fixes/spec.md`

## Summary

Two coupled bug fixes in the standalone-`.rpm` reader at `mikebom-cli/src/scan_fs/package_db/rpm_file.rs`, both surfaced by the external `yocto-test` testbed (`core-image-minimal` on qemux86-64):

1. **PURL namespace bug** (D-01) — the fallback branch returns the literal string `"rpm"`, producing non-conformant `pkg:rpm/rpm/<name>@<ver>` PURLs for all 4584 RPM components in the yocto corpus. Fix: re-order the precedence ladder per the clarification — CLI override > `/etc/os-release` `ID=` > per-RPM RPMTAG_VENDOR/RPMTAG_PACKAGER > **empty** (the new fallback). Emit `pkg:rpm/<name>@<ver>` (no namespace segment) when the ladder bottoms out.
2. **Size-cap bug** (D-02) — the 200 MB cap silently drops three well-formed Yocto debug RPMs (`kernel-dbg` 279 MB, `openssl-ptest` 260 MB, `gcc-dbg` 379 MB) with misleading `WARN skipping malformed .rpm file ... reason="size-cap-exceeded"`. Fix: raise default cap to 512 MB, decouple the size-cap WARN from the malformed WARN (drop the word "malformed" while keeping `reason="size-cap-exceeded"` structured field unchanged for log-grep tools), and add operator-facing override `--max-rpm-bytes <N>`.

Plus two new operator-facing CLI flags (`--rpm-distro <ID>` and `--max-rpm-bytes <N>`) threaded through clap → `scan_fs` orchestration → a new `RpmReaderConfig` struct → the existing `read()` / `parse_rpm_file()` entry points. Same FR-008-symmetric raise for `MAX_RPMDB_BYTES` in the rpmdb reader (`rpm.rs:39`), no flag override (deferred per spec).

Total code surface: ~150 LOC reader change + ~30 LOC clap surface + ~200 LOC test additions. No new Cargo dependencies. Estimated 2 commits (fix + golden refresh — though the golden-refresh commit may be empty per the FR-009 audit that found zero `pkg:rpm/rpm/` fixtures).

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–143; no nightly required for this user-space-only work).
**Primary Dependencies**: Existing only — `rpm = "0.22"` (already used by `rpm_file.rs` for the `rpm::Package::open()` path), `serde`/`serde_json`, `tracing`, `anyhow`, `thiserror`, `clap` (new flags via `Args`-derive). Reuses milestone-003's `mikebom_common::types::purl::Purl::new` for PURL validation and `mikebom_common::types::purl::encode_purl_segment` for segment encoding. Reuses milestone-135-era `mikebom-cli/src/scan_fs/os_release.rs::read_id_from_rootfs()` helper. **No new Cargo dependencies.**
**Storage**: N/A — all state in-process per scan; no caches, no persistence (matches every milestone since 002).
**Testing**: `cargo +stable test --workspace`; new tests are pure-Rust unit tests in `mikebom-cli/src/scan_fs/package_db/rpm_file.rs` (in-module `#[cfg(test)] mod tests`) — no external RPM fixtures > 1 KB needed because the size cap is parameterized via `RpmReaderConfig` (Phase 0 R1) so tests can use a tiny synthetic file with valid magic bytes and a small cap (e.g., 100 bytes) to exercise the cap path.
**Target Platform**: Linux x86_64 + macOS arm64 (matches CI lanes; cross-platform is implicit — the reader doesn't shell out, doesn't depend on the host RPM toolchain).
**Project Type**: CLI / library — touches one crate (`mikebom-cli`); no kernel-space changes; no workspace topology change.
**Performance Goals**: No new performance budget — the size-cap raise from 200 → 512 MB changes the upper bound on per-file work proportionally; the `rpm::Package::open` parser is the long-tail cost, not a new code path. The CLI-flag plumbing is one-time per scan invocation (negligible).
**Constraints**:
- **Constitution V (standards-native > `mikebom:*`)** — explicitly NO new `mikebom:*` properties for size-cap or distro-detection metadata (codified in Out of Scope §5 of the spec). Compliance audit complete.
- **Constitution VIII (Completeness)** — raising the cap DIRECTLY improves Completeness. The three previously-dropped Yocto debug RPMs come back.
- **Constitution IX (Accuracy)** — empty / distro namespace is MORE accurate than the literal-`"rpm"` fallback (the latter is provably non-conformant to purl-spec).
- **No subprocess calls** — `/etc/os-release` is read as a regular file via stdlib `BufReader`.
- **Pre-PR gate** — `./scripts/pre-pr.sh` (clippy `-D warnings` + `cargo test --workspace`) MUST exit 0 before PR open per Constitution Development Workflow.
**Scale/Scope**: ~4587 RPMs in the largest empirically-observed corpus (Yocto `core-image-minimal`). Largest single RPM observed: ~379 MB (`gcc-dbg`). Cap raise to 512 MB provides ~130 MB headroom. CLI-flag values: `--max-rpm-bytes` u64 (effectively unbounded). `--rpm-distro` non-empty string up to OS arg-length limits.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | Principle / Boundary | Status | Note |
|---|---|---|---|
| I | Pure Rust, Zero C | ✅ PASS | No new C dependencies; existing `rpm = "0.22"` already vetted in milestone 004. |
| II | eBPF-Only Observation | ✅ N/A | Static reader; eBPF doesn't apply. `/etc/os-release` is filesystem enrichment of an already-discovered RPM (Principle XII compliant). |
| III | Fail Closed | ✅ PASS | No change to scan-failure semantics. The reader continues to emit WARN + skip on per-file failures (`FR-017` behavior preserved). |
| IV | Type-Driven Correctness | ✅ PASS | All new fields use newtypes (`Purl`, new `DistroId(String)` newtype per Phase 1). No new `.unwrap()` in production paths. |
| V | Specification Compliance | ✅ PASS | **The whole point of D-01 fix is PURL-spec compliance.** No new `mikebom:*` properties (Out of Scope §5 records the audit). Existing CDX/SPDX/SPDX-3 emitters consume the corrected PURL string transparently (FR-010). |
| VI | Three-Crate Architecture | ✅ PASS | All changes in `mikebom-cli`. No new crates. |
| VII | Test Isolation | ✅ PASS | All new tests are pure-logic unit tests; no eBPF privilege requirements. |
| VIII | Completeness | ✅ IMPROVES | Cap raise re-includes 3 previously-dropped components in the Yocto baseline (4584 → 4587). |
| IX | Accuracy | ✅ IMPROVES | Empty / distro namespace is MORE accurate than today's literal-`"rpm"`. |
| X | Transparency | ✅ PASS | WARN log preserves structured `reason="size-cap-exceeded"` field; cap value is auto-discoverable via `--help`. |
| XI | Enrichment | ✅ N/A | No enrichment-source changes. |
| XII | External Data Source Enrichment | ✅ PASS | `/etc/os-release` already in scan-root scope (not external); used to refine an already-observed RPM's PURL, never to introduce new components. |
| SB-1 | No lockfile-based discovery | ✅ PASS | N/A. |
| SB-2 | No MITM proxy | ✅ PASS | N/A. |
| SB-3 | No C code | ✅ PASS | N/A. |
| SB-4 | No `.unwrap()` in production | ✅ PASS | Test-module uses `#[cfg_attr(test, allow(clippy::unwrap_used))]` per existing convention. |
| SB-5 | No file-tier duplicates in default mode | ✅ N/A | Not a file-tier change. |

**All gates pass. No Complexity Tracking entries required.**

## Project Structure

### Documentation (this feature)

```text
specs/144-rpm-purl-size-fixes/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   ├── cli-flags.md          # CLI surface contract (--rpm-distro, --max-rpm-bytes)
│   └── rpm-reader-api.md     # rpm_file.rs::read + parse_rpm_file + resolve_rpm_vendor_slug signature contract
└── checklists/
    └── requirements.md   # Already exists from /speckit-specify
```

### Source Code (repository root)

Touched files (all in `mikebom-cli`):

```text
mikebom-cli/
├── Cargo.toml                                          # No change (zero new deps)
├── src/
│   ├── cli/                                            # Where the new clap flags surface
│   │   └── <scan-subcommand>/args.rs                   # Add --rpm-distro + --max-rpm-bytes to ScanArgs
│   └── scan_fs/
│       ├── mod.rs                                      # Thread RpmReaderConfig from CLI args → readers
│       ├── os_release.rs                               # READ-ONLY reuse (no change)
│       └── package_db/
│           ├── rpm_file.rs                             # PRIMARY CHANGE — precedence reorder, cap parameterization, WARN-text decouple
│           └── rpm.rs                                  # MAX_RPMDB_BYTES const raise only (FR-008)
└── tests/
    └── rpm_file_yocto_regression.rs                    # NEW integration test — synth fixture exercising US1/US2 end-to-end
```

**Structure Decision**: Single existing crate (`mikebom-cli`). No new modules. The new `RpmReaderConfig` struct lives inside `rpm_file.rs` (it's reader-private; only `mod.rs` constructs one to pass in). CLI args use the existing clap `Args`-derive on the scan subcommand's struct (location confirmed in Phase 0 R2).

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

Not applicable — all gates pass.
