# Quickstart — How to add a new ecosystem reader after this migration

**Feature**: 114-safe-walk-migration
**Audience**: mikebom contributors adding a new ecosystem source (a hypothetical Swift Package Manager reader, a freshly-discovered C/C++ build tool, an Erlang `rebar.lock` walker, …) — anyone who would have copy-pasted `cargo.rs::walk_for_cargo_manifests` pre-migration.

## The TL;DR

Two imports, three lines of `WalkConfig` setup, one closure that holds the per-ecosystem logic. The shared helper handles the canonicalize-keyed visited-set, the depth bound, the directory-exclusion check, and the skip-debug logging for you. You can't accidentally drop a loop-protection invariant — there's no recursion to write.

## Five-minute walkthrough

You're adding a Swift Package Manager reader (`scan_fs/package_db/swift_pm.rs`). You need to find every `Package.swift` file under the scan rootfs.

### Step 1 — Import the helper

```text
use super::super::walk::{safe_walk, WalkConfig};
use super::super::package_db::project_roots::should_skip_default_descent;
```

### Step 2 — Write your reader's entry point

```text
pub fn read(
    rootfs: &Path,
    include_dev: bool,
    exclude_set: &super::exclude_path::ExclusionSet,
) -> Vec<PackageDbEntry> {
    let mut out = Vec::new();
    let cfg = WalkConfig {
        max_depth: 6,                          // monorepo + image layouts
        should_skip: &|p, _| {
            // Mirror the existing convention: skip built-in noise dirs.
            p.file_name()
                .and_then(|s| s.to_str())
                .map(should_skip_default_descent)
                .unwrap_or(true)
        },
        exclude_set,
    };
    safe_walk(rootfs, &cfg, |path| {
        if path.file_name() == Some(OsStr::new("Package.swift")) {
            // Parse the manifest, build the per-component entries.
            if let Some(entry) = parse_swift_package(path) {
                out.push(entry);
            }
        }
    });
    out
}
```

That's it. No `read_dir` recursion to write; no visited-set to manage; no symlink-loop hazard; no need to remember the milestone-054 hang fix.

### Step 3 — Add it to the dispatcher

In `scan_fs/package_db/mod.rs::read_all`:

```text
out.extend(swift_pm::read(rootfs, include_dev, exclude_set));
```

### Step 4 — Verify the audit grep is clean

```bash
$ grep -rn '^fn walk' mikebom-cli/src/scan_fs/
mikebom-cli/src/scan_fs/walk.rs:N:pub(crate) fn safe_walk<F: FnMut(&Path)>(
mikebom-cli/src/scan_fs/walker.rs:N:fn walk(...)
mikebom-cli/src/scan_fs/package_db/npm/walk.rs:N:fn walk_node_modules(...)
```

Three matches: the helper itself + the two documented known exceptions. If your new reader shows up here, you wrote your own `fn walk_*` and should refactor to delegate to `safe_walk`.

## When to choose what

### Choosing `max_depth`

- **6** — typical: monorepo / single-project / image-layout. Enough for `/usr/src/app/services/api/` (4 levels) plus 2 levels of safety margin. Every existing ecosystem walker uses 6 unless it has a documented reason otherwise.
- **8** — when scanning install trees that need extra depth (`maven.rs`'s install-state walker uses this).
- **10** — when scanning system-wide gem installation trees (`gem.rs::walk_for_gemspecs` uses this for `/usr/lib/ruby/gems/<ver>/specifications/`-style paths).

If you're not sure, pick 6.

### Choosing `should_skip`

- **Standard skip-set** — use `should_skip_default_descent(name)`. Filters `node_modules/`, `vendor/`, `target/`, `dist/`, `build/`, `out/`, `coverage/`, `__pycache__/`, `venv/`, `.`-prefixed dirs. Covers 95% of cases.
- **Standard + ecosystem-specific** — compose:
  ```text
  should_skip: &|p, _| {
      let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
      should_skip_default_descent(name) || name == "swift-build"
  },
  ```
- **Path-relative-to-rootfs** — pass both args through to a helper that computes the relative path:
  ```text
  should_skip: &|p, root| {
      let rel = p.strip_prefix(root).unwrap_or(p);
      rel.starts_with("third_party/vendored")
  },
  ```

### Choosing the visit callback

- **File extension** — `if path.extension() == Some(OsStr::new("swift")) { ... }`.
- **File name** — `if path.file_name() == Some(OsStr::new("Package.swift")) { ... }`.
- **Directory marker** — `if path.is_dir() && path.join("Package.swift").is_file() { roots.push(path.to_path_buf()); }`.
- **Multiple file types** — match on `extension()` / `file_name()` inside the callback; the helper invokes you once per visited path.
- **Emit-inside-walk** — build your `Vec<PackageDbEntry>` directly inside the callback (like `yocto/recipe.rs` does post-migration).

## When NOT to use `safe_walk` — known exceptions

The helper is designed for the project-root-discovery shape. Two existing walkers don't fit and stay hand-rolled:

| File | Why exempt |
|---|---|
| `scan_fs/walker.rs` | Whole-filesystem file enumeration with a size cap + content-hashing inside the walk. No skip list, no depth bound, no project-root discrimination. The helper's `WalkConfig` doesn't fit, and adding a `size_cap` / `compute_hash` knob would bloat the helper's surface. |
| `scan_fs/package_db/npm/walk.rs` | npm `@scope`-aware walker. Only recurses one level into `@scope`-named directories AND propagates `in_npm_internals: bool` per-descent state through the recursive calls. The generic `should_skip` predicate can't express either of those semantics. |

If you're writing a walker whose semantics genuinely don't fit `safe_walk`, FIRST: try harder to fit (~95% of cases do). If you've genuinely confirmed your walker can't, add it to the comment block at the top of `scan_fs/walk.rs` enumerating the known exceptions, with a one-sentence reason. Reviewers should push back hard on growing this list — a third exception is plausible, a tenth is the abstraction failing.

## Common mistakes to avoid

| Mistake | Fix |
|---|---|
| Writing `fn walk_*` outside `walk.rs` because you "just need a quick recursion" | Use `safe_walk` with a one-line visit callback. The recursion is already written for you. |
| Setting `max_depth: usize::MAX` because you "don't want to miss anything" | Don't. The depth bound is defense-in-depth; you do want it. Pick 6 / 8 / 10 per the guidance above. |
| Putting filesystem I/O inside `should_skip` | `should_skip` is called for every child directory; per-call I/O is a performance hazard. Cache outside or rethink the predicate. |
| Forgetting the audit grep when reviewing a PR | One-liner: `grep -rn '^fn walk' mikebom-cli/src/scan_fs/`. Three acceptable matches. Reject PRs that add a fourth. |
