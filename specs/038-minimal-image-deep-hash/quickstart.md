# Quickstart: Verify Milestone 038

**Feature**: 038-minimal-image-deep-hash

After implementation, the milestone's user-visible value is best
verified by scanning a real distroless image and inspecting the
output. This document is the post-merge verification recipe a
future maintainer can run to confirm coverage hasn't regressed.

## Prerequisites

- mikebom built with the default feature set (`oci-registry` is
  on by default since milestone 033, so registry pulls work
  out-of-the-box).
- Network access to `gcr.io` (no auth required for distroless).
- `jq` for output inspection.

## Verify US1 — distroless deb per-file evidence

```bash
mikebom sbom scan \
    --image gcr.io/distroless/static-debian12:latest \
    --output /tmp/distroless.cdx.json
```

**Expected outputs** (after milestone 038):

- 4 deb components in the SBOM.
- Each component carries a non-empty `evidence.occurrences[]`.
- Each occurrence has `location` (the file path) and `hashes`
  (with at least SHA-256).

```bash
# Component count: 4 expected.
jq '.components | length' /tmp/distroless.cdx.json

# Per-component file evidence count.
jq '.components[] | { name, occurrences: (.evidence.occurrences | length // 0) }' \
    /tmp/distroless.cdx.json
```

Example expected output (component names and counts):

| Component | Approx. file count |
|---|---|
| `base-files` | ~50 (file list dominated by `/etc/*` config files) |
| `media-types` | 1 (`/etc/mime.types`) |
| `netbase` | 4 (`/etc/protocols`, `/etc/services`, etc.) |
| `tzdata` | ~600 (timezone data) |

The exact counts vary by image vintage; the assertion is "non-zero
for every component", and SC-002 of the spec asserts parity with
`dpkg-query -L <pkg>` on full-fat debian:12 for these 4 packages.

**Regression check** (post-milestone, this should NEVER report
zero):

```bash
total=$(jq '[.components[].evidence.occurrences | length // 0] | add' /tmp/distroless.cdx.json)
echo "Total file occurrences: $total"
test "$total" -gt 0 && echo "✓ milestone 038 still healthy"
```

## Verify US1 — fast-hash path is empty (FR-003)

```bash
mikebom sbom scan \
    --image gcr.io/distroless/static-debian12:latest \
    --no-deep-hash \
    --output /tmp/distroless-fast.cdx.json

# Component count: still 4.
jq '.components | length' /tmp/distroless-fast.cdx.json

# Per-component file evidence: ZERO under fast-hash.
jq '[.components[].evidence.occurrences | length // 0] | add' \
    /tmp/distroless-fast.cdx.json
# Expected: 0 (fast-hash is component-level only).
```

## Verify US1 — full-fat byte-identity (SC-003)

The 27-fixture byte-identity goldens cover the legacy single-file
layout. The post-milestone-038 regen produces zero diff:

```bash
MIKEBOM_UPDATE_CDX_GOLDENS=1 \
MIKEBOM_UPDATE_SPDX_GOLDENS=1 \
MIKEBOM_UPDATE_SPDX3_GOLDENS=1 \
    cargo +stable test -p mikebom --test '*' >/dev/null 2>&1

git status --short -- tests/fixtures/27/
# Expected: empty (zero golden drift).
```

## Verify US2 — chainguard apko coverage

```bash
mikebom sbom scan \
    --image cgr.dev/chainguard/static:latest \
    --output /tmp/chainguard.cdx.json

# Components present.
jq '.components | length' /tmp/chainguard.cdx.json
# Expected: > 0 (apko ships some packages).

# Per-component file evidence (default deep-hash mode).
jq '[.components[].evidence.occurrences | length // 0] | add' \
    /tmp/chainguard.cdx.json
# Expected: > 0.
```

## Verify with the gated smoke test

```bash
MIKEBOM_OCI_NETWORK_TESTS=1 cargo +stable test \
    -p mikebom --test oci_registry_smoke
```

The renamed
`pulls_distroless_static_and_emits_dpkg_status_d_components` test
already exists (added in milestone 037 commit 1) and asserts the
4 expected packages are present. Milestone 038 extends it (or
adds a sibling) to also assert non-empty per-file evidence.

## Troubleshooting

- **`jq '.components[].evidence.occurrences | length' returns
  null`**: the `.evidence` property is missing on every
  component. This indicates the per-file evidence path didn't
  fire. Check `RUST_LOG=debug mikebom sbom scan ...` for any
  `OCI cache insert failed` or `could not hash file` warnings.
- **`Total file occurrences: 0` post-merge**: regression. The
  most likely cause is a path drift — e.g.
  `var/lib/dpkg/status.d/<pkg>.md5sums` is being looked up at
  the wrong relative path. Re-extract the image manually:
  `tar -xf <docker-save-tarball> -C /tmp/extract && ls
  /tmp/extract/var/lib/dpkg/status.d/` and confirm the .md5sums
  files exist with the expected names.
