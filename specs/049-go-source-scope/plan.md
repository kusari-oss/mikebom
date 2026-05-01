# Implementation Plan: Go source-tree full transitive closure with test-vs-prod tagging

**Branch**: `049-go-source-scope` | **Date**: 2026-05-01 | **Spec**: [spec.md](./spec.md)
**Input**: [spec.md](./spec.md)

## Summary

Two structural changes to the Go reader in
`mikebom-cli/src/scan_fs/package_db/golang.rs` + the G4 filter in
`mikebom-cli/src/scan_fs/package_db/mod.rs`:

1. **Replace the "drop test-only entries" G4 filter with a "tag
   test-only entries" classifier.** Same source-import
   reachability analysis the filter already does today;
   different output policy (set `is_dev = Some(true)` instead of
   dropping the entry).
2. **Extend the prod-set from "this project's direct imports" to
   "transitive closure of those imports through deps' `go.mod`
   `require` blocks."** mikebom already reads each dep's go.mod
   (`cache_lookup_depends` at golang.rs:570-580) for relationship-
   edge emission; this milestone uses the same data to expand
   the reachability set used by the classifier.

Result: full transitive go.sum closure emitted by default, with
test-only deps cleanly classified and filterable via the existing
`--include-dev` flag.

**Zero new infrastructure**:
- ✅ No new C-row (reuses existing **C6** `mikebom:dev-dependency`)
- ✅ No new CLI flag (reuses existing `--include-dev`)
- ✅ No new parity-extractor wiring (the existing C6 row already
  ships SPDX 2.3 + SPDX 3 emission)
- ✅ No new crate dependency (uses existing `golang.rs` cache
  reader + existing source-import collector)

## Technical Context

**Language/Version**: Rust stable.
**Primary Dependencies**: existing only — `std::collections`,
`std::path`. No new crates.
**Storage**: N/A.
**Testing**: existing inline tests in `golang.rs` cover the
parsers; new inline tests cover the prod-vs-test classification +
transitive-closure walk. Existing 27 byte-identity goldens cover
the cross-format wiring (the C6 emission path is already exercised
by npm/Poetry/Pipfile fixtures).
**Target Platform**: cross-platform.
**Project Type**: code-modifying milestone (Go reader + G4 filter
+ Go fixture goldens).
**Performance Goals**: N/A — the transitive-closure walk is
O(modules × deps) but bounded by go.sum size (~thousands max);
each dep go.mod is already cached on disk and read at single-pass.
**Constraints**: zero diff on **non-Go** goldens (cargo, gem, pip,
npm, deb, apk, rpm, maven). Go fixture golden regenerates with
the new prod-closure expansion.
**Scale/Scope**: ~120 LOC of Rust modifications + ~30 LOC of
new tests + 1 set of Go fixture goldens regenerated.

No NEEDS CLARIFICATION markers — Phase 0 research below resolves
the small remaining plan-level questions.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1
design.*

| Principle | Engaged? | Status |
|---|---|---|
| I. Pure Rust, Zero C | Yes (Rust-only edits) | ✅ |
| II. eBPF-Only Observation | No (no discovery code change; this is enrichment over already-discovered Go components) | ✅ vacuous |
| III–IV. (existing principles) | No code change to scan / generate hot path beyond Go reader | ✅ vacuous |
| V. Specification Compliance (SPDX 2.3 / 3.x labeling) | Yes — uses existing C6 emission shape; no format violation | ✅ |
| VI. Three-crate architecture | No `mikebom-common` / `mikebom-ebpf` change beyond reading the existing `is_dev` field on `ResolvedComponent` | ✅ untouched |
| Pre-PR Verification | All commits' `./scripts/pre-pr.sh` clean | ✅ enforced by SC-008 |

No gate violations.

## Phase 0: Research (resolved inline)

Pre-spec recon resolved the integration points:

### R1. G4 filter location

**Decision**: invert in place at
`mikebom-cli/src/scan_fs/package_db/mod.rs::apply_go_production_set_filter`
(lines 360-387). The function currently drops Go entries not in
`production_imports`; the new behavior tags them
`is_dev = Some(true)` instead, with one subsequent pass that
respects `--include-dev` (drops the tagged entries when the flag
is off).

**Alternative considered**: split into two functions
(`compute_test_only_set` + `tag_or_drop`). Rejected — the
single-function form is closer to the existing shape and keeps
the diff smaller.

### R2. Source-import collection (already test-vs-prod aware)

**Existing primitive**:
`mikebom-cli/src/scan_fs/package_db/golang.rs::collect_production_imports`
(lines 712-762) walks `.go` files **excluding `_test.go`** and
records prod imports. The test-vs-prod split is **inherent** —
test files are skipped entirely.

**Decision**: add a parallel `collect_test_imports` function
(same loop, includes only `_test.go` files). Compute
`test_only_imports = test_imports - prod_imports` (set
difference). The result is the set of modules reachable ONLY
from test source.

### R3. Transitive prod scope

**Output set is go.sum (FR-001).** Every entry in go.sum is emitted
as a component. We do NOT compute a separate transitive prod
"closure" via go.mod-`require` BFS — that's a dead-end for
test-vs-prod tagging because a dep's go.mod can declare a module
purely for the dep's own tests (e.g., logrus's go.mod declares
testify). Walking those edges would falsely promote test-only
deps to prod whenever a transitively-prod dep also `require`s
them.

**Decision**: keep the prod set narrow. `production_imports` =
direct non-test imports of this project. `test_only_imports` =
`test_imports - production_imports`. Indirect transitives in
go.sum that this project doesn't directly import (prod or test)
pass through as prod. The classifier only ever TAGS
`test_only_imports`; it never drops indirect deps.

This is correct semantics: at this project's compile,
go.sum lists everything `go mod tidy` ever fetched (including
transitive test-of-test deps), but only modules that this
project's own non-test source imports — directly OR
transitively-via-other-prod-modules-in-our-source — end up in
the binary. We over-include slightly (some indirect-only deps
might in fact be test-only at consumer level), but we never
drop a legitimately-loaded module. Mirrors what trivy/syft do:
trust go.sum for output scope.

### R4. Existing infrastructure (no change)

- **C6 catalog row** at `docs/reference/sbom-format-mapping.md`
  line 51 already documents `mikebom:dev-dependency`.
- **Parity extractors** for C6 already wired across CDX +
  SPDX 2.3 + SPDX 3.
- **CDX serializer** already conditionally emits the property at
  `cyclonedx/builder.rs:315` based on `include_dev && is_dev ==
  Some(true)`. Same gate exists for SPDX 2.3 (`spdx/annotations.rs:147`)
  and SPDX 3 (`spdx/v3_annotations.rs:163`).
- **`--include-dev` plumbing**: parsed at `cli/scan_cmd.rs:584`,
  threaded through to readers. Go reader already receives it; no
  new threading.
- **Collection-time skip pattern**: npm walker (line 191 of
  `npm/walk.rs`) drops entries when `is_dev = true` AND
  `!include_dev`. Go reader will mirror this at the post-G4-tag
  pass.

### R5. Fixture impact

**Single source-tree fixture**: `tests/fixtures/go/simple-module/`
(go.mod with 5 direct + 10 indirect requires; go.sum with ~13
distinct modules). Current golden emits 11 components. Post-
milestone:
- Default scan should emit ≥ same 11 (no fewer; the prod set is
  a superset of the previous "direct imports" set).
- With `--include-dev` ON, may emit a few more if any of the
  current-dropped go.sum entries are test-only (need to verify
  during implementation by running the regen).

**Real audit fixture** (separate from CI): user's
`apigatewayv2/config` → 6 → ≥ 50 default; ≥ 53 with
`--include-dev`. Verified empirically by re-running mikebom
post-implementation. Documented in spec SC-001 / SC-003.

**Other Go fixtures**: `tests/fixtures/go/binaries/` is
binary-mode (BuildInfo) and untouched by this milestone — its
golden stays byte-identical.

## Approach

Two commits, ordered so the source-tree behavior change lands
first and the goldens regen reflects the new emission.

### Commit 1 — `feat(049/us1+us2)` — Go transitive closure + test-tagging

Touched files:

- **`mikebom-cli/src/scan_fs/package_db/golang.rs`**
  - Add `collect_test_imports` mirroring `collect_production_imports`
    but INCLUDING only `_test.go` files (inverse predicate).
    Implemented via a shared `collect_imports_filtered` helper
    + `FileScope::{ProdOnly,TestOnly}` enum.
  - Compute `signals.test_only_imports = test_imports -
    production_imports` (set difference, source-walk only).
  - Inline tests covering: prod/test split on a synthetic rootfs
    with `main.go` + `main_test.go`; module imported from BOTH
    yields empty test-only.

- **`mikebom-cli/src/scan_fs/package_db/mod.rs`**
  - Rewrite `apply_go_production_set_filter` (lines 372-434):
    instead of dropping entries not in `production_imports`,
    only TAG entries whose module path is in `test_only_imports`
    with `is_dev = Some(true)`. When `!include_dev`, drop tagged
    entries. Indirect transitives in go.sum (in neither prod nor
    test imports) pass through unchanged.
  - Rename log message: "G4 classifier: tagged X test-only
    modules; dropped Y when --include-dev=off".
  - Update the call site to pass `production_imports` and
    `test_only_imports` from `GoScanSignals`, plus `include_dev`.

- **`mikebom-cli/tests/fixtures/golden/cyclonedx/golang.cdx.json`**
  + SPDX 2.3 + SPDX 3 goldens for the Go fixture: regen with the
  new prod-closure emission. Expected: same components as
  before plus any newly-included transitive prod deps.

Verification:
- `cargo +stable test -p mikebom -- package_db::golang
  package_db::mod` — all unit tests pass (existing + new).
- `MIKEBOM_UPDATE_*_GOLDENS=1 cargo +stable test -p mikebom
  --test cdx_regression --test spdx_regression --test spdx3_regression`
  — Go goldens regen; non-Go goldens stay byte-identical.
- `git diff main..HEAD --
  mikebom-cli/tests/fixtures/golden/cyclonedx/cargo.cdx.json` etc.
  → empty (verifies non-Go ecosystems are untouched).
- Real-world manual smoke: `mikebom sbom scan --path
  ~/Projects/iac/app-code/apigatewayv2/config` → ≥ 50
  components default; ≥ 53 with `--include-dev`.

### Commit 2 — `chore(049)` — CHANGELOG + spec scaffolding

- `CHANGELOG.md` `[Unreleased]` entry under Changed: name the
  Go source-tree behavior expansion, default-vs-`--include-dev`
  semantics, no new flag.
- `specs/049-go-source-scope/` scaffolding (spec.md + plan.md +
  tasks.md + checklists/requirements.md).
- `CLAUDE.md` if `update-agent-context.sh` produced any change.

## Touched files

| File | Commit | LOC |
|---|---|---|
| `mikebom-cli/src/scan_fs/package_db/golang.rs` | 1 | +80 source / +30 tests |
| `mikebom-cli/src/scan_fs/package_db/mod.rs` | 1 | ~30 modified |
| `mikebom-cli/tests/fixtures/golden/cyclonedx/golang.cdx.json` | 1 | regen |
| `mikebom-cli/tests/fixtures/golden/spdx-2.3/golang.spdx.json` | 1 | regen |
| `mikebom-cli/tests/fixtures/golden/spdx-3/golang.spdx3.json` | 1 | regen |
| `CHANGELOG.md` | 2 | +1 entry |
| `specs/049-go-source-scope/` | 2 | scaffolding |

Total: ~140 LOC of Rust + 3 Go-fixture goldens + scaffolding.

## Risks

- **R1: Indirect-transitive deps default to prod.** Modules in
  go.sum that this project doesn't directly import (prod or
  test) — e.g., aws-sdk-go-v2 internals pulled by gin —
  pass through as prod (unmarked). This is correct in the
  common case but conservative: theoretically a dep's
  test-only requires could land in go.sum and look like a
  prod indirect to us. Trivy and syft have the same blind spot.
  Distinguishing would require unzipping every dep's source
  zip in `$GOMODCACHE` and walking imports — too expensive.
  Spec'd behavior: tag conservatively when source-walk doesn't
  prove test-only; over-include unmarked rather than drop.

- **R2: Go fixture golden churn.** The current
  `tests/fixtures/go/simple-module/` golden has 11 components.
  Post-milestone the count may shift — could be the same 11
  (if all currently-emitted modules are still prod-reachable
  AND the fixture has no test-only entries to add) OR more (if
  any indirect requires get newly included). The
  goldens-regen step in commit 1 will surface the actual delta;
  worst case we discover a transitive dep that wasn't being
  emitted today and now is. That's correct behavior, not a
  regression.

- **R3: `--include-dev` semantics differ from npm.** npm's
  `--include-dev` controls inclusion (drop is_dev=true entries
  when off). pip/Poetry populate the field but downstream
  serializers gate the property emission, while the entry stays
  in components[]. Go could go either way. Spec requires the
  npm pattern (drop test-only at collection time when off) for
  the user's stated need. Plan locks in this choice in the
  classifier function.

- **R4: holistic_parity must remain green.** Adding `is_dev`
  population for Go MAY surface previously-untested cross-format
  parity edges if any Go fixture component now carries
  `is_dev=true`. The C6 row's parity extractors are already
  wired so this should pass automatically; verify post-regen.

## Phasing

| Phase | Commits | Effort |
|---|---|---|
| Setup + recon | done (Phase 0 above) | 0 |
| Commit 1 (classifier + transitive walk + tests + goldens) | 1 | 2 hr |
| Commit 2 (CHANGELOG + scaffolding) | 1 | 10 min |
| Verify + PR | 0 | 30 min (incl. real-world apigatewayv2/config smoke) |
| **Total** | **2 commits** | **~3 hr** |

## What this milestone does NOT do

- Does NOT change non-Go ecosystems. cargo / gem / maven test-
  tagging is **milestone 050** per the spec's Out-of-scope.
- Does NOT add `mikebom:requirement-direct` or similar
  direct-vs-indirect markers. Could be future-milestone if
  consumers need it; today's `--include-dev` covers the user's
  stated need.
- Does NOT change vendor-mode Go scans. Still tracked as a
  separate `docs/design-notes.md` deferred item.
- Does NOT change Go binary-mode scans (BuildInfo path) — those
  already emit full transitive closure.
- Does NOT change the existing `mikebom:dev-dependency` C-row,
  parity-extractor wiring, CDX/SPDX emission paths, or
  `--include-dev` flag semantics.
- Does NOT introduce build-tag / OS-conditional reachability.
  All `//go:build`-conditional imports are treated as prod.
- Does NOT update the milestone-047 README explainer ("What kind
  of SBOM does mikebom emit?") with a Go-specific footnote.
  Optional follow-on doc PR.

## Why no `data-model.md` / `contracts/` / `quickstart.md`

Same rationale milestones 021/022/023/042/046/047/048 used (the
project's tighter 4-file template):

- `data-model.md`: no new types. `is_dev`, `extra_annotations`,
  `ResolvedComponent` are all existing.
- `contracts/`: no new public-API surface change. CLI flag set
  unchanged. Output format spec unchanged. The behavior change
  is contained in the Go reader's emission policy.
- `quickstart.md`: spec's User Stories include grep / jq /
  cargo-test acceptance scenarios that read like quickstart
  steps.

This is the eighth use of the tighter 4-file template — pattern
stable for genuinely contained, additive milestones.
