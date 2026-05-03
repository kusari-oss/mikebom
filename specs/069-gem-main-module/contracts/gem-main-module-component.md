# Contract: gem main-module component placement per format

Same per-format placement as cargo (064) / npm (066) / pip (068). Inherits multi-main-module super-root + plural-DESCRIBES from #127. Only differences: PURL prefix `pkg:gem/...` and the `name` field is the literal `s.name` value verbatim (no normalization step — gem names are already URL-safe per A10).

## CycloneDX 1.6

### Single gem main-module

```json
{
  "metadata": {
    "component": {
      "bom-ref": "pkg:gem/foo@1.2.3",
      "type": "application",
      "name": "foo",
      "version": "1.2.3",
      "purl": "pkg:gem/foo@1.2.3",
      "properties": [
        { "name": "mikebom:component-role", "value": "main-module" },
        { "name": "mikebom:sbom-tier", "value": "source" }
      ]
    }
  }
}
```

**Key invariants** (same as cargo/npm/pip):
- `metadata.component.type` MUST be `"application"`.
- `metadata.component.bom-ref` MUST equal the gem main-module's PURL.
- The same PURL MUST NOT appear in `components[]`.
- C40 supplementary tag in `metadata.component.properties[]`.

## SPDX 2.3

```json
{
  "documentDescribes": ["SPDXRef-Package-pkg-gem-foo-1-2-3"],
  "packages": [
    {
      "SPDXID": "SPDXRef-Package-pkg-gem-foo-1-2-3",
      "name": "foo",
      "versionInfo": "1.2.3",
      "primaryPackagePurpose": "APPLICATION",
      "annotations": [
        { "annotator": "Tool: mikebom-...",
          "annotationType": "OTHER",
          "comment": "{\"mikebom:component-role\": \"main-module\"}" }
      ],
      "externalRefs": [
        { "referenceCategory": "PACKAGE-MANAGER",
          "referenceType": "purl",
          "referenceLocator": "pkg:gem/foo@1.2.3" }
      ]
    }
  ]
}
```

## SPDX 3.0.1

```json
{
  "@graph": [
    {
      "type": "SpdxDocument",
      "rootElement": ["https://mikebom.kusari.dev/spdx3/doc-.../pkg-..."],
      ...
    },
    {
      "type": "software_Package",
      "name": "foo",
      "software_packageVersion": "1.2.3",
      "software_primaryPurpose": "application",
      "software_packageUrl": "pkg:gem/foo@1.2.3"
    }
  ]
}
```

## Same-PURL collision behavior

Same as cargo/npm/pip: first-discovered-wins, `tracing::warn!` lists drops. Divergent-PURL detection deferred to #125.

## Cross-format invariants

- PURL byte-identical across all 3 formats.
- C40 supplementary tag in all 3 formats.
- Application-style projects (no `*.gemspec`) emit NO main-module in any format per FR-002.

## Does NOT change

- No new property/annotation key.
- No new SPDX `primaryPackagePurpose` enum value.
- No new CDX component `type`.
- No CLI flag changes.
