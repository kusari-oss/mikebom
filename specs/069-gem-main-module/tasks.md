---
description: "Task list for milestone 069 — gem source-tree main-module component"
---

# Tasks: gem source-tree main-module component for top-level *.gemspec roots

**Input**: Design documents from `/specs/069-gem-main-module/`
**Prerequisites**: spec.md ✅, plan.md ✅, data-model.md ✅, contracts/gem-main-module-component.md ✅, quickstart.md ✅

## Format: `[ID] [P?] [Story] Description`

## Phase 1: Setup

- [ ] T001 Confirm working tree clean and on branch `069-gem-main-module`.

## Phase 2: Foundational

- [ ] T002 [P] Update `docs/reference/sbom-format-mapping.md` C40 row to extend ecosystem-coverage matrix: Go ✅, cargo ✅, npm ✅, pip ✅, gem ✅; maven still in #104.

## Phase 3: User Story 1 — Ruby project SBOMs identify the project itself (P1) 🎯 MVP

### Implementation

- [ ] T003 [US1] Implement `find_top_level_gemspecs(rootfs: &Path) -> Vec<PathBuf>` in `mikebom-cli/src/scan_fs/package_db/gem.rs`. Walks rootfs alphabetically (cross-host determinism); returns every `*.gemspec` at a project root level (i.e., NOT inside `vendor/`, `gems/`, `specifications/`, `.bundle/`, or `node_modules/`). Reuse `should_skip_descent` from the existing walker. Distinct from `find_gemspecs` (which targets install-state `specifications/` dirs).
- [ ] T004 [US1] Implement `build_gem_main_module_entry(gemspec_path: &Path) -> Option<PackageDbEntry>` in `mikebom-cli/src/scan_fs/package_db/gem.rs`. Reads + parses via existing `parse_gemspec_full`. If `name` is unparseable → `None`. Else: build PURL via existing `build_gem_purl`; resolve version (literal `s.version` parsed → use; non-literal → `0.0.0-unknown` placeholder). Set `parent_purl: None`, `sbom_tier: Some("source")`, C40 annotation `mikebom:component-role: "main-module"`. Populate `depends` from existing `parse_gemspec_groups` (union runtime + dev-dependency keys, post-existing-scope-filter — `parse_gemspec_groups` returns groups; flatten to a single Vec<String>).
- [ ] T005 [US1] Implement `dedup_gem_main_modules_by_purl(entries: &mut Vec<PackageDbEntry>) -> Vec<DroppedDuplicate>` in `mikebom-cli/src/scan_fs/package_db/gem.rs`. Mirrors cargo (064 T010) / npm (066) / pip (068) C40-tag-driven dedup. Returns `DroppedDuplicate` for caller-side `tracing::warn!`.
- [ ] T006 [US1] Wire main-module emission into `gem::read()` in `mikebom-cli/src/scan_fs/package_db/gem.rs`. Phase A (after the existing `Gemfile.lock` + `find_gemspecs` install-state loops): walk `find_top_level_gemspecs(rootfs)`, call `build_gem_main_module_entry` per gemspec. Augment-existing-or-emit-new pattern: when a same-PURL Gemfile.lock-derived entry exists, layer C40 + `parent_purl: None` on top while preserving the existing entry's `depends` list (the Gemfile.lock-resolved deps are richer than the gemspec's). When no same-PURL match exists, emit net-new. Call `dedup_gem_main_modules_by_purl` + emit consolidated `tracing::warn!` for collisions. Add `tracing::info!` reporting `main_modules_emitted` count.
- [ ] T007 [US1] Add unit tests in `gem::tests` mod: (a) literal `s.name` + `s.version` → entry with PURL `pkg:gem/<name>@<version>`; (b) literal name + non-literal `s.version = SomeConstant` → entry with `0.0.0-unknown` placeholder; (c) `.freeze` chained on version literal (`s.version = "1.0.0".freeze`) → resolves to `1.0.0` (existing `parse_gemspec_full` handles); (d) name unparseable → returns `None`; (e) `dedup_gem_main_modules_by_purl`: no collision → empty Vec; two same-PURL → first kept, one DroppedDuplicate; predicate is C40-tag-driven (regular gem components untouched); (f) `find_top_level_gemspecs` excludes `vendor/`, `gems/`, `specifications/`, `.bundle/`.

### Tests for US1

- [ ] T008 [P] [US1] Create `mikebom-cli/tests/fixtures/gem-source-project/`: minimal `foo.gemspec` with `s.name = "foo"`, `s.version = "1.0.0"`, `s.summary = "demo gem"`, `s.add_dependency "rake"`, plus README.md.
- [ ] T009 [US1] Add integration tests in `mikebom-cli/tests/scan_gem.rs` (new file or extension): `scan_gem_top_level_gemspec_emits_main_module_in_metadata_component` (US1 AS#1 / SC-001), `scan_gem_non_literal_version_uses_placeholder` (US1 AS#2), `scan_gem_application_style_skips_main_module` (US1 AS#4 / FR-002 / SC-002 — Gemfile + Gemfile.lock without `*.gemspec` should emit no main-module), `scan_gem_install_state_paths_skipped` (FR-003 — synthesize a tempdir with both top-level `foo.gemspec` AND `vendor/bar/bar.gemspec`; only `foo` emits as main-module).

## Phase 4: Polish

- [ ] T010 Regenerate `mikebom-cli/tests/fixtures/golden/{cyclonedx,spdx-2.3,spdx-3}/gem.{cdx,spdx,spdx3}.json` if the existing gem golden fixture changes shape (verify by running tests pre-regen — likely no diff if the fixture is application-style or installed-tree-only). Apply cross-host playbook if regen is needed.
- [ ] T011 CHANGELOG.md `[Unreleased]` entry for milestone 069 — same structure as 064/066/068. Reference #104 (now only maven left), #103 (license follow-up), #125. Note this is the 4th per-ecosystem main-module milestone post-alpha.12 and updates the per-ecosystem coverage matrix to Go ✅, cargo ✅, npm ✅, pip ✅, gem ✅; maven still pending.
- [ ] T012 Run `./scripts/pre-pr.sh`; fix any issues. Update any pre-existing tests that assumed gem-app-style projects emit zero components if they now hit the milestone-069 main-module path (parallel to the milestone-068 updates that touched `pip::dist_info::tests`).
- [ ] T013 Open PR via `gh pr create` with title `feat(069): gem source-tree main-module component (closes gem slice of #104)`.

## Dependencies

```text
T001 → T002 → [T003 → T004 → T005 → T006 → T007] (US1 implementation chain)
                    → T008 (fixture, parallel with helpers)
                    → T009 (integration tests, depends on T006 wire-up)
                          → [T010, T011] → T012 → T013
```

## Format validation

All 13 tasks follow the required format. Setup (T001), Foundational (T002), US1 (T003-T009), Polish (T010-T013).

## MVP scope

US1 alone covers SC-001 + SC-002 + FR-002 (the dominant value). US2 (consumer signal) and US3 (doc root) inherit from milestones 053+064+066+068+#127 with zero additional implementation work — verifying them is implicit in the pre-PR test sweep at T012.
