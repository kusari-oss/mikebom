# Implementation Plan: Shared `safe_walk` Helper Migration

**Branch**: `114-safe-walk-migration` | **Date**: 2026-06-12 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/114-safe-walk-migration/spec.md`

## Summary

Extract `mikebom-cli/src/scan_fs/package_db/project_roots.rs`'s canonicalize-keyed visited-set + depth-bound + skip-predicate machinery into a new top-level module `mikebom-cli/src/scan_fs/walk.rs` exposing a single `safe_walk(rootfs, &cfg, |path| visit)` helper. Port every hand-rolled `fn walk_*` recursion in `scan_fs/` to delegate to it. Two known exceptions stay documented inside the new module: the whole-filesystem deep-hash walker (`scan_fs/walker.rs::walk_and_hash` — pre-existing pre-emission file enumeration with its own size cap; structurally doesn't fit the project-discovery model) and the npm `@scope`-aware tree walker (`scan_fs/package_db/npm/walk.rs::walk_node_modules` — parent-directory-name-aware semantics + per-descent `in_npm_internals: bool` state propagation).

**Technical approach**: greenfield helper file built from `project_roots.rs`'s known-good descent loop with the API shape clarified in Q1+Q2 — single `FnMut(&Path)` visit callback + `FnMut(&Path, &Path) -> bool` skip predicate over `(candidate, rootfs)`. The new helper composes the milestone-113 `ExclusionSet` check into the skip-predicate closure rather than carrying it as a separate `WalkConfig` field (collapsing today's two-step check). Migration order: helper module first (replaces project_roots.rs without behavior change), then each per-ecosystem walker individually, each commit preserving byte-identity against the 33 committed goldens (FR-009 / SC-002).

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–113; no nightly required for this user-space-only refactor).
**Primary Dependencies**: existing only — `std::fs::{canonicalize, read_dir}`, `std::path::{Path, PathBuf}`, `std::collections::HashSet`, `tracing` (for debug-skip logs). Reuses milestone-113's `ExclusionSet` at `mikebom-cli/src/scan_fs/package_db/exclude_path.rs`. **Zero new Cargo dependencies** per FR-011 (no `walkdir`, no `ignore`, no `globwalk`).
**Storage**: N/A — visited-set is per-call in-memory state; cleared at each `safe_walk` invocation. No persistence.
**Testing**: `cargo +stable test --workspace` (existing harness). New tests: ~12 unit tests in `scan_fs::walk::tests` covering canonicalize-keyed dedup, depth bound, skip-predicate invocation, ExclusionSet integration, symlink-loop bounded termination, unreadable-dir tolerance. Migration-specific tests are the existing per-walker tests (which must continue passing byte-identically — SC-002).
**Target Platform**: Linux x86_64 + macOS aarch64 + Windows x86_64 (same matrix as every milestone since 001).
**Project Type**: Single-project Rust CLI (`mikebom-cli/`).
**Performance Goals**: ≤105% scan time vs pre-migration (SC-004). Achieved by preserving the existing descent shape — no new allocations per-directory, same `HashSet<PathBuf>` visited-set semantics, same `tracing::debug!` cadence.
**Constraints**: byte-identical SBOM output across all 33 committed goldens with no `MIKEBOM_UPDATE_*_GOLDENS` regen (FR-009 / SC-002); no new Cargo deps (FR-011); the audit grep `grep -rn 'fn walk' mikebom-cli/src/scan_fs/` must match only the new helper module after all migration PRs land (FR-010 / SC-001).
**Scale/Scope**: 15 hand-rolled walkers identified across `scan_fs/` (inventory in §"Walker inventory" below — counts include both top-level entry points and their internal recursive helpers across 11 source files); 2 documented known exceptions (`walker.rs` deep-hash, `npm/walk.rs` @scope-aware).

## Constitution Check

| Principle | Status | Notes |
|---|---|---|
| I. Pure Rust, Zero C | ✓ | std-only refactor; no new C deps. |
| II. eBPF-Only Observation | N/A | `scan` codepath; trace is untouched. |
| III. Fail Closed | ✓ | Helper silently skips unreadable directories (matches every existing walker's pre-migration behavior; FR-003). |
| IV. Type-Driven Correctness | ✓ | New `WalkConfig` carries `&'a dyn Fn(&Path, &Path) -> bool` + `&'a ExclusionSet` + `max_depth: usize` — typed surfaces, no raw `String`s across boundaries; zero `.unwrap()` in production code. |
| V. Specification Compliance | ✓ | No SBOM output change — pure internal refactor; the byte-identity guarantee in FR-009 is the contractual check. |
| VI. Three-Crate Architecture | ✓ | Lives in `mikebom-cli/`; no new crates. |
| VII. Test Isolation | ✓ | Pure-logic + filesystem-walk tests; no eBPF, no privileged operations. |
| VIII. Completeness | ✓ | Byte-identity preserves the existing path-discovery completeness. |
| IX. Accuracy | ✓ | Byte-identity preserves the existing accuracy posture. |
| X. Transparency | ✓ | The existing per-walker `tracing::debug!` skip-event emissions are centralized into the helper — net-positive observability (today only `project_roots.rs` logs skip events; post-migration every walker does). |
| XI. Enrichment | N/A | |
| XII. External Data Source Enrichment | N/A | |
| Strict Boundary 1 (no lockfile-based discovery) | ✓ | Unchanged. |
| Strict Boundary 2 (no MITM) | N/A | |
| Strict Boundary 3 (no C code) | ✓ | |
| Strict Boundary 4 (no `.unwrap()` in production) | ✓ | All new error paths use `?` + `Option::unwrap_or_else` over Path-canonicalize-failure (matches existing pattern). |

**Result**: Constitution Check PASSES. No violations.

## Walker inventory

Hand-rolled walkers under `mikebom-cli/src/scan_fs/` enumerated by `grep -rn '^fn walk' mikebom-cli/src/scan_fs/`. Each gets ported in its own per-walker phase (see Tasks).

**Standard ecosystem walkers — migrate to `safe_walk`** (15 walker functions across 11 source files):

| File | Function | Yields | Notes |
|---|---|---|---|
| `package_db/project_roots.rs` | `walk_for_project_roots` + `walk_inner` | dirs matching `is_project_root` | **The reference**. First port: replaced by `safe_walk` wrapper exposing the same public API to existing pip/npm/gradle/nuget/yocto closures. |
| `package_db/cargo.rs` | `walk_for_cargo_manifests` | `Cargo.toml` files | Has milestone-113 `&ExclusionSet` threading. |
| `package_db/cargo.rs` | `walk_for_cargo_lockfiles` | `Cargo.lock` files | Same. |
| `package_db/go_binary.rs` | `walk_for_binaries` | binary files | Carries `claimed_paths` + `claimed_inodes` + `seen_purls` mutable refs through descent; callback pattern fits (closure captures them). |
| `package_db/golang/legacy.rs` | `walk_for_go_roots` | `go.mod` parent dirs | Has milestone-113 Go testdata/underscore unconditional skips. |
| `package_db/maven.rs` | `walk_for_maven` | `pom.xml` + `.jar` files | Two file types; one callback handles both via extension switch. |
| `package_db/maven.rs` | `walk_for_top_level_poms` | top-level `pom.xml` files | |
| `package_db/gem.rs` | `walk_for_top_level_gemspecs` | top-level `.gemspec` files | |
| `package_db/gem.rs` | `walk_for_gemfile_locks` | `Gemfile.lock` files | |
| `package_db/gem.rs` | `walk_for_gemspecs` | `.gemspec` files | |
| `package_db/rpm_file.rs` | `walk_dir` | `.rpm` files | |
| `package_db/nuget/mod.rs` | `walk_inner` | `.csproj` / `.vbproj` / `.fsproj` files | Currently calls `should_skip_default_descent` + ExclusionSet inline; folds into the predicate closure. |
| `package_db/yocto/recipe.rs` | `walk` | `.bb` files (emits `PackageDbEntry` inside the walk) | Callback pattern fits emit-inside-walk. |
| `binary/discover.rs` | `walk_dir` | binary files | |
| `binary/source_binding/cmake_observer.rs` | `walk_for_cmake_build_dirs` | cmake build directories | |

**Documented known exceptions — STAY hand-rolled** (2):

| File | Function | Reason it cannot delegate |
|---|---|---|
| `scan_fs/walker.rs` | `walk_and_hash` / `walk` | Whole-filesystem file enumeration with a size cap + content-hashing inside the walk; semantics are "yield every file ≤ N bytes with its SHA-256". No skip list, no project-root discrimination, no depth bound — none of the `safe_walk` config makes sense here. Migrating would either bloat the helper's API surface with file-walking-specific knobs (`size_cap: u64`, `compute_hash: bool`) — violating the helper's "stays minimal" intent — or force a parallel helper variant. Documented as a known exception. |
| `scan_fs/package_db/npm/walk.rs` | `walk_node_modules` | Treats `@scope` directories specially: it only recurses one level into them (to find the actual packages under the scope) and propagates `in_npm_internals: bool` per-descent state through the recursive calls (npm-self-bundled-internals tagging — feature 005 US1). Generic depth-bound + opaque skip-predicate can't express "recurse one level only into directories whose name starts with `@`" plus "propagate this bool unchanged through children." Documented as a known exception. |

Both exceptions get a comment block at the top of `scan_fs/walk.rs` enumerating them with reasons. The audit grep `grep -rn 'fn walk' mikebom-cli/src/scan_fs/` will then show: matches in `walk.rs` (the helper itself) + matches in `walker.rs` and `npm/walk.rs` (the documented exceptions). The doc-section in `design-notes.md` (FR-012, SC-006) instructs reviewers to verify any new `fn walk_*` match is either inside the helper module OR documented as a new exception (and to push back on the latter aggressively).

## Project Structure

### Documentation (this feature)

```text
specs/114-safe-walk-migration/
├── plan.md              # This file
├── research.md          # Phase 0 — known-exception identification rationale + per-walker port shape decisions
├── data-model.md        # Phase 1 — WalkConfig, SafeWalkCallback, signature contracts
├── quickstart.md        # Phase 1 — "how to add a new ecosystem reader" walkthrough
├── contracts/
│   └── walk-api.md      # The single public API contract for safe_walk
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   ├── scan_fs/
│   │   ├── walk.rs                                # NEW — the shared helper
│   │   ├── walker.rs                              # UNCHANGED (documented known exception)
│   │   ├── mod.rs                                 # +pub(crate) mod walk;
│   │   └── package_db/
│   │       ├── project_roots.rs                   # PORTED — becomes a thin wrapper that calls safe_walk
│   │       ├── cargo.rs                           # PORTED
│   │       ├── maven.rs                           # PORTED
│   │       ├── gem.rs                             # PORTED
│   │       ├── go_binary.rs                       # PORTED
│   │       ├── golang/legacy.rs                   # PORTED
│   │       ├── rpm_file.rs                        # PORTED
│   │       ├── nuget/mod.rs                       # PORTED (replaces its custom walk_inner)
│   │       ├── yocto/recipe.rs                    # PORTED
│   │       ├── npm/walk.rs                        # UNCHANGED (documented known exception)
│   │       └── exclude_path.rs                    # UNCHANGED (consumed by safe_walk via WalkConfig)
│   ├── scan_fs/binary/
│   │   ├── discover.rs                            # PORTED
│   │   └── source_binding/cmake_observer.rs       # PORTED
docs/
└── design-notes.md                                # +"Filesystem walking" section
mikebom-common/                                     # UNCHANGED
mikebom-ebpf/                                       # UNCHANGED
```

**Structure Decision**: Single-project layout (every milestone since 001). The new `scan_fs/walk.rs` lives ONE LEVEL UP from `package_db/project_roots.rs` because the helper is used by both `package_db/*` ecosystem walkers AND the `binary/*` walkers — neither parent owns it. The `binary/source_binding/cmake_observer.rs` file imports it through `crate::scan_fs::walk::safe_walk`.

## Complexity Tracking

No constitution violations. No complexity to justify.
