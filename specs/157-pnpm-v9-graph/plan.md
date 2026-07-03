# Implementation Plan: pnpm-lock v9 dep-graph — parse `snapshots:` for edges

**Branch**: `157-pnpm-v9-graph` | **Date**: 2026-07-03 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/157-pnpm-v9-graph/spec.md`

## Summary

Fix the pnpm-lock v9 dep-graph regression the team reported against `kusari-sandbox/argo-cd`: mikebom's parser at `mikebom-cli/src/scan_fs/package_db/npm/pnpm_lock.rs:83-91` reads `dependencies` from `packages:` entries only, but pnpm v9 moved dep-graph edges out of `packages:` into a new top-level `snapshots:` section. Post-fix: `parse_pnpm_lock` pre-scans `snapshots:` into a lookup keyed by canonical `name@version` (peer-dep suffix stripped via existing `parse_pnpm_key`), then the existing packages loop pulls edges from that lookup when the entry's own inline `dependencies:` is empty (v9 case) or walks its own inline sub-mappings (v6/v7 case).

Per Q1 clarification (2026-07-03): the milestone also brings pnpm to parity with npm's `package_lock.rs` (milestone 147). Both v9 `snapshots:` entries AND v6/v7 `packages:` entries walk the union of three sub-mappings: `dependencies:` + `peerDependencies:` + `optionalDependencies:`. Constitution Principle VIII (Completeness) drove this scope expansion; the consequence is a monotonic-additive regeneration of pnpm v6/v7 goldens (edges added, none removed or altered).

**Approach**: Single-file diff to `pnpm_lock.rs`. Zero new Cargo dependencies. Zero emitter changes. Reuses `serde_yaml` (workspace) + existing `parse_pnpm_key` helper. Pre-scan is O(N) YAML walk building a `HashMap<String, Vec<String>>`; per-packages-entry lookup is O(1) amortized. Diagnostic emission at `tracing::info!` (every v9 parse) + `tracing::warn!` (v9 with no snapshots — anomalous lockfile).

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–156; no nightly required).
**Primary Dependencies**: Existing only — `serde_yaml` (workspace) for the YAML walk (already parsing pnpm-lock.yaml today), `tracing` (info + warn + debug), `anyhow`/`thiserror` (error propagation), `std::collections::HashMap` for the lookup table. Reuses the existing `parse_pnpm_key` helper at `pnpm_lock.rs:129`. **Zero new Cargo dependencies.**
**Storage**: N/A — lookup table is a per-parse `HashMap<String, Vec<String>>` in-process state; dropped at function return. Mirrors every milestone since 002.
**Testing**: `cargo +stable test --workspace --no-fail-fast` per the mandatory pre-PR gate + the milestone-155 fix memory. Inline `#[cfg(test)] mod tests` in `pnpm_lock.rs` for 8 unit tests + 1 new `mikebom-cli/tests/npm_pnpm_v9_dep_graph.rs` integration test.
**Target Platform**: Cross-platform. YAML reader is std-only; no `#[cfg(unix)]` gates.
**Project Type**: cli (mikebom is a single-binary CLI in the `mikebom-cli` crate of the three-crate workspace).
**Performance Goals**: Argo-cd scan (1834-package pnpm-lock.yaml) completes in <1.5x pre-fix wall clock (empirical measurement at impl-time will confirm the O(N) snapshots pre-scan adds <15% per US1 A7). For argo-cd this is <500ms typical.
**Constraints**: No new Cargo deps (FR-014). No wire-format changes to CDX / SPDX 2.3 / SPDX 3 emitter code (FR-012 + SC-010). No changes to any other reader (FR-011). No changes to any other npm sub-reader (FR-010) beyond compilation-necessitated parity. No new `mikebom:*` annotation keys (FR-013 + SC-010). No reader dispatch order changes (FR-015). No new catalog rows in `docs/reference/sbom-format-mapping.md` (SC-010). No changes to milestone-090 non-pnpm golden fixtures (SC-002 dual-side guard). Monotonic-additive pnpm golden regeneration verified via a new helper (research §R5).
**Scale/Scope**: Single-file primary diff to `mikebom-cli/src/scan_fs/package_db/npm/pnpm_lock.rs` (~+80 LOC net production + ~+120 LOC tests including 8 new unit tests). One new integration test file (~120 LOC). One monotonic-additive helper (~30 LOC, either inline in the integration test or `mikebom-cli/tests/common/monotonic.rs`). CHANGELOG entry. CLAUDE.md auto-update. 3 pnpm golden fixture regenerations (npm.cdx.json / npm.spdx.json / npm.spdx3.json).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Reviewed against `.specify/memory/constitution.md` v1.5.0 (ratified 2026-04-15, last amended 2026-06-20).

| Principle | Status | Notes |
|-----------|--------|-------|
| I — Pure Rust, Zero C | ✅ PASS | Zero new deps. std + workspace deps only. No FFI, no C toolchain. |
| II — eBPF-Only Observation | N/A | Scanner-tier work (lockfile reader); Principle II governs the `mikebom trace` command's dependency-discovery path, not `mikebom sbom scan`. Consistent with every scanner-tier milestone since 002. |
| III — Fail Closed | ✅ PASS | Parser tolerates malformed shapes (missing snapshots, orphan snapshot, non-registry dep values) by emitting empty `depends` + `debug` diagnostic, not crashing. Fail-open matches pre-existing v6/v7 behavior; the SBOM stays emitted with the honest graph mikebom could recover. Anomalous v9 lockfiles fire a `warn` (Constitution Principle X). |
| IV — Type-Driven Correctness | ✅ PASS | `Purl::new` validates every emitted PURL string through the existing milestone-155-era pipeline (unchanged). No `.unwrap()` in production code paths added by this milestone; existing test-only `.unwrap()` remains guarded by `#[cfg_attr(test, allow(clippy::unwrap_used))]`. |
| V — Specification Compliance | ✅ PASS (audit trivial) | See §V audit below. |
| VI — Three-Crate Architecture | ✅ PASS | Zero new crates. All work lives inside `mikebom-cli`. |
| VII — Test Isolation | ✅ PASS | All new tests are pure-logic unit tests + `mikebom-cli/tests/*` integration tests. Run under `cargo test --workspace` without root or `CAP_BPF`. |
| VIII — Completeness | ✅ PASS (directly improves) | This IS a Completeness-driven milestone. Pre-fix, mikebom silently drops peer + optional edges (v6/v7) AND drops ALL non-root edges (v9). Post-fix, the dep graph reflects the full lockfile-authoritative shape. Q1 clarification made explicit that Principle VIII drove the scope expansion beyond the initial bug report. |
| IX — Accuracy | ✅ PASS | No new PURL-emission logic. `parse_pnpm_key` continues to gate all identity + edge-value strings; non-registry sources continue to be dropped with a debug log. No phantom edges: FR-006's orphan-snapshot skip + Q1's defensive de-dup guarantee no synthetic emissions. |
| X — Transparency | ✅ PASS | FR-007's info-level diagnostic surfaces per-parse stats (`packages_count`, `snapshots_count`, `fell_back_to_snapshots`) for grep-friendly CI-log analysis. FR-008's warn-level diagnostic fires on anomalous v9 lockfiles (empty snapshots section). Operators can diagnose graph-completeness issues without re-running with `RUST_LOG=debug`. |
| XI — Enrichment | N/A | This milestone doesn't perform enrichment. |
| XII — External Data Source Enrichment | N/A | No external sources introduced. Parsing is local to the scanned pnpm-lock.yaml. |

### Principle V audit (standards-native fields first)

**No new `mikebom:*` annotation keys introduced** (per FR-013). The change is exclusively to the semantic contents of the `PackageDbEntry.depends` field, which flows through the existing CDX `dependencies[].dependsOn` / SPDX 2.3 `DEPENDS_ON` relationship / SPDX 3 dependency-relationship emitters. All three format emitters already have native constructs for this — CDX 1.6 §4.7 `dependencies[]`, SPDX 2.3 §11 `DEPENDS_ON` relationships, SPDX 3.0.1 `dependsOn` relationship type. **Audit result**: trivially satisfied — nothing new introduced.

**No new CDX properties, SPDX 2.3 annotations, or SPDX 3 annotation elements**. No new PURL types.

### Strict Boundaries audit

| Boundary | Status |
|----------|--------|
| §1 — No lockfile-based dependency discovery | ✅ PASS (implicit exemption) — pnpm-lock.yaml is a manifest-driven scan target for the SOURCE-tier reader tier that mikebom's `sbom scan --path` operates in. Consistent with milestones 002–156's scanner-tier exemption from Principle II. |
| §2 — No MITM proxy | ✅ PASS — no network activity. |
| §3 — No C code | ✅ PASS. |
| §4 — No `.unwrap()` in production | ✅ PASS — new code uses `if let Some(...)` / `unwrap_or_default()` patterns; test-only `.unwrap()` guarded by existing `#[cfg_attr(test, allow(clippy::unwrap_used))]` on the parent module. |
| §5 — No file-tier duplicates in default mode | ✅ PASS — no touch to file-tier walker. The change is exclusively to package-DB emission edges. |

**Result**: ✅ Constitution Check gates pass. No violations to justify. `Complexity Tracking` section empty.

## Project Structure

### Documentation (this feature)

```text
specs/157-pnpm-v9-graph/
├── plan.md                    # This file (/speckit.plan output)
├── spec.md                    # Feature spec (/speckit.specify + /speckit.clarify)
├── research.md                # Phase 0 output (/speckit.plan)
├── data-model.md              # Phase 1 output (/speckit.plan)
├── quickstart.md              # Phase 1 output (/speckit.plan)
├── checklists/
│   └── requirements.md        # /speckit.specify output
└── tasks.md                   # Phase 2 output (/speckit.tasks — NOT this command)
```

Note: no `contracts/` directory — this milestone exposes no new external interfaces. All new APIs are module-internal to `pnpm_lock.rs`. The two data-model.md-declared helpers (`build_snapshots_lookup`, `walk_pnpm_dep_sections`) are private and not part of any external contract surface.

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   └── scan_fs/
│       └── package_db/
│           └── npm/
│               └── pnpm_lock.rs   # PRIMARY DELIVERABLE
│                                  #   - Add: PNPM_DEP_SECTIONS module constant
│                                  #   - Add: build_snapshots_lookup fn (pre-scan)
│                                  #   - Add: walk_pnpm_dep_sections fn (3-section walker)
│                                  #   - Modify: parse_pnpm_lock (call pre-scan at top;
│                                  #     replace lines 83-91 with walker + snapshots fallback)
│                                  #   - Modify: module doc-comment (line 27-30) to
│                                  #     document milestone-157 shape post-implementation
│                                  #   - Add: FR-007 tracing::info! diagnostic
│                                  #   - Add: FR-008 tracing::warn! diagnostic
│                                  #   - Add: 8 new unit tests in mod tests block
│                                  #     (all named pnpm_v6_ or pnpm_v9_ or pnpm_walks_
│                                  #     per SC-007 grep + SC-011)
└── tests/
    ├── npm_pnpm_v9_dep_graph.rs      # NEW — SC-008 integration test
    │                                  # Includes assert_monotonic_additive_pnpm_golden helper
    │                                  # (spec inline; no shared common module needed for a
    │                                  # single-use helper)
    └── fixtures/
        └── golden/
            ├── cyclonedx/npm.cdx.json          # REGENERATED — monotonic-additive edges
            ├── spdx-2.3/npm.spdx.json          # REGENERATED — parallel
            └── spdx-3/npm.spdx3.json           # REGENERATED — parallel
```

**Files intentionally NOT touched**:

- `mikebom-cli/src/generate/cyclonedx/**` — no emitter changes.
- `mikebom-cli/src/generate/spdx/**` — no emitter changes.
- `mikebom-cli/src/parity/extractors/**` — no new parity extractors (no new annotation keys).
- `mikebom-cli/src/scan_fs/package_db/npm/mod.rs` — dispatch order unchanged per FR-015.
- `mikebom-cli/src/scan_fs/package_db/npm/package_lock.rs` — the mirror-parity target; unchanged.
- `mikebom-cli/src/scan_fs/package_db/npm/bun_lock.rs`, `yarn_lock.rs`, `walk.rs`, `enrich.rs`, `jsonc.rs` — untouched.
- `mikebom-cli/src/scan_fs/package_db/*.rs` (non-npm readers) — untouched.
- `mikebom-common/**`, `mikebom-ebpf/**` — other crates untouched.
- `docs/reference/sbom-format-mapping.md` — no catalog row changes.
- Non-pnpm golden fixtures (10 of 11 ecosystems — apk, bazel, cargo, cmake, deb, gem, golang, maven, pip, rpm across 3 formats) — byte-identical per SC-002.

**Files updated at plan phase**:

- `CLAUDE.md` — appended by `.specify/scripts/bash/update-agent-context.sh claude` per Phase 1 §3 below.
- `CHANGELOG.md` — updated during implementation phase per SC-009.

**Structure Decision**: single-crate mikebom-cli additive-only diff. The milestone lives entirely inside the existing three-crate architecture; no new crates, no new modules. The primary deliverable is a ~+80-LOC extension of `pnpm_lock.rs` (production) + ~+120 LOC tests inside its `mod tests` block + a new integration test file. Constitution + SC-010 wire-format guards are trivially satisfied by scope.

## Phase 0 — research.md pointer

Complete. See [research.md](./research.md) — 11 sections (R1 through R11) covering:

- R1 — snapshots pre-scan design (HashMap keyed by canonical name@version)
- R2 — packages-loop consumer (inline wins, snapshots fallback)
- R3 — peer-dep suffix stripping on VALUES via reused `parse_pnpm_key`
- R4 — shared `PNPM_DEP_SECTIONS` constant (SC-011 code anchor)
- R5 — monotonic-additive golden diff helper (SC-002 dual-side guard)
- R6 — diagnostic emissions (FR-007 info + FR-008 warn)
- R7 — lockfileVersion detection (string vs float shape)
- R8 — test inventory (8 unit + 2 integration = 10 total; ≥7 floor cleared)
- R9 — CHANGELOG entry shape
- R10 — verification approach per SC
- R11 — no interaction with milestone-155/156 code paths

**No NEEDS CLARIFICATION markers remain**. Q1 (peer + optional dep handling) locked pre-Phase-0 via `/speckit-clarify` 2026-07-03.

## Phase 1 — data-model.md + quickstart pointers

Complete. See:

- [data-model.md](./data-model.md) — new `PNPM_DEP_SECTIONS` constant, `build_snapshots_lookup` + `walk_pnpm_dep_sections` helper signatures, `parse_pnpm_lock` signature unchanged, wire example showing CDX `dependencies[].dependsOn` growth, non-pnpm golden byte-identity + pnpm golden monotonic-additive regeneration policy, reader dispatch order unchanged. **No** `contracts/` directory (no external contract surface — the helpers are module-internal).
- [quickstart.md](./quickstart.md) — 11 verification scenarios covering all 11 success criteria. Scenario 1 is the manual operator-cadence argo-cd testbed; Scenarios 2–11 are automated pre-PR. Explicitly invokes `--no-fail-fast` per the milestone-155 fix memory.

**Agent context update**: run `.specify/scripts/bash/update-agent-context.sh claude` per Phase 1 §3 of the plan template — appends this milestone's technology row to `CLAUDE.md`'s Active Technologies list. Executed as part of this plan invocation.

## Post-Phase-1 Constitution Check

Re-checked after Phase 1 design. Result: unchanged from pre-Phase-0. All principles remain green; no violations discovered during data-model or research authoring.

Notable: the data-model.md's explicit note that pnpm encodes dev status via `dev: true` boolean (not a `devDependencies:` sub-mapping) is a Constitution IX / X hygiene point — it prevents the SC-011 parity assertion from being over-broad (asserting the two readers walk identical section lists when they legitimately differ by lockfile format).

## Complexity Tracking

*Empty — Constitution Check passes without violations.*

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none) | — | — |

## Next command

- `/speckit.tasks` — generate the per-user-story task breakdown. Given the single-file scope + narrow US1, expect a short task list (~10-12 tasks vs milestone 156's 19).
- Optionally: `/speckit.analyze` — read-only cross-artifact consistency check. Worth a run given the Q1 scope expansion.
