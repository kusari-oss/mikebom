# Implementation Plan: File-tier component emission for unattributed content

**Branch**: `133-file-tier-components` | **Date**: 2026-06-19 | **Spec**: [spec.md](./spec.md)

## Summary

Three behavioral changes to `mikebom sbom scan` + one Constitution amendment +
one reference doc:

- **US1** (P1, MVP): orphan-mode file-tier emission. Default behavior change.
  Walks rootfs after current readers; emits file-tier components for files
  passing the FR-005 content-shape allowlist AND failing the FR-011 hybrid
  dedupe (path OR hash coverage).
- **US2** (P1): `mikebom:component-path` + `mikebom:layer-digest` properties on
  every package-tier component identified from a rootfs path. Trivy-inspired
  additive metadata; tiny SBOM-size cost; enables forensic queries.
- **US3** (P2): opt-in `--file-inventory=full` mode emits every unique-hash file
  regardless of package coverage; document-level `mikebom:file-inventory-mode`
  annotation.
- **US4** (P3): Constitution amendment (Strict Boundary §5; §VIII clarification;
  1.4.0 → 1.5.0) + `docs/reference/component-tiers.md` + new C-rows in
  `docs/reference/sbom-format-mapping.md`.

The FR-022 measure-first projection (BLOCKING, per the 2026-06-19 Q2
clarification) ran successfully — see `research.md §Orphan projection`. The
projection found 3 276 upper-bound orphans (far above the original 200-800
band) and surfaced three over-emission root causes: PE binaries in
`dotnet/packs/`, JARs in `node_modules/`, and `package.json` files next to
lockfiles. **FR-005 was tightened in-place at plan time per the FR-022
contract**: a path-prefix exclusion list + an adjacent-lockfile check on
manifest classification dropped the projected count to ~245 (within the
SC-001 180-440 band). The same projection tool re-runs at SC-001 verification
time to confirm the actual count.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones
001–132; no nightly required for this user-space-only work).
**Primary Dependencies**: Existing only — `walkdir` is NOT used per
milestone-114's `safe_walk` migration; the new file-tier rootfs walker reuses
`scan_fs::walk::safe_walk` (the std-only walker). `sha2` (existing workspace dep;
SHA-256 hashing — sequential is fine within SC-004's orphan-mode <50 % growth
budget; full mode may need `tokio::task::spawn_blocking` for parallel hashing,
decided at implementation time based on first SC-004 measurement). `clap` for
the new `--file-inventory` flag + `--file-inventory-size-limit` flag (via
existing derive). `serde`/`serde_json` for annotation construction.
`globset = "0.4"` (existing direct dep from milestone-113 `--exclude-path`) for
the FR-005 path-prefix exclusion patterns. `tracing` (existing). `anyhow`/
`thiserror` (existing). **No new Cargo dependencies for US1 / US2 / US4.** US3
MAY add a parallel-hashing dep IF first SC-004 measurement shows the sequential
path exceeds the <300 % growth budget.
**Storage**: N/A — all state in-process per scan; no caches, no persistence.
Mirrors every milestone since 002. The projection tool from FR-022 is a
one-off bash script at `/tmp/mb-133-projection/project.sh`; not shipped.
**Testing**: `cargo +stable test --workspace` + new integration tests at
`mikebom-cli/tests/file_tier_orphan.rs` (US1), `mikebom-cli/tests/file_tier_full.rs`
(US3), `mikebom-cli/tests/package_tier_path_layer.rs` (US2) using synthetic
fixture rootfs trees + the sbom-comparison harness for SC-003 / SC-007. The
existing milestone-094 perf-test infrastructure for SC-004.
**Target Platform**: Linux primary (audit baseline is `linux/amd64`); macOS dev;
Windows experimental per milestones 100 / 101.
**Project Type**: cli (mikebom workspace, `mikebom-cli` crate).
**Performance Goals**: SC-004 — orphan mode <50 % scan-time growth, full mode
<300 % scan-time growth, both relative to milestone-132 post-MVP offline
baseline. SC-001 — orphan-emit count in 180-440 band (the post-projection-
tightening range).
**Constraints**: Pre-PR gate per `CLAUDE.md` (clippy + workspace tests clean).
Standards-native fields first (Constitution Principle V); every new `mikebom:*`
annotation gets a C-row with audit citation. No new C deps (Principle I).
Production code MUST NOT call `.unwrap()` (Principle IV).
**Scale/Scope**: ~5-7 source files modified across `mikebom-cli/src/scan_fs/`
(rootfs walker extension, file-tier emission, content-shape classifier);
~3 files modified across `mikebom-cli/src/cli/scan_cmd.rs` (new flag wiring);
~6 spec/doc edits (`docs/reference/component-tiers.md` new, C-rows added to
`sbom-format-mapping.md`, Constitution amendment). 4 new integration test
files; 1 retired projection script.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| I. Pure Rust, Zero C | ✅ | No new C deps. |
| II. eBPF-Only Observation | N/A | scan_fs path; trace path unchanged. |
| III. Fail Closed | ✅ | Orphan-mode emission failures (unreadable file, hash error) degrade to per-file warnings + a document-level skip-count annotation per Principle X. Scan still emits. |
| IV. Type-Driven Correctness | ✅ | New `FileInventoryMode` enum (typed clap `ValueEnum`); typed `mikebom_common::Sha256Hex` newtype for FR-008 hashes. Tests guarded `#[cfg_attr(test, allow(clippy::unwrap_used))]`. |
| V. Specification Compliance | ✅ | **Standards-native audit**: US1's file-tier components use CDX `components[].type = "file"` (native enum value, CDX 1.6 §components.type) + SPDX 2.3 `Package` (no native File type in 2.3 packages array — emit as Package + `mikebom:component-tier = "file"` parity-bridge) + SPDX 3 `software_File` (native element type per `research.md §SPDX 3 element type`). US2's path/layer properties have NO native CDX/SPDX equivalent for "source path on rootfs" or "OCI layer digest" — they're parity-bridge `mikebom:*` annotations per Principle V's fifth bullet (audit citation inline in FR-012 / FR-013 + new C-rows). US3's `mikebom:file-inventory-mode` is document-level meta — no native field for "this SBOM was emitted with a non-default inventory mode" — parity-bridge. |
| VI. Three-Crate Architecture | ✅ | Only `mikebom-cli` touched. |
| VII. Test Isolation | ✅ | sbom-comparison harness + integration tests run unprivileged. No eBPF involvement. |
| VIII. Completeness | ✅ | THIS milestone is the structural Completeness improvement — surfacing unattributed content as file-tier components. §VIII clarification in the Constitution amendment codifies the new behavior. |
| IX. Accuracy | ✅ | FR-011 hybrid dedupe prevents duplicate emission in default mode; the explicit `--file-inventory=full` override is a clearly-marked exception with a document-level annotation. |
| X. Transparency | ✅ | Document-level annotations for skipped files (oversize, unreadable, special, paths-truncated); `mikebom:file-inventory-mode` for the full-mode override; `mikebom:component-tier` on every file-tier component. |
| XI. Enrichment | ✅ | File-tier components MAY be enriched by future milestones (e.g. license inference from embedded text); no Principle XI violation. |
| XII. External Data Source Enrichment | N/A | File-tier discovery is local-rootfs-only; no external lookups. |

**Gate: PASS** for Phase 0. Re-checked post-Phase 1 — no design changes
introduce new principles violations. Gate remains PASS.

## Project Structure

### Documentation (this feature)

```text
specs/133-file-tier-components/
├── plan.md              # This file (/speckit-plan command output)
├── spec.md              # Feature spec (already exists; written by /speckit-specify; FR-005 tightened during /speckit-plan per FR-022)
├── research.md          # Phase 0 output — orphan-projection results + design decisions
├── data-model.md        # Phase 1 output — file-tier component shape + classifier + dedupe entities
├── quickstart.md        # Phase 1 output — full SC verification protocol
├── contracts/
│   └── component-tiers.md  # Reference doc draft (lives at docs/reference/component-tiers.md when shipped)
├── checklists/
│   └── requirements.md  # Already exists from /speckit-specify
└── tasks.md             # Phase 2 output (/speckit-tasks command; NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
mikebom-cli/src/
├── scan_fs/
│   ├── file_tier/                       # NEW directory
│   │   ├── mod.rs                       # Orphan + full emission entry point
│   │   ├── content_shape.rs             # FR-005 content-shape classifier
│   │   ├── dedupe.rs                    # FR-011 hybrid dedupe (path OR hash)
│   │   └── walker.rs                    # rootfs walker that drives both modes
│   ├── docker_image.rs                  # US2: layer-digest accessor passed through to readers
│   └── package_db/
│       ├── apk.rs                       # US2: emit mikebom:component-path
│       ├── dpkg.rs                      # US2: same
│       ├── rpm.rs                       # US2: same
│       ├── nuget/pe_clr.rs              # US2: same
│       ├── cargo.rs                     # US2: same
│       ├── maven.rs                     # US2: same
│       ├── npm/walk.rs                  # US2: same
│       └── (... every reader that surfaces components from a rootfs path)
└── cli/
    └── scan_cmd.rs                       # US3: --file-inventory + --file-inventory-size-limit flags + dispatch

mikebom-cli/tests/
├── file_tier_orphan.rs                  # US1 integration tests (synthetic fixture trees)
├── file_tier_full.rs                    # US3 integration tests
└── package_tier_path_layer.rs           # US2 integration tests

docs/reference/
├── component-tiers.md                   # NEW reference doc (US4)
└── sbom-format-mapping.md               # US4: new C-rows for mikebom:component-tier, :component-path, :layer-digest, :file-paths, :file-inventory-mode, plus edge-case skip-counters

.specify/memory/
└── constitution.md                      # US4: Strict Boundary §5 + §VIII clarification + 1.4.0 → 1.5.0 SYNC IMPACT REPORT
```

**Structure Decision**: Minimal directory addition (`scan_fs/file_tier/`)
encapsulates the new behavior so existing scan_fs code stays unchanged. US2's
per-reader edits are narrow (one annotation insertion per reader) but touch
many files — implementer should batch them in a single commit per the existing
milestone-001 convention.

## Phase 0 status

`research.md` produced. Contains:

- §Orphan projection — measured 3 276 upper-bound, identified 3 root causes,
  tightened FR-005 in-place, projected ~245 post-tightening (within SC-001 band).
- §SPDX 3 element type — decision: `software_File` (FR-001 deferred decision
  resolved).
- §100 MB file size limit — confirmed reasonable default.
- §Multi-arch image handling — keep existing arch-selection behavior.
- §Nested-archive path semantics — `<archive-path>!/<inner-path>` (JAR-URL
  convention).
- §All Q-level clarifications resolved.

No `NEEDS CLARIFICATION` markers remain.

## Phase 1 status

`data-model.md`, `contracts/component-tiers.md`, `quickstart.md` produced. The
agent-context update script invoked at end of Phase 1.

## Complexity Tracking

No constitution violations. Table omitted.
