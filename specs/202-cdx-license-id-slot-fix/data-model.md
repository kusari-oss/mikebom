# Data Model: CDX License Splitter — LicenseRef Escape Hatch

**Date**: 2026-07-17
**Purpose**: Document the 3-branch classification logic replacing today's 2-branch, plus the shared sanitizer's new home. No new struct fields or wire-format constructs — the fix uses the CDX 1.6 spec's existing `license.id` / `license.name` shape correctly.

## E1: `license_entry_for_token` — three-branch classification (was two)

**Location**: `mikebom-cli/src/generate/cyclonedx/builder.rs:1494`.

**Pre-m202 (2 branches)**:

```text
INPUT: token, acknowledgement
├── if token starts with "LicenseRef-" OR "DocumentRef-":
│      → {"license": {"name": <token>, "acknowledgement": <ack>}}
└── else:
       → {"license": {"id": <token>, "acknowledgement": <ack>}}  ← BUG
```

The `else` branch dumps ANY non-prefixed token into `license.id`, even if the token isn't on the SPDX License List (schema-invalid per CDX 1.6 §5.4.4.1).

**Post-m202 (3 branches)**:

```text
INPUT: token, acknowledgement
├── if token starts with "LicenseRef-" OR "DocumentRef-":
│      → {"license": {"name": <token>, "acknowledgement": <ack>}}
│      (unchanged — no double-prefixing)
├── else if spdx::LicenseId::from_name(token).is_some()
│         OR spdx::exception_id(token).is_some():
│      → {"license": {"id": <token>, "acknowledgement": <ack>}}
│      (canonical SPDX identifier or exception — schema-legal)
└── else:
       → {"license": {"name": "LicenseRef-<sanitized>", "acknowledgement": <ack>}}
       (non-canonical → LicenseRef escape hatch per CDX 1.6 §5.4.4.2)
```

**Validation rules**:
- Branch 1 preserves the raw token verbatim (already pre-formed `LicenseRef-*` / `DocumentRef-*`).
- Branch 2 preserves the raw token verbatim (canonical SPDX identifier — MUST NOT be normalized to avoid golden drift; that's why `LicenseId::from_name` is used instead of `try_canonical`).
- Branch 3 sanitizes the token via the shared sanitizer helper (E2 below) and wraps with `LicenseRef-`.
- Empty token → the m190 splitter already skips these; m202 inherits.
- Whitespace-containing token → the m190 splitter rejects these; m202 inherits.

## E2: `sanitize_license_operand_to_ref` — shared sanitizer (extracted)

**New location**: `mikebom-common/src/types/license.rs` as `pub fn sanitize_license_operand_to_ref(s: &str) -> Option<String>`.

**Extracted from**: `mikebom-cli/src/scan_fs/package_db/rpm_file.rs:778` (existing `sanitize_to_license_ref_idstring`).

**Wire behavior** (unchanged from the extraction source):
- Input: raw operand token (`bzip2-1.0.4`, `custom-license`, `bzip2-1.0.4-ish!with!chars`).
- Filter: keep alphanumeric chars, `-`, `.`. Drop everything else.
- Prefix: `LicenseRef-` prepended to the filtered result.
- Return: `Some("LicenseRef-<filtered>")` on success, `None` if the filtered result is empty (all chars stripped).

**Examples**:
- `sanitize_license_operand_to_ref("bzip2-1.0.4")` → `Some("LicenseRef-bzip2-1.0.4")`
- `sanitize_license_operand_to_ref("bzip2 with spaces")` → `Some("LicenseRef-bzip2withspaces")` (spaces stripped)
- `sanitize_license_operand_to_ref("!!!")` → `None` (all chars stripped)
- `sanitize_license_operand_to_ref("")` → `None`

**Idempotent**: `sanitize_license_operand_to_ref(sanitize_license_operand_to_ref(x).unwrap())` == `sanitize_license_operand_to_ref(x)` — a value that's already been sanitized to `LicenseRef-<name>` (with hyphen from prefix + hyphens from content) passes through unchanged. Guaranteed by the character-filter approach: hyphens are preserved, so re-running the sanitizer on `LicenseRef-bzip2-1.0.4` yields `LicenseRef-LicenseRef-bzip2-1.0.4` — actually, this is NOT idempotent as I stated. Let me fix.

**Corrected idempotency**: the function is NOT idempotent for arbitrary inputs. Callers MUST NOT re-sanitize an already-prefixed `LicenseRef-*` value. The CDX splitter's Branch 1 (checks `LicenseRef-` prefix BEFORE calling the sanitizer) enforces this precondition. The SPDX 2.3 emitter (existing caller) has the same guard.

**Consumers post-extraction**:
1. `mikebom-cli/src/scan_fs/package_db/rpm_file.rs` — existing SPDX 2.3 emission path, now imports from `mikebom_common::types::license`.
2. `mikebom-cli/src/generate/cyclonedx/builder.rs` — new caller from Branch 3 of `license_entry_for_token`.

Both consumers guarantee the input token is NOT already `LicenseRef-*`-prefixed (Branch 1 in E1 above; equivalent guard in the SPDX 2.3 path).

## E3: SPDX List membership check via `spdx::LicenseId::from_name`

**No new code introduced** — direct call to the workspace `spdx` crate's existing API. Documented here for reviewer clarity.

**Signature** (from the `spdx` crate):

```rust
pub fn spdx::LicenseId::from_name(name: &str) -> Option<&'static LicenseId>
```

**Behavior**: static-table lookup. Returns `Some(&LicenseId)` if `name` matches one of the ~600 SPDX License List canonical identifiers (case-sensitive per SPDX spec). Returns `None` otherwise.

**No normalization**: `from_name` does NOT translate legacy long-form names (like `GPL-2.0`) to their canonical short-form. This is exactly what m202 needs: preserve the token verbatim in `license.id` when it's on the list; route to `LicenseRef-*` when it's not. No silent value drift.

**Similarly for exceptions**: `spdx::exception_id(&str) -> Option<&'static ExceptionId>` for SPDX exception identifiers (Classpath-exception-2.0, LLVM-exception, etc.). Used by m202 when a token comes from a `WITH` clause per spec edge cases.

## Cross-cutting: FR-002 CDX/SPDX 2.3 parity

**Guarantee**: for any input operand token `X`, both the CDX Branch 3 and the SPDX 2.3 emitter's LicenseRef path MUST call `sanitize_license_operand_to_ref(X)` — the same shared function — producing byte-identical `LicenseRef-<sanitized>` identifiers.

**Enforcement**:
- Extraction to `mikebom-common` makes it structurally impossible for the two emitters to diverge (single source of truth per data-model E2).
- FR-002 integration test asserts (a) CDX `license.name` value AND (b) SPDX 2.3 `hasExtractedLicensingInfos[].licenseId` value are string-equal for the same scan input.

**Consumer-side benefit**: downstream tooling that reads BOTH CDX and SPDX 2.3 emissions of the same scan can cross-reference `LicenseRef-*` identifiers directly (join key). Pre-m202, they couldn't — CDX was silently emitting the raw operand in the id slot while SPDX 2.3 was correctly LicenseRef-ing it.
