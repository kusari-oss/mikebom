# Data Model — milestone 157

Phase 1 output. Types + wire shapes introduced by the pnpm-lock v9 dep-graph fix.

## 1. New module-level constant

```rust
/// pnpm dep-section names walked by both the snapshots pre-scan (v9)
/// AND the packages-inline path (v6/v7). Kept in one place so a future
/// dep-section addition/removal is a single edit + so SC-011's parity
/// assertion has a stable code anchor.
///
/// **NOT identical to package_lock.rs's 4-section list** — pnpm encodes
/// dev status via the per-package `dev: true` boolean at the entry
/// level (handled at `pnpm_lock.rs:56`), NOT via a `devDependencies:`
/// sub-mapping. So `PNPM_DEP_SECTIONS` walks only the three non-dev
/// sections. The `dev: true` boolean continues to gate whole-package
/// filtering when `include_dev = false`, per pre-milestone-157
/// behavior.
const PNPM_DEP_SECTIONS: &[&str] = &[
    "dependencies",
    "peerDependencies",
    "optionalDependencies",
];
```

Location: top of `mikebom-cli/src/scan_fs/package_db/npm/pnpm_lock.rs`, immediately after the `use` block.

## 2. New internal helper functions

### `build_snapshots_lookup`

Signature:

```rust
fn build_snapshots_lookup(
    root: &serde_yaml::Value,
) -> std::collections::HashMap<String, Vec<String>>;
```

Purpose: pre-scan the `snapshots:` section into a lookup table keyed by canonical `name@version` (peer-dep suffix stripped). Values are the sorted-deduped union of the three sub-mappings' keys, each normalized to canonical `name@version` form.

Behavior:
- Returns empty HashMap when the top-level `snapshots:` key is missing or not a mapping.
- Skips snapshot entries whose keys fail `parse_pnpm_key` normalization (non-registry sources: git URLs, tarballs, file paths).
- Skips snapshot entries whose values fail `parse_pnpm_key` normalization; logs at `tracing::debug!` level with the raw dep string for operator visibility.
- Defensive de-dup: `sort` + `dedup` before insertion. A package listed in `dependencies:` AND `peerDependencies:` appears once in the result.

### `walk_pnpm_dep_sections`

Signature:

```rust
fn walk_pnpm_dep_sections(entry_tbl: &serde_yaml::Mapping) -> Vec<String>;
```

Purpose: walk the three sub-mappings (`dependencies:` / `peerDependencies:` / `optionalDependencies:`) inside a single packages entry (v6/v7 path). Returns the sorted-deduped union of the sub-mappings' keys, each normalized to canonical `name@version` form.

Behavior:
- Returns empty Vec when NO sub-mappings are populated (v9 case — falls through to snapshots lookup at the caller).
- Same value-normalization + dedup guarantees as `build_snapshots_lookup`.

## 3. Modified function signature

`parse_pnpm_lock` signature is **unchanged**. The three-arg shape `(root, source_path, include_dev)` at `pnpm_lock.rs:20` stays the same. All new logic lives inside the function body.

## 4. `PackageDbEntry` shape — UNCHANGED

**No struct-level changes.** The existing `PackageDbEntry` at `mikebom-cli/src/scan_fs/package_db/mod.rs` is unchanged. Milestone 157's contribution is exclusively to the `depends: Vec<String>` field's contents — the field is populated more completely, but its shape and semantics (list of canonical `name@version` strings representing outbound edges) are stable.

## 5. `extra_annotations` — UNCHANGED

No new annotation keys. Per FR-013 + SC-010, no `mikebom:*` extensions land in this milestone.

## 6. Wire examples — CDX (only `dependsOn` counts change)

Before milestone 157 (v9 argo-cd shape):

```json
{
  "components": [
    {"bom-ref": "pkg:npm/%40actions/core@3.0.1", "purl": "pkg:npm/%40actions/core@3.0.1", ...}
  ],
  "dependencies": [
    {"ref": "pkg:npm/%40actions/core@3.0.1", "dependsOn": []}
  ]
}
```

After milestone 157 (same input; edges populated from `snapshots:`):

```json
{
  "components": [
    {"bom-ref": "pkg:npm/%40actions/core@3.0.1", "purl": "pkg:npm/%40actions/core@3.0.1", ...}
  ],
  "dependencies": [
    {
      "ref": "pkg:npm/%40actions/core@3.0.1",
      "dependsOn": [
        "pkg:npm/%40actions/exec@3.0.0",
        "pkg:npm/%40actions/http-client@4.0.1"
      ]
    }
  ]
}
```

Component shape is unchanged. Only the `dependencies[]` entries grow.

## 7. SPDX 2.3 / SPDX 3 — UNCHANGED emitter code path

The two SPDX emitters read `PackageDbEntry.depends` via the same `resolve::deduplicator` → `ResolvedComponent` pipeline as CDX. Post-milestone-157 the emitter sees a richer `depends` list; the emitted `relationships[]` (SPDX 2.3) and dependency relationships (SPDX 3) grow correspondingly. No emitter code change, no annotation key additions, no schema shape drift.

## 8. Fixture layout (new)

New integration test fixture: **synthesized inside the test file** at test-runtime (per milestone-155 precedent for the dedup integration test — no vendored files under `tests/fixtures/`). Fixture size is small (~5 packages, ~15 lines of YAML), fits inline in a raw string literal.

Optional: if the SC-002 monotonic-additive golden helper needs a pre-157 reference snapshot, embed it as a `const OLD_SNAPSHOT: &str = r#"..."#;` inside the integration test file.

## 9. Golden fixtures — WILL REGENERATE (pnpm-only)

Per Q1 clarification. Expected regenerating goldens:

- `mikebom-cli/tests/fixtures/golden/cyclonedx/npm.cdx.json` — the npm fixture family that includes pnpm-lock v6 test data (verify at impl-time).
- `mikebom-cli/tests/fixtures/golden/spdx-2.3/npm.spdx.json` — parallel.
- `mikebom-cli/tests/fixtures/golden/spdx-3/npm.spdx3.json` — parallel.

Verification: the monotonic-additive helper (R5) asserts that each regenerated golden STRICTLY ADDS edges vs milestone-156. No pre-existing edge is dropped.

Non-pnpm goldens (`apk`, `bazel`, `cargo`, `cmake`, `deb`, `gem`, `golang`, `maven`, `pip`, `rpm`) remain byte-identical.

## 10. `docs/reference/sbom-format-mapping.md` — UNCHANGED

No new catalog rows. Milestone 157 emits no new annotation keys. Existing D1 (evidence identity) + D2 (evidence occurrences) + relationship rows continue to cover the enriched output.

## 11. Reader dispatch order — UNCHANGED

`npm/mod.rs:82-97` dispatch order stays: `package_lock` → `pnpm_lock` → `bun_lock` → `yarn_lock`. Per FR-015. Milestone 157 modifies only `pnpm_lock`'s internal parsing; the tier-A priority order is untouched.
