---

description: "Task list for milestone 137 — Dart/Flutter pub ecosystem reader"
---

# Tasks: Dart/Flutter pub ecosystem reader

**Input**: Design documents from `/specs/137-dart-pub-reader/`
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/pub-component-purl.md ✓, quickstart.md ✓

**Tests**: Integration tests included — established convention for milestones 064 / 066 / 068 / 069 / 070 / 122 / 135 / 136 main-module-reader work. Synthetic-fixture pattern via `tempfile::tempdir()`.

**Organization**: Tasks grouped by user story (US1 = P1 MVP; US2 = P2 source-discriminator distinction; US3 = P3 design-tier mode). Setup + Foundational phases are blocking prerequisites for ALL user stories.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Maps task to user story phase (US1 / US2 / US3)
- Setup / Foundational / Polish phases: no story label

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Module skeleton + cyclonedx evidence-kind enum extension before any logic lands.

- [X] T001 Create `mikebom-cli/src/scan_fs/package_db/dart.rs` with module-level docstring (mirrors cargo.rs:1–30 + maven.rs:1–25 preamble: milestone reference, FR list, PURL shape summary), `use` block (`anyhow`, `serde`, `serde_yaml`, `tracing`, `std::collections::BTreeMap`, `std::path::{Path, PathBuf}`, `mikebom_common::types::purl::Purl`, the existing `PackageDbEntry`/`SourceType`/`LifecycleScope` types from `super`), and `pub fn read(rootfs: &Path, include_dev: bool, exclude_set: &ExclusionSet) -> Vec<PackageDbEntry>` stub returning `Vec::new()`.

- [X] T002 Add `pub mod dart;` declaration to `mikebom-cli/src/scan_fs/package_db/mod.rs` (placed alphabetically between `pub mod conan;` and `pub mod gem;`). No `read_all` integration yet — that lands in T008.

- [X] T002b Extend the cyclonedx evidence-kind allowlist in `mikebom-cli/src/generate/cyclonedx/builder.rs` to accept `"pubspec-lock"` and `"pubspec-yaml"`. The `debug_assert!` gate currently enumerates {rpm-file, rpmdb-sqlite, rpmdb-bdb, dynamic-linkage, elf-note-package, embedded-version-string, symbol-fingerprint, python-stdlib-collapsed, jdk-runtime-collapsed, alpm-local-db, brew-install-receipt, brew-cask-metadata} — add the two new values per the milestone-135 + milestone-136 T002b pattern. Without this, the implementation in T010 + T018 would panic in debug builds.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Reader-private types + parsing + PURL helpers + dispatcher integration. MUST complete before ANY user story phase.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T003 Define reader-private serde structs in `mikebom-cli/src/scan_fs/package_db/dart.rs` per `data-model.md`: `PubspecYaml` (name, version, description, dependencies, dev_dependencies, dependency_overrides, environment), `PubspecLock` (packages, sdks), `LockfileEntry` (dependency, description, source, version), `LockfileDescription` enum with `#[serde(untagged)]` covering `Sdk(String)` and `Map(LockfileDescriptionMap)` variants, and `LockfileDescriptionMap` (name, sha256, url, ref_, resolved_ref, path, relative). All fields use `#[serde(default)]` except the universally-present-in-modern-Dart-2.0+ four (`dependency`, `description`, `source`, `version` on `LockfileEntry`). `#[allow(dead_code)]` on each struct.

- [X] T004 Implement `fn find_dart_projects(rootfs: &Path, exclude_set: &ExclusionSet) -> Vec<PathBuf>` in `dart.rs` that walks via `scan_fs::walk::safe_walk` (milestone 114) returning every absolute path containing a `pubspec.yaml` (one per directory). For each `pubspec.yaml`, the caller will look for a sibling `pubspec.lock`. Skip directories matching `exclude_set`.

- [X] T005 Implement `fn parse_pubspec_yaml(path: &Path) -> Result<PubspecYaml>` and `fn parse_pubspec_lock(path: &Path) -> Result<PubspecLock>` in `dart.rs` using `serde_yaml::from_slice` over `std::fs::read`. Errors propagate via `anyhow::Result` so callers can warn-and-skip per FR-007. Use `?` operator — no `.unwrap()` per Constitution Principle IV.

- [X] T006 Implement `fn build_purl_for_lockfile_entry(name: &str, entry: &LockfileEntry) -> Result<Purl>` in `dart.rs` per FR-003 + contracts/pub-component-purl.md:
  - `source: "hosted"` → `pkg:pub/<name>@<version>`; if `description.url.as_deref()` is `Some(u)` AND `u != "https://pub.dev"` AND `u != "https://pub.dartlang.org"`, append `?repository_url=<u>` qualifier. URL-encode per the minimal-encoding rule defined in T016 (PURL `pchar` rule allows `:` / `/` / `?` / `@` in qualifier values; encode only ` `, `?`, `#`, `&`). Do NOT use `url::form_urlencoded` — that would over-escape `:`/`/` and break URL readability.
  - `source: "git"` → require `description.resolved_ref` present + 40 hex chars; produce `pkg:pub/<name>@<resolved-sha>?vcs_url=git+<url>` with `#<subpath>` fragment when `description.path` is `Some(p)` AND `p != "."` AND `!p.is_empty()`.
  - `source: "path"` → `pkg:generic/<name>@<version>` (placeholder per R1; discriminator surfaces via annotation, not PURL).
  - `source: "sdk"` → `pkg:pub/<sdk-name>@0.0.0` where `<sdk-name>` is from `LockfileDescription::Sdk(s)`; preserve `0.0.0` verbatim per purl-spec canonical example.
  - Unknown `source:` value → `Err(...)` so caller warns-and-skips per FR-007.

- [X] T007 Implement `fn build_extra_annotations(name: &str, entry: &LockfileEntry) -> BTreeMap<String, serde_json::Value>` in `dart.rs` per data-model.md per-source-type fields. Always sets `"mikebom:source-type"` to one of `pub-hosted` / `pub-git` / `pub-path` / `pub-sdk`. Source-specific extras: git → `"mikebom:vcs-ref": <ref_>`; path → `"mikebom:path": <path>`; sdk → `"mikebom:sdk-name": <sdk-family>` (from the `LockfileDescription::Sdk` payload, which IS the sdk-family per R2).

- [X] T008 Wire `dart::read(rootfs, include_dev, exclude_set)` into `read_all` in `mikebom-cli/src/scan_fs/package_db/mod.rs`. Place the call alphabetically between `conan::read(...)` and `gem::read(...)` calls. Mirror the cargo pattern (`out.extend(cargo_out.entries);`) — Dart's signature is simpler (returns `Vec<PackageDbEntry>` directly, no separate divergence record set per R4). NO `collect_claimed_paths` integration — language readers don't claim binary paths.

**Checkpoint**: Foundation ready — `dart::read` is callable from the dispatcher, returns empty Vec, and the cyclonedx evidence-kind gate accepts the new values. User story phases (US1 / US2 / US3) can now proceed in parallel.

---

## Phase 3: User Story 1 — Operator scans a Flutter app project (Priority: P1) 🎯 MVP

**Goal**: Lockfile-driven SBOM emission for the canonical Flutter-app case — one main-module per `pubspec.yaml` + one component per lockfile entry + dep edges from the main-module to its direct deps.

**Independent Test (SC-001)**: Synthetic fixture with `pubspec.yaml` (name=my_flutter_app, version=1.2.3, 3 direct deps) + `pubspec.lock` (5 packages — 3 direct + 2 transitives). Scan produces exactly 5 `pkg:pub/*` components + 1 main-module + main-module's `depends` lists the 3 direct deps by name.

### Implementation for User Story 1

- [X] T009 [US1] Implement `fn emit_main_module(pubspec_path: &Path, pubspec_yaml: &PubspecYaml) -> Option<PackageDbEntry>` in `dart.rs` per FR-012. Returns `None` when `pubspec_yaml.name.is_empty()` (warn-and-skip per R7). Builds a `PackageDbEntry` with: `purl = Purl::new(format!("pkg:pub/{name}@{version}", version = pubspec_yaml.version.as_deref().unwrap_or("0.0.0-unknown")))?`, `name = pubspec_yaml.name.clone()`, `version = pubspec_yaml.version.clone().unwrap_or_else(|| "0.0.0-unknown".into())`, `source_path = Some(pubspec_path.to_path_buf())`, `evidence_kind = Some("pubspec-yaml".into())`, `sbom_tier = Some("source".into())`, `source_type = Some("pub-main-module".into())`, `extra_annotations` containing `"mikebom:component-role": "main-module"` + `"mikebom:source-type": "pub-main-module"`. `depends` populated in T011.

- [X] T010 [US1] Implement `fn emit_lockfile_entries(lockfile_path: &Path, pubspec_lock: &PubspecLock, include_dev: bool) -> Vec<PackageDbEntry>` in `dart.rs` per FR-002 + FR-003 + FR-008. For each `(name, entry)` in `pubspec_lock.packages`:
  - If `entry.dependency == "direct dev"` AND `!include_dev`, skip (per FR-008 + language-reader convention).
  - Call `build_purl_for_lockfile_entry(name, entry)` — on `Err`, `tracing::warn!` with the package name + lockfile path and `continue`.
  - Construct `PackageDbEntry` with: `purl`, `name = name.clone()`, `version = entry.version.clone()`, `source_path = Some(lockfile_path.to_path_buf())`, `lifecycle_scope = if entry.dependency == "direct dev" { Some(LifecycleScope::Development) } else { Some(LifecycleScope::Runtime) }`, `evidence_kind = Some("pubspec-lock".into())`, `sbom_tier = Some("source".into())`, `source_type = Some(source_type_string)` (where source_type_string is the `pub-hosted`/`pub-git`/`pub-path`/`pub-sdk` value matching the source), `extra_annotations = build_extra_annotations(name, entry)`, `hashes = ...` (for hosted with sha256 present, build `ContentHash::sha256(hex)`; else empty).

- [X] T011 [US1] Implement the `dart::read` orchestrator body in `dart.rs`: walk for `pubspec.yaml` files via `find_dart_projects`; for each `pubspec_path`, attempt `parse_pubspec_yaml`; on error → `tracing::warn!` + skip. Compute sibling `pubspec.lock` path (same directory). If lockfile present and parses → emit main-module via T009 (with `depends` populated from `pubspec_lock.packages` keys filtered to `direct main` + (if include_dev) `direct dev`) + emit lockfile entries via T010. If lockfile absent → defer to design-tier path (T019). Append all `PackageDbEntry`s to the output Vec.

- [X] T012 [US1] Write integration test file `mikebom-cli/tests/dart_flutter_app_baseline.rs` with `#[test]` functions covering:
  - `flutter_app_baseline_emits_lockfile_count_plus_main_module` — SC-001 5-component count assertion (3 direct + 2 transitive + 1 main-module = 6 dart components total in the emitted CDX).
  - `main_module_emission` — SC-008 PURL `pkg:pub/my_flutter_app@1.2.3` exists in `components[]` with `mikebom:component-role = main-module` property.
  - `main_module_depends_lists_direct_deps` — US1 acceptance scenario 4: the SBOM's `dependencies[]` entry for the main-module's `bom-ref` carries `dependsOn` targeting each direct dep's bom-ref.
  - `dev_scope_filterability` — SC-007: a fixture with a `direct dev` entry produces a component with `mikebom:lifecycle-scope = development`; running with `--no-include-dev` suppresses it.
  
  Use `tempfile::tempdir()` + helper functions to write synthetic `pubspec.yaml` + `pubspec.lock` files; invoke via `std::process::Command::new(env!("CARGO_BIN_EXE_mikebom"))`. Pattern matches `mikebom-cli/tests/arch_alpm_baseline.rs` (milestone 135) and `mikebom-cli/tests/homebrew_baseline.rs` (milestone 136). Guard `.unwrap()` calls with `#[cfg_attr(test, allow(clippy::unwrap_used))]` per CLAUDE.md convention.

**Checkpoint**: At this point, US1 (the headline MVP — Flutter app scan with main-module + lockfile-driven components + dep edges) is fully functional and SC-001, SC-007, SC-008 all pass independently.

---

## Phase 4: User Story 2 — Operator distinguishes hosted vs git/path/SDK deps (Priority: P2)

**Goal**: Surface the four-way source-type discriminator so downstream supply-chain tooling can correctly classify each dep's risk profile.

**Independent Test (SC-002)**: Synthetic fixture with one each of hosted / git / path / sdk in `pubspec.lock`. Scan. Assert correct PURL shape per FR-003 for each + correct `mikebom:source-type` annotation value.

### Implementation for User Story 2

US2's source-discriminator helpers (`build_purl_for_lockfile_entry` + `build_extra_annotations`) are already implemented in foundational phase (T006 + T007). This phase adds end-to-end correctness validation + the self-hosted hosted-source qualifier branch.

- [X] T013 [US2] Augment `build_purl_for_lockfile_entry` (T006) with the inline correctness invariants for git source: `Err` when `description.resolved_ref` is absent or not 40 hex chars (per spec Edge Cases "Git deps without a resolved SHA"). Strip `#<subpath>` when `path == "."` or empty per R3. Use `.chars().all(|c| c.is_ascii_hexdigit())` for SHA validation.

- [X] T014 [US2] Augment `build_purl_for_lockfile_entry` (T006) with the path-source variant: `pkg:generic/<name>@<version>`. No URL encoding needed on `<name>` (dart package names are lowercase-snake-case per pub.dev publish rules); use the verbatim lockfile key.

- [X] T015 [US2] Augment `build_purl_for_lockfile_entry` (T006) with the SDK-source variant: match `entry.description` against `LockfileDescription::Sdk(sdk_name)` (NOT `Map`) — preserving the polymorphism per R2; produce `pkg:pub/{sdk_name}@0.0.0` (the `0.0.0` is literal, never read from `entry.version` which itself is `"0.0.0"` per pub convention).

- [X] T016 [US2] Augment `build_purl_for_lockfile_entry` (T006) with the self-hosted hosted-source qualifier branch: when `description.url.as_deref() == Some(u)` AND `u != "https://pub.dev"` AND `u != "https://pub.dartlang.org"`, append `?repository_url=<u>` qualifier. Apply minimal URL-encoding for qualifier-value safety (PURL spec allows `:`/`/` in qualifier values; encode only ` `, `?`, `#`, `&`).

- [X] T017 [US2] Write integration test file `mikebom-cli/tests/dart_source_discriminators.rs` with `#[test]` functions covering SC-002 per the contracts/pub-component-purl.md example table:
  - `hosted_default_pubdev_emits_bare_purl` — `pkg:pub/http@1.1.0` (no qualifier).
  - `hosted_self_hosted_emits_repository_url_qualifier` — `pkg:pub/internal_lib@2.0.0?repository_url=https://pub.acme.example.com`.
  - `git_source_emits_resolved_sha_plus_vcs_url` — `pkg:pub/window_size@eb39649...3d5601?vcs_url=git+https://github.com/google/flutter-desktop-embedding.git#plugins/window_size`.
  - `path_source_emits_generic_placeholder` — `pkg:generic/my_local_lib@0.1.0` with `mikebom:source-type = pub-path` property.
  - `sdk_source_emits_zero_zero_zero_version` — `pkg:pub/flutter@0.0.0` and `pkg:pub/flutter_test@0.0.0` with `mikebom:source-type = pub-sdk`.

**Checkpoint**: US1 + US2 both functional. The full lockfile-driven SBOM (hosted + git + path + sdk) is correctly classified, addressable via standard `purl` and `properties[].name = mikebom:source-type` filters.

---

## Phase 5: User Story 3 — Operator scans a Dart project WITHOUT a committed lockfile (Priority: P3)

**Goal**: Design-tier emission for library projects + pre-`pub get` workflows — `pubspec.yaml`-only scans surface declared direct deps (runtime + dev) with the constraint string preserved as evidence.

**Independent Test (SC-003)**: Synthetic fixture with `pubspec.yaml` declaring 2 direct deps but NO `pubspec.lock`. Scan. Assert 2 components emit with `mikebom:sbom-tier = "design"` annotation + `mikebom:requirement-range = <constraint-str>` evidence.

### Implementation for User Story 3

- [X] T018 [US3] Implement `fn emit_design_tier_components(pubspec_path: &Path, pubspec_yaml: &PubspecYaml, include_dev: bool) -> Vec<PackageDbEntry>` in `dart.rs` per FR-005. For each `(name, value)` in `pubspec_yaml.dependencies` (always) and `pubspec_yaml.dev_dependencies` (only when `include_dev`):
  - Extract constraint string from `value` — if scalar string, use verbatim; if map (e.g., `path:` / `git:` / `sdk:` directive), use placeholder string `"unspecified"` (lockfile is the only place where constraints get resolved for non-hosted sources).
  - Build PURL `pkg:pub/<name>@<constraint-str>` for hosted-shaped declarations; for non-scalar values, emit `pkg:pub/<name>@unspecified`.
  - Construct `PackageDbEntry` with: `purl`, `name = name.clone()`, `version = constraint-str.clone()`, `source_path = Some(pubspec_path.to_path_buf())`, `lifecycle_scope = if is_dev_block { Some(LifecycleScope::Development) } else { Some(LifecycleScope::Runtime) }`, `evidence_kind = Some("pubspec-yaml".into())`, `sbom_tier = Some("design".into())`, `requirement_range = Some(constraint-str.clone())`, `source_type = Some("pub-hosted".into())` (design-tier is best-effort), `extra_annotations = { "mikebom:source-type": "pub-hosted" }`.

- [X] T019 [US3] Wire design-tier path into the `dart::read` orchestrator (T011): when sibling `pubspec.lock` does NOT exist OR fails to parse, fall back to `emit_design_tier_components(pubspec_path, &pubspec_yaml, include_dev)` per R7. The main-module STILL emits via T009 (with `depends` populated from `pubspec_yaml.dependencies` keys + (if include_dev) `dev_dependencies` keys).

- [X] T020 [US3] Write integration test file `mikebom-cli/tests/dart_design_tier.rs` with `#[test]` functions covering:
  - `design_tier_no_lockfile_emits_constraints` — SC-003: fixture with `pubspec.yaml` declaring `http: ^1.0.0` + `provider: ^6.1.0`; assert 2 components emit each with `mikebom:sbom-tier = design` and `mikebom:requirement-range = ^1.0.0` (and `^6.1.0`).
  - `design_tier_no_transitive_deps` — US3 acceptance scenario 2: assert NO components emit for packages not declared in pubspec.yaml (transitive deps require lockfile per spec).
  - `design_tier_dev_deps_carry_lifecycle_scope` — US3 acceptance scenario 3: fixture with `dev_dependencies: { test: ^1.24.0 }` → emitted `test` component carries `mikebom:lifecycle-scope = development`; rerun with `--no-include-dev` suppresses it.

**Checkpoint**: All three user stories independently functional. US3 enables the library-publisher scan workflow without requiring a committed lockfile.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Edge-case coverage + invariant validation + pre-PR gate.

- [X] T021 [P] Write integration test file `mikebom-cli/tests/dart_edge_cases.rs` covering the spec Edge Cases section + SC-005:
  - `malformed_lockfile_falls_back_to_design_tier` — SC-005: 4-project fixture where one lockfile has corrupted YAML; scan succeeds (exit 0); the 3 valid projects emit normally + the 4th falls back to design-tier from its pubspec.yaml; `tracing::warn!` fires.
  - `workspace_monorepo_emits_one_main_module_per_pubspec` — FR-009: Melos-shaped fixture with 3 packages under `packages/` each with own `pubspec.yaml` + `pubspec.lock`; assert 3 main-module components emit; assert NO synthetic workspace-root component.
  - `missing_version_falls_back_to_unknown_placeholder` — FR-012: `pubspec.yaml` without `version:` field emits main-module with PURL `pkg:pub/<name>@0.0.0-unknown`.
  - `self_hosted_registry_emits_repository_url_qualifier` — spec Edge Cases: a lockfile entry from `https://pub.acme.example.com` emits PURL with `?repository_url=https://pub.acme.example.com` qualifier.
  - `sdk_pseudo_deps_emit_zero_zero_zero` — FR-011: lockfile with `flutter` SDK pseudo-dep emits `pkg:pub/flutter@0.0.0` with `mikebom:source-type = pub-sdk` annotation.
  - `direct_overridden_treated_as_runtime` — R2 lifecycle mapping: lockfile entry with `dependency: "direct overridden"` emits as `lifecycle-scope: Runtime` (no `mikebom:lifecycle-scope = development` annotation).
  - `empty_packages_block_emits_only_main_module` — Edge Cases: `pubspec.lock` with `packages: {}` emits just the main-module; no warnings; no dep components.
  - `git_source_missing_resolved_ref_warns_and_skips` — Edge Cases: lockfile with git entry lacking `resolved-ref:` triggers `tracing::warn!` + skips that single entry (other lockfile entries still emit).

- [X] T022 [P] Verify SC-004 no-Dart-rootfs byte-identity invariant by running the existing CDX/SPDX 2.3/SPDX 3 regression test suites against a synthetic fixture containing zero Dart files. Confirm the emitted SBOMs are byte-identical (modulo timestamps + serial numbers) to a pre-feature baseline. Command: `cargo +stable test -p mikebom --test cdx_regression --test spdx_regression --test spdx3_regression`. Document the invariant validation in the test file comments.

- [X] T023 Run `./scripts/pre-pr.sh` from repo root to confirm clippy + workspace test gates pass per CLAUDE.md MANDATORY pre-PR gate. Fix any clippy warnings (especially `unwrap_used` in test files — guard with `#[cfg_attr(test, allow(clippy::unwrap_used))]` per convention) and any failing tests. Re-run until both lanes show `0 errors` / `N passed; 0 failed`.

- [X] T024 Run the quickstart.md SC-006 standard-PURL-filter check + the cross-format byte-equivalence diff (CDX vs SPDX 2.3 vs SPDX 3) on a synthetic Flutter-app fixture. The three formats' Dart-component PURL sets MUST be identical when sorted. Document any divergences (none expected — Dart components flow through the standard PackageDbEntry pipeline).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001 → T002 (T002 imports the module declared in T001); T002b independent. T002b BLOCKS T010 (cyclonedx gate would panic in debug builds without T002b).
- **Foundational (Phase 2)**: All foundational tasks depend on Phase 1 completion. Within Phase 2: T003 → T005 (parser needs struct definitions); T003 → T006 → T007 (PURL helper needs structs; annotation helper has no logical dep on PURL helper but lives in same file); T004 independent; T008 depends on `dart::read` stub existing (T001) — but `read_all` integration in T008 can land before T011 fills in the orchestrator body (intermediate state: dispatcher calls `dart::read` which returns empty Vec).
- **User Story phases (Phase 3 / 4 / 5)**: ALL depend on Foundational completion. Within each phase, tasks are sequential by default unless marked `[P]`.
- **Polish (Phase 6)**: T021 + T022 marked `[P]` — independent files. T023 + T024 depend on all preceding phases.

### User Story Dependencies

- **US1 (P1) MVP**: Depends on Foundational. T012 (integration test) depends on T009 + T010 + T011. T009 / T010 / T011 can be implemented in sequence (T009 first since main-module is the simplest, then T010 for lockfile entries, then T011 for orchestration + dep edges).
- **US2 (P2)**: Depends on Foundational (T006 + T007 already exist after Foundational). T013–T016 augment T006/T007 in-place. T017 (integration test) depends on T013–T016 + T010 (US2 builds atop US1's emit_lockfile_entries).
- **US3 (P3)**: Depends on Foundational + T009 (main-module emission is shared between lockfile + design-tier paths). T018 → T019 → T020.

### Within Each User Story

- Models → services → integration tests (standard ordering).
- No TDD ordering imposed for this milestone — tests follow implementation per the milestone-135/136 precedent (tests are integration-test-shaped, not unit-test-shaped; they validate end-to-end CDX emission against the running binary).

### Parallel Opportunities

- **Phase 1**: T001 + T002b can run in parallel (different files). T002 sequential after T001.
- **Phase 2**: T004 independent of T003/T005/T006/T007 (walker logic doesn't need struct definitions).
- **Phase 3**: T009 + T010 can run in parallel only if developer is careful — both modify `dart.rs`. In practice, sequentialize: T009 → T010 → T011 → T012.
- **Phase 4**: T013/T014/T015/T016 all modify `build_purl_for_lockfile_entry` in `dart.rs` — sequential.
- **Phase 5**: T018 → T019 → T020 sequential.
- **Phase 6**: T021 + T022 in parallel (different test files); T023 + T024 sequential after.

---

## Parallel Example: Phase 1 + Foundational kickoff

```bash
# Phase 1 — module + cyclonedx gate extension in parallel:
Task: "T001 Create mikebom-cli/src/scan_fs/package_db/dart.rs skeleton"
Task: "T002b Extend evidence-kind enum in mikebom-cli/src/generate/cyclonedx/builder.rs"

# Then T002 (depends on T001):
Task: "T002 Add `pub mod dart;` to mikebom-cli/src/scan_fs/package_db/mod.rs"

# Phase 2 — walker + struct definitions in parallel:
Task: "T003 Define PubspecYaml/PubspecLock/LockfileEntry/LockfileDescription structs"
Task: "T004 Implement find_dart_projects walker"
```

---

## Implementation Strategy

### MVP First (US1 — P1)

1. Complete Phase 1: Setup (T001, T002, T002b — module skeleton + cyclonedx gate extension).
2. Complete Phase 2: Foundational (T003–T008 — structs, parsers, PURL+annotation helpers, dispatcher integration).
3. Complete Phase 3: US1 (T009–T012 — main-module + lockfile-driven emission + dep edges + integration test).
4. **STOP and VALIDATE**: Run `cargo +stable test -p mikebom --test dart_flutter_app_baseline` and confirm SC-001 + SC-007 + SC-008 pass.
5. Deploy/demo if ready — the headline use case (Flutter app scan with PURLs + dep edges) ships independently.

### Incremental Delivery

1. Setup + Foundational → dispatch wired, empty Vec.
2. + US1 → MVP shippable: Flutter app scans emit full pinned dep graph + main-module.
3. + US2 → adds source-discriminator filterability (hosted vs git vs path vs sdk).
4. + US3 → adds library-project / pre-`pub get` workflow support (design-tier from pubspec.yaml only).
5. + Polish → edge-case coverage + invariant validation + pre-PR gate green.

### Single-PR Pattern (per milestone-135/136 convention)

All phases land in ONE PR per the established Dart-reader milestone pattern. The single PR includes:
- Module: `mikebom-cli/src/scan_fs/package_db/dart.rs` (~500–800 LOC including doc-comments)
- Dispatcher integration: `mikebom-cli/src/scan_fs/package_db/mod.rs` (≤10 LOC)
- Cyclonedx evidence-kind extension: `mikebom-cli/src/generate/cyclonedx/builder.rs` (≤5 LOC)
- Four integration test files: `mikebom-cli/tests/dart_*.rs` (~600–900 LOC total)
- Closes #420.

---

## Notes

- `[P]` tasks = different files, no dependencies on incomplete tasks.
- `[Story]` label maps task to specific user story for traceability.
- Each user story independently completable and testable.
- Commit after each logical group (e.g., T001–T002b together; T003–T008 together; T009–T012 together; etc.).
- Stop at any checkpoint to validate independently.
- Avoid: cross-story dependencies that break independence (US2 + US3 can ship after US1 in either order).
- **Pre-PR gate (CLAUDE.md MANDATORY)**: `./scripts/pre-pr.sh` MUST pass before opening PR; per-crate `cargo test -p mikebom` is insufficient (clippy `--all-targets` enforces `unwrap_used` inside test mods).
- **No-op invariant (SC-004)**: every change MUST preserve byte-identical SBOM output on non-Dart source trees. T022 is the validation.
- **Per-PR PR-friendly diff size estimate**: ~1,500–2,000 LOC total (matches milestone-135 + milestone-136 PR sizes).
