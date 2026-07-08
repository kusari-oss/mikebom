# Feature Specification: Opt-in Go cache warming for accurate transitive graphs

**Feature Branch**: `173-warm-go-cache`
**Created**: 2026-07-08
**Status**: Draft
**Input**: User description: "m173 — add opt-in Go cache warming so mikebom can produce accurate transitive graphs on monorepos without operators writing per-workspace shell loops; preserve the m172 fallback-count signal by default (no silent warming); advise on how to fix a degraded env"

## Clarifications

### Session 2026-07-08

- Q: Concurrency for cache warming — sequential or bounded-parallel? → A: Default concurrency = 4, with `--warm-go-cache-concurrency <N>` knob for operator tuning (1 = sequential; higher = faster; 0 = auto = min(cpus, 8)). Rationale: matches m055/m091 fetch-concurrency precedent; monorepos are the motivating use case and sequential defeats the purpose; `go mod download` respects `GOPROXY` rate-limiting on its own.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Monorepo operator warms every Go workspace in one flag (Priority: P1)

Operator has a repository with N Go workspaces (each with its own `go.mod`). The CI runner has network access but a cold module cache. Without cache-warming, mikebom's Go transitive-resolution ladder falls through to step 5 (go.sum flat fallback) for a large fraction of modules, producing an SBOM with flat root→transitive edges instead of the true parent-child topology. The operator wants ONE flag on the mikebom scan invocation that primes every workspace's cache before scanning, without hand-writing a `find | xargs go mod download` loop.

**Why this priority**: this is the direct motivation for the milestone. Monorepos are the class of scan where operator-side cache warming is most tedious (multiple workspaces to warm, each with its own directory) and where the failure mode (widespread step-5 fallback) is most impactful (many components with wrong topology). Solving it with one flag delivers the entire ergonomics win.

**Independent Test**: run mikebom against a synthesized 2-workspace Go monorepo fixture on a machine where `$GOMODCACHE` starts empty. First without the flag → confirm the emitted SBOM's `mikebom:go-transitive-fallback-count` doc-scope annotation reports a positive count reflecting the step-5 fallbacks. Then with `--warm-go-cache=per-workspace` → confirm the count drops to `"0"` and the transitive edges show real parent-child topology (verified by presence of `mikebom:go-transitive-source == "go-mod-graph"` on transitive components instead of `"go-sum-fallback"`).

**Acceptance Scenarios**:

1. **Given** a Go monorepo with 2+ workspaces AND a cold module cache AND network access, **When** the operator runs `mikebom sbom scan --path <root> --warm-go-cache=per-workspace`, **Then** the emitted SBOM's `mikebom:go-transitive-fallback-count` doc-scope annotation reports `"0"` (assuming all modules are reachable on the proxy) AND every transitive Go component's `mikebom:go-transitive-source` per-component annotation is `"go-mod-graph"` or `"gomodcache"` (not `"go-sum-fallback"`).
2. **Given** the same monorepo AND cold cache AND network, **When** the operator runs mikebom WITHOUT the flag, **Then** the SBOM's fallback-count annotation reports a positive integer AND the tool emits an advisory log line naming the flag as remediation.
3. **Given** the same monorepo AND cold cache AND network, **When** the operator runs `mikebom sbom scan --path <root> --warm-go-cache=off` (explicit off), **Then** behavior matches the "no flag" case AND no advisory log line is emitted (operator has explicitly acknowledged the trade-off).

---

### User Story 2 — Operator sees a helpful hint when the env is degraded (Priority: P2)

Operator runs mikebom against a Go project in non-offline mode without the warm-cache flag, and the scan lands with a positive fallback count. Rather than reading docs to figure out why, they want the tool to point them at the fix inline — a single INFO-level log line naming the flag OR the manual `go mod download` recipe.

**Why this priority**: reduces the "why does my SBOM show wrong topology?" support-load class of issue. Complements the m172 `mikebom:go-transitive-fallback-count` annotation — m172 makes the problem visible in the SBOM; m173's advisory log makes the fix discoverable at scan time. Ranked P2 because US1 solves the monorepo case in one flag; the advisory is a nice-to-have for operators who don't yet know the flag exists.

**Independent Test**: run mikebom against any Go fixture with a cold cache and no warm-cache flag AND non-offline mode. Assert the stderr / structured log stream contains a single line matching a stable pattern like `"mikebom:go-transitive-fallback-count > 0 detected. Prime the cache with --warm-go-cache=per-workspace or 'go mod download' per workspace before scanning."` — the exact wording (with the `mikebom:` annotation-name prefix) is stable enough for `grep -F` matching in operator dashboards without being over-precise on formatting. Consumer-side automation should key on the `--warm-go-cache=per-workspace` substring specifically, since it names the actionable remediation.

**Acceptance Scenarios**:

1. **Given** a Go fixture with a cold cache AND non-offline mode AND no explicit `--warm-go-cache` flag, **When** the scan finishes with fallback-count > 0, **Then** exactly one advisory log line at INFO level is emitted naming both the flag AND the manual `go mod download` remediation.
2. **Given** the same Go fixture AND non-offline mode AND `--warm-go-cache=off` (explicit), **When** the scan finishes with fallback-count > 0, **Then** NO advisory log line is emitted (operator explicitly opted out).
3. **Given** any scan in `--offline` mode, **When** the scan finishes with any fallback-count value, **Then** NO advisory log line is emitted (the flag would be a no-op in offline mode; suggesting it would be misleading).

---

### User Story 3 — Cache-warming failure degrades gracefully instead of aborting the scan (Priority: P2)

Operator uses `--warm-go-cache=per-workspace` on a monorepo where one workspace's cache-warming step fails (e.g., a module in that workspace is unreachable, a proxy timeout fires on one workspace but not others, the host lacks the `go` binary, or one workspace has a malformed `go.mod`). The operator expects the scan to complete and produce an SBOM. Failures during warming should be surfaced as diagnostic annotations, not as a hard scan abort.

**Why this priority**: preserves mikebom's "fail loud but keep producing an SBOM" posture. A monorepo operator running warm-cache under CI cannot tolerate a single-workspace failure taking down the whole SBOM job; they'd revert to no-warming, which defeats the purpose. Ranked P2 because it's a robustness property that US1 depends on for real-world monorepo adoption.

**Independent Test**: synthesize a 3-workspace fixture where one `go.mod` is malformed (or the workspace's modules are unreachable via a mock GOPROXY). Run with the flag. Confirm the scan exits with success status AND the emitted SBOM includes a document-scope annotation naming the failing workspace(s) AND the successful workspaces still emit `"go-transitive-fallback-count": "0"` behavior in their scoped subtree.

**Acceptance Scenarios**:

1. **Given** a monorepo with 3 Go workspaces where workspace 2 has a malformed `go.mod`, **When** the operator runs mikebom with `--warm-go-cache=per-workspace`, **Then** the scan exits successfully AND the SBOM includes a `mikebom:go-cache-warming-failed` document-scope annotation naming workspace 2 (with a reason class like `parse-error`) AND workspaces 1 and 3 warm successfully.
2. **Given** a scan environment WITHOUT the `go` binary on PATH AND the flag set to `per-workspace`, **When** the operator runs mikebom, **Then** the scan exits successfully AND emits a doc-scope annotation reason class of `go-binary-absent` AND the ladder proceeds directly to step 5 for all workspaces (i.e., behavior converges with the no-flag case — no worse than baseline).
3. **Given** a workspace whose modules include one unreachable via GOPROXY (a 404 or timeout on a specific module) AND the flag set to `per-workspace`, **When** the operator runs mikebom, **Then** the scan exits successfully AND the emitted SBOM records the failure reason class AND resolution proceeds for reachable modules.

---

### Edge Cases

- **Very large monorepo (100+ workspaces)**: cache-warming at the default concurrency of 4 processes 4 workspaces at a time. The behavior on scale is bounded by the overall wall-clock budget (FR-006); per-workspace timeouts may fire but individual failures degrade gracefully per US3. Operators can raise `--warm-go-cache-concurrency` for faster completion on wide monorepos or drop it to `1` when running under CI shared-runner constraints.
- **Workspace with `go.work` present**: a Go workspace-mode repo (m161) has its own graph resolution semantics. Cache warming operates at the same granularity mikebom's existing Go reader uses (per workspace, respecting `go.work` if present) so results are consistent with the transitive resolver.
- **Operator sets both `--offline` and `--warm-go-cache=per-workspace`**: the warm-cache flag is a no-op in offline mode. The tool emits a warn-level log naming the conflict and continues in offline mode (no cache-warming attempted). No advisory log for US2 in this case either.
- **Cache-warming succeeds but transitive resolution still hits step 5**: possible when the operator's `$GOPROXY` returns a module list that fails a downstream integrity check. The C117 count still reflects reality; the advisory log (US2) is suppressed because the operator DID pass the warm-cache flag (they've already taken the recommended action).
- **`GOFLAGS=-mod=vendor` in the operator's env**: vendor-mode changes what `go mod download` does. Cache warming SHOULD respect the operator's env variables (don't override `GOFLAGS`, `GOPROXY`, `GOMODCACHE`, etc.). If the vendor mode makes cache warming a no-op, no failure is emitted.
- **Concurrent `mikebom` scans of the same repo**: two scans in parallel both trying to warm `$GOMODCACHE` should not corrupt shared state. Standard Go toolchain cache-locking behavior applies; mikebom does not add its own locking.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The `mikebom sbom scan` command MUST accept a new `--warm-go-cache` flag with the values `off` and `per-workspace`. The default value MUST be `off`.
- **FR-002**: When the flag is set to `per-workspace` AND the tool is in non-offline mode AND the scan target contains at least one Go workspace, the tool MUST invoke `go mod download` in each detected workspace before the transitive-resolution ladder runs against that workspace. Enumeration of workspaces MUST reuse the existing Go reader's workspace-discovery output; no new tree-walk is introduced.
- **FR-003**: The tool MUST NOT invoke any cache-warming subcommand when `--offline` is set, regardless of the `--warm-go-cache` value. When both flags are set with warming requested, the tool MUST emit exactly one warn-level log line naming the conflict and continue in offline mode.
- **FR-004**: In non-offline mode, when the emitted SBOM contains a `mikebom:go-transitive-fallback-count` annotation with a value greater than `"0"` AND the `--warm-go-cache` flag was NOT explicitly set (i.e., the value took its default), the tool MUST emit exactly one INFO-level advisory log line naming BOTH the `--warm-go-cache=per-workspace` flag AND the manual `go mod download` remediation. When the flag was explicitly set (to any value including `off`), the tool MUST NOT emit the advisory line (the operator has taken a stance).
- **FR-005**: When cache-warming fails for a workspace (subcommand exits non-zero, times out, or the `go` binary is absent), the tool MUST log the failure at warn level naming the workspace path and a stable reason class, MUST continue warming remaining workspaces, MUST NOT abort the scan, AND MUST emit a document-scope `mikebom:go-cache-warming-failed` annotation aggregating the failing workspaces + reason classes.
- **FR-006**: The tool MUST enforce a per-workspace timeout AND an overall wall-clock budget for the cache-warming phase. Individual per-workspace timeouts emit a `timeout` reason class per FR-005. When the overall budget is exhausted, remaining workspaces skip warming with a `budget-exhausted` reason class emitted for each. The overall budget is a wall-clock ceiling regardless of concurrency setting.
- **FR-007**: The `mikebom:go-cache-warming-failed` annotation MUST be document-scope with a value shape that carries per-workspace records `{workspace_path, reason_class}`. Reason classes MUST be a closed enum including at least: `go-binary-absent`, `spawn-failed`, `timeout`, `subcommand-failed`, `parse-error`, `budget-exhausted`. The annotation MUST be emitted across CDX 1.6, SPDX 2.3, and SPDX 3.0.1 with the same envelope shape milestone 172 established.
- **FR-008**: The tool MUST invoke `go mod download` with the operator's env variables preserved (`GOPROXY`, `GOFLAGS`, `GOMODCACHE`, `GOPRIVATE`, `GONOSUMCHECK`, `GOSUMDB`, etc.). The tool MUST NOT override these variables and MUST NOT inject additional command-line flags beyond what's necessary to scope invocation to the workspace directory.
- **FR-009**: The advisory log line from FR-004 MUST be suppressed when the scan target contains no Go components (nothing to warm; the message would be irrelevant).
- **FR-010**: The `--warm-go-cache` flag's value MUST be parseable in the equals-form (`--warm-go-cache=per-workspace`) as the primary interface, matching the existing style used by `--offline` per prior milestones. The bare form (`--warm-go-cache per-workspace`) MAY also be accepted; the boolean-shorthand form (`--warm-go-cache` alone) MUST NOT be accepted because there is no natural default.
- **FR-011**: The tool MUST emit a document-scope `mikebom:go-cache-warming-mode` annotation carrying the effective mode value (`"off"`, `"per-workspace"`, or `"offline-inhibited"` when the operator requested warming but offline mode overrode it). The annotation is emitted whenever the scan target contains at least one Go component. This annotation is companion to FR-004's advisory log — the annotation is machine-readable for CI-side auditing; the log line is a one-time hint.
- **FR-012**: The tool MUST NOT invoke `go mod tidy`, `go build`, `go test`, `go generate`, or any other Go subcommand that mutates the operator's `go.mod`, `go.sum`, or runs Go code from the target. Only `go mod download` (and existing `go mod graph` / `go mod why -m -vendor` from the transitive resolver) is permitted.
- **FR-013**: The tool MUST NOT run cache-warming as a side effect of the presence-check pattern (i.e., the `go version` invocation that already gates the resolver). Warming MUST be tied to the explicit flag value and MUST NOT be inferred from any other signal.
- **FR-014**: The `mikebom sbom scan` command MUST accept a new `--warm-go-cache-concurrency <N>` flag where `N` is a non-negative integer. The default value MUST be `4`. `N=1` runs warming sequentially (one workspace at a time). `N=0` MUST resolve to `min(logical_cpus, 8)` at runtime (auto-detect). `N>=2` runs at most `N` per-workspace `go mod download` invocations in flight simultaneously. The flag MUST be a no-op when `--warm-go-cache=off` (the default) or when the offline-inhibited condition of FR-003 applies (no warming happens, so concurrency is moot). Values above 32 MUST be clamped to 32 with a warn-level log line (defense against operator typos / config-file mistakes flooding GOPROXY).

### Key Entities

- **CacheWarmingMode**: enum with variants `off`, `per-workspace`, and (internal only) `offline-inhibited`. Sourced from the `--warm-go-cache` flag with `off` as the default. Consumed by the Go reader before it invokes the transitive resolver AND by the SBOM emitter for the FR-011 annotation.
- **CacheWarmingResult**: aggregates per-workspace outcomes. For each detected Go workspace, records either `success` or `failure(reason_class)`. Feeds the FR-005 warn logs, the FR-007 doc-scope annotation, and any operator-facing summary lines.
- **AdvisoryContext**: three-input predicate driving the FR-004 advisory log: `fallback_count > 0`, `warm-flag was NOT explicitly set`, `offline mode is NOT active`. When all three are true, emit one advisory line. When any is false, suppress. Kept as an explicit entity because the suppression rules are the load-bearing UX contract of US2.
- **CacheWarmingConcurrency**: integer in `{0, 1, 2, ..., 32}` where `0` resolves to `min(logical_cpus, 8)` at runtime. Sourced from the `--warm-go-cache-concurrency` flag with `4` as the default per the Q1 clarification. Consumed by the warmer to bound the number of concurrent `go mod download` invocations. Values above `32` are clamped with a warn-level log per FR-014.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An operator scanning a 3-workspace Go monorepo with a cold cache goes from a positive `mikebom:go-transitive-fallback-count` value (baseline: n modules degraded, per-workspace flat-fallback shape) to `"0"` (all modules resolved via the toolchain-authoritative step-1 `go mod graph` path) by adding a single flag (`--warm-go-cache=per-workspace`) to their scan invocation. Verified end-to-end via an integration test asserting the before/after C117 value delta.
- **SC-002**: In non-offline mode with default flag settings and fallback-count > 0, exactly ONE advisory log line matching a stable substring is emitted per scan (not zero, not many; verified by `grep -c` on captured stderr equaling `1`).
- **SC-003**: A monorepo scan where one workspace's cache warming fails (malformed `go.mod`) completes with exit status 0 AND emits a document-scope `mikebom:go-cache-warming-failed` annotation naming exactly the failing workspace with the correct reason class, verified across all three format outputs (CDX 1.6, SPDX 2.3, SPDX 3.0.1).
- **SC-004**: Non-Go scans (Rust, npm, Yocto, etc.) emit ZERO new annotations, ZERO new log lines, and produce byte-identical SBOMs to the pre-173 baseline. Verified by the existing byte-identity golden regression suite showing zero delta on any non-Go golden.
- **SC-005**: The end-to-end scan wall-clock time for the milestone's own test fixture (a synthesized 2-workspace repo pointed at a hermetic mock proxy) with cache warming enabled completes within 60 seconds. This is a signal-only measure; production scan times depend on operator network + module closure size and are not bounded here.
- **SC-006**: An operator can determine the effective cache-warming mode of a completed scan by reading the `mikebom:go-cache-warming-mode` document-scope annotation from the emitted SBOM (no need to consult the tool's stderr / log stream). Verified via jq query returning one of `"off"` / `"per-workspace"` / `"offline-inhibited"` for every Go-containing SBOM produced post-173.

## Assumptions

- Cache warming is invoked via the operator's host `go` binary discovered on `PATH`. If the operator's env sets `GOROOT` or `GOTOOLCHAIN` to select a specific toolchain, the warming inherits that same selection.
- Warming is opt-in by design. This preserves milestone 172's `mikebom:go-transitive-fallback-count` as an actionable diagnostic — a silent-by-default warmer would mask the very signal m172 shipped to surface. The advisory log (FR-004) is the discovery mechanism; the flag is the fix.
- Cache warming is a per-workspace operation, not a repo-global one. Monorepos with heterogeneous Go modules (different `go.mod` per workspace with disjoint transitive closures) benefit from per-workspace scoping; a repo-global `go mod download` from the top-level directory would need a `go.work` file that many monorepos don't have.
- Concurrency for cache warming defaults to 4 concurrent per-workspace `go mod download` invocations, tunable via `--warm-go-cache-concurrency <N>` per FR-014. The default matches m055/m091's existing fetch-concurrency posture for HTTP proxy fetches (`GraphResolverConfig.fetch_concurrency = 16`). Concurrency-1 preserves the "sequential" behavior for operators who prefer it. The overall wall-clock budget (FR-006) still applies regardless of concurrency setting.
- The `mikebom:go-cache-warming-failed` and `mikebom:go-cache-warming-mode` annotations occupy the same envelope shape as existing document-scope Go annotations (`mikebom:go-transitive-coverage`, `mikebom:go-transitive-fallback-count` from m172). No new envelope format is introduced.
- Cross-platform behavior: warming works on Linux, macOS, and Windows hosts wherever `go mod download` itself works. mikebom does not attempt cache warming on hosts where the `go` binary is missing; FR-005's `go-binary-absent` reason class is emitted instead.
