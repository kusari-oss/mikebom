# Phase 0: Research — Deeper Yocto / OpenEmbedded SBOM coverage

This research note resolves the open implementation questions left after `/speckit-clarify` and grounds the milestone-128 design in observable evidence (the 145-component balena-OS reference CDX and the three balena meta-layer clones).

## R1 — Native-field audit per Principle V (all 15 new keys)

**Decision**: Every new `mikebom:yocto-*` annotation key gets a Principle V row in `docs/reference/sbom-format-mapping.md` justifying its existence against the CDX 1.6 / SPDX 2.3 / SPDX 3.0.1 native field set. The major standard-native carriers are USED where they exist; only the parity-bridging signals get a `mikebom:*` key.

| New annotation | Standards-native first? | Native field used | `mikebom:*` justification |
|---|---|---|---|
| LICENSE (extracted) | ✓ | CDX `components[].licenses[]` (acknowledgement: declared), SPDX 2.3 `Package.licenseDeclared`, SPDX 3 `Package.declaredLicense` | None — no `mikebom:*` key needed |
| HOMEPAGE (extracted) | ✓ | CDX/SPDX `externalReferences[type=website]` | None |
| SRC_URI git | ✓ | CDX/SPDX `externalReferences[type=vcs]` | None |
| SRC_URI tarball | ✓ | CDX/SPDX `externalReferences[type=distribution]` | None |
| SUMMARY | ✓ | CDX `component.description` (single-string), SPDX `Package.summary` | None |
| DESCRIPTION | ✓ (when differs from SUMMARY) | CDX `component.notes` (or property), SPDX `Package.description` | `mikebom:yocto-description` only when DESCRIPTION differs materially from SUMMARY |
| `mikebom:srcrev` | ✗ (parity bridge) | n/a | None of CDX/SPDX has a "commit SHA pinning the vcs ref" field. CDX `externalReferences[].comment` is free-form prose; not structured. Bridge needed. |
| `mikebom:src-uri` | ✗ (parity bridge) | n/a | Carries the full list of SRC_URI entries (including `file://` and `http://` tarballs) for compliance auditability. No native field carries "every fetch source the recipe uses." |
| `mikebom:srcrev-by-machine` | ✗ (parity bridge) | n/a | Multi-arch SRCREV pins. No native field carries "per-machine commit pins." |
| `mikebom:yocto-layer` | ✗ (parity bridge) | n/a | Layer-collection name. No native CDX/SPDX field models Yocto's layer-attribution concept. CDX `component.group` is for namespace, not source-of-truth attribution. |
| `mikebom:yocto-layer-version` | ✗ (parity bridge) | n/a | Layer version (`LAYERVERSION_<collection>`). |
| `mikebom:yocto-layer-series` | ✗ (parity bridge) | n/a | Layer series compatibility (`LAYERSERIES_COMPAT_<collection>`). |
| `mikebom:bbappend-applied` | ✗ (parity bridge) | n/a | BitBake-specific append-customization tracking. No native equivalent. |
| `mikebom:depends-unresolved` | ✗ (parity bridge) | n/a | Closure-gap transparency. Principle X mandates surfacing unresolved deps; no native field carries this. |
| `mikebom:rdepends-unresolved` | ✗ (parity bridge) | n/a | Same as above for runtime deps. |
| `mikebom:yocto-unexpanded-vars` | ✗ (parity bridge) | n/a | Transparency for ${VAR} references the reader couldn't expand. No native field. |
| `mikebom:yocto-license-closed` | ✗ (parity bridge) | n/a | Distinguishes "proprietary by declaration (LICENSE=CLOSED)" from "no license declared." CDX/SPDX have NOASSERTION but not the discriminator. |
| `mikebom:src-uri-local-only` | ✗ (parity bridge) | n/a | Signals "recipe is layer-local only, no external upstream." |
| `mikebom:depends-overrides-merged` | ✗ (parity bridge) | n/a | FR-016 transparency: signals the union-merge approximation. No native field carries "we approximated this." |
| `mikebom:yocto-class-extend` | ✗ (parity bridge) | n/a | `BBCLASSEXTEND` flavor signal. No native equivalent. |
| `mikebom:yocto-overrides-merged` | ✗ (parity bridge) | n/a | FR-016 transparency at the recipe level. |

**Rationale**: Principle V (post v1.4.0) requires every new `mikebom:*` annotation to first audit each target format for a native construct. The audit shows: license, homepage, SRC_URI (as vcs/distribution external refs), summary, and description ALL go native; no `mikebom:*` key needed for them. The remaining 15 keys carry Yocto-specific signals no standard format models — each is a parity-bridge addition justified by Constitution Principle X transparency requirements.

**Alternatives considered**:

- Use CDX `component.evidence.identity[]` for the parity-bridging signals. Rejected: that field is per-component identification confidence, not document-scope signals like layer attribution.
- Use CDX `metadata.lifecycles` for layer attribution. Rejected: that's for build-lifecycle phases (build / runtime / install), not for layer-source attribution.
- Inline the layer name into the PURL `<namespace>/<name>` segment instead of a qualifier. Rejected: `pkg:generic/<layer>/<name>` would break vuln-scanner matching since the layer isn't part of the upstream identity; `?layer=<collection>` qualifier is the right shape (scanners ignore qualifiers they don't understand).

## R2 — CPE-name normalization mapping source (FR-017)

**Decision**: Embed the openembedded-core CVE-extra-exclusions table as a compile-time `&'static [(recipe: &str, cpe_product: &str)]` slice in `cpe_name_map.rs`. Source: `meta/conf/distro/include/cve-extra-exclusions.inc` from openembedded-core's master branch (~50-80 entries; stable across releases).

**Rationale**: The mapping is finite, stable, and small enough to embed without bloating the binary. Embedding avoids the runtime-fetch / cache-staleness concerns that would come with a network or file-based table. A future Yocto release that adds new mappings would prompt a minor mikebom update — same maintenance shape as the existing PURL-spec mapping in `mikebom_common::types::purl`.

The actual entries (sampled from the reference SBOM's normalizations):

```rust
const CPE_NAME_MAP: &[(&str, &str)] = &[
    ("linux-kernel", "linux_kernel"),
    ("nss", "network_security_services"),
    ("nspr", "netscape_portable_runtime"),
    ("dropbear", "dropbear_ssh"),
    ("zstd", "zstandard"),
    // ... ~50 more from cve-extra-exclusions.inc
];
```

**Alternatives considered**:

- Fetch the table from openembedded-core at scan time. Rejected: requires network access during scans (violates milestone-107's "no external data" posture) AND requires an offline-cache fallback.
- Ship the table as a separate data file shipped with the binary. Rejected: same reliability concerns + filesystem-layout coupling.
- Skip the normalization entirely; rely on milestone-097's existing CPE-candidates synthesizer. Rejected: the synthesizer doesn't know about `linux_kernel` vs `linux-kernel` or `network_security_services` vs `nss` — those mappings are domain-specific to NVD's CPE dictionary.

## R3 — Reference SBOM analysis (the 145-component balena-OS CDX)

**Decision**: The reference SBOM provided by the user IS the empirical baseline mikebom must measurably beat. Key observations consumed into the spec:

1. **PURL convention**: `pkg:generic/<name>@<version>` is the upstream Yocto-tooling default. mikebom adopts the same shape with `?openembedded=true&layer=<collection>` qualifiers (FR-011).
2. **Multi-CPE fan-out**: `curl` appears 6 times as separate components; `dbus` 3 times. mikebom emits ONE component with all candidates in the milestone-097 array (FR-019).
3. **CPE-name normalization**: `linux_kernel`, `network_security_services`, `dropbear_ssh`, `netscape_portable_runtime` are present in the reference. mikebom applies the same mapping (FR-017).
4. **"version: git" smell**: `libnss-ato@git`, `tcgtool@git`, `mobynit@git` — the upstream tooling emits literal "git" when PV is unresolvable. mikebom rejects this; derives version from SRCREV (FR-018).
5. **"AUTOINC+<sha>" smell**: `bindmount@0.0.4.AUTOINC+f597fb0266` — the SRCREV gets mashed into the version string. mikebom splits it: version is `0.0.4` (or SRCREV-derived if PV is fully AUTOINC), SRCREV is in `mikebom:srcrev` (FR-018).
6. **Missing fields**: license, dependencies, metadata.component, externalReferences ALL absent from the reference. mikebom adds them via US1, US2, US3, US5.
7. **`group` field**: when vendor is known (e.g., `gnu`/`oberhumer`/`mobyproject`), the reference puts it in CDX `component.group`. mikebom uses the existing `<namespace>` PURL segment + `component.group` (already populated by the milestone-077 emitter); no new code needed.

**Rationale**: The reference SBOM is the operator's existing reality. Every milestone-128 design decision was checked against it. Where the reference is good (PURL convention, CPE-name normalization), mikebom aligns. Where the reference is bad (version-git smell, multi-CPE fan-out, missing licenses), mikebom does better.

**Alternatives considered**:

- Treat the reference SBOM as authoritative — emit identical shape. Rejected: the reference is missing the headline value-add (LICENSE) and contains anti-patterns we'd be propagating.
- Ignore the reference; ship a from-scratch design. Rejected: the reference is what consumers expect today; aligning where sensible reduces friction.

## R4 — BitBake parsing: scope of body-parser

**Decision**: Line-oriented regex parser that recognizes the BitBake assignment grammar:

```
FIELD = "value"
FIELD ?= "value"           (weak assignment; treated identically to =)
FIELD ??= "value"          (default assignment; only fires if FIELD undefined)
FIELD += "value"           (append-with-space)
FIELD =+ "value"           (prepend-with-space)
FIELD .= "value"           (append-no-space)
FIELD =. "value"           (prepend-no-space)
FIELD:append:<override> = "value"     (override-syntax; merged per FR-016)
FIELD:prepend:<override> = "value"    (same)
FIELD:remove:<override> = "value"     (subtract from base; mikebom treats as merge per FR-016 caveat)
```

Multi-line values via `\` line-continuation. Variable expansion limited to `${PN}` and `${PV}` per FR-005; all other `${...}` references emit `mikebom:yocto-unexpanded-vars`. `require <path>` and `include <path>` directives resolved per FR-004 with depth bound 8 and cycle detection.

**Rationale**: This is the smallest grammar that covers the BitBake fields mikebom needs (LICENSE, SRC_URI, SRCREV, HOMEPAGE, SUMMARY, DESCRIPTION, DEPENDS, RDEPENDS, BBCLASSEXTEND). It does NOT cover BitBake's full evaluation engine (no function definitions, no `inherit` class processing, no Python expressions). The spec explicitly scopes those out (Assumptions).

**Alternatives considered**:

- Shell out to `bitbake -e <recipe>` to get the fully-evaluated metadata. Rejected: requires bitbake installed (Constitution Principle I-style violation: introduces external runtime dep), wildly heavyweight for recipe-body parsing, and would couple mikebom to bitbake's version evolution.
- Use a published BitBake-grammar parser crate. Rejected: no such crate exists in the Rust ecosystem; bringing one in would violate the "zero new Cargo deps" constraint.

## R5 — Field-precedence semantics for `.bb` vs `.inc` (clarification Q1)

**Decision**: Last-write-in-source-order wins. Process each include in include-order; for each include, process its assignments in source-order. The `.bb` is the LAST file processed; its assignments override conflicting earlier ones.

**Rationale**: This is what `bitbake -e <recipe>` actually does. Any other rule produces SBOMs that disagree with bitbake's view of the same recipe. The Q1 clarification chose this explicitly.

**Implementation note**: in code, each parsed `.inc` returns a `RecipeMetadata` struct that the parent `.bb`'s parsing MERGES into (last-write-wins). The merge function is a simple per-field update (`if recipe_value.is_some() { merged_value = recipe_value }`).

## R6 — Layer-attribution heuristic (clarification Q2)

**Decision**: Nearest-ancestor `conf/layer.conf` directory wins. Walk each recipe's filesystem path upward until a directory containing `conf/layer.conf` is found. No `BBFILES`-pattern parsing.

**Rationale**: This is correct for ≥99% of real meta-layers (the three motivating fixtures all use the conventional `<layer>/recipes-*/<dir>/*.bb` hierarchy). The Q2 clarification chose this explicitly.

**Implementation note**: After all recipes are parsed, do ONE pass: for each recipe, walk up from its `.bb` path. Cache the result per ancestor-directory so we don't re-scan the same directories for sibling recipes. For meta-balena's 163 recipes spread across ~14 layers, this is ~163 walks of average depth 3-4; trivial.

## R7 — Cross-source dedup on mixed scans (clarification Q3)

**Decision**: Per-source emission + milestone-105 PURL-based dedup. No explicit cross-reader correlation pass. Recipe-reader fields propagate onto the post-dedup winning component as `mikebom:also-detected-via` evidence (the existing milestone-105 mechanism).

**Rationale**: This is the existing mikebom architecture; introducing a new cross-correlation layer for Yocto would couple the readers and create new test-fixture combinatorics. The milestone-105 dedup is already designed for the maven `co_owned_by` + cargo / npm cross-tier patterns; Yocto fits the same shape.

**Implementation note**: VERIFY (no code change expected). The implementation step is an integration test: scan a mixed fixture (meta-layer tree + a synthesized opkg DB containing the same recipe by name+version), then assert the post-dedup component carries BOTH the recipe-reader's license AND the opkg-DB's installed-file occurrences. If the assertion fails, the bug is in `resolve/deduplicator.rs`'s field-propagation logic for the `also-detected-via` path; we fix it there, not in the Yocto reader.

## R8 — Perf budget on motivating fixtures (SC-008)

**Decision**: Target <500ms incremental cost on meta-balena (the largest fixture: 163 .bb + 337 .bbappend + 64 .inc + 14 conf/layer.conf). Verified via a new milestone-094-style perf benchmark target `yocto_recipe_enrich`.

**Rationale**: Each recipe is small (typical .bb is <10 KB; the largest is <100 KB). Line-oriented regex parsing is ~10-50 MB/s on modern CPUs; for ~563 files × ~5 KB avg = ~2.8 MB total, parsing takes ~50-300ms. Layer-attribution walks bounded by walker depth 8 + caching at ancestor level → linear in component count, ~O(1) amortized. CPE-name lookup is O(1) per recipe (HashSet over the embedded table). Total budget: comfortably under the <2× milestone-107 multiplier and well under 30s wall-clock.

**Alternatives considered**:

- Multi-threaded recipe-body parsing via rayon. Rejected for v1: adds parallelism complexity for sub-second wins on the motivating fixtures. Single-threaded gives deterministic ordering for tests; rayon can be retrofitted if a future fixture pushes the budget.

## R9 — Existing milestone-097 cpe-candidates channel (FR-019)

**Decision**: Reuse the existing `mikebom:cpe-candidates` annotation key. No new key. The Yocto reader populates the candidates array with `(recipe-name, version)` permutations: the literal recipe name, the CPE-normalized recipe name (from FR-017), and any vendor permutations the recipe declares (typically via a hand-curated table since recipes don't natively declare vendor).

**Rationale**: Milestone 097 already solves "multiple CPE candidates per component" — Yocto recipes are just another emission source.

**Implementation note**: The recipe-body parser doesn't need to know about CPE candidates. After parsing produces a `RecipeMetadata`, a separate `cpe_candidates::synthesize_for_recipe(meta) -> Vec<String>` helper builds the candidates list. The list is then folded into the component's existing `extra_annotations["mikebom:cpe-candidates"]` JSON array.

## R10 — Goldens scope (SC-006)

**Decision**: NO new byte-identity goldens for Yocto. The three balena clones become integration-test inputs (assertion on shape, not bytes), not pinned goldens. The 33 alpha.48 goldens stay byte-identical (they cover non-Yocto ecosystems). Milestone-107's existing image-tier fixtures (opkg-installed + image-manifest goldens) stay byte-identical because this feature only changes source-tier `.bb` reader output.

**Rationale**: Yocto recipe content is inherently version-volatile (every meta-balena release would churn the golden). Integration tests asserting on count + key presence + license-attribution rate + edge-count are more durable.

**Alternatives considered**:

- Pin a synthetic-fixture golden. Considered for the `single_layer_meta/` fixture; deferred to a follow-up if integration tests prove insufficient. The Q3 clarification (per-source emission) makes a small synthetic golden tractable, but adds maintenance cost for marginal gain.

## Open questions

None. All `/speckit-clarify` answers + reference-SBOM-evidence observations resolved.
