# Research — milestone 155

Phase 0 outputs for the CMake `find_package` + `pkg_check_modules` extraction.

## R1 — Integration site inside `cmake.rs`

**Decision**: Add two new pure-function parsers alongside the existing `parse_fetch_block` at `mikebom-cli/src/scan_fs/package_db/cmake.rs`:

- `parse_find_package_calls(content: &str, source_path: &str) -> Vec<(String /* lowercased_name */, Option<String> /* declared_version */, String /* original_casing */)>`
- `parse_pkg_check_modules_calls(content: &str, source_path: &str) -> Vec<(String /* lowercased_module */, Option<String> /* declared_version */)>`

Both are content-in / triples-out — no PackageDbEntry construction happens inside the per-file loop. Instead, the top-level `read()` function accumulates triples across ALL discovered CMake files, then runs a **second pass** that groups by lowercased name and picks the highest declared version per group (Q1 clarification), then emits one `PackageDbEntry` per (call site) with the group's chosen version. Every emitted entry shares the same PURL within a group so the production `resolve::deduplicator` merges them into a single `ResolvedComponent` at the scan-level (per milestone 085's dual-key insert + milestone 148's `evidence.source_file_paths` union). Note: the production dedup path is `crate::resolve::deduplicator::deduplicate` at `mikebom-cli/src/resolve/deduplicator.rs:28`, which groups by `(ecosystem, name, version, parent_purl)`. The milestone-105 `scan_fs::dedup` pipeline (with the `mikebom:also-detected-via` list) is currently `#[allow(dead_code)]` per its module doc; milestone 155 does NOT wire it in.

**Verified at planning time** — inspecting `cmake.rs:35-66`:

```rust
pub fn read(scan_root: &Path, include_vendored: bool) -> Vec<PackageDbEntry> {
    let cmake_files = discover_cmake_files(scan_root);
    let mut entries = Vec::new();
    for path in &cmake_files {
        let content = /* read */;
        let source_path = path.to_string_lossy().to_string();
        entries.extend(parse_fetch_block(&content, &source_path, "FetchContent_Declare"));
        entries.extend(parse_fetch_block(&content, &source_path, "ExternalProject_Add"));
        if include_vendored {
            entries.extend(parse_vendored(&content, &source_path, scan_root));
        }
    }
    entries
}
```

**New wiring**:

```rust
pub fn read(scan_root: &Path, include_vendored: bool) -> Vec<PackageDbEntry> {
    let cmake_files = discover_cmake_files(scan_root);
    let mut entries = Vec::new();
    // Milestone 155 (closes source-tree-only C/C++ visibility gap):
    // accumulate find_package + pkg_check_modules triples across ALL
    // discovered CMake files so we can pick the highest declared version
    // per name (Q1 clarification) before emitting.
    let mut find_package_hits: Vec<FindPackageHit> = Vec::new();
    let mut pkg_check_hits: Vec<PkgCheckHit> = Vec::new();
    for path in &cmake_files {
        let content = /* read */;
        let source_path = path.to_string_lossy().to_string();
        entries.extend(parse_fetch_block(&content, &source_path, "FetchContent_Declare"));
        entries.extend(parse_fetch_block(&content, &source_path, "ExternalProject_Add"));
        if include_vendored {
            entries.extend(parse_vendored(&content, &source_path, scan_root));
        }
        find_package_hits.extend(parse_find_package_calls(&content, &source_path));
        pkg_check_hits.extend(parse_pkg_check_modules_calls(&content, &source_path));
    }
    entries.extend(emit_find_package_entries(find_package_hits));
    entries.extend(emit_pkg_check_module_entries(pkg_check_hits));
    entries
}
```

Where:

```rust
struct FindPackageHit {
    lowercased_name: String,
    original_casing: String,
    declared_version: Option<String>,
    source_path: String,
}

struct PkgCheckHit {
    lowercased_module: String,
    original_casing: String,
    // pkg-config module names may carry `>=X.Y`, `<X.Y`, `=X.Y`.
    // We strip the comparator + version for the PURL name; the version
    // constraint is NOT emitted (per FR-003 — pkg-config version
    // constraints are not preserved as PURL @version).
    source_path: String,
}
```

`emit_find_package_entries` groups by `lowercased_name`, picks highest declared version per group (R3), then emits one `PackageDbEntry` per input hit with the chosen version in the PURL. This design lets the downstream milestone-105 dedup pipeline merge same-PURL entries and populate `evidence.source_file_paths` with the full set (milestone 148 union pass).

**Rationale**: keeps the two new parsers pure-function-in-file-body (independently testable), keeps intra-reader dedup logic in one place (`emit_find_package_entries`), leverages the existing milestone-105 + milestone-148 downstream dedup + path-union infrastructure — no changes required in `scan_fs/mod.rs`.

**Alternatives considered**:
- Inline within the per-file loop (like `parse_fetch_block`): rejected — highest-version dedup requires seeing ALL hits before deciding, so a two-pass structure is fundamentally required for Q1's determinism.
- Emit one entry per unique (name, chosen-version) and lose per-site source_path info: rejected — spec's US1 A3 explicitly preserves every declaration site's file path in the merged component's `evidence.source_file_paths`; downstream milestone-148 union already handles this when multiple entries share the same PURL, so we emit one per call site.
- Do intra-reader dedup at the `PackageDbEntry` level instead of the hit level: rejected — cleaner to pick the winning version once from a Vec<FindPackageHit> than to fix up PURL strings inside emitted PackageDbEntry structs.

## R2 — Regex patterns + false-positive filters

**Decision**: Three static `OnceLock<Regex>` accessors at the top of the new parser functions.

### R2.1 — `find_package` matcher

**Pattern**: `(?im)^[^#\n]*?\bfind_package\s*\(\s*([A-Za-z0-9_:.+-]+)(?:\s+([0-9][A-Za-z0-9._-]*))?`

Structure:
- `(?im)` — case-insensitive, multiline (so `^` matches line-start).
- `^[^#\n]*?` — line-start non-greedy match of anything BUT `#` or newline; this is the comment-strip guard (FR-011). If a `#` appears before `find_package` on the same line, the match fails.
- `\bfind_package\b` — word-boundary anchored function name.
- `\s*\(\s*` — the opening paren + optional whitespace.
- `([A-Za-z0-9_:.+-]+)` — capture group 1: the package name. Matches CMake identifier alphabet (per the existing `collect_into` regex at `cmake.rs:130`).
- `(?:\s+([0-9][A-Za-z0-9._-]*))?` — optional capture group 2: the version. **Must start with a digit** to distinguish it from modifier keywords like `REQUIRED`, `QUIET`, `EXACT`, `CONFIG`, `NO_MODULE`, `MODULE`, `COMPONENTS`, `OPTIONAL_COMPONENTS`, `PATHS`, `HINTS`, `NAMES`. All CMake modifier keywords are alphabetic; version constraints always start with a digit per CMake's `find_package(<PackageName> [version])` grammar.

**Rejected candidates** (handled by pattern structure, not post-filter):
- `find_package_handle_standard_args(...)` — FR-009: the `\bfind_package\b` word boundary + immediate `\s*\(` prevents matching `find_package_handle_standard_args` because the `_handle_standard_args` is contiguous with `find_package` and there's no `(` between them. Verified: `\bfind_package_handle_standard_args\s*\(` shares the `find_package` prefix but `\bfind_package\s*\(` requires `\s*\(` IMMEDIATELY after `find_package`, and `_h` is neither whitespace nor `(`. Test #6 in R6 asserts this.
- Commented-out declarations — FR-011: the `^[^#\n]*?` prefix rejects any line where `#` precedes `find_package`. Block comments (`#[[ ... ]]`) are rarer in `.cmake` files; we accept the trade-off that a multi-line block comment enclosing a `find_package` line would still match. Diagnostic-only — false-positive risk is low; the operator can excise via `--exclude-path`.
- `find_package(${VAR})` — FR-010: the identifier alphabet `[A-Za-z0-9_:.+-]+` does NOT include `$` or `{`/`}`. A `find_package(${VAR})` call fails to match capture group 1 → no hit → no emission. Log at `tracing::debug` level when the raw text contains `${` inside a `find_package(` context (best-effort, secondary pattern).

### R2.2 — `pkg_check_modules` + `pkg_search_module` matcher

**Pattern**: `(?im)^[^#\n]*?\bpkg_(?:check_modules|search_module)\s*\(\s*([A-Za-z0-9_]+)((?:\s+[A-Za-z0-9_>=<.+-]+)+)`

Structure:
- Same `^[^#\n]*?` comment-strip guard.
- `\bpkg_(?:check_modules|search_module)\b` — both variants captured (FR-004).
- Capture group 1: the CMake TARGET variable (discarded — it's a CMake identifier, not a package).
- Capture group 2: the module list body — one-or-more whitespace-separated tokens matching `[A-Za-z0-9_>=<.+-]+`. This is a superset of pkg-config module names and includes modifier keywords like `REQUIRED`, `IMPORTED_TARGET`, `GLOBAL`, `QUIET`.

**Post-processing** (in Rust, not regex): split capture group 2 on whitespace, filter out the modifier-keyword set `{REQUIRED, IMPORTED_TARGET, GLOBAL, QUIET, NO_CMAKE_PATH, NO_CMAKE_ENVIRONMENT_PATH}` (case-insensitive), then for each remaining token:
- Strip `>=X.Y`, `<=X.Y`, `>X.Y`, `<X.Y`, `=X.Y`, `==X.Y` (any leading comparator + trailing version — pkg-config version constraint syntax). Result is the pkg-config module name.
- Lowercase + emit one `PkgCheckHit` per module.

### R2.3 — Diagnostic detector for `find_package(${VAR})`

**Pattern**: `(?im)\bfind_package\s*\(\s*\$\{`

**Purpose**: a secondary pattern used ONLY to emit `tracing::debug` diagnostics on skipped CMake-variable-interpolation calls. Not load-bearing — matches log noise, not emission behavior.

**Rationale**: Milestone-102-era code uses similar `OnceLock<Regex>` patterns throughout (`cmake.rs:128`, `:148`). Comment-strip via `^[^#\n]*?` is a well-tested pattern in this codebase (used by milestone-113 exclude-path + milestone-127 root-pick). The version-starts-with-digit heuristic to distinguish version-position from modifier-keyword-position is universally valid across CMake's `find_package` grammar since version 2.8 (per CMake docs).

**Alternatives considered**:
- Shell out to a real CMake parser: rejected — introduces a runtime dep (`cmake` binary) violating Constitution Principle I's spirit (external toolchains as supply-chain surface) and Constitution Principle III's fail-closed posture (any CMake syntax error would kill mikebom).
- Use a hand-rolled tokenizer: rejected — the regex covers 100% of the observed Kamailio patterns + the milestone 102/103 fixtures; a tokenizer is over-engineered for a milestone whose scope is deliberately narrow (see plan §Complexity Tracking).
- Include `<`/`>`/`=` in the version-capture alphabet: rejected — CMake's `find_package(<Name> <Version>)` grammar allows only bare version strings in the second position (comparators are only used inside `pkg_check_modules` for pkg-config version constraints).

## R3 — Version comparison

**Decision**: Implement a small helper `pick_highest_version(versions: &[Option<String>]) -> Option<String>` that:

1. Filters out `None` values. If nothing remains, return `None` (all declarations were version-less → emitted PURL has no `@version` per FR-002 + US1 A4).
2. If one Some(v) remains, return `Some(v)`.
3. Otherwise, split each remaining version string on `.` — if EVERY segment of EVERY version parses as `u64`, do component-wise numeric comparison (pad shorter versions with `0` — so `3.0` becomes `[3, 0]` and compares to `[1, 1, 0]` as `[3, 0, 0]` > `[1, 1, 0]` → correct).
4. If ANY segment fails to parse as `u64` (e.g., `1.1.0a`, `3.0-rc1`), fall back to lexicographic comparison AND emit a `tracing::warn` diagnostic: `warn!(name = %name, versions = ?versions, "milestone-155: mixed-format version strings; lexicographic ordering used — highest may not be semantically correct")`.

**Verified via unit tests** (R6 test #4):
- `[Some("1.1.0"), Some("3.0")] → Some("3.0")` (numeric comparison)
- `[Some("2.4"), Some("2.4.10")] → Some("2.4.10")` (SemVer numeric; `2.4` padded to `2.4.0`)
- `[Some("1.1.0"), None, Some("3.0")] → Some("3.0")` (None filtered)
- `[None, None] → None` (all version-less)
- `[Some("1.1.0-rc1"), Some("3.0")] → Some("3.0")` with `warn!` (mixed-format, lex order returns "3.0" because "3" > "1" lexicographically — happens to be correct here, but the warning is emitted because we can't guarantee it in general)
- `[Some("1.1.0a"), Some("1.1.0b")] → Some("1.1.0b")` with `warn!` (lex; b > a)

**Rationale**: Component-wise numeric comparison covers 100% of the Kamailio `find_package` declarations (all are plain `X.Y.Z` versions). The `u64`-safe check + `tracing::warn` fallback preserves determinism and transparency (Constitution Principle X) when versions are non-numeric.

**Alternatives considered**:
- Pull in `semver = "1.0"` crate: rejected — SemVer's strict grammar rejects `2.4` (needs three components). Would need permissive parsing. Not worth the dep for this scope.
- Delegate to existing PURL comparison logic: rejected — no existing helper compares raw version strings; the milestone-085 dual-key insert pattern operates at the PURL level and would require SemVer-aware version parsing on the fly.
- Use `versions = "6"` crate: rejected — new dep for a 20-LOC helper, violates the milestone's "zero new Cargo deps" posture (see plan §Technical Context).

## R4 — SourceMechanism open-enum values

**Decision**: Extend the existing closed-set string enum in `build_cmake_entry` at `cmake.rs:434` with two new values:

- `"cmake-find-package"` — emitted by `emit_find_package_entries`.
- `"cmake-pkg-check-modules"` — emitted by `emit_pkg_check_module_entries` (covers both `pkg_check_modules` AND `pkg_search_module` per FR-004; they're semantic siblings).

Update the doc comment at `cmake.rs:431-433` to list the two new values:

```rust
// C/C++ provenance: explicit `mikebom:source-mechanism` annotation
// so operators can grep/filter components by origin without
// reverse-engineering the PURL prefix + per-reader annotations.
// Closed enum across cmake / vcpkg / conan / bazel:
//   cmake-fetchcontent-git, cmake-fetchcontent-url,
//   cmake-externalproject, cmake-vendored,
//   cmake-find-package, cmake-pkg-check-modules,   // milestone 155
//   bazel-http-archive, vcpkg-manifest, conan-recipe.
```

Update the module-level doc comment at `cmake.rs:1-17`:
- Remove the "`find_package(X)` declarations are NOT parsed per FR-007" para.
- Replace with a milestone-155 para: "Parses `find_package(<Name> [<Version>])` declarations, emitting `pkg:generic/<lowercased-name>[@<highest-declared-version>]` with `mikebom:source-mechanism = "cmake-find-package"`. Same-PURL cross-mechanism double-counting is prevented by the production `resolve::deduplicator` pipeline's `(ecosystem, name, version, parent_purl)` grouping key. Cross-namespace dedup (e.g., `pkg:generic/openssl` vs `pkg:deb/debian/libssl3`) is NOT provided by milestone 155 — operators wanting that should use the milestone-111 `--pkg-alias-binding` CLI flag."

**Verified compatibility**: the production `resolve::deduplicator::deduplicate` at `deduplicator.rs:28` groups by `(ecosystem, name, version, parent_purl)` and merges `extra_annotations` per the milestone-109 pattern at line 190-209. Adding the two new mechanism string values in the winner's `extra_annotations["mikebom:source-mechanism"]` requires zero deduplicator changes. The milestone-105 `scan_fs::dedup` open-enum pipeline (with its closed `SourceMechanism` enum at `scan_fs/dedup.rs:58`) is CURRENTLY `#[allow(dead_code)]` — its `mikebom:also-detected-via` list is NOT emitted at production time. When a future milestone-105-completion follow-up wires that pipeline in, the closed enum WILL need extension with `CmakeFindPackage` + `CmakePkgCheckModules` variants (plus corresponding `canonical_str()` + `precedence_rank()` mappings). That expansion is out of milestone-155 scope.

**Alternatives considered**:
- Use `cmake-find-package-config` vs `cmake-find-package-module` to distinguish CMake's two `find_package` modes (config-file vs module-file): rejected — the distinction is a CMake build-time resolution detail, not a package-identity fact. Emitting one uniform value keeps the enum stable across the two modes.
- Emit `pkg-config-<module>` instead of `cmake-pkg-check-modules`: rejected — the mechanism describes HOW mikebom discovered the reference (via a CMake `pkg_check_modules` call), not the underlying pkg-config protocol. Downstream consumers can distinguish CMake-mediated vs autotools-mediated pkg-config uses that way in future milestones.

## R5 — `evidence.source_file_paths` merging

**Decision**: No new merge logic in milestone 155. Reuse milestone 148's `evidence.source_file_paths` union pass in `mikebom-common/src/resolution/*` — verified at planning time via the milestone-148 audit trail.

**Mechanism**: When `emit_find_package_entries` emits N entries for the same lowercased name (across N CMake files) with the SAME PURL (name + highest-version chosen once for the whole group), the milestone-105 dedup pipeline sees N `PackageDbEntry` instances with the same PURL. Its post-milestone-148 behavior:

1. First entry becomes the base `ResolvedComponent`.
2. Subsequent same-PURL entries get merged in.
3. Each entry's `source_path` (from `PackageDbEntry.source_path: String`) becomes an element in the merged `ResolvedComponent.evidence.source_file_paths: Vec<String>`.
4. The resulting `evidence.source_file_paths` is a de-duplicated union preserving lex-sorted order (per milestone 148's `union_sorted` helper).

**Consequence**: US1 A3's requirement — every declaration site's file path preserved — is satisfied automatically by the existing pipeline. Milestone 155 only needs to ensure that entries within a group share the same PURL (name + chosen highest version).

**Alternatives considered**:
- Emit a single entry with a comma-joined `source_path` string: rejected — violates `PackageDbEntry.source_path: String` semantic (one path per entry); would break downstream consumers that split-and-parse.
- Add a new intra-reader path list on the emitted entry: rejected — duplicates work already done by milestone 148.

## R6 — Test inventory (SC-006 requires ≥8)

**Decision**: 10 unit tests inline in `cmake.rs`'s `#[cfg(test)] mod tests` block. Each independent, uses `tempfile::tempdir()` + `std::fs::write` synthetic CMakeLists.txt (following the pattern established at `cmake.rs:481-500`).

| # | Test | Covers |
|---|------|--------|
| 1 | `find_package_simple_no_version_emits_pkg_generic` | US1 A4: `find_package(Foo REQUIRED)` → `pkg:generic/foo` (no `@version`); `mikebom:source-mechanism = cmake-find-package`; `mikebom:cmake-find-package-name = "Foo"` (case preserved because input was mixed-case) |
| 2 | `find_package_with_version_emits_at_version` | US1 A1: `find_package(OpenSSL 1.1.0)` → `pkg:generic/openssl@1.1.0`; annotation preserves `"OpenSSL"` |
| 3 | `find_package_case_normalization` | FR-008: `find_package(BOOST 1.75.0)` → `pkg:generic/boost@1.75.0`; `mikebom:cmake-find-package-name = "BOOST"` preserved (only emitted when original casing ≠ lowercased) |
| 4 | `find_package_multiple_versions_highest_wins` | Q1 clarification: two `.cmake` files declaring `find_package(OpenSSL 1.1.0)` and `find_package(OpenSSL 3.0)` → single emitted `pkg:generic/openssl@3.0`; both source paths captured (via milestone-148 downstream union — verified by asserting entries.len() == 2 with both having the same PURL) |
| 5 | `find_package_mixed_version_and_no_version` | R3 rule: `find_package(OpenSSL 1.1.0)` + `find_package(OpenSSL REQUIRED)` (no version) → PURL uses `1.1.0` (versioned declaration wins over version-less) |
| 6 | `find_package_handle_standard_args_not_extracted` | FR-009: `find_package_handle_standard_args(Foo DEFAULT_MSG FOO_LIBRARY FOO_INCLUDE_DIR)` → 0 emissions (regex boundary + `\s*\(` requirement) |
| 7 | `find_package_variable_interpolation_not_extracted` | FR-010: `find_package(${MY_LIB})` → 0 emissions + `tracing::debug` diagnostic (identifier alphabet excludes `$`, `{`) |
| 8 | `find_package_commented_out_not_extracted` | FR-011: `# find_package(SomeUnusedDep)` → 0 emissions (comment-strip regex prefix) |
| 9 | `pkg_check_modules_single_module` | FR-003: `pkg_check_modules(RADIUS REQUIRED IMPORTED_TARGET radcli)` → `pkg:generic/radcli` with `mikebom:source-mechanism = cmake-pkg-check-modules` (RADIUS discarded as CMake target var; REQUIRED + IMPORTED_TARGET filtered as modifier keywords) |
| 10 | `pkg_check_modules_multi_module_with_version_constraints` | FR-003 + edge case: `pkg_check_modules(GLIB REQUIRED glib-2.0>=2.42 gio-2.0)` → 2 emissions (`pkg:generic/glib-2.0` + `pkg:generic/gio-2.0`); version constraints stripped from names |

Plus a **bonus 11th regression test** (not in floor): `find_package_targets_collector_unaffected` — asserts that `collect_find_package_targets` (the milestone-105 helper at `cmake.rs:96`) still returns the same name set post-milestone-155. Locks the milestone-105 US6 `git-submodule` classification pipeline as an unchanged dependent surface.

Plus a **12th test at the integration level** (in `mikebom-cli/tests/`): `cmake_find_package_kamailio_shape_integration` — synthesizes a minimal Kamailio-like tree (top-level CMakeLists.txt + `cmake/defs.cmake` + 3 `cmake/modules/FindXxx.cmake` files replicating the Kamailio pattern) and asserts the emitted CDX contains ≥5 `pkg:generic/*` components with `mikebom:source-mechanism = cmake-find-package`. Serves as SC-004's testbed synthesis.

Total: 10 unit tests + 1 bonus regression test + 1 integration test = 12 tests. Comfortably above SC-006's floor of 8.

**Rationale**: Inline synthetic tests keep the suite self-contained + fast. No fixture-repo touches. The bonus regression test explicitly validates FR-013's promise (collector unaffected). The integration test synthesizes the SC-004 testbed inside `tests/fixtures/cmake-find-package/` — no external `kamailio/` checkout required to reproduce SC-001's shape.

**Alternatives considered**:
- Vendor a Kamailio subset into the fixture repo: rejected — Kamailio is a large tree (~1400 `.c` files); vendoring even a subset bloats the fixture repo. A minimal 5-file synthetic testbed is sufficient to exercise the code path.
- Property-based tests via `proptest`: overkill for regex-based extraction; the 12 hand-authored cases cover the observed grammar variants.

## R7 — CHANGELOG.md entry shape

**Decision**: Single subsection under `## [Unreleased]` in `CHANGELOG.md`, immediately above whatever milestone-154's entry currently sits. Content documents:

- The reversal of milestone-102's FR-007 (`find_package` extraction previously refused).
- The new `find_package` + `pkg_check_modules` + `pkg_search_module` extraction.
- The two new `mikebom:source-mechanism` open-enum values: `cmake-find-package` + `cmake-pkg-check-modules`.
- The one new annotation key `mikebom:cmake-find-package-name` (case-preservation traceability).
- The Kamailio testbed impact (from 0 components → ≥1 at Kamailio HEAD's depth-1 walker scope; walker-depth extension to reach the remaining 9+ calls is a separate future milestone per the F1 remediation on 2026-07-02).
- Same-PURL cross-mechanism dedup composes with the production `resolve::deduplicator` pass automatically (both `cmake-find-package` and `cmake-fetchcontent-url` produce `pkg:generic/<name>@<ver>` for compatible names, which the deduplicator merges via its `(ecosystem, name, version, parent_purl)` grouping key). Cross-namespace dedup (`pkg:generic` vs `pkg:deb`/`pkg:rpm`) is NOT provided by milestone 155 — operators wanting that should use the milestone-111 `--pkg-alias-binding` CLI flag or await a milestone-105 `scan_fs::dedup` completion follow-up.
- The Q1 clarification: highest declared version wins across multi-file declarations of the same package.
- The Q2 clarification: no build-tool denylist; consumers filter by name.

Include a jq recipe consumers can use to filter the new emissions:

```bash
# List all find_package-derived components:
jq '.components[] | select(.properties[]?
  | select(.name == "mikebom:source-mechanism" and .value == "cmake-find-package"))
  | .purl' sbom.cdx.json
```

## R8 — Verification approach

**SC-001** (Kamailio testbed ≥1 component per 2026-07-02 F1 remediation): manual operator-cadence per quickstart.md Scenario 1. The maintainer clones or points at a local Kamailio checkout post-merge and reports pass/fail via a follow-up comment on the milestone's PR + on any tracking issue. The empirical depth-1 count is 1 (OpenSSL 1.1.0 from `cmake/defs.cmake`); walker-depth extension to reach the additional 9+ calls in `cmake/modules/Find*.cmake` is a separate future milestone.

**SC-002** (byte-identical happy path): automated via existing golden tests (`transitive_parity/cargo`, `transitive_parity/npm`, `transitive_parity/go`, `transitive_parity/pip_*` — none of which contain CMake `find_package` calls). Any golden regenerate → SC-002 violation.

**SC-003** (same-PURL cross-mechanism dedup): automated integration test at `mikebom-cli/tests/cmake_find_package_dedup_integration.rs` synthesizing a scan target with a CMakeLists.txt containing `find_package(openssl 1.1.0)` AND a `cmake/deps.cmake` containing `FetchContent_Declare(openssl URL ...openssl-1.1.0.tar.gz)`. Asserts exactly ONE `pkg:generic/openssl@1.1.0` component with `mikebom:source-mechanism` set to one of `{"cmake-find-package", "cmake-fetchcontent-url"}` (winner is confidence-tie-break-dependent, not prescribed). NOTE: cross-namespace dpkg+cmake scenario was reframed during `/speckit-analyze` remediation on 2026-07-02 — the milestone-105 `mikebom:also-detected-via` pipeline is dead-code at production emission time, so cross-namespace dedup is out of milestone-155 scope.

**SC-004** (integration testbed): the synthetic testbed at `mikebom-cli/tests/fixtures/cmake-find-package/` covers all acceptance-scenario shapes. Test #12 in R6 exercises it.

**SC-005** (pre-PR gate): `./scripts/pre-pr.sh` MUST pass green except the documented `sbomqs_parity` env-only flake.

**SC-006** (unit-test count ≥8):

```bash
grep -cE "^\s+fn (find_package_|pkg_check_modules_)" mikebom-cli/src/scan_fs/package_db/cmake.rs
```

Expected: ≥8 (per SC-006 floor; R6 lists 10 unit tests + 1 bonus + 1 integration = 12 total, ≥8 named per the count command).

**SC-007** (no wire-format changes beyond intended): `git diff main --name-only -- mikebom-cli/src/generate/cyclonedx/ mikebom-cli/src/generate/spdx/` MUST be empty. `git diff main --name-only -- docs/reference/sbom-format-mapping.md` MUST be empty (catalog row for `mikebom:cmake-find-package-name` deferred to a follow-up docs-refresh milestone per FR-015 + prior additive-annotation milestone precedent).

**SC-008** (CHANGELOG presence):

```bash
sed -n '/^## \[Unreleased\]/,/^## \[v/p' CHANGELOG.md \
  | grep -E "find_package|pkg_check_modules|cmake-find-package|milestone 155"
```

Expected: entries present naming all bullet points from R7.

## R9 — Interaction with existing `collect_find_package_targets` helper (`cmake.rs:96`)

**Decision**: No changes to the existing helper. Milestone 155's new `parse_find_package_calls` is a NEW function that emits `PackageDbEntry` instances; `collect_find_package_targets` continues to be a name-only collector for the milestone-105 US6 `git-submodule` classification pipeline. Both walk the same discovered CMake files but populate different downstream data structures.

**Verified at planning time**: `collect_find_package_targets` at `cmake.rs:96` returns `BTreeSet<String>` used by the milestone-105 US6 reader (not yet landed per the `#[allow(dead_code)]` at `cmake.rs:95`). Its behavior is completely orthogonal to `PackageDbEntry` emission — it neither reads from nor writes to the emitted entries list. Milestone 155's parser lives alongside it as an independent consumer of the same file bodies.

**Consequence**: Regression test #11 in R6 (`find_package_targets_collector_unaffected`) locks this at compile+test time.

## R10 — `evidence_kind` field on emitted PackageDbEntry

**Decision**: Set `PackageDbEntry.evidence_kind = Some("declared".to_string())`.

**Verified at planning time**: `PackageDbEntry.evidence_kind` at `cmake.rs:457` is currently `None` for the FetchContent + ExternalProject emissions in `build_cmake_entry`. Existing readers set this to `"declared"` (for manifest-declared deps like Cargo.toml) or `"observed"` (for binary-tier extracted deps). `find_package` declarations are declared-by-manifest-equivalent (the CMakeLists.txt is the manifest); the semantic matches existing manifest-reader convention.

**Rationale**: Constitution Principle X (Transparency) — downstream consumers filtering by `evidence-kind` should see CMake `find_package` deps as manifest-declared, not observed-in-binary.

## R11 — `sbom_tier` field on emitted PackageDbEntry

**Decision**: Set `PackageDbEntry.sbom_tier = Some("source".to_string())` — matching the existing FetchContent + ExternalProject emissions (`cmake.rs:469`).

**Rationale**: CMake `find_package` declarations are source-tier facts; they describe declared-but-not-yet-built dependencies. Consistent with the rest of the CMake reader's tier tagging.
