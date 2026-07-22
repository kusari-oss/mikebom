# Implementation Plan: Auto-split monorepo SBOM into per-subproject SBOMs

**Branch**: `215-sbom-auto-split` | **Date**: 2026-07-22 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/215-sbom-auto-split/spec.md`

## Summary

New `--split` flag on `waybill sbom scan`. When set, emit **one SBOM per detected subproject boundary** (workspace member) instead of one combined SBOM. Reuses the existing per-ecosystem workspace-detection layer (m127 root selector + m176 workspace visibility + m201 `is_workspace_root` disambiguation) — no new detection logic. Each sub-SBOM is a scope-narrowed projection of the single-SBOM output filtered to the reachable set of one workspace member's dep-graph. Shared transitive deps duplicate across sub-SBOMs per Clarification Q1 (each SBOM self-contained). Emit a sibling `split-manifest.json` describing the split so downstream tooling can reason about the emitted file set as a whole.

Implementation split:
1. **Boundary enumeration** — reuse existing m127/m201 code path that identifies main-module-tagged components as workspace roots. When `--split` is set, list the N distinct workspace-root PURLs as the split axis.
2. **Per-subproject projection** — for each workspace root, compute its dep-graph reachable set (BFS from root over `Relationship` edges), extract those components + relationships from the resolved `Vec<ResolvedComponent>`, and emit as a scoped SBOM. Reuses the existing emit pipeline unchanged; only the input set narrows.
3. **Filename + manifest emission** — generate filesystem-safe names per sub-SBOM (subproject-name.ecosystem.format.json) with collision-detection fallback; write `split-manifest.json` alongside.
4. **Multi-format multiplication** — when `--format` is passed multiple times (e.g., `--format cyclonedx-json --format spdx-2.3-json`), emit N × M files; manifest lists all grouped by subproject.

**Not touching**: eBPF trace pipeline, resolver chain, any of the ecosystem readers. This is purely a post-resolve, pre-emit fan-out.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–214; no version change). No nightly required.

**Primary Dependencies**: Existing only. **Zero new Cargo dependencies.** Reuses `serde`/`serde_json` (manifest emission), `clap` (new `--split` flag via `Args`-derive), `mikebom_common::types::purl::Purl` (workspace-root identity), `sha2` + `data-encoding` (deterministic serial numbers per FR-012), `tracing` (INFO summary + WARN on zero-boundary fallback). The m127 root-selector at `waybill-cli/src/generate/root_selector.rs` and the m201 workspace-root disambiguation at `waybill-cli/src/scan_fs/mod.rs::2367` provide the boundary-enumeration substrate.

**Storage**: N/A — pure in-process split. No caches, no persistence.

**Testing**:
- Unit tests in `waybill-cli/src/generate/split.rs::tests` (new module) — dep-graph BFS from root; shared-dep counting; filename disambiguation.
- Integration tests in `waybill-cli/tests/scan_split_basic.rs` (new) — 3 fixtures: (a) cargo-workspace with 4 members using the `two_binaries_diverge` m212 fixture, (b) heterogeneous multi-ecosystem `npm + pypi + swift`, (c) single-package project (fallback to single SBOM + WARN).
- Golden tests under `waybill-cli/tests/fixtures/golden/split/<fixture>/` — one CDX per member for the cargo-workspace fixture; validates SC-004 (union-of-components-across-subs = pre-feature single SBOM).
- Schema-validity check: each emitted sub-SBOM independently validates against its format's schema (CDX 1.6, SPDX 2.3, SPDX 3.0.1) — reuses existing `spdx3-validate` CI gate and CDX schema check.
- Reproducibility test: with `WAYBILL_FIXED_TIMESTAMP`, running the same split scan twice produces byte-identical output (all N sub-SBOMs + manifest).

**Target Platform**: All existing platforms (linux-x86_64 default + ebpf-tracing, macOS, Windows). No platform-specific behavior.

**Project Type**: Same three-crate architecture (Constitution Principle VI) untouched — this milestone touches only `waybill-cli/src/generate/` (new split module + wire-up) and `waybill-cli/src/cli/scan_cmd.rs` (new flag + output-file plumbing). `waybill-common`: no changes. `waybill-ebpf`: no changes.

**Performance Goals**:
- Split overhead should be O(N × E) where N = subproject count and E = average dep-graph size. Concretely: on a monorepo with 10 workspace members × 500 components each × ~1500 relationships each, split-mode adds at most 2 seconds over single-SBOM baseline emit time (which is already 5-10 seconds for that size).
- Multi-format multiplication (N × M) — 10 subs × 3 formats = 30 emit passes. Existing emit code is ~200 ms per SBOM; total 6 seconds for the multi-format case. Acceptable.

**Constraints**:
- **FR-007 self-contained**: each sub-SBOM MUST parse independently. No cross-SBOM references, no `externalReferences`-based indirection. Downstream tools can consume any single sub-SBOM in isolation.
- **FR-012 reproducibility**: `WAYBILL_FIXED_TIMESTAMP` must produce byte-identical output on repeat scans. Serial numbers become `urn:uuid:<sha256(subproject_purl + fixed_ts)>` deterministic hashes (matches the pre-feature deterministic serial pattern for whole-repo SBOMs).
- **FR-015 full fidelity**: each sub-SBOM retains the same annotations / evidence / license expressions / hashes as the pre-feature single-SBOM would emit for its subset. Splitting is a scope-narrowing operation, never a fidelity reduction.
- **FR-016 all-or-nothing emission**: any emit failure aborts the whole invocation. No half-emitted split output on disk.
- **CI grep gate compatibility**: post-m214 the new flag is `--split` (no `mikebom` string), the new env vars are `WAYBILL_UPDATE_SPLIT_GOLDENS` (matches `WAYBILL_*` prefix), the new manifest uses `waybill:*` annotation namespace. CI gate stays green.

**Scale/Scope**:
- New file: `waybill-cli/src/generate/split.rs` (~250 LOC — boundary enumeration + BFS projection + filename generation + manifest emission).
- New file: `waybill-cli/src/generate/split_manifest.rs` (~100 LOC — manifest serialization types + JSON schema pin).
- Modified: `waybill-cli/src/cli/scan_cmd.rs` (add `--split` flag; wire into emit-dispatch — ~40 LOC).
- Modified: `waybill-cli/src/generate/mod.rs` (extend emit-dispatch to fan out on split — ~30 LOC).
- New tests: 3 integration tests + ~5 unit tests (~250 LOC).
- New golden fixtures: 4 CDX + 4 SPDX-2.3 + 4 SPDX-3 files (12 total) for the cargo-workspace fixture; smaller sets for the heterogeneous fixture.
- Total estimated diff: **~700 LOC production + ~400 LOC test + 12 golden files**.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Pure Rust, Zero C**: ✅ No C. Split module is pure Rust in `waybill-cli`.
- **II. eBPF-Only Observation**: ✅ N/A. Split is post-resolve, pre-emit. No new observation surface.
- **III. Fail Closed**: ✅ Any emit failure aborts the whole invocation (FR-016). No half-emitted split output.
- **IV. Type-Driven Correctness**: ✅ Split uses existing `mikebom_common::types::purl::Purl` for subproject identity. New types (`SplitManifest`, `SplitEntry`) are `#[derive(Serialize, Deserialize)]` with explicit field names — no untyped `String` blobs for functional identifiers. No `.unwrap()` in production code; test-only unwraps guarded per convention.
- **V. Specification Compliance**: ✅ Each sub-SBOM independently valid per its format's spec (CDX 1.6, SPDX 2.3, SPDX 3.0.1). No new `waybill:*` annotations added to the SBOMs themselves — only the manifest (which is Waybill-side operator-facing, not a wire-format artifact) uses the namespace. Standards-native audit: no new `waybill:*` on the SBOM wire; the `metadata.component` / `describes` / root Element fields are STANDARD spec fields that get one-per-sub-SBOM per FR-004. **Standards-precedence-preserved.**
- **VI. Three-Crate Architecture**: ✅ Only `waybill-cli` changes. `waybill-common` + `waybill-ebpf` untouched.
- **VII. Test Isolation**: ✅ Split tests are pure-Rust unit + integration. No eBPF required. Cross-platform.
- **VIII. Completeness**: ✅ Split is scope-narrowing per FR-015. Union of components across sub-SBOMs equals pre-feature single-SBOM component set (SC-004). No completeness loss.
- **IX. Accuracy**: ✅ Same accuracy semantics as pre-feature single-SBOM emit. Each sub-SBOM is a projection; component identity + confidence unchanged.
- **X. Transparency**: ✅ Split manifest surfaces the split state (which SBOMs were emitted, which shared deps duplicate, source dirs). WARN log line on zero-boundary fallback (FR-009). INFO summary line at scan end (FR-014).
- **XI. Enrichment**: ✅ N/A.
- **XII. External Data Source Enrichment**: ✅ N/A. Split reuses existing dep-graph relationships (from lockfiles per XII); no new external calls.

**Strict Boundaries**:
- 1. No lockfile-based discovery — N/A.
- 2. No MITM proxy — N/A.
- 3. No C code — enforced.
- 4. No `.unwrap()` in production — enforced.
- 5. No file-tier duplicates in default mode — N/A (touched only if fixture uses file-tier components; per FR-015 sub-SBOMs preserve whatever the pre-feature output emitted).

**Verdict**: ✅ Constitution check passes. Zero unjustified violations. Proceed to Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/215-sbom-auto-split/
├── plan.md                          # This file
├── research.md                      # Phase 0 — boundary enumeration strategy, BFS projection algorithm, filename disambiguation, manifest schema shape, reproducibility
├── data-model.md                    # Phase 1 — SplitManifest, SplitEntry, SubprojectBoundary entity definitions
├── quickstart.md                    # Phase 1 — operator recipe: single-ecosystem split + multi-ecosystem split + manifest inspection
├── contracts/
│   ├── cli-flag.md                  # `--split` flag contract (interaction with --output-dir, --output, --format)
│   ├── split-manifest-schema.md     # JSON schema for split-manifest.json
│   └── filename-convention.md       # deterministic naming for sub-SBOMs + collision handling
├── checklists/
│   └── requirements.md              # (already exists from /speckit.specify)
└── tasks.md                         # Phase 2 output — NOT created here
```

### Source Code (repository root)

```text
waybill-cli/
└── src/
    ├── cli/
    │   └── scan_cmd.rs               # +--split flag on ScanArgs; +conflict check with --output; +wire into emit-dispatch
    ├── generate/
    │   ├── mod.rs                    # +extend emit-dispatch: when split_mode == true, fan out to split module
    │   ├── split.rs                  # NEW ~250 LOC — boundary enumeration, per-subproject BFS projection,
    │   │                             #   filename generation, orchestration of N × M emit passes
    │   └── split_manifest.rs         # NEW ~100 LOC — SplitManifest + SplitEntry serde types, schema-pinned
    └── (rest unchanged)

waybill-cli/tests/
├── scan_split_basic.rs               # NEW — 3 integration test scenarios
├── fixtures/
│   ├── split_cargo_workspace/        # NEW — reuses/wraps m212 two_binaries_diverge
│   ├── split_heterogeneous/          # NEW — npm+pypi+swift fixture
│   └── golden/split/
│       ├── cargo-workspace/          # NEW — 4 CDX + 4 SPDX-2.3 + 4 SPDX-3 golden per-member SBOMs + 1 manifest
│       └── heterogeneous/            # NEW — 3 CDX + 3 SPDX-2.3 + 3 SPDX-3 + 1 manifest
└── (rest unchanged)

.github/workflows/ci.yml               # No changes required — m214 grep gate stays green (no mikebom refs added)

waybill-common/                        # UNTOUCHED
waybill-ebpf/                          # UNTOUCHED
xtask/                                 # UNTOUCHED
```

**Structure Decision**: One new module (`waybill-cli/src/generate/split.rs`) + one new schema-serialization module (`waybill-cli/src/generate/split_manifest.rs`) + edits to `cli/scan_cmd.rs` (flag) + `generate/mod.rs` (dispatch). Everything else — resolver chain, per-ecosystem readers, `waybill-common`, `waybill-ebpf` — untouched. This is a **post-resolve, pre-emit fan-out** layered cleanly onto the existing pipeline.

## Complexity Tracking

> No Constitution violations. Complexity tracking section unused.
