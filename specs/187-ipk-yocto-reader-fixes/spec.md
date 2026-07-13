# Feature Specification: ipk reader — modern ar-format extraction + filename-fallback arch fix

**Feature Branch**: `187-ipk-yocto-reader-fixes`
**Created**: 2026-07-12
**Status**: Draft
**Input**: User description: "Fix ipk reader misclassifying modern Yocto opkg-build ipks as legacy ar-format + filename-fallback arch-parsing regression for underscore-in-arch names. Two coupled follow-ups to milestone 185: (a) #543 — every Yocto-emitted ipk is currently classified as legacy ar-format and routed through the filename-only fallback, so License / Depends / Section / Maintainer / Homepage / Recommends metadata is never extracted; (b) #542 — the filename fallback path's rsplit-based arch parser regresses for arches containing `_` (`qemux86_64`, `powerpc_e500v2`, `mips_i6400`), emitting `?arch=64` + a corrupted `version` string with `_qemux86` glued on."

## Clarifications

### Session 2026-07-12

- Q: How does mikebom decide the parent-directory name is authoritative for `?arch=` in the filename-fallback path? → A: Parent-dir wins IFF the filename (with `.ipk` stripped) ends with `_<parent-dir-name>` — i.e., filename and parent-dir agree on the arch. This matches the Yocto convention exactly (e.g., `tmp/deploy/ipk/qemux86_64/foo_1.0-r0_qemux86_64.ipk`) and unambiguously identifies parent-dir-as-arch without misfiring on loose-file scans (`~/downloads/foo_1.0_all.ipk` where the parent is `downloads/` will NOT match because the filename ends `_all.ipk`, not `_downloads.ipk`).

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Extract control-file metadata from modern Yocto ipks (Priority: P1)

An operator scanning a Yocto build output (`tmp/deploy/ipk/<arch>/*.ipk`) or an on-target rootfs with an opkg installation wants mikebom to extract the full ipk control-file metadata — `License:`, `Depends:`, `Recommends:`, `Section:`, `Priority:`, `Maintainer:`, `Homepage:`, `Architecture:`, `Description:` — for every emitted opkg component. This is the metadata mikebom already extracts cleanly from `.rpm` and `.deb` files; the fact that ipk lags is a Yocto-specific gap that blocks SBOM consumers from making license, dependency, and provenance decisions on embedded-Linux images.

**Why this priority**: This is a data-fidelity regression: 100% of Yocto-emitted ipk components currently ship with NOASSERTION license, empty dependency edges, and no supplier metadata. It renders m185's license-extraction fix a no-op for the primary target ecosystem (Yocto). The paired filing (#543) reports 4587 components in a `core-image-minimal` test set, all missing the metadata. Fixing this unblocks Yocto SBOM consumers immediately.

**Independent Test**: Point mikebom at any Yocto-emitted `.ipk` file (e.g., `tmp/deploy/ipk/core2-64/busybox_1.36.1-r0_core2-64.ipk`) via `mikebom sbom scan --path <dir>` and confirm (a) the emitted component's `licenses[]` contains the value from the ipk's control file (e.g., `GPL-2.0-only & bzip2-1.0.4`), (b) the component's declared `Depends:` entries surface as edges in the resolved graph (`Recommends:` handling deferred per FR-006), (c) the `mikebom:source-mechanism` property reports something other than `ipk-file-filename-fallback`, indicating the archive-extraction path was taken.

**Acceptance Scenarios**:

1. **Given** a modern opkg-build ipk (`debian-binary` + `control.tar.gz` + `data.tar.gz` packed as an ar archive), **When** mikebom scans it, **Then** the emitted component MUST contain the `License:` value from the ipk's control file (normalized via the milestone-153/185 spdx-canonicalization path), the `Architecture:` value emitted as the PURL `?arch=` qualifier, and every `Depends:` entry surfaced as a dependency edge. (`Recommends:` handling deferred per FR-006.)
2. **Given** the same ipk, **When** mikebom emits the component's `properties[]`, **Then** the `mikebom:source-mechanism` property MUST report a value distinct from `ipk-file-filename-fallback` — indicating the archive-extraction path was taken (suggested value: `ipk-file-archive-extraction` for consistency with existing `ipk-file-*` mechanism naming).
3. **Given** a pre-2015 opkg-build ipk (`gzip( tar )` outer envelope directly wrapping `debian-binary` + `control.tar.gz` + `data.tar.gz`) — the OLD format — **When** mikebom scans it, **Then** mikebom MUST STILL parse it correctly via the existing `gzip(tar)` outer path. Backward compatibility is preserved for the legacy shape.
4. **Given** an ipk whose ar-archive body is malformed (truncated, missing `debian-binary`, or missing `control.tar.gz`), **When** mikebom scans it, **Then** mikebom MUST fall through to the filename-fallback path with a WARN log naming the specific structural failure (e.g., `ar-format: missing control.tar.gz member`, not the generic "legacy ar-format" language).
5. **Given** a Yocto build output containing a stock `busybox` ipk with `License: GPL-2.0-only & bzip2-1.0.4` in its control file, **When** mikebom emits the SBOM, **Then** the busybox component's `licenses[]` MUST contain both `GPL-2.0-only` and `bzip2-1.0.4` (post-canonicalization; the exact wire form matches milestone-152/185's operand-preservation rules).

---

### User Story 2 — Correct arch parsing when the arch name contains underscores (Priority: P1)

An operator scanning a Yocto build whose target uses an arch name containing `_` (e.g., `qemux86_64`, `powerpc_e500v2`, `mips_i6400`) wants mikebom to emit correct `?arch=` PURL qualifiers AND correct `version` strings for those packages — NOT the current `?arch=64` + `version=...+_qemux86` regression. The correct arch is already discoverable from either (a) the ipk's parent directory name (Yocto's `tmp/deploy/ipk/<arch>/` layout) or (b) the ipk control file's `Architecture:` field (once US1 is delivered).

**Why this priority**: 552 / 4587 components (12%) of the Yocto test set are affected. The corrupt `version` and `?arch=` values render vulnerability matching, PURL-based database lookups, and downstream SBOM consumers unable to resolve these components correctly. The fix is small (consult the parent-dir name in the filename-fallback path) and defensive against future arch-naming conventions that use `_`.

**Independent Test**: Scan any Yocto ipk whose parent directory contains `_` in its name — e.g., `tmp/deploy/ipk/qemux86_64/kernel-6.6.127-yocto-standard_6.6.127+git0+45f69741c7_70af2998be-r0_qemux86_64.ipk`. Invoke `mikebom sbom scan --path <dir> --format cyclonedx-json --output out.cdx.json`. Verify (a) the emitted PURL is `pkg:opkg/kernel-6.6.127-yocto-standard@6.6.127%2Bgit0%2B45f69741c7_70af2998be-r0?arch=qemux86_64` (correct arch, no gluing), (b) the `version` field is `6.6.127+git0+45f69741c7_70af2998be-r0` (no `_qemux86` glued on), (c) NO component in the emitted SBOM has `?arch=64`.

**Acceptance Scenarios**:

1. **Given** an ipk file located at `<any-parent>/qemux86_64/<file>.ipk` (Yocto layout), **When** mikebom emits the component via the filename-fallback path (US1 did not enter the archive path — genuinely legacy or malformed), **Then** the emitted PURL's `?arch=` qualifier MUST be `qemux86_64` (from the parent directory name), NOT `64` (from the filename's last `_`-delimited segment).
2. **Given** the same ipk in the same location, **When** mikebom parses the filename to extract `name`/`version`, **Then** the `version` field MUST NOT contain any part of the arch name — e.g., NOT `..._qemux86`.
3. **Given** an ipk file whose parent directory name contains multiple underscores (`powerpc_e500v2`, `mips_i6400`), **When** mikebom emits the component, **Then** the emitted `?arch=` qualifier MUST be the full parent directory name verbatim, and the `version` MUST be preserved verbatim from the filename minus arch minus name.
4. **Given** an ipk located at a path WITHOUT the Yocto convention (e.g., loose in a directory or in `/var/lib/opkg/info/`), **When** mikebom parses the filename via the fallback path, **Then** mikebom MUST use the existing rsplit-based heuristic AND emit a diagnostic property `mikebom:arch-source = "filename-heuristic"` so operators can see the confidence level, versus `mikebom:arch-source = "parent-directory"` when the Yocto convention is present.
5. **Given** an ipk whose control file was successfully extracted via US1's archive-extraction path, **When** mikebom emits the component, **Then** the `?arch=` qualifier MUST come from the control file's `Architecture:` field (US1 authoritative source), NOT the parent-directory or filename-heuristic fallbacks — regardless of whether the parent directory name matches.

---

### Edge Cases

- **ar-archive member ordering**: the canonical opkg-build order is `debian-binary`, `control.tar.gz`, `data.tar.gz`. Some builds may emit `control.tar.gz` before `debian-binary` OR omit `debian-binary` entirely. mikebom MUST accept any order as long as `control.tar.gz` is present. Missing `debian-binary` MUST NOT be treated as a hard failure (a WARN log flags the anomaly but extraction proceeds if `control.tar.gz` is present).
- **`debian-binary` content mismatch**: the standard content is `2.0\n`. Some non-conforming builds emit `1.0\n` or an empty string. mikebom MUST parse the value defensively — accept anything, log a WARN if the value is not `2.0\n` — and still proceed with control-file extraction. The value is diagnostic, not gating.
- **`control.tar` (uncompressed) vs `control.tar.gz`**: some ar-format ipks contain `control.tar` (uncompressed tar without gzip). Same for `data.tar`. mikebom MUST accept both suffixes. `.tar.zst` is out of scope for this milestone (defer to a follow-up).
- **Malformed inner tar**: if `control.tar.gz` extracts but the tar inside is truncated / missing the `control` file itself, mikebom MUST fall through to the filename fallback with a WARN reporting `control file missing from control.tar.gz`.
- **Both formats present in one scan**: a scan that walks a directory containing both modern-ar-format AND pre-2015-`gzip(tar)`-format ipks MUST process each correctly according to its detected shape. The dispatch is per-file, not per-scan.
- **Fallback path arch source disambiguation** (see FR-010): mikebom uses the parent-directory name as authoritative IFF the filename (with `.ipk` stripped) ends with `_<parent-dir-name>` — i.e., the filename's last `_`-delimited segment equals the parent-dir name byte-for-byte. This correctly identifies parent-dir-as-arch for every Yocto layout (`ipk/qemux86_64/foo_1.0-r0_qemux86_64.ipk`, `ipk/all/foo_1.0_all.ipk`, `ipk/core2-64/foo_1.0_core2-64.ipk`) AND correctly falls back for loose-file layouts (`~/downloads/foo_1.0_all.ipk` — parent `downloads` ≠ filename suffix `_all` → filename-heuristic wins). Random dirs holding ipks with unrelated arch suffixes never misfire.
- **Nested Yocto layout**: some builds wrap the arch dir under an additional layer like `tmp/deploy/ipk/machine-x/qemux86_64/`. mikebom MUST use the IMMEDIATE parent directory as the arch source — not the grand-parent.
- **`?arch=` qualifier normalization**: when the arch source is the parent directory or control file's `Architecture:` field, the value MUST be URL-encoded per the PURL spec (matches the existing behavior for other qualifiers). Underscores + hyphens + dots are all valid PURL qualifier characters without escaping.
- **License normalization**: the `License:` field from the control file MUST flow through the same spdx-canonicalization path as m152/m185's rpm license fixes — preserving operands, applying the LicenseRef normalization, and emitting the canonical wire form.
- **`Depends:` handling**: `Depends:` produces a hard dependency edge (mirrors the existing `parse_depends_field_with_alternatives` behavior at `ipk_file.rs:449`, unchanged from pre-m187). `Recommends:` handling is out of scope — see FR-006 + §Deferred.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: mikebom's ipk reader MUST recognize the modern ar-format (magic bytes `!<arch>\n` at offset 0, canonical opkg-build layout since ~2015) as the PRIMARY parse path — not the current "legacy fallback". The `gzip(tar)` outer path remains supported for backward compatibility with pre-2015 ipks but MUST be a secondary code path selected after the ar-format probe fails.
- **FR-002**: mikebom's ar-format parser MUST extract the `control.tar.gz` (or `control.tar`) member from the ar archive AND the `data.tar.gz` (or `data.tar`) member. The `debian-binary` member is optional (a WARN log flags absence but extraction proceeds).
- **FR-003**: mikebom MUST extract the `control` file from `control.tar[.gz]` and parse every RFC 822-style field: `Package:`, `Version:`, `Architecture:`, `License:`, `Depends:`, `Recommends:`, `Section:`, `Priority:`, `Maintainer:`, `Homepage:`, `Description:`. Unknown fields MUST be ignored (forward-compat).
- **FR-004**: The parsed `License:` value MUST flow through the same spdx-canonicalization path (`SpdxExpression::try_canonical` + LicenseRef normalization) that milestone 152 + 185 established for the rpm reader. Byte-identity preserved for canonical inputs; non-canonical inputs surface via the existing wholesale-wrap convention.
- **FR-005**: The parsed `Architecture:` value MUST become the PURL `?arch=` qualifier, taking precedence over both the parent-directory and filename-heuristic fallback sources. The value MUST be URL-encoded per the PURL spec.
- **FR-006**: The parsed `Depends:` field MUST produce dependency edges via the existing resolution pipeline (same as .deb's `Depends:` processing). `Recommends:` handling (soft/optional edges with `mikebom:lifecycle-scope` metadata) is DEFERRED to a follow-up milestone — no existing reader (`ipk_file.rs`, `opkg.rs`, `dpkg.rs`, `control_file.rs`) currently parses `Recommends:`, so adding it to m187 would creep beyond a targeted bug fix into new-feature territory.
- **FR-007**: mikebom MUST emit the `mikebom:source-mechanism = "ipk-file-archive-extraction"` property (or a documented equivalent) when the archive-extraction path succeeds — distinct from `ipk-file-filename-fallback`. Operators can distinguish extraction-path components from fallback-path components at parse time.
- **FR-008**: The pre-2015 `gzip(tar)` outer path MUST remain functional for backward compatibility — mikebom detects the outer envelope shape (gzip magic bytes `0x1F 0x8B` vs ar magic bytes `!<arch>\n`) and dispatches to the appropriate parser.
- **FR-009**: When neither the ar-format nor the `gzip(tar)` outer path succeeds (both fail with structural errors), mikebom MUST fall through to the filename-fallback path per US2 with a WARN log naming the specific structural failure (NOT the current generic "legacy ar-format" message, which is now inaccurate).
- **FR-010**: In the filename-fallback path, mikebom MUST use the ipk file's IMMEDIATE parent directory name as the authoritative `?arch=` source IFF the filename (with `.ipk` stripped) ends with `_<parent-dir-name>` — i.e., the filename's last `_`-delimited segment matches the parent-dir name exactly (byte-for-byte, case-sensitive). This gate correctly identifies the Yocto convention (`tmp/deploy/ipk/qemux86_64/foo_1.0-r0_qemux86_64.ipk` → parent `qemux86_64` matches filename suffix `_qemux86_64` → parent-dir wins) AND correctly falls back for loose-file layouts (`~/downloads/foo_1.0_all.ipk` → parent `downloads` does NOT match filename suffix `_all` → filename-heuristic wins).
- **FR-011**: When the parent-directory arch source is used per FR-010, mikebom MUST strip the exact suffix `_<parent-dir-name>` from the pre-`.ipk` filename portion and preserve everything before it verbatim in the parsed `name` + `version` fields. Specifically, if the parent directory is `qemux86_64` and the filename is `kernel-6.6.127-yocto-standard_6.6.127+git0+45f69741c7_70af2998be-r0_qemux86_64.ipk`, mikebom strips the `_qemux86_64.ipk` suffix, then applies the existing `<name>_<version>` split on what remains — yielding `name = kernel-6.6.127-yocto-standard`, `version = 6.6.127+git0+45f69741c7_70af2998be-r0`. The internal `_` inside the version is preserved unmolested because arch extraction is no longer competing for it.
- **FR-012**: When FR-010's suffix-match condition does NOT hold (filename doesn't end with `_<parent-dir-name>` — e.g., loose in a directory or in a non-Yocto layout), mikebom MUST use the existing rsplit-based heuristic AND emit a diagnostic property `mikebom:arch-source = "filename-heuristic"` for observability. The property helps operators identify low-confidence arch emissions.
- **FR-013**: When the parent directory IS used as the arch source, mikebom MUST emit `mikebom:arch-source = "parent-directory"` on the component. When the control-file `Architecture:` field is used (US1 succeeded), emit `mikebom:arch-source = "control-file"`.
- **FR-014**: Every existing `ipk_file.rs` unit test + integration test + regression test MUST continue to pass byte-identically — this milestone extends the reader; it does not change existing behavior for well-formed pre-2015 ipks OR for the residual filename-fallback path.
- **FR-015**: mikebom MUST NOT introduce any new production Cargo dependency. The ar-format parser is trivial (60-byte fixed-size headers per member; big-endian ASCII digits for sizes; text-based member names) — hand-rolled in `~150-200 lines` of pure Rust. The existing `tar` crate + `flate2` crate cover the inner tar / gzip extraction.
- **FR-016**: The `LegacyArFormat` error variant in `IpkParseError` MUST be renamed (or replaced) to reflect its new meaning — under FR-001 the ar-format IS the modern format, so the current variant name is actively misleading. The old variant name may be retained as a deprecated alias for one milestone if downstream test suites depend on it, or removed entirely.

### Key Entities

- **ipk file** — a package artifact matching one of two shapes: modern (ar container: `debian-binary` + `control.tar.gz` + `data.tar.gz`) or legacy (`gzip(tar)` outer with the same three members inside).
- **control file** — an RFC 822-style plain-text file inside `control.tar.gz`. Fields: `Package:`, `Version:`, `Architecture:`, `License:`, `Depends:`, `Recommends:`, `Section:`, `Priority:`, `Maintainer:`, `Homepage:`, `Description:`. Field values may span multiple lines with leading whitespace continuation.
- **arch source** — the origin of the `?arch=` PURL qualifier, one of three: `control-file` (US1 authoritative), `parent-directory` (US2 primary), `filename-heuristic` (US2 fallback of last resort). Emitted as `mikebom:arch-source` property on the component.
- **filename-fallback path** — the code path invoked when both the ar-format and `gzip(tar)` parsers fail structurally. Extracts `name` / `version` / `arch` from the filename's `_`-delimited segments, consulting the parent directory for arch authoritative-ness.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a stock Yocto `core-image-minimal` output (e.g., scarthgap release, `qemux86-64` machine), scanning the `tmp/deploy/ipk/` tree MUST produce components with `licenses[]` populated for at least 95% of packages that have a `License:` field in their control file. The pre-m187 baseline is 0% (0/4587 components).
- **SC-002**: The same scan MUST produce dependency edges for at least 90% of packages whose control file declares a `Depends:` field. Pre-m187 baseline is 0%.
- **SC-003**: The `?arch=` qualifier on emitted PURLs MUST be correct (matching the arch value in the control file or parent directory name) for 100% of components. Specifically, `?arch=64` MUST NOT appear on any component whose arch source is a Yocto multi-underscore name like `qemux86_64`.
- **SC-004**: Every `mikebom:source-mechanism = "ipk-file-archive-extraction"` component MUST also carry a non-empty `licenses[]` array (or an explicit `NOASSERTION` when the control file's `License:` field is truly empty). Zero components with the extraction mechanism AND empty licenses AND non-empty control-file `License:` value.
- **SC-005**: The pre-2015 `gzip(tar)`-outer path MUST continue to parse existing regression fixtures byte-identically. Every existing ipk-related unit and integration test in `mikebom-cli/` MUST pass with zero drift.
- **SC-006**: The filename-fallback path (US2) MUST correctly resolve `?arch=qemux86_64` for at least 100% of Yocto qemux86_64 ipks located under a `qemux86_64/` parent directory — even when the archive-extraction path fails. Verified via a targeted integration test that fabricates a malformed ar-body but preserves the parent-directory layout.
- **SC-007**: Zero new production Cargo dependencies added — `cargo tree --workspace | wc -l` MUST be identical pre- vs post-m187.

## Assumptions

- The ar container format used by opkg-build since ~2015 is the standard Unix ar format (`!<arch>\n` magic, 60-byte fixed-size headers per member) — the same format used by `.deb` files, static libraries, and every Unix `ar(1)` tool since the 1970s. mikebom's parser can be trivially hand-rolled in ~150-200 lines of pure Rust (Constitution Principle I / SC-007 zero-new-deps posture).
- The `debian-binary` member is optional — some opkg-build variants omit it. mikebom accepts the anomaly with a WARN log but does not gate on it.
- The `control` file inside `control.tar.gz` uses RFC 822 field syntax. Multi-line values with leading whitespace continuation are handled the same way as the existing `.deb` control-file parser (milestone 003-era code).
- License-string normalization for the extracted `License:` field reuses the m152/m185 rpm normalization pipeline — no new normalization logic. The wholesale-wrap fallback for unparseable expressions applies verbatim.
- Dependency edge parsing for `Depends:` reuses the existing comma-separated + version-constraint-embedded shape (`libc6 (>= 2.39+git0+ce65d944e3), update-alternatives-opkg`). The existing `parse_depends_field_with_alternatives` helper at `ipk_file.rs:449` (m185-era) is the reference. `Recommends:` deferred per FR-006.
- The `mikebom:arch-source` diagnostic property is a new property. Its introduction follows Constitution Principle V (native-first): CDX / SPDX 2.3 / SPDX 3 have no native construct for "which layer supplied this qualifier", so a `mikebom:*` property is the only option. Documented in `docs/reference/sbom-format-mapping.md` alongside the existing `mikebom:source-mechanism` documentation.
- The `mikebom:source-mechanism = "ipk-file-archive-extraction"` value is a new source-mechanism string. Existing values (`ipk-file-filename-fallback`, `ipk-file-tar-outer-envelope`) are preserved unchanged. This mirrors m105's source-mechanism convention: one string per code path, distinguishable in emitted output.
- The `LegacyArFormat` error variant in `IpkParseError` may be renamed to `LegacyGzipTarFallbackFailed` (or similar) to accurately reflect that ar-format is now the primary path, not the fallback. Alternatively, the variant may be dropped and its old callers routed to a more specific error class.

## Constitution Alignment

- **Principle I (Pure Rust, Zero C)**: SC-007 verified. ar-format parser is hand-rolled pure Rust; no new Cargo deps.
- **Principle III (Fail Closed)**: FR-008 + FR-009 preserve the multi-tier detection with each failure surfacing a specific WARN. Zero silent drops.
- **Principle IV (Type-Driven Correctness)**: `IpkParseError` variants distinguish structural failure classes (ar-malformed vs gzip-tar-malformed vs control-file-missing) so the dispatch logic is compile-time-typed.
- **Principle V (Specification Compliance + Native-first)**: `License:` field flows through spdx-canonicalization (native SPDX field). `?arch=` uses PURL-standard qualifier syntax. Two new `mikebom:*` properties (`mikebom:source-mechanism = "ipk-file-archive-extraction"`, `mikebom:arch-source`) — documented per Principle V + Assumption above; no native construct exists for either concept.
- **Principle VIII (Completeness)**: Extraction of `License:` / `Depends:` / `Recommends:` / `Section:` / `Priority:` / `Maintainer:` / `Homepage:` closes a completeness gap identified in #543. Reduces false negatives on Yocto scans by ~100 percentage points on those fields.
- **Principle IX (Accuracy)**: Correct `?arch=` values (US2) reduce false positives / phantom PURLs on Yocto qemux86_64 scans by 12 percentage points (552 / 4587 components).
- **Principle X (Transparency)**: `mikebom:source-mechanism` + `mikebom:arch-source` properties give SBOM consumers explicit signals about which code path emitted the data.

## Deferred to Future Milestones

- **`.tar.zst`-compressed inner archives** — some experimental opkg builds emit `control.tar.zst` + `data.tar.zst`. Not addressed in m187; treated as a structural failure until zstd inner support lands.
- **ar-format archives with non-standard member ordering** — m187 requires the standard order (debian-binary → control.tar.gz → data.tar.gz). Non-standard orderings fall through with a WARN.
- **PGP-signed ipks** — some registries emit `.ipk.sig` sidecars or embed signatures. m187 emits the component without signature verification.
- **Yocto-specific arch names from arbitrary machine configs** — the FR-010 arch-name regex (`^[a-z0-9_.-]+$`) is deliberately permissive. If a machine config uses an arch name matching `[A-Z]` or `~` or other unusual characters, the check may need widening in a follow-up.
- **Rename of `LegacyArFormat` variant** — the enum-variant rename is scope-flexible; if the rename introduces a wider blast radius than expected (e.g., public API surface used by downstream consumers), it may be deferred with the old variant retained as a deprecated alias.
- **`Recommends:` field parsing** — mikebom's ipk / opkg / dpkg readers currently do NOT extract `Recommends:` fields from control files. Adding soft/optional edge emission (with `mikebom:lifecycle-scope` metadata per m183's convention) requires changes to `control_file.rs::parse_stanzas` + `resolution.rs` + edge-emission plumbing across three reader modules — beyond m187's bug-fix scope. Follow-up milestone should cover all three package-manager readers together for consistency.
