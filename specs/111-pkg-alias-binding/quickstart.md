# Quickstart: Binding a primary binary to its source-tier PURL

**Feature**: 111-pkg-alias-binding (Option A of issue #225)
**Audience**: operators following the textbook source-to-image workflow whose flagship binary's binding currently lands in `Unknown`.

## The problem (before this feature)

You have a Rust app `baz` at `github.com/foo/bar`. You produce two SBOMs:

```bash
# 1. Source-tier SBOM (run against your checkout)
mikebom sbom scan --path . --output baz-source.cdx.json
# emits a component for the main module:
#   "purl": "pkg:cargo/baz@1.0.0"

# 2. Image-tier SBOM (run against the built container)
mikebom sbom scan --image myregistry/myimage:tag \
    --bind-to-source baz-source.cdx.json \
    --output baz-image.cdx.json
```

You inspect the image SBOM and find your flagship `baz` component:

```json
{
  "purl": "pkg:generic/baz",
  "properties": [{
    "name": "mikebom:binding-result-v1",
    "value": "{\"strength\":\"unknown\",\"reason\":\"source-not-found-in-bind-target\",...}"
  }]
}
```

`pkg:generic/baz` (image-tier) ≠ `pkg:cargo/baz@1.0.0` (source-tier), so the binder couldn't find a match. The single most important component in the scan has `unknown` binding.

## The fix (after this feature)

Add one `--pkg-alias` flag to your image-tier scan:

```bash
mikebom sbom scan --image myregistry/myimage:tag \
    --bind-to-source baz-source.cdx.json \
    --pkg-alias "pkg:generic/baz=pkg:cargo/baz@1.0.0" \
    --output baz-image.cdx.json
```

The same component now shows:

```json
{
  "purl": "pkg:generic/baz",
  "properties": [{
    "name": "mikebom:binding-result-v1",
    "value": "{\"strength\":\"verified\",\"hash\":\"...\",\"source_doc_id\":\"...\",\"alias_from\":\"pkg:generic/baz\",\"alias_to\":\"pkg:cargo/baz@1.0.0\",...}"
  }]
}
```

Verified. Auditors can later trace the alias:

```bash
mikebom verify-binding baz-image.cdx.json baz-source.cdx.json
# prints:
# {
#   "purl": "pkg:generic/baz",
#   "binding": { "strength": "verified", ... },
#   "applied_alias": "pkg:generic/baz → pkg:cargo/baz@1.0.0"
# }
```

No `--pkg-alias` flag needed at verify time — the alias is recorded in the SBOM.

## Multiple binaries (workspace project)

A Cargo workspace with two binaries:

```bash
mikebom sbom scan --image myregistry/myimage:tag \
    --bind-to-source workspace-source.cdx.json \
    --pkg-alias "pkg:generic/baz=pkg:cargo/baz@1.0.0" \
    --pkg-alias "pkg:generic/baz-debug=pkg:cargo/baz-debug@1.0.0" \
    --output workspace-image.cdx.json
```

Or via env var (CI-friendly):

```bash
export MIKEBOM_PKG_ALIAS="pkg:generic/baz=pkg:cargo/baz@1.0.0,pkg:generic/baz-debug=pkg:cargo/baz-debug@1.0.0"
mikebom sbom scan --image myregistry/myimage:tag \
    --bind-to-source workspace-source.cdx.json \
    --output workspace-image.cdx.json
```

Both forms produce identical SBOMs.

## What can go wrong

| Symptom | Cause | Fix |
|---|---|---|
| `error: --pkg-alias value 'pkg:generic/baz' is missing the '=' separator` | Forgot the `=` | Reformat as `LHS=RHS` |
| `error: --pkg-alias LHS PURL '...' failed to parse` | Malformed PURL | Validate via `purl-spec.org` or copy from the source SBOM |
| `error: --pkg-alias LHS 'X' declared twice with conflicting RHS values` | Same LHS appears with two RHSes (e.g., env var + CLI flag disagree) | Pick one RHS and remove the other |
| `WARN: --pkg-alias declared but --bind-to-source was not supplied` | Forgot `--bind-to-source` | Add the flag |
| `INFO: --pkg-alias LHS '...' did not match any scan-output component` | Typo'd LHS, or the scan didn't emit the expected component | Run without alias first, find the actual LHS PURL, then fix the alias |
| `binding.strength = unknown, reason = "alias-target-not-found-in-bind-target"` | The RHS PURL isn't in `--bind-to-source` | Check the source SBOM has the expected `pkg:cargo/...` (or `pkg:npm/...`, etc.) component |

## What this feature does NOT do

- Doesn't auto-detect the alias for you — Option B of issue #225 is a follow-on milestone.
- Doesn't do wildcard / pattern matching — strict PURL canonical-form equality only.
- Doesn't rewrite the component's emitted PURL — the LHS stays as the component's `purl` field; the RHS is referenced only via the envelope's `alias_to`.
- Doesn't apply in reverse — operator-declared alias is uni-directional LHS → RHS.

## Round-tripping through CI

Typical pipeline:

```yaml
# CI step 1: source SBOM
- run: mikebom sbom scan --path . --output baz-source.cdx.json

# CI step 2: build image, no SBOM yet
- run: docker build -t $IMAGE_TAG .

# CI step 3: image SBOM with binding
- run: |
    mikebom sbom scan --image $IMAGE_TAG \
        --bind-to-source baz-source.cdx.json \
        --pkg-alias "pkg:generic/baz=$SOURCE_PURL" \
        --output baz-image.cdx.json

# CI step 4: verify (no alias re-supply needed)
- run: mikebom verify-binding baz-image.cdx.json baz-source.cdx.json
```

The `$SOURCE_PURL` can be extracted from `baz-source.cdx.json` with `jq`:

```bash
SOURCE_PURL=$(jq -r '.metadata.component.purl' baz-source.cdx.json)
```

(For workspace projects with multiple binaries, this gets one alias declaration per binary — see the `MIKEBOM_PKG_ALIAS` env-var form above for the cleanest shape.)
