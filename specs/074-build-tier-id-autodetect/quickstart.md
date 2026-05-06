# Quickstart — milestone 074 build-tier identifier auto-detection

Five operator-facing recipes. Each runs end-to-end against a post-074 mikebom build with no special setup beyond a normal git checkout (Recipes 1, 3, 4, 5) or a tempdir (Recipe 2).

## Recipe 1 — Zero-config build-tier in a git checkout

The headline new behavior. No flags needed.

```bash
cd ~/projects/my-rust-app  # any project that's a git checkout
mikebom trace run -- ./build.sh
```

Inspect the emitted attestation's payload (build-tier SBOM):

```bash
jq '.payload | @base64d | fromjson \
    | .predicate.products[] \
    | .digest // empty' /path/to/build.attestation.json

# Or for the SBOM payload directly:
jq '.metadata.component.externalReferences[] | select(.type == "vcs")' \
    /path/to/build.cdx.json
```

Expected output — both `repo:` and `git:` entries:

```json
{
  "type": "vcs",
  "url": "git@github.com:acme/my-rust-app.git",
  "comment": "auto-detected from build-tier git remote `origin`"
}
{
  "type": "vcs",
  "url": "git@github.com:acme/my-rust-app.git#abc1234567890abcdef1234567890abcdef1234",
  "comment": "auto-detected from build-tier `git rev-parse HEAD`"
}
```

The first entry is the `repo:` identifier (repo identity). The second is the `git:` identifier — same URL, additionally carrying the build-of-record commit SHA.

## Recipe 2 — Build-tier in a non-git directory

When the build runs from a tempdir or any non-git location:

```bash
mkdir -p /tmp/scratch && cd /tmp/scratch
mikebom trace run -- ./build.sh
```

The emitted build-tier SBOM contains zero auto-detected identifiers. A single info-level log line records the skip:

```
INFO not a git checkout; build-tier identifier auto-detection skipped
```

The scan completes successfully. This is consistent with milestone 073's source-tier behavior in non-git fixtures.

## Recipe 3 — Override auto-detect with a manual flag

When the operator wants the build-tier SBOM to carry a different repo URL than `git remote get-url origin` would return — for example, a public mirror URL when the build runs against a private read-replica:

```bash
cd ~/projects/my-rust-app
mikebom trace run --repo git@github.com:acme/public-mirror.git -- ./build.sh
```

The manual `--repo` value wins. Emitted SBOM carries `repo:git@github.com:acme/public-mirror.git` — not the auto-detected origin URL. A log line records the override:

```
INFO manual --repo flag overrides build-tier auto-detected `repo:git@github.com:acme/my-rust-app.git`
```

The `git:` identifier is similarly overridable via `--git-ref`. Mixed cases are supported — supplying `--repo` alone keeps `git:` auto-detection enabled (since `git:` reuses the manual repo URL), while supplying `--git-ref` alone keeps `repo:` auto-detection enabled. Inheriting milestone 073's manual-wins precedence rule.

## Recipe 4 — Detached HEAD (typical CI state)

CI runners frequently check out commits by SHA rather than by branch name, leaving the working tree in detached-HEAD state. Build-tier auto-detection treats this as a normal scan, not an error condition:

```bash
git checkout abc1234  # detached HEAD on commit abc1234
mikebom trace run -- ./build.sh
```

The emitted `git:` identifier contains the detached SHA exactly as `git rev-parse HEAD` returns it:

```json
{
  "type": "vcs",
  "url": "git@github.com:acme/my-rust-app.git#abc1234567890abcdef1234567890abcdef1234",
  "comment": "auto-detected from build-tier `git rev-parse HEAD`"
}
```

This is the milestone's CI-friendly path — no special flag required, no special handling needed by the operator.

## Recipe 5 — Cross-tier correlation walkthrough

The end-to-end scenario this milestone enables: an operator with all three tier SBOMs can correlate them by reading identifier fields directly. No mikebom-side resolver, no index — just identifier byte-equality.

Three scans, all from the same checkout at the same commit:

```bash
cd ~/projects/my-rust-app

# Source-tier scan (milestone 073 — auto-detects `repo:`)
mikebom sbom scan --path . --format cyclonedx-json \
    --output cyclonedx-json=/tmp/source.cdx.json

# Build-tier scan (milestone 074 — auto-detects `repo:` + `git:`)
mikebom trace run -- ./build.sh
# (build-tier SBOM lands at /tmp/build.cdx.json or similar)

# Image-tier scan (milestone 073 — auto-detects `image:`)
mikebom sbom scan --image my-app:v1 --format cyclonedx-json \
    --output cyclonedx-json=/tmp/image.cdx.json
```

Now correlate by reading the emitted identifier fields:

```bash
# Source SBOM's `repo:` value
jq -r '.metadata.component.externalReferences[] \
       | select(.type == "vcs") | .url' /tmp/source.cdx.json
# git@github.com:acme/my-rust-app.git

# Build SBOM's `repo:` value (must match source byte-for-byte per SC-002)
jq -r '.metadata.component.externalReferences[] \
       | select(.type == "vcs") | .url' /tmp/build.cdx.json | head -1
# git@github.com:acme/my-rust-app.git

# Build SBOM's `git:` commit SHA (matches `git rev-parse HEAD` of source)
jq -r '.metadata.component.externalReferences[] \
       | select(.type == "vcs") | .url' /tmp/build.cdx.json | tail -1
# git@github.com:acme/my-rust-app.git#abc1234567890abcdef1234567890abcdef1234

git -C ~/projects/my-rust-app rev-parse HEAD
# abc1234567890abcdef1234567890abcdef1234   ← matches the SHA above

# Image SBOM has its own `image:` identifier (carries the image digest, not
# the source/build identifiers — those are recoverable via the source/build
# SBOMs the operator chooses to associate with this image).
jq -r '.metadata.component.externalReferences[] \
       | select(.type == "distribution") | .url' /tmp/image.cdx.json
# my-app:v1@sha256:...
```

The operator (or an external tool) now has a complete picture: the image was built from the build-tier SBOM whose `git:` identifier records commit `abc1234...`, and that commit lives in the same `repo:` whose source SBOM the operator has on file. Three SBOMs, three identifier slots, one consistent correlation.

This is the cross-tier story milestone 074 unlocks. Future milestones may automate the correlation (via a local index, OCI referrers, or external registries), but the foundation — every tier carrying its identifiers automatically — is in place starting with this release.

## Recipe (bonus) — Inspecting build-tier `source_label` for tier disambiguation

When reading a CDX `externalReferences` entry, the `comment` field carries the `source_label` that distinguishes how the identifier was discovered. After milestone 074, build-tier auto-detected entries are explicitly labeled "build-tier" so consumers can tell tiers apart at a glance:

```bash
# Source-tier comment (milestone 073)
"comment": "auto-detected from git remote `origin`"

# Build-tier comment (milestone 074, this milestone)
"comment": "auto-detected from build-tier git remote `origin`"

# Build-tier `git:` comment (milestone 074)
"comment": "auto-detected from build-tier `git rev-parse HEAD`"
```

The `mikebom:sbom-tier` annotation in the emitted document already disambiguates tiers at the document level; this label disambiguates per-identifier so a reader scanning a flat list of identifiers (e.g., from `jq` output) doesn't lose tier context.
