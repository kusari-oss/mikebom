---

description: "Task list for milestone 142 — Scala/SBT ecosystem reader"
---

# Tasks: Scala/SBT ecosystem reader

**Input**: Design documents from `/specs/142-scala-sbt-reader/`
**Prerequisites**: plan.md ✓, spec.md ✓ (with Q1+Q2+Q3 clarifications), research.md ✓, data-model.md ✓, contracts/scala-component-purl.md ✓, quickstart.md ✓

**Tests**: Integration tests included — established convention for milestones 064 / 066 / 068 / 069 / 070 / 122 / 135 / 136 / 137 / 138 / 139 / 140 / 141. Synthetic-fixture pattern via `tempfile::tempdir()`.

**Organization**: Tasks grouped by user story (US1 = P1 MVP `.sbt.lock` baseline; US2 = P2 Scala 2 vs 3 vs Java discriminator + cross-built distinct components + main-module emission; US3 = P3 design-tier fallback + Q1 cascade + Q2 multi-project + Q3 content-shape). Setup + Foundational phases are blocking prerequisites for ALL user stories.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Maps task to user story phase (US1 / US2 / US3)
- Setup / Foundational / Polish phases: no story label

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Module skeleton + mod.rs declaration. No `read_all` integration yet — that lands in T014 once the parse pipeline is wired up.

- [X] T001 Create `mikebom-cli/src/scan_fs/package_db/scala.rs` with module-level docstring (mirrors erlang.rs preamble: milestone reference 142, FR list, PURL shape summary, Q1+Q2+Q3 clarifications recap, research §R1+R3+R5 references), `use` block (`anyhow`, `serde_json::{self, json, Value}`, `tracing::{warn, debug}`, `std::collections::{BTreeMap, HashSet, HashMap}`, `std::path::{Path, PathBuf}`, `std::sync::OnceLock`, `regex::Regex`, `mikebom_common::types::purl::Purl`, `mikebom_common::types::hash::{ContentHash, HashAlgorithm}`, `mikebom_common::resolution::LifecycleScope`, the existing `PackageDbEntry` from `super`, `ExclusionSet` from `super::exclude_path`), and `pub fn read(rootfs: &Path, _include_dev: bool, exclude_set: &ExclusionSet) -> Vec<PackageDbEntry>` stub returning `Vec::new()`.

- [X] T002 Add `pub mod scala;` declaration to `mikebom-cli/src/scan_fs/package_db/mod.rs` (placed alphabetically — after `pub mod rpm*;` family and before `pub mod swift;`). No `read_all` integration yet — that lands in T014.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared helpers used by all user stories — types, PURL constructors, regex compile-once helpers, the `apply_scala_suffix` algorithm, the Q1 inference cascade helper, the Q3 content-shape validation gate, the multi-line paren-counted tokenizer.

- [X] T003 Add private enums + structs in `mikebom-cli/src/scan_fs/package_db/scala.rs` per data-model §2: `enum DeclKind { SinglePercent, DoublePercent, TriplePercent }` (with `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`), `struct SbtLockEntry { org, name, version, configurations: Vec<String>, sha256: Option<String> }`, `struct DeclaredSbtDep { group, artifact, declaration_kind, version, configuration: Option<String>, subproject: Option<String> }`, `struct SbtSubproject { name, project_dir: PathBuf, build_sbt_path: Option<PathBuf>, declared_in_root: bool }`, `struct SbtMainModule { subproject, organization: Option<String>, name_setting: Option<String>, version_setting: Option<String>, scala_version: Option<String> }`, `enum ScalaVersionSource { BuildSbtExplicit, BuildPropertiesEmbedded, DefaultFallback }` (with `fn to_annotation_value(&self) -> &'static str` returning `"build-sbt-explicit"` / `"build-properties-embedded"` / `"default-fallback"`).

- [X] T004 [P] Add `fn apply_scala_suffix(kind: DeclKind, bare_artifact: &str, scala_version: Option<&str>) -> String` in `mikebom-cli/src/scan_fs/package_db/scala.rs` per data-model §4 algorithm. Rules: `SinglePercent` → bare artifact verbatim; `TriplePercent` → bare artifact verbatim (caller warns-and-skips); `DoublePercent` with `Some("3.x")` → bare + `_3`; `DoublePercent` with `Some("2.13.12")` → bare + `_2.13` (drop patch); `DoublePercent` with `None` → bare + `_2.13` (Q1 default-fallback). Include 4+ unit tests covering each branch.

- [X] T005 [P] Add private helper `fn validate_sbt_lock_shape(json: &serde_json::Value) -> bool` in `mikebom-cli/src/scan_fs/package_db/scala.rs` per Q3 content-shape gate. Returns `true` only when the JSON contains a top-level `lockVersion` (integer) AND `modules` (array) key. Used by `parse_lockfiles` to skip non-SBT-plugin files that happen to match the `*.sbt.lock` glob. Cite Q3 + research §R2 inline.

- [X] T006 [P] Add private helper `fn derive_scala_version(main: &SbtMainModule, build_properties: Option<&str>) -> (Option<String>, ScalaVersionSource)` in `mikebom-cli/src/scan_fs/package_db/scala.rs` per Q1 inference cascade. Resolution order: (1) `main.scala_version.clone()` → `ScalaVersionSource::BuildSbtExplicit`; (2) regex-extract `scala\.version\s*=\s*(\S+)` from `build_properties` string → `ScalaVersionSource::BuildPropertiesEmbedded`; (3) `Some("2.13".to_string())` → `ScalaVersionSource::DefaultFallback`. Return None for the version only when explicitly absent AND the caller has indicated "pure-Java SBT project" (rare); the cascade's rung 3 effectively never returns None.

- [X] T007 [P] Add private regex `OnceLock` helpers in `mikebom-cli/src/scan_fs/package_db/scala.rs` for the parse patterns per research §R4 + §R5: `LIBRARY_DEPENDENCY_SINGLE_RE` (matches `libraryDependencies += "<group>" %{1,3} "<artifact>" % "<version>" [% <Config>]`), `LIBRARY_DEPENDENCY_SEQ_RE` (matches `libraryDependencies ++= Seq(` opener for paren-counted Seq extraction), `SBT_DEP_TUPLE_RE` (matches an individual `"<group>" %{1,3} "<artifact>" % "<version>" [% <Config>]` inside a Seq body), `NAME_SETTING_RE` (matches `name := "<value>"`), `VERSION_SETTING_RE` (matches `version := "<value>"`), `ORGANIZATION_SETTING_RE` (matches `organization := "<value>"`), `SCALA_VERSION_RE` (matches `scalaVersion := "<value>"`), `LAZY_VAL_PROJECT_RE` (matches `lazy val <ident> = project.in(file("<path>"))` per Q2 Surface A), `DEPENDENCIES_VAL_RE` (matches `val <ident> = "<group>" %{1,3} "<artifact>" % "<version>"` per research §R7 best-effort Dependencies.scala extraction). Each regex stored in a private `static REGEX_NAME: OnceLock<Regex> = OnceLock::new();` slot at MODULE scope (NOT inside loops — hoist per research §R8 + milestone-141 R7 lesson). Expose via `fn library_dependency_single_re() -> &'static Regex` accessor functions.

- [X] T008 [P] Add private paren-counted tokenizer `fn tokenize_sbt_seq_body(seq_body: &str) -> Vec<String>` in `mikebom-cli/src/scan_fs/package_db/scala.rs` that splits a `Seq(...)` body at top-level commas while respecting nested `()` and `[]` brackets AND quoted strings. Returns one raw entry-string per dep tuple. Mirror the shape of `elixir.rs::tokenize_mix_lock` + `erlang.rs::tokenize_rebar_lock` per research §R4; include the `// Shape mirrors elixir.rs::tokenize_mix_lock + erlang.rs::tokenize_rebar_lock; factor to shared module when a 4th DSL-extracted ecosystem needs it` comment.

---

## Phase 3: User Story 1 — Operator scans an SBT-managed Scala project with `*.sbt.lock` (P1) 🎯 MVP

**Goal** (SC-001): A scan of a synthetic SBT project (3 direct deps + 4 transitives = 7 lockfile entries) produces a CDX SBOM with 7 `pkg:maven/*` components.

**Independent Test**: `cargo test -p mikebom --test scala_sbt_baseline` passes.

- [X] T009 [US1] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, implement `fn discover_sbt_locks(rootfs: &Path, exclude_set: &ExclusionSet) -> Vec<PathBuf>` + `fn discover_build_sbts(...)` + `fn discover_build_properties(...)` + `fn discover_dependencies_scala(...)` using `crate::scan_fs::walk::safe_walk` (per research §R10). Filter by file name AND directory context: `discover_sbt_locks` matches `*.sbt.lock`; `discover_build_sbts` matches `build.sbt`; `discover_build_properties` matches `project/build.properties` (parent dir name must be `project`); `discover_dependencies_scala` matches `project/Dependencies.scala`. Standard excludes: `target/`, `project/target/`, `_build/`, `.git/`, `node_modules/`. Each helper returns sorted PathBuf vec for deterministic emission.

- [X] T010 [US1] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, implement `fn parse_lockfiles(paths: &[PathBuf]) -> Vec<(PathBuf, SbtLockEntry)>` handling schema v1 + v2 per data-model §3.1 + §2.1. For each path: read file → parse via `serde_json::from_str::<Value>` → validate via `validate_sbt_lock_shape` from T005 (warn-and-skip if false) → iterate `modules[]` array → extract `org` / `name` / `version` / `configurations` / optional `checksums[]`. For schema-v2 `checksums`, pick the first entry where `type` (case-insensitive) equals `"SHA-256"` and extract `checksum`. Returns flat Vec of (lockfile_path, entry) tuples for downstream emission. Warn-and-skip on per-file parse errors per FR-007.

- [X] T011 [US1] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, implement `fn build_lockfile_component(lockfile_path: &Path, entry: &SbtLockEntry) -> anyhow::Result<PackageDbEntry>` per data-model §3.1. Construct PURL `pkg:maven/<entry.org>/<entry.name>@<entry.version>` (entry.name already contains the Scala-version suffix from the plugin). Emit `hashes` from `entry.sha256` via `ContentHash::with_algorithm(HashAlgorithm::Sha256, &sha)` when present. Populate `extra_annotations`: `mikebom:source-type = "scala-sbt-lock"`, `mikebom:evidence-kind = "sbt-lock"`. Set `lifecycle_scope` to `Some(LifecycleScope::Development)` when any `configurations[]` entry equals `"test"` (case-insensitive); else `Some(LifecycleScope::Runtime)` per FR-008. Other PackageDbEntry fields default-initialized.

- [X] T012 [US1] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, wire up the `read()` entry-point's US1 path: discover lockfiles (T009), parse them (T010), emit lockfile components (T011) with `seen_purls: HashSet<String>` dedup. Per-file parse errors warn-and-skip (FR-007). Return the accumulated `Vec<PackageDbEntry>`.

- [X] T013 [US1] [P] Create `mikebom-cli/tests/scala_sbt_baseline.rs` with synthetic-fixture test `sc001_baseline_seven_components`: construct `tempdir()` with `build.sbt` declaring 3 direct deps + `build.sbt.lock` (schema v2) pinning those 3 plus 4 transitives (= 7 modules each with a 64-hex-char deterministic SHA-256 like `"a".repeat(64)`). Invoke the mikebom binary via `Command::new(env!("CARGO_BIN_EXE_mikebom"))` with `--offline sbom scan --path <tempdir> --no-deep-hash --format cyclonedx-json --output cyclonedx-json=<out>`. Parse the emitted CDX JSON. Assert SC-001: exactly 7 `pkg:maven/*` components emit with correct PURLs (`pkg:maven/org.typelevel/cats-core_2.13@2.10.0` etc.) + correct hash entries. Add `sc001_inner_sha256_hash_emitted` test asserting `hashes` array on one component. Add `sc001_source_type_annotation` test asserting `mikebom:source-type = "scala-sbt-lock"` on a sample component. Test module uses `#[cfg_attr(test, allow(clippy::unwrap_used))]` per project convention.

- [X] T014 [US1] In `mikebom-cli/src/scan_fs/package_db/mod.rs`, integrate `scala::read(...)` into the `read_all` dispatcher (place alphabetically after the `rpm*` family calls and before `swift::read(...)` — first reader starting with 's'). Pass the same `(rootfs, include_dev, exclude_set)` triple the existing readers receive; extend the returned `Vec<PackageDbEntry>` with the new entries. **Checkpoint**: After T014, `cargo test -p mikebom --test scala_sbt_baseline` MUST pass (US1 independently complete).

---

## Phase 4: User Story 2 — Operator distinguishes Scala 2 vs Scala 3 vs Java + cross-built libs + main-module emission (P2)

**Goal** (SC-002 + SC-008 + SC-010): A scan distinguishes `%` (pure-Java, no suffix) vs `%%` Scala-2.13 (`_2.13`) vs `%%` Scala-3 (bare `_3`); cross-built libraries emit as TWO distinct components; main-modules emit per FR-012.

**Independent Test**: `cargo test -p mikebom --test scala_source_discriminators` passes.

- [X] T015 [US2] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, implement `fn parse_build_sbt(path: &Path) -> anyhow::Result<(SbtMainModule, Vec<DeclaredSbtDep>)>` per data-model §2.2 + §2.4 + research §R4. Use regexes from T007 + tokenizer from T008. Extract main-module settings (`name`, `version`, `organization`, `scalaVersion`). Extract `libraryDependencies +=` single-add entries via `LIBRARY_DEPENDENCY_SINGLE_RE`. Extract `libraryDependencies ++= Seq(...)` multi-add entries by matching the opener via `LIBRARY_DEPENDENCY_SEQ_RE`, finding the matching closing paren via paren-counting (mirror `tokenize_rebar_lock`'s logic), tokenizing the body via T008, and applying `SBT_DEP_TUPLE_RE` to each tokenized entry. Detect `DeclKind` from the percent-count of the operator (`%` = SinglePercent, `%%` = DoublePercent, `%%%` = TriplePercent). Detect configuration scope from the trailing `% Test` / `% Provided` etc. suffix. `%%%` declarations warn-and-skip per Out-of-Scope (do not emit a `DeclaredSbtDep`).

- [X] T016 [US2] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, implement `fn build_main_module_component(subproj: &SbtSubproject, main: &SbtMainModule, build_properties_text: Option<&str>, doc_has_lockfile: bool) -> anyhow::Result<PackageDbEntry>` per data-model §3.3 + FR-012. Compute organization fallback (`"unknown"`), name fallback (parent-dir basename → `"unknown"`), version fallback (`"0.0.0-unknown"`). Derive Scala-version via `derive_scala_version` from T006 (capture both the version AND the `ScalaVersionSource` enum value). Apply `apply_scala_suffix(DoublePercent, &name, scala_version.as_deref())` from T004 to compute the artifactId-with-suffix. Construct PURL `pkg:maven/<organization>/<artifactid>@<version>`. Populate `extra_annotations`: `mikebom:component-role = "main-module"`, `mikebom:source-type = "scala-main-module"`. **Per F6 remediation**: additionally emit `mikebom:scala-version-source = <ScalaVersionSource.to_annotation_value()>` on the main-module (matches the design-tier `%%` deps' transparency convention from T021; closes the operator-debugging gap where main-module's `_2.13` suffix origin would otherwise be invisible). `sbom_tier`: `"source"` when `doc_has_lockfile == true` else `"design"`.

- [X] T017 [US2] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, implement `fn parse_dependencies_scala(path: &Path) -> Vec<DeclaredSbtDep>` per research §R7 best-effort extraction. Use `DEPENDENCIES_VAL_RE` from T007 to match `val <ident> = "<group>" %{1,3} "<artifact>" % "<version>"` patterns; emit one `DeclaredSbtDep` per match with `subproject = None` (sidecar applies globally). Computed forms (`def foo(v: String) = ...`) silently drop per the best-effort posture.

- [X] T018 [US2] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, extend `read()` to: (a) parse all `build.sbt` paths via `parse_build_sbt` from T015; (b) for the root project + each discovered subproject directory, construct an `SbtSubproject` instance (Q2 union — full implementation lands in T025; for US2 emit ONLY the root-project main-module). Emit `build_main_module_component` from T016 with `doc_has_lockfile` set based on whether `lock_data` is non-empty. Dedup via `seen_purls`.

- [X] T019 [US2] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, verify that cross-built deps in `*.sbt.lock` (e.g., `cats-core_2.13` + `cats-core_3` both pinned) emit as distinct components — this should be automatic via the standard `seen_purls: HashSet<String>` dedup since the PURLs differ in the `name` slot. Add a unit test `cross_built_purls_are_distinct` asserting `build_lockfile_component(<entry for cats-core_2.13>).purl != build_lockfile_component(<entry for cats-core_3>).purl`.

- [X] T020 [US2] [P] Create `mikebom-cli/tests/scala_source_discriminators.rs` with synthetic-fixture tests covering SC-002 + SC-008 + SC-010: `sc002_scala_2_13_lockfile_purl` (asserts `pkg:maven/org.typelevel/cats-core_2.13@2.10.0`), `sc002_scala_3_lockfile_purl` (asserts `pkg:maven/org.typelevel/cats-core_3@2.10.0` — bare `_3`), `sc002_pure_java_lockfile_purl` (asserts `pkg:maven/org.postgresql/postgresql@42.7.0` — no suffix), `sc010_cross_built_distinct` (fixture lockfile contains both `cats-core_2.13` AND `cats-core_3`; assert TWO components emit), `sc008_main_module_emission` (fixture with `build.sbt` setting `name := "my_app"`, `version := "1.2.3"`, `organization := "com.example"`, `scalaVersion := "2.13.12"` + sibling lockfile; assert main-module emits with PURL `pkg:maven/com.example/my_app_2.13@1.2.3` + `mikebom:component-role = "main-module"`). **Checkpoint**: After T020, US2 independently complete.

---

## Phase 5: User Story 3 — Operator scans an SBT project without lockfile + Q1 cascade + Q2 multi-project (P3)

**Goal** (SC-003 + SC-007 + SC-009): `build.sbt`-only projects emit design-tier components with Q1 cascade applied to `%%` deps; `Test`-config deps tag as dev-scope and suppress under `--exclude-scope dev`; multi-project SBT builds emit one main-module per subproject per Q2 union discovery.

**Independent Test**: `cargo test -p mikebom --test scala_tier_fallbacks` passes.

- [X] T021 [US3] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, implement `fn build_design_tier_component(dep: &DeclaredSbtDep, subproject_scala_version: Option<&str>, scala_version_source: ScalaVersionSource, build_sbt_path: &Path) -> Option<PackageDbEntry>` per data-model §3.2 + FR-005. For `TriplePercent` declarations, return `None` (warn-and-skip per Out-of-Scope). For `SinglePercent` / `DoublePercent`: apply `apply_scala_suffix` from T004; construct PURL `pkg:maven/<group>/<artifact-with-suffix>@<sanitized-version>` (use `sanitize_purl_version` helper from milestones 140/141). Populate `extra_annotations`: `mikebom:source-type = "scala-sbt-design"`, `mikebom:evidence-kind = "sbt-build"`, `mikebom:requirement-range = <dep.version>`. For `DoublePercent` ONLY, additionally emit `mikebom:scala-version-source = <scala_version_source.to_annotation_value()>` per Q1. `sbom_tier = Some("design")`. Configuration → lifecycle-scope per FR-008 (`Some("Test")` → Development; else Runtime).

- [X] T022 [US3] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, extend `read()` to: (a) detect when a `build.sbt` lacks a sibling `*.sbt.lock` (use canonicalized parent-directory comparison); (b) when no lockfile present, iterate the parsed `DeclaredSbtDep` set for that subproject; (c) emit each via `build_design_tier_component` from T021 with the per-subproject `scala_version_source` from `derive_scala_version` (T006). Dedup via `seen_purls`. Per the milestone-141 F1 analysis, design-tier emission only fires when no lockfile is present at the SAME directory.

- [X] T023 [US3] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, implement `fn discover_subprojects(build_sbt_paths: &[PathBuf]) -> Vec<SbtSubproject>` per Q2 union discovery + research §R5. Phase A: parse each root-level `build.sbt` (the one whose parent has the most build artifacts — typically the scan root) for `LAZY_VAL_PROJECT_RE` matches; emit one `SbtSubproject { name: <lazy-val-ident>, project_dir: <root>/<file-path>, build_sbt_path: <root>/<file-path>/build.sbt if exists, declared_in_root: true }` per match. Phase B: for each `build_sbt_path` whose parent directory is NOT already represented in the Phase A set (canonicalized-path dedup), emit one `SbtSubproject { name: <parent-dir basename>, project_dir: <parent>, build_sbt_path: Some(<path>), declared_in_root: false }`. Phase C: emit the root project itself as an additional `SbtSubproject` (the implicit root). Returns the union deduplicated by canonicalized `project_dir`.

- [X] T024 [US3] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, extend `read()` to use `discover_subprojects` from T023: emit one main-module per surfaced subproject (via T016's `build_main_module_component`), plus per-subproject design-tier emission when the subproject's directory has no sibling lockfile (via T022's flow but scoped per-subproject). Each subproject's `DeclaredSbtDep` list contributes to its own main-module's `depends` per FR-009. Same-PURL components across subprojects collapse via standard `seen_purls` dedup. **Per F3 remediation**: before invoking `derive_scala_version` (T006) for each subproject, the caller MUST read the subproject's `project/build.properties` file via `std::fs::read_to_string(subproject_dir.join("project").join("build.properties")).ok()` (returns `Option<String>`) and pass the result as the second argument so the Q1 cascade's rung 2 (build-properties-embedded `scala.version=`) can resolve correctly. Missing-file is the dominant case and surfaces as `None`, advancing the cascade to rung 3 (default-fallback).

- [X] T025 [US3] In `mikebom-cli/src/scan_fs/package_db/scala.rs`, when `project/Dependencies.scala` is discovered alongside any `build.sbt`, parse it via `parse_dependencies_scala` from T017 and merge the resulting `DeclaredSbtDep` list into the surrounding subproject's dep set. The Scala-version cascade applies identically to Dependencies.scala-derived deps per research §R7. Emit via the same `build_design_tier_component` path from T021 when design-tier mode applies.

- [X] T026 [US3] [P] Create `mikebom-cli/tests/scala_tier_fallbacks.rs` with synthetic-fixture tests covering SC-003 + SC-007 + SC-009 + Q1: `sc003_design_tier_from_build_sbt_only` (fixture with `build.sbt` + `scalaVersion := "2.13.12"` declaring 2 `%%` deps + NO lockfile; assert 2 components emit with `mikebom:sbom-tier = "design"` + `mikebom:scala-version-source = "build-sbt-explicit"`), `q1_default_fallback_when_scalaversion_absent` (fixture without `scalaVersion`; assert `%%` dep emits with `_2.13` suffix + `mikebom:scala-version-source = "default-fallback"`), `sc007_test_config_dev_scope` (fixture has `"org.scalatest" %% "scalatest" % "3.2.18" % Test`; assert CDX `scope` field equals `"excluded"` per milestone-052 bridge; assert `--exclude-scope dev` (top-level flag BEFORE sbom subcommand) suppresses scalatest), `sc009_multi_project_three_subprojects` (fixture with root `build.sbt` declaring 3 `lazy val ... = project.in(file("<path>"))` blocks; assert 3 main-modules + 1 root main-module = 4 total per FR-009; assert same-PURL deps across subprojects collapse). **Checkpoint**: After T026, US3 independently complete.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Edge-case coverage, CDX builder extension, docs, pre-PR gate.

- [X] T027 [P] Create `mikebom-cli/tests/scala_edge_cases.rs` with: `sc004_no_op_on_non_scala_tree` (covers FR-006 + SC-004 — fixture has no `build.sbt` / `*.sbt.lock` / `project/`; assert ZERO `mikebom:source-type` values starting with `"scala-"` AND ZERO `scala:` / `sbt:` warnings in stderr), `sc005_malformed_lockfile_falls_back_to_design_tier` (covers FR-007 + Q3 content-shape gate — corrupted `build.sbt.lock` + valid sibling `build.sbt`; assert scan exits 0, warns about parse failure, AND emits design-tier components from build.sbt), `q3_content_shape_validation_skips_non_sbt_files` (fixture with a file named `something.sbt.lock` containing valid JSON but no `lockVersion` / `modules` keys; assert ZERO components emit AND warn-and-skip diagnostic appears), `main_module_version_fallback_to_0_0_0_unknown` (covers FR-012 fallback — `build.sbt` without `version :=`; assert main-module PURL contains `@0.0.0-unknown`), `main_module_name_fallback_to_dir_basename` (covers FR-012 fallback — `build.sbt` without `name :=`; assert main-module PURL contains the parent-dir basename), `triple_percent_warn_and_skip` (covers Out-of-Scope `%%%` — assert `%%%`-declared dep does NOT emit as a component AND a warning appears in stderr). Test module uses `#[cfg_attr(test, allow(clippy::unwrap_used))]`.

- [X] T028 In `mikebom-cli/src/generate/cyclonedx/builder.rs`, extend the `mikebom:evidence-kind` allowlist enum (per the milestone-141 + earlier-reader precedent — search for the `"rebar-lock"` / `"rebar-config"` / `"app-src"` entries) to include `"sbt-lock"` and `"sbt-build"`. Per the F4 empirical lesson from milestone 141: builder's allowlist hard-rejects unknown values via `debug_assert!` — without this extension, the US1 baseline test will panic at scan time. The `mikebom:source-type` value-set is NOT hardcoded in the builder (verified during milestone 141); no extension needed there.

- [X] T029 Update `docs/reference/sbom-format-mapping.md` per Constitution Principle V + research §R3 — add a new row "Milestone 142 (Scala/SBT)" in Section I documenting the parity-bridge annotation introduced by this milestone: `mikebom:scala-version-source` (with justification clause: "no native CDX `evidence` / `scope` field carries 'this artifactId suffix was inferred via heuristic cascade'; SPDX 2.3 `Annotation` and SPDX 3 `Annotation` carry general comments but have no heuristic-derived-attribute axis; the annotation is a durable parity-bridge specific to the Scala-version-inference cascade from Q1"). Cross-reference the milestone-141 `mikebom:erlang-app-dep-kind` precedent for the doc shape.

- [X] T030 Run `./scripts/pre-pr.sh` and confirm both `cargo +stable clippy --workspace --all-targets -- -D warnings` AND `cargo +stable test --workspace` pass clean (zero warnings + every suite `ok. N passed; 0 failed`). Per Constitution mandatory pre-PR gate. Capture the full output (not greppped) per memory `feedback_prepr_gate_full_output.md`. If clippy flags `unwrap_used` inside any new test module, guard with `#[cfg_attr(test, allow(clippy::unwrap_used))]` per project convention. If clippy flags `regex_creation_in_loops`, hoist the affected `OnceLock<Regex>` to function/module scope per the milestone-141 R7 lesson + research §R8. **Per F2 remediation**: as a final FR-010 enforcement step, run `grep -E 'reqwest\|tokio::net\|hyper::Client\|ureq\|isahc' mikebom-cli/src/scan_fs/package_db/scala.rs` and confirm zero matches — closes the "no network calls during scan" verification gap. This matches the milestone-141 F3 finding's recommended pattern.

---

## Dependencies

```text
Phase 1 (Setup: T001 → T002)
    ↓
Phase 2 (Foundational: T003 → T004 ‖ T005 ‖ T006 ‖ T007 ‖ T008 — types first, then parallel helpers)
    ↓
Phase 3 (US1 P1 MVP: T009 → T010 → T011 → T012 → T013 ‖ T014)
    ↓
Phase 4 (US2 P2: T015 → T016 → T017 → T018 → T019 → T020)
    ↓
Phase 5 (US3 P3: T021 → T022 → T023 → T024 → T025 → T026)
    ↓
Phase 6 (Polish: T027 ‖ T028 ‖ T029 → T030)
```

**Notes**:
- US2 depends on US1 because T015/T018 extend the `read()` flow established in T012.
- US3 depends on US2 because T024 extends T018's per-subproject loop with the Q2 union-discovery output from T023.
- T013, T014, T020, T026, T027 are integration tests that can technically parallelize across user stories, but milestone-scoped execution should follow the dependency order so each test asserts the cumulative reader state at its phase.
- T028 (builder.rs allowlist extension) is critical-path for T013 — without it the US1 test panics at scan time per the F4 empirical lesson from milestone 141. Hoist T028 to run BEFORE T013 if running tests interactively; the tasks.md order keeps it in Polish phase because both can land in the same PR and the test order during CI runs is alphabetical.

## Parallel Execution Examples

**Phase 2 (Foundational)**: T003 must complete first (types feed every other helper). T004–T008 (5 tasks) touch the same `scala.rs` file but add INDEPENDENT helpers — sequential commits, parallelizable for review/drafting.

**Phase 6 (Polish)**: T027 (new test file) ‖ T028 (builder.rs) ‖ T029 (docs) are touchable in parallel — different files entirely. T030 (pre-PR gate) MUST run last after every code change.

## Implementation Strategy

**MVP scope**: Phases 1-3 (T001-T014, 14 tasks) — closes the headline "SBT with sbt-dependency-lock plugin lockfile" case. ~14 tasks, ~600 LOC including the test fixture.

**Incremental delivery** (after MVP merge):
- Phase 4 (T015-T020) adds source-discriminator richness + main-module emission.
- Phase 5 (T021-T026) adds design-tier fallback + Q1 cascade + Q2 multi-project + Q3 content-shape.
- Phase 6 (T027-T030) tightens edges + docs + pre-PR.

**Single-PR delivery** (recommended, matches milestones 137-141 convention): Ship Phases 1-6 in one PR. Branch is already `142-scala-sbt-reader`; one PR per milestone keeps the changelog clean.

## Format Validation

All 30 tasks above follow the required format: `- [ ] T<NNN> [P?] [Story?] <description with file path>`. Checkbox + ID + optional `[P]` marker + optional `[US1]`/`[US2]`/`[US3]` story label (story label REQUIRED for Phase 3-5 tasks, ABSENT from Phase 1-2 + Phase 6 tasks) + clear file path in every description. Verified.
