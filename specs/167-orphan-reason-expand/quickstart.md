# Quickstart — milestone 167 (orphan-reason vocabulary expansion)

**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Contracts**: [contracts/README.md](./contracts/README.md)

## Prerequisites

- Rust stable (workspace toolchain — same as milestones 001–166)
- `git`, `cargo`
- Fresh checkout on branch `167-orphan-reason-expand`

## Verification path

### Step 1: Build

```bash
cargo +stable build --workspace
```

**Expected**: clean compile; the new `mikebom-cli/src/generate/orphan_reason.rs` module compiles, `GraphCompletenessResult.reachable_set` field added, call-site in `scan_fs/mod.rs` invokes classifier.

### Step 2: Unit tests

```bash
cargo +stable test -p mikebom-cli orphan_reason::
```

**Expected**: unit tests for the classifier pass:
- `stale-go-sum-entry` emitted when Go BFS-unreachable + same-name reachable sibling exists
- `dead-lockfile-entry` emitted when npm BFS-unreachable + same-name reachable sibling exists
- `hoisted-unused` emitted when npm BFS-unreachable + NO same-name reachable sibling
- `unresolved-indirect-require` preserved when Go BFS-unreachable + NO same-name sibling AND Go-reader-time had already set it
- `flat-attached-fallback` preserved (never overwritten) on backfill-attached Go modules

### Step 3: Integration test (real fixture)

```bash
cargo +stable test -p mikebom-cli --test orphan_reason_integration
```

**Expected**: end-to-end scan against a synthesized npm fixture (with hoisted-but-declared-only dep) or Go fixture (with stale go.sum entry) emits `mikebom:orphan-reason` with the correct code on the orphan-classified components.

### Step 4: Full pre-PR gate

```bash
./scripts/pre-pr.sh
```

**Expected**: `4000+ passed; 0 failed` across all workspace tests; clippy zero errors.

### Step 5: Manual smoke on podman-desktop or ArgoCD (optional)

Reuse milestone 165's fixtures:

```bash
target/release/mikebom sbom scan \
    --path ~/mikebom-audit-fixtures/argo-cd \
    --format cyclonedx-json \
    --output /tmp/argocd.cdx.json

jq '[.components[] |
     select(.properties[]?.name == "mikebom:orphan-reason") |
     .properties[] | select(.name == "mikebom:orphan-reason") | .value] |
     group_by(.) | map({code: .[0], count: length})' \
    /tmp/argocd.cdx.json
```

**Expected**: histogram shows the 5-code vocabulary in use; no orphan components lack the annotation; no non-orphan components carry the annotation.

Compare against pre-167 baseline (should reveal `hoisted-unused` + `dead-lockfile-entry` counts previously invisible).

### Step 6: Observability check

```bash
target/release/mikebom -v sbom scan --path ~/mikebom-audit-fixtures/argo-cd \
    --format cyclonedx-json --output /dev/null 2>&1 \
    | grep 'orphan-reason classification complete'
```

**Expected**: single tracing::info! line emitted with the five FR-008 fields:
- `orphan_reason_stale_go_sum_entry=<N>`
- `orphan_reason_dead_lockfile_entry=<N>`
- `orphan_reason_hoisted_unused=<N>`
- `orphan_reason_unresolved_indirect_require=<N>`
- `orphan_reason_flat_attached_fallback=<N>`

## Rollback / bail-out path

Milestone 167 is a pure additive metadata milestone. If a regression surfaces:

1. Revert the merge commit — this restores the pre-167 2-code vocabulary. No golden-file breakage on non-Go/npm ecosystems (SC-005 preserves this invariant).
2. C45 catalog row can be manually reverted to 2-code documentation.
3. No wire-format breakage — the annotation KEY is unchanged; only VALUES expanded.
