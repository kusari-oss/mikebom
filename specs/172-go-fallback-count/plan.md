# Implementation Plan: Go-transitive fallback attachment count doc-scope annotation

**Branch**: `172-go-fallback-count` | **Date**: 2026-07-07 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/172-go-fallback-count/spec.md`

## Summary

**Primary requirement**: emit a new document-scope annotation `mikebom:go-transitive-fallback-count = "N"` (as a stringified integer) in CDX 1.6, SPDX 2.3, and SPDX 3.0.1 outputs when a scan has ≥1 Go component. N counts Go modules whose final resolution step was `ResolutionStep::GoSumFallback` (m091 step 5). Per Q1 clarification, emit `"0"` explicitly on healthy scans (not omit). Emission is gated identically to milestone-160 C110 emission: annotation present iff Go scan happened + Go-transitive resolver ran.

**Technical approach**: Aggregate + emit — no new resolver logic, just count and expose. Four surgical changes:

1. **Compute the count at scan_fs pipeline output time**. `ModuleGraphMap.entries: HashMap<ModuleId, ModuleGraphEntry>` already tracks each module's `source: ResolutionStep` per m091. Add a getter method `fallback_count() -> usize` on `ModuleGraphMap` that returns `entries.values().filter(|e| e.source == ResolutionStep::GoSumFallback).count()`. Attach the count to the existing `GoTransitiveCoverage` context flow (either as a new field on `LadderSummary` or as a sibling in `ScanResult.go_transitive_coverage_context`).

2. **Thread the count from `scan_fs::ScanResult` through `SbomEmission` to the three format emitters**. Mirror the m160 C110 plumbing: add `go_transitive_fallback_count: Option<usize>` field on `SbomEmission`, populate from the scan result, drop into each of `metadata.rs` (CDX at line 466-481 next to the existing C110 block), `spdx/annotations.rs` (SPDX 2.3), `spdx/v3_annotations.rs` (SPDX 3) alongside the existing `mikebom:go-transitive-coverage` emission code.

3. **Wire the new parity-catalog row C117**. Add `ParityExtractor { row_id: "C117", label: "mikebom:go-transitive-fallback-count", ... }` to `mikebom-cli/src/parity/extractors/mod.rs`; add `c117_cdx`, `c117_spdx23`, `c117_spdx3` extractor helper functions (each using the existing `cdx_property_values` / `extract_mikebom_annotation_values` scaffolding — annotation extraction, not native-field mapping).

4. **Docs**: (a) add row C117 to `docs/reference/sbom-format-mapping.md` per Constitution Principle V standards-native audit ("KEEP-NO-NATIVE" — no CDX/SPDX 2.3/SPDX 3.0.1 native equivalent for "how many Go modules degraded to flat fallback"). (b) enrich `docs/reference/reading-a-mikebom-sbom.md`'s existing `mikebom:go-transitive-coverage` section (§3.5 per research §R5) with the 5-step ladder mechanism explanation + jq recipe from the guac investigation (per FR-007 + US2).

**Golden regeneration**: run `MIKEBOM_UPDATE_CDX_GOLDENS=1` + `MIKEBOM_UPDATE_SPDX_GOLDENS=1` + `MIKEBOM_UPDATE_SPDX3_GOLDENS=1`. Every Go-touching golden gets a +1 line for the new annotation. Expected diff: `+  { "name": "mikebom:go-transitive-fallback-count", "value": "N" },` in each Go-emitting golden. Non-Go goldens (apk, cargo, deb, gem, maven, npm, pip, rpm, cmake, bazel) MUST show zero delta (SC-008).

**Blast radius**: ~30 lines added in emitters + plumbing, ~10 lines added in parity extractors, ~50 lines added in docs, 3 golden updates (per research §R4 — the 3 Go-touching goldens across cyclonedx/spdx-2.3/spdx-3, each with 1-2 line delta), 1 new integration test.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–171; no nightly required).

**Primary Dependencies**: Existing only — `serde`/`serde_json` (JSON round-trip), `tracing`, `anyhow`, `thiserror`. Reuses milestone-091 `ResolutionStep::GoSumFallback` enum variant (already at `graph_resolver.rs:76`) + milestone-160 `GoTransitiveCoverage`/`LadderSummary` infrastructure verbatim. **Zero new Cargo dependencies.**

**Storage**: N/A — pure metadata-emission transform. The count is computed once per scan at Go-resolver exit time and threaded through the emission pipeline. No persistence.

**Testing**: `cargo test` — 1 new integration test at `mikebom-cli/tests/go_fallback_count.rs` covering the 3 scenarios of FR-006: healthy Go scan → `"0"`; degraded Go scan → `> 0`; non-Go scan → absent. Existing regression tests cover golden byte-identity (the 33+ goldens across CDX/SPDX 2.3/SPDX 3) and the m071 parity gate.

**Target Platform**: All hosts mikebom builds on — Linux (CI), macOS (dev), Windows (m100-experimental). No host-specific code paths touched.

**Project Type**: cli (mikebom sbom-generation CLI).

**Performance Goals**: N/A — the count aggregation is `O(N)` over the module graph map (typically < 1000 modules); happens once per scan. Not observable in scan timing.

**Constraints**: SC-005 correctness gate — the doc-scope count MUST equal the per-component `mikebom:go-transitive-source == "go-sum-fallback"` sum (both derive from the same `ResolutionStep` enum values, so this is structurally guaranteed but the integration test enforces it).

**Scale/Scope**: Small. ~4 source files edited on the emission side, 3-4 parity-extractor files edited, 1 new integration test, 2 docs files edited, 3 golden updates (per research §R4).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Pure Rust, Zero C**: ✅ No new Cargo dependencies. Pure Rust addition.
- **II. eBPF-Only Observation**: ✅ `mikebom-ebpf` untouched.
- **III. Fail Closed**: ✅ Emission is gated on Go presence; if Go resolver didn't run, annotation absent (matches m160 C110 convention).
- **IV. Type-Driven Correctness**: ✅ Reuses the existing `ResolutionStep::GoSumFallback` enum discriminant. No new type introduced; only a count derived from existing typed data.
- **V. Specification Compliance**: ✅ Standards-native audit KEEP-NO-NATIVE justification: CDX 1.6 / SPDX 2.3 / SPDX 3.0.1 have no native "count of transitive resolutions that degraded to flat fallback" field. The `mikebom:*` annotation is the parity-bridging carve-out. Documented in the new C117 row per Principle V's mandate.
- **VI. Three-Crate Architecture**: ✅ Change contained to `mikebom-cli` (emission + parity + tests) + docs. No `mikebom-common` or `mikebom-ebpf` changes.
- **VII. Test Isolation**: ✅ New integration test uses per-test tempdir + fixture; no shared state.
- **VIII. Completeness**: ✅ **Improved**. This milestone directly serves the Completeness principle — closes a diagnostic gap that was making Go-graph output shape-variance opaque to consumers. Post-172, consumers can *affirmatively verify* Go graph completeness in ways they couldn't before.
- **IX. Accuracy**: ✅ **Improved**. The signal makes the accuracy story more honest — mikebom now explicitly reports "N modules had degraded resolution" instead of leaving the consumer to infer from graph shape.
- **X. Transparency**: ✅ **Directly serves Principle X**. New annotation makes fetch-environment degradation visible. Every failure mode that was previously invisible to consumers now surfaces via a concrete integer.
- **XI. Enrichment**: N/A — no enrichment path touched.
- **XII. External Data Source Enrichment**: N/A.

**Strict Boundaries check**: no new subprocess calls, no new network access, no new filesystem writes, no new `mikebom:*` annotation namespaces (this milestone introduces C117 within the existing `mikebom:go-transitive-*` cluster established by m160 C110/C111), no new Cargo dependencies.

**Verdict**: All principles pass. Zero violations. Milestone actively improves Principles VIII (Completeness), IX (Accuracy), and X (Transparency).

## Project Structure

### Documentation (this feature)

```text
specs/172-go-fallback-count/
├── plan.md              # This file
├── research.md          # Phase 0 — inventory of touchpoints + m160 C110 precedent trace
├── data-model.md        # Phase 1 — SbomEmission field + ModuleGraphMap accessor + parity row
├── quickstart.md        # Phase 1 — 3-scenario manual verification recipe
├── contracts/           # Phase 1 — wire-shape contract + Q1 emit-zero-explicit decision
├── checklists/          # Requirements checklist (spec-phase output)
└── tasks.md             # Phase 2 output (/speckit.tasks — NOT created by /speckit.plan)
```

### Source Code (repository root)

Files touched by this feature:

```text
mikebom-cli/
├── src/
│   ├── scan_fs/
│   │   ├── mod.rs                                   # Thread `go_transitive_fallback_count: Option<usize>` through ScanResult
│   │   └── package_db/golang/
│   │       └── graph_resolver.rs                    # Add `fallback_count()` getter on ModuleGraphMap or LadderSummary
│   ├── generate/
│   │   ├── mod.rs                                   # SbomEmission field addition + docs
│   │   ├── cyclonedx/
│   │   │   └── metadata.rs                          # C117 emission after existing C110 block (~lines 466-481)
│   │   └── spdx/
│   │       ├── annotations.rs                       # SPDX 2.3 C117 emission next to C110
│   │       └── v3_annotations.rs                    # SPDX 3 C117 emission next to C110
│   ├── cli/
│   │   └── scan_cmd.rs                              # Wire scan-result's fallback_count into SbomEmission struct
│   └── parity/
│       └── extractors/
│           ├── mod.rs                               # ADD ParityExtractor row + import c117_*
│           ├── cdx.rs                               # ADD c117_cdx helper
│           ├── spdx2.rs                             # ADD c117_spdx23 helper
│           └── spdx3.rs                             # ADD c117_spdx3 helper
├── tests/
│   └── go_fallback_count.rs                         # NEW integration test — 3 scenarios per FR-006

docs/
├── reference/
│   ├── sbom-format-mapping.md                       # ADD row C117 to Section C
│   └── reading-a-mikebom-sbom.md                    # Enrich the mikebom:go-transitive-coverage section with 5-step ladder + fallback-count jq recipe

# Golden fixtures (regenerated via env vars):
mikebom-cli/tests/fixtures/golden/{cyclonedx,spdx-2.3,spdx-3}/golang.*.json
```

**Structure Decision**: Standard three-crate workspace layout preserved. Change contained to `mikebom-cli` only (per Constitution VI).

## Complexity Tracking

No Constitution violations; no complexity to track. This is a small aggregation-and-emission feature that reuses existing m091 + m160 infrastructure.

## Phase 0 — Outline & Research

Research questions this feature raises:

1. **Where exactly does the m160 C110 emission chain thread the `go_transitive_coverage` value from the resolver to the emitters?** — need to identify the analogous plumbing path for the new fallback_count so both signals travel together.
2. **Is `ModuleGraphMap` still the right aggregation site**, or has the m160 `LadderSummary` intermediate structure superseded it as the right place for aggregate counts?
3. **How does the m160 C110 emission handle the "no Go scan happened" case** so we mirror it exactly for FR-002?
4. **Are there any Go-touching goldens beyond the primary `golang.*.json` set** (e.g., under `pkg_alias_binding/` per m171 T005's discovery)? If yes, they need regen too.
5. **Does `docs/reference/reading-a-mikebom-sbom.md` §3.5 currently exist** (as populated by m170 T029b)? If yes, this milestone enriches it. If not, this milestone creates it.

`research.md` will consolidate.

## Phase 1 — Design & Contracts

Design outputs for this feature:

- **data-model.md** — 3 affected entities:
  1. `ModuleGraphMap` gains a new `fn fallback_count(&self) -> usize` accessor
  2. `SbomEmission` gains a new `go_transitive_fallback_count: Option<usize>` field
  3. `EXTRACTORS` table gains new row C117

- **contracts/** — 2 files:
  1. `annotation-wire-shape.md` — the exact JSON shape of the new C117 annotation across all 3 formats (with sample values)
  2. `emission-gating.md` — Q1's emit-0-explicit decision, per-format emission rules, edge cases 3-6 from the spec

- **quickstart.md** — 3 verification paths per FR-006: (a) healthy scan → value=`"0"`; (b) degraded scan (`--offline` or `GOPROXY=off`) → value>0; (c) non-Go scan → annotation absent. Include the guac@ebb808e reproduction command as a bonus scenario.

- Agent context update via `.specify/scripts/bash/update-agent-context.sh claude` — appends the m172 no-new-dependencies note.

Post-design Constitution re-check: no drift from Phase 0 verdict. All principles remain green.
