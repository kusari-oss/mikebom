# Research: Operator-supplied PURL alias for cross-tier binding

**Feature**: 111-pkg-alias-binding
**Date**: 2026-06-09
**Status**: Complete

Five research items, each resulting in a Decision + Rationale + Alternatives Considered block.

---

## §1 Standards-native audit (Principle V mandatory)

**Question**: Do CycloneDX 1.6 or SPDX 2.3 / 3.0.1 provide a native construct for "this binary-tier component is cross-tier-bound to a source-tier component identified by a different PURL"? Per Constitution Principle V, every new `mikebom:*` property requires a documented audit of the target formats showing no native equivalent exists.

**Decision**: No native construct exists in any of the three target formats. The existing milestone-072 `SourceDocumentBinding` envelope (carried as a `mikebom:binding-result-v1` property in CDX 1.6 and via the `MikebomAnnotationCommentV1` envelope in SPDX 2.3 + SPDX 3) is the established parity-bridging mechanism for cross-tier binding semantics. This milestone extends that envelope additively (two optional fields `alias_from` / `alias_to`); no new top-level property is introduced.

**Rationale**:
- **CDX 1.6 cross-doc reference**: `bom-link` URN format references EXTERNAL SBOM documents but doesn't describe a "this PURL was rewritten" relationship. `dependencies[]` describes dependency relationships WITHIN a single BOM. `components[].properties[]` is the catch-all key/value carrier — which is what milestone 072 already uses.
- **CDX 1.6 `declarations.claims[]`**: an operator-asserted claim — could in principle carry "I claim component X corresponds to component Y" — but `declarations` is scoped to compliance attestations (BSI, EU CRA-style), not provenance rewrites. Using it would be a semantic stretch and would create a new emission surface area for marginal gain.
- **SPDX 2.3**: `Relationships` graph supports many predicate types (`DESCRIBES`, `CONTAINS`, `DEPENDS_ON`, `VARIANT_OF`, `OTHER`) but none semantically express cross-tier binding. The closest, `VARIANT_OF`, describes a single-package variant relationship within one SBOM. Adding a `Relationship` between an image-tier Package and a source-tier Package across SBOM boundaries via `ExternalDocumentRef` is technically expressible but would require operators to author the relationship structure manually — exactly the verbose-CDX problem the tool-survey thread (issue #326 context) flagged as hostile.
- **SPDX 3.0.1**: `ExternalMap` provides cross-document Element references with `verifiedUsing` integrity. Closer to fit-for-purpose than SPDX 2.3, but still not a "rewrite alias" semantic — it's a content-addressable reference. SPDX 3 also has no native "binding strength" enum, so the milestone-072 envelope already does the cross-format bridging work.
- **Confirmation by absence in OSS practice**: a literature survey across the SBOM-author tooling ecosystem (cyclonedx-cli, syft, sbom-tool, GUAC, ClearlyDefined, in-toto, CMake-SBOM-Builder — full results in issue #326's three-thread research) surfaced zero tools that emit a "this PURL was rewritten for binding match" annotation in standards-native form. Every cross-tier-binding tool that exists today (mikebom milestone 072, in-toto VSA, GUAC's `HasSBOM`) uses tool-specific envelopes.

The existing `docs/reference/sbom-format-mapping.md` C56 row already documents the milestone-072 envelope's parity-bridging justification. This milestone amends the C56 row with a note that the envelope can carry the additive alias fields; no new C-row is needed.

**Alternatives considered**:
- **New top-level CDX `properties[]` entry**: `mikebom:pkg-alias-from` + `mikebom:pkg-alias-to` as two separate properties on the affected component. Rejected per Principle V — the envelope's `SourceDocumentBinding` already covers cross-tier-binding-result semantics, so adding sibling properties would split a logically-single concept across two emission surfaces. Verify-binding would have to read from two places.
- **CDX 1.6 `declarations.claims[]`**: a claim per affected component asserting the alias. Rejected — semantically wrong (claims are compliance attestations) and ergonomically heavier (operators rarely interact with the declarations surface).
- **SPDX 2.3 `Relationship` with `relationshipType: OTHER` + comment**: would require manual cross-document reference setup. Rejected as too verbose for a binding-time concept and not parity-bridgeable to CDX without an additional envelope anyway.

---

## §2 Envelope wire-compatibility

**Question**: When adding `alias_from` / `alias_to` fields to the existing `SourceDocumentBinding` struct, do we bump the envelope's `algo` field from `v1` to `v2`, or treat the additive change as pure backward-compatible extension?

**Decision**: Pure additive extension — no `algo` bump. Use `#[serde(default, skip_serializing_if = "Option::is_none")]` on the two new `Option<Purl>` fields. When the operator does NOT supply `--pkg-alias`, the fields are `None` and serde omits them from emission, producing byte-identical output to the pre-feature baseline (satisfies SC-004 directly). When the operator DOES supply `--pkg-alias`, the fields serialize as additional keys inside the existing envelope; pre-feature consumers' serde-derived deserializers ignore unknown fields by default and parse the envelope successfully.

**Rationale**:
- The existing `SourceDocumentBinding` already uses `#[serde(default, skip_serializing_if = "Option::is_none")]` on its `hash` and `reason` fields (verified by reading `binding/mod.rs:189-196`). The two new fields follow the same pattern, so the wire shape is consistent.
- The `algo` field's documented contract (`binding/mod.rs:197`: "Always `\"v1\"` for milestone 072") is a version tag for the LAYERED-HASH algorithm, not for the envelope's field set. This milestone does NOT change the hash algorithm; the only change is two metadata fields that don't participate in hash computation. Bumping `algo` would falsely signal an algorithm change.
- Serde's standard behavior (ignore-unknown-fields on `#[derive(Deserialize)]` without `deny_unknown_fields`) makes the additive change safe for pre-feature consumers. The existing envelope struct does NOT carry `deny_unknown_fields` (verified by reading `binding/mod.rs:184`), so this safety property holds.
- Byte-identity for the no-alias case is the load-bearing regression guard. The `skip_serializing_if` directive makes this property mechanical rather than convention-based.

**Alternatives considered**:
- **Bump to `algo: "v2"`**: would force every pre-feature SBOM consumer (including milestone-072 verify-binding) to either accept both versions or fail. Rejected — gratuitous churn.
- **New envelope type `SourceDocumentBindingV2`**: would require a new property name (`mikebom:binding-result-v2`) and dual-emission compatibility. Rejected — over-engineering for an additive change.
- **Embedded sub-struct**: `binding.alias: Option<AliasSpec> { from: Purl, to: Purl }` — slightly cleaner nesting but adds a serialization layer and a parity-extractor row for the sub-struct. Rejected — flat `alias_from` / `alias_to` is simpler and matches the existing flat envelope shape.

---

## §3 Match semantics (canonical equality)

**Question**: When comparing the alias LHS PURL against scan-output component PURLs, what equality model is used?

**Decision**: Strict equality on canonical PURL form. Both sides are run through `Purl::canonical()` (the milestone-005 normalization: lowercase scheme, lowercase host, sorted qualifiers, percent-encoding normalization) and string-compared. No partial / name-only / version-pattern matches. This decision was locked at /speckit-clarify Q1; this section captures the implementation primitive.

**Rationale**:
- `Purl` is already a newtype in `mikebom-common` (per Constitution Principle IV and milestone 005 — workspace contract). The newtype's `canonical()` method exists and is the comparison primitive used elsewhere in the codebase for PURL equality (e.g., in milestone-072's component matching at `binding/verify.rs:431+`).
- Operators ARE in control of the input — they author the LHS knowing the scan-output PURL shape. The issue #225 problem statement specifically references `pkg:generic/baz` as the scan-output shape; operators declare exactly that. Future evolution (e.g., milestone 096 emitting versioned generic PURLs) requires operators to update their aliases — an explicit-cost change, not a silent breakage.
- Strict equality is the lowest-surprise default. Pattern matching (name-only, wildcard) introduces collision risk: `pkg:generic/log` (Rust crate? Unix log? something else?) shouldn't silently bind to a randomly-named source-tier `log` component.

**Alternatives considered**: 4 options surveyed at /speckit-clarify Q1 (strict / name-only / name+version / bare-form); strict equality chosen.

---

## §4 Conflict resolution (same-LHS-different-RHS rejected at parse time)

**Question**: When the operator declares two `--pkg-alias` flags with the same LHS but different RHS values, how does mikebom respond?

**Decision**: Reject at CLI parse time with an actionable error message naming both conflicting declarations. Mikebom exits with non-zero status before any scan work begins. No scan runs with partial or first-wins semantics.

**Rationale**:
- Fail-closed posture (Principle III) — silent precedence would silently distort binding results, exactly the failure mode milestone 072's `unknown { reason }` transparency was designed to avoid. Operator typos should surface as immediate failures, not as wrong-component bindings discovered hours later.
- The CLI-parse-time check is cheap (linear in N_aliases, executed before any I/O). Implementation: collect aliases into a `HashMap<Purl, Purl>` keyed by LHS; on insert, check for an existing entry with a different RHS and emit the error.
- Same-RHS-multiple-LHS is NOT rejected (per Edge Cases) — multiple image-tier PURLs may legitimately bind to one source-tier PURL (e.g., a single source crate produces two distinct binaries that the binary scanner names differently). This is operator intent, not error.

**Alternatives considered**:
- **First-declaration-wins with a warning**: rejected — silent override violates fail-closed.
- **Last-declaration-wins**: same rejection rationale.
- **Reject at binding-time instead of CLI-parse-time**: rejected — wastes scan work and surfaces the error later than necessary.

---

## §5 CLI ergonomics — reuse the milestone-073 `LHS=RHS` parser pattern

**Question**: How is the `LHS=RHS` flag value parsed at the clap layer? Does a similar parser exist in the codebase already?

**Decision**: Reuse the parser pattern from milestone-073's `--component-id KEY=VALUE` flag (located in `mikebom-cli/src/cli/scan_cmd.rs:398` per the issue #225 references). The pattern is: a clap `value_parser` function `parse_pkg_alias(raw: &str) -> Result<PurlAlias, String>` that splits on `=`, validates both sides as canonical `Purl`s via `Purl::parse_canonical()`, and returns the typed result. Errors map to clap's stderr surface with the offending input shown verbatim. The env-var form (`MIKEBOM_PKG_ALIAS`) uses `,` as entry separator (matching milestone-110's `MIKEBOM_FINGERPRINTS_SOURCES`) and re-uses the per-entry parser.

**Rationale**:
- Consistency with milestone-073 reduces operator cognitive load — two adjacent flags with similar shapes parse the same way.
- Consistency with milestone-110 env-var format means operators who already configured `MIKEBOM_FINGERPRINTS_SOURCES` recognize the comma-separated shape.
- Clap's `value_parser` integration means error messages render via clap's standard error path, which already includes the flag name and offending value.

**Alternatives considered**:
- **String-typed CLI field with later parsing**: rejected — defers error reporting and complicates the test surface.
- **Custom clap `Args` derive with a sub-struct**: over-engineering for a two-string parse.
- **JSON-encoded flag values**: hostile to operator hand-authoring; rejected.

---

## Summary

Five research questions resolved; no [NEEDS CLARIFICATION] markers remain. The implementation is fully unblocked for Phase 1 design + Phase 2 task generation.

Key decisions for downstream phases:
1. Standards audit clears Principle V; envelope-extension approach is the only viable carrier.
2. Wire-compatibility approach: pure additive, no `algo` bump.
3. Match primitive: `Purl::canonical()` equality on both sides.
4. Conflict policy: fail-closed at CLI parse time.
5. Parser pattern: reuse milestone-073 `KEY=VALUE` shape + milestone-110 env-var conventions.
