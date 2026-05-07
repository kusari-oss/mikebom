# Data Model — milestone 078 SPDX 3.0.1 conformance pass

The milestone introduces zero new Rust types. All changes live in the SPDX 3 emission code path (`mikebom-cli/src/generate/spdx/v3_document.rs`) and in test/CI integration. The "data model" here is the **SPDX 3 wire-format graph entries** that change as a result of this milestone.

## Wire-format entities (SPDX 3 graph elements)

### `Organization` element (NEW)

```json
{
  "@context": "(inherited from document)",
  "type": "Organization",
  "spdxId": "https://mikebom.kusari.dev/spdx3/doc-<HASH>/agent/mikebom-contributors",
  "creationInfo": "_:creation-info",
  "name": "mikebom contributors"
}
```

**Lifetime**: emitted per SPDX 3 document. Fully deterministic given the document's `<HASH>` portion (which is itself deterministic per scan inputs per the existing v3_document.rs pattern).

**Fields**:
- `type`: literal `"Organization"` (extends abstract `Agent` per SPDX 3 Core model)
- `spdxId`: document-scoped IRI per research §4 (mirrors existing Tool pattern)
- `creationInfo`: reference to the same `_:creation-info` blank node as every other element in the graph
- `name`: literal `"mikebom contributors"` matching CDX `metadata.tools[0].publisher`

**Reference site**: `CreationInfo.createdBy` array points at this element's `spdxId`.

### `CreationInfo` element (MODIFIED)

```json
// BEFORE (today's emission, conformance-broken)
{
  "type": "CreationInfo",
  "@id": "_:creation-info",
  "specVersion": "3.0.1",
  "created": "...",
  "createdBy": [".../tool/mikebom"]      // ← Tool IRI in Agent slot — SHACL violation
}

// AFTER (post-fix, conformant)
{
  "type": "CreationInfo",
  "@id": "_:creation-info",
  "specVersion": "3.0.1",
  "created": "...",
  "createdBy":    [".../agent/mikebom-contributors"],   // ← Organization (Agent subclass)
  "createdUsing": [".../tool/mikebom"]                  // ← Tool moves here
}
```

**Field change**: `createdBy` value changes (was Tool IRI; now Organization IRI). New `createdUsing` field added. Specversion + created + @id unchanged.

### `Tool` element (UNCHANGED in shape; reference-slot moves)

```json
{
  "type": "Tool",
  "spdxId": "https://mikebom.kusari.dev/spdx3/doc-<HASH>/tool/mikebom",
  "creationInfo": "_:creation-info",
  "name": "mikebom-<version>"
}
```

The element's identity (spdxId, name, etc.) is unchanged. Only the slot referencing it on `CreationInfo` changes from `createdBy` to `createdUsing`.

### `simplelicensing_LicenseExpression` element (NEW — for `dataLicense`)

```json
{
  "@context": "(inherited from document)",
  "type": "simplelicensing_LicenseExpression",
  "spdxId": "https://spdx.org/licenses/CC0-1.0",
  "creationInfo": "_:creation-info",
  "simplelicensing_licenseExpression": "CC0-1.0"
}
```

**Note**: the concrete-subclass name + property name were verified during the T001(c) audit against the local SPDX 3 JSON-LD schema (`mikebom-cli/tests/fixtures/schemas/spdx-3.0.1.json`). The schema's `simplelicensing_AnyLicenseInfo_derived` enumeration lists `simplelicensing_LicenseExpression` (NOT `simplelicensing_License`) as the concrete subclass for license-expression carriers, with required field `simplelicensing_licenseExpression` (NOT `simplelicensing_simpleLicensingText`). This element shape is byte-identical to the per-component license-expression elements that mikebom's existing emission already produces via `v3_licenses.rs`, so the implementation reuses an established, already-conformant pattern.

**Lifetime**: emitted once per SPDX 3 document (a single shared element representing the dataLicense). Stable IRI `https://spdx.org/licenses/CC0-1.0` is used because it's an SPDX-listed-license; downstream tools that already understand SPDX-listed-license IRIs will recognize it.

### `SpdxDocument` element (MODIFIED — `dataLicense` field changes)

```json
// BEFORE (today's emission, conformance-broken)
{
  "type": "SpdxDocument",
  "spdxId": "https://mikebom.kusari.dev/spdx3/doc-<HASH>",
  "dataLicense": "https://spdx.org/licenses/CC0-1.0",     // ← bare URL, SHACL violation
  ...
}

// AFTER (post-fix, conformant)
{
  "type": "SpdxDocument",
  "spdxId": "https://mikebom.kusari.dev/spdx3/doc-<HASH>",
  "dataLicense": "https://spdx.org/licenses/CC0-1.0",     // ← still the same IRI value...
  ...
}
```

The wire-format change here is subtle: the `dataLicense` field's **value** is unchanged (still the SPDX-listed-license IRI), but the value now resolves (within `@graph`) to a typed `simplelicensing_License` element rather than being a bare URI string. The validator's SHACL constraint requires the IRI to resolve to an instance of `AnyLicenseInfo` (or subclass); adding the License element to `@graph` makes that resolution succeed.

## Validation rules

- **VR-078-001**: Every emitted SPDX 3 document MUST satisfy the SHACL ClassConstraint on `Core/createdBy` — the IRI in `CreationInfo.createdBy[]` MUST resolve to a graph element typed as a subclass of `Core/Agent` (Person, Organization, or SoftwareAgent).
- **VR-078-002**: Every emitted SPDX 3 document MUST satisfy the SHACL ClassConstraint on `Core/dataLicense` — the IRI in `SpdxDocument.dataLicense` MUST resolve to a graph element typed as a subclass of `SimpleLicensing/AnyLicenseInfo`.
- **VR-078-003**: `CreationInfo.createdUsing[]` is added as a new field referencing the existing Tool element. Cardinality 1..*.
- **VR-078-004**: All emitted SPDX 3 documents MUST pass `spdx3-validate --json <fixture>` with zero SHACL violations and zero schema violations. Verified by integration test per FR-002.
- **VR-078-005**: The Organization + License element shapes are deterministic — same scan inputs → byte-identical elements across re-runs. Per research §6.

## Backward compatibility

- No new `Cargo.toml` deps; no MSRV change; no nightly required.
- mikebom binary stays pure Rust per Constitution Principle I. Python is added only at the CI/test layer.
- Existing milestone-073/074/075/076/077 byte-identity goldens for **CDX 1.6** and **SPDX 2.3** stay byte-identical (FR-007). Only SPDX 3 fixtures regenerate.
- Existing milestone-073/074/075/076/077 SPDX 3 byte-identity goldens regenerate as the expected operator-visible change of this milestone (FR-006). Per-fixture diff is at minimum: 1 new `Organization` element, 1 new `simplelicensing_LicenseExpression` element, 1 changed `CreationInfo.createdBy` reference, 1 added `CreationInfo.createdUsing` field. Other elements unchanged.
- Downstream tools that consumed mikebom's pre-fix SPDX 3 output may need to update their expectations (e.g., a tool that hard-coded "the SBOM has exactly 4 graph elements: Tool, SpdxDocument, root-Package, deps" now sees 6 — `+Organization, +simplelicensing_LicenseExpression`). Tools that consume by spec-defined paths (`CreationInfo.createdBy` → resolve to Agent) will work post-fix without changes; tools that hard-coded specific IRIs may need adjustment.
