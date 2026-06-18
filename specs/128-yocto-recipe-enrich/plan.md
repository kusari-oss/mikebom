# Implementation Plan: Deeper Yocto / OpenEmbedded SBOM coverage

**Branch**: `128-yocto-recipe-enrich` | **Date**: 2026-06-18 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/128-yocto-recipe-enrich/spec.md`

## Summary

Milestone 107 ships a `.bb`-filename walker that emits one component per `<name>_<version>.bb` with NO body parsing. That's the entire scope of mikebom's current Yocto source-tier output: no LICENSE, no SRC_URI, no SRCREV, no HOMEPAGE, no SUMMARY, no DEPENDS edges, no layer-collection attribution, no `.bbappend` provenance, no CPE-name normalization. The PURL type is mikebom-invented `pkg:bitbake/...?layer=...`. A reference SBOM produced by the upstream Yocto tooling itself (a 145-component balena-OS image CDX) confirms (a) `pkg:generic/...` is the ecosystem convention, (b) `version: "git"` and `AUTOINC+<sha>` are smells we must reject, (c) multi-CPE-vendor fan-out is the wrong pattern. Milestone 128 closes those gaps in one coordinated PR by replacing the filename-only walker with a body-parsing reader that:

1. Parses `LICENSE`, `SRC_URI`, `SRCREV`, `HOMEPAGE`, `SUMMARY`, `DESCRIPTION`, `DEPENDS`, `RDEPENDS` from every `.bb` and follows `require` / `include` directives transitively (FR-001 through FR-005, FR-009, FR-010).
2. Parses `conf/layer.conf` for `BBFILE_COLLECTIONS` / `LAYERVERSION_<collection>` / `LAYERSERIES_COMPAT_<collection>` and attributes each recipe to its nearest-ancestor layer (FR-006). Emits one main-module-tagged layer-root component per detected layer.conf so milestone-127's root-selection ladder elects the layer as the BOM subject (FR-007).
3. Walks `.bbappend` files and annotates the targeted base recipes with `mikebom:bbappend-applied` (FR-008).
4. Migrates the PURL type from `pkg:bitbake/...?layer=...` to `pkg:generic/<recipe-name>@<version>?openembedded=true&layer=<collection>` aligning with the upstream Yocto-tooling convention (FR-011, Clarifications Q1).
5. Applies the openembedded-core CPE-name normalization table when emitting milestone-097 CPE candidates (FR-017) AND derives PURL versions from SRCREV when `PV` is `git` / `AUTOINC` (FR-018) AND emits one component per recipe with all CPE vendor permutations in the milestone-097 candidates array (FR-019).
6. Field-precedence semantics: last-write-in-source-order wins on `.bb` vs `.inc` conflicts. Layer attribution by nearest-ancestor `conf/layer.conf` heuristic. Cross-source dedup on mixed scans (meta-layer tree + opkg DB) flows through milestone-105's PURL-based pipeline; no explicit cross-reader correlation pass.

Three motivating fixtures (`meta-balena`, `balena-raspberrypi`, `balena-generic`) anchor the success criteria (SC-001 ≥80% license, SC-002 ≥60% vcs ref / ≥40% srcrev, SC-005 ≥50 edges, SC-007 sbomqs +30 points). Goldens land as small synthesized trees under `mikebom-cli/tests/fixtures/yocto_recipe_enrich/`; the three balena clones are integration-test inputs in `MIKEBOM_FIXTURES_DIR` (the milestone-090 fixture cache), not goldens.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–127; no nightly required).

**Primary Dependencies**: Existing only — `regex` (already direct dep since milestones 113 + 127), `std::str::Lines` (recipe-body parsing), `std::path::{Path, PathBuf}` + `std::fs::canonicalize` (layer attribution + include resolution), `mikebom_common::types::license::SpdxExpression::try_canonical` (LICENSE canonicalization), `mikebom_common::types::purl::Purl::new` (PURL construction, already validates against purl-spec), `tracing`, `anyhow`, `serde`/`serde_json`. **Zero new Cargo dependencies.**

**Storage**: N/A — all state in-process per scan; persisted only inside the emitted SBOM via the existing per-component `extra_annotations` channel + the milestone-127 layer-root component shape. Mirrors every milestone since 002.

**Testing**: `cargo +stable test --workspace` integration tests. Synthesized fixture trees under `mikebom-cli/tests/fixtures/yocto_recipe_enrich/` for unit-level acceptance (each AC scenario). End-to-end smoke against the three balena clones via the milestone-090 `MIKEBOM_FIXTURES_DIR` cache (a sibling-repo fetch on first run). The milestone-094 perf benchmark gains a `yocto_recipe_enrich` target to enforce SC-008 (<2× milestone-107 baseline on the motivating fixtures, ≤30s per scan).

**Target Platform**: Linux + macOS + Windows. No OS-specific code paths; recipe-file parsing is text-only.

**Project Type**: Existing three-crate workspace. NO new crates. All changes in `mikebom-cli/src/scan_fs/package_db/yocto/` (extends milestone 107's existing `recipe.rs` + adds `recipe_body.rs` + `layer_conf.rs` + `bbappend.rs` + `cpe_name_map.rs`) + minor wire-up in `scan_fs/package_db/mod.rs` + the dedup pipeline at `resolve/deduplicator.rs` (FR-014a `also-detected-via` path is already there per milestone 105; verify it works for Yocto-recipe ↔ opkg-DB cross-source merges).

**Performance Goals**: Per SC-008, <2× milestone-107 baseline AND ≤30s wall-clock on each of the three balena clones. The body-parser is a single-pass line-oriented scan over recipe files; each recipe is small (<10 KB typical). For meta-balena's 163 recipes + 337 bbappends + 64 inc files, total parse cost is dominated by I/O, not regex evaluation. Layer-attribution walks ancestor directories once per recipe (O(depth) per recipe; bounded by the walker's depth-8 convention). LCP and similar cross-recipe analyses are linear in component count. Target: <500ms incremental cost on meta-balena (the largest motivating fixture).

**Constraints**: 33 alpha.48 byte-identity goldens stay byte-identical (no Yocto fixtures among them). Milestone-107's existing image-tier fixtures (`opkg.rs` + `yocto/manifest.rs` paths) stay byte-identical (FR-014). The new milestone-128 enrichment fires ONLY when `.bb` files are present at the scan root or beneath — no change to non-Yocto scans.

**Scale/Scope**: Per FR-001..FR-019 + FR-002a (17 functional reqs, 5 user stories). Adds ~700-900 LOC across `mikebom-cli/src/scan_fs/package_db/yocto/` (recipe-body parser, layer.conf parser, bbappend walker, CPE-name normalization table, host-typed-PURL detector). 17 new `mikebom:yocto-*` annotation keys (FR-013) + 17 new parity-catalog C-rows (C70..C86). FR-019 reuses the existing milestone-097 `mikebom:cpe-candidates` channel — no new key for that one. Three new motivating-fixture integration tests via `MIKEBOM_FIXTURES_DIR`; ~8-10 synthesized fixture trees for per-AC unit-style coverage. The PURL migration (FR-011) is a 2-site format-string update in `recipe.rs` plus the byte-identity-exempt CHANGELOG note. FR-002a host-typed PURL emission adds a small `git_url_parser.rs` helper (~50 LOC) plus a SRC_URI-host dispatch before `build_bitbake_purl` in `recipe.rs`.

## Constitution Check

Evaluating against mikebom Constitution v1.4.0 (`.specify/memory/constitution.md`).

| Principle | Status | Notes |
|---|---|---|
| I. Pure Rust, Zero C | ✓ | User-space Rust only; no C; no eBPF changes. |
| II. eBPF-Only Observation | ✓ N/A | Source-tier reader; no discovery layer change — recipes are already discovered by milestone 107's `.bb` walker, this feature enriches the per-component fields. |
| III. Fail Closed | ✓ | Every parse-failure path emits `tracing::warn!` and returns `NOASSERTION` / skips the affected field. Malformed recipes don't crash the scan. Missing `conf/layer.conf` ancestor → continue without layer annotation + warn (US3 AC#4). Orphan `.bbappend` → warn + don't synthesize a phantom component (US4 AC#3). |
| IV. Type-Driven Correctness | ✓ | New entities (`RecipeMetadata`, `LayerConf`, `BbAppendIndex`) are typed structs/enums in `scan_fs/package_db/yocto/`. License canonicalization goes through the existing `SpdxExpression::try_canonical` newtype. PURL construction goes through `Purl::new` which validates against the purl-spec. CPE-name normalization uses a typed `&'static [(recipe: &str, cpe_product: &str)]` lookup. No `.unwrap()` in production code. |
| V. Specification Compliance | ✓ + audit | **Native-field audit performed for every new `mikebom:yocto-*` annotation.** Per FR-013, each of the 15 new keys needs a Principle V row in `docs/reference/sbom-format-mapping.md`. The major standards-native carriers ARE used where they exist: `LICENSE` → CDX `components[].licenses[]` + SPDX 2.3 `packages[].licenseDeclared` + SPDX 3 `Package.declaredLicense` (no parity-bridging needed); `HOMEPAGE` → CDX/SPDX `externalReferences[type=website]`; `SRC_URI` git → CDX/SPDX `externalReferences[type=vcs]`; `SUMMARY` → CDX `component.description` + SPDX `Package.summary`. The parity-bridging `mikebom:yocto-*` keys carry signals no standard field models (layer attribution, SRCREV pinning, bbappend provenance, override-merge transparency, unexpanded-variable disclosure). The PURL migration (FR-011) is to a published purl-spec type (`pkg:generic`); qualifiers carry the Yocto-specific signals. CHANGELOG documents the PURL behavior change. |
| VI. Three-Crate Architecture | ✓ | No new crates. Changes in `mikebom-cli/src/scan_fs/package_db/yocto/`. |
| VII. Test Isolation | ✓ | Tests use `tempfile::tempdir()`-isolated synthesized fixtures + the milestone-090 `MIKEBOM_FIXTURES_DIR` cache for the three balena clones. No eBPF privilege requirement. |
| VIII. Completeness | ✓ | Per Constitution VIII: orphan `.bbappend`s do NOT synthesize phantom components (would violate "completeness measured against trace observations"). Recipes that fail body-parse still emit their filename-derived component (better partial signal than dropping). |
| IX. Accuracy | ✓ | The reader IMPROVES accuracy: today's `pkg:bitbake/<name>@<version>` carries zero CVE-matchable evidence; post-128 the same recipe carries an SPDX-canonical license + a vcs+SRCREV pin + a CPE-normalized candidate name → vuln scanners can match. FR-018's rejection of `version: "git"` is a direct accuracy improvement over the upstream Yocto tooling itself. |
| X. Transparency | ✓ | FR-005 + FR-016 + FR-017 + FR-018 each emit a transparency annotation when a heuristic / approximation / normalization fired (`mikebom:yocto-unexpanded-vars`, `mikebom:yocto-overrides-merged`, etc.). Operators can see EVERY decision the reader made. |
| XI. Enrichment | ✓ N/A | No enrichment-source interaction. |
| XII. External Data Source Enrichment | ✓ N/A | No external data source. CPE-name mapping table is compiled-in. |

**Verdict**: No violations. No complexity-tracking entries.

## Project Structure

### Documentation (this feature)

```text
specs/128-yocto-recipe-enrich/
├── plan.md              # This file
├── spec.md              # Feature spec (with Clarifications)
├── research.md          # Phase 0: Principle V native-field audits + CPE-name mapping source + reference-SBOM analysis + perf budget
├── data-model.md        # Phase 1: RecipeMetadata, LayerConf, BbAppendIndex, CpeNameMap, ScopedField
├── quickstart.md        # Phase 1: end-to-end repro recipes for SC-001..SC-011 against the three balena clones
├── contracts/           # Phase 1: per-format emission shape contracts + annotation JSON schema contract + reader-behavior contract
└── checklists/
    └── requirements.md  # /speckit-specify-time quality gate (16/16 items pass)
```

### Source Code (repository root)

```text
mikebom-cli/src/
├── scan_fs/
│   └── package_db/
│       ├── yocto/
│       │   ├── recipe.rs                     # EXTEND — replace filename-only walker with body-parsing reader (FR-001..FR-005, FR-010, FR-011, FR-018)
│       │   ├── recipe_body.rs                # NEW — line-oriented .bb / .inc body parser (FIELD = "...", FIELD += "...", FIELD:append:<override> = "...")
│       │   ├── layer_conf.rs                 # NEW — conf/layer.conf parser (FR-006) + nearest-ancestor attribution (Q2)
│       │   ├── bbappend.rs                   # NEW — .bbappend walker + base-recipe match index (FR-008)
│       │   ├── cpe_name_map.rs               # NEW — embedded openembedded-core recipe→CPE-product normalization table (FR-017)
│       │   ├── manifest.rs                   # UNCHANGED — milestone-107 image-manifest reader (FR-014 byte-identity)
│       │   ├── context.rs                    # UNCHANGED — milestone-107 sysroot/rootfs detection
│       │   └── mod.rs                        # UPDATE — wire layer_conf::read + bbappend::read into read_all
│       └── mod.rs                            # MINOR — recipe + bbappend + layer.conf walker invocation order
├── resolve/
│   └── deduplicator.rs                       # VERIFY — milestone-105's also-detected-via path handles Yocto-recipe ↔ opkg-DB cross-source merge per FR-014a (no code change expected; verify with integration test)
└── generate/
    └── cyclonedx/builder.rs                  # MINOR — no Yocto-specific change; existing CDX `licenses[]`, `externalReferences[]`, `description` slots all populate from the new RecipeMetadata fields automatically

mikebom-cli/tests/
├── yocto_recipe_enrich_us1_license.rs        # NEW — SC-001 license coverage (≥80% on synthesized fixture matching meta-balena's shape)
├── yocto_recipe_enrich_us2_src_uri.rs        # NEW — SC-002 vcs/srcrev coverage
├── yocto_recipe_enrich_us3_layer_attr.rs     # NEW — SC-003/SC-004 layer attribution + layer-root BOM subject
├── yocto_recipe_enrich_us4_bbappend.rs       # NEW — bbappend provenance
├── yocto_recipe_enrich_us5_depends.rs        # NEW — SC-005 DEPENDS_ON edges
├── yocto_recipe_enrich_balena_smoke.rs       # NEW — end-to-end smoke against the three balena clones via MIKEBOM_FIXTURES_DIR
├── yocto_recipe_enrich_byte_identity.rs      # NEW — SC-006 zero-regression on all 33 alpha.48 goldens + milestone-107 image-tier fixtures
└── fixtures/
    └── yocto_recipe_enrich/
        ├── single_layer_meta/                # ONE conf/layer.conf, 3 recipes, mixed LICENSE shapes (MIT, GPL-2.0-only & LGPL-2.1-or-later, CLOSED)
        ├── multi_layer_polyglot/             # Two nested layers + cross-layer bbappend + DEPENDS edges (mirrors balena-generic-shape)
        ├── include_chain/                    # foo.bb → require foo.inc → require foo-shared.inc (FR-004 last-write-in-source-order)
        ├── git_srcuri_srcrev/                # FR-002 + FR-018 vcs+srcrev coverage
        ├── autoinc_version/                  # FR-018 — recipe with PV containing AUTOINC; expect derived-from-srcrev version
        ├── multi_cpe_curl/                   # FR-019 — single component with multi-CPE-candidates (no fan-out)
        ├── unexpanded_var/                   # FR-005 — recipe with ${BPN} reference; expect mikebom:yocto-unexpanded-vars annotation
        └── orphan_bbappend/                  # FR-008 — bbappend with no matching recipe; expect warn + no phantom

docs/
└── reference/sbom-format-mapping.md          # ADD — 15 new C-rows (C70..C84) for the new mikebom:yocto-* annotation keys, each with full Principle V native-field audit narrative

CHANGELOG.md                                   # ADD — Unreleased entry documenting the PURL migration (pkg:bitbake → pkg:generic) + the 5 user stories + the 3 reference-SBOM-grounded behavior wins (license, srcrev, CPE-name normalization)
```

**Structure Decision**: Existing three-crate workspace (`mikebom-cli` + `mikebom-common` + `mikebom-ebpf`); zero new crates per Constitution Principle VI. The Yocto-specific work all lives inside `mikebom-cli/src/scan_fs/package_db/yocto/` — the existing milestone-107 module that already owns the `.bb` walker, image-manifest reader, and sysroot/rootfs detector. The new files (`recipe_body.rs`, `layer_conf.rs`, `bbappend.rs`, `cpe_name_map.rs`) extend the same module with body-parsing capability. The `recipe.rs` file gets refactored (rather than rewritten) so the existing milestone-107 filename-walker code becomes the fallback when body-parsing fails; this preserves the milestone-107 contract for malformed recipes. No changes outside the yocto/ module are needed except a 2-line wire-up in `package_db/mod.rs` and the verification step in `resolve/deduplicator.rs` for FR-014a.

## Complexity Tracking

> No Constitution violations. No entries.
