---

description: "Task list for milestone 140 — Elixir/Mix ecosystem reader"
---

# Tasks: Elixir/Mix ecosystem reader

**Input**: Design documents from `/specs/140-elixir-mix-reader/`
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/elixir-component-purl.md ✓, quickstart.md ✓

**Tests**: Integration tests included — established convention for milestones 064 / 066 / 068 / 069 / 070 / 122 / 135 / 136 / 137 / 138 / 139. Synthetic-fixture pattern via `tempfile::tempdir()`.

**Organization**: Tasks grouped by user story (US1 = P1 MVP; US2 = P2 source-discriminator distinction; US3 = P3 design-tier fallback). Setup + Foundational phases are blocking prerequisites for ALL user stories.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Maps task to user story phase (US1 / US2 / US3)
- Setup / Foundational / Polish phases: no story label

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Module skeleton + cyclonedx evidence-kind enum extension.

- [X] T001 Create `mikebom-cli/src/scan_fs/package_db/elixir.rs` with module-level docstring (mirrors cocoapods.rs preamble: milestone reference, FR list, PURL shape summary, Phase 0 correction notes), `use` block (`anyhow`, `serde_json`, `tracing`, `std::collections::{BTreeMap, HashSet, HashMap}`, `std::path::{Path, PathBuf}`, `std::sync::OnceLock`, `regex::Regex`, `mikebom_common::types::purl::Purl`, `mikebom_common::types::hash::{ContentHash, HashAlgorithm}`, `mikebom_common::resolution::LifecycleScope`, the existing `PackageDbEntry` from `super`, `ExclusionSet` from `super::exclude_path`), and `pub fn read(rootfs: &Path, _include_dev: bool, exclude_set: &ExclusionSet) -> Vec<PackageDbEntry>` stub returning `Vec::new()`.

- [X] T002 Add `pub mod elixir;` declaration to `mikebom-cli/src/scan_fs/package_db/mod.rs` (placed alphabetically — after `pub mod dpkg;` and before the next `pub mod`). No `read_all` integration yet — that lands in T012.

- [X] T002b Extend the cyclonedx evidence-kind allowlist in `mikebom-cli/src/generate/cyclonedx/builder.rs` to accept `"mix-lock"` and `"mix-exs"`. Append the two new values per the milestone-135 / 136 / 137 / 138 / 139 T002b pattern. Without this, T015 + T021 would panic in debug builds.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Reader-private types + parsing + PURL helpers + dispatcher integration. MUST complete before ANY user story phase.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T003 Define reader-private types in `mikebom-cli/src/scan_fs/package_db/elixir.rs` per `data-model.md`: `LockEntry` enum with `Hex { name, version, inner_sha256, managers, repo, outer_sha256 }`, `Git { name, url, resolved_sha, declared_ref }`, `Path { name, path, in_umbrella }` variants. Plus `MixExsInfo { app_name, version, is_umbrella, deps }` struct + inner `DeclaredDep { name, constraint, dev_scope, in_conditional, source_kind }` where `source_kind: DeclaredDepSource` is an enum with `Hex` / `Git { url, declared_ref }` / `Path { path, in_umbrella }` variants per C2 remediation (drives T018 design-tier PURL dispatch). `#[allow(dead_code)]` on each.

- [X] T004 Implement `fn find_mix_locks(rootfs: &Path, exclude_set: &ExclusionSet) -> Vec<PathBuf>` in `elixir.rs` walking via `scan_fs::walk::safe_walk` returning every `mix.lock` file. Skip descent into `.git`, `.svn`, `.hg`, `_build`, `deps`, `node_modules`, `priv`, `cover` (standard Elixir/Phoenix build-output dirs). Output lex-sorted.

- [X] T005 Implement `fn find_mix_exs(rootfs: &Path, exclude_set: &ExclusionSet) -> Vec<PathBuf>` in `elixir.rs` walking via `safe_walk` returning every `mix.exs` file. Same skip-set as T004. Output lex-sorted. Used for: (a) main-module name extraction in lockfile mode (T016), (b) design-tier fallback in Pass B (T021/T022).

- [X] T006 Implement `fn tokenize_mix_lock(text: &str) -> Vec<(String, String)>` in `elixir.rs` that splits the lockfile map literal into per-entry `(name, tuple_body)` pairs. Strategy: find the outer `%{...}`, then iterate top-level entries via brace-counting (each entry is `"<name>": {...},` where the value's `{...}` may contain nested `[...]` lists with their own delimiters — track `paren_depth + brace_depth + bracket_depth` to find the matching closing `}` and trailing comma). Names are double-quoted strings; tuple bodies are everything from the first `{` to its matching `}` inclusive.

- [X] T007 Implement `fn parse_lock_entry(name: &str, tuple_body: &str) -> Option<LockEntry>` in `elixir.rs` per data-model.md. Dispatch on the first atom inside the tuple:
  - `{:hex, :<atom_name>, "<version>", "<inner_sha>", [<managers>], [<deps>], "<repo>", "<outer_sha>"}` → `LockEntry::Hex`. Use regex `^\{\s*:hex\s*,\s*:([a-zA-Z_][a-zA-Z0-9_]*)\s*,\s*"([^"]+)"\s*,\s*"([0-9a-f]{64})"\s*,\s*\[([^\]]*)\]\s*,\s*\[(.*)\]\s*,\s*"([^"]+)"\s*(?:,\s*"([0-9a-f]{64})")?\s*\}$` (with `(?s)` flag for multi-line). Outer SHA-256 is optional (group 8 may be `None`).
  - `{:git, "<url>", "<resolved_sha>", [<opts>]}` → `LockEntry::Git`. Use regex `^\{\s*:git\s*,\s*"([^"]+)"\s*,\s*"([0-9a-f]{40})"\s*,\s*\[(.*)\]\s*\}$`. Extract first `(<key>: <value>)` from opts as `declared_ref` (e.g., `ref: "main"` → `"ref: main"`).
  - `{:path, "<path>", [<opts>]}` → `LockEntry::Path`. Use regex `^\{\s*:path\s*,\s*"([^"]+)"\s*,\s*\[(.*)\]\s*\}$`. Detect `in_umbrella: true` in opts.
  - Unknown discriminator → return `None` (caller warns + skips).

- [X] T008 Implement `fn parse_mix_exs(path: &Path) -> Result<MixExsInfo>` in `elixir.rs` per data-model.md + R3. Read file via `std::fs::read_to_string`. Use four `OnceLock<Regex>` patterns:
  - `app:` (project name): `(?m)^\s*app:\s*:([a-zA-Z_][a-zA-Z0-9_]*)\b` — first-match wins
  - `version:`: `(?m)^\s*version:\s*"([^"]+)"` — first-match wins
  - `apps_path:` (umbrella sentinel): `(?m)^\s*apps_path:\s*` — KEY-presence detection per R4 (value ignored)
  - Dep tuple: `\{\s*:([a-zA-Z_][a-zA-Z0-9_]*)\s*,\s*("[^"]+")?(?:,\s*([^}]*))?\}` — capture name, optional first-string constraint, optional opts blob

  For `deps[]` extraction: iterate line-by-line, tracking `in_conditional: bool` via simple keyword counting (`if Mix.env()`, `unless `, `case `, `do` increment; `end` decrement). When a dep tuple regex matches, set `DeclaredDep.in_conditional` to the current stack state. Detect `dev_scope` by inspecting the opts blob for `only:` (followed by `:dev`, `:test`, or list containing them) or `runtime: false`. Per C2 remediation, also populate `DeclaredDep.source_kind` via opts-blob inspection: when opts contain `git: "<url>"` → `DeclaredDepSource::Git { url, declared_ref }`; when `github: "<owner>/<repo>"` → expand to `Git { url: format!("https://github.com/{owner}/{repo}.git"), declared_ref }` per research R3; when `path: "<path>"` → `DeclaredDepSource::Path { path, in_umbrella }` (with `in_umbrella` set when `in_umbrella: true` is also in opts); otherwise → `DeclaredDepSource::Hex` (default).

- [X] T009 Implement `fn build_purl_for_lock_entry(entry: &LockEntry) -> Result<Purl, String>` in `elixir.rs` per FR-003 + contracts/elixir-component-purl.md:
  - `LockEntry::Hex` with `repo == "hexpm"` → `pkg:hex/<lc_name>@<version>`.
  - `LockEntry::Hex` with `repo == "hexpm:<org>"` → split on first colon; emit `pkg:hex/<org>/<lc_name>@<version>?repository_url=https://repo.hex.pm` per Phase 0 correction.
  - `LockEntry::Git` → `pkg:generic/<name>@<resolved_sha>?vcs_url=git+<url>` per Phase 0 correction (purl-spec doesn't bless `vcs_url=` for hex).
  - `LockEntry::Path` → `pkg:generic/<name>@unspecified` (path-source has no version; placeholder).
  - On error (PURL construction failure) → `Err(...)`.

- [X] T010 Implement `fn classify_source_type(entry: &LockEntry) -> &'static str` in `elixir.rs`. Always `"hex-hex"` for Hex variant (private-org distinction surfaces via PURL namespace + `repository_url=`, not source-type); `"hex-git"` for Git; `"hex-path"` for Path.

- [X] T011 Implement `fn build_extra_annotations(entry: &LockEntry, source_type_value: &str) -> BTreeMap<String, serde_json::Value>` in `elixir.rs` per data-model.md per-source-type fields. Always sets `mikebom:source-type`. Source-specific:
  - Git: `mikebom:vcs-declared-ref = "<declared_ref>"` when `LockEntry::Git.declared_ref` is `Some`.
  - Path: `mikebom:path = "<path>"`; plus `mikebom:in-umbrella = "true"` when `LockEntry::Path.in_umbrella` is true.

- [X] T012 Wire `elixir::read(rootfs, include_dev, exclude_set)` into `read_all` in `mikebom-cli/src/scan_fs/package_db/mod.rs`. Place the call alphabetically — after the `dpkg::read(...)` call. Mirror the cocoapods/composer pattern (`out.extend(...)`). NO `collect_claimed_paths` integration — language readers don't claim binary paths.

**Checkpoint**: Foundation ready — `elixir::read` callable from dispatcher, returns empty Vec, cyclonedx gate accepts new evidence-kind values. US1/US2/US3 phases can proceed.

---

## Phase 3: User Story 1 — Phoenix/Nerves baseline (Priority: P1) 🎯 MVP

**Goal**: Lockfile-driven SBOM emission for canonical Phoenix/Nerves projects — one main-module per `mix.exs` + one component per `mix.lock` entry + dep edges from main-module to direct deps + inner+outer SHA-256 hashes per Q3.

**Independent Test (SC-001)**: Synthetic fixture with `mix.exs` (3 direct deps) + `mix.lock` (5 entries — 3 direct + 2 transitives). Scan produces exactly 5 `pkg:hex/*` lockfile-derived components + 1 main-module + main-module's `depends` lists the 3 direct deps by name.

### Implementation for User Story 1

- [X] T013 [US1] Implement `fn emit_main_module(project_dir: &Path, mix_exs_path: Option<&Path>, lockfile_path: Option<&Path>, info: Option<&MixExsInfo>, doc_has_lockfile: bool, declared_dep_names_for_depends: &[String]) -> Option<PackageDbEntry>` in `elixir.rs` per FR-012. App-name derivation cascade per the milestone-139 Q1 pattern: `info.and_then(|i| i.app_name.clone())` else `project_dir.file_name().and_then(|s| s.to_str()).map(String::from)`. Version from `info.and_then(|i| i.version.clone()).unwrap_or_else(|| "0.0.0-unknown".into())`. PURL: `pkg:hex/<lc_app_name>@<version>`. Annotations: `mikebom:component-role = "main-module"` + `mikebom:source-type = "hex-main-module"` + (when `info.is_umbrella`) `mikebom:umbrella-root = "true"`. `depends` populated from `declared_dep_names_for_depends`. `sbom_tier = Some("source")` when `doc_has_lockfile`; else `Some("design")`. `evidence_kind = Some("mix-exs")`.

- [X] T014 [US1] Implement `fn emit_lockfile_components(lockfile_path: &Path, entries: &[LockEntry], mix_exs_info: Option<&MixExsInfo>) -> Vec<PackageDbEntry>` in `elixir.rs` per FR-002 + FR-003 + FR-008 + FR-011. For each `LockEntry`:
  - Call `build_purl_for_lock_entry` (T009) — on `Err`, `tracing::warn!` with name + path and `continue`.
  - Call `classify_source_type` (T010) → `source_type_value`.
  - Call `build_extra_annotations` (T011) → `extra_annotations`.
  - Build `hashes` per FR-011 + Q3: for `LockEntry::Hex` only — push `ContentHash::with_algorithm(HashAlgorithm::Sha256, inner_sha256)` always; push outer only when `outer_sha256` is `Some` AND non-empty. Validate 64-char lowercase hex; skip individual hash on failure.
  - Cross-reference scope per FR-008: if `mix_exs_info` is `Some` AND its `DeclaredDep` matching the entry's name has `dev_scope == true`, set `lifecycle_scope = Some(LifecycleScope::Development)`; else `Some(LifecycleScope::Runtime)`.
  - Construct `PackageDbEntry` per data-model.md common-fields table.

- [X] T015 [US1] Implement the `elixir::read` orchestrator body — Pass A (lockfile walker) per R7. Maintain `seen_purls: HashSet<String>` for orchestrator dedup. Track `lockfile_dirs: HashSet<PathBuf>` (parent dirs of each parsed `mix.lock`) for use by Pass B's design-tier skip-logic. For each `mix.lock`:
  1. Read text via `std::fs::read_to_string`. On error → warn + skip.
  2. Tokenize via T006 → `Vec<(name, tuple_body)>`. On parse error → `tracing::warn!`; check for sibling `mix.exs`; if present → fall back to design-tier emission per Pass B (orchestrator wiring in T019; emitter T018).
  3. Parse each entry via T007 → `Vec<LockEntry>`. Skip malformed individual entries with debug log.
  4. Look for sibling `mix.exs` → parse via T008 → `Option<MixExsInfo>`.
  5. Mark project dir in `lockfile_dirs`.
  6. Emit main-module via T013 with `doc_has_lockfile = true` + `declared_dep_names_for_depends` populated from `info.deps[*].name` (or empty Vec if no mix.exs).
  7. Emit lockfile components via T014; dedupe through `seen_purls`.

- [X] T016 [US1] Write integration test file `mikebom-cli/tests/elixir_phoenix_baseline.rs` with `#[test]` functions:
  - `phoenix_baseline_emits_lock_count_plus_main_module` — SC-001: 3 direct + 2 transitives = 5 lockfile + 1 main-module = 6 hex-derived components.
  - `main_module_from_mix_exs_app_keyword` — SC-008: `mix.exs` `app: :my_app, version: "0.5.2"` → main-module PURL `pkg:hex/my_app@0.5.2`.
  - `main_module_depends_lists_direct_deps` — SC-008 cont: dependencies[] for main-module bom-ref targets each `deps/0` entry's bom-ref.
  - `inner_and_outer_sha256_emitted` — SC-009 + Q3: hex entry with both inner + outer SHA-256 produces TWO CDX `hashes[]` entries with `alg = SHA-256` and distinct contents.
  - `pre_hex_2_entry_emits_only_inner_sha256` — Q3 edge: lockfile entry with only inner SHA-256 (no outer / empty outer) emits ONE hash entry per Principle IX accuracy.
  - `dev_scope_filterability` — SC-007: lockfile entry whose `mix.exs::deps/0` has `only: [:dev, :test]` carries `mikebom:lifecycle-scope = "development"`; `--exclude-scope dev` suppresses.

  Use `tempfile::tempdir()` + `env!("CARGO_BIN_EXE_mikebom")`. Splice `--exclude-scope dev` BEFORE `sbom scan` subcommand per the milestone-137/138/139 pattern. Guard `.unwrap()` with `#[cfg_attr(test, allow(clippy::unwrap_used))]`.

**Checkpoint**: US1 (Phoenix baseline + dual SHA-256 + dev-scope) fully functional. SC-001/007/008/009 pass independently.

---

## Phase 4: User Story 2 — Source discriminators + private Hex orgs (Priority: P2)

**Goal**: Surface trunk vs git vs path vs private-org distinction so downstream tooling can correctly classify. Validate Phase 0 corrections (private-org namespace form + git `pkg:generic/` placeholder).

**Independent Test (SC-002)**: Synthetic fixture with one each of `:hex` (default), `:hex` (private org), `:git`, `:path` in `mix.lock`. Scan. Assert correct PURL shape per FR-003.

### Implementation for User Story 2

US2's helpers (`build_purl_for_lock_entry` + `classify_source_type` + `build_extra_annotations`) already exist after Foundational. This phase adds end-to-end correctness validation.

- [X] T017 [US2] Write integration test file `mikebom-cli/tests/elixir_source_discriminators.rs` with `#[test]` functions covering SC-002 + Phase 0 corrections:
  - `default_hexpm_emits_bare_purl` — `pkg:hex/jason@1.4.4` with `mikebom:source-type = "hex-hex"`.
  - `private_hexpm_org_emits_namespace_and_repository_url` — Phase 0 correction regression: lockfile entry with `"hexpm:acme"` repo string emits `pkg:hex/acme/internal_lib@2.0.0?repository_url=https://repo.hex.pm`. NOT a `mikebom:hex-repo` annotation.
  - `git_source_emits_pkg_generic_with_vcs_url` — Phase 0 correction regression: `:git` entry emits `pkg:generic/my_fork@<sha>?vcs_url=git+https://github.com/foo/my-fork.git` (NOT `pkg:hex/.../?vcs_url=`). With `mikebom:source-type = "hex-git"`.
  - `git_source_carries_vcs_declared_ref` — git entry with `[ref: "main"]` opts carries `mikebom:vcs-declared-ref = "ref: main"` annotation.
  - `path_source_emits_pkg_generic_placeholder` — `:path` entry emits `pkg:generic/shared_lib@unspecified` with `mikebom:source-type = "hex-path"` + `mikebom:path = "<path>"`.
  - `path_in_umbrella_carries_in_umbrella_annotation` — `:path` entry with `[in_umbrella: true]` opts carries `mikebom:in-umbrella = "true"` annotation.
  - `hex_name_lowercased_in_purl` — purl-spec canonical-form regression: lockfile entry with mixed-case name (rare since Hex.pm enforces lowercase, but verify the rule) → PURL lowercased.

**Checkpoint**: US1 + US2 functional. All 4 source discriminators correctly surfaced; private-org Phase 0 correction validated.

---

## Phase 5: User Story 3 — Design-tier + conditional-flattened + umbrella (Priority: P3)

**Goal**: Design-tier emission for library projects (no `mix.lock`); Q1 conditional-flattened extraction with precision-loss annotation; Q2 umbrella root aggregation.

**Independent Test (SC-003 + Q1 + Q2 + SC-010)**: Three sub-fixtures: (1) `mix.exs` only → design-tier with constraints; (2) `mix.exs` with `if Mix.env() == :test do ... end` → conditional-flattened annotation; (3) umbrella project with 3 sub-apps → 4 main-modules (root + 3 sub-apps) with root `depends` listing each sub-app.

### Implementation for User Story 3

- [X] T018 [US3] Implement `fn emit_design_tier_components(mix_exs_path: &Path, info: &MixExsInfo) -> Vec<PackageDbEntry>` in `elixir.rs` per FR-005 + Q1 + C2 remediation. For each `DeclaredDep` in `info.deps`:
  - Skip if dep name equals `info.app_name` (defensive).
  - Constraint: `decl.constraint.clone().unwrap_or_else(|| "unspecified".to_string())`. Sanitize for PURL safety via local helper `sanitize_purl_version` (mirror milestone-138/139's helper — replace `/`, `?`, `#`, ` ` with `_`).
  - **Per C2 remediation**: dispatch on `decl.source_kind` for PURL construction matching FR-003:
    - `DeclaredDepSource::Hex` → `pkg:hex/<lc_name>@<sanitized>`; `source_type = "hex-hex"`.
    - `DeclaredDepSource::Git { url, declared_ref }` → `pkg:generic/<name>@unspecified?vcs_url=git+<url>` (no resolved SHA in design-tier — that's the lockfile's role; use `unspecified` as the version segment); `source_type = "hex-git"`; add `mikebom:vcs-declared-ref = "<ref>"` annotation when `declared_ref` is `Some`.
    - `DeclaredDepSource::Path { path, in_umbrella }` → `pkg:generic/<name>@unspecified`; `source_type = "hex-path"`; add `mikebom:path = "<path>"` annotation; plus `mikebom:in-umbrella = "true"` when `in_umbrella`.
  - Construct `PackageDbEntry` with `sbom_tier = Some("design")`, `evidence_kind = Some("mix-exs")`, `requirement_range = Some(decl.constraint.clone().unwrap_or_default())`, dispatched `source_type` per above, `lifecycle_scope = Some(LifecycleScope::Development)` when `decl.dev_scope`, else `Runtime`.
  - `extra_annotations`: dispatched `mikebom:source-type` per above; PLUS source-kind-specific annotations per above; PLUS `mikebom:elixir-extraction-mode = "conditional-flattened"` when `decl.in_conditional` per Q1.

- [X] T019 [US3] Implement Pass B + Pass C (umbrella root aggregation) of the `elixir::read` orchestrator per R7:
  - **Pass B** (design-tier walker): For each `mix.exs` whose project dir is NOT in `lockfile_dirs` (Pass A skip): parse via T008. Emit main-module via T013 with `doc_has_lockfile = false`. Emit design-tier components via T018; dedupe through `seen_purls`.
  - **Pass C** (umbrella root aggregation per Q2): After Pass A + B, iterate the emitted main-modules. For any main-module whose source `MixExsInfo.is_umbrella == true`: enumerate sibling `apps/<sub_app>/mix.exs` files; for each that produced a main-module in Pass A or B, append that sub-app's main-module NAME (e.g., `"core"` from `app: :core` declaration) to the umbrella root's `depends` list — per I1 remediation, `PackageDbEntry.depends` is a list of NAMES (strings), not bom-refs (the orchestrator handles name→bom-ref translation at the dep-edge wiring stage; see milestone-002 dpkg/apk convention). (Since main-modules are emitted into the `out` Vec, this requires a post-pass mutation: find the root by `mikebom:umbrella-root = "true"` annotation, then mutate its `depends`.)

- [X] T020 [US3] Implement helper `fn collect_apps_subdirs(umbrella_root: &Path) -> Vec<PathBuf>` in `elixir.rs` — given an umbrella project root (parent of `apps/`), enumerate `apps/<sub_app>/mix.exs` files. Used by T019 Pass C to find sub-app main-module bom-refs.

- [X] T021 [US3] Write integration test file `mikebom-cli/tests/elixir_tier_fallbacks.rs` with `#[test]` functions:
  - `design_tier_mix_exs_only_emits_constraints` — SC-003: `mix.exs` only → 2 components with `mikebom:sbom-tier = "design"` + `mikebom:requirement-range = "~> 1.7"` preserved.
  - `design_tier_no_constraint_uses_unspecified_placeholder` — `deps/0` entry `{:foo, git: "..."}` (no version) → PURL `pkg:hex/foo@unspecified`.
  - `design_tier_no_transitive_deps` — design-tier scan emits ONLY declared `deps/0` entries; no inferred transitives.
  - `conditional_block_dep_carries_extraction_mode_annotation` — Q1: `if Mix.env() == :test do {:meck, "~> 0.9"} end` → emitted `meck` carries `mikebom:elixir-extraction-mode = "conditional-flattened"`.
  - `unconditional_dep_does_not_carry_extraction_mode_annotation` — Q1 negative: top-level dep tuple does NOT carry the annotation.
  - `umbrella_root_depends_lists_sub_app_main_modules` — Q2 + SC-010 + I1 remediation: umbrella project with 3 sub-apps under `apps/` → 4 main-modules total. Verify via CDX `dependencies[]` block: find the `dependencies[].ref` matching the umbrella root's `bom-ref`, assert its `dependsOn` array contains each sub-app's main-module `bom-ref` (the orchestrator translates names → bom-refs during dep-edge wiring; `PackageDbEntry.depends` itself carries names, NOT bom-refs).
  - `umbrella_root_carries_umbrella_root_annotation` — Q2 cont: umbrella root main-module carries `mikebom:umbrella-root = "true"` annotation; sub-app main-modules do NOT.
  - `design_tier_git_declared_dep_emits_pkg_generic` — C2 remediation: `mix.exs::deps/0` entry `{:my_fork, git: "https://github.com/foo/my-fork.git", branch: "main"}` (no lockfile) emits `pkg:generic/my_fork@unspecified?vcs_url=git+https://github.com/foo/my-fork.git` with `mikebom:source-type = "hex-git"` AND `mikebom:vcs-declared-ref = "branch: main"`. Defends against re-introduction of the `pkg:hex/my_fork@unspecified` (incorrect Hex shape for a git-declared dep).
  - `design_tier_path_declared_dep_emits_pkg_generic` — C2 remediation: `{:shared, path: "../shared"}` (no lockfile) emits `pkg:generic/shared@unspecified` with `mikebom:source-type = "hex-path"` AND `mikebom:path = "../shared"`.
  - `design_tier_github_shortcut_expanded_to_git_url` — C2 remediation + R3: `{:foo, github: "owner/repo"}` emits `pkg:generic/foo@unspecified?vcs_url=git+https://github.com/owner/repo.git` (shortcut expanded at extraction time).

**Checkpoint**: All three user stories functional. Design-tier + conditional-flattened + umbrella aggregation all validated.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Edge-case coverage + invariant validation + pre-PR gate.

- [X] T022 [P] Write integration test file `mikebom-cli/tests/elixir_edge_cases.rs` covering spec Edge Cases + Phase 0 + Q1/Q2/Q3 nuances:
  - `malformed_mix_lock_falls_back_to_design_tier` — SC-005: malformed lockfile + sibling `mix.exs` → design-tier components emit; warning fires.
  - `multi_line_tuple_in_mix_lock_parses_correctly` — multi-line tuple form (e.g., `:deps` list breaks across lines) parses correctly via brace-counting.
  - `github_shortcut_treated_as_git_in_design_tier` — `mix.exs::deps/0` entry `{:foo, github: "owner/repo"}` in design-tier mode emits as if `git: "https://github.com/owner/repo.git"`.
  - `private_org_namespace_lowercased` — private-org `:hex` entry with mixed-case org slug (rare; Hex.pm enforces lowercase orgs too) → PURL emits lowercased org.
  - `unknown_source_type_atom_warns_and_skips` — lockfile entry with unknown discriminator atom (e.g., `:hg`) → warn + skip that single entry; other entries still emit.
  - `apps_path_with_custom_value_still_detected_as_umbrella` — R4 KEY-presence regression: `apps_path: "modules"` (custom value, not `"apps"`) still triggers umbrella detection.
  - `multi_target_mix_exs_extracts_all_deps_flattened` — Q1 regression: `defp deps(:dev), do: [...]` + `defp deps(:prod), do: [...]` multi-clause → all entries flattened with conditional-extraction annotation.
  - `outer_sha256_empty_string_treated_as_absent` — Q3 edge: lockfile entry with empty-string outer SHA-256 (`""`) emits ONE hash (inner only), NOT two with one empty.
  - `pre_elixir_1_4_lockfile_warns_and_skips` — Out-of-Scope: pre-1.4 lockfile (missing `:hex` discriminator) → warn + skip; rare in 2026.

- [X] T023 [P] Verify SC-004 no-Elixir byte-identity invariant by running existing CDX/SPDX 2.3/SPDX 3 regression test suites. Command: `cargo +stable test -p mikebom --test cdx_regression --test spdx_regression --test spdx3_regression`. All passing = baseline preserved.

- [X] T024 Run `./scripts/pre-pr.sh` from repo root per CLAUDE.md MANDATORY pre-PR gate. Fix any clippy warnings (especially `unwrap_used` in test files — guard with `#[cfg_attr(test, allow(clippy::unwrap_used))]`) and failing tests. Re-run until both lanes show `0 errors` / `N passed; 0 failed`.

- [X] T025 Run quickstart.md SC-006 standard-PURL-filter check + cross-format byte-equivalence diff (CDX vs SPDX 2.3 vs SPDX 3) on synthetic Phoenix fixture. The three formats' Hex PURL sets MUST be identical when sorted.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001 → T002 (T002 imports module declared in T001); T002b independent. T002b BLOCKS T014 + T018 (cyclonedx gate would panic in debug builds).
- **Foundational (Phase 2)**: All depend on Phase 1. Within Phase 2: T003 → T006/T007/T008 (parsers need struct definitions); T003 → T009/T010/T011 (helpers need structs); T004 + T005 independent walkers. T012 depends on `elixir::read` stub (T001).
- **User Story phases (3 / 4 / 5)**: ALL depend on Foundational. Within each phase, tasks sequential unless `[P]`.
- **Polish (Phase 6)**: T022 + T023 marked `[P]` — independent files. T024 + T025 depend on all preceding phases.

### User Story Dependencies

- **US1 (P1) MVP**: Depends on Foundational. T013 → T014 → T015 → T016 sequential.
- **US2 (P2)**: Depends on Foundational only (helpers already exist). T017 is pure integration test.
- **US3 (P3)**: Depends on Foundational + T013 (main-module helper shared) + T015 (orchestrator structure). T018 → T019 → T020 → T021 sequential.

### Parallel Opportunities

- **Phase 1**: T001 + T002b parallel; T002 sequential after T001.
- **Phase 2**: T004 + T005 + T006 + T008 can run in parallel (independent functions in same file).
- **Phase 3**: T013–T016 sequential (all touch `elixir.rs`).
- **Phase 4**: T017 standalone.
- **Phase 5**: T018 → T019 → T020 → T021 sequential.
- **Phase 6**: T022 + T023 parallel.

---

## Implementation Strategy

### MVP First (US1)

1. Phase 1: Setup (T001–T002b).
2. Phase 2: Foundational (T003–T012).
3. Phase 3: US1 (T013–T016).
4. **STOP and VALIDATE**: `cargo +stable test -p mikebom --test elixir_phoenix_baseline` — confirm SC-001/007/008/009 pass.
5. Headline use case shippable: Phoenix/Nerves app scan with main-module + lockfile-driven hex components + dual SHA-256 + dev-scope filtering.

### Incremental Delivery

1. Setup + Foundational → dispatcher wired, empty Vec.
2. + US1 → MVP: Phoenix/Nerves baseline + dev-scope + dual SHA-256.
3. + US2 → adds private-org Phase 0 correction validation + git/path discriminator coverage.
4. + US3 → adds library-project design-tier + Q1 conditional-flattened + Q2 umbrella aggregation.
5. + Polish → edge-case coverage + pre-PR gate green.

### Single-PR Pattern

All phases land in ONE PR per established convention:
- Module: `mikebom-cli/src/scan_fs/package_db/elixir.rs` (~1000–1200 LOC incl. unit tests)
- Dispatcher integration: `mikebom-cli/src/scan_fs/package_db/mod.rs` (≤10 LOC)
- Cyclonedx evidence-kind extension: `mikebom-cli/src/generate/cyclonedx/builder.rs` (≤4 LOC)
- Four integration test files: `mikebom-cli/tests/elixir_*.rs` (~900–1200 LOC total)
- Closes #422.

---

## Notes

- `[P]` tasks = different files, no dependencies on incomplete tasks.
- `[Story]` label maps task to specific user story for traceability.
- Each user story independently completable and testable.
- Commit after each logical group.
- **Pre-PR gate (CLAUDE.md MANDATORY)**: `./scripts/pre-pr.sh` MUST pass before opening PR.
- **No-op invariant (SC-004)**: every change MUST preserve byte-identical SBOM output on non-Elixir source trees. T023 is the validation.
- **`--exclude-scope dev` is a TOP-LEVEL mikebom flag** (BEFORE `sbom scan` subcommand) per milestone-137/138/139 convention.
- **Per-PR diff size estimate**: ~2,000–2,500 LOC total (similar to milestone 138/139).
