# Feature Specification: Bazel + CMake source-tree readers (milestone 102 PR-B)

**Feature Branch**: `103-bazel-cmake-impl`
**Created**: 2026-05-14
**Status**: Draft
**Input**: User description: "let's work on Bazel + CMake"

## Context

This milestone completes the C/C++ source-tree-reader expansion that began in **milestone 102 PR-A** (PR #215, merged). PR-A shipped the foundational architecture (4-reader dispatch wiring, `--include-vendored` CLI flag plumbing, `MIKEBOM_INCLUDE_VENDORED` env var, stub `bazel.rs` + `cmake.rs` files) plus US3 readers (vcpkg + Conan with 12 unit tests + 3 integration tests + cross-ecosystem dedup verification). The deferred US1 (Bazel) and US2 (CMake) regex parsers land here.

The original milestone-102 spec at [`specs/102-cpp-bazel-cmake-readers/spec.md`](../102-cpp-bazel-cmake-readers/spec.md) is the **authoritative design reference** for the Bazel + CMake user stories (US1 + US2), clarifications (parse-error policy, vendored-deps opt-in, cross-ecosystem dedup), and edge cases. This milestone-103 spec narrows scope to the implementation work specifically deferred from PR-A, and explicitly inherits 102's clarifications + FR-001..FR-006 + FR-011 + FR-013 + FR-014 + FR-015 + FR-016 + FR-017.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Bazel project author gets SBOM coverage for declared deps (Priority: P1) 🎯 MVP

(Inherited verbatim from milestone-102 US1, scope unchanged.) A team building a C++ service with Bazel maintains a `MODULE.bazel` (Bzlmod, Bazel 6+) declaring third-party deps via `bazel_dep(name = "abseil-cpp", version = "20240722.0")` plus a `WORKSPACE.bazel` carrying legacy `http_archive(...)` declarations. When they run `mikebom sbom scan --path .`, every declared Bazel dependency surfaces in the emitted SBOM with the right ecosystem PURL, declared version, and (when available) upstream URL + SHA-256 from the `http_archive` rule.

**Why this priority**: dominant build system for large-scale C++ codebases (Google, Meta, LinkedIn, Tesla); machine-parseable build files; lowest-risk highest-coverage source of C/C++ dependency truth without runtime tracing. P1 same as 102.

**Independent Test**: scan `mikebom-cli/tests/fixtures/bazel/` (NEW in this PR) containing `MODULE.bazel` (≥2 `bazel_dep` entries, one with `dev_dependency = True`) and `WORKSPACE.bazel` (≥1 `http_archive` + ≥1 `git_repository`); verify emitted CDX 1.6 contains the expected components with `pkg:bazel/<name>@<version>` PURLs, upstream URLs as `mikebom:download-url`, SHA-256 hashes in `hashes[]`, and `LifecycleScope::Development` on the dev_dependency entry.

**Acceptance Scenarios**:

1. **Given** a directory containing a `MODULE.bazel` with `bazel_dep(name = "abseil-cpp", version = "20240722.0")` and `bazel_dep(name = "googletest", version = "1.14.0", dev_dependency = True)`, **When** the operator runs `mikebom sbom scan --path .`, **Then** the emitted SBOM contains 2 components with `pkg:bazel/abseil-cpp@20240722.0` and `pkg:bazel/googletest@1.14.0` PURLs, the googletest one has `LifecycleScope::Development` (CDX `scope = "excluded"` per the existing milestone-052 mapping), and both carry `mikebom:source-files = ["MODULE.bazel"]`.
2. **Given** the same directory also has a `WORKSPACE.bazel` with `http_archive(name = "rules_python", urls = ["https://github.com/bazelbuild/rules_python/archive/0.30.0.tar.gz"], sha256 = "abc...")`, **When** the same scan runs, **Then** the SBOM contains a `rules_python` component with the upstream URL recorded as `mikebom:download-url`, the declared SHA-256 in `hashes[]`, and `mikebom:bazel-archive-name = "rules_python"`.
3. **Given** a `WORKSPACE.bazel` with `git_repository(name = "foo", remote = "https://github.com/owner/foo.git", commit = "abc1234...")`, **When** the scan runs, **Then** the SBOM contains a `foo` component whose PURL encodes the git ref (`pkg:bazel/foo@abc1234`) and the upstream remote URL is recorded as `mikebom:download-url`.
4. **Given** a malformed `WORKSPACE.bazel` (unbalanced parens), **When** the scan runs, **Then** a `tracing::warn!` log fires naming the file + parse error, zero components emit from that file, AND the scan continues processing other readers per FR-013 (inherited from milestone-102 FR-015).

---

### User Story 2 - CMake project author gets SBOM coverage for FetchContent + ExternalProject (Priority: P1)

(Inherited from milestone-102 US2, scope unchanged.) A team building C/C++ libraries with CMake declares third-party deps via `FetchContent_Declare(googletest GIT_REPOSITORY ... GIT_TAG release-1.14.0)` and `ExternalProject_Add(zlib URL ... URL_HASH SHA256=...)` directives in `CMakeLists.txt` and sometimes in included `.cmake` modules under `cmake/` or `third_party/`. After this PR these surface as components with version, upstream URL, and SHA-256 where declared.

**Why this priority**: most widely deployed C/C++ build system across open-source (LLVM, OpenSSL, every Linux distro's `-cmake` package); FetchContent + ExternalProject are structured enough to parse heuristically. P1 same as 102.

**Independent Test**: scan `mikebom-cli/tests/fixtures/cmake/` (NEW) with `CMakeLists.txt` containing ≥1 `FetchContent_Declare(GIT_REPOSITORY)` + ≥1 `ExternalProject_Add(URL ... URL_HASH ...)` + 1 `find_package(zlib REQUIRED)` (per the FR-007 negative-emission test); also `cmake/third_party.cmake` with 1 `FetchContent_Declare(URL)` to exercise the included-file walk. Verify emitted CDX contains the expected components, zero `pkg:*/zlib` from `find_package`, and proper `mikebom:source-files` on each.

**Acceptance Scenarios**:

1. **Given** a `CMakeLists.txt` with `FetchContent_Declare(googletest GIT_REPOSITORY https://github.com/google/googletest.git GIT_TAG release-1.14.0)`, **When** mikebom scans, **Then** the SBOM contains a `googletest` component with PURL `pkg:github/google/googletest@release-1.14.0` and `mikebom:source-files = ["CMakeLists.txt"]`.
2. **Given** `ExternalProject_Add(zlib URL https://zlib.net/zlib-1.3.1.tar.gz URL_HASH SHA256=...)`, **When** the scan runs, **Then** the SBOM contains a `zlib` component with PURL `pkg:generic/zlib@1.3.1` (version from URL filename), `mikebom:download-url` set to the declared URL, declared SHA-256 in `hashes[]`.
3. **Given** the same CMakeLists.txt has `find_package(zlib REQUIRED)`, **When** the scan runs, **Then** the emitted SBOM contains ZERO `pkg:*/zlib` components attributed to the cmake reader (FR-007: `find_package` is system-resolved, would double-count against OS-package readers + vcpkg/Conan).
4. **Given** `cmake/third_party.cmake` (an included file) with `FetchContent_Declare(boost URL ... URL_HASH SHA256=...)`, **When** scan walks the project root, **Then** the boost component emits with `mikebom:source-files = ["cmake/third_party.cmake"]` (the included file, not the top-level `CMakeLists.txt`).

---

### User Story 3 - Vendored-dep opt-in works end-to-end (Priority: P2)

(Implementing the `--include-vendored` CLI flag's behavioral effect — the flag itself was wired in PR-A but consumed no logic since `cmake.rs` was a stub.) Operator runs `mikebom sbom scan --path .` against a CMake project that uses `add_subdirectory(third_party/foo)` for vendored deps. Without the flag, foo does NOT emit (PR-A baseline: stub returns empty). With `--include-vendored` (or `MIKEBOM_INCLUDE_VENDORED=1`), foo emits as `pkg:generic/foo@<version-from-version.txt>` when a co-located `version.txt` exists.

**Why this priority**: P2 because US1 + US2 are independently shippable as the headline value. Vendored-dep opt-in is a quality enhancement that doesn't change the reader-architecture decisions. Naturally lands with US2 since cmake.rs is the consumer.

**Independent Test**: scan `mikebom-cli/tests/fixtures/cmake/` (extended with a `third_party/foo/version.txt` file) WITH and WITHOUT `MIKEBOM_INCLUDE_VENDORED=1`. Without: zero foo components. With: 1 `pkg:generic/foo@1.2.3` component with `mikebom:vendored = true` annotation.

**Acceptance Scenarios**:

1. **Given** a CMake project with `add_subdirectory(third_party/foo)` and a `third_party/foo/version.txt` containing `1.2.3`, **When** mikebom scans without `--include-vendored`, **Then** zero `pkg:generic/foo` components emit.
2. **Given** the same project, **When** mikebom scans with `--include-vendored`, **Then** exactly one `pkg:generic/foo@1.2.3` component emits with `mikebom:vendored = true` annotation and `mikebom:source-files = ["CMakeLists.txt"]`.
3. **Given** `add_subdirectory(src)` (a first-party project sub-module, not a vendored dep — path doesn't start with `third_party/` or `vendor/`), **When** mikebom scans with `--include-vendored`, **Then** NO `pkg:generic/src` component emits. The path-prefix gate protects Principle IX (Accuracy).

---

### Edge Cases (inherited from milestone-102; see [`specs/102-cpp-bazel-cmake-readers/spec.md`](../102-cpp-bazel-cmake-readers/spec.md) Edge Cases section for the authoritative list)

- MODULE.bazel + WORKSPACE.bazel both declaring same dep — MODULE.bazel wins.
- WORKSPACE.bazel without `sha256` — emit component without `hashes[]` entry; `mikebom:download-url` still recorded.
- `bazel_dep` with development version pin (`0.0.0-...`) — emitted verbatim.
- CMakeLists.txt with `FetchContent_Declare` inside `if(BUILD_TESTING)` block — emit with `scope = "test"` when block detected as test-only conditional (best-effort).
- CMakeLists.txt with macro-emitted `FetchContent_Declare` — not detected (heuristic-coverage gap; documented per SC-002 90% floor).
- Non-GitHub `GIT_REPOSITORY` URLs in FetchContent — fall back to `pkg:generic/` with `mikebom:download-url`.
- `find_package(X CONFIG)` against system packages — NOT parsed per FR-007 (covered separately by OS readers + vcpkg + Conan).

## Requirements *(mandatory)*

All FRs here implement requirements inherited from milestone-102 spec; numbering is restarted at FR-001 for this milestone's scope clarity but each FR cross-references its 102 counterpart.

### Functional Requirements

- **FR-001** (= 102:FR-001): System MUST detect Bazel projects by the presence of `MODULE.bazel`, `WORKSPACE`, or `WORKSPACE.bazel` and walk the project root to extract dependency declarations.
- **FR-002** (= 102:FR-002): System MUST parse `MODULE.bazel` to extract every `bazel_dep(name = "...", version = "...", dev_dependency = ...)` declaration. `dev_dependency = True` MUST set `LifecycleScope::Development` (which maps to standards-native CDX `scope` + SPDX 2.3 `DEV_DEPENDENCY_OF` + SPDX 3 `LifecycleScopeType` per Principle V).
- **FR-003** (= 102:FR-003): System MUST parse `WORKSPACE` / `WORKSPACE.bazel` to extract `http_archive(name, urls, sha256)`, `http_file(name, urls, sha256)`, and `git_repository(name, remote, commit OR tag)` rules. Best-effort version: parse from URL filename for archives, use `commit`/`tag` for git refs.
- **FR-004** (= 102:FR-004): System MUST record the declared upstream URL on every Bazel-derived component as `mikebom:download-url` AND record the declared `sha256` (when present) in the component's `hashes[]` array.
- **FR-005** (= 102:FR-005): System MUST detect CMake projects by `CMakeLists.txt` in the scan root or its immediate subdirectories and walk `CMakeLists.txt` files at the root + `*.cmake` files at depth 1 of `cmake/`, `Modules/`, `third_party/`.
- **FR-006** (= 102:FR-006): System MUST parse `CMakeLists.txt` + included `.cmake` files for `FetchContent_Declare(...)` and `ExternalProject_Add(...)`, distinguishing GIT_REPOSITORY-form (emit `pkg:github/<owner>/<repo>@<tag>` for GitHub URLs, `pkg:generic/<name>@<tag>` otherwise) from URL-form (emit `pkg:generic/<name>@<version-from-url>` with `mikebom:download-url` + SHA-256 from `URL_HASH SHA256=...` when present).
- **FR-007** (= 102:FR-011): System MUST NOT emit components for `find_package(X)` declarations alone. These are system-resolved; OS-package readers + vcpkg + Conan handle them. Explicit negative-emission test required.
- **FR-008** (= 102:FR-016): System MUST honor the `--include-vendored` CLI flag (and `MIKEBOM_INCLUDE_VENDORED` env var, both wired in PR-A) by emitting `pkg:generic/<name>@<version-from-version.txt>` components for `add_subdirectory(third_party/...)` and `add_subdirectory(vendor/...)` calls when the flag is set. When unset (default), these calls emit zero components.
- **FR-009**: Vendored components MUST carry a `mikebom:vendored` annotation whose value is the **JSON boolean `true`** (NOT the string `"true"`) per the existing `mikebom:shade-relocation` boolean-flag precedent from milestone 009. Per milestone-102 FR-016.
- **FR-010** (= 102:FR-012): System MUST emit `mikebom:source-files = [...]` on every Bazel/CMake-derived component pointing back to the declaring file path (including the specific `.cmake` file for included declarations).
- **FR-011** (= 102:FR-013): System MUST treat Bazel + CMake readers as cross-platform — no `#[cfg(unix)]` gates. Same regex parsers run identically on Linux / macOS / Windows.
- **FR-012** (= 102:FR-014): System MUST conform to CycloneDX 1.6, SPDX 2.3, and SPDX 3 emission for every component. Standards-native scope field MUST be used per Principle V.
- **FR-013** (= 102:FR-015): When a manifest file fails to parse, the reader MUST `tracing::warn!` the file path + error, emit zero components from that file, and let the scan continue. Other readers + other files MUST process normally. Skip-with-warn precedent matches existing maven/golang readers.
- **FR-014**: System MUST update `docs/user-guide/cli-reference.md` to document the `--include-vendored` flag's behavior (default-off, false-positive risks for `src/`/`tests/` sub-modules, `version.txt` version-backfill convention). Per 102:FR-017.
- **FR-015**: System MUST extend the existing `cdx_regression.rs` / `spdx_regression.rs` / `spdx3_regression.rs` test files with 2 new ecosystem test functions each (one for `bazel`, one for `cmake`), regenerating 6 byte-identity goldens total. The existing 11 ecosystems' goldens (apk, cargo, deb, gem, golang, maven, npm, pip, rpm, vcpkg, conan — vcpkg + conan landed in PR-A) MUST stay byte-identical per 102:SC-006.
- **FR-016**: System MUST NOT introduce new Cargo dependencies. Use workspace `regex` for pattern extraction (already direct dep per milestone 013). Per 102:SC-008.

### Key Entities

- **Bazel dependency**: (name, version, source) where source ∈ {bzlmod, http_archive, http_file, git_repository}. Optional: declared upstream URL, declared SHA-256, dev_dependency flag.
- **CMake fetched dependency**: (name, version, source) where source ∈ {fetchcontent_git, fetchcontent_url, externalproject_git, externalproject_url}. Optional: declared GIT_REPOSITORY URL, declared GIT_TAG, declared URL, declared URL_HASH SHA-256, test-scope flag.
- **CMake vendored dependency**: (name, optional version-from-version.txt). Path-prefix gated by `third_party/` or `vendor/`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An operator scanning a Bazel C++ project (MODULE.bazel + WORKSPACE.bazel) sees ≥95% of declared dependencies surface in the emitted SBOM with correct ecosystem PURLs and declared versions, matching `bazel mod graph` output for direct deps.
- **SC-002**: An operator scanning a CMake project using FetchContent + ExternalProject sees ≥90% of declared deps surface with correct PURLs. The 10% headroom accounts for `FetchContent_Declare` calls inside macros or with non-literal arguments (documented heuristic-coverage gap).
- **SC-003**: An operator scanning a CMake project with `find_package(X)` calls sees ZERO `pkg:*/X` components attributed to the cmake reader (FR-007 negative-emission contract).
- **SC-004**: Vendored-dep opt-in works as designed: zero `pkg:generic/...` vendored components by default; ≥1 per `add_subdirectory(third_party/...)` call when `--include-vendored` is set.
- **SC-005**: Existing 11 ecosystems' goldens (apk/cargo/deb/gem/golang/maven/npm/pip/rpm/vcpkg/conan) stay byte-identical post-merge. Verifiable via `git diff --stat mikebom-cli/tests/fixtures/golden/`.
- **SC-006**: Pre-PR gate stays clean: `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1 ./scripts/pre-pr.sh` → `>>> all pre-PR checks passed.`
- **SC-007**: Diff scope bounded: ≤2 modified production files (`bazel.rs` + `cmake.rs` — both stubs from PR-A become real implementations) + ≤4 new test files + ≤2 new fixture directories + 6 new goldens + ≤1 modified doc file. Zero new Cargo dependencies.
- **SC-008**: Windows CI lane on the resulting PR passes — verifies FR-011 (cross-platform: no `#[cfg(unix)]` gates) empirically.

## Assumptions

- Inherits all assumptions from milestone-102 spec (Bazel scope, CMake script-parsing, PURL ecosystems, etc.).
- PR-A's foundational architecture is unchanged: `bazel.rs` + `cmake.rs` stub signatures (`pub fn read(scan_root: &Path) -> Vec<PackageDbEntry>` / `pub fn read(scan_root: &Path, include_vendored: bool) -> Vec<PackageDbEntry>`) are kept; the stubs are replaced with real implementations.
- Bazel `bazel_dep` calls and `http_archive` / `git_repository` rules use literal string arguments in ≥98% of real-world projects (verified via spot-checks of bazelbuild/rules_python, abseil-cpp, googletest, grpc, envoy MODULE.bazel files in 102's research).
- CMake `FetchContent_Declare` + `ExternalProject_Add` use literal arguments in ≥90% of the open-source corpus (verified via spot-check of LLVM, gRPC, Envoy, RocksDB).
- Synthetic fixtures (small, in-repo) are sufficient for the integration tests; no need to vendor multi-MB real-world projects.
- The `--include-vendored` CLI flag's surface (`#[arg(long, env = "MIKEBOM_INCLUDE_VENDORED")]`) is unchanged from PR-A; only the cmake.rs consumer needs to be implemented.
- Constitution alignment: same as milestone-102 — all 12 principles PASS post-design. The 3 new `mikebom:*` properties (`download-url`, `vendored`, `bazel-archive-name`) were audited in 102's plan.md and remain unchanged.
