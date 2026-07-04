# Implementation Plan: Go transitive-edge coverage investigation + gap surface

**Branch**: `160-go-transitive-coverage` | **Date**: 2026-07-04 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/160-go-transitive-coverage/spec.md`

## Summary

Milestone 155–159 audit against `kusari-sandbox/test-podman` (2026-07-03) surfaced that mikebom's Go transitive-edge coverage vs `go mod graph` is **52.2%** in online mode and **7.29%** in offline mode — a completeness failure (Principle VIII) that silently drops ~half the closure from downstream vulnerability scans.

The fix is investigation-heavy: FR-006a/b/c prescribe root-cause classes to look for during T014–T016 empirical work (parser-drop of `// indirect` requires, milestone-091 go.sum fallback not being exercised on proxy-fetch failures, offline-mode per-fetch warn spam vs at-config-time early skip). The spec's `containernetworking/plugins@v1.9.1` example — 5 concrete missing direct requires, 2 of which are cross-platform — is the load-bearing SC-002 spot-check.

**Technical approach**:

1. **Extend milestone-055 `ResolutionStep` enum** with per-module attribution surfaced back to the emitter (`legacy::read`'s per-module `PackageDbEntry.extra_annotations` insertion path).
2. **New per-component annotation `mikebom:go-transitive-source`** (universal per Q2) naming which of the 4 ladder steps (`go-mod-graph` / `module-cache` / `proxy-fetch` / `go-sum-fallback` / `unresolved`) resolved that module's requires.
3. **New conditional per-component annotation `mikebom:go-transitive-unresolved-reason`** naming the reason class when source == `unresolved` (per FR-003, closed 7-code vocab).
4. **Decision-critical: document-scope signal**. Milestone 061 registered `mikebom:graph-completeness` (C44 originally, C104 after milestone 158's ecosystem-broadening) as a Go-specific document-scope signal populated from the resolver's diagnostics. Milestone 160 must EITHER (a) reuse this existing annotation with extended semantics, OR (b) introduce a distinct `mikebom:go-transitive-coverage`. Resolved in Phase 0 research (R1) — see research.md decision.
5. **Fix the FR-006 root causes** discovered during T014–T016 empirical investigation.

Q1–Q4 clarifications (spec §Clarifications) lock: reason-code-driven `partial`/`unknown` decision rule; universal per-component emission; `go mod graph` as SC-001 ground-truth; closed-but-extensible reason vocab.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–159; no nightly required for this user-space-only work).

**Primary Dependencies**: Existing only — `reqwest = "0.12"` (workspace, `default-features = false, features = ["json", "rustls-tls", "blocking"]`) reused by `proxy_fetch.rs`; `tokio` (workspace) for the semaphore-driven parallel fetches; `serde`/`serde_json` (annotation values); `tracing` (info-level ladder summary + FR-010 log line); `anyhow`/`thiserror` (error propagation). The `GraphResolverConfig.fetch_concurrency = 16` at `graph_resolver.rs:344` (milestone-055 FR-008a) is preserved unchanged. **Zero new Cargo dependencies.**

**Storage**: N/A — all state in-process per scan; matches every milestone since 002. Per-module `ModuleGraphEntry.source: ResolutionStep` is already stored in `ModuleGraphMap.entries: HashMap<ModuleId, ModuleGraphEntry>`; this milestone surfaces it to the emitter, not to disk.

**Testing**: `cargo +stable test --workspace --no-fail-fast` per Constitution Development Workflow. New tests live in three tiers per milestone-055/091/158 precedent:
- Unit tests in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs` (module-inline `#[cfg(test)]`) covering the reason-code classification + `partial`/`unknown` decision rule.
- Integration test at `mikebom-cli/tests/go_transitive_coverage.rs` (per SC-009) exercising the release binary against a synthetic Go workspace with a `wiremock`-style local HTTP proxy (milestone 055 already vendored the pattern per `wiremock = "0.6"` dev-dep at `mikebom-cli/Cargo.toml`).
- SC-001 audit-fixture regression against the milestone-090 fixture-repo's `test-podman` (verified via the milestone-090 fixture cache at `~/.cache/mikebom/fixtures/<pinned-sha>/transitive_parity/`; the fixture repo may need a golang-fixture update if T014–T016 reveals a new anchor).

**Target Platform**: Linux + macOS + Windows dev hosts (per milestones 100/101). No new host-portability concerns — the `go` binary shell-out at `go_mod_graph.rs:81–158` already handles Windows path canonicalization.

**Project Type**: CLI (Rust workspace with 3 crates per Constitution Principle VI).

**Performance Goals**: Preserve milestone-055 posture — 16-way concurrent proxy fetches with 10s connect + 30s total timeout per `graph_resolver.rs:342–344`. FR-007 explicit. No new perf regression target introduced; scan-time is expected to hold within milestone-090 fixture baseline (verified via the existing perf-test suite at `mikebom-cli/tests/perf_*`).

**Constraints**: **No new Cargo dependencies** (FR spec assumption). **No subprocess calls added** (`go mod graph` shell-out is milestone 055's existing `go_mod_graph.rs:81` invocation; the SC-001 test harness uses the same subprocess). **No `.unwrap()` in production** per Constitution Principle IV. **Standards-native precedence** per Principle V — the FR-008 audit conclusion (no CDX/SPDX-native ecosystem-completeness field exists as of 2026-07-04) permits the `mikebom:go-transitive-*` prefix subject to R1 resolution.

**Scale/Scope**: ~300 Go components per `test-podman` scan; the milestone-158 pattern (universal per-component annotation) added ~300 annotations per scan without measurable perf regression, so US2's universal `mikebom:go-transitive-source` addition has a known-safe budget. Golden regeneration impact: 11 milestone-090 fixtures × 3 formats = 33 goldens, of which 30 must be byte-identical (SC-003) and 3 (the `golang` fixture) will change.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Principle-by-principle assessment

**I. Pure Rust, Zero C** — ✅ PASS. All work is user-space Rust in `mikebom-cli`. No FFI. No C dependencies added. `mikebom-ebpf` untouched.

**II. eBPF-Only Observation** — ✅ N/A. Milestone 160 does not touch discovery — it's a parity/emission-layer refinement of what mikebom already reads from `go.mod`/`go.sum` + proxy fetches. No new discovery source.

**III. Fail Closed** — ✅ PASS. The FR-006c `--offline` early-skip is a fail-truthful design: rather than silently emit empty per-module edges, the doc-scope annotation reports `unknown` with reason `offline-mode`. Per-module `mikebom:go-transitive-source = "unresolved"` + `mikebom:go-transitive-unresolved-reason = "proxy-off-in-chain"` is the fail-transparent posture.

**IV. Type-Driven Correctness** — ✅ PASS. `ResolutionStep` enum at `graph_resolver.rs:63` already implements this pattern; milestone 160 extends it with the new `UnresolvedReasonClass` enum (per data-model.md). All new annotation values are enum-backed with a single `as_str()` serializer, matching the milestone-055 `ErrorClass` at `graph_resolver.rs:292` shape. No `.unwrap()` in production paths.

**V. Specification Compliance** — ⚠️ GATE. Two audit checks required:
- **Native-first check**: FR-008 explicitly documents the audit — no CDX 1.6 or SPDX 3.0.1 native field for "SBOM-completeness-per-ecosystem" as of 2026-07-04. The `mikebom:go-transitive-*` prefix is compliant per Principle V's "parity-bridging" clause.
- **Existing-mikebom-annotation check** (added by this plan): milestone 061's `mikebom:graph-completeness` (C44) is Go-specific per its historical semantics. Phase 0 R1 must resolve whether milestone 160 reuses vs introduces distinct. Failure to resolve R1 blocks Phase 1.

**VI. Three-Crate Architecture** — ✅ PASS. All changes are in `mikebom-cli`; no new crates. `mikebom-common` gains no new types.

**VII. Test Isolation** — ✅ PASS. New unit tests are pure logic (reason-code classifier + `partial`/`unknown` decision rule). Integration test uses `wiremock` — no eBPF privilege. SC-001 audit uses a shell-out to `go mod graph` but is gated behind the `MIKEBOM_TRANSITIVE_PARITY_AUDIT=1` env var (matches the milestone-083 pattern for external-tool tests).

**VIII. Completeness** — ✅ CENTRAL. This milestone directly addresses the completeness gap discovered in the milestone-155–159 audit (52.2% → ≥90% edge coverage). Unattributed content (unresolved modules) is surfaced explicitly via `mikebom:go-transitive-source = "unresolved"` + reason.

**IX. Accuracy** — ✅ PASS. FR-006a fixes false positives (parser-drop of legitimate `// indirect` edges was over-filtering, not under-filtering). No new phantom edges introduced. SC-002's 5 specific missing edges are validated against `go mod graph` ground truth.

**X. Transparency** — ✅ CENTRAL. Every ladder-step outcome is annotated (universal per Q2); every unresolved case names a reason class; document-scope signal (Q1 caution-first) reports `unknown` when we can't measure. Consumer trust surface is explicit.

**XI. Enrichment** — ✅ N/A. Milestone 160 does not fetch new external data — it uses what the milestone-055 ladder already fetches.

**XII. External Data Source Enrichment** — ✅ PASS. `go mod graph` shell-out is milestone 055's existing enrichment source; unchanged. No new external source introduced.

### Strict Boundary compliance

**§1 (No lockfile discovery)** — ✅ N/A. `go.sum` is used only for edge INTERSECTION (post-fetch), not for component discovery. Milestone 055's contract, preserved.

**§2 (No MITM proxy)** — ✅ PASS. HTTP fetches remain via `reqwest::blocking::Client` per milestone 055.

**§3 (No C code)** — ✅ PASS.

**§4 (No `.unwrap()` in production)** — ✅ PASS. New code follows the milestone-055/091 pattern with `anyhow::Result` + `?` propagation.

**§5 (No file-tier duplicates in default mode)** — ✅ N/A. File-tier emission not touched.

### Gate result

Constitution Check **PASSES** subject to R1 (the `mikebom:graph-completeness` vs `mikebom:go-transitive-coverage` semantic-distinction decision) being resolved in Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/160-go-transitive-coverage/
├── plan.md              # This file
├── research.md          # Phase 0 output (R1–R7 below)
├── data-model.md        # Phase 1 output (entities: ladder step, reason class, 4 annotations)
├── quickstart.md        # Phase 1 output (contributor path: build+test+audit)
├── contracts/
│   └── annotations.md   # Phase 1 output (per-format wire shapes for the 4 new annotations)
├── checklists/
│   └── requirements.md  # Already exists from /speckit-specify
└── tasks.md             # /speckit-tasks output (NOT created by this command)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   ├── scan_fs/
│   │   └── package_db/
│   │       └── golang/
│   │           ├── graph_resolver.rs     # EXTEND: LadderCounters → LadderSummary already has fields; per-module `source` already stored; NEW: UnresolvedReasonClass enum; NEW: partial/unknown decision fn
│   │           ├── legacy.rs             # EDIT: read() populates each PackageDbEntry.extra_annotations with mikebom:go-transitive-source + conditional -unresolved-reason
│   │           ├── proxy_fetch.rs        # EDIT: fetch_module_mod returns the reason class on failure (currently just returns Err); UnresolvedReasonClass::from_err
│   │           ├── go_mod_graph.rs       # unchanged (subprocess success/degraded already surfaced via ResolutionStep)
│   │           └── mod.rs                # unchanged
│   ├── scan_fs/
│   │   └── mod.rs                        # EDIT: ScanDiagnostics.go_graph_completeness already exists; milestone 160 either reuses OR adds separate go_transitive_coverage field (per R1 decision)
│   ├── cli/
│   │   └── scan_cmd.rs                   # EDIT: doc-scope annotation emission wiring (uses R1's decision on reuse-vs-new field)
│   └── parity/
│       └── extractors/
│           ├── mod.rs                    # EDIT: register C108/C109 per-component + (conditionally on R1) C110/C111 document-scope rows
│           ├── cdx.rs                    # EDIT: cdx_anno!() macro invocations for the 2 (or 4) new rows
│           ├── spdx2.rs                  # EDIT: spdx23_anno!() macro invocations
│           └── spdx3.rs                  # EDIT: spdx3_anno!() macro invocations
└── tests/
    └── go_transitive_coverage.rs         # NEW: SC-009 integration test (wiremock-driven mock proxy + release-binary invocation)
```

**Structure Decision**: Milestone 160 is a targeted extension of milestone 055's Go transitive-edge resolver plus the milestone-071 parity catalog. No new crates. No new source-tree directories. The two edit hot-spots are `mikebom-cli/src/scan_fs/package_db/golang/{graph_resolver,legacy,proxy_fetch}.rs` (emission + reason classification) and `mikebom-cli/src/parity/extractors/*.rs` (catalog registration).

## Complexity Tracking

*No Constitution violations. Section not applicable.*

## Phase completion status

- ✅ **Phase 0 (research)** — see `research.md` for R1–R7 resolutions.
- ✅ **Phase 1 (design & contracts)** — see `data-model.md`, `contracts/annotations.md`, `quickstart.md`.
- 🔲 **Phase 2 (task decomposition)** — deferred to `/speckit-tasks`.

## Post-design constitution re-check

R1 resolved in research.md as: **keep `mikebom:graph-completeness` (C104) unchanged; introduce `mikebom:go-transitive-coverage` (C110) as a distinct signal** — the semantics are non-overlapping (C104 = "did we get the graph at all?"; C110 = "what fraction of Go modules had their transitive requires resolved via the ladder, and via which step?"). Rationale + alternatives detailed in R1.

With R1 resolved, Constitution Check re-passes.

## Notes

- The plan preserves the milestone-055 concurrency + timeout posture unchanged (FR-007 explicit).
- The FR-006a/b/c investigation-heavy tasks (T014–T016 in the forthcoming tasks.md) will need concrete test-podman fixture access via the milestone-090 fixture cache; the empirical work is expected to take 3–5 iterations of scan-diff-hypothesize-fix.
- The SC-001 ≥90% target is empirically-adjustable per Assumptions §7 in the spec — if T014–T016 investigation reveals the FR-006 root causes are more complex than anticipated, revising SC-001 to a demonstrated-achievable floor is a legitimate outcome (milestone-156/157/158/159 precedent).
