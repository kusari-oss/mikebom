# Feature Specification: Yocto / OpenEmbedded Reader

**Feature Branch**: `107-yocto-recipe-reader`
**Created**: 2026-06-01
**Status**: Draft
**Input**: User description: "Add a Yocto / OpenEmbedded recipe-based reader to mikebom so that scans of embedded Linux applications (OpenSTLinux, Poky-based BSPs, Yocto sysroots) emit components for the SDK / sysroot — libc, libstdc++, openssl, gstreamer, vendor HAL libraries, kernel modules, every package built into the image. Today these scans produce app-level-only SBOMs because no reader understands BitBake recipes, layer.conf metadata, or the build/tmp/deploy/ artifacts that ship with a BSP. This is the explicit follow-on to milestone 105 (US7 was split off during clarifications), and the largest remaining C/C++ coverage gap."

## Context

Yocto-based embedded Linux distributions (OpenSTLinux, Poky, Wind River, Mentor Embedded, BitBake-driven hardware-vendor BSPs) ship hundreds of curated packages built from layer recipes. mikebom currently emits zero of these components on four concrete scan shapes:

| Scan target | Today's SBOM | Components mikebom should produce | Required new reader |
|---|---|---|---|
| OpenSTLinux app source-tree (the typical developer scan against an SDK sysroot) | App-level only | The application's source files PLUS the sysroot it links against (libc, libstdc++, openssl, gstreamer, ST HAL, …) | opkg installed-DB reader + sysroot-context detection |
| Yocto build directory after `bitbake core-image-*` (CI/CD artifact) | 0 | Every package in the produced image's manifest | `manifests/<image>.manifest` reader |
| Yocto-built device rootfs at runtime (a flashed image dump or container snapshot) | 0 today (no opkg reader) | Every installed package | opkg installed-DB reader |
| Yocto layer tree (a `meta-vendor/` repo checked out in isolation, no build yet) | 0 | The recipes the layer declares (best-effort: name+version from `.bb` filenames) | BitBake recipe walker |

This milestone is the explicit follow-on to milestone 105, where US7 ("Yocto / OpenSTLinux recipe parsing") was split off during the clarifications session. Milestone 105 delivered C/C++ source-tree readers for CPM.cmake / Conan / west.yml / idf_component.yml / vcpkg classic / git-submodule correlation; this milestone closes the embedded Linux gap that 105 explicitly deferred and remains the largest C/C++ coverage gap. For an OpenSTLinux developer running `mikebom sbom scan --path /opt/st/openstlinux-6.6/sysroots/`, today's output is empty; with this milestone, that scan produces a complete SBOM of the cross-compile sysroot.

## Clarifications

### Session 2026-06-01

- Q: PURL ecosystem name for opkg / Yocto components? → A: **`pkg:opkg/<name>@<version>?arch=<arch>`** for installed-DB + manifest tiers (US1/US2/US3); **`pkg:bitbake/<recipe>@<version>?layer=<layer>`** for recipe-tier (US4). Mirrors syft's emission for opkg interop; two distinct source-mechanism enum values feed the milestone-105 dedup pipeline.
- Q: `.bb` filenames with unexpanded BitBake variables (`${PN}_${PV}.bb`) — skip silently or emit with `unresolved` sentinel? → A: **Skip silently** with `tracing::warn!` only. Avoids polluting the SBOM with placeholder components that downstream tools can't deduplicate or match against advisory databases.
- Q: Sysroot vs rootfs disambiguation heuristic — single-signal (include + no init.d) or multi-signal? → A: **Two-signal**: primary positive = presence of an `environment-setup-*` script (Yocto SDK installer always writes one) OR a `SDKPATHNATIVE`-style env-var in the same; secondary positive = `/usr/include/` present + `/etc/init.d/` absent. Components emit build-scope when EITHER signal fires. Handles systemd-based rootfs (no init.d but no env-script either) AND unusual SDK layouts.
- Q: Cross-tier dedup when the same canonical PURL appears in both a Yocto build-dir manifest AND a device rootfs's opkg-installed-DB during one scan? → A: **Collapse by canonical PURL** per milestone 105's dedup pipeline. Same coord → one component; lifecycle-scope from the higher-precedence reader; loser's source-mechanism recorded in `mikebom:also-detected-via`. The "without conflating" language in Edge Cases applies to **different** packages (build-tier `nativesdk-openssl` ≠ runtime-tier `openssl`), not to the same package seen by two readers.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Scan a Yocto-built device rootfs (Priority: P1) 🎯 MVP

A platform-security engineer ships an embedded Linux device. Once a week they grab the latest rootfs image, extract its filesystem, and run `mikebom sbom scan --path /tmp/rootfs/`. Today they see zero `pkg:*/` components; with this milestone they see one component per package the BitBake build installed into the image, drawn from the rootfs's opkg installed-DB at `/var/lib/opkg/status`.

**Why this priority**: opkg is the package manager used by every Yocto/OE-based distribution that doesn't explicitly opt into rpm or dpkg. The opkg installed-DB has the same shape as dpkg's `/var/lib/dpkg/status` (which mikebom already reads), so the implementation pattern is well-trodden and the impact is immediate — every Yocto-built rootfs becomes scannable. This is the closest analogue to the existing `dpkg.rs` / `apk.rs` / `rpm.rs` ecosystem readers.

**Independent Test**: Take a Yocto reference image (e.g. `poky-glibc-x86_64-core-image-minimal-qemux86-toolchain-*.tar.xz`), extract it to a tempdir, and run `mikebom sbom scan --path <tempdir>/sysroot/ --format cyclonedx-json`. Assert: at least 20 components emerge with the chosen Yocto PURL form; `libc` / `busybox` / `kernel-image-*` are present; each component carries the package's reported version, architecture, and a deterministic source-files annotation pointing at `/var/lib/opkg/status` or per-package `/usr/lib/opkg/info/<pkg>.list`.

**Acceptance Scenarios**:

1. **Given** a Yocto/OE rootfs containing `/var/lib/opkg/status` with N package stanzas, **When** I run `mikebom sbom scan --path <rootfs>/`, **Then** the emitted SBOM contains N components — one per stanza — each with a PURL, version, and `mikebom:source-files` annotation naming the opkg DB path.
2. **Given** a rootfs where a package's `/usr/lib/opkg/info/<pkg>.list` enumerates installed files, **When** the binary walker also examines those files, **Then** the binary walker MUST skip files claimed by opkg (same claim-path pattern dpkg / apk / rpm use today) so we don't emit duplicate `pkg:generic/<basename>` components alongside the real `pkg:opkg/<name>`.
3. **Given** an opkg-installed package carrying a `License:` field in its status stanza, **When** mikebom emits the component, **Then** the license field flows into `licenses[]` in the CycloneDX output and the corresponding SPDX expression in SPDX 2.3 / 3.

---

### User Story 2 — Scan a Yocto build directory's image manifest (Priority: P1)

A CI/CD pipeline runs `bitbake core-image-st-image-weston` on every merge. After the build, the pipeline runs `mikebom sbom scan --path build/tmp/deploy/images/stm32mp1/` and uploads the emitted SBOM as an artifact. mikebom reads `<image-name>.manifest` (one line per package: `<name> <arch> <version>`) and emits one component per line.

**Why this priority**: this is the canonical SBOM-generation point in a Yocto build pipeline. The `.manifest` file is **authoritative** — it's what BitBake itself recorded as the contents of the image — and it's machine-readable without any recipe parsing. Implementation is line-oriented and trivially testable. Together with US1, this covers the two highest-impact scan shapes that don't require BitBake variable expansion.

**Independent Test**: Vendor a `.manifest` fixture in `tests/fixtures/golden_inputs/yocto_manifest/`; assert the resulting SBOM contains one component per `.manifest` line with the correct (name, version, arch) triple and a `mikebom:source-files` annotation pointing at the manifest path.

**Acceptance Scenarios**:

1. **Given** a Yocto build directory containing `build/tmp/deploy/images/<machine>/<image>.manifest`, **When** I run `mikebom sbom scan --path build/tmp/deploy/`, **Then** the SBOM contains one component per manifest line.
2. **Given** a `.manifest` referencing the same package under multiple architectures (target + nativesdk), **When** mikebom emits components, **Then** both architectures emerge as distinct components with `arch` qualifiers, and a `mikebom:component-role` annotation distinguishes target vs nativesdk-host.

---

### User Story 3 — Scan a cross-compile SDK sysroot (Priority: P2)

An OpenSTLinux app developer extracts the vendor SDK to `/opt/st/openstlinux-6.6/sysroots/` and runs `mikebom sbom scan --path /opt/st/openstlinux-6.6/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi/`. The sysroot is a complete Yocto-built rootfs structure (no actual device, just headers + libraries an app links against). mikebom reads its opkg installed-DB the same way US1 reads a device rootfs, but additionally tags every component with `mikebom:lifecycle-scope: "build"` because nothing in the sysroot ships to the device at runtime — it's compile-time only.

**Why this priority**: this is the daily scenario for embedded developers (versus US1's once-a-week CI job). The implementation reuses US1's opkg reader; the only new logic is the sysroot-vs-rootfs distinction.

**Independent Test**: Vendor a stripped-down sysroot fixture (just the opkg DB + a couple `.list` files) at `tests/fixtures/golden_inputs/yocto_sysroot/`; assert the scan emits sysroot components tagged with `lifecycle-scope: "build"` and that the milestone-052 emission path translates that to CDX `scope: "excluded"`.

**Acceptance Scenarios**:

1. **Given** a Yocto SDK sysroot (has an `environment-setup-*` script in the parent dir), **When** mikebom scans it, **Then** every emitted component carries `lifecycle-scope: "build"`, regardless of whether `/etc/init.d/` is present.
2. **Given** a directory matching only the secondary signal (`/usr/include/` present + `/etc/init.d/` absent), **When** mikebom scans it, **Then** every emitted component carries `lifecycle-scope: "build"` (the secondary signal alone is sufficient — common for hand-extracted SDK sysroots).
3. **Given** a systemd-based device rootfs (no `/etc/init.d/`, but no env-script either, and `/usr/include/` absent), **When** mikebom scans it, **Then** components emit WITHOUT build-scope (correctly identified as a runtime rootfs).
4. **Given** a sysroot where the primary signal fires but the secondary disagrees (env-script present AND `/etc/init.d/` also present — rare but possible in some SDK layouts), **When** mikebom scans it, **Then** the build-scope tag is applied (primary signal wins) and a `mikebom:scan-ambiguity` diagnostic annotation is recorded on the SBOM metadata.

---

### User Story 4 — Scan a Yocto layer tree (Priority: P3)

A vendor publishes `meta-<vendor>/` on GitHub. A security researcher clones it and runs `mikebom sbom scan --path meta-vendor/` to enumerate what packages the layer is *declared* to build, before any build runs. mikebom walks `recipes-*/<name>/<name>_<version>.bb` files and emits one component per recipe, drawn from the `.bb` filename pattern without any BitBake variable expansion.

**Why this priority**: this is the lowest-impact scan shape because it doesn't tell you what's *actually shipped* — only what the layer is *capable of shipping* — but it's the only signal for a layer-tree scan with no build artifacts present. Useful for supply-chain pre-screening of vendor layers before adoption. Lower priority because it doesn't require variable expansion (just regex on filenames) but produces a less authoritative SBOM than US1/US2.

**Independent Test**: Vendor a small fixture mimicking a `recipes-*/` directory tree; assert the scan emits one component per `.bb` file with name + version extracted from the filename, and `mikebom:source-files` pointing at the `.bb` file.

**Acceptance Scenarios**:

1. **Given** a `meta-foo/recipes-bar/baz/baz_1.2.3.bb` file, **When** mikebom scans the layer, **Then** the emitted SBOM contains a component for `baz@1.2.3` annotated as recipe-tier evidence.
2. **Given** a `.bb` file using BitBake variable expansion in its filename (`${PN}_${PV}.bb`), **When** the parser encounters it, **Then** the reader emits a `tracing::warn!` and skips the recipe (FR-008 warn-and-continue; no abort).

---

### User Story 5 — Distinguish nativesdk / target / multilib variants (Priority: P3)

A Yocto image often contains multiple variants of the same package: the target ARM build of `openssl`, the host x86_64 `nativesdk-openssl` used by the build itself, and possibly multilib variants. mikebom tags these distinctly so downstream consumers can filter by lifecycle scope.

**Why this priority**: improves accuracy of the SBOMs US1/US2 produce. Lower priority because it's a refinement of an already-shipping pipeline rather than a new scan shape. The information is already in the opkg DB stanza (`Architecture:` field) — this milestone just ensures we annotate it correctly.

**Independent Test**: Scan a sysroot containing both `openssl_3.0.5-r0_armhf` AND `nativesdk-openssl_3.0.5-r0_x86_64`; assert both components emerge with distinct PURLs (different arch qualifiers) and lifecycle-scope tags that distinguish target-runtime from native-host-tool.

**Acceptance Scenarios**:

1. **Given** an opkg DB with both `openssl_arm` and `nativesdk-openssl_x86_64` stanzas, **When** mikebom emits components, **Then** the target build carries `lifecycle-scope` absent (runtime default) and the nativesdk build carries `lifecycle-scope: "build"`.
2. **Given** a multilib variant (`lib32-openssl_3.0.5_x86_64`), **When** mikebom emits it, **Then** the component's PURL qualifiers or annotations indicate the multilib origin so it's filterable.

---

### Edge Cases

- **Hand-rolled / vendor-modified opkg DBs**: some BSPs ship opkg with non-standard control fields. The reader MUST treat unknown stanza fields as informational (preserved in `extra_annotations` when known to be useful; silently ignored otherwise) and never abort.
- **Missing `Version:` field**: opkg's source stanza is technically permitted to omit `Version:` in odd-ball cases. Reader emits the package with `mikebom:version-status: "missing"` annotation rather than dropping the entry.
- **License field carrying Yocto-specific identifiers** (e.g. `LICENSE = "GPL-2.0-or-later & LGPL-2.1-or-later"`): the reader passes the field verbatim to the existing license-resolution pipeline without attempting Yocto-specific translation.
- **Recipe file with no `_<version>` segment** (rare but legal `.bb`): emitted with `version: "unknown"` and a `mikebom:version-status: "missing"` annotation.
- **`<image>.manifest` file in an unexpected location** (BSP customizations may put it elsewhere): mikebom walks `build/tmp/deploy/images/*/` exhaustively rather than hard-coding a single path.
- **Conflicting package names across multiple opkg feeds** (the same `<name>` appearing in both a vendor feed and the upstream OE feed): mikebom emits both as distinct components — they have different version + arch and the feed identity is recorded in the source-files annotation.
- **Stripped rootfs without `/var/lib/opkg/status`** (common in production images that delete opkg metadata to save space): reader emits a diagnostic warning into the SBOM metadata; the scan still completes via the binary walker's existing path-pattern matching.
- **Container image of a Yocto build** (a CI worker's container with the build environment): the container will have BOTH a Yocto build directory AND a Yocto-built target rootfs mounted. Distinct identities (build-tier `nativesdk-openssl` vs runtime-tier `openssl`) emerge as separate components with different lifecycle-scope tags per US3 + US5. Identical canonical PURLs that genuinely appear in both sources (e.g. the same runtime `openssl@3.0.5?arch=armhf` showing up in both the image's `.manifest` AND the rootfs's opkg-installed-DB) collapse via milestone 105's dedup pipeline into a single component; the lower-precedence reader's source-mechanism is recorded in `mikebom:also-detected-via`.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: mikebom MUST read opkg's installed-package database at `/var/lib/opkg/status` (stanza-formatted, same shape as dpkg's status file) and emit one component per package stanza.
- **FR-002**: mikebom MUST collect file claims from `/usr/lib/opkg/info/<pkg>.list` so the binary walker doesn't emit duplicate `pkg:generic/<basename>` components for files already owned by an opkg package (same pattern as dpkg / apk / rpm).
- **FR-003**: mikebom MUST read `build/tmp/deploy/images/<machine>/<image>.manifest` line-oriented format (`<name> <arch> <version>`) and emit one component per line.
- **FR-004**: mikebom MUST emit PURLs as **`pkg:opkg/<name>@<version>?arch=<arch>`** for installed-DB and image-manifest readers (US1, US2, US3), and **`pkg:bitbake/<recipe>@<version>?layer=<layer>`** for the BitBake recipe walker (US4). PURL segments MUST be percent-encoded per the package-url spec. Architecture qualifier values pass through verbatim from the source (`armhf` / `x86_64` / `cortexa7t2hf-neon-vfpv4` / etc.).
- **FR-005**: mikebom MUST detect sysroot vs rootfs context via filesystem-shape heuristics (FR-005a) and tag sysroot components with `lifecycle-scope: "build"`.
- **FR-005a**: Sysroot detection MUST use a two-signal heuristic. **Primary signal**: presence of an `environment-setup-*` script in the scan-target dir or its immediate parent (Yocto's SDK installer always writes one — e.g. `environment-setup-cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi`). **Secondary signal**: `/usr/include/` present AND `/etc/init.d/` absent within the scan target. A scan is treated as a sysroot when EITHER signal fires. When neither fires it's treated as a normal rootfs (no build-scope tag). Cases where the primary signal fires but the secondary signal disagrees (or vice versa) record a `mikebom:scan-ambiguity` diagnostic annotation on the SBOM metadata but still apply the build-scope tag — the primary env-script signal is authoritative when present.
- **FR-006**: For nativesdk-prefixed packages and packages whose `Architecture:` field names a known host arch literal (`x86_64`, `i686`, `aarch64`, `arm64`), mikebom MUST emit them with `lifecycle-scope: "build"` regardless of containing-directory context. The host arch literal list is maintained in `contracts/opkg-installed-db.md`; adding a new host arch (e.g., a future RISC-V dev machine) is an implementation-detail extension that does NOT require a spec amendment.
- **FR-007**: mikebom MUST walk `meta-*/recipes-*/<name>/<name>_<version>.bb` files when scanning a layer tree (US4) and emit one component per recipe. The reader MUST extract name + version from the filename (`<name>_<version>.bb`) and MUST NOT attempt BitBake variable expansion within the recipe body in this milestone.
- **FR-008**: `.bb` files with variable-expansion patterns in their filenames (`${PN}_${PV}.bb`, etc.) MUST emit a `tracing::warn!` naming the recipe path and MUST be skipped silently — no component emitted, no placeholder, no `unresolved` sentinel. Variable-expansion support is explicit out-of-scope; downstream consumers who care about which recipes were skipped can grep the scan logs.
- **FR-009**: All license fields read from opkg stanzas (the `License:` field) MUST pass verbatim through to the existing license-resolution pipeline. mikebom MUST NOT introduce Yocto-specific license-name translations in this milestone.
- **FR-010**: All readers in this milestone MUST honor the milestone-105 dedup pipeline pattern. The new source-mechanism enum values are: **`opkg-installed`** (US1, US3 — `/var/lib/opkg/status` reader), **`yocto-image-manifest`** (US2 — `build/tmp/deploy/images/<machine>/<image>.manifest` reader), **`bitbake-recipe`** (US4 — `.bb` filename walker). The dedup pipeline applies the established precedence + `mikebom:also-detected-via` annotation when multiple readers identify the same canonical PURL. Per the milestone-105 precedence convention, the more-authoritative installed-DB readers (`opkg-installed`, `yocto-image-manifest`) outrank the layer-declaration `bitbake-recipe` reader when canonical PURLs collide cross-tier.
- **FR-011**: All readers MUST be pure filesystem-only (FR-012 audit pattern carried over from milestone 106 — no network, no subprocess, no `reqwest::` / `tokio::net::` / `Command::new("curl"|"wget")` calls). The build-time audit MUST be extended to grep the new reader modules.
- **FR-012**: Parse failures in any reader MUST emit a `tracing::warn!` and skip the offending entry rather than aborting the scan (warn-and-continue per the FR-015 pattern carried over from milestones 105/106).
- **FR-013**: Documentation MUST be updated: `docs/ecosystems.md` gains a new top-level section covering the new detection sources, and the coverage matrix gains a row.
- **FR-014**: Polyglot robustness MUST extend the milestone-106 SC-006 pattern — a new regression test placing well-formed AND malformed Yocto inputs (broken `opkg/status` stanza, malformed `.manifest`, unparseable `.bb` filename) alongside well-formed inputs from previously-shipping ecosystems; scan MUST exit 0 and each well-formed ecosystem MUST still emit its representative component.
- **FR-015**: No new Cargo dependencies. The opkg DB stanza format is identical to dpkg's, so the existing dpkg parser machinery in `mikebom-cli/src/scan_fs/package_db/dpkg.rs` can be refactored / reused. The `<image>.manifest` parser is line-oriented (std only). The `.bb` recipe walker uses `walkdir` + `regex` (both workspace deps).

### Key Entities

- **Opkg installed-package stanza**: name, version, architecture, maintainer, license, source-files (the `<pkg>.list` path), claimed-files (the file paths inside `<pkg>.list`). Stanza format is field-name + `:` + space + value, blank-line-separated (identical to dpkg).
- **Yocto image manifest**: line-oriented `<name> <arch> <version>` triples in `build/tmp/deploy/images/<machine>/<image>.manifest`. No nested structure; one component per line.
- **BitBake `.bb` recipe file**: filename pattern `<name>_<version>.bb` in a `recipes-*/<name>/` directory under a layer's root. This milestone reads only the filename — the recipe body is not parsed.
- **SDK sysroot vs rootfs context**: a filesystem-shape attribute attached to the SBOM emission (informs `lifecycle-scope` tagging per US3). Not a per-component field.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An OpenSTLinux-6.6 SDK sysroot scan emits ≥150 components today (post-milestone) vs 0 components pre-milestone. Specific representative components present: `libc`, `libstdc++`, `openssl`, `gstreamer1.0`, `linux-libc-headers`. **Verified manually** via the Phase 6 quickstart Scenario 3 (T052) against an extracted SDK sysroot — same posture as milestone 105's component-count validation. Maintaining a vendored 150-component sysroot fixture in-repo would inflate the fixture corpus disproportionately (~5 MB of mocked stanzas vs the rest of the in-repo fixture set's ~50 KB).
- **SC-002**: A Poky reference image's `core-image-minimal.manifest` scan emits exactly N components where N is the manifest's line count.
- **SC-003**: Run-time of a typical scan (USDK sysroot, ~250 packages) MUST NOT exceed 5% over the milestone-106 baseline (54.2s wall-clock workspace-test gate).
- **SC-004**: The SC-006 polyglot regression test extension passes: malformed Yocto inputs (broken opkg stanza, malformed manifest, unparseable `.bb`) MUST NOT abort the scan and MUST NOT block other ecosystems (uv, Bun, Gradle, NuGet, Yarn — established by milestone 106) from emitting components from their well-formed siblings.
- **SC-005**: A Yocto-build-directory scan (US2) produces a deterministic byte-identity SBOM across runs (same scan, same SHA-256). The deterministic-emission invariant is verified by the existing milestone-052 byte-identity golden suite (any non-determinism in the milestone-107 readers would surface as a golden diff at release-cut time when the workspace-wide goldens regenerate). **Dedicated US2 byte-identity goldens are deferred to follow-up** per the milestone-106 precedent (uv / Bun / Gradle / NuGet / Yarn goldens all deferred with the same rationale): emission shape goes through the existing `PackageDbEntry → ResolvedComponent` pipeline; existing per-ecosystem goldens cover byte-identity broadly. Dedicated yocto-manifest goldens will be added if any parity round-trip skew surfaces.
- **SC-006**: All new reader source files (`opkg.rs`, `yocto/mod.rs`, `yocto/manifest.rs`, `yocto/recipe.rs`, `yocto/context.rs`) clear the FR-011 offline-mode audit — no `reqwest::` / `tokio::net::` / `hyper::` / `Command::new("curl"|"wget"|"http"` / `TcpStream` / `TcpListener` / `std::net::TcpStream|TcpListener` substring appears in any of them. Verified at build-time by the audit test landing in T049.
- **SC-007**: All four new readers populate the milestone-105 dedup pipeline's source-mechanism enum, and the cross-reader determinism test (e.g. opkg-installed + yocto-manifest both identifying `openssl@3.0.5`) produces deterministic precedence with `mikebom:also-detected-via` annotation per the milestone-105 contract.

## Assumptions

- **opkg installed-DB shape is dpkg-compatible enough** that the existing `mikebom-cli/src/scan_fs/package_db/dpkg.rs` parser can be refactored / reused. If opkg's stanza format diverges materially, planning re-evaluates whether to factor `dpkg.rs` into a shared `package_db/control_file.rs` helper or write a parallel implementation.
- **No new Cargo dependencies needed.** Reuses existing workspace deps.
- **The reader is filesystem-only.** No `bitbake` subprocess invocation, no live opkg feed lookups, no recipe-variable expansion. This matches the FR-011 offline-only constraint carried over from milestones 105/106.
- **Variable expansion in `.bb` recipes is explicit out-of-scope** (FR-008). A future "yocto-variable-expansion" milestone can layer on top if there's demand to extract dependency edges from recipe bodies (e.g. `DEPENDS = "libc-headers openssl"`).
- **Yocto-specific license-expression translation is out-of-scope** (FR-009). The existing SPDX expression pipeline already handles standard SPDX names; Yocto-specific names flow through verbatim.
- **Multilib + nativesdk variants are emitted as distinct components**, not collapsed. mikebom's existing dedup pipeline keys on canonical PURL string (per milestone 105); since arch is part of the PURL qualifier, target + nativesdk variants will keep distinct identities without any new dedup logic.
- **Existing milestone 105 + 106 invariants carry forward**: dedup pipeline + parity catalog + FR-012 offline audit + SC-006 polyglot robustness + synthetic-fixture-naming convention all apply unchanged.

## Out of Scope (this milestone)

- BitBake variable expansion in `.bb` recipe bodies. Recipe-tier emission is filename-only.
- Yocto-specific license-name translation.
- `bitbake -c <task>` introspection. mikebom does not invoke `bitbake`.
- Dependency edges between recipes (`DEPENDS`, `RDEPENDS_${PN}`). Recipe-tier emission is identity-only in this milestone; recipe-body dep-graph is a follow-on.
- PURL spec registration for any new ecosystem chosen in Q1 (parallel doc-deliverable; happens upstream at the package-url repo).
- `manifest.import:` transitive chasing in `west.yml` (milestone 105's other deferred item — separate milestone).
- CPM.cmake less-common shapes (milestone 105's third deferred item — separate milestone).

## Open Clarifications (resolve via `/speckit.clarify`)

(All initial open clarifications resolved in Session 2026-06-01.)
