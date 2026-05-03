# Contract: Cargo main-module component placement per format

This contract specifies the per-format placement of the cargo main-module component(s) in the SBOM output. It parallels `053-go-main-module-edges/contracts/main-module-component.md` and generalizes to handle the workspace-multi-member case that's more common for cargo than for Go.

## CycloneDX 1.6

### Single cargo main-module (single-crate scan)

```json
{
  "bomFormat": "CycloneDX",
  "specVersion": "1.6",
  "metadata": {
    "component": {
      "bom-ref": "pkg:cargo/clap@4.5.7",
      "type": "application",
      "name": "clap",
      "version": "4.5.7",
      "purl": "pkg:cargo/clap@4.5.7",
      "properties": [
        { "name": "mikebom:component-role", "value": "main-module" },
        { "name": "mikebom:sbom-tier", "value": "source" }
      ]
    }
  },
  "components": [
    /* cargo main-module is NOT here — it's exclusively in metadata.component */
    { "bom-ref": "pkg:cargo/serde@1.0.193", ... },
    ...
  ],
  "dependencies": [
    {
      "ref": "pkg:cargo/clap@4.5.7",
      "dependsOn": ["pkg:cargo/serde@1.0.193", "pkg:cargo/anstyle@1.0.4", ...]
    },
    ...
  ]
}
```

**Key invariants:**

- `metadata.component.type` MUST be `"application"`.
- `metadata.component.bom-ref` MUST equal the cargo main-module's PURL.
- The same PURL MUST NOT appear in `components[]`.
- The `mikebom:component-role: main-module` property MUST be in `metadata.component.properties[]`.
- All direct-dep edges from `[dependencies]`/`[dev-dependencies]`/`[build-dependencies]` (post-scope-filter) MUST appear in `dependencies[]` keyed by `metadata.component.bom-ref`.

### Multiple cargo main-modules (workspace scan)

When the scan contains a workspace with N member crates (or any combination of cargo + other-ecosystem main-modules), `metadata.component` becomes a synthetic super-root and each main-module appears as a regular `components[]` entry with the C40 supplementary tag.

```json
{
  "metadata": {
    "component": {
      "bom-ref": "mikebom-super-root-<scan-fingerprint>",
      "type": "application",
      "name": "<filesystem-root-name>",
      "purl": "pkg:generic/mikebom-super-root@<scan-fingerprint>"
    }
  },
  "components": [
    {
      "bom-ref": "pkg:cargo/mikebom@0.1.0-alpha.11",
      "type": "application",
      "name": "mikebom",
      "version": "0.1.0-alpha.11",
      "purl": "pkg:cargo/mikebom@0.1.0-alpha.11",
      "properties": [{ "name": "mikebom:component-role", "value": "main-module" }, ...]
    },
    {
      "bom-ref": "pkg:cargo/mikebom-common@0.1.0-alpha.11",
      "type": "application",
      ...
    },
    ...
  ],
  "dependencies": [
    {
      "ref": "mikebom-super-root-<scan-fingerprint>",
      "dependsOn": [
        "pkg:cargo/mikebom-ebpf@<v>",
        "pkg:cargo/mikebom@0.1.0-alpha.11",
        "pkg:cargo/mikebom-common@0.1.0-alpha.11",
        "pkg:cargo/xtask@0.1.0-alpha.11"
      ]
    },
    {
      "ref": "pkg:cargo/mikebom@0.1.0-alpha.11",
      "dependsOn": ["pkg:cargo/mikebom-common@0.1.0-alpha.11", "pkg:cargo/serde@...", ...]
    },
    ...
  ]
}
```

**Key invariants (workspace case):**

- The super-root's `dependsOn[]` lists every cargo main-module's PURL alongside any other-ecosystem main-module PURLs, sorted deterministically (alphabetical PURL string sort).
- Each cargo main-module appears in `components[]` with `type: "application"` and the C40 property.
- Each cargo main-module's own `dependencies[]` entry lists ITS direct deps (independent of the super-root's children list).

## SPDX 2.3

### Single or multiple cargo main-modules

```json
{
  "spdxVersion": "SPDX-2.3",
  "documentDescribes": [
    "SPDXRef-Package-pkg-cargo-mikebom-0-1-0-alpha-11",
    "SPDXRef-Package-pkg-cargo-mikebom-common-0-1-0-alpha-11",
    "SPDXRef-Package-pkg-cargo-xtask-0-1-0-alpha-11",
    "SPDXRef-Package-pkg-cargo-mikebom-ebpf-..."
  ],
  "packages": [
    {
      "SPDXID": "SPDXRef-Package-pkg-cargo-mikebom-0-1-0-alpha-11",
      "name": "mikebom",
      "versionInfo": "0.1.0-alpha.11",
      "primaryPackagePurpose": "APPLICATION",
      "annotations": [
        {
          "annotator": "Tool: mikebom-...",
          "annotationType": "OTHER",
          "comment": "{\"mikebom:component-role\": \"main-module\"}"
        }
      ],
      "externalRefs": [
        { "referenceCategory": "PACKAGE-MANAGER",
          "referenceType": "purl",
          "referenceLocator": "pkg:cargo/mikebom@0.1.0-alpha.11" }
      ],
      ...
    },
    ...
  ],
  "relationships": [
    { "spdxElementId": "SPDXRef-DOCUMENT",
      "relatedSpdxElement": "SPDXRef-Package-pkg-cargo-mikebom-0-1-0-alpha-11",
      "relationshipType": "DESCRIBES" },
    ...
  ]
}
```

**Key invariants:**

- Every cargo main-module package MUST have `primaryPackagePurpose: "APPLICATION"`.
- `documentDescribes[]` MUST list every cargo main-module's SPDXID, sorted deterministically.
- For each main-module, a `SPDXRef-DOCUMENT DESCRIBES <main-module-spdxid>` relationship MUST appear in `relationships[]`.
- The `mikebom:component-role: main-module` annotation MUST be attached to each main-module package via the existing C40 annotation envelope wiring.

## SPDX 3.0.1

### Single or multiple cargo main-modules

```json
{
  "@graph": [
    {
      "type": "SpdxDocument",
      "spdxId": "spdx:doc:...",
      ...
    },
    {
      "type": "software_Package",
      "spdxId": "spdx:Package:pkg-cargo-mikebom-0-1-0-alpha-11",
      "name": "mikebom",
      "software_packageVersion": "0.1.0-alpha.11",
      "software_primaryPurpose": "application",
      ...
    },
    {
      "type": "Relationship",
      "spdxId": "spdx:Relationship:doc-describes-mikebom",
      "relationshipType": "describes",
      "from": "spdx:doc:...",
      "to": ["spdx:Package:pkg-cargo-mikebom-0-1-0-alpha-11"]
    },
    ...
  ]
}
```

**Key invariants:**

- Every cargo main-module element MUST have `software_primaryPurpose: "application"`.
- A document-level `describes` Relationship MUST point at every cargo main-module element.
- The C40-mapped native field representing the main-module role MUST be set per the existing v3 wiring.

## Same-PURL collision behavior (FR-001 + Q1)

When the walker discovers two-or-more `Cargo.toml` files that resolve to the same `pkg:cargo/<name>@<version>` PURL (vendored copies under `vendor/`, mirrors under `examples/`, extractions under `target/package/`):

1. Exactly **one** main-module component is emitted, keyed by PURL.
2. The first-discovered crate's outgoing direct-dep edges are retained (deterministic on the existing alphabetical walker order).
3. A single `tracing::warn!` is emitted listing all dropped duplicate paths in one consolidated message:

   ```text
   cargo: deduped 2 same-PURL Cargo.toml files for pkg:cargo/foo@1.2.3 — kept /tmp/scan/crates/foo/Cargo.toml, dropped: /tmp/scan/vendor/foo-1.2.3/Cargo.toml, /tmp/scan/examples/foo/Cargo.toml
   ```

4. The SBOM bytes do not encode the dedup occurrence (no `mikebom:duplicate-purl` annotation in this milestone). Divergent-PURL detection — same PURL with mismatched content hashes or dep sets — is tracked in issue #125 and will introduce the SBOM signal there.

## Cross-format invariants

- The PURL emitted in CDX `metadata.component.purl`, SPDX `externalRefs[*].referenceLocator`, and SPDX 3 element identity MUST be byte-identical for the same scan.
- The C40 role tag (`mikebom:component-role: main-module`) MUST be present in all three formats per the existing C40 catalog row's per-format mapping.
- The set of cargo main-module identities (their PURLs) emitted by the scan MUST be the same across all three formats. Tested by parity-extractor C40 in `tests/holistic_parity.rs`.

## Does NOT change

- No new property/annotation key (C40 already exists in the catalog).
- No new SPDX `primaryPackagePurpose` enum value (`APPLICATION` already wired by milestone 053).
- No new CDX component `type` value (`application` already valid).
- No new relationship type (`DESCRIBES` / `describes` / `dependsOn` already in use).
- No CLI flag changes.
