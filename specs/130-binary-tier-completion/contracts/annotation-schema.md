# Annotation schema contract — milestone 130

Four new `mikebom:*` annotation keys, all component-scope, all `SymmetricEqual` across
CDX + SPDX 2.3 + SPDX 3 per the existing milestone-128 parity-extraction convention. Catalogued into
`docs/reference/sbom-format-mapping.md` as C-rows C92..C95 (final numbering subject to in-flight
milestone collisions; tasks.md will pin the exact range at implementation time).

All four keys originate in US3 (PE/CLR managed-assembly metadata). US1 and US2 introduce NO new
annotation keys — they reuse existing channels (US1: existing milestone-029 `parent_purl` cross-link
unchanged; US2: existing `mikebom:source-mechanism` channel with new `"maven-jar-nested"` value
variant).

## C92: `mikebom:assembly-version-informational`

**Scope**: component

**Value**: the verbatim string from the .NET assembly's `AssemblyInformationalVersionAttribute` custom
attribute. Example: `"8.0.27-servicing.26230.7+sha.a1b2c3d"`.

**Emitted by**: PE/CLR managed-assembly reader (US3, FR-021).

**Principle V audit**:

- CDX 1.6 native equivalent? `version` is single-valued. **No.**
- SPDX 2.3 native equivalent? `Package.versionInfo` is single-valued. **No.**
- SPDX 3 native equivalent? `software_packageVersion` is single-valued. **No.**

**Verdict**: Valid parity-bridging extension per the Principle V "finer-grained information the standard
does not express" carve-out. The single native version field is consumed by mikebom's PURL-version
selection ladder (`InformationalVersion → FileVersion → AssemblyVersion`); the other two versions
remain available for audit / forensics via the three `mikebom:assembly-version-*` annotations.

## C93: `mikebom:assembly-version-file`

**Scope**: component

**Value**: the verbatim string from the .NET assembly's `AssemblyFileVersionAttribute` custom attribute.
Example: `"8.0.27.26230"`.

**Emitted by**: PE/CLR managed-assembly reader (US3, FR-021).

**Principle V audit**: same as C92. **Valid parity-bridging extension.**

## C94: `mikebom:assembly-version-runtime`

**Scope**: component

**Value**: the rendered 4-tuple from the assembly's `Assembly` metadata-table row 0
(`MajorVersion.MinorVersion.BuildNumber.RevisionNumber`). Example: `"8.0.27.0"`. This is the
CLR-binding-relevant version (what the runtime uses for assembly-binding decisions).

**Emitted by**: PE/CLR managed-assembly reader (US3, FR-021).

**Principle V audit**: same as C92. **Valid parity-bridging extension.**

## C95: `mikebom:assembly-cultures`

**Scope**: component

**Value**: comma-joined sorted list of non-"neutral" cultures detected across all DLL files that merged
into this component during the milestone-130 US3 resource-assembly dedup. Example:
`"de,fr,ja,ko,zh-Hans,zh-Hant"`. Omitted entirely when the dedup-collapsed component has only the
"neutral" culture (the common case).

**Emitted by**: PE/CLR managed-assembly reader (US3, FR-024).

**Principle V audit**:

- CDX 1.6 native equivalent? `properties[]` could in principle hold per-component metadata, but there
  is no standardized name + value shape for "list of detected resource cultures". **No native
  equivalent.**
- SPDX 2.3 native equivalent? `Package.annotations[]` could carry the same — but again, no
  standardized name. **No.**
- SPDX 3 native equivalent? **No.**

**Verdict**: Valid parity-bridging extension per the Principle V carve-out. The annotation preserves
the audit trail (which culture variants were observed for the package) without inflating component
counts by ~30× per package. Resolved per the 2026-06-18 clarification Q1 (Option B chosen).

## Cross-cutting catalog wiring

Each of the four C-rows MUST be:

1. Catalogued in `docs/reference/sbom-format-mapping.md` with the audit narrative above.
2. Registered as a `cdx_anno!`, `spdx23_anno!`, and `spdx3_anno!` entry in
   `mikebom-cli/src/parity/extractors/{cdx,spdx2,spdx3}.rs`.
3. Registered as a `ParityExtractor` slice entry in `mikebom-cli/src/parity/extractors/mod.rs` with
   matching `use` imports.
4. Covered by the existing `extractors_table_is_sorted_by_row_id` +
   `every_catalog_row_has_an_extractor` shape tests (no new test added; existing tests fail if any
   new row breaks invariants).
