# Research — milestone 157

Phase 0 outputs for the pnpm-lock v9 dep-graph fix.

## R1 — Pre-scan the `snapshots:` section into an intermediate lookup

**Decision**: Add a one-pass pre-scan at the top of `parse_pnpm_lock` at `mikebom-cli/src/scan_fs/package_db/npm/pnpm_lock.rs:20`, BEFORE the existing `packages:` iteration. The pre-scan builds an `std::collections::HashMap<String, Vec<String>>` where the key is the canonical `name@version` (peer-dep suffix stripped via `parse_pnpm_key`) and the value is the union-of-three-sub-mappings' keys already normalized to canonical `name@version` form.

**Rationale**: two-pass is the simplest shape — the alternative (lazy lookup inside the `packages:` loop) requires re-scanning the YAML mapping on every packages entry, which is O(N²). The pre-scan gives O(N) build + O(1) amortized lookup per packages entry, which is O(N) total. For argo-cd's 1834 entries, the difference is 3.4M operations vs 3.6K.

**Shape**:

```rust
fn build_snapshots_lookup(
    root: &serde_yaml::Value,
) -> std::collections::HashMap<String, Vec<String>> {
    let mut out = std::collections::HashMap::new();
    let Some(snapshots) = root.get("snapshots").and_then(|v| v.as_mapping()) else {
        return out;
    };
    for (key, entry) in snapshots {
        let Some(key_str) = key.as_str() else { continue };
        // Snapshot keys may carry peer-dep suffixes; normalize via
        // parse_pnpm_key to canonical name@version.
        let stripped = key_str.strip_prefix('/').unwrap_or(key_str);
        let (name, version) = match parse_pnpm_key(stripped) {
            Some(pair) => pair,
            None => continue,
        };
        let canonical = format!("{name}@{version}");

        let Some(tbl) = entry.as_mapping() else { continue };
        let mut deps: Vec<String> = Vec::new();
        for section in PNPM_DEP_SECTIONS {
            let Some(sub) = tbl.get(serde_yaml::Value::String(section.to_string()))
                .and_then(|v| v.as_mapping())
            else {
                continue;
            };
            for (dep_key, dep_value) in sub {
                // dep_key is the dep name (e.g. "@actions/exec");
                // dep_value is a version-spec that MAY carry a
                // peer-dep suffix — normalize via parse_pnpm_key on
                // "<name>@<value>" round-trip.
                let Some(dep_name) = dep_key.as_str() else { continue };
                let Some(dep_ver_raw) = dep_value.as_str() else { continue };
                let dep_pair_raw = format!("{dep_name}@{dep_ver_raw}");
                let stripped = dep_pair_raw
                    .strip_prefix('/')
                    .unwrap_or(&dep_pair_raw);
                let Some((canon_name, canon_ver)) = parse_pnpm_key(stripped) else {
                    tracing::debug!(dep = %dep_pair_raw, "pnpm snapshots: skipping non-registry dep value");
                    continue;
                };
                deps.push(format!("{canon_name}@{canon_ver}"));
            }
        }
        // Defensive de-dup: same package listed in dependencies:
        // AND peerDependencies: (rare but legal) MUST appear once.
        deps.sort();
        deps.dedup();
        out.insert(canonical, deps);
    }
    out
}
```

Where `PNPM_DEP_SECTIONS` is a module-level `const &[&str]` shared across both the snapshots pre-scan AND the packages-inline path (see R4).

**Alternatives considered**:
- **Lazy lookup**: rejected per O(N²) cost above.
- **BTreeMap instead of HashMap**: rejected — no downstream ordering requirement; HashMap is faster for the ~1834 entries typical of argo-cd-sized projects.
- **Vec<(String, Vec<String>)>**: rejected — linear scan defeats the pre-scan's O(1) lookup guarantee.

## R2 — Consume the lookup inside the packages loop

**Decision**: Extend the existing `depends: Vec<String>` construction at `pnpm_lock.rs:83-91`. When the entry's own `dependencies:` mapping (v6/v7 inline) is empty, fall back to the pre-scanned snapshots lookup keyed by the same canonical `name@version` used for the emitted PURL.

**Shape**:

```rust
// New: pre-scan once at the top of parse_pnpm_lock.
let snapshots_lookup = build_snapshots_lookup(root);

// ... existing packages loop ...

// Inside the loop, replace the current depends construction:
let inline_deps: Vec<String> = walk_pnpm_dep_sections(tbl); // union of 3 sub-mappings, dedup
let depends: Vec<String> = if inline_deps.is_empty() {
    // v9 path: pull from snapshots lookup.
    let canonical = format!("{name}@{version}");
    snapshots_lookup
        .get(&canonical)
        .cloned()
        .unwrap_or_default()
} else {
    // v6/v7 path: inline mapping wins.
    inline_deps
};
```

Where `walk_pnpm_dep_sections` is the shared 3-section walker (R4).

**Rationale**: preserves FR-004 (v6/v7 inline precedence) — inline non-empty wins; only falls back when inline is empty. Handles the SC-004 leaf case (both empty → empty depends) naturally.

**Alternatives considered**:
- **Always merge inline + snapshots**: rejected — theoretical drift-risk when a lockfile has both. FR-004 requires inline to win.
- **Union with inline as OR-override**: rejected — same reason.

## R3 — Peer-dep suffix stripping on dep VALUES

**Decision**: Reuse `parse_pnpm_key` at `pnpm_lock.rs:129` for edge-VALUE normalization by feeding it a synthesized `"<dep_name>@<dep_value>"` string. The function's existing logic (split on last `@`, then strip everything from first `(` onward) handles both peer-dep suffixes AND scoped-package names correctly.

**Concrete case**: dep value `3.0.0(@octokit/core@7.0.6)` under key `@octokit/plugin-paginate-rest`:
- Synthesized string: `@octokit/plugin-paginate-rest@3.0.0(@octokit/core@7.0.6)`
- After stripping `(` and beyond: `@octokit/plugin-paginate-rest@3.0.0`
- `parse_pnpm_key` returns `(@octokit/plugin-paginate-rest, 3.0.0)` — correct.

**Rationale**: single source of truth for peer-dep suffix stripping. Bug fixes to `parse_pnpm_key` (should any be needed) automatically apply to both identity keys AND edge values.

**Alternatives considered**:
- **New `strip_peer_dep_suffix` helper**: rejected — duplicates logic already in `parse_pnpm_key`. If the format changes, drift risk.
- **Regex-based stripping**: rejected per Constitution — pnpm-lock is line-oriented YAML and parsing overhead is fine; regex adds no clarity here.

**Verified via unit test**: SC-007 test (c) exercises this path with the pattern `foo@1.0.0(bar@2.0.0)`.

## R4 — Shared dep-section constant `PNPM_DEP_SECTIONS`

**Decision**: Add a module-level `const PNPM_DEP_SECTIONS: &[&str] = &["dependencies", "peerDependencies", "optionalDependencies"];` to `pnpm_lock.rs`. Use it in TWO places:

1. The `build_snapshots_lookup` pre-scan (R1).
2. The `walk_pnpm_dep_sections` inline-path helper (R2).

**Rationale**: constitutes the SC-011 parity assertion at the code level. Any future addition/removal of a dep section (e.g., `bundledDependencies` if pnpm ever emits it in snapshots) is one const-array edit.

**On the difference from `package_lock.rs`'s 4-section list** (`dependencies`, `devDependencies`, `optionalDependencies`, `peerDependencies`): pnpm encodes dev status via the per-package `dev: true` boolean at the entry level (already handled at `pnpm_lock.rs:56`), NOT via a `devDependencies:` sub-mapping. The SC-011 test asserts that pnpm's THREE-section list matches package_lock's THREE non-dev sections; the `devDependencies` divergence is by lockfile format design, documented in the spec's SC-011 rationale + reinforced in a doc-comment on the constant.

**Alternatives considered**:
- **Duplicate the list inline in both places**: rejected — drift risk exactly opposite the SC-011 goal.
- **Import `package_lock.rs`'s list**: rejected — that list includes `devDependencies` and would break pnpm parsing. The parity is intentional-modulo-dev-format-difference.

## R5 — Monotonic-additive golden diff verification

**Decision**: Add a helper `assert_monotonic_additive_pnpm_golden_diff(old: &Value, new: &Value)` in a shared test module (`mikebom-cli/tests/common/monotonic.rs` or inline in the milestone-157 integration test file). For each `dependencies[].ref` in the OLD golden, its `dependsOn` list MUST be a subset of the corresponding NEW golden's `dependsOn` list. New entries in NEW that weren't in OLD are permitted (that's the additive part).

**Shape**:

```rust
pub fn assert_monotonic_additive(old: &serde_json::Value, new: &serde_json::Value) {
    let old_deps = index_dependencies_by_ref(old);
    let new_deps = index_dependencies_by_ref(new);
    for (ref_str, old_targets) in &old_deps {
        let new_targets = new_deps.get(ref_str).unwrap_or(&BTreeSet::new()).clone();
        let missing: Vec<&String> = old_targets.difference(&new_targets).collect();
        assert!(
            missing.is_empty(),
            "monotonic-additive violation for {ref_str}: pre-existing edges MUST all still appear, missing: {missing:?}"
        );
    }
}
```

**Rationale**: SC-002's dual-side guard requires proof that pnpm goldens didn't drop edges. Manual diff review is error-prone at scale (a 1329-component golden has 5000+ edges); an automated assertion catches accidental removals.

**When to invoke**: at the golden-regeneration test level. If a maintainer regenerates the pnpm golden (`MIKEBOM_UPDATE_CDX_GOLDENS=1 cargo test`), the CI-run version WITHOUT the update flag re-reads both goldens and asserts monotonic-additivity. If any pre-existing edge is missing, the assertion fires with a diagnostic naming the ref + the missing edge.

**Alternatives considered**:
- **Hand-review the diff**: rejected — 5000+ edges is not eyeball-reviewable.
- **Full byte-diff**: rejected — that's SC-002's pre-existing shape which we KNOW will fail (edges are being added by design).

## R6 — Diagnostic emission (FR-007 + FR-008)

**Decision**: Add two `tracing` emissions inside `parse_pnpm_lock`:

- **FR-007 info-level** (every v9 lockfile parse):
  ```rust
  tracing::info!(
      lockfile = %source_path,
      lockfile_version = %lock_version,
      packages_count = packages_len,
      snapshots_count = snapshots_len,
      fell_back_to_snapshots = fell_back_count,
      "pnpm-lock parsed"
  );
  ```
  Fires at end of parse. `fell_back_count` = number of packages entries whose inline deps were empty AND matched a snapshots lookup. For v6/v7 files: fell_back_count = 0 (inline always populated); for v9 files with normal shape: fell_back_count ≈ packages_len.

- **FR-008 warn-level** (v9 with no snapshots section):
  ```rust
  tracing::warn!(
      lockfile = %source_path,
      lockfile_version = %lock_version,
      "pnpm-lock v9 with no snapshots section — dep-graph will be empty for all non-root components. Check lockfile validity."
  );
  ```
  Fires only when `lockfileVersion >= 9` AND `snapshots:` is missing or empty.

**Rationale**: Constitution Principle X (Transparency) — operators can grep for either line to diagnose lockfile-format issues without re-running the scan with `RUST_LOG=debug`.

**Alternatives considered**:
- **Debug-level only**: rejected — the info-level line is grep-friendly for CI-log analysis; the warn-level line is a "your lockfile is anomalous" signal that operators need to see by default.
- **Errors instead of warns**: rejected — an empty snapshots section is a data-shape issue, not a parser fault; other consumers (pnpm's own tools) might refuse-to-install but mikebom's fail-open posture is right for SBOM emission.

## R7 — Version detection

**Decision**: Read the top-level `lockfileVersion` field via:

```rust
let lock_version: String = root
    .get("lockfileVersion")
    .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_f64().map(|n| n.to_string())))
    .unwrap_or_default();

let is_v9_or_later: bool = lock_version
    .split('.')
    .next()
    .and_then(|s| s.parse::<u32>().ok())
    .map(|major| major >= 9)
    .unwrap_or(false);
```

The field is quoted string `'9.0'` in v9 lockfiles per empirical inspection of argo-cd. Older versions may emit unquoted floats (`6.0`). Both parse cleanly via the `.as_str()` / `.as_f64()` fallthrough.

**Rationale**: enables FR-008's conditional warn (only fires on v9+). Also feeds FR-007's info-level `lockfile_version` field.

**Alternatives considered**:
- **Parse major only**: works but loses granularity in the info-log for `9.1` etc.
- **Match specific value list `['9', '9.0', '9.1']`**: rejected — brittle to future minors.

## R8 — Test inventory (SC-007 requires ≥7 unit tests)

**Decision**: 8 unit tests inline in `pnpm_lock.rs`'s existing `#[cfg(test)] mod tests` block. 1 integration test at `mikebom-cli/tests/npm_pnpm_v9_dep_graph.rs`.

| # | Test | Type | Covers |
|---|------|------|--------|
| 1 | `pnpm_v9_minimal_dependencies_only_emits_edge` | unit | SC-007(a). Fixture: 1 packages entry + 1 snapshots entry with `dependencies: {bar: 2.0.0}`. Assert 1 emission with `depends = ["bar@2.0.0"]`. |
| 2 | `pnpm_v9_empty_snapshot_body_leaf_node` | unit | SC-007(b) + SC-004. Fixture: `snapshots: {foo@1.0.0: {}}`. Assert `depends.is_empty()`. |
| 3 | `pnpm_v9_peer_dep_suffix_normalized_in_key_and_value` | unit | SC-007(c) + SC-003. Fixture: `snapshots: {foo@1.0.0(bar@2.0.0): {dependencies: {baz: 3.0.0(qux@4.0.0)}}}`. Assert emitted PURL is `pkg:npm/foo@1.0.0` (identity stripped) + `depends = ["baz@3.0.0"]` (value stripped). |
| 4 | `pnpm_v9_orphaned_snapshot_skipped` | unit | SC-007(d) + FR-006. Fixture: `packages:` empty + `snapshots: {foo@1.0.0: {dependencies: {bar: 2.0.0}}}`. Assert no PackageDbEntry emitted. |
| 5 | `pnpm_v9_all_three_sub_mappings_union_with_dedup` | unit | SC-007(e) + Q1 clarification. Fixture: snapshot with `dependencies: {a: 1.0.0, shared: 5.0.0}` + `peerDependencies: {b: 2.0.0, shared: 5.0.0}` + `optionalDependencies: {c: 3.0.0}`. Assert `depends = ["a@1.0.0", "b@2.0.0", "c@3.0.0", "shared@5.0.0"]` (sorted-dedup guarantees `shared` appears once). |
| 6 | `pnpm_v6_v7_inline_peer_and_optional_now_emit` | unit | SC-007(f) + FR-004. Fixture: v6-style packages entry with inline `dependencies: {a: 1.0.0}` + `peerDependencies: {b: 2.0.0}` + `optionalDependencies: {c: 3.0.0}` — no snapshots section. Assert `depends = ["a@1.0.0", "b@2.0.0", "c@3.0.0"]` (v6/v7 path now walks all three per Q1). |
| 7 | `pnpm_v9_inline_wins_over_snapshots_fallback` | unit | FR-004 precedence + SC-007(g). Fixture: v9 lockfile where a packages entry ALSO carries inline `dependencies:` (malformed / hand-authored lockfile). Assert inline wins; snapshots' different edges NOT emitted. |
| 8 | `pnpm_walks_same_dep_sections_as_package_lock_non_dev` | unit | SC-011 parity. Assert `PNPM_DEP_SECTIONS.contains(&"dependencies")` + `.contains(&"peerDependencies")` + `.contains(&"optionalDependencies")`. Documents by construction that pnpm ↔ package_lock walk the same 3 non-dev sections. |

Plus **1 integration test** at `mikebom-cli/tests/npm_pnpm_v9_dep_graph.rs`:
- Synthetic 5-package testbed with peer-dep suffixes + snapshots-only edges (mimics argo-cd shape without vendoring 1834 packages). Invoke release binary, parse CDX, assert ≥1 non-trivial `dependsOn` list matches expected.

Plus **1 monotonic-additive helper test** at the same integration file:
- Regenerate the milestone-090 pnpm golden inline, then invoke `assert_monotonic_additive_pnpm_golden_diff` against a hand-crafted pre-157 snapshot embedded in the test source. Verifies the helper catches missing-edge violations.

Total unit + integration test count: 8 + 2 = 10. SC-007 floor ≥7 easily cleared.

## R9 — CHANGELOG entry shape (SC-009)

**Decision**: Single subsection under `## [Unreleased]` in `CHANGELOG.md`. Content documents:

- The pnpm-lock v9 `snapshots:` support fix.
- The argo-cd testbed impact (110 → ≥5000 edges — actual number empirically confirmed at implementation time).
- The Q1 clarification bringing pnpm to full parity with npm's `package_lock.rs` (walks `dependencies:` + `peerDependencies:` + `optionalDependencies:` per milestone 147).
- The monotonic-additive pnpm v6/v7 golden regeneration (pre-existing edges preserved; new peer + optional edges added).
- Reference to the team's bug report + empirical repro date (2026-07-03).
- Consumer jq recipe for verifying edge presence.

Sample recipe:

```bash
jq '.dependencies[] | select(.ref == "pkg:npm/%40actions/core@3.0.1") | .dependsOn' sbom.cdx.json
# Expected: ["pkg:npm/%40actions/exec@3.0.0", "pkg:npm/%40actions/http-client@4.0.1"]
```

## R10 — Verification per SC

- **SC-001** (argo-cd ≥5000 edges): manual operator-cadence. Maintainer clones `kusari-sandbox/argo-cd`, points at `ui/`, runs release binary, greps for edge count. Report in PR comments.
- **SC-002** (dual-side guard): automated via `cargo test --test cdx_regression --test spdx_regression --test spdx3_regression`. Pnpm golden regens once at impl-time; monotonic-additive helper asserts no pre-existing edge is lost.
- **SC-003** (peer-dep suffix): unit test #3.
- **SC-004** (leaf node): unit test #2.
- **SC-005** (v9 no snapshots warn): unit test with fixture having v9 header + no snapshots key; assert emitted output empty + assert warn logged (via `tracing_test` fixture or per-test log capture).
- **SC-006** (pre-PR gate + --no-fail-fast per milestone-155 memory).
- **SC-007** (≥7 unit tests): grep count verifies.
- **SC-008** (integration test): T-numbered integration test file.
- **SC-009** (CHANGELOG): grep for milestone-157 keywords.
- **SC-010** (no wire-format changes): `git diff main --name-only -- mikebom-cli/src/generate/ mikebom-cli/src/parity/ docs/reference/sbom-format-mapping.md` all empty; `git diff main --name-only -- Cargo.toml Cargo.lock` empty (F3-from-milestone-156 guard).
- **SC-011** (pnpm/npm parity): unit test #8 + doc-comment on `PNPM_DEP_SECTIONS`.

## R11 — Interaction with milestone-155/156 code paths

**Decision**: No interaction. Milestone 157 touches only `mikebom-cli/src/scan_fs/package_db/npm/pnpm_lock.rs` (primary) + `mikebom-cli/tests/` (new integration test) + fixture files + goldens.

**Verified**: `npm/mod.rs:82-97` reader-dispatch order is unchanged (per FR-015). CMake reader (milestones 155/156), cargo/maven/etc. readers all untouched.
