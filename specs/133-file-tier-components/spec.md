# Feature Specification: File-tier component emission for unattributed content

**Feature Branch**: `133-file-tier-components`
**Created**: 2026-06-19
**Status**: Draft
**Input**: User description: "follow the best path forward and include some of trivy's good ideas as well here" — formalizing the milestone-132 close-out design conversation about whether/how mikebom should emit file-level components to address the Completeness 1★ vs 5★ structural gap captured in `specs/132-sc-closeout/spec.md §Out of Scope`.

## Context

Milestone 132 closed 6 of 7 SCs against the pinned audit baseline. The one structural gap not addressed by metadata work was **Completeness 1★ (mikebom) vs 5★ (syft)** — driven by syft emitting 27,006 file-tier components (no PURL, pure file inventory) on top of its 3,772 package-tier components. mikebom today emits ZERO file-tier components; files surviving every package-DB / binary-fingerprint reader simply vanish from the SBOM.

Three industry design points exist, surveyed against the pinned audit image during the close-out conversation:

| Tool | Total components | Package-tier (with PURL) | File-tier (no PURL) | Approach |
|---|---|---|---|---|
| **Syft** | 30,778 | 3,772 | 27,006 | Per-file inventory as separate components; per-(path,hash) duplication |
| **Trivy** | 581 | 579 | 0 | Package-tier only; per-(package,path) duplication via `aquasecurity:trivy:FilePath` property |
| **mikebom (today)** | 2,926 | 2,926 | 0 | Package-tier only; per-unique-package dedupe; no path context |

The user's framing during the close-out conversation: **"if we know the package we don't need to track every file, but if we don't we still want people to know about stuff."** That points to file-tier emission as a *fallback* for unattributed content, not blanket emission. A separate explicit "give me everything" mode preserves the forensic/diff/malware-detection use cases.

Trivy's design choice — path/layer context as properties on package-tier components — is a complementary additive win that mikebom doesn't have today. Adding it costs almost nothing in SBOM size but enables real forensic queries.

## Clarifications

### Session 2026-06-19

- Q: Default behavior change — flip to orphan in 133, or stage it? → A: Flip default to `orphan` in milestone 133. Existing users see ~200-800 new file-tier components per image scan on upgrade; all new components are content-shape-gated (signal, not noise) and carry a `mikebom:component-tier = "file"` annotation so consumers can filter them out client-side. Operators wanting bit-for-bit pre-133 behavior opt out via `--file-inventory=off`. Codified in FR-015. The CHANGELOG entry for milestone 133 MUST explicitly call out this default flip as a behavior change so consumers reading the release notes see it.
- Q: SC-001 measurement — measure-first during planning, or measure at verification? → A: Measure-first during planning. **BLOCKING**: before any production code lands, `/speckit-plan` MUST produce a one-off rootfs-walk projection tool that walks the milestone-132 pinned audit baseline after current mikebom emission, applies the FR-005 content-shape allowlist, and reports the projected orphan-emit count. The measured count drives a TIGHTENED SC-001 range recorded in `research.md §Orphan projection`. If the measured count is outside the original 200-800 band, the FR-005 allowlist is adjusted at plan time (NOT after code lands). Closes the assumption-driven pattern milestone 132's three plan-corrections taught us. Tracked as a new FR-022 obligation under §Cross-cutting.
- Q: Orphan dedupe semantics — hash, path, or both? → A: Hybrid — skip file-tier emission if EITHER (a) the file's path appears in a package-tier component's `mikebom:component-paths` property OR (b) the file's SHA-256 matches a binary-tier component's hash (milestone-104 binary readers carry per-file hashes). The original FR-011 hash-only rule was broken because package-tier readers carry package-identity hashes, not per-file hashes — under the literal rule, every package-owned file would over-emit as orphan. Hybrid handles the common case (apk/dpkg/cargo packages → US2 `mikebom:component-paths` makes their files path-covered) without requiring every reader to populate exhaustive owned-file lists. Reader-level path coverage CAN expand over time (e.g. milestone 134 might add apk `.list` parsing for full file ownership); the hybrid contract stays unchanged. FR-011 rewritten in-place.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Orphan file-tier emission (Priority: P1, headline user-value)

**Implementation-order note**: although US1 is the headline user-value increment for milestone 133 (surfacing unattributed content was the structural Completeness gap from milestone 132), **US2 ships FIRST as the implementation MVP** because US1's FR-011 hybrid dedupe reads `mikebom:component-paths` annotations US2 emits — without US2 already merged, US1 over-emits ~13× the SC-001 budget (3 276 vs 245 per the FR-022 projection). See `tasks.md §Critical sequencing note` for the merge-order contract.

**As a security consumer** scanning an OCI image with custom binaries dropped into `/opt/vendor/` or static-linked artifacts vendored without a manifest, **I want mikebom to surface those files as components** so my vulnerability scanner / license auditor / SBOM consumer can SEE the unattributed content instead of silently missing it.

**Why this priority**: Closes the highest-signal portion of the Completeness 1★ gap. The audit baseline has ~200-800 such files (custom binaries, vendored libraries without package metadata, embedded archives). These are real components mikebom is omitting today; they represent the bulk of the "unknown" surface area on a typical container image.

**Independent Test**: scan an image containing a known unattributed binary (e.g., a `curl-static` binary at `/usr/local/bin/curl` with no apk/dpkg package owning it); assert the emitted SBOM contains a file-tier component for that binary with the correct SHA-256, with a `mikebom:component-tier = "file"` annotation, and that the package-tier component count is unchanged from a pre-milestone-133 scan of the same image.

**Acceptance Scenarios**:

1. **Given** an image with a binary at `/opt/custom-tool` not in any package DB and not matching any binary-tier symbol fingerprint, **When** the image is scanned in default mode, **Then** the emitted SBOM contains a file-tier component for `/opt/custom-tool` with its SHA-256, with no PURL, with a `mikebom:component-tier = "file"` annotation, and the existing package-tier component count is unchanged.
2. **Given** a file already covered by a package-tier component (e.g., `/usr/bin/curl` owned by apk's `curl` package), **When** the image is scanned in default mode, **Then** NO file-tier component is emitted for that file (Principle IX accuracy — no duplicate).
3. **Given** a source code file like `/app/src/main.rs` or a config like `/etc/hostname`, **When** the image is scanned in default mode, **Then** NO file-tier component is emitted (content-shape allowlist filter prevents source/config noise).
4. **Given** the same file content (identical SHA-256) appears at two paths neither covered by a package, **When** the image is scanned in default mode, **Then** ONE file-tier component is emitted with both paths in the `mikebom:file-paths` property array (per-unique-hash dedupe).

---

### User Story 2 — Path coverage + layer-digest for package-tier components (Priority: P1, implementation MVP)

**Spec-correction history (2026-06-20, see milestone-132 §Honest accounting pattern)**: the original "trivy-style path/layer" framing was based on incomplete reads of the codebase. Ground-truth discovery during implementation prep:

- mikebom **already emits** a `mikebom:source-files` property on every component carrying the rootfs path the reader identified the package from. The original plan to add a `mikebom:component-path` annotation would have duplicated this existing field — direct Principle V violation.
- The existing `mikebom:source-files` property has **three real defects**: (A) leaks the macOS tempdir/rootfs prefix (`/private/var/folders/.../mikebom-image-XXX/rootfs/`); (B) emits as comma-separated string instead of JSON array; (C) leading `/` not stripped.
- mikebom **richly populates** CDX-native `evidence.occurrences[].location` for OS-package readers (apk/dpkg/rpm — 177 / 177 = 100 % coverage on the audit baseline) but emits **zero** occurrences for any language-ecosystem reader (cargo/npm/nuget/maven/pypi/gem/golang — 2 749 components, 0 occurrences). This is the real path-coverage gap.
- `mikebom:layer-digest` genuinely doesn't exist anywhere — this remains a valid new parity-bridge.

US2 corrected scope: (1) fix the 3 defects on existing `mikebom:source-files`; (2) add `mikebom:layer-digest` for image scans; (3) populate CDX-native `evidence.occurrences[].location` for every language-ecosystem reader.

**As a forensic / diff / supply-chain analyst** querying "what package lives at path X?" or "which OCI layer introduced package Y?", **I want every package-tier component to carry its rootfs path (correctly normalized, not leaking the scanner host's tempdir) and (when applicable) source layer digest** so I can answer those queries against any mikebom SBOM without re-scanning the image.

**Why this priority**: P1 alongside US1 because two of its three deliverables are real bug fixes (tempdir leak; non-machine-parseable comma-string) on an already-shipped property; the third (layer-digest) closes a genuine standards-native gap; and the fourth (evidence.occurrences[] coverage) lifts mikebom from 6 % component path coverage to ~100 % using the CDX-native field.

**Independent Test**: scan an image; for every component, assert `properties[mikebom:source-files]` is present, contains NO tempdir-prefix substring (`/private/var/folders/` / `/tmp/mikebom-image-`), is a JSON-encoded array, and has no leading `/` on any path. For image scans assert `properties[mikebom:layer-digest]` is present and matches the OCI layer containing the source path. For every language-ecosystem component, assert `evidence.occurrences[].location` is non-empty.

**Acceptance Scenarios**:

1. **Given** an apk package identified by reading `/var/lib/apk/db/installed`, **When** the image is scanned, **Then** the emitted component's `mikebom:source-files` property carries the value `["var/lib/apk/db/installed"]` (JSON array, no leading `/`, no tempdir prefix) AND `mikebom:layer-digest = "sha256:<the apk layer digest>"`.
2. **Given** a `pkg:cargo` component identified from cargo-auditable bytes inside `/usr/local/bin/foo`, **When** the image is scanned, **Then** the emitted component carries `evidence.occurrences[0].location = "usr/local/bin/foo"` (CDX-native, no leading `/`, no tempdir prefix) AND `mikebom:layer-digest`. The component MUST NOT be missing path data the way pre-milestone-133 cargo components were.
3. **Given** a package-tier component identified from a non-image scan (`mikebom sbom scan --path .`), **When** the SBOM is emitted, **Then** `mikebom:source-files` is populated from the source path BUT `mikebom:layer-digest` is omitted (not applicable; no OCI layer concept).
4. **Given** a package-tier component identified purely from network / registry metadata (no rootfs path), **When** the SBOM is emitted, **Then** neither `mikebom:source-files` nor `mikebom:layer-digest` is emitted.

---

### User Story 3 — Opt-in full file inventory mode (Priority: P2)

**As a forensic analyst / malware hunter / image-diff tool maintainer**, **I want a single flag that surfaces every file on the image as a component** regardless of package coverage, so I can write queries like "is `sha256:abc...` (a known IOC) present anywhere on this image?" or "what files changed between image v1 and v2?" against the SBOM alone.

**Why this priority**: P2 because the orphan-default (US1) handles the common cases; full mode addresses a narrower but real set of use cases. Shipping it also closes the formal Completeness 5★ band on the sbom-comparison scorecard, which has measurable comparison value.

**Independent Test**: scan the same image twice — once with default mode, once with `--file-inventory=full`. Assert: (a) `full` mode component count is at least 10× the `default` component count; (b) `full` mode emits one file-tier component per unique SHA-256 across the entire rootfs; (c) `full` mode SBOM carries a document-level `mikebom:file-inventory-mode = "full"` annotation; (d) consumers can identify a file by hash via a single component lookup.

**Acceptance Scenarios**:

1. **Given** an image where `/etc/ssl/openssl.cnf` is owned by both `openssl` and a custom layer's vendored copy with identical content, **When** the image is scanned with `--file-inventory=full`, **Then** ONE file-tier component is emitted for that hash with both paths listed in `mikebom:file-paths`, AND the package-tier `openssl` component is also still present (full mode does NOT replace package-tier emission).
2. **Given** an image with 2,926 package-tier components and ~3,500 unique file hashes in the rootfs, **When** the image is scanned with `--file-inventory=full`, **Then** the SBOM contains ~6,400 total components (~3,500 file-tier + ~2,926 package-tier; exact count depends on rootfs walk).
3. **Given** the same image, **When** the image is scanned with `--file-inventory=orphan` (the default), **Then** the SBOM contains only ~200-800 file-tier components (the unattributed subset) + the unchanged 2,926 package-tier.

---

### User Story 4 — Constitution amendment + component-tiers reference doc (Priority: P3)

**As a future mikebom contributor** trying to understand what file-tier emission is permitted, what counts as "orphan", and how the three tiers compose, **I want a single authoritative reference doc** plus a Constitution Strict Boundary I can cite during PR review.

**Why this priority**: P3 because the user-facing behavior (US1-US3) can ship without the documentation work; the docs are the durability layer for future contributors and Principle V audit citations. Lower-priority but blocking the milestone closeout.

**Independent Test**: a new contributor reading `docs/reference/component-tiers.md` plus the amended Constitution can correctly answer: "given file X at path Y, would it emit in default mode? what about full mode? what tier?" without reading any source code. Plus: every `mikebom:*` annotation new to milestone 133 has a C-row in `docs/reference/sbom-format-mapping.md` with a Principle V audit clause.

**Acceptance Scenarios**:

1. **Given** the new `docs/reference/component-tiers.md`, **When** a reviewer is auditing a future PR that touches file-tier emission, **Then** they can cite the doc's orphan-content-shape allowlist + precedence rules + Strict Boundary §5 to evaluate the PR's compliance.
2. **Given** the Constitution amendment (version 1.4.0 → 1.5.0), **When** the change is reviewed, **Then** it carries: (a) a new Strict Boundary §5 documenting the "no duplicates in default mode" rule with the explicit `--file-inventory=full` override carve-out; (b) a Principle VIII Completeness clarification noting unattributed content; (c) NO new top-level Principle (the existing VIII + IX + X cover the design space).
3. **Given** the new C-rows in `docs/reference/sbom-format-mapping.md` for `mikebom:component-tier`, `mikebom:component-path`, `mikebom:layer-digest`, `mikebom:file-paths`, `mikebom:file-inventory-mode`, **When** Principle V audit is run during PR review, **Then** each row contains an inline audit clause showing the native CDX 1.6 / SPDX 2.3 / SPDX 3 constructs considered and why the parity-bridging `mikebom:*` annotation is justified for each.

---

### Edge Cases

- **Symlinks**: existing scan_fs walkers already resolve them; file-tier emission keys on the resolved target's hash, not the link. Don't double-count.
- **Sparse files, device files, sockets, FIFOs**: skip — they aren't content. Annotate at document level if any encountered (Principle X transparency: `mikebom:file-inventory-skipped-special-files = <count>`).
- **Files inside compressed archives we don't unpack**: out of scope. That's archive-tier emission; file-tier is for the host rootfs, not nested.
- **Files larger than 100 MB**: skip by default to avoid hashing-time explosion. Operator override via `--file-inventory-size-limit <bytes>`. Skipped files surface as a document-level `mikebom:file-inventory-skipped-oversize` count annotation.
- **Zero-byte files**: hash is the canonical empty-SHA256 (`e3b0c4...`); in orphan mode skip (empty is never "interesting" content); in full mode emit ONE component for the empty hash with paths-as-property.
- **Files we can't read due to permissions**: log at warn level per Principle X; skip; emit document-level `mikebom:file-inventory-unreadable` count.
- **The same content at 10,000 paths** (e.g., copyright notices duplicated across packages): one component with 10,000-element paths array. Sane upper bound: cap paths array at 100 entries with a `mikebom:file-paths-truncated = true` flag.

## Requirements *(mandatory)*

### Functional Requirements

#### Cross-cutting

- **FR-001**: File-tier components MUST use standards-native CDX `components[].type = "file"` (CDX 1.6 supports this enum value) + SPDX 2.3 `Package` (no native "File" component in 2.3 `Packages` array; emit as `Package` with `filesAnalyzed: false` and a `mikebom:component-tier = "file"` annotation as the parity-bridge) + SPDX 3 — `software_File` is the native element type per SPDX 3.0.1 schema, to be verified at plan time.
- **FR-002**: Every file-tier component MUST carry an explicit `mikebom:component-tier = "file"` annotation regardless of format-specific type field. Disambiguates from package-tier or binary-tier components.
- **FR-002.1 (Principle V v1.4.0 audit citation — consolidated; CORRECTED 2026-06-20 after the spec-correction history documented in US2)**: This milestone introduces **8 new `mikebom:*`-prefixed fields** AND consolidates one existing field (`mikebom:source-files`) with corrected behavior. Per Constitution Principle V's fifth bullet, an audit of each target format's existing native constructs MUST be cited in this Functional Requirements section. Native-construct audit:
  - **`mikebom:component-tier`** (per-component; FR-002, NEW): no native CDX/SPDX field disambiguates "this Package represents a file vs a package vs a binary" — CDX 1.6 `components[].type = "file"` partially fits but doesn't disambiguate binary-tier vs package-tier. **Native fit: partial. Justified parity-bridge.**
  - **`mikebom:source-files`** (per-component; FR-012, **PRE-EXISTING**, defect-fixed in milestone 133): the path the reader identified the package from. **CDX-native `evidence.occurrences[].location` IS the right field for this semantic** AND mikebom populates it richly for OS-package readers (100 % coverage on apk/dpkg/rpm) but NOT for language-ecosystem readers. The `mikebom:source-files` property pre-dates milestone 133 and is retained as a single-value denormalized convenience (matches trivy's `aquasecurity:trivy:FilePath` shape), but the corrected delivery in this milestone is to FIX its three defects (tempdir leak, comma-string vs JSON array, leading-`/`) AND populate the native `evidence.occurrences[]` for the language-ecosystem readers per FR-014. **Native fit: yes (occurrences); `mikebom:source-files` is a denormalized convenience, NOT a parity-bridge for missing native data.**
  - **`mikebom:layer-digest`** (per-component; FR-013, NEW): no CDX / SPDX construct for "OCI layer digest containing this component's source path". **No native fit. Justified parity-bridge.**
  - **`mikebom:file-paths`** (per-component; FR-007, NEW): per-unique-hash paths-as-property for file-tier components. No native CDX / SPDX construct for "this file appears at N paths under one identity". **No native fit. Justified parity-bridge.**
  - **`mikebom:file-paths-truncated`** (per-component; edge case, NEW): boolean flag indicating the paths array was capped at 100 entries. No native CDX / SPDX construct for "this list was truncated". **No native fit. Justified parity-bridge.**
  - **`mikebom:file-inventory-mode`** (document-level; FR-017, NEW): operator-set inventory mode recorded on the SBOM so consumers detect the override at parse time. No native CDX / SPDX construct. **No native fit. Justified parity-bridge.**
  - **`mikebom:file-inventory-skipped-oversize`**, **`mikebom:file-inventory-skipped-special-files`**, **`mikebom:file-inventory-unreadable`** (document-level; edge case skip-counters, NEW): aggregate counts of files the walker skipped. No native CDX / SPDX construct. **No native fit. Justified parity-bridge.**
  Eight new parity-bridging `mikebom:*` annotations + one defect-fixed pre-existing field. Each new annotation lands as a new C-row in `docs/reference/sbom-format-mapping.md` per FR-021 with the audit clause inline.
- **FR-003**: No new Cargo dependencies for US1 + US2. US3 MAY add a Cargo dep IF parallel-hashing is required to keep within SC-004's scan-time budget (current `sha2` workspace dep is sequential).
- **FR-004**: Byte-identity preservation across existing alpha-N package-tier goldens that aren't image-scan-derived. Image-scan goldens MAY churn from US2's path/layer property additions (intentional, declared in SC-005).
- **FR-022** (Measure-first planning obligation, per 2026-06-19 Q2 clarification): `/speckit-plan` is BLOCKED on producing a one-off rootfs-walk projection tool (`cargo run --example orphan-projection -- --image <pinned-digest>` or equivalent) that: (a) walks the milestone-132 pinned audit baseline rootfs AFTER current mikebom emission, (b) applies the FR-005 content-shape allowlist as written, (c) reports the projected orphan-emit count per content-shape category. The measured count drives a TIGHTENED SC-001 range recorded in `research.md §Orphan projection`. If the measured count is outside the original 200-800 band, the FR-005 allowlist is adjusted at plan time and the projection re-run BEFORE production code lands.

#### US1 — Orphan file-tier emission

- **FR-005** (TIGHTENED 2026-06-19 per FR-022 measure-first projection — see `research.md §Orphan projection`): Orphan-mode content-shape allowlist (gate at file-emission time):
  - Unattributed ELF / Mach-O / PE binaries (identified by magic-number probe of first 4 bytes)
  - Unattributed shared libraries (`.so` / `.dylib` / `.dll` by extension OR by file-magic if extension absent)
  - Unattributed archives (`.jar` / `.war` / `.ear` / `.deb` / `.rpm` / `.apk` / `.tar` / `.tgz` / `.zip` not already extracted by mikebom's archive walkers)
  - Lone package manifests **with NO adjacent lockfile**: `package.json` qualifies ONLY when no `package-lock.json` / `yarn.lock` / `pnpm-lock.yaml` exists in the same directory; `Cargo.toml` qualifies ONLY when no `Cargo.lock` exists in the same directory or any parent up to the workspace root; `pom.xml` qualifies ONLY when no `target/` build-output directory exists alongside it. `requirements.txt`, `Gemfile`, `go.mod` apply the same adjacent-lockfile rule.
  - Executable scripts (any file with executable bit AND first 2 bytes = `#!`)
  - **EXPLICITLY EXCLUDED — content shape**: source code (`.rs`, `.py`, `.go`, `.c`, `.cpp`, `.h`, `.cs`, `.java`, `.js`, `.ts`, `.rb`, `.php`, `.swift`, `.kt`), plain text (`.md`, `.txt`, `.rst`), structured config (`.json`, `.yaml`, `.yml`, `.toml`, `.ini`, `.conf`, `.xml` when not a known archive), documentation
  - **EXPLICITLY EXCLUDED — path-prefix list** (per FR-022 projection finding: these are known package install roots where the package-tier reader DOES know the package identity via PURL but hasn't yet been extended to emit `mikebom:component-paths`; the exclusion list is a pragmatic stop-gap until US2 expansion covers those readers):
    - `**/dotnet/packs/**`
    - `**/dotnet/shared/**`
    - `**/dotnet/sdk/**`
    - `**/dotnet/store/**`
    - `**/usr/share/dotnet/**`
    - `**/node_modules/**`
    - `**/lib/python*/site-packages/**`
    - `**/.cargo/registry/**`
    - `**/ruby/gems/**`
    - `**/jvm/openjdk*/lib/**`
- **FR-006**: Per-unique-hash dedupe: identical SHA-256 across multiple paths → ONE component with paths-as-property.
- **FR-007**: `mikebom:file-paths` property MUST carry a JSON-encoded array of relative rootfs paths (no leading `/`); paths sorted lex-ascending for byte-identity determinism.
- **FR-008**: Every file-tier component MUST carry a SHA-256 hash via the standards-native CDX `hashes[]` / SPDX 2.3 `checksums[]` / SPDX 3 `Hash` element.
- **FR-009**: File-tier components MUST NOT carry a PURL. Identity is via `bom-ref` + `hashes[]` + `name` (the basename of the first sorted path).
- **FR-010**: Files larger than 100 MB MUST be skipped from file-tier emission by default; aggregate skip count emitted as document-level `mikebom:file-inventory-skipped-oversize` property. Operator override via `--file-inventory-size-limit <bytes>`.
- **FR-011** (CORRECTED 2026-06-19 per Q3 clarification): Constitution Principle IX accuracy gate — orphan-mode emission MUST skip a file-tier component when EITHER condition holds:
  - **Path coverage** (a): the file's relative path appears in ANY package-tier component's `mikebom:component-paths` property (set by US2's FR-012); OR
  - **Hash coverage** (b): the file's SHA-256 matches the hash field of any binary-tier component (milestone-104 binary-tier readers populate per-file hashes via the existing `evidence.identity[]` mechanism).
  Implementation MUST run BOTH checks AFTER all package-tier + binary-tier readers complete. The original hash-only rule was broken because package-tier readers carry package-identity hashes, not per-file hashes; under the literal rule every package-owned file over-emitted as orphan. The Q3 clarification fixes the contract in-place. Over time, individual package-tier readers MAY expand their `mikebom:component-paths` coverage (e.g. milestone-134 apk `.list` parsing to claim every file the package owns); FR-011's hybrid contract accommodates that expansion without re-spec.

#### US2 — Trivy-style path/layer properties on package-tier

- **FR-012** (CORRECTED 2026-06-20 — original FR-012 invented a new `mikebom:component-path` annotation that would have duplicated the pre-existing `mikebom:source-files` property; this is the corrected obligation): The existing `mikebom:source-files` property emission MUST be fixed for three defects observed on the milestone-132 audit baseline:
  - **Defect A — rootfs/tempdir prefix leak**: the emitted value carries the absolute scanner-host path including the temporary extraction directory (observed: `/private/var/folders/.../mikebom-image-XXXXXX/rootfs/<actual-path>`). The fix is to strip the rootfs prefix at emission time; emit only the path relative to the rootfs root (`<actual-path>`).
  - **Defect B — non-machine-parseable shape**: emitted today as a comma-separated string `"path1, path2"`. The fix is to emit as a JSON-encoded array `["path1", "path2"]` — consumers MUST be able to parse multi-path components reliably.
  - **Defect C — leading `/`**: the relative-to-rootfs paths MUST NOT carry a leading `/` (per the no-leading-`/` convention shared with FR-007).
  All three defects MUST be fixed for OS-package readers (apk/dpkg/rpm) AND for the language-ecosystem readers covered by FR-014. The existing C-row in `docs/reference/sbom-format-mapping.md` for `mikebom:source-files` MUST be updated to document the corrected shape (JSON array; rootfs-relative; no leading `/`).
- **FR-013**: Every package-tier component whose source path was inside an OCI image layer MUST carry `mikebom:layer-digest` property with the SHA-256 layer digest from the docker save manifest. Non-image scans (`--path`) MUST omit this property. The layer-digest determination uses the milestone-130 `scan_fs::docker_image::ExtractedImage` layer-mapping plumbing: each layer's tarball entry list is known at extraction time; map every emitted source path back to the layer containing it.
- **FR-014** (CORRECTED 2026-06-20 — original FR-014 invented a `mikebom:component-paths` plural annotation; the corrected obligation is to use the CDX-native `evidence.occurrences[]` field for language-ecosystem reader path coverage): Language-ecosystem readers (cargo, npm, nuget, maven, pypi, gem, golang — every reader other than apk/dpkg/rpm) emit ZERO `evidence.occurrences[]` entries today, despite mikebom's `ResolvedComponent.evidence` already supporting the field and OS-package readers using it richly (100 % coverage on apk in the milestone-132 audit baseline). The corrected obligation: every language-ecosystem reader MUST populate `evidence.occurrences[].location` with the rootfs-relative path the reader identified the package from. Same path-normalization rules as FR-012 apply (no rootfs/tempdir prefix; no leading `/`). The native CDX 1.6 `evidence.occurrences[].location` field is the carrier; SPDX 2.3 / SPDX 3 emission paths route through the existing `mikebom-cli/src/generate/spdx/evidence` mapping (verify location at task start). **Principle V audit**: native field. No new `mikebom:*` annotation introduced by FR-014; the original "trivy-style `mikebom:component-paths` plural" was a misread of the milestone-132 audit data.

#### US3 — Opt-in full file inventory mode

- **FR-015**: New CLI flag `--file-inventory <off|orphan|full>` on `mikebom sbom scan`. Default value `orphan`. Replaces the implicit "off" of pre-milestone-133 mikebom.
- **FR-016**: Full mode emits per-unique-SHA-256 file-tier component regardless of package coverage. Same per-unique-hash dedupe + paths-as-property shape as FR-006 / FR-007.
- **FR-017**: Full-mode SBOMs MUST carry a document-level `mikebom:file-inventory-mode = "full"` annotation so consumers can detect at parse time that duplicates with package-tier exist.
- **FR-018**: Full mode respects the same FR-010 size limit by default; the same `--file-inventory-size-limit` override applies.

#### US4 — Constitution amendment + reference doc

- **FR-019**: `.specify/memory/constitution.md` amendment lands with: (a) NEW Strict Boundary §5 — "File-tier emission MUST NOT introduce duplicate components in default mode. The `--file-inventory=full` flag is an explicit override; full-mode SBOMs MUST carry a document-level `mikebom:file-inventory-mode` annotation so consumers can detect the override at parse time"; (b) §VIII Completeness clarification — "Unattributed content — files surviving all package-DB, binary-tier, and fingerprint readers — counts toward Completeness when surfaced as file-tier components per the orphan-fallback contract"; (c) MINOR version bump per existing convention (1.4.0 → 1.5.0); (d) SYNC IMPACT REPORT block at the doc head per existing convention.
- **FR-020**: New `docs/reference/component-tiers.md` reference doc covering: three tiers (package, binary, file) and how they compose; orphan content-shape allowlist + the explicit excluded extensions; full-mode behavior + the `mikebom:file-inventory-mode` annotation contract; precedence rules; cite trivy's path/layer-as-property approach as informing US2's design (give credit; document the design decision against syft's per-(path,hash) duplication).
- **FR-021** (CORRECTED 2026-06-20 — dropped `mikebom:component-path` and `mikebom:component-paths` per FR-002.1 spec correction): New C-rows in `docs/reference/sbom-format-mapping.md` for every NEW annotation introduced by this milestone: `mikebom:component-tier`, `mikebom:layer-digest`, `mikebom:file-paths`, `mikebom:file-inventory-mode`, and the edge-case skip-counter annotations (`mikebom:file-inventory-skipped-oversize`, `mikebom:file-inventory-skipped-special-files`, `mikebom:file-inventory-unreadable`, `mikebom:file-paths-truncated`). Each row carries a Principle V v1.4.0 audit clause inline. **Existing C-row update**: the pre-existing C-row for `mikebom:source-files` (verify number at implementation time; greppable in `docs/reference/sbom-format-mapping.md`) MUST be updated to document the corrected shape per FR-012 — JSON-encoded array, rootfs-relative, no leading `/`.

### Key Entities

- **File-tier component**: a CDX `component` (type `"file"`) or SPDX Package/File representing content that exists in the scan target rootfs and does NOT correspond to a known package or fingerprint-identified binary. Identity is its SHA-256.
- **Orphan content-shape allowlist**: the FR-005 enumeration of file shapes that qualify for default-mode file-tier emission. The exclusion list (source code / docs / configs) is part of the contract.
- **Path/layer context property pair**: `mikebom:component-path` + `mikebom:layer-digest` (image scans) OR `mikebom:component-path` only (non-image scans). Trivy-inspired additive metadata on package-tier components.
- **File-inventory mode**: enum (`off` | `orphan` | `full`); operator-set via `--file-inventory`; documented in emitted SBOM via `mikebom:file-inventory-mode` annotation.
- **Per-unique-hash component**: file-tier components are keyed by SHA-256; multiple rootfs paths with identical content collapse to ONE component with paths-as-property.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the milestone-132 pinned audit baseline (`@sha256:4e7b…`) scanned in default mode (`--file-inventory=orphan`, the new default), orphan-mode file-tier emission adds a count of new components in the range **180-440** (target ~245 per the `research.md §Orphan projection` post-allowlist-tightening estimate; ±10 % band per FR-022 contract). The original 200-800 budget held; the projection tightened the range based on actual rootfs walk. Implementation re-runs the projection tool from `/tmp/mb-133-projection/project.sh` to confirm, OR a future improved projection that incorporates the FR-005 path-prefix exclusion list.
- **SC-002**: Orphan-mode emission MUST add ZERO components whose hash matches an existing package-tier or binary-tier component. Verified by post-scan SHA-256 set intersection check.
- **SC-003**: Full mode (`--file-inventory=full`) lifts the sbom-comparison Completeness score from 1★ to **≥4★** (Completeness's `coverageStarsPct` band keys on unique-component count; ≥80% of syft's per-(path,hash) count maps to 4★).
- **SC-004**: Scan-time growth on the pinned audit baseline (offline-vs-offline measurement to isolate from deps.dev): **orphan mode <50%** growth, **full mode <300%** growth — both relative to the milestone-132 post-MVP baseline. Full-mode upper bound is liberal because hashing the entire rootfs is fundamentally expensive; we just want it to finish within human-tolerable time on the audit image.
- **SC-005**: Byte-identity preserved across existing alpha-N package-tier goldens for non-image-scan fixtures. Image-scan goldens MAY require regen from US2 path/layer property additions — that churn is intentional, documented, and is the only acceptable golden churn for milestone 133.
- **SC-006**: Constitution amendment lands a single MINOR version bump (1.4.0 → 1.5.0) with the SYNC IMPACT REPORT block at the head. Reference doc `docs/reference/component-tiers.md` contains ≥1 worked example per tier per format (CDX / SPDX 2.3 / SPDX 3).
- **SC-007**: Full-mode and orphan-mode produce a measurable shape difference: on the audit baseline, full-mode component count is **≥10× orphan-mode component count**. Confirms the two modes meaningfully differ; smaller ratios suggest the orphan content-shape allowlist is too permissive (close to full mode) OR the full-mode emission is broken (close to orphan mode).

## Assumptions

- **Hash algorithm**: SHA-256 is the canonical file-tier hash. Matches existing `scan_fs/binary/` hash convention. SBOM-format-mapping CDX `hashes[].alg = "SHA-256"` / SPDX `checksums[].algorithm = "SHA256"` / SPDX 3 `Hash.algorithm = "sha256"`.
- **Pinned audit baseline**: same as milestone 132's pin (`767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner@sha256:4e7b05811ce4885d8a7183819b4e0e209662784fe24b7553ceea3d149e3c719c`). All SC measurements run against this digest.
- **Layer-digest mapping**: image scans use the existing milestone-130 `docker save` extraction; `mikebom:layer-digest` reads from the per-layer manifest entries. Multi-arch images: use the architecture mikebom was already scanning (consistent with existing behavior).
- **Existing CLI surface**: `--offline`, `--no-deps-dev`, `--enrich-sources` are unchanged. `--file-inventory` is a NEW flag in a previously-unused namespace; no conflicts.
- **Plan-correction discipline**: every "this code exists at X" claim in this spec MUST be verified by reading the actual source file at planning time. Every CLI-flag claim MUST cite `mikebom sbom scan --help` output. Milestone 132 had THREE in-place plan corrections from fabricated claims (`LICENSE_FINGERPRINT_TABLE`, `--enrich-licenses=depsdev` flag, "cargo arm" in `enrich/depsdev_source.rs`). This milestone explicitly opts into the same discipline.

## Out of Scope

- **Source code emission**: the "every `.rs` file becomes a component" trap — explicitly excluded by FR-005's content-shape allowlist + exclusion list.
- **Per-(path, hash) duplication** (syft's model): we chose per-unique-hash with paths-as-property. Operators wanting the syft shape can use syft.
- **File ownership / permissions / mode**: mikebom is not an image-forensics tool. We don't emit `chmod` bits or owner UIDs.
- **Per-file dependency-graph edges**: file-tier components are NOT linked into the `dependencies[]` graph. They're standalone catalog entries.
- **Patching sbom-comparison's `normVersion` to read milestone-132's stripped Informational annotation**: separate concern, separate milestone if pursued.
- **VERSION_MISMATCH `<50`** (milestone-132 SC-002 deferred item): out of milestone 133 scope; characterized for milestone 134+.
- **Audit-corpus expansion** (additional test images beyond remediation-planner): out of milestone 133 scope; tracked separately.
- **eBPF trace-path file-tier emission**: out of scope. This milestone modifies the `scan_fs` filesystem-scan path only. Trace-path discovery rules (Constitution Principle II) are unchanged.

## Dependencies

- **Existing milestone-001 `scan_fs` walker** (re-used to enumerate rootfs paths after package-DB + binary-tier readers run).
- **Existing milestone-104 binary-tier component classification** (the "package or binary" check that determines orphan-ness).
- **Existing milestone-130 OCI layer extraction** (`scan_fs/docker_image.rs` — for FR-013 layer-digest mapping).
- **Existing `sha2` workspace dep** (file hashing; sequential is fine for SC-004's orphan-mode budget; full mode may parallelize via existing `tokio` or `rayon`-style approach — decided at plan time).
- **sbom-comparison harness** at `/Users/mlieberman/Projects/sbom-comparison/sbom-comparison` for SC-003 / SC-007 verification (re-used from milestone 132).
- **Pinned audit baseline** `@sha256:4e7b…` (carried forward from milestone 132).
- **Trivy CDX output** captured during the design conversation at `/tmp/trivy-rp-132.cdx.json` — reference shape for FR-012 / FR-013 path/layer properties.

## Honest accounting clauses

This milestone's spec was written immediately after milestone 132 closed with **three documented in-place plan corrections** for fabricated claims (PR #382's `LICENSE_FINGERPRINT_TABLE`, PR #383's `--enrich-licenses=depsdev` flag, PR #383's "cargo arm" in `enrich/depsdev_source.rs`). To prevent recurrence:

1. **Source verification at plan time, not implementation time**: every claim in `data-model.md` / `tasks.md` of the form "this code exists at X" or "this CLI flag is Y" MUST be verified by reading the actual file or running `--help` BEFORE the spec section is written. The verification step is itself a planning task.
2. **Plan-correction PRs are still allowed** if fabrications slip through. They land as part of the implementation PR with an explicit "the spec said X; reality is Y; correcting in-place" callout in the PR description, matching the milestone-132 #382 / #383 pattern.
3. **No silent rewrites of the spec record**. If reality differs from the spec, surface it explicitly so future contributors see both the original mistake and the correction. The course-correction trail is the durability.
