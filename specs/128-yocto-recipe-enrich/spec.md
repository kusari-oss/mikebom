# Feature Specification: Deeper Yocto / OpenEmbedded SBOM coverage

**Feature Branch**: `128-yocto-recipe-enrich`
**Created**: 2026-06-18
**Status**: Draft
**Input**: User description: "Deeper Yocto / OpenEmbedded SBOM coverage. Today milestone-107 emits one component per .bb filename via name+version regex; no LICENSE, no SRC_URI/SRCREV, no DEPENDS edges, no HOMEPAGE, no layer-collection annotation (from conf/layer.conf BBFILE_COLLECTIONS + LAYERVERSION), no bbappend tracking. PURL type is pkg:bitbake which may not match the purl-spec openembedded type. Three balena meta-layers (meta-balena: 163 bb + 337 bbappend + 14 layer.conf; balena-raspberrypi: 12+30+1; balena-generic: 1+9+1) are the motivating fixtures. Operators scanning meta-layer trees today get an unauditable PURL list with no license, no provenance, no edges."

## Clarifications

### Session 2026-06-18

- Q: Which PURL type should milestone-128 emit for Yocto recipe components — `pkg:bitbake/` (current), `pkg:openembedded/` (proposed in purl-spec discussion), or `pkg:generic/<name>@<version>?openembedded=true&layer=<collection>` (qualifiers carry the type signal)? → A: **Option C — `pkg:generic/` with qualifiers**. Evidence: an examined production Yocto-tooling-emitted CDX (balena-OS image, 145 components) uses `pkg:generic/<name>@<version>` for every component — the upstream tooling itself converged on this shape. Aligning with the upstream convention avoids reinventing a non-published type AND keeps PURLs accepted by every vuln scanner (which all support `pkg:generic`). Yocto-specific signal lives in qualifiers (`?openembedded=true&layer=<collection>`).
- Q: Should override-syntax fields (`FIELD:append:<override>`, `SRCREV_machine:<arch>`) be evaluated for a user-selected machine/distro mask, OR always merged-as-union (best-effort upper bound)? → A: **Option A — merge as union**. Evidence: the same Yocto-tooling-emitted CDX is itself a union upper bound (145 components include arch-specific recipes that would never all run on the same target machine). Bitbake's own SBOM emission semantic IS the union. Operators wanting per-machine precision should run bitbake against their target; mikebom doesn't reinvent BitBake's metadata evaluator.
- Q: How does mikebom handle the `version: "git"` and `<base>.AUTOINC+<sha>` anti-patterns the Yocto-tooling SBOM emits when recipes use git SRC_URI without a fixed `PV`? → A: **Reject both patterns; derive a real version from `SRCREV`**. When the recipe's effective `PV` is literally `git` or includes `AUTOINC`, mikebom MUST take the first 12 hex chars of `SRCREV` as the version segment AND emit a `mikebom:srcrev` annotation with the full SHA. Cleaner identifiers, sortable versions, vuln-scanner-compatible.
- Q: How does mikebom handle multiple plausible CPE-vendor candidates (the `curl` case: Yocto tooling emits 6 separate `pkg:generic/<vendor>/curl@8.7.1` components for `daniel_stenberg`, `curl`, `haxx`, `libcurl`, `libcurl/libcurl`, `curl/libcurl`)? → A: **Use the milestone-097 `mikebom:cpe-candidates` annotation array on ONE component, NOT separate components per vendor**. Explicitly diverges from the Yocto-tooling fan-out approach because mikebom already has a cleaner annotation channel for "many CPE candidates per component". One component, many CPE candidates in one array.
- Q: Does mikebom apply Yocto's recipe-name-to-CPE-product-name normalization (e.g., `linux-kernel` → `linux_kernel`, `nss` → `network_security_services`)? → A: **Yes — apply the openembedded-core CPE-name mapping table when emitting CPE candidates** so NVD vuln-matching works. The normalized name lives in the CPE candidates array; the human-readable recipe name stays in the PURL `name` segment.
- Q: When a recipe `foo_1.0.bb` does `require foo.inc` and BOTH the .bb AND the .inc set the same field (e.g., LICENSE), which value wins? → A: **Last-write-in-source-order wins** (BitBake-native semantics). Process the `.inc`'s assignments first, then process the `.bb`'s assignments; the `.bb`'s assignment overrides any conflicting `.inc` value. Matches BitBake's actual `bitbake -e <recipe>` evaluation order so mikebom's emitted fields agree with what bitbake itself sees at build time.
- Q: How does mikebom decide which layer a given recipe belongs to when multiple `conf/layer.conf`s are in scope (meta-balena has 14)? → A: **Nearest-ancestor `conf/layer.conf`** — walk the recipe's filesystem path upward until the first directory containing `conf/layer.conf` is found; that's the owning layer. NOT parsing BBFILES patterns (BitBake-faithful but unnecessary on conventional layouts). Justification: every layer in the three motivating fixtures uses the conventional `<layer>/recipes-*/<dir>/*.bb` hierarchy where the nearest-ancestor heuristic produces the correct answer.
- Q: On a mixed scan (meta-layer tree AND a built rootfs containing opkg DB + image manifest), do recipe-derived fields propagate onto opkg-DB-discovered components? → A: **Per-source emission + milestone-105 PURL-based dedup**. Each reader emits its own component independently; the existing milestone-105 dedup pipeline collapses cross-source emissions on canonical PURL. Recipe-reader fields (LICENSE, SRC_URI, layer attribution) propagate onto the winning post-dedup component as `mikebom:also-detected-via` evidence. NO explicit cross-correlation pass between the readers — keeps the readers loosely coupled and respects each one's authority (recipe = "what the layer ships," opkg DB = "what's installed in this rootfs").
- Q: For OSV (osv.dev) vulnerability matching, neither `pkg:bitbake/` nor `pkg:generic/` returns hits — OSV's PURL-query path only handles ecosystem-typed PURLs (github, gitlab, cargo, npm, etc.). Should mikebom emit ecosystem-typed PURLs when SRC_URI is git-shaped with a recognizable host, falling back to `pkg:generic/` per FR-011 only when the host is opaque? → A: **Yes — adopt FR-002a multi-shape strategy**. When `SRC_URI` contains a git URI whose host matches one of {github.com, gitlab.com, bitbucket.org, codeberg.org}, emit `pkg:<host-token>/<owner>/<repo>@<srcrev-12-hex>` as the primary PURL (the matching ecosystem-typed PURL OSV's commit/github/gitlab query paths accept). The original recipe name + version + layer attribution move to `mikebom:yocto-recipe-name`, `mikebom:yocto-recipe-version`, and `mikebom:yocto-layer` annotations. When SRC_URI is non-git or the host is unrecognized, fall back to FR-011's `pkg:generic/<recipe-name>@<version>?openembedded=true&layer=<collection>` shape. The change closes the OSV-direct-match gap on the ~50-70% of meta-balena recipes that fetch from recognizable git hosts; the remaining recipes still match OSV via the milestone-097 `mikebom:cpe-candidates` array per FR-019.

## Context and motivation

Yocto / OpenEmbedded meta-layer trees are a real and growing SBOM target class — embedded Linux distributions (balena-os, automotive OS images, IoT vendor stacks, the Raspberry Pi reference distro) ship as collections of `.bb` BitBake recipes inside layer directories. Today mikebom's milestone-107 source-tier path detects these layers and emits one component per `.bb` file's `name_version.bb` filename, but does NOT read the recipe body. The resulting SBOM has:

- **Zero LICENSE attribution** on any component (compliance scanners cannot judge legal posture).
- **Zero upstream-source provenance** — `SRC_URI` (the URL the recipe fetches from) and `SRCREV` (the git commit pin) are both ignored; vuln scanners have nothing to match against the actual upstream artifact.
- **Zero relationship edges** — `DEPENDS` and `RDEPENDS` are skipped, so the SBOM is a flat list, not a graph.
- **No layer attribution** — every component is detached from the meta-layer that ships it. `conf/layer.conf`'s `BBFILE_COLLECTIONS` (layer name) and `LAYERVERSION` are unread, so consumers cannot answer "which layer ships this recipe?"
- **No `.bbappend` provenance** — the 337 bbappends in `meta-balena` carry zero signal that they modify base recipes. The resulting SBOM cannot show "this recipe was patched by my layer."
- **PURL type uncertainty** — the current `pkg:bitbake/...` PURL is not clearly aligned to the purl-spec, which lists `pkg:openembedded/...` discussion threads but no published canonical type. Vuln scanners that do not understand `pkg:bitbake/...` silently drop these components.
- **No HOMEPAGE / SUMMARY / DESCRIPTION** — basic metadata operators expect on every component is absent.

The three balena projects in the user input illustrate the scale: `meta-balena` carries 163 recipes + 337 `.bbappend`s + 14 `layer.conf` files; `balena-raspberrypi` adds 12 + 30 + 1; `balena-generic` adds 1 + 9 + 1. Each of these meta-layers IS the BOM subject of any image built atop it, AND each is consumed by downstream balena-OS-image scans. Operators using these meta-layers today see no license, no provenance, no graph, no layer attribution — none of the signals an SBOM exists to carry.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Recipe-level license attribution (Priority: P1)

A compliance operator scans a Yocto meta-layer tree (e.g., `meta-balena`) to produce an SBOM for their legal review pipeline. The emitted SBOM carries the `LICENSE` field from every parseable `.bb` recipe on the corresponding component, mapped to SPDX expression syntax where possible.

**Why this priority**: License attribution is the single biggest gap for Yocto SBOMs today and the most-cited operator use case for embedded-Linux SBOMs. Without licenses, the SBOM cannot answer the most basic compliance question ("what are we shipping?"). syft v1.42, the LF SPDX tools, and bitbake's own `create-spdx.bbclass` all emit per-recipe licenses; mikebom emits none. Fixing this single behavior unblocks compliance use cases for every operator scanning a Yocto layer tree.

**Independent Test**: On a fresh clone of `balena-os/meta-balena`, running `mikebom sbom scan --path . --format spdx-2.3-json` produces an SBOM where ≥80% of recipe-derived `Package` entries carry a non-empty `licenseDeclared` field with a valid SPDX expression (or `NOASSERTION` only when the recipe explicitly declares `CLOSED` or omits LICENSE).

**Acceptance Scenarios**:

1. **Given** a `.bb` recipe with `LICENSE = "MIT"`, **When** the operator scans the layer tree, **Then** the corresponding component's `licenseDeclared` is the SPDX expression `MIT` across CDX `components[].licenses[]`, SPDX 2.3 `packages[].licenseDeclared`, and SPDX 3 `Package.declaredLicense`.
2. **Given** a recipe with `LICENSE = "GPL-2.0-only & LGPL-2.1-or-later"` (BitBake `&`-joined dual-license syntax), **When** the operator scans, **Then** the component carries a canonicalized SPDX expression equivalent to `GPL-2.0-only AND LGPL-2.1-or-later`.
3. **Given** a recipe with `LICENSE = "CLOSED"` (BitBake's proprietary marker), **When** the operator scans, **Then** the component's license field is `NOASSERTION` and a `mikebom:yocto-license-closed` annotation marks the recipe as proprietary (so downstream tools can distinguish "no license declared" from "explicitly proprietary").
4. **Given** a recipe whose LICENSE field couldn't be extracted (malformed body, unexpanded `${VAR}` reference), **When** the operator scans, **Then** the component carries `NOASSERTION` AND a `tracing::warn!` log line names the file path so the operator can audit it.

---

### User Story 2 — Source-pinned upstream provenance (Priority: P1)

A vulnerability-scanning operator scans the same meta-layer tree and wants components to carry an upstream-source pointer (a `vcs` external reference + an `SRCREV` commit pin where applicable) so downstream tools can match emitted components against the actual upstream artifact, not just the recipe's `<name>_<version>.bb` filename.

**Why this priority**: Same severity as US1. Today the recipe component PURL (`pkg:bitbake/u-boot@2024.07`) does NOT match against any upstream advisory feed — advisory feeds use git URLs / GitHub coordinates / upstream PURLs. Without `SRC_URI` + `SRCREV` parsing, mikebom's Yocto SBOM is invisible to every vuln-scanner pipeline that expects `pkg:github/...` or `pkg:generic/...?download_url=...` shapes. Operators using mikebom for vuln matching today see false-negative coverage on every Yocto component.

**Independent Test**: On a fresh clone of `meta-balena`, ≥60% of recipe-derived components carry a non-empty `externalReference` of type `vcs` (CDX) / `referenceType: "vcs"` (SPDX 2.3) / `externalIdentifier[].externalIdentifierType: "vcs"` (SPDX 3) populated from the recipe's `SRC_URI` git URL, AND ≥40% carry a `mikebom:srcrev` annotation pinning the upstream commit. The 60%/40% reflects the empirical mix of git-fetched vs http-fetched recipes in typical meta-layers.

**Acceptance Scenarios**:

1. **Given** a recipe with `SRC_URI = "git://github.com/balena-os/u-boot.git;branch=main;protocol=https"` and `SRCREV = "abc123..."`, **When** the operator scans, **Then** the component carries a `vcs` external reference `https://github.com/balena-os/u-boot.git` AND a `mikebom:srcrev` annotation with value `abc123...`.
2. **Given** a recipe with multiple `SRC_URI` entries mixing `git://`, `file://`, and `http://`, **When** the operator scans, **Then** the first `git://` (or `git+https://`) entry becomes the canonical `vcs` reference; `file://` (layer-local patches) and `http://` (tarball fetches) become `mikebom:src-uri` annotations preserving every entry for compliance auditability.
3. **Given** a recipe with `SRC_URI = "http://example.com/foo-1.0.tar.gz"` (tarball, no git), **When** the operator scans, **Then** the component carries an external reference of type `distribution` pointing at the tarball URL.
4. **Given** a recipe with `SRC_URI` referencing only `file://` paths (recipe builds entirely from in-tree patches), **When** the operator scans, **Then** no `vcs` reference is emitted AND a `mikebom:src-uri-local-only` annotation explains the absence so consumers know this recipe has no external upstream.

---

### User Story 3 — Layer attribution + layer-root BOM subject (Priority: P2)

An operator scanning a meta-layer tree wants every emitted recipe component annotated with its owning layer collection name (from `conf/layer.conf`'s `BBFILE_COLLECTIONS`) AND wants the SBOM root to identify the layer ITSELF (not `pkg:generic/<directory-basename>@0.0.0` as today). The milestone-127 root-selection ladder picks the layer-collection root via the FR-002 repo-root tiebreaker.

**Why this priority**: Per-component layer attribution is essential for downstream consumers correlating SBOMs across layer hierarchies (e.g., balena-OS image SBOMs that combine `meta-balena` + `balena-raspberrypi` + `balena-generic` + `poky` recipes — without layer attribution, consumers can't answer "which layer ships u-boot?"). Layer-root BOM subject closes the milestone-127 follow-up: today `meta-balena` scans emit `pkg:generic/meta-balena@0.0.0` as the BOM subject, which doesn't match the layer's actual collection name (`meta-balena-rust`, `meta-balena-bsp`, etc. at the per-layer level).

**Independent Test**: On a `meta-balena` scan, every recipe-derived component carries a `mikebom:yocto-layer` annotation naming the owning `BBFILE_COLLECTIONS` value (e.g., `meta-balena-rust` for recipes under `meta-balena-rust/`). The CDX `metadata.component`, SPDX 2.3 `documentDescribes`, and SPDX 3 `rootElement` all identify the workspace layer-collection name (e.g., `pkg:<yocto-type>/meta-balena-rust@<LAYERVERSION>`) rather than `pkg:generic/<directory-basename>@0.0.0`.

**Acceptance Scenarios**:

1. **Given** a layer directory with `conf/layer.conf` declaring `BBFILE_COLLECTIONS += "balena-generic"` and `LAYERVERSION_balena-generic = "1"`, **When** the operator scans, **Then** every recipe under that layer carries `mikebom:yocto-layer = "balena-generic"` AND `mikebom:yocto-layer-version = "1"`.
2. **Given** the same layer, **When** the operator scans, **Then** the BOM subject is the layer itself (a synthesized layer-root component carrying the collection name + version + `mikebom:component-role: "main-module"` annotation so milestone-127's FR-002 repo-root tiebreaker picks it).
3. **Given** a multi-layer tree (e.g., `meta-balena` contains 14 nested `conf/layer.conf`s), **When** the operator scans from the repo root, **Then** each recipe is attributed to its closest-ancestor layer's collection name; the milestone-127 FR-002/FR-003/FR-004 ladder picks one of them as the BOM subject per the operator's existing override-or-tiebreaker semantics.
4. **Given** a recipe whose layer.conf is missing or malformed, **When** the operator scans, **Then** the recipe still emits its component but carries no `mikebom:yocto-layer` annotation AND a `tracing::warn!` records the missing layer.conf path.

---

### User Story 4 — `.bbappend` provenance (Priority: P2)

An operator scanning a meta-layer tree where recipes have been customized via `.bbappend` files wants the appended recipe components to declare which appends modified them. This lets downstream consumers see "this u-boot was patched by my BSP layer" without inspecting the source tree.

**Why this priority**: Mid-priority because the appended-recipe component still gets emitted (just without the append signal), so the SBOM stays usable for the P1 license + provenance use cases. But appends are a first-class Yocto customization mechanism, and a Yocto SBOM that hides them mis-represents the layer hierarchy. The 337 bbappends in `meta-balena` are 2x the recipe count — appends are the dominant customization point.

**Independent Test**: On a `meta-balena` scan that walks both `.bb` and `.bbappend` files, every recipe component that has ≥1 matching `.bbappend` carries a `mikebom:bbappend-applied` annotation listing the relative paths of the appends modifying it.

**Acceptance Scenarios**:

1. **Given** a recipe `u-boot_2024.07.bb` AND a `u-boot_%.bbappend` file in another layer, **When** the operator scans both layers in one invocation, **Then** the `u-boot@2024.07` component carries a `mikebom:bbappend-applied` annotation listing the append's path.
2. **Given** a recipe with multiple matching appends across different layers (multi-layer customization), **When** the operator scans, **Then** the annotation lists ALL matching appends in deterministic (lex-sorted) order.
3. **Given** an `.bbappend` file that doesn't match any recipe in the scan (orphan append from another not-included layer), **When** the operator scans, **Then** the append is recorded via a `tracing::warn!` log naming the orphan AND mikebom does NOT emit a phantom component for it (out-of-scope per Constitution VIII completeness).

---

### User Story 5 — DEPENDS / RDEPENDS relationship edges (Priority: P3)

The operator wants the emitted SBOM to carry `DEPENDS_ON` relationship edges between recipes, derived from each recipe's `DEPENDS` (build-time) and `RDEPENDS` (runtime) declarations.

**Why this priority**: Lower priority because the recipe component set is still complete and license-attributed without edges. But edges are what makes an SBOM a *graph*, not a list — operators using the SBOM for impact analysis ("which recipes depend on u-boot?") need them. P3 because the parsing complexity is meaningful (RDEPENDS can reference package names that differ from recipe names per PROVIDES; multi-architecture overrides) and the P1+P2 stories unblock the dominant use cases first.

**Independent Test**: On `meta-balena`, the emitted SBOM contains ≥50 `DEPENDS_ON` edges derived from recipe `DEPENDS` declarations, AND a separate `mikebom:rdepends` annotation counts the runtime deps per recipe.

**Acceptance Scenarios**:

1. **Given** a recipe with `DEPENDS = "libssl-dev openssl"`, **When** the operator scans, **Then** the component emits `DEPENDS_ON` edges to the `libssl-dev` and `openssl` recipe components (when those components also exist in the scan).
2. **Given** a `DEPENDS` entry that doesn't resolve to any scanned recipe (e.g., `DEPENDS = "external-binary-tool"` where the tool ships in another not-scanned layer), **When** the operator scans, **Then** the unresolvable name is recorded under `mikebom:depends-unresolved` annotation (not silently dropped) so consumers see the closure gap.
3. **Given** a recipe with override-syntax DEPENDS (e.g., `DEPENDS:append:rpi4 = "foo"`), **When** the operator scans, **Then** the override is treated as if applied unconditionally (mikebom doesn't model BitBake variable evaluation; emit the union of base + all overrides) AND the recipe-scope `mikebom:yocto-overrides-merged: true` annotation (per FR-016) reflects this approximation.

---

### Edge Cases

- **`${PN}_${PV}.bb` shared-base recipes paired with `.inc` companions** — today silently skipped per milestone-107 FR-008. The new reader SHOULD resolve `${PN}` and `${PV}` from the recipe directory's siblings when both are co-present.
- **`require recipes-foo/foo.inc` / `include foo.inc` directives** — recipes split fields across `.bb` + `.inc` + (transitively) more `.inc`s. The reader MUST follow `require` and `include` directives transitively (with a depth bound, e.g., 8) to find LICENSE / HOMEPAGE / SRC_URI / SRCREV.
- **`LICENSE:append:<override>` syntax** — override-syntax LICENSE additions append to base. mikebom MUST merge them (concatenate with `&`/`AND` per BitBake semantics) when computing the canonical license expression.
- **`SRC_URI:remove:<override>` syntax** — override-syntax SRC_URI removal subtracts entries from the base list. Document the chosen approximation (treat as union, or skip the override entirely) in Assumptions.
- **Multi-arch `SRCREV_machine:<arch>` overrides** — each machine target has its own SRCREV pin. Emit one component per (recipe, machine) pair? Or merge into a single component with a `mikebom:srcrev-by-machine` annotation array? Defer to clarification.
- **Recipes with `BBCLASSEXTEND = "native nativesdk"`** — one recipe spawns three build flavors (target / native / nativesdk). Today milestone-107 emits one component. Should each flavor be a separate component (matching BitBake's actual emission)? Defer to clarification.
- **Image-tier rootfs scans** (e.g., scanning `build/tmp/deploy/images/<machine>/<image>.tar.gz`) — already covered by milestone 107's `yocto/manifest.rs` + `opkg.rs`. This feature is source-tier scope only; image-tier behavior unchanged.
- **Layer with two `conf/layer.conf` files** (rare but legal) — emit one main-module per layer.conf (matches milestone-127's existing multi-main-module flow).
- **`LICENSE = "${PN}"` (variable-substituted LICENSE)** — variable expansion is out of scope; treat as unresolvable, emit `NOASSERTION` + warn.
- **`SRC_URI = ""` (empty)** — recipe is built entirely from in-tree files (often kernel patches). Emit the layer-local annotation per US2 AC#4.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST parse the `LICENSE` field from every `.bb` and `.inc` file in the scan target. Multi-line LICENSE values, `LICENSE:append:<override>` syntax, and `LICENSE = "&"`/`|`/`AND`/`OR` combinations MUST canonicalize to an SPDX expression via the existing `mikebom-common::types::license::SpdxExpression` round-trip. Invalid / unresolvable LICENSE values emit `NOASSERTION` + warn.

- **FR-002**: System MUST parse the `SRC_URI` field from every `.bb` and `.inc` file. Each `git://` / `git+https://` / `git+ssh://` URI MUST be normalized to an `https://` vcs URL and recorded as the component's `vcs` external reference. `file://` and `http://`/`https://` (tarball) URIs MUST be preserved in a `mikebom:src-uri` annotation array.

- **FR-002a** (OSV-direct-match optimization): When the parsed `SRC_URI` contains a `git://` / `git+https://` / `git+ssh://` URI AND the URI's host matches one of `{github.com, gitlab.com, bitbucket.org, codeberg.org}` AND `SRCREV` is set, System MUST emit a host-typed PURL (`pkg:github/<owner>/<repo>@<srcrev-12-hex>` for github.com; `pkg:gitlab/<owner>/<repo>@<srcrev-12-hex>` for gitlab.com; etc.) as the primary PURL — NOT FR-011's `pkg:generic/...` fallback. The 12-hex SRCREV prefix matches FR-018's version-derivation rule (lowercased). The original recipe name + version + layer attribution preserve via these new annotations:
  - `mikebom:yocto-recipe-name = "<recipe-name>"` — the recipe's filename-derived name
  - `mikebom:yocto-recipe-version = "<recipe-version>"` — the recipe's filename-derived version (pre-FR-018 substitution; matches the operator's mental model of the recipe identity)
  - `mikebom:yocto-layer = "<collection>"` — per FR-006 (unchanged)
  When SRC_URI is non-git OR the host is not in the recognized set OR SRCREV is absent, System MUST fall through to FR-011's `pkg:generic/<recipe-name>@<version>?openembedded=true&layer=<collection>` shape unchanged. Drives SC-012 — the OSV-match-rate success criterion on `meta-balena`.

- **FR-003**: When `SRCREV` is present alongside a git `SRC_URI`, System MUST emit the SRCREV as a `mikebom:srcrev` annotation. Multi-arch `SRCREV_machine:<arch>` overrides MUST emit a `mikebom:srcrev-by-machine` annotation as a JSON object `{<arch>: <hex>, ...}`.

- **FR-004**: System MUST follow `require` and `include` directives transitively (depth bound 8, mirroring milestone 107's existing walker convention) to merge fields from `.inc` companions into their referencing `.bb` recipes. Field-precedence semantics: **last-write-in-source-order wins** per BitBake's native evaluation order. For each recipe, process the included `.inc` files' field assignments first (in include-order), then process the referencing `.bb`'s assignments; conflicting fields take the `.bb`'s value. Override-syntax (`FIELD:append:<override>` / `FIELD:prepend:<override>`) is then applied per FR-016's union-merge. Cyclic include chains MUST be detected and broken (the cycle's tail include is silently dropped + a `tracing::debug!` log emitted).

- **FR-005**: System MUST resolve `${PN}` and `${PV}` from the filename of the referencing `.bb` (where `${PN}` is the recipe name and `${PV}` is the version segment per Yocto convention). Other BitBake variables (`${BPN}`, `${DISTRO}`, `${MACHINE}`, etc.) are NOT expanded — recipes referencing them as part of identity fields emit a `mikebom:yocto-unexpanded-vars` annotation naming the unresolved variables.

- **FR-006**: System MUST parse `conf/layer.conf` for `BBFILE_COLLECTIONS`, `LAYERVERSION_<collection>`, and `LAYERSERIES_COMPAT_<collection>` fields. Recipe-to-layer attribution uses the **nearest-ancestor `conf/layer.conf`** heuristic: walk each recipe's filesystem path upward until the first directory containing `conf/layer.conf` is found, and attribute the recipe to that layer. `BBFILES` patterns are NOT parsed (the heuristic matches the conventional `<layer>/recipes-*/<dir>/*.bb` layout used by all three motivating fixtures). Every attributed recipe carries `mikebom:yocto-layer`, `mikebom:yocto-layer-version`, and `mikebom:yocto-layer-series` annotations. Recipes with no ancestor `conf/layer.conf` carry no layer annotations + emit a `tracing::warn!` log per US3 AC#4.

- **FR-007**: System MUST emit one main-module-tagged component per detected `conf/layer.conf` so milestone-127's root-selection ladder elects the layer at the scan root as the BOM subject. The layer main-module's PURL uses the type token resolved per FR-011, name `<BBFILE_COLLECTIONS>`, version `<LAYERVERSION>`.

- **FR-008**: System MUST walk for `.bbappend` files alongside `.bb` files. When an append's basename (with `.bbappend` stripped, `_%` glob expanded) matches an existing recipe's basename, the recipe component MUST receive a `mikebom:bbappend-applied` annotation listing the matching append paths (lex-sorted, deduped). Orphan appends (no matching recipe in scan) MUST emit a `tracing::warn!` and NOT produce phantom components.

- **FR-009**: System MUST parse `DEPENDS` and `RDEPENDS_<package>` fields. For each entry that resolves to a recipe component also present in the scan, emit a `DEPENDS_ON` relationship edge (build scope for `DEPENDS`, runtime scope for `RDEPENDS`). Unresolved entries MUST be recorded under `mikebom:depends-unresolved` / `mikebom:rdepends-unresolved` annotations (NOT silently dropped) so consumers see closure gaps.

- **FR-010**: System MUST parse `HOMEPAGE`, `SUMMARY`, and `DESCRIPTION` fields. `HOMEPAGE` MUST be recorded as an external reference of type `website`. `SUMMARY` MUST populate `component.description` (CDX) / `Package.summary` (SPDX 3) / `Package.summary` (SPDX 2.3). `DESCRIPTION` MUST be preserved in a `mikebom:yocto-description` annotation when it differs from SUMMARY.

- **FR-011**: System MUST emit Yocto recipe components as `pkg:generic/<recipe-name>@<version>?openembedded=true&layer=<collection>` (Clarifications Q1). The `openembedded=true` qualifier carries the Yocto-source signal; the `layer=<collection>` qualifier carries the owning layer's `BBFILE_COLLECTIONS` value when known (omitted when no ancestor `conf/layer.conf` was found). Aligns with the upstream Yocto-tooling convention observed in the 145-component balena-OS reference SBOM. Existing `pkg:bitbake/` callers MUST be migrated; CHANGELOG documents this as a behavior change with byte-identity exemption (Yocto recipe components are not in the 33 alpha.48 goldens).

- **FR-012**: When `LICENSE = "CLOSED"` (BitBake's proprietary marker), System MUST emit `NOASSERTION` as the license expression AND emit a `mikebom:yocto-license-closed` annotation so consumers can distinguish "no license declared" from "proprietary by declaration".

- **FR-013**: All 17 new annotation keys (`mikebom:srcrev`, `mikebom:src-uri`, `mikebom:srcrev-by-machine`, `mikebom:yocto-layer`, `mikebom:yocto-layer-version`, `mikebom:yocto-layer-series`, `mikebom:bbappend-applied`, `mikebom:depends-unresolved`, `mikebom:rdepends-unresolved`, `mikebom:yocto-unexpanded-vars`, `mikebom:yocto-license-closed`, `mikebom:yocto-description`, `mikebom:src-uri-local-only`, `mikebom:yocto-class-extend`, `mikebom:yocto-overrides-merged`, `mikebom:yocto-recipe-name`, `mikebom:yocto-recipe-version` — the last two carry the recipe-identity provenance when FR-002a emits a host-typed PURL pointing at the upstream artifact) MUST receive parity-catalog C-rows (C70..C86) AND pass the Principle V native-field audit before merge. FR-019 reuses the existing milestone-097 `mikebom:cpe-candidates` annotation; no new key needed.

- **FR-014**: The new fields MUST NOT break milestone-107's existing image-manifest / opkg-installed / context detectors. Existing fixture-based tests for image-tier Yocto scans MUST remain byte-identical.

- **FR-014a**: On mixed scans (meta-layer tree AND a built rootfs producing opkg-DB + image-manifest components in the same scan), the recipe reader and the milestone-107 image-tier readers MUST emit independently. The existing milestone-105 PURL-based dedup pipeline is the single cross-source merge point; recipe-reader fields (LICENSE, SRC_URI, layer attribution) propagate onto the post-dedup winning component as `mikebom:also-detected-via` evidence. NO explicit cross-reader correlation pass is introduced by this milestone.

- **FR-015**: System MUST handle the three motivating balena fixtures (`meta-balena`, `balena-raspberrypi`, `balena-generic`) end-to-end without crash or panic — see Success Criteria SC-001 through SC-008 for measurable thresholds.

- **FR-016**: System MUST handle override-syntax fields (`FIELD:append:<override>`, `FIELD:remove:<override>`, `SRCREV_machine:<arch>`) by **merging as union** — taking the unconditional combination of base + every override (Clarifications Q2). NO operator-selectable machine/distro mask flags. Justification: the Yocto-tooling-native CDX emission IS itself a union upper bound; mikebom matches the ecosystem convention. Operators wanting machine-precise output should run `bitbake -e <recipe>` against their target — that's the right tool for that job. A `mikebom:yocto-overrides-merged` annotation on every recipe with ≥1 override signals the union-emission semantic to consumers.

- **FR-017** (CPE-name normalization): When emitting CPE candidates for a Yocto recipe component, System MUST apply the openembedded-core recipe-to-CPE-product-name mapping table (e.g., `linux-kernel` → `linux_kernel`, `nss` → `network_security_services`, `dropbear` → `dropbear_ssh`, `nspr` → `netscape_portable_runtime`). The normalized name appears in the `mikebom:cpe-candidates` annotation array entries (per FR-019); the human-readable recipe name stays unchanged in the PURL `name` segment. The mapping table SHOULD be embedded from `meta/conf/distro/include/cve-extra-exclusions.inc`-style data so future Yocto releases can refresh it without code change. Driven by NVD vuln-matching parity with the Yocto-tooling-native emission.

- **FR-018** (version derivation from SRCREV): System MUST reject the `version: "git"` and `version: "<base>.AUTOINC+<sha>"` anti-patterns observed in the Yocto-tooling-native CDX emission. When a recipe's effective `PV` is literally the string `git` OR contains the BitBake `AUTOINC` token, System MUST derive the emitted PURL version segment from `SRCREV` (first 12 hex chars, lowercased) AND emit the full SHA as a `mikebom:srcrev` annotation. When `SRCREV` is also absent (rare, malformed recipe), the component MUST be skipped with a `tracing::warn!` log naming the file. Cleaner identifiers (`pkg:generic/mobynit@f597fb026637` instead of `pkg:generic/mobynit@git`) are sortable, comparable across releases, AND vuln-scanner compatible.

- **FR-019** (CPE-candidates over multi-component fan-out): When a Yocto recipe has multiple plausible CPE vendor/product candidates (e.g., `curl` could be `daniel_stenberg/curl`, `haxx/curl`, `curl/curl`, `libcurl/libcurl`), System MUST emit ONE component carrying every candidate in the existing milestone-097 `mikebom:cpe-candidates` annotation array. System MUST NOT emit separate components per vendor (the Yocto-tooling-native fan-out approach — the balena-OS reference SBOM has 6 components for `curl` and 3 for `dbus`). The single-component-multi-candidate shape is the canonical mikebom approach since milestone 097; this FR makes it explicit for Yocto recipes too.

### Key Entities *(include if feature involves data)*

- **BitBake recipe** — a `.bb` file describing how to fetch, configure, compile, and package a software component. Identity is `(<name>, <version>)` from the filename; metadata is in the file body (LICENSE, SRC_URI, SRCREV, HOMEPAGE, SUMMARY, DESCRIPTION, DEPENDS, RDEPENDS).
- **BitBake include / require** — `.inc` files providing shared field values to multiple recipes via `include <path>` / `require <path>` directives. Field inheritance is upward-only (the requiring `.bb` inherits the included `.inc`'s fields).
- **BitBake append** — a `.bbappend` file matching a base recipe by `name_version_glob` and adding fields to it. Multi-layer customization mechanism.
- **Yocto layer** — a directory containing `conf/layer.conf` declaring `BBFILE_COLLECTIONS` (the layer's stable name) and `LAYERVERSION_<name>` (the layer's version). All `.bb` files under the layer's BBFILES path belong to it.
- **Layer collection** — the canonical name of a layer (e.g., `meta-balena-rust`, `meta-balena-bsp`). Used as the layer-root BOM subject identity per FR-007.
- **Yocto recipe component** — the emitted SBOM component representing one recipe. Existing entity from milestone 107; this feature enriches it with the new fields.
- **`mikebom:yocto-*` annotation family** — new internal annotations covering layer attribution, append provenance, source pinning, and unresolved-dependency tracking. Each gets a parity-catalog C-row per FR-013.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A fresh scan of `balena-os/meta-balena@master` produces an SBOM where ≥80% of recipe-derived components carry a non-empty `licenseDeclared` SPDX expression (excluding only recipes where LICENSE is `CLOSED` or where extraction failed).

- **SC-002**: The same scan emits ≥60% of recipe components with a `vcs` external reference populated from `SRC_URI` AND ≥40% with a `mikebom:srcrev` annotation pinning the upstream commit.

- **SC-003**: Every recipe component in the same scan carries a `mikebom:yocto-layer` annotation naming its owning `BBFILE_COLLECTIONS` value. Recipes without an ancestor `conf/layer.conf` are noted in a `tracing::warn!` log.

- **SC-004**: The BOM subject (CDX `metadata.component`, SPDX 2.3 `documentDescribes`, SPDX 3 `rootElement`) of a `meta-balena` scan identifies a layer-collection-named component (e.g., `meta-balena-rust`) via milestone-127's FR-002 / FR-003 / FR-004 root-selection ladder. NOT `pkg:generic/meta-balena@0.0.0` as today.

- **SC-005**: The emitted SBOM carries ≥50 `DEPENDS_ON` relationship edges derived from recipe `DEPENDS` parsing on `meta-balena`.

- **SC-006**: All 33 alpha.48 byte-identity goldens stay byte-identical AND all existing milestone-107 image-manifest / opkg-installed fixtures stay byte-identical (this feature only changes source-tier `.bb` recipe-walker output).

- **SC-007**: sbomqs (the SBOM-Quality Score harness, milestone 060's reference tool) score on a `meta-balena` SBOM improves from the current ~25% baseline (zero licenses, zero provenance) to ≥55% after this feature lands. The 30-point jump reflects sbomqs's `Sharing` (license clarity) + `Provenance` (source pinning) + `Structural` (relationship edges) score weights.

- **SC-008**: A `balena-raspberrypi` scan + a `meta-balena` scan + a `balena-generic` scan all complete in under 30 seconds on the existing milestone-094 perf baseline (these are small layer trees; the recipe-body parsing adds <2× the milestone-107 baseline cost).

- **SC-009** (FR-017): A `meta-balena` scan produces ≥10 recipe components whose `mikebom:cpe-candidates` array includes a CPE-normalized name (e.g., `linux_kernel`, `network_security_services`, `dropbear_ssh`, `netscape_portable_runtime`) — proving the normalization table fires on real recipes.

- **SC-010** (FR-018): No emitted Yocto recipe component carries `version: "git"` OR `version: "<base>.AUTOINC+<sha>"`. Every git-fetched recipe with unstable `PV` emits a SRCREV-derived version (12-char hex prefix) AND a `mikebom:srcrev` annotation with the full SHA.

- **SC-011** (FR-019): For multi-CPE-vendor recipes (e.g., `curl`, `dbus`, `zlib`), the emitted SBOM has EXACTLY ONE component per `(recipe, version)` pair — NOT the Yocto-tooling-native fan-out (6 components for `curl`, 3 for `dbus`). The vendor permutations live in the `mikebom:cpe-candidates` array on the single component.

- **SC-012** (FR-002a): A `meta-balena` scan produces ≥40% of recipe-derived components with a host-typed PURL (`pkg:github/...`, `pkg:gitlab/...`, `pkg:bitbucket/...`, or `pkg:codeberg/...`) instead of the `pkg:generic/...` fallback. The 40% threshold reflects the empirical mix of git-vs-tarball recipes in meta-balena where ≥40% of recipes fetch from github.com (verified by surveying the SRC_URI fields in meta-balena's 163 recipes). Drives OSV-direct-match coverage: every host-typed PURL is queryable against OSV via the github/gitlab/bitbucket ecosystem mappings, returning advisories without needing CPE-matching fallback.

## Assumptions

- **PURL type for recipe components is `pkg:generic/<recipe-name>@<version>?openembedded=true&layer=<collection>`** per Clarifications Q1 + FR-011. Justified by the upstream Yocto-tooling convention observed in a production balena-OS reference SBOM (145 components, all `pkg:generic/...`). The qualifiers carry the Yocto-specific signals.

- **BitBake variable expansion is out of scope.** The reader does NOT execute BitBake's metadata-evaluation engine. Variables `${PN}` and `${PV}` are resolved from filename only; all others (`${BPN}`, `${DISTRO}`, `${MACHINE}`, `${PV_MAJOR}`, etc.) are treated as unresolvable. Recipes whose identity-relevant fields reference unresolved variables emit `mikebom:yocto-unexpanded-vars` annotations naming the affected variables, then continue with best-effort.

- **`require` / `include` depth bound is 8.** Matches milestone 107's walker convention. Deep include chains are rare in practice (Poky core uses ~3 levels).

- **Override-syntax (`FIELD:append:<override>`, `FIELD:remove:<override>`)** is approximated by MERGING (union of base + every override) per Clarifications Q2 + FR-016. No operator-selectable mask flags are added in this milestone. Matches the upstream Yocto-tooling-native CDX emission semantic (the balena-OS reference SBOM is itself a union upper bound). Consumers wanting precise per-machine output should run bitbake's own emission against their target machine.

- **`BBCLASSEXTEND = "native nativesdk"`** treats each flavor as the same component (one PURL emitted) with a `mikebom:yocto-class-extend` annotation naming the additional flavors. Matches the existing milestone-107 emission shape.

- **Image-tier scans** (the milestone-107 `yocto/manifest.rs` + `opkg.rs` + `yocto/context.rs` paths) are UNCHANGED by this feature. Source-tier-only scope per the milestone-127 precedent for source-tier-only specs.

- **The three balena meta-layers are the motivating fixtures**, NOT prescriptive goldens. Their per-scan thresholds (SC-001 ≥80%, SC-002 ≥60%, etc.) reflect the empirical mix of recipe shapes in those layers; actual deployment fixtures may differ. The goldens land in `mikebom-cli/tests/fixtures/yocto_recipe_enrich/` as small synthesized trees, NOT as full balena clones (per the milestone-090 fixture-cache convention).

- **No new Cargo dependencies.** Recipe body parsing uses `std::str::Lines` + existing `regex` (already a workspace dep from milestone 113 / milestone 127). `mikebom-common::types::license::SpdxExpression::try_canonical` handles the SPDX canonicalization. No new crates added to the dependency closure.

- **Rust stable, user-space only.** No nightly, no eBPF interaction. The reader runs in the same `scan_path` pass as milestone 107's existing readers.

- **No new CLI flags.** The reader fires whenever the existing milestone-107 `.bb`-walker fires (i.e., whenever the scan target contains `.bb` files). The new fields populate automatically; operators get the enriched output without changing their scan command.

- **The CPE-name normalization table (FR-017) ships as an embedded const slice**, sourced from `meta/conf/distro/include/cve-extra-exclusions.inc`-style data. The mapping is finite and stable across releases; a future Yocto release would prompt a minor mikebom update to refresh the table. Not driven by a runtime fetch — the table lives in the build.

- **Reference SBOM evidence base.** The Q1/Q2/FR-017/FR-018/FR-019 decisions are grounded in a production balena-OS bitbake-emitted CDX (145 components, attached to the spec session 2026-06-18 record). Observed patterns: `pkg:generic/` universally adopted, "version: git" + "AUTOINC+<sha>" smell on three components, six `curl` components fanning out CPE-vendor candidates, `linux_kernel` + `network_security_services` CPE-name normalization in evidence.

- **`mikebom:yocto-*` annotation namespace.** All new annotations carry the `mikebom:yocto-` prefix so they're discoverable in one grep and so future Yocto-related work has an established namespace boundary.

- **The parity-catalog C-rows are sequential from C70+.** Working assumption: the next-free C-row at PR time. Verified per the milestone-127 precedent (research R7).
