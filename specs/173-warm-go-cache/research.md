# Phase 0 Research: Go Cache Warming (m173)

**Feature**: 173-warm-go-cache
**Date**: 2026-07-08

Six research questions resolved to unblock implementation. Each question is a decision point with the chosen answer + rationale + rejected alternatives.

---

## R1 — Which Go subcommand actually populates the module cache?

**Decision**: `go mod download` (per-workspace, no additional flags beyond `-x` optional for verbose logging).

**Rationale**:
- `go mod download` explicitly fetches every module in the current workspace's transitive closure and writes `.mod`, `.zip`, and `.info` files to `$GOMODCACHE/cache/download/`.
- Does NOT modify `go.mod` or `go.sum` (unlike `tidy`, which we explicitly forbid via FR-012).
- Does NOT execute code from downloaded modules (unlike `go build ./...`, `go test`, `go generate`).
- Idempotent — running twice on a warm cache is a fast no-op.
- Uses the operator's `$GOPROXY` chain and honors `GOFLAGS`, `GOPRIVATE`, `GONOSUMCHECK`, etc.
- After `go mod download` completes for a workspace, a subsequent `go mod graph` call (m055 step 1) reads modules directly from `$GOMODCACHE` — no network needed for the graph step itself.

**Alternatives considered**:
- **`go mod tidy`**: Rejected — mutates `go.mod` and `go.sum`, which violates the "don't touch operator's source tree" principle. Explicitly forbidden per FR-012.
- **`go build ./...`**: Rejected — runs Go code (build directives, generate directives), which is a massive trust escalation and often has non-deterministic side effects.
- **`go mod download -x`**: Considered — the `-x` flag makes the download log every URL fetched. Useful for debugging but noisy in normal operation. **Decision**: don't add by default; may add as an internal debug flag in a future milestone.
- **`go mod graph` alone**: Rejected — `go mod graph` fetches missing modules as a side effect, but only fetches what's needed to construct the graph, not the full closure. Reader-side transitive walks may still hit misses. Explicit `download` is more thorough.

---

## R2 — What existing subprocess-with-timeout pattern should the warmer mirror?

**Decision**: Mirror `mikebom-cli/src/scan_fs/package_db/golang/go_mod_graph.rs:81-158`'s `run_go_mod_graph` pattern verbatim.

**Rationale**:
- Same subprocess model: `std::process::Command` spawned in a worker `std::thread` with `mpsc::channel()` for the result, `rx.recv_timeout(duration)` to enforce the timeout, `StepResult<T>` enum returned to the caller.
- Reuses the same `ErrorClass` enum (already includes `Timeout`, `Other`, etc.); m173 extends with `SubcommandFailed`, `GoBinaryAbsent`, `SpawnFailed`, `ParseError`, `BudgetExhausted` variants via a new sibling enum `WarmingFailureReason` (per FR-007's closed-enum requirement).
- Explicit `go version` presence probe first, per `go_mod_graph.rs:90-101` — returns `WarmingFailureReason::GoBinaryAbsent` if `go` isn't on PATH, avoiding a confusing "spawn failed" from `Command::new`.
- Timeout behavior is well-tested in m055/m091 and observed by the operator community — no need to invent something new.

**Alternatives considered**:
- **Async via `tokio::process::Command`**: Rejected. The Go reader's call chain is sync (`golang::read()`), matching the go_mod_graph.rs deviation note at line 75-80. Wrapping the warmer in async would require a `Runtime::block_on` somewhere or refactoring the reader — out of scope. m055's HTTP fetcher uses `std::thread::spawn`, so sync is precedent.
- **Wait-then-kill via `Child::kill()` at timeout**: Rejected. m055/m091's approach (worker thread + `recv_timeout`) leaves the subprocess to be reaped by the OS. This is simpler and matches existing behavior. Downside: on very short timeouts, the subprocess may briefly keep running after the caller has moved on, but the OS handles cleanup.

---

## R3 — What concurrency pattern should the warmer use?

**Decision**: Mirror `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs:1001-1050`'s `parallel_fetch` verbatim — `std::thread::spawn` worker pool + `mpsc::sync_channel(workers)` job queue + `mpsc::channel()` result collector.

**Rationale**:
- Same pattern is already load-bearing in the m055/m091 HTTP proxy fetcher — has been in production since milestone 055 with no observed issues.
- No new dependencies (avoids pulling tokio into the warmer just for concurrency).
- Scales the worker count as `concurrency.max(1).min(n_workspaces)` — matches the m055 line 1011 formula, ensures no oversized worker pool when workspaces < concurrency.
- Simple mental model: one worker per running `go mod download` invocation; workers pull from a shared bounded channel; results ship back via a separate collector channel.
- The `mpsc::sync_channel(workers)` bound is the memory ceiling — a monorepo with 1000 workspaces still uses only O(workers) queued jobs at any moment.

**Alternatives considered**:
- **`tokio::sync::Semaphore` + `tokio::process::Command`**: Rejected per R2 — no async requirement, and precedent is sync.
- **`rayon` for the worker pool**: Rejected. `rayon` is a fine choice but not in the current dependency closure at the mikebom-cli level. Adding it just for m173 would be a new Cargo dep, which the plan's Constitution Check row I explicitly says we don't take.

---

## R4 — What default per-workspace timeout and overall wall-clock budget should we ship?

**Decision**:
- **Per-workspace timeout**: 60 seconds. Rationale: a typical `go mod download` on a warm-ish cache takes < 5s; on a cold cache with a full closure, 30-45s is plausible for a medium-sized module (100+ modules). 60s gives headroom for slow proxies without waiting forever.
- **Overall budget**: 5 minutes (300 seconds). Rationale: at concurrency=4, 5 minutes covers ~20 workspaces at the 60s per-workspace cap. A monorepo with more workspaces than that will hit `BudgetExhausted` on the tail, which is the correct signal ("this is too big for the default; either raise `--warm-go-cache-concurrency` or set an explicit timeout override in a future milestone").

**Rationale**:
- The overall budget is a wall-clock, not a per-workspace ceiling — so `concurrency=4` with 20 workspaces at 60s each finishes in ~5 minutes even in the worst case.
- Defer configurability of the two timeouts to a future milestone. m173 ships fixed defaults matching typical enterprise-monorepo behavior; operators who need different values can layer their own timeout logic via `timeout(1)` wrapping.
- Precedent: m055 default `go mod graph` timeout is 45s (per `graph_resolver.rs:527` era value). We're higher because `download` fetches more artifacts than `graph`.

**Alternatives considered**:
- **User-configurable timeouts (`--warm-go-cache-per-workspace-timeout`, `--warm-go-cache-overall-budget`)**: Deferred. Adds two more flags. Wait for real feedback from monorepo operators before committing to a public flag surface. Internal constants are sufficient for v1.
- **No overall budget (only per-workspace)**: Rejected. A 5000-workspace monorepo at 60s each × concurrency 4 would be > 20 hours — mikebom would appear hung. Overall budget provides safety.
- **Aggressive defaults (10s per / 60s overall)**: Rejected — too tight for cold-cache scenarios on modestly slow proxies.

---

## R5 — How should the warmer integrate with the existing Go reader?

**Decision**: Insert warming as a pre-resolver step in `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs` immediately before line ~1670 (the existing `graph_map.coverage()` aggregation loop). Warming operates on the same workspace paths the existing resolver already discovered.

**Rationale**:
- The existing legacy.rs `read_workspaces_and_resolve` (or equivalent) already walks the tree, finds every `go.mod`, and constructs a workspace list. Warming reuses this discovered list; no new tree-walk.
- Warming MUST complete before step 1 (`go mod graph`) runs for a given workspace. Sequencing: for each workspace, `warm_workspace(path)` → then the existing resolver ladder → the resolver observes the warmed cache.
- Since warming is per-workspace, we can interleave: run the warmer's worker pool over ALL workspaces, then run the resolver's ladder over ALL workspaces sequentially. Simpler than interleaving warming + resolution per workspace.
- The warming result feeds `GoScanSignals` (new field `cache_warming_result: Option<CacheWarmingResult>`) — this is the sink for the FR-005 warn logs, FR-007 doc-scope annotation, and FR-011 mode annotation.

**Alternatives considered**:
- **Warm-inside-resolver**: Rejected. Coupling warming to the resolver ladder would spread the "did we warm?" state across two modules. Cleaner boundary: warmer runs and terminates BEFORE resolver, produces a result struct, resolver is unchanged.
- **Warm-at-CLI-entry-point**: Rejected. The CLI doesn't know the workspace list until the reader has walked the tree. Placing the warmer inside the Go reader means it reuses the walker's output.
- **Skip warming for `go.work` workspaces (warm at go.work root once)**: Considered. `go mod download` inside a `use`d workspace under a `go.work` repo does resolve deps via the workspace-wide unification. Running it at the `go.work` root once would be more efficient but adds a special case. **Decision**: warm per-`go.mod` uniformly; `go mod download` at a `use`d workspace is not incorrect, just redundant. Future milestone can optimize.

---

## R6 — How should the FR-011 mode annotation and FR-007 failed annotation compose with m172's C117?

**Decision**: Both new annotations (C118 mode, C119 failed) are document-scope, share the m172-established envelope shape (`MikebomAnnotationCommentV1` for SPDX 2.3; property for CDX; typed `Annotation` graph element for SPDX 3), and are alphabetically-adjacent to C117 (`mikebom:go-transitive-fallback-count`) in the emission sequence.

**Ordering (post-173)**:
```
mikebom:go-cache-warming-failed          # C119, conditional (only if any failure)
mikebom:go-cache-warming-mode            # C118, unconditional when Go present
mikebom:go-transitive-coverage           # C110 (m160)
mikebom:go-transitive-coverage-reason    # C111 (m160)
mikebom:go-transitive-fallback-count     # C117 (m172)
```

Alphabetic sort places `cache-warming-*` before `transitive-*`, which is consistent with mikebom's alphabetized property/annotation emission convention (verified via existing golden inspection — see `golang.cdx.json` metadata.properties order).

**Wire shapes**:

**C118 — mode** (unconditional per Go-present scan):
```json
{ "name": "mikebom:go-cache-warming-mode", "value": "off" }
```
Value is one of `"off"` / `"per-workspace"` / `"offline-inhibited"`.

**C119 — failed** (conditional):
```json
{ "name": "mikebom:go-cache-warming-failed",
  "value": "[{\"workspace\":\"cmd/foo\",\"reason\":\"timeout\"},{\"workspace\":\"cmd/bar\",\"reason\":\"parse-error\"}]"
}
```
Value is a JSON-encoded string (matching m134 `mikebom:purl-collisions-detected` convention for arrays-in-property-strings on CDX). Records are sorted alphabetically by `workspace` for byte-identity across regenerations. Reason class is one of the six FR-007 closed-enum values.

**Rationale for the JSON-encoded-string value on CDX**:
- CDX `metadata.properties[]` entries have string values (per CDX spec).
- SPDX 2.3 envelope wraps the same string.
- SPDX 3 typed `Annotation.statement` carries the same string.
- Wire-shape parity across all three formats is byte-identical on the payload — matches parity-catalog `SymmetricEqual` directionality.

**Alternatives considered**:
- **Value as native JSON array**: Rejected for CDX — CDX property values are string-typed. Would require a per-format shape divergence.
- **Two separate annotations (one per failing workspace)**: Rejected. Would inflate the property list; the aggregated form is easier to consume.
- **Never emit C118 when mode is "off"**: Rejected. The mode annotation's whole purpose (FR-011) is to be machine-readable evidence of the operator's chosen mode. Emitting `"off"` explicitly matches the m172 emit-`"0"`-explicit precedent.

---

## Summary table

| ID | Question | Decision |
|---|---|---|
| R1 | Which Go subcommand? | `go mod download` (FR-012 forbids `tidy`/`build`/`test`/`generate`) |
| R2 | Subprocess pattern? | Mirror `run_go_mod_graph` (`std::thread` + `mpsc::channel` + `recv_timeout`) |
| R3 | Concurrency pattern? | Mirror `parallel_fetch` (`std::thread` worker pool + `sync_channel`) |
| R4 | Timeout defaults? | 60s per-workspace, 300s overall wall-clock |
| R5 | Integration point? | Pre-resolver step in `legacy.rs` using the reader's workspace list |
| R6 | Annotation shapes? | Doc-scope, envelope-shared with m172; C118 unconditional / C119 conditional; JSON-encoded string values for cross-format parity |
