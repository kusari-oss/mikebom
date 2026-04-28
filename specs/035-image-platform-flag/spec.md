---
description: "--image-platform <linux/arch[/variant]> CLI flag for cross-arch OCI image scans"
status: spec
milestone: 035
closes: "#67"
---

# Spec: `--image-platform` flag (031.y)

## Background

Milestone 031 (#63) auto-resolves multi-arch image indexes to
`linux/<host-arch>` only. The host-arch detection lives in
`mikebom-cli/src/scan_fs/oci_pull/mod.rs::host_oci_arch` (line 204-220),
which maps Rust's `std::env::consts::ARCH` to OCI platform-arch names
(`x86_64` → `amd64`, `aarch64` → `arm64`, etc.). The platform-list
resolver in `mikebom-cli/src/scan_fs/oci_pull/platform.rs::resolve_manifest_list_to_linux`
already takes `target_arch: &str` as a parameter — only the call site
needs to learn how to override the host default.

Common cross-arch scan scenarios:
- macOS (arm64) developer scanning a `linux/amd64` image deployed to
  AWS / a typical Linux server fleet.
- Linux x86_64 CI scanning an `arm64` image deployed to AWS Graviton
  / Raspberry Pi.
- Multi-arch CI fan-out where each lane wants a specific variant.

Today these users see "no manifest in image index matches
linux/<host>; available: [...]. Cross-arch image pulls deferred to
milestone 031.y." This milestone closes that hint.

## User story (US1, P1)

**As an SBOM consumer scanning a container image whose deployment arch
differs from my dev machine's arch**, I want a `--image-platform <os/arch>`
CLI flag that overrides the host-arch default, so that I can scan
images for the platform they actually run on.

**Why P1 (not P2)**: this is correctness-flavored. macOS arm64 dev
machines that scan `linux/amd64` images today get a clear error rather
than wrong data, but the workflow is broken — they can't produce an
SBOM for the platform they care about without `docker pull --platform
linux/amd64 && docker save`. That's exactly the friction milestone 031
was supposed to remove.

### Independent test

After implementation:
- `mikebom sbom scan --image alpine:3.19 --image-platform linux/amd64
  --output amd64.cdx.json` succeeds on an arm64 host.
- The CDX has `pkg:apk/alpine/...?arch=x86_64` PURLs (apk arch field
  carries the linux/amd64 arch — apk's `x86_64`).
- Same scan with `--image-platform linux/arm64` produces `arch=aarch64`
  PURLs.
- `--image-platform linux/nonexistent` produces a clear error listing
  the available platforms in the index.
- `--image-platform linux/amd64` with `--image alpine.tar` (a tarball
  path, not a registry ref) produces "the --image-platform flag only
  applies to registry image references, not pre-extracted tarballs".

## Acceptance scenarios

**Scenario 1: Override default arch**
```
Given: a multi-arch index with linux/amd64 + linux/arm64 entries
       AND a host machine that's neither (or both)
When:  mikebom sbom scan --image <ref> --image-platform linux/amd64
Then:  the linux/amd64 manifest is fetched, the SBOM reflects amd64
       components, and the scan log records the chosen platform.
```

**Scenario 2: Variant disambiguation**
```
Given: a multi-arch index with linux/arm/v6, linux/arm/v7, linux/arm64
When:  mikebom sbom scan --image <ref> --image-platform linux/arm/v7
Then:  the linux/arm/v7 manifest is fetched (NOT v6 or v8 or arm64).
```

**Scenario 3: No matching platform → clear error**
```
Given: a multi-arch index with linux/amd64 + linux/arm64 only
When:  mikebom sbom scan --image <ref> --image-platform linux/s390x
Then:  scan fails with a non-zero exit and an error listing
       [linux/amd64, linux/arm64] as available platforms.
```

**Scenario 4: Single-platform manifest is a no-op**
```
Given: an image whose ref points at a single-platform manifest
       (no index)
When:  mikebom sbom scan --image <ref> --image-platform linux/amd64
Then:  scan proceeds normally; the flag is silently ignored because
       there's no index to pick from. (No error, no warning — single-
       platform images don't carry platform metadata to validate
       against.)
```

**Scenario 5: Tarball path + flag → clear error**
```
Given: --image points at a docker-save tarball on disk
When:  --image-platform is also passed
Then:  scan fails immediately with "--image-platform only applies to
       registry image references, not pre-extracted tarballs."
```

**Scenario 6: Unset flag → host default (regression guard)**
```
Given: no --image-platform flag passed
When:  mikebom sbom scan --image <multi-arch-ref>
Then:  behaviour is identical to milestone 031 — linux/<host-arch> is
       chosen automatically.
```

## Edge cases

- **OS != linux.** mikebom's scanner is Linux-rootfs-shaped — Windows
  containers would need a different extraction path. Reject
  `--image-platform windows/amd64` with "only `linux/<arch>` is
  supported; mikebom doesn't scan non-Linux container images."
- **Variant absence with arch=arm.** `linux/arm` (no variant) on an
  index that has `linux/arm/v7` and `linux/arm/v6` entries: the
  resolver picks the FIRST matching arm entry. This matches today's
  "first match wins" behaviour for amd64 (deterministic-by-iteration
  order; the index order is whatever the registry returns). Document
  the convention; users who care should specify a variant.
- **Variant present in user input but not in index entry.** e.g. user
  asks for `linux/arm64/v8` and the index entry has variant=None.
  Resolver requires exact-or-superset match: user's variant=Some must
  match entry's variant=Some — entry's variant=None means the entry
  is "unconstrained" and can satisfy any user variant. (Common in
  practice: most arm64 indexes don't set variant.)
- **Flag with `--image` unset.** clap's `requires = "image"` ensures
  this is a clap-level error, not a runtime check.
- **Empty / malformed flag value.** e.g. `--image-platform foo`,
  `--image-platform linux/`, `--image-platform /amd64`. Clear parse
  error from the value-parser; tests cover each shape.

## Functional requirements

- **FR-001**: `mikebom-cli/src/cli/scan_cmd.rs::ScanArgs` gains a new
  field `image_platform: Option<String>` with clap attributes:
  `#[arg(long, requires = "image", value_name = "linux/ARCH[/VARIANT]")]`.
  Doc-comment names the supported shapes + canonical OCI arch values.

- **FR-002**: `mikebom-cli/src/scan_fs/oci_pull/platform.rs` gains a
  module-private `parse_platform_string(s: &str) -> Result<ParsedPlatform>`
  with `ParsedPlatform { os: String, architecture: String, variant: Option<String> }`.
  Accepts `<os>/<arch>` and `<os>/<arch>/<variant>`. Rejects empty
  fields, more than two `/`-separators, non-`linux` OS values. Inline
  tests for ≥6 input shapes.

- **FR-003**: `mikebom-cli/src/scan_fs/oci_pull/platform.rs::ManifestListEntry`
  gains `pub variant: Option<String>`. The resolver becomes
  `resolve_manifest_list_to_linux<I>(entries, target_arch, target_variant: Option<&str>)`
  with a variant-aware match: entry matches if `os=="linux" &&
  architecture==target_arch && (target_variant.is_none() ||
  entry.variant.as_deref() == target_variant)`. Existing tests stay
  green; new tests cover variant-required + variant-as-superset cases.

- **FR-004**: `mikebom-cli/src/scan_fs/oci_pull/mod.rs::pull_to_tarball`
  signature becomes `pull_to_tarball(image_ref: &str, image_platform:
  Option<&str>) -> Result<TempDir>`. When `image_platform` is `Some`,
  parse via FR-002 and use the parsed arch+variant in the resolver
  call. When `None`, use `host_oci_arch()` (today's default).

- **FR-005**: `mod.rs` populates `ManifestListEntry.variant` from
  `oci_spec`'s `Platform::variant()` accessor in the existing
  manifest-index → ManifestListEntry mapping.

- **FR-006**: `mikebom-cli/src/cli/scan_cmd.rs` reject paths:
  - `--image <tarball-path> --image-platform <X>` → bail with the
    "only applies to registry refs" message before any registry code
    runs.
  - `--image-platform <not-linux>/<arch>` → the parser surfaces
    "only `linux/<arch>` is supported" and the bail propagates.

- **FR-007**: `mikebom-cli/tests/oci_registry_smoke.rs` gains a gated
  test (under `MIKEBOM_OCI_NETWORK_TESTS=1`) that pulls
  `alpine:3.19 --image-platform <non-host-arch>` and asserts the
  resulting SBOM has apk PURLs whose `arch=` qualifier reflects the
  cross-arch platform, not the host.

## Success criteria

- **SC-001**: `./scripts/pre-pr.sh` clean.
- **SC-002**: `git diff main..HEAD -- mikebom-cli/src/parity/ \
  mikebom-cli/src/generate/ mikebom-cli/src/resolve/` empty —
  no parity / generate / resolve touches.
- **SC-003**: 27-golden regen produces zero diff. `--image-platform`
  is a runtime knob; the byte-identity goldens were generated
  WITHOUT the flag, and FR-001's `Option<String>` default of `None`
  preserves the existing call site.
- **SC-004**: All 3 CI lanes green.
- **SC-005**: `wc -l mikebom-cli/src/scan_fs/oci_pull/platform.rs`
  ≤ 250 (today: 112; budget: ~138 LOC of additions).

## Clarifications

- **Why expose `--image-platform` rather than `--platform`?** The
  Docker CLI uses `--platform`. Reusing that name in mikebom would
  invite confusion when users mix mikebom invocations into pipelines
  that already pipe through `docker buildx build --platform`. The
  `--image-platform` name is unambiguous in mikebom's own CLI and
  signals scope (it's about the image being scanned).
- **Why reject non-linux OS?** mikebom's package-DB readers
  (dpkg/apk/rpm) are linux-rootfs-shaped. Even if we resolved a
  Windows manifest, the extracted layers would yield zero
  components and a confusing "no packages found" outcome. A clear
  upfront error is the right UX.
- **Why first-match wins for variant=None on user side?** Mirrors
  today's amd64/arm64 behaviour and matches Docker's own resolver.
  Determinism is bounded by index order; users who care about a
  specific variant should specify it.

## Out of scope

- **Multi-platform fan-out in one invocation** —
  `--image-platform linux/amd64,linux/arm64` producing two SBOMs.
  Defer; users can run twice today. File a follow-on if real demand
  surfaces.
- **`os_version` / `os_features`** — exotic Windows-server-only
  fields. We reject non-linux upfront so these don't matter.
- **`--platform` short alias** — keep the CLI surface narrow.
- **031.z layer caching** (#68) is the next OCI follow-on; orthogonal
  to this milestone.
