# Quickstart — milestone 144 RPM reader fixes

Operator-facing walkthrough of the scenarios this milestone surfaces.

## Scenario 1 — Yocto build output (the original D-01 + D-02 reproducer)

The motivating use case. Before this milestone, scanning the Yocto `tmp/deploy/rpm/` tree silently dropped 3 debug RPMs and emitted `pkg:rpm/rpm/...` PURLs.

```bash
# Yocto build output for core-image-minimal on qemux86-64
cd build-qemux86-64/
mikebom sbom scan --path tmp/deploy/rpm/ --format cyclonedx-json --output /tmp/yocto.cdx.json
```

**Verify (US1 + US2)**:

```bash
# Zero double-rpm PURLs (D-01 fix):
jq -r '.components[] | select(.purl | startswith("pkg:rpm/rpm/")) | .purl' /tmp/yocto.cdx.json | wc -l
# 0

# 4587 RPM components, not 4584 (D-02 fix — three debug RPMs restored):
jq -r '[.components[] | select(.purl | startswith("pkg:rpm/"))] | length' /tmp/yocto.cdx.json
# 4587

# kernel-dbg, gcc-dbg, openssl-ptest are present:
jq -r '.components[] | select(.name | test("kernel-dbg|gcc-dbg|openssl-ptest")) | .name' /tmp/yocto.cdx.json | sort -u
# gcc-dbg
# kernel-dbg
# openssl-ptest
```

## Scenario 2 — Yocto with explicit DISTRO identifier

Yocto operators who want their image's `DISTRO` variable (`poky`, or a vendor-specific override) to appear in PURLs use `--rpm-distro`.

```bash
mikebom sbom scan --path tmp/deploy/rpm/ --rpm-distro poky --format cyclonedx-json --output /tmp/yocto-poky.cdx.json
```

**Verify (US4)**:

```bash
# All RPM PURLs have the poky namespace:
jq -r '.components[] | select(.purl | startswith("pkg:rpm/")) | .purl' /tmp/yocto-poky.cdx.json | head -3
# pkg:rpm/poky/acl-dbg@2.3.2-r0?arch=core2_64
# pkg:rpm/poky/alsa-lib@1.2.10-r0?arch=core2_64
# pkg:rpm/poky/avahi-libs@0.8-r0?arch=core2_64

# Count distinct namespaces (should be exactly 1):
jq -r '.components[] | select(.purl | startswith("pkg:rpm/")) | .purl | capture("pkg:rpm/(?<ns>[^/]+)/").ns' /tmp/yocto-poky.cdx.json | sort -u | wc -l
# 1
```

## Scenario 3 — Installed system scan (Fedora / RHEL / Rocky rootfs)

Scanning an installed-system rootfs picks up `/etc/os-release` automatically; no flags needed.

```bash
# Extract a container image with skopeo+umoci, then scan its rootfs
mikebom sbom scan --path /tmp/fedora-39-rootfs/ --format cyclonedx-json --output /tmp/fedora.cdx.json
```

**Verify (US1 acceptance scenario 2)**:

```bash
# All standalone-.rpm components use the auto-detected fedora namespace:
jq -r '.components[] | select(.purl | startswith("pkg:rpm/")) | .purl' /tmp/fedora.cdx.json | head -3
# pkg:rpm/fedora/curl@8.4.0-1.fc39?arch=x86_64
# pkg:rpm/fedora/glibc@2.38-10.fc39?arch=x86_64
# pkg:rpm/fedora/openssl@3.1.4-1.fc39?arch=x86_64
```

## Scenario 4 — Larger-than-default RPM (enterprise/appliance images)

When even the new 512 MB cap is too small (e.g., a 700 MB kernel-modules-extra appliance RPM), the operator lifts the cap.

```bash
mikebom sbom scan --path /tmp/appliance-rpms/ --max-rpm-bytes 1073741824 --format cyclonedx-json --output /tmp/appliance.cdx.json
```

**Verify (US3 acceptance scenario 1)**:

```bash
# 700 MB RPM is included; no size-cap WARN:
mikebom sbom scan --path /tmp/appliance-rpms/ --max-rpm-bytes 1073741824 --format cyclonedx-json --output /tmp/appliance.cdx.json 2>&1 | grep -c size-cap-exceeded
# 0

jq '[.components[] | select(.purl | startswith("pkg:rpm/"))] | length' /tmp/appliance.cdx.json
# (count includes the 700 MB RPM)
```

## Scenario 5 — Default 512 MB cap exceeded (transparency)

Operator runs default-cap scan; an oversized RPM gets a clear non-misleading WARN.

```bash
mikebom sbom scan --path /tmp/dir-with-600mb-rpm/ --format cyclonedx-json --output /tmp/out.cdx.json 2>/tmp/scan.log
```

**Verify (US2 acceptance scenario 2 + FR-006)**:

```bash
# WARN does NOT contain "malformed":
grep size-cap-exceeded /tmp/scan.log
# WARN ... skipping oversized .rpm file path=... size=629145600 cap=536870912 reason="size-cap-exceeded"

! grep -q "malformed.*size-cap-exceeded" /tmp/scan.log && echo "FR-006 holds"
# FR-006 holds
```

## Scenario 6 — Empty `--rpm-distro` rejected at parse time

```bash
mikebom sbom scan --path /tmp/dir/ --rpm-distro "" --format cyclonedx-json --output /tmp/out.cdx.json
# error: invalid value '' for '--rpm-distro <ID>': must be non-empty
echo $?
# 2 (clap exit code for invalid args)
```

## Scenario 7 — Zero `--max-rpm-bytes` rejected at parse time

```bash
mikebom sbom scan --path /tmp/dir/ --max-rpm-bytes 0 --format cyclonedx-json --output /tmp/out.cdx.json
# error: invalid value '0' for '--max-rpm-bytes <BYTES>': must be > 0
echo $?
# 2
```

## Scenario 8 — Both flags combined

```bash
mikebom sbom scan --path tmp/deploy/rpm/ \
    --rpm-distro poky \
    --max-rpm-bytes 1073741824 \
    --format cyclonedx-json --output /tmp/yocto-full.cdx.json
```

Both flags apply independently — `--rpm-distro` sets the namespace; `--max-rpm-bytes` sets the cap. The 700 MB hypothetical RPM (if present) would be both scanned (cap raised) and emitted with `pkg:rpm/poky/...` namespace.

## Verification commands

```bash
cargo test -p mikebom rpm_file::tests::                               # unit tests in rpm_file.rs
cargo test --test rpm_file_yocto_regression                           # new integration test
cargo +stable clippy --workspace --all-targets -- -D warnings         # pre-PR gate step 1
cargo +stable test --workspace                                        # pre-PR gate step 2
./scripts/pre-pr.sh                                                   # both, in order — the SC-008 gate
```

## Cross-format byte-equivalence check (FR-010)

The change is wire-level — same corrected PURL string appears in CDX 1.6, SPDX 2.3, and SPDX 3 outputs:

```bash
mikebom sbom scan --path tmp/deploy/rpm/ \
    --format cyclonedx-json --format spdx-2.3-json --format spdx-3-json \
    --output cyclonedx-json=/tmp/r.cdx.json \
    --output spdx-2.3-json=/tmp/r.spdx.json \
    --output spdx-3-json=/tmp/r.spdx3.json

jq -r '[.components[] | select(.purl | startswith("pkg:rpm/")) | .purl] | sort' /tmp/r.cdx.json > /tmp/cdx-rpm.txt
jq -r '[.packages[].externalRefs[]? | select(.referenceType == "purl") | .referenceLocator | select(startswith("pkg:rpm/"))] | sort' /tmp/r.spdx.json > /tmp/spdx-rpm.txt
jq -r '[.["@graph"][] | select(.software_packageUrl? | tostring | startswith("pkg:rpm/")) | .software_packageUrl] | sort' /tmp/r.spdx3.json > /tmp/spdx3-rpm.txt

diff /tmp/cdx-rpm.txt /tmp/spdx-rpm.txt
diff /tmp/cdx-rpm.txt /tmp/spdx3-rpm.txt
# (no output = byte-identical PURL sets across all three formats)
```

## Known deferrals (spec Out of Scope)

- Per-subtree distro selection (one operator-set namespace per scan invocation).
- Operator override of `MAX_RPMDB_BYTES` (rpmdb cap raised symmetrically to 512 MB but not overridable).
- Per-RPM size-cap exemption lists.
- Changes to the rpmdb-path (`rpm.rs`) PURL construction.
- `mikebom:*` CDX/SPDX properties for size-cap or distro-detection metadata.
- Live `rpm` subprocess invocation.
- Changing the WARN-text for genuinely malformed RPMs (those keep "malformed" wording per FR-007).
