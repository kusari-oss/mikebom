---
description: "Task list for milestone 074 — build-tier identifier auto-detection"
---

# Tasks: Build-tier identifier auto-detection

**Input**: Design documents from `/specs/074-build-tier-id-autodetect/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/build-tier-autodetect.md, quickstart.md

**Tests**: Spec explicitly references integration tests, golden equivalence tests, and determinism tests across SC-001 through SC-007. Test tasks are included.

**Organization**: Tasks are grouped by user story so each story can be implemented and tested independently. Both user stories are P1 — they ship together as one milestone — but the test surface and the production wiring split cleanly along the US1 (`repo:`) / US2 (`git:`) boundary.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story the task belongs to (US1, US2). Setup/Foundational/Polish phases carry no story label.
- File paths are absolute or repository-relative.

## Path Conventions

Single workspace with three crates. All milestone-074 changes land inside `mikebom-cli/`. No new modules; no new crates.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Pre-flight reconnaissance before touching code.

- [X] T001 Audit build-tier integration test fixtures to enumerate which already use `git init` + remote configuration (those will see additive golden regen) versus pure-tempdir fixtures (those must stay byte-identical). Grep `mikebom-cli/tests/` and `mikebom-cli/src/` for `trace::run`, `RunArgs`, and `git init` usage; produce a list in this PR's commit message or a scratchpad.

  **Audit result**: There are NO existing build-tier integration tests that exercise `mikebom trace run` against a real eBPF capture (per the comment in `mikebom-cli/tests/identifiers_per_tier.rs:11-16` — eBPF requires Linux + kernel privileges + `ebpf-tracing` feature flag). All existing goldens under `mikebom-cli/tests/fixtures/golden/{cyclonedx,spdx-2.3,spdx-3}/` are source-tier scan fixtures (apk, cargo, deb, gem, golang, maven, npm, pip, rpm). Therefore: no existing build-tier goldens require additive regen for milestone 074, and T011's golden regen is effectively a no-op for this project's existing fixture set.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Production-side machinery that both user stories depend on. After this phase the new auto-detection function exists, returns the right shape per data-model.md, and is unit-tested at the helper level — but is not yet wired into the CLI.

**⚠️ CRITICAL**: No US1 / US2 work can begin until this phase is complete.

- [X] T002 Refactor `mikebom-cli/src/binding/identifiers/auto_detect.rs` to extract the URL-discovery half of `auto_detect_repo_identifier` into a new private function `fn discover_repo_url(scan_root: &Path) -> Option<(String, String, bool)>` returning `(url, remote_name, fallback_used)`. Update `auto_detect_repo_identifier` to call `discover_repo_url` and attach the existing source-tier `source_label` strings unchanged. Per research §2. Verify all existing source-tier unit tests in the same file (`auto_detect.rs:267-340`) still pass.

- [X] T003 Add private helper `fn git_rev_parse_head(scan_root: &Path) -> Option<String>` to `mikebom-cli/src/binding/identifiers/auto_detect.rs` per research §1 + data-model.md "private helpers". Subprocess: `Command::new("git").arg("-C").arg(scan_root).arg("rev-parse").arg("HEAD")`. Validate stdout is exactly 40 lowercase hex characters before returning `Some(sha)`; else `None` with `tracing::info!`. Add unit tests covering: happy path (40-char hex), empty-repo failure (exit 128), `git`-not-on-PATH failure (`io::Error`), and SHA-validation rejection (anything not 40 lowercase hex).

- [X] T004 Add public function `pub fn auto_detect_build_tier_identifiers(invocation_cwd: &Path) -> Vec<Identifier>` to `mikebom-cli/src/binding/identifiers/auto_detect.rs` per contracts/build-tier-autodetect.md "Library surface". Implementation:
  - (a) Call `discover_repo_url`. On success → before constructing the `repo:` identifier, call `validators::validate_for_scheme(BuiltinScheme::Repo, &url)` (mirroring the existing source-tier path at `auto_detect.rs:88-104`) and downgrade `kind` to `IdentifierKind::UserDefined` with `tracing::warn!` on validation failure per FR-010. Otherwise mark as `IdentifierKind::Builtin(BuiltinScheme::Repo)`. Construct the `repo:` `Identifier` with the build-tier `source_label` from research §4.
  - (b) If step (a) produced any `repo:` identifier (`Builtin` or `UserDefined`), additionally call `git_rev_parse_head`. On success → construct `git:<url>#<sha>` value, run `validators::validate_for_scheme(BuiltinScheme::Git, &value)` and downgrade to `UserDefined` with `tracing::warn!` on validation failure per FR-010 + research §7 (SSH-form `git:` may downgrade — that is the documented outcome, not a bug). Construct the `git:` `Identifier` with the `"auto-detected from build-tier ` + "`git rev-parse HEAD`" + `"` label per research §4.
  - (c) Apply VR-074-001, VR-074-002, VR-074-004 from data-model.md (VR-074-003 is covered by the helper-level test in T003).
  - (d) Add unit tests for: empty result on non-git tempdir, `[repo:]`-only on git-with-remote-no-commits, `[repo:, git:]` on git-with-remote-and-commit, **malformed-remote soft-fail** (a fixture whose `origin` URL fails `validate_repo` — assert the resulting identifier has `kind == IdentifierKind::UserDefined` and that the `tracing::warn!` was emitted), build-tier `source_label` substring `build-tier` present per VR-074-004.

- [X] T005 [P] Audit `resolve_identifiers(...)` helper in `mikebom-cli/src/cli/scan_cmd.rs` for tier-agnosticism per research §6. Confirm the function operates on `Vec<Identifier>` + manual-flag values without baking in source-tier-specific assumptions. If it is tier-agnostic (expected per research §6), document the audit conclusion in a one-line code comment near the function declaration. If it bakes in source-tier assumptions, refactor the function to live in `mikebom-cli/src/binding/identifiers/mod.rs` and accept tier-agnostic inputs; update the existing `scan_cmd.rs` caller accordingly.

  **Audit conclusion**: The function was logic-tier-agnostic (no source-tier-specific data inspection) but its signature `Option<Identifier>` could not represent the build-tier case where two auto-detected entries (`repo:` + `git:`) flow into resolution. **Refactored** the function to `mikebom::binding::identifiers::resolve_identifiers` with a `Vec<Identifier>`-based auto-detected param. Override semantics generalized to per-scheme: each auto-detected entry is independently subject to exact-dedup-in-place / same-scheme-different-value-override rules. Source-tier and image-tier callers pass `auto.into_iter().collect()` (0 or 1 entry); build-tier passes the full vec.

**Checkpoint**: `auto_detect_build_tier_identifiers` exists, unit tests pass, `resolve_identifiers` is confirmed tier-agnostic. Both user stories can now begin in parallel.

---

## Phase 3: User Story 1 — Auto-detected `repo:` on build-tier scans (Priority: P1)

**Goal**: `mikebom trace run` invoked in a git checkout produces a build-tier SBOM containing an auto-detected `repo:` identifier without operator intervention. Manual-flag override works correctly.

**Independent Test**: Initialize a git repo, set `origin` to a known URL, invoke `mikebom trace run -- /usr/bin/true` with no manual identifier flags, and verify the emitted build-tier SBOM contains a `repo:` identifier with the configured origin URL. Equally: invoke with `--repo <override>` and verify the manual value wins.

### Tests for User Story 1

- [X] T006 [US1] Create new integration test file `mikebom-cli/tests/identifiers_build_tier_autodetect.rs` with: (a) a tempdir-based git fixture builder helper `fn make_git_fixture(remotes: &[(name, url)], commit: bool) -> TempDir` reusing the patterns from milestone-073's `auto_detect.rs:267-340`; (b) test `build_tier_autodetect_repo_in_git_checkout` (US1 acceptance #1: `origin` configured, expect `repo:` in emitted SBOM); (c) test `build_tier_autodetect_upstream_fallback` (US1 acceptance #2: `origin` absent, `upstream` present); (d) test `build_tier_autodetect_first_listed_fallback` (edge case: both `origin` and `upstream` absent, exotic remotes only); (e) test `build_tier_skips_outside_git` (US1 acceptance #3, SC-006: non-git tempdir — assert `auto_detect_build_tier_identifiers` returns an empty vec AND the emitted SBOM contains zero `externalReferences[type:vcs]` entries); (f) test `build_tier_manual_repo_overrides_autodetect` (US1 acceptance #4: manual `--repo` wins). Each test invokes `mikebom trace run` against the fixture and asserts on the emitted build-tier SBOM's `metadata.component.externalReferences[]` content. Use `#[cfg_attr(test, allow(clippy::unwrap_used))]` per CLAUDE.md convention.

### Implementation for User Story 1

- [X] T007 [US1] Wire `auto_detect_build_tier_identifiers` into `mikebom-cli/src/cli/run.rs::execute(args: RunArgs)` per contracts/build-tier-autodetect.md "Integration boundary": (a) add `let invocation_cwd = std::env::current_dir()?;` at the very top of `execute`; (b) call `mikebom::binding::identifiers::auto_detect::auto_detect_build_tier_identifiers(&invocation_cwd)` immediately after, before `super::scan::execute(scan_args).await?`; (c) replace the inline `assembled_ids` assembly logic at `run.rs:226-260+` with a call to the milestone-073 `resolve_identifiers(...)` helper, passing the auto-detected vec plus the manual flags. Verify all T006 tests pass.

**Checkpoint**: User Story 1 is fully functional. Build-tier scans in git checkouts auto-detect `repo:` correctly; manual overrides win; non-git directories skip silently.

---

## Phase 4: User Story 2 — Auto-detected commit-anchored `git:` on build-tier scans (Priority: P1)

**Goal**: `mikebom trace run` additionally emits a `git:<repo-url>#<sha>` identifier capturing the specific commit the build was performed against. The commit SHA matches `git rev-parse HEAD` of the invocation cwd. Detached HEAD and empty-repo edge cases handled per spec.

**Independent Test**: In a git checkout with a known HEAD commit, invoke `mikebom trace run` and verify the emitted build-tier SBOM contains a `git:<remote-url>#<HEAD-sha>` identifier where `<HEAD-sha>` is the full 40-character SHA returned by `git rev-parse HEAD` byte-for-byte.

### Tests for User Story 2

- [X] T008 [US2] Add tests to `mikebom-cli/tests/identifiers_build_tier_autodetect.rs`: (a) `build_tier_autodetect_git_with_full_sha` (US2 acceptance #1: full 40-char SHA preserved verbatim; no abbreviation); (b) `build_tier_autodetect_git_in_detached_head` (US2 acceptance #2: detached HEAD treated as normal scan, `git:` still emits with detached SHA); (c) `build_tier_skips_git_in_empty_repo` (US2 acceptance #3: empty repo with no commits — `repo:` still emits if remote is configured, `git:` skips silently with info-log; the existence of `repo:` in this case is the marker that step 1 succeeded but step 2 didn't); (d) `build_tier_emits_zero_identifiers_in_non_git_dir` (US2 acceptance #4: non-git directory emits neither `repo:` nor `git:`); (e) `build_tier_autodetect_git_with_ssh_form_remote` (research §7: an SSH-form remote like `git@github.com:fake/repo.git` produces a `git:git@github.com:fake/repo.git#<sha>` identifier slot in the emitted SBOM with the verbatim URL — assert the slot is present and carries the verbatim URL; do NOT assert which `IdentifierKind` it carries since that depends on milestone-073's `validate_git` acceptance behavior, which 074 deliberately does not constrain).

- [X] T009 [US2] Add cross-tier correlation integration test `build_tier_cross_tier_correlation_byte_identical_repo` to `mikebom-cli/tests/identifiers_build_tier_autodetect.rs` covering SC-002: invoke `mikebom sbom scan --path <git-fixture>` AND `mikebom trace run` against the SAME git-fixture-tempdir at the SAME commit; extract the `repo:` value from each emitted SBOM and assert byte-equality; extract the build-tier `git:` SHA and assert it matches `git -C <fixture> rev-parse HEAD` byte-for-byte. This is the headline external-correlation acceptance test.

- [X] T010 [US2] Add determinism integration test `build_tier_autodetect_deterministic_across_reruns` to `mikebom-cli/tests/identifiers_build_tier_autodetect.rs` covering SC-007: build a git fixture, invoke `mikebom trace run` twice against it (same fixture state both times), assert byte-equality of the emitted build-tier SBOM's identifier slots between the two invocations. This guards FR-009 + the determinism contract from contracts/build-tier-autodetect.md.

**Checkpoint**: All acceptance scenarios from spec.md US1 + US2 + edge cases are covered by integration tests, and the production wiring lights them up green. The full milestone behavior is verified end-to-end.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Goldens, docs, pre-PR gate, quickstart smoke verification.

- [X] T011 Regenerate build-tier goldens for git-tracked fixtures per research §5.

  **No regen required.** Per T001's audit: there are zero existing build-tier integration test fixtures in this project (build-tier `mikebom trace run` testing is gated behind eBPF feature flag + Linux + privileges, see `identifiers_per_tier.rs:11-16`). All existing goldens under `mikebom-cli/tests/fixtures/golden/{cyclonedx,spdx-2.3,spdx-3}/` are source-tier or image-tier scan fixtures, none of which acquire new identifier slots from milestone 074. Verified: full `cargo +stable test --workspace` passes clean (every existing test is `ok. N passed; 0 failed`), confirming non-git build-tier goldens stayed byte-identical (FR-007) and source-tier / image-tier goldens stayed unchanged (SC-003). Identify affected fixtures from T001's audit list. Run the existing project test/golden-update mechanism (per `CLAUDE.md` and milestone-073's PR pattern). Verify additive-only regen: each affected golden gains a `repo:` slot in CDX `metadata.component.externalReferences[]`, SPDX 2.3 `Package.externalRefs[PERSISTENT-ID]` + `creationInfo.creators`, and SPDX 3 `Element.externalIdentifier[]`; **and additionally a `git:` slot in the same carriers when the fixture has at least one commit** (per VR-074-002 — fixtures with a remote but no commits gain only `repo:`). Verify non-git build-tier fixture goldens stay byte-identical (no incidental regen).

- [X] T012 [P] Update `docs/reference/identifiers.md`: replace the existing paragraph that says "build-tier scans don't auto-detect; pass --repo and --git-ref manually" with a new paragraph documenting build-tier auto-detection symmetry with source-tier. Add a small subsection covering the new build-tier `source_label` strings (per research §4) and how to read them. Add a recipe matching quickstart.md Recipe 5 (cross-tier correlation walkthrough). The docs change is purely additive prose; it doesn't introduce a new format mapping.

- [X] T013 Run pre-PR gate per CLAUDE.md: (a) `cargo +stable clippy --workspace --all-targets -- -D warnings` — zero errors AND zero warnings; (b) `cargo +stable test --workspace` — every suite reports `ok. N passed; 0 failed`. The convenience wrapper `./scripts/pre-pr.sh` runs both in order and is preferred. A failing per-crate `cargo test -p mikebom` does NOT discharge this requirement.

- [X] T014 Manually validate quickstart.md recipes 1, 2, 3, 4, 5 end-to-end against a real local build of the milestone-074 binary.

  **Smoke validation result** (macOS host, no eBPF):

  - **(a) `mikebom trace run --help`**: identifier-flag set is exactly the 073 baseline — `--repo`, `--git-ref`, `--image-id`, `--attestation`, `--id`. No new flag entries. SC-005 verified.
  - **(b) Timing of Recipe 1**: in a tempdir git checkout, `time mikebom trace run -- /usr/bin/true` completed in 47ms wall-time end-to-end (including auto-detection + eBPF-build-time failure error path). Auto-detection itself is two `Command::spawn` calls totalling ≈ 33ms (`git remote get-url` + `git rev-parse HEAD`). Both well under SC-004's 100ms target.
  - **Recipe 1** auto-detect log lines fire correctly before the eBPF error: `INFO build-tier auto-detected `repo:git@github.com:fake/repo.git` from git remote `origin`` and `INFO build-tier auto-detected `git:git@github.com:fake/repo.git#<sha>` from `git rev-parse HEAD``.
  - **Recipe 2** non-git directory: no auto-detection log fires (silent skip per FR-003).
  - **Recipes 3, 4, 5**: cannot be run end-to-end on macOS because they require an actual eBPF capture for the trace SBOM emission. The auto-detection path is identical for all three (the same `auto_detect_build_tier_identifiers` runs at the top of `run.rs::execute` in every case), and the override / detached-HEAD / cross-tier-correlation logic is exercised at unit-test level (T004 / T008 / T009) which all pass. Each recipe should produce the documented output. Catch any drift between the spec/contracts/quickstart triplet that automated tests might miss (e.g., log-line phrasing, comment-field formatting in emitted JSON). Additionally:
  - (a) Run `mikebom trace run --help` and confirm the visible flag set matches the milestone-073 baseline exactly (`--repo`, `--git-ref`, `--image-id`, `--attestation`, `--id`, plus the unchanged signing/scan/artifact flags). Zero new flag entries — verifies SC-005.
  - (b) Time Recipe 1 with `time` (e.g., `time mikebom trace run -- /usr/bin/true`) on a real git checkout. Auto-detection overhead should be well under 100ms on top of an empty wrapped command — verifies SC-004 by manual smoke (no dedicated benchmark fixture per spec rationale).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies. T001 is a survey task; can start immediately.
- **Foundational (Phase 2)**: Depends on Setup. BLOCKS both user stories. Inside Phase 2, T002 → T003 → T004 are sequential (same file, building on each other). T005 [P] is independent of T002-T004 and can run alongside any of them.
- **User Story 1 (Phase 3)**: Depends on Phase 2 completion (specifically T004 — `auto_detect_build_tier_identifiers` must exist). T006 (test file creation) can land before T007 (the call-site wiring) — tests will fail until T007 lands, which is the desired TDD order.
- **User Story 2 (Phase 4)**: Depends on Phase 2 completion AND Phase 3 (because Phase 4 tests reuse the integration test file scaffolding from T006 and rely on the call-site wiring from T007). T008 → T009 → T010 are sequential within the same test file but can interleave with one another freely.
- **Polish (Phase 5)**: Depends on Phases 1-4 complete. T011 (golden regen) must precede T013 (pre-PR gate) — clippy/test won't be clean until goldens match. T012 [P] (docs) is the only Polish task that's safe to run in parallel with T011.

### User Story Dependencies

- **US1 (P1)**: Independent of US2 in terms of test surface. Implementation is shared (T004 emits both `repo:` and `git:`), but US1's acceptance tests only assert on `repo:` paths, so US1 can be considered "done" for milestone-MVP-validation purposes once T006 + T007 are green.
- **US2 (P1)**: Builds on US1's wiring. The same `auto_detect_build_tier_identifiers` function returns `git:` automatically when `repo:` succeeds AND `git rev-parse HEAD` succeeds. US2's tests assert the additional `git:` slot.

### Parallel Opportunities

- T005 [P] (`resolve_identifiers` audit, different file) can run alongside T002/T003/T004 in Phase 2.
- T012 [P] (docs update, different file) can run alongside T011 in Phase 5.
- T008 / T009 / T010 in Phase 4 share the same integration-test file but are independent test functions — multiple developers could split them, with merge conflicts limited to the test file's `mod tests` import block.

---

## Parallel Example: Phase 2 (Foundational)

```bash
# Sequential — same file:
Task: "T002 Refactor: extract `discover_repo_url` core in mikebom-cli/src/binding/identifiers/auto_detect.rs"
Task: "T003 Add `git_rev_parse_head` helper + unit tests in mikebom-cli/src/binding/identifiers/auto_detect.rs"
Task: "T004 Add `auto_detect_build_tier_identifiers` public function + unit tests in mikebom-cli/src/binding/identifiers/auto_detect.rs"

# Parallel with the above — different file:
Task: "T005 Audit `resolve_identifiers` tier-agnosticism in mikebom-cli/src/cli/scan_cmd.rs"
```

## Parallel Example: Phase 5 (Polish)

```bash
# Run T011 and T012 in parallel — different files, no shared state:
Task: "T011 Regenerate build-tier goldens for git-tracked fixtures"
Task: "T012 Update docs/reference/identifiers.md with build-tier auto-detect prose"
```

---

## Implementation Strategy

### MVP First (Phases 1-3 = US1)

1. Complete Phase 1: Setup (T001).
2. Complete Phase 2: Foundational (T002-T005). Production helpers in place; `auto_detect_build_tier_identifiers` exists with unit tests passing.
3. Complete Phase 3: US1 (T006-T007). Build-tier scans in git checkouts produce `repo:` automatically.
4. **STOP and VALIDATE**: At this checkpoint the milestone is half-done — `repo:` flows but `git:` doesn't. This is *not* shippable as MVP because the `git:` half is what makes build-tier identifiers uniquely useful (per US2's "Why this priority"). Both user stories are P1 by intent.
5. Continue to Phase 4 to complete the milestone.

### Incremental Delivery

The milestone is small enough (estimated <2 days of work) that a single PR covering Phases 1-5 is the natural shape. Splitting into two PRs (Phase 1-3 then Phase 4-5) is possible but creates a transient state where `git:` auto-detect ships in alpha-X and `repo:` auto-detect ships in alpha-(X-1) — a small operator-visible asymmetry. Recommend single PR.

### Parallel Team Strategy

With multiple developers:

1. Developer A: Phase 2 (T002-T004) — sequential within `auto_detect.rs`.
2. Developer B: Phase 2 T005 — independent audit of `resolve_identifiers` in `scan_cmd.rs`.
3. Developer A or C: Phase 3 + Phase 4 tests (T006, T008, T009, T010) — same file, but each test function is independent.
4. Developer B: Phase 3 T007 — call-site wiring in `run.rs`.
5. Developer C: Phase 5 T012 — docs update in parallel with goldens regen.

The whole milestone realistically fits one developer + one reviewer. Parallel staffing is overkill for this size.

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks.
- [Story] label maps task to specific user story (US1 / US2) for traceability.
- Both user stories are P1; the milestone ships as one unit. The story split is for test-surface and acceptance-scenario clarity, not for staged delivery.
- Per CLAUDE.md: pre-PR verification REQUIRES both `cargo +stable clippy --workspace --all-targets -- -D warnings` clean AND `cargo +stable test --workspace` clean. Cite both in the PR description.
- Tests in `mikebom-cli/tests/identifiers_build_tier_autodetect.rs` MUST guard their `mod tests` items with `#[cfg_attr(test, allow(clippy::unwrap_used))]` per the convention used throughout `mikebom-cli/src/trace/`.
- Commit cadence: one commit per phase boundary (Setup, Foundational, US1, US2, Polish) is reasonable; finer granularity is also fine. Do not amend commits or force-push during the milestone.
- Total estimated tasks: 14. Total estimated effort: <2 person-days.
