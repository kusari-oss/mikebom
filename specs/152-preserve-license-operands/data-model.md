# Data model — milestone 152

This milestone adds 3 internal-to-`rpm_file.rs` types/functions; no public `mikebom-common` API surface changes; no Cargo.toml changes.

## §1 — `Token` enum (internal to `rpm_file.rs`)

The tokenizer's output shape per research.md §R2.

```rust
#[derive(Debug, Clone, PartialEq)]
enum Token<'a> {
    /// A license-id-candidate string (anything that's not whitespace
    /// and not paren). May be:
    ///   - A bare SPDX license id (e.g., "MIT", "Apache-2.0+")
    ///   - An imprecise synonym (e.g., "GPLv2") that
    ///     `spdx::imprecise_license_id` recognizes
    ///   - An already-LicenseRef-prefixed id (e.g., "LicenseRef-foo")
    ///   - A DocumentRef-prefixed id (e.g., "DocumentRef-doc:LicenseRef-foo")
    ///   - A genuinely unknown token (e.g., "PD", "bzip2-1.0.4") that
    ///     the helper wraps as LicenseRef-<sanitized>
    Operand(&'a str),
    /// The literal "AND" keyword (case-sensitive per SPDX 2.3 strict).
    And,
    /// The literal "OR" keyword.
    Or,
    /// The literal "WITH" keyword. The operand immediately following a
    /// WITH token is treated as an exception identifier (validated via
    /// `spdx::exception_id`) per FR-013 + Clarifications Q2.
    With,
    /// "(" — opens a sub-expression grouping.
    LParen,
    /// ")" — closes a sub-expression grouping.
    RParen,
    /// One or more contiguous whitespace chars (collapsed to a single
    /// Token). Emitted to preserve operator-spacing during the rebuild
    /// step; not load-bearing for parsing (could be omitted from the
    /// token stream and re-inserted during rebuild, but emitting them
    /// keeps the rebuild logic trivial).
    Whitespace,
}
```

Validation rules:
- The tokenizer emits operands BEFORE the operator re-classification pass. After lexing, any `Token::Operand(s)` where `s == "AND" | "OR" | "WITH"` (case-sensitive) is re-classified to the corresponding operator variant. This two-pass design keeps the lexer single-character lookahead.
- Empty input → empty `Vec<Token>` → the helper returns `None` (caller falls through to NOASSERTION).
- Whitespace-only input → `Vec<Token>` containing only `Token::Whitespace` → the helper returns `None`.

## §2 — `sanitize_to_license_ref_idstring` (internal helper)

Pure function per research.md §R5.

```rust
/// Sanitize an unrecognized operand to a SPDX 2.3 LicenseRef idstring
/// matching the grammar `[a-zA-Z0-9-.]+`.
///
/// Algorithm (per Clarifications Q1 — replace + collapse + strip):
///   1. Replace each char outside `[a-zA-Z0-9-.]` with `-`.
///   2. Collapse runs of consecutive `-` to a single `-`.
///   3. Strip leading and trailing `-`.
///
/// Returns `None` when the algorithm produces an empty string (i.e.,
/// the input contained no valid chars).
///
/// Idempotent: `sanitize(sanitize(s)) == sanitize(s)` for any input
/// where the first call returns `Some(_)`.
///
/// Worked examples:
/// | Input             | Output                |
/// |-------------------|-----------------------|
/// | `"GPLv2+"`        | `Some("GPLv2")`       |
/// | `"My License v2"` | `Some("My-License-v2")` |
/// | `"(custom)"`      | `Some("custom")`      |
/// | `"LGPL-2.1+"`     | `Some("LGPL-2.1")`    |
/// | `"bzip2-1.0.4"`   | `Some("bzip2-1.0.4")` |
/// | `"PD"`            | `Some("PD")`          |
/// | `"!@#$"`          | `None`                |
fn sanitize_to_license_ref_idstring(s: &str) -> Option<String>
```

## §3 — `preserve_known_operands_with_license_ref` (the main helper)

Pure function per research.md §R6.

```rust
/// Wrap each unrecognized operand in a compound SPDX expression as
/// `LicenseRef-<sanitized>` to preserve the recognized portion when
/// `SpdxExpression::try_canonical` fails on the raw expression.
///
/// Activated as the second pass in the pipeline:
///   raw -> normalize_bitbake_license_operators -> try_canonical
///     (on failure) -> preserve_known_operands_with_license_ref
///                  -> try_canonical (final validation)
///     (on failure) -> NOASSERTION (existing fail-closed behavior)
///
/// Returns `None` when:
///   - Input is empty or whitespace-only (FR-008).
///   - The input contains a WITH-clause whose exception is unrecognized
///     (FR-013 + Clarifications Q2 — whole compound collapses to
///     NOASSERTION via the caller falling through).
///   - Sanitization produces an empty idstring for some operand (e.g.,
///     all chars were invalid + got stripped).
///
/// Returns `Some(rebuilt)` when:
///   - All recognized operands pass through unchanged.
///   - All unrecognized operands get wrapped as `LicenseRef-<sanitized>`.
///   - All WITH-clause exceptions are recognized (passes through unchanged).
///   - The rebuilt string is then run through `SpdxExpression::try_canonical`
///     by the caller (line 474+ at rpm_file.rs) to produce the final
///     `SpdxExpression`.
///
/// Note: this helper does NOT call `try_canonical` itself — the caller
/// performs the final canonicalization so a single Result-handling site
/// covers both passes' failure modes.
///
/// Idempotent: feeding wrapped output back in produces the same output
/// (LicenseRef-/DocumentRef-prefixed tokens are detected and passed
/// through unchanged per FR-006).
fn preserve_known_operands_with_license_ref(raw: &str) -> Option<String>
```

Internal algorithm (pseudo-code; matches research.md §R3 + §R4):

```text
1. tokens = tokenize(raw)
2. if tokens.is_empty() OR tokens.iter().all(|t| matches!(t, Token::Whitespace)):
       return None
3. is_next_operand_an_exception = false
4. for token in tokens:
       match token:
           Token::With => is_next_operand_an_exception = true
           Token::Operand(s) =>
               if is_next_operand_an_exception:
                   if spdx::exception_id(s).is_none():
                       return None  // FR-013 + Q2 whole-compound NOASSERTION
                   is_next_operand_an_exception = false
                   // else: pass through unchanged
               else:
                   // R4 classification:
                   if s.starts_with("LicenseRef-") || s.starts_with("DocumentRef-"):
                       pass_through
                   else if spdx::license_id(s).is_some():
                       pass_through
                   else if spdx::imprecise_license_id(s).is_some():
                       pass_through  // try_canonical will resolve it
                   else:
                       sanitized = sanitize_to_license_ref_idstring(s)?
                       replace token with Operand("LicenseRef-<sanitized>")
           _ => (no-op for operators / parens / whitespace)
5. rebuild string from tokens (each Token serializes to its source form;
   replaced operands serialize as the new LicenseRef string).
6. return Some(rebuilt)
```

**Per-token serialization rule** (per analysis remediation A2 — settles the whitespace question for §1's `Token::Whitespace`):

| Token | Rebuild-time serialization |
|-------|---------------------------|
| `Operand(s)` | the borrowed slice `s` verbatim (or the new `LicenseRef-<sanitized>` string when wrapped per R4 step 4) |
| `And` / `Or` / `With` | the literal uppercase keyword `"AND"` / `"OR"` / `"WITH"` |
| `LParen` / `RParen` | the literal `"("` / `")"` |
| `Whitespace` | a single space `" "` (collapses arbitrary input whitespace runs to canonical single-space) |

The final `SpdxExpression::try_canonical(&rebuilt)` call accepts single-space-separated input and may further canonicalize the output (e.g., normalizing operator order in homogeneous chains per milestone 146 dedup). Consumers should not depend on EXACT preservation of input whitespace through the second-pass — only structural correctness.

## §4 — Tokenizer (internal)

```rust
/// Tokenize a raw SPDX-like expression string into a flat Vec<Token>.
/// Hand-rolled per research.md §R2 grammar — single-pass char walk +
/// second pass to re-classify "AND"/"OR"/"WITH" operands as operators.
///
/// The tokenizer is INTENTIONALLY permissive: it does NOT validate
/// operand strings, only structurally decomposes the input. Per-operand
/// validation happens in `preserve_known_operands_with_license_ref`.
fn tokenize(raw: &str) -> Vec<Token<'_>>
```

The tokenizer + the main helper are co-located in `rpm_file.rs` (per FR-009 scope guard).

## §5 — Pipeline diff at `rpm_file.rs:472-476`

The integration site changes from:

```rust
let licenses: Vec<SpdxExpression> = normalized_license
    .as_deref()
    .and_then(|l| SpdxExpression::try_canonical(l).ok())
    .into_iter()
    .collect();
```

to:

```rust
let licenses: Vec<SpdxExpression> = normalized_license
    .as_deref()
    .and_then(|l| {
        // First-pass try_canonical (unchanged happy path):
        SpdxExpression::try_canonical(l)
            .ok()
            // Second-pass LicenseRef fallback (NEW per milestone 152):
            .or_else(|| {
                preserve_known_operands_with_license_ref(l)
                    .and_then(|wrapped| SpdxExpression::try_canonical(&wrapped).ok())
            })
    })
    .into_iter()
    .collect();
```

Net delta at the call site: 4 lines → 9 lines. The change is purely additive (adds the `.or_else(...)` block); first-pass behavior is byte-identical.

## §6 — Idempotency invariants (per SC-003 + FR-006)

The following invariants MUST hold:

1. `sanitize_to_license_ref_idstring(sanitize_to_license_ref_idstring(s).unwrap_or_default())` ≡ `sanitize_to_license_ref_idstring(s)` for any `s` where the first call returns `Some(_)`. Tested via test #8 (`sanitization_worked_examples`).

2. `preserve_known_operands_with_license_ref(preserve_known_operands_with_license_ref(raw).unwrap_or_default())` ≡ `preserve_known_operands_with_license_ref(raw)` for any `raw` where the first call returns `Some(_)`. Tested via test #7 (`idempotent_on_already_wrapped_input`).

3. Tokens already starting with `LicenseRef-` or `DocumentRef-` pass through R4 step 1 unchanged. Tested via test #7.

## §7 — Test inventory (per research.md §R7 + analysis remediations C1 + C2)

14 unit tests inline in `rpm_file.rs#[cfg(test)] mod tests` (12 from initial R7 plan + 2 added per analysis remediations C1 + C2). Each test independent (no shared mutable state); each uses a synthetic license string + asserts the resulting `SpdxExpression::as_str()` (or `None` for the no-match cases). Final naming convention matches the existing milestone-478 + #475 test pattern at `rpm_file.rs:1061+`.

Test #13 + #14 (added per analysis):

| # | Test name | Covers | Input → Expected output |
|---|-----------|--------|-------------------------|
| 13 | `with_clause_unknown_license_wrapped` | FR-013 first clause (C1 closes the test gap) | `UnknownLicense WITH Classpath-exception-2.0` → `LicenseRef-UnknownLicense WITH Classpath-exception-2.0` (unrecognized LEFT side wrapped; recognized exception preserved) |
| 14 | `mixed_precedence_preserved` | FR-005 implicit operator precedence (C2 closes the test gap) | `MIT OR PD AND GPLv2` → `MIT OR LicenseRef-PD AND GPL-2.0-only` (or canonical equivalent — SPDX precedence `AND` > `OR` preserved without explicit parens) |

Plus 2 tokenizer tests from T006: `tokenize_simple_compound` + `tokenize_with_parens_and_whitespace`. Grand total: **16 tests** (14 helper-validation tests + 2 tokenizer tests). SC-006 floor of ≥8 satisfied with comfortable margin.

## §8 — CHANGELOG.md entry shape

Single `### Fixed` bullet under `## [Unreleased]` per research.md §R9. Content tracked in research.md §R9 verbatim.
