# Implementation Plan: CMake `find_package` + `pkg_check_modules` extraction

**Branch**: `155-cmake-find-package` | **Date**: 2026-07-02 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/155-cmake-find-package/spec.md`

## Summary

Extend `mikebom-cli/src/scan_fs/package_db/cmake.rs` to parse `find_package(<Name> [<Version>])` and `pkg_check_modules` / `pkg_search_module` declarations from `CMakeLists.txt` and `.cmake` files. This closes a source-tree-only C/C++ visibility gap surfaced by an ad-hoc scan of Kamailio (mikebom currently reports 0 identified components on that tree; post-milestone it reports ≥1 at the current depth-1 walker scope per the 2026-07-02 F1 remediation — walker-depth extension to reach Kamailio's remaining 9+ `find_package` calls at depth-2 `cmake/modules/Find*.cmake` is a separate future milestone).

**Approach**: two new pure-function parsers alongside the existing `parse_fetch_block`, invoked from `read()` inside the per-file loop. Emissions carry `mikebom:source-mechanism = "cmake-find-package"` (or `"cmake-pkg-check-modules"`); the production `resolve::deduplicator` pipeline (grouping by `(ecosystem, name, version, parent_purl)`) merges same-PURL entries automatically, and the milestone-109 `extra_annotations` merge at `deduplicator.rs:190-209` preserves the winner's mechanism value while folding non-conflicting loser annotations. Multi-file same-name deduplication picks the highest declared version (Q1 clarification), implemented as a two-pass parser inside `read()`. Zero new Cargo dependencies. Zero emitter changes (CDX / SPDX 2.3 / SPDX 3 all unchanged). No changes to any other reader. No changes to `resolve::deduplicator`. The milestone-105 `scan_fs::dedup` open-enum pipeline (with the `mikebom:also-detected-via` list) is currently `#[allow(dead_code)]` at production emission time; milestone 155 does NOT wire it in — that is a milestone-105-completion follow-up.

This milestone explicitly reverses milestone-102's FR-007 (the non-extraction rule for `find_package`) — the original double-counting concern is resolved by the production `resolve::deduplicator` pipeline's same-PURL grouping (both mechanisms produce `pkg:generic/<name>@<ver>` for compatible names).

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–154; no nightly required for this user-space-only work).
**Primary Dependencies**: Existing only — `regex = "1"` (workspace; already used pervasively in `cmake.rs`), `tracing` (`warn!` / `debug!` diagnostics), `anyhow`/`thiserror` (error propagation), `serde`/`serde_json` (annotation value construction). Reuses `mikebom_common::types::purl::Purl` for PURL construction + validation. **Zero new Cargo dependencies.**
**Storage**: N/A — all state in-process per scan. The two-pass parser accumulates `Vec<FindPackageHit>` + `Vec<PkgCheckHit>` in a local of `read()` for the scan's duration; both are dropped at function return. Mirrors every milestone since 002.
**Testing**: `cargo +stable test --workspace` at the workspace level + inline `#[cfg(test)] mod tests` in `cmake.rs` (following the existing `cmake.rs:476-500` convention). SC-004 integration tests live at `mikebom-cli/tests/`.
**Target Platform**: Cross-platform (no `#[cfg(unix)]` gates on the extraction path); the existing CMake reader is already cross-platform per `cmake.rs:23`.
**Project Type**: cli (mikebom is a single-binary CLI in the `mikebom-cli` crate of the three-crate workspace).
**Performance Goals**: Reader latency budget is well under 1 s per scanned tree for the observed regex complexity; the two new regexes add ≤2 × the existing `collect_into` cost (which runs today unnoticed at `cmake.rs:96`). Kamailio checkout (~2750 CMake-adjacent files) completes in <200 ms.
**Constraints**: No new Cargo deps, no wire-format changes (CDX / SPDX 2.3 / SPDX 3 unchanged), no changes to any other reader, no changes to `resolve::deduplicator` (production dedup pipeline), no changes to `scan_fs::dedup` (milestone-105 open-enum pipeline — currently `#[allow(dead_code)]`), no changes to the milestone-133 file-tier walker. Emitter code paths untouched. Byte-identity guaranteed on scan targets with zero CMake `find_package` / `pkg_check_modules` calls (SC-002).
**Scale/Scope**: Single-file diff to `mikebom-cli/src/scan_fs/package_db/cmake.rs` (~+220 LOC net including tests) + two new integration test files (~200 LOC combined) + one fixture directory (~10 KB) + CHANGELOG entry + CLAUDE.md auto-update. No other production files touched.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Reviewed against `.specify/memory/constitution.md` v1.5.0 (ratified 2026-04-15, last amended 2026-06-20).

| Principle | Status | Notes |
|-----------|--------|-------|
| I — Pure Rust, Zero C | ✅ PASS | No new deps, no C source, no FFI. The new parsers use only `regex` (pure Rust) + `tracing` + std. |
| II — eBPF-Only Observation | N/A | This milestone is scanner-tier work (source-tree readers); Principle II governs the `mikebom trace` command's dependency-discovery path, not `mikebom sbom scan`. The existing CMake reader operates at the same tier under the same exemption. |
| III — Fail Closed | ✅ PASS | Parser tolerates malformed input (`find_package(${VAR})`, commented lines, `find_package_handle_standard_args`) by emitting no component + logging at `debug`. No fallback path; if a file can't be read, the existing `tracing::warn!` continues per `cmake.rs:42-48`. |
| IV — Type-Driven Correctness | ✅ PASS | `Purl::new()` validates every emitted PURL string; invalid PURLs cause the emission to be skipped (not silently included). No `.unwrap()` in production. Test code follows the `#[cfg_attr(test, allow(clippy::unwrap_used))]` convention at `cmake.rs:477`. |
| V — Specification Compliance | ✅ PASS (audited) | See §V audit below. |
| VI — Three-Crate Architecture | ✅ PASS | Zero new crates. All work lives inside `mikebom-cli`. |
| VII — Test Isolation | ✅ PASS | All new tests are pure-logic unit tests + `mikebom-cli/tests/*` integration tests running under `cargo test --workspace` without root or `CAP_BPF`. |
| VIII — Completeness | ✅ PASS | Milestone directly improves Completeness — closes a 0-components gap on a real C/C++ scan target. The file-tier walker's orphan behavior is unchanged, so unattributed content still surfaces per the strict boundary §5 rule. |
| IX — Accuracy | ✅ PASS | PURL emitted only when the regex captures a valid CMake identifier + optional numeric-prefixed version. Modifier keywords (REQUIRED / QUIET / EXACT / etc.) cannot contaminate the version because the version-capture pattern requires a leading digit. The `find_package_handle_standard_args` false-positive is prevented by the `\bfind_package\s*\(` boundary + immediate-open-paren requirement. |
| X — Transparency | ✅ PASS | Emitted entries carry `mikebom:source-mechanism = "cmake-find-package"` (existing parity-bridging annotation, Constitution Principle V-compliant) so consumers can trace which reader produced them. Multi-file version-consolidation emits `tracing::warn!` on mixed-format version strings (research §R3). The new `mikebom:cmake-find-package-name` annotation preserves original-casing for source-fidelity traceability (research §R2 + contracts). |
| XI — Enrichment | N/A | This milestone does not perform enrichment. It surfaces declared deps; downstream `deps.dev` / license enrichment continues to work uniformly per the existing infrastructure. |
| XII — External Data Source Enrichment | N/A | No external sources introduced. Parsing is local to the scanned CMake files. |

### Principle V audit (standards-native fields first)

Per Constitution Principle V, every new `mikebom:*` property MUST first audit each target format for a native construct carrying the same semantic:

| Semantic | CDX 1.6 native | SPDX 2.3 native | SPDX 3.0.1 native | Decision |
|----------|----------------|-----------------|-------------------|----------|
| "which reader emitted this component" | none | none | none | `mikebom:source-mechanism` — parity-bridging annotation, existing key. Milestone 155 extends the open-enum value space with two additive values; no new key added. See contracts/mikebom-source-mechanism.md. |
| "original casing of a name we lowercased for the PURL" | none — `component.name` is a single normalized string | none — `Package.name` is a single normalized string | none — `Package.name` is a single normalized string | `mikebom:cmake-find-package-name` — new parity-bridging annotation. See contracts/mikebom-cmake-find-package-name.md for the full audit trail. |

**Catalog documentation deferral** per FR-015 + SC-007: `docs/reference/sbom-format-mapping.md` is NOT updated in this milestone; the catalog row addition is deferred to a follow-up docs-refresh milestone, matching prior additive-annotation milestone precedent (e.g., milestone 105's `mikebom:source-mechanism` catalog row landed in a follow-up docs-refresh).

**Reviewer guidance**: this deferral is acceptable per the historical precedent AND per FR-015's explicit accommodation. If the reviewer wants the catalog row inline with this milestone, that's a scope expansion — 155 as written accepts the follow-up-catalog posture.

### Strict Boundaries audit

| Boundary | Status |
|----------|--------|
| §1 — No lockfile-based dependency discovery | ✅ PASS — CMake `find_package` declarations are manifest-declared package intents, not lockfile entries. `mikebom sbom scan` (the source-tree scanner) is exempt from Principle II per the existing scanner-tier scope; the strict boundary here is about the `mikebom trace` eBPF path, which is untouched. |
| §2 — No MITM proxy | ✅ PASS — no network activity introduced. |
| §3 — No C code | ✅ PASS — no C source, no `libbpf`, no C toolchain. |
| §4 — No `.unwrap()` in production | ✅ PASS — new parsers use `Purl::new()` + `if let Ok(...)` pattern; test-module `.unwrap()` continues to be guarded by the existing `#[cfg_attr(test, allow(clippy::unwrap_used))]` on `mod tests` at `cmake.rs:477`. |
| §5 — No file-tier duplicates in default mode | ✅ PASS — this milestone does NOT touch the file-tier walker. File-tier orphan behavior for the Kamailio scan is unchanged. Milestone-155 emissions are package-tier components (source-tier subtype), and per milestone 133 FR-011's hybrid dedupe, file-tier components covered by package-tier `evidence.occurrences[].location` are suppressed automatically — so the identified `find_package` deps' source files will suppress overlapping file-tier orphans, tightening the orphan set as an incidental benefit. |

**Result**: ✅ Constitution Check gates pass. No violations to justify. `Complexity Tracking` section is empty (below).

## Project Structure

### Documentation (this feature)

```text
specs/155-cmake-find-package/
├── plan.md                            # This file (/speckit.plan output)
├── spec.md                            # Feature spec (/speckit.specify + /speckit.clarify)
├── research.md                        # Phase 0 output (/speckit.plan)
├── data-model.md                      # Phase 1 output (/speckit.plan)
├── quickstart.md                      # Phase 1 output (/speckit.plan)
├── contracts/                         # Phase 1 output (/speckit.plan)
│   ├── mikebom-source-mechanism.md    # Open-enum extension contract
│   └── mikebom-cmake-find-package-name.md  # New annotation key contract
├── checklists/
│   └── requirements.md                # /speckit.specify output
└── tasks.md                           # Phase 2 output (/speckit.tasks — NOT this command)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   └── scan_fs/
│       └── package_db/
│           └── cmake.rs               # PRIMARY DELIVERABLE
│                                      #   - Add: parse_find_package_calls()
│                                      #   - Add: parse_pkg_check_modules_calls()
│                                      #   - Add: emit_find_package_entries()
│                                      #   - Add: emit_pkg_check_module_entries()
│                                      #   - Add: pick_highest_version() helper
│                                      #   - Add: FindPackageHit + PkgCheckHit structs
│                                      #   - Modify: read() to invoke the new parsers
│                                      #     (two-pass structure — hits accumulate,
│                                      #     emit in a second pass after all files walked)
│                                      #   - Modify: module-level doc comment (remove
│                                      #     the milestone-102 FR-007 refusal para)
│                                      #   - Modify: build_cmake_entry doc comment
│                                      #     (add the 2 new SourceMechanism values)
│                                      #   - Add: 10 new unit tests inside
│                                      #     #[cfg(test)] mod tests block
│                                      #   - Add: 1 regression test locking
│                                      #     collect_find_package_targets unchanged
│                                      #   Preserve: existing FetchContent /
│                                      #     ExternalProject / vendored paths unchanged
│                                      #   Preserve: existing collect_find_package_targets
│                                      #     helper unchanged (locked by regression test)
└── tests/
    ├── cmake_find_package_kamailio_shape_integration.rs   # NEW — SC-004 integration
    ├── cmake_find_package_dedup_integration.rs            # NEW — SC-003 same-PURL cross-mechanism
    └── fixtures/
        └── cmake-find-package/
            └── kamailio-shape/
                ├── CMakeLists.txt         # NEW — top-level find_package(OpenSSL 1.1.0)
                ├── cmake/
                │   ├── defs.cmake          # NEW — 3 find_package calls
                │   └── modules/
                │       ├── FindLibev.cmake         # NEW — find_package(Libev ...)
                │       ├── FindNETSNMP.cmake       # NEW — find_package(NETSNMP ...)
                │       └── FindMariaDB.cmake       # NEW — find_package + pkg_check
                └── src/main.c                       # NEW — placeholder so the tree
                                                     #        has a plausible shape
```

**Files intentionally NOT touched**:

- `mikebom-cli/src/generate/cyclonedx/` — no emitter changes.
- `mikebom-cli/src/generate/spdx/` — no emitter changes.
- `mikebom-cli/src/scan_fs/package_db/*.rs` (except `cmake.rs`) — no cross-reader changes.
- `mikebom-cli/src/scan_fs/mod.rs` — reader dispatcher; no changes required. Reader outputs flow through the existing `PackageDbEntry` → `ResolvedComponent` pipeline unchanged.
- `mikebom-cli/src/resolve/deduplicator.rs` — production dedup pipeline (grouping by ecosystem+name+version+parent_purl); no changes required. Same-PURL merging + milestone-109 `extra_annotations` folding handle the milestone-155 emissions automatically.
- `mikebom-cli/src/scan_fs/dedup.rs` — milestone-105 open-enum pipeline (currently `#[allow(dead_code)]`); no changes required. When a future milestone-105-completion follow-up wires this in, its `SourceMechanism` closed enum will need extension with `CmakeFindPackage` + `CmakePkgCheckModules` variants — but that expansion is out of milestone-155 scope.
- `mikebom-common/` — no shared type changes; the `PackageDbEntry` struct in `mikebom-cli/src/scan_fs/package_db/mod.rs` gets no new fields.
- `mikebom-ebpf/` — untouched (this milestone is user-space-only).
- `docs/reference/sbom-format-mapping.md` — catalog row addition deferred per FR-015 + SC-007.

**Files updated at plan phase**:

- `CLAUDE.md` — appended by `.specify/scripts/bash/update-agent-context.sh claude` per §Phase 1 pointer below.
- `CHANGELOG.md` — updated during implementation phase per SC-008.

**Structure Decision**: single-crate mikebom-cli additive-only diff. The milestone lives entirely inside the existing three-crate architecture; no new crates, no new modules, no cross-crate refactor. The primary deliverable is a ~+220-LOC extension of `cmake.rs` following the module's existing pure-function pattern (regex-in-content + PackageDbEntry-out).

## Phase 0 — research.md pointer

Complete. See [research.md](./research.md) — 11 sections (R1 through R11) covering integration site, regex patterns + false-positive filters, version comparison, SourceMechanism enum values, evidence.source_file_paths merging, test inventory (10 unit + 1 regression + 1 integration = 12 total; ≥8 floor easily cleared), CHANGELOG entry shape, verification approach, orthogonality with the existing `collect_find_package_targets` helper, `evidence_kind`/`sbom_tier` defaults.

**No NEEDS CLARIFICATION markers remain**; both spec-clarifications (Q1 highest-version-wins, Q2 no build-tool denylist) locked before Phase 0.

## Phase 1 — data-model.md + contracts + quickstart pointers

Complete. See:

- [data-model.md](./data-model.md) — internal `FindPackageHit` + `PkgCheckHit` types, `PackageDbEntry` field-population table for both emission classes, `extra_annotations` shape, wire examples in all three target formats.
- [contracts/mikebom-source-mechanism.md](./contracts/mikebom-source-mechanism.md) — open-enum extension contract, consumer + provider guarantees.
- [contracts/mikebom-cmake-find-package-name.md](./contracts/mikebom-cmake-find-package-name.md) — new annotation key contract, Principle V audit trail, conditional-emission rules.
- [quickstart.md](./quickstart.md) — 8 verification scenarios covering all 8 success criteria. Scenario 1 is the manual operator-cadence Kamailio testbed; scenarios 2–8 are automated pre-PR.

**Agent context update**: run `.specify/scripts/bash/update-agent-context.sh claude` (per Phase 1 §3 of the plan template) — appends this milestone's technology row to `CLAUDE.md`'s Active Technologies list. Executed as part of this plan invocation.

## Post-Phase-1 Constitution Check

Re-checked after Phase 1 design (per plan template §Constitution Check §GATE). Result: unchanged from the pre-Phase-0 check. All principles remain green; no violations discovered during data-model or contract authoring. The parity-bridging annotation `mikebom:cmake-find-package-name` was audited fully in contracts/mikebom-cmake-find-package-name.md per Principle V's normative requirement.

## Complexity Tracking

*Empty — Constitution Check passes without violations.*

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none) | — | — |

## Next command

- `/speckit.tasks` — generate the per-user-story task breakdown (see spec §User Scenarios & Testing for US1 P1 + US2 P2).
- Optionally: `/speckit.analyze` — read-only cross-artifact consistency check across spec.md + plan.md + tasks.md before `/speckit.implement`.
