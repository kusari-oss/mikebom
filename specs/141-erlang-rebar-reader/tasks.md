---

description: "Task list for milestone 141 — Erlang/OTP rebar ecosystem reader"
---

# Tasks: Erlang/OTP rebar ecosystem reader

**Input**: Design documents from `/specs/141-erlang-rebar-reader/`
**Prerequisites**: plan.md ✓, spec.md ✓ (with Q1+Q2+Q3 clarifications), research.md ✓, data-model.md ✓, contracts/erlang-component-purl.md ✓, quickstart.md ✓

**Tests**: Integration tests included — established convention for milestones 064 / 066 / 068 / 069 / 070 / 122 / 135 / 136 / 137 / 138 / 139 / 140. Synthetic-fixture pattern via `tempfile::tempdir()`.

**Organization**: Tasks grouped by user story (US1 = P1 MVP rebar.lock baseline; US2 = P2 source-discriminator distinction + OTP-runtime placeholders + main-module emission; US3 = P3 design-tier fallback + Q3 keyword family + umbrella). Setup + Foundational phases are blocking prerequisites for ALL user stories.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Maps task to user story phase (US1 / US2 / US3)
- Setup / Foundational / Polish phases: no story label

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Module skeleton + mod.rs declaration. No `read_all` integration yet — that lands in T014 once the parse pipeline is wired up.

- [X] T001 Create `mikebom-cli/src/scan_fs/package_db/erlang.rs` with module-level docstring (mirrors elixir.rs preamble: milestone reference 141, FR list, PURL shape summary, Q1+Q2+Q3 clarifications recap, research §R1+R3 references), `use` block (`anyhow`, `serde_json::{self, json}`, `tracing::{warn, debug}`, `std::collections::{BTreeMap, HashSet, HashMap}`, `std::path::{Path, PathBuf}`, `std::sync::OnceLock`, `regex::Regex`, `mikebom_common::types::purl::Purl`, `mikebom_common::types::hash::{ContentHash, HashAlgorithm}`, `mikebom_common::resolution::LifecycleScope`, the existing `PackageDbEntry` from `super`, `ExclusionSet` from `super::exclude_path`), and `pub fn read(rootfs: &Path, _include_dev: bool, exclude_set: &ExclusionSet) -> Vec<PackageDbEntry>` stub returning `Vec::new()`.

- [X] T002 Add `pub mod erlang;` declaration to `mikebom-cli/src/scan_fs/package_db/mod.rs` (placed alphabetically — after `pub mod elixir;` and before the next `pub mod`). No `read_all` integration yet — that lands in T014.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared helpers used by all user stories — PURL constructors, regex compile-once helpers, brace-counted tokenizer, OTP stdlib allowlist, AppDepKind precedence rule.

- [X] T003 Add `OTP_STDLIB_ALLOWLIST: &'static [&'static str]` const in `mikebom-cli/src/scan_fs/package_db/erlang.rs` containing the spec-Assumptions list (`kernel`, `stdlib`, `crypto`, `ssl`, `inets`, `mnesia`, `runtime_tools`, `sasl`, `os_mon`, `tools`, `compiler`, `syntax_tools`, `xmerl`, `public_key`, `asn1`, `ftp`, `tftp`, `eldap`, `observer`, `wx`). Document inline that the list is informational only per Q1 — non-membership does NOT suppress emission; it only suppresses the `mikebom:otp-stdlib = "true"` annotation.

- [X] T004 [P] Add private helper `fn build_hex_purl(name: &str, version: &str, repo: Option<&str>) -> anyhow::Result<Purl>` in `mikebom-cli/src/scan_fs/package_db/erlang.rs`. Lowercase the name per purl-spec canonical form. When `repo == Some("hexpm:<org>")` or `Some("<org>")` (bare org without prefix), emit `pkg:hex/<org>/<lc-name>@<version>?repository_url=https://repo.hex.pm`. When `repo == None` or `Some("hexpm")`, emit `pkg:hex/<lc-name>@<version>`. Cite research §R1 inline.

- [X] T005 [P] Add private helper `fn build_git_purl(name: &str, resolved_ref: &str, url: &str) -> anyhow::Result<Purl>` in `mikebom-cli/src/scan_fs/package_db/erlang.rs` emitting `pkg:generic/<name>@<resolved_ref>?vcs_url=git+<url>`. Cite research §R1 + the milestone-140 git-source convention inline.

- [X] T006 [P] Add private brace-counted tokenizer `fn tokenize_rebar_lock(body: &str) -> Vec<String>` in `mikebom-cli/src/scan_fs/package_db/erlang.rs` that splits the rebar.lock pinned-deps list at top-level commas while respecting nested `{}` and `[]` braces AND quoted strings (`"..."` and `<<"...">>` binary-string atoms). Returns one raw entry-string per pinned-dep tuple. Mirror the shape of `elixir.rs::tokenize_mix_lock` (per research §R4 reuse decision); include the `// Shape mirrors elixir.rs::tokenize_mix_lock; factor to shared module when a 3rd ecosystem needs it` comment.

- [X] T007 [P] Add private regex `OnceLock` helpers in `mikebom-cli/src/scan_fs/package_db/erlang.rs` for the parse patterns: `LOCK_TOP_REGEX` (matches outer `{"<version>", [<inner-list>]}.` shape), `LOCK_ENTRY_HEX_MODERN_REGEX` (matches modern `{<<name>>, {pkg, <<name>>, <<version>>[, <<sha>>]}, depth}` shape), `LOCK_ENTRY_HEX_MAP_REGEX` (matches map-form private-org `{<<name>>, {pkg, ..., #{repo => <<"hexpm:org">>}}, depth}` shape), `LOCK_ENTRY_HEX_LEGACY_REGEX` (matches flat `{<<name>>, <<version>>, depth}` shape), `LOCK_ENTRY_GIT_REGEX` (matches `{<<name>>, {git, "url", {ref|tag|branch, "value"}}, depth}` shape), `CONFIG_DEPS_BLOCK_REGEX` (matches `{deps, [...]}` top-level block via brace-counting), `CONFIG_PROFILES_BLOCK_REGEX` (matches `{profiles, [{<env>, [...]}]}` blocks), `APPSRC_APPLICATION_REGEX` (matches `{application, <atom>, [` outer wrapper), `APPSRC_VSN_REGEX` (matches `{vsn, "<text>"}`), `APPSRC_APPLICATIONS_REGEX`, `APPSRC_INCLUDED_REGEX`, `APPSRC_OPTIONAL_REGEX` (each matches the keyword + extracts the atom list per data-model §1.3). Each regex stored in a private `static REGEX_NAME: OnceLock<Regex> = OnceLock::new();` slot; expose via a `fn lock_top_regex() -> &'static Regex` accessor pattern matching the milestone-140 elixir.rs convention.

- [X] T008 [P] Add private enums `enum LockEntry { HexModern { ... }, HexLegacy { ... }, Git { ... } }`, `enum DeclaredDepSource { Hex, Git, Path }`, `struct DeclaredDep { ... }`, `struct AppSrcManifest { ... }`, `enum AppDepKind { Required, Included, Optional, BuildOnly }` (with `fn precedence(&self) -> u8` returning 3/2/1/0 for ordering) in `mikebom-cli/src/scan_fs/package_db/erlang.rs` per data-model §2. Each carries the fields documented in data-model.md; include `#[derive(Debug, Clone, PartialEq, Eq)]` and unit-test-relevant `Default` where applicable.

---

## Phase 3: User Story 1 — Operator scans a rebar3-managed Erlang/OTP project (P1) 🎯 MVP

**Goal** (SC-001): A scan of a synthetic rebar3 project (3 direct deps + 2 transitives = 5 lockfile entries) produces a CDX SBOM with 5 `pkg:hex/*` components.

**Independent Test**: `cargo test -p mikebom --test erlang_rebar_baseline` passes.

- [X] T009 [US1] In `mikebom-cli/src/scan_fs/package_db/erlang.rs`, implement `fn discover_rebar_locks(rootfs: &Path, exclude_set: &ExclusionSet) -> Vec<PathBuf>` + `fn discover_rebar_configs(...)` + `fn discover_app_src_files(...)` using `crate::scan_fs::walk::safe_walk` (per research §R9). Filter by file name (`rebar.lock`, `rebar.config`) and by extension (`*.app.src`). Each helper returns sorted PathBuf vec for deterministic emission.

- [X] T010 [US1] In `mikebom-cli/src/scan_fs/package_db/erlang.rs`, implement `fn parse_rebar_lock(path: &Path) -> anyhow::Result<Vec<LockEntry>>` handling the MODERN-HEX shape ONLY for this US1 phase (the other shapes land in US2 per T015). Read the file, regex-extract the outer `{"<version>", [<inner-list>]}.` body, tokenize via T006, dispatch each entry-string against `LOCK_ENTRY_HEX_MODERN_REGEX` from T007. Extract name (lowercased), version, optional inner SHA-256. Non-matching entries warn-and-skip with `tracing::warn!` carrying the file path.

- [X] T011 [US1] In `mikebom-cli/src/scan_fs/package_db/erlang.rs`, implement `fn lock_entry_to_pdb_entry(entry: &LockEntry) -> anyhow::Result<PackageDbEntry>` for the MODERN-HEX variant per data-model §3.1. Construct PURL via `build_hex_purl` from T004. Emit hashes vec with `ContentHash::with_algorithm(HashAlgorithm::Sha256, sha)` when `inner_sha256.is_some()`. Populate `extra_annotations` with `mikebom:source-type = "erlang-hex"` + `mikebom:evidence-kind = "rebar-lock"`. Other PackageDbEntry fields default-initialized.

- [X] T012 [US1] In `mikebom-cli/src/scan_fs/package_db/erlang.rs`, wire up the `read()` entry-point's US1 path: discover rebar.lock files (T009), parse each (T010), emit components (T011). Return the accumulated `Vec<PackageDbEntry>`. Per-file parse errors warn-and-skip (FR-007).

- [X] T013 [US1] [P] Create `mikebom-cli/tests/erlang_rebar_baseline.rs` with synthetic-fixture test `test_us1_basic_hex_deps_emit`: construct `tempdir()` with `rebar.config` declaring `[cowboy, jiffy, lager]` (3 deps) + `rebar.lock` pinning those 3 plus `ranch` and `cowlib` as 2 transitives (= 5 modern-hex entries each with a real-looking SHA-256). Invoke the mikebom binary via `Command::new(env!("CARGO_BIN_EXE_mikebom"))` with `sbom scan --path <tempdir> --output <out>`. Parse the emitted CDX JSON. Assert SC-001: exactly 5 `pkg:hex/*` components emit with correct PURLs (`pkg:hex/cowboy@2.10.0` etc.) + correct hash entries. Include 60-second-timeout pattern from milestone-101 Windows-smoke precedent if applicable (otherwise plain `Command::output()`). Test module uses `#[cfg_attr(test, allow(clippy::unwrap_used))]` per project convention.

- [X] T014 [US1] In `mikebom-cli/src/scan_fs/package_db/mod.rs`, integrate `erlang::read(...)` into the `read_all` dispatcher (place alphabetically after the `elixir::read(...)` call). Pass the same `(rootfs, include_dev, exclude_set)` triple the existing readers receive; extend the returned `Vec<PackageDbEntry>` with the new entries. **Checkpoint**: After T014, `cargo test -p mikebom --test erlang_rebar_baseline` MUST pass (US1 independently complete).

---

## Phase 4: User Story 2 — Operator distinguishes Hex vs git vs OTP runtime + main-module emission (P2)

**Goal** (SC-002 + SC-008): A scan of a fixture mixing one Hex + one git dep + one `*.app.src` declaring OTP-runtime libs produces correct PURLs per FR-003 with correct `mikebom:source-type` annotations; main-module emits per FR-012.

**Independent Test**: `cargo test -p mikebom --test erlang_source_discriminators` passes.

- [X] T015 [US2] Extend `parse_rebar_lock` in `mikebom-cli/src/scan_fs/package_db/erlang.rs` to handle the LEGACY-HEX shape (`{<<name>>, <<version>>, depth}` flat form) + the MAP-FORM private-org shape (`{<<name>>, {pkg, <<name>>, <<version>>, <<sha>>, #{repo => <<"hexpm:org">>}}, depth}`) + the GIT shape (`{<<name>>, {git, "url", {ref|tag|branch, "value"}}, depth}`). Dispatch via try-each-regex-in-order against each entry-string from T006; preserve the original declared-ref-form ("ref"/"tag"/"branch") as `Git.declared_ref_form`. Add a unit test `test_parse_rebar_lock_all_shapes` covering one entry per shape.

- [X] T016 [US2] Extend `lock_entry_to_pdb_entry` in `mikebom-cli/src/scan_fs/package_db/erlang.rs` to handle the HEX-LEGACY variant (no inner SHA-256 → empty hashes vec) and the GIT variant per data-model §3.2 (construct PURL via `build_git_purl` from T005, populate `mikebom:source-type = "erlang-git"` + `mikebom:vcs-declared-ref` evidence). Private-org map-form HEX-MODERN variants route through `build_hex_purl` from T004 with the extracted `repo` argument.

- [X] T017 [US2] In `mikebom-cli/src/scan_fs/package_db/erlang.rs`, implement `fn parse_app_src(path: &Path) -> anyhow::Result<AppSrcManifest>` per data-model §1.3 + §2.3. Use `APPSRC_APPLICATION_REGEX` (extract app-name atom from `{application, <atom>, [`), `APPSRC_VSN_REGEX` (extract version from `{vsn, "..."}`), `APPSRC_APPLICATIONS_REGEX` (extract atom-list from `{applications, [...]}`). Fall back per FR-012 when patterns don't match: app-name → parent-directory basename (call site of `parse_app_src` already has the path); version → `"0.0.0-unknown"`. `included_apps` and `optional_apps` populated by T024 in Phase 5 — leave them empty for this task (the helper signature is forward-compatible).

- [X] T018 [US2] In `mikebom-cli/src/scan_fs/package_db/erlang.rs`, implement `fn emit_main_module(manifest: &AppSrcManifest, lock_data: &HashMap<PathBuf, Vec<LockEntry>>, entries: &mut Vec<PackageDbEntry>) -> anyhow::Result<PackageDbEntry>` per data-model §3.4. Build main-module's PURL via `build_hex_purl(manifest.app_name, manifest.version, None)`. Populate the 4 `extra_annotations` from §3.4. Compute `depends` as the union of `manifest.required_apps` (for US2 scope; included + optional unioning lands in T024). Use NAMES not bom-refs (orchestrator-resolved). Return the constructed entry — caller pushes to `entries`.

- [X] T019 [US2] In `mikebom-cli/src/scan_fs/package_db/erlang.rs`, implement `fn emit_otp_runtime_placeholders(manifest: &AppSrcManifest, lock_data: &HashMap<PathBuf, Vec<LockEntry>>, entries: &mut Vec<PackageDbEntry>)` per data-model §3.3. For each atom in `manifest.required_apps` NOT matching a name in `lock_data` (across all parsed lockfiles), emit one placeholder component with PURL `pkg:generic/<atom>@unspecified` + `mikebom:source-type = "erlang-otp-runtime"` + `mikebom:evidence-kind = "app-src"`. When `OTP_STDLIB_ALLOWLIST.contains(&atom.as_str())`, additionally emit `mikebom:otp-stdlib = "true"` per Q1. Dedup via standard `seen_purls: HashSet<String>` so the same OTP atom (e.g., `kernel`) referenced from multiple `*.app.src` files emits ONE placeholder.

- [X] T020 [US2] [P] Create `mikebom-cli/tests/erlang_source_discriminators.rs` with synthetic-fixture tests covering SC-002: `test_us2_hex_git_otp_discrimination` (fixture mixes 1 Hex + 1 git + 1 `*.app.src` with `applications: [kernel, stdlib, cowboy]`; assert correct PURLs per FR-003 + correct `mikebom:source-type` values), `test_us2_private_org_hex` (fixture has rebar.lock entry with `#{repo => <<"hexpm:acme">>}`; assert PURL is `pkg:hex/acme/internal_lib@2.0.0?repository_url=https://repo.hex.pm`), `test_us2_legacy_hex_shape` (fixture uses pre-3.7 `{<<name>>, <<version>>, depth}` flat shape; assert PURL emits + hashes vec is empty), `test_us2_git_ref_forms` (one entry per `{ref, ...}` / `{tag, ...}` / `{branch, ...}` form; assert `mikebom:vcs-declared-ref` evidence reflects each), `test_us2_otp_stdlib_annotation` (assert `kernel` carries `mikebom:otp-stdlib = "true"` and a custom OTP atom does NOT), `test_us2_main_module_emission` (covers SC-008 — assert main-module PURL `pkg:hex/<app>@<vsn>` + `mikebom:component-role = "main-module"` annotation). **Checkpoint**: After T020, US2 independently complete (`cargo test -p mikebom --test erlang_source_discriminators` passes).

---

## Phase 5: User Story 3 — Operator scans without rebar.lock + Q3 keyword family + umbrella (P3)

**Goal** (SC-003 + SC-009 + SC-010): Library projects without `rebar.lock` emit design-tier components from `rebar.config`. Umbrella projects emit one main-module per `*.app.src`. The Q3 keyword family (`applications:` + `included_applications:` + `optional_applications:`) unions into main-module `depends` with correct `mikebom:erlang-app-dep-kind` annotations.

**Independent Test**: `cargo test -p mikebom --test erlang_tier_fallbacks` passes.

- [X] T021 [US3] In `mikebom-cli/src/scan_fs/package_db/erlang.rs`, implement `fn parse_rebar_config(path: &Path) -> anyhow::Result<Vec<DeclaredDep>>` per data-model §1.2 + §2.2. Use `CONFIG_DEPS_BLOCK_REGEX` to extract the top-level `{deps, [...]}` body, parse each `{<atom>, <version>}` or `{<atom>, {pkg, <atom>, <version>}}` or `{<atom>, {git, "url", {ref|tag|branch, "..."}}}` tuple via brace-counted iteration over the body, populate `DeclaredDep` with `profile = None` for top-level deps. Then use `CONFIG_PROFILES_BLOCK_REGEX` to find each `{profiles, [{<env>, [...]}]}` block; for each `<env>` block extract its inner `{deps, [...]}` (recursive structure) and populate `DeclaredDep` with `profile = Some(env_name)`. Warn-and-skip on malformed entries.

- [X] T022 [US3] In `mikebom-cli/src/scan_fs/package_db/erlang.rs`, implement `fn design_tier_entry(dep: &DeclaredDep) -> anyhow::Result<PackageDbEntry>` per FR-005 + data-model §3 (design-tier branch). For Hex source, PURL `pkg:hex/<name>@<url-encoded-constraint>`. Populate `extra_annotations` with `mikebom:sbom-tier = "design"`, `mikebom:requirement-range = <raw_constraint_string>`, `mikebom:evidence-kind = "rebar-config"`, `mikebom:source-type = "erlang-hex"` (or `"erlang-git"` for git-source declared deps). When `dep.profile == Some("dev"|"test"|"doc")`, additionally emit `mikebom:lifecycle-scope = "development"` per FR-008. URL-encode constraint strings (e.g., `~> 2.10` → `~>%202.10`) using the `url` crate's percent-encoding helpers (already a workspace dep per milestone 075).

- [X] T023 [US3] In `mikebom-cli/src/scan_fs/package_db/erlang.rs`, extend `read()` to emit design-tier entries when `rebar.lock` is absent but sibling `rebar.config` is present. Add helper `fn has_sibling_lockfile(config_path: &Path, lock_data: &HashMap<PathBuf, Vec<LockEntry>>) -> bool` that checks for a `rebar.lock` in the same directory as the config. When false, dispatch each `DeclaredDep` through `design_tier_entry` from T022 and push to entries. When true, skip design-tier emission (lockfile is authoritative).

- [X] T024 [US3] Extend `parse_app_src` in `mikebom-cli/src/scan_fs/package_db/erlang.rs` to populate `manifest.included_apps` via `APPSRC_INCLUDED_REGEX` and `manifest.optional_apps` via `APPSRC_OPTIONAL_REGEX` per data-model §1.3 + Q3. Both keywords are OPTIONAL — absence is not a parse error (a valid OTP application descriptor may omit both). Then extend `emit_main_module` from T018 to: (a) union `required_apps ∪ included_apps ∪ optional_apps ∪ nearby_rebar_config.deps` into the `depends` set per Q2+Q3; (b) for each dep-atom, determine its `AppDepKind` (per the precedence rule from T008 — `Required` > `Included` > `Optional` > `BuildOnly`); (c) thread the kind into the corresponding edge-target component's `extra_annotations` as `mikebom:erlang-app-dep-kind = "required" | "included" | "optional"` (no annotation for `BuildOnly` per data-model §3.5). For OTP-runtime placeholders (T019), also update `emit_otp_runtime_placeholders` to source from `required_apps ∪ included_apps ∪ optional_apps` (not just required) so all three keyword families produce placeholders for non-lockfile atoms.

- [X] T025 [US3] In `mikebom-cli/src/scan_fs/package_db/erlang.rs`, add umbrella support — per FR-009, multiple `*.app.src` files under `apps/<sub_app>/src/<sub_app>.app.src` each produce their own main-module. The existing T012/T024 loop over `app_src_data` already handles this naturally (one main-module per file). Add `fn find_nearest_rebar_config(app_src_path: &Path, config_data: &HashMap<PathBuf, Vec<DeclaredDep>>) -> Option<&Vec<DeclaredDep>>` helper that walks up from `app_src_path` looking for a sibling-or-ancestor `rebar.config` (matches umbrella project layout — root `rebar.config` shared across `apps/*/`). The nearest config's deps contribute to the main-module's `depends` union per T024 step (a).

- [X] T026 [US3] [P] Create `mikebom-cli/tests/erlang_tier_fallbacks.rs` with synthetic-fixture tests covering SC-003 + SC-009 + SC-010: `test_us3_design_tier_from_rebar_config` (fixture has rebar.config only — 2 direct deps; assert components emit with `mikebom:sbom-tier = "design"` + `mikebom:requirement-range` evidence), `test_us3_profile_scoped_dev_dep` (fixture has `{profiles, [{test, [{deps, [{meck, "~> 0.9"}]}]}]}`; assert `meck` component carries `mikebom:lifecycle-scope = "development"`; also test that running with top-level `--exclude-scope dev` BEFORE the `sbom scan` subcommand suppresses meck), `test_us3_umbrella_three_subapps` (fixture with `apps/{my_app,my_lib,my_worker}/src/<name>.app.src`; assert 3 main-module components + same-PURL deps across sub-apps collapse to 1 component via dedup), `test_us3_keyword_family_union` (covers SC-010 — fixture's `*.app.src` declares applications/included/optional all populated; assert FIVE main-module dep-edge targets emit; assert correct `mikebom:erlang-app-dep-kind` values; assert filtering on `mikebom:erlang-app-dep-kind == "optional"` retrieves exactly `telemetry`). **Checkpoint**: After T026, US3 independently complete.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Edge-case coverage, CDX builder + metadata.rs propagation extensions, docs, pre-PR gate.

- [X] T027 [P] Create `mikebom-cli/tests/erlang_edge_cases.rs` with: `test_malformed_lockfile_falls_back_to_design_tier` (covers SC-005 + FR-007), `test_binary_string_atom_encoding_in_rebar_lock` (asserts `<<"cowboy">>` and bare-atom-form parse equivalently), `test_legacy_hex_shape_no_hash` (covers contract §9), `test_main_module_version_fallback_to_0_0_0_unknown` (covers contract §6), `test_main_module_appname_fallback_to_dir_basename` (covers contract §7 + FR-012 cascade), `test_no_op_on_non_erlang_tree` (covers SC-004 + FR-006 — fixture has no rebar/app-src files; assert zero erlang-derived components + zero erlang-related warnings), `test_optional_applications_absent_keyword_is_not_error` (covers research §R3 OTP-version-compatibility note — OTP-25-and-earlier `*.app.src` without `optional_applications:` parses cleanly with empty `optional_apps`). Test module uses `#[cfg_attr(test, allow(clippy::unwrap_used))]`.

- [X] T028 In `mikebom-cli/src/generate/cyclonedx/builder.rs`, extend the `mikebom:evidence-kind` allowlist enum (per the milestone-140 + earlier-reader precedent — search for the `"hex-metadata"` / `"mix-lock"` entries) to include `"rebar-lock"`, `"rebar-config"`, `"app-src"`. Also extend the `mikebom:source-type` allowlist (if curated) with `"erlang-hex"`, `"erlang-git"`, `"erlang-otp-runtime"`, `"erlang-main-module"`. If `mikebom:erlang-app-dep-kind`, `mikebom:otp-stdlib`, `mikebom:vcs-declared-ref` need explicit allowlisting in the builder's property-emission curation, add them too — verify by examining how milestone-140's `mikebom:elixir-extraction-mode` was wired.

- [X] T029 In `mikebom-cli/src/generate/cyclonedx/metadata.rs`, IF a main-module is promoted to `metadata.component` (per the milestone-127 smarter-root-pick logic), ensure the curated property-propagation list passes through `mikebom:erlang-app-dep-kind`, `mikebom:source-type`, `mikebom:component-role`, `mikebom:sbom-tier`. Mirror the milestone-140 `mikebom:umbrella-root` propagation pattern (the file's curated allowlist is a `match`-style explicit list — extend, don't reorganize). Add a regression test in `mikebom-cli/tests/erlang_tier_fallbacks.rs` (or alongside the umbrella test in T026) that asserts the propagation: when a single-rebar3-project (1 main-module + N deps) is scanned, the emitted CDX `metadata.component.properties[]` includes the main-module's `mikebom:source-type = "erlang-main-module"`.

- [X] T030 Update `docs/reference/sbom-format-mapping.md` per Constitution Principle V + research §R6 — add a new section "Milestone 141 (Erlang/OTP)" documenting the parity-bridge annotations introduced by this milestone: `mikebom:erlang-app-dep-kind` (with the justification clause: "OTP `applications:` / `included_applications:` / `optional_applications:` discrimination is a runtime-startup-behavior axis orthogonal to the CDX `scope` / SPDX 2.3 `*_DEPENDENCY_OF` / SPDX 3 `LifecycleScopeType` lifecycle-scope axis; no standards-native carrier exists"), `mikebom:otp-stdlib` (informational allowlist marker per Q1), `mikebom:vcs-declared-ref` (ref/tag/branch original declaration preserved as evidence). Cross-reference the milestone-140 `mikebom:elixir-extraction-mode` precedent for the doc shape.

- [X] T031 Run `./scripts/pre-pr.sh` and confirm both `cargo +stable clippy --workspace --all-targets -- -D warnings` AND `cargo +stable test --workspace` pass clean (zero warnings + every suite `ok. N passed; 0 failed`). Per Constitution mandatory pre-PR gate. Capture the full output (not greppped) per memory `feedback_prepr_gate_full_output.md`. If clippy flags `unwrap_used` inside any new test module, guard with `#[cfg_attr(test, allow(clippy::unwrap_used))]` per project convention.

---

## Dependencies

```text
Phase 1 (Setup: T001 → T002)
    ↓
Phase 2 (Foundational: T003 ‖ T004 ‖ T005 ‖ T006 ‖ T007 ‖ T008 — all parallel)
    ↓
Phase 3 (US1 P1 MVP: T009 → T010 → T011 → T012 → T013 ‖ T014)
    ↓
Phase 4 (US2 P2: T015 → T016 → T017 → T018 → T019 → T020)
    ↓
Phase 5 (US3 P3: T021 → T022 → T023 → T024 → T025 → T026)
    ↓
Phase 6 (Polish: T027 ‖ T028 ‖ T029 → T030 → T031)
```

**Notes**:
- US2 depends on US1 because T015/T016 extend the same `parse_rebar_lock` / `lock_entry_to_pdb_entry` functions established in T010/T011, and T018 extends `read()` integration from T012.
- US3 depends on US2 because T024 extends `parse_app_src` from T017 + `emit_main_module` from T018 + `emit_otp_runtime_placeholders` from T019.
- T013, T014, T020, T026, T027 are integration tests that can technically parallelize across user stories, but milestone-scoped execution should follow the dependency order so each test asserts the cumulative reader state at its phase.

## Parallel Execution Examples

**Phase 2 (Foundational)**: All 6 tasks (T003-T008) touch the same `erlang.rs` file but add INDEPENDENT helpers (const, fn, regex, types) — when working sequentially this is fine; when reviewing in parallel, conflicts are limited to file-end appends. Mark them `[P]` for review parallelization but expect sequential commits.

**Phase 6 (Polish)**: T027 (new test file) ‖ T028 (builder.rs) ‖ T029 (metadata.rs) are touchable in parallel — different files entirely. T030 (docs) is parallel with all of T027-T029. T031 (pre-PR gate) MUST run last after every code change.

## Implementation Strategy

**MVP scope**: Phases 1-3 (T001-T014) — closes the headline "rebar3 with modern lockfile" case. ~14 tasks, ~600 LOC including the test fixture.

**Incremental delivery** (after MVP merge):
- Phase 4 (T015-T020) adds source-discriminator richness + main-module emission.
- Phase 5 (T021-T026) adds design-tier fallback + Q3 keyword family + umbrella.
- Phase 6 (T027-T031) tightens edges + docs + pre-PR.

**Single-PR delivery** (recommended, matches milestones 137/138/139/140 convention): Ship Phases 1-6 in one PR. Branch is already `141-erlang-rebar-reader`; one PR per milestone keeps the changelog clean.

## Format Validation

All 31 tasks above follow the required format: `- [ ] T<NNN> [P?] [Story?] <description with file path>`. Checkbox + ID + optional `[P]` marker + optional `[US1]`/`[US2]`/`[US3]` story label (story label REQUIRED for Phase 3-5 tasks, ABSENT from Phase 1-2 + Phase 6 tasks) + clear file path in every description. Verified.
