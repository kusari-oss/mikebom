# Quickstart: Verify Milestone 040

**Feature**: 040-pkg-db-followups

Post-merge verification recipe. Three independent checks, one
per user story.

## US1 — Stale comment cleanup

```bash
grep -rn 'deferred to milestone 031' mikebom-cli/src/
# expected: zero matches (today: 1 in oci_pull/mod.rs:215)

grep -rn 'deferred to milestone' mikebom-cli/src/
# expected: only matches that point at genuinely deferred items;
# eyeball the list and confirm nothing references 031.x/y/z (all
# shipped in 034/035/036).
```

## US2 — Apk SHA-1 cross-reference

```bash
mikebom sbom scan --image alpine:3.19 --output /tmp/alpine.cdx.json

# Each populated occurrence on apk components carries both
# sha256 and sha1 in additionalContext.
jq '.components[0].evidence.occurrences[0].additionalContext' /tmp/alpine.cdx.json
# expected: a JSON-string containing both "sha256" and "sha1" keys.

# Audit count: every populated apk occurrence should have sha1
# (extracting from additionalContext requires a second jq parse).
jq '
  [
    .components[].evidence.occurrences[]
    | (.additionalContext // "")
    | select(. != "")
    | fromjson
    | { sha256: (.sha256 // null), sha1: (.sha1 // null) }
  ]
  | { with_sha1: map(select(.sha1 != null)) | length,
      total:     length }
' /tmp/alpine.cdx.json
# expected: with_sha1 ≈ total (a few may legitimately lack apk-
# provided sha1 if the upstream Z: line is absent — rare).
```

## US3 — Rpm per-file deep-hashing

```bash
# fedora:40 is a small-ish rpm-based image.
mikebom sbom scan --image fedora:40 --output /tmp/fedora.cdx.json

# Total file occurrences across rpm components.
jq '[.components[].evidence.occurrences | length // 0] | add' /tmp/fedora.cdx.json
# expected: > 0 (was 0 pre-040)

# Per-component breakdown.
jq '[.components[] | { name, occs: (.evidence.occurrences | length // 0) }]' \
    /tmp/fedora.cdx.json | head -30
```

## Cross-cutting

```bash
# Goldens regen — must produce zero diff (the 27 fixtures all use
# --no-deep-hash so they're insulated from the deep-hash path).
MIKEBOM_UPDATE_CDX_GOLDENS=1 \
MIKEBOM_UPDATE_SPDX_GOLDENS=1 \
MIKEBOM_UPDATE_SPDX3_GOLDENS=1 \
    cargo +stable test -p mikebom --test '*' >/dev/null 2>&1

git status --short -- mikebom-cli/tests/fixtures/
# expected: zero entries.

# No regression on milestone-038/-039 fixtures.
mikebom sbom scan --image gcr.io/distroless/static-debian12:latest -o /tmp/distroless.cdx.json
jq '[.components[].evidence.occurrences | length // 0] | add' /tmp/distroless.cdx.json
# expected: still ≈ 938 (matches milestone 038 numbers).
```

## Troubleshooting

- **US1 grep returns matches in `specs/`**: expected — spec docs
  record history. Only `mikebom-cli/src/` is in scope.
- **US2 `with_sha1` < `total`**: some apk packages legitimately
  lack a Z: line for some files (rare; usually metadata-only or
  mountpoint-style entries). Cross-check against the apk
  installed-db to confirm these are the expected cases.
- **US3 reports 0 occurrences post-040**: most likely path-
  resolution issue. Manually extract the rpm rootfs:
  ```
  docker save fedora:40 -o /tmp/fedora.tar
  mkdir /tmp/fedora-rootfs && tar -xf /tmp/fedora.tar -C /tmp/fedora-rootfs/
  ls /tmp/fedora-rootfs/var/lib/rpm/
  ```
  Confirm the rpmdb (Packages or rpmdb.sqlite) is present.
