# Implementation Plan: Cargo source-tree main-module component for crate / workspace-member roots

**Branch**: `064-cargo-main-module` | **Date**: 2026-05-02 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/064-cargo-main-module/spec.md`

## Summary

Extend the milestone 053 main-module pattern to cargo: emit one synthetic main-module component per `Cargo.toml` containing `[package]` (skipping workspace-only roots), placed in standards-native "BOM subject" slots (CDX `metadata.component` for single-crate scans, an existing 053-pattern super-root DESCRIBES set for workspace/polyglot scans; SPDX `documentDescribes` + `primaryPackagePurpose: APPLICATION`; SPDX 3 `software_primaryPurpose: application`). Carries `mikebom:component-role: main-module` (C40) as a supplementary signal. Same-PURL collisions (vendored copies, `examples/` mirrors) dedup with `tracing::warn!`; divergent-PURL detection deferred to issue #125. No git ladder — `Cargo.toml` is authoritative — and no binary-path interaction (cargo binaries don't carry BuildInfo). Closes the cargo slice of issue #104.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–063; no nightly required for this user-space-only work).
**Primary Dependencies**: Existing only — `toml = "0.8"` (already used by `mikebom-cli/src/scan_fs/package_db/cargo.rs:305`), `serde`/`serde_json`, `tracing`, `anyhow`. **No new crates.** No subprocess calls (manifest-only resolution; the `git describe` ladder from milestone 053 is *not* needed because `[package].version` is always declared in cargo manifests).
**Storage**: N/A — all state in-process per scan; no persistence (matches every milestone since 002).
**Testing**: `cargo +stable test --workspace` (unit + integration); existing golden infrastructure regen for cargo-bearing fixtures (single-crate `cargo` golden + new workspace fixture); new dogfood-style smoke test scanning the mikebom workspace itself for SC-002.
**Target Platform**: Linux (x86_64 + aarch64) + macOS (aarch64) — matches existing CI lanes. No platform-specific code in cargo manifest parsing.
**Project Type**: CLI (workspace-rooted; reuses `mikebom-cli/src/scan_fs/package_db/cargo.rs` and the milestone-053 generator-side machinery).
**Performance Goals**: One additional `Cargo.toml` re-parse per discovered manifest with `[package]` (the `parse_lockfile` path already parses `Cargo.toml`s as part of workspace expansion at `cargo.rs:365`; the main-module emission piggybacks on the existing parse). Sub-millisecond impact per scan; orders-of-magnitude smaller than milestone 055's transitive-edge proxy fetches. Does not materially affect the dual-format perf gate (`tests/dual_format_perf.rs` doesn't include a cargo manifest in its test fixture).
**Constraints**: Cross-host byte-identity for goldens — the cargo fixture's main-module PURL is fully determined by the in-fixture `Cargo.toml`'s declared `name` and `version` strings, which are themselves committed to the repo and identical across hosts. No `git describe`-style host-state dependency. Same-PURL dedup must be deterministic on first-discovered (alphabetical filesystem walker order from the existing `walk_for_cargo_lockfiles` algorithm).
**Scale/Scope**: One main-module component per `Cargo.toml` with `[package]` discovered. Mikebom's own workspace exercises 4 main-modules; typical single-crate projects emit 1; rust-cli monorepos may emit dozens.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Per `.specify/memory/constitution.md` v1.4.0:

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Pure Rust, Zero C** | ✅ Pass | All changes in pure Rust. No subprocess calls (no `git describe`). |
| **II. eBPF-Only Observation** | ✅ Pass | Same as 053: the cargo main-module represents the workspace root that's implicitly the scan target; no new dependency-discovery surface. The cargo reader continues to read `Cargo.toml` / `Cargo.lock` per Principle XII (enrichment). |
| **III. Fail Closed** | ✅ Pass | When `version.workspace = true` resolution fails (workspace root absent, key undeclared), the implementation falls back to the literal `0.0.0-unknown` placeholder — transparent and deterministic. Same-PURL collisions degrade gracefully via dedup with operator-visible `tracing::warn!` (no SBOM mutation). |
| **IV. Type-Driven Correctness** | ✅ Pass | Reuses existing `PackageDbEntry`, `Purl`, and `CargoPackage` newtypes. No raw `String` cross-boundary use. Production code uses `anyhow::Result` for the new manifest-resolution helpers; `.unwrap()` only inside `#[cfg(test)]` modules guarded by `#[cfg_attr(test, allow(clippy::unwrap_used))]` per the existing convention. |
| **V. Specification Compliance** | ✅ Pass — **AUDIT PERFORMED** | Per FR-001a: primary signal is native CDX `metadata.component` with `type: "application"` (or its presence as a child of an existing 053-pattern super-root for workspace cases), and SPDX `primaryPackagePurpose: "APPLICATION"` + `documentDescribes`, and SPDX 3 `software_primaryPurpose: "application"`. C40 is supplementary (preserves consumer compat + carries the finer "this is the project itself" semantic that the SPDX `APPLICATION` enum can't encode). PURL conforms to PURL spec (`pkg:cargo/<name>@<version>`). Audit recorded in spec FR-001a + Clarifications session. |
| **VI. Three-Crate Architecture** | ✅ Pass | All changes within `mikebom-cli/`. No new crates. |
| **VII. Test Isolation** | ✅ Pass | New tests are unit-level (`build_cargo_main_module_entry`, `version.workspace = true` resolution, same-PURL dedup) + integration-level (golden regen for cargo + new workspace fixture). No eBPF involvement, no elevated privileges. |
| **VIII. Completeness** | ✅ Pass | This feature *increases* completeness — adds a project-self component that today is silently absent in cargo SBOMs. Consumers gain the ability to answer "what is this an SBOM for?" from the SBOM bytes alone. |
| **IX. Accuracy** | ✅ Pass | Manifest-derived versions are authoritative (no inference); the `0.0.0-unknown` placeholder is transparently distinguishable from real versions. Same-PURL dedup is deterministic (first-discovered, alphabetical walker order); no phantom components. |
| **X. Transparency** | ✅ Pass | Same-PURL dedup emits `tracing::warn!` listing dropped paths (operator visibility per the Constitution-Principle-X transparency stance from milestone 061). Version resolution is observable: a literal `0.0.0-unknown` PURL signals "manifest declared `version.workspace = true` but the workspace root was not resolvable in this scan." Future divergent-PURL detection (issue #125) extends this same transparency pattern. |
| **XI. Enrichment** | ✅ Pass | LICENSE detection is *deferred* to issue #103 explicitly (not silently skipped). Cargo's `[package].license` and `[package].license-file` keys are not read in this milestone — same posture as Go's milestone 053 took for the `LICENSE` file. |
| **XII. External Data Source Enrichment** | ✅ Pass | The `Cargo.toml` `[package]` table is read for the main-module's identity (PURL components: name + version) per Principle XII bullet 1. The main-module component represents the workspace root which IS the scan target — not a new component imported from a lockfile. Per Strict Boundaries clause #1, lockfiles MAY be read for enrichment, MUST NOT introduce components not observed; the main-module is "observed" in the trivial sense that the scan literally targets the workspace. |
| **Strict Boundary #1 (No lockfile-based dependency discovery)** | ✅ Pass | `Cargo.toml`'s `[dependencies]`/`[dev-dependencies]`/`[build-dependencies]` tables are used to *relocate* existing direct edges (FR-007: change the edge `from` from synthetic placeholder root to the new main-module) — no new components are introduced from manifest data. |

**Gate result**: Pass. No constitution violations; no Complexity Tracking entries needed.

## Project Structure

### Documentation (this feature)

```text
specs/064-cargo-main-module/
├── plan.md              # This file
├── spec.md              # Feature specification (Q1 clarification recorded)
├── research.md          # Phase 0 output: design decisions + parallels with 053
├── data-model.md        # Phase 1 output: MainModuleEntry, version-resolution behaviour
├── quickstart.md        # Phase 1 output: dogfood smoke + clap-rs verification recipe
├── contracts/
│   └── cargo-main-module-component.md   # Phase 1 output: per-format placement contract
├── checklists/
│   └── requirements.md  # Spec-quality checklist (all items pass)
└── tasks.md             # Phase 2 output (created by /speckit.tasks)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   ├── scan_fs/
│   │   └── package_db/
│   │       └── cargo.rs                    # ⬅️ MAIN CHANGE — new build_cargo_main_module_entry()
│   │                                       #     called from `read()` for each Cargo.toml with
│   │                                       #     `[package]` discovered (one per workspace member).
│   │                                       #     Adds resolve_workspace_inherited_version() helper
│   │                                       #     for `version.workspace = true` resolution.
│   │                                       #     Adds same-PURL dedup logic with tracing::warn!
│   │                                       #     before returning the entry vec.
│   ├── generate/
│   │   ├── cyclonedx/
│   │   │   ├── metadata.rs                 # ⬅️ GENERALIZE — the existing
│   │   │   │                                #     `go_main_module` selector at L156
│   │   │   │                                #     becomes `any_main_module` filtered
│   │   │   │                                #     by C40 role tag (not by PURL prefix).
│   │   │   │                                #     When N=1: that's metadata.component.
│   │   │   │                                #     When N>1: super-root pattern (existing
│   │   │   │                                #     053 polyglot path) handles the case.
│   │   │   ├── builder.rs                  # ⬅️ EXCLUSION — generalize the milestone 053
│   │   │   │                                #     "filter Go main-module from components[]"
│   │   │   │                                #     to filter ANY main-module (C40 tag-based).
│   │   │   └── dependencies.rs             # (no change — edge-emission is bom-ref-keyed
│   │   │                                    #  and ecosystem-agnostic)
│   │   └── spdx/
│   │       ├── packages.rs                 # ⬅️ EMIT primary_package_purpose=APPLICATION
│   │       │                                #     for cargo main-module entries (already
│   │       │                                #     wired for Go; the field is conditional
│   │       │                                #     on the C40 role tag, so generalizing the
│   │       │                                #     condition picks up cargo automatically)
│   │       ├── document.rs                 # ⬅️ ROOT-SELECTION — milestone 053's
│   │       │                                #     case 3 (multiple top-levels → synthetic
│   │       │                                #     super-root DESCRIBES each) ALREADY
│   │       │                                #     handles workspace member crates;
│   │       │                                #     verify by reading + add a regression test.
│   │       └── v3_relationships.rs         # (no change — DESCRIBES emission is
│   │                                        #  C40-driven, generalizes naturally)
│   ├── parity/
│   │   └── extractors/
│   │       └── (no change — C40 catalog row already wired)
│   └── cli/
│       └── (no change — main-module emission is internal to the cargo reader)
└── tests/
    ├── fixtures/
    │   ├── cargo/                              # ⬅️ NEW FIXTURE DIR — minimal cargo workspace
    │   │   ├── Cargo.toml                      #   workspace root with members [a, b]
    │   │   ├── crates/
    │   │   │   ├── a/Cargo.toml                #   member a, with version.workspace = true
    │   │   │   └── b/Cargo.toml                #   member b, depends on member a via path
    │   │   └── Cargo.lock                      #   committed to make the scan deterministic
    │   └── golden/
    │       ├── cyclonedx/
    │       │   ├── cargo.cdx.json              # ⬅️ REGEN — single-crate fixture gains
    │       │   │                               #     metadata.component swap
    │       │   └── cargo-workspace.cdx.json    # ⬅️ NEW — workspace fixture golden
    │       ├── spdx-2.3/
    │       │   ├── cargo.spdx.json             # ⬅️ REGEN — primaryPackagePurpose +
    │       │   │                               #     documentDescribes swap
    │       │   └── cargo-workspace.spdx.json   # ⬅️ NEW
    │       └── spdx-3/
    │           ├── cargo.spdx3.json            # ⬅️ REGEN
    │           └── cargo-workspace.spdx3.json  # ⬅️ NEW
    ├── scan_cargo.rs                       # ⬅️ NEW TESTS — main-module emission for
    │                                        #     single crate (US1 AS#1), workspace
    │                                        #     members (US1 AS#2), version.workspace
    │                                        #     inheritance (US1 AS#3), dogfood mikebom
    │                                        #     itself (US1 AS#4 + SC-002), same-PURL
    │                                        #     dedup with `tracing::warn!` capture
    └── holistic_parity.rs                  # (existing C40 parity test passes for cargo
                                              #  via the same supplementary tag — extend
                                              #  test data to include cargo fixture row)

docs/
├── reference/
│   └── sbom-format-mapping.md              # ⬅️ DOC UPDATE — annotate C40 row to mention
│                                            #     cargo coverage; add cargo to the per-
│                                            #     ecosystem main-module status table.
└── design-notes.md                         # ⬅️ DOC UPDATE — update the Go-vs-other-
                                             #     ecosystems asymmetry note from milestone
                                             #     053: cargo now landed; npm/pip/maven/gem
                                             #     remain in #104.

CHANGELOG.md                                # ⬅️ DOC UPDATE — `[Unreleased]` →
                                             #     `### Changed (BREAKING — SBOM output
                                             #     shape, milestone 064)` entry: cargo
                                             #     scans now emit a main-module component
                                             #     per crate; goldens regen.
```

**Structure Decision**: Single-crate (`mikebom-cli`) feature. The cargo reader (`scan_fs/package_db/cargo.rs`) emits the new entries; the existing milestone-053 generator-side machinery handles them automatically once the CDX `metadata.component` selector and the SPDX `primary_package_purpose` predicate are generalized from "Go-only" to "C40-tag-driven" (a 2-3 line change in each of `metadata.rs` and `builder.rs`). New fixture directory `tests/fixtures/cargo/` provides a workspace exercising path-deps, `version.workspace = true`, and the polyglot-super-root case; mikebom's own scan is the dogfood test for SC-002.

## Phase 0: Outline & Research — COMPLETE (in-spec)

Phase 0 research is captured directly in the spec's Clarifications section + existing milestone-053 research artifact (`specs/053-go-main-module-edges/research.md`). The cargo design is a deliberate adaptation of 053 with the following decisions recorded in spec assumptions A1–A9:

- **Decision**: Manifest is authoritative; no `git describe` ladder. **Rationale**: cargo's `Cargo.toml` always declares `[package].version` (or inherits via `version.workspace = true`) — there is no equivalent to Go's "module declared in `go.mod` without a tag" gap. **Alternatives considered**: copy the 3-step `git describe` ladder from 053 (rejected — adds subprocess overhead for no gain; cargo manifests don't have the gap that motivated the ladder).
- **Decision**: Workspace-only `Cargo.toml` (no `[package]`) emits no main-module; each member crate emits its own. **Rationale**: matches the cargo publishability model (workspace root is not a publishable artifact unless it has `[package]`). Issue #104 explicitly endorses this approach. **Alternatives**: emit a synthetic workspace-level main-module (rejected — invents a PURL that doesn't correspond to any real cargo crate).
- **Decision**: Same-PURL collisions dedup with `tracing::warn!`, first-discovered wins, divergent case deferred to #125. **Rationale**: per spec Clarifications Q1; matches mikebom's existing edge-dedup discipline; avoids breaking byte-identity goldens. **Alternatives**: emit both as separate components (rejected — violates US1 AS#1 "exactly one component" expectation); fail loud (rejected — too aggressive for the common vendored-crate case).
- **Decision**: Excluded crates (`[workspace].exclude = [...]`) emit main-modules anyway. **Rationale**: the filesystem walker is authoritative; an excluded crate is still a real crate in the source tree (e.g., `mikebom-ebpf` is excluded for build-target reasons but is genuinely a separate publishable artifact). **Alternatives**: skip excluded dirs (rejected — would erase legitimate components and surprise users with `mikebom-ebpf` missing from a mikebom self-scan).
- **Decision**: No binary-path interaction. **Rationale**: cargo binaries don't embed BuildInfo-style metadata, so there is no parallel "binary main-module" emission to dedup with. Cargo source-tree main-module is unconditionally source-tree-derived. **Alternatives**: copy 053 FR-009's source-vs-binary precedence table (rejected — unused; cargo has no binary path that emits a main-module to compete with).
- **Decision**: Generalize the existing milestone-053 generator hooks (`go_main_module` selector → `any_main_module` C40-tag selector) rather than add a parallel cargo-specific code path. **Rationale**: the C40 tag is the universal signal; consumer-side dispatch on ecosystem is a future-cost we should never pay. **Alternatives**: add `cargo_main_module` selectors mirroring 053's `go_main_module` (rejected — duplicative; will cause every future ecosystem milestone to repeat the same pattern; better to fix once with the C40-driven filter).

No further Phase 0 work needed. **Output**: this section + spec Clarifications + reference to `053-go-main-module-edges/research.md`.

## Phase 1: Design & Contracts

### 1. Data model

`data-model.md` (new, this run) — captures:

- **CargoMainModuleEntry**: a `PackageDbEntry` with the following constrained shape:
  - `purl: Purl` — `pkg:cargo/<package.name>@<resolved-version>` per FR-001
  - `name: String` — the `[package].name` value verbatim
  - `version: String` — output of the manifest-resolution function (literal → workspace-inherited → `0.0.0-unknown`)
  - `source: Some("path+file://<absolute-cargo-toml-dir>")` — matching the existing cargo source-classification convention
  - `lifecycle_scope: None` — Runtime-by-default; this milestone doesn't touch lifecycle
  - `sbom_tier: Some("source")` — per FR-006
  - `extra_annotations: vec![mikebom:component-role: "main-module"]` — supplementary C40 signal per FR-004
  - `parent_purl: None` — top-level (so SPDX root-selection picks it via case 1 / case 3 of `build_document::root_id`)
  - `depends: Vec<String>` — direct-dep PURLs (from `[dependencies]` / `[dev-dependencies]` / `[build-dependencies]`, post-scope-filter), per FR-007
  - `licenses: vec![]` — empty per FR-005 (LICENSE detection is #103 follow-up)
- **Workspace-inherited version resolution**: a private helper `resolve_cargo_main_module_version(manifest_path: &Path, package_table: &toml::Value, workspace_lookup: &WorkspaceContext) -> String` returning either the literal version string, the resolved `[workspace.package].version`, or the `0.0.0-unknown` fallback. `WorkspaceContext` is an in-scan-built map of `workspace_root_path → workspace_package_version` populated by an upfront pass over discovered `Cargo.toml` files (one extra parse per workspace root; cheap).
- **Same-PURL dedup output**: a private helper `dedup_main_modules_by_purl(entries: &mut Vec<PackageDbEntry>) -> Vec<DroppedDuplicate>` that mutates the entry vec in-place to retain only the first occurrence per PURL and returns a list of `(purl, dropped_path)` tuples for the caller to log via `tracing::warn!`. Deterministic on the walker's existing alphabetical traversal order.
- **Direct-dep edge**: a `Relationship { from: <cargo-main-module-purl>, to: <dep-target-purl>, relationship_type: DependsOn, provenance: { source: <Cargo.toml-path>, data_type: "cargo-manifest-direct-dep" } }`. Existing edge-emission machinery picks these up unchanged.

No new public type emerges from this milestone. The `SpdxPrimaryPackagePurpose::Application` enum value (introduced by milestone 053 in `mikebom-cli/src/generate/spdx/packages.rs`) is reused unchanged.

### 2. Contracts

`contracts/cargo-main-module-component.md` (new, this run) — captures the per-format placement contract for cargo specifically (parallel to `053-go-main-module-edges/contracts/main-module-component.md`):

- **CycloneDX 1.6**:
  - **Single cargo main-module (single-crate scan)**: appears as `metadata.component` with `type: "application"`, `bom-ref` = its PURL, NOT in top-level `components[]`.
  - **Multiple cargo main-modules (workspace scan)**: synthetic super-root in `metadata.component` (existing 053 polyglot pattern), each cargo main-module appears as a regular entry in `components[]` with C40 `mikebom:component-role: main-module` property; the super-root's `dependencies[]` entry references each main-module's `bom-ref`.
  - **Polyglot scan (cargo + Go + …)**: same as workspace case — synthetic super-root DESCRIBES every per-ecosystem main-module in deterministic order.
- **SPDX 2.3**:
  - Each cargo main-module appears in `packages[]` with `primaryPackagePurpose: "APPLICATION"`.
  - `documentDescribes[]` targets every cargo main-module's SPDXID (one or many — the array is plural).
  - Document-level relationship `SPDXRef-DOCUMENT DESCRIBES <main-module-spdxid>` emitted per main-module.
  - Annotation `mikebom:component-role: main-module` attached on each package via the existing C40 wiring.
- **SPDX 3.0.1**:
  - Each cargo main-module appears as a regular Element/Package with `software_primaryPurpose: "application"`.
  - Document-level `DESCRIBES` relationship targets each main-module element.
  - C40-mapped native field set per the existing v3 wiring.

The contract is identical to 053's pattern, generalized — the difference is the ecosystem-specific PURL prefix (`pkg:cargo/...` vs `pkg:golang/...`) and the workspace-multi-root case being more common for cargo than for Go.

### 3. Quickstart

`quickstart.md` (new, this run) — gives implementers + reviewers a 4-step verification recipe for both single-crate and workspace cases:

**Single-crate (clap-rs):**

1. `git clone --depth 1 https://github.com/clap-rs/clap /tmp/clap-064`
2. `target/debug/mikebom sbom scan --path /tmp/clap-064 --format cyclonedx-json --output /tmp/clap-064.cdx.json --no-deep-hash`
3. `jq '.metadata.component | {name, type, purl}' /tmp/clap-064.cdx.json`
4. **Expect**: `{name: "clap", type: "application", purl: "pkg:cargo/clap@<x.y.z>"}` where `<x.y.z>` matches `git -C /tmp/clap-064 cat-file blob HEAD:Cargo.toml | grep '^version'`.

**Dogfood (mikebom workspace itself):**

1. `target/debug/mikebom sbom scan --path . --format spdx-2.3-json --output /tmp/mikebom.spdx.json --no-deep-hash`
2. `jq '[.packages[] | select(.primaryPackagePurpose == "APPLICATION") | {name, purl: (.externalRefs[]? | select(.referenceType == "purl") | .referenceLocator)}]' /tmp/mikebom.spdx.json`
3. **Expect**: an array of 4 entries — `{name: "mikebom", purl: "pkg:cargo/mikebom@0.1.0-alpha.11"}`, `{name: "mikebom-common", purl: "pkg:cargo/mikebom-common@0.1.0-alpha.11"}`, `{name: "xtask", purl: "pkg:cargo/xtask@0.1.0-alpha.11"}`, plus `{name: "mikebom-ebpf", purl: "pkg:cargo/mikebom-ebpf@<its-version>"}` (the excluded crate exercises FR-003).
4. `documentDescribes` contains exactly 4 SPDXIDs corresponding to those 4 packages.

### 4. Agent context update

Run `.specify/scripts/bash/update-agent-context.sh claude` after this plan is committed — adds milestone 064 entry to `CLAUDE.md`'s Active Technologies list. No new technologies to register (no new crates, no new languages); the script will record `064-cargo-main-module: Existing only — toml = "0.8" (already used), serde/serde_json, tracing, anyhow. **No new crates.**`.

### 5. Re-evaluate Constitution Check

Re-checked above table after Phase 1 design — no new violations introduced. The generalize-053-hooks-to-C40-driven approach actively *strengthens* Principle V compliance (one mechanism for all ecosystems' main-modules going forward) and Principle X transparency (the same-PURL dedup `tracing::warn!` is a deliberate operator-visibility signal mirroring the milestone-061 graph-completeness pattern).

The Phase 1 design materially reduces estimated implementation cost vs. milestone 053 because (a) no `git describe` subprocess to time-out and test, (b) generator-side hooks are generalize-not-add, (c) the `parse_lockfile` path already parses `Cargo.toml` so the main-module data is essentially free.

**Phase 1 outputs**: this section + `data-model.md` (next run, T010 of /speckit.tasks), `contracts/cargo-main-module-component.md` (next run), `quickstart.md` (next run), agent-context update (mechanical).

## Complexity Tracking

*No constitution violations to justify. Section intentionally empty.*
