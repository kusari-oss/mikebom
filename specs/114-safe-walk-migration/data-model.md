# Phase 1 Data Model — Shared `safe_walk` Helper Migration

**Feature**: 114-safe-walk-migration
**Date**: 2026-06-12

In-process types only. The helper is stateless beyond a per-call visited-set; no on-disk state, no caches.

---

## `WalkConfig<'a>`

Per-call configuration carried by reference into `safe_walk`. Three fields, all narrowly-typed; no future-extensibility via builder/marker tricks until a third walker needs a new field.

```text
pub(crate) struct WalkConfig<'a> {
    /// Maximum descent depth. The helper stops descending below this
    /// level even if directories exist underneath. The depth bound is
    /// a defense-in-depth backstop for the canonicalize-keyed visited-
    /// set: if canonicalization is unavailable or unreliable (e.g.,
    /// filesystem with bind-mounts that defeat resolution), the depth
    /// limit guarantees bounded recursion.
    ///
    /// Per the milestone-054 audit: every existing walker picks 6 or
    /// 8 or 10. The helper does not impose a default; callers MUST
    /// supply a value appropriate for their use case.
    pub max_depth: usize,

    /// Predicate consulted before descent into each child directory.
    /// `(candidate, rootfs)` lets the caller extract the candidate's
    /// filename via `.file_name()` AND compute the candidate's path
    /// relative to the scan root (today required by the milestone-113
    /// directory-exclusion mechanism, tomorrow potentially by other
    /// path-relative skip rules). Returning `true` suppresses descent.
    pub should_skip: &'a dyn Fn(&Path, &Path) -> bool,

    /// Milestone-113 user-supplied directory exclusion. Consulted by
    /// the helper AFTER `should_skip`, as a separate fast-path step
    /// (the helper short-circuits when `exclude_set.is_empty()`
    /// rather than invoking the closure). Centralizing the exclusion
    /// check inside the helper rather than inside every per-walker
    /// `should_skip` closure means EVERY walker logs skip events
    /// uniformly via `tracing::debug!` post-migration; pre-migration
    /// only `project_roots.rs` did.
    pub exclude_set: &'a super::package_db::exclude_path::ExclusionSet,
}
```

**Invariants**:
- `max_depth` is supplied by the caller; the helper does not impose a default. Documented in the API contract.
- `should_skip` and `exclude_set` are both consulted at every descent decision; the order is fixed (`should_skip` first, then `exclude_set`). Documented in the API contract.

---

## `safe_walk`

The single public entry point. Generic over the visit callback so closures with captured `&mut` state work without boxing.

```text
pub(crate) fn safe_walk<F: FnMut(&Path)>(
    rootfs: &Path,
    cfg: &WalkConfig,
    visit: F,
);
```

**Operations**:

| Operation | When fired | Argument |
|---|---|---|
| Visited-set guard | At entry to each directory | `canonicalize(dir).unwrap_or(dir.to_path_buf())` |
| `visit(dir)` | After visited-set insertion, before descent | The current directory's `&Path` |
| Depth-bound check | Before iterating children | `depth >= cfg.max_depth → return` |
| `read_dir` | Before iterating children | Result tolerated; unreadable dirs silently skipped |
| Per-child filter | For each `read_dir` entry | Files yielded via `visit(path)`; non-dirs continue without recursion; dirs proceed to skip-predicate check |
| `should_skip(candidate, rootfs)` | For each directory child | Returning true → child skipped |
| `exclude_set.matches(candidate-rel-to-rootfs)` | For each directory child (skipped via short-circuit when `is_empty()`) | Returning true → child skipped, `tracing::debug!` emitted |
| Recursive descent | If child passes both filters | `safe_walk(child, cfg, visit)` |

**Invariants**:
- `visit` is invoked exactly once per visited path. Symlink loops produce zero duplicate `visit` calls (visited-set guard fires before `visit`).
- Order of `visit` invocations matches the existing `project_roots.rs` order: parent before children, `read_dir` ordering within each directory (which is platform-dependent — caller MUST sort if order matters).
- The helper never panics. All I/O errors are silently swallowed; the spec covers this in FR-003.

---

## Threading through the migration

Each ported walker's hand-rolled recursion is replaced by a single `safe_walk` call. The shape across all 13 ports is uniform:

```text
// Pre-migration
fn walk_for_cargo_manifests(dir, rootfs, depth, visited, out, exclude_set) {
    /* canonicalize + insert visited + depth-bound + read_dir + skip checks */
    for child in dir.read_dir() {
        if !child.is_dir() { continue; }
        if should_skip_descent(name) { continue; }
        if exclude_set.matches(rel) { continue; }
        walk_for_cargo_manifests(&child, ...);
    }
}

// Post-migration
fn find_cargo_manifests(rootfs, exclude_set) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let cfg = WalkConfig {
        max_depth: MAX_PROJECT_ROOT_DEPTH,
        should_skip: &|c, _| should_skip_descent(c.file_name()...),
        exclude_set,
    };
    safe_walk(rootfs, &cfg, |p| {
        if p.file_name() == Some(OsStr::new("Cargo.toml")) {
            out.push(p.to_path_buf());
        }
    });
    out
}
```

Each ported walker loses ~30 lines (the recursive descent loop) and gains ~10 lines (the `WalkConfig` construction + the callback). Net ~20 lines saved per walker; ~260 lines saved across the 13-walker migration. **More importantly**: the loop-protection invariants live in exactly one place, not 13.

---

## Migration mapping

| Pre-migration symbol | Post-migration replacement |
|---|---|
| `package_db/project_roots.rs::walk_for_project_roots` | Thin wrapper around `safe_walk` (keeps `WalkConfig` re-export for back-compat with existing pip/npm/gradle/nuget/yocto closures, then deletes after PR 5). |
| `package_db/project_roots.rs::walk_inner` | DELETED (its logic IS the helper). |
| `package_db/project_roots.rs::WalkConfig` | DELETED (replaced by the new `scan_fs::walk::WalkConfig`). |
| `package_db/cargo.rs::walk_for_cargo_manifests` | Inlined into `find_cargo_manifests` as a `safe_walk` call. |
| `package_db/cargo.rs::walk_for_cargo_lockfiles` | Inlined into `find_cargo_lockfiles`. |
| `package_db/gem.rs::walk_for_*` (3 of them) | Inlined into the three `find_*` callers. |
| `package_db/maven.rs::walk_for_maven` | Inlined into `find_maven_artifacts`. |
| `package_db/maven.rs::walk_for_top_level_poms` | Inlined into `find_top_level_poms`. |
| `package_db/golang/legacy.rs::walk_for_go_roots` | Inlined into `candidate_project_roots`. |
| `package_db/go_binary.rs::walk_for_binaries` | Inlined into `read`. |
| `package_db/rpm_file.rs::walk_dir` | Inlined into `read`. |
| `package_db/nuget/mod.rs::walk_inner` | Inlined into `walk_project_files`. |
| `package_db/yocto/recipe.rs::walk` | Inlined into `read`. |
| `binary/discover.rs::walk_dir` | Inlined into caller. |
| `binary/source_binding/cmake_observer.rs::walk_for_cmake_build_dirs` | Inlined into caller. |
| `scan_fs/walker.rs::walk` / `walk_and_hash` | **UNCHANGED** — documented known exception. |
| `package_db/npm/walk.rs::walk_node_modules` | **UNCHANGED** — documented known exception. |
