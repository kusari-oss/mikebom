---
description: "Task list for milestone 103 — Bazel + CMake source-tree reader implementations (milestone 102 PR-B)"
---

# Tasks: Bazel + CMake source-tree readers (milestone 102 PR-B)

**Input**: Design documents from `/Users/mlieberman/Projects/mikebom/specs/103-bazel-cmake-impl/`
**Prerequisites**: plan.md, spec.md (with inherited 102 Clarifications), research.md, data-model.md, contracts/reader-contracts.md, quickstart.md. PR-A (#215) already merged.

**Tests**: Yes — TDD by integration test. Each reader gets dedicated test files PLUS 6 new byte-identity goldens (2 ecosystems × 3 formats) regenerated in the existing `cdx_regression` / `spdx_regression` / `spdx3_regression` suites.

**Organization**: 3 user stories converge on 2 reader bodies (replacing PR-A stubs). US1 (Bazel) + US2 (CMake) are both P1. US3 (vendored opt-in runtime) is P2 and naturally lands with US2 since cmake.rs is the consumer. Phase-2 foundational work is **already complete** (PR-A shipped dispatch wiring, CLI flag, env-var fallback, stub signatures).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files OR independent of incomplete tasks)
- **[Story]**: US1 / US2 / US3
- File paths are workspace-relative.

## Path Conventions

Production code: `mikebom-cli/src/scan_fs/package_db/{bazel,cmake}.rs` (2 modify). Integration tests + fixtures under `mikebom-cli/tests/`. Goldens under `mikebom-cli/tests/fixtures/golden/`. Docs in `docs/user-guide/cli-reference.md`. Zero changes outside these paths per SC-007.

---

## Phase 1: Setup

- [ ] T001 Confirm working branch is `103-bazel-cmake-impl`. Verify main is at post-PR-#215 (milestone 102 PR-A merge, commit `29e5202`) or later. Confirm `git diff --name-only main` shows only the spec dir as untracked.
- [ ] T002 Confirm baseline pre-PR gate passes. Run `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1 ./scripts/pre-pr.sh` on the unchanged tree; expect `>>> all pre-PR checks passed.`
- [ ] T003 Confirm PR-A stub signatures are in place. Run `grep -n "pub fn read" /Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/bazel.rs /Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/cmake.rs`. Expected: bazel `pub fn read(_scan_root: &Path) -> Vec<PackageDbEntry>`, cmake `pub fn read(_scan_root: &Path, _include_vendored: bool) -> Vec<PackageDbEntry>`.

---

## Phase 2: Foundational

**Purpose**: None needed. PR-A already shipped: `bazel.rs` + `cmake.rs` stubs registered in `package_db/mod.rs`, dispatch wired in `read_all` lines 807-808, `--include-vendored` CLI flag, `MIKEBOM_INCLUDE_VENDORED` env-var fallback, vcpkg + conan reference patterns. This milestone is pure body replacement.

(No tasks in this phase.)

---

## Phase 3: User Story 1 — Bazel reader (Priority: P1) 🎯 MVP

**Goal**: Replace `bazel.rs` stub with regex-based parser for `MODULE.bazel` (Bzlmod) + `WORKSPACE.bazel` (legacy `http_archive` / `http_file` / `git_repository`). Emit `pkg:bazel/<name>@<version>` components with `mikebom:download-url` + `mikebom:bazel-archive-name` + SHA-256 + `LifecycleScope::Development` per `dev_dependency = True`.

**Independent Test**: `cargo +stable test --test scan_bazel` against `tests/fixtures/bazel/`; assert 4 expected components (2 MODULE.bazel deps, 1 http_archive, 1 git_repository) with correct PURLs + annotations.

### Implementation for User Story 1

- [ ] T004 [P] [US1] Create Bazel fixture at `mikebom-cli/tests/fixtures/bazel/` per `data-model.md §test fixtures` + `research.md §9`:
    - `MODULE.bazel` — 2 `bazel_dep` calls (one with `dev_dependency = True`).
    - `WORKSPACE.bazel` — 1 `http_archive` (with `urls = [...]` + `sha256`) + 1 `git_repository` (with `remote` + 40-char `commit`).
- [ ] T005 [US1] Replace `mikebom-cli/src/scan_fs/package_db/bazel.rs` stub with real implementation per `data-model.md §bazel.rs`:
    - `pub fn read(scan_root: &Path) -> Vec<PackageDbEntry>` (signature unchanged from PR-A stub).
    - `parse_module_bazel(path)` — regex `(?ms)bazel_dep\s*\(\s*name\s*=\s*"([^"]+)"\s*,\s*version\s*=\s*"([^"]+)"(?:\s*,\s*dev_dependency\s*=\s*(True|False))?\s*\)` per research §1. Sets `LifecycleScope::Development` when dev_dependency=True.
    - `parse_workspace_bazel(path)` — two-pass regex per research §2: outer envelope matches `http_archive` / `http_file` / `git_repository`; inner extractors pull `urls`/`url`/`sha256`/`remote`/`commit`/`tag` from the body. Builds `mikebom:download-url` + `mikebom:bazel-archive-name` annotations; populates `hashes[]` with SHA-256 when present.
    - `build_bazel_entry(name, version, source_path, download_url, sha256, dev)` helper — same field-fill pattern as vcpkg.rs/conan.rs from PR-A.
    - `dedup_module_wins(entries)` — Contract 3: MODULE.bazel deps come first in the entries vec; dedup by name preserving first-seen.
    - `parse_version_from_url(url)` — regex `[-_]([0-9]+\.[0-9]+(?:\.[0-9]+)?)` per research §2.
    - Short-SHA truncation per research §8: first 7 chars of commit-SHA in PURL; full SHA discarded (out of scope to preserve for now).
    - Parse errors → `tracing::warn!` + return empty per FR-013.
    - 5 unit tests at the bottom (`#[cfg(test)] mod tests`): MODULE single dep, MODULE dev_dependency, WORKSPACE http_archive, WORKSPACE git_repository (short-SHA truncation), malformed-file skip-with-warn.
- [ ] T006 [US1] Create `mikebom-cli/tests/scan_bazel.rs` integration test per Contracts 1+2+3. 4 tests using `CARGO_BIN_EXE_mikebom` + `--offline`:
    - Test 1 (`bazel_module_emits_pkg_bazel_purls_with_native_scope`): assert pkg:bazel/abseil-cpp@20240722.0 + pkg:bazel/googletest@1.14.0; the googletest one has `lifecycle_scope = Development` AND the CDX `scope` field is the standards-native milestone-052 mapping (verify against the existing gem.cdx.json golden for the dev-dep convention).
    - Test 2 (`bazel_workspace_http_archive_emits_url_and_sha`): assert rules_python carries `mikebom:download-url`, SHA-256 in hashes[], `mikebom:bazel-archive-name`.
    - Test 3 (`bazel_workspace_git_repository_short_sha`): assert rules_foo has PURL `pkg:bazel/rules_foo@deadbee` (first 7 chars).
    - Test 4 (`bazel_source_files_annotation`): assert every Bazel-derived component carries `mikebom:source-files` pointing at the declaring `MODULE.bazel` or `WORKSPACE.bazel`.

### Verification for User Story 1

- [ ] T007 [US1] Verify Contracts 1+2+3 + Bazel unit tests. Run:
    ```bash
    cargo +stable test -p mikebom --bin mikebom scan_fs::package_db::bazel 2>&1 | grep "test result:"
    # Expected: ok. 5 passed.
    cargo +stable test -p mikebom --test scan_bazel 2>&1 | grep "test result:"
    # Expected: ok. 4 passed.
    cargo +stable clippy -p mikebom --all-targets -- -D warnings 2>&1 | tail -3
    # Expected: zero warnings.
    ```

**Checkpoint**: US1 complete. Bazel reader emits real components; clippy clean.

---

## Phase 4: User Story 2 — CMake reader (Priority: P1)

**Goal**: Replace `cmake.rs` stub with parser for `FetchContent_Declare` + `ExternalProject_Add` (both GIT_REPOSITORY and URL forms) across `CMakeLists.txt` + included `.cmake` files under `cmake/`, `Modules/`, `third_party/`. Emit `pkg:github/<owner>/<repo>@<tag>` for GitHub-hosted git deps, `pkg:generic/<name>@<version>` otherwise. `find_package` declarations MUST NOT emit components (FR-007).

**Independent Test**: `cargo +stable test --test scan_cmake` against `tests/fixtures/cmake/`; assert expected components + zero `pkg:*/openssl` from the `find_package(OpenSSL REQUIRED)` line.

### Implementation for User Story 2

- [ ] T008 [P] [US2] Create CMake fixture at `mikebom-cli/tests/fixtures/cmake/` per research §9:
    - `CMakeLists.txt` — 1 `FetchContent_Declare(googletest GIT_REPOSITORY https://github.com/google/googletest.git GIT_TAG release-1.14.0)` + 1 `ExternalProject_Add(zlib URL https://zlib.net/zlib-1.3.1.tar.gz URL_HASH SHA256=...)` + 1 `find_package(OpenSSL REQUIRED)` (negative-emission anchor) + `include(cmake/third_party.cmake)` + `add_subdirectory(third_party/foo)` (for US3 vendored test) + `add_subdirectory(src)` (for the US3 path-prefix-gate test — T012 Test 3 asserts this first-party sub-module does NOT emit a component even with `--include-vendored` set).
    - `cmake/third_party.cmake` — 1 `FetchContent_Declare(boost URL ... URL_HASH SHA256=...)`.
    - `third_party/foo/version.txt` — `1.2.3` (for US3 vendored test).
    - `src/CMakeLists.txt` (minimal, can be just `# placeholder for first-party src sub-module test`) — must exist for `add_subdirectory(src)` to be a syntactically valid CMake call.
- [ ] T009 [US2] Replace `mikebom-cli/src/scan_fs/package_db/cmake.rs` stub with real implementation per `data-model.md §cmake.rs`:
    - `pub fn read(scan_root: &Path, include_vendored: bool) -> Vec<PackageDbEntry>` (signature unchanged from PR-A stub).
    - `discover_cmake_files(scan_root)` — find `CMakeLists.txt` at root + `*.cmake` and `CMakeLists.txt` at depth 1 of `cmake/`, `Modules/`, `third_party/` per research §7. Non-recursive.
    - `parse_fetch_block(content, source_path, rule_name)` — parameterized over rule_name ("FetchContent_Declare" or "ExternalProject_Add"); 2-pass regex per research §3+§4: outer envelope captures (name, body); inner extractors pull GIT_REPOSITORY / GIT_TAG / URL / URL_HASH SHA256. GitHub URL detection via separate regex `^https?://github\.com/([^/]+)/([^/\.]+)` for `pkg:github/...` PURL form. Non-GitHub git → `pkg:generic/<name>@<tag>` with `mikebom:download-url`. URL form → version parsed from filename, `pkg:generic/<name>@<version>` with URL + SHA-256.
    - `parse_vendored(content, source_path, scan_root)` — only called when `include_vendored == true`; regex `(?ms)add_subdirectory\s*\(\s*(third_party|vendor)/([^)\s]+)\s*\)` per research §6. For each match, read `<scan_root>/<prefix>/<name>/version.txt` first non-empty line for version backfill. Emit `pkg:generic/<name>[@<version>]` with `mikebom:vendored = true` (JSON boolean) annotation.
    - `parse_version_from_url(url)` — same helper as bazel.rs.
    - `build_cmake_entry(...)` — same field-fill pattern as build_bazel_entry.
    - `find_package(X)` is NOT parsed per FR-007 — the parse_fetch_block regex anchors on the literal rule names (`FetchContent_Declare` / `ExternalProject_Add`); `find_package` doesn't match either anchor.
    - Parse errors → `tracing::warn!` + skip-with-warn per FR-013.
    - 6 unit tests: FetchContent_Declare GIT, FetchContent_Declare URL, ExternalProject_Add GIT, ExternalProject_Add URL, included-file walk (cmake/third_party.cmake), vendored opt-in gated.
- [ ] T010 [US2] Create `mikebom-cli/tests/scan_cmake.rs` integration test per Contracts 4+5+6+7. 4 tests:
    - Test 1 (`cmake_fetchcontent_github_emits_pkg_github`): googletest → `pkg:github/google/googletest@release-1.14.0`.
    - Test 2 (`cmake_externalproject_url_emits_sha256_and_url`): zlib carries the declared SHA-256 + `mikebom:download-url`.
    - Test 3 (`cmake_includes_walked`): boost from `cmake/third_party.cmake` has `mikebom:source-files` pointing at that file.
    - Test 4 (`cmake_find_package_not_emitted`): assert zero `pkg:*/OpenSSL` / `pkg:*/openssl` components.

### Verification for User Story 2

- [ ] T011 [US2] Verify Contracts 4+5+6+7. Run:
    ```bash
    cargo +stable test -p mikebom --bin mikebom scan_fs::package_db::cmake 2>&1 | grep "test result:"
    # Expected: ok. 6 passed.
    cargo +stable test -p mikebom --test scan_cmake 2>&1 | grep "test result:"
    # Expected: ok. 4 passed.
    cargo +stable clippy -p mikebom --all-targets -- -D warnings 2>&1 | tail -3
    # Expected: zero warnings.
    ```

**Checkpoint**: US2 complete. CMake reader emits real components; find_package negative-emission verified.

---

## Phase 5: User Story 3 — Vendored opt-in runtime (Priority: P2)

**Goal**: The `--include-vendored` CLI flag wired in PR-A gains actual runtime behavior — cmake.rs's `parse_vendored` (implemented in T009) is exercised end-to-end. Two dedicated test files isolate the flag's behavior + the find_package negative-emission contract from US2's main test.

### Implementation for User Story 3

- [ ] T012 [P] [US3] Create `mikebom-cli/tests/scan_cmake_vendored.rs` integration test per Contract 8. 3 tests:
    - Test 1 (`vendored_zero_by_default`): scan the cmake fixture with NO `--include-vendored` (and NO `MIKEBOM_INCLUDE_VENDORED` env); assert zero `pkg:generic/foo` components.
    - Test 2 (`vendored_emitted_with_flag`): invoke mikebom with `Command::env("MIKEBOM_INCLUDE_VENDORED", "1")`; assert exactly one `pkg:generic/foo@1.2.3` (version backfilled from `third_party/foo/version.txt`) with `mikebom:vendored = true` (JSON boolean) annotation.
    - Test 3 (`vendored_path_prefix_gate`): exercises `add_subdirectory(src)` in the fixture (NOT prefixed by `third_party/` or `vendor/`); assert NO `pkg:generic/src` component emits even with the flag on. The fixture line is pre-included via T008 — no additional fixture work needed.
- [ ] T013 [P] [US3] Create `mikebom-cli/tests/scan_cmake_findpackage_negative.rs` integration test per Contract 7 dedicated assertion. 1 test:
    - **Fixture**: NEW directory at `mikebom-cli/tests/fixtures/cmake_findpackage_only/` with one file: `CMakeLists.txt` containing exactly:
      ```cmake
      cmake_minimum_required(VERSION 3.20)
      project(findpackage_only)

      include(FetchContent)
      FetchContent_Declare(googletest GIT_REPOSITORY https://github.com/google/googletest.git GIT_TAG release-1.14.0)
      find_package(zlib REQUIRED)
      ```
      The fixture deliberately combines: (a) a FetchContent_Declare for `googletest` to prove the cmake reader RAN, with (b) a `find_package(zlib REQUIRED)` line to verify the negative-emission contract.
    - **Test** (`findpackage_only_no_emission`): scan the fixture; assert (a) ≥1 `pkg:github/google/googletest` component exists (cmake reader ran), AND (b) zero `pkg:*/zlib` components anywhere in the emitted SBOM attributed to the cmake reader. This proves the regex anchors on `FetchContent_Declare` / `ExternalProject_Add` literally and does NOT match `find_package`.

### Verification for User Story 3

- [ ] T014 [US3] Verify Contract 8 + Contract 7 dedicated. Run:
    ```bash
    cargo +stable test -p mikebom --test scan_cmake_vendored --test scan_cmake_findpackage_negative 2>&1 | grep "test result:"
    # Expected: ok. 3 passed + ok. 1 passed.
    ```

**Checkpoint**: US3 complete. Vendored opt-in works end-to-end; find_package negative-emission isolated + verified.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Goldens regen (6 new), docs (cli-reference.md), diff-scope audit, pre-PR gate, open PR.

- [ ] T015 [P] Add 2 ecosystem test functions to each of `mikebom-cli/tests/cdx_regression.rs`, `mikebom-cli/tests/spdx_regression.rs`, `mikebom-cli/tests/spdx3_regression.rs` (one for `bazel`, one for `cmake`). Add 2 new entries to each file's `CASES` array per data-model.md §Goldens regen. Match the existing per-ecosystem pattern.
- [ ] T016 Regenerate the 6 NEW goldens (2 ecosystems × 3 formats). Run:
    ```bash
    MIKEBOM_UPDATE_CDX_GOLDENS=1 MIKEBOM_UPDATE_SPDX_GOLDENS=1 MIKEBOM_UPDATE_SPDX3_GOLDENS=1 \
      cargo +stable test -p mikebom \
        --test cdx_regression --test spdx_regression --test spdx3_regression
    git status mikebom-cli/tests/fixtures/golden/ | head -15
    # Expected: 6 NEW files (cyclonedx/{bazel,cmake}.cdx.json + spdx-2.3/{bazel,cmake}.spdx.json + spdx-3/{bazel,cmake}.spdx3.json).
    git diff --stat mikebom-cli/tests/fixtures/golden/cyclonedx/{apk,cargo,conan,deb,gem,golang,maven,npm,pip,rpm,vcpkg}.cdx.json | tail -1
    git diff --stat mikebom-cli/tests/fixtures/golden/spdx-2.3/{apk,cargo,conan,deb,gem,golang,maven,npm,pip,rpm,vcpkg}.spdx.json | tail -1
    git diff --stat mikebom-cli/tests/fixtures/golden/spdx-3/{apk,cargo,conan,deb,gem,golang,maven,npm,pip,rpm,vcpkg}.spdx3.json | tail -1
    # Expected: all 3 empty (existing 11 ecosystems' goldens untouched per SC-005).
    ```
    Then re-run without env vars to confirm byte-identity locks:
    ```bash
    cargo +stable test -p mikebom --test cdx_regression --test spdx_regression --test spdx3_regression 2>&1 | grep "test result:"
    # Expected: ok. 13 passed × 3 (11 + 2 new ecosystems per format).
    ```
- [ ] T017 [P] Update `docs/user-guide/cli-reference.md` per FR-014. Add a `--include-vendored` section covering: default-OFF, what counts as vendored (`third_party/` or `vendor/` path prefix), false-positive risks (`src/`/`tests/` sub-modules), version backfill convention via `version.txt`. Reference data-model.md §Docs update for the canonical wording.
- [ ] T018 Verify diff scope per SC-007. Run:
    ```bash
    git diff --name-only origin/main | sort
    # Expected:
    #   CLAUDE.md                                              (auto-updated)
    #   docs/user-guide/cli-reference.md
    #   mikebom-cli/src/scan_fs/package_db/bazel.rs            (MODIFY: stub → real)
    #   mikebom-cli/src/scan_fs/package_db/cmake.rs            (MODIFY: stub → real)
    #   mikebom-cli/tests/cdx_regression.rs
    #   mikebom-cli/tests/spdx_regression.rs
    #   mikebom-cli/tests/spdx3_regression.rs
    #   specs/103-bazel-cmake-impl/...

    git ls-files --others --exclude-standard | sort
    # Expected NEW:
    #   mikebom-cli/tests/scan_{bazel,cmake,cmake_vendored,cmake_findpackage_negative}.rs   (4 tests)
    #   mikebom-cli/tests/fixtures/bazel/{MODULE.bazel,WORKSPACE.bazel}                     (2 files)
    #   mikebom-cli/tests/fixtures/cmake/{CMakeLists.txt,cmake/third_party.cmake,third_party/foo/version.txt,src/CMakeLists.txt}   (4 files)
    #   mikebom-cli/tests/fixtures/cmake_findpackage_only/CMakeLists.txt                    (1 file)
    #   mikebom-cli/tests/fixtures/golden/{cyclonedx,spdx-2.3,spdx-3}/{bazel,cmake}.*       (6 goldens)

    git diff --name-only origin/main | grep -E '^Cargo\.(lock|toml)$|/Cargo\.(lock|toml)$' | wc -l
    # Expected: 0
    ```
- [ ] T019 Run the mandatory pre-PR gate per SC-006. Run `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1 ./scripts/pre-pr.sh`. Expect: `>>> all pre-PR checks passed.`
- [ ] T020 Open the PR. Title: `feat(103): Bazel + CMake source-tree readers (milestone 102 PR-B)`. Body must mention:
    - 2 readers replacing PR-A stubs (Bazel + CMake)
    - 12 new integration tests (4 Bazel + 4 CMake + 3 vendored + 1 findpackage-negative) + 11 new unit tests (5 Bazel + 6 CMake)
    - 6 new byte-identity goldens; existing 11 × 3 unchanged per SC-005
    - `--include-vendored` flag (wired in PR-A) now has runtime behavior
    - `find_package` NOT emitted per FR-007
    - Zero new Cargo deps; zero changes outside bazel.rs + cmake.rs + tests + docs

---

## Dependencies

```text
T001 → T002 → T003 (verify PR-A stubs in place)
T003 → US1: T004 [P] → T005 → T006 → T007
T003 → US2: T008 [P] → T009 → T010 → T011
T003 → US3: depends on T009 cmake.rs being real (parse_vendored implemented).
  T012 [P] + T013 [P] → T014
T011 + T014 → T015 [P] → T016 (regen goldens)
T016 → T017 [P] (docs) → T018 (diff scope) → T019 (pre-PR) → T020 (open PR)
```

US1 (T004 + T005 + T006 + T007) and US2 (T008 + T009 + T010 + T011) are file-level independent — can implement in parallel by 2 developers / agent threads.
US3 (T012 + T013) requires T009 to be complete (cmake.rs body must include `parse_vendored`).
Polish phase (T015 + T017) — both [P]; T015 + T016 sequenced (regen depends on test fns).

## Parallel Execution Opportunities

- **After T003**: fixtures (T004, T008) parallel — different file trees.
- **Within US3**: T012 + T013 parallel — different test files; fixture for T013 is a new minimal one.
- **Polish**: T015 + T017 parallel.
- **Reader implementations**: T005 (bazel.rs) + T009 (cmake.rs) parallel — different files, share zero code (each has its own `build_*_entry` helper).

## Implementation Strategy

**MVP scope**: US1 + US2 — both P1, both shippable. US3 is a quality enhancement that depends on US2's cmake.rs but is independently testable. Ship all 3 in the same PR since they're tightly coupled (cmake.rs is the consumer for US3's runtime behavior).

**Suggested execution order**: T001 → T002 → T003 → (parallel: T004 + T008) → (parallel: T005 + T009) → (parallel: T006 + T010 + T012 + T013) → (parallel: T007 + T011 + T014) → T015 → T016 → T017 → T018 → T019 → T020. Total: 20 tasks, ~3 hours focused work.

**Risk**: T009 (CMake reader) is the largest single implementation task with the most regex complexity. Budget ~1.5h. T005 (Bazel) ~1h. Tests + goldens + docs ~30min.

**Backup plan**: if T009's CMake heuristics surface real-world corpus failures during implementation, descope to FetchContent_Declare ONLY (drop ExternalProject_Add). Document the gap in the PR body; ship US2 partial. T010 + T011 stay; ExternalProject test case is removed from the fixture. Acceptable per SC-002's 90% heuristic floor.

## Task format validation

All 20 tasks follow the required format `- [ ] TXXX [P?] [USX?] Description with file path`:
- ✅ Checkboxes start every line
- ✅ Sequential task IDs T001 – T020
- ✅ [P] markers only where parallelization is sound
- ✅ [US1/US2/US3] labels on every user-story-phase task
- ✅ Setup (T001-T003), Polish (T015-T020) — NO story label
- ✅ Every task includes a concrete file path or meta-action descriptor
