# Implementation Plan: Verify-and-close cargo proc-macro outgoing dep edges (closes #173)

**Branch**: `088-cargo-procmacro-edges` | **Date**: 2026-05-08 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/088-cargo-procmacro-edges/spec.md`

## Summary

Issue #173 (proc-macro crates emit zero outgoing dep edges, surfaced by milestone-083 against the clap-rs/clap audit fixture) is **already closed** by milestone 087's skip-removal fix. Same root cause as #172: `parse_lockfile`'s `pkg.source.is_none()` skip dropped workspace MEMBERS from the component set, so the source crate (`clap_derive@4.5.18`) had no PURL for outgoing edges to attach to. With the skip removed, `clap_derive` now emits 4 outgoing `DEPENDS_ON` edges (heck, proc-macro2, quote, syn) matching the lockfile + matching trivy + syft.

This milestone is **verify-and-close** scope: pin the now-correct outgoing edges in milestone-083's regression test, update the audit research doc to mark gap #2 closed, and close GitHub issue #173. **No code changes** to `mikebom-cli/src/scan_fs/package_db/cargo.rs` or `mikebom-cli/src/scan_fs/mod.rs`.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–087; no nightly required for this user-space-only test-pinning work).
**Primary Dependencies**: existing only — no new crates. The regression test (`mikebom-cli/tests/transitive_parity_cargo.rs`) already uses `serde_json` (parsing emitted SBOM JSON), `tracing`, `anyhow`, and the milestone-083 `transitive_parity_common` helper. **No additions to the dependency tree.**
**Storage**: N/A — purely test infrastructure. The regression test runs against the milestone-083 audit fixture (`mikebom-cli/tests/fixtures/transitive_parity/cargo/`), which is unchanged.
**Testing**: `cargo +stable test -p mikebom --test transitive_parity_cargo` (existing test, additive change to `EXPECTED_REPRESENTATIVE_EDGES`).
**Target Platform**: All platforms supported by the cargo reader (Linux + macOS via `cargo +stable test`).
**Project Type**: Single project — Rust workspace test-suite addition only.
**Performance Goals**: Test wall-time bound ≤5 s (matches existing milestone-083 invariant; SC-003).
**Constraints**: Zero new code in `cargo.rs` / `scan_fs/mod.rs`; zero golden regenerations; zero new Cargo dependencies (FR-004, FR-005, FR-006).
**Scale/Scope**: ~5 LOC of test additions + ~20 LOC of doc edits + 1 GitHub issue closure comment.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Justification |
|-----------|--------|---------------|
| I. Pure Rust, Zero C | ✅ PASS | No code changes; existing Rust test suite gets a representative-edge addition. No C touched. |
| II. eBPF-Only Observation | ✅ PASS | Not applicable — this milestone touches only the lockfile-enrichment path (Cargo.lock dep extraction), which is permitted under Principle XII. The eBPF trust model is unaffected. |
| III. Fail Closed | ✅ PASS | No change to scan-failure semantics. |
| IV. Type-Driven Correctness | ✅ PASS | Test code; existing `#[cfg_attr(test, allow(clippy::unwrap_used))]` guards on `mod tests` items continue to apply. No new `String`-typed domain values introduced. |
| V. Specification Compliance | ✅ PASS | No changes to SBOM emission. The behavior under test (Cargo.lock `dependencies = [...]` → `DEPENDS_ON` edges) was already constitution-compliant pre-fix; milestone 087 just made the source crate available for attachment. |
| V — Standards-native precedence | ✅ PASS | No new `mikebom:*` properties / annotations / relationships introduced. |
| VI. Three-Crate Architecture | ✅ PASS | No new crates. |
| VII. Test Isolation | ✅ PASS | Test runs in standard CI without elevated privileges (the audit fixture is a manifest+lockfile pair, no eBPF needed). |
| VIII. Completeness | ✅ PASS | This milestone REDUCES false negatives on cargo by pinning the now-correct proc-macro outgoing-edge behavior. Aligns with the principle. |
| IX. Accuracy | ✅ PASS | The 4 edges being pinned are exact matches against `Cargo.lock` `[[package]] clap_derive dependencies = [...]`. No phantom edges. |
| X. Transparency | ✅ PASS | Existing `provenance: "package-database-depends"` on emitted DEPENDS_ON relationships continues to apply. |
| XI. Enrichment | ✅ PASS | No new enrichment sources. |
| XII. External Data Source Enrichment | ✅ PASS | The Cargo.lock-based dep-tree extraction is the canonical milestone-XII path; behavior unchanged from milestone 087. |

**Gate verdict**: ✅ all gates pass. No constitution amendments required.

## Project Structure

### Documentation (this feature)

```text
specs/088-cargo-procmacro-edges/
├── plan.md              # This file
├── research.md          # Phase 0 output (decisions about edge pinning)
├── data-model.md        # Phase 1 output (entities + validation rules)
├── quickstart.md        # Phase 1 output (maintainer recipes)
├── contracts/
│   └── procmacro-edge-pin.md   # Regression-test pinning contract
├── checklists/
│   └── requirements.md  # Spec-quality checklist (already created)
└── tasks.md             # Phase 2 output (/speckit.tasks command — NOT created here)
```

### Source Code (repository root)

```text
mikebom-cli/
├── tests/
│   ├── transitive_parity_cargo.rs   # MODIFIED: 1+ representative edges added
│   ├── transitive_parity_common/    # unchanged (helper)
│   └── fixtures/
│       └── transitive_parity/cargo/ # unchanged (clap-rs/clap @ v4.5.21 audit fixture)
└── src/                             # NO CHANGES (FR-004)

specs/
└── 083-transitive-correctness/
    └── research.md                  # MODIFIED: §8 Ecosystem: cargo gap #2 marked closed
```

**Structure Decision**: Existing single-project Rust workspace. This milestone touches:
1. `mikebom-cli/tests/transitive_parity_cargo.rs` — additive change to `EXPECTED_REPRESENTATIVE_EDGES`.
2. `specs/083-transitive-correctness/research.md` — gap #2 audit-row update.
3. GitHub issue #173 (closure comment).

No new crates, no new test files, no new fixtures. The PR diff is intentionally minimal (~25 LOC across 2 files + 1 issue comment).

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No constitution violations. Complexity tracking N/A.
