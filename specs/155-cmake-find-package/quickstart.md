# Quickstart — milestone 155

Validation walkthrough for the CMake `find_package` + `pkg_check_modules` extraction. SC-001 is the manual operator-cadence testbed (Kamailio); the remaining SCs are automated.

## Scenario 1 — SC-001 Kamailio testbed lower-bound (MANUAL operator-cadence)

After the milestone-155 PR merges, the maintainer (or any reviewer with a Kamailio checkout) runs:

```bash
# 1. Build mikebom at milestone-155 HEAD:
cargo +stable build --release -p mikebom

# 2. Ensure a Kamailio checkout is available. If none exists locally:
git clone --depth 1 https://github.com/kamailio/kamailio /tmp/kamailio

# 3. Run mikebom:
./target/release/mikebom sbom scan \
    --path /tmp/kamailio \
    --format cyclonedx-json \
    --output cyclonedx-json=/tmp/mikebom-m155/kamailio.cdx.json

# 4. Count identified components (excluding file-tier orphans):
jq '[.components[] | select(.properties[]?
      | select(.name == "mikebom:source-mechanism"
               and (.value == "cmake-find-package"
                    or .value == "cmake-pkg-check-modules")))] | length' \
    /tmp/mikebom-m155/kamailio.cdx.json
```

**Expected output**: an integer ≥ 1. Per the walker-scope-honest SC-001 floor set 2026-07-02 during `/speckit-analyze` remediation, the empirical depth-1 Kamailio count is 1 (`OpenSSL 1.1.0` from `cmake/defs.cmake`). Deeper `find_package` calls in `cmake/modules/Find*.cmake` are at depth-2 and NOT walked by the existing `discover_cmake_files` helper; extending the walker is a separate future milestone.

```bash
# 5. Enumerate the identified deps + their source mechanisms:
jq '[.components[] | select(.properties[]?
      | select(.name == "mikebom:source-mechanism"
               and (.value == "cmake-find-package"
                    or .value == "cmake-pkg-check-modules")))
    | {name: .name, purl: .purl, mechanism: (.properties[]
        | select(.name == "mikebom:source-mechanism") | .value)}]
    | sort_by(.name)' \
    /tmp/mikebom-m155/kamailio.cdx.json
```

**Expected shape** (Kamailio HEAD 2026-07-02, depth-1 only):
```json
[
  {"name": "openssl", "purl": "pkg:generic/openssl@1.1.0", "mechanism": "cmake-find-package"}
]
```

If this returns ≥1 entry (with OpenSSL at minimum) → ✅ SC-001 PASS. Report PASS in the PR comments + on any tracking issue.

**Deferred to walker-extension follow-up milestone** (would appear once `discover_cmake_files` walks depth-2):
```json
[
  "erlang", "ldap", "libev", "libfreeradiusclient", "mariadbclient",
  "netsnmp", "oracle", "radcli", "radius", "unistring"
]
```

The above 9-10 names come from Kamailio's `cmake/modules/Find*.cmake` files at depth-2. Milestone 155 delivers the parsing code path; a walker-depth-extension milestone would deliver Kamailio-tree reach.

## Scenario 2 — SC-002 byte-identical happy path (automated)

```bash
cargo +stable test --workspace
```

**Expected**: every test passes except the documented `sbomqs_parity::sbomqs_spdx_score_meets_or_beats_cdx_across_ecosystems` env-only flake. Specifically:

- All milestone-090 fixture testbed tests (`transitive_parity/cargo`, `transitive_parity/npm`, `transitive_parity/go`, `transitive_parity/pip_*`) pass without golden regeneration — none of those testbeds contain CMake `find_package` calls in the fixture, so their emitted SBOMs are byte-identical to pre-milestone-155.

Any golden test failure → SC-002 regression (indicates emissions leaked through code paths not exercised by the intended CMake-only extraction).

## Scenario 3 — SC-003 same-PURL cross-mechanism dedup (automated)

```bash
cargo +stable test --workspace --test cmake_find_package_dedup_integration
```

**Expected**: the test passes. It synthesizes a scan target with:
- `CMakeLists.txt` (depth-0 discoverable) containing `find_package(openssl 1.1.0)`
- `cmake/deps.cmake` (depth-1 discoverable) containing `FetchContent_Declare(openssl URL https://example.com/openssl-1.1.0.tar.gz)`

And asserts:
- Exactly ONE component with PURL `pkg:generic/openssl@1.1.0` in the emitted CDX.
- That component's `mikebom:source-mechanism` property is one of `{"cmake-find-package", "cmake-fetchcontent-url"}` (either is acceptable — `resolve::deduplicator`'s confidence-based tie-break is opaque and not prescribed by the spec).
- The component's evidence records BOTH source-file paths (top-level `CMakeLists.txt` AND `cmake/deps.cmake`), verifying the milestone-148 source-file-paths union pass composes correctly with the milestone-155 emissions.

**NOT asserted**: the presence of a `mikebom:also-detected-via` property. That annotation belongs to the milestone-105 `scan_fs::dedup` pipeline which is currently `#[allow(dead_code)]` at production emission time. Wiring it is a milestone-105-completion follow-up. Milestone 155's US2 is intentionally scoped to the same-PURL merge behavior that the production `resolve::deduplicator` already handles.

## Scenario 4 — SC-004 integration testbed (automated)

```bash
cargo +stable test --workspace --test cmake_find_package_kamailio_shape_integration
```

**Expected**: the test passes. It exercises the minimal Kamailio-shape synthetic fixture at `mikebom-cli/tests/fixtures/cmake-find-package/kamailio-shape/` and asserts:

- ≥5 `pkg:generic/*` components emitted with `mikebom:source-mechanism = cmake-find-package`.
- Each component's `evidence.occurrences[].location` matches the fixture file where the declaration appeared.
- The output SBOM parses cleanly under CDX 1.6, SPDX 2.3 JSON schema, AND SPDX 3.0.1 JSON schema (via the milestone-078 `spdx3-validate` harness).

## Scenario 5 — SC-005 pre-PR gate (mandatory)

```bash
./scripts/pre-pr.sh
```

**Expected**: green except the documented `sbomqs_parity` env-only flake.

**Do not open a PR without this passing.** Per Constitution "Pre-PR Verification (MANDATORY)" at `.specify/memory/constitution.md:450-480`: a PR that has not passed both `cargo +stable clippy --workspace --all-targets -- -D warnings` AND `cargo +stable test --workspace` locally MUST NOT be opened.

## Scenario 6 — SC-006 unit-test count (automated)

```bash
grep -cE "^\s+fn (find_package_|pkg_check_modules_)" mikebom-cli/src/scan_fs/package_db/cmake.rs
```

**Expected**: ≥8 (per SC-006 floor; research.md §R6 lists 10 unit tests + 1 bonus regression + 1 integration test).

## Scenario 7 — SC-007 wire-format guard (manual diff check)

```bash
# Only these files should change:
git diff main --name-only
```

**Expected file list** (order may vary):
- `CHANGELOG.md`
- `CLAUDE.md` (auto-updated by speckit plan)
- `mikebom-cli/src/scan_fs/package_db/cmake.rs` (primary deliverable)
- `mikebom-cli/tests/cmake_find_package_kamailio_shape_integration.rs` (new integration test)
- `mikebom-cli/tests/cmake_find_package_dedup_integration.rs` (new integration test)
- `mikebom-cli/tests/fixtures/cmake-find-package/kamailio-shape/**` (new fixture files)
- `specs/155-cmake-find-package/**` (speckit branch artifacts)

**Prohibited changes** (must be empty):
```bash
# No CycloneDX emitter changes:
git diff main --name-only -- mikebom-cli/src/generate/cyclonedx/
# Expected: (empty)

# No SPDX 2.3 or SPDX 3 emitter changes:
git diff main --name-only -- mikebom-cli/src/generate/spdx/
# Expected: (empty)

# No catalog / mapping changes:
git diff main --name-only -- docs/reference/sbom-format-mapping.md
# Expected: (empty)

# No other reader changes:
git diff main --name-only -- mikebom-cli/src/scan_fs/package_db/ \
  | grep -v "cmake.rs"
# Expected: (empty)

# No mikebom-common or mikebom-ebpf changes:
git diff main --name-only -- mikebom-common/ mikebom-ebpf/
# Expected: (empty)
```

## Scenario 8 — SC-008 CHANGELOG presence (manual)

```bash
sed -n '/^## \[Unreleased\]/,/^## \[v/p' CHANGELOG.md \
  | grep -E "find_package|pkg_check_modules|cmake-find-package|milestone 155"
```

**Expected**: entries present naming the milestone-102 FR-007 reversal, the new extraction, the new mechanism values, the Kamailio testbed impact, and the milestone-105 dedup pipeline handling.

## Post-merge — operator-cadence external review

SC-001 (Kamailio testbed) is manual per Assumption 6. The maintainer runs the testbed after merge and reports pass/fail via a follow-up comment. SC-002 / SC-003 / SC-004 / SC-005 / SC-006 / SC-007 / SC-008 are automated and verified pre-merge.

## Known deferrals (spec Out of Scope)

- No `Find<Name>.cmake` script content parsing (Assumption 5).
- No `find_package(<Name> COMPONENTS a b c)` sub-component enumeration (Edge Cases).
- No autotools / raw pkg-config `.pc` file parsing.
- No `CMakePresets.json` parsing.
- No new catalog row for `mikebom:cmake-find-package-name` in this milestone (deferred to follow-up docs refresh per FR-015 + SC-007).
- No build-tool denylist / `mikebom:component-role = build-tool` tagging pass (Q2 clarification — deferred to a natural follow-up if operator demand surfaces).
