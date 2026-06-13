# Contract — `scan_fs::walk::safe_walk` API

**Feature**: 114-safe-walk-migration

The single public API every ecosystem-reader filesystem walker uses post-migration. Documented here so reviewers can enforce the contract against future PRs.

## Public surface

```text
// Module: mikebom-cli/src/scan_fs/walk.rs
// Visibility: pub(crate) — binary-internal per Constitution Principle VI.

pub(crate) struct WalkConfig<'a> {
    pub max_depth: usize,
    pub should_skip: &'a dyn Fn(&Path, &Path) -> bool,
    pub exclude_set: &'a super::package_db::exclude_path::ExclusionSet,
}

pub(crate) fn safe_walk<F: FnMut(&Path)>(
    rootfs: &Path,
    cfg: &WalkConfig,
    visit: F,
);
```

## Semantic guarantees

| Guarantee | Mechanism |
|---|---|
| **Symlink-loop bounded termination** | Canonicalize-keyed `HashSet<PathBuf>` visited-set. `safe_walk` inserts the canonical path before any `visit` call. A symlink loop's second arrival re-inserts the same canonical key and returns early. |
| **Depth-bounded recursion** | Defense-in-depth backstop. Even if canonicalization is unavailable (sandboxed filesystem, missing permissions on `realpath`), the `cfg.max_depth` ceiling guarantees terminations. Callers MUST supply a value — no default. |
| **Tolerant of unreadable directories** | `read_dir().is_err() → return`. Helper continues processing peer directories at the parent level. Operator may see fewer paths than expected on a partially-restricted filesystem; the helper does NOT surface the I/O error. |
| **Tolerant of non-UTF-8 path components** | The visit callback receives `&Path` so the caller decides how to extract names. The helper's internal `cfg.should_skip(candidate, rootfs)` accepts `&Path` arguments so non-UTF-8 names round-trip correctly. The exclusion check converts the relative path via `to_string_lossy()` — same lossy behavior milestone 113 ships today. |
| **Order-stable per directory** | Within a single directory, `read_dir`'s yield order applies. Cross-platform that order is filesystem-dependent (macOS vs Linux vs Windows). Callers that need a stable order must sort the collected output. |
| **Single-`visit`-per-path** | The visited-set guard fires before `visit`. A symlink pointing at an already-visited canonical path produces zero duplicate `visit` calls. |

## Caller contract

1. Caller supplies `rootfs` as a `&Path`. The helper treats this as the descent root AND as the "relative-to" anchor for the `should_skip` predicate and the `ExclusionSet` check.
2. Caller's `cfg.should_skip` MUST be a pure function of `(candidate, rootfs)` — no mutable state, no external I/O. The helper invokes it once per child directory.
3. Caller's `visit` callback receives the visited path's `&Path` and may have any `FnMut` shape — closures with captured `&mut` state are supported.
4. The helper does NOT clone the rootfs, the visited paths, or the candidate paths. Caller is responsible for any cloning needed in its callback.
5. The helper does NOT cap `max_depth` to any specific upper bound — callers SHOULD pick a number ≤ ~20 for any reasonable use case (the existing per-walker constants are 6, 8, 10).

## Out of scope

- **Follow-symlinks toggle**: not exposed. The helper uses `canonicalize` which dereferences symlinks for visited-set keys but the descent itself walks the as-named paths. This matches every existing walker's pre-migration behavior.
- **Async / iterator return shape**: the helper is sync + callback-based. Iterator-based alternatives (`impl Iterator<Item=PathBuf>`) were considered and rejected because every existing walker emits-inside-the-walk (multi-type yields, mutable-ref captures) and would require fighting borrowck to express the same patterns through an iterator.
- **Parallelism**: the helper is single-threaded. Parallel discovery is out of scope; the per-directory cost is dominated by `read_dir` syscall latency, not user-space work, so parallelizing would buy little for the typical use case.
- **Result-typed return**: the helper does not return `Result<()>`. Per FR-003, I/O errors are silently swallowed (matches every existing walker's pre-migration behavior). A Result-typed variant could be added later if a new use case needs surfaced errors.

## Audit pattern

`grep -rEn 'fn walk[_(]' mikebom-cli/src/scan_fs/`. The pattern catches every `fn walk_*` and `fn walk(` declaration regardless of visibility prefix (`pub`, `pub(crate)`) or indentation. Paired with the explicit known-non-walker list documented in `scan_fs/walk.rs`'s comment block.

**Acceptable matches — filesystem ecosystem-discovery walkers**:

| File | Acceptable function names |
|---|---|
| `mikebom-cli/src/scan_fs/walk.rs` | `pub(crate) fn safe_walk` + any internal helpers it factors out |
| `mikebom-cli/src/scan_fs/walker.rs` | `pub fn walk_and_hash`, `fn walk` (documented known exception: whole-FS deep-hash) |
| `mikebom-cli/src/scan_fs/package_db/npm/walk.rs` | `fn walk_node_modules` (documented known exception: @scope-aware) |

**Acceptable matches — non-filesystem-walker false positives** (functions whose name matches the pattern but are NOT filesystem walkers; documented in the helper module's comment block):

| File | Function | Reason it matches but isn't a walker |
|---|---|---|
| `mikebom-cli/src/scan_fs/package_db/maven.rs` | `fn walk_m2_jars` | Iterates a precomputed `Vec<PathBuf>` from `MavenRepoCache::discover`; no `read_dir` recursion. |
| `mikebom-cli/src/scan_fs/package_db/maven.rs` | `pub(crate) fn walk_jar_maven_meta` | Walks JAR archive internal content, not the filesystem. |
| `mikebom-cli/src/scan_fs/package_db/rpmdb_sqlite/schema.rs` | `fn walk_schema_page` | SQLite B-tree page walker, not filesystem. |
| Various test modules | `fn walks_*`, `fn walk_jar_*`, `fn walk_fat_jar_*`, `fn walk_rootfs_poms_*` | Tests OF walkers (typically indented inside `#[cfg(test)] mod tests`), not walkers themselves. |

Matches outside the union of the two tables above are a regression: a contributor introduced a new hand-rolled walker bypassing the shared helper. Reviewer action: reject the PR or push back to either migrate the new walker to `safe_walk` OR document a new entry in `scan_fs/walk.rs`'s comment block.

## Versioning

`pub(crate)` API; not user-visible; no SemVer obligation. Internal changes ship in normal `[Unreleased]` `### Changed` notes.
