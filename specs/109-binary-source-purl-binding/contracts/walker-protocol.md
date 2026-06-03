# Contract: build-directory walker protocol

## Discovery algorithm

Inputs: `scan_root: &Path` (the operator's `--path` argument).

```text
1. Walk scan_root with std::fs::read_dir + bounded recursion (max
   depth 6 from scan_root per R1).
2. At each directory, test both conditions:
   a. <dir>/CMakeCache.txt exists as a regular file
   b. <dir>/_deps/ exists as a directory AND read_dir returns ≥1
      entry
3. If BOTH conditions hold: record <dir> as a cmake-project build
   root; do NOT descend into _deps/ (the per-dep sub-build dirs are
   what we'll stat lookups against).
4. Continue walking siblings + descendants of <dir> EXCLUDING
   _deps/. Multi-project workspaces (e.g., subprojects/A/build/ +
   subprojects/B/build/) yield multiple cmake-project build roots.
5. Stop descending altogether at depth 6.
```

## Per-declaration observation

Inputs:
- A list of `PackageDbEntry` instances tagged `mikebom:source-mechanism = cmake-fetchcontent-{git,url}` from the existing milestone-102/103 cmake reader.
- The set of cmake-project build roots from the discovery step.

```text
For each (cmake declaration, cmake project build root) pair:
  1. Compute candidate path: <build_root>/_deps/<name>-build/
     where <name> is the cmake declaration's name parameter VERBATIM
     (not lowercased; cmake preserves the operator's casing in
     directory names).
  2. Check candidate path exists as a directory.
  3. If yes: emit CmakeBuildDirObservation with:
       library_name = declaration's name
       source_tier_purl = declaration's PackageDbEntry.purl
       source_mechanism = declaration's mikebom:source-mechanism value
       build_artifact_dir = candidate path
       cmake_project_build_root = build_root
  4. If no: emit NOTHING (declared-but-unbuilt case; falls back to
     milestone-108 generic-PURL behavior at the matcher level).
```

## Discovery determinism

Discovery output MUST be byte-identical across runs for the same input filesystem state:

1. `walkdir`-style iteration order is platform-dependent; the observer MUST sort entries before recording observations (lexical sort by `cmake_project_build_root`'s string form, then by `library_name`).
2. Symlinks ARE followed (`std::fs::canonicalize` resolves them). Cycle detection inherited from milestone 054's `fix-walker-symlink-hang` work.
3. Permission errors on a subdirectory of `_deps/` emit `tracing::warn!` and skip that entry; the rest of the walk proceeds (FR-011 fail-closed-on-warn posture).

## Out-of-scope path patterns (this milestone)

- `<build>/<name>-prefix/...` — `ExternalProject_Add` default layout. Per Phase-2 clarification, deferred.
- `bazel-out/<config>/...` — Bazel layout. Future milestone.
- `subprojects/<name>/build/` — Meson layout. Future milestone.
- Hand-written Makefile layouts (no convention).
- vcpkg's `vcpkg_installed/<triplet>/lib/` — vcpkg components are already classified by their own milestone-102/103 reader; this milestone is FetchContent-specific.

## Bounds + budgets

- Total walk wall-clock: ≤100ms at pathological scale (10000 `_deps/` entries; unrealistic). Typical: ≤10ms.
- Memory: O(N) where N = number of cmake declarations × number of cmake projects in the scan root. Each `CmakeBuildDirObservation` is ~5 small strings + 2 `PathBuf`s — a few hundred bytes. 100 deps × 2 projects = 200 records ≈ 50KB. Negligible.
- Syscalls: 1 `read_dir` per cmake-project build root + 1 `stat` per cmake declaration. At 100 deps × 2 projects = 202 syscalls. Microseconds on SSD.

## Failure handling

| Failure | Behavior |
|---|---|
| `scan_root` doesn't exist | mikebom aborts upstream of this milestone; not our concern. |
| `scan_root` exists but contains no cmake build dirs | Registry is empty; matcher falls back to milestone-108 generic-PURL behavior. No warn (this is the common no-cmake-project case). |
| `CMakeCache.txt` exists but `_deps/` does not | Skip; not a valid cmake-with-FetchContent project for our purposes. No warn (the operator may have used find_package only). |
| `_deps/` exists but is empty | Skip; no FetchContent deps declared. No warn. |
| `_deps/<name>-build/` exists but is not a directory (operator created a regular file with that name — pathological) | Skip; tracing::warn!. |
| Permission denied on `_deps/` traversal | tracing::warn!; skip this cmake project. Other projects' walks continue. |
| Symlink cycle inside `_deps/` (cmake doesn't create these, but a user mod might) | std::fs::canonicalize detects + skips. |

## Forward-compat hook

The walker logic is encapsulated in `source_binding::cmake_observer::observe(scan_root, cmake_declarations) -> Vec<CmakeBuildDirObservation>`. A future Bazel observer at `source_binding::bazel_observer::observe(...)` returns the SAME `Vec<CmakeBuildDirObservation>` type with `source_mechanism` values like `bazel-http-archive` and `build_artifact_dir` pointing into `bazel-out/<config>/bin/external/<repo>/`. The registry's lookup logic is observer-agnostic.
