# Implementation Plan: Binary Role Classification (Application vs Library) in Emitted SBOMs

**Branch**: `104-binary-role-classification` | **Date**: 2026-05-24 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/104-binary-role-classification/spec.md`

## Summary

Currently every binary discovered by the file-level binary reader (`mikebom-cli/src/scan_fs/binary/`) ends up emitted with `type: "library"` in CycloneDX (`generate/cyclonedx/builder.rs:577`), and with `primaryPackagePurpose` absent in SPDX 2.3 (`generate/spdx/packages.rs:509-514`). This mis-types executables like `/bin/ls` as libraries — the reporter's "inversion feel" was real: a consumer reads `type: library` on `ls` as "ls is something other things link into," which is the inverse of how an executable actually relates to its dynamic libraries.

This milestone classifies every binary-reader-discovered component into one of four roles (`Application`, `SharedLibrary`, `Object`, `Other`) by reading the source file's format header — Mach-O `mh_filetype` (`MH_EXECUTE`/`MH_DYLIB`/`MH_BUNDLE`/`MH_OBJECT`), ELF `e_type` (`ET_EXEC`/`ET_DYN`/`ET_REL`) disambiguated by `PT_INTERP` and `DT_SONAME`, and PE `IMAGE_FILE_HEADER.Characteristics` (`IMAGE_FILE_DLL` bit). The role is preserved on `ResolvedComponent` via a new optional `binary_role: Option<BinaryRole>` field, then mapped at emission time to each format's native enum value (CDX `type`, SPDX 2.3 `primaryPackagePurpose`, SPDX 3 `software_primaryPurpose`).

No new crate dependencies — the `object = "0.36"` workspace dep already exposes all three formats' filetype/characteristic fields. Diff scope per SC-008 below: ~5 modified production files, 1 new role-detection module, 6 unit/integration tests, and goldens regen on file-level fixtures only (manifest-driven cargo/gem/npm/etc. fixtures are byte-identical because they don't go through the binary reader).

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–103; no nightly required).
**Primary Dependencies**: Existing only — `object = "0.36"` (workspace; already used pervasively by `scan_fs/binary/`), `serde`/`serde_json`, `tracing`, `anyhow`. **Zero new Cargo dependencies.**
**Storage**: N/A — classification is a pure function of the binary's first 1KB of header bytes; no persistence, no cache.
**Testing**: Standard `cargo test`. Unit tests for the four-way classifier next to the role-detection module (`scan_fs/binary/role.rs::tests`). Integration tests in `mikebom-cli/tests/` covering: cross-format role parity (`binary_role_parity.rs`), ELF PIE-vs-library disambiguation (small synthetic ELF fixtures), Mach-O bundle fallback (vendored or generated at test time), and the existing holistic-parity suite extended with a new catalog row.
**Target Platform**: Cross-platform — the role classifier reads bytes, not OS APIs, so it works on macOS/Linux/Windows hosts identically. (The binary reader itself is already cross-platform per milestone 100.)
**Project Type**: Single-crate workspace extension. Net addition: 1 new module (`scan_fs/binary/role.rs`), 5 modified files (`scan_fs/binary/scan.rs`, `scan_fs/binary/entry.rs`, `mikebom-common/src/resolution.rs`, `generate/cyclonedx/builder.rs`, `generate/spdx/packages.rs`, `generate/spdx/v3_packages.rs`), 2 new integration tests, 1 modified doc file.
**Performance Goals**: <1ms per binary classification. The role read is `object`-crate header parsing only — same code path as the existing `BinaryScan` construction; we just capture one additional field. No measurable scan-time impact.
**Constraints**: Diff scope per SC-008 — ≤6 modified production files, ≤2 new integration tests, no new fixtures (use existing or generate at test time), goldens regen on file-level fixtures only.
**Scale/Scope**: ~120-line `role.rs` (four-way classifier + ELF disambiguation helper + tests), ~30-line net additions across the 5 modified files, ~80-line integration test. Total: ~260 LOC net, plus regenerated goldens.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Constitution version: **1.4.0**.

| Principle | Compliance |
|---|---|
| I. Pure Rust, Zero C | ✅ PASS (no new crates; `object`/`serde`/`tracing`/`anyhow` already in tree) |
| II. eBPF-Only Observation | N/A (this is filesystem-scan enrichment, not runtime discovery — same posture as every prior `scan_fs/` milestone) |
| III. Fail Closed | ✅ PASS (header-parse failures fall through to `BinaryRole::Other` with a `tracing::warn!` per FR-004; no silent gap-fill, no panic, no `.unwrap()`) |
| IV. Type-Driven Correctness | ✅ PASS (new `BinaryRole` enum in `mikebom-common` with named variants; no raw-string typing; production code uses `match` not `unwrap` on the role field) |
| V. Specification Compliance | ✅ PASS — **standards-native audit cited in FR-002**: CDX 1.6 §4.2 defines `Component.type` with `application` and `library` as enum members; SPDX 2.3 §7.24 defines `Package.primaryPackagePurpose` with `APPLICATION` and `LIBRARY`; SPDX 3.0.1 defines `software_Package.software_primaryPurpose` with the same vocabulary. All three native fields exist in their respective specs and are the right home for the role signal. **No new `mikebom:*` annotation is introduced by this milestone** — the existing `mikebom:binary-class` annotation (which carries the format `elf`/`macho`/`pe`, not the role) is preserved unchanged per FR-003. |
| VI. Three-Crate Architecture | ✅ PASS (`BinaryRole` enum lives in `mikebom-common/src/resolution.rs` alongside the other shared component-typing enums like `LifecycleScope`; the reader writes the enum from `mikebom-cli/src/scan_fs/binary/`; the emitters read it from `mikebom-cli/src/generate/`) |
| VII. Test Isolation | ✅ PASS (unit tests + integration tests run under `cargo test --workspace`; no eBPF privileges, no root, no network) |
| VIII. Completeness | ✅ PASS (no components added or removed by this feature; existing component set unchanged) |
| IX. Accuracy | ✅ PASS (classification is deterministic per FR-009; the `Other` fallback for genuinely ambiguous inputs is documented at FR-004 and tracing-logged so operators can audit) |
| X. Transparency | ✅ PASS (the spec-native fields the feature populates ARE the standards' transparency surface for component typing; per FR-004 the fallback rule applied to ambiguous binaries is tracing-logged with the component PURL) |
| XI. Enrichment | ✅ PASS (this is metadata-completeness improvement — replacing a default-bucket value with the spec-native true value) |
| XII. External Data Source Enrichment | N/A (no external data sources — classification reads the binary file's own header bytes) |

**Result**: All gates PASS. No complexity-tracking entries required.

## Project Structure

### Documentation (this feature)

```text
specs/104-binary-role-classification/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output (this command)
├── data-model.md        # Phase 1 output (this command)
├── quickstart.md        # Phase 1 output (this command)
├── contracts/           # Phase 1 output (this command)
│   └── binary-role-cross-format-mapping.md
├── checklists/
│   └── requirements.md  # Spec-quality checklist (already written)
├── spec.md              # Feature spec (already written)
└── tasks.md             # Phase 2 output (/speckit.tasks command — NOT created here)
```

### Source Code (repository root)

```text
mikebom-common/src/
└── resolution.rs                              # ADD `BinaryRole` enum + field on ResolvedComponent

mikebom-cli/src/scan_fs/binary/
├── role.rs                                    # NEW — four-way classifier (Mach-O / ELF / PE)
├── scan.rs                                    # ADD `binary_role: BinaryRole` field on BinaryScan
├── entry.rs                                   # PROPAGATE role from BinaryScan → PackageDbEntry → ResolvedComponent
├── macho.rs                                   # (read-only — re-used filetype byte from header parse)
├── elf.rs                                     # (read-only — re-used e_type + program-header + dynamic-section parses)
└── mod.rs                                     # (re-exports for `role`)

mikebom-cli/src/generate/
├── cyclonedx/builder.rs                       # SWAP hardcoded `"library"` for role-aware mapping
├── spdx/packages.rs                           # EXTEND `primary_package_purpose` derivation
└── spdx/v3_packages.rs                        # EXTEND SPDX 3 equivalent

mikebom-cli/tests/
├── binary_role_parity.rs                      # NEW — cross-format role-typing parity test
└── binary_role_disambiguation.rs              # NEW — PIE-vs-library, MH_BUNDLE, ET_REL edge cases

docs/reference/
└── sbom-format-mapping.md                     # ADD new row for the binary-role mapping table
```

**Structure Decision**: Standard single-workspace extension. The role enum lives in `mikebom-common` (alongside `LifecycleScope`, the closest existing analog — also a four-valued enum classifying a component along a single axis); the reader populates it; the three format-specific emitters consume it. The new `scan_fs/binary/role.rs` module isolates the format-detection logic so the existing `macho.rs` / `elf.rs` / `entry.rs` modules don't bloat. The integration tests live next to the existing `mikebom-cli/tests/` files following the milestone-100 / milestone-098 convention.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No constitution-check violations. Complexity-tracking table omitted.

## Phase 0 output

See [research.md](./research.md).

## Phase 1 outputs

- [data-model.md](./data-model.md) — `BinaryRole` enum shape, `ResolvedComponent` extension, format-specific enum mapping table
- [quickstart.md](./quickstart.md) — manual reproduction walkthrough (scan `/bin/ls`, inspect the type field, verify the new behavior)
- [contracts/binary-role-cross-format-mapping.md](./contracts/binary-role-cross-format-mapping.md) — the docs-reference-quality cross-format role-mapping table; this is the contract surface the new sbom-format-mapping.md row points at

## Phase 2 output

Not produced by `/speckit.plan`. Run `/speckit.tasks` to materialize `tasks.md`.

## Post-Phase-1 Constitution re-check

After completing Phase 1 (data-model.md, contracts/, quickstart.md), re-evaluating the gates against the now-concrete design:

- **Principle I (Pure Rust)**: ✅ unchanged — design adds 1 new module + 5 file edits, all Rust, no FFI introduced.
- **Principle IV (Type-Driven)**: ✅ confirmed — `BinaryRole` is a proper enum (4 named variants, no string-typing), serde-derived for envelope-correctness, exposed on `ResolvedComponent` as `Option<BinaryRole>` (not raw `Option<String>`). Production code uses `match` to dispatch the format-native mapping.
- **Principle V (Spec Compliance)**: ✅ confirmed — the contracts/binary-role-cross-format-mapping.md table maps each role directly to an existing spec-defined enum value in each of CDX 1.6, SPDX 2.3, and SPDX 3.0.1. No `mikebom:binary-role` annotation introduced. The existing `mikebom:binary-class` annotation (carrying format `elf`/`macho`/`pe`, not role) is preserved as a separate signal that fills a different gap — its non-redundancy with the new native role field is explicit (format ≠ role).
- **Principle VI (Three-Crate)**: ✅ unchanged — `BinaryRole` enum joins `LifecycleScope` and friends in `mikebom-common/src/resolution.rs`; the workspace stays three crates.
- **Principle VII (Test Isolation)**: ✅ confirmed — both new integration tests (`binary_role_parity.rs`, `binary_role_disambiguation.rs`) run under standard `cargo test --workspace`. The synthetic fixtures are built in-test via the `object` crate's writer support (no privileges, no vendored real binaries).
- **Principle X (Transparency)**: ✅ confirmed — the per-component fallback rule applied to ambiguous binaries is `tracing::info!`-logged at scan time, so operators have an audit trail.

**Result**: All gates remain PASS post-design. No complexity-tracking entries required.
