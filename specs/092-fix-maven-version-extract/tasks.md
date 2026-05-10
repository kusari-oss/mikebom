---
description: "Task list for milestone 092 — fix Maven pom.xml version-extraction bug"
---

# Tasks: Fix Maven pom.xml version-extraction bug (closes #175 partially)

**Input**: Design documents from `/Users/mlieberman/Projects/mikebom/specs/092-fix-maven-version-extract/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Included. mikebom's pre-PR gate (CLAUDE.md mandatory) runs `cargo +stable test --workspace`; every milestone ships test coverage for its production change.

**Organization**: Tasks are grouped by user story. US1 (P1, the main-module version fix) is the MVP increment; US2 (P2, property substitution) is layered on top of the same `self_version` field added in Foundational.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- File paths are absolute or workspace-relative.

## Path Conventions

Single-crate Rust workspace fix in `mikebom-cli`. All production changes land in one file: `mikebom-cli/src/scan_fs/package_db/maven.rs`. New integration test in `mikebom-cli/tests/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm branch + build state before touching production code.

- [X] T001 Confirm working branch is `092-fix-maven-version-extract` and the working tree matches the spec's expected starting state. Run `git status` + `git log -1` and verify the branch was created by `/speckit.specify`.
- [X] T002 Run baseline `./scripts/pre-pr.sh` once to confirm the pre-092 workspace passes clippy + tests cleanly (so any test failure post-edit is unambiguously caused by milestone 092, not pre-existing flakes).

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Add the `self_version` field to `PomXmlDocument` and wire it up in `parse_pom_xml`. Both US1 (main-module fix) and US2 (property substitution) read from this field, so it MUST exist before either story's consumer-update tasks can land.

**⚠️ CRITICAL**: No user story work can begin until T003 + T004 complete.

- [X] T003 Add `pub self_version: Option<String>` field to `PomXmlDocument` struct in `mikebom-cli/src/scan_fs/package_db/maven.rs` (~line 546, immediately after `self_artifact_id`). Place the field with a doc-comment that mirrors the existing `self_artifact_id` rationale, per `data-model.md`'s post-092 shape. Update the surrounding doc-comment on `self_artifact_id` (lines 540-546) to also reference `self_version` for the parallel `<version>` inheritance gap.
- [X] T004 Wire `self_version` in `parse_pom_xml` at `mikebom-cli/src/scan_fs/package_db/maven.rs` (~line 711). Add `doc.self_version = self_v.clone();` immediately after the existing `doc.self_artifact_id = self_a.clone();` line. Verify the constructor at line 703-707 (the `if let (Some(g), Some(a)) = ...` block) is **left unchanged** per research.md §1's rationale (preserves `self_coord` semantics for `MavenInheritanceContext::by_coord`).
- [X] T005 [P] Run `cargo +stable build -p mikebom` to confirm the struct change compiles. No behavioral change yet (no consumer reads `self_version`); this is a compile-only checkpoint.

**Checkpoint**: Foundation ready — `PomXmlDocument` carries the project's own version even when project-level `<groupId>` is absent. US1 and US2 implementations can now begin.

---

## Phase 3: User Story 1 - Operator gets correct version on main-module component (Priority: P1) 🎯 MVP

**Goal**: Operator scanning a Maven project with a `<parent>` element sees the project's main-module emitted with the **project's own** version (`pkg:maven/<group>/<artifact>@<project-version>`), NOT the parent POM's version.

**Independent Test**: `target/release/mikebom sbom scan --path $MIKEBOM_FIXTURES_DIR/transitive_parity/maven --format spdx-2.3-json --output /tmp/post-092.spdx.json`. Confirm the emitted SBOM contains `pkg:maven/org.apache.commons/commons-lang3@3.14.0` (NOT `@64`). Recipe 3 from `quickstart.md`.

### Tests for User Story 1

> **NOTE**: TDD-style — write the regression test FIRST, confirm it fails pre-fix, then apply T008 and confirm it passes.

- [X] T006 [P] [US1] Add unit test `parse_pom_xml_extracts_self_version_when_groupId_inherited` to `mikebom-cli/src/scan_fs/package_db/maven.rs`'s test module. Construct a synthetic pom.xml byte string matching the commons-lang3 shape (parent block with `<groupId>`/`<artifactId>`/`<version>`, no project-level `<groupId>`, project-level `<artifactId>` + `<version>`). Assert: `doc.self_coord == None`, `doc.self_artifact_id == Some("commons-lang3")`, `doc.self_version == Some("3.14.0")`, `doc.parent_coord == Some(("org.apache.commons", "commons-parent", "64"))`. Cover the four `parse_pom_xml`-level rows from `contracts/maven-version-extraction.md` Contract 1's test-case table.
- [X] T007 [P] [US1] Create new integration test file `mikebom-cli/tests/maven_pom_version_extraction.rs` and add three tests covering the FR-001 / FR-002 / FR-003 trio (so the post-fix fallback chain is fully exercised in one place). Mirror the temp-dir pattern from `scan_maven.rs`. Tests:
    - **`main_module_emits_project_version_when_groupId_inherited_from_parent`** (FR-001 / SC-001): pom.xml with parent block (groupId+artifactId+version) and project-level `<artifactId>`/`<version>` only (no project-level `<groupId>`). Assert main-module PURL = `pkg:maven/org.apache.commons/commons-lang3@3.14.0`. MUST fail pre-fix (emits `@64`) and pass post-fix.
    - **`main_module_falls_back_to_parent_version_when_project_version_absent`** (FR-002 / closes coverage gap C2): pom.xml with parent block (with `<version>1.0</version>`) and project-level `<artifactId>` only (NO project-level `<version>`, NO project-level `<groupId>`). Assert main-module PURL emission uses parent's `1.0`. Confirms the post-fix chain still falls through to `parent_coord.2` when both `self_coord` AND `self_version` are `None`.
    - **`main_module_emits_nothing_when_both_versions_absent`** (FR-003 / closes coverage gap C1): pom.xml with parent block lacking `<version>` AND project lacking `<version>`. Assert NO main-module `PackageDbEntry` is emitted (or emitted with empty `version` and dropped by the existing `is_empty()` guard at maven.rs:3436). No malformed PURL appears in output.

### Implementation for User Story 1

- [X] T008 [US1] Update `build_maven_main_module_entry`'s `raw_version` resolution chain in `mikebom-cli/src/scan_fs/package_db/maven.rs` (~line 3412) to insert `.or_else(|| doc.self_version.clone())` between the existing `self_coord.2` lookup and the `parent_coord.2` fallback. Exact post-edit shape per `contracts/maven-version-extraction.md` Contract 2. Do not modify the `raw_group` or `raw_artifact` chains at lines 3402-3411.
- [X] T009 [US1] Run `cargo +stable test -p mikebom --test maven_pom_version_extraction` and `cargo +stable test -p mikebom maven::tests::parse_pom_xml_extracts_self_version_when_groupId_inherited` (or whichever test-name pattern resolves). Confirm both pass. If either fails, debug — do NOT mark T008 complete until both pass.

**Checkpoint**: US1 is fully functional and testable independently. The commons-lang3 fixture now emits the correct PURL. The milestone-083 transitive-parity baseline can be unfrozen (Phase 5 task).

---

## Phase 4: User Story 2 - No regression for `<version>${property}` substitution (Priority: P2)

**Goal**: A `pom.xml` that uses `${revision}` or `${project.version}` for its own version OR for dep versions continues to resolve correctly post-092. Specifically, `${project.version}` resolved inside a child POM whose `<groupId>` is parent-inherited returns the project's own version (not the parent's).

**Independent Test**: `cargo +stable test -p mikebom maven_property_substitution` (or the existing maven property-substitution tests, name verified at impl time). Plus the new `${project.version}`-with-inherited-groupId test from T011.

### Tests for User Story 2

- [X] T010 [P] [US2] Add regression test `existing_property_substitution_unchanged` to `mikebom-cli/tests/maven_pom_version_extraction.rs`. Build a temp pom.xml with `<version>${revision}</version>` + `<properties><revision>1.2.3</revision></properties>`, run the scanner, assert main-module PURL contains `@1.2.3`. Pre-existing milestone-085 / milestone-070 maven property substitution tests cover this case for `<groupId>`-present POMs; this new test specifically pairs property substitution with the `<groupId>`-inherited shape (the milestone 092 territory).
- [X] T011 [P] [US2] Add test `dep_version_uses_project_version_property_when_groupId_inherited` to `mikebom-cli/tests/maven_pom_version_extraction.rs`. Build a temp pom.xml with: parent block (groupId=org.example, artifactId=parent, version=10.0); project-level `<artifactId>app` + `<version>3.14.0</version>` (no project-level groupId); a dep with `<version>${project.version}</version>`. Assert the emitted dep edge has version `3.14.0` (NOT `10.0`). This exercises Contract 3 + Contract 4.

### Implementation for User Story 2

- [X] T012 [P] [US2] Update `resolve_pom_property_value`'s `"project.version"` match arm in `mikebom-cli/src/scan_fs/package_db/maven.rs` (~line 3351) to insert `.or_else(|| self_doc.self_version.clone())` between the `self_coord.2` lookup and the `parent_coord.2` fallback. Exact post-edit shape per `contracts/maven-version-extraction.md` Contract 3.
- [X] T013 [P] [US2] Update `resolve_maven_property`'s `"project.version"` match arm in `mikebom-cli/src/scan_fs/package_db/maven.rs` (~line 738) to add `.or_else(|| doc.self_version.clone())` after the existing `doc.self_coord.as_ref().map(|(_, _, v)| v.clone())`. Exact post-edit shape per `contracts/maven-version-extraction.md` Contract 4.
- [X] T014 [US2] Run `cargo +stable test -p mikebom --test maven_pom_version_extraction` and confirm T010 + T011 pass. Run any pre-existing maven property-substitution tests (`cargo +stable test -p mikebom maven_property_substitution` and any milestone-070 / milestone-085 tests touching `${...}` resolution); confirm zero regressions.

**Checkpoint**: US1 + US2 both pass. The fix is complete on the production-code side. Phase 5 covers downstream test/golden updates.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Update the milestone-083 transitive-parity baseline (FR-006), regenerate any maven goldens that flip from `@<parent-version>` to `@<project-version>` (FR-005), confirm byte-stability for non-maven goldens, and pass the mandatory pre-PR gate.

- [X] T015 [P] Update `mikebom-cli/tests/transitive_parity_maven.rs` per FR-006: bump the pre-092 baseline from `pkg:maven/org.apache.commons/commons-lang3@64` to `@3.14.0` in `EXPECTED_REPRESENTATIVE_EDGES` (or whichever constant pins the version). The edge **count** does NOT change (still 1 emitted edge); only the version string within the pinned representative changes. Update the doc-comment to add a "Closed by milestone 092" subsection mirroring milestone-087/088 pattern.
- [X] T016 [P] Update `specs/083-transitive-correctness/research.md` §8 — Ecosystem: Maven audit row to mark the version-extraction gap closed (mirror the milestone-087 / milestone-088 / milestone-091 pattern).
- [X] T017 Regenerate maven goldens if any flip. Run, in sequence: `MIKEBOM_UPDATE_CDX_GOLDENS=1 cargo +stable test -p mikebom --test cdx_regression`; `MIKEBOM_UPDATE_SPDX_GOLDENS=1 cargo +stable test -p mikebom --test spdx_regression`; `MIKEBOM_UPDATE_SPDX3_GOLDENS=1 cargo +stable test -p mikebom --test spdx3_regression`. Then `git status --short mikebom-cli/tests/fixtures/golden/` — expected: AT MOST 3 maven goldens modified (the milestone-070 maven main-module fixture, or any other maven golden whose source pom.xml declares a `<parent>` element with its own `<version>`), zero non-maven goldens. If any non-maven golden modifies, STOP and investigate scope creep.
- [X] T018 Audit the golden diff scope. `git diff mikebom-cli/tests/fixtures/golden/` — confirm the only changes are version-string flips on commons-lang3-style entries (parent-version → project-version). NO new components, NO new annotations, NO PURL count changes. Per `quickstart.md` Recipe 5.
- [X] T019 Run the mandatory pre-PR gate: `./scripts/pre-pr.sh`. Confirm zero clippy warnings AND every test suite reports `0 failed`. This is the CLAUDE.md mandatory gate; failure here blocks PR.
- [X] T020 Run `cargo +stable test -p mikebom --test transitive_parity_maven` and confirm 4/4 tests pass post-T015 update.
- [X] T021 Update `CLAUDE.md`'s "Recent Changes" section to add a milestone-092 line. The agent-context script already populated `## Active Technologies` with the milestone's stack; verify it reads correctly.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately.
- **Foundational (Phase 2)**: Depends on Setup. **BLOCKS** US1 + US2 (both consumers read `self_version`).
- **US1 (Phase 3)**: Depends on Foundational. Independent of US2 — can ship as MVP.
- **US2 (Phase 4)**: Depends on Foundational. Independent of US1.
- **Polish (Phase 5)**: Depends on US1 + US2 production code being merged in.

### User Story Dependencies

- **US1 (P1)**: Reads `doc.self_version` populated by T004. No dependency on US2.
- **US2 (P2)**: Reads `doc.self_version` populated by T004. No dependency on US1.

The two stories share the same Foundational field (`self_version`) but their consumer-update sites are in different functions (US1 → `build_maven_main_module_entry`; US2 → `resolve_pom_property_value` + `resolve_maven_property`). They CAN ship in either order or in parallel after Foundational completes.

### Within Each User Story

- Tests written before implementation (TDD-style).
- Single production file (`maven.rs`) — implementation tasks within the same story affect different functions, so [P] applies between US2's T012 and T013 (different match-arm sites).
- US1's T008 and US2's T012/T013 all touch the same file but DIFFERENT functions; they can happen in any order without merge-conflict risk.

### Parallel Opportunities

- T005 (build check) is independent of T003+T004 once those complete sequentially.
- US1 tests T006 + T007 are in different files (test module in `maven.rs` vs new `maven_pom_version_extraction.rs`) — run in parallel.
- US2 tests T010 + T011 are in the same new test file but different test functions — write together; no parallel-write conflict.
- US2 implementation T012 + T013 touch different functions in `maven.rs` — can land in parallel commits if cleanly separated.
- US1 + US2 implementation tasks (T008, T012, T013) all touch different functions in `maven.rs` — independent.
- Phase 5 T015 (transitive_parity_maven.rs) + T016 (specs/083 doc update) are different files — parallel.

---

## Parallel Example: User Story 1 Tests

```bash
# Both tests touch different files; write them in parallel:
Task: "Unit test parse_pom_xml_extracts_self_version_when_groupId_inherited in mikebom-cli/src/scan_fs/package_db/maven.rs's tests module"
Task: "Integration test main_module_emits_project_version_when_groupId_inherited_from_parent in new file mikebom-cli/tests/maven_pom_version_extraction.rs"
```

## Parallel Example: User Story 2 Implementation

```bash
# Different functions in the same file — sequential commits, but logically independent:
Task: "Update resolve_pom_property_value's project.version arm at maven.rs:~3351"
Task: "Update resolve_maven_property's project.version arm at maven.rs:~738"
```

---

## Implementation Strategy

### MVP First (US1 only)

1. Phase 1: Setup (T001–T002)
2. Phase 2: Foundational (T003–T005) — adds `self_version` field
3. Phase 3: US1 (T006–T009) — main-module fix
4. **STOP and VALIDATE**: run Recipe 3 from `quickstart.md` against the commons-lang3 fixture. Confirm `@3.14.0` emission. This is the milestone's headline win.

### Incremental Delivery

1. Setup + Foundational → field exists
2. US1 → main-module emits correct version → MVP ready
3. US2 → property substitution preserved → completes the milestone
4. Polish → milestone-083 baseline + golden regen + pre-PR gate → ready for PR

### Single-Developer Strategy (this milestone)

This is a small surgical fix; one developer (you) does it sequentially:

1. T001–T002 (setup, ~5 min)
2. T003–T005 (foundational, ~10 min)
3. T006–T009 (US1, ~30 min — bulk of the milestone)
4. T010–T014 (US2, ~20 min)
5. T015–T021 (polish + gate, ~20 min — golden regen is the longest single step)

Total: ~90 min including pre-PR gate. The fix itself is ~10 lines of production code.

---

## Notes

- [P] tasks = different files OR different functions in the same file with no implicit dependency.
- [Story] label maps task to specific user story for traceability.
- The fix is pure-additive: one new struct field, four `.or_else(|| ...)` insertions, no field removals or signature changes.
- All edits land in a single production file (`mikebom-cli/src/scan_fs/package_db/maven.rs`) plus one new test file (`mikebom-cli/tests/maven_pom_version_extraction.rs`) plus one updated regression test (`mikebom-cli/tests/transitive_parity_maven.rs`).
- The milestone-083 baseline pinning was either pinning the BUGGY value (`@64` — needs flipping) OR was already pinning `@3.14.0` and was failing pre-092 (needs verification at T015 time). Either way, T015's deliverable is a test that passes post-T008.
- Verify tests fail pre-fix and pass post-fix per TDD discipline (US1 + US2 test tasks run before their corresponding implementation tasks).
- Commit boundary suggestion: one commit per phase (Foundational, US1, US2, Polish). Optionally squash to a single commit at PR time.
- Avoid: relaxing the `self_coord` constructor at line 703 (would break `MavenInheritanceContext::by_coord` keying — see research.md §1's rejected alternative).
