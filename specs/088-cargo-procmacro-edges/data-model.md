# Data Model — milestone 088 verify-and-close cargo proc-macro outgoing dep edges

This is a verify-and-close milestone with no new entities, no new validation rules at the SBOM-emission layer, and no new domain types. The only "model" change is in the regression-test pinning array.

## Entities

### Representative Edge (in `EXPECTED_REPRESENTATIVE_EDGES`)

A `(from_purl_prefix, to_purl_prefix)` tuple in `mikebom-cli/tests/transitive_parity_cargo.rs` declaring an edge that mikebom MUST emit when scanning the cargo audit fixture. Existing entity reused; this milestone adds 4 rows.

**Fields**:
- `from_purl_prefix: &'static str` — PURL of the source component, version-stripped (e.g., `"pkg:cargo/clap_derive"`).
- `to_purl_prefix: &'static str` — PURL of the dependency target, version-stripped (e.g., `"pkg:cargo/heck"`).

**Validation rules**:
- VR-088-001: each new edge MUST be present in the post-087 SBOM emitted against `mikebom-cli/tests/fixtures/transitive_parity/cargo/`. Verified by `cargo +stable test -p mikebom --test transitive_parity_cargo`.
- VR-088-002: each new edge MUST correspond to a `[[package]] dependencies = [...]` entry for `clap_derive@4.5.18` in `Cargo.lock`. Verified by manual `Cargo.lock` inspection (4 entries: heck, proc-macro2, quote, syn).
- VR-088-003: PURL-prefix format MUST omit `@<version>` so `strip_version` at `transitive_parity_cargo.rs:133-138` can match against any version (forward-compat for fixture refreshes).
- VR-088-004: total array length post-add MUST be 8 (4 milestone-087 baseline + 4 milestone-088 additions). Verified by reading the test source.

## Audit-Doc Gap Status

The `specs/083-transitive-correctness/research.md §8 — Ecosystem: cargo` gap-list is a markdown checklist enumerating known divergences. This milestone toggles gap #2's status from "open" → "closed by milestone 087".

**Validation rules**:
- VR-088-005: post-edit, the gap-list MUST contain zero "open" gaps for mikebom-side. Both #172 (closed by 087) and #173 (closed by 087, pinned by 088) are marked closed. Cross-tool gaps #3 + #4 remain ("not a mikebom gap" per their existing classification).
- VR-088-006: the "Filed follow-up issues" footer MUST mark #173 with the same "Closed by milestone 087" annotation pattern used for #172. Mirrors the existing convention.

## GitHub Issue #173 Closure

**Validation rules**:
- VR-088-007: the PR description MUST contain a `Closes #173` line so GitHub auto-closes the issue on merge.
- VR-088-008: post-merge, issue #173 state MUST be `closed`. Verified via `gh issue view 173`.
