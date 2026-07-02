---

description: "Task list for milestone 155 — CMake find_package + pkg_check_modules extraction"
---

# Tasks: CMake `find_package` + `pkg_check_modules` extraction (milestone 155)

**Input**: Design documents from `/specs/155-cmake-find-package/`
**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Included. SC-006 requires ≥8 new unit tests; the spec + research inventory 10 unit tests + 1 regression test + 2 integration tests = 13 tests total.

**Organization**: Tasks are grouped by user story. US1 (P1 compliance auditor gets declared deps) is the MVP. US2 (P2 same-PURL cross-mechanism dedup) is additive — it verifies the production `resolve::deduplicator` pipeline correctly merges two same-PURL CMake mechanisms (`find_package` + `FetchContent_Declare` URL) without requiring any new code (only a new integration test).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1 / US2)
- Include exact file paths in descriptions

## Path Conventions

- Primary deliverable: `mikebom-cli/src/scan_fs/package_db/cmake.rs`
- Integration tests: `mikebom-cli/tests/`
- Fixture files: `mikebom-cli/tests/fixtures/cmake-find-package/`
- CHANGELOG: `CHANGELOG.md` at repo root
- No changes to: `mikebom-cli/src/generate/`, other readers, `mikebom-common/`, `mikebom-ebpf/`, `docs/reference/sbom-format-mapping.md`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Baseline verification. No project scaffolding needed — this is a single-file additive change to an existing crate.

- [X] T001 Verify baseline state: `git log -1 --oneline`, confirm branch `155-cmake-find-package`, capture pre-milestone `cmake.rs` LOC (`wc -l mikebom-cli/src/scan_fs/package_db/cmake.rs`), and pre-milestone test count (`grep -cE "^\s+fn " mikebom-cli/src/scan_fs/package_db/cmake.rs`) so post-implementation delta can be reported.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared struct + helper definitions used by both `find_package` and `pkg_check_modules` extraction paths in US1. The `read()` refactor lives in US1 because it's the load-bearing behavioral change.

**⚠️ CRITICAL**: T002 + T003 must complete before US1 work begins. They edit the same file (`cmake.rs`) so they are sequential.

- [X] T002 Add private struct definitions `FindPackageHit` (fields per data-model.md §1: `lowercased_name`, `original_casing`, `declared_version: Option<String>`, `source_path`) and `PkgCheckHit` (fields: `lowercased_module`, `original_casing`, `source_path`) at the top of `mikebom-cli/src/scan_fs/package_db/cmake.rs` between the `use` block (line ~33) and the `pub fn read` declaration (line ~35). Both structs are module-private (`struct`, no `pub`).

- [X] T003 Implement helper function `fn pick_highest_version(versions: &[Option<String>]) -> Option<String>` in `mikebom-cli/src/scan_fs/package_db/cmake.rs`, placed after the two struct definitions from T002 and before `pub fn read`. Implementation per research §R3: filter out `None`; if empty, return `None`; if all remaining versions' dot-separated segments parse as `u64`, do component-wise numeric comparison with zero-padding for shorter versions; otherwise fall back to lexicographic ordering and emit `tracing::warn!(versions = ?versions, "milestone-155: mixed-format version strings; lexicographic ordering used")`. Must handle single-Some case (return that Some directly) as a fast path.

**Checkpoint**: Foundation ready — US1 implementation can now begin.

---

## Phase 3: User Story 1 — Compliance auditor scanning a C/C++ source tree gets declared deps (Priority: P1) 🎯 MVP

**Goal**: `find_package` + `pkg_check_modules` + `pkg_search_module` declarations from CMake files emit `PackageDbEntry` instances tagged with the appropriate `mikebom:source-mechanism` value; the production `resolve::deduplicator` pipeline merges same-canonical-PURL entries automatically via its 4-tuple grouping key.

**Independent Test**: Run the SC-004 integration test (T014); it exercises a Kamailio-shaped fixture and asserts ≥5 `pkg:generic/*` components emitted with `cmake-find-package` mechanism. Additionally, the 10 in-module unit tests exercise every edge case.

### Tests for User Story 1 (TDD — written before implementation)

> **NOTE**: Per the constitutional TDD convention, tests are added first + committed with the empty function skeletons; they fail; then implementation lands and turns them green. In practice for this milestone: T004+T005 add both the test bodies AND the function skeletons in one commit-block since the tests depend on function signatures existing to compile at all.

- [X] T004 [US1] Add 10 unit test bodies inside the existing `#[cfg(test)] mod tests` block in `mikebom-cli/src/scan_fs/package_db/cmake.rs` (starting near line 476), following the pattern established by `fetchcontent_github_emits_pkg_github` at line ~488. The 10 tests per research §R6 table:
  1. `find_package_simple_no_version_emits_pkg_generic` — synthesize `find_package(Foo REQUIRED)` → assert 1 entry with PURL `pkg:generic/foo`, `mikebom:source-mechanism = "cmake-find-package"`, `mikebom:cmake-find-package-name = "Foo"`.
  2. `find_package_with_version_emits_at_version` — `find_package(OpenSSL 1.1.0)` → PURL `pkg:generic/openssl@1.1.0`, annotation preserves `"OpenSSL"`.
  3. `find_package_case_normalization` — `find_package(BOOST 1.75.0)` → PURL `pkg:generic/boost@1.75.0`, `mikebom:cmake-find-package-name = "BOOST"`.
  4. `find_package_multiple_versions_highest_wins` — two files, one with `find_package(OpenSSL 1.1.0)` and one with `find_package(OpenSSL 3.0)` → both entries emitted with PURL `pkg:generic/openssl@3.0` so downstream milestone-148 union merges them.
  5. `find_package_mixed_version_and_no_version` — `find_package(OpenSSL 1.1.0)` + `find_package(OpenSSL REQUIRED)` → PURL uses `1.1.0` (versioned wins).
  6. `find_package_handle_standard_args_not_extracted` — `find_package_handle_standard_args(Foo DEFAULT_MSG FOO_LIBRARY FOO_INCLUDE_DIR)` alone → assert emitted count for `cmake-find-package` mechanism is 0.
  7. `find_package_variable_interpolation_not_extracted` — `find_package(${MY_LIB})` → 0 emissions.
  8. `find_package_commented_out_not_extracted` — `# find_package(SomeUnusedDep)` → 0 emissions.
  9. `pkg_check_modules_single_module` — `pkg_check_modules(RADIUS REQUIRED IMPORTED_TARGET radcli)` → 1 entry with PURL `pkg:generic/radcli`, mechanism `cmake-pkg-check-modules`. RADIUS target var + REQUIRED + IMPORTED_TARGET filtered out.
  10. `pkg_check_modules_multi_module_with_version_constraints` — `pkg_check_modules(GLIB REQUIRED glib-2.0>=2.42 gio-2.0)` → 2 entries `pkg:generic/glib-2.0` + `pkg:generic/gio-2.0` (version constraints stripped from names).

  Each test uses the existing `tempfile::tempdir()` + `std::fs::write` pattern. All 10 test functions live in the same `mod tests` block.

- [X] T005 [US1] Add 4 more test bodies to the same `#[cfg(test)] mod tests` block:
  - `find_package_targets_collector_unaffected` — creates a fixture with `find_package(Foo)` + `add_library(Foo::Foo ALIAS foo)`, calls `collect_find_package_targets(scan_root)`, asserts the returned `BTreeSet<String>` contains both `foo` and `foo` (dedup, per the existing collector behavior at `cmake.rs:171-190`). Locks the milestone-105 US6 helper as an unchanged dependent surface (research §R9).
  - `find_package_all_lowercase_no_annotation` — `find_package(zlib 1.2.11)` → assert the emitted entry does NOT carry `mikebom:cmake-find-package-name` in `extra_annotations` (per contracts/mikebom-cmake-find-package-name.md conditional emission rule).
  - `find_package_pkg_search_module_alias` — `pkg_search_module(ZLIB REQUIRED zlib)` → assert 1 entry with PURL `pkg:generic/zlib` and mechanism `cmake-pkg-check-modules` (per FR-004 covering the sibling macro).
  - `find_package_modifier_keywords_ignored` (F4 remediation) — `find_package(Boost 1.75.0 NO_MODULE COMPONENTS system filesystem thread PATHS /usr/local/lib)` → assert exactly 1 entry emitted with PURL `pkg:generic/boost@1.75.0` (name + version captured, modifier keywords + COMPONENTS list + PATHS all ignored via the name-capture-until-whitespace + version-must-start-with-digit regex behavior — no sub-component emission per spec Edge Cases + FR-002).

### Implementation for User Story 1

- [X] T006 [US1] Implement `fn parse_find_package_calls(content: &str, source_path: &str) -> Vec<FindPackageHit>` in `mikebom-cli/src/scan_fs/package_db/cmake.rs`. Use a `static FIND_PACKAGE_V155: OnceLock<Regex>` with pattern per research §R2.1: `(?im)^[^#\n]*?\bfind_package\s*\(\s*([A-Za-z0-9_:.+-]+)(?:\s+([0-9][A-Za-z0-9._-]*))?`. For each capture: name (group 1) becomes both `lowercased_name` (via `.to_lowercase()`) + `original_casing` (as-is); version (group 2) becomes `declared_version: Option<String>`. Do NOT dedup at this layer — hits are collected first, dedup happens in T008.

- [X] T007 [US1] Implement `fn parse_pkg_check_modules_calls(content: &str, source_path: &str) -> Vec<PkgCheckHit>` in `mikebom-cli/src/scan_fs/package_db/cmake.rs`. Use a `static PKG_CHECK: OnceLock<Regex>` with pattern per research §R2.2: `(?im)^[^#\n]*?\bpkg_(?:check_modules|search_module)\s*\(\s*([A-Za-z0-9_]+)((?:\s+[A-Za-z0-9_>=<.+-]+)+)`. Extract capture group 2 (the module list body), split on whitespace, filter out modifier-keyword set `{REQUIRED, IMPORTED_TARGET, GLOBAL, QUIET, NO_CMAKE_PATH, NO_CMAKE_ENVIRONMENT_PATH}` (case-insensitive via `.to_ascii_uppercase()`). For each remaining token, strip trailing `>=X.Y` / `<=X.Y` / `>X.Y` / `<X.Y` / `=X.Y` / `==X.Y` via a second small regex `^([A-Za-z0-9_.+-]+)([<>=]=?.*)?$` — the first capture is the module name. Lowercase + emit one `PkgCheckHit` per module. Also add a secondary `static FIND_PACKAGE_VAR_DIAGNOSTIC: OnceLock<Regex>` with pattern `(?im)\bfind_package\s*\(\s*\$\{` used only to emit `tracing::debug!` diagnostics (research §R2.3); place this diagnostic emission at the end of `parse_find_package_calls` (T006).

- [X] T008 [US1] Implement `fn emit_find_package_entries(hits: Vec<FindPackageHit>) -> Vec<PackageDbEntry>` in `mikebom-cli/src/scan_fs/package_db/cmake.rs`. Group hits by `lowercased_name` into a `BTreeMap<String, Vec<FindPackageHit>>`. For each group: call `pick_highest_version(...)` (T003) over the group's `declared_version` values to determine the winning version. Then, for EACH hit in the group, emit ONE `PackageDbEntry` using `build_cmake_entry` (existing helper at `cmake.rs:408`) with: `name = hit.lowercased_name`, `version = winner or ""`, `source_path = hit.source_path`, `purl = Purl::new("pkg:generic/<lowercased>[@<winner>]")`, `download_url = None`, `sha256_hex = None`, `_vendored = false`, `source_mechanism = "cmake-find-package"`. After the `build_cmake_entry` call, if `hit.original_casing.to_lowercase() != hit.original_casing` (i.e., the input had mixed/upper case), insert `"mikebom:cmake-find-package-name" -> json!(hit.original_casing)` into the returned entry's `extra_annotations`. Also, when `hit.declared_version.is_some() && hit.declared_version.as_deref() != Some(winner)` (site declared a different version than the group's chosen one), set `entry.raw_version = Some(hit.declared_version.clone().unwrap())` per data-model.md §2 to preserve per-site version forensics. Additionally set `entry.evidence_kind = Some("declared".to_string())` per research §R10.

- [X] T009 [US1] Implement `fn emit_pkg_check_module_entries(hits: Vec<PkgCheckHit>) -> Vec<PackageDbEntry>` in `mikebom-cli/src/scan_fs/package_db/cmake.rs`. No version-dedup needed (pkg-config version constraints are stripped in T007). Emit one `PackageDbEntry` per hit via `build_cmake_entry` with: `name = hit.lowercased_module`, `version = ""`, `purl = Purl::new("pkg:generic/<lowercased>")`, `source_mechanism = "cmake-pkg-check-modules"`. Do NOT emit `mikebom:cmake-find-package-name` annotation for pkg_check_modules emissions (contracts/mikebom-cmake-find-package-name.md exclusion). Set `entry.evidence_kind = Some("declared".to_string())`.

- [X] T010 [US1] Refactor `pub fn read` in `mikebom-cli/src/scan_fs/package_db/cmake.rs` to two-pass structure per research §R1: within the per-file loop, additionally call `find_package_hits.extend(parse_find_package_calls(&content, &source_path))` and `pkg_check_hits.extend(parse_pkg_check_modules_calls(&content, &source_path))`; declare both accumulators (`let mut find_package_hits: Vec<FindPackageHit> = Vec::new(); let mut pkg_check_hits: Vec<PkgCheckHit> = Vec::new();`) before the loop. After the loop closes, call `entries.extend(emit_find_package_entries(find_package_hits))` and `entries.extend(emit_pkg_check_module_entries(pkg_check_hits))`. Preserve all existing FetchContent / ExternalProject / vendored extraction calls unchanged.

- [X] T011 [US1] Update the module-level doc comment at `mikebom-cli/src/scan_fs/package_db/cmake.rs:1-23`. Remove the para at lines 15-17 stating "`find_package(X)` declarations are NOT parsed per FR-007 — they resolve to system-installed packages and would double-count against OS-package readers + vcpkg + Conan." Replace with a milestone-155 para: "Parses `find_package(<Name> [<Version>])` declarations, emitting `pkg:generic/<lowercased-name>[@<highest-declared-version>]` with `mikebom:source-mechanism = \"cmake-find-package\"`. Multi-file same-name declarations are consolidated to the highest declared version per milestone 155. Cross-tier double-counting is prevented by the milestone-105 dedup pipeline via the `mikebom:also-detected-via` annotation." Also add a bullet listing the `pkg_check_modules` + `pkg_search_module` extraction with mechanism `cmake-pkg-check-modules`.

- [X] T012 [US1] Update the doc comment inside `build_cmake_entry` at `mikebom-cli/src/scan_fs/package_db/cmake.rs:427-437`. Extend the closed-enum comment to include the two new mechanism values:
  ```rust
  // Closed enum across cmake / vcpkg / conan / bazel:
  //   cmake-fetchcontent-git, cmake-fetchcontent-url,
  //   cmake-externalproject, cmake-vendored,
  //   cmake-find-package, cmake-pkg-check-modules,   // milestone 155
  //   bazel-http-archive, vcpkg-manifest, conan-recipe.
  ```

- [X] T013 [P] [US1] Create SC-004 fixture directory `mikebom-cli/tests/fixtures/cmake-find-package/kamailio-shape/` containing:
  - `CMakeLists.txt` — top-level with `find_package(OpenSSL 1.1.0 REQUIRED)`, one `find_package(Threads REQUIRED)` (build-tool emitted uniformly per Q2), one `find_package(ZLIB REQUIRED)` (5th find_package call — added during F8 remediation to satisfy T014's ≥5 assertion), and `include(cmake/defs.cmake)`.
  - `cmake/defs.cmake` — declaring `find_package(Libev)`, `find_package(NETSNMP)`, and one `pkg_check_modules(RADIUS REQUIRED IMPORTED_TARGET radcli)`.
  - `cmake/modules/FindLibev.cmake` — placeholder `Find<Name>.cmake` script containing `find_package_handle_standard_args(Libev DEFAULT_MSG LIBEV_LIBRARY LIBEV_INCLUDE_DIR)` (asserts FR-009 negative — must NOT emit a component). Note: since discovery is depth-1 on `cmake/`, `Modules/`, `third_party/`, files under `cmake/modules/` will NOT be discovered by the existing `discover_cmake_files` helper at `cmake.rs:195` — the fixture must therefore keep the primary find_package calls in `CMakeLists.txt` + `cmake/defs.cmake`. The `cmake/modules/FindLibev.cmake` file exists only for documentation of the Kamailio-shape tree; it will not be walked. (If future milestone extends the walk depth, this fixture becomes load-bearing for FR-009 too.)
  - `src/main.c` — trivial `int main(void) { return 0; }` — makes the tree plausibly a real C project.

  Depth-1-walked find_package call count: 5 (OpenSSL, Threads, ZLIB, Libev, NETSNMP). Depth-1-walked pkg_check_modules call count: 1 (radcli). Satisfies T014's ≥5 cmake-find-package assertion + ≥1 cmake-pkg-check-modules assertion.

- [X] T014 [P] [US1] Create SC-004 integration test at `mikebom-cli/tests/cmake_find_package_kamailio_shape_integration.rs`. Test invokes `mikebom sbom scan --path <fixture> --format cyclonedx-json` (via `Command::new(env!("CARGO_BIN_EXE_mikebom"))` following the milestone-101 test pattern) against the T013 fixture, then parses the emitted CDX JSON. Asserts: (a) at least 5 components in `.components[]` with a property `mikebom:source-mechanism = "cmake-find-package"`; (b) at least 1 component with mechanism `cmake-pkg-check-modules`; (c) the OpenSSL component has PURL `pkg:generic/openssl@1.1.0` and property `mikebom:cmake-find-package-name = "OpenSSL"`; (d) the RADCLI component has PURL `pkg:generic/radcli` (no `@version`, no `mikebom:cmake-find-package-name` since it came from `pkg_check_modules`); (e) the CDX parses cleanly under CDX 1.6 (existing serde-based validation).

**Checkpoint**: At this point, US1 is fully functional. `mikebom sbom scan` against Kamailio (SC-001 manual test in T020 below) should produce ≥1 identified component at the current depth-1 walker scope (per the 2026-07-02 F1 remediation).

---

## Phase 4: User Story 2 — Same-PURL cross-mechanism dedup produces exactly one component (Priority: P2)

**Goal**: Verify that when the same package is declared via TWO different CMake mechanisms (`find_package` + `FetchContent_Declare` URL) with matching PURL, the production `resolve::deduplicator` pass (grouping by `(ecosystem, name, version, parent_purl)`) merges them into exactly ONE emitted component.

**Independent Test**: Run the SC-003 integration test (T015). No production code changes needed beyond US1's cmake.rs work — the production dedup pipeline (`crate::resolve::deduplicator::deduplicate`) already merges same-canonical-PURL entries via its 4-tuple grouping key.

**Cross-tier scope note**: A cross-namespace scenario (e.g., `pkg:generic/openssl@1.1.0` from cmake + `pkg:deb/debian/libssl3@3.0.14` from dpkg) is EXPLICITLY OUT OF SCOPE per the spec's US2 cross-tier note. The `mikebom:also-detected-via` annotation used by the milestone-105 `scan_fs::dedup` pipeline is NOT emitted by the production `resolve::deduplicator` path (per the `#[allow(dead_code)]` markers at `mikebom-cli/src/scan_fs/dedup.rs`); the wiring is a milestone-105-completion follow-up, not a milestone-155 deliverable.

### Implementation for User Story 2

- [X] T015 [US2] Create SC-003 integration test at `mikebom-cli/tests/cmake_find_package_dedup_integration.rs`. Test synthesizes a scan target with BOTH:
  - `CMakeLists.txt` at the scan root containing `find_package(openssl 1.1.0)` (lowercase 'openssl' name for parity with the FetchContent_Declare emission naming).
  - `cmake/deps.cmake` (depth-1 discoverable) containing `FetchContent_Declare(openssl URL https://example.com/openssl-1.1.0.tar.gz)` — no GIT_REPOSITORY/GIT_TAG (so the existing `parse_fetch_block` path takes the URL branch and emits `pkg:generic/openssl@1.1.0` per `cmake.rs:281-292`'s URL-form logic).

  Invokes `mikebom sbom scan --path <target> --format cyclonedx-json` using `Command::new(env!("CARGO_BIN_EXE_mikebom"))` (matches T014's pattern), parses the emitted CDX, and asserts:
  - Exactly ONE component with `purl = "pkg:generic/openssl@1.1.0"` in `.components[]`.
  - That component's `mikebom:source-mechanism` property is one of `{"cmake-find-package", "cmake-fetchcontent-url"}` (either winner is acceptable per SC-003's non-prescriptive tie-break note).
  - Test tolerates either presence OR absence of a `mikebom:also-detected-via` property (per SC-003 note — presence would indicate the milestone-105 pipeline was wired to production during milestone-155's window; absence is the expected default).
  - The `evidence.occurrences[].location` list (or per-emitter equivalent) contains BOTH source-file paths — the top-level `CMakeLists.txt` AND `cmake/deps.cmake` — verifying the milestone-148 source-file-paths union pass fires correctly across the two mechanisms.

- [X] T016 [US2] Verify — as a plan-time correctness check, NOT a production code change — that `crate::resolve::deduplicator::deduplicate` (at `mikebom-cli/src/resolve/deduplicator.rs:28`) groups `ResolvedComponent` instances by `(ecosystem, name, version, parent_purl)` per its existing behavior, and that its milestone-109 `extra_annotations` merge logic at line 190-209 preserves the winner's `mikebom:source-mechanism` value. Report if any behavioral change appears necessary — this would indicate a research gap. Do NOT modify `deduplicator.rs` in this milestone (per FR-014 no-other-reader-changes intent extended: dedup pipeline is a downstream stage, unchanged).

**Checkpoint**: US2 verified. Same-PURL cross-mechanism dedup composes cleanly with milestone 155's new emissions.

---

## Phase 5: Polish & Cross-Cutting Concerns

- [X] T017 Add CHANGELOG.md entry under `## [Unreleased]` per research §R7 + SC-008. Entry names: (a) the reversal of milestone-102 FR-007; (b) the new `find_package` + `pkg_check_modules` + `pkg_search_module` extraction; (c) the two new `mikebom:source-mechanism` values `cmake-find-package` + `cmake-pkg-check-modules`; (d) the new `mikebom:cmake-find-package-name` annotation key; (e) the Kamailio testbed impact (0 → ≥1 component at depth-1 walker scope per 2026-07-02 F1 remediation; a walker-depth-extension follow-up milestone would raise the count); (f) the production `resolve::deduplicator` handles same-PURL cross-mechanism merges automatically; cross-namespace dedup out of scope; (g) the Q1 clarification (highest declared version wins); (h) the Q2 clarification (no build-tool denylist; consumers filter by name). Include the jq recipe from research §R7 for consumers.

- [X] T018 Run SC-005 pre-PR gate: `./scripts/pre-pr.sh`. Expected: green except the documented `sbomqs_parity::sbomqs_spdx_score_meets_or_beats_cdx_across_ecosystems` env-only flake. Do NOT open a PR until this passes locally per Constitution "Pre-PR Verification (MANDATORY)" at `.specify/memory/constitution.md:450-480`.

- [X] T019 Verify SC-007 wire-format guard via `git diff main --name-only`. Expected files changed (order may vary): `CHANGELOG.md`, `CLAUDE.md`, `mikebom-cli/src/scan_fs/package_db/cmake.rs`, `mikebom-cli/tests/cmake_find_package_kamailio_shape_integration.rs`, `mikebom-cli/tests/cmake_find_package_dedup_integration.rs`, `mikebom-cli/tests/fixtures/cmake-find-package/kamailio-shape/**`, `specs/155-cmake-find-package/**`. Prohibited (must be empty): `mikebom-cli/src/generate/`, `mikebom-cli/src/scan_fs/package_db/` (except cmake.rs), `docs/reference/sbom-format-mapping.md`, `mikebom-common/`, `mikebom-ebpf/`. Run the 5 guard `git diff` commands from quickstart.md Scenario 7 and confirm each returns the expected empty / expected-set result.

- [X] T020 SC-001 manual operator-cadence Kamailio testbed verification per quickstart.md Scenario 1. Clone or point at a local Kamailio checkout (e.g., `git clone --depth 1 https://github.com/kamailio/kamailio /tmp/kamailio`), build mikebom `cargo +stable build --release -p mikebom`, run `./target/release/mikebom sbom scan --path /tmp/kamailio --format cyclonedx-json --output cyclonedx-json=/tmp/mikebom-m155/kamailio.cdx.json`, then run the SC-001 jq recipe to count identified components. Expected: **≥1** (per the walker-scope-honest floor set during `/speckit-analyze` remediation on 2026-07-02; the empirical depth-1 count for Kamailio HEAD is 1 — `OpenSSL 1.1.0` from `cmake/defs.cmake`). Assert the emitted OpenSSL component has PURL `pkg:generic/openssl@1.1.0`, mechanism `cmake-find-package`, and annotation `mikebom:cmake-find-package-name = "OpenSSL"`. Report PASS/FAIL in the PR comments. If Kamailio HEAD has moved additional `find_package` calls to depth-1 files since 2026-07-02, the count may be higher (still passes ≥1). If FAIL (count = 0): investigate whether Kamailio's tree layout has moved OpenSSL out of `cmake/defs.cmake` — file a follow-up regression issue. Do NOT change milestone-155 scope or FR-013 to chase deeper walker depth in this milestone; that is a separate future milestone opportunity per Assumption 6.

- [X] T021 SC-006 verification: run `grep -cE "^\s+fn (find_package_|pkg_check_modules_)" mikebom-cli/src/scan_fs/package_db/cmake.rs` and confirm ≥8. Report the count.

- [X] T022 Update the requirements checklist at `specs/155-cmake-find-package/checklists/requirements.md` to reflect completion: mark the "Ready for `/speckit-plan`" line as "Ready for `/speckit-tasks` and beyond" or add a completion timestamp footer.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1, T001)**: No dependencies. Runs first.
- **Foundational (Phase 2, T002-T003)**: Depends on T001. Sequential (both edit `cmake.rs`).
- **User Story 1 (Phase 3, T004-T014)**: All depend on Phase 2 completion. Within-phase order below.
- **User Story 2 (Phase 4, T015-T016)**: Depends on Phase 3 completion (needs the emitted `cmake-find-package` mechanism value to be flowing through the pipeline to test the dedup). Could run in parallel with the polish tasks in Phase 5.
- **Polish (Phase 5, T017-T022)**: T017 can run at any time after T010 (once the code exists). T018 (pre-PR gate) MUST run last locally before PR. T019-T021 verify various SCs; T022 documentation update. T020 manual Kamailio verification MUST run after T018 (against the built release binary).

### User Story Dependencies

- **US1 (P1)**: Depends on foundational (T002-T003). No dependency on other stories.
- **US2 (P2)**: Depends on US1 completion (T004-T014 must produce the `cmake-find-package` emission for T015's integration test to observe the same-PURL cross-mechanism dedup with `cmake-fetchcontent-url`).

### Within US1

- T004-T005 add tests (TDD). Sequential — both edit the same `mod tests` block in `cmake.rs`.
- T006-T010 implement production code. Sequential — all edit `cmake.rs`.
- T011-T012 doc comment updates in `cmake.rs`. Can bundle with T006-T010 commits.
- T013-T014 add fixture files + integration test. Different files from `cmake.rs`. Parallel to each other; independent of T004-T012's within-cmake.rs ordering.

### Parallel Opportunities

- **Foundational + US1 within cmake.rs**: sequential, single-file conflicts.
- **T013 (fixture creation) + T014 (integration test file)**: both create new files, no dependency between them, can run in parallel.
- **T015 (US2 integration test)**: independent file, can run in parallel with T017 (CHANGELOG update) once US1 is done.
- **T019 + T020 + T021**: independent verification tasks, can be done in any order after T018 succeeds.

---

## Parallel Example: US1 fixture + integration test authoring

Once T012 (production code + doc updates in cmake.rs) is done:

```bash
# These two tasks touch different files with no dependency:
Task T013: Create SC-004 fixture files at mikebom-cli/tests/fixtures/cmake-find-package/kamailio-shape/
Task T014: Create integration test at mikebom-cli/tests/cmake_find_package_kamailio_shape_integration.rs
```

## Parallel Example: Post-implementation polish

Once T014 passes:

```bash
Task T015: US2 dedup integration test (mikebom-cli/tests/cmake_find_package_dedup_integration.rs)
Task T017: CHANGELOG entry (CHANGELOG.md)
```

Both are independent files.

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001 — 5 min baseline capture)
2. Complete Phase 2: Foundational (T002-T003 — struct + helper — ~30 LOC)
3. Complete Phase 3: US1 (T004-T014 — the primary deliverable)
4. **STOP + VALIDATE**: Run `cargo test -p mikebom cmake` to confirm all 13 new tests pass. Run T014 integration test to confirm SC-004 shape.
5. Optional MVP ship: this alone closes the source-tree-only C/C++ visibility gap. US2 adds the same-PURL cross-mechanism dedup verification but doesn't add new emission behavior.

### Incremental Delivery

1. Complete Setup + Foundational → foundation ready.
2. Add US1 → test independently via T014 → SC-004 verified. **MVP shippable.**
3. Add US2 → test independently via T015 → SC-003 verified.
4. Polish (T017-T022) → run pre-PR gate → open PR.

### Suggested commit shape

The milestone author has historically shipped speckit milestones as a 4-commit chain per the CLAUDE.md project convention:

- `spec(155): ...` — spec + clarify session (already committed at 574c9ee-adjacent to the milestone-155 branch head; verify via `git log --oneline`).
- `plan(155): ...` — plan.md + research.md + data-model.md + contracts/ + quickstart.md + CLAUDE.md.
- `tasks(155): ...` — this tasks.md file.
- `impl(155): ...` — T002-T016 production + test code.
- `docs(155): ...` — T017 CHANGELOG + T022 checklist update.
- `pre-pr(155): ...` — pre-PR gate green + PR opened.

---

## Notes

- [P] tasks = different files, no dependencies.
- All `cmake.rs` edits are sequential (same file). Parallelism is limited to fixture + integration test authoring.
- Verify tests compile-fail before implementing (TDD): T004 + T005 add tests that reference `parse_find_package_calls` + `parse_pkg_check_modules_calls` which don't exist yet — the test module will fail to compile until T006 + T007 provide the function signatures.
- Constitution "Pre-PR Verification" is mandatory (T018).
- SC-001 (Kamailio testbed) is manual per Assumption 6 — cannot be automated in-tree without vendoring Kamailio (rejected in research §R6).
- SC-007 (wire-format guard) is a set of `git diff` sanity checks; automate if desired via a shell script but not required to pass the milestone.
- Do NOT touch `docs/reference/sbom-format-mapping.md` in this milestone (catalog row addition deferred per FR-015 + SC-007).
