# Implementation Plan: Opt-in Go cache warming for accurate transitive graphs

**Branch**: `173-warm-go-cache` | **Date**: 2026-07-08 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/173-warm-go-cache/spec.md`

## Summary

**Primary requirement**: add opt-in Go cache-warming via `--warm-go-cache=<off|per-workspace>` (default `off`) with a concurrency knob `--warm-go-cache-concurrency <N>` (default `4`, `0`=auto, values clamped to 32). Warming runs `go mod download` per discovered Go workspace **before** the m055/m091 transitive-resolution ladder runs, so step 1 (`go mod graph`) can find every module locally and produce true parent-child topology instead of falling through to step 5's flat go.sum fallback. Preserves m172's `mikebom:go-transitive-fallback-count` (C117) diagnostic — warming is opt-in specifically so the count remains an actionable signal.

**Technical approach**: Insert a pre-resolver warming phase into the Go reader; emit two new doc-scope annotations (mode + failed); add one advisory log line in the default-flag + non-offline + C117>0 case.

Six surgical changes:

1. **New warming module `mikebom-cli/src/scan_fs/package_db/golang/warm_cache.rs`** (~200 lines). Exposes `warm_workspaces(workspace_paths: &[PathBuf], mode: CacheWarmingMode, concurrency: usize, per_workspace_timeout: Duration, overall_budget: Duration) -> CacheWarmingResult`. Uses `std::thread::spawn` + `mpsc::sync_channel` for concurrency bounding — mirrors the m055/m091 `parallel_fetch` pattern at `graph_resolver.rs:1001` verbatim (worker pool sized `concurrency.max(1).min(n_workspaces)`; workers pull jobs from a bounded channel; results ship back via a collector channel). Per-workspace invocation shape: `Command::new("go").args(["mod", "download"]).current_dir(&workspace_path)` (env inherited from parent per FR-008). Per-workspace timeout enforced via the same spawn-and-recv-timeout pattern as `run_go_mod_graph` at `go_mod_graph.rs:81-158`. Failures classify into six reason classes (FR-007) via a new `WarmingFailureReason` enum. No tokio — keeps the warmer synchronous like every other Go-subprocess call in the codebase.

2. **Integration point in `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs`** (~10 lines). Before line 1671 (where `graph_map.coverage()` is aggregated), if warming mode is `per-workspace` AND non-offline AND `signals.workspace_paths.is_empty()` is false, invoke `warm_workspaces(...)` and attach the result to `GoScanSignals` for downstream annotation emission. The warming phase strictly precedes the transitive-resolver ladder for each workspace so step 1 sees the warmed cache.

3. **Plumbing** — same chain as m172 C117: add `cache_warming_result: Option<CacheWarmingResult>` to `GoScanSignals` (`legacy.rs:1384`) → `ScanDiagnostics.go_cache_warming` (`package_db/mod.rs:308`) → `ScanResult.go_cache_warming` (`scan_fs/mod.rs:98`) → `ScanArtifacts.go_cache_warming` (`generate/mod.rs:50`) → `SbomEmission.go_cache_warming` → emitters. Also thread the effective `CacheWarmingMode` value (may be `Off`/`PerWorkspace`/`OfflineInhibited`) so FR-011's mode annotation can fire.

4. **CLI surface in `mikebom-cli/src/cli/scan_cmd.rs`** (~40 lines). Two new `#[arg(long)]` fields: `warm_go_cache: WarmGoCacheMode` (clap `ValueEnum` with `Off`/`PerWorkspace`; default `Off`) and `warm_go_cache_concurrency: u32` (default `4`). Use `matches.value_source(...) != Some(ValueSource::DefaultValue)` to distinguish "operator explicitly passed" from "took the default" for the FR-004 advisory-suppression rule. Wire the parsed values into the scan pipeline's config surface.

5. **Emit C118 (mode) + C119 (failed) annotations across all 3 formats.** Follow the m172 C117 pattern precisely:
   - `cyclonedx/metadata.rs`: two new `if let Some(...) { properties.push(json!(...)) }` blocks immediately after the C117 emission block.
   - `spdx/annotations.rs` and `spdx/v3_annotations.rs`: parallel emissions using the existing envelope push helpers.
   - Six new parity extractor helpers (`c118_cdx`/`c118_spdx23`/`c118_spdx3`/`c119_cdx`/`c119_spdx23`/`c119_spdx3`) using `cdx_anno!`/`spdx23_anno!`/`spdx3_anno!` with `document` scope.
   - Two new EXTRACTORS rows in `parity/extractors/mod.rs` (C118 + C119, both `SymmetricEqual`, `order_sensitive: false` for C118 — scalar value; `order_sensitive: false` for C119 too since the array will be sorted at emission time).

6. **Advisory log at the CLI-emission-tail site.** In `mikebom-cli/src/cli/scan_cmd.rs` immediately after the SBOM write, check the `AdvisoryContext` three-input predicate (`fallback_count > 0`, `warm_go_cache_was_default`, `!offline`) and emit exactly one `tracing::info!` line matching the stable substring from spec §US2 Independent Test. Also add the "Go-workspaces-empty ⇒ suppress" rule per FR-009.

**Docs**:
- Two new rows (C118 + C119) in `docs/reference/sbom-format-mapping.md` after C117, each with the Constitution Principle V "KEEP-NO-NATIVE" audit.
- New subsection in `docs/reference/reading-a-mikebom-sbom.md` positioned alongside the m172 C117 section, explaining the mode annotation + failed annotation + advisory-log discovery mechanism. Include the "prime the cache" recipe requested at the end of the m172 conversation.

**Golden regeneration**: run `MIKEBOM_UPDATE_*_GOLDENS=1` for CDX / SPDX 2.3 / SPDX 3. Every Go-touching golden gets +1 line for the new C118 `mikebom:go-cache-warming-mode = "off"` annotation (since goldens run in default mode). The C119 failed-annotation is conditional and won't fire on healthy golden fixtures. Non-Go goldens MUST show zero delta (SC-004).

**Blast radius**: ~200 lines new warmer + ~50 lines plumbing + ~40 lines CLI + ~30 lines emitters + ~30 lines parity extractors + ~80 lines docs + 3 golden updates (each with 4–8 line delta for the new C118 annotation) + 1 new integration test at `mikebom-cli/tests/warm_go_cache.rs`.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–172; no nightly required). Synchronous throughout — the warmer uses `std::thread::spawn` + `std::sync::mpsc` like the m055/m091 parallel HTTP fetcher, not tokio.

**Primary Dependencies**: Existing only — `std::process::Command` (subprocess spawn; same pattern as `go_mod_graph.rs:81-158`), `std::thread` + `std::sync::mpsc` (concurrency; same pattern as `graph_resolver.rs:1001-1050`'s `parallel_fetch`), `serde`/`serde_json` (annotation values), `tracing` (advisory + warn logs), `anyhow`/`thiserror` (error propagation + reason-class enum), `clap` (the two new `Args`-derive flags with `ValueEnum` for the mode flag). **Zero new Cargo dependencies. No tokio in the warmer.**

**Storage**: N/A — cache warming mutates the operator's `$GOMODCACHE` (owned by the Go toolchain, not mikebom). mikebom's own state remains in-process per scan; the warming-result record dies at scan end after emission.

**Testing**: `cargo test` — 1 new integration test at `mikebom-cli/tests/warm_go_cache.rs` covering US1 (before/after C117 delta on a hermetic mock-proxy fixture), US2 (advisory-log-fires-exactly-once), US3 (graceful degradation on malformed workspace). Existing golden regression suites + m071 parity gate cover byte-identity guarantees.

**Target Platform**: Linux (CI), macOS (dev), Windows (m100-experimental). Cross-platform-safe: `go mod download` behavior is uniform across `go` toolchain builds. No POSIX-only assumptions.

**Project Type**: cli (mikebom sbom-generation CLI).

**Performance Goals**: SC-005 caps the milestone's own test fixture at 60 seconds wall-clock. Production scan times are proxy-fetch-bound and outside mikebom's control. Concurrency default of 4 per Q1 clarification balances throughput vs GOPROXY politeness.

**Constraints**: SC-004 correctness gate — non-Go scans MUST produce byte-identical SBOMs to pre-173 baseline. Enforced by: (a) FR-002 gate on Go workspace presence; (b) FR-011 mode annotation only emits when scan target has Go components; (c) FR-009 advisory-log suppression for non-Go.

**Scale/Scope**: Small-to-medium. ~10 source files edited/created on the mikebom-cli side, 1 new integration test, 2 docs files edited, 3 Go golden updates.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Pure Rust, Zero C**: ✅ No new Cargo dependencies. Warmer is pure Rust (`tokio::process::Command` + `Semaphore`).
- **II. eBPF-Only Observation**: ✅ `mikebom-ebpf` untouched.
- **III. Fail Closed**: ✅ Every failure mode has a stable reason class + graceful degradation (FR-005). The scan never aborts due to warming failure; the operator gets a diagnostic annotation instead. `--offline` conflict emits a warn log and falls back to non-warming (FR-003).
- **IV. Type-Driven Correctness**: ✅ New `CacheWarmingMode` enum (three variants) is the single source of truth for mode signaling; `CacheWarmingResult` + `WarmingFailureReason` closed enum (six variants) encode every failure class. Concurrency `usize` value validated at parse time.
- **V. Specification Compliance**: ⚠️ **AUDITED — KEEP-NO-NATIVE justified**. Two new `mikebom:*` annotations (C118 + C119) — each audited against CDX 1.6 / SPDX 2.3 / SPDX 3.0.1 for existing native constructs. C118 (cache-warming mode) has no standards-native equivalent (SBOM formats don't model "which tool-side cache-warming mode was active during scan"). C119 (per-workspace failure records) — CDX `component.evidence.identity[]` is the closest but is per-component evidence, not doc-scope failure aggregation. SPDX `Package.filesAnalyzed = false` is a per-package flag with unrelated semantics. Rejected alternatives documented in the two new mapping-table rows.
- **VI. Three-Crate Architecture**: ✅ Change contained to `mikebom-cli` (warming module + CLI + emission + parity + tests) + docs. No `mikebom-common` or `mikebom-ebpf` changes.
- **VII. Test Isolation**: ✅ Integration test uses per-test tempdir + per-test hermetic mock-proxy (fixed port via `tokio::net::TcpListener::bind("127.0.0.1:0")` for concurrent-safe binding). No shared state.
- **VIII. Completeness**: ✅ **Improved**. Post-173 an operator running with the flag can achieve *higher* completeness on monorepos than the pre-173 tool ever offered — moves from "step 5 flat topology" to "step 1 true topology" for the same fixture.
- **IX. Accuracy**: ✅ **Improved**. The warmed-cache path produces authoritative `go mod graph` output; the fallback path produces flat approximation. Warming improves accuracy of what's already emitted.
- **X. Transparency**: ✅ **Directly serves Principle X**. Two new doc-scope annotations expose the operator's chosen warming mode + any failures. Combined with m172's C117, the SBOM becomes fully self-describing about how its Go graph was resolved.
- **XI. Enrichment**: N/A — no enrichment path touched. Warming is pre-resolver preparation, not post-resolver enrichment.
- **XII. External Data Source Enrichment**: N/A.

**Strict Boundaries check**:
- **New subprocess**: ✅ **`go mod download`** — added to the whitelist. Precedent: existing `go mod graph` (m055/m091) + `go mod why -m -vendor` (m112). FR-012 explicitly forbids `go mod tidy`, `go build`, `go test`, `go generate` — no code execution, no operator-file mutation.
- **New network access**: ✅ Only via `go mod download`, which mikebom does not control directly. The operator's `$GOPROXY` env is preserved (FR-008).
- **New filesystem writes**: ⚠️ **`$GOMODCACHE` mutation** — `go mod download` writes `.mod` and `.zip` files to the operator's module cache. This is documented in the spec (Assumptions section) as an intentional trade-off; the flag is opt-in specifically so operators consent to this side effect.
- **New `mikebom:*` annotation namespaces**: ⚠️ Two new (C118 + C119), both within the existing `mikebom:go-*` cluster (siblings of C110/C111/C112/C117). No new namespace prefix.
- **New Cargo dependencies**: ✅ Zero.

**Verdict**: All principles pass; the strict-boundary items (subprocess whitelist expansion, filesystem writes to `$GOMODCACHE`, two new annotations) are (a) minimal-additions to existing categories mikebom already occupies and (b) audited against Principle V's standards-native mandate. Milestone actively improves Principles VIII/IX/X.

## Project Structure

### Documentation (this feature)

```text
specs/173-warm-go-cache/
├── plan.md              # This file
├── research.md          # Phase 0 — subprocess + concurrency + Go toolchain research
├── data-model.md        # Phase 1 — CacheWarmingMode, CacheWarmingResult, WarmingFailureReason, AdvisoryContext
├── quickstart.md        # Phase 1 — 3-scenario manual verification recipe + hermetic mock-proxy setup
├── contracts/           # Phase 1 — annotation wire shapes + advisory-log stable substring
├── checklists/          # Requirements checklist (spec-phase output)
└── tasks.md             # Phase 2 output (/speckit.tasks — NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   ├── cli/
│   │   └── scan_cmd.rs                       # +40 lines: two new flags + advisory-log emission
│   ├── generate/
│   │   ├── mod.rs                            # ~5 lines: ScanArtifacts.go_cache_warming field
│   │   ├── cyclonedx/
│   │   │   ├── metadata.rs                   # ~20 lines: two new emission blocks for C118 + C119
│   │   │   ├── builder.rs                    # ~15 lines: builder field + setter
│   │   │   └── mod.rs                        # ~3 lines: .with_go_cache_warming(...) wiring
│   │   ├── spdx/
│   │   │   ├── annotations.rs                # ~15 lines: two new envelope pushes
│   │   │   ├── v3_annotations.rs             # ~15 lines: two new envelope pushes
│   │   │   ├── document.rs                   # ~2 lines: two ScanArtifacts construction sites
│   │   │   ├── v3_document.rs                # ~1 line: one ScanArtifacts construction site
│   │   │   └── {mod,packages,relationships}.rs  # ~1 line each: test-harness stubs
│   │   └── openvex/mod.rs                    # ~1 line: test-harness stub
│   ├── parity/
│   │   └── extractors/
│   │       ├── mod.rs                        # ~4 lines: C118 + C119 rows + import
│   │       ├── cdx.rs                        # ~2 lines: c118_cdx + c119_cdx helpers
│   │       ├── spdx2.rs                      # ~2 lines: c118_spdx23 + c119_spdx23 helpers
│   │       └── spdx3.rs                      # ~2 lines: c118_spdx3 + c119_spdx3 helpers
│   └── scan_fs/
│       ├── mod.rs                            # ~5 lines: ScanResult.go_cache_warming field + wiring
│       └── package_db/
│           ├── mod.rs                        # ~5 lines: ScanDiagnostics.go_cache_warming field
│           └── golang/
│               ├── warm_cache.rs             # NEW ~200 lines: warmer module
│               ├── legacy.rs                 # ~10 lines: pre-resolver warm_workspaces() call
│               └── mod.rs                    # ~1 line: pub mod warm_cache
└── tests/
    └── warm_go_cache.rs                      # NEW ~250 lines: 3-scenario integration suite

docs/reference/
├── sbom-format-mapping.md                    # ~2 lines: C118 + C119 rows
└── reading-a-mikebom-sbom.md                 # ~60 lines: new subsection + table/list/changelog entries

mikebom-cli/tests/fixtures/golden/
├── cyclonedx/golang.cdx.json                 # +4 lines (C118 mode annotation)
├── spdx-2.3/golang.spdx.json                 # +5-6 lines (envelope-wrapped mode)
└── spdx-3/golang.spdx3.json                  # +7-8 lines (typed Annotation graph element)
```

**Structure Decision**: The feature is a pre-resolver preparation phase in the Go reader, plus emission plumbing that mirrors m172's C117 wiring verbatim. The warming code lives in `mikebom-cli/src/scan_fs/package_db/golang/warm_cache.rs` as a new module — the Go readers are the correct home because warming is a Go-toolchain interaction. The CLI surface is two new flags in `scan_cmd.rs`. Emission follows the m172 6-emitter pattern (CDX / SPDX 2.3 / SPDX 3, plus their test-harness `ScanArtifacts` construction sites) — the compiler will flag each missing field.

## Complexity Tracking

No constitution violations to justify. The plan reuses existing patterns (m055/m091 tokio Semaphore concurrency; m172 emission/plumbing; m160 doc-scope annotation shape) with minimal additions.
