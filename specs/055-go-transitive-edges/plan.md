# Implementation Plan: Go transitive dependency edges, anchored on `go.sum`

**Branch**: `055-go-transitive-edges` | **Date**: 2026-05-02 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/055-go-transitive-edges/spec.md`

## Summary

Generalize milestone 053's main-module direct-edge emission to **all** components sourced from `go.sum`, by populating each `PackageDbEntry`'s `depends` field with the module's transitive requires. The implementation hinges on a 4-step resolution ladder for obtaining each `(module, version)`'s `go.mod`:

1. **`go mod graph`** subprocess (when `go` is on PATH and `--offline=false`) — one invocation supplies the full DAG.
2. **`$GOMODCACHE` walk** — current 053 codepath (`cache_lookup_depends` in `mikebom-cli/src/scan_fs/package_db/golang.rs:570`).
3. **Proxy fetch** from `$GOPROXY` (default `https://proxy.golang.org`) using the Go module proxy's `<proxy>/<escaped-mod>/@v/<ver>.mod` endpoint, gated by `--offline=false`, `GOPRIVATE`, and a 16-way concurrency semaphore.
4. **Graceful no-edges fallback** — component still emits, `tracing::warn` names the fallthrough, ladder summary line records the count.

Edges are intersected with `go.sum` before emission (FR-003: `go.sum` is canonical for what's installed). Output schema is unchanged — populates the existing `depends` field consumed by CDX `dependencies[].dependsOn`, SPDX 2.3 `relationships[type=DEPENDS_ON]`, and SPDX 3 `relationship[type=dependsOn]`.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–054; no nightly required for this user-space-only work).
**Primary Dependencies**: Existing only — `reqwest` (workspace, `default-features = false, features = ["json", "rustls-tls"]`) for proxy `.mod` fetches; `tokio` (workspace) for async semaphore + concurrent fetches; `std::process::Command` for `go mod graph` subprocess (same pattern as `git describe` at `golang.rs:733`); `serde_json`/`tracing`/`anyhow` already pervasive. **One new dev-only dep**: `wiremock = "0.6"` for FR-011/FR-012 hermetic HTTP fixture (alternative: hand-rolled `tokio::net::TcpListener` stub if dev-dep addition is contested in review).
**Storage**: N/A — all state in-process per scan; no persistence (matches milestones 002–053 posture, restated in spec Q3 clarification).
**Testing**: `cargo +stable test --workspace` (unit + integration); `cargo +stable clippy --workspace --all-targets -- -D warnings` (per pre-PR gate). Network-isolated tests via `wiremock` local listener.
**Target Platform**: linux-x86_64 + macos-latest CI runners. No platform-specific code — `Command`, `reqwest`, and `tokio` are all cross-platform.
**Project Type**: CLI tool / library crate (mikebom-cli). Single workspace; all changes in `mikebom-cli/src/scan_fs/package_db/golang*` plus a new integration test file.
**Performance Goals**:
- ≤ 15% wall-clock regression on existing 9-ecosystem fixtures (SC-004).
- ≤ 30 s end-to-end for `tests/fixtures/go/argo-style-no-cache/argo-workflows/` with the proxy-fetch path active against a hermetic mock proxy (SC-001 + FR-012).
- Proxy fetches: 16-way concurrency, 10 s connect timeout, 30 s total per-request (FR-008 + FR-008a).
- `go mod graph`: 30 s timeout per invocation (FR-007).
**Constraints**:
- No new top-level crate (Principle VI: three-crate architecture).
- No `.unwrap()` in production code (Principle IV); test code requires `#[cfg_attr(test, allow(clippy::unwrap_used))]` per CLAUDE.md convention.
- No new output-format properties — `mikebom:*` annotations explicitly forbidden where a native `dependsOn` exists (Principle V; spec FR-010).
- `--offline` MUST disable both step 1 and step 3 (FR-005); existing flag wiring is at `mikebom-cli/src/main.rs:70` → `scan_cmd.rs:583` → ecosystem readers.
- `GOPRIVATE` MUST short-circuit step 3 to prevent leaking private module names to public proxies (FR-004).
**Scale/Scope**:
- Typical workspace: 0–500 `go.sum` entries.
- Outliers (kubernetes, open-telemetry/opentelemetry-collector-contrib, knative): up to ~1500 entries.
- Per-scan ladder summary: O(N) modules, expected wall-clock ≤ 30 s on the worst case with mock proxy at near-zero RTT.

## Constitution Check

Auditing 055 against `.specify/memory/constitution.md` v1.4.0:

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Pure Rust, Zero C** | ✅ Compliant | No C touched; `reqwest`'s `rustls-tls` feature already pinned in workspace excludes OpenSSL. `wiremock` is pure Rust. |
| **II. eBPF-Only Observation** | ⚠️ N/A — scan mode | The principle applies to `mikebom trace` (eBPF path). The `mikebom sbom scan` codepath (where 055 lives) is a documented separate static-analysis path that has shipped with milestones 001–054. 055 does not change which mode dependencies come from. |
| **III. Fail Closed** | ✅ Compliant | Trace-mode principle. Scan mode's analog is FR-007/FR-008/FR-009 graceful fallthrough with explicit logging — the user can SEE the gap rather than being misled by silently-empty edges. |
| **IV. Type-Driven Correctness** | ✅ Compliant — but watch | New `(module, version)` pair will be a tuple-typed `ModuleId` newtype, NOT a raw `String`. `anyhow::Error` for application errors. No `.unwrap()` in production; `?` propagation throughout. Test code uses `#[cfg_attr(test, allow(clippy::unwrap_used))]`. |
| **V. Specification Compliance** | ✅ Compliant | Spec FR-010 explicitly locks "no new output schema." Edges flow into the existing `depends` field → CDX `dependencies[].dependsOn` (native), SPDX 2.3 `relationships` (native), SPDX 3 `relationship` (native). The standards-native audit was performed: `dependsOn` exists and is the right primary signal in all three formats. No `mikebom:*` properties are added. |
| **VI. Three-Crate Architecture** | ✅ Compliant | All code changes in `mikebom-cli/`. No new crate. |
| **VII. Test Isolation** | ✅ Compliant | Unit tests for parser + fetcher run with no privileges. Integration test (FR-012) uses `wiremock`'s local-loopback listener — no real network, runs in unprivileged CI. |
| **VIII. Completeness / IX. Accuracy** | ✅ Compliant | Trace-mode principles; spirit applies. FR-001 + FR-003 prevent false positives (no edge to a module not in `go.sum`); FR-009 ladder summary surfaces completeness gaps. |
| **X. Transparency** | ✅ Compliant | FR-009's `tracing::info` summary uses spec-native logging mechanism. The specifically-cited Principle X examples (CDX `confidence`, `evidence`, `property` fields) apply to in-SBOM data quality annotations; the edge data we emit is canonical (matches `go mod graph` output regardless of source), so per-edge SBOM evidence is not required. See **Principle XII flag** below for the related question. |
| **XI. Enrichment** | ✅ Compliant | Edges are enrichment data per the principle's framing; no enrichment-source unavailability blocks SBOM generation (FR-005 graceful degradation). |
| **XII. External Data Source Enrichment** | ⚠️ **Flagged for review** | Constraint #1 (no new components from external sources): satisfied by FR-003 intersection. Constraint #3 (graceful degradation): satisfied by ladder step 4. Constraint #4 (eBPF trace authoritative): N/A in scan mode. **Constraint #2 ("Data from external sources MUST be annotated with its provenance")**: ambiguous for 055. Strict reading: every edge derived from a proxy fetch should carry per-edge provenance ("relationship from proxy.golang.org"). Permissive reading (consistent with milestone 053 precedent — 053 emits cache-derived edges with no per-edge SBOM provenance, only `tracing::debug` breadcrumbs): the FR-009 scan-summary log is sufficient. **Plan resolution: research the precedent in milestones 049/053, then either (a) accept FR-009 as the documented provenance mechanism, or (b) add per-edge `evidence` annotations only when the edge came from step 3 (proxy), not steps 1 or 2 (local data sources).** Tracked in research.md Q8. |
| **Strict Boundary 1 (No lockfile-based discovery)** | ✅ Compliant | `go.sum` is used to define the *module set* (already done by milestone 049, pre-existing) and to *intersect* candidate edges. 055 doesn't change discovery; it adds relationships, which Principle XII bullet 1 explicitly permits. |
| **Strict Boundary 4 (No `.unwrap()` in production)** | ✅ Compliant | Pre-PR gate enforces. |
| **Pre-PR Verification** | ✅ Will be enforced | FR-013 codifies the mandatory `cargo +stable clippy --workspace --all-targets -- -D warnings` + `cargo +stable test --workspace` gate. |

**Pre-Phase-0 verdict**: Plan proceeds. One open question (XII Constraint #2) escalated to research.md Q8 — does NOT block planning, but its resolution may add 1–2 FRs and 1 SC.

## Project Structure

### Documentation (this feature)

```text
specs/055-go-transitive-edges/
├── spec.md                        # Feature spec (already written)
├── plan.md                        # This file
├── research.md                    # Phase 0 output (this command)
├── data-model.md                  # Phase 1 output (this command)
├── quickstart.md                  # Phase 1 output (this command)
├── contracts/
│   └── resolver-api.md            # Phase 1 — internal Resolver trait + ladder contracts
├── checklists/
│   └── requirements.md            # From /speckit.specify
└── tasks.md                       # Phase 2 (NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   └── scan_fs/
│       └── package_db/
│           └── golang/                                # T008 promotes existing `golang.rs` to a directory module
│               ├── mod.rs                             # NEW — declares submodules, re-exports public API
│               ├── legacy.rs                          # FORMERLY `golang.rs` (~2018 lines, milestones 049/053/054); body unchanged in T008, only the path moves
│               ├── module_id.rs                       # NEW — ModuleId newtype + Display + parsing
│               ├── graph_resolver.rs                  # NEW — 4-step ladder orchestrator + all resolver types
│               ├── go_mod_graph.rs                    # NEW — step 1 subprocess + output parser
│               ├── proxy_fetch.rs                     # NEW — step 3 HTTP client + module-path escape
│               └── goprivate.rs                       # NEW — GOPROXY/GOPRIVATE env-var parsers
├── tests/
│   └── go_transitive_edges.rs                         # NEW integration test (FR-012)
└── Cargo.toml                                         # +1 dev-dep: `wiremock = "0.6"`

tests/fixtures/go/
└── argo-style-no-cache/                               # Existing fixture (milestone 053) — reused
    ├── argo-workflows/
    │   ├── go.mod
    │   └── go.sum
    └── proxy-mock/                                    # NEW — synthesized .mod files for the integration test (T026)
```

**Structure Decision**: The existing `mikebom-cli/src/scan_fs/package_db/golang.rs` is **promoted to a directory module** in T008 — the file moves to `golang/legacy.rs` (body unchanged) and a new `golang/mod.rs` declares the submodule structure and re-exports the public API. Rust's module system disallows having both `golang.rs` and a `golang/` directory at the same level, so the existing file MUST be renamed when the submodule directory is introduced; T008 does this in a single commit with all import-path updates. New resolver code (~800 LOC across 5 new files) lives alongside `legacy.rs`. This keeps blast radius small (existing 2018 lines untouched in 055, just relocated) while giving the resolver enough room to grow into separate files (one per ladder step + helpers). All changes are inside the existing `mikebom-cli` crate per Principle VI.

The resolver is invoked from `legacy.rs::read()` (line ~826 in the pre-T008 file; same line number post-rename) once per scan, replacing the per-entry `cache_lookup_depends()` call with a single `graph_resolver::GraphResolver::resolve()` call that returns a `ModuleGraphMap`. Each `PackageDbEntry`'s `depends` field is then populated by lookup against this map.

## Complexity Tracking

> No constitution violations require justification. The Principle XII Constraint #2 question is flagged but not violated — the resolution is in research.md.

The only complexity adder is the new `golang/` submodule structure (vs. continuing to grow the single `golang.rs` past 2000 lines). Justification: the resolver has 4 distinct concerns (subprocess, HTTP, env-var parsing, orchestration); inlining all of them into the existing 2000-line file pushes it past readable maintainability and conflicts with the existing convention that modules cap around 1500 lines (`mikebom-cli/src/scan_fs/package_db/cargo.rs` is the largest sibling at ~900 lines). One submodule directory is cheaper than the readability tax of keeping the file flat.

| "Violation" | Why Needed | Simpler Alternative Rejected Because |
|-------------|------------|--------------------------------------|
| Promote `golang.rs` to `golang/` directory module + add 5 new submodule files | 4 distinct concerns × ~150–300 LOC each = ~800 new LOC; folding into `golang.rs` (preserved as `legacy.rs`) brings it to ~2800 lines. Rust's module system disallows `golang.rs` + `golang/` siblings, so the rename is unavoidable once we want submodule files. | Inline everything in `golang.rs`: rejected because the file becomes the largest in the workspace by 2× and grep-based navigation degrades. Keep `golang.rs` and put new files elsewhere: rejected because it scatters one ecosystem's logic across distant directories. |
| 1 new dev-dep (`wiremock`) | Hermetic HTTP fixture for FR-011/FR-012; otherwise integration test would either hit real proxy.golang.org (forbidden — Principle VII test isolation, plus flaky CI) or hand-roll a `tokio::net::TcpListener` stub (≥150 LOC of test infrastructure that re-implements wiremock badly) | Hand-rolled stub: rejected because `wiremock` is a 1-line Cargo.toml change vs. ongoing maintenance of a homemade mock; dev-deps don't affect production binary size |
