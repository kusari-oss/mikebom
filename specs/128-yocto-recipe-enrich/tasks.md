# Tasks: Deeper Yocto / OpenEmbedded SBOM coverage

**Input**: Design documents from `/specs/128-yocto-recipe-enrich/`
**Prerequisites**: plan.md, spec.md (with Clarifications), research.md, data-model.md, contracts/reader-behavior.md, contracts/annotation-schema.md, quickstart.md

**Tests**: Included. Every SC requires an integration test; the spec mandates synthesized fixture trees + a balena-clone smoke target via the milestone-090 `MIKEBOM_FIXTURES_DIR` cache.

**Organization**: Tasks grouped by user story (US1 license = P1, US2 src_uri/srcrev = P1, US3 layer-attribution = P2, US4 bbappend = P2, US5 DEPENDS edges = P3). Setup + Foundational phases land the body-parser scaffold (`recipe_body.rs`, `layer_conf.rs`, `bbappend.rs`, `cpe_name_map.rs`), the data-model entity types, and the PURL migration. Each user-story phase is independently testable.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Different files, no dependencies on incomplete tasks — safe to run in parallel.
- **[Story]**: User-story label on Phase 3+ tasks (US1 / US2 / US3 / US4 / US5).

## Path Conventions

Mikebom three-crate workspace (`mikebom-cli/`, `mikebom-common/`, `mikebom-ebpf/`). All changes in `mikebom-cli/` per plan.md "Structure Decision". Integration tests in `mikebom-cli/tests/`. Fixtures in `mikebom-cli/tests/fixtures/yocto_recipe_enrich/`. The three balena clones live in `MIKEBOM_FIXTURES_DIR` (milestone-090 cache), NOT in-tree.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Stand up the new module files + fixture directory before any parser logic lands.

- [ ] T001 Create the four new module files as empty stubs in `mikebom-cli/src/scan_fs/package_db/yocto/`: `recipe_body.rs`, `layer_conf.rs`, `bbappend.rs`, `cpe_name_map.rs`. Wire them into `mikebom-cli/src/scan_fs/package_db/yocto/mod.rs` via `pub(crate) mod recipe_body; pub(crate) mod layer_conf; pub(crate) mod bbappend; pub(crate) mod cpe_name_map;`.
- [ ] T002 [P] Create the fixture directory `mikebom-cli/tests/fixtures/yocto_recipe_enrich/` with subdirectories `single_layer_meta/`, `multi_layer_polyglot/`, `include_chain/`, `git_srcuri_srcrev/`, `autoinc_version/`, `multi_cpe_curl/`, `unexpanded_var/`, `orphan_bbappend/`. Each gets a `.gitkeep` placeholder.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Land the body-parser scaffold, data-model entity types, CPE-name table, PURL migration, and milestone-127 root-selection wiring. Every US phase depends on these.

⚠️ **CRITICAL**: All US-phase tasks depend on these. No US-phase work begins until Phase 2 is complete.

- [ ] T003 [P] In `mikebom-cli/src/scan_fs/package_db/yocto/cpe_name_map.rs`, embed the openembedded-core recipe-to-CPE-product mapping as `pub const CPE_NAME_MAP: &[(&str, &str)]`. Source the entries (~50) from openembedded-core's `meta/conf/distro/include/cve-extra-exclusions.inc` master branch per research R2 — minimum coverage: `linux-kernel` → `linux_kernel`, `nss` → `network_security_services`, `nspr` → `netscape_portable_runtime`, `dropbear` → `dropbear_ssh`, `zstd` → `zstandard`. Add `pub fn cpe_product_for_recipe(recipe_name: &str) -> &str` returning the mapped name or the input unchanged. Lex-sort the table for stable diffs.
- [ ] T004 [P] In `mikebom-cli/src/scan_fs/package_db/yocto/recipe_body.rs`, define the `RecipeMetadata` struct exactly per `data-model.md` Entity 1 (all 16 fields including `unexpanded_vars`, `overrides_merged`, `source_path`, `include_paths`). Derive `Debug, Clone, Default`. Add `pub fn try_canonicalize_license(raw: &str) -> Option<SpdxExpression>` using `mikebom_common::types::license::SpdxExpression::try_canonical`. Replace BitBake-syntax license operators `&` → SPDX `AND`, `|` → `OR` before canonicalization.
- [ ] T005 [P] In `mikebom-cli/src/scan_fs/package_db/yocto/layer_conf.rs`, define the `LayerConf` struct per `data-model.md` Entity 2 (`collection`, `version`, `series_compat`, `source_path`). Implement `pub fn parse(path: &Path) -> Result<Vec<LayerConf>, anyhow::Error>` reading `BBFILE_COLLECTIONS += "..."` / `LAYERVERSION_<collection> = "..."` / `LAYERSERIES_COMPAT_<collection> = "..."` lines. Handle multi-`BBFILE_COLLECTIONS` files (return `Vec`).
- [ ] T006 [P] In `mikebom-cli/src/scan_fs/package_db/yocto/bbappend.rs`, define the `BbAppendIndex` struct per `data-model.md` Entity 3 (`by_recipe`, `orphans`). Add `pub fn build_from_walk(rootfs: &Path, exclude_set: &ExclusionSet) -> Self` using `scan_fs::walk::safe_walk` with depth 8. Parse each `.bbappend` filename into `(name, version-glob)` via regex `^(?P<name>[a-zA-Z0-9_\-\+\.]+)_(?P<version>%|[a-zA-Z0-9_\-\+\.\~]+)\.bbappend$`. Add `pub fn appends_for(&self, name: &str, version: &str) -> Vec<&PathBuf>` matching both exact-version AND `%` wildcard.
- [ ] T007 In `mikebom-cli/src/scan_fs/package_db/yocto/recipe_body.rs`, implement the line-oriented BitBake assignment parser. Recognize: `FIELD = "..."`, `FIELD ?= "..."`, `FIELD ??= "..."`, `FIELD += "..."`, `FIELD =+ "..."`, `FIELD .= "..."`, `FIELD =. "..."` per research R4 grammar. Handle multi-line values via `\` line-continuation. Variable expansion limited to `${PN}` and `${PV}` per FR-005; collect unresolved `${...}` references into a `Vec<String>` returned alongside the parsed values.
- [ ] T008 In `mikebom-cli/src/scan_fs/package_db/yocto/recipe_body.rs`, extend the parser to handle override-syntax: `FIELD:append:<override> = "..."`, `FIELD:prepend:<override> = "..."`, `FIELD:remove:<override> = "..."`. Apply FR-016 merge-as-union: concatenate base + every override into one value (for `:append`/`:prepend`); set `RecipeMetadata.overrides_merged = true` when ≥1 fires. The `:remove:` flavor is also merged-as-union per the FR-016 caveat (no subtraction).
- [ ] T009 In `mikebom-cli/src/scan_fs/package_db/yocto/recipe_body.rs`, implement `require <path>` and `include <path>` resolution. Depth bound 8. Detect cycles via a `BTreeSet<PathBuf>` of in-progress paths; on cycle, drop the tail include with a `tracing::debug!`. Apply FR-004 last-write-in-source-order semantics: process each include FIRST (in include-order), then process the referencing file. The referencing file's assignments override conflicting earlier ones. Implement via a `merge_metadata(into: &mut RecipeMetadata, from: RecipeMetadata)` helper that overwrites fields where `from.<field>.is_some()`.
- [ ] T010 In `mikebom-cli/src/scan_fs/package_db/yocto/recipe.rs`, migrate the PURL emission from `pkg:bitbake/<name>@<version>?layer=<layer-dir-basename>` to `pkg:generic/<name>@<version>?openembedded=true&layer=<collection>` per FR-011. Change the format string at recipe.rs:202 + the bare fallback at recipe.rs:208. Update the test fixture string at recipe.rs:258 (`pkg:bitbake/mikebom-fixture-lib@1.2.3?layer=meta-mikebom-fixture` → `pkg:generic/mikebom-fixture-lib@1.2.3?openembedded=true&layer=meta-mikebom-fixture`). Update the same string at `mikebom-cli/src/scan_fs/package_db/mod.rs:1250` + `mod.rs:1256` (comment + dedup-comment references).
- [ ] T011 In `mikebom-cli/src/scan_fs/package_db/yocto/recipe.rs`, refactor the existing milestone-107 `read()` function to: (a) walk for `.bb` files as today, (b) for each `.bb`, attempt body-parsing via `recipe_body::parse(&bb_path)` returning `Option<RecipeMetadata>`, (c) when body-parsing succeeds, populate the new fields on `PackageDbEntry`; when it fails, fall back to the existing filename-only emission. This preserves milestone-107's behavior for malformed recipes (Constitution Principle VIII).
- [ ] T012 In `mikebom-cli/src/scan_fs/package_db/yocto/recipe.rs`, implement FR-018 version-derivation: when `RecipeMetadata.recipe_version` is literally `"git"` OR contains the BitBake `"AUTOINC"` token, derive the PURL version segment from `RecipeMetadata.srcrev` first 12 hex chars (lowercased). Always also emit `mikebom:srcrev` carrying the FULL SHA. When SRCREV is also absent, skip the component with a `tracing::warn!` log naming the path.
- [ ] T013 In `mikebom-cli/src/scan_fs/package_db/yocto/recipe_body.rs`, add unit tests for: (a) BitBake assignment grammar (every `=` / `+=` / `.=` / `=+` / `=.` / `?=` / `??=` variant), (b) multi-line `\`-continuation, (c) `${PN}` and `${PV}` resolution, (d) unresolved `${BPN}` collection into `unexpanded_vars`, (e) override-syntax `:append`/`:prepend` merge-as-union, (f) `require`/`include` cycle detection, (g) `merge_metadata` last-write-in-source-order semantics, (h) FR-018 version-from-SRCREV derivation for `"git"` AND `"AUTOINC"` inputs.

**Checkpoint**: At end of Phase 2 the body-parser compiles, all unit tests pass, CDX regression remains byte-identical (existing `.bb` filename-only fallback still fires for non-body-parseable recipes), and the PURL migration is wired. NO emitter wiring beyond what milestone-107 already did; the new annotation keys aren't yet populated.

---

## Phase 3: User Story 1 (P1) — Recipe-level license attribution

**Goal**: SC-001 — `mikebom sbom scan --path <meta-balena>` produces an SBOM where ≥80% of recipe-derived components carry a non-empty SPDX `licenseDeclared` expression.

**Independent Test**: `cargo +stable test --workspace --test yocto_recipe_enrich_us1_license` passes. The test scans the `single_layer_meta/` fixture (3 recipes: MIT, dual-license, CLOSED) and asserts the CDX `components[].licenses[]`, SPDX 2.3 `packages[].licenseDeclared`, and SPDX 3 `Package.declaredLicense` all carry the expected canonicalized values.

### Implementation for US1

- [ ] T014 [US1] In `recipe_body.rs`, wire LICENSE extraction: copy the canonicalized `SpdxExpression` from the parsed body into `RecipeMetadata.license` per FR-001. Set `RecipeMetadata.license_closed = true` when the raw LICENSE value is exactly `"CLOSED"`; in that case set `license = None` (emit NOASSERTION downstream). On canonicalization failure, set `license = None` AND record the field in `unexpanded_vars` AND emit a `tracing::warn!` log per US1 AC#4.
- [ ] T015 [US1] In `recipe.rs`, propagate `RecipeMetadata.license` to the emitted `PackageDbEntry.licenses: Vec<SpdxExpression>` so the existing milestone-005 generator pipeline picks it up on CDX `components[].licenses[]` / SPDX 2.3 `packages[].licenseDeclared` / SPDX 3 `Package.declaredLicense` per the Principle V native-first audit in research R1.
- [ ] T016 [US1] In `recipe.rs`, when `RecipeMetadata.license_closed`, emit `mikebom:yocto-license-closed: true` as a per-component annotation (extra_annotations bag). When `unexpanded_vars` contains entries, emit `mikebom:yocto-unexpanded-vars` as a JSON-object annotation per contracts/annotation-schema.md.
- [ ] T017 [P] [US1] Build the `single_layer_meta/` fixture at `mikebom-cli/tests/fixtures/yocto_recipe_enrich/single_layer_meta/`. Layout: one `conf/layer.conf` declaring `BBFILE_COLLECTIONS += "meta-fixture-single"` + `LAYERVERSION_meta-fixture-single = "1"`. Three recipes: `recipe-a_1.0.bb` with `LICENSE = "MIT"`; `recipe-b_2.0.bb` with `LICENSE = "GPL-2.0-only & LGPL-2.1-or-later"`; `recipe-c_3.0.bb` with `LICENSE = "CLOSED"`. All recipes minimal (just LICENSE + SUMMARY fields).
- [ ] T018 [P] [US1] Build the `include_chain/` fixture at `mikebom-cli/tests/fixtures/yocto_recipe_enrich/include_chain/`. Layout: one `conf/layer.conf`. Three files: `foo_1.0.bb` does `require foo.inc` AND sets `LICENSE = "MIT"`; `foo.inc` does `require foo-shared.inc` AND sets `LICENSE = "GPL-2.0-only"`; `foo-shared.inc` sets `LICENSE = "Apache-2.0"`. After last-write-in-source-order semantics fire (FR-004 + Q1), the resulting `recipe_a` license must be `MIT`.
- [ ] T019 [US1] Add the integration test at `mikebom-cli/tests/yocto_recipe_enrich_us1_license.rs`. Test cases: (a) `single_layer_meta/` MIT recipe emits `licenseDeclared = "MIT"` in SPDX 2.3 + `licenses[0].license.id = "MIT"` in CDX + `Package.declaredLicense` in SPDX 3; (b) the dual-license recipe canonicalizes to `"GPL-2.0-only AND LGPL-2.1-or-later"`; (c) the CLOSED recipe has `licenseDeclared = "NOASSERTION"` AND carries `mikebom:yocto-license-closed = true` annotation; (d) `include_chain/` fixture asserts the `.bb`'s `LICENSE = "MIT"` overrides both `.inc`s (last-write-in-source-order). Use `tempfile::tempdir()` + `Command::new(env!("CARGO_BIN_EXE_mikebom"))` pattern from `identifiers_root_purl_control.rs`.

**Checkpoint**: At end of Phase 3, license attribution works end-to-end. SC-001 verified on `single_layer_meta/`; the `meta-balena` smoke test (Phase 11) will hit the ≥80% threshold.

---

## Phase 4: User Story 2 (P1) — Source-pinned upstream provenance

**Goal**: SC-002 — `meta-balena` scan emits ≥60% vcs externalRefs + ≥40% SRCREV annotations on recipe components.

**Independent Test**: `cargo +stable test --workspace --test yocto_recipe_enrich_us2_src_uri` passes. The test scans the `git_srcuri_srcrev/` and `autoinc_version/` fixtures and asserts the CDX/SPDX externalRefs + `mikebom:srcrev` annotation are correctly populated.

### Implementation for US2

- [ ] T020 [US2] In `recipe_body.rs`, wire SRC_URI extraction per FR-002: split SRC_URI on whitespace; normalize `git://` / `git+https://` / `git+ssh://` URIs by extracting the host+path and emitting an `https://` form; preserve `file://` and `http://` / `https://` entries verbatim in `RecipeMetadata.src_uris`. When the FIRST entry is git, set the primary upstream source. When ALL entries are `file://`, set a flag for FR-002 AC#4 (the local-only annotation).
- [ ] T021 [US2] In `recipe_body.rs`, parse `SRCREV` (single value) and `SRCREV_<machine> = "..."` (multi-arch overrides). Multi-arch entries go into `RecipeMetadata.srcrev_by_machine: BTreeMap<String, String>` keyed by arch.
- [ ] T022 [US2] In `recipe.rs`, populate per FR-002: when SRC_URI git, emit `external_references` entry of type `vcs` with the normalized https URL; when SRC_URI tarball (http/https with archive extension), emit type `distribution`; when SRC_URI is all `file://`, emit annotation `mikebom:src-uri-local-only = true` per US2 AC#4. Also emit `mikebom:src-uri` as JSON-encoded array carrying ALL entries verbatim per FR-002 + contracts/annotation-schema.md.
- [ ] T022a [US2] **FR-002a host-typed PURL emission** — in `recipe.rs`, BEFORE calling `build_bitbake_purl`, check if the first `git://` / `git+https://` / `git+ssh://` URI in `RecipeMetadata.src_uris` has a host in `{github.com, gitlab.com, bitbucket.org, codeberg.org}` AND `RecipeMetadata.srcrev` is `Some`. If both hold, parse the URI's path into `(owner, repo)` (strip `.git` suffix from repo) and emit `pkg:<host-token>/<owner>/<repo>@<srcrev-12-hex>` as the primary PURL via `Purl::new`. The 12-hex SRCREV prefix matches FR-018. Add two annotations carrying the recipe-identity provenance: `mikebom:yocto-recipe-name = "<recipe-name>"` and `mikebom:yocto-recipe-version = "<recipe-version>"`. Layer attribution annotations from FR-006 stay unchanged. When host-detection fails or SRCREV is absent, fall through to `build_bitbake_purl` (the existing FR-011 `pkg:generic/...` shape). Unit-test the host-detection regex with all 4 hosts + non-matching shapes.
- [ ] T023 [US2] In `recipe.rs`, emit per FR-003: `mikebom:srcrev` annotation when `RecipeMetadata.srcrev` is Some AND SRC_URI is git; `mikebom:srcrev-by-machine` as JSON object when `srcrev_by_machine` is non-empty.
- [ ] T024 [P] [US2] Build the `git_srcuri_srcrev/` fixture: one recipe `widget_1.0.bb` with `SRC_URI = "git://github.com/example/widget.git;branch=main;protocol=https"`, `SRCREV = "abc123def456abc123def456abc123def456abcd"`, `SRC_URI:append = " file://patch-1.patch"`. Recipe is the FR-002a host-typed-PURL test case (host=github.com → expect `pkg:github/example/widget@abc123def456`).
- [ ] T025 [P] [US2] Build the `autoinc_version/` fixture: one recipe `gadget_0.0.4.AUTOINC+f597fb026637.bb` (literal AUTOINC in filename) with `SRCREV = "f597fb026637abcdef..."`. Also include `gadget-git_git.bb` (literal "git" PV) with `SRCREV = "1234567890abcdef..."`.
- [ ] T026 [US2] Add the integration test at `mikebom-cli/tests/yocto_recipe_enrich_us2_src_uri.rs`. Test cases: (a) `git_srcuri_srcrev/` produces a component whose PURL is `pkg:github/example/widget@abc123def456` per FR-002a (host-typed PURL since SRC_URI host is github.com AND SRCREV is set) AND a `mikebom:srcrev` annotation with the full 40-hex value AND a `mikebom:src-uri` array containing both the git URI and the file:// patch AND `mikebom:yocto-recipe-name = "widget"` AND `mikebom:yocto-recipe-version = "1.0"`; (b) `autoinc_version/` AUTOINC recipe with no git SRC_URI emits FR-011's `pkg:generic/gadget@f597fb026637?openembedded=true&layer=...` per FR-018 fallback; (c) the `git_git.bb` recipe with no git SRC_URI emits `version = "..."` (12-hex SRCREV prefix) NOT literal `"git"` per FR-018 + SC-010; (d) a recipe with `SRC_URI = "http://example.com/widget-1.0.tar.gz"` (tarball, no git) emits `pkg:generic/.../...?openembedded=true&layer=...` (FR-011 fallback, NOT FR-002a host-typed because SRC_URI is not git).

**Checkpoint**: At end of Phase 4, US1 + US2 are independently shippable as an MVP slice. License-complete + source-pinned SBOMs for any Yocto meta-layer scan. SC-001, SC-002, SC-010 verified on the synthetic fixtures.

---

## Phase 5: User Story 3 (P2) — Layer attribution + layer-root BOM subject

**Goal**: SC-003 (every recipe carries `mikebom:yocto-layer`) + SC-004 (BOM subject is a layer-collection, not `pkg:generic/<basename>@0.0.0`).

**Independent Test**: `cargo +stable test --workspace --test yocto_recipe_enrich_us3_layer_attr` passes. The test scans the `multi_layer_polyglot/` fixture (2 nested layers + cross-layer recipes) and asserts each recipe is attributed to the correct layer AND the BOM root is a layer-collection.

### Implementation for US3

- [ ] T027 [US3] In `layer_conf.rs`, add `pub fn build_index(rootfs: &Path, exclude_set: &ExclusionSet) -> Vec<LayerConf>` walking the scan tree for `conf/layer.conf` files via `safe_walk` depth 8. Parse each into a `LayerConf`.
- [ ] T028 [US3] In `layer_conf.rs`, add `pub fn attribute_recipe(recipe_path: &Path, layer_index: &[LayerConf]) -> Option<&LayerConf>` implementing the nearest-ancestor heuristic per FR-006 + Q2: walk `recipe_path.ancestors()` and for each ancestor check if any LayerConf's `source_path.parent().parent()` (the layer root, since layer.conf sits at `<layer>/conf/layer.conf`) equals the ancestor. Return the deepest match.
- [ ] T029 [US3] In `recipe.rs`, after recipes are emitted, populate per FR-006: for each main-module-tagged recipe component, call `attribute_recipe(...)` and emit `mikebom:yocto-layer = <collection>`, `mikebom:yocto-layer-version = <version>`, `mikebom:yocto-layer-series = <JSON array>` annotations. When no ancestor layer.conf is found, emit `tracing::warn!` log naming the recipe path per US3 AC#4.
- [ ] T030 [US3] In `recipe.rs`, emit one synthesized main-module-tagged `PackageDbEntry` per `LayerConf` per FR-007. PURL: `pkg:generic/<collection>@<LAYERVERSION>?openembedded=true&layer=<collection>`. Name: `<collection>`. Set `extra_annotations["mikebom:component-role"] = "main-module"` AND `extra_annotations["mikebom:source-files"] = [<conf/layer.conf path>]` so milestone-127's `tag_main_modules_with_workspace_root` picks it up + `select_root` elects it via the FR-002 repo-root tiebreaker.
- [ ] T031 [P] [US3] Build the `multi_layer_polyglot/` fixture: two nested layer directories under a parent dir. `layer-a/conf/layer.conf` with `BBFILE_COLLECTIONS += "layer-a"` + `LAYERVERSION_layer-a = "2"`. `layer-b/conf/layer.conf` with `BBFILE_COLLECTIONS += "layer-b"` + `LAYERVERSION_layer-b = "1"`. Three recipes: `layer-a/recipes-x/foo_1.0.bb`, `layer-a/recipes-y/bar_1.0.bb`, `layer-b/recipes-z/baz_1.0.bb`. Each with minimal LICENSE + SRC_URI.
- [ ] T032 [US3] Add the integration test at `mikebom-cli/tests/yocto_recipe_enrich_us3_layer_attr.rs`. Test cases: (a) on `multi_layer_polyglot/`, each recipe carries the correct `mikebom:yocto-layer` value (recipes-x and recipes-y → `layer-a`; recipes-z → `layer-b`); (b) the BOM subject (CDX `metadata.component.purl`) identifies one of the two layer-collections (whichever milestone-127's repo-root tiebreaker picks); (c) on `single_layer_meta/`, the BOM subject is `pkg:generic/meta-fixture-single@1?openembedded=true&layer=meta-fixture-single`.

**Checkpoint**: At end of Phase 5, layer attribution is end-to-end. SC-003 + SC-004 verified on synthetic fixtures.

---

## Phase 6: User Story 4 (P2) — `.bbappend` provenance

**Goal**: Every recipe component with ≥1 matching `.bbappend` in the scan carries `mikebom:bbappend-applied` listing the appends' paths.

**Independent Test**: `cargo +stable test --workspace --test yocto_recipe_enrich_us4_bbappend` passes.

### Implementation for US4

- [ ] T033 [US4] In `bbappend.rs`, complete `build_from_walk` to populate `BbAppendIndex.by_recipe` AND `BbAppendIndex.orphans`. The orphan classification happens lazily — initially every parsed `.bbappend` goes into `by_recipe`; the `appends_for` call site marks consumed entries; at scan-end, unconsumed `.bbappend`s move to `orphans`. Add a `pub fn finalize_orphans(&mut self, recipe_keys: &BTreeSet<(String, String)>) -> Vec<&PathBuf>` that performs the move + returns the new orphans.
- [ ] T034 [US4] In `recipe.rs`, after recipes are collected, build `BbAppendIndex` from the scan. For each recipe component, call `appends_for(&name, &version)` and emit `mikebom:bbappend-applied` as a JSON-encoded array (lex-sorted, deduped, workspace-relative paths) per FR-008 + contracts/annotation-schema.md.
- [ ] T035 [US4] In `recipe.rs`, at scan-end emit `tracing::warn!` for each orphan from `BbAppendIndex.finalize_orphans` per US4 AC#3. Do NOT emit phantom components for orphans (Constitution Principle VIII).
- [ ] T036 [P] [US4] Build the `orphan_bbappend/` fixture: one `recipe-z_1.0.bb` (recipe present) + one `nonexistent-recipe_%.bbappend` (orphan, no matching recipe). Layer.conf optional (test isolates US4 from US3).
- [ ] T037 [US4] Add the integration test at `mikebom-cli/tests/yocto_recipe_enrich_us4_bbappend.rs`. Test cases: (a) extend `multi_layer_polyglot/` (T031) with `layer-b/recipes-x/foo_%.bbappend` and assert the `foo@1.0` component (from layer-a) carries `mikebom:bbappend-applied` listing the layer-b append path; (b) on `orphan_bbappend/`, assert NO component named `nonexistent-recipe` is emitted AND the captured stderr contains the warn log.

**Checkpoint**: At end of Phase 6, bbappend provenance is end-to-end.

---

## Phase 7: User Story 5 (P3) — DEPENDS / RDEPENDS relationship edges

**Goal**: SC-005 — `meta-balena` scan emits ≥50 `DEPENDS_ON` edges derived from recipe DEPENDS.

**Independent Test**: `cargo +stable test --workspace --test yocto_recipe_enrich_us5_depends` passes.

### Implementation for US5

- [ ] T038 [US5] In `recipe_body.rs`, parse `DEPENDS = "..."` (space-separated recipe names) into `RecipeMetadata.depends: Vec<String>`. Parse `RDEPENDS_<pkg> = "..."` patterns into `rdepends: BTreeMap<String, Vec<String>>` keyed by `<pkg>` suffix.
- [ ] T039 [US5] In `recipe.rs`, after all recipes are collected, build a `BTreeMap<String, Purl>` index from recipe name → emitted PURL. Walk every recipe's `depends` and `rdepends`; for each entry that resolves to a recipe-name in the index, push a `Relationship { source, target, kind: DependsOn, lifecycle_scope }` onto the scan's relationship list. Use build scope for DEPENDS, runtime scope for RDEPENDS per FR-009.
- [ ] T040 [US5] In `recipe.rs`, accumulate UNRESOLVED entries (DEPENDS / RDEPENDS that didn't match any scanned recipe) and emit `mikebom:depends-unresolved` / `mikebom:rdepends-unresolved` annotations on the affected recipe components per FR-009 + contracts/annotation-schema.md. NEVER silently drop — every unresolved entry MUST surface so consumers see closure gaps.
- [ ] T041 [P] [US5] Extend the `multi_layer_polyglot/` fixture (T031) with concrete dependency declarations: `foo_1.0.bb` adds `DEPENDS = "bar openssl-dev"` (where `bar` resolves to the existing `bar_1.0.bb`, `openssl-dev` is unresolvable); `baz_1.0.bb` adds `RDEPENDS_${PN} = "bar"`.
- [ ] T042 [US5] Add the integration test at `mikebom-cli/tests/yocto_recipe_enrich_us5_depends.rs`. Test cases: (a) the SBOM contains a `DEPENDS_ON` edge from `foo@1.0` to `bar@1.0` with build scope; (b) the SBOM contains a `DEPENDS_ON` edge from `baz@1.0` to `bar@1.0` with runtime scope; (c) the `foo@1.0` component carries `mikebom:depends-unresolved` annotation listing `"openssl-dev"` (because no `openssl-dev` recipe was in the scan).

**Checkpoint**: At end of Phase 7, the SBOM is a graph, not a list. SC-005 verified on synthetic fixture.

---

## Phase 8: HOMEPAGE / SUMMARY / DESCRIPTION / BBCLASSEXTEND / transparency annotations

**Purpose**: Land the FR-010 metadata fields + FR-005 transparency for unexpanded variables + FR-016 override-merge transparency + BBCLASSEXTEND.

⚠️ This phase has NO user-story label because its FRs span US1–US5 transparency requirements.

- [ ] T043 [P] In `recipe_body.rs`, parse `HOMEPAGE = "..."`, `SUMMARY = "..."`, `DESCRIPTION = "..."`, `BBCLASSEXTEND = "..."` fields into `RecipeMetadata`.
- [ ] T044 In `recipe.rs`, populate the per-component standards-native fields per FR-010: HOMEPAGE → `external_references` entry of type `website`; SUMMARY → `ResolvedComponent.description` (CDX `component.description`, SPDX `Package.summary`); DESCRIPTION → `mikebom:yocto-description` annotation ONLY when it differs materially from SUMMARY. Emit `mikebom:yocto-class-extend` as JSON-encoded array when BBCLASSEXTEND is non-empty.
- [ ] T045 In `recipe.rs`, emit `mikebom:yocto-overrides-merged: true` annotation per FR-016 when `RecipeMetadata.overrides_merged == true`. Emit `mikebom:yocto-unexpanded-vars` as a JSON object keyed by field-name per FR-005 + contracts/annotation-schema.md when `RecipeMetadata.unexpanded_vars` is non-empty.
- [ ] T046 [P] Build the `unexpanded_var/` fixture: one recipe `mystery_1.0.bb` with `LICENSE = "${BPN}"` (variable-substituted LICENSE — `${BPN}` is BitBake's "base PN" derived from PROVIDES at metadata-evaluation time and is explicitly NOT in FR-005's `{${PN}, ${PV}}` resolvable set). Expected: component emits `licenses = []` AND `mikebom:yocto-unexpanded-vars = {"LICENSE": ["${BPN}"]}` AND a `tracing::warn!` log naming the file.
- [ ] T047 In `mikebom-cli/tests/yocto_recipe_enrich_us1_license.rs`, add a test case for `unexpanded_var/` asserting the unexpanded-vars annotation shape.

---

## Phase 9: CPE-name normalization + multi-CPE candidates (FR-017 + FR-019)

**Purpose**: Wire the `cpe_name_map.rs` table into the milestone-097 cpe-candidates synthesizer for Yocto recipes.

- [ ] T048 In `recipe.rs`, after each recipe component is built, populate `extra_annotations["mikebom:cpe-candidates"]` as a JSON-encoded array per FR-019 + FR-017. The array MUST contain: (a) the raw recipe name AND (b) `cpe_product_for_recipe(recipe_name)` from FR-017's mapping (omitted when identical to the raw name — no duplicates). Lex-sort + dedup. Additional suffix-stripping / variant-expansion rules are OUT OF SCOPE for milestone 128 — the openembedded-core mapping table from FR-017 is the single source of truth; further normalization can land as a follow-up milestone if vuln-scanner-coverage metrics show a need.
- [ ] T049 [P] Build the `multi_cpe_curl/` fixture: one recipe `curl_8.7.1.bb` with minimal LICENSE + SRC_URI. The CPE-candidates synthesizer should produce ≥3 candidates (`curl`, `curl_client`, `libcurl` mappings — verify against the embedded mapping table).
- [ ] T050 Add the integration test at `mikebom-cli/tests/yocto_recipe_enrich_us3_layer_attr.rs` (reused; or add a dedicated `yocto_recipe_enrich_cpe.rs`). Test cases: (a) on `single_layer_meta/`, a recipe named `linux-kernel` (synthesize one) emits `mikebom:cpe-candidates` array containing `"linux_kernel"` per FR-017; (b) on `multi_cpe_curl/`, EXACTLY ONE component named `curl@8.7.1` is emitted (NOT 6 as in the reference Yocto-tooling SBOM) and the candidates array contains ≥3 vendor/product permutations per FR-019 + SC-011.

---

## Phase 10: Mixed-scan cross-source (FR-014a / Q3) — verify-only

**Purpose**: Confirm milestone-105's `also-detected-via` path works for Yocto-recipe ↔ opkg-DB cross-source merges. No code change expected.

- [ ] T051 Add the integration test at `mikebom-cli/tests/yocto_recipe_enrich_mixed_scan.rs`. Setup: synthesize a fixture containing BOTH a meta-layer (one `conf/layer.conf` + one recipe `widget_1.0.bb` with `LICENSE = "MIT"`) AND a synthetic opkg DB (`/var/lib/opkg/status` with a `Package: widget` / `Version: 1.0` stanza). Scan the combined directory. Assert: (a) the post-dedup output contains EXACTLY ONE component for `widget@1.0`; (b) that component carries BOTH the recipe's `licenses = ["MIT"]` AND the opkg DB's `installed-files` evidence; (c) the surviving component's `mikebom:also-detected-via` annotation lists both source-mechanisms. If the assertion fails, the milestone-105 also-detected-via path needs adjustment for the recipe-reader's source-mechanism string — patch `resolve/deduplicator.rs` then.

---

## Phase 11: Polish — balena smoke, byte-identity, perf, docs, parity-catalog, CHANGELOG, pre-PR

**Purpose**: End-to-end validation on the three balena clones, byte-identity preservation on existing fixtures, docs updates, parity-catalog C-rows, CHANGELOG, pre-PR gate.

- [ ] T052 Add the integration test at `mikebom-cli/tests/yocto_recipe_enrich_balena_smoke.rs`. Use `env!("MIKEBOM_FIXTURES_DIR")` (the milestone-090 fixture cache) to locate the three balena clones (`meta-balena`, `balena-raspberrypi`, `balena-generic`). For each, run `mikebom sbom scan --path <clone>` and assert: SC-001 (license coverage ≥80% on meta-balena), SC-002 (vcs ≥60% / srcrev ≥40%), SC-003 (every recipe has `mikebom:yocto-layer`), SC-005 (≥50 DEPENDS_ON edges), SC-009 (≥10 CPE-normalized names), SC-010 (NO `version: "git"` or `AUTOINC+...` strings), SC-011 (NO duplicate `curl` components). The test is gated by `MIKEBOM_FIXTURES_DIR` availability — graceful-skip when absent (matches the milestone-090 convention).
- [ ] T053 Add the integration test at `mikebom-cli/tests/yocto_recipe_enrich_byte_identity.rs`. Iterate every fixture under `mikebom-cli/tests/fixtures/golden/` and assert the emitted CDX / SPDX 2.3 / SPDX 3 outputs are byte-identical to the committed golden. NO `MIKEBOM_UPDATE_*` env vars set. Implements SC-006 (zero regression on 33 alpha.48 goldens — none of them are Yocto fixtures, so the new reader should be a no-op for all of them).
- [ ] T054 [P] In `mikebom-cli/benches/perf.rs` (or wherever milestone-094 benchmarks live), add a `yocto_recipe_enrich` benchmark target. Measure wall-clock on `MIKEBOM_FIXTURES_DIR`'s `meta-balena` clone. Assert SC-008: <2× the milestone-107 filename-only baseline AND ≤30s wall-clock. Bench MAY graceful-skip when `MIKEBOM_FIXTURES_DIR` is unset.
- [ ] T055 [P] Add 15 new C-rows to `docs/reference/sbom-format-mapping.md` per contracts/annotation-schema.md "Catalog C-rows" section. Use the next-free integer (working assumption C70..C84; verify by scanning the current catalog at PR time). Each row carries a full Principle V native-field audit narrative per research R1's table. Mirror the milestone-127 C69 entry's shape.
- [ ] T056 [P] Register the 15 new C-rows in `mikebom-cli/src/parity/extractors/`. For each new annotation key, add an extractor function (component-scope) in `cdx.rs`, `spdx2.rs`, `spdx3.rs` mirroring the milestone-127 C69 pattern, and register the entries in `mod.rs`'s `ParityExtractor` slice + the `use` statements at the top of `mod.rs`.
- [ ] T057 [P] Update `docs/ecosystems.md` to document the new Yocto source-tier emission shape: PURL convention (`pkg:generic/<name>@<version>?openembedded=true&layer=<collection>`), license attribution, source pinning via SRCREV, layer-root BOM subject. Cross-link milestones 107 (image-tier) and 127 (root selector).
- [ ] T058 [P] Update `CHANGELOG.md`'s `[Unreleased]` section with a milestone-128 entry. Document: (a) the PURL migration `pkg:bitbake/...?layer=...` → `pkg:generic/...?openembedded=true&layer=<collection>` as a behavior change, (b) the 5 user stories (license, src_uri/srcrev, layer attribution, bbappend, DEPENDS edges), (c) the 3 reference-SBOM-grounded wins over the upstream Yocto tooling (no `version: "git"` smell, no multi-CPE fan-out, CPE-name normalization), (d) zero byte-identity churn on 33 alpha.48 goldens (SC-006), (e) the 15 new `mikebom:yocto-*` annotation keys with C-row C70..C84.
- [ ] T059 Run `./scripts/pre-pr.sh` (mandatory pre-PR gate per CLAUDE.md). MUST exit 0 with `cargo +stable clippy --workspace --all-targets -- -D warnings` clean AND `cargo +stable test --workspace` showing every target `0 failed`.
- [ ] T060 Run `./scripts/regen-goldens.sh` (milestone-126 wrapper) and verify `git status` shows NO golden churn. Any churn means SC-006 was broken — debug before merging.

---

## Dependencies & Execution Order

```text
Phase 1 (Setup): T001, T002 [P]
   ↓
Phase 2 (Foundational): T003, T004, T005, T006 [P] (independent data-model types + CPE table)
                        → T007 (body parser depends on RecipeMetadata)
                        → T008 (override-syntax depends on T007)
                        → T009 (include resolver depends on T007 + T008)
                        → T010 (PURL migration in existing recipe.rs)
                        → T011 (refactor recipe.rs to call body parser)
                        → T012 (FR-018 version-derivation depends on T011)
                        → T013 (unit tests; depends on T007–T012)
   ↓
Phase 3 (US1 license): T014 → T015 → T016 (sequential — all in recipe.rs)
                        T017, T018 [P] (fixtures independent)
                        → T019 (integration test depends on T014–T018)
   ↓
Phase 4 (US2 src_uri): T020 → T021 → T022 → T023 (sequential — all in recipe.rs)
                       T024, T025 [P] (fixtures independent)
                       → T026 (integration test depends on T020–T025)
   ↓
Phase 5 (US3 layer): T027 → T028 (layer_conf.rs sequential)
                     → T029 → T030 (recipe.rs sequential)
                     T031 [P] (fixture)
                     → T032 (integration test depends on T027–T031)
   ↓
Phase 6 (US4 bbappend): T033 → T034 → T035 (sequential — bbappend.rs + recipe.rs)
                        T036 [P] (fixture)
                        → T037 (integration test depends on T033–T036)
   ↓
Phase 7 (US5 DEPENDS): T038 → T039 → T040 (sequential)
                       T041 [P] (fixture extension)
                       → T042 (integration test depends on T038–T041)
   ↓
Phase 8 (HOMEPAGE/SUMMARY/etc): T043 [P], T046 [P]
                                → T044 → T045 (recipe.rs sequential)
                                → T047 (test extension depends on T043–T046)
   ↓
Phase 9 (CPE-name + multi-CPE): T048 (recipe.rs)
                                T049 [P] (fixture)
                                → T050 (integration test depends on T048 + T049)
   ↓
Phase 10 (Mixed scan): T051 (integration test only; no production change unless milestone-105 needs patching)
   ↓
Phase 11 (Polish):
   T052 (balena smoke), T053 (byte-identity), T054 [P] (perf bench)
   T055 [P] (parity catalog C-rows), T056 [P] (parity extractor registrations), T057 [P] (ecosystems.md), T058 [P] (CHANGELOG)
   T059 (pre-PR gate; depends on all production code being landed)
   T060 (regen-goldens sanity; depends on T059)
```

**Story dependencies**: US1 and US2 are independent slices once Phase 2 is done — either can land first as the MVP. US3 builds on the same body-parser scaffold but its emission path is independent. US4 depends on US3's `layer_conf` index existing (since bbappend matching happens after recipes are collected and indexed by name+version). US5 depends on US1+US2 having populated the recipe-name → PURL index.

## Parallel execution opportunities

**Phase 2 fan-out (T003, T004, T005, T006)**: Four independent files. With 4-way fan-out, this phase compresses to one person-task-duration.

**Phase 3 fixtures (T017, T018)**: Two different fixture trees, no overlap.

**Phase 4 fixtures (T024, T025)**: Two different fixture trees.

**Phase 11 polish (T054–T058)**: 5 independent files (perf bench, parity catalog markdown, parity extractor Rust files, ecosystems.md, CHANGELOG.md). Maximum fan-out.

## Suggested MVP scope

The minimal shippable increment is **Phase 1 + Phase 2 + Phase 3 (US1 license) + Phase 8 transparency annotations**. That delivers:

- SC-001: License coverage on every recipe-derived component (the headline value-add over the upstream Yocto-tooling baseline).
- The PURL migration (pkg:bitbake → pkg:generic) lands in Phase 2 with the other foundational work.
- Byte-identity preserved on alpha.48 goldens (the new reader is a no-op for non-Yocto scans).

Phase 4 (US2) + Phase 5 (US3) + Phase 6 (US4) + Phase 7 (US5) + Phase 9 (CPE) + Phase 10 (mixed-scan) follow in subsequent PRs. Each has its own integration test, so each PR is independently reviewable and revertible.

**Recommendation**: Given the spec's emphasis on cross-format consistency (FR-005-style) and the fact that all five US phases share the body-parser scaffold, ship as ONE coordinated PR covering Phase 1–11. The selector and emitter share the same scaffold; splitting risks shipping a partial story that confuses consumers.

## Format validation

All 60 tasks above strictly follow `- [ ] T### [P?] [Story?] Description with file path`. Setup (T001..T002), Foundational (T003..T013), Phase 8 (T043..T047), Phase 9 (T048..T050), Phase 10 (T051), and Polish (T052..T060) carry NO `[Story]` label. US-phase tasks (T014..T042) carry `[US1]`, `[US2]`, `[US3]`, `[US4]`, or `[US5]`. Every task description names at least one absolute or workspace-relative file path.
