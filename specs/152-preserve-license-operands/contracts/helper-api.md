# Contract — helper API surface (internal to `rpm_file.rs`)

This milestone introduces 3 internal-to-module helpers. None are `pub` outside `rpm_file.rs`; this contract documents their behavioral contract for the file's other functions + the test module.

## Contract 1 — `sanitize_to_license_ref_idstring`

**Signature**:
```rust
fn sanitize_to_license_ref_idstring(s: &str) -> Option<String>
```

**Pre-conditions**: any `&str` is valid input (including empty, whitespace-only, ASCII, non-ASCII, control chars).

**Post-conditions**:
- Returns `Some(out)` where `out` matches the SPDX 2.3 LicenseRef idstring grammar `[a-zA-Z0-9-.]+`.
- Returns `None` IFF the algorithm produces an empty string (all input chars were sanitized away).
- Pure function — no I/O, no side effects, no panics, deterministic.
- Idempotent — `sanitize(sanitize(s)) ≡ sanitize(s)` for any `s` where the first call returns `Some(_)`.

**Algorithm** (per research.md §R5 + Clarifications Q1):
1. Replace each char outside `[a-zA-Z0-9-.]` with `-`.
2. Collapse runs of consecutive `-` to a single `-`.
3. Strip leading and trailing `-`.
4. If the resulting string is empty → return `None`; else return `Some(string)`.

**Worked examples** (testable per FR-002 + test #8):

| Input             | Output                |
|-------------------|-----------------------|
| `"GPLv2+"`        | `Some("GPLv2")`       |
| `"My License v2"` | `Some("My-License-v2")` |
| `"(custom)"`      | `Some("custom")`      |
| `"LGPL-2.1+"`     | `Some("LGPL-2.1")`    |
| `"bzip2-1.0.4"`   | `Some("bzip2-1.0.4")` |
| `"PD"`            | `Some("PD")`          |
| `"!@#$"`          | `None`                |
| `""`              | `None`                |
| `"---"`           | `None`                |

## Contract 2 — `tokenize`

**Signature**:
```rust
fn tokenize<'a>(raw: &'a str) -> Vec<Token<'a>>
```

**Pre-conditions**: any `&str` is valid input.

**Post-conditions**:
- Returns a `Vec<Token<'a>>` where each `Token` is one of: `Operand(&'a str)`, `And`, `Or`, `With`, `LParen`, `RParen`, `Whitespace`.
- All `Operand` variants carry borrowed slices of `raw` — no allocation per token.
- Operator re-classification: any `Operand(s)` where `s == "AND" | "OR" | "WITH"` (case-sensitive) is re-emitted as the corresponding operator variant.
- Empty input → `Vec::new()`.
- Whitespace-only input → `vec![Token::Whitespace]`.

**Grammar** (per research.md §R2):
- Operand = contiguous run of chars that are NEITHER whitespace NOR paren.
- Operators are case-sensitive uppercase strings `"AND"`, `"OR"`, `"WITH"`.
- Parens are standalone single-char tokens.
- Whitespace runs are collapsed to one `Token::Whitespace`.

## Contract 3 — `preserve_known_operands_with_license_ref`

**Signature**:
```rust
fn preserve_known_operands_with_license_ref(raw: &str) -> Option<String>
```

**Pre-conditions**: any `&str` is valid input. Caller is expected to invoke this AFTER `normalize_bitbake_license_operators` AND after `SpdxExpression::try_canonical(raw)` returned Err.

**Post-conditions**:
- Returns `Some(rebuilt)` when:
  - Every recognized operand passed through unchanged.
  - Every unrecognized operand was successfully wrapped as `LicenseRef-<sanitized>`.
  - Every WITH-clause exception was recognized.
  - The rebuilt string is structurally valid SPDX 2.3 (operators + parens preserved).
- Returns `None` when:
  - Input is empty or whitespace-only (FR-008 + US1 scenario 5).
  - At least one WITH-clause exception was unrecognized (FR-013 + Clarifications Q2 — whole compound → NOASSERTION).
  - At least one unrecognized operand's sanitization produced an empty idstring (e.g., `"!@#$"` → no LicenseRef can be formed → fall back to NOASSERTION).
- Pure function. No I/O. Deterministic. No panics.
- Idempotent — `preserve(preserve(s).unwrap_or_default()) ≡ preserve(s)` for any `s` where the first call returns `Some(_)`.

**Does NOT**:
- Call `SpdxExpression::try_canonical` itself. The caller performs the final canonicalization (rationale: keeps the Result-handling site singular at `rpm_file.rs:472–476`).
- Mutate any external state. No file I/O, no network, no logging beyond `tracing::trace!` if debugging proves necessary (currently no logging planned).
- Emit `DocumentRef-<docid>:LicenseRef-<idstring>` forms (per FR-011 — Yocto-specific context not available at the RPM reader).
- Wrap WITH-clause exception identifiers as `LicenseRef-` (per FR-013 — SPDX 2.3 doesn't define ExceptionRef-).

## Contract 4 — Integration at `rpm_file.rs:472-476`

**Pre-condition**: `normalized_license: Option<String>` from the milestone-478 normalizer is in scope.

**Behavior** (per research.md §R6):
```rust
let licenses: Vec<SpdxExpression> = normalized_license
    .as_deref()
    .and_then(|l| {
        SpdxExpression::try_canonical(l)
            .ok()
            .or_else(|| {
                preserve_known_operands_with_license_ref(l)
                    .and_then(|wrapped| SpdxExpression::try_canonical(&wrapped).ok())
            })
    })
    .into_iter()
    .collect();
```

**Post-conditions**:
- `licenses.len()` ∈ {0, 1}. Length 0 when both passes fail (NOASSERTION downstream). Length 1 when either pass succeeds.
- When the first pass succeeds: `licenses[0].as_str()` is byte-identical to pre-milestone-152 mikebom output for the same input. (SC-002 — happy-path regression guard.)
- When the first pass fails AND the second pass succeeds: `licenses[0].as_str()` carries the recovered compound expression with `LicenseRef-<sanitized>` wrappers for unrecognized operands. (SC-001 — the issue-#481 fix.)
- When both passes fail: `licenses` is empty → downstream emitters produce NOASSERTION (existing fail-closed behavior, Principle III).

## Contract 5 — Idempotency invariants (SC-003)

The following equality MUST hold for the milestone-152 code path:

```rust
// Given: any raw RPM License: header value `raw`
let first_pass = preserve_known_operands_with_license_ref(raw);
let second_pass = first_pass.as_deref().and_then(preserve_known_operands_with_license_ref);
assert_eq!(first_pass, second_pass);
```

Equivalently: the helper's output is a fixed point under self-composition. Tested via test #7 (`idempotent_on_already_wrapped_input`).

## What this contract DOES NOT change

- Public `mikebom-common::types::license::SpdxExpression` API (no method additions).
- `PackageDbEntry::licenses` field shape (still `Vec<SpdxExpression>`).
- `normalize_bitbake_license_operators` (untouched per FR-007).
- Any other reader (`deb_file.rs`, `apk_file.rs`, etc. — untouched per FR-009).
- Any format emitter (`generate/cyclonedx/`, `generate/spdx/`, `generate/spdx/v3_*` — untouched per FR-017 + SC-007).
- Any catalog row in `docs/reference/sbom-format-mapping.md` (untouched per FR-018 + SC-007).
- Any consumer-facing doc (untouched — CHANGELOG.md update only per SC-008).
