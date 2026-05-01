# Feature Specification: Native lifecycle-scope dependency tagging — include dev by default, emit standards-defined fields per format

**Feature Branch**: `052-lifecycle-dep-scope`
**Created**: 2026-05-01
**Status**: Draft

## Summary

Today (post milestone 051), mikebom uses a single boolean
`is_dev: Option<bool>` to flag development / test / build dependencies,
serializes it as a custom `mikebom:dev-dependency = true` annotation,
and **drops** flagged components from default-mode SBOMs (consumers
must pass `--include-dev` to see them).

Three things are wrong with that:

1. **Default behavior loses data.** The default scan output omits a
   meaningful slice of the dependency graph. Vulnerability scanners,
   license auditors, and SBOM consumers can't see what's there to
   choose to filter — the data isn't in the file at all.

2. **The custom annotation reinvents what every format already
   defines.** Every SBOM format mikebom emits has a native field for
   this distinction:
   - **CycloneDX 1.6** — `components[].scope` enum
     (`required` / `optional` / `excluded`); plus CycloneDX
     `metadata.tools[]` for build-time utilities; plus the
     `formulation` section (CDX 1.5+) for build environment recipes.
   - **SPDX 2.3** — `BUILD_DEPENDENCY_OF`,
     `DEV_DEPENDENCY_OF`, `TEST_DEPENDENCY_OF` relationship types.
   - **SPDX 3.0.1** — `LifecycleScopeType` enum on relationships
     (`runtime`, `development`, `build`, `test`, `design`); plus
     the SPDX 3 Build Profile (`hasInput`, `usesTool`, `hasOutput`,
     `invokedBy`) for build environment recipes.

3. **One bit isn't enough granularity.** Cargo
   `[dev-dependencies]` vs `[build-dependencies]` is a real
   distinction (criterion is dev/test; cc / bindgen are build).
   Maven `<scope>test</scope>` vs `<scope>provided</scope>` likewise.
   `is_dev: Option<bool>` collapses all of them to one flag.

## Clarifications

### Session 2026-05-01

- Q: Behavior of the legacy `mikebom:dev-dependency` annotation
  during the deprecation window? → A: Option C — drop
  `mikebom:dev-dependency` immediately in this milestone (no
  parallel emission). Native fields (CDX `scope` + new
  `mikebom:lifecycle-scope` property; SPDX 2.3 native
  `DEV/BUILD/TEST_DEPENDENCY_OF` relationships; SPDX 3
  `lifecycleScope` parameter on `dependsOn`) are the sole
  signal post-052. Rationale: nobody is using the legacy
  annotation in production yet (alpha-stage tool, narrow
  consumer base); skipping the deprecation window saves a
  follow-on milestone and keeps the SBOM output clean. The C6
  catalog row is removed from `docs/reference/sbom-format-mapping.md`
  as part of this milestone.

This milestone delivers all three: replace the boolean with a
typed lifecycle-scope enum, emit it via each format's native
construct (replacing the legacy custom annotation outright per
Q1 → Option C), and flip the default to **include all
dependencies, with appropriate scope tags**.

This is a breaking change to default scan output. Operators
relying on `--include-dev=off` to drop dev/test/build deps must
adopt the new opt-out flag (`--exclude-scope <list>`).

## User Scenarios & Testing

### User Story 1 - Default scan emits all dependencies with native scope tags (Priority: P1)

**As an SBOM consumer**, when I read mikebom's default-mode
output, I want every dependency the scanner identified to be
present in the SBOM, with a standards-defined scope tag
identifying it as runtime / development / build / test. That
way I can filter on the appropriate field per my use case
(strict deployed-artifact analysis, full audit, build provenance)
without having to re-scan.

**Why this priority**: This is the core motivation. Today the
default scan strips the dev/build/test slice entirely;
consumers need to know to add `--include-dev`. The fix flips
the default and uses native fields so any compliant SBOM tool
can filter — not just mikebom-aware ones.

**Independent Test**: Run
`mikebom sbom scan --path ~/Projects/mikebom --output sbom.cdx.json`.
Assert: total component count equals the alpha.9 baseline ×
`--include-dev` count (i.e., what we emit today *with*
`--include-dev`, but as default). Assert: ≥1 component carries
CDX `scope: "excluded"`. Same for SPDX 2.3 (≥1
`DEV_DEPENDENCY_OF`-or-similar relationship) and SPDX 3 (≥1
relationship with `scope: "development"` from
`LifecycleScopeType`).

**Acceptance Scenarios**:

1. **Given** a Rust project with `[dev-dependencies] criterion`,
   **When** I run `mikebom sbom scan --path .`,
   **Then** criterion appears in every output format AND carries
   a `Development` scope tag in the format's native
   construct (CDX `scope: "excluded"` + properties; SPDX 2.3
   `DEV_DEPENDENCY_OF` relationship; SPDX 3
   `LifecycleScopeType: "development"`).

2. **Given** a Rust project with `[build-dependencies] cc`,
   **When** I run `mikebom sbom scan --path .`,
   **Then** cc carries a `Build` scope tag (SPDX 2.3
   `BUILD_DEPENDENCY_OF`; SPDX 3 `LifecycleScopeType: "build"`).

3. **Given** a Maven project with `<scope>test</scope>`,
   **When** I run `mikebom sbom scan --path .`,
   **Then** the test dep carries a `Test` scope tag (SPDX 2.3
   `TEST_DEPENDENCY_OF`; SPDX 3 `LifecycleScopeType: "test"`).

---

### User Story 2 - Optional `--exclude-scope` flag drops scoped components (Priority: P1)

**As an operator who wants the strict "deployed runtime" view**,
when I pass `--exclude-scope dev,build,test`, I want
mikebom to drop those scopes from the SBOM before serialization
— restoring the alpha.9 default behavior, but explicit and
finer-grained.

**Why this priority**: Closes the migration path. Operators
currently relying on the post-049 default need a one-line CLI
addition to keep working.

**Independent Test**: Run scan with
`--exclude-scope dev,build,test` on the same Rust project.
Assert: the resulting SBOM has the alpha.9 (`--include-dev=off`)
component count.

**Acceptance Scenarios**:

1. **Given** a Rust project with both runtime and dev deps,
   **When** I run `mikebom sbom scan --path . --exclude-scope dev,build,test`,
   **Then** only runtime-scope components emit; dev/build/test
   absent.

2. **Given** the same project,
   **When** I run `mikebom sbom scan --path . --exclude-scope test`,
   **Then** test-scope components are dropped; dev and build
   scopes still emit (with their tags).

3. **Given** the existing `--include-dev` flag (deprecated by
   this milestone),
   **When** an operator passes it,
   **Then** mikebom emits a one-line deprecation warning
   pointing at `--exclude-scope` and treats it as
   "no exclusions" (the new default).

---

### User Story 3 - SPDX 2.3 emits native dep-type relationships (Priority: P1)

**As an SPDX 2.3 consumer**, when I read mikebom's SPDX output,
I want dev / build / test dependencies expressed as the
standards-defined relationship types (`DEV_DEPENDENCY_OF`,
`BUILD_DEPENDENCY_OF`, `TEST_DEPENDENCY_OF`), not just as a
mikebom-specific annotation. That way any SPDX-aware tool can
process the signal.

**Why this priority**: SPDX 2.3 is the most-widely-deployed
format; consumers expect the native types. Existing scaffolding
in `spdx/relationships.rs:134-139` already maps internal
`DevDependsOn` / `BuildDependsOn` to native types — but no
reader emits those edges today, so the native types never fire.

**Independent Test**: Inspect any SPDX 2.3 output: assert
≥1 relationship of each type (`DEV_DEPENDENCY_OF`, etc.) when
the source project has the corresponding deps.

---

### User Story 4 - SPDX 3 emits LifecycleScopeType on dependsOn relationships (Priority: P1)

**As an SPDX 3 consumer**, when I read a dependency relationship,
I want the lifecycle scope expressed via the standards-defined
`LifecycleScopeType` enum on the relationship (the SPDX-3-native
mechanism — separate from the relationship type itself).

**Why this priority**: SPDX 3.0.1 dropped the dedicated dev/test
relationship-type names and replaced them with this contextual
enum. Without it, mikebom's SPDX 3 output is meaningfully less
informative than its SPDX 2.3 output.

**Independent Test**: SPDX 3 output: assert ≥1 relationship
carries a `lifecycleScope: "development"` (and similarly for
`build` / `test`). The relationship `relationshipType` stays
`dependsOn` per spec.

---

### User Story 5 - CycloneDX emits native `scope` attribute (Priority: P2)

**As a CycloneDX consumer**, when I read a component, I want
its inclusion-status in the deployment footprint expressed via
the native `scope` field (`required` / `optional` /
`excluded`) — the standards-defined construct.

**Why this priority**: CDX `scope` is well-known to all
CDX-aware tools; using it makes mikebom output interoperable
with vulnerability scanners and license auditors that already
filter on this field.

**Independent Test**: CDX output: assert a runtime-scope
component carries no `scope` field (or `scope: "required"`);
a dev/build/test-scope component carries `scope: "excluded"`
PLUS a `mikebom:lifecycle-scope` property carrying the
finer-grained `dev` / `build` / `test` value.

---

### Edge Cases

- **Mixed-source classification disagreement** (e.g., maven
  `<scope>provided</scope>` could be argued as `Build` or
  `Runtime`): default mapping is documented per ecosystem in
  the data model. Operators can override per-component via the
  existing `mikebom sbom enrich` JSON Patch path.
- **Test deps in CycloneDX 1.6**: `scope: "excluded"` is
  documented for "out of deployment". `scope: "optional"` is
  closer to "soft dep" semantics. Decision: dev/build/test all
  map to `excluded` (explicit "not in deployment"); the finer
  classification lives in `mikebom:lifecycle-scope` property.
- **Components reachable via multiple scopes** (e.g., a crate
  used for both `[dependencies]` and `[dev-dependencies]`):
  production wins (existing FR-003 / FR-006 semantic). The
  scope assigned is `Runtime`; no `Dev` tag.
- **The existing `mikebom:dev-dependency` annotation**:
  removed entirely in this milestone (per Q1 → Option C). The
  legacy CDX property + SPDX 2.3/3 annotation emission paths
  go away; the C6 catalog row is deleted; the
  `cdx_dev_deps` / `spdx23_dev_deps` / `spdx3_dev_deps`
  parity extractors are dropped or repurposed for the new
  native-field signal. SBOM consumers that were filtering on
  this annotation MUST migrate to the format-native fields
  (CDX `scope` + new `mikebom:lifecycle-scope`; SPDX 2.3
  `DEV/BUILD/TEST_DEPENDENCY_OF`; SPDX 3 `lifecycleScope`).
- **Existing `is_dev` field**: will be replaced by a new
  `lifecycle_scope: Option<LifecycleScope>` field. Migration
  strategy is a single PR — no shim layer (the field is
  internal to the resolver pipeline).
- **CycloneDX Formulation v1.5+ and SPDX 3 Build Profile**:
  out of scope. Both are dedicated build-recipe sections; this
  milestone is about per-component lifecycle scope, not whole-
  build provenance. Tracked separately.

## Requirements

### Functional Requirements

- **FR-001**: Replace the existing `is_dev: Option<bool>` field
  on `PackageDbEntry` and `ResolvedComponent` with a new
  `lifecycle_scope: Option<LifecycleScope>` field, where
  `LifecycleScope` is an enum with variants `Runtime`,
  `Development`, `Build`, `Test`. `None` means "scope unknown
  / unclassified" (sources that don't carry the distinction —
  dpkg, apk, etc.).
- **FR-002 (default behavior change)**: `mikebom sbom scan`
  (and `--image`) MUST emit components of every scope
  (`Runtime`, `Development`, `Build`, `Test`) by default. No
  silent filtering of any scope.
- **FR-003 (opt-out)**: Add a CLI flag
  `--exclude-scope <comma-list>` accepting any subset of
  `dev,build,test`. Components carrying an excluded scope MUST
  be dropped before SBOM serialization. `Runtime` cannot be
  excluded (a no-runtime SBOM is meaningless).
- **FR-004 (deprecation)**: When the existing `--include-dev`
  flag is passed, mikebom MUST emit a single-line stderr
  deprecation warning pointing at `--exclude-scope` and treat
  the flag as a no-op (no exclusions = new default behavior).
  The flag MUST continue to parse and exit zero — no breakage
  for users with `--include-dev` baked into automation.
- **FR-005 (Cargo classifier)**: Crates declared in
  `[dependencies]` map to `Runtime`; `[dev-dependencies]` map
  to `Development`; `[build-dependencies]` map to `Build`. A
  crate reachable through multiple scopes resolves to the
  most-restrictive in this priority: `Runtime` > `Build` >
  `Development` > `Test`. (Production wins; among non-runtime
  scopes, build wins because build deps end up in the linker's
  output more often than dev/test.)
- **FR-006 (Gem classifier)**: Gems in the default group map
  to `Runtime`; gems in `:test` group map to `Test`; gems in
  any other non-default group (`:development`, `:doc`, custom)
  map to `Development`. Gemspec `add_dependency` /
  `add_runtime_dependency` → `Runtime`;
  `add_development_dependency` → `Development`. Multi-source
  conflicts resolve via FR-005's priority.
- **FR-007 (Maven classifier)**: `<scope>test</scope>` →
  `Test`. `<scope>provided</scope>` → `Build`.
  `<scope>compile</scope>` (or absent) → `Runtime`.
  `<scope>runtime</scope>` → `Runtime`. `<scope>system</scope>`
  → `Runtime` (treat as compile). `<scope>import</scope>` →
  `Runtime` (BOM imports are managed deps).
- **FR-008 (Go classifier)**: Test-only imports from `_test.go`
  source (per milestone 049) → `Test`. The existing
  `mikebom:not-linked` annotation (milestone 050) is
  ORTHOGONAL to scope and stays as-is. A go.sum entry that's
  test-only AND not-linked carries both signals.
- **FR-009 (npm / Poetry / Pipfile classifier)**: Existing
  `devDependencies` / `[tool.poetry.dev-dependencies]` /
  Pipfile `develop:` → `Development` (no finer signal
  available without ecosystem-specific naming heuristics).
- **FR-010 (CDX serialization)**: Components with
  `lifecycle_scope: Some(Runtime)` or `None` MUST emit no
  `scope` field (CDX default = `required`). Components with
  any of `Development` / `Build` / `Test` MUST emit
  `scope: "excluded"` AND a new property
  `mikebom:lifecycle-scope` carrying the lower-cased variant
  name (`development`, `build`, `test`).
- **FR-011 (SPDX 2.3 serialization)**: Each `Development` /
  `Build` / `Test` component MUST emit a native relationship
  edge of type `DEV_DEPENDENCY_OF` / `BUILD_DEPENDENCY_OF` /
  `TEST_DEPENDENCY_OF` from the parent component to the
  scoped dep. The legacy `mikebom:dev-dependency` annotation
  MUST NOT emit (removed per Q1 → Option C).
- **FR-012 (SPDX 3 serialization)**: Each `Development` /
  `Build` / `Test` dependency relationship MUST emit
  `lifecycleScope: "development"` / `"build"` / `"test"` on
  the `dependsOn` relationship element (the SPDX 3 native
  `LifecycleScopeType`). The legacy `mikebom:dev-dependency`
  annotation MUST NOT emit (removed per Q1 → Option C).
- **FR-013 (parity)**: A new C-row in
  `docs/reference/sbom-format-mapping.md` for
  `mikebom:lifecycle-scope` (CDX-only finer signal). A new
  B-row covering the native dep-type relationships across
  all three formats. Both wired into `holistic_parity` via
  the existing extractor pattern.

  **Native-field audit (Constitution Principle V, v1.4.0)**:
    - **CDX 1.6** — native `scope` enum has only 3 values
      (`required` / `optional` / `excluded`). No native field
      distinguishes `development` vs `build` vs `test`. The
      `mikebom:lifecycle-scope` property is permitted under
      the "finer-grained information the standard does not
      express" carve-out.
    - **SPDX 2.3** — native `DEV_DEPENDENCY_OF` /
      `BUILD_DEPENDENCY_OF` / `TEST_DEPENDENCY_OF` cover the
      three cases natively. Per FR-011, mikebom emits these
      as the primary signal; no SPDX 2.3-side custom property
      needed.
    - **SPDX 3.0.1** — native `LifecycleScopeType` enum on
      `dependsOn` relationships covers `development` /
      `build` / `test` natively. Per FR-012, mikebom emits
      these as the primary signal; no SPDX 3-side custom
      property needed.
  The `mikebom:lifecycle-scope` CDX property is therefore the
  ONE place a `mikebom:*` field is justified, and it bridges
  a parity gap (CDX consumers can't access the dev/build/test
  split via native fields alone) per the second
  Principle-V carve-out.
- **FR-014 (removal of `mikebom:dev-dependency`)**: Per Q1 →
  Option C, this milestone removes the C6 annotation
  entirely. Specifically: (a) delete the C6 row from
  `docs/reference/sbom-format-mapping.md`; (b) drop the
  per-format emitters at `cyclonedx/builder.rs:317`,
  `spdx/annotations.rs:148`, `spdx/v3_annotations.rs:164`;
  (c) drop the `cdx_dev_deps` / `spdx23_dev_deps` /
  `spdx3_dev_deps` parity extractors (or repurpose them to
  cover the new native fields if their plumbing applies).
  SBOM consumers MUST migrate to the format-native fields.

### Key Entities

- **`LifecycleScope` enum**: `Runtime`, `Development`,
  `Build`, `Test`. Lives in `mikebom_common::resolution`
  alongside `RelationshipType`.
- **`RelationshipType` enum** (existing): already has
  `DevDependsOn` and `BuildDependsOn`; need to add
  `TestDependsOn` and ensure readers emit the right variant.
- **CLI flags**: `--exclude-scope <comma-list>` (new),
  `--include-dev` (deprecated parse-and-warn shim that
  no-ops; per FR-004 retained only so existing automation
  doesn't break on flag-parse error).
- **C-row C42** (new): `mikebom:lifecycle-scope` — CDX
  property carrying `development` / `build` / `test`.
- **B-row B3** (new or extension of B2): native dep-type
  relationships per format.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Default `mikebom sbom scan` on a Rust project
  with both runtime and dev deps emits ALL components AND
  ≥1 carries CDX `scope: "excluded"` + SPDX 2.3
  `DEV_DEPENDENCY_OF` relationship + SPDX 3
  `lifecycleScope: "development"`.
- **SC-002**: `mikebom sbom scan --exclude-scope dev,build,test`
  on the same project reproduces the alpha.9 default
  component count (i.e., the post-051 component count under
  `--include-dev=off`).
- **SC-003**: `--include-dev` flag still parses, prints a
  one-line deprecation warning to stderr, and is treated as
  a no-op.
- **SC-004**: Cargo `[build-dependencies]` cc emits with SPDX
  2.3 `BUILD_DEPENDENCY_OF` (not `DEV_DEPENDENCY_OF`) and
  SPDX 3 `lifecycleScope: "build"` (not `"development"`).
- **SC-005**: Maven `<scope>test</scope>` junit emits with
  SPDX 2.3 `TEST_DEPENDENCY_OF` and SPDX 3
  `lifecycleScope: "test"`.
- **SC-006**: Gem `:test` group rspec emits with SPDX 2.3
  `TEST_DEPENDENCY_OF` and SPDX 3 `lifecycleScope: "test"`.
- **SC-007**: 27 byte-identity goldens regen with the new
  fields appearing in dev-affected fixtures (cargo, gem,
  maven, npm, pip, golang). Non-dev-affected goldens stay
  byte-identical.
- **SC-008**: `holistic_parity` 11/11 ok with the new
  C42 + B3 rows wired in.
- **SC-009**: Each ecosystem reader has ≥1 new integration
  test asserting native-format scope tags appear.
- **SC-010**: `mikebom:dev-dependency` annotation MUST NOT
  appear in any default-mode SBOM output (per Q1 → Option C —
  removed entirely from CDX `properties[]`, SPDX 2.3
  `annotations[]`, and SPDX 3 `annotations[]`). C6 row
  deleted from `docs/reference/sbom-format-mapping.md`.
- **SC-011**: `pre-pr.sh` clean.
- **SC-012**: 3 CI lanes green.

## Assumptions

- The breaking change to default behavior is acceptable in
  the 0.1.0-alpha series — pre-1.0 contract per Constitution
  V. Documented in CHANGELOG with a clear migration path
  (`--exclude-scope dev,build,test`).
- CycloneDX `scope: "excluded"` is the right mapping for
  dev/build/test (vs. `optional`). Per CDX docs:
  *"the component is excluded from the bill of materials,
  not used at runtime, and excluded from the deployment
  footprint."* The finer dev-vs-build-vs-test split lives
  in our new `mikebom:lifecycle-scope` property.
- SPDX 3's `LifecycleScopeType` enum applies to relationships,
  NOT components — different shape from SPDX 2.3 (where the
  relationship type itself encodes scope). The serializer
  needs to emit a separate `lifecycleScope` field on each
  scoped `dependsOn` relationship.
- Removing `is_dev: Option<bool>` is a one-PR migration. The
  field is internal to the resolver pipeline (PackageDbEntry
  → ResolvedComponent → serializers). No external SBOM
  contract depends on it directly — the contract is the
  emitted output.
- The existing `mikebom:dev-dependency` annotation is
  removed entirely in this milestone (per Q1 → Option C).
  No deprecation window; native fields are the sole signal
  going forward. Acceptable because the annotation has no
  known production consumers — alpha-stage tool, narrow
  user base, breaking change documented in CHANGELOG with
  migration guidance pointing at the format-native fields.
- CDX Formulation v1.5+ and SPDX 3 Build Profile are
  separate, larger milestones. Each represents a new SBOM
  section (build-recipe documentation), not just per-
  component scope tagging.
- rpm `Recommends:` / `Suggests:` and similar OS-package soft
  deps stay scope-`None` (unknown) — they're not dev/build/test
  in the same sense as language-ecosystem deps.
