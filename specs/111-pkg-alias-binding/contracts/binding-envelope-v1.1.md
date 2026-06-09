# Binding Envelope Schema (extended for milestone 111)

**Feature**: 111-pkg-alias-binding
**Base envelope**: milestone-072 `SourceDocumentBinding`
**Change kind**: pure additive (two optional fields). `algo` field unchanged (`"v1"`).

## Schema (JSON)

```json
{
  "source_doc_id": { "...": "milestone-072 SourceDocumentId" },
  "hash": "...",
  "strength": "verified" | "weak" | "unknown",
  "reason": "string (when strength=unknown)",
  "algo": "v1",

  "alias_from": "pkg:generic/baz",
  "alias_to": "pkg:cargo/baz@1.0.0"
}
```

| Field | Type | Required | Milestone | Notes |
|---|---|---|---|---|
| `source_doc_id` | object | yes | 072 | Pointer to source-tier SBOM document |
| `hash` | string | no | 072 | Per-component layered hash; `None` when `strength == unknown` |
| `strength` | enum | yes | 072 | `verified` / `weak` / `unknown`. **No new variants in milestone 111** (per /speckit-clarify Q3) |
| `reason` | string | no | 072 | Required when `strength == unknown`. New value `"alias-target-not-found-in-bind-target"` (milestone 111) |
| `algo` | string | yes | 072 | Always `"v1"`. **Not bumped for milestone 111** (additive metadata, not algorithm change) |
| `alias_from` | string (PURL) | no | **111** | LHS PURL operator declared. Paired with `alias_to`. |
| `alias_to` | string (PURL) | no | **111** | RHS PURL binder matched against. Paired with `alias_from`. |

## Invariants

1. `alias_from.is_some() == alias_to.is_some()` — paired presence.
2. When `alias_from` is present, `strength != "unknown"` OR `reason == "alias-target-not-found-in-bind-target"`. (An alias was applied → either it succeeded or it failed for the alias-specific reason; other unknown reasons should not coexist with `alias_from`.)
3. PURL values are canonical-form (lowercase scheme, sorted qualifiers, percent-encoding normalized).
4. Both fields are absent in the wire form (via `skip_serializing_if = "Option::is_none"`) when no alias was applied.

## Wire compatibility

**Pre-feature emit → pre-feature consume**: byte-identical to pre-feature baseline. No alias fields present. ✓

**Post-feature emit (no alias) → pre-feature consume**: byte-identical to pre-feature baseline (alias fields omitted via `skip_serializing_if`). ✓

**Post-feature emit (with alias) → pre-feature consume**: pre-feature deserializer ignores unknown `alias_from` / `alias_to` fields by serde-default behavior. Existing binding result interpreted as verified/weak as usual; the pre-feature consumer does NOT surface that an alias was applied (no `applied_alias` sibling in their output). ✓

**Post-feature emit → post-feature consume**: full round-trip; `applied_alias` sibling appears in verify-binding output. ✓

## Format-parity emission

The envelope serializes identically (modulo JSON whitespace + wrapper-property name) into all three target formats per the existing milestone-072 + milestone-071 parity infrastructure:

| Format | Carrier |
|---|---|
| CDX 1.6 | `components[*].properties[]` entry with `name = "mikebom:binding-result-v1"` and `value = <JSON string of envelope>` |
| SPDX 2.3 | `Package.annotations[].comment` wrapped in the existing `MikebomAnnotationCommentV1` envelope; `annotationType = "OTHER"`, `annotator = "Tool: mikebom-X.Y.Z"` |
| SPDX 3.0.1 | `Annotation.statement` carrying the same `MikebomAnnotationCommentV1` envelope shape, attached via `Annotation.subject` to the affected Element |

No new C-rows in `docs/reference/sbom-format-mapping.md`. The existing C56 (cross-tier binding annotation) row is amended with a note: "Envelope MAY additionally carry `alias_from` + `alias_to` fields when an operator-supplied `--pkg-alias` was applied (milestone 111)."

## Validation contract

Mikebom's existing milestone-072 envelope-validation logic (round-trip check, hash-shape check, strength-vs-reason consistency) is extended with:

- **Paired presence**: `alias_from.is_some() XOR alias_to.is_some()` → reject envelope as malformed (defense-in-depth — this should never happen for envelopes mikebom emits, but defends against corrupted input).
- **Canonical-form check on read**: a parsed `alias_from` / `alias_to` value MUST round-trip through `Purl::canonical()` unchanged; otherwise the envelope is rejected. Prevents abuse where a downstream tool injects a non-canonical PURL hoping mikebom will treat it as canonical.
- **Same-PURL-on-both-sides check on read**: `alias_from == alias_to` → reject (matches `AliasError::LhsEqualsRhs` at CLI parse time).

## Verify-binding output schema (sibling field)

When verify-binding / trace-binding emits its per-component JSON result, an aliased binding gains a sibling field:

```json
{
  "purl": "pkg:generic/baz",
  "binding": {
    "strength": "verified",
    "reason": null,
    "hash": "...",
    "source_doc_id": "..."
  },
  "applied_alias": "pkg:generic/baz → pkg:cargo/baz@1.0.0"
}
```

| Field | Type | Required | Notes |
|---|---|---|---|
| `applied_alias` | string | no | Present when the envelope's `alias_from` and `alias_to` are populated. Format: `"<LHS> → <RHS>"` (UTF-8 `→` U+2192) |

`applied_alias` is sibling to the existing `binding` object, NOT nested inside it. This preserves the existing `binding` schema for pre-feature consumers and gives downstream JSON-querying tools an obvious `jq '.[] | select(.applied_alias != null)'` filter idiom.
