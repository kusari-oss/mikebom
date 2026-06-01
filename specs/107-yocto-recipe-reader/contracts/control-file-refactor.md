# Contract: Shared control-file stanza parser (foundation refactor)

**New module**: `mikebom-cli/src/scan_fs/package_db/control_file.rs`
**Modified module**: `mikebom-cli/src/scan_fs/package_db/dpkg.rs` (delegates to the new helper)

## Why this exists

opkg's `/var/lib/opkg/status` uses byte-identical RFC822-style "control file" stanza syntax to dpkg's `/var/lib/dpkg/status` (see research R1). Rather than duplicate the existing ~400-LOC dpkg parser, we extract the parser into a shared `control_file.rs` module. dpkg.rs becomes a thin shell that calls the shared helper and constructs `PackageDbEntry` values from the parsed stanzas; opkg.rs does the same.

## API surface

```rust
/// One parsed stanza — field name -> field value, multi-line `Description:`
/// continuation lines correctly merged.
pub(super) struct ControlStanza {
    pub fields: std::collections::BTreeMap<String, String>,
}

impl ControlStanza {
    pub fn get(&self, name: &str) -> Option<&str>;
    pub fn name(&self) -> Option<&str>;       // shorthand for fields.get("Package")
    pub fn version(&self) -> Option<&str>;    // shorthand for fields.get("Version")
    pub fn architecture(&self) -> Option<&str>; // shorthand
    // ... (named accessors for the half-dozen fields both dpkg + opkg consume)
}

/// Parse a full control-file (multi-stanza). Stanzas separated by blank lines.
/// Returns a Vec; malformed stanzas emit `tracing::warn!` and are skipped.
pub(super) fn parse_stanzas(text: &str) -> Vec<ControlStanza>;
```

## Refactor mechanics

### Before (dpkg.rs, today)

`dpkg.rs` has a private inline parser (~400 LOC including field-value extraction, multi-line continuation handling, and edge-case warns). The parser's output is consumed by `dpkg::read()` to construct `PackageDbEntry` values.

### After (this milestone)

1. New `control_file.rs` houses the stanza parser (the ~400 LOC moves there with zero logical changes).
2. `dpkg.rs::read()` calls `control_file::parse_stanzas(text)`, then iterates the resulting `Vec<ControlStanza>` to build `PackageDbEntry` values — exactly what the inline parser fed into before.
3. `opkg.rs::read()` does the same, with opkg-specific field interpretations (e.g. `Filename:` URL handling, `Installed-Time:` annotation).

The refactor is **net behavior-neutral** for dpkg — same parser, same output. The existing dpkg golden fixtures (the 33 byte-identity goldens regenerated each release) MUST pass byte-identically post-refactor.

## Validation

- `cargo +stable test --workspace` — full suite continues to pass
- The dpkg golden (`mikebom-cli/tests/fixtures/golden/cyclonedx/deb.cdx.json` etc.) MUST be byte-identical pre- and post-refactor — verified by running goldens BEFORE applying the refactor and confirming zero diff after
- New unit tests in `control_file.rs::tests` cover the stanza parser in isolation (multi-line continuation, blank-line separation, unknown field tolerance, malformed-stanza warn-and-skip)

## Sequencing

This refactor lands as the **foundation PR** for milestone 107 — it MUST merge before US1 (the opkg reader) can ship. Same sequencing as milestone 106's foundation PR (#283) which extracted the JSONC stripper and workspace helper before US1-US5 could consume them.

The refactor PR body must explicitly state: "Net behavior-neutral for dpkg. The 33 byte-identity goldens are unchanged. Justified by US1 (opkg reader) landing in PR #NEXT which reuses this helper."
