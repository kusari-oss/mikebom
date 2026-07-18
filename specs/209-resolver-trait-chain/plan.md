# Implementation Plan: Resolver Trait + Chain Refactor

**Branch**: `209-resolver-trait-chain` | **Date**: 2026-07-18 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/209-resolver-trait-chain/spec.md`

## Summary

Refactor `mikebom-cli/src/resolve/pipeline.rs` from a fixed URL → hash → hostname (per-connection) + path (per-file-op) function-call sequence into a **`Resolver` trait** + **chain iteration**. Extract the 832-LOC `url_resolver.rs` monolith into 7 per-ecosystem modules (Cargo, PyPI, npm, Golang, Maven, RubyGems, Deb). Wrap `hash_resolver`, `path_resolver`, `hostname_resolver` as `Resolver`-trait implementations. Byte-identical output on the existing test corpus (SC-001). Zero new Cargo dependencies. Zero pipeline-orchestration edits required to add a new ecosystem (SC-002).

Technical approach: native Rust `async fn` in traits (stable since Rust 1.75; no `async-trait` crate needed) with a `Result<Vec<ResolvedComponent>, ResolverError>` return per clarifications Q1. Compile-time priority-uniqueness via a `const fn` uniqueness check on a `const RESOLVER_REGISTRY: [(&str, u32); N]` table per clarifications Q2 (no new crates). `std::panic::catch_unwind` wrapped in `AssertUnwindSafe` (safe because resolvers are stateless per FR-014) provides the pipeline-layer panic-catch of FR-013.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–208). Requires the RPITIT (return-position `impl Trait` in trait) feature stabilized in Rust 1.75 — no MSRV bump since the workspace is already past 1.75 (verified: milestone 100 confirmed Windows/macOS/Linux support). No nightly features.

**Primary Dependencies**: Existing only — `tokio` (async runtime, already pervasive), `anyhow`/`thiserror` (error surface), `tracing` (WARN + INFO logs), `std::panic::catch_unwind` + `std::panic::AssertUnwindSafe` (panic-catch per FR-013). **Zero new Cargo dependencies.** No `async-trait` crate needed (RPITIT covers it).

**Storage**: N/A — pipeline state lives on the stack for the duration of a single `resolve()` call. Matches every resolver milestone.

**Testing**: `cargo +stable test --workspace` for unit + integration. New per-resolver unit test suites at `mikebom-cli/src/resolve/resolvers/<ecosystem>.rs::mod tests` — each targets one ecosystem in isolation per FR-009. Compile-time-collision test at `mikebom-cli/tests/resolver_priority_collision.rs` (a `trybuild`-style compile-fail test — but implemented as a `build.rs`-driven fixture rather than adding the `trybuild` dep per zero-new-deps discipline). Byte-identity regression test at `mikebom-cli/tests/resolver_chain_byte_identity.rs` — replays the fixture corpus through the chain, asserts output matches a checked-in reference (regenerated once from the pre-refactor pipeline before the refactor lands, then locked). Perf-regression test at `mikebom-cli/tests/resolver_chain_perf.rs` `#[ignore]`-gated per m094 / m208 convention — runs the m083 audit fixture and asserts wall-clock ≤ 105 % of a baseline stored in `mikebom-cli/tests/fixtures/resolver_chain/perf_baseline.json`.

**Target Platform**: All hosts mikebom supports today — Linux, macOS, Windows. Pure user-space refactor; no eBPF touched. `catch_unwind` works on all three.

**Project Type**: cli (extends the existing `mikebom-cli` crate; no new crates per Principle VI).

**Performance Goals**: Per SC-004, ≤5 % wall-clock regression on the resolution benchmark. Async dispatch overhead is dominated by the deps.dev network call; refactor overhead is dominated by `catch_unwind` cost (~50 ns per resolver invocation on prior perf work) + dynamic dispatch (~1 ns per call). Aggregate overhead on a 500-connection scan: ~200 µs — well under the 5 % ceiling on a typical 30-second full resolution pass.

**Constraints**: Byte-identical output on existing golden corpus (SC-001 — no golden regen). CLI surface unchanged (FR-011 preserves `--skip-purl-validation`). Panic-safe dispatch (FR-013). Constitution Principle IV (no `.unwrap()` in production). Constitution Principle VI (no new crate). Zero new Cargo deps.

**Scale/Scope**: 4 existing "resolver categories" (URL family, Hash, Path, Hostname) — after refactor, 11 total `Resolver`-trait impls (7 URL-family ecosystems split out + Hash + Path + Hostname + a NuGet proof-of-concept for SC-002). Chain length: 11. Per-scan invocation count: N connections + M file-ops (typical: N=50–500 connections, M=1000–10000 file-ops for a container-image scan).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

**Principle I (Pure Rust, Zero C)** — PASS. All refactor code is pure Rust. `catch_unwind` is stdlib. No FFI. ✓

**Principle II (eBPF-Only Observation)** — N/A. Refactor is post-trace resolution; doesn't touch the trace path. ✓

**Principle III (Fail Closed)** — PASS. Individual resolver failures (Err or panic) are caught + logged (per FR-013) but the pipeline continues; the OVERALL resolution pass still fails-closed if the deps.dev tier is required and unavailable, matching pre-refactor semantics. ✓

**Principle IV (Type-Driven Correctness)** — PASS with attention. The refactor MUST NOT introduce `.unwrap()` calls in production code. `catch_unwind`'s return type is `Result<T, Box<dyn Any + Send>>`; the pipeline handles it via `match` (not `.unwrap()`). The compile-time priority-uniqueness check uses `const fn` + `assert!` (which panics at const-eval time — this is a compile error, not a runtime panic; Principle IV explicitly permits panics at compile time). ✓

**Principle V (Specification Compliance)** — N/A. Refactor doesn't touch SBOM emission; downstream serializers see the same `ResolvedComponent` shape as before. ✓

**Principle VI (Three-Crate Architecture)** — PASS. All refactor code lives in `mikebom-cli`. The `Resolver` trait is `pub(crate)` per Assumptions ("No public-API exposure"). No new crate. ✓

**Principle VII (Test Isolation)** — PASS. All tests run without root; no eBPF privileges required. Per-resolver test isolation is the whole point of the refactor (US2). ✓

**Principle VIII (Completeness)** — N/A. Refactor doesn't change what components are surfaced; SC-001 byte-identity guarantees no completeness regression. ✓

**Principle IX (Accuracy)** — PASS. Byte-identical resolution (SC-001) preserves accuracy exactly. ✓

**Principle X (Transparency)** — PASS. `ResolutionEvidence.technique` signal explicitly preserved by US3 + SC-005. FR-013 WARN logs surface resolver failures visibly. ✓

**Principle XI (Enrichment)** — N/A. Refactor doesn't touch enrichment; `DepsDevHashResolver` continues to enrich per pre-refactor semantics. ✓

**Principle XII (External Data Source Enrichment)** — PASS. `DepsDevHashResolver` (wrapping the existing `hash_resolver`) continues to honor `--skip-purl-validation` per FR-011. ✓

**Strict Boundaries** — all pass:
- #1 (No lockfile-based discovery): N/A — refactor is post-trace resolution.
- #2 (No MITM proxy): N/A.
- #3 (No C code): PASS.
- #4 (No `.unwrap()` in production): PASS — panic-catch uses `match`, not `.unwrap()`.
- #5 (No file-tier duplicates in default mode): N/A.

**No violations.** Complexity Tracking table omitted.

## Project Structure

### Documentation (this feature)

```text
specs/209-resolver-trait-chain/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
├── checklists/
│   └── requirements.md  # From /speckit-specify (all 16 items pass)
└── tasks.md             # Phase 2 output (via /speckit-tasks)
```

### Source Code (repository root)

Refactor within existing `mikebom-cli/src/resolve/` — same crate, same top-level module tree, reorganized internally:

```text
mikebom-cli/
├── Cargo.toml                                # unchanged (zero new deps)
├── src/
│   ├── resolve/
│   │   ├── mod.rs                            # UPDATED: re-exports post-refactor public API + registers module tree
│   │   ├── pipeline.rs                       # SHRUNK: from 507 LOC to ~150; delegates to chain iteration
│   │   ├── resolver_trait.rs                 # NEW: `Resolver` trait + `ResolverError` + `ResolveInput` enum
│   │   ├── resolver_chain.rs                 # NEW: `ResolverChain` + `RESOLVER_REGISTRY` const + compile-time uniqueness check
│   │   ├── resolvers/                        # NEW: per-ecosystem + per-technique resolver modules
│   │   │   ├── mod.rs                        # NEW: re-exports each resolver's public type
│   │   │   ├── cargo.rs                      # NEW: extracted from url_resolver::resolve_cargo
│   │   │   ├── pypi.rs                       # NEW: extracted from url_resolver::resolve_pypi
│   │   │   ├── npm.rs                        # NEW: extracted from url_resolver::resolve_npm
│   │   │   ├── golang.rs                     # NEW: extracted from url_resolver::resolve_golang
│   │   │   ├── maven.rs                      # NEW: extracted from url_resolver::resolve_maven
│   │   │   ├── rubygems.rs                   # NEW: extracted from url_resolver::resolve_rubygems
│   │   │   ├── deb.rs                        # NEW: extracted from url_resolver::resolve_deb (deb-codename context)
│   │   │   ├── deps_dev_hash.rs              # NEW: wraps existing hash_resolver::HashResolver
│   │   │   ├── path.rs                       # NEW: wraps existing path_resolver::resolve_path_with_context
│   │   │   └── hostname_fallback.rs          # NEW: wraps existing hostname_resolver::resolve_hostname
│   │   ├── url_resolver.rs                   # DELETED: 832 LOC extracted into resolvers/{cargo,pypi,npm,golang,maven,rubygems,deb}.rs
│   │   ├── hash_resolver.rs                  # PRESERVED: internal impl; called by resolvers/deps_dev_hash.rs wrapper
│   │   ├── path_resolver.rs                  # PRESERVED: internal impl; called by resolvers/path.rs wrapper
│   │   ├── hostname_resolver.rs              # PRESERVED: internal impl; called by resolvers/hostname_fallback.rs wrapper
│   │   ├── component_role.rs                 # unchanged
│   │   ├── deduplicator.rs                   # unchanged
│   │   └── reconciler.rs                     # unchanged
│   └── ... (rest of crate unchanged)
├── tests/
│   ├── resolver_chain_byte_identity.rs       # NEW: SC-001 regression harness
│   ├── resolver_chain_perf.rs                # NEW: SC-004 perf regression (#[ignore]-gated)
│   ├── resolver_priority_collision.rs        # NEW: compile-fail test verifying two resolvers with matching priorities fail cargo build
│   └── fixtures/
│       └── resolver_chain/
│           ├── attestation_corpus/           # NEW: 5–10 checked-in attestation fixtures spanning every resolver's happy path
│           ├── byte_identity_reference.json  # NEW: pre-refactor pipeline output — locked at PR merge, byte-compared post-refactor
│           └── perf_baseline.json            # NEW: wall-clock baseline for SC-004
└── docs/
    └── architecture/
        └── resolvers.md                       # NEW per FR-016 — resolver-authoring guide (trait shape, registration, priority conventions, testing patterns, panic + error semantics)
```

**Structure Decision**: All new code lives inside `mikebom-cli/src/resolve/` — no new crate (Principle VI). The `Resolver` trait + registry live at `resolve/resolver_trait.rs` and `resolve/resolver_chain.rs` (siblings of `pipeline.rs`). Per-resolver implementations live under a new `resolve/resolvers/` subdirectory, one file per resolver. The three legacy resolver modules (`hash_resolver.rs`, `path_resolver.rs`, `hostname_resolver.rs`) are preserved as internal implementation modules; the new `resolvers/{deps_dev_hash,path,hostname_fallback}.rs` files are thin `Resolver`-trait wrappers around them. `url_resolver.rs` is deleted — its 7 ecosystem-specific functions split into 7 separate files under `resolvers/`.

## Complexity Tracking

No constitution violations. Section deliberately empty — this is a within-module refactor that adds no principle-level exceptions.
