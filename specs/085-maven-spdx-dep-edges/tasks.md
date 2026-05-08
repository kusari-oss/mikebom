---
description: "Tasks: Maven SPDX dep-edge emission"
---

# Tasks: Maven SPDX dep-edge emission (milestone-070 follow-up)

**Input**: Design documents from `/specs/085-maven-spdx-dep-edges/`
**Prerequisites**: spec.md ✅, plan.md ✅

**Organization**: Single small fix; phases collapse to setup → fix → polish.

## Phase 1: Setup

- [X] T001 Capture pre-fix maven SPDX 2.3 + SPDX 3 DEPENDS_ON counts to confirm starting state matches spec ("0 each")

## Phase 2: Foundational (the fix)

- [X] T002 Modify `mikebom-cli/src/scan_fs/mod.rs:373-379` `name_to_purl` insert loop to ALSO insert a `("maven", "groupId:artifactId")` key for entries whose ecosystem is `"maven"` (groupId from `e.purl.namespace()`, artifact-id from `e.purl.name()`). Both keys point at the same PURL. ~6 LOC.
- [X] T003 Smoke-test: rebuild release binary, emit SPDX 2.3 against `tests/fixtures/maven/pom-three-deps`, assert 3 `DEPENDS_ON` relationships present.

## Phase 3: US1 — SPDX 2.3 + SPDX 3 dep edges

- [X] T004 [US1] Regenerate maven SPDX 2.3 golden: `MIKEBOM_UPDATE_SPDX_GOLDENS=1 cargo +stable test -p mikebom --test spdx_regression`. Verify ONLY `maven.spdx.json` changes; other 8 stay byte-identical.
- [X] T005 [US1] Regenerate maven SPDX 3 golden: `MIKEBOM_UPDATE_SPDX3_GOLDENS=1 cargo +stable test -p mikebom --test spdx3_regression`. Verify ONLY `maven.spdx3.json` changes.
- [X] T006 [US1] Audit diff scope of regenerated maven SPDX goldens — only added `DEPENDS_ON` (SPDX 2.3) / `software_dependsOn` (SPDX 3); no other field changes.

## Phase 4: US2 — clean parity gate

- [X] T007 [US2] Remove the `("maven", "B1", _)` entry from `KNOWN_PARITY_GAPS` in `mikebom-cli/tests/holistic_parity.rs`. The const becomes an empty slice `&[]`.
- [X] T008 [US2] Run `cargo +stable test -p mikebom --test holistic_parity` — confirm `parity_maven` passes without the allowlist.

## Phase 5: US3 — CDX byte-identity

- [X] T009 [US3] Run `cargo +stable test -p mikebom --test cdx_regression` without env var; confirm all 9 CDX goldens (including maven) pass byte-identical (no regen needed).
- [X] T010 [US3] Run `cargo +stable test -p mikebom --test cdx_ref_closure_invariant` — confirm closure invariant + reverse-walk + compositions-anchor still pass for all 6 ecosystems including maven.

## Phase 6: Polish

- [X] T011 Pre-PR gate: `./scripts/pre-pr.sh` MUST exit 0 with zero clippy warnings + every test suite `0 failed`.
- [X] T012 Update CLAUDE.md "Recent Changes" section if the speckit infrastructure didn't auto-update it.

---

## Dependencies

- T001 → T002 → T003 (setup → fix → smoke)
- T002 must complete before any of T004/T005/T007/T009/T010
- T004/T005/T007 can run in parallel after T002 (different files, different test suites)
- T011 last
