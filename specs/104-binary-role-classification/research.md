# Research: Binary Role Classification

## R1 — How to classify binaries across the three formats with minimal new code

**Decision**: Use the `object` crate's existing `Object::kind()` method as the primary classifier, supplemented by ELF program-header inspection for the ET_DYN PIE-vs-library ambiguity.

**Rationale**: The `object = "0.36"` crate (already a workspace dependency, pervasively used across `scan_fs/binary/`) exposes a cross-format `ObjectKind` enum (`object::read::ObjectKind` — `Unknown` / `Relocatable` / `Executable` / `Dynamic` / `Core`) that maps almost directly onto the four `BinaryRole` variants the spec calls for. The crate's `Object::kind()` impl for each format reads:

- **Mach-O** (`object-0.36.7/src/read/macho/file.rs:309-317`): `MH_OBJECT` → `Relocatable`, `MH_EXECUTE` → `Executable`, `MH_DYLIB` → `Dynamic`, `MH_CORE` → `Core`, everything else (including `MH_BUNDLE`, `MH_KEXT_BUNDLE`, etc.) → `Unknown`.
- **ELF** (`object-0.36.7/src/read/elf/file.rs:296-305`): `ET_REL` → `Relocatable`, `ET_EXEC` → `Executable`, `ET_DYN` → `Dynamic`, `ET_CORE` → `Core`. The crate has a literal `// TODO: check for DF_1_PIE?` comment at this line — it does not disambiguate PIE executables from shared libraries.
- **PE** (`object-0.36.7/src/read/pe/file.rs:225-234`): `IMAGE_FILE_DLL` bit set → `Dynamic`; `IMAGE_FILE_SYSTEM` bit set → `Unknown`; otherwise → `Executable`. Correct for our purposes — no disambiguation needed.

**ELF disambiguation strategy**: For ELF binaries whose `ObjectKind::Dynamic` could mean either a PIE executable or a shared library, walk the program-header table looking for a `PT_INTERP` segment. PIE executables on every modern Linux distribution carry `PT_INTERP` (the path to the dynamic linker — `/lib64/ld-linux-x86-64.so.2` etc.); shared libraries do not. The `object` crate's `ElfSegments` iterator exposes `p_type` directly, so this is a one-line check after the initial `kind()` call.

The `DF_1_PIE` flag in the `DT_FLAGS_1` dynamic-section entry is the other commonly-cited signal, but it's not as universal — older linkers and statically-built PIE binaries can omit it. `PT_INTERP` presence is the empirically-strongest signal and matches the heuristic used by GNU `file(1)` and `readelf -e`.

**Alternatives considered**:
- *Manual format-specific header parsing in mikebom*: rejected. We'd be re-implementing what `object::ObjectKind` already does correctly for 90%+ of the cases, just to add the 10% ELF PIE disambiguation. Better to lean on the existing crate impl and add only the ELF supplement.
- *Use the `DF_1_PIE` flag exclusively*: rejected for the universality reason above. `PT_INTERP` is the primary check; if the architecture is ELF and `DF_1_PIE` is set but `PT_INTERP` is absent, we still emit Application (safety net — that combo is rare but unambiguous).
- *Shell out to `file(1)`*: rejected per Constitution Principle I (Pure Rust). The `object` crate gives us all the bytes we need.

## R2 — Mach-O MH_BUNDLE handling

**Decision**: Map `ObjectKind::Unknown` to `BinaryRole::Other`, which in turn maps to CDX `type: library` (the historic default for binary-reader components — preserves consumer compatibility for everything that's not clearly an Application or a SharedLibrary), SPDX 2.3 omits `primaryPackagePurpose`, SPDX 3 omits `software_primaryPurpose`.

**Rationale**: `MH_BUNDLE` (loadable plugin modules), `MH_KEXT_BUNDLE` (kernel extensions), and other minority Mach-O filetypes all come through `ObjectKind` as `Unknown`. CycloneDX 1.6 has no dedicated enum value for "loadable plugin module" — the spec's `library` is closest semantically ("a static linkable or dynamically loadable code unit"), and consumers that want finer distinction can read the existing `mikebom:binary-class` annotation (which already carries the format-level signal — `macho`). Distinguishing MH_BUNDLE from "other unknown" inside the classifier would require Mach-O-specific filetype-byte parsing in the new `role.rs` module; per R1's rationale, we lean on the crate's abstraction and accept the slightly coarser bucketing in exchange for not duplicating format parsing.

**Alternatives considered**:
- *Special-case MH_BUNDLE → SharedLibrary*: rejected. Functionally a bundle IS a kind of dynamically loadable library, but the Mach-O filetype `MH_BUNDLE` is distinct from `MH_DYLIB` for a reason (different runtime loading semantics) — collapsing them silently would lose information the existing `mikebom:binary-class` doesn't preserve. Better to be honest about "we can't classify this further" than to falsify a classification.
- *Add a fifth BinaryRole::Plugin variant*: rejected. SPDX 2.3 and SPDX 3 have no equivalent enum value for "plugin" — adding the variant means inventing a per-format mapping rule that doesn't have a standards-native target. Per Constitution Principle V, we don't invent typing where the spec hasn't.

## R3 — `metadata.component.type` in CDX for binary-subject scans

**Decision**: When the scan's `metadata.component` represents a single binary (e.g., scanning a single executable directly via `--path /path/to/binary`), apply the same role classification to the synthetic-root level so the document is internally consistent. For non-binary scan subjects (a directory containing manifests, a workspace with a cargo main-module, etc.), the existing CDX `metadata.component.type: "application"` default is unchanged.

**Rationale**: Today the CDX `metadata.component.type` defaults to `application` for the scan subject, regardless of whether the subject is a directory, a manifest-driven workspace, or a single binary file. That's correct for directory and manifest scans (the scan subject conceptually IS the application being inventoried). But for image scans and synthesized-root cases where the subject is itself a binary, the type should reflect the binary's actual role — otherwise the scan-subject identity diverges from how the same binary appears in `components[]`. The fix is to apply the same `BinaryRole`-to-format-type mapping at the synth-root level when (and only when) the metadata.component is binary-derived.

**Alternatives considered**:
- *Leave `metadata.component.type` always `application`*: rejected. For an image scan where `metadata.component` is the synthetic root `pkg:generic/postgres:16@0.0.0`, the document already says "this metadata.component is an application" which is correct (a container image IS an application bundle). The discrepancy only matters for the niche case of `--path /single/binary` scans — even there, calling the metadata.component `application` is defensible. Marking as N/A and leaving the field untouched is the lower-risk path.
- *Suppress `metadata.component.type` for binary-only scans*: rejected. Removing the field would break consumer expectations and fail conformance checks.

Final call: applies same role to metadata.component only when the metadata.component IS a binary-reader-emitted entity (not for image scans where it's a synthetic root, not for manifest scans where it's auto-derived from the workspace).

## R4 — Goldens regen scope

**Decision**: Regenerate goldens only for fixtures that exercise the binary reader. The fixtures listed in `mikebom-cli/tests/fixtures/golden/cyclonedx/`, `spdx-2.3/`, and `spdx-3/` for ecosystems whose readers don't emit file-level binary components (cargo, gem, golang, maven, npm, pip, rpm, deb, apk) are byte-identical to alpha.34 because none of their components go through `binary/`. Goldens for `polyglot-rpm-binary`, `binaries`, and any image-scan-based fixture may regen.

**Rationale**: The binary-role classification only changes the `type` field on components emitted from `scan_fs/binary/`. Manifest- and lockfile-driven readers (cargo's Cargo.toml/Cargo.lock parsers, npm's package-lock.json walker, etc.) populate the `ResolvedComponent` directly without going through the binary reader; their `binary_role` field stays `None` and emission falls back to the existing per-ecosystem default (which is `library` for all of them today). So most golden files don't change at all.

**Alternatives considered**:
- *Regenerate all goldens*: would create noise in the diff and obscure which behavioral changes are actually new. The targeted regen is the audit-friendly choice.

## R5 — How operators reach the new annotation

**Decision**: Do not introduce any new `mikebom:*` annotation. The `BinaryRole` signal rides exclusively through the standards-native typing slots (CDX `Component.type`, SPDX 2.3 `Package.primaryPackagePurpose`, SPDX 3 `software_Package.software_primaryPurpose`). The existing `mikebom:binary-class` annotation (which carries the format `elf`/`macho`/`pe`, not the role) is preserved unchanged per FR-003.

**Rationale**: Per Constitution Principle V (standards-native fields take precedence), all three target formats have purpose-built fields for the "what kind of artifact is this component" signal — these are textbook native homes. There is no parity-gap carve-out to invoke. Introducing a `mikebom:binary-role` annotation when CDX `type`/SPDX `primaryPackagePurpose`/SPDX 3 `software_primaryPurpose` exist would be the same anti-pattern Principle V cites as its motivating case (the milestone-049 → milestone-052 `mikebom:dev-dependency` removal). The audit requirement is satisfied by the FR-002 mapping table in spec.md.

**Alternatives considered**:
- *Emit `mikebom:binary-role` alongside the native fields as a redundant signal for consumers who don't want to read enum values*: rejected. Redundant annotations are exactly what Principle V forbids.

## R6 — Cross-format parity-extractor extension

**Decision**: Extend the milestone-085 holistic-parity catalog with a new row (proposed: `A13` — component-typing role) covering the three native-type fields. The row is `Directionality::SymmetricEqual` (all three formats must agree component-by-component) with a per-format extractor returning a `BTreeMap<purl, role-enum-string>`.

**Rationale**: SC-003 commits to "no format diverges on role." The existing holistic-parity machinery (`mikebom-cli/tests/holistic_parity.rs`, `mikebom-cli/src/parity/catalog.rs`, `parity/extractors/`) is the right home for this invariant — it's what enforces the same cross-format property for license, supplier, CPE, etc. Adding a row is one entry per file in `extractors/{cdx,spdx2,spdx3}.rs` plus a catalog entry in `parity/catalog.rs` — small, mechanical, and inherits the existing per-ecosystem coverage matrix automatically.

**Alternatives considered**:
- *Write a dedicated `binary_role_parity.rs` integration test*: also doing this for tighter feedback on the specific scenarios (PIE detection, MH_BUNDLE fallback) — but the holistic-parity row is the long-term home for the cross-format invariant. Both, not either.
