# Quickstart — Scanning Yocto/OE projects with mikebom

Once milestone 107 ships (v0.1.0-alpha.43 or later), `mikebom sbom scan` handles four Yocto/OE scan shapes. This guide is for operators / SBOM consumers.

---

## Scenario 1 — Scan a Yocto-built device rootfs

You have a flashed device image. Mount it or extract the rootfs to a directory, then scan.

```bash
# Mount the image (example: a Yocto core-image-minimal qemu image)
$ mkdir -p /tmp/rootfs
$ sudo mount -o loop core-image-minimal-qemux86-64.ext4 /tmp/rootfs

# Scan
$ mikebom sbom scan --path /tmp/rootfs --format cyclonedx-json --output rootfs-sbom.cdx.json

# Expected: ~250 components, all `pkg:opkg/...` PURLs, drawn from
# /var/lib/opkg/status. Each component has its architecture qualifier
# and `mikebom:source-files` annotation pointing at the opkg DB.
```

Verify:

```bash
$ jq '[.components[] | select(.purl | startswith("pkg:opkg/"))] | length' rootfs-sbom.cdx.json
247
$ jq '.components[] | select(.name == "libc") | .purl' rootfs-sbom.cdx.json
"pkg:opkg/libc@2.38-r0?arch=core2_64"
```

---

## Scenario 2 — Scan a Yocto build directory's image manifest

You're in a CI pipeline that just ran `bitbake core-image-st-image-weston`. Generate an SBOM from the resulting build directory.

```bash
$ cd build/
$ mikebom sbom scan --path tmp/deploy/ --format cyclonedx-json --output image-sbom.cdx.json

# Expected: ~150-500 components depending on image type, all from the
# <image>.manifest at build/tmp/deploy/images/<machine>/<image>.manifest.
```

The `<image>.manifest` file is the **authoritative** record of what BitBake recorded as the image's contents — preferred over scanning the rootfs because it's deterministic and doesn't require flashing/mounting.

---

## Scenario 3 — Scan a Yocto SDK sysroot (OpenSTLinux dev machine)

You're an embedded-Linux developer with the vendor SDK installed. Scan the sysroot to get a build-time-tier SBOM (every header / library your app links against).

```bash
$ source /opt/st/openstlinux-6.6/environment-setup-cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi
$ mikebom sbom scan \
    --path /opt/st/openstlinux-6.6/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi \
    --format cyclonedx-json \
    --output sdk-sysroot-sbom.cdx.json

# Expected: ~400 components, all with `scope: "excluded"` in the
# emitted CDX (build-time only; not shipping to the device).
```

Verify the sysroot detection fired:

```bash
$ jq '.components[] | select(.purl | startswith("pkg:opkg/")) | .scope' sdk-sysroot-sbom.cdx.json | sort -u
"excluded"
```

Every emitted opkg component carries `scope: "excluded"` because the heuristic detected the `environment-setup-*` script in the SDK's parent dir. If the heuristic was ambiguous, the SBOM's `metadata.properties[]` will contain a `mikebom:scan-ambiguity` annotation explaining why.

---

## Scenario 4 — Scan a Yocto layer tree

You're auditing a vendor's published `meta-vendor/` layer before adopting it. Enumerate the recipes the layer declares.

```bash
$ git clone https://github.com/example/meta-vendor.git
$ mikebom sbom scan --path meta-vendor/ --format cyclonedx-json --output layer-audit.cdx.json

# Expected: one `pkg:bitbake/<name>@<version>?layer=meta-vendor`
# component per `.bb` file in the layer.
```

Each recipe component is filename-derived — mikebom does NOT execute BitBake or evaluate recipe-variable expansions. Recipes whose filenames contain unexpanded `${...}` (e.g. `${PN}_${PV}.bb`) are silently skipped with a `tracing::warn!` log entry. This is by design (resolving them would require a full BitBake parser, which is out of scope for this milestone).

---

## What's NOT supported (this milestone)

- **Recipe-body dep edges**: `pkg:bitbake/...` components emit identity only, no `DEPENDS`/`RDEPENDS_${PN}` extraction.
- **BitBake variable expansion**: `${PN}_${PV}.bb` recipes are skipped, not resolved.
- **Yocto-specific license-name translation**: license fields flow through verbatim to mikebom's existing SPDX-expression pipeline.
- **bitbake subprocess invocation**: no `bitbake -e` introspection. All parsing is filesystem-only.

These deferrals are explicit in the milestone-107 spec's "Out of Scope" section and may be addressed in follow-on milestones.

---

## Troubleshooting

### "My device rootfs scan emits zero `pkg:opkg/...` components"

Check whether `/var/lib/opkg/status` exists in your rootfs. Some production images strip the opkg metadata to save space. mikebom will still emit binary-walker components (`pkg:generic/<basename>` for unclaimed binaries) but won't have package-tier identity. Symptom:

```bash
$ ls /tmp/rootfs/var/lib/opkg/
# (empty or missing)
```

Mitigation: build the image with `INHIBIT_PACKAGE_STRIP = "1"` or include opkg metadata in the image manifest.

### "Sysroot scan didn't tag components as build-scope"

Verify the sysroot's parent dir has an `environment-setup-*` script:

```bash
$ ls -1 /opt/st/openstlinux-6.6/environment-setup-*
environment-setup-cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi
```

If the env-script is missing but the sysroot still has `/usr/include/` + no `/etc/init.d/`, the secondary signal should still fire. If neither signal fires (e.g. the sysroot was reshuffled into a non-standard layout), components emit without the build-scope tag. Workaround: re-install the SDK to its canonical layout, or file an issue with your specific BSP shape and we'll extend FR-005a's heuristic.

### "I see duplicate components — same coord in `pkg:opkg/` and `pkg:bitbake/`"

This is expected when you scan a directory that contains both a built image AND its source layer (e.g. a CI container with both `build/` and the layers mounted). The opkg-installed-DB tier and the bitbake-recipe tier emit different PURLs (different ecosystem name), so they're distinct components — they're not duplicates. The dedup pipeline collapses **same-coord cross-source** emissions (opkg-installed + image-manifest both seeing `openssl@3.0.5`) but keeps the recipe-tier emission as a separate component because the PURL differs.

If you want to filter to a single tier, query the emitted SBOM:

```bash
# Just installed components
$ jq '.components[] | select(.purl | startswith("pkg:opkg/"))' sbom.cdx.json

# Just declared-recipe components
$ jq '.components[] | select(.purl | startswith("pkg:bitbake/"))' sbom.cdx.json
```

---

## Further reading

- Spec: `specs/107-yocto-recipe-reader/spec.md`
- Plan: `specs/107-yocto-recipe-reader/plan.md`
- Data model: `specs/107-yocto-recipe-reader/data-model.md`
- Per-reader contracts: `specs/107-yocto-recipe-reader/contracts/`
- Project ecosystem coverage matrix: `docs/ecosystems.md`
