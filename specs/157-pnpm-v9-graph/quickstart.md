# Quickstart — milestone 157

Validation walkthrough for the pnpm-lock v9 dep-graph fix. SC-001 is the manual operator-cadence argo-cd testbed; the remaining SCs are automated.

## Scenario 1 — SC-001 argo-cd testbed dep-graph completeness (MANUAL operator-cadence)

```bash
# 1. Build mikebom at milestone-157 HEAD:
cargo +stable build --release -p mikebom

# 2. Ensure argo-cd (kusari-sandbox fork or upstream) is available:
git clone --depth 1 https://github.com/kusari-sandbox/argo-cd.git /tmp/argo-cd

# 3. Scan the ui/ subtree (the pnpm-lock.yaml lives there):
./target/release/mikebom --offline sbom scan \
    --path /tmp/argo-cd/ui \
    --format cyclonedx-json \
    --output cyclonedx-json=/tmp/mikebom-m157/argo-cd.cdx.json \
    --no-deep-hash

# 4. Count total dep-graph edges:
jq '[.dependencies[] | .dependsOn // [] | length] | add' \
    /tmp/mikebom-m157/argo-cd.cdx.json
```

**Expected**: integer ≥ 5000 (empirical baseline; actual number may be higher given Q1 clarification adds peer + optional edges).

```bash
# 5. Verify a specific known component's edges:
jq '.dependencies[] | select(.ref == "pkg:npm/%40actions/core@3.0.1") | .dependsOn' \
    /tmp/mikebom-m157/argo-cd.cdx.json
```

**Expected** (order-independent):

```json
[
  "pkg:npm/%40actions/exec@3.0.0",
  "pkg:npm/%40actions/http-client@4.0.1"
]
```

```bash
# 6. Sanity spot-check react (a leaf node in the snapshots: section):
jq '.dependencies[] | select(.ref == "pkg:npm/react@19.2.6") | .dependsOn' \
    /tmp/mikebom-m157/argo-cd.cdx.json
```

**Expected**: `[]` (empty — matches the `react@19.2.6: {}` shape in the lockfile's `snapshots:` section).

If steps 4-6 all pass → ✅ SC-001 PASS. Report edge count + PR comment.

## Scenario 2 — SC-002 dual-side golden guard (automated)

```bash
cargo +stable test --workspace --no-fail-fast \
    --test cdx_regression --test spdx_regression --test spdx3_regression
```

**Expected**:
- 33 tests total (11 per format × 3 formats).
- **10 of 11 ecosystems byte-identical** to milestone-156 goldens: apk, bazel, cargo, cmake, deb, gem, golang, maven, pip, rpm.
- **1 of 11 (npm) regenerates** — the pnpm sub-fixture inside `npm.cdx.json` / `npm.spdx.json` / `npm.spdx3.json` gains peer + optional dep edges. **Regeneration expected** (Q1 monotonic-additive change).
- After committing regenerated goldens, the byte-identity tests pass on subsequent runs.

Post-regeneration verification (monotonic-additive assertion):

```bash
cargo +stable test --workspace --test npm_pnpm_v9_dep_graph \
    -- --exact assert_monotonic_additive_pnpm_golden
```

**Expected**: passes. Reads the pre-157 golden snapshot (embedded as `const OLD_SNAPSHOT` in the test) and the current golden, asserts every pre-existing edge is present in the new golden.

## Scenario 3 — SC-003 peer-dep suffix normalization (automated)

```bash
cargo +stable test --bin mikebom pnpm_v9_peer_dep_suffix_normalized
```

**Expected**: single test passes. Fixture uses `foo@1.0.0(bar@2.0.0)` key + `baz: 3.0.0(qux@4.0.0)` value; asserts emitted PURL is `pkg:npm/foo@1.0.0` (identity peer-suffix stripped) + `depends` contains `baz@3.0.0` (value peer-suffix stripped).

## Scenario 4 — SC-004 leaf-node correctness (automated)

```bash
cargo +stable test --bin mikebom pnpm_v9_empty_snapshot_body_leaf_node
```

**Expected**: single test passes. Empty snapshot body → empty `depends` (no crash, no synthetic edges).

## Scenario 5 — SC-005 missing snapshots warning (automated)

```bash
cargo +stable test --bin mikebom pnpm_v9_no_snapshots_emits_warn
```

**Expected**: single test passes. v9 lockfile with no `snapshots:` key → scan completes with all components carrying empty `depends`; test captures the `tracing::warn!` line naming the lockfile path + version.

## Scenario 6 — SC-006 pre-PR gate (mandatory)

```bash
./scripts/pre-pr.sh
```

Per the milestone-155 fix memory (`feedback_prepr_gate_bails_on_first_failure.md`):

```bash
cargo +stable test --workspace --no-fail-fast 2>&1 | grep -E '^---- .+ stdout ----'
```

**Expected**: only `sbomqs_spdx_score_meets_or_beats_cdx_across_ecosystems` appears (documented env-only flake). Every other test binary passes.

**Do not open a PR without both commands green.**

## Scenario 7 — SC-007 unit-test count (automated)

```bash
grep -cE "^\s+fn pnpm_v(6|9)" mikebom-cli/src/scan_fs/package_db/npm/pnpm_lock.rs
```

**Expected**: ≥7 (per research §R8 inventory: 8 unit tests total, all with `pnpm_v6_` or `pnpm_v9_` prefix). SC-007 floor easily cleared.

Plus the parity test:

```bash
grep -cE "^\s+fn pnpm_walks_same_dep_sections" mikebom-cli/src/scan_fs/package_db/npm/pnpm_lock.rs
```

**Expected**: 1 (SC-011).

## Scenario 8 — SC-008 integration test (automated)

```bash
cargo +stable test --workspace --test npm_pnpm_v9_dep_graph
```

**Expected**: passes. Synthesizes a 5-package testbed with peer-dep suffixes + snapshots-only edges; invokes release binary; parses CDX; asserts ≥1 non-trivial `dependsOn` list matches expected shape.

## Scenario 9 — SC-009 CHANGELOG (manual)

```bash
sed -n '/^## \[Unreleased\]/,/^## \[v/p' CHANGELOG.md \
    | grep -E 'pnpm-lock v9|snapshots|milestone 157|monotonic-additive'
```

**Expected**: entries present naming the v9 snapshots fix, Q1 clarification, argo-cd testbed impact, monotonic-additive golden regeneration, and reference to team's bug report.

## Scenario 10 — SC-010 wire-format guard (manual diff check)

```bash
# Prohibited: emitter changes, catalog changes, parity extractor changes.
git diff main --name-only -- mikebom-cli/src/generate/     # Expected: empty
git diff main --name-only -- mikebom-cli/src/parity/       # Expected: empty
git diff main --name-only -- docs/reference/sbom-format-mapping.md  # Expected: empty
git diff main --name-only -- mikebom-common/ mikebom-ebpf/ # Expected: empty
git diff main --name-only -- Cargo.toml Cargo.lock         # Expected: empty (no new deps)

# Non-pnpm goldens: also empty.
git diff main --name-only -- mikebom-cli/tests/fixtures/golden/ \
    | grep -v npm                                          # Expected: empty
```

**Expected**: all commands return empty. Only npm.cdx.json / npm.spdx.json / npm.spdx3.json goldens should change (per Q1 monotonic-additive regeneration).

## Scenario 11 — SC-011 pnpm/npm parity (automated)

```bash
cargo +stable test --bin mikebom pnpm_walks_same_dep_sections_as_package_lock_non_dev
```

**Expected**: passes. Asserts by construction that `PNPM_DEP_SECTIONS` matches the non-`devDependencies` subset of `package_lock.rs`'s walked sections.

## Post-merge — operator-cadence external review

SC-001 (argo-cd testbed) is manual. All other SCs are automated. Report SC-001 result in the PR merge comment; open a follow-up issue if argo-cd HEAD has shifted materially by shipping time.

## Known deferrals (spec Out of Scope)

- No `devDependencies:` sub-mapping handling (pnpm doesn't emit it under individual packages; dev status via `dev: true` boolean already handled).
- No pnpm workspace root file (`pnpm-workspace.yaml`) support.
- No non-registry dep resolution (git/tarball/file URLs continue to be dropped with a `debug` log).
- No integrity/hash changes.
