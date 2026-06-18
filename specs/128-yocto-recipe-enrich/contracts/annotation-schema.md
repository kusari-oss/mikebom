# Contract: New `mikebom:yocto-*` annotation JSON shapes

All milestone-128 annotation values follow the existing `mikebom-annotation/v1` envelope convention (milestones 011, 080, 119, 127). Operators consuming SBOMs parse the JSON value structurally and version-gate on the envelope's `schema` discriminator.

## Common envelope

For multi-field or structured values:

```json
{
  "schema": "mikebom-annotation/v1",
  "field": "mikebom:<key>",
  "value": <field-specific>
}
```

For simple scalar / string values (most milestone-128 annotations), the envelope is implicit — the value is the bare string the CDX `properties[].value` or SPDX `annotations[].comment` field carries.

## Per-annotation contract

### `mikebom:srcrev` (FR-003)

| Format | Carrier | Value shape | Example |
|---|---|---|---|
| CDX | `components[].properties[]` entry | string (40-char hex SHA) | `"abc123def456..."` |
| SPDX 2.3 | `packages[].annotations[]` entry, annotationType `OTHER` | string | same |
| SPDX 3 | `Package.annotations[]` (graph-element Annotation) | string | same |

### `mikebom:src-uri` (FR-002)

JSON-encoded array of every SRC_URI entry (verbatim, no normalization):

```json
["git://github.com/foo/bar.git;branch=main;protocol=https", "file://patch-1.patch", "file://patch-2.patch"]
```

### `mikebom:srcrev-by-machine` (FR-003)

JSON-encoded object keyed by machine arch:

```json
{"qemuarm": "ee78c602...", "qemuarm64": "3e243d2a...", "qemumips": "62ea92a5..."}
```

### `mikebom:yocto-layer` (FR-006)

String — the BBFILE_COLLECTIONS value:

```
"meta-balena-rust"
```

### `mikebom:yocto-layer-version` (FR-006)

String:

```
"1"
```

### `mikebom:yocto-layer-series` (FR-006)

JSON-encoded array of compatible Yocto series names:

```json
["scarthgap"]
```

### `mikebom:bbappend-applied` (FR-008)

JSON-encoded array of `.bbappend` paths (lex-sorted, deduped, workspace-relative):

```json
["layers/meta-balena-bsp/recipes-extended/u-boot/u-boot_%.bbappend"]
```

### `mikebom:depends-unresolved` (FR-009)

JSON-encoded array of unresolvable DEPENDS entries:

```json
["external-binary-tool", "vendor-private-lib"]
```

### `mikebom:rdepends-unresolved` (FR-009)

Same shape as above, but for RDEPENDS.

### `mikebom:yocto-unexpanded-vars` (FR-005)

JSON-encoded object — keys are field names (LICENSE, SRC_URI, …); values are arrays of unresolved variable names:

```json
{"LICENSE": ["${PN}"], "SRC_URI": ["${BPN}", "${PV_MAJOR}"]}
```

### `mikebom:yocto-license-closed` (FR-012)

Boolean (always `true` when emitted; absent otherwise):

```
true
```

### `mikebom:yocto-description` (FR-010)

String — the DESCRIPTION field when it differs from SUMMARY:

```
"This recipe builds the foo library, providing the bar API and..."
```

### `mikebom:src-uri-local-only` (FR-002)

Boolean (always `true` when emitted):

```
true
```

### `mikebom:yocto-class-extend` (BBCLASSEXTEND)

JSON-encoded array of class-extend flavor names:

```json
["native", "nativesdk"]
```

### `mikebom:yocto-overrides-merged` (FR-016)

Boolean (always `true` when ≥1 override-syntax merge fired on this recipe):

```
true
```

### `mikebom:yocto-recipe-name` (FR-002a)

String — the recipe's filename-derived name (preserved when FR-002a emits a host-typed PURL pointing at the upstream artifact instead of the recipe). Emitted ONLY when FR-002a's host-typed-PURL path fires; absent when the FR-011 `pkg:generic/...` fallback fires (the recipe name is already in the PURL `name` segment in that case).

```
"u-boot"
```

### `mikebom:yocto-recipe-version` (FR-002a)

String — the recipe's filename-derived version (preserved when FR-002a emits a host-typed PURL whose version segment is a SRCREV-derived 12-hex prefix instead of the recipe's PV). Emitted ONLY when FR-002a's host-typed-PURL path fires; absent when the FR-011 fallback fires.

```
"2024.07"
```

## Native-field-first reminders (Principle V)

These fields go to native carriers — NO `mikebom:*` annotation needed:

| Recipe field | Native carrier |
|---|---|
| LICENSE | CDX `components[].licenses[].expression`, SPDX 2.3 `packages[].licenseDeclared`, SPDX 3 `Package.declaredLicense` |
| HOMEPAGE | CDX/SPDX `externalReferences[type=website]` |
| SRC_URI git | CDX/SPDX `externalReferences[type=vcs]` |
| SRC_URI tarball | CDX/SPDX `externalReferences[type=distribution]` |
| SUMMARY | CDX `component.description`, SPDX `Package.summary` |
| DEPENDS edges | SBOM-format-native relationship: CDX `dependencies[]`, SPDX `Relationships[type=DEPENDS_ON]` |

## Catalog C-rows

C70 through C86 are reserved for milestone-128 annotations (15 from the original FR-013 list + 2 from FR-002a: `mikebom:yocto-recipe-name` and `mikebom:yocto-recipe-version`). Working assumption verified at PR time per the milestone-127 + milestone-119 precedent.
