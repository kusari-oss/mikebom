# Contracts — milestone 103 Bazel + CMake readers

8 behavioral contracts. Each specifies the invariant and a verification recipe.

## Contract 1 — Bazel MODULE.bazel emits `pkg:bazel/<name>@<version>` (FR-001 / FR-002 / SC-001)

**Invariant**: every `bazel_dep(name = "X", version = "Y")` in MODULE.bazel emits exactly one `pkg:bazel/X@Y` component with `mikebom:source-files = ["MODULE.bazel"]`. `dev_dependency = True` sets `LifecycleScope::Development` (which maps to CDX `scope = "excluded"` per milestone-052 + Principle V).

**Verification**: `cargo +stable test --test scan_bazel` — Test 1 asserts both expected components emit; the googletest one has `lifecycle_scope = Development`.

## Contract 2 — Bazel WORKSPACE.bazel http_archive emits URL + sha256 (FR-003 / FR-004)

**Invariant**: `http_archive(name = N, urls = [U], sha256 = S)` produces a component with PURL `pkg:bazel/N@<version-from-url>`, `mikebom:download-url = U`, `mikebom:bazel-archive-name = N`, and a `ContentHash` of `(SHA-256, S)` in `hashes[]`.

**Verification**: `scan_bazel.rs` Test 2 asserts these fields on the `rules_python` http_archive entry.

## Contract 3 — Bazel WORKSPACE.bazel git_repository uses short-SHA commit (FR-003 + research §8)

**Invariant**: `git_repository(name = N, remote = R, commit = <40-char SHA>)` produces a component with PURL `pkg:bazel/N@<first-7-chars-of-SHA>` and `mikebom:download-url = R`. Full SHA preserved... (currently NOT preserved as a separate annotation in this milestone; future enhancement).

**Verification**: `scan_bazel.rs` Test 3 asserts the `rules_foo` git_repository entry has PURL `pkg:bazel/rules_foo@deadbee` (first 7 chars of `deadbeef0123...`).

## Contract 4 — CMake FetchContent GIT_REPOSITORY → pkg:github when applicable (FR-006)

**Invariant**: `FetchContent_Declare(<name> GIT_REPOSITORY https://github.com/<owner>/<repo>(.git)? GIT_TAG <tag>)` emits component with PURL `pkg:github/<owner>/<repo>@<tag>` and `mikebom:source-files` pointing at the declaring file. Non-GitHub `GIT_REPOSITORY` URLs fall back to `pkg:generic/<name>@<tag>` with `mikebom:download-url`.

**Verification**: `scan_cmake.rs` Test 1 asserts the googletest fixture component has PURL `pkg:github/google/googletest@release-1.14.0`.

## Contract 5 — CMake ExternalProject_Add URL+URL_HASH emits sha256 (FR-006)

**Invariant**: `ExternalProject_Add(<name> URL <u> URL_HASH SHA256=<digest>)` emits component with PURL `pkg:generic/<name>@<version-from-url>`, `mikebom:download-url = <u>`, and `ContentHash(SHA-256, <digest>)` in `hashes[]`.

**Verification**: `scan_cmake.rs` Test 2 asserts the zlib fixture component carries the expected SHA-256.

## Contract 6 — CMake included-file walk attributes `source-files` correctly (FR-005 + FR-010)

**Invariant**: when a `CMakeLists.txt` does `include(cmake/third_party.cmake)` and that file contains a `FetchContent_Declare`, the resulting component's `mikebom:source-files` points at `cmake/third_party.cmake`, NOT the top-level `CMakeLists.txt`.

**Verification**: `scan_cmake.rs` Test 3 asserts the boost component (declared in `cmake/third_party.cmake`) has source-files = `["cmake/third_party.cmake"]` (relative or absolute path; absolute under the test tempdir).

## Contract 7 — CMake find_package does NOT emit components (FR-007 / SC-003)

**Invariant**: a `CMakeLists.txt` containing `find_package(X REQUIRED)` (with NO `FetchContent_Declare` or `ExternalProject_Add` for X) produces zero `pkg:*/X` components attributed to the cmake reader. The reader's regexes anchor on `FetchContent_Declare` and `ExternalProject_Add` literally; `find_package` doesn't match.

**Verification**: `scan_cmake_findpackage_negative.rs` dedicated test — fixture is a `CMakeLists.txt` containing ONLY `find_package(zlib REQUIRED)`; assert SBOM has zero `pkg:*/zlib` components.

## Contract 8 — `--include-vendored` gates `add_subdirectory(third_party/...)` emission (FR-008 / FR-009 / SC-004)

**Invariant**:
- Without `--include-vendored` (and `MIKEBOM_INCLUDE_VENDORED` env unset): NO components emit from `add_subdirectory(third_party/<name>)` or `add_subdirectory(vendor/<name>)` calls.
- With the flag (or env=`1`): one `pkg:generic/<name>@<version-from-version.txt>` component per matching call. Version-segment omitted when no `version.txt` exists. Each component carries `mikebom:vendored = true` (JSON boolean) annotation.
- First-party `add_subdirectory(src)` / `add_subdirectory(tests)` calls do NOT emit, regardless of flag — path-prefix gate (`third_party/` or `vendor/` only).

**Verification**: `scan_cmake_vendored.rs` — 3 tests covering all three branches above.
