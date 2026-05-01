---
description: "Task list — milestone 052 native lifecycle-scope dep tagging"
---

# Tasks: Native lifecycle-scope dependency tagging

**Input**: spec.md ✅, plan.md ✅, checklists/requirements.md ✅. (No
research.md / data-model.md / contracts/ / quickstart.md — same
4-file tighter template milestones 047-051 use; plan resolves the
integration-point lookups inline in §Phase 0.)

**Tests**: explicitly requested (SC-009 — at least one new
integration test per ecosystem; plus inline unit tests for the
new helpers and the resolver-pipeline edge-rewrite step).

**Organization**: 5 user stories, 3 commits per the plan. The
commits cross-cut user stories — commit 1 lays groundwork for
US1/US3/US4/US5; commit 2 completes those four; commit 3 ships
US2's CLI flag plus chore. Tasks are organized by commit
(implementation phase) and tagged with all user-story labels they
contribute to.

## Format: `[ID] [P?] [Story?] Description`

---

## Phase 1: Setup

- [ ] T001 Branch `052-lifecycle-dep-scope` already created (via
  /speckit.specify auto-allocation; sits on top of unmerged
  `051-polyglot-dev-tagging`). After 051 lands on main, rebase
  onto main: `git fetch origin main && git rebase origin/main`.
  Resolve any 051-spec-dir conflicts via 051-branch's version.
- [ ] T002 Verify spec.md, plan.md, checklists/requirements.md
  authored. Confirm Q1 → Option C clarification is recorded under
  `## Clarifications` in spec.md.
- [ ] T003 Baseline `./scripts/pre-pr.sh` clean from a fresh
  shell (should pass — no edits yet).

---

## Phase 2: Foundational

(No foundational tasks — every change in this milestone lives in
the data model, the per-ecosystem readers, the resolver pipeline,
the per-format serializers, the parity wiring, and the CLI. The
shared infrastructure — `RelationshipType` enum, `is_dev` field,
extra_annotations bag, parity extractor pattern — is already in
place per Phase 0 R1. This milestone replaces, extends, and
removes pieces of that infrastructure but adds no new
foundational layer.)

---

## Phase 3: Commit `feat(052/us1+us3+us4+us5): LifecycleScope data model + reader migration`

**Goal**: Replace `is_dev: Option<bool>` with
`lifecycle_scope: Option<LifecycleScope>` end-to-end on the model
side; add the `TestDependsOn` relationship variant; update every
reader to set `lifecycle_scope` with the right enum variant. NO
serializer changes yet — they land in commit 2 so this commit can
be reviewed for correctness without churn from goldens regen.

**Independent test**: existing 1200+ unit tests + the integration
suite still pass after this commit (the field rename is internal;
no observable output change yet because serializers still consume
`is_dev` paths that we keep until commit 2 — see T009 for the
shim approach).

### Common-side data model

- [ ] T004 [US1] [US3] [US4] [US5] In
  `mikebom-common/src/resolution.rs`, add a new public enum
  `LifecycleScope { Runtime, Development, Build, Test }` with
  `#[serde(rename_all = "snake_case")]` and the standard derive
  set (`Clone, Debug, PartialEq, Eq, Hash, Serialize,
  Deserialize`). Place it directly above the existing
  `RelationshipType` enum.
- [ ] T005 [US3] In `mikebom-common/src/resolution.rs`, add a
  `TestDependsOn` variant to the `RelationshipType` enum, between
  `BuildDependsOn` and the closing brace.
- [ ] T006 [P] [US1] [US3] [US4] [US5] Add inline unit tests in
  `mikebom-common/src/resolution.rs::tests`: (a) `LifecycleScope`
  serde round-trip for each variant (assert
  `serde_json::to_string(&LifecycleScope::Build)` returns
  `"\"build\""`); (b) `RelationshipType::TestDependsOn` serde
  round-trip.

### Field replacement on common-side ResolvedComponent

- [ ] T007 [US1] [US3] [US4] [US5] In
  `mikebom-common/src/resolution.rs::ResolvedComponent`, replace
  `pub is_dev: Option<bool>` with
  `pub lifecycle_scope: Option<LifecycleScope>`. Update every
  `ResolvedComponent { ... is_dev: ... }` initializer in the
  same file (typically the test fixtures + Default impl).
- [ ] T008 [US1] [US3] [US4] [US5] In
  `mikebom-cli/src/scan_fs/package_db/mod.rs::PackageDbEntry`,
  replace `pub is_dev: Option<bool>` with
  `pub lifecycle_scope: Option<LifecycleScope>` (import
  `LifecycleScope` from `mikebom_common::resolution`). Update
  the ~15 `is_dev: None` initializer sites visible via
  `grep -n 'is_dev:' mikebom-cli/src/scan_fs/package_db/`. The
  initializer for new entries becomes `lifecycle_scope: None`.
- [ ] T009 [US1] [US3] [US4] [US5] In
  `mikebom-cli/src/scan_fs/mod.rs:523`, replace
  `is_dev: entry.is_dev.clone()` with
  `lifecycle_scope: entry.lifecycle_scope.clone()` on the
  PackageDbEntry → ResolvedComponent conversion. Audit the
  function for any other `is_dev` references (per recon there's
  one); update them all.

### Per-reader lifecycle_scope population

- [ ] T010 [US1] [US3] [US4] [US5] In
  `mikebom-cli/src/scan_fs/package_db/cargo.rs`, add a helper
  `compute_cargo_build_set(lock: &CargoLock, direct_build:
  &HashSet<String>) -> HashSet<(String, String)>` (parallel to
  the existing `compute_cargo_prod_set`). Seed BFS from
  `direct_build` (the workspace-wide `[build-dependencies]`
  union from `parse_cargo_toml`); walk lockfile dep edges.
- [ ] T011 [US1] [US3] [US4] [US5] In `cargo.rs::parse_lockfile`,
  modify the per-package emission: replace the existing
  `is_dev: prod_set.contains(...).not().then_some(true)` line
  with a 4-way classifier:
    - In `prod_set` → `LifecycleScope::Runtime`
    - In `build_set` (NEW) → `LifecycleScope::Build`
    - Otherwise (in lock but not in either prod or build set) →
      `LifecycleScope::Development`
  Production-wins-over-build-wins-over-dev priority per FR-005.
  Lockfile-only mode (no Cargo.toml found) keeps
  `lifecycle_scope: None` (can't classify).
- [ ] T012 [US1] [US3] [US4] [US5] In `cargo.rs::read`, update
  the existing post-051 drop logic at line ~610: replace
  `if entry.is_dev == Some(true)` with
  `if !matches!(entry.lifecycle_scope, Some(LifecycleScope::Runtime) | None)`.
  Wire `compute_cargo_build_set` next to `compute_cargo_prod_set`
  and pass both to `parse_lockfile`.
- [ ] T013 [P] [US1] [US3] [US4] [US5] Update inline cargo unit
  tests in `cargo.rs::tests`: (a) add a 4-way classifier test —
  one crate in `[dependencies]` only, one in `[dev-dependencies]`
  only, one in `[build-dependencies]` only, one in BOTH prod and
  dev (production wins); (b) update existing
  `compute_prod_set_*` tests to assert correct interaction with
  the new build-set helper.
- [ ] T014 [US1] [US3] [US4] [US5] In
  `mikebom-cli/src/scan_fs/package_db/gem.rs::read`, replace
  the `is_dev: Some(true)` site with a classifier:
    - Default group → `LifecycleScope::Runtime`
    - Group set contains `"test"` → `LifecycleScope::Test`
    - Other non-default group(s) → `LifecycleScope::Development`
  Update the drop logic at the equivalent of cargo.rs line
  ~610 to drop on non-Runtime when `!include_dev`.
- [ ] T015 [P] [US1] [US3] [US4] [US5] Add inline gem unit
  tests in `gem.rs::tests` covering: `:test` → Test;
  `:development` → Development; `:doc` → Development; default
  → Runtime; gem in `:test` AND default → Runtime
  (production wins).
- [ ] T016 [US1] [US3] [US4] [US5] In
  `mikebom-cli/src/scan_fs/package_db/maven.rs:1823`, replace
  the existing `is_dev: matches!(dep.scope.as_deref(),
  Some("test")).then_some(true)` line with a helper call:
  `lifecycle_scope: lifecycle_scope_from_maven(dep.scope.as_deref())`.
  Add the helper to maven.rs:
  ```rust
  fn lifecycle_scope_from_maven(scope: Option<&str>) -> Option<LifecycleScope> {
      match scope {
          Some("test") => Some(LifecycleScope::Test),
          Some("provided") => Some(LifecycleScope::Build),
          Some("compile") | Some("runtime") | Some("system") | Some("import") | None
              => Some(LifecycleScope::Runtime),
          Some(_) => None,  // Unknown scope value
      }
  }
  ```
  Update the existing pre-emission filter at maven.rs:1786
  (`if !include_dev && matches!(dep.scope.as_deref(), Some("test"))`)
  to instead check
  `!include_dev && !matches!(lifecycle_scope_from_maven(...), Some(LifecycleScope::Runtime) | None)`.
- [ ] T017 [P] [US1] [US3] [US4] [US5] Add inline maven unit
  tests in `maven.rs::tests` for `lifecycle_scope_from_maven`:
  test → Test, provided → Build, compile/runtime/system/import
  → Runtime, None → Runtime, unknown → None.
- [ ] T018 [US1] [US3] [US4] [US5] In
  `mikebom-cli/src/scan_fs/package_db/mod.rs::apply_go_production_set_filter`
  (added in milestone 049), update the tagging block: replace
  `e.is_dev = Some(true)` with
  `e.lifecycle_scope = Some(LifecycleScope::Test)` (Go's
  test-only signal maps to `Test` per FR-008). Update the
  drop predicate at the same function to drop on
  `!matches!(e.lifecycle_scope, Some(Runtime) | None)`.
- [ ] T019 [US1] [US3] [US4] [US5] In
  `mikebom-cli/src/scan_fs/package_db/pip/pipfile.rs:75`,
  replace `is_dev: Some(is_dev)` with
  `lifecycle_scope: if is_dev { Some(LifecycleScope::Development) } else { Some(LifecycleScope::Runtime) }`.
  Audit pip/poetry sibling files
  (`pip/poetry.rs`, `pip/pyproject.rs` if present) for matching
  `is_dev: Some(...)` sites; apply the same transformation.
- [ ] T020 [US1] [US3] [US4] [US5] In
  `mikebom-cli/src/scan_fs/package_db/npm.rs` (and
  `npm/walk.rs` if it has its own emission site), replace the
  `is_dev: Some(true)` site for devDependencies with
  `lifecycle_scope: Some(LifecycleScope::Development)`.
  Production-wins is already enforced in the existing npm
  flattener; preserve that semantic.
- [ ] T021 [US1] [US3] [US4] [US5] In every other
  `mikebom-cli/src/scan_fs/package_db/*.rs` file (apk, dpkg,
  rpm, rpmdb_*, rpm_file, golang.rs source-tree readers,
  go_binary.rs, gem `gemspec_to_entry`, maven cache readers,
  etc.), replace `is_dev: None` with `lifecycle_scope: None`.
  Audit via `grep -rn 'is_dev:' mikebom-cli/src/scan_fs/`.

### Other internal sites with is_dev

- [ ] T022 [US1] [US3] [US4] [US5] Update generation-side
  scaffolding sites with `is_dev: None` (per recon at
  `generate/cpe.rs:185`, `generate/openvex/mod.rs:175`,
  `generate/spdx/mod.rs:289`, `generate/spdx/packages.rs:395`,
  `generate/spdx/relationships.rs:213`,
  `generate/cyclonedx/builder.rs:687`,
  `mikebom-cli/src/scan_fs/package_db/mod.rs:927`). Replace with
  `lifecycle_scope: None`.
- [ ] T023 [US1] [US3] [US4] [US5] In the resolver pipeline
  (`mikebom-cli/src/resolve/`), audit for `is_dev` references via
  `grep -rn is_dev mikebom-cli/src/resolve/`. Update each to use
  `lifecycle_scope` with appropriate variant matching.
- [ ] T024 [US1] [US3] [US4] [US5] Audit
  `mikebom-cli/tests/` integration tests via
  `grep -rn 'is_dev\|mikebom:dev-dependency' mikebom-cli/tests/`.
  These are the integration tests that assert on the legacy
  field/annotation. Note them but **do not change them yet** —
  they get updated in commit 2 alongside the serializer changes.

### Verify + commit

- [ ] T025 [US1] [US3] [US4] [US5] `cargo +stable build -p
  mikebom` clean (no compile errors).
- [ ] T026 [US1] [US3] [US4] [US5] `cargo +stable test -p
  mikebom-common` — common-side unit tests pass with the new
  enum + variant.
- [ ] T027 [US1] [US3] [US4] [US5] `cargo +stable test -p
  mikebom --bin mikebom -- package_db` — per-reader unit tests
  pass (cargo, gem, maven, others).
- [ ] T028 [US1] [US3] [US4] [US5] `cargo +stable test -p
  mikebom --test scan_cargo --test scan_gem --test scan_maven
  --test scan_go --test scan_npm --test scan_python` — per-
  ecosystem integration tests still pass (the legacy
  `mikebom:dev-dependency` annotation is still emitted by the
  serializers, which will be removed in commit 2).
- [ ] T029 [US1] [US3] [US4] [US5] Commit:
  `feat(052/us1+us3+us4+us5): LifecycleScope data model + reader migration`.

---

## Phase 4: Commit `feat(052/us1+us3+us4+us5): native CDX/SPDX 2.3/SPDX 3 emission + edge rewrite`

**Goal**: Serializer changes + resolver-pipeline edge-rewrite
step. This is where output behavior changes — SBOMs gain native
scope tags, the legacy `mikebom:dev-dependency` annotation is
removed, the C6 catalog row is deleted. Goldens regen.

**Independent test**: SC-001 (default scan emits ALL deps with
native scope tags), SC-004 (cargo build-deps → SPDX 2.3
`BUILD_DEPENDENCY_OF` + SPDX 3 `lifecycleScope: "build"`),
SC-005 (maven test → `TEST_DEPENDENCY_OF` + `lifecycleScope:
"test"`), SC-006 (gem `:test` → same), SC-010 (legacy
annotation absent), SC-007 (27 goldens regen cleanly).

### Edge-type rewrite (resolver pipeline)

- [ ] T030 [US3] [US4] In `mikebom-cli/src/scan_fs/mod.rs`, add a
  new helper `apply_lifecycle_scope_to_edges(components: &[ResolvedComponent], relationships: &mut Vec<Relationship>)`.
  For each relationship of type `RelationshipType::DependsOn`,
  look up the target component (by purl/bom-ref); if its
  `lifecycle_scope` is `Some(Development)` → rewrite to
  `RelationshipType::DevDependsOn`; `Some(Build)` →
  `RelationshipType::BuildDependsOn`; `Some(Test)` →
  `RelationshipType::TestDependsOn`. `Some(Runtime)` and `None`
  leave the edge as `DependsOn`. Call this helper after
  `apply_go_cache_zip_filter` (which is the last step that
  modifies the resolved component list).
- [ ] T031 [P] [US3] [US4] Add inline unit tests for
  `apply_lifecycle_scope_to_edges` in `scan_fs/mod.rs::tests`:
  (a) Runtime + None targets leave edges as DependsOn;
  (b) Development target → DevDependsOn; (c) Build target →
  BuildDependsOn; (d) Test target → TestDependsOn;
  (e) target not found in components → leave edge unchanged.

### CycloneDX serializer (US5)

- [ ] T032 [US5] In
  `mikebom-cli/src/generate/cyclonedx/builder.rs:315-318`,
  REMOVE the existing `mikebom:dev-dependency` property emission
  block (the `if self.config.include_dev && component.is_dev ==
  Some(true) { ... "name": "mikebom:dev-dependency", "value":
  "true" ... }` chunk).
- [ ] T033 [US5] In the same builder, ADD new emission logic at
  the same location:
    1. If `component.lifecycle_scope` is `Some(Development)` |
       `Some(Build)` | `Some(Test)`, set
       `serialized["scope"] = "excluded"`.
    2. Emit a `mikebom:lifecycle-scope` property with value
       equal to the lower-cased variant name (`"development"`,
       `"build"`, `"test"`).
    3. `Some(Runtime)` and `None` → omit both the `scope` field
       and the property (CDX default scope is `required`).
  Use the existing properties[] emission helper.

### SPDX 2.3 serializer (US3)

- [ ] T034 [US3] In
  `mikebom-cli/src/generate/spdx/relationships.rs`, extend the
  match at line 134-139 with a `TestDependsOn` arm mapping to
  `Some(SpdxRelationshipType::TestDependencyOf)` (with the same
  reverse-direction behavior as the existing `DevDependsOn` and
  `BuildDependsOn` arms — see lines 73-78 for the documented
  reversal). Verify `SpdxRelationshipType::TestDependencyOf`
  exists in the relationship-type enum; if not, add it.
- [ ] T035 [US3] In `mikebom-cli/src/generate/spdx/annotations.rs:147-148`,
  REMOVE the `mikebom:dev-dependency` annotation emission block
  entirely. The lifecycle scope now travels via the native
  relationship type set in T034.
- [ ] T036 [P] [US3] Extend the existing test at
  `relationships.rs:319-332` (which asserts
  `DEV_DEPENDENCY_OF` direction reversal) to also cover
  `TEST_DEPENDENCY_OF`. Same test pattern; new assertion.

### SPDX 3 serializer (US4)

- [ ] T037 [US4] In
  `mikebom-cli/src/generate/spdx/v3_annotations.rs:163-164`,
  REMOVE the `mikebom:dev-dependency` annotation emission block.
- [ ] T038 [US4] In
  `mikebom-cli/src/generate/spdx/v3_relationships.rs`, extend
  the `dependsOn` emission to set the SPDX 3 `scope` field
  (LifecycleScopeType) when the source relationship's variant is
  non-`DependsOn`:
    - `DevDependsOn` → `"scope": "development"`
    - `BuildDependsOn` → `"scope": "build"`
    - `TestDependsOn` → `"scope": "test"`
    - `DependsOn` → omit `scope` (default = unspecified)
  Note the field name in SPDX 3.0.1 may be `lifecycleScope`
  (per spec) or context-prefixed (`software_lifecycleScope`); the
  existing other fields in this serializer (e.g.,
  `software_primaryPurpose`) use the prefixed form. Verify which
  applies for `LifecycleScopeType` against
  `mikebom-cli/tests/fixtures/spdx-3-schema/...` schema and
  match.
- [ ] T039 [P] [US4] Update the docstring comment at
  `v3_relationships.rs:51-78` documenting the new
  `lifecycleScope` field emission. Remove the "via the C6
  Annotation" reference since C6 is being removed.
- [ ] T040 [P] [US4] Add inline unit tests in
  `v3_relationships.rs::tests` (or as integration tests if no
  unit-test infra exists there) covering the four-way
  scope emission.

### Catalog + parity

- [ ] T041 [US3] [US4] [US5] In
  `docs/reference/sbom-format-mapping.md`, DELETE the C6 row
  (the `mikebom:dev-dependency` row at line 51). Renumber any
  subsequent C-row references in inline prose only if
  necessary; the row IDs themselves (C7 onward) stay stable
  per existing convention.
- [ ] T042 [US5] In `docs/reference/sbom-format-mapping.md`,
  ADD a new C-row C42 for `mikebom:lifecycle-scope` documenting
  the CDX-only finer-grained signal, with explicit native-field
  audit per Constitution Principle V (v1.4.0) — cite that CDX
  1.6 native `scope` enum has only 3 values
  (`required`/`optional`/`excluded`) and cannot express the
  dev/build/test split. Reference the spec at
  `specs/052-lifecycle-dep-scope/spec.md::FR-013`.
- [ ] T043 [US3] [US4] [US5] In
  `docs/reference/sbom-format-mapping.md`, EXTEND row B2
  (dependency edge dev) — or add a new row B3 — to document
  the now-emitted native `DEV/BUILD/TEST_DEPENDENCY_OF` in
  SPDX 2.3 and `LifecycleScopeType` enum in SPDX 3. The
  parallel CDX-side signal is the new
  `mikebom:lifecycle-scope` property (C42) plus
  `scope: "excluded"`.
- [ ] T044 [US3] [US4] [US5] Drop or repurpose the parity
  extractors `cdx_dev_deps`, `spdx23_dev_deps`,
  `spdx3_dev_deps` in `mikebom-cli/src/parity/extractors/`.
  Replace with new extractors:
    - `cdx_lifecycle_scope` — extracts the new
      `mikebom:lifecycle-scope` property values from CDX.
    - `spdx23_dep_type` — extracts the relationship-type
      strings (`DEV_DEPENDENCY_OF`, `BUILD_DEPENDENCY_OF`,
      `TEST_DEPENDENCY_OF`) from SPDX 2.3.
    - `spdx3_lifecycle_scope` — extracts the
      `lifecycleScope` (or context-prefixed equivalent)
      field values from SPDX 3 relationships.
  Wire C42 + B3 into the EXTRACTORS table with
  `Directionality::SymmetricEqual` (matching the precedent
  set by milestone 048's C40 + milestone 050's C41).
- [ ] T045 [US3] [US4] [US5] Verify
  `every_catalog_row_has_an_extractor` test passes after the
  C-row catalog edits (this is the same enforcement test that
  caught milestone 050's missed extractor wiring).

### Goldens regen + verification

- [ ] T046 [US1] [US3] [US4] [US5] Regen all 27 byte-identity
  goldens via
  `MIKEBOM_UPDATE_CDX_GOLDENS=1 MIKEBOM_UPDATE_SPDX_GOLDENS=1
  MIKEBOM_UPDATE_SPDX3_GOLDENS=1 cargo +stable test -p mikebom
  --test cdx_regression --test spdx_regression --test
  spdx3_regression -- --test-threads=1`.
- [ ] T047 [US1] [US3] [US4] [US5] Inspect golden diffs for
  expected shape:
    - dev-affected fixtures (cargo, gem, maven, npm, pip,
      golang) should show new `scope: "excluded"` on dev/build/
      test components in CDX, new `mikebom:lifecycle-scope`
      properties, new `DEV/BUILD/TEST_DEPENDENCY_OF`
      relationships in SPDX 2.3, new `lifecycleScope` fields in
      SPDX 3, AND removal of `mikebom:dev-dependency` annotation.
    - non-dev-affected fixtures (apk, deb, rpm) should diff ONLY
      on tool-version-derived hashes (same shape as a stamp
      bump). If a non-dev fixture's golden shows component-shape
      changes, investigate before regen-ing.
- [ ] T048 [US1] [US3] [US4] [US5] `cargo +stable test -p mikebom
  --test holistic_parity` — 11/11 ok with the new C42 + B3
  rows wired in.

### Integration test updates + new tests per ecosystem

- [ ] T049 [US1] [US3] [US4] [US5] Update existing integration
  tests in `mikebom-cli/tests/scan_*.rs` that assert on the
  legacy `mikebom:dev-dependency` property (per T024's audit).
  Replace assertions like
  `props.iter().any(|p| p["name"].as_str() == Some("mikebom:dev-dependency"))`
  with checks for the new native fields:
    - CDX: `component["scope"].as_str() == Some("excluded")`
    - CDX: `props.iter().any(|p| p["name"].as_str() == Some("mikebom:lifecycle-scope") && p["value"].as_str() == Some("development"))`
    - For SPDX-side assertions: use the SPDX 2.3 / SPDX 3 native
      relationship type / scope field.
- [ ] T050 [P] [US3] Add new integration test
  `scan_cargo_build_dep_emits_native_build_dep_relationship` in
  `tests/scan_cargo.rs` asserting that `[build-dependencies]`
  cc emits with SPDX 2.3 `BUILD_DEPENDENCY_OF` (separate from
  `DEV_DEPENDENCY_OF`) per SC-004.
- [ ] T051 [P] [US4] Add new integration test
  `scan_cargo_build_dep_emits_native_lifecycle_scope_build` in
  `tests/scan_cargo.rs` asserting SPDX 3
  `lifecycleScope: "build"` (separate from `"development"`) per
  SC-004.
- [ ] T052 [P] [US3] [US4] Add new integration test
  `scan_maven_test_scope_emits_native_test_dep_type` in
  `tests/scan_maven.rs` asserting `TEST_DEPENDENCY_OF` (SPDX
  2.3) + `lifecycleScope: "test"` (SPDX 3) per SC-005.
- [ ] T053 [P] [US3] [US4] Add new integration test
  `scan_gem_test_group_emits_native_test_dep_type` in
  `tests/scan_gem.rs` asserting same for gem `:test` group
  per SC-006.

### Real-world smoke

- [ ] T054 [US1] [US3] [US4] [US5] `cargo +stable build -p
  mikebom --release && /Users/mlieberman/Projects/mikebom/target/release/mikebom
  sbom scan --path /Users/mlieberman/Projects/mikebom --output
  /tmp/mb052-default.cdx.json` (use the same isolated-tempdir
  workaround from milestone 051's T018 if needed to avoid the
  v1 npm fixture). Assert: a cargo dev-dep component carries
  `scope: "excluded"` AND
  `properties[]` contains
  `{"name": "mikebom:lifecycle-scope", "value": "development"}`.
  Compare with the corresponding SPDX 2.3 + SPDX 3 outputs
  (re-emit via `--output sbom.spdx.json` and `sbom.spdx3.json`
  separately or via dual-format if available).

### Verify + commit

- [ ] T055 [US1] [US3] [US4] [US5] `./scripts/pre-pr.sh` clean.
- [ ] T056 [US1] [US3] [US4] [US5] Commit:
  `feat(052/us1+us3+us4+us5): native CDX/SPDX 2.3/SPDX 3 emission + edge rewrite`.

---

## Phase 5: Commit `feat(052/us2): --exclude-scope flag + --include-dev deprecation + chore`

**Goal**: CLI flag plumbing + chore. Adds the opt-out flag
(`--exclude-scope <list>`) and deprecates `--include-dev` to a
parse-and-warn no-op. Bundles CHANGELOG entry + speckit
scaffolding.

**Independent test**: SC-002 (`--exclude-scope dev,build,test`
reproduces alpha.9 default component count), SC-003
(`--include-dev` parses + warns + is a no-op).

### CLI flag

- [ ] T057 [US2] In `mikebom-cli/src/cli/scan_cmd.rs`, add the
  new flag:
  ```rust
  #[arg(long, value_delimiter = ',')]
  pub exclude_scope: Vec<ExcludeScopeArg>,
  ```
  with a new local enum `ExcludeScopeArg { Dev, Build, Test }`
  deriving `clap::ValueEnum` (`#[clap(rename_all = "lowercase")]`).
  Help text: "Drop components whose lifecycle scope matches any
  of the listed values. Comma-separated. Valid: dev, build, test."
- [ ] T058 [US2] Mark the existing `--include-dev` flag as
  deprecated. Keep clap parsing (so existing automation
  doesn't error). When the flag is present at parse time, emit
  a one-line `tracing::warn!("--include-dev is deprecated; use
  --exclude-scope to filter dev/build/test scopes (the default
  now includes them)")`.
- [ ] T059 [US2] In `read_all` (or its callsite in
  `scan_cmd.rs`), after the resolver pipeline produces the
  final component list and BEFORE serialization, filter the
  list by `--exclude-scope`: drop components whose
  `lifecycle_scope` matches any element in
  `args.exclude_scope`. Apply the same filter to relationships
  (drop edges referencing dropped components).

### Tests

- [ ] T060 [P] [US2] Add CLI integration test
  `scan_exclude_scope_drops_dev_components` in a new test file
  `mikebom-cli/tests/cli_exclude_scope.rs` (or in
  `tests/scan_cli.rs` if more fitting). Synthetic Rust project
  with both runtime + dev deps. Run with
  `--exclude-scope dev,build,test`. Assert: dev and build deps
  absent; runtime deps present. Component count matches the
  alpha.9 default (which dropped them).
- [ ] T061 [P] [US2] Add CLI integration test
  `scan_include_dev_emits_deprecation_warning_and_is_noop`.
  Same synthetic project. Run with `--include-dev`. Assert:
  stderr contains `"--include-dev is deprecated"` substring;
  output component count matches the new default
  (no exclusions).

### CHANGELOG + scaffolding

- [ ] T062 [US2] Edit `CHANGELOG.md` `[Unreleased]`. Two new
  subsections:
    - `### Changed (BREAKING)`:
      - Default behavior change: dev/build/test deps now
        emitted by default with native scope tags (CDX
        `scope: "excluded"`, SPDX 2.3 dep-type relationships,
        SPDX 3 `lifecycleScope`).
      - `--include-dev` flag deprecated to a parse-and-warn
        no-op. Users wanting the strict deployed-runtime view
        adopt `--exclude-scope dev,build,test`.
      - Removal of `mikebom:dev-dependency` annotation; consumers
        migrate to native fields.
      - C6 row deleted from
        `docs/reference/sbom-format-mapping.md`; new C42 row added
        for `mikebom:lifecycle-scope` (CDX-only finer signal); B2
        extended (or new B3) for native dep-type relationships.
    - `### Added`:
      - `--exclude-scope <comma-list>` flag for the strict
        deployed-runtime view.
      - `LifecycleScope` enum on `ResolvedComponent` /
        `PackageDbEntry`; `TestDependsOn` variant on
        `RelationshipType`.
- [ ] T063 [US2] Stage `specs/052-lifecycle-dep-scope/`
  (spec.md, plan.md, tasks.md, checklists/requirements.md) and
  `CLAUDE.md` (auto-updated by `update-agent-context.sh` during
  /speckit.plan).

### Verify + commit

- [ ] T064 [US2] `./scripts/pre-pr.sh` clean.
- [ ] T065 [US2] Commit:
  `feat(052/us2): --exclude-scope flag + --include-dev deprecation + chore`.

---

## Phase 6: PR

- [ ] T066 Verify final `./scripts/pre-pr.sh` clean from a
  fresh shell.
- [ ] T067 Push branch:
  `git push -u origin 052-lifecycle-dep-scope`.
- [ ] T068 Open PR titled
  `feat(052): native lifecycle-scope dep tagging — CDX + SPDX 2.3 + SPDX 3 native fields, dev/build/test included by default`.
  Body covers:
    - 3-commit summary.
    - The audit gap from alpha.9: `mikebom:dev-dependency`
      reinvented CDX `scope` + SPDX 2.3 dep types + SPDX 3
      LifecycleScopeType.
    - Migration path: `--exclude-scope dev,build,test` for
      operators wanting the strict deployed-runtime view.
    - Audit-grounded smoke numbers from T054.
    - 12-SC verification commands.
    - Constitution v1.4.0 citation: this milestone IS the
      codification case for Principle V's
      "standards-native-precedence" clause.
    - Link to issue #97 (the C-row audit) for downstream
      KEEP-WITH-JUSTIFICATION rows that are candidates for
      additive native emission in follow-on milestones.
- [ ] T069 Verify SC-012: 3 CI lanes green on the PR.

---

## Dependencies

- **Phase 1 (Setup)** blocks all subsequent phases.
- **Phase 2 (Foundational)** is a no-op.
- **Phase 3 (commit 1 — data model + readers)** must complete
  before Phase 4 (the serializers and edge-rewrite consume the
  new types).
- **Phase 4 (commit 2 — serializers + edge rewrite)** must
  complete before Phase 5 (the new `--exclude-scope` flag filters
  on the new `lifecycle_scope` field, which Phase 4's edge
  rewrite preserves end-to-end).
- **Phase 5 (commit 3 — CLI + chore)** independent of Phase 4
  data flow but logically lands last.
- **Phase 6 (PR)** blocked on Phases 3+4+5 all complete.

## Parallel execution opportunities

Within Phase 3:

- T006 ‖ T013 ‖ T015 ‖ T017 — all unit-test additions in
  different files.
- T010-T014 (cargo) ‖ T014-T015 (gem) ‖ T016-T017 (maven) —
  per-reader migrations are independent ecosystems.

Within Phase 4:

- T031 ‖ T036 ‖ T039 ‖ T040 — unit-test additions in different
  serializer files.
- T050 ‖ T051 ‖ T052 ‖ T053 — integration tests in different
  `tests/scan_*.rs` files.

Within Phase 5:

- T060 ‖ T061 — CLI tests in the same file but independent
  scenarios; can be authored in parallel.

Cross-phase: no — Phases 3/4/5 have linear dependencies.

## Implementation strategy

**MVP scope**: Phase 3 (data model + reader migration) + Phase 4
(serializer changes). Once both land, the milestone's headline
deliverable is real: SBOMs gain native scope tags. Phase 5
(CLI flag) is the migration off-ramp; it's needed for SC-002 +
SC-003 but doesn't change emitted-SBOM shape.

**Incremental delivery**: Phase 3 lands the type changes
without observable output change (back-compat shim while
serializers still consume the legacy field-style code paths via
T009's clone update). Phase 4 makes the actual breaking
change in output. Phase 5 adds the opt-out + deprecation
warning.

**Format validation**: All 69 tasks follow the strict checklist
format — `- [ ]` checkbox, `T###` ID, optional `[P]`, `[US#]`
labels (omitted in Setup/Foundational/PR), file paths in
descriptions.
