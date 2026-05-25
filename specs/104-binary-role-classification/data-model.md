# Data Model: Binary Role Classification

## New enum: `BinaryRole`

**Location**: `mikebom-common/src/resolution.rs` (alongside the existing `LifecycleScope`, `RelationshipType`, etc.).

```rust
/// Milestone 104 — role classification for binary-reader-discovered
/// components. Derived from the source file's format header. Maps
/// at emission time to the format-native component-type slot in each
/// of CycloneDX, SPDX 2.3, and SPDX 3 per the table below.
///
/// `None` on `ResolvedComponent.binary_role` means the component did
/// not come from the binary reader (manifest- and lockfile-driven
/// readers leave the field unset) — emitters fall back to the
/// per-ecosystem default (today: CDX `library`, SPDX 2.3 omitted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryRole {
    /// Executable program — Mach-O `MH_EXECUTE`, ELF `ET_EXEC`, ELF
    /// `ET_DYN` with `PT_INTERP` present (PIE executables), PE
    /// without `IMAGE_FILE_DLL` characteristic.
    Application,

    /// Dynamically loadable code unit — Mach-O `MH_DYLIB`, ELF
    /// `ET_DYN` without `PT_INTERP` (canonical shared object), PE
    /// with `IMAGE_FILE_DLL` characteristic.
    SharedLibrary,

    /// Relocatable object file (.o, intermediate build artifact) —
    /// Mach-O `MH_OBJECT`, ELF `ET_REL`.
    Object,

    /// Format-specific bucket that doesn't map cleanly to
    /// Application / SharedLibrary / Object: Mach-O `MH_BUNDLE`,
    /// `MH_KEXT_BUNDLE`, etc.; ELF `ET_CORE`; PE with
    /// `IMAGE_FILE_SYSTEM` characteristic; corrupted or
    /// unrecognized headers.
    Other,
}
```

Variant choice matches the four-way distinction the spec calls for. `Other` is the catch-all bucket per R2 — collapses bundles, kernel extensions, core dumps, and unparseable bytes into one fallback rather than fanning out into format-specific micro-variants.

## `ResolvedComponent` extension

Single new optional field. All existing fields unchanged. Same shape as the previous milestone-064 `binary_class` extension.

```rust
pub struct ResolvedComponent {
    // ... existing fields unchanged ...

    /// Milestone 104 — role classification when the component was
    /// emitted by the binary reader. `None` for components from
    /// manifest- and lockfile-driven readers. See `BinaryRole` docs
    /// for the format-to-role mapping.
    pub binary_role: Option<BinaryRole>,
}
```

All `ResolvedComponent { .. }` struct-literal sites in production + tests gain `binary_role: None,` defaults — same mechanical update pattern as previous milestone-061 / 064 / 071 / 077 / 080 / 081 / 096 field additions. The binary-reader entry-construction at `mikebom-cli/src/scan_fs/binary/entry.rs` is the only site that sets it to `Some(_)`.

## Format-to-role classification table

The `BinaryRole` value is derived from `object::ObjectKind` (from `object` crate 0.36) plus a single ELF supplemental check:

| Source format       | Header signal                                       | `object::ObjectKind` | mikebom `BinaryRole` |
|---------------------|-----------------------------------------------------|----------------------|----------------------|
| Mach-O `MH_EXECUTE` | filetype = `0x2`                                    | `Executable`         | `Application`        |
| Mach-O `MH_DYLIB`   | filetype = `0x6`                                    | `Dynamic`            | `SharedLibrary`      |
| Mach-O `MH_OBJECT`  | filetype = `0x1`                                    | `Relocatable`        | `Object`             |
| Mach-O `MH_BUNDLE` / `MH_KEXT_BUNDLE` / `MH_CORE` / other | filetype ≠ above | `Unknown` / `Core`   | `Other`              |
| ELF `ET_EXEC`       | `e_type = 0x2`                                      | `Executable`         | `Application`        |
| ELF `ET_DYN` + `PT_INTERP` present | `e_type = 0x3` and program-header table contains a `PT_INTERP` segment | `Dynamic` + supplemental check | `Application` |
| ELF `ET_DYN` − `PT_INTERP`         | `e_type = 0x3` and no `PT_INTERP` | `Dynamic`            | `SharedLibrary`      |
| ELF `ET_REL`        | `e_type = 0x1`                                      | `Relocatable`        | `Object`             |
| ELF `ET_CORE`       | `e_type = 0x4`                                      | `Core`               | `Other`              |
| PE without `IMAGE_FILE_DLL`        | `Characteristics & 0x2000 = 0`     | `Executable`         | `Application`        |
| PE with `IMAGE_FILE_DLL`           | `Characteristics & 0x2000 ≠ 0`     | `Dynamic`            | `SharedLibrary`      |
| PE with `IMAGE_FILE_SYSTEM`        | `Characteristics & 0x1000 ≠ 0`     | `Unknown`            | `Other`              |

The `object` crate already does the first 9 cross-format mappings inside its `Object::kind()` impl. The ELF `PT_INTERP` supplemental check is the only mikebom-side logic we add, and it lives in the new `scan_fs/binary/role.rs` module.

## Format-native enum mapping

| mikebom `BinaryRole` | CycloneDX 1.6 `Component.type` | SPDX 2.3 `Package.primaryPackagePurpose` | SPDX 3.0.1 `software_Package.software_primaryPurpose` |
|----------------------|---------------------------------|------------------------------------------|------------------------------------------------------|
| `Application`        | `"application"`                 | `"APPLICATION"`                          | `"application"`                                       |
| `SharedLibrary`      | `"library"`                     | `"LIBRARY"`                              | `"library"`                                           |
| `Object`             | `"file"`                        | `"FILE"`                                 | `"file"`                                              |
| `Other`              | `"library"` (preserves historic default — no closer enum value exists; CDX `file` is more accurate for some Other cases but would regress consumers expecting `library` for what was previously a library-defaulting component) | _omitted_ | _omitted_ |

`None` (component did not come from the binary reader):

| Format       | Behavior                                                |
|--------------|---------------------------------------------------------|
| CycloneDX    | Falls back to the existing per-ecosystem default ( `library` for every reader today) — byte-identical to current behavior. |
| SPDX 2.3     | Falls back to existing logic — sets `APPLICATION` for components carrying the `main-module` annotation (milestone 053+ / 064), otherwise omits. Byte-identical. |
| SPDX 3       | Same fallback as SPDX 2.3 in shape, applied to `software_primaryPurpose`. Byte-identical. |

## Update sites by file

| File | Change kind | Lines (approx.) |
|------|-------------|-----------------|
| `mikebom-common/src/resolution.rs` | Add `BinaryRole` enum + new field on `ResolvedComponent` | +35 |
| `mikebom-common/src/resolution.rs` (existing defaults) | `binary_role: None` in `ResolvedComponent::default()` | +1 |
| `mikebom-cli/src/scan_fs/binary/role.rs` | NEW module — `classify(file: &object::File) -> BinaryRole`, ELF `PT_INTERP` helper, unit tests | +120 |
| `mikebom-cli/src/scan_fs/binary/mod.rs` | `pub mod role;` re-export | +1 |
| `mikebom-cli/src/scan_fs/binary/scan.rs` | Add `binary_role: BinaryRole` field on `BinaryScan`; populate via `role::classify` after `File::parse` | +5 |
| `mikebom-cli/src/scan_fs/binary/entry.rs` | Propagate `binary_role` from `BinaryScan` → `PackageDbEntry` → `ResolvedComponent` | +5 |
| `mikebom-cli/src/generate/cyclonedx/builder.rs` | Replace hardcoded `"library"` with `binary_role_to_cdx_type(component.binary_role)` helper | +15 |
| `mikebom-cli/src/generate/spdx/packages.rs` | Extend `primary_package_purpose` derivation to consider `binary_role` | +15 |
| `mikebom-cli/src/generate/spdx/v3_packages.rs` | Same shape as SPDX 2.3 emitter | +15 |
| `mikebom-cli/src/parity/catalog.rs` | New catalog row `A13` for component-typing role | +5 |
| `mikebom-cli/src/parity/extractors/{cdx,spdx2,spdx3}.rs` | Extractors returning `BTreeMap<purl, role-string>` | +30 |
| All `ResolvedComponent { .. }` struct-literal sites (production + tests) | `binary_role: None,` defaults | +1 per site (~25 sites — same mechanical update as milestone 096) |
| `docs/reference/sbom-format-mapping.md` | New row documenting CDX type ↔ SPDX 2.3 primaryPackagePurpose ↔ SPDX 3 software_primaryPurpose mapping | +1 row |
| `mikebom-cli/tests/binary_role_parity.rs` | NEW — cross-format parity integration test | +80 |
| `mikebom-cli/tests/binary_role_disambiguation.rs` | NEW — PIE / bundle / object-file edge cases | +120 |

Net: 1 new module (`role.rs`), 1 new test file, 6 production files modified, ~25 mechanical `binary_role: None,` additions in struct-literal sites, 1 docs row, plus regenerated goldens on file-level fixtures only.
