# Quickstart — milestone 103 maintainer recipes

Five recipes for landing the Bazel + CMake reader implementations. Total estimated implementation time: ~3 hours single-developer.

## Recipe 1 — Replace bazel.rs stub with real implementation

Open `mikebom-cli/src/scan_fs/package_db/bazel.rs` (currently a stub from PR-A). Replace the body per `data-model.md §bazel.rs`. Roughly:

```rust
pub fn read(scan_root: &Path) -> Vec<PackageDbEntry> {
    let mut entries = Vec::new();
    if let Some(path) = first_existing(&[scan_root.join("MODULE.bazel")]) {
        entries.extend(parse_module_bazel(&path));
    }
    if let Some(path) = first_existing(&[
        scan_root.join("WORKSPACE.bazel"), scan_root.join("WORKSPACE"),
    ]) {
        entries.extend(parse_workspace_bazel(&path));
    }
    dedup_module_wins(entries)
}
```

3 helper fns: `parse_module_bazel`, `parse_workspace_bazel`, `build_bazel_entry`. Two regexes for MODULE (single multiline), two regexes per rule for WORKSPACE (envelope + inner-extractor pairs for `urls`/`url`/`sha256`/`remote`/`commit`/`tag`).

Add 5 unit tests at the bottom (`#[cfg(test)] mod tests`) covering: MODULE single dep, MODULE dev_dependency, WORKSPACE http_archive, WORKSPACE git_repository, malformed file skip-with-warn.

Verify with `cargo +stable test -p mikebom --bin mikebom scan_fs::package_db::bazel`.

## Recipe 2 — Replace cmake.rs stub with real implementation

Open `mikebom-cli/src/scan_fs/package_db/cmake.rs` (also currently a stub). Replace per `data-model.md §cmake.rs`:

```rust
pub fn read(scan_root: &Path, include_vendored: bool) -> Vec<PackageDbEntry> {
    let mut entries = Vec::new();
    for path in discover_cmake_files(scan_root) {
        let content = read_or_warn(&path);
        entries.extend(parse_fetch_block(&content, &path, "FetchContent_Declare"));
        entries.extend(parse_fetch_block(&content, &path, "ExternalProject_Add"));
        if include_vendored {
            entries.extend(parse_vendored(&content, &path, scan_root));
        }
    }
    entries
}
```

5 helper fns: `discover_cmake_files`, `parse_fetch_block` (single fn handles both FetchContent + ExternalProject), `parse_vendored`, `parse_version_from_url`, `build_cmake_entry`. The github-URL → `pkg:github/` detection is the trickiest part — handle via a separate regex that captures owner + repo from the GIT_REPOSITORY URL.

Add 6 unit tests at the bottom: FetchContent GIT, FetchContent URL, ExternalProject GIT, ExternalProject URL, find_package NOT parsed (regex anchors don't match), vendored opt-in gated.

Verify with `cargo +stable test -p mikebom --bin mikebom scan_fs::package_db::cmake`.

## Recipe 3 — Create fixtures + integration tests

Create `tests/fixtures/bazel/MODULE.bazel` + `WORKSPACE.bazel` per data-model.md §fixtures.

Create `tests/fixtures/cmake/CMakeLists.txt` + `cmake/third_party.cmake` + `third_party/foo/version.txt`.

Create 4 integration test files in `tests/`:
- `scan_bazel.rs` — 4 tests per Contracts 1+2+3.
- `scan_cmake.rs` — 4 tests per Contracts 4+5+6+7.
- `scan_cmake_vendored.rs` — 3 tests per Contract 8.
- `scan_cmake_findpackage_negative.rs` — 1 dedicated test per Contract 7.

All four follow the PR-A pattern using `env!("CARGO_BIN_EXE_mikebom")` + `Command::new(...)`. Use `--offline` to skip enrichment paths.

Verify with `cargo +stable test --test scan_bazel --test scan_cmake --test scan_cmake_vendored --test scan_cmake_findpackage_negative`. Expected: 4 + 4 + 3 + 1 = 12 tests pass.

## Recipe 4 — Add bazel + cmake to existing goldens regression suite

Open each of `tests/cdx_regression.rs` / `tests/spdx_regression.rs` / `tests/spdx3_regression.rs`. Add 2 new entries to the `CASES` array:

```rust
("bazel", "tests/fixtures/bazel"),
("cmake", "tests/fixtures/cmake"),
```

Add 2 new `#[test] fn` per file (e.g., `cdx_regression_bazel`, `cdx_regression_cmake`). Match the existing per-ecosystem pattern.

Regenerate the 6 new goldens:

```bash
MIKEBOM_UPDATE_CDX_GOLDENS=1 MIKEBOM_UPDATE_SPDX_GOLDENS=1 MIKEBOM_UPDATE_SPDX3_GOLDENS=1 \
  cargo +stable test -p mikebom \
    --test cdx_regression --test spdx_regression --test spdx3_regression
```

Verify the 11 existing ecosystems' goldens stay byte-identical:

```bash
git diff --stat mikebom-cli/tests/fixtures/golden/cyclonedx/{apk,cargo,conan,deb,gem,golang,maven,npm,pip,rpm,vcpkg}.cdx.json | tail -1
# Expected: empty.
```

Then re-run without env vars to confirm:

```bash
cargo +stable test -p mikebom --test cdx_regression --test spdx_regression --test spdx3_regression \
  2>&1 | grep "test result:"
# Expected: ok. 13 passed × 3 (11 + 2 new ecosystems per format).
```

## Recipe 5 — Update cli-reference docs + open PR

`docs/user-guide/cli-reference.md`: add a `--include-vendored` section per data-model.md §docs (FR-014).

Pre-PR gate + diff scope audit:

```bash
MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1 ./scripts/pre-pr.sh
# Expected: `>>> all pre-PR checks passed.`

git diff --name-only main | grep -E '^Cargo\.(lock|toml)$|/Cargo\.(lock|toml)$' | wc -l
# Expected: 0

git diff --stat mikebom-cli/tests/fixtures/golden/ | tail -1
# Should show ONLY the 6 new goldens added; existing 33 (11 ecosystems × 3 formats) unchanged.
```

Open PR. Title: `feat(103): Bazel + CMake source-tree readers (milestone 102 PR-B)`. Body references the 102 spec for design + PR-A for foundational architecture; calls out the 6 new goldens + 12 new tests + the new docs.

## When in doubt

- **Regex doesn't match a real-world CMakeLists.txt** — the parser is heuristic by design (SC-002 caps at 90%). Document the case in the PR description; don't expand the regex if it would cause false positives.
- **`http_archive` with `urls = ["mirror1", "mirror2"]`** — take the first URL only; Bazel allows mirror lists but for SBOM purposes one canonical URL is sufficient.
- **`FetchContent_Declare` arguments with comments inline (`# ...`)** — the regex `\S+` for keyword values will consume the comment fragment. Acceptable for the heuristic ceiling; documented in SC-002 floor.
- **`find_package` AND `FetchContent_Declare` both present for the same dep** — only the FetchContent component emits (find_package skipped per FR-007). The find_package + FetchContent same-name pattern is common in CMake projects that maintain dual code paths.
- **`add_subdirectory(third_party/foo/sub)`** — only top-level vendored deps (`third_party/<name>` with `<name>` being a single path segment) emit. Nested vendored is out of scope.
- **CMake `option(USE_EXTERNAL_GTEST OFF)` controlling whether FetchContent fires** — the regex matches the `FetchContent_Declare` regardless. If a user wants conditional handling, they can use `--exclude-scope=test` or similar; this milestone doesn't introduce conditional-flow detection.
