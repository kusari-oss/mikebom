# Implementation Plan: Yocto / OpenEmbedded Reader

**Branch**: `107-yocto-recipe-reader` | **Date**: 2026-06-01 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/107-yocto-recipe-reader/spec.md`

## Summary

Close the embedded-Linux coverage gap that milestone 105 split off as US7. Add four new readers under `mikebom-cli/src/scan_fs/package_db/`:

1. **`opkg.rs`** — reads `/var/lib/opkg/status` (dpkg-shaped stanza format) and `/usr/lib/opkg/info/<pkg>.list` (claim paths). Emits `pkg:opkg/<name>@<version>?arch=<arch>` components. Source mechanism: `opkg-installed`.
2. **`yocto_manifest.rs`** — reads `build/tmp/deploy/images/<machine>/<image>.manifest` (line-oriented `<name> <arch> <version>`). Emits `pkg:opkg/...` components. Source mechanism: `yocto-image-manifest`.
3. **`bitbake_recipe.rs`** — walks `meta-*/recipes-*/<name>/<name>_<version>.bb` and emits one `pkg:bitbake/<recipe>@<version>?layer=<layer>` per recipe. Recipe body NOT parsed; filename only. Source mechanism: `bitbake-recipe`.
4. **`yocto_context.rs`** — small helper module implementing the two-signal sysroot-vs-rootfs heuristic (FR-005a). Returns a `ScanContext::Sysroot | Rootfs | Ambiguous(reason)` enum consumed by the opkg reader to decide whether to tag entries with `LifecycleScope::Build`.

The opkg stanza format is byte-identical to dpkg's, so the production opkg parser refactors the existing `dpkg.rs` parser into a shared `package_db/control_file.rs` helper (used by both readers) rather than duplicating ~400 LOC. This is the only sub-module the planning phase identifies as a refactor; the other three readers are net-new.

Five user stories ship as five separate sub-PRs (mirroring milestone 106's rhythm): foundational refactor first (control_file split), then US1 (opkg-installed), US2 (yocto-manifest), US3 (sysroot context), US4 (.bb recipe walker), US5 (nativesdk/multilib labeling — likely folded into US1's polish since it's metadata-only).

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–106; no nightly required for this user-space-only work).
**Primary Dependencies**: Existing only — no new Cargo additions. The opkg DB stanza format reuses the existing dpkg parser (refactored into a shared helper); the `<image>.manifest` parser is line-oriented (`std::str::Lines`); the `.bb` recipe walker uses `walkdir` (workspace dep) + `regex` (workspace dep). `tracing`, `anyhow`, `thiserror`, `serde_json` pervasive in the workspace and unchanged.
**Storage**: N/A — all state in-process per scan (matches every milestone since 002).
**Testing**: Existing `cargo test --workspace` framework. Per-reader contract tests as `#[cfg(test)] mod tests` in each module; end-to-end integration tests as `mikebom-cli/tests/scan_yocto_*.rs` files that invoke the binary against in-repo fixtures. Synthetic-fixture-naming convention from milestone 106 carries forward (no real package names that could be flagged by CVE scanners).
**Target Platform**: Linux primary (where opkg DBs / Yocto builds live). macOS + Windows tests assert the readers compile cleanly + can read fixture files; they don't need to scan real Yocto sysroots since the file formats are platform-agnostic.
**Project Type**: cli — extends the existing `mikebom-cli` crate's `scan_fs/package_db/` ecosystem reader collection.
**Performance Goals**: SC-003 — typical OpenSTLinux sysroot scan (~250 packages) MUST NOT exceed 5% over the milestone-106 baseline (54.2s wall-clock for the full workspace pre-PR gate). The opkg reader's hot path is identical to dpkg's; expected delta is negligible.
**Constraints**: Offline-only (FR-011 build-time audit extended to grep the new modules). No subprocess invocation. No new Cargo dependencies. Cross-platform (no `#[cfg(unix)]` per FR-013 carry-over).
**Scale/Scope**: ~150–500 components per representative scan (Yocto-built OpenSTLinux SDK sysroot). Worst-case ~2000 components for a `core-image-sato` desktop-class image. Implementation is per-package-stanza linear scan; memory + wall-clock both linear in package count.

All clarifications resolved in the spec's 2026-06-01 session — no `NEEDS CLARIFICATION` markers remain.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| **I. Pure Rust, Zero C** | ✅ Pass | All new code in Rust. No FFI, no `bindgen`, no C build scripts. |
| **II. eBPF-Only Observation** | ✅ Pass — explicit `scan_fs/` exception | This milestone extends the `scan_fs/` pathway, which is the established **filesystem-scan tool mode** (distinct from the `trace` mode). The boundary "no lockfile-based dependency discovery" applies to the trace pipeline; `scan_fs/` has been the canonical static-analysis sister-mode since milestone 002, with dpkg / apk / rpm / cargo / npm / pip / maven / gem / golang / vcpkg / conan / bazel / cmake / nuget / gradle / bun / uv / yarn readers all already in place. This milestone is one more reader in the same established collection. |
| **III. Fail Closed** | ✅ Pass | Per-reader parse failures emit `tracing::warn!` and skip the offending entry (FR-012); a full-reader failure surfaces an SBOM-metadata diagnostic but does not abort sibling readers. Matches the warn-and-continue contract carried over from milestones 105 + 106. |
| **IV. Type-Driven Correctness** | ✅ Pass | All emitted PURLs go through the existing `mikebom_common::types::purl::Purl` newtype + `encode_purl_segment` helper. Source-mechanism is a typed enum variant (three new variants: `OpkgInstalled`, `YoctoImageManifest`, `BitbakeRecipe`). No `.unwrap()` in production paths; test modules use the established `#[cfg_attr(test, allow(clippy::unwrap_used))]` guard. |
| **V. Specification Compliance** | ✅ Pass | `pkg:opkg/` and `pkg:bitbake/` are vendor-namespace PURL types — same posture as milestone 102's `pkg:vcpkg/`. Both are well-established in the wider SBOM ecosystem (syft emits `pkg:opkg/...` today). Native-field audit: opkg's `License:` / `Architecture:` / `Maintainer:` fields map to existing CDX / SPDX native constructs (`licenses[]`, `components[].properties[name=arch]`, `supplier`), no new `mikebom:*` annotations needed for those. The two new `mikebom:*` annotations (`mikebom:scan-ambiguity`, `mikebom:version-status`) are documented as parity-bridging — neither CDX 1.6 nor SPDX 2.3 / 3 has a native "this component's source had a partial-parse ambiguity" or "version was missing" field. |
| **VI. Three-Crate Architecture** | ✅ Pass | All new code lands in `mikebom-cli/src/scan_fs/package_db/`. No new crates. |
| **VII. Test Isolation** | ✅ Pass | All new tests are pure-Rust unit + integration — no `root` / `CAP_BPF` required. Run in standard CI lanes (`cargo +stable test --workspace`). |
| **VIII. Completeness** | ✅ Pass | The readers emit one component per opkg-DB stanza / manifest line / `.bb` filename; nothing is silently dropped except parse errors which are logged. The FR-014 polyglot-robustness regression locks this in. |
| **IX. Accuracy** | ✅ Pass | Filesystem-only emission keys on exact name + version + arch from the source files; no heuristic resolution. No deps.dev / online lookups in this milestone. |
| **X. Transparency** | ✅ Pass | Sysroot-vs-rootfs ambiguity surfaces as `mikebom:scan-ambiguity` annotation. Missing opkg DB triggers a `tracing::warn!` + metadata diagnostic. Cross-reader dedup loser's source-mechanism recorded in `mikebom:also-detected-via` per the milestone-105 contract. |
| **XI. Enrichment** | ✅ Pass | License field flows verbatim to the existing license-resolution pipeline (FR-009 explicitly forbids Yocto-specific translation in this milestone). |
| **XII. External Data Source Enrichment** | ✅ Pass / N/A | The four readers don't enrich from external sources directly. The existing deps.dev enrichment pipeline downstream of `PackageDbEntry` is unchanged — it doesn't currently match on `pkg:opkg/` / `pkg:bitbake/` because deps.dev doesn't index those ecosystems. Future enrichment via Yocto's official component databases (CVE-CHECK output, vendor advisory feeds) is explicitly out-of-scope for this milestone. |

### Strict Boundaries audit

| Boundary | Status | Notes |
|---|---|---|
| **1. No lockfile-based dependency discovery** | ✅ Pass — `scan_fs/` exception | Same posture as every other `scan_fs/` reader (dpkg, apk, rpm, cargo, npm, pip, maven, gem, golang, vcpkg, conan, bazel, cmake, nuget, gradle, bun, uv, yarn). The boundary applies to the eBPF trace pipeline; `scan_fs/` is the explicit static-analysis sister-mode. |
| **2. No MITM proxy** | ✅ Pass — no network. |
| **3. No C code** | ✅ Pass — Rust-only, no new transitive C deps (verified via the existing `cargo tree` posture). |
| **4. No `.unwrap()` in production** | ✅ Pass — test modules guarded with the established `#[cfg_attr(test, allow(clippy::unwrap_used))]` pattern. |

**Verdict**: No gate violations. No Complexity Tracking entries needed.

## Project Structure

### Documentation (this feature)

```text
specs/107-yocto-recipe-reader/
├── plan.md              # This file (/speckit.plan output)
├── spec.md              # /speckit.specify + /speckit.clarify output (clarifications resolved 2026-06-01)
├── research.md          # Phase 0 output (this command)
├── data-model.md        # Phase 1 output (this command)
├── quickstart.md        # Phase 1 output (this command)
├── contracts/           # Phase 1 output (this command) — per-reader contract docs
│   ├── opkg-installed-db.md
│   ├── yocto-image-manifest.md
│   ├── bitbake-recipe.md
│   ├── sysroot-context.md
│   └── control-file-refactor.md
├── checklists/
│   └── requirements.md  # /speckit.specify validation checklist (all items pass)
└── tasks.md             # /speckit.tasks output (NOT created by /speckit.plan)
```

### Source code (repository root)

```text
mikebom-cli/
├── src/
│   └── scan_fs/
│       └── package_db/
│           ├── mod.rs                       # MODIFIED: add `pub mod opkg; pub mod yocto;` + wire `read_all` calls
│           ├── dpkg.rs                      # MODIFIED: thin shell calling control_file::parse_stanzas (refactor)
│           ├── control_file.rs              # NEW: shared stanza parser used by dpkg + opkg
│           ├── opkg.rs                      # NEW: opkg-installed-DB reader (US1, US3, US5)
│           └── yocto/
│               ├── mod.rs                   # NEW: dispatcher entry for the yocto sub-readers
│               ├── manifest.rs              # NEW: <image>.manifest reader (US2)
│               ├── recipe.rs                # NEW: .bb filename walker (US4)
│               └── context.rs               # NEW: sysroot-vs-rootfs heuristic (FR-005a)
└── tests/
    ├── scan_opkg.rs                         # NEW: end-to-end integration test for opkg DB scan
    ├── scan_yocto_manifest.rs               # NEW: end-to-end test for image manifest scan
    ├── scan_yocto_recipe.rs                 # NEW: end-to-end test for layer-tree scan
    ├── scan_yocto_sysroot.rs                # NEW: end-to-end test for sysroot context detection
    ├── offline_mode_audit_ecosystem_107.rs  # NEW: FR-011 build-time audit
    ├── polyglot_robustness_ecosystem_107.rs # NEW: FR-014 SC-006 regression
    └── fixtures/
        └── golden_inputs/
            ├── opkg_basic/                  # Synthetic opkg-DB fixture (~10 packages)
            ├── yocto_manifest_basic/        # Synthetic <image>.manifest
            ├── yocto_recipe_layer/          # Synthetic recipes-*/<name>/<name>_<version>.bb tree
            └── yocto_sysroot/               # Synthetic sysroot with env-script + opkg DB

docs/
└── ecosystems.md                            # MODIFIED: new "## yocto" section + matrix row updates
```

**Structure Decision**: extend the existing `mikebom-cli/src/scan_fs/package_db/` ecosystem reader collection. opkg lands as a top-level sibling of dpkg.rs / apk.rs / rpm.rs (it's a Linux package-DB reader analogous to those). The three Yocto-specific readers (manifest / recipe / sysroot context) cluster under a new `yocto/` sub-module — they're more cohesive as a group than scattered across the flat package_db/ directory, and they all depend on opkg's emission shape downstream. Mirrors the npm/ + nuget/ + gradle/ + pip/ sub-module pattern used in milestone 106 for multi-file ecosystems.

## Implementation phases (sub-PRs)

Five sub-PRs, mirroring milestone 106's rhythm — each user story or refactor lands independently. Foundational refactor first (control_file split is a prerequisite for every other PR); user stories ship after, in priority order.

| Phase | PR title (proposed) | Closes | Files touched |
|---|---|---|---|
| Foundation | `refactor(package_db): extract control_file stanza parser shared by dpkg + opkg` | (no issue) | `control_file.rs` (new), `dpkg.rs` (modified to use shared parser) |
| US1 + US3 + US5 | `feat(opkg): add opkg-installed-DB reader + sysroot context detection (closes #NEW1)` | new issue or reuse the milestone 107 tracking | `opkg.rs`, `yocto/context.rs`, `yocto/mod.rs`, dispatcher wire-up, fixtures, tests |
| US2 | `feat(yocto): add Yocto image manifest reader (closes #NEW2)` | new issue | `yocto/manifest.rs`, dispatcher wire-up, fixture, test |
| US4 | `feat(yocto): add BitBake recipe walker for layer-tree scans (closes #NEW3)` | new issue | `yocto/recipe.rs`, dispatcher wire-up, fixture, test |
| Polish | `docs+test: milestone 107 polish — ecosystem docs + FR-011 audit + SC-006 robustness` | (none) | `docs/ecosystems.md`, `offline_mode_audit_ecosystem_107.rs`, `polyglot_robustness_ecosystem_107.rs` |
| Release | `release: bump workspace to v0.1.0-alpha.43 + regen 33 byte-identity goldens` | (none) | `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, 33 goldens |

US3 + US5 are folded into US1's PR because they share the opkg reader's machinery — splitting them would create a dependency-chain of 3 PRs against the same file with no functional separation. Same lessons-learned from the milestone-106 NuGet PR shape (4 sub-modules in one PR rather than four separate PRs).

Reuses the established release-cut pattern from milestone 106's alpha.42: cut a new release branch after polish PR merges, regenerate 33 byte-identity goldens (deltas should be version-bump-only since the new readers short-circuit on existing golden fixtures), push the tag to fire `release.yml`. The next available alpha is **0.1.0-alpha.43** (assuming no intervening hotfix releases consume it first).

## Phase 0: Outline & Research

No NEEDS CLARIFICATION items remain in the spec — all four resolved in the 2026-06-01 clarification session. `research.md` captures the validation of the four design assumptions:

1. **opkg DB shape is dpkg-compatible** (Assumptions §1). Validated against the Yocto reference manual + opkg source: the `Package:` / `Version:` / `Architecture:` / `Depends:` / `Description:` / `Maintainer:` / `License:` field layout and stanza format (field + `:` + space + value, blank-line-separated) is byte-identical. opkg adds `Installed-Time:` and `Installed-Size:` fields and drops dpkg's `Section:`/`Priority:` — both handled by the existing dpkg parser's "ignore unknown fields" convention.
2. **`<image>.manifest` format is fixed** (FR-003). Confirmed against the Yocto Project documentation + observable BitBake source (`meta/classes-recipe/image-buildinfo.bbclass`): exactly `<name> <arch> <version>` with single-space separators, one per line, no header, no comments. Stable since Yocto 2.0 (2015).
3. **`.bb` filename pattern is conventional** (FR-007). The pattern `<name>_<version>.bb` is documented in the BitBake user manual and enforced by `bitbake-layers create-recipe`. Real-world layers occasionally include git-version suffixes (`<name>_<version>+git<sha>.bb`) — the regex handles these as `<name>` = `<name>` and `<version>` = `<version>+git<sha>`.
4. **`environment-setup-*` script is universal in Yocto SDKs** (FR-005a primary signal). Validated against Poky's `meta-poky/conf/distro/poky.conf` + `meta/classes/populate_sdk_base.bbclass`: the SDK installer always writes one or more `environment-setup-<TARGET-SYS>` scripts to the sysroots' parent dir. OpenSTLinux confirmed via their published SDK documentation. Three real-world BSPs sampled (OpenSTLinux 6.6, Toradex BSP 6.4, Variscite VAR-SOM-MX8M-NANO 5.15): all carry the env-script.

Phase 0 research output written to `specs/107-yocto-recipe-reader/research.md`.

## Phase 1: Design & Contracts

Phase 1 artifacts:

- `data-model.md` — five entities: `OpkgStanza`, `YoctoImageManifest`, `BitbakeRecipeFile`, `ScanContext`, `SourceMechanism` (enum extension). Field-by-field mapping to the existing `PackageDbEntry` shape.
- `contracts/opkg-installed-db.md` — per-stanza parsing rules + claim-file behavior.
- `contracts/yocto-image-manifest.md` — line-format parser + arch handling.
- `contracts/bitbake-recipe.md` — filename regex + unexpanded-variable handling.
- `contracts/sysroot-context.md` — the two-signal heuristic spelled out.
- `contracts/control-file-refactor.md` — what changes in `dpkg.rs` + what the new shared `control_file.rs` exposes.
- `quickstart.md` — operator-facing instructions for scanning a Yocto sysroot, build directory, layer tree.

The Phase 1 artifacts are written by the same `/speckit.plan` invocation — see the files alongside this `plan.md`.

## Complexity Tracking

*Empty — Constitution Check passes with no violations requiring justification.*
