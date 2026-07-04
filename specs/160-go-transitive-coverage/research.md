# Research: Milestone 160 (Go transitive-edge coverage)

**Date**: 2026-07-04
**Feature**: [spec.md](./spec.md)
**Plan**: [plan.md](./plan.md)

Phase-0 outline of unknowns + design decisions. Four ambiguities were resolved in `/speckit-clarify` (Q1–Q4, see spec §Clarifications). This research resolves the remaining plan-time technical questions.

## R1 — `mikebom:graph-completeness` (existing) vs `mikebom:go-transitive-coverage` (proposed): reuse or distinct?

**Decision**: **Distinct.** Milestone 160 introduces a NEW `mikebom:go-transitive-coverage` (C110) alongside the existing `mikebom:graph-completeness` (C104 per milestone 158, originally C44 per milestone 061). The two signals answer different consumer questions:

- **`mikebom:graph-completeness` (C104, existing)** — Document-scope. "Did we successfully build a top-level component graph in this scan?" Ecosystem-neutral; populated from `ScanDiagnostics.go_graph_completeness` today but conceptually spans all ecosystems. Values: `complete` / `partial` / `unknown`. Reason codes are graph-construction-level (e.g., "no go.sum found in Go workspace", "npm lockfile missing").

- **`mikebom:go-transitive-coverage` (C110, new)** — Document-scope. "For the Go modules we DID discover, what fraction had their per-module TRANSITIVE requires successfully resolved via the milestone-055 ladder?" Go-specific. Values: `complete` / `partial` / `unknown` (same vocabulary, different subject). Reason codes are ladder-step-level (`proxy-fetch-degraded`, `offline-mode`, `goproxy-off-in-chain`, `go-mod-graph-degraded`, `module-cache-empty-and-no-proxy`) per FR-005.

Concrete example that shows the two signals are non-redundant: a `test-podman` scan in online mode where `go.sum` is present + valid (so C104 = `complete`) but 15% of proxy fetches fail (so C110 = `partial` with `proxy-fetch-degraded: 45 of 300 modules unresolved`). C104's semantic can't express this — it operates one level up.

**Rationale**: Consumers gating on graph-completeness today (grype / trivy / Kusari Inspector) treat C104 as a coarse "trust the graph or not" signal. Overloading it with per-module ladder-step outcomes would break their existing switch statements. Introducing a distinct C110 preserves backward compatibility for C104 while adding the finer-grained transitive-coverage signal.

**Alternatives considered**:

- **A. Reuse C104 with extended reason vocab**: rejected. Would silently change C104's semantics; consumers with pinned reason-code allowlists would break. Also mixes two conceptually distinct signals (graph-existence vs per-module-ladder-outcome).
- **B. Rename C104 to `mikebom:package-graph-completeness` and add C110 for Go transitive**: rejected. C104 is a stable consumer surface; renaming is a breaking change requiring a MAJOR milestone rev. Not proportionate.
- **C. Introduce `mikebom:go-transitive-coverage` (chosen)**: additive, backward-compatible, semantically distinct. Q1 reason-code-driven decision rule applies cleanly.

## R2 — Per-module `mikebom:go-transitive-source` annotation emission point

**Decision**: Populate each Go module component's `PackageDbEntry.extra_annotations` inside `legacy::read()`'s per-module loop, immediately after the `ModuleGraphMap::entry(module_id)` lookup that already exists at `legacy.rs:1723` (verified via `grep -n "extra_annotations" legacy.rs`). The `ResolutionStep` enum is already stored on each `ModuleGraphEntry.source` field at `graph_resolver.rs:87`; milestone 160 exposes it via a new `ResolutionStep::as_wire_str()` method returning the kebab-case string.

Emission code shape:

```rust
if let Some(entry) = module_graph.entry(&module_id) {
    pkg.extra_annotations.insert(
        "mikebom:go-transitive-source".to_string(),
        serde_json::Value::String(entry.source.as_wire_str().to_string()),
    );
    if entry.source == ResolutionStep::None {
        pkg.extra_annotations.insert(
            "mikebom:go-transitive-unresolved-reason".to_string(),
            serde_json::Value::String(unresolved_reason.as_wire_str().to_string()),
        );
    }
}
```

**Rationale**: Reuses the existing `extra_annotations: BTreeMap<String, Value>` per-component channel (milestones 127/134/158/159 precedent). No new emission plumbing; just a new value insertion at an existing hot-spot.

**Alternatives considered**:

- **A. Emit at `scan_fs/mod.rs`'s post-resolution component-assembly loop**: rejected — the `ModuleGraphMap` reference doesn't survive to that layer; would require plumbing a resolver-diagnostics parameter through 3 function signatures.
- **B. Emit via a new post-emit pass reading the per-component annotation map**: rejected — decouples emission from the semantic source of truth. If the `ModuleGraphEntry.source` field's enum grows a variant, a separate pass would drift.

## R3 — Reason-class enum for `mikebom:go-transitive-unresolved-reason`

**Decision**: New enum `UnresolvedReasonClass` in `graph_resolver.rs`, sibling to the existing `ResolutionStep` enum. Kebab-case wire values per the milestone-055 `ErrorClass::as_str()` pattern at `graph_resolver.rs:305`. Closed vocabulary (per spec FR-003):

```rust
pub enum UnresolvedReasonClass {
    ProxyFetchTimeout,      // wire: "proxy-fetch-timeout"
    ProxyFetchNotFound,     // wire: "proxy-fetch-not-found"
    ProxyFetchForbidden,    // wire: "proxy-fetch-forbidden"
    ProxyOffInChain,        // wire: "proxy-off-in-chain"
    GoPrivateMatched,       // wire: "goprivate-matched"
    ModuleCacheMiss,        // wire: "module-cache-miss"
    UnknownError,           // wire: "unknown-error"
}
```

Population logic: the milestone-055 `StepResult<T>` at `graph_resolver.rs:271` already carries `ErrorClass` on `Failed(StepError)` outcomes. Milestone 160 adds a `From<&StepError> for UnresolvedReasonClass` impl mapping the existing `ErrorClass::{Timeout, Http404, Http4xx, ...}` to the new reason-class enum. This is a pure classifier — no new fetch-side behavior.

**Rationale**: Mirrors milestone 055's existing pattern. Zero-cost extension of the current type system.

**Alternatives considered**:

- **A. Reuse `ErrorClass` directly**: rejected — `ErrorClass` conflates the wire vocabulary with fetch-side internals (e.g., `Tls`, `Dns` classes). Consumers care about "why did this module end up unresolved" at a coarser granularity.
- **B. String field**: rejected — violates Principle IV (Type-Driven Correctness).

## R4 — `partial` vs `unknown` decision function (per Q1)

**Decision**: New function `compute_coverage(summary: &LadderSummary, ctx: &WorkspaceContext) -> GoTransitiveCoverage` in `graph_resolver.rs`. Reason-code-driven per Q1 clarification:

```rust
pub enum GoTransitiveCoverage {
    Complete,
    Partial(String),  // reason code + detail per FR-005 grammar
    Unknown(String),
}

pub fn compute_coverage(
    summary: &LadderSummary,
    ctx: &WorkspaceContext,
) -> GoTransitiveCoverage {
    // Priority 1: Unknown-fires-first (Q1 caution-first)
    if ctx.offline {
        return GoTransitiveCoverage::Unknown(
            "offline-mode: transitive edges from proxy fetches unavailable".into(),
        );
    }
    if ctx.goproxy.contains_off() {
        return GoTransitiveCoverage::Unknown(
            format!("goproxy-off-in-chain: GOPROXY={}", ctx.goproxy.as_wire_str()),
        );
    }
    if summary.go_mod_graph_degraded {
        return GoTransitiveCoverage::Unknown(
            "go-mod-graph-degraded: subprocess failed or returned partial output".into(),
        );
    }

    // Priority 2: Partial-fires-second
    if summary.missing_count > 0 {
        return GoTransitiveCoverage::Partial(
            format!(
                "proxy-fetch-degraded: {} of {} modules unresolved",
                summary.missing_count,
                summary.total_modules(),
            ),
        );
    }

    GoTransitiveCoverage::Complete
}
```

Note: `LadderSummary` at `graph_resolver.rs:222` gains ONE new field `go_mod_graph_degraded: bool` (previously the subprocess-degraded signal was implicit in `graph_count == 0`); the `total_modules()` helper is a sum-of-counters accessor already achievable via existing fields.

**Rationale**: Q1 reason-code-driven mechanics implemented as a straight priority ladder. Deterministic. Testable in isolation (10 unit tests per SC-008).

**Alternatives considered**:

- **A. Count-based threshold**: rejected per Q1 clarification.
- **B. Hybrid**: rejected per Q1 clarification.

## R5 — SC-001 verification methodology (per Q3)

**Decision**: A new gated integration test at `mikebom-cli/tests/go_transitive_coverage_audit.rs` (naming mirrors milestone-083's `transitive_parity_*.rs` pattern). Gated behind `MIKEBOM_TRANSITIVE_COVERAGE_AUDIT=1` env var per the milestone-083 external-tool test convention.

Test flow:

1. Shell out to `go mod graph` on the fixture repo (fixture path resolved via milestone-090's `MIKEBOM_FIXTURES_DIR` env var; expected fixture is `test-podman` under `transitive_parity/golang/test-podman/`).
2. Parse output into `HashSet<(String, String)>` of `(from_module@version, to_module@version)` edges.
3. Invoke the release binary with `mikebom sbom scan --path <fixture>` producing a CDX SBOM.
4. Extract mikebom's edges from `dependencies[].dependsOn[]` mapping component `bom-ref` back to `pkg:golang/<module>@<version>` PURL.
5. Compute intersection ratio `|mikebom_edges ∩ go_mod_graph_edges| / |go_mod_graph_edges|`.
6. Assert ratio ≥ 0.90 (SC-001) with a diagnostic-friendly failure message listing 20 sample missing edges.

**Rationale**: Per Q3, `go mod graph` is the ground-truth generator. Direct shell-out preserves reproducibility.

**Alternatives considered**:

- **A. Cache `go mod graph` output in the fixture repo as a golden file**: rejected — `go mod graph` output changes when Go SDK ships new versions of modules-under-test; stale golden becomes silent SC-001 drift. Preferring live invocation.

## R6 — FR-006 empirical investigation methodology

**Decision**: T014–T016 (forthcoming in tasks.md) will follow a scan-diff-hypothesize-fix loop:

1. Scan `test-podman` fixture in online mode; emit CDX; record per-module `mikebom:go-transitive-source` values via jq.
2. Shell out to `go mod graph`; diff module-edge sets.
3. For each missing edge, inspect: which ladder step ran for the source module? If `proxy-fetch`, was the fetch response parsed correctly? Check the 3 FR-006 candidate root causes in order:
   - **FR-006a** — `// indirect` deps being dropped by `proxy_fetch.rs::parse_module_mod`. Inspect via debug-log injection in the parser.
   - **FR-006b** — Milestone-091 go.sum fallback not being wired to trigger on proxy-fetch-failures (only on step 3 → step 5 fallthrough, not on step 3 → error → step 5). Inspect via `ResolutionStep::None` count vs `ResolutionStep::GoSumFallback` count.
   - **FR-006c** — Not applicable in online mode; will be tested via offline scan in a separate cycle.
4. Land the fix. Re-scan. Verify edge-coverage improvement.

Concrete anchoring: the 5 missing edges from `containernetworking/plugins@v1.9.1` are the SC-002 spot-checks. If T014–T016 concludes some are legitimately platform-filtered, SC-002 permits annotating them per its "OR annotated with reason" clause.

**Rationale**: Investigation-first when the exact fix isn't knowable at spec time (milestone-158 precedent).

**Alternatives considered**:

- **A. Full rewrite of the ladder step 3 (proxy-fetch)**: rejected — the milestone-055 code is stable; targeted fixes are lower-risk.

## R7 — Parity catalog row allocation

**Decision**: Reserve C108/C109 for per-component + C110/C111 for document-scope, continuing the milestone-158 (C104/C105) + milestone-159 (C106/C107) numbering:

- **C108**: `mikebom:go-transitive-source` (per-component, `Directionality::SymmetricEqual`, `order_sensitive: false`)
- **C109**: `mikebom:go-transitive-unresolved-reason` (per-component, `Directionality::SymmetricEqual`, `order_sensitive: false`)
- **C110**: `mikebom:go-transitive-coverage` (document-scope, `Directionality::SymmetricEqual`, `order_sensitive: false`)
- **C111**: `mikebom:go-transitive-coverage-reason` (document-scope, `Directionality::SymmetricEqual`, `order_sensitive: false`)

All 4 rows follow the milestone-127/158 macro pattern using `cdx_anno!` / `spdx23_anno!` / `spdx3_anno!` at `mikebom-cli/src/parity/extractors/{cdx,spdx2,spdx3}.rs`. Registration entries land at `mikebom-cli/src/parity/extractors/mod.rs` in the same block as C104/C105.

**Rationale**: Continues the deterministic slot-allocation pattern established since milestone 127. No collisions with milestone 158 (C104/C105) or milestone 159 (C106/C107).

## Open items (none blocking)

All research questions resolved. Ready for Phase 1 (data-model.md + contracts/).
