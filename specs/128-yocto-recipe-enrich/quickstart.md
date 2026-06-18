# Quickstart — Deeper Yocto / OpenEmbedded SBOM coverage

End-to-end repro recipes for each success criterion. Run against the three motivating fixtures (`meta-balena`, `balena-raspberrypi`, `balena-generic`) which are cached in the `MIKEBOM_FIXTURES_DIR` per the milestone-090 sibling-repo convention.

## SC-001 — License coverage ≥80% on meta-balena

```sh
cd /tmp && rm -rf meta-balena
git clone --depth 1 https://github.com/balena-os/meta-balena.git
cd meta-balena
mikebom sbom scan --path . --format spdx-2.3-json --output /tmp/meta-balena.spdx.json
```

```sh
total_recipes=$(jq '[.packages[] | select(.SPDXID | test("SPDXRef-Package-"))] | length' /tmp/meta-balena.spdx.json)
licensed=$(jq '[.packages[] | select(.licenseDeclared != "NOASSERTION" and .licenseDeclared != null)] | length' /tmp/meta-balena.spdx.json)
echo "license coverage: $licensed / $total_recipes"
```

Expected: license coverage ≥80% of recipe-derived components (excluding CLOSED-license recipes).

## SC-002 — vcs external ref ≥60%, srcrev annotation ≥40%

```sh
total=$(jq '[.packages[] | select(.SPDXID | test("SPDXRef-Package-"))] | length' /tmp/meta-balena.spdx.json)
with_vcs=$(jq '[.packages[] | select(.externalRefs[]?.referenceType == "vcs")] | length' /tmp/meta-balena.spdx.json)
with_srcrev=$(jq '[.packages[] | select(.annotations[]?.comment // "" | test("mikebom:srcrev"))] | length' /tmp/meta-balena.spdx.json)
echo "vcs: $with_vcs / $total — srcrev: $with_srcrev / $total"
```

Expected: vcs ≥60%, srcrev ≥40%.

## SC-003 — Layer attribution on every recipe

```sh
without_layer=$(jq '[.packages[] | select((.SPDXID | test("SPDXRef-Package-")) and (.annotations[]?.comment // "" | test("mikebom:yocto-layer") | not))] | length' /tmp/meta-balena.spdx.json)
echo "recipes without mikebom:yocto-layer: $without_layer (must be 0)"
```

Expected: 0.

## SC-004 — BOM subject identifies a layer-collection

```sh
jq -r '.documentDescribes[0] as $r | .packages[] | select(.SPDXID == $r) | "name=\(.name) ver=\(.versionInfo) purl=\(.externalRefs[]? | select(.referenceType == "purl") | .referenceLocator)"' /tmp/meta-balena.spdx.json
```

Expected: name is a layer-collection (e.g., `meta-balena-rust`, `meta-balena-bsp`), PURL `pkg:generic/<collection>@<LAYERVERSION>?openembedded=true&layer=<collection>` per FR-007 + milestone-127 root selection.

NOT expected: `name=meta-balena ver=0.0.0 purl=pkg:generic/meta-balena@0.0.0` (the pre-128 baseline).

## SC-005 — ≥50 DEPENDS_ON edges

```sh
jq '[.relationships[] | select(.relationshipType == "DEPENDS_ON" or .relationshipType == "BUILD_DEPENDENCY_OF")] | length' /tmp/meta-balena.spdx.json
```

Expected: ≥50.

## SC-006 — Zero regression on 33 alpha.48 goldens + image-tier fixtures

```sh
cd /Users/mlieberman/Projects/mikebom
cargo +stable test --workspace --test cdx_regression --test spdx_regression --test spdx3_regression --test yocto_scan
```

Expected: every test target passes with `0 failed`. Plus `./scripts/regen-goldens.sh` followed by `git status --short | grep fixtures/golden/` returns zero lines.

## SC-007 — sbomqs score improvement

```sh
sbomqs score --json /tmp/meta-balena.spdx.json | jq '.scores.summary.avg_score'
```

Expected: ≥55 (up from ~25 pre-128 baseline). The 30-point jump reflects license coverage + vcs externalRefs + DEPENDS_ON edges.

## SC-008 — Performance on motivating fixtures

```sh
time mikebom sbom scan --path /tmp/meta-balena --format spdx-2.3-json --output /tmp/meta-balena.spdx.json --offline
```

Expected: real <30s on each balena clone, and <2× the milestone-107-only baseline (measured by `cargo bench -p mikebom yocto_recipe_enrich` against the milestone-107 baseline target).

## SC-009 — CPE-name normalization fires

```sh
jq -r '[.packages[] | select(.annotations[]?.comment // "" | test("mikebom:cpe-candidates")) | .annotations[].comment | fromjson? | .value[]?] | unique | .[] | select(test("linux_kernel|network_security_services|dropbear_ssh|netscape_portable_runtime"))' /tmp/meta-balena.spdx.json | wc -l
```

Expected: ≥10 normalized-name hits (verifies FR-017's mapping table fires on real meta-balena recipes).

## SC-010 — No "version: git" or "AUTOINC+<sha>" anti-patterns

```sh
bad=$(jq '[.packages[] | select(.versionInfo == "git" or (.versionInfo // "" | test("AUTOINC")))] | length' /tmp/meta-balena.spdx.json)
echo "anti-pattern versions: $bad (must be 0)"
```

Expected: 0. Every git-fetched recipe with unstable PV uses SRCREV-derived version (FR-018).

## SC-011 — No CPE-vendor-fan-out (single component per recipe)

```sh
duplicates=$(jq -r '[.packages[] | select(.SPDXID | test("SPDXRef-Package-"))] | group_by(.name + "@" + .versionInfo) | map(select(length > 1)) | flatten | .[] | "\(.name)@\(.versionInfo)"' /tmp/meta-balena.spdx.json | sort -u)
echo "$duplicates" | wc -l
```

Expected: count is 0 OR the only duplicates are legitimately-different recipes (e.g., a base + a -native flavor, which are distinct PURLs). Specifically: `curl@8.7.1` appears EXACTLY ONCE in the output, NOT 6 times as in the reference Yocto-tooling SBOM (FR-019).

## End-to-end smoke

```sh
cd /Users/mlieberman/Projects/mikebom
cargo +stable test --workspace --test yocto_recipe_enrich_balena_smoke -- --nocapture
```

This runs the three balena clones via `MIKEBOM_FIXTURES_DIR` and asserts each SC.

## Pre-PR gate

```sh
./scripts/pre-pr.sh
```

Must exit 0. Clippy clean, all tests pass.
