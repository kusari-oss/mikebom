# Contract: Cross-Format Binary Role Mapping

This contract pins the cross-format mapping that the milestone-104 emitters honor. Downstream SBOM consumers writing role-aware filters or audit tools can rely on this contract — the mapping is stable across mikebom alpha releases (any change requires a deliberate constitution-tracked update, the same governance bar that applies to the format-mapping doc).

## Role definitions

A binary component emitted by mikebom (`mikebom-cli/src/scan_fs/binary/`) carries one of four roles, derived from the source file's format header:

- **Application** — the component is an executable program (entry-point bearing, intended to be invoked directly by the OS or another process). Source-format markers: Mach-O `MH_EXECUTE`; ELF `ET_EXEC`; ELF `ET_DYN` with a `PT_INTERP` program-header segment (PIE executables); PE without the `IMAGE_FILE_DLL` characteristic bit set.
- **SharedLibrary** — the component is a dynamically loadable code unit (no entry point of its own; loaded by another process at runtime via dynamic linker or `dlopen`/`LoadLibrary`). Source-format markers: Mach-O `MH_DYLIB`; ELF `ET_DYN` without a `PT_INTERP` segment; PE with the `IMAGE_FILE_DLL` characteristic bit set.
- **Object** — the component is a relocatable object file (intermediate build artifact; not directly executable, not a deployable library). Source-format markers: Mach-O `MH_OBJECT`; ELF `ET_REL`.
- **Other** — the source format is recognized but doesn't map to any of the above (Mach-O `MH_BUNDLE`, `MH_KEXT_BUNDLE`, `MH_CORE`, etc.; ELF `ET_CORE`; PE with `IMAGE_FILE_SYSTEM`), or the file's header bytes were unparseable.

## Format-native field per role

| Role            | CycloneDX 1.6 `Component.type` | SPDX 2.3 `Package.primaryPackagePurpose` | SPDX 3.0.1 `software_Package.software_primaryPurpose` |
|-----------------|---------------------------------|------------------------------------------|------------------------------------------------------|
| Application     | `"application"`                 | `"APPLICATION"`                          | `"application"`                                       |
| SharedLibrary   | `"library"`                     | `"LIBRARY"`                              | `"library"`                                           |
| Object          | `"file"`                        | `"FILE"`                                 | `"file"`                                              |
| Other           | `"library"`                     | _omitted_                                | _omitted_                                             |

The CDX `library` value for Object's mapping is `file` (matching the spec's "structured file artifact that isn't an application or library" semantic). The CDX `library` value for Other's mapping preserves the historic default — pre-milestone-104 every binary-reader component was emitted as `library`, and components whose role we genuinely cannot classify continue to emit that way to minimize consumer-side churn.

The SPDX 2.3 and SPDX 3 `Other`-row mapping is "omitted" because (a) neither spec has an `OTHER` enum value that semantically matches "could not classify," and (b) the existing per-format default (omit the field) is structurally valid and matches pre-milestone behavior for these components.

## Consumer contract

Mikebom commits to:

1. **Determinism**: identical input bytes → identical role classification → byte-identical type-field emission across runs. Same `mikebom-version` re-running on the same input produces the same SBOM bytes.
2. **Native-only emission**: the role signal rides exclusively through the CDX `Component.type` / SPDX `primaryPackagePurpose` / SPDX 3 `software_primaryPurpose` slots. No `mikebom:binary-role` annotation is emitted. Consumers reading these fields get the complete role signal.
3. **Format-independent agreement**: for any single binary component emitted by the same scan in all three formats, the role values in the three native fields agree (per the table above). A consumer reading the same component in CDX vs SPDX 2.3 vs SPDX 3 sees a consistent role.
4. **Stable enum vocabulary**: the four roles (`Application` / `SharedLibrary` / `Object` / `Other`) are stable across alpha releases. New roles MAY be added in a future spec milestone; existing roles MAY NOT be removed or renamed without a deliberate breaking-change announcement.
5. **No surprises for non-binary components**: components emitted by manifest- and lockfile-driven readers (npm, cargo, maven, pip, gem, deb, rpm, apk, etc.) continue to emit their existing types. This contract is scoped to binary-reader-emitted components.

Mikebom does NOT commit to:

- Disambiguating MH_BUNDLE from MH_KEXT_BUNDLE from "format unparseable" inside the `Other` bucket. Consumers wanting finer detail read the existing `mikebom:binary-class` annotation (carries `elf` / `macho` / `pe`) and the `mikebom:elf-build-id` / `mikebom:macho-uuid` / `mikebom:pe-pdb-id` identity annotations.
- Reading runtime-side data (memory maps, ld.so cache, ptrace) to refine the classification. The classification is a pure function of the file's on-disk header bytes.

## Reference test cases

The milestone's `mikebom-cli/tests/binary_role_parity.rs` integration test covers the canonical cases:

- `/bin/ls` on macOS → Application in all three formats
- `/usr/lib/libSystem.B.dylib` on macOS → SharedLibrary in all three formats
- `/usr/bin/ls` on a modern Debian → Application in all three formats (PIE executable, ET_DYN with PT_INTERP)
- `/lib/x86_64-linux-gnu/libc.so.6` on Debian → SharedLibrary in all three formats (ET_DYN without PT_INTERP)
- `cmd.exe` on Windows → Application in all three formats (PE without IMAGE_FILE_DLL)
- `kernel32.dll` on Windows → SharedLibrary in all three formats (PE with IMAGE_FILE_DLL)

Tests use synthetic fixtures generated at test time (small, deterministic binaries built per-platform) rather than vendoring real system binaries (which would balloon the fixture repo and create per-host non-determinism).
