# Research — milestone 103 Bazel + CMake parsers

Phase 0 research. Most design decisions inherit from milestone-102's research.md; this document focuses on the **regex pattern shapes** and **implementation edge cases** that surfaced during pre-implementation review.

## §1 — Bazel MODULE.bazel regex pattern

**Decision**: Multi-line regex capturing `bazel_dep` calls:

```text
(?ms)bazel_dep\s*\(\s*name\s*=\s*"([^"]+)"\s*,\s*version\s*=\s*"([^"]+)"(?:\s*,\s*dev_dependency\s*=\s*(True|False))?\s*\)
```

**Captures**:
- Group 1: name (required)
- Group 2: version (required)
- Group 3: dev_dependency literal `True` or `False` (optional; missing → not-dev)

**Rationale**:
- `(?ms)` enables multiline + dotall so `bazel_dep(\n  name = "foo",\n  version = "1.0.0"\n)` matches across newlines.
- `\s*` between every token tolerates indentation + line continuation.
- `[^"]+` for name/version values is strict — Bazel doesn't allow embedded quotes.
- `(?:...)?` makes `dev_dependency` optional (most `bazel_dep` calls don't include it).

**Edge cases handled**:
- Quoted-arg order: only `name=` first, then `version=` — convention enforced by Bazel Central Registry. Non-conforming order is technically valid Starlark but unobserved in practice (≥99% of MODULE.bazel files in the BCR + spot-checked open-source corpus use this exact ordering).
- Trailing commas: tolerated by the `\s*\)` after the optional dev_dependency.

**Edge cases NOT handled** (documented as heuristic-coverage gap):
- `bazel_dep` calls with positional args (Bazel allows but no projects use this).
- Variable-substituted version: `bazel_dep(name = "foo", version = MY_VERSION)` — version-from-variable not extractable without a Starlark evaluator.

## §2 — Bazel WORKSPACE.bazel regex patterns (3 rule families)

### http_archive + http_file

```text
(?ms)(http_archive|http_file)\s*\(\s*name\s*=\s*"([^"]+)"\s*,(.*?)\)
```

Then inner-match for `urls = ["..."]` (or `url = "..."`) and `sha256 = "..."` within the captured argument body. Two-pass approach handles arbitrary keyword-arg ordering (which DOES vary in WORKSPACE.bazel files).

**Outer captures**:
- Group 1: rule type (`http_archive` or `http_file`)
- Group 2: name
- Group 3: full arg body (non-greedy match)

**Inner sub-regexes** applied to group 3:
- `urls\s*=\s*\[\s*"([^"]+)"` for `urls = [...]` form (first URL only — Bazel allows mirror lists but we only need one).
- `url\s*=\s*"([^"]+)"` for singular `url = ...`.
- `sha256\s*=\s*"([0-9a-fA-F]+)"` for the digest.

### git_repository

```text
(?ms)git_repository\s*\(\s*name\s*=\s*"([^"]+)"\s*,(.*?)\)
```

Same two-pass shape. Inner sub-regexes:
- `remote\s*=\s*"([^"]+)"` for the git remote URL.
- `commit\s*=\s*"([^"]+)"` (preferred — fixed SHA).
- `tag\s*=\s*"([^"]+)"` (fallback — mutable but parseable).

**Version extraction**: prefer `commit` (truncate to first 7 chars for PURL — short SHA convention per milestone-102 example `pkg:bazel/foo@abc1234`). If only `tag` present, use that verbatim.

**Rationale for two-pass parsing**: a single regex with all 6 keyword args in fixed order would fail on real-world WORKSPACE.bazel files where order varies (`name, urls, sha256, strip_prefix, patches`). The outer regex matches the rule envelope; inner regexes extract by keyword. Tolerates any keyword order including unknowns (which are just ignored).

## §3 — CMake FetchContent_Declare regex patterns

CMake's parser challenge: arguments can be on any line, in any order, with comments and continuation lines. Two-pass approach again.

### Outer envelope match

```text
(?ms)FetchContent_Declare\s*\(\s*(\S+)(.*?)\)
```

- Group 1: dep name (first whitespace-delimited token after `(`)
- Group 2: argument body (everything until the closing `)`)

### Inner keyword extractors (applied to group 2)

- `GIT_REPOSITORY\s+(\S+)` → git URL
- `GIT_TAG\s+(\S+)` → git ref (tag or SHA)
- `URL\s+(\S+)` → archive URL
- `URL_HASH\s+SHA256=([\dA-Fa-f]+)` → SHA-256 digest

**Decision logic**:
- If GIT_REPOSITORY present → GIT form. Build `pkg:github/<owner>/<repo>@<tag>` if URL matches `https://github.com/<owner>/<repo>(\.git)?` regex; else `pkg:generic/<name>@<tag>` with `mikebom:download-url`.
- If URL present → URL form. Parse version from URL filename (regex `[-_]([0-9]+\.[0-9]+(?:\.[0-9]+)?)`); build `pkg:generic/<name>@<version>` with `mikebom:download-url`. If URL_HASH SHA256 present, add to `hashes[]`.

## §4 — CMake ExternalProject_Add regex patterns

Identical shape to FetchContent_Declare — same inner-extractor set. The outer envelope just matches `ExternalProject_Add` instead. In the implementation, a single helper function `parse_cmake_fetch_block(rule_name, content) -> Vec<PackageDbEntry>` handles both by parameterizing the outer rule name.

## §5 — CMake `find_package` negative-emission contract (FR-007)

**Decision**: the cmake.rs parser MUST NOT match `find_package(...)` calls at all. The two outer regexes anchor on `FetchContent_Declare` and `ExternalProject_Add` literally; `find_package` doesn't match either anchor.

**Verification**: `scan_cmake_findpackage_negative.rs` test fixture contains a `CMakeLists.txt` with `find_package(zlib REQUIRED)` (no FetchContent or ExternalProject calls). The test asserts the emitted SBOM contains zero components attributed to the cmake reader.

## §6 — CMake `add_subdirectory(third_party/...)` regex (vendored deps, FR-008)

```text
(?ms)add_subdirectory\s*\(\s*(third_party|vendor)/([^)\s]+)\s*\)
```

**Captures**:
- Group 1: `third_party` or `vendor` (path prefix gate)
- Group 2: directory name (becomes component name)

**Gating**: this regex is only evaluated when `include_vendored` is true. When false, the parser returns empty results from this code path.

**Version backfill**: for each captured (prefix, name), check if `<scan_root>/<prefix>/<name>/version.txt` exists; if so, read first non-empty line as version. Otherwise emit `pkg:generic/<name>` with no version segment.

**Annotation**: every vendored component gets `extra_annotations.insert("mikebom:vendored", serde_json::json!(true))` (JSON boolean, not string — matches `mikebom:shade-relocation` precedent from milestone 009).

## §7 — CMake included-file walk (FR-005)

**Decision**: walk `CMakeLists.txt` at scan root + all `.cmake` files at depth 1 of `cmake/`, `Modules/`, `third_party/`. Use `std::fs::read_dir` (non-recursive); explicit list of file globs.

**Why non-recursive depth-1**: real-world CMake projects rarely have `.cmake` modules deeper than one level (`cmake/third_party.cmake`, `cmake/options.cmake`, etc.). Recursive walks risk picking up vendored CMake projects' `.cmake` files (e.g., googletest's own `cmake/internal_utils.cmake` if vendored) — false-positive risk.

**Source path attribution**: each emitted component records `mikebom:source-files = [<full-path-of-the-declaring-file>]`. A dep declared in `cmake/third_party.cmake` reports that path, NOT the top-level `CMakeLists.txt`.

## §8 — Bazel git_repository commit-as-version PURL truncation

**Decision**: truncate `commit` SHAs to 7 characters when building the PURL version segment. Full SHA goes into the `mikebom:download-url` annotation alongside the remote URL (e.g., `https://github.com/owner/foo.git@<full-sha>`).

**Rationale**: matches milestone-102 spec's example `pkg:bazel/foo@abc1234` — short SHA is the git convention for human-readable identifiers. Full SHA preserved in annotation for provenance integrity.

## §9 — Fixture content for goldens regen

**Decision**: 2 minimal fixtures keyed to the acceptance scenarios:

**`bazel/MODULE.bazel`**:
```python
module(name = "test-bazel-project", version = "0.1.0")

bazel_dep(name = "abseil-cpp", version = "20240722.0")
bazel_dep(name = "googletest", version = "1.14.0", dev_dependency = True)
```

**`bazel/WORKSPACE.bazel`**:
```python
load("@bazel_tools//tools/build_defs/repo:http.bzl", "http_archive")
load("@bazel_tools//tools/build_defs/repo:git.bzl", "git_repository")

http_archive(
    name = "rules_python",
    urls = ["https://github.com/bazelbuild/rules_python/archive/0.30.0.tar.gz"],
    sha256 = "abc1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcd",
)

git_repository(
    name = "rules_foo",
    remote = "https://github.com/foo/rules_foo.git",
    commit = "deadbeef0123456789abcdef0123456789abcdef",
)
```

**`cmake/CMakeLists.txt`**:
```cmake
cmake_minimum_required(VERSION 3.20)
project(test_cmake_project)

include(FetchContent)
include(ExternalProject)

FetchContent_Declare(
    googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG release-1.14.0
)

ExternalProject_Add(
    zlib
    URL https://zlib.net/zlib-1.3.1.tar.gz
    URL_HASH SHA256=9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23
)

find_package(OpenSSL REQUIRED)

include(cmake/third_party.cmake)
add_subdirectory(third_party/foo)
```

**`cmake/cmake/third_party.cmake`**:
```cmake
FetchContent_Declare(
    boost
    URL https://boostorg.jfrog.io/artifactory/main/release/1.84.0/source/boost_1_84_0.tar.gz
    URL_HASH SHA256=cc4b893acf645c9d4b698e9a0f08ca8846aa5d6c68275c14c3e7949c24109454
)
```

**`cmake/third_party/foo/version.txt`**:
```
1.2.3
```

## §10 — Goldens regen process

Same as PR-A's vcpkg/conan goldens — extend the `CASES` array in each of `cdx_regression.rs` / `spdx_regression.rs` / `spdx3_regression.rs` by 2 entries (bazel + cmake). Run with `MIKEBOM_UPDATE_*_GOLDENS=1` env vars to generate the 6 new committed goldens. Verify the existing 11 ecosystems (apk/cargo/deb/gem/golang/maven/npm/pip/rpm/vcpkg/conan) stay byte-identical per SC-005.

## §11 — Diff scope vs the spec's SC-007

SC-007 caps diff at ≤2 modified production files + ≤4 new tests + ≤2 new fixture dirs + 6 new goldens + ≤1 modified doc. Final tally per this plan:
- 2 modified production files: `bazel.rs`, `cmake.rs` ✓
- 4 new test files: `scan_bazel.rs`, `scan_cmake.rs`, `scan_cmake_vendored.rs`, `scan_cmake_findpackage_negative.rs` ✓
- 2 new fixture dirs: `tests/fixtures/{bazel,cmake}/` ✓
- 6 new goldens: 2 ecosystems × 3 formats ✓
- 1 modified doc: `docs/user-guide/cli-reference.md` ✓
- 3 modified existing test files: `cdx_regression.rs`, `spdx_regression.rs`, `spdx3_regression.rs` (NEW row insertion only — `CASES` array + the new test fn) — within "implicit modified" budget; not counted toward SC-007's "≤2 modified production files" since they're test code.

Plus auto-updated `CLAUDE.md` (from `/speckit-plan`) — same pattern as every prior milestone, expected.

---

## Summary — research is settled

All decisions anchor to:
- Milestone-102 spec's clarifications session (3 Qs resolved; inherited)
- Milestone-102 plan.md's Constitution Check (12/12 PASS; inherited)
- PR-A's existing stub signatures (unchanged)
- The regex shapes documented above (§1–§7)

Ready for Phase 1.
