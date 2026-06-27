# Feature Specification: RPM reader — fix double-`rpm` PURL namespace + raise size cap

**Feature Branch**: `144-rpm-purl-size-fixes`
**Created**: 2026-06-26
**Status**: Draft
**Input**: User description: "Fix RPM reader: drop literal 'rpm' namespace from PURLs (use distro from /etc/os-release or empty), raise default size cap from current value to 512 MB, decouple size-cap warning from malformed warning, and add operator-facing knobs (--rpm-distro override, --max-rpm-bytes override). Surfaced by the yocto-test testbed against core-image-minimal."

## Origin

Two issues surfaced by the external `yocto-test` testbed comparing `mikebom` against Yocto's native SPDX output for `core-image-minimal` on qemux86-64 (scarthgap LTS):

- **D-01** — `mikebom sbom scan` of `tmp/deploy/rpm/` emits `pkg:rpm/rpm/<name>@<ver>?arch=...` (the literal string `rpm` as both PURL `type` and `namespace`) for all 4584 RPM components. Per purl-spec, the namespace should be the distribution (e.g., `fedora`) or empty — never the literal `rpm`.
- **D-02** — Three well-formed Yocto debug RPMs (`kernel-dbg` ~279 MB, `openssl-ptest` ~260 MB, `gcc-dbg` ~378 MB) are skipped with warning `WARN skipping malformed .rpm file ... reason="size-cap-exceeded"` because the current default per-file cap is 200 MB. The warning misleads operators into thinking the files are corrupt when they are merely larger than the default cap.

Both fixes touch `mikebom-cli/src/scan_fs/package_db/rpm_file.rs` (and, for size-cap consistency, `rpm.rs` for the rpmdb path).

## Clarifications

### Session 2026-06-26

- Q: When a `.rpm` file has non-empty RPMTAG_VENDOR/RPMTAG_PACKAGER, `--rpm-distro` is set, AND `/etc/os-release` exists at scan root — which source wins for the PURL namespace? → A: CLI override is authoritative across all sources. Strict priority: `--rpm-distro` > `/etc/os-release ID=` > per-RPM RPMTAG_VENDOR / RPMTAG_PACKAGER > empty. Both CLI and `/etc/os-release` override per-RPM header metadata (a deliberate behavior change for the standalone-`.rpm` reader; the rpmdb-path reader is out of scope per its Out-of-Scope entry).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - RPM PURLs no longer contain literal `rpm` namespace (Priority: P1)

A security engineer scans a directory of `.rpm` files (Yocto build output, a yum repo dump, or any other RPM corpus that lacks `RPMTAG_VENDOR`/`RPMTAG_PACKAGER` distro hints). Today every PURL in the resulting SBOM has the shape `pkg:rpm/rpm/<name>@<ver>?arch=...`. The double-`rpm` confuses downstream vulnerability scanners and PURL-equality tools because it does not match canonical PURLs published by other tools or upstream advisories. After this milestone, the namespace segment is either the detected distro identifier (from `/etc/os-release` at the scan root, when present) or omitted entirely — yielding `pkg:rpm/<name>@<ver>?arch=...` or `pkg:rpm/<distro>/<name>@<ver>?arch=...`.

**Why this priority**: This is a correctness defect that affects 100% of RPM components in scans of bare-RPM corpora (no installed-system context). Downstream consumers (PURL-keyed vulnerability databases, advisory matchers, third-party SBOM-validators) cannot match the malformed PURLs to advisories, silently breaking the security-scanning use case the SBOM exists to support.

**Independent Test**: Place a single `.rpm` file in a temp directory with no `etc/os-release` sibling, run `mikebom sbom scan --path <dir> --format cyclonedx-json`, and assert that no emitted PURL contains the substring `pkg:rpm/rpm/`. Repeat with a temp directory containing both the `.rpm` and a sibling `etc/os-release` declaring `ID=fedora`, and assert PURLs have shape `pkg:rpm/fedora/<name>@<ver>...`.

**Acceptance Scenarios**:

1. **Given** a directory containing one `.rpm` file and no `etc/os-release`, **When** the operator runs `mikebom sbom scan --path <dir>`, **Then** the emitted PURL has no namespace segment (shape `pkg:rpm/<name>@<ver>?arch=<arch>`) and the rendered URL does not contain the substring `pkg:rpm/rpm/`.
2. **Given** a directory containing one `.rpm` file and a sibling `etc/os-release` declaring `ID=fedora`, **When** the operator runs `mikebom sbom scan --path <dir>`, **Then** the emitted PURL has shape `pkg:rpm/fedora/<name>@<ver>?arch=<arch>`.
3. **Given** a Yocto `tmp/deploy/rpm/` directory tree (no `/etc/os-release` at scan root), **When** the operator runs `mikebom sbom scan --path tmp/deploy/rpm/`, **Then** none of the emitted PURLs contain `pkg:rpm/rpm/`.
4. **Given** an existing CycloneDX byte-identity golden test that relied on the buggy `pkg:rpm/rpm/...` output, **When** the milestone lands, **Then** the golden is refreshed to reflect the corrected shape (diff inspectable as a single grep-able pattern change).
5. **Given** a directory containing one `.rpm` whose header declares `RPMTAG_VENDOR=Some Vendor` AND a sibling `etc/os-release` declaring `ID=fedora`, **When** the operator runs `mikebom sbom scan --path <dir>`, **Then** the emitted PURL has shape `pkg:rpm/fedora/<name>@<ver>?arch=<arch>` (the scan-root `/etc/os-release` overrides the per-RPM RPMTAG_VENDOR).

---

### User Story 2 - Well-formed Yocto debug RPMs (up to 512 MB) are no longer silently skipped (Priority: P1)

A Yocto release engineer scans `tmp/deploy/rpm/` for `core-image-minimal`, expecting all 4587 RPMs to appear in the SBOM. Today three of them — `kernel-dbg-*`, `gcc-dbg-*`, `openssl-ptest-*`, each between 260 and 380 MB — are silently dropped because the default per-file size cap is 200 MB. The result is a coverage hole for debug surface (kernel debuginfo, toolchain debuginfo) that security audits and debuginfo-provenance tooling depend on. After this milestone, the default cap is 512 MB, which accommodates every RPM observed in the yocto-test corpus and matches the user-stated upper bound of "≈ 400 MB for kernel and toolchain debug packages" with margin.

**Why this priority**: Silent component-loss in an SBOM is the single worst failure mode — the operator has no signal that anything is missing unless they happen to read the structured WARN log. Three missing components out of 4587 is small in percentage but includes precisely the security-critical debug surface that operators are most likely to care about.

**Independent Test**: Stage a real ~300 MB Yocto debug RPM (or any well-formed `.rpm` between 200 and 512 MB) in a temp directory, run `mikebom sbom scan --path <dir>`, and assert the component count in the emitted SBOM equals 1 (not 0). Compare to a pre-milestone scan which would emit 0 components and the misleading `malformed` WARN.

**Acceptance Scenarios**:

1. **Given** a directory containing a single well-formed `.rpm` file of size 300 MB, **When** the operator runs `mikebom sbom scan --path <dir>`, **Then** the emitted SBOM contains exactly one component for that RPM, and no WARN is logged about the file.
2. **Given** a directory containing a single well-formed `.rpm` file of size 600 MB (above the new 512 MB default cap), **When** the operator runs `mikebom sbom scan --path <dir>`, **Then** the file is skipped with WARN text that does NOT include the word "malformed" and includes the structured field `reason="size-cap-exceeded"`.
3. **Given** the same 600 MB `.rpm` file, **When** the operator runs `mikebom sbom scan --path <dir> --max-rpm-bytes 1073741824` (1 GB cap), **Then** the file is scanned and emitted as one component (no skip).

---

### User Story 3 - Operator can override default RPM size cap (Priority: P2)

An operator scanning a build farm or appliance image discovers an RPM that exceeds even the new 512 MB default (e.g., a kernel-modules-extra RPM in a heavily-loaded enterprise image). Today they would have to rebuild mikebom from source with a patched constant. After this milestone, they pass `--max-rpm-bytes <N>` (where `<N>` is bytes) to the `sbom scan` subcommand to lift the cap for the current scan.

**Why this priority**: The 512 MB default covers the empirical Yocto + Fedora + RHEL corpora observed today, but enterprise / appliance / firmware images periodically grow past arbitrary fixed thresholds. Exposing the knob avoids re-litigating the cap in every milestone-after-this.

**Independent Test**: Stage a 700 MB `.rpm`, scan with default cap → 1 skip + 0 components emitted. Scan with `--max-rpm-bytes 1073741824` → 1 component emitted. Scan with `--max-rpm-bytes 0` → flag rejected at clap parse time (or, equivalently, treated as "unlimited" — see Assumptions).

**Acceptance Scenarios**:

1. **Given** the operator passes `--max-rpm-bytes 1073741824` (1 GB) and a 700 MB `.rpm`, **When** the scan runs, **Then** the RPM is included as a component.
2. **Given** the operator passes `--max-rpm-bytes 100000000` (100 MB) — a value smaller than the new default — and a 300 MB `.rpm`, **When** the scan runs, **Then** the RPM is skipped with the size-cap-exceeded WARN.
3. **Given** the operator passes `--max-rpm-bytes` with a non-numeric value, **When** clap parses the args, **Then** the program exits with a clear error message before any scan begins.

---

### User Story 4 - Operator can override distro identity for Yocto-style scans (Priority: P2)

A Yocto release engineer scanning a build output knows the distro is `poky` (the DISTRO variable in the build config) but the scan root has no `/etc/os-release` because the bare RPMs are sitting in `tmp/deploy/rpm/`. They pass `--rpm-distro poky` so the emitted PURLs have shape `pkg:rpm/poky/<name>@<ver>...` matching what Yocto's own future PURL emission would produce.

**Why this priority**: This is convenience, not correctness — the empty-namespace fallback (US1) already produces valid PURLs. The override exists so Yocto operators can encode the DISTRO identity in PURLs for cross-tool matching with Yocto's native SPDX output (when that adds PURL declarations) and with downstream consumers that key off the distro segment.

**Independent Test**: Stage a `.rpm` in a temp directory with no `/etc/os-release`. Scan with no flag → PURL has no namespace. Scan with `--rpm-distro poky` → PURL has shape `pkg:rpm/poky/<name>@<ver>...`. Both runs MUST succeed without errors.

**Acceptance Scenarios**:

1. **Given** the operator passes `--rpm-distro poky` for a scan of a directory containing `.rpm` files, **When** the scan runs, **Then** all emitted RPM-component PURLs have namespace segment `poky`.
2. **Given** the operator passes `--rpm-distro poky` AND the scan root contains an `/etc/os-release` declaring `ID=fedora`, **When** the scan runs, **Then** the CLI flag takes precedence over the auto-detected `/etc/os-release` identity (all emitted PURLs use namespace `poky`, not `fedora`).
3. **Given** the operator passes `--rpm-distro ""` (empty string), **When** clap parses the args, **Then** the empty value is rejected with a clear error (operators wanting the no-namespace behavior should simply omit the flag).
4. **Given** the operator passes `--rpm-distro poky` AND the directory contains an `.rpm` whose header declares `RPMTAG_VENDOR=Fedora Project`, **When** the scan runs, **Then** the emitted PURL has namespace segment `poky` (the CLI flag overrides per-RPM RPMTAG_VENDOR; explicit operator intent is authoritative).

---

### Edge Cases

- **Multiple `.rpm` files with conflicting RPMTAG_VENDOR values** — When no `--rpm-distro` is set AND no `/etc/os-release` is present at the scan root, each RPM uses its own vendor metadata when available (the existing per-file ladder at `rpm_file.rs:94` is consulted only in this no-CLI-override-and-no-os-release case). When `--rpm-distro` OR `/etc/os-release` IS present, the scan-wide identity overrides per-RPM RPMTAG_VENDOR uniformly — so all emitted PURLs share one namespace per scan regardless of per-RPM vendor tags. This is a deliberate behavior change for the standalone-`.rpm` reader; the rpmdb-path reader's existing precedence is unchanged (Out of Scope §4).
- **`/etc/os-release` with `ID=` containing characters outside `[a-z0-9-]`** — Per purl-spec, the namespace segment must be percent-encoded if it contains reserved characters. Existing PURL canonicalization in `mikebom_common::Purl::new()` handles this; no new escaping logic needed.
- **`/etc/os-release` declaring an empty `ID=`** — Treated as "no distro detected" (empty namespace fallback). The CLI flag still overrides.
- **Scan root is `/` on a live system** — `/etc/os-release` is read from the actual system root, yielding the live system's distro (`fedora`, `rhel`, `centos`, `rocky`, etc.). Behavior is identical to today for the rpmdb code path; the new code path only changes how the **fallback** namespace is computed when an individual `.rpm` file's own vendor tags are absent.
- **`--max-rpm-bytes` very large value (e.g., u64::MAX)** — Accepted; the cap simply never triggers. Memory pressure is bounded by the OS, not by this flag.
- **`--max-rpm-bytes` zero or negative** — Rejected at clap parse time. Operators wanting "no cap" should pass a very large number (documented in `--help`).
- **Scan root has `/etc/os-release` but the file is unreadable (permission denied)** — Treated as "no os-release present"; fall back to empty namespace (or CLI override if set). A single debug-level log line records the skip; no WARN, no scan failure.
- **A `.rpm` is exactly at the cap (size == cap)** — Included (the comparison is `>`, not `>=`, matching today's semantics at `rpm_file.rs:226`).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The RPM-file reader (`mikebom-cli/src/scan_fs/package_db/rpm_file.rs`) MUST NOT emit PURLs with the literal string `"rpm"` as the namespace segment under any code path. The fallback namespace MUST be either (a) the detected distro identifier or (b) empty.
- **FR-002**: When the scan root contains `/etc/os-release` (or `etc/os-release` for relative-path scans), the reader MUST parse the file's `ID=` field and use the value as the RPM PURL namespace for every `.rpm` file read in this scan, **overriding any per-RPM RPMTAG_VENDOR / RPMTAG_PACKAGER metadata in the individual `.rpm` headers**. If `/etc/os-release` is missing, unreadable, or has no `ID=` line, the reader falls through to per-RPM RPMTAG_VENDOR (existing ladder at `rpm_file.rs:94`); if that is also absent, the namespace is empty.
- **FR-003**: The `sbom scan` subcommand MUST accept a new optional flag `--rpm-distro <ID>` (string). When present, the value overrides ALL other distro-identity sources for the duration of the scan — including auto-detected `/etc/os-release` `ID=` AND per-RPM RPMTAG_VENDOR / RPMTAG_PACKAGER metadata. The flag value MUST be non-empty (clap rejects empty strings at parse time).
- **FR-004**: The reader MUST raise the default per-file size cap from the current 200 MB to **512 MB** (`512 * 1024 * 1024` bytes).
- **FR-005**: The `sbom scan` subcommand MUST accept a new optional flag `--max-rpm-bytes <N>` (unsigned integer, bytes). When present, the value overrides the 512 MB default for the duration of the scan. The flag value MUST be > 0 (clap rejects zero at parse time).
- **FR-006**: When an RPM is skipped because its on-disk size exceeds the active cap, the WARN log MUST NOT include the word "malformed". The exact wording MAY change (e.g., to "skipping oversized .rpm file") but the structured field `reason="size-cap-exceeded"` MUST be preserved unchanged so existing log-parsing tooling continues to work.
- **FR-007**: The existing WARN paths for genuinely malformed RPMs (empty file, lead-magic mismatch, header parse failure) MUST continue to use the word "malformed" with their existing structured `reason="..."` fields unchanged.
- **FR-008**: The size-cap raise MUST also apply to `MAX_RPMDB_BYTES` in `mikebom-cli/src/scan_fs/package_db/rpm.rs` (currently `200 * 1024 * 1024`) → raised to **512 MB** for consistency. Operator override of the rpmdb cap is **out of scope** for this milestone.
- **FR-009**: All existing byte-identity SBOM golden tests that contain `pkg:rpm/rpm/...` PURLs MUST be refreshed in the same PR. The refresh diff MUST be limited to PURL-shape changes (drop `/rpm/` segment) plus any cascading `bom-ref`-uses-purl changes — no unrelated golden drift.
- **FR-010**: The change MUST be observable in CDX, SPDX 2.3, and SPDX 3 output formats simultaneously (all three downstream emitters consume the same in-memory PURL string, so a single fix in the reader satisfies cross-format invariance).
- **FR-011**: The CLI `--help` output for both new flags MUST include their default values and a one-line description naming the milestone-144 use case (Yocto debug RPMs for `--max-rpm-bytes`; Yocto DISTRO override for `--rpm-distro`).

### Key Entities

- **RPM file** — A standalone `.rpm` archive on the filesystem. Has on-disk size (bytes), an optional RPMTAG_VENDOR + RPMTAG_PACKAGER + RPMTAG_DISTRIBUTION trio in its header, a name, version, release, epoch, and architecture.
- **Distro identity** — A short lowercase string (typical examples: `fedora`, `rhel`, `centos`, `rocky`, `poky`, `opensuse`). Sourced in **strict priority order** (later sources are consulted ONLY if all earlier sources are empty/absent): (1) `--rpm-distro` CLI override — authoritative; overrides every other source including per-RPM header metadata, (2) `/etc/os-release` `ID=` field at scan root — authoritative when present; overrides per-RPM RPMTAG_VENDOR / RPMTAG_PACKAGER, (3) per-RPM RPMTAG_VENDOR / RPMTAG_PACKAGER (existing ladder at `rpm_file.rs:94`), (4) empty (the new fallback replacing today's literal `"rpm"`).
- **Size cap** — A u64 byte count. Default: 512 MiB (`512 * 1024 * 1024`). Overridable per-scan via `--max-rpm-bytes`. Applied as `if size > cap { skip }` (strict greater-than, matching today's semantics).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: After this milestone, scanning the yocto-test corpus (`yocto-test` feature `001-yocto-baseline-build`, `core-image-minimal` for qemux86-64) produces an SBOM with **4587 RPM components** (not 4584). The three previously-dropped debug RPMs (`kernel-dbg`, `gcc-dbg`, `openssl-ptest`) MUST appear in the emitted CDX/SPDX/SPDX-3 output.
- **SC-002**: After this milestone, scanning the same yocto-test corpus produces **zero PURLs containing the substring `pkg:rpm/rpm/`** in the emitted SBOM (verifiable via `grep -c 'pkg:rpm/rpm/' core-image-minimal.cdx.json` returning `0`).
- **SC-003**: A unit test in `mikebom-cli/src/scan_fs/package_db/rpm_file.rs` constructs a synthetic well-formed `.rpm` (or uses an existing fixture) and asserts that the emitted PURL has shape `pkg:rpm/<name>@<ver>...` (no namespace) when neither `/etc/os-release` nor a CLI override is present.
- **SC-004**: A unit test asserts that with `--rpm-distro poky` (simulated via the reader-level helper API), the same synthetic RPM emits `pkg:rpm/poky/<name>@<ver>...`.
- **SC-005**: A unit test asserts that with `--rpm-distro` unset and an `etc/os-release` file declaring `ID=fedora` in the scan root, the emitted PURL has shape `pkg:rpm/fedora/<name>@<ver>...`.
- **SC-006**: A unit test asserts that scanning a 300 MB synthetic `.rpm` (or any test fixture in the 200–512 MB range) emits one component (not zero) and logs no WARN about size.
- **SC-007**: A unit test asserts that scanning a >512 MB synthetic `.rpm` with default cap emits zero components and logs WARN text that does NOT contain the substring `"malformed"`, while still containing `reason="size-cap-exceeded"`.
- **SC-008**: `cargo +stable clippy --workspace --all-targets -- -D warnings` and `cargo +stable test --workspace` both pass clean on the milestone branch — i.e., `./scripts/pre-pr.sh` exits 0.
- **SC-009**: All refreshed byte-identity golden files (CDX + SPDX 2.3 + SPDX 3 per ecosystem that includes RPM components in goldens — likely the rpm-fixture-based tests under `mikebom-cli/tests/`) match the new wire output byte-for-byte across `linux-x86_64` and `macos-arm64` CI lanes.
- **SC-010**: The `mikebom sbom scan --help` output includes lines for both `--rpm-distro <ID>` and `--max-rpm-bytes <N>` with default values and one-line descriptions.
- **SC-011**: A unit test asserts that an `.rpm` whose header declares `RPMTAG_VENDOR=Some Vendor`, scanned with NO CLI override but WITH an `etc/os-release` declaring `ID=fedora` in the scan root, emits PURL with namespace `fedora` (not `Some Vendor`) — validates the FR-002 os-release-over-per-RPM-metadata override.
- **SC-012**: A unit test asserts that an `.rpm` whose header declares `RPMTAG_VENDOR=Fedora Project`, scanned with `--rpm-distro poky`, emits PURL with namespace `poky` (not `Fedora Project` and not `fedora`) — validates the FR-003 absolute CLI override.

## Assumptions

- **`/etc/os-release` is the right auto-detection source.** It is the freedesktop.org-standardized cross-distro source for distro identity (Fedora, RHEL, CentOS, Rocky, openSUSE, and Yocto all ship it in installed-system rootfs). Yocto build outputs (`tmp/deploy/rpm/`) do NOT ship it at the scan root, which is exactly why US4's `--rpm-distro` override exists.
- **The PURL `ID=` value goes in verbatim (lowercased) without translation.** I.e., `fedora` stays `fedora`, `rhel` stays `rhel`, `centos-stream` stays `centos-stream`. We do not translate `rhel` → `redhat` or apply any other mapping. If a downstream consumer needs a translated namespace, they can use `--rpm-distro` to set whatever they want.
- **512 MB is generous enough for the foreseeable future.** The largest RPM in any production corpus we are aware of (Fedora kernel-debuginfo, Yocto gcc-dbg, RHEL kernel-debug-modules-extra) is under 500 MB. The cap exists primarily as a safety rail against pathological inputs (truncated multi-GB sparse files, fuzz-corpus artifacts), not as a routine gate.
- **`--max-rpm-bytes` and `--rpm-distro` apply scan-wide, not per-path.** A single scan invocation produces one set of PURLs with one consistent namespace policy. Mixed-distro scans (rare; e.g., scanning a directory tree containing both a Fedora rootfs and a Yocto build output) will need two separate scan invocations — no per-subtree distro selection in this milestone.
- **The rpmdb cap raise (FR-008) is uncontroversial.** RPM databases >200 MB do exist on appliance/edge images with thousands of installed packages; raising to 512 MB is symmetric with the per-file cap and adds no new attack surface (the database is already validated before reading).
- **The warning-text change (FR-006) is a low-risk wording fix, not a structured-log breaking change.** The `reason="size-cap-exceeded"` field — which is what any sane log-parsing tool greps for — is unchanged. Only the free-text prefix changes.
- **No new dependencies needed.** `/etc/os-release` parsing is a stdlib `BufReader` + line-iteration pass (the file format is `KEY=VALUE` with optional quoting, ~30 lines of code). `mikebom-cli/src/scan_fs/os_release.rs` already exists (referenced by Arch/Homebrew readers in milestones 135–136); reuse it.
- **Yocto's `tmp/deploy/rpm/` does not have `/etc/os-release` at the scan root.** The operator runs `mikebom sbom scan --path tmp/deploy/rpm/` not `mikebom sbom scan --path /` — so auto-detection cannot find an `/etc/os-release`. This is the case US4 (`--rpm-distro`) solves directly.
- **Scope of this milestone is bounded to RPM reader changes.** The yocto-test feedback also noted that "Yocto's own SPDX doesn't declare PURLs at all" — that's a Yocto upstream issue, not a mikebom one, and is out of scope.

## Out of Scope

- Per-subtree distro selection (e.g., a YAML config file mapping path prefixes to distros).
- Operator override of `MAX_RPMDB_BYTES` (no production corpus has surfaced the need; deferred to a follow-up if a real-world report appears).
- Per-RPM size-cap exemption lists.
- Changes to the rpmdb-path PURL construction (which already correctly uses the distro from `/etc/os-release` when scanning an installed-system rootfs — the bug is specifically in the standalone-`.rpm`-file path's fallback branch).
- New CDX/SPDX `mikebom:*` properties for size-cap or distro-detection metadata — the structured WARN log is sufficient operator-facing signal; the SBOM is not the place to record "I tried to scan a 600 MB RPM and skipped it" (per Constitution Principle V, native fields take precedence over `mikebom:*` annotations, and there's no native field for this).
- Live `rpm` subprocess invocation (mikebom remains a pure-Rust direct-reader).
- Changing the WARN-text for genuinely malformed RPMs (FR-007 explicitly preserves the existing wording for those paths).
