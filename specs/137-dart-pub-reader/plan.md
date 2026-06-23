# Implementation Plan: Dart/Flutter pub ecosystem reader

**Branch**: `137-dart-pub-reader` | **Date**: 2026-06-23 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/137-dart-pub-reader/spec.md`

## Summary

First **language-ecosystem reader** added to mikebom since milestone 122 (Kotlin DSL) — joins cargo, npm, pip, gem, maven, golang, nuget, swift, kotlin, conan, vcpkg, bazel, cmake in the language-reader family. Parses Dart's `pubspec.lock` (YAML) and `pubspec.yaml` (for design-tier scans without a lockfile); emits one main-module component per `pubspec.yaml` (`name:` + `version:` → `pkg:pub/<name>@<version>` per the milestone-064 cargo precedent confirmed in Clarifications Q1+Q2) plus one component per lockfile entry. Source-discriminator handling per FR-003 — hosted gets `pkg:pub/...?repository_url=`, git gets `pkg:pub/...@<sha>?vcs_url=git+...`, path falls back to `pkg:generic/` placeholder, sdk emits as `pkg:pub/<sdk>@0.0.0` per purl-spec canonical example. Direct-dev deps tagged with `mikebom:lifecycle-scope = "development"` per existing language-reader precedent. Integrates into the existing `read_all` dispatcher; `serde_yaml = "0.9"` is already a workspace dep (per `npm/yarn_lock.rs` + `npm/pnpm_lock.rs`); zero new Cargo dependencies.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–136; no nightly required for this user-space-only work).

**Primary Dependencies**: Existing only — `serde`/`serde_yaml = "0.9"` (workspace; already used by `npm/yarn_lock.rs` + `npm/pnpm_lock.rs`), `serde_json` for evidence-annotation construction, `tracing` (warn-and-skip per FR-007), `anyhow`/`thiserror` (error propagation), `mikebom_common::types::purl::Purl` (PURL construction + validation; the `pub` type is purl-spec-blessed). **No new Cargo dependencies.**

**Storage**: N/A — all state is in-process for the duration of a single scan. Mirrors every language-reader since milestone 002.

**Testing**: `cargo +stable test --workspace`. Synthetic-fixture pattern via `tempfile::tempdir()` constructing minimal `pubspec.yaml` + `pubspec.lock` trees. Four new integration test files at `mikebom-cli/tests/dart_*.rs` mirroring the milestone-135/136 OS-reader test families. SC-004 byte-identity preservation guarded by the existing 11-ecosystem golden suite (no Dart project present → those goldens stay unchanged).

**Target Platform**: Cross-platform reader. Same cargo/maven/npm precedent applies — mikebom's host portability is independent of the scanned target's OS. The reader is pure-Rust YAML parsing.

**Project Type**: CLI tool — extends the `mikebom sbom scan` pipeline via the `read_all` dispatcher.

**Performance Goals**: ≤2 ms overhead per lockfile entry on the read path; ≤300 ms for a heavy Flutter app (~150 deps in a typical Material-Design Flutter project). The no-Dart-detected fast path (walker doesn't find any `pubspec.yaml` or `pubspec.lock`) MUST add ≤5 µs per non-Dart scan.

**Constraints**:
- Byte-identical SBOM goldens when no Dart project present (SC-004).
- Zero new Cargo deps (matches the milestone-002/064/066/068/069/070/135/136 OS-and-language-reader posture).
- Per-lockfile YAML parse failures MUST warn-and-skip, not fail the scan (FR-007).
- The `pub` PURL type IS purl-spec-blessed — no informal-type follow-up needed (unlike `brew` in milestone 136 and `yocto` in milestone 128).
- Workspace structure NOT represented in SBOM — one main-module per `pubspec.yaml`, no synthetic workspace-root (per Clarifications Q2).

**Scale/Scope**: Typical Flutter app: 30–80 direct + transitive deps. Heavy app (Material gallery): ~200. Monorepo with 5 members × 50 deps each: ~250 components total. Per-lockfile YAML parse: ~500 µs warm-cache.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Verdict | Justification |
|---|---|---|
| I. Pure Rust, Zero C | ✓ | All new code is user-space Rust; no FFI, no C. YAML parsing via `serde_yaml` (pure-Rust crate already in the workspace per `npm/yarn_lock.rs` precedent). |
| II. eBPF-Only Observation | N/A | This reader processes manifest/lockfile metadata for components ALREADY declared in source-tree files; no new dependency-discovery surface. Matches every language reader (cargo/npm/pip/gem/maven/golang/etc.) — language readers parse source-tree manifests, OS readers parse system-DB files; both are pre-existing "discovery surface" categories. |
| III. Fail Closed | ✓ | A source tree without `pubspec.yaml` AND without `pubspec.lock` is a clean no-op (FR-006), NOT a fail-closed condition. Per-lockfile parse failures warn-and-skip (FR-007). |
| IV. Type-Driven Correctness | ✓ | Uses the existing `Purl` newtype for PURL construction; no stringly-typed identifiers. `description:` field is polymorphic (string for sdk, map for others) — handled via `#[serde(untagged)]` enum in `LockfileDescription`. Production code MUST NOT call `.unwrap()` — error propagation via `Result`. |
| V. Specification Compliance | ✓ | **`pub` IS a purl-spec-defined type** ([purl-spec PURL-TYPES.rst pub-definition.md](https://github.com/package-url/purl-spec/blob/main/types-doc/pub-definition.md)). Path-sourced placeholder uses `pkg:generic/` (well-defined purl-spec type) with `mikebom:source-type = "path"` annotation as the discriminator — the annotation is a PARITY-BRIDGE per Principle V (path identity has no standards-native type-name; `pkg:generic/` + annotation surfaces what the standard doesn't express). The `mikebom:source-type` reuses the existing C1 parity-catalog row (introduced in milestone 002); no new C-row added. Documented in research §R1. |
| VI. Three-Crate Architecture | ✓ | All new code lives in `mikebom-cli`. No new workspace crate. Reader is a peer of `cargo.rs` / `gem.rs` / `maven.rs` / `golang.rs` under `mikebom-cli/src/scan_fs/package_db/`. |
| VII. Test Isolation | ✓ | Synthetic tempfile fixtures only; no host-state dependency. Pure-Rust YAML parsing — runs on any host. |
| VIII. Completeness | ✓ | This feature IS a completeness improvement — eliminates the false-negative gap where every Dart/Flutter dep was invisible to the scan. Mobile + cross-platform projects gain SBOM coverage. |
| IX. Accuracy | ✓ | PURL identity comes directly from the on-disk lockfile / manifest; no heuristic guesses. Dep-name extraction uses the lockfile's `dependencies:` array verbatim. SDK pseudo-deps emit with the purl-spec-blessed `0.0.0` placeholder — explicit per purl-spec example, not a heuristic guess. |
| X. Transparency | ✓ | Per-lockfile parse failures (FR-007) emit `tracing::warn!` with the affected lockfile path. Source-type discriminator surfaces via the standard `mikebom:source-type` evidence property — operators see exactly what discrimination class each dep falls into. No silent drops. |
| XII. External Data Source Enrichment | ✓ | The lockfile + manifest ARE the discovery sources — same posture as cargo/npm/pip/gem/maven. No external enrichment in this feature (license extraction explicitly out of scope per spec). |

**Verdict: PASS.** No violations, no justifications required.

## Project Structure

### Documentation (this feature)

```text
specs/137-dart-pub-reader/
├── plan.md              # This file
├── spec.md              # Feature spec (already written; corrected post-Phase-0)
├── research.md          # Phase 0 output — Principle V audit + pubspec.lock schema + purl-spec pub canonical form
├── data-model.md        # Phase 1 output — PubspecLock + LockfileEntry + PackageDbEntry field mapping
├── quickstart.md        # Phase 1 output — operator-facing walkthrough
├── contracts/           # Phase 1 output — wire-format contracts
│   └── pub-component-purl.md
├── checklists/
│   └── requirements.md  # Spec-quality checklist (already written)
└── tasks.md             # Phase 2 output (via /speckit.tasks)
```

### Source Code (repository root)

```text
mikebom-cli/src/
├── scan_fs/
│   ├── package_db/
│   │   ├── mod.rs                     # MODIFY: register dart in read_all dispatcher
│   │   │                              # (language-reader pattern; no claim_paths)
│   │   ├── dart.rs                    # NEW: pubspec.yaml + pubspec.lock parsing,
│   │   │                              # main-module emission, source-type
│   │   │                              # discrimination, design-tier fallback
│   │   ├── cargo.rs                   # REFERENCE: milestone 064 main-module + workspace pattern
│   │   ├── npm/yarn_lock.rs           # REFERENCE: serde_yaml YAML lockfile parsing precedent
│   │   ├── npm/pnpm_lock.rs           # REFERENCE: same
│   │   └── maven.rs                   # REFERENCE: language-reader source-tree walk pattern
│   └── (no other scan_fs changes — dart is purely additive)
├── generate/cyclonedx/builder.rs       # MODIFY: extend mikebom:evidence-kind enum to
│                                       # include "pubspec-lock" + "pubspec-yaml"
│                                       # (mirrors milestone-135 T002b + 136 T002b pattern)
└── (no changes to other generate/, parity/, common/)

mikebom-cli/tests/
├── dart_flutter_app_baseline.rs       # NEW: US1 — pubspec.yaml + pubspec.lock fixture
├── dart_source_discriminators.rs      # NEW: US2 — hosted + git + path + sdk fixture
├── dart_design_tier.rs                # NEW: US3 — pubspec.yaml only (no lockfile)
└── dart_edge_cases.rs                 # NEW: malformed lockfile + workspace + dev-deps +
                                       # SDK 0.0.0 + self-hosted registry

docs/reference/
└── (no changes — pkg:pub is purl-spec-blessed; no new mikebom:* annotation introduced for
   identity. The `mikebom:source-type` annotation reuses existing C1 row per parity catalog;
   dart components contribute new VALUES (`pub-hosted`/`pub-git`/`pub-path`/`pub-sdk`) to
   that row's value set but do not alter wire shape.)
```

**Structure Decision**: Extends the existing `mikebom-cli/src/scan_fs/package_db/` reader family with a new language-ecosystem reader. New file `dart.rs` is a peer of cargo/npm/pip/gem/maven/golang/nuget/swift/kotlin/conan/vcpkg/bazel/cmake. Integration site is the existing `read_all` dispatcher; no file-claim tracker integration (language readers don't claim binary paths). Test files follow the existing `<reader>_<scenario>.rs` integration-test naming convention. **No new workspace crate per Principle VI; no new Cargo deps; no new annotation in the parity catalog for identity (source-type discriminator reuses existing C1 row).**

## Complexity Tracking

> No Constitution Check violations — no justifications required.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none)    | n/a        | n/a                                  |
