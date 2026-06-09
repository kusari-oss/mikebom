# Data Model: Operator-supplied PURL alias for cross-tier binding

**Feature**: 111-pkg-alias-binding
**Date**: 2026-06-09

## Types

### `PurlAlias` (new — `mikebom-cli/src/binding/alias.rs`)

Newtype carrying a single `(LHS, RHS)` PURL pair. Constructed only via `parse_flag_value` or `from_env_entry`, both of which run both sides through `Purl::canonical()`.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PurlAlias {
    pub lhs: Purl,   // image-tier component PURL
    pub rhs: Purl,   // source-tier ecosystem PURL
}
```

**Invariants**:
- `lhs != rhs` (constructor enforces; same-on-both-sides is operator error).
- Both fields are canonical-form PURLs (constructor runs `Purl::canonical()`).

**Equality**: derived. Two aliases are equal iff both PURLs match exactly.

---

### `AliasMap` (new — `mikebom-cli/src/binding/alias.rs`)

Container of validated aliases for one scan invocation. Lookups by LHS PURL are O(log N) via `BTreeMap` (deterministic iteration order for SBOM emission stability; N is typically < 10 so the constant matters more than the asymptote).

```rust
#[derive(Debug, Clone, Default)]
pub(crate) struct AliasMap {
    by_lhs: std::collections::BTreeMap<Purl, Purl>,
}

impl AliasMap {
    pub fn insert(&mut self, alias: PurlAlias) -> Result<(), AliasError> { ... }
    pub fn get(&self, lhs: &Purl) -> Option<&Purl> { ... }
    pub fn is_empty(&self) -> bool { ... }
    pub fn iter(&self) -> impl Iterator<Item = (&Purl, &Purl)> { ... }
}
```

**Invariants**:
- `insert` rejects same-LHS-different-RHS pairs via `AliasError::ConflictingRhs` (FR-008).
- Same-LHS-same-RHS insert is idempotent (no error).

---

### `AliasError` (new — `mikebom-cli/src/binding/alias.rs`)

```rust
#[derive(Debug, thiserror::Error)]
pub(crate) enum AliasError {
    #[error("malformed --pkg-alias value '{raw}': missing '=' separator")]
    MissingSeparator { raw: String },
    #[error("malformed --pkg-alias LHS PURL '{lhs}': {source}")]
    MalformedLhs { lhs: String, source: PurlError },
    #[error("malformed --pkg-alias RHS PURL '{rhs}': {source}")]
    MalformedRhs { rhs: String, source: PurlError },
    #[error("--pkg-alias LHS '{lhs}' declared twice with conflicting RHS values: '{existing_rhs}' and '{new_rhs}'")]
    ConflictingRhs { lhs: Purl, existing_rhs: Purl, new_rhs: Purl },
    #[error("--pkg-alias LHS '{lhs}' identical to RHS; aliases must specify distinct PURLs")]
    LhsEqualsRhs { lhs: Purl },
}
```

Maps cleanly to clap's `value_parser` error stringification (clap calls `.to_string()` on the error). Each variant's Display message satisfies the SC-003 single-line-actionable contract.

---

### `SourceDocumentBinding` (MODIFY — `mikebom-cli/src/binding/mod.rs:185`)

Additive: two new optional fields. Existing fields unchanged.

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceDocumentBinding {
    pub source_doc_id: SourceDocumentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<BindingHash>,
    pub strength: BindingStrength,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default = "default_algo_v1")]
    pub algo: String,

    // NEW (milestone 111):
    /// When this binding result was reached via a `--pkg-alias` declaration,
    /// the LHS PURL the operator declared. None otherwise. MUST be `Some`
    /// iff `alias_to` is `Some` (constructor enforces).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias_from: Option<Purl>,
    /// Pair of `alias_from`. When `Some`, this is the RHS PURL the binder
    /// matched against in the bind-source SBOM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias_to: Option<Purl>,
}
```

**Invariants**:
- `alias_from.is_some() == alias_to.is_some()` (paired presence; never one without the other).
- `algo` remains `"v1"` — this is an additive metadata extension, not an algorithm change (research.md §2).

**Wire compatibility**: pre-feature consumers' deserialization sees two extra fields when present; serde's default ignore-unknown-fields behavior accepts the envelope. Pre-feature SBOMs (without the new fields) deserialize into the new struct with both fields `None` via `#[serde(default)]`. Round-trip byte-identity for no-alias scans is preserved by `skip_serializing_if`.

---

### `AliasFailureReason` (extension — string constant)

New string value added to the enumerated `unknown { reason }` vocabulary:

| Existing reason (milestone 072) | New reason (milestone 111) |
|---|---|
| `"source-not-found-in-bind-target"` | `"alias-target-not-found-in-bind-target"` |
| `"no-evidence"` | (unchanged) |
| `"base-layer-system-package"` | (unchanged) |
| ... | (unchanged) |

The new reason fires when an alias was declared, the LHS matched a scan-output component, but the RHS was NOT found in the bind-source SBOM. Distinguishes operator-misconfiguration (alias target missing) from genuine no-source (FR-007).

---

## Relationships

```
   CLI flag --pkg-alias LHS=RHS
              │
              ▼
   value_parser::parse_pkg_alias()
              │  Result<PurlAlias, AliasError>
              ▼
   AliasMap::insert()
              │  Result<(), AliasError>
              ▼
   AliasMap held by ScanContext for the scan duration
              │
              ▼
   binding::BindingComputer::compute(component_purl)
   │   1. canonical_lhs = component_purl.canonical()
   │   2. rhs = alias_map.get(canonical_lhs)
   │   3. if rhs.is_some():
   │        match_in_source(rhs)  → SourceDocumentBinding {
   │                                    ..., alias_from: lhs, alias_to: rhs
   │                                }
   │      else:
   │        match_in_source(canonical_lhs)  → SourceDocumentBinding {
   │                                              ..., alias_from: None, alias_to: None
   │                                          }
              │
              ▼
   Envelope serialized into CDX/SPDX 2.3/SPDX 3 emission via parity extractors
              │
              ▼  (later, on verify-binding / trace-binding)
              │
   verify::ComponentBinding::binding_for_purl()
   │   Reads alias_from/to back from envelope; emits applied_alias sibling
   │   in output JSON when present.
```

## Lifecycle

Per-scan:
1. Operator passes `--pkg-alias` flags and/or sets `MIKEBOM_PKG_ALIAS`.
2. `scan_cmd::execute` parses both sources into an `AliasMap`. Conflicts and malformed PURLs fail here.
3. `AliasMap` is held by the scan context; consulted once per binary-tier component when `--bind-to-source` is active.
4. Aliased components get the extended envelope with `alias_from` / `alias_to` populated.
5. Unused aliases (LHS that matched no component) get an info-level log.
6. SBOM is emitted; alias state is now baked into the document.

Per-verify (later, no operator state):
1. `verify-binding` / `trace-binding` parses the envelope.
2. Encounters populated `alias_from` / `alias_to` → emits `applied_alias` sibling in output.
3. Reproduces binding strength using only what the envelope records — no CLI re-supply required (FR-006).

## Non-goals

- Persisting the `AliasMap` outside the SBOM (no config file, no state file).
- Wildcards, regex, name-only fuzz matching.
- Reverse-direction (RHS → LHS) alias application.
