# Implementation Plan: Package-DB Follow-Ons (Trifecta)

**Branch**: `040-pkg-db-followups` | **Date**: 2026-04-29 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/040-pkg-db-followups/spec.md`

## Summary

Three sequenced follow-on items that close coverage / hygiene
gaps after milestones 037 / 038 / 039:

1. **US1** — Replace one stale comment in `oci_pull/mod.rs` that
   names `--image-platform` as "deferred to milestone 031.y"
   (shipped in PR #72). One-line edit; the only goal is making
   the error message accurate.
2. **US2** — Extend the milestone-039 apk per-file evidence path
   to also surface the apk-provided per-file SHA-1 (the `Z:` line
   that follows each `R:` in `/lib/apk/db/installed`). Carried
   in `additionalContext` alongside the existing `sha256`,
   matching the way deb's `additionalContext` carries `md5`.
3. **US3** — Mirror the milestone-037+039 deep-hash pattern for
   rpm. mikebom's existing rpm reader already extracts
   `BASENAMES` / `DIRNAMES` / `DIRINDEXES` (used by
   `collect_claimed_paths`); a new exported helper exposes it as
   a per-package file-list map; a new `hash_rpm_package_files`
   mirrors the apk one; a new `is_rpm` branch in
   `scan_fs/mod.rs` wires it up.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited
from milestone 039; no nightly required for user-space-only
work).

**Primary Dependencies**: existing only — `sha2`, `base64`
(already direct), the `rpm` crate (direct dep since milestone
004), `tempfile` (dev-dep). No new top-level deps; the
`no_c_dependencies` regression test continues to pass.

**Storage**: N/A — all state in-process per scan.

**Testing**: `./scripts/pre-pr.sh` (Constitution Pre-PR
Verification gate). Inline tests for the apk SHA-1 parser, rpm
file-list extraction, and the rpm hashing loop. Gated network
smoke tests for end-to-end verification on real images.

**Target Platform**: Linux containers (scan target). The mikebom
CLI runs on Linux + macOS.

**Project Type**: CLI / library (Rust three-crate workspace,
Constitution Principle VI preserved).

**Performance Goals**: per-file SHA-256 cap inherited from prior
milestones (256 MB/file). Rpm packages are typically larger than
apk's, but the deep-hash cost is IO-bound; no architectural
performance change.

**Constraints**: Constitution Principles I, IV, VI; FR-011 (zero
golden drift on existing 27-fixture suite).

**Scale/Scope**: ~325 LOC across 8 files. Single PR with 5
sequenced commits.

## Constitution Check

| Principle | Status |
|---|---|
| I. Pure Rust, Zero C | ✅ no new deps; regression test still green |
| II. eBPF-Only Observation | ✅ N/A — touches scan-path code, not trace-path |
| III. Fail Closed | ✅ N/A — scan-path code |
| IV. Type-Driven Correctness | ✅ Option/Result throughout; reuses existing newtypes; new optional `apk_sha1` field on `FileOccurrence` is additive |
| V. Specification Compliance | ✅ no SBOM schema changes at the CDX/SPDX level; reuses `additionalContext` carrier |
| VI. Three-Crate Architecture | ✅ touches `mikebom-cli` (and one new optional field on `mikebom-common::resolution::FileOccurrence`) |
| VII. Test Isolation | ✅ all new tests run unprivileged |
| VIII. Completeness | ✅ closes a known false-negative (rpm per-file evidence missing) |
| IX. Accuracy | ✅ file content hashes are observed-bytes truth |
| X. Transparency | ✅ apk SHA-1 cross-ref is annotated upstream-provenance metadata |

**No constitution violations.** Complexity Tracking section
intentionally empty.

## Project Structure

### Documentation (this feature)

```text
specs/040-pkg-db-followups/
├── plan.md              # This file
├── research.md          # Phase 0 — 5 design decisions
├── data-model.md        # Phase 1 — signature changes + emission flow
├── quickstart.md        # Phase 1 — post-merge verification recipe
├── checklists/
│   └── requirements.md  # /speckit.specify quality checklist (passing)
└── tasks.md             # Phase 2 output (/speckit.tasks; not created here)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   ├── scan_fs/
│   │   ├── mod.rs                     # ★ is_rpm branch added
│   │   ├── oci_pull/
│   │   │   └── mod.rs                 # ★ stale-comment cleanup (US1)
│   │   └── package_db/
│   │       ├── apk.rs                 # ★ ApkFileEntry + Z: parsing (US2)
│   │       ├── file_hashes.rs         # ★ thread sha1; new hash_rpm_* (US2+US3)
│   │       └── rpm.rs                 # ★ read_file_lists exposed (US3)
└── tests/
    └── oci_registry_smoke.rs          # ★ extend alpine smoke (US2)

mikebom-common/                        # one optional field added
└── src/resolution/                    # ★ FileOccurrence.apk_sha1
mikebom-ebpf/                          # untouched
```

**Structure Decision**: Three-crate workspace preserved. The
`mikebom-common` change is one optional `Option<String>` field
on an existing struct — additive and required because
`FileOccurrence` flows through all three downstream emitters
(CycloneDX, SPDX 2.3, SPDX 3) and the field needs to survive
serialization. Alternative (carry the SHA-1 only in the
emission-site `additionalContext` JSON-string) was considered in
data-model.md and rejected as fragile.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be
> justified**

No constitution violations. Section intentionally empty.
