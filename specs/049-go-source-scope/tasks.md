---
description: "Task list — milestone 049 Go source-tree full transitive closure with test-vs-prod tagging"
---

# Tasks: Go source-tree full transitive closure with test-vs-prod tagging

**Input**: spec.md ✅, plan.md ✅, checklists/requirements.md ✅. (No
research.md / data-model.md / contracts/ / quickstart.md — same
4-file tighter template milestones 021/022/023/042/046/047/048
use; plan resolves the integration-point lookups inline in §Phase 0.)

**Tests**: included as inline unit tests on the new
`collect_test_imports` helper plus updates to the existing
`scan_go_source_test_only_import_is_dropped` integration test.
End-to-end coverage via existing Go fixture goldens (unchanged
because simple-module has no .go source) + `holistic_parity`
continuing to pass + real-world smoke on apigatewayv2/config.

**Organization**: Two user stories. US1 (full transitive closure)
and US2 (test-vs-prod tagging) share the same Go-reader code
path and the same G4-classifier inversion — bundled into one
commit. US3 doesn't exist (this milestone has only US1 + US2 per
spec).

## Format: `[ID] [P?] [Story?] Description`

---

## Phase 1: Setup

- [X] T001 Confirm clean working tree on branch `049-go-source-scope`. `git status` shows only the un-tracked `specs/049-go-source-scope/` scaffolding from `/speckit.specify` + the un-tracked `CLAUDE.md` from `/speckit.plan`.
- [X] T002 `./scripts/pre-pr.sh` clean (baseline; should pass since no edits yet).

---

## Phase 2: Foundational

(No foundational tasks — every change in this milestone lives in
the Go reader and its single call site. The "shared infrastructure"
of `is_dev` field + `--include-dev` flag + C6 catalog row +
parity-extractor wiring + CDX/SPDX emission paths is all already
in place per Phase 0 R4. This milestone is pure additive
population on existing infrastructure.)

---

## Phase 3: Commit `feat(049/us1+us2)` — Go transitive closure + test-tagging (single commit)

**Goal**: Bundle US1 (full transitive closure of go.sum prod-reachable modules) and US2 (test-only deps tagged via `is_dev = Some(true)`) into one commit because they share code paths. The G4 filter at `package_db/mod.rs::apply_go_production_set_filter` is rewritten from "drop test-only" to "tag test-only and conditionally drop based on `--include-dev`."

**Independent test**: SC-001 + SC-002 (≥50 components default on apigatewayv2/config; full prod-reachable closure of go.sum), SC-003 + SC-004 + SC-005 (test-only deps tagged with `mikebom:dev-dependency = true` when `--include-dev`; absent when `--include-dev=off`), SC-006 (existing Go fixture component sets are supersets of pre-milestone), SC-007 (non-Go goldens unchanged).

### New helpers in `golang.rs`

- [X] T003 [US2] Add `collect_test_imports` to `mikebom-cli/src/scan_fs/package_db/golang.rs`. Mirrors `collect_production_imports` but walks ONLY `_test.go` files. Implemented via a shared `collect_imports_filtered(scope: FileScope)` helper + `FileScope::{ProdOnly, TestOnly}` enum so both functions share the directory-walk logic.
- [X] T004 [US1] **Pivoted from BFS to direct source-walk.** Original plan was to add a `compute_transitive_prod_set` helper that BFS-walked dep go.mod `require` blocks via `cache_lookup_depends`. Discarded after the integration test surfaced the bug: deps' go.mod can declare modules purely for the dep's own tests (logrus → testify), so BFS over-promotes test-only deps to prod. Replaced with: emit every go.sum entry as a component (FR-001) and only TAG the small subset proven test-only by source-walking this project's `_test.go` files.
- [X] T005 [P] [US1] [US2] Inline tests in `golang.rs::tests`: `collect_test_imports_records_only_test_files` (mixed `main.go` + `main_test.go` synthetic rootfs); `collect_test_imports_records_modules_imported_from_both` (verifies test-only-difference is empty when shared).

### G4 classifier rewrite in `package_db/mod.rs`

- [X] T006 [US1] [US2] Rewrote `mikebom-cli/src/scan_fs/package_db/mod.rs::apply_go_production_set_filter` (lines 372-434). New signature accepts `production_imports`, `test_only_imports`, and `include_dev` (instead of the prior `production_imports` + drop-non-prod logic). Behavior:
    1. For each Go source-tier `PackageDbEntry`, if its module path is in `test_only_imports` → set `is_dev = Some(true)`.
    2. Indirect transitives (in neither prod nor test imports) pass through unchanged (default to prod-untagged).
    3. If `!include_dev`, drop entries with `is_dev = Some(true)` from `entries`.
- [X] T007 [US1] [US2] Updated call site at `mod.rs:688` to pass `&go_signals.production_imports`, `&go_signals.test_only_imports`, and `include_dev`.
- [X] T008 [US1] [US2] Updated diagnostic log message: "G4 classifier: tagged Go test-only modules; dropped tagged entries when --include-dev=off" with `tagged_test_only`, `dropped_when_no_include_dev`, `production_imports`, `include_dev` fields.

### Real-world smoke (manual, documented in PR description)

- [X] T009 [US1] [US2] Built `--release` binary; smoke against `~/Projects/iac/app-code/apigatewayv2/config`: 63 components default (SC-001 ≥50 ✅, was 6 pre-049, ~10× growth).
- [X] T010 [US2] Smoke with `--include-dev`: 64 components, 1 dev-tagged (`github.com/stretchr/testify`). Note: only 1 test-dep tagged (not 3), because `davecgh/go-spew` and `pmezard/go-difflib` are go.sum-indirect deps not directly imported from this project's `_test.go` source — they pass through as prod-indirect under the simpler design. SC-003 (≥53 ✅), SC-004 (testify tagged ✅).
- [X] T011 [US2] SC-005 verified: testify absent in default-mode SBOM (`jq` of `/tmp/mikebom-049-default.cdx.json` confirms 63 components, no testify entry, only logrus-and-friends as prod).

### Goldens regen + verification

- [X] T012 [US1] [US2] Goldens regen NOT needed: simple-module fixture has no `.go` source, so `production_imports` is empty → filter no-ops → output is identical to pre-049. All 9 cdx + 9 spdx goldens still pass byte-identical.
- [X] T013 [US1] [US2] SC-007 verified: all 27 byte-identity goldens (9 ecosystems × 3 formats) unchanged.
- [X] T014 [US1] [US2] `holistic_parity` 11/11 ok.

### Pre-PR + commit

- [X] T015 [US1] [US2] `./scripts/pre-pr.sh` clean.
- [ ] T016 [US1] [US2] Commit: `feat(049/us1+us2): Go source-tree full transitive closure with test-vs-prod tagging via is_dev`.

---

## Phase 4: Commit `chore(049)` — CHANGELOG + spec scaffolding

- [ ] T017 Edit `CHANGELOG.md` under `[Unreleased]` → `### Changed`: name the Go source-tree behavior expansion. Cover (a) default scan now emits full transitive closure (not just direct imports); (b) test-only deps tagged via existing `mikebom:dev-dependency = true` annotation; (c) `--include-dev` flag (default off) drops test-only deps from emission, mirroring npm/Poetry/Pipfile semantics; (d) no new flag, no new annotation, no new catalog row.
- [ ] T018 Stage `specs/049-go-source-scope/` (spec.md, plan.md, tasks.md, checklists/requirements.md) and `CLAUDE.md` (auto-updated by `update-agent-context.sh` during /speckit.plan).
- [ ] T019 `./scripts/pre-pr.sh` clean.
- [ ] T020 Commit: `chore(049): CHANGELOG entry + speckit spec/plan/tasks scaffolding`.

---

## Phase 5: Polish & PR

- [ ] T021 Verify SC-008 (pre-PR + CI green): final `./scripts/pre-pr.sh` clean from a fresh shell.
- [ ] T022 Push branch: `git push -u origin 049-go-source-scope`.
- [ ] T023 Open PR titled `feat(049): Go source-tree full transitive closure + test-vs-prod tagging via is_dev`. Body covers: 2-commit summary, audit-grounded rationale (the apigatewayv2/config 6→55 gap), 8 SC verification commands (SC-001 through SC-008), the milestone-050 follow-on pointer for cargo/gem/maven test-tagging, real-world smoke test results from T009-T011.
- [ ] T024 Verify SC-008 (CI lanes): all 3 CI lanes (linux x86_64, linux ebpf, macos-latest) green on the PR.

---

## Dependency graph

```text
T001-T002 (setup, baseline)
   │
   ▼
T003-T005 (new helpers in golang.rs: collect_test_imports +
           compute_transitive_prod_set + inline tests)
   │
   ▼
T006-T008 (G4 classifier rewrite + call-site update + log update
           in package_db/mod.rs)
   │
   ▼
T009-T011 (real-world smoke against apigatewayv2/config — manual,
           verifies SC-001 through SC-005)
   │
   ▼
T012-T014 (goldens regen + non-Go-unchanged verification +
           holistic_parity verification)
   │
   ▼
T015-T016 (pre-PR + commit US1+US2)
   │
   ▼
T017-T020 (Phase 4 — CHANGELOG + scaffolding commit)
   │
   ▼
T021-T024 (Phase 5 — verify + push + PR + CI)
```

**Why US1 and US2 share a single commit**: they're built from the
same code paths. The G4 filter inversion at `package_db/mod.rs`
both expands the prod-set (US1's transitive closure) AND tags
test-only entries (US2's `is_dev` population). The new helpers in
`golang.rs` (`collect_test_imports`, `compute_transitive_prod_set`)
serve both. Splitting US1 and US2 into separate commits would
require shipping a half-implemented classifier in commit 1 — not
honest.

## Parallel opportunities

| Bucket | Parallel-eligible tasks |
|---|---|
| Phase 3 helpers | T003 + T004 (different new functions, same file → sequential within file but logically parallel) |
| Phase 3 tests | T005 (inline tests for both helpers — compose into one test module) |
| Phase 3 verification | T013 + T014 (different test invocations, fully parallel) |
| Phase 4 | T017 + T018 (CHANGELOG + staging — different files) |

## Estimated effort

| Phase | Effort | Notes |
|---|---|---|
| Phase 1 (setup) | 5 min | Just baseline check |
| Phase 3 helpers (T003-T005) | 1 hr | New code paths + inline tests |
| Phase 3 classifier rewrite (T006-T008) | 30 min | Mostly modifying existing function |
| Phase 3 smoke (T009-T011) | 30 min | Real-world fixture verification |
| Phase 3 goldens + verification (T012-T014) | 30 min | Regen + diff inspection |
| Phase 3 commit (T015-T016) | 10 min | Mechanical |
| Phase 4 (T017-T020) | 10 min | Mechanical |
| Phase 5 (T021-T024) | 15 min | Push + PR + CI watch |
| **Total** | **~3 hr** | One focused session |

## MVP scope

**The MVP is US1+US2 bundled — full transitive closure + test-vs-prod tagging.** US1 alone (closure without classification) ships a flat trivy-style list with no value-add over trivy; US2 alone (tagging without closure expansion) keeps the current "direct imports only" undercount. Neither in isolation closes the user-reported gap on the apigatewayv2/config scan. Together they close it cleanly.

The milestone-050 follow-on (cargo + gem + maven test-tagging extension) is **separately scoped per the spec's Out-of-scope section** and ships in a future PR. Not part of this milestone's MVP.
