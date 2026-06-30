# Implementation Plan: Preserve license operands — milestone 152 (closes #481)

**Branch**: `152-preserve-license-operands` | **Date**: 2026-06-30 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/152-preserve-license-operands/spec.md`

## Summary

Close GitHub issue [#481](https://github.com/kusari-oss/mikebom/issues/481) by extending the RPM reader (`mikebom-cli/src/scan_fs/package_db/rpm_file.rs`) with a second-pass `SpdxExpression::try_canonical` fallback that wraps unrecognized operands as SPDX 2.3-spec-blessed `LicenseRef-<sanitized>` escape-hatch identifiers, recovering the known portion of compound expressions instead of collapsing the whole expression to `NOASSERTION`.

The 5 issue-#481 affected packages (`busybox`, `busybox-hwclock`, `busybox-syslog`, `busybox-udhcpc`, `liblzma5`) will emit `GPL-2.0-only AND LicenseRef-bzip2-1.0.4` (4 busybox-*) and `LicenseRef-PD` (liblzma5) instead of `NOASSERTION`.

Net new code surface: ~150 LOC in `rpm_file.rs` (1 small tokenizer + 1 sanitization helper + 1 fallback wrapper + ~8 new unit tests). No new crates. No catalog changes. No wire-format changes. Pipeline composition with the milestone-478 BitBake-operator normalizer is the only inter-milestone dependency.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–151; no nightly required for this user-space-only RPM-reader extension).
**Primary Dependencies**: Existing only — `spdx = "0.10"` (workspace; already used by `SpdxExpression::try_canonical` at `mikebom-common/src/types/license.rs:135`), `mikebom_common::types::license::SpdxExpression` newtype (`try_canonical` + `as_str` + `Display`), `tracing`, `anyhow`. **No new Cargo dependencies.** No subprocess calls. No network access.
**Storage**: N/A — pure-function transformation; the new helper takes a `&str` and returns `Option<String>`. No caches, no persistence (matches the milestone-478 pattern at `rpm_file.rs:603`).
**Testing**: New unit tests inline in `rpm_file.rs#[cfg(test)] mod tests` block, mirroring the milestone-478 + #475 test pattern at `rpm_file.rs:1061+`. Synthetic license strings hardcoded inline (no fixture changes); the milestone-090 sibling-fixture repo does NOT carry Yocto RPM fixtures.
**Target Platform**: Cross-platform Rust binary (Linux + macOS + Windows). Same target surface as the rest of the RPM reader (which is itself cross-platform; the `rpm = "0.22"` crate is pure-Rust and runs anywhere).
**Project Type**: Library + CLI surface unchanged. This is an internal helper added to one existing file (`rpm_file.rs`) + one `.or_else(...)` call site at line 474.
**Performance Goals**: No measurable perf impact. The new fallback path activates ONLY when `try_canonical` returns Err — i.e., for the small subset of RPM headers with unrecognized operands. On the issue-#481 testbed, that's 5/35 packages (14%); on a typical Cargo/npm/pip scan, it's 0%.
**Constraints**: SC-007 — no wire-format changes; no new catalog rows; no new `mikebom:*` annotation keys. SC-002 — byte-identical happy-path output for fully-canonicalizable expressions (mechanically verified via the existing milestone-090 golden test infrastructure).
**Scale/Scope**: ~150 LOC additive in `rpm_file.rs`; ~8 new unit tests; 1 new CHANGELOG entry. Final file size grows from ~1500 to ~1650 LOC. The new helper is a pure function with no I/O, no async, no state.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Constitution v1.5.0 evaluation against this milestone's deliverable:

| Principle | Applicability | Status | Notes |
|-----------|---------------|--------|-------|
| I. Pure Rust, Zero C | APPLIES | PASS | All new code is Rust; the existing `spdx` crate is pure-Rust (no FFI / C transitives). |
| II. eBPF-Only Observation | N/A | PASS | No discovery / enrichment logic touched. License-text extraction is from already-discovered RPM headers. |
| III. Fail Closed | N/A | PASS | No emission-path semantic changes — this milestone CHANGES what value a successful canonicalization produces; it doesn't alter the fail-closed contract (empty / opaque-garbage input still produces NOASSERTION per FR-008). |
| IV. Type-Driven Correctness | APPLIES | PASS | All new helpers use `&str` / `Option<String>` / `Result` — no `.unwrap()` in production code (Strict Boundary #4). The `SpdxExpression` newtype is the canonical typed carrier per Principle IV. |
| **V. Specification Compliance** | **APPLIES** | **PASS** | The `LicenseRef-<idstring>` escape hatch is the SPDX 2.3-spec-blessed carrier for unknown license identifiers. **No new `mikebom:*` annotation key introduced** per FR-010 — the standards-native solution is sufficient. No catalog changes per FR-018 + SC-007. |
| VI. Three-Crate Architecture | APPLIES | PASS | All edits land in `mikebom-cli`; no new crates; no crate-boundary refactors. The helper could theoretically live in `mikebom-common` next to `SpdxExpression::try_canonical` but per FR-009 (RPM-only scope) we keep it in `rpm_file.rs`. |
| VII. Test Isolation | APPLIES | PASS | New unit tests are unprivileged (no eBPF, no root). They run via `cargo test --workspace` in standard CI environments. |
| VIII. Completeness | APPLIES | PASS | Improves completeness in the sense of reducing false-negative license signal (was `NOASSERTION`, now structured). Matches Principle VIII's spirit ("minimize false negatives — dependencies / properties surfaced in the trace but absent from output"). |
| **IX. Accuracy** | **APPLIES** | **PASS** | The LicenseRef escape hatch is more accurate than NOASSERTION — it's an honest "we don't recognize this operand" signal rather than a misleading "no assertion possible at all." Per Principle IX ("flag ambiguous or low-confidence matches rather than silently include as definitive"), the LicenseRef carrier IS that flag. |
| **X. Transparency** | **APPLIES** | **PASS** | Consumer-visible: a `LicenseRef-bzip2-1.0.4` in the output explicitly tells the consumer "this token isn't on the SPDX license list" — they can decide whether to ignore, resolve via deps.dev / clearly-defined, or escalate to source review. The sanitization rule (per Clarifications Q1) is documented in code + CHANGELOG so consumers can reverse-engineer the original token. |
| XI. Enrichment | N/A | PASS | This milestone is not an enrichment path. License data is extracted from the RPM header (the canonical source); no external API call. |
| XII. External Data Source Enrichment | N/A | PASS | No external sources. |
| Strict Boundary 5 (no file-tier duplicates in default mode) | N/A | PASS | File-tier emission untouched. |

**Gate Outcome**: PASS. No violations. No complexity-tracking entries needed.

## Project Structure

### Documentation (this feature)

```text
specs/152-preserve-license-operands/
├── plan.md                       # This file
├── research.md                   # Phase 0 — spdx-crate API survey + tokenizer grammar + WITH-clause algorithm
├── data-model.md                 # Phase 1 — Token enum + helper signatures + sanitization rule formalization
├── quickstart.md                 # Phase 1 — issue-#481 testbed verification + happy-path regression check
├── contracts/
│   └── helper-api.md             # Phase 1 — the new public-to-module helper signatures + idempotency guarantees
├── checklists/
│   └── requirements.md           # Spec validation checklist (from /speckit-specify)
└── tasks.md                      # Phase 2 — generated by /speckit-tasks (NOT created here)
```

### Source Code (repository root)

Edits are confined to ONE Rust source file (FR-009 — RPM-only scope) plus one CHANGELOG.md entry (SC-008):

```text
mikebom-cli/src/scan_fs/package_db/
└── rpm_file.rs                   # +~150 LOC: new helper `preserve_known_operands_with_license_ref` +
                                  # `sanitize_to_license_ref_idstring` + small tokenizer +
                                  # ~8 new unit tests; +1 `.or_else(...)` call site at line 474.
                                  # `normalize_bitbake_license_operators` (M478) and the existing
                                  # `try_canonical` first-pass are UNCHANGED.

CHANGELOG.md                      # +~10 LOC: new entry under [Unreleased] describing the LicenseRef
                                  # escape-hatch behavior + the replace+collapse+strip sanitization rule
                                  # + named worked examples per FR-002.
```

**Structure Decision**: Single-file Rust edit + single CHANGELOG entry. Mirrors the milestone-478 (#475 close) shape exactly — same file, same `#[cfg(test)] mod tests` extension pattern, same `normalize_*`-style helper-naming convention.

## Constitution Check — POST-DESIGN re-evaluation

Phase 0 (research.md) + Phase 1 (data-model.md, contracts/helper-api.md, quickstart.md) produced no surprises that change the constitution evaluation:

- The new helper (`preserve_known_operands_with_license_ref`) is pure-function, takes `&str`, returns `Option<String>`. No `.unwrap()` in production paths (Principle IV + Strict Boundary #4). All errors from the spdx crate's `license_id` / `exception_id` lookups are Option-typed (`None` = unrecognized), so `?`-propagation is sufficient.
- The hand-rolled tokenizer (research.md §R2) is small (~30 LOC) + pure-function + no dependencies. The grammar is documented in the helper's doc comment (data-model.md §3) — no new crate added.
- The sanitization helper (`sanitize_to_license_ref_idstring`) implements the Q1-blessed replace+collapse+strip rule. Idempotent. Documented per FR-002.
- The WITH-clause detection algorithm (research.md §R3) walks tokens linearly, identifies the operand position immediately following a `WITH` keyword as the exception, and validates it via `spdx::exception_id`. On exception-unrecognized, the helper returns `None` (whole-compound NOASSERTION per FR-013 + Q2).
- No new Cargo dependencies. No subprocess calls. No network. No new I/O.
- Per Principle V: the `LicenseRef-<idstring>` escape hatch is SPDX 2.3-spec-blessed. The catalog (`docs/reference/sbom-format-mapping.md`) is untouched per FR-018 + SC-007.

**Post-design gate outcome**: PASS. No new violations surfaced. No complexity-tracking entries needed.

## Complexity Tracking

No constitution-gate violations to justify.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| _(none)_  | _(none)_   | _(none)_                            |
