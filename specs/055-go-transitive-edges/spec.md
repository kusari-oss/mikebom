# Feature Specification: Go transitive dependency edges, anchored on `go.sum`

**Feature Branch**: `055-go-transitive-edges`
**Created**: 2026-05-02
**Status**: Draft
**Input**: User description: "look at how to do go transitive edges. while investigating this yes look at `go mod graph`, but in some cases we might not have go installed so we should look at how to also do go dependency resolution. Also deps.dev is useful BUT is not the canonical source of truth for dependency resolution BECAUSE in some cases projects will have optional dependencies or just generally deps.dev will be incorrect. So again, you should always look at real go dependency resolution here. Yes, prefer the go.sum file since it's canonically what gets installed and figure out the graph based on that."

## Clarifications

### Session 2026-05-02

- Q: What is the canonical source of truth for the **module set** (which components appear in the SBOM)? → A: **`go.sum`**. It is the lockfile — the exact set of `(module, version)` pairs that get installed by `go build` after MVS resolution. Mikebom already enumerates components from `go.sum` per milestone 049. 055 does NOT change the module set; it only adds edges between modules that are already there.
- Q: What is the canonical source of truth for **edges** (which module requires which)? → A: **the per-module `go.mod` file**. Each `(module, version)` pair in `go.sum` has its own `go.mod`, which lists that module's `require` entries. The graph is the union of those requires, intersected with `go.sum` (so we never emit an edge to a module that isn't actually installed).
- Q: Where do we obtain each per-module `go.mod`? → A: **A 4-step resolution ladder**, in priority order:
  1. `go mod graph` when `go` is on PATH AND `--offline` is not set. One subprocess gives the entire resolved DAG with replace/exclude already applied.
  2. `$GOMODCACHE` walk (current 053 behavior in `cache_lookup_depends`). Works without `go` if the cache is populated.
  3. Direct fetch from the Go module proxy (`$GOPROXY`, default `https://proxy.golang.org`) — `<proxy>/<escaped-module>/@v/<version>.mod` returns the bare `go.mod` file (typically a few KB). This is "real Go dependency resolution without `go` installed" — the same protocol Go itself uses, so we honor `GOPROXY`, `GOPRIVATE`, and `GONOSUMCHECK`.
  4. Graceful no-edges fallback. The component is still emitted; its outgoing edges are empty. Logged at `tracing::warn`.
- Q: Is `deps.dev` part of the ladder? → A: **No.** Per the user's directive, deps.dev is NOT canonical for Go because (a) its index can lag, (b) it doesn't accurately reflect `replace`/`exclude` directives in the user's local `go.mod`, and (c) where the user has private or vendored modules deps.dev simply doesn't have the data. We only want results that match what `go build` would actually install. Steps 1–3 of the ladder are all "real Go dependency resolution"; deps.dev is not.
- Q: Network access default for step 3 (proxy fetch)? → A: **Opt-in via the existing `--offline` semantic, inverted.** If `--offline` is set, step 3 is disabled. If `--offline` is NOT set (the default for `mikebom sbom scan`), step 3 IS allowed when steps 1–2 have failed. Rationale: the default user expectation is "produce a correct SBOM"; silently leaving 90% of edges off because we didn't try the network is worse than a few KB per module of network traffic. CI fixtures and reproducible-build pipelines that need hermetic behavior already pass `--offline`.
- Q: How do we handle drift between `go.mod` (declared) and `go.sum` (installed)? → A: **`go.sum` wins.** Edge targets MUST appear in `go.sum`; targets that don't are dropped silently (existing 053 dangling-target dedup, restated for transitive case). This guarantees we never emit an edge to a module that isn't actually installed, even if `go mod graph` (which resolves from `go.mod`) would name one. Concretely: a stale `go.sum` (user edited `go.mod` but didn't run `go mod tidy`) results in an edge set that reflects the install state, not the declaration state — which is correct per the user's "go.sum is canonically what gets installed" directive.
- Q: Is proxy-fetch (step 3 of the resolution ladder) on-by-default or opt-in? → A: **On-by-default; `--offline` disables.** Matches `go`'s own behavior: `proxy.golang.org` is the standard module proxy, and a default scan that "just works" is more valuable than one that silently emits edge-poor SBOMs until the user discovers a flag. Privacy footprint is identical to running `go` itself — `GOPRIVATE` matches keep private module names off the public proxy regardless. Users with stricter postures already pass `--offline` or set `GOPROXY=off` (both are honored).
- Q: How many concurrent proxy fetches? → A: **Fixed cap at 16, no CLI flag.** Single global semaphore in the resolver. Saturates a single proxy comfortably, polite to `proxy.golang.org`, and on a 500-module workspace caps total fetch time near `(500 / 16) × p50_latency` ≈ a few seconds. No surface area to maintain. If real users hit a wall, a flag is cheap to add later; cheaper than removing one we shipped speculatively.
- Q: Should proxy-fetched `.mod` files be cached on disk between scans? → A: **No — in-memory only, per-scan.** Mikebom does not write a Go-mod cache to `$XDG_CACHE_HOME` or anywhere else. Rationale: (a) fetches are cheap (~5 KB × 500 modules over 16-way concurrency completes in seconds), (b) no cache-invalidation logic to write/test/get-wrong, (c) repeated scans of the same project still benefit from `$GOMODCACHE` (step 2) once the user has run `go build` or `go mod download` themselves. If repeat-scan latency becomes a real complaint, on-disk caching is a separate milestone.

## Investigation findings (recorded here so the spec is grounded, not aspirational)

The current state of Go edge emission in mikebom (post-053):

- **Main-module direct edges**: ✅ emitted unconditionally from the workspace `go.mod` via `build_main_module_entry()` in `mikebom-cli/src/scan_fs/package_db/golang.rs:610`. Works offline.
- **Transitive edges (per-module `depends` field)**: ⚠️ emitted ONLY when `$GOMODCACHE` is populated. `cache_lookup_depends()` in `golang.rs:570` reads the cached `go.mod` for each `go.sum` entry; if the cache is empty (CI runners without `go mod download`, fresh checkouts, containers, the `tests/fixtures/go/argo-style-no-cache/` reproduction), the function returns an empty vec and the component emits with **zero outgoing edges**. This is issue #102's residual gap.
- **Components from `go.sum`**: ✅ already at full transitive closure per milestone 049 (`go.sum` enumerated end-to-end, scope-classified for prod/test).

Comparative analysis vs. peer SBOM tools (verified against current source — Trivy main @ 99eabdf 2026-04-24, Syft current main, cdxgen current):

| Tool | Edge depth | Without `go` toolchain | Without populated cache | Notes |
|------|-----------|------------------------|-------------------------|-------|
| **Trivy** | 1 hop (leaf `go.mod` read) | ✅ works | ❌ indirects orphan-attach under root | `pkg/fanal/analyzer/language/golang/mod/mod.go:147-258`. Encoder names this fall-through explicitly: `pkg/sbom/io/encode.go:515-516` *"Case 3: Relationship: known, DependsOn: unknown (e.g., go.mod without $GOPATH) — All packages are included in the parent."* No MVS, no `go mod graph`, no proxy fetch. |
| **Syft** (source cataloger) | Full MVS via `packages.Load` with `NeedDeps`/`NeedImports` | ❌ requires `go` | ❌ requires `go` | `syft/pkg/cataloger/golang/parse_go_mod.go:85-324`. Defers entirely to the Go toolchain's package loader; emits true module→module edges when toolchain is present, falls back to flat list otherwise. |
| **cdxgen** | Full via `go list -m all` | ❌ requires `go` | ❌ requires `go` | Shells out to the toolchain. |
| **cyclonedx-gomod** | Full via `go list -m all` | ❌ requires `go` | ❌ requires `go` | Same shape as cdxgen. |
| **mikebom 053 (today)** | Full via per-module cache walk | ✅ works | ❌ empty edges | `cache_lookup_depends` in `golang.rs:570`. Trivy-class behavior. |
| **mikebom 055 (target)** | Full via 4-step ladder | ✅ works | ✅ proxy fetch fills gap | The differentiator is specifically "no `go` AND no cache" — every other tool degrades in that cell. |

The Go module proxy protocol (`<proxy>/<module>/@v/<version>.mod`) is documented at <https://proxy.golang.org/> and is the same endpoint Go itself uses to resolve modules. Fetching just the `.mod` file (not the source `.zip`) is cheap (typically 1–10 KB per module) and does not require any Go toolchain. This is what makes step 3 of the ladder "real Go dependency resolution," distinct from a third-party API like deps.dev.

### Capability matrix — what mikebom 055 can and can't produce

Edge fidelity is a function of which inputs are available at scan time. The 4-step ladder degrades gracefully; this table makes the degradation explicit so SBOM consumers (and future maintainers) know what an empty edge means in any given scan.

| Environment | Step 1: `go mod graph` | Step 2: `$GOMODCACHE` walk | Step 3: proxy fetch | Edge result |
|-------------|------------------------|----------------------------|---------------------|-------------|
| `go` on PATH, cache populated, `--offline=false` | ✅ supplies full DAG | (skipped — step 1 sufficient) | (skipped) | **Full MVS-resolved edges**, matches `go mod graph` |
| `go` on PATH, empty cache, `--offline=false` | ✅ (subprocess auto-populates the cache as a side effect) | (skipped) | (skipped) | **Full MVS-resolved edges** |
| No `go`, cache populated, `--offline=false` | ❌ | ✅ supplies per-module requires | (used only for modules missing from cache) | **Full edges via cache walk + proxy fill-in** |
| No `go`, empty cache, `--offline=false` | ❌ | ❌ | ✅ supplies per-module requires | **Full edges via proxy fetch** (the 055 differentiator vs. all peers) |
| No `go`, cache populated, `--offline=true` | ❌ (disabled by `--offline`) | ✅ | ❌ (disabled by `--offline`) | **Full edges via cache walk**, no fill-in for cache-misses |
| No `go`, empty cache, `--offline=true` | ❌ | ❌ | ❌ | **No transitive edges**; main-module direct edges from 053 still present; FR-009 summary names this case |
| `GOPROXY=off` (any other state) | ✅ if `go` available, else ❌ | ✅ if cache populated | ❌ (honored — proxy disabled by env) | Same as `--offline=true` cases plus whatever step 1 supplies |
| `GOPRIVATE` matches a module | partially — step 1 covers private modules if cache has them | ✅ for matched modules with cache hits | ❌ for matched modules (never sent to public proxy) | Edges for `GOPRIVATE`-matched modules emit only via steps 1–2; step 3 declines |

What 055 **can** produce:
- Full transitive `dependsOn` edges between every component sourced from `go.sum`, given any one of: (a) `go` on PATH, (b) populated `$GOMODCACHE`, (c) network access to `$GOPROXY`.
- Partial edges (some modules have outgoing edges, others don't) when only a subset of the ladder succeeds — e.g., proxy fetches some modules but not others due to transient errors.
- A `tracing::info` ladder summary (FR-009) showing which step contributed which edges per scan, so the gap is observable rather than invisible.

What 055 **cannot** produce:
- Edges to modules not in `go.sum` (by design, FR-003 — guarantees no edge to a module that wouldn't actually be installed).
- Edges for `go.work` workspace member modules' transitive deps (out of scope; tracked for follow-up).
- Edges for vendor-mode-resolved modules where `go.sum` doesn't reflect the install set (out of scope; modern Go projects with `go.sum` still work).
- Edges via source-VCS fallback (`GOPROXY=direct`, no cache, no proxy chain) — the resolver does not implement git/hg/svn fetch in 055; falls through to no-edges with a `tracing::warn`.
- Edges for `GOPRIVATE`-matched modules without a cache hit and without step 1 supplying them — by design, to keep private module names off the public proxy.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Transitive edges populate even when `go` is not installed and `GOMODCACHE` is empty (Priority: P1)

A developer or CI pipeline runs `mikebom sbom scan --path ./<go-project>` on a host that does NOT have `go` on PATH and has no populated `$GOMODCACHE` (a typical container build, a CI runner that hasn't run `go mod download`, a fresh checkout). The resulting SBOM contains transitive `dependsOn` edges between every component sourced from `go.sum` — not just the main-module's direct requires. Edges accurately reflect what `go build` would install in that workspace.

**Why this priority**: This is the headline gap from issue #102. Without it, Go SBOMs from mikebom are component-rich but edge-poor — consumers can see "what's in the build" but not "what depends on what," which is what dep-graph–driven tooling (vulnerability propagation, license-compatibility analysis, attack-path graphs) actually consumes. Trivy and Syft both have this same gap; closing it differentiates mikebom and matches the user's directive.

**Independent Test**: Use the existing `tests/fixtures/go/argo-style-no-cache/argo-workflows/` fixture (already committed for milestone 053 specifically because it has 14 direct requires and a non-trivial transitive closure). Run `mikebom sbom scan --path <fixture>` on a host with `PATH` scrubbed of `go` and `$GOMODCACHE` set to an empty temp dir. Assert: the SBOM contains ≥ 90% of the edges that would be present if `go mod graph` had been run. (The 90% floor accounts for proxy-fetch failures on individual modules; aiming for 100% is brittle.)

**Acceptance Scenarios**:

1. **Given** the `argo-style-no-cache/argo-workflows/` fixture, **When** `mikebom sbom scan --path <fixture>` runs with `go` removed from `PATH` and `GOMODCACHE` set to an empty temp dir, **Then** the SBOM contains transitive edges for every `go.sum` module-version whose `.mod` file the proxy returns successfully (target: ≥ 90% of `go.sum` entries have ≥ 1 outgoing edge if their declared requires include other `go.sum` entries).
2. **Given** the same fixture and conditions but with `--offline`, **When** the scan runs, **Then** components still emit but edges may be empty for modules not in cache; the scan completes successfully (no panic, no hang) and emits a `tracing::warn` line naming the resolution-ladder fallthrough.
3. **Given** any `go.sum`-bearing project, **When** the resulting SBOM is consulted, **Then** every emitted edge's target is itself a component in the SBOM (no dangling targets, per FR-006 from milestone 053 generalized to the transitive case).

---

### User Story 2 — When `go` is installed, mikebom uses `go mod graph` for canonical resolution (Priority: P2)

A developer running `mikebom sbom scan --path ./<go-project>` on a host where `go` IS on PATH gets edges that exactly match what `go mod graph` would produce (intersected with `go.sum`). One subprocess call replaces the per-module cache lookups, runs faster on large workspaces, and matches Go's own resolution semantics including replace/exclude/MVS without mikebom re-implementing them.

**Why this priority**: Most developer machines have `go` installed, so this is the common case. Using `go mod graph` here is cheaper (one process vs. N file reads) and more accurate (Go applies MVS correctly across the whole graph; per-module cache walks can drift on the rare `replace` interaction). It also gives us a free oracle for testing: in CI we can run with `go` installed and assert the proxy-fetch path produces identical edges.

**Independent Test**: On a host with `go` on PATH, run `mikebom sbom scan --path <fixture>` and capture the edge set. Independently run `go mod graph` in the same directory and parse its output. Assert the edge sets are identical after intersection with `go.sum` (i.e., dropping edges from `go mod graph` whose endpoints aren't in `go.sum` — these are typically test-scope or pruned-by-go-mod-tidy modules).

**Acceptance Scenarios**:

1. **Given** any committed Go fixture, **When** `mikebom sbom scan` runs with `go` on PATH, **Then** mikebom invokes `go mod graph` once and uses its output as the primary edge source (verifiable via `tracing::debug` breadcrumb naming `go mod graph: hit, N edges`).
2. **Given** the `simple-module/` fixture, **When** the scan runs with `go` on PATH, **Then** the emitted edges for the main-module match `go mod graph`'s top-level edges, and the emitted edges for each transitive module match the corresponding lines from `go mod graph`, after both are intersected with `go.sum`.
3. **Given** a fixture with a `replace` directive pointing at a different module-version, **When** the scan runs with `go` on PATH, **Then** edges reflect the replaced version (`go mod graph` already applies the replace), matching mikebom's existing 053 behavior for the main-module's direct requires.

---

### User Story 3 — Realistic-project regression suite gains a transitive-edge assertion (Priority: P3)

The realistic-project CI job introduced in milestone 054 (which scans `knative/func` end-to-end) asserts not just "scan completes" but also "the resulting SBOM contains a connected dependency graph." A regression that emits components without transitive edges (e.g., a future refactor that breaks the proxy-fetch path) fails CI in the same job that already exists, with a clear failure message naming the missing edge count.

**Why this priority**: Closes the same regression-prevention gap that drove the 054 realistic-project suite. Without this, a refactor could silently regress edge emission to the pre-053 state and a downstream consumer would notice before mikebom's own tests do. P3 because it's a CI hardening step, not a user-visible feature.

**Independent Test**: Add a per-fixture edge-count threshold to the existing realistic-project job's assertion set. Verify that an artificial regression (e.g., temporarily stub `cache_lookup_depends` to return empty) causes the job to fail with a clear diagnostic.

**Acceptance Scenarios**:

1. **Given** the post-054 realistic-project CI job, **When** a PR introduces a regression that drops transitive edge emission to zero, **Then** the job fails with a message naming the per-fixture expected vs. actual edge count.
2. **Given** the same CI job, **When** mikebom's edge emission works correctly, **Then** the job passes and reports the edge count for the record (so future PRs can see the trend).
3. **Given** the `knative/func @ knative-v1.22.0` fixture, **When** the post-055 scan runs in the realistic-project CI job, **Then** the resulting SBOM contains ≥ 200 transitive edges among `pkg:golang` components (extending milestone 054 SC-007's component-count assertion to edges).

---

### Edge Cases

- **Workspace with no `go.sum`** (very early-stage projects, single-file modules): no transitive edges to emit. Main-module's direct edges (053) still work. The transitive-edge ladder is skipped entirely; this is not an error.
- **`replace` directive pointing at a local relative path** (`replace example.com/foo => ../foo`): the replaced module's `go.mod` is read from the local filesystem, not the cache or proxy. This is what `go mod graph` does natively; the cache and proxy fetchers must mirror this behavior by short-circuiting to a local-path read for replaces.
- **`exclude` directive** (`exclude example.com/foo v1.2.3`): the excluded version is already absent from `go.sum` (Go won't install it), so it never appears as a component or an edge target. No special handling needed beyond what 053 already does for `replace`/`exclude` on the main-module's direct requires.
- **`// indirect` requires**: these are part of the resolved closure and ARE in `go.sum`. Treated identically to direct requires for graph purposes (no separate classification on the edge).
- **Module with retracted version** (`retract` directive): retraction is policy, not graph topology. If `go.sum` lists it, the module is in the SBOM; its edges are emitted normally. A downstream consumer can act on the retraction independently.
- **`go.work` workspaces** (multi-module): out of scope for milestone 055 — the workspace-mode resolution is meaningfully different (each member module has its own `go.mod` and they share a top-level `go.work`). Tracked in follow-up issue (TBD; will be filed at planning time if not pre-existing).
- **Vendor directory present** (`vendor/modules.txt`): out of scope for milestone 055. We use `go.sum` for the module set, not `vendor/modules.txt`. A vendored project still has `go.sum`, so the ladder still applies; however, `go mod graph` may behave differently when `-mod=vendor` is the default. If this manifests in practice we file a follow-up; the 055 design assumes the standard non-vendor case.
- **Network failure during proxy fetch** (transient DNS, proxy 5xx, timeout): the failed module's edges are dropped from the SBOM, a `tracing::warn` line names the module-version + error, and the scan continues. The SBOM remains valid (no incomplete-component entries; just fewer edges). User-visible reporting at the end of the scan summarizes "M of N modules had transitive edges resolved."
- **`GOPROXY=off`**: the proxy fetcher refuses to fetch (matching `go`'s behavior). The ladder collapses to step 1 (`go mod graph` if `go` available) or step 4 (no edges). This is honored, not overridden — users who set `GOPROXY=off` in their environment expect strictly-offline behavior.
- **`GOPRIVATE` matches a module** (e.g., a `github.com/our-org/*` glob): the proxy fetcher MUST NOT contact `proxy.golang.org` for that module. Either step 1 or step 2 must supply the data, or the edges are dropped with a `tracing::debug` breadcrumb. This prevents leaking private module names to a public proxy.
- **Cyclic dependency** (theoretically possible, vanishingly rare in real Go projects): the resolution walk uses a visited-set keyed by `(module, version)`, so a cycle terminates after both endpoints are seen. Standard graph hygiene, no special-case logic needed.
- **Workspace `go.mod` declares a require not in `go.sum`** (stale `go.sum` — user edited `go.mod` but didn't run `go mod tidy`): per the Q6 clarification, `go.sum` wins. The required module isn't a component, so it can't be an edge target; the edge is silently dropped. The main-module's direct-edges set already exhibits this behavior in 053; 055 inherits it.
- **`go mod graph` fails or hangs** (extremely large workspaces, broken Go install): the ladder falls through to step 2 (cache walk) and step 3 (proxy fetch) instead of failing the scan. A 30-second per-invocation timeout on the `go mod graph` subprocess prevents indefinite hangs (mirroring 053's `git describe` 2-second timeout discipline).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: For every `(module, version)` pair in the workspace's `go.sum`, mikebom MUST emit a transitive `dependsOn` edge to every module that pair declares as a `require` in its own `go.mod`, after applying the workspace's `replace` and `exclude` directives, AND after intersecting with the workspace's `go.sum` (no edges to modules not actually installed). This generalizes 053's main-module direct-edge behavior to all `go.sum` components.

- **FR-002**: Mikebom MUST implement a 4-step resolution ladder for obtaining each module's `go.mod`, in priority order:
  1. `go mod graph` subprocess invocation IF `go` is on PATH AND `--offline` is not set. One invocation per scan; output is parsed into the full `(parent, child)` edge set.
  2. `$GOMODCACHE` walk via the existing `cache_lookup_depends` codepath (`mikebom-cli/src/scan_fs/package_db/golang.rs:570`).
  3. Direct fetch from `$GOPROXY` (default `https://proxy.golang.org`) using the Go module proxy protocol's `<proxy>/<escaped-module>/@v/<version>.mod` endpoint. ONLY attempted when `--offline` is not set, the module is NOT matched by `$GOPRIVATE`, and steps 1–2 did not supply the data.
  4. Graceful no-edges fallback: the component is still emitted with an empty `depends` field. A `tracing::warn` line names the module-version that fell through.

- **FR-003**: `go.sum` MUST be the canonical source of truth for the module set. Any edge whose target is a `(module, version)` pair NOT present in `go.sum` MUST be dropped silently. (This is 053's dangling-target dedup, restated for the transitive case.) This guarantees: if a stale `go.sum` and a fresh `go mod graph` disagree about which versions are installed, the SBOM reflects what's actually installed.

- **FR-004**: Step 3's proxy fetcher MUST honor the standard Go module-proxy environment variables:
  - `GOPROXY` (comma- or pipe-separated list of proxy URLs; `off` disables fetching; `direct` falls through to source-control fetch — for 055, `direct` is treated as "skip the proxy, fall through to step 4," since implementing direct-VCS fetch is out of scope).
  - `GOPRIVATE` (comma-separated glob list of module-path prefixes that MUST NOT be fetched from a public proxy; matched modules fall through to step 4 immediately, even if step 3 would otherwise apply).
  - `GONOSUMCHECK` (advisory; mikebom does NOT verify proxy-fetched `.mod` files against `go.sum`'s `h1:` hashes in 055 because the hash protects the source `.zip`, not the `.mod` — the `.mod` has its own entry — and re-implementing `go.sum` verification is large enough to defer; tracked as a follow-up).
  - The mikebom-fetched `go.mod` files are NOT cached on disk (per-scan in-memory only); they're cheap enough that a per-scan re-fetch is acceptable, and not persisting them avoids polluting the user's `$GOMODCACHE`.

- **FR-005**: When `--offline` is set on the scan command, step 1 (`go mod graph`) and step 3 (proxy fetch) MUST be disabled. The ladder collapses to step 2 (cache walk) and step 4 (no-edges fallback). This preserves the existing `--offline` contract: mikebom scans MUST NOT make network calls. CI fixtures and reproducible-build pipelines that set `--offline` get deterministic edge emission bounded by what's in `$GOMODCACHE`.

- **FR-006**: The workspace's `replace` directives MUST be applied to transitive resolution, not just the main-module's direct edges. Concretely: when looking up a module's `go.mod` (steps 1–3), if the workspace `go.mod` has `replace foo v1.0.0 => bar v2.0.0`, the resolved `go.mod` for `foo@v1.0.0` is the `go.mod` of `bar@v2.0.0`. Step 1 (`go mod graph`) handles this natively; steps 2–3 must explicitly apply the workspace's replace map before computing the cache key / proxy URL.

- **FR-007**: A per-invocation timeout of 30 seconds MUST bound the `go mod graph` subprocess (mirroring 053's `git describe` 2-second discipline scaled for the larger expected workload). On timeout, the ladder falls through to step 2; the scan is NOT aborted. A `tracing::warn` line names the timeout.

- **FR-008**: Each proxy fetch MUST have a 10-second connect timeout and a 30-second total-request timeout. Failed fetches drop the affected module's outgoing edges from the SBOM and emit `tracing::warn` with module path and error class (timeout, 4xx, 5xx, DNS, connection-reset). The scan does NOT abort on individual fetch failures.

- **FR-008a**: Proxy fetches MUST run concurrently with a fixed cap of **16** in-flight requests, enforced by a single global semaphore in the resolver. The cap is not user-configurable in milestone 055; if real-world workloads require tuning we add a flag in a follow-up. Cap is shared across the resolver — even if the resolver eventually fans out per-fixture (e.g., parallel `go.work` member modules in a future milestone), the semaphore stays singleton-per-process to keep aggregate request rate bounded.

- **FR-009**: At the end of every scan that exercised the resolution ladder, mikebom MUST emit a single summary `tracing::info` line of the form `go transitive edges: ladder=[graph:N1, cache:N2, proxy:N3, missing:N4]` so users can see at a glance which sources contributed which edges, and which modules fell through to step 4.

- **FR-010**: All transitive edges MUST be emitted into the same output channels as 053's direct edges — i.e., the existing `depends` field on each `PackageDbEntry`, which the SBOM emitter renders as CDX `dependencies[].dependsOn`, SPDX 2.3 `relationships` of type `DEPENDS_ON`, and SPDX 3 `relationship` elements of type `dependsOn`. No new output schema; FR-010 is restated to lock in that 055 is purely a data-population change, not a format change.

- **FR-011**: A new unit test under `mikebom-cli/src/scan_fs/package_db/golang.rs`'s `#[cfg(test)] mod tests` MUST exercise the 4-step ladder with each step's success path independently mocked, AND each step's failure-fall-through path. The proxy-fetch path's tests MUST use a wiremock-style HTTP fixture (or equivalent) so the test suite remains hermetic — no real `proxy.golang.org` calls during `cargo test`.

- **FR-012**: A new integration test MUST scan the `tests/fixtures/go/argo-style-no-cache/argo-workflows/` fixture with `go` removed from `PATH` and `$GOMODCACHE` pointed at an empty temp dir, asserting that step 3 (proxy fetch, against a hermetic local mock proxy) produces the expected edge set. This is the direct regression test for issue #102's residual gap.

- **FR-013**: The pre-PR gate (`./scripts/pre-pr.sh`) MUST pass with the new code. Per the project CLAUDE.md, both `cargo +stable clippy --workspace --all-targets` AND `cargo +stable test --workspace` must show zero errors and `0 failed` across every suite.

### Key Entities

- **Resolution ladder**: an ordered list of 4 strategies (`go mod graph` → `$GOMODCACHE` → proxy fetch → no-edges) that mikebom tries in order to obtain each `(module, version)` pair's `go.mod`. The ladder is consulted once per scan for step 1 (a single subprocess) and per-module-version for steps 2–3.
- **Module-graph map**: a `HashMap<(String, String), Vec<(String, String)>>` keyed by `(module, version)` pair, mapping each module to the list of its required `(module, version)` pairs (after workspace replace/exclude applied). Built once per scan from whichever ladder step succeeds; consumed by the SBOM emitter.
- **`go mod graph` parser**: a small `parse_go_mod_graph(stdout: &str) -> ModuleGraphMap` function. Each output line is `parent[@version] child@version`; the main module has no `@version` suffix. Parser splits on whitespace, strips the optional `@version` from the parent, and accumulates into the map.
- **Proxy fetcher**: a `fetch_module_mod(proxy: &str, module: &str, version: &str) -> Result<String>` function that constructs the URL per the Go module proxy protocol (with module-path escape rules: uppercase letters become `!<lowercase>`, e.g., `Azure` → `!azure`), issues an HTTP GET, and returns the raw `go.mod` body. Honors `$GOPROXY`'s comma/pipe separator semantics: comma means "fall through on 404 only," pipe means "fall through on any error."
- **Edge intersection filter**: a closure applied to every candidate edge `(parent, child)` before emission, dropping the edge if `child` is not in the workspace's `go.sum`. Implemented as a `HashSet<(String, String)>` lookup against the existing `go.sum` parse.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the `tests/fixtures/go/argo-style-no-cache/argo-workflows/` fixture, with `go` removed from `PATH` and `$GOMODCACHE` set to an empty temp dir, mikebom emits transitive edges for ≥ 90% of the `go.sum` modules whose declared requires include other `go.sum` modules. Measured via the new integration test (FR-012).

- **SC-002**: On any committed Go fixture, with `go` on PATH and `$GOMODCACHE` populated, the edge set produced by mikebom matches the edge set produced by `go mod graph` (intersected with `go.sum`) with zero divergence. Asserted in CI for the `simple-module/` and `argo-style-no-cache/` fixtures.

- **SC-003**: On the `knative/func @ knative-v1.22.0` realistic-project fixture (introduced in milestone 054), the post-055 scan emits ≥ 200 transitive edges between `pkg:golang` components — a meaningful fraction of the project's full transitive dep graph and a clear regression backstop.

- **SC-004**: Wall-clock scan time on the existing 9-ecosystem fixture suite regresses by no more than 15% (matching milestone 054's noise envelope). Measured via the existing `tests/dual_format_perf.rs` infrastructure or an equivalent quick benchmark.

- **SC-005**: With `--offline` set, mikebom MUST NOT make any outbound network calls during the scan. Verified by running the existing offline test suite under a network-deny sandbox (e.g., `unshare -n` on Linux runners) — any DNS resolution or HTTP call fails the test.

- **SC-006**: Every emitted edge's target is a component in the same SBOM. Asserted by a post-emission scan that walks `dependencies[].dependsOn` (CDX) / `relationships` (SPDX) and verifies every referenced bom-ref / SPDXID exists in the `components` / `packages` array.

- **SC-007**: The summary `tracing::info` line from FR-009 appears in the scan log with non-zero counts for at least one of `graph:`, `cache:`, `proxy:` on every Go fixture in the test suite. This is the operational visibility check — it lets us see at a glance whether the ladder is actually exercising its branches in CI.

- **SC-008**: The pre-PR gate (`./scripts/pre-pr.sh`) passes locally and in CI on every supported runner (linux-x86_64, macos-latest). No skipped tests, no `0 failed; 1 ignored` shortcuts.

## Assumptions

- **`go.sum` is up-to-date relative to the workspace's actual install state.** This is the standard developer expectation; running `go build` in a workspace with a stale `go.sum` produces an error, so projects that build cleanly have a fresh `go.sum` by construction. Stale `go.sum` is handled (Q6 clarification: `go.sum` wins) but is not the design target.
- **`proxy.golang.org`'s `.mod` endpoint is stable.** The Go module proxy protocol is documented at <https://proxy.golang.org/> and is part of Go's stable public API since 1.13. If the protocol changes incompatibly, that's a separate maintenance item; the spec assumes the current contract holds.
- **Per-module `go.mod` fetch is cheap.** Each `.mod` file is typically 1–10 KB. Even for a workspace with 500 modules, the total network volume is a few megabytes — tolerable for the default-online behavior. If this assumption breaks on pathological projects (e.g., a workspace with 10000 modules), revisit during implementation; a per-scan budget cap could be added as an FR.
- **`go mod graph`'s output is parseable line-by-line as `parent child` pairs.** This format is documented and stable in `go help mod graph`.
- **Module-path proxy escaping rules are stable.** The escape spec (uppercase letters → `!<lowercase>`) is documented in Go's source (`cmd/go/internal/module/module.go`) and has been stable since modules launched. We replicate this escape in mikebom rather than depending on a Go binary to do it.
- **No new crate.** The standard library has `std::process::Command` for `go mod graph`, the existing `reqwest` (already a dep for proxy fetches in other ecosystems — verify at planning time) for HTTP, and `serde`/`anyhow`/`tracing` for the rest. If `reqwest` is NOT already a dep, the planning-phase decision is between adding it as a dep or using `ureq` / a `curl` subprocess; this is an implementation choice not a spec choice.
- **`GOPRIVATE` matching uses the standard glob rules.** `GOPRIVATE=*.corp.example.com,github.com/our-org/*` matches via Go's `module.MatchPrefixPatterns` semantics; mikebom replicates this rather than depending on `go env GOPRIVATE`.
- **Out of scope: `go.work` workspaces.** Multi-module Go workspaces have a meaningfully different resolution model and warrant their own milestone. 055 is single-module-workspace only, matching the existing 049/053 scope. A follow-up issue will be filed at planning time if not pre-existing.
- **Out of scope: `vendor/` directory–driven resolution.** Vendored projects still have `go.sum`, so the ladder still applies, but if `-mod=vendor` is the default in the user's environment `go mod graph` may behave differently. This case is rare enough in modern Go projects that 055 doesn't address it; if it manifests in the realistic-project CI suite (054) we file a follow-up.
- **Out of scope: deps.dev.** Per the user's directive, deps.dev is NOT a data source for transitive resolution. It can be added as a supplementary cross-check tool in a separate observability/QA milestone; it does not produce the SBOM's edges.
- **Out of scope: source-VCS fallback** (the `direct` arm of `GOPROXY`). Implementing direct git/hg/svn fetches to obtain a module's `go.mod` is materially more complex (auth, cloning, version resolution from tags) and is reserved for a follow-up if real users hit it. Step 4's no-edges fallback is the documented behavior when only `direct` is configured.
- **Out of scope: `go.sum` hash verification on proxy-fetched `.mod` files.** The `go.sum` `h1:` hash protects the source `.zip` and a separate `/go.mod` hash protects the `.mod`; verifying the latter requires reproducing Go's per-`.mod` hash function. Worth doing eventually for supply-chain hardening, but not part of 055 — tracked as a follow-up.
