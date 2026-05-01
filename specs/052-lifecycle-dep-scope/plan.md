---
description: "Plan — milestone 052 native lifecycle-scope dep tagging (CDX + SPDX 2.3 + SPDX 3)"
---

# Plan: Native lifecycle-scope dependency tagging

**Branch**: `052-lifecycle-dep-scope` | **Spec**: spec.md ✅
**Output**: 4-file tighter template (no research.md / data-model.md /
contracts/ / quickstart.md — pattern from
021/022/023/042/046/047/048/049/050/051).

## Constitution Check

Reviewed against `.specify/memory/constitution.md` v1.4.0:

- **I. Pure Rust, Zero C**: zero new deps; existing
  serde/serde_json/quick-xml carry the load.
- **III. Fail Closed**: scope-classification failures
  (e.g., a malformed Cargo.toml section) fall back to
  `lifecycle_scope: None` — same warn-and-skip pattern as
  milestone 051. The opt-out flag (`--exclude-scope`) accepts
  only the documented values; clap rejects invalid tokens at
  parse time.
- **IV. Type-Driven Correctness**: replace
  `is_dev: Option<bool>` with `lifecycle_scope:
  Option<LifecycleScope>`, where `LifecycleScope` is a 4-variant
  enum. Three-state semantics preserved (`None` = unknown).
- **V. Specification Compliance (v1.4.0)**: this milestone IS
  the codification case for the new "standards-native fields
  take precedence" clause. Spec FR-013 cites the per-format
  audit explicitly. The one new `mikebom:*` property
  (`mikebom:lifecycle-scope`) is permitted under the
  finer-grained-info carve-out: CDX 1.6 native `scope` enum
  has only 3 values (`required`/`optional`/`excluded`) and
  cannot express dev-vs-build-vs-test.
- **VIII. Completeness / IX. Accuracy**: directly improves
  both — the default-mode SBOM gains an entire dependency
  slice that was previously dropped silently.

## Phase 0: Recon (resolved inline; no `research.md`)

### R1. Current `is_dev` + relationship-type wiring

**Findings** (from `grep -rn` on the current tree):

1. **Field at common-side**: `mikebom_common::resolution::ResolvedComponent::is_dev: Option<bool>`
   (existing). Plus the same field on
   `package_db::PackageDbEntry::is_dev: Option<bool>`
   (existing — propagated through `scan_fs/mod.rs:523`'s
   `entry.is_dev` clone).
2. **Reader call sites** (15 hits): every reader sets
   `is_dev: None` by default; cargo + gem + maven have
   non-trivial classification (post milestones 049-051):
   - `cargo.rs:610`: drops on `entry.is_dev == Some(true)
     && !include_dev`.
   - `maven.rs:1786-1823`: filters + tags `<scope>test</scope>`.
   - `gem.rs`: post-051 multi-source classification.
   - `golang.rs` + `mod.rs::apply_go_production_set_filter`:
     test-only tagging post-049.
   - `pip/pipfile.rs:75`: tags Pipfile `develop:` deps.
3. **Internal relationship enum**
   (`mikebom_common::resolution::RelationshipType`):
   has `DependsOn` / `DevDependsOn` / `BuildDependsOn`. The
   SPDX 2.3 mapper at `spdx/relationships.rs:134-139` already
   maps the latter two to native `DEV_DEPENDENCY_OF` /
   `BUILD_DEPENDENCY_OF`. **But no reader emits the typed
   variants** — every dep edge is `DependsOn`. The native
   types are dead-code-pathwise reachable but never fired.
4. **`TestDependsOn` does NOT exist** as a `RelationshipType`
   variant. SPDX 2.3 has `TEST_DEPENDENCY_OF`; the mapper
   needs to be extended.
5. **Serializer call sites for the legacy annotation**:
   - `cyclonedx/builder.rs:315-318` emits
     `mikebom:dev-dependency` as a `components[].properties[]`
     entry.
   - `spdx/annotations.rs:147-148` emits it as a 2.3 Annotation.
   - `spdx/v3_annotations.rs:163-164` emits it as a 3.0.1
     Annotation.
   - `v3_relationships.rs:55` documents the
     "all variants → dependsOn" collapse — no
     `lifecycleScope` parameter today.

### R2. Native field shapes per format

**CycloneDX 1.6**: `components[].scope` enum, three values:
`required` (default — the component must be included for
the BOM's described system to function), `optional`
(presence is desirable but not required), `excluded`
(component is NOT used at runtime, NOT in deployment
footprint). No native sub-distinction between dev / build /
test — that's the carve-out we use the new
`mikebom:lifecycle-scope` property for.

**SPDX 2.3**: relationship types `DEV_DEPENDENCY_OF`,
`BUILD_DEPENDENCY_OF`, `TEST_DEPENDENCY_OF`. Direction is
**reversed** from the natural reading: SPDX
`B DEV_DEPENDENCY_OF A` means "B is a dev dep of A". Mikebom's
internal model uses `(A DependsOn B)` = "A depends on B"; the
mapper at `relationships.rs:73-78` documents the reversal.

**SPDX 3.0.1**: `dependsOn` Relationship element with a
`scope` field of type `LifecycleScopeType`. Enum values:
`build`, `design`, `development`, `runtime`, `test`. The
`scope` field IS the standards-defined dev-vs-build-vs-test
parameterization in SPDX 3.

### R3. Mapping the 4-variant `LifecycleScope` to per-format constructs

| `LifecycleScope` | CDX `scope` | CDX `mikebom:lifecycle-scope` | SPDX 2.3 relationship | SPDX 3 `LifecycleScopeType` |
|---|---|---|---|---|
| `Runtime` (or `None`) | omitted (default `required`) | omitted | `DEPENDS_ON` | `runtime` |
| `Development` | `excluded` | `development` | `DEV_DEPENDENCY_OF` | `development` |
| `Build` | `excluded` | `build` | `BUILD_DEPENDENCY_OF` | `build` |
| `Test` | `excluded` | `test` | `TEST_DEPENDENCY_OF` | `test` |

### R4. Per-ecosystem reader classifier mapping

Per spec FR-005 through FR-009. Each reader's existing
`is_dev` site becomes a `lifecycle_scope` site setting the
right variant.

- **Cargo** (post-051): `[dev-dependencies]` set →
  `Development`; `[build-dependencies]` set → `Build`;
  prod set → `Runtime`. The existing
  `compute_cargo_prod_set` already produces the prod set;
  add a parallel `compute_cargo_build_set` (BFS seeded on
  `build_deps`) so we can distinguish `Build` from
  `Development` at tagging time. Production-wins precedence
  per FR-005.
- **Gem** (post-051): `:test` group → `Test`; any other
  non-default group → `Development`; default → `Runtime`.
  The existing 3-source union already produces the
  grouping map. Add a check on group names: presence of
  `"test"` → Test (otherwise → Development).
- **Maven** (existing post-049): `<scope>test</scope>` →
  `Test`; `<scope>provided</scope>` → `Build`. Update
  `maven.rs:1823` to set `lifecycle_scope` instead of
  `is_dev`.
- **Go** (post-049): test-only imports already detected
  (`signals.test_only_imports`); set
  `lifecycle_scope: Some(Test)`. Other Go entries →
  `Runtime` or `None`.
- **npm / Poetry / Pipfile**: existing dev-dep detection
  → `Development` (no finer signal — FR-009).
- **Other readers** (apk, dpkg, rpm, rpmdb, etc.): leave
  `lifecycle_scope: None` (unknown — these formats don't
  carry the distinction).

### R5. Edge typing — when readers emit dep edges

For SPDX 2.3 to emit native `DEV/BUILD/TEST_DEPENDENCY_OF`,
the relationship edges flowing through the resolver
pipeline need the matching `RelationshipType` variant set.
Two options:

- **Option A (edge-time)**: each reader, when emitting a
  dep edge to a target whose `lifecycle_scope` is non-
  `Runtime`, picks the matching variant. Touches every
  reader's relationship-emission code.
- **Option B (resolver-time)**: leave reader code emitting
  `DependsOn` as today; in the resolver pipeline (after
  components are resolved), rewrite each relationship's
  type to match its target component's `lifecycle_scope`.
  Centralized; readers untouched.

**Decision**: Option B. Single rewrite step in
`scan_fs/mod.rs` after component resolution finishes,
before relationships are passed to serializers. Keeps the
diff small and avoids risk of one reader picking the wrong
variant. New helper `apply_lifecycle_scope_to_edges` runs
after `apply_go_cache_zip_filter` (which operates on the
resolved component list).

### R6. CLI surface

- **New flag** `--exclude-scope <comma-list>`: clap-defined
  `Vec<ExcludeScopeArg>` where `ExcludeScopeArg` is a
  3-variant enum (`Dev`, `Build`, `Test`) — `Runtime` is
  unconditionally included so it's not in the enum.
- **Deprecated flag** `--include-dev`: kept for parse
  compatibility; clap accepts it; `cli/scan_cmd.rs` emits a
  one-line `tracing::warn!` if present and otherwise
  ignores it (the value's effect is now the new default).

### R7. Existing `mikebom:dev-dependency` annotation removal

Per spec Q1 → Option C. Remove:

- `cyclonedx/builder.rs:315-318` (the property emission block)
- `spdx/annotations.rs:147-148` (the SPDX 2.3 emission)
- `spdx/v3_annotations.rs:163-164` (the SPDX 3 emission)
- C6 row in `docs/reference/sbom-format-mapping.md`
- The 3 parity extractors `cdx_dev_deps`,
  `spdx23_dev_deps`, `spdx3_dev_deps` — repurpose them to
  cover the new native fields, OR drop and add new
  extractors `cdx_lifecycle_scope`, `spdx23_dep_type`,
  `spdx3_lifecycle_scope`.

### R8. Goldens

Every fixture exercising dev/test/build deps will see
goldens churn. Audit during impl: `cargo`, `gem`, `maven`,
`npm`, `pip`, `golang` likely all need regen. Non-affected
ecosystems (`apk`, `deb`, `rpm`) stay byte-identical.

## Phase 1: Implementation strategy

Single PR, three commits:

### Commit 1 — `feat(052/us1+us3): LifecycleScope data model + reader migration`

**Scope**: replace `is_dev` with `lifecycle_scope`
end-to-end on the model side; update every reader to set
the new field with the right variant. No serializer
changes — those land in commit 2 so this commit can be
reviewed for correctness without churn from goldens regen.

**Touched files**:

- **`mikebom-common/src/resolution.rs`** (~30 LOC):
  - Add `pub enum LifecycleScope { Runtime, Development,
    Build, Test }` with `serde(rename_all = "snake_case")`.
  - Add `pub lifecycle_scope: Option<LifecycleScope>` field
    on `ResolvedComponent`. Remove `pub is_dev:
    Option<bool>`.
  - Add `TestDependsOn` variant to `RelationshipType`.

- **`mikebom-cli/src/scan_fs/package_db/mod.rs::PackageDbEntry`**
  (~5 LOC): replace `is_dev: Option<bool>` with
  `lifecycle_scope: Option<LifecycleScope>`. Update the
  ~15 `is_dev: None` initializer sites.

- **`mikebom-cli/src/scan_fs/mod.rs:523`** (~5 LOC):
  update the `entry.is_dev.clone()` line to
  `entry.lifecycle_scope.clone()`.

- **Reader updates** (~80 LOC across 6 files):
  - `cargo.rs` (~30 LOC): add `compute_cargo_build_set`
    BFS helper; in `parse_lockfile`, set
    `lifecycle_scope` based on prod/build/dev membership.
    Update the existing post-051 drop logic.
  - `gem.rs` (~20 LOC): in `read`, set `lifecycle_scope`
    based on group membership (`:test` → Test,
    other-non-default → Development, default → Runtime).
  - `maven.rs:1823` (~5 LOC): replace
    `is_dev` with a helper
    `lifecycle_scope_from_maven(dep.scope.as_deref())`
    (handling test → Test, provided → Build, others →
    Runtime).
  - `golang.rs` + `package_db/mod.rs::apply_go_production_set_filter`
    (~10 LOC): set `lifecycle_scope: Some(Test)` on
    test-only entries.
  - `pip/pipfile.rs:75` (~5 LOC): replace `is_dev`
    setter.
  - npm: same pattern.

**Verification** (Commit 1):

- `cargo +stable build -p mikebom` clean.
- Inline unit tests for the new enum's serde round-trip
  + the new `compute_cargo_build_set` helper (~20 LOC of
  tests).
- Existing 1200+ unit tests should pass — the change is
  field rename + enum population, no behavior change at
  serialization time YET.
- Existing integration tests still pass because the
  serializer code (commit 2) hasn't moved yet.

### Commit 2 — `feat(052/us2+us4+us5): native CDX/SPDX 2.3/SPDX 3 emission + edge rewrite`

**Scope**: serializer changes + the resolver-pipeline edge
rewrite step. This is where output behavior changes; goldens
regen here.

**Touched files**:

- **`mikebom-cli/src/scan_fs/mod.rs`** (~30 LOC):
  - New helper `apply_lifecycle_scope_to_edges` (after
    `apply_go_cache_zip_filter`): for each `DependsOn`
    edge in `relationships`, look up the target component's
    `lifecycle_scope` and rewrite the edge to
    `DevDependsOn` / `BuildDependsOn` / `TestDependsOn`
    when the target has the matching variant. `Runtime`
    and `None` leave the edge as `DependsOn`.

- **`mikebom-cli/src/generate/cyclonedx/builder.rs`** (~25 LOC):
  - Replace the `mikebom:dev-dependency` property emission
    (current lines 315-318) with:
    1. Set `scope: "excluded"` on components with
       `lifecycle_scope` of `Development` / `Build` /
       `Test`.
    2. Emit `mikebom:lifecycle-scope` property with the
       lower-cased variant name.

- **`mikebom-cli/src/generate/spdx/relationships.rs`** (~10 LOC):
  - Extend the existing `RelationshipType::DevDependsOn` /
    `BuildDependsOn` mapper at line 134-139 with a
    `TestDependsOn → TEST_DEPENDENCY_OF` arm.

- **`mikebom-cli/src/generate/spdx/annotations.rs:147-148`**
  (~5 LOC): remove the `mikebom:dev-dependency` emission
  block.

- **`mikebom-cli/src/generate/spdx/v3_annotations.rs:163-164`**
  (~5 LOC): same removal.

- **`mikebom-cli/src/generate/spdx/v3_relationships.rs`**
  (~30 LOC): extend `dependsOn` emission to set the SPDX 3
  `scope` field (LifecycleScopeType) when the target's
  `lifecycle_scope` is non-Runtime. Translate
  `Development → "development"`, `Build → "build"`,
  `Test → "test"`. Update the docstring at line 55.

- **`docs/reference/sbom-format-mapping.md`**:
  - Delete the C6 row.
  - Add new C-row C42 for `mikebom:lifecycle-scope`
    (CDX-only, finer-grained info per the Principle V
    audit clause).
  - Extend B2 (or add a new B-row B3) to document the
    native dep-type relationships across all three formats.

- **Parity extractors** (`mikebom-cli/src/parity/extractors/`):
  - Replace `cdx_dev_deps` / `spdx23_dev_deps` /
    `spdx3_dev_deps` with new extractors that pick up the
    native fields.
  - Wire C42 + B3 into the EXTRACTORS table with
    SymmetricEqual directionality.

- **Goldens regen**: `MIKEBOM_UPDATE_*_GOLDENS=1` for
  cargo, gem, maven, npm, pip, golang fixtures. Non-
  dev-affected ecosystems (apk, deb, rpm) stay
  byte-identical.

- **Integration tests**: update existing `scan_*.rs` tests
  that asserted on the legacy annotation. New tests
  asserting the native fields appear (one per ecosystem,
  one per format → ~6 new tests).

**Verification** (Commit 2):

- `holistic_parity` 11/11 ok.
- 27 byte-identity goldens regen cleanly with sensible
  diffs (no spurious churn).
- Smoke on the mikebom workspace: default scan emits CDX
  components with `scope: "excluded"` for cargo dev-deps;
  SPDX 2.3 carries `DEV_DEPENDENCY_OF` relationships;
  SPDX 3 carries `lifecycleScope: "development"`.

### Commit 3 — `feat(052): --exclude-scope flag + --include-dev deprecation + CHANGELOG + scaffolding`

**Scope**: CLI flag plumbing + chore.

**Touched files**:

- **`mikebom-cli/src/cli/scan_cmd.rs`** (~30 LOC):
  - Add `--exclude-scope <comma-list>` flag: clap-derived
    `Vec<ExcludeScopeArg>` where `ExcludeScopeArg` is
    `{ Dev, Build, Test }`.
  - Mark `--include-dev` as deprecated: keep clap parsing,
    emit a single-line `tracing::warn!` when present.
  - In `read_all` / its callsite: filter the resolved
    component list, dropping components whose
    `lifecycle_scope` matches any element in
    `--exclude-scope` BEFORE serialization.

- **`CHANGELOG.md`** (~25 LOC): `[Unreleased]` →
  `### Changed` (BREAKING) entries for:
  - Default behavior change: dev/build/test deps now
    emitted by default with native scope tags.
  - `--exclude-scope <list>` for the strict deployed-
    runtime view.
  - `--include-dev` deprecated to a parse-and-warn no-op.
  - Removal of `mikebom:dev-dependency` annotation;
    consumers migrate to native fields.
  - C6 row deleted; new C42 + B3 rows.

- **`specs/052-lifecycle-dep-scope/`** scaffolding
  (spec.md, plan.md, tasks.md, checklists/requirements.md
  — already authored; bundle into this commit per the
  4-file pattern).

- **`CLAUDE.md`** auto-update from
  `update-agent-context.sh`.

**Verification** (Commit 3):

- `pre-pr.sh` clean.
- New CLI integration test asserting
  `--exclude-scope dev,build,test` reproduces the alpha.9
  default component count (per SC-002).
- New CLI integration test asserting `--include-dev`
  parses, prints a deprecation warning to stderr, and
  doesn't change output (per SC-003).

## Touched files

| File | Commit | LOC |
|---|---|---|
| `mikebom-common/src/resolution.rs` | 1 | +30 / -3 |
| `mikebom-cli/src/scan_fs/package_db/mod.rs` | 1 | +5 / -3 |
| `mikebom-cli/src/scan_fs/mod.rs` | 1+2 | +35 |
| Per-ecosystem readers (cargo/gem/maven/golang/pip/etc.) | 1 | +80 |
| `mikebom-cli/src/generate/cyclonedx/builder.rs` | 2 | +25 / -10 |
| `mikebom-cli/src/generate/spdx/relationships.rs` | 2 | +10 |
| `mikebom-cli/src/generate/spdx/annotations.rs` | 2 | -5 |
| `mikebom-cli/src/generate/spdx/v3_annotations.rs` | 2 | -5 |
| `mikebom-cli/src/generate/spdx/v3_relationships.rs` | 2 | +30 |
| `docs/reference/sbom-format-mapping.md` | 2 | +20 / -5 |
| `mikebom-cli/src/parity/extractors/{cdx,spdx2,spdx3,mod}.rs` | 2 | ~+30 |
| `mikebom-cli/src/cli/scan_cmd.rs` | 3 | +30 |
| `CHANGELOG.md` | 3 | +25 |
| `specs/052-lifecycle-dep-scope/` | 3 | scaffolding |
| 27 byte-identity goldens | 2 | regen (6 affected, 21 unchanged) |

Total: ~370 LOC of Rust + ~50 LOC docs + scaffolding +
goldens regen.

## Risks

- **R1: Goldens churn larger than expected.** CDX
  serialization shifts (new `scope` field, new property
  ordering) may cause more diff than just the dev-affected
  fixtures. Audit during impl; if a non-dev fixture's
  golden shifts purely from version-string ordering,
  investigate before regen-ing.

- **R2: SPDX 2.3 relationship-direction reversal.** The
  existing mapper at `relationships.rs:73-78` documents
  the `DEV_DEPENDENCY_OF` reversal. The new
  `TestDependsOn → TEST_DEPENDENCY_OF` arm must follow the
  same convention. The existing test at line 319-332 is
  the regression guard; extend it to cover Test.

- **R3: Multi-source classification conflicts** (e.g.,
  cargo `[dependencies]` + `[dev-dependencies]` for the
  same crate). FR-005's priority hierarchy (`Runtime >
  Build > Development > Test`) needs to be enforced
  CONSISTENTLY in the classifier. Inline unit test
  covering each pairwise conflict.

- **R4: `--exclude-scope` flag parsing.** clap's
  `value_delimiter = ','` should accept
  `--exclude-scope dev,build,test` cleanly. Verify with
  an explicit CLI test; multi-flag occurrence
  (`--exclude-scope dev --exclude-scope test`) should
  also work and union.

- **R5: SPDX 3 `lifecycleScope` JSON-LD shape**. The
  SPDX 3.0.1 schema places `scope` as a property on the
  Relationship element, not as a separate object. Verify
  the existing schema validator (test fixture) accepts
  the new field; if it requires a context-URL prefix
  (e.g., `software_lifecycleScope`), match the
  serializer.

## Out of scope

- **CycloneDX Formulation v1.5+ build-recipe section** —
  separate larger milestone (whole new SBOM section
  describing the build environment).
- **SPDX 3 Build Profile** (`hasInput` / `usesTool` /
  `hasOutput` / `invokedBy`) — separate larger milestone
  (whole-build provenance, parallel to CDX Formulation).
- **rpm `Recommends:` / `Suggests:` soft-deps** — stay
  scope-`None` (unknown). Different semantic.
- **Future `mikebom:lifecycle-scope` removal** —
  CDX-finer-grained-info carve-out per Principle V; stays
  indefinitely.
