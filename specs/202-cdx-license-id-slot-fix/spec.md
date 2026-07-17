# Feature Specification: CDX License Splitter — LicenseRef Escape Hatch for Non-Canonical Operands

**Feature Branch**: `202-cdx-license-id-slot-fix`
**Created**: 2026-07-17
**Status**: Draft
**Input**: User description: "579" (GitHub issue #579 — sbom scan (ipk): CDX license splitter emits non-canonical identifiers as license.id (should be LicenseRef-* via license.name))

## Overview

CycloneDX 1.6 spec §5.4.4.1 restricts the `license.id` field to values from the SPDX License List. Non-canonical identifiers (like `bzip2-1.0.4` — SPDX has `bzip2-1.0.6` but not `-1.0.4`) MUST be emitted as `license.name` with a `LicenseRef-*` prefix, matching the SPDX 2.3 spec-blessed escape hatch (also documented at CDX 1.6 §5.4.4.2).

The current CDX license splitter at `mikebom-cli/src/generate/cyclonedx/builder.rs::license_entry_for_token` (line 1494) only checks whether the token STARTS WITH `LicenseRef-` / `DocumentRef-`. Any other token — including invalid SPDX-list operands surfaced by the m190 compound-expression splitter — falls through to the `license.id` slot, producing schema-invalid CDX output.

The SPDX 2.3 emitter side of the same pipeline already handles this correctly via the `normalize_license_operand` helper at `mikebom-cli/src/scan_fs/package_db/rpm_file.rs:657+` (landed in m152 for issue #481). m202 achieves CDX/SPDX parity by extending the CDX splitter's per-token classification to consult the SPDX License List membership check that already exists in the codebase.

**User-observable symptom** (from #579 evidence): 9 busybox-family components in the yocto-test `core-image-minimal` build emit `"id": "bzip2-1.0.4"` — schema-invalid per CDX 1.6 §5.4.4.1. Broader impact scales across Yocto BSPs whose recipe `License:` fields reference non-SPDX-list operands.

**Downstream impact**: strict-mode CDX license auditors (SBOM allowlist enforcers, procurement tools) either accept invalid IDs silently or fail loudly on the invalid entry. Neither is what operators want.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Non-canonical license operands route to LicenseRef via license.name (Priority: P1)

An operator scans a Yocto image (or any ipk source) containing packages whose recipe `License:` field lists a non-SPDX-list operand — e.g., `GPL-2.0-only & bzip2-1.0.4`. In the emitted CDX 1.6 output, the SPDX-recognized `GPL-2.0-only` half appears in `license.id`; the non-canonical `bzip2-1.0.4` half appears in `license.name` as `LicenseRef-bzip2-1.0.4` (matching the SPDX 2.3 emitter's `LicenseRef-*` escape hatch and CDX 1.6 §5.4.4.2). The CDX output is now schema-valid for strict-mode auditors.

**Why this priority**: Schema-invalid CDX output is a shipping blocker for consumers running strict validators. Every Yocto/ipk user scanning core-image-minimal today produces invalid CDX at each busybox-family component (9 components in the m190 verification build). Closes #579.

**Independent Test**: Fixture with a synthetic ipk carrying `License: GPL-2.0-only & bzip2-1.0.4`. Scan → assert the emitted CDX's `licenses[]` contains ONE entry `{"license": {"id": "GPL-2.0-only", ...}}` AND ONE entry `{"license": {"name": "LicenseRef-bzip2-1.0.4", ...}}`. No `"id": "bzip2-1.0.4"` anywhere.

**Acceptance Scenarios**:

1. **Given** an ipk with a synthesized control file containing `License: GPL-2.0-only & bzip2-1.0.4`,
   **When** mikebom scans and emits CycloneDX 1.6 JSON,
   **Then** the emitted component's `licenses[]` array contains: (a) `{"license": {"id": "GPL-2.0-only", "acknowledgement": "declared"}}` AND (b) `{"license": {"name": "LicenseRef-bzip2-1.0.4", "acknowledgement": "declared"}}`. No entry has `"id": "bzip2-1.0.4"`.

2. **Given** the same ipk,
   **When** mikebom emits SPDX 2.3 alongside CycloneDX,
   **Then** the SPDX 2.3 output's `hasExtractedLicensingInfos[]` includes a `LicenseRef-*` entry whose `extractedText` matches the compound expression — SAME semantic as the CDX output, matching the format-parity contract per Constitution Principle V.

3. **Given** the real-world reproducer (`yocto-test scarthgap core-image-minimal` from #550 evidence),
   **When** mikebom scans post-fix,
   **Then** the query `jq '[.components[] | .licenses[]? | .license? | select(.id == "bzip2-1.0.4")] | length' core-image-minimal.cdx.json` returns `0` (was `9` pre-fix), AND `jq '[.components[] | .licenses[]? | .license? | select(.name == "LicenseRef-bzip2-1.0.4")] | length'` returns `>= 9` (the reclassified entries).

---

### User Story 2 - SPDX-canonical operands and existing LicenseRef tokens retain their current slot (Priority: P1)

The fix MUST NOT reclassify operands that ARE on the SPDX License List (they continue to emit as `license.id`), and MUST NOT double-prefix tokens that already start with `LicenseRef-` / `DocumentRef-` (they continue to emit as `license.name` unchanged).

**Why this priority**: Regression risk. The CDX license emitter is exercised by every ecosystem scan; drift on well-formed operands would break most existing goldens.

**Independent Test**: Existing CDX goldens (across cargo, npm, python, maven, etc.) hold byte-identically. Grep on `mikebom-cli/tests/fixtures/golden/cyclonedx/` post-fix reveals only ipk-adjacent entries as candidates for change.

**Acceptance Scenarios**:

1. **Given** a package with `License: MIT` (canonical SPDX identifier),
   **When** mikebom emits CDX,
   **Then** the license entry is `{"license": {"id": "MIT", "acknowledgement": "declared"}}` — unchanged from pre-m202.

2. **Given** a package whose control file License field references a pre-formed `LicenseRef-` token (rare but legal),
   **When** mikebom emits CDX,
   **Then** the license entry retains its `{"license": {"name": "LicenseRef-...", ...}}` shape — no double-prefixing to `LicenseRef-LicenseRef-...`.

3. **Given** an SPDX license expression like `Apache-2.0 OR MIT`,
   **When** mikebom's m190 splitter fires + m202 per-token classification runs,
   **Then** both `Apache-2.0` and `MIT` land in `license.id` slots (both are on the SPDX License List).

---

### Edge Cases

- **Empty operand token**: the m190 splitter already skips empty tokens; m202 inherits that behavior unchanged.
- **Whitespace-containing operand token**: the m190 splitter rejects these; m202 inherits unchanged.
- **SPDX License List membership uncertainty at parse time**: the `spdx = "0.10"` crate (already a workspace dep per m152's use in `SpdxExpression::try_canonical`) provides authoritative list-membership check. Use it directly.
- **License operand with hyphens/dots matching SPDX-list format but NOT on the list** (like `bzip2-1.0.4`): correctly routed to `LicenseRef-*` post-fix. This is the primary bug class from #579.
- **Non-ASCII characters in operand token**: the SPDX 2.3 sanitizer (`normalize_license_operand`) already applies alphanumeric+`-`+`.` filtering before prefixing. m202 leverages the same sanitizer for CDX to produce identical `LicenseRef-*` output across formats (parity per Constitution V).
- **Compound expression where EVERY operand is non-canonical**: e.g., `bzip2-1.0.4 & bzip2-1.0.5`. All get routed to `LicenseRef-*` correctly.
- **License expression that fails SPDX-crate parsing entirely** (severely malformed): existing m190 splitter behavior — falls back to a single `text` entry — is preserved. Not in m202 scope.
- **License-splitter output containing an SPDX exception ID** (e.g., `Classpath-exception-2.0` from a `WITH` clause): m202 treats exception IDs the same as license IDs — checks membership in the exception list too, routes non-members to `LicenseRef-*`. Not the primary bug class but a natural extension of the check.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The CDX license splitter at `mikebom-cli/src/generate/cyclonedx/builder.rs::license_entry_for_token` MUST check each token against the SPDX License List (via the existing `spdx` crate that m152 already uses). Tokens found on the list emit as `{"license": {"id": <token>, ...}}` (unchanged pre-m202). Tokens NOT on the list AND NOT already prefixed with `LicenseRef-`/`DocumentRef-` MUST be re-routed to `{"license": {"name": "LicenseRef-<sanitized>", ...}}` — same sanitization the SPDX 2.3 emitter uses.
- **FR-002 (revised at implement-phase after mid-implementation verification)**: The per-token sanitizer used by the CDX `LicenseRef-` prefix wrapping MUST be the SAME shared function that the SPDX 2.3 emitter's per-operand path uses — extracted to `mikebom_common::types::license::sanitize_license_operand_to_ref`. This is a STRUCTURAL parity guarantee (single-source-of-truth for the per-operand-sanitization behavior), not a byte-identical wire-format-identifier guarantee. The original spec's "byte-identical `LicenseRef-<sanitized>` identifier" claim was over-reach: the SPDX 2.3 emitter's top-level compound-expression path uses a DIFFERENT hash-based `LicenseRef-<HASH>` identifier (referencing the full expression in `hasExtractedLicensingInfos[]`) that pre-dates m152 and is unrelated to the per-operand sanitizer. Both approaches are spec-legal escape hatches per CDX 1.6 §5.4.4.2 / SPDX 2.3 respectively. The user-observable outcome (non-canonical operands routed away from schema-invalid `license.id` slot) is achieved uniformly across both formats.
- **FR-003**: Existing `LicenseRef-*` / `DocumentRef-*` prefixed tokens (pre-formed by upstream code or user-supplied) MUST NOT be double-prefixed. Post-fix, tokens starting with either prefix continue to emit as `{"license": {"name": <token>, ...}}` unchanged.
- **FR-004**: Existing CDX goldens (cargo, npm, pip, maven, gem, etc. — all ecosystems whose License fields carry canonical SPDX identifiers) MUST hold byte-identically. No test-side assertion updates required beyond the new m202 regression test.
- **FR-005**: A new regression fixture (synthetic ipk carrying `License: GPL-2.0-only & bzip2-1.0.4`) MUST reproduce the #579 pattern. Integration test asserts the split produces one canonical `id` entry AND one `LicenseRef-*` `name` entry.
- **FR-006**: `./scripts/pre-pr.sh` MUST continue to pass green post-fix.
- **FR-007**: No new `mikebom:*` annotations introduced. The fix corrects an existing wire-format encoding to be CDX-1.6-schema-conformant per Constitution Principle V's "standards-native fields take precedence" clause — routing non-canonical operands to `license.name` (SPDX-blessed escape hatch) IS the standards-native construct.

### Key Entities

- **SPDX License List**: canonical list of license identifiers maintained by the SPDX project. Referenced via the `spdx = "0.10"` workspace dependency (already used by m152's `SpdxExpression::try_canonical` at `mikebom-common/src/types/license.rs`). Membership check informs the m202 id-vs-name slot classification.
- **`license.id` (CDX 1.6 spec §5.4.4.1)**: JSON field on `license` objects. Value MUST be from the SPDX License List. Schema-invalid otherwise.
- **`license.name` + `LicenseRef-<sanitized>` (CDX 1.6 spec §5.4.4.2)**: JSON field for free-text license labels. The `LicenseRef-*` prefix convention is SPDX-blessed as an escape hatch for non-canonical identifiers. This slot IS the standards-native construct for the non-canonical case.
- **`normalize_license_operand` helper (existing at `rpm_file.rs:657+`)**: sanitizes an operand token to the alphanumeric + `-` + `.` character set, prefixes with `LicenseRef-`. Landed in m152 for SPDX 2.3 emission per #481. m202 reuses this (or extracts it to a shared location if the m152 site is too RPM-specific).
- **`license_entry_for_token` (existing at `builder.rs:1494`)**: the CDX splitter's per-token classification helper. Two-branch logic pre-m202; three-branch logic post-m202 (id / LicenseRef-name / already-prefixed-name).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For the #579 reproducer (synthetic ipk with `License: GPL-2.0-only & bzip2-1.0.4`), `jq '[.components[] | .licenses[]? | .license? | select(.id == "bzip2-1.0.4")] | length'` returns `0` post-fix (was `1` pre-fix per the fixture; scales to `9` on the real yocto-test core-image-minimal build).
- **SC-002**: For the same fixture, `jq '[.components[] | .licenses[]? | .license? | select(.name == "LicenseRef-bzip2-1.0.4")] | length'` returns `1` post-fix (was `0` pre-fix). The reclassified entry lands in the correct slot.
- **SC-003**: For the same fixture's `licenses[]` array, exactly one entry has `.id == "GPL-2.0-only"` (unchanged — canonical SPDX identifier stays in `license.id`).
- **SC-004 (revised at implement-phase per FR-002 revision above)**: For the SPDX 2.3 emission of the same scan, the `hasExtractedLicensingInfos[]` MUST contain at least one `LicenseRef-*` entry (structural presence guarantee). Byte-identity with the CDX `license.name` value is NOT achievable because the SPDX 2.3 emitter uses a hash-based LicenseRef scheme distinct from the CDX per-operand scheme. Both are spec-legal escape hatches.
- **SC-005**: Every existing CDX golden test in `mikebom-cli/tests/fixtures/golden/cyclonedx/` passes byte-identically. Zero goldens require regen for the fix itself. The public-corpus goldens (rust-ripgrep, python-flask, maven-guice, npm-express, go-cobra, image-postgres16) MAY require regen IF any of their license operands turn out to be non-canonical (empirical re-verification at implement time per m199 lesson).
- **SC-006**: `./scripts/pre-pr.sh` wall-clock delta ≤ 5 seconds vs pre-m202 baseline.
- **SC-007**: Post-merge, `#579` closes automatically via `Closes #579` in the PR body.

## Assumptions

- **`spdx = "0.10"` crate already provides SPDX List membership check**: the workspace dep is already available at `mikebom-common` per m152. m202 uses it directly; no new crate needed.
- **`normalize_license_operand` at `rpm_file.rs:657+` is the right sanitizer to reuse**: it may need extraction to a shared module (e.g., `mikebom-common/src/types/license.rs`) so the CDX emitter can call it without cross-module leakage from the RPM reader. Plan phase decides between extract-and-share vs re-implement-with-shared-tests.
- **CDX/SPDX 2.3 parity is a strict correctness requirement**: two emissions of the same scan MUST produce byte-identical `LicenseRef-*` identifiers for the same operand token. FR-002 codifies this.
- **Non-cargo/non-ipk ecosystems are UNAFFECTED**: only the CDX splitter's per-token classification changes. Ecosystems whose License strings are always canonical SPDX identifiers (cargo Cargo.toml `license = "MIT"`, npm `"license": "Apache-2.0"`, etc.) skip the LicenseRef path entirely because their tokens ARE on the SPDX list.
- **Regression goldens scope is bounded but non-zero POSSIBLE**: Yocto-derived corpus SBOMs (if any exist in `tests/fixtures/`) may drift. Re-verified at implement time via `git diff --stat mikebom-cli/tests/fixtures/`; if non-zero, scope-drift disposition triggered per m199 lesson.
- **Zero new Cargo dependencies**: reuse existing `spdx` crate machinery.
- **Constitution Principle V compliance**: no new `mikebom:*` annotations. The fix uses the CDX 1.6 spec-blessed `license.name` + `LicenseRef-*` construct — a standards-native escape hatch, not a mikebom-invented one.
