# Research — milestone 152

Phase 0 outputs for the RPM LicenseRef-fallback fix. Each section resolves a planning-time unknown by deciding what algorithm/API/approach the implementation will use.

## R1 — `spdx` crate API surface available for per-operand validation

**Decision**: Use `spdx::license_id(&str) -> Option<LicenseId>` and `spdx::exception_id(&str) -> Option<ExceptionId>` for per-operand validation. Both are crate-level free functions; both return `None` when the input isn't on the SPDX license/exception list. We do NOT use `spdx::Expression::parse_mode(..., ParseMode::LAX)` because LAX mode does not relax the unknown-identifier check — it only allows lowercase operators + slash-as-or + the small `IMPRECISE_NAMES` synonym table.

**Verified at planning time**:
- `spdx::Expression::parse(s)` calls `parse_mode(s, ParseMode::STRICT)`. Source: `~/.cargo/registry/src/.../spdx-0.10.9/src/expression.rs`.
- `spdx::ParseMode` has 3 boolean flags: `allow_lower_case_operators`, `allow_slash_as_or_operator`, `allow_imprecise_license_names`. None of these enable unknown-identifier acceptance.
- `spdx::imprecise_license_id(name)` walks the `IMPRECISE_NAMES` table (e.g., `GPLv2` → `GPL-2.0`). This is how `try_canonical` already recovers `GPLv2`.
- `spdx::Expression::iter()` walks the AST as `ExprNode` enums; `requirements()` yields per-operand `ExpressionReq` entries. **NOT USED** in this milestone because we can't parse the raw string into an `Expression` when it contains unknown operands.

**Rationale**: The hand-rolled tokenizer + per-operand validation via `license_id()` is simpler than trying to coerce the spdx crate's parser into accepting unknowns. The validation surface is small (license_id, exception_id) and the spdx crate's IMPRECISE_NAMES table already handles common synonyms — we benefit from it transitively by running our recombined output through `try_canonical` for final validation.

**Alternatives considered**:
- **Use `Expression::parse_mode` with LAX**: rejected — LAX mode doesn't accept unknown identifiers, just operator-syntax variants.
- **Patch the `spdx` crate to add an "accept-unknown" parse mode**: rejected — adds an upstream dependency and changes the cross-cutting validation contract for every other reader (deb, npm, etc.).
- **Use a third-party SPDX parser (e.g., `spdx-expression`)**: rejected — Constitution Principle I (pure Rust, no C) + research overhead. The `spdx` crate is already in the workspace + actively maintained.

## R2 — Tokenizer grammar for raw SPDX expressions

**Decision**: Hand-roll a small tokenizer (~30 LOC) that emits a `Vec<Token>` where:

```rust
enum Token<'a> {
    Operand(&'a str),  // borrowed slice of the raw input (a license id candidate)
    And,               // the literal "AND" keyword
    Or,                // the literal "OR" keyword
    With,              // the literal "WITH" keyword
    LParen,            // "("
    RParen,            // ")"
    Whitespace,        // " " / "\t" / "\n" (run-collapsed; one Token::Whitespace per run)
}
```

The tokenizer walks the input char-by-char, skipping whitespace runs (emitting one `Whitespace` per run), recognizing parens as standalone tokens, and consuming contiguous runs of operand-valid chars (anything that isn't whitespace or paren) as one operand. After lexing, it does a SECOND pass that re-classifies operand tokens equal to `"AND"` / `"OR"` / `"WITH"` (case-sensitive per SPDX strict spec) as operator tokens.

**Operand grammar**: anything that's not whitespace and not paren. Includes `+` (for "or later"), `:` (for `DocumentRef-...:LicenseRef-...`), `-`, `.`, alphanumerics. The grammar is INTENTIONALLY permissive because the tokenizer's job is decomposition, not validation — per-operand validation happens AFTER tokenization via `license_id()` / `exception_id()`.

**Rationale**: Hand-rolling avoids adding a parser combinator dep AND avoids invasive changes to the `spdx` crate. The grammar is regular (no nested structure beyond parens), so a single-pass char-by-char walk is sufficient. The two-pass design (lex first, then re-classify operators) keeps the tokenizer simple — it doesn't need lookahead to disambiguate operators from operands.

**Alternatives considered**:
- **`nom` parser combinator**: overkill for a regular grammar; new transitive dep.
- **Regex-based**: harder to handle parens correctly; the rebuild step is messier.
- **Treat WITH-clause as a single operand `"<license> WITH <exception>"`**: rejected — we need to validate the exception independently via `exception_id()` to detect unrecognized exceptions per FR-013.

**Edge cases the tokenizer handles**:
- `MIT` → `[Operand("MIT")]`
- `MIT OR Apache-2.0` → `[Operand("MIT"), Whitespace, Or, Whitespace, Operand("Apache-2.0")]`
- `(MIT OR Apache-2.0) AND PD` → `[LParen, Operand("MIT"), Whitespace, Or, Whitespace, Operand("Apache-2.0"), RParen, Whitespace, And, Whitespace, Operand("PD")]`
- `GPL-2.0-only WITH Classpath-exception-2.0` → `[Operand("GPL-2.0-only"), Whitespace, With, Whitespace, Operand("Classpath-exception-2.0")]`
- `LicenseRef-foo OR DocumentRef-bar:LicenseRef-baz` → `[Operand("LicenseRef-foo"), Whitespace, Or, Whitespace, Operand("DocumentRef-bar:LicenseRef-baz")]` (passes the colon through as operand-internal).

## R3 — WITH-clause detection algorithm

**Decision**: After tokenization, walk the token stream left-to-right. Maintain a single piece of state: `is_next_operand_an_exception: bool` (starts `false`). On each token:

- `Token::With` → set `is_next_operand_an_exception = true`.
- `Token::Operand(s)` →
  - If `is_next_operand_an_exception` is `true`: validate via `spdx::exception_id(s)`. If `Some(_)` → keep the operand unchanged. If `None` → return `None` from the helper (whole-compound NOASSERTION per FR-013 + Q2). Reset `is_next_operand_an_exception = false`.
  - Else: validate via the license-operand rule (R4 below).
- Any other token → leave `is_next_operand_an_exception` unchanged.

**Rationale**: WITH is a binary infix operator in SPDX. The right-hand side is always an exception identifier — never a parenthesized sub-expression, never another compound. So a one-token lookahead via the state flag is sufficient.

**Edge case — `(LICENSE WITH EXCEPTION) AND ...`**: parens around a WITH-clause are valid SPDX. The tokenizer emits `LParen, Operand, Whitespace, With, Whitespace, Operand, RParen, ...`. The state machine still correctly identifies the second operand as the exception.

**Edge case — `LICENSE WITH (EXCEPTION)`**: technically invalid per SPDX 2.3 grammar (exception MUST be a bare identifier, not a parenthesized sub-expression). The state machine would set `is_next_operand_an_exception = true` then encounter `LParen` instead of an operand; reset the flag (no validation triggered) and the recombined expression would fail `try_canonical` anyway. **Decision**: don't special-case this — let the final `try_canonical` fail-through handle it.

**Alternatives considered**:
- **Recursive descent parser that builds an AST**: overkill for one operator's special-casing.
- **Treat WITH as a license-side operand modifier**: doesn't compose with SPDX's parens grammar.

## R4 — Per-operand license validation rule

**Decision**: For each `Token::Operand(s)` that is NOT in WITH-exception position, classify as one of:

1. **Already a LicenseRef / DocumentRef** → if `s.starts_with("LicenseRef-")` OR `s.starts_with("DocumentRef-")`, pass through unchanged. (Idempotency guarantee per FR-006.)
2. **Recognized SPDX license id** → if `spdx::license_id(s)` returns `Some(_)`, pass through unchanged. The spdx crate handles `+`-suffixed ids (e.g., `Apache-2.0+`) per its internal grammar.
3. **Recognized via imprecise lookup** → if `spdx::imprecise_license_id(s)` returns `Some((id, _))`, pass through unchanged (the final `try_canonical` will canonicalize via the same lookup). E.g., `GPLv2` → `GPL-2.0-only` happens at the `try_canonical` stage, not in this helper.
4. **Otherwise** → replace with `LicenseRef-<sanitize(s)>` per R5.

**Rationale**: The classification order matters. Step 1 prevents double-wrapping `LicenseRef-` tokens. Step 2 preserves bare SPDX ids (most common). Step 3 catches imprecise synonyms (e.g., `GPLv2`) — passing them through unchanged lets the final `try_canonical` canonicalize via the same lookup, producing `GPL-2.0-only`. Step 4 is the LicenseRef escape hatch for genuinely unknown tokens.

**Verified at planning time**: `spdx::license_id("Apache-2.0+")` returns `Some(LicenseId)` because the `+` suffix is handled inside `license_id`. `spdx::license_id("bzip2-1.0.4")` returns `None`. `spdx::imprecise_license_id("GPLv2")` returns `Some((GPL-2.0, 5))`.

**Alternatives considered**:
- **Skip step 3 (imprecise lookup)**: would treat `GPLv2` as an unknown operand and wrap it as `LicenseRef-GPLv2`, losing the imprecise-→-canonical recovery. Real cost: e.g., `GPLv2 & bzip2-1.0.4` would yield `LicenseRef-GPLv2 AND LicenseRef-bzip2-1.0.4` instead of `GPL-2.0-only AND LicenseRef-bzip2-1.0.4`. Skipping imprecise lookup loses signal.
- **Try `try_canonical` on each operand individually**: more expensive (full parser invocation per operand); functionally equivalent to step 2 for bare identifiers.

## R5 — Sanitization algorithm (per Clarifications Q1)

**Decision**: Implement `sanitize_to_license_ref_idstring(s: &str) -> Option<String>` with the replace+collapse+strip algorithm:

```rust
fn sanitize_to_license_ref_idstring(s: &str) -> Option<String> {
    // Replace each char outside [a-zA-Z0-9-.] with '-'.
    // Collapse consecutive '-' to single '-'.
    // Strip leading/trailing '-'.
    let mut out = String::with_capacity(s.len());
    let mut prev_was_dash = false;
    for c in s.chars() {
        let safe = c.is_ascii_alphanumeric() || c == '-' || c == '.';
        let emit = if safe { c } else { '-' };
        if emit == '-' {
            if !prev_was_dash {
                out.push('-');
                prev_was_dash = true;
            }
            // else: skip — collapses run of dashes to one
        } else {
            out.push(emit);
            prev_was_dash = false;
        }
    }
    // Strip leading/trailing dashes.
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        None  // sanitization stripped to nothing → no LicenseRef can be formed
    } else {
        Some(trimmed.to_string())
    }
}
```

**Rationale**: Matches the Q1 strawman exactly. Idempotent (running the algorithm twice produces the same output). Pure ASCII (non-ASCII chars in input get replaced with `-`, which is the conservative safe-side default). Returns `Option<String>` so the caller can distinguish "sanitized to a valid idstring" from "sanitized to empty" (e.g., input `"!@#$"` → all chars get replaced, all are run-collapsed, all are stripped → `None`).

**Worked examples** (per FR-002 doc-comment requirement):
- `GPLv2+` → `GPLv2-` → `GPLv2-` (collapse no-op) → `GPLv2` (strip trailing)
- `My License v2` → `My-License-v2` → (no consecutive dashes) → `My-License-v2`
- `(custom)` → `-custom-` → (no consecutive dashes) → `custom` (strip both sides)
- `LGPL-2.1+` → `LGPL-2.1-` → (no consecutive dashes) → `LGPL-2.1` (strip trailing)
- `bzip2-1.0.4` → `bzip2-1.0.4` (unchanged — already valid)
- `PD` → `PD` (unchanged)
- `!@#$` → `----` → `-` (collapsed) → empty (stripped) → returns `None`

**Idempotency check**:
- `sanitize("LGPL-2.1+")` → `"LGPL-2.1"` → `sanitize("LGPL-2.1")` → `"LGPL-2.1"` ✓
- `sanitize("My-License-v2")` → `"My-License-v2"` → `sanitize("My-License-v2")` → `"My-License-v2"` ✓

**Alternatives considered**:
- **Encode `+` as `-plus`**: rejected — Q1 chose Option A (lossy replace), not Option C (mnemonic encoding).
- **Drop invalid chars**: rejected — Q1 chose Option A.
- **Base32-encode the raw operand**: rejected — Q1 chose Option A (machine-readable trumps reversibility for this milestone).

## R6 — Pipeline ordering + integration site

**Decision**: Wrap the new fallback in a `.or_else(|| ...)` closure at the existing call site (`rpm_file.rs:472–476`). Pipeline ordering per FR-007:

```rust
// Existing milestone-478 normalization (UNCHANGED):
let normalized_license = license_str
    .as_deref()
    .map(normalize_bitbake_license_operators);

let licenses: Vec<SpdxExpression> = normalized_license
    .as_deref()
    .and_then(|l| {
        // First-pass try_canonical (UNCHANGED happy path):
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

**Rationale**: The two passes compose without affecting each other. The first pass preserves milestone-150 + earlier behavior byte-identically for every component whose expression was already fully canonicalizable (SC-002 safeguard). The second pass activates ONLY on first-pass failure. The final `try_canonical` call validates the LicenseRef-wrapped expression — if the wrapper somehow produces invalid SPDX (e.g., due to a bug in the WITH-clause handling), it falls through to NOASSERTION, preserving the fail-closed posture (Principle III).

**Alternatives considered**:
- **Move the fallback into `SpdxExpression::try_canonical` itself** (in `mikebom-common`): rejected — per FR-009 this milestone is RPM-only. The fallback affects only the RPM reader's call site. Moving it to common would change behavior for every reader (deb, npm, etc.) — out of scope.
- **Make the fallback opt-in via a new CLI flag**: rejected — the behavior is a strict improvement (replaces NOASSERTION with structured info). Per the Out of Scope section, no CLI flag.

## R7 — Test fixture strategy (per SC-006)

**Decision**: Add ≥8 new unit tests in `rpm_file.rs#[cfg(test)] mod tests`, all using synthetic inline license strings (no fixture file additions, no sibling-fixture-repo touches). Mirrors the milestone-478 + #475 test pattern at `rpm_file.rs:1061+`. Test inventory:

| # | Test name | Covers | Input → Expected output |
|---|-----------|--------|-------------------------|
| 1 | `preserve_busybox_compound` | SC-001 + FR-001 | `GPLv2 & bzip2-1.0.4` → `GPL-2.0-only AND LicenseRef-bzip2-1.0.4` |
| 2 | `preserve_liblzma5_single_unknown` | SC-001 + FR-004 | `PD` → `LicenseRef-PD` |
| 3 | `preserve_or_operator` | US1 scenario 3 | `GPLv2 | bzip2-1.0.4` → `GPL-2.0-only OR LicenseRef-bzip2-1.0.4` |
| 4 | `happy_path_unchanged_for_fully_recognized` | SC-002 + FR-003 | `GPLv2 & LGPLv2.1+` → `GPL-2.0-only AND LGPL-2.1-or-later` (first-pass succeeds; fallback never fires) |
| 5 | `empty_input_remains_noassertion` | FR-008 + US1 scenario 5 | `""` / `"   "` → no SpdxExpression produced (NOASSERTION downstream) |
| 6 | `opaque_garbage_remains_noassertion` | FR-008 + US1 scenario 6 | `"!@#$"` → no SpdxExpression produced (sanitization strips to empty; final try_canonical fails) |
| 7 | `idempotent_on_already_wrapped_input` | SC-003 + FR-006 | `GPL-2.0-only AND LicenseRef-bzip2-1.0.4` → unchanged (LicenseRef detection passes through) |
| 8 | `sanitization_worked_examples` | FR-002 | parametric: each (raw, expected) pair from R5's worked-examples table |
| 9 | `with_clause_known_exception_preserved` | FR-013 + US1 baseline | `GPL-2.0-or-later WITH Classpath-exception-2.0` → unchanged (happy path) |
| 10 | `with_clause_unknown_exception_collapses_to_noassertion` | FR-013 + Q2 | `GPL-2.0-only WITH UnknownExc AND MIT` → no SpdxExpression produced (whole compound → NOASSERTION) |
| 11 | `parens_preserved_through_fallback` | edge case | `(GPLv2 OR LGPLv2.1+) AND PD` → `(GPL-2.0-only OR LGPL-2.1-or-later) AND LicenseRef-PD` |
| 12 | `imprecise_synonym_canonicalized_not_wrapped` | R4 step 3 | `GPLv2` alone → `GPL-2.0-only` (first-pass try_canonical handles it via imprecise_license_id; fallback never fires) |

Final count: 12 tests (exceeds SC-006's floor of ≥8). Each test is independent (no fixture cross-contamination per SC-006).

**Rationale**: Inline synthetic strings keep the test suite self-contained — no fixture-repo sync required, no per-test SBOM scan needed. The 12 tests cover every FR + every Clarifications-derived edge case + the 2 issue-#481 reference cases.

**Alternatives considered**:
- **Add a Yocto RPM fixture to the sibling repo**: rejected for this milestone — adding real RPMs would balloon the fixture repo size and the unit-level coverage from inline strings is sufficient. The end-to-end SC-001 verification is a manual operator-cadence check against the maintainer's local `yocto-test/` testbed (Assumption 3 + Quickstart Scenario 1).
- **Property-based tests via `proptest`**: rejected — new dev-dep + verification overhead. The 12 hand-picked tests cover the relevant cases concretely.

## R8 — Verification against the issue-#481 testbed

**Decision**: The SC-001 verification is a MANUAL operator-cadence check, mirroring the milestone-478 pattern. The maintainer (mike@kusari.dev) has the testbed locally at `yocto-test/` and will:

1. Build mikebom at the milestone-152 head: `cargo +stable build --release -p mikebom`.
2. Re-run the same scan command the issue #481 documents against `core-image-minimal` qemux86-64 scarthgap LTS poky `802e4c1`.
3. Inspect the emitted SPDX 2.3 SBOM and confirm the 5 affected packages now emit non-NOASSERTION `licenseDeclared` per the SC-001 expected strings.
4. Report PASS / FAIL in the milestone-152 PR description; close issue #481 on merge.

**Rationale**: The issue-#481 testbed is not in the milestone-090 sibling-fixture repo (it's local to the maintainer's machine). Adding it would require synthesizing RPM artifacts in CI, which is out of scope per the spec's Assumption 3. The manual operator-cadence pattern is the established convention for milestones #478, #150, #149, etc.

**Alternatives considered**:
- **Add an integration test that builds a synthetic RPM at test time**: would require the `rpmbuild` toolchain in CI; Linux-only; adds complexity for marginal value. Rejected.
- **Mock the RPM-header reading directly**: feasible but moves the test farther from end-to-end behavior; the unit-test coverage in R7 already exercises the LICENSE-string transformation in isolation. Rejected as redundant.

## R9 — CHANGELOG.md format + placement

**Decision**: Add a single-line entry under `## [Unreleased]` in `CHANGELOG.md`. Inspect the file first to match the existing convention.

Inspection at planning time (via `head -50 CHANGELOG.md`):
- The repo follows a standard Keep-a-Changelog format with `## [Unreleased]` and per-release `## [v0.1.0-alpha.NN] — YYYY-MM-DD` sections.
- Under each, subsections are `### Added`, `### Changed`, `### Fixed`, `### Deprecated`, `### Removed`.

Milestone 152's entry goes under `### Fixed` (since it closes a bug, #481):

```markdown
- `sbom scan`: RPM license expressions with unrecognized operands now preserve the
  recognized portion via SPDX 2.3 `LicenseRef-<sanitized>` escape hatches instead of
  collapsing to `NOASSERTION`. Sanitization rule: replace each char outside
  `[a-zA-Z0-9-.]` with `-`, collapse consecutive `-` to single, strip leading/
  trailing `-`. Worked examples: `GPLv2+` → `LicenseRef-GPLv2`,
  `bzip2-1.0.4` → `LicenseRef-bzip2-1.0.4`, `My License v2` →
  `LicenseRef-My-License-v2`. Closes issue #481 (5/35 Yocto-built
  `core-image-minimal` packages affected).
```

**Rationale**: Matches the existing CHANGELOG convention. The entry is consumer-relevant (downstream license-policy filters that pattern-match on `NOASSERTION` should now match against `LicenseRef-*` for these packages). The sanitization rule is documented inline so consumers can apply the inverse heuristically.

## R10 — Cross-format wire-shape verification

**Decision**: No cross-format work required. The `licenses: Vec<SpdxExpression>` field on `PackageDbEntry` is consumed by all three format emitters (CDX 1.6 / SPDX 2.3 / SPDX 3.0.1) via shared canonicalization logic. The LicenseRef-wrapped output flows through unchanged.

**Verified at planning time**: Grep `mikebom-cli/src/generate/` for `licenses` shows that all three emitters use the same `SpdxExpression::as_str()` accessor to serialize the canonical form. The wire shapes differ per format (CDX `license.expression`, SPDX 2.3 `licenseDeclared`, SPDX 3 `software_packageLicenseDeclared`) but the STRING content is the same `SpdxExpression` value across all three.

**Alternative considered**:
- **Add per-format unit tests confirming the LicenseRef value survives serialization**: probably overkill for this milestone (SC-002's byte-identity test on the happy path is sufficient regression guard; the failure path is straightforward `String` propagation). Defer to ad-hoc verification during the operator-cadence SC-001 check.
