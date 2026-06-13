---

description: "Task list for milestone 114 — shared `safe_walk` helper migration"
---

# Tasks: Shared `safe_walk` Helper Migration

**Input**: Design documents from `/specs/114-safe-walk-migration/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md (all present)

**Organization**: Tasks are grouped by user story. US1 (P1, MVP) ports every walker; US2 (P2) ships the audit machinery + docs; US3 (P1, byte-identity) is verified inline at each per-walker port (each port's gate is the byte-identity check, not a separate phase).

**Migration shape**: tasks.md groups the work by user story, but the implementation strategy maps each US1 sub-phase to a separate PR per research R4 — five PRs in sequence with byte-identity verification at each PR boundary.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can be implemented in parallel (different files, no incomplete deps).
- **[Story]**: Maps to a user story (US1/US2/US3). Absent on Setup, Foundational, and Polish phases.
- Every task names the exact file path it touches.

## Path Conventions

Single-project layout (matches every milestone since 001). All paths are relative to repository root `/Users/mlieberman/Projects/mikebom/`.

---

## Phase 1: Setup (Pre-migration baselining)

**Purpose**: Capture the pre-migration baseline so US3's byte-identity guarantee can be verified objectively at every PR boundary.

- [X] T001 Run `MIKEBOM_SKIP_DOCKER_INTEGRATION=1 ./scripts/pre-pr.sh` against the current `main` HEAD (pre-migration); confirm clippy `--workspace --all-targets -D warnings` + full workspace tests both green — this is the baseline every per-PR port must match
- [X] T002 Capture pre-migration audit-grep output: `grep -rn '^fn walk' mikebom-cli/src/scan_fs/` → save to `/tmp/walker-inventory-pre.txt`; commit nothing — this is the reference set for SC-001 verification at PR 5
- [X] T003 [P] Smoke-verify the two known exceptions (`scan_fs/walker.rs::walk_and_hash`, `scan_fs/package_db/npm/walk.rs::walk_node_modules`) actually have the structural complications cited in `research.md` R5 — read each, confirm the size-cap-inside-walk and `@scope` / `in_npm_internals: bool` propagation respectively, document the result in a 2-sentence note appended to `research.md` R5 if either claim turns out wrong

---

## Phase 2: Foundational (Blocking Prerequisites for every user story)

**Purpose**: Introduce the shared `safe_walk` helper with full unit-test coverage so every subsequent walker port can delegate to it.

**CRITICAL**: No US1/US2/US3 work can begin until this phase is complete.

- [X] - [ ] T004 Create `mikebom-cli/src/scan_fs/walk.rs` with the module skeleton: `use` declarations (`std::collections::HashSet`, `std::path::{Path, PathBuf}`, `tracing`), public `WalkConfig<'a>` struct per `contracts/walk-api.md` (fields: `max_depth: usize`, `should_skip: &'a dyn Fn(&Path, &Path) -> bool`, `exclude_set: &'a super::package_db::exclude_path::ExclusionSet`), and a stub `pub(crate) fn safe_walk<F: FnMut(&Path)>(rootfs: &Path, cfg: &WalkConfig, visit: F)` that does nothing yet. Register the module via `pub(crate) mod walk;` in `mikebom-cli/src/scan_fs/mod.rs`. Run `cargo +stable check -p mikebom` to confirm compile
- [X] - [ ] T005 Implement `safe_walk`'s descent loop in `mikebom-cli/src/scan_fs/walk.rs` per `data-model.md`: canonicalize-keyed `HashSet<PathBuf>` visited-set guard before any `visit` invocation; `visit(dir)` after insertion; depth-bound check after `visit`; tolerant `read_dir().ok()` early-return on error; per-child iteration that yields files immediately via `visit(path)` and gates directory descent on `cfg.should_skip(candidate, rootfs)` + `cfg.exclude_set.matches(rel)` (short-circuit when `exclude_set.is_empty()`); `tracing::debug!` emitted at every skip decision with cause label (`"built-in"` vs `"exclude-path"`) per research R3
- [X] - [ ] T006 Unit tests in `mikebom-cli/src/scan_fs/walk.rs` `#[cfg(test)] mod tests` covering 12 scenarios: (a) bare-minimum walk yields rootfs once; (b) canonicalize-keyed dedup — `a/link → a` symlink loop produces zero duplicate `visit` calls and terminates bounded; (c) depth bound stops descent (verify by counting `visit` calls at depth N+1); (d) skip predicate suppresses descent; (e) skip predicate receives `(candidate, rootfs)` — verify via captured arg; (f) `ExclusionSet::is_empty()` short-circuits without invoking the closure; (g) non-empty `ExclusionSet` match suppresses descent + emits `tracing::debug!` (verify via `tracing::subscriber` capture); (h) files AND directories both flow through the visit callback; (i) unreadable directory (permissions denied) does NOT abort descent on peer dirs; (j) `FnMut` callback with captured `&mut` state works (port-equivalent test); (k) order within a single directory is `read_dir` order (platform-dependent — assert via spy callback that order matches `read_dir().collect()`); (l) `tracing::debug!` skip-cause labels are `"built-in"` and `"exclude-path"` as documented
- [X] - [ ] T007 Add comment block at the top of `mikebom-cli/src/scan_fs/walk.rs` documenting (a) the two known-exception filesystem walkers per research R5 (`scan_fs/walker.rs`, `scan_fs/package_db/npm/walk.rs`) with one-sentence reasons; (b) the audit pattern `grep -rEn 'fn walk[_(]' mikebom-cli/src/scan_fs/`; (c) the **non-walker false-positives** list per research R6 (`maven.rs::walk_m2_jars`, `maven.rs::walk_jar_maven_meta`, `rpmdb_sqlite/schema.rs::walk_schema_page`, plus the test functions `walks_*` / `walk_jar_*` / `walk_fat_jar_*` / `walk_rootfs_poms_*`) so reviewers can recognize them; (d) the review-policy rule "any match outside the union of those two lists is a regression — reviewer either rejects the PR or pushes back to migrate / document a new entry."
- [X] - [ ] T008 Run `cargo +stable test -p mikebom --bins 'scan_fs::walk::'` — all 12 unit tests pass

**Checkpoint**: foundational complete. The helper is callable from anywhere in `scan_fs/`, its unit tests pass, its observability contract is in place. Per-walker ports can now proceed.

---

## Phase 3: User Story 1 — Migrate every standard ecosystem walker to `safe_walk` (Priority: P1) 🎯 MVP

**Goal**: Every hand-rolled `fn walk_*` recursion under `mikebom-cli/src/scan_fs/` (excluding the two documented exceptions) delegates to `safe_walk`. After this phase, contributors writing a new ecosystem reader use the helper rather than copy-pasting an existing walker.

**Independent Test**: Run `grep -rn '^fn walk' mikebom-cli/src/scan_fs/` after every PR in this phase. Matches outside the 3 documented files (`walk.rs`, `walker.rs`, `npm/walk.rs`) get progressively eliminated PR-by-PR. After the last per-walker port PR, the grep matches exactly 3 files.

### PR 1 — project_roots reference port (US1 MVP foundation)

- [X] T009 [US1] Port `mikebom-cli/src/scan_fs/package_db/project_roots.rs`: turn `walk_for_project_roots` into a thin wrapper that constructs a `scan_fs::walk::WalkConfig` from the existing `WalkConfig` fields and forwards to `safe_walk`; preserve the existing `WalkConfig` struct in `project_roots.rs` AS A RE-EXPORT/SHIM (keeps pip/npm/gradle/nuget/yocto closures compiling untouched until PR 5 cleans up); DELETE the local `walk_inner` function (its logic now lives in `safe_walk`). The existing per-walker `is_project_root` field semantics are preserved by calling it from inside the new `safe_walk` visit closure when the visited path is a directory
- [X] T010 [US1] Run `MIKEBOM_SKIP_DOCKER_INTEGRATION=1 ./scripts/pre-pr.sh` + verify (a) clippy `--workspace --all-targets -D warnings` clean, (b) every workspace test passes byte-identically (no `cargo test --workspace -- --test=*` failure), (c) zero golden regeneration required — the 33 byte-identity goldens in `mikebom-cli/tests/fixtures/golden/` pass via `cargo test --workspace` without any `MIKEBOM_UPDATE_*_GOLDENS=1` invocation. Per-PR byte-identity gate per US3 / SC-002

**Checkpoint — end of PR 1**: helper exists, project_roots delegates to it, pip/npm/gradle/nuget/yocto reach the helper via the project_roots shim. Audit grep still shows ~12 hand-rolled walkers remaining outside `walk.rs`.

### PR 2 — cargo + gem + rpm_file ports

- [X] T011 [P] [US1] Port `mikebom-cli/src/scan_fs/package_db/cargo.rs::walk_for_cargo_manifests` AND `walk_for_cargo_lockfiles` to use `scan_fs::walk::safe_walk`. Each replaces its hand-rolled descent with a single `safe_walk(rootfs, &cfg, |path| { if path.file_name() == Some(OsStr::new("Cargo.toml")) { out.push(path.to_path_buf()); } })` (or `"Cargo.lock"` for the lockfiles variant). The closure captures `&mut out: Vec<PathBuf>`. `WalkConfig.should_skip` wraps the existing `should_skip_descent(name)` predicate via `|p, _| p.file_name().and_then(|s| s.to_str()).map(should_skip_descent).unwrap_or(true)`. `WalkConfig.exclude_set` is the existing `&ExclusionSet` parameter — no longer threaded into the recursion since the helper handles it
- [X] T012 [P] [US1] Port `mikebom-cli/src/scan_fs/package_db/gem.rs::walk_for_top_level_gemspecs` AND `walk_for_gemfile_locks` AND `walk_for_gemspecs` to use `safe_walk`. Three separate `safe_walk` calls — one per pre-migration walker function. The `MAX_GEMSPEC_WALK_DEPTH` constant from gem.rs becomes the `max_depth` field of the corresponding `WalkConfig`
- [X] T013 [P] [US1] Port `mikebom-cli/src/scan_fs/package_db/rpm_file.rs::walk_dir` to use `safe_walk`. Yields `.rpm` files via the closure's extension check
- [X] T014 [US1] Run `MIKEBOM_SKIP_DOCKER_INTEGRATION=1 ./scripts/pre-pr.sh` + verify per-PR byte-identity gate per US3 / SC-002

**Checkpoint — end of PR 2**: ~9 hand-rolled walkers remain.

### PR 3 — maven + golang + go_binary ports

- [X] T015 [P] [US1] Port `mikebom-cli/src/scan_fs/package_db/maven.rs::walk_for_maven` AND `walk_for_top_level_poms` to use `safe_walk`. `walk_for_maven` yields both `pom.xml` and `.jar`/`.war`/`.ear` files — the visit closure dispatches via extension switch onto the respective `&mut poms`/`&mut jars` vectors. `walk_for_top_level_poms` preserves the existing skip set extension that adds `target`, `.m2`, `node_modules`, `vendor` to `should_skip_default_descent`'s output
- [X] T016 [P] [US1] Port `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs::walk_for_go_roots` to use `safe_walk`. Preserve the milestone-113 Go testdata/`_`-prefix unconditional skips by including them in the `should_skip` closure: `|p, _| { let name = p.file_name()...; should_skip_descent(p) }` — the existing `should_skip_descent(path: &Path)` function (which already takes a `&Path` and handles both name-based + `go/pkg/mod` window match) wraps cleanly
- [X] T017 [P] [US1] Port `mikebom-cli/src/scan_fs/package_db/go_binary.rs::walk_for_binaries` to use `safe_walk`. The closure captures `&mut out: Vec<PackageDbEntry>` + `&mut seen_purls: HashSet<String>` + `&mut main_modules: HashSet<String>` + `&claimed_paths` + `#[cfg(unix)] &claimed_inodes` and reproduces the existing per-file binary-shape filter (extension blocklist for `.o`/`.a`/`.rlib`/`.rmeta`, MIN_BINARY_SIZE_BYTES gate) inside the visit. `should_skip_binary_descent(&path)` becomes the `WalkConfig.should_skip` closure
- [X] T018 [US1] Run `MIKEBOM_SKIP_DOCKER_INTEGRATION=1 ./scripts/pre-pr.sh` + verify per-PR byte-identity gate per US3 / SC-002

**Checkpoint — end of PR 3**: ~4 hand-rolled walkers remain.

### PR 4 — nuget + yocto + binary walker ports

- [X] T019 [P] [US1] Port `mikebom-cli/src/scan_fs/package_db/nuget/mod.rs`: rewrite `walk_project_files` (the entry point, currently at line 58) to invoke `safe_walk` directly and DELETE `walk_inner` (currently at line 67 — its recursive body is now obsolete). The existing `should_skip_default_descent(name)` + milestone-113 `ExclusionSet::matches` check folds into the helper's machinery. The `safe_walk` visit closure tests `path.extension()` against `PROJECT_EXTENSIONS` (`csproj`/`vbproj`/`fsproj`) and pushes matching paths into `&mut out: Vec<PathBuf>`
- [X] T020 [P] [US1] Port `mikebom-cli/src/scan_fs/package_db/yocto/recipe.rs::walk` to use `safe_walk`. The visit closure emits `PackageDbEntry` records directly (continuing the emit-inside-walk pattern); `&mut out: Vec<PackageDbEntry>` is captured
- [X] T021 [P] [US1] Port `mikebom-cli/src/scan_fs/binary/discover.rs::walk_dir` to use `safe_walk`
- [X] T022 [P] [US1] Port `mikebom-cli/src/scan_fs/binary/source_binding/cmake_observer.rs::walk_for_cmake_build_dirs` to use `safe_walk`
- [X] T023 [US1] Run `MIKEBOM_SKIP_DOCKER_INTEGRATION=1 ./scripts/pre-pr.sh` + verify per-PR byte-identity gate per US3 / SC-002

**Checkpoint — end of PR 4**: every standard ecosystem walker now delegates to `safe_walk`. Audit grep shows exactly 3 file matches: `walk.rs` + `walker.rs` + `npm/walk.rs`. US1 is complete.

---

## Phase 4: User Story 2 — Audit-grep + docs (Priority: P2) — PR 5

**Goal**: Ship the audit machinery so future PRs that introduce a `fn walk_*` outside the helper get caught by a one-liner grep at review time. Pair it with the design-notes section that points future contributors at the helper.

**Independent Test**: `grep -rn '^fn walk' mikebom-cli/src/scan_fs/` returns exactly the three documented files (`walk.rs`, `walker.rs`, `npm/walk.rs`) and nothing else. A contributor reading `docs/design-notes.md`'s "Filesystem walking pattern" section can write a new ecosystem reader following the example without consulting any other source.

- [X] T024 [US2] Delete the now-dead shim in `mikebom-cli/src/scan_fs/package_db/project_roots.rs`: remove the local `WalkConfig` re-export struct AND remove `walk_for_project_roots`. Every consumer in pip/npm/gradle/nuget/yocto migrates to importing `WalkConfig` from `scan_fs::walk` and calling `safe_walk` directly (no intermediate wrapper). `project_roots.rs` retains only the `should_skip_default_descent` helper function (which is still useful as a default skip-predicate component for ecosystem closures) — every other symbol in the file is deleted
- [X] T025 [US2] Update every consumer of `project_roots::WalkConfig` to import from `scan_fs::walk::WalkConfig`: `mikebom-cli/src/scan_fs/package_db/pip/mod.rs`, `mikebom-cli/src/scan_fs/package_db/npm/mod.rs`, `mikebom-cli/src/scan_fs/package_db/gradle/mod.rs`, `mikebom-cli/src/scan_fs/package_db/nuget/mod.rs`, `mikebom-cli/src/scan_fs/package_db/yocto/recipe.rs`
- [X] T026 [US2] Verify the audit grep: run `grep -rEn 'fn walk[_(]' mikebom-cli/src/scan_fs/` and confirm every match is either (i) inside one of the three documented filesystem-walker files (`scan_fs/walk.rs`, `scan_fs/walker.rs`, `scan_fs/package_db/npm/walk.rs`) OR (ii) on the documented non-walker false-positive list from research R6 (`maven.rs::walk_m2_jars`, `maven.rs::walk_jar_maven_meta`, `rpmdb_sqlite/schema.rs::walk_schema_page`, and the test functions `walks_*` / `walk_jar_*` / `walk_fat_jar_*` / `walk_rootfs_poms_*`). Any match outside the union of those two lists is a regression — migrate it to `safe_walk` OR add it to the comment block in `scan_fs/walk.rs` with a one-sentence reason
- [X] T027 [US2] Add the "Filesystem walking pattern" section to `docs/design-notes.md` per FR-012: include (a) the audit grep `grep -rEn 'fn walk[_(]' mikebom-cli/src/scan_fs/` + its acceptable-match list (the three filesystem-walker files AND the non-walker false-positive list); (b) the known-filesystem-exception list + reasons; (c) a copy-paste-ready one-paragraph example of how a new ecosystem reader uses `safe_walk` (lifted from `specs/114-safe-walk-migration/quickstart.md`); (d) a pointer to `scan_fs/walk.rs`'s comment block as the authoritative source of the audit pattern + exception list
- [X] T028 [US2] Add `CHANGELOG.md` `[Unreleased]` → `### Changed` entry per research R10: `Internal cleanup: every ecosystem-reader filesystem walker migrated to a shared `safe_walk` helper. No user-visible behavior change. (Issue #108.)`
- [X] T029 [US2] Run `MIKEBOM_SKIP_DOCKER_INTEGRATION=1 ./scripts/pre-pr.sh` + verify per-PR byte-identity gate per US3 / SC-002 (this PR is docs+cleanup so byte-identity is automatically preserved, but the gate runs for completeness)

**Checkpoint — end of PR 5**: every acceptance criterion from spec.md US1+US2 is met. SC-001 verified by grep. SC-005 verified by the inventory match against research R5. SC-006 verified by reading `docs/design-notes.md`.

---

## Phase 5: User Story 3 — Byte-identity guarantee verification (Priority: P1)

**Goal**: Operators see byte-identical SBOM output across the migration. The 33 committed byte-identity goldens pass without regen.

**Independent Test**: `cargo +stable test --workspace` post-migration passes without any `MIKEBOM_UPDATE_*_GOLDENS=1` invocation. A scan of the knative/func realistic-project fixture pre- and post-migration produces byte-identical CDX output.

**Note**: This story's verification is embedded in T010 / T014 / T018 / T023 / T029 (the per-PR gates inside US1/US2). The tasks below are the cross-PR sweep that confirms the cumulative result.

- [X] T030 [US3] Final workspace pre-PR gate after all 5 PRs merge: run `MIKEBOM_SKIP_DOCKER_INTEGRATION=1 ./scripts/pre-pr.sh` against the post-migration branch HEAD; both clippy `--workspace --all-targets -D warnings` and full workspace tests must pass clean
- [X] T031 [US3] Run the realistic-projects workflow locally OR observe its CI run on the final PR: scan `knative/func` (the existing realistic-project CI fixture) on Linux x86_64 + macOS aarch64; confirm CDX output bytes match the pre-migration baseline captured at T001 (modulo the version string the goldens already mask)
- [X] T032 [US3] Spot-check SC-004 (≤105% scan time): time `cargo run -p mikebom -- sbom scan --path mikebom-cli/tests/fixtures/exclude_path --format cyclonedx-json --no-deep-hash --offline --output /tmp/out.cdx.json` on the pre-migration HEAD vs the post-migration HEAD; the with-migration wall time should be ≤ 1.05 × pre-migration. Documented as a single-run observation rather than a CI gate — formal benchmark is out of scope per research R8

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T033 [P] Update agent-context file (`/Users/mlieberman/Projects/mikebom/CLAUDE.md`) to mention the new `scan_fs::walk` module — automated via `.specify/scripts/bash/update-agent-context.sh claude` (already executed during /speckit-plan; verify the entry persisted in the CLAUDE.md `Active Technologies` section)
- [X] T034 Spot-check the audit grep produces a result consistent with T026: run `grep -rEn 'fn walk[_(]' mikebom-cli/src/scan_fs/` and assert every output line is in the documented acceptable-match union (three filesystem-walker files + the non-walker false-positive list from research R6). The line count itself is NOT a fixed expected number — test modules add/remove `walks_*` tests over time, and that's fine. The audit passes iff every line is documented; the audit fails iff a single line is undocumented (which indicates a regression)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 Setup**: no deps, run first.
- **Phase 2 Foundational**: depends on Phase 1; BLOCKS US1/US2/US3.
- **Phase 3 US1 (per-PR migration)**: depends on Phase 2 complete; sequenced as PR 1 → PR 2 → PR 3 → PR 4 (each PR's byte-identity gate must pass before the next PR opens).
- **Phase 4 US2 (audit + docs)**: depends on US1 complete (the dead-code cleanup in T024–T025 requires all walkers already migrated).
- **Phase 5 US3 (byte-identity verification)**: per-PR verification (T010, T014, T018, T023, T029) embedded in US1/US2 tasks; T030–T032 are the cross-PR sweep at the end.
- **Phase 6 Polish**: T033/T034 run after every other phase complete.

### Per-PR Mapping

| PR | Tasks | Files touched | Risk |
|---|---|---|---|
| **PR 1** (foundational + reference port) | T004 → T010 | `scan_fs/walk.rs` (new), `scan_fs/mod.rs`, `scan_fs/package_db/project_roots.rs` | Low — helper is greenfield, project_roots is the source-of-truth structure |
| **PR 2** (cargo + gem + rpm_file) | T011 → T014 | `cargo.rs`, `gem.rs`, `rpm_file.rs` | Medium — 6 walker functions ported; each ecosystem's test suite is the verification |
| **PR 3** (maven + golang + go_binary) | T015 → T018 | `maven.rs`, `golang/legacy.rs`, `go_binary.rs` | Medium-High — biggest PR by line count; preserves milestone-113 Go skips and milestone-112 build-inclusion test scope |
| **PR 4** (nuget + yocto + binary walkers) | T019 → T023 | `nuget/mod.rs`, `yocto/recipe.rs`, `binary/discover.rs`, `binary/source_binding/cmake_observer.rs` | Medium — 4 walkers; smallest tests per ecosystem |
| **PR 5** (cleanup + docs + audit) | T024 → T029, T033, T034 | `project_roots.rs` (delete dead code), pip/npm/gradle/nuget/yocto (import path), `docs/design-notes.md`, `CHANGELOG.md` | Low — pure cleanup + docs, no behavior change |

### Parallel Opportunities

- T010/T011/T012/T013 within PR 2: each ports a different file → all 3 implementation tasks ([P]).
- T015/T016/T017 within PR 3: each ports a different file → all 3 ([P]).
- T019/T020/T021/T022 within PR 4: all 4 ([P]).
- T011 + T012 + T013 are independent of each other (different ecosystem files); same for T015–T017 and T019–T022.

---

## Parallel Example: PR 2 (cargo + gem + rpm_file)

```bash
# Three independent ports in one PR:
Task T011: Port cargo.rs walk functions in mikebom-cli/src/scan_fs/package_db/cargo.rs
Task T012: Port gem.rs walk functions in mikebom-cli/src/scan_fs/package_db/gem.rs
Task T013: Port rpm_file.rs walk_dir in mikebom-cli/src/scan_fs/package_db/rpm_file.rs
# Three developers (or three parallel agent runs) port their respective files in parallel.
# After all three land, T014 runs the byte-identity gate.
```

---

## Implementation Strategy

### MVP (User Story 1 only — PR 1 alone)

1. Complete Phase 1 (T001–T003): baseline + smoke-verify the known exceptions.
2. Complete Phase 2 (T004–T008): introduce the helper module + 12 unit tests.
3. Complete PR 1 of Phase 3 (T009–T010): port `project_roots.rs` to delegate to the helper. The thin-shim approach keeps pip/npm/gradle/nuget/yocto closures compiling unchanged.
4. STOP and VALIDATE: this is a shippable PR by itself. The helper is in production and project_roots is the proof-of-concept port. The 12 remaining walker ports can ship in follow-up PRs without coordination.

### Incremental Delivery (recommended — matches research R4's 5-PR plan)

1. **PR 1** (Phase 2 + Phase 3 PR 1): helper module + project_roots port. Demonstrates the migration shape; ~300 lines.
2. **PR 2** (Phase 3 PR 2): cargo + gem + rpm_file ports. ~250 lines.
3. **PR 3** (Phase 3 PR 3): maven + golang + go_binary ports. ~450 lines (biggest).
4. **PR 4** (Phase 3 PR 4): nuget + yocto + binary/discover + binary/source_binding/cmake_observer ports. ~300 lines.
5. **PR 5** (Phase 4): cleanup, audit verification, docs, CHANGELOG. ~150 lines.

Each PR independently passes the byte-identity gate at its boundary (T010, T014, T018, T023, T029). The migration as a whole is reviewable PR-by-PR, with no PR depending on a later one for correctness.

### Parallel Team Strategy

With multiple developers:

1. PR 1 lands first (single developer; foundational).
2. After PR 1 merges, PR 2 / PR 3 / PR 4's individual file ports can be parallelized across developers:
   - Developer A: cargo + gem + rpm_file (PR 2)
   - Developer B: maven (one file in PR 3)
   - Developer C: golang + go_binary (PR 3 cont'd)
   - Developer D: nuget + yocto + 2 binary walkers (PR 4)
3. PR 5 runs after every walker is ported (sequential dependency).

---

## Notes

- **Tests are inline, not a separate phase**: the byte-identity gate at every PR boundary (T010, T014, T018, T023, T029) substitutes for a dedicated US3 test phase. The existing 33 golden files + every walker's existing per-ecosystem test suite already cover the path-set equality invariant; no new test infrastructure is needed beyond the 12 unit tests on the helper itself.
- **The audit grep is the durability mechanism**: T026 is a one-liner gate but it's the project's principal defense against re-introduction of hand-rolled walkers. A future CI hook can codify it (deferred per spec Assumptions).
- **Known exceptions**: only two walkers (the deep-hash whole-FS walker and the npm `@scope`-aware walker) stay hand-rolled. Both are documented inside `scan_fs/walk.rs`'s comment block. Any new known exception requires a research-equivalent justification at review time.
