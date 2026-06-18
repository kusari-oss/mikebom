# Contract: Yocto reader behavior

This feature does NOT add CLI flags. The behavior contract is the *change* in what the existing `mikebom sbom scan` command emits for Yocto source-tree scans (scans where one or more `.bb` files are present at or below the scan root).

## Affected command

```text
mikebom sbom scan --path <yocto-source-tree> --format <cyclonedx-json|spdx-2.3-json|spdx-3-json>
```

No new flags. No new env vars.

## Pre-feature behavior (today, alpha.48)

When `.bb` files are present, milestone 107's `recipe.rs` walker fires and emits one component per recipe with the filename-derived `(name, version)` pair:

```text
component.name           = "<recipe-name>"
component.version        = "<version-segment-from-filename>"
component.purl           = "pkg:bitbake/<name>@<version>?layer=<layer-dir-basename>"
component.licenses       = []         (not extracted)
component.externalRefs   = []         (no homepage, no vcs, no distribution)
component.properties     = [{mikebom:component-role: source-tier},
                            {mikebom:source-files: [<bb-file-path>]}]
metadata.component       = the milestone-127 root-selector picks a generic root
```

No layer attribution, no SRCREV, no DEPENDS edges, no `.bbappend` provenance, no CPE-name normalization.

## Post-feature behavior

Same command, same flags. The internal reader gains the milestone-128 body-parser and the layer-attribution + bbappend-index + CPE-name-normalization passes. Per-component output shape:

```text
# FR-011 fallback shape (non-git SRC_URI OR unrecognized host OR SRCREV absent):
component.name           = "<recipe-name>"
component.version        = <SRCREV-derived (12-hex) when PV is "git" or AUTOINC; else filename PV>
component.purl           = "pkg:generic/<name>@<version>?openembedded=true&layer=<collection>"

# FR-002a host-typed shape (git SRC_URI from github/gitlab/bitbucket/codeberg + SRCREV set):
component.name           = "<repo>"      (extracted from the SRC_URI path)
component.version        = <srcrev-12-hex, lowercased>
component.purl           = "pkg:github/<owner>/<repo>@<srcrev-12-hex>"   (or gitlab/bitbucket/codeberg)
                           — provides direct OSV match via the github/gitlab/bitbucket/codeberg ecosystem mapping
# In the FR-002a shape, recipe-identity provenance is preserved via:
component.properties     += {mikebom:yocto-recipe-name: "<recipe-name>", mikebom:yocto-recipe-version: "<recipe-version>"}

# Both shapes emit:
component.licenses       = [<SPDX-canonical expression from LICENSE>]   (FR-001)
component.description    = "<SUMMARY>"                                  (FR-010)
component.externalRefs   = [{type: "vcs", url: "<git-https-url>"},      (FR-002 when SRC_URI git)
                            {type: "website", url: "<HOMEPAGE>"},        (FR-010 when set)
                            {type: "distribution", url: "<tarball>"}]   (FR-002 when SRC_URI tarball)
component.properties     = [{mikebom:component-role: source-tier},
                            {mikebom:source-files: [<bb-path>, <inc-paths>...]},
                            {mikebom:srcrev: "<full-SHA>"},               (FR-003)
                            {mikebom:src-uri: ["...", "..."]},            (FR-002 full URI list)
                            {mikebom:yocto-layer: "<collection>"},        (FR-006)
                            {mikebom:yocto-layer-version: "<version>"},   (FR-006)
                            {mikebom:bbappend-applied: ["<path>", ...]}, (FR-008 when applies)
                            {mikebom:cpe-candidates: [...]},              (milestone-097, with FR-017 normalized names)
                            ...]
relationships            = DEPENDS_ON edges per recipe DEPENDS / RDEPENDS  (FR-009)

metadata.component       = layer-collection (FR-007 — milestone-127 picks the layer-root)
```

## Out-of-scope (unchanged)

- Image-tier scans (`yocto/manifest.rs` + `opkg.rs` + `yocto/context.rs` paths) continue to fire whenever a built rootfs is detected. milestone 107 behavior preserved byte-identically (FR-014).
- Non-Yocto scans (any tree without `.bb` files) — zero change. SC-006 byte-identity on all 33 alpha.48 goldens.

## Behavior on edge cases

| Case | Pre-128 | Post-128 |
|---|---|---|
| `LICENSE = "MIT"` | not extracted | `licenses[0] = "MIT"` |
| `LICENSE = "GPL-2.0-only & LGPL-2.1-or-later"` | not extracted | `licenses[0] = "GPL-2.0-only AND LGPL-2.1-or-later"` (BitBake `&` → SPDX `AND`) |
| `LICENSE = "CLOSED"` | not extracted | `licenses[0] = "NOASSERTION"` + property `mikebom:yocto-license-closed: true` |
| LICENSE absent OR variable-substituted | not extracted | `licenses = []` + property `mikebom:yocto-unexpanded-vars: ["LICENSE"]` + warn log |
| `SRC_URI = "git://github.com/x/y.git"` + `SRCREV = "abc..."` | not extracted | `externalRefs[type=vcs] = https://github.com/x/y.git` + property `mikebom:srcrev: "abc..."` |
| `SRC_URI = "http://x/y.tar.gz"` | not extracted | `externalRefs[type=distribution] = http://x/y.tar.gz` |
| `SRC_URI = "file://patch.patch"` only | not extracted | `externalRefs = []` + property `mikebom:src-uri-local-only: true` |
| `PV = "git"` (no fixed version) | `version: "git"` | version derived from SRCREV first 12 hex chars |
| `PV = "0.0.4.AUTOINC+f597fb"` | `version: "0.0.4.AUTOINC+f597fb"` | `version: "0.0.4"`, `mikebom:srcrev: "f597fb..."` |
| Layer `conf/layer.conf` exists at ancestor | not parsed | `mikebom:yocto-layer = "<BBFILE_COLLECTIONS>"`, `mikebom:yocto-layer-version = "<LAYERVERSION>"` |
| No ancestor `conf/layer.conf` | n/a | no layer annotation + warn log |
| `.bbappend` matches recipe | not parsed | base recipe carries `mikebom:bbappend-applied: ["<path>"]` |
| Orphan `.bbappend` | n/a | warn log + no phantom component |
| Recipe with `DEPENDS = "openssl libssl-dev"` and openssl is also a scanned recipe | no edges | `DEPENDS_ON` edge from recipe → openssl (when resolvable) |
| Recipe in `recipes-foo/foo/foo_1.0.bb`, with `foo.inc` also in same dir | filename-only | `.bb` and `.inc` both parsed; merged per FR-004 last-write-in-source-order |
| `curl` recipe (5 vendor CPE candidates) | 1 component, no CPE candidates | 1 component, 5 CPE candidates in array (FR-019; no fan-out) |
| `linux-kernel` recipe | 1 component, no CPE normalization | 1 component, `mikebom:cpe-candidates` includes `linux_kernel` normalized name |

## Stderr behavior

Warnings emitted (via `tracing::warn!`):

- LICENSE extraction failed for `<path>`: <reason>
- `.bbappend <path>` matches no recipe (orphan); not synthesizing phantom component
- Recipe `<path>` has no ancestor `conf/layer.conf`; layer attribution skipped
- Recipe `<path>` references unresolved `${<VAR>}` in `<FIELD>`; emitting `mikebom:yocto-unexpanded-vars`
- DEPENDS entry `<name>` for `<recipe>` is unresolvable in scan; recording in `mikebom:depends-unresolved`

Debug-level (via `tracing::debug!`):

- Cyclic include chain detected at `<path>`; dropping tail include
- Override-syntax merge applied to field `<F>` on `<recipe>`; merging as union (FR-016)
