# Feature Specification: Go source-tree full transitive closure with test-vs-prod tagging

**Feature Branch**: `049-go-source-scope`
**Created**: 2026-05-01
**Status**: Draft
**Input**: User description: "yes" (continuation of conversation about Go source-tree scope)

## Background

A v0.1.0-alpha.8 mikebom scan of a real-world Go API project
(`github.com/kusaridev/iac/app-code/apigatewayv2/config`) emitted
**6 components**. trivy scanning the same source tree emitted
**55**. The 49-component gap traces to mikebom's deliberate
"source-import-driven" Go scope — which only emits modules
directly named in the project's non-`_test.go` `import`
statements. trivy walks `go.sum` and emits everything.

The user's critique:

> "I need to know also indirect dependencies. What's the purpose
> of a software bill of materials that doesn't describe the
> software?"
>
> "I care if a dependency is only a test dependency and in some
> cases might want to not include that as that would not be part
> of the resultant binary."

This is correct. mikebom's current Go behavior fails the basic
SBOM contract: an SBOM that omits transitively-pulled deps
isn't usable for vuln scanning, license compliance, or
supply-chain review. The fix is structural — emit the full
transitive closure (matching trivy / syft) and use the existing
`is_dev` + `--include-dev` infrastructure (already populated for
npm / Poetry / Pipfile) to classify test-only deps.

The audit-grounded improvement opportunity goes beyond parity:
**trivy doesn't classify test-vs-prod for Go either** (verified
against the existing `trivy.cdx.json` — every component has the
same flat shape with no test/prod marker). After this milestone,
mikebom on Go source trees will be:

- Equally complete as trivy (full go.sum closure).
- **Strictly more informative** — test-only deps tagged
  `mikebom:dev-dependency = true` and filterable via the
  existing `--include-dev` flag.

Per `docs/design-notes.md:223`, this is a known deferred design
item ("Go source-tree scope — investigate switching from
go.sum-driven to `go.mod Require`-driven enumeration"). The
audit-grounded scope this milestone settles on is fuller still:
go.sum-driven *with* test-vs-prod classification.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Emit full transitive closure for Go source-tree scans (Priority: P1) 🎯 MVP

An operator runs `mikebom sbom scan --path ./my-go-project`
expecting a complete inventory of what their compiled binary
will depend on, the same way trivy / syft / a binary-mode mikebom
scan would report. Today they get only the modules named in
their project's `import` statements; transitive deps (e.g.
`github.com/aws/smithy-go` reached through `aws-sdk-go-v2`) are
missing.

After this milestone, the default scan emits the full
transitive closure — every prod-reachable entry in `go.sum`. For
the audit-grounded fixture (`apigatewayv2/config`), the count
goes from 6 → ~52 components, matching what trivy reports minus
test-only deps (which US2 handles).

**Why this priority**: closes the basic SBOM contract gap. Until
this lands, mikebom-on-Go-source isn't usable for downstream
consumers expecting transitive coverage. Pure additive; no
breaking change to existing users (a tighter SBOM is always
included in a fuller SBOM).

**Independent Test**: After implementation, scanning
`apigatewayv2/config` produces a CycloneDX SBOM whose
`components[]` array contains at least 50 entries (vs the 6
current pre-milestone). All distinct external Go modules listed
in the project's `go.sum` that are reachable from any prod
import path appear as components.

**Acceptance Scenarios**:

1. **Given** the audit-grounded fixture
   `~/Projects/iac/app-code/apigatewayv2/config` (a Go module
   with 64 go.sum entries split across direct prod imports +
   transitive prod deps + 3 test-only deps), **When** the
   operator runs `mikebom sbom scan --path .`, **Then** the
   emitted SBOM contains components for all transitive prod
   deps including `github.com/aws/smithy-go`,
   `github.com/aws/aws-sdk-go-v2/credentials`,
   `github.com/gabriel-vasile/mimetype`, etc. — roughly ~52
   components total (matching trivy minus test-only).
2. **Given** a synthetic minimal Go project that imports just
   one external module which itself transitively requires three
   more, **When** mikebom scans, **Then** all four (the direct
   import + its three transitive prod requires) appear as
   components.
3. **Given** any existing Go fixture in `mikebom-cli/tests/fixtures/`
   that today produces N components on a `--path` scan, **When**
   the scan runs post-milestone, **Then** the result is a
   superset of N — every previously-emitted component is still
   present (no regression on direct imports).

---

### User Story 2 — Tag test-only deps via `is_dev` and existing `--include-dev` flag (Priority: P1, bundled with US1) 🎯 MVP

An operator scanning a Go source tree wants the SBOM to
distinguish test-only deps (e.g., `github.com/stretchr/testify`,
`github.com/davecgh/go-spew`, `github.com/pmezard/go-difflib`)
from prod deps. Today, the only ways to make that distinction
are:
- Drop test-only deps entirely (current default — too aggressive,
  fails US1).
- Emit them flat alongside prod deps with no marker (trivy's
  approach — fails the user's "I care if a dep is only test"
  requirement).

After this milestone, every test-only Go component carries
`is_dev = Some(true)` populated by the same source-import
reachability analysis the existing G4 filter does (just
re-purposed: tag instead of drop). The CDX/SPDX 2.3/SPDX 3
emission then surfaces this via the existing
`mikebom:dev-dependency = true` annotation already used for
npm / Poetry / Pipfile components — same C-row catalog entry,
no new annotation infrastructure.

`--include-dev` (off by default) controls inclusion, identical
to the existing semantics in other ecosystems:
- **Default (`--include-dev=off`)**: test-only Go deps are
  dropped from the SBOM. The default scan reports the prod
  transitive closure.
- **`--include-dev=on`**: test-only deps are emitted with
  `mikebom:dev-dependency = true` so consumers can include or
  filter as they need.

**Why this priority**: bundled with US1 because shipping the full
closure (US1) without test-vs-prod classification (US2) leaves
the user without the ability to filter — they'd see the same flat
list trivy gives, missing the project's stated value-add.
Together, US1 + US2 is the full deliverable.

**Independent Test**: After implementation, scanning a Go
project with both prod and test imports:
- Default scan emits ONLY the prod transitive closure (test-only
  deps absent).
- `--include-dev` scan emits prod + test, with test deps
  carrying `mikebom:dev-dependency = true` (a CDX `properties[]`
  entry, an SPDX 2.3 `packages[].annotations[]` entry, and an
  SPDX 3 top-level `annotations[]` entry — the existing
  cross-format triple already shipped for other ecosystems).

**Acceptance Scenarios**:

1. **Given** the `apigatewayv2/config` fixture (which has 3
   test-only deps: `stretchr/testify`,
   `davecgh/go-spew`, `pmezard/go-difflib`), **When** the
   operator runs `mikebom sbom scan --path .` (default,
   no `--include-dev`), **Then** none of the 3 test-only deps
   appear in the SBOM.
2. **Given** the same fixture, **When** the operator runs
   `mikebom sbom scan --path . --include-dev`, **Then** all 3
   test-only deps appear with `mikebom:dev-dependency = true`
   in CDX `properties[]`. SPDX 2.3 and SPDX 3 outputs carry the
   same annotation in their respective slots (cross-format
   parity via the existing C-row).
3. **Given** an npm or Poetry source tree with both prod and
   dev deps, **When** the operator runs the milestone-049 build
   with `--include-dev=on`, **Then** the existing semantics for
   those ecosystems are byte-identical pre-and-post milestone
   (this milestone only POPULATES `is_dev` for Go; npm /
   Poetry / Pipfile behavior unchanged).

---

### Edge Cases

- **Components reachable from BOTH prod and test imports**: a
  module imported by both `main.go` (prod) and `main_test.go`
  (test) is classified as **prod** (`is_dev = Some(false)`).
  Mirrors npm / Poetry — a package listed in both `dependencies`
  AND `devDependencies` is treated as prod. Documented in spec;
  the test-only classification requires the import to be EXCLUSIVELY
  reachable from `_test.go` files.
- **Indirect-only deps in go.sum** (entries that resolve via
  Go's MVS but aren't actually imported by any file in this
  project, including transitively through deps' go.mod
  `require` blocks): treated as **test-only**
  (`is_dev = Some(true)`). Rationale: if no source file in the
  project's prod-reachable closure imports them, they're not
  in the compiled binary, which is the relevant prod boundary.
  Acceptable approximation; downstream consumers running with
  `--include-dev` get full visibility.
- **Replaced modules** (`replace` directive in go.mod): the
  resolved replacement target is what gets emitted (not the
  original). Same as today's behavior; this milestone doesn't
  change `replace` handling.
- **Vendored deps** (`vendor/` directory present): out of scope
  for this milestone — vendor-mode Go scans are tracked as a
  separate item in `docs/design-notes.md`. Default behavior
  (read go.sum) is unchanged.
- **Components with versions resolved via `+incompatible` or
  `pseudo-versions`**: emitted unchanged with the resolved
  version string. Cosmetic concern only; no semantic impact.
- **Standard library imports** (e.g., `import "fmt"`): never
  appear as components — they're not in go.sum and don't have
  PURL coords. Same as today.
- **Already-emitted components**: any component the
  pre-milestone scan emitted MUST still be emitted with the
  same `is_dev` value (`Some(false)` in practice — those 6
  modules are all directly imported by prod source).
  Backwards-compatible deepening, not a behavioral pivot.

## Requirements *(mandatory)*

### Functional Requirements

#### US1 — Full transitive closure

- **FR-001**: `mikebom sbom scan --path <go-project>` MUST
  emit a component for every distinct external module in the
  project's `go.sum` that is reachable through any chain of
  prod imports (this project's non-`_test.go` source imports
  → those modules' transitive prod requires).
- **FR-002**: The emission MUST include indirect dependencies
  reachable through dependency chains (e.g., `aws-sdk-go-v2/config`
  → `aws-sdk-go-v2/credentials`), not just direct imports.
- **FR-003**: The emission MUST exclude Go standard-library
  packages (they have no PURL).
- **FR-004**: The number of emitted components on the
  audit-grounded fixture (`apigatewayv2/config`) MUST be at
  least 50 (with `--include-dev=off`) or at least 53 (with
  `--include-dev=on`), reflecting full transitive closure.

#### US2 — Test-vs-prod classification

- **FR-005**: For Go source-tree scans, mikebom MUST populate
  the `is_dev` field on every emitted Go `ResolvedComponent`:
  - `is_dev = Some(false)` when the component is reachable from
    at least one non-`_test.go` import (transitively through
    deps' go.mod `require` blocks).
  - `is_dev = Some(true)` when the component is reachable ONLY
    from `_test.go` imports (this project's tests transitively).
  - `is_dev = Some(true)` when the component is in `go.sum` but
    not reachable from any import (indirect-only-not-loaded —
    treated as test/indirect for filtering purposes).
- **FR-006**: When `is_dev = Some(true)`, the existing
  `mikebom:dev-dependency = true` annotation MUST appear in
  CDX `properties[]`, SPDX 2.3 `packages[].annotations[]`, and
  SPDX 3 top-level `annotations[]`. No new C-row needed; reuses
  the existing catalog row already shipped for other ecosystems.
- **FR-007**: The existing `--include-dev` CLI flag's semantics
  MUST extend to Go source-tree scans without behavior change to
  npm / Poetry / Pipfile:
  - Default (`--include-dev=off`): components with
    `is_dev = Some(true)` are dropped from emission.
  - `--include-dev` set: components are emitted with the
    annotation.

#### Cross-cutting

- **FR-008**: Goldens regen with semantic deltas only on Go
  fixtures (`mikebom-cli/tests/fixtures/golang/*`). Non-Go
  ecosystem fixtures (deb, apk, rpm, npm, cargo, gem, pip,
  maven) MUST be byte-identical pre-and-post milestone.
- **FR-009**: Pre-PR gate (`./scripts/pre-pr.sh`) clean.
  `holistic_parity` 11/11 ok. `every_catalog_row_has_an_extractor`
  passes (no new catalog row added by this milestone).
- **FR-010**: No new top-level Cargo dependencies. The
  reachability analysis uses existing infrastructure
  (`golang.rs::parse_go_mod`, the module-cache walker, the
  source-tree import collector that already powers the G4
  filter).
- **FR-011**: No CLI flag change beyond `--include-dev` extension
  (no new flag; no semantics shift on `--include-dev` for other
  ecosystems).

### Key Entities

- **Go module**: a module identified by its `module-path` +
  `version`. Each entry in `go.sum` corresponds to a module +
  version. mikebom emits one CDX component per distinct
  module (de-duplicating go.sum's two-line-per-module
  representation).
- **Reachability set**: for a Go source tree, the union of
  modules reachable from any non-`_test.go` import path
  (transitively through dep go.mod `require` blocks). The
  complement within go.sum (modules NOT in this set) is the
  test-only / indirect-only set.

## Success Criteria *(mandatory)*

### Measurable Outcomes

#### US1

- **SC-001**: Scanning `~/Projects/iac/app-code/apigatewayv2/config`
  with default flags emits **at least 50 components** in the
  CDX output (vs the current 6).
- **SC-002**: For every distinct external module in that
  project's `go.sum` that is reachable through prod imports,
  a corresponding component appears in the SBOM. Verified via:
  parse `go.sum` (~32 distinct modules), filter to prod-reachable,
  jq the SBOM for matching PURLs — count must equal.

#### US2

- **SC-003**: Scanning `~/Projects/iac/app-code/apigatewayv2/config`
  with `--include-dev` emits **at least 53 components**
  (the prod set + 3 test-only deps).
- **SC-004**: Each of the 3 test-only deps (`stretchr/testify`,
  `davecgh/go-spew`, `pmezard/go-difflib`) carries property
  `name = "mikebom:dev-dependency"`, `value = "true"` in the
  CDX output. SPDX 2.3 and SPDX 3 carry the same annotation.
- **SC-005**: Scanning the same fixture without `--include-dev`
  (default) does NOT emit any of those 3 test-only deps.
- **SC-006**: Existing fixtures (`mikebom-cli/tests/fixtures/golang/*`)
  produce SBOMs whose component sets are SUPERSETS of their
  pre-milestone component sets (no previously-emitted Go
  component is dropped).

#### Cross-cutting

- **SC-007**: Non-Go goldens (CDX + SPDX 2.3 + SPDX 3 across
  cargo, gem, pip, npm, deb, apk, rpm, maven) are byte-identical
  pre-and-post milestone. `git diff main..HEAD --
  mikebom-cli/tests/fixtures/golden/` shows changes only in
  files named `*golang*` or similar.
- **SC-008**: `./scripts/pre-pr.sh` clean. `holistic_parity`
  11/11 ok. All 3 CI lanes green on the milestone PR.

## Assumptions

- **The audit-grounded scan numbers are reliable**. The user
  shared a real `mikebom.cdx.json` (6 components) and
  `trivy.cdx.json` (55 components) for the
  `apigatewayv2/config` fixture. SC-001 / SC-003 thresholds
  derive from those measurements; if implementation surfaces a
  reason the counts shift (e.g., a different reachability
  algorithm than `go.mod Require`-walk), the SCs adjust to
  match — but the SHAPE of the requirement (full closure +
  test-tag) is settled.
- **`--include-dev` semantics are unchanged for non-Go
  ecosystems**. The flag already exists with documented
  behavior for npm / Poetry / Pipfile; this milestone only
  populates `is_dev` for Go components and lets the existing
  flag mechanism flow through.
- **No CHANGELOG entry needed** beyond a single Added/Changed
  line. The change is user-visible (new components in Go SBOMs
  by default) but mechanical from the user's perspective.
- **Component-role classifier (milestone 048) is orthogonal**.
  A Go test-only dep could in theory also live under a heuristic-
  matched path (unlikely in practice), in which case both
  annotations apply. No conflict.

## Out of scope

- **Vendored deps** (`vendor/` directory): scanning vendored
  deps directly. Today's behavior (read go.sum) is preserved;
  vendor-mode Go scans are a separate `docs/design-notes.md`
  deferred item.
- **Direct vs indirect** explicit annotation. Could add
  `mikebom:requirement-direct = true|false` keyed on whether
  the module appears in `go.mod`'s `require` block (vs only via
  transitive). Useful but not required by the user's stated
  needs; possible follow-on if downstream consumers ask.
- **Build-tag / OS-conditional reachability**: a module
  imported only under a specific `//go:build` tag is treated
  as prod (the tag is treated as always-active for reachability).
  Acceptable approximation; surveying conditional imports is a
  separate concern.
- **Go binary mode behavior**: this milestone only changes
  `--path` source-tree scans. Binary-mode scans
  (`runtime/debug.BuildInfo` parsing) already emit full
  transitive closure and don't need to change.
- **Other ecosystems** (cargo, gem, pip, npm, etc.): no change
  to transitive coverage in this milestone. Per the pre-spec
  audit, only Go has the transitive-coverage gap; cargo / gem /
  maven / npm / pip already emit full lockfile-driven closures.
  However, **test-vs-prod tagging is incomplete in cargo, gem,
  and maven** (only npm / Poetry / Pipfile / Go-after-this-
  milestone have `is_dev` populated). Tracked as the immediate
  follow-on **milestone 050** (cargo + gem + maven test-tagging
  extension) — same `is_dev` + `--include-dev` mechanism as
  this milestone, three additional ecosystem readers updated to
  populate the field from each ecosystem's test/dev manifest
  convention.
- **Conformance suite consumption** (declaring fixtures with
  test-dep awareness): sbom-conformance repo follow-on.
- **README explainer update** (milestone 047 added a "What kind
  of SBOM does mikebom emit?" section that describes
  `--include-dev` for npm/Poetry/Pipfile; that section may want
  a footnote noting Go-source-tree extension): out of scope; can
  be a small docs follow-on after this milestone merges.
