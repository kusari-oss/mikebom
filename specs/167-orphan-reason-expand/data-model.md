# Data Model: milestone 167 — orphan-reason vocabulary extension

**Date**: 2026-07-06
**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Research**: [research.md](./research.md)

Phase 1 data model. Milestone 167 extends existing infrastructure — new module `orphan_reason.rs` + `GraphCompletenessResult` API extension + call-site addition in `scan_fs/mod.rs`.

## Rust types

### E1 — `GraphCompletenessResult.reachable_set` (EDITED field)

**Location**: `mikebom-cli/src/generate/graph_completeness/mod.rs:75`

**Pre-167** (current shape):

```rust
pub struct GraphCompletenessResult {
    pub value: GraphCompletenessValue,
    pub reachable_count: usize,
    pub total_count: usize,
    pub orphan_count: usize,
    pub reason_codes: Vec<ReasonCode>,
}
```

**Post-167**:

```rust
pub struct GraphCompletenessResult {
    pub value: GraphCompletenessValue,
    pub reachable_count: usize,
    pub total_count: usize,
    pub orphan_count: usize,
    pub reason_codes: Vec<ReasonCode>,
    // Milestone 167 (T003): expose the BFS-computed reachable set so
    // downstream classifier (orphan_reason.rs) can determine per-component
    // orphan status without re-computing BFS.
    pub reachable_set: std::collections::HashSet<String>,
}
```

**Rationale**: The `multi_source_bfs` function in `bfs.rs:141` already computes this set; the result is currently discarded after use in the mod.rs pass/fail decision. Exposing the field is a 3-line change.

**Validation rules**: `reachable_set.len() == reachable_count`. This is an invariant that helper `trivially_complete()` + `unknown()` constructors must preserve.

### E2 — `OrphanReasonCode` enum (NEW type)

**Location**: NEW at `mikebom-cli/src/generate/orphan_reason.rs`

```rust
/// Milestone 167 (T004) — the C45 `mikebom:orphan-reason` vocabulary
/// after extension. Total 5 codes: 2 preserved from m061 + 3 new.
///
/// Per FR-005 priority order (most-specific to least-specific):
/// 1. `stale-go-sum-entry` (Go, BFS-unreachable, same-name sibling reachable)
/// 2. `dead-lockfile-entry` (npm, BFS-unreachable, same-name sibling reachable)
/// 3. `hoisted-unused` (npm, BFS-unreachable, no same-name reachable sibling)
/// 4. `unresolved-indirect-require` (Go, BFS-unreachable, no same-name sibling)
///    — preserved from m061; emit-time classifier falls through to this if
///    Go-reader-time already set it.
/// 5. `flat-attached-fallback` (Go, backfill-attached, technically BFS-reachable)
///    — preserved from m061; NEVER overwritten by m167 classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanReasonCode {
    StaleGoSumEntry,
    DeadLockfileEntry,
    HoistedUnused,
    UnresolvedIndirectRequire,
    FlatAttachedFallback,
}

impl OrphanReasonCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::StaleGoSumEntry => "stale-go-sum-entry",
            Self::DeadLockfileEntry => "dead-lockfile-entry",
            Self::HoistedUnused => "hoisted-unused",
            Self::UnresolvedIndirectRequire => "unresolved-indirect-require",
            Self::FlatAttachedFallback => "flat-attached-fallback",
        }
    }
}
```

**Validation rules**: `as_str()` values MUST NEVER change once a code is documented (SBOM consumers may key on them). New codes added in future milestones extend the enum; existing values are frozen.

### E3 — `classify_orphans` function (NEW)

**Location**: `mikebom-cli/src/generate/orphan_reason.rs`

**Signature**:

```rust
/// Milestone 167 (T004, implements m167 FR-001 through FR-006) —
/// emit-time classifier. Iterates every `pkg:golang/*` and `pkg:npm/*`
/// component and stamps `mikebom:orphan-reason` if BFS-unreachable per
/// the Q1 clarification.
///
/// Called from `scan_fs::mod.rs` after `compute_graph_completeness`
/// (which populates `GraphCompletenessResult.reachable_set`).
///
/// Returns a per-code drop count for FR-008 tracing observability.
pub fn classify_orphans(
    components: &mut [ResolvedComponent],
    reachable_set: &std::collections::HashSet<String>,
) -> OrphanReasonCounts {
    // Build same-name index up-front so per-orphan lookup is O(1).
    let mut by_name: std::collections::HashMap<(String, String), Vec<String>> =
        std::collections::HashMap::new();
    for c in components.iter() {
        let ecosystem = c.purl.ecosystem().to_string();
        if ecosystem != "npm" && ecosystem != "golang" { continue; }
        let key = (ecosystem, c.purl.name().to_string());
        by_name.entry(key).or_default().push(c.purl.as_str().to_string());
    }

    let mut counts = OrphanReasonCounts::default();

    for c in components.iter_mut() {
        let ecosystem = c.purl.ecosystem().to_string();
        if ecosystem != "npm" && ecosystem != "golang" { continue; }
        let purl_str = c.purl.as_str().to_string();
        if reachable_set.contains(&purl_str) { continue; } // Not orphan.

        // Preserve `flat-attached-fallback` from Go-reader-time — never overwrite.
        if let Some(existing) = c.extra_annotations.get("mikebom:orphan-reason")
                                .and_then(|v| v.as_str()) {
            if existing == "flat-attached-fallback" {
                counts.flat_attached_fallback += 1;
                continue;
            }
        }

        // FR-005 priority — check same-name reachable sibling first.
        let key = (ecosystem.clone(), c.purl.name().to_string());
        let siblings = by_name.get(&key).cloned().unwrap_or_default();
        let has_reachable_sibling = siblings.iter()
            .any(|s| s != &purl_str && reachable_set.contains(s));

        let code = match (ecosystem.as_str(), has_reachable_sibling) {
            ("golang", true)  => OrphanReasonCode::StaleGoSumEntry,
            ("npm",    true)  => OrphanReasonCode::DeadLockfileEntry,
            ("npm",    false) => OrphanReasonCode::HoistedUnused,
            ("golang", false) => OrphanReasonCode::UnresolvedIndirectRequire,
            _ => unreachable!(), // guarded above
        };

        counts.tally(code);

        c.extra_annotations.insert(
            "mikebom:orphan-reason".to_string(),
            serde_json::Value::String(code.as_str().to_string()),
        );
    }
    counts
}

#[derive(Debug, Default)]
pub struct OrphanReasonCounts {
    pub stale_go_sum_entry: usize,
    pub dead_lockfile_entry: usize,
    pub hoisted_unused: usize,
    pub unresolved_indirect_require: usize,
    pub flat_attached_fallback: usize,
}

impl OrphanReasonCounts {
    pub fn tally(&mut self, code: OrphanReasonCode) {
        match code {
            OrphanReasonCode::StaleGoSumEntry           => self.stale_go_sum_entry += 1,
            OrphanReasonCode::DeadLockfileEntry         => self.dead_lockfile_entry += 1,
            OrphanReasonCode::HoistedUnused             => self.hoisted_unused += 1,
            OrphanReasonCode::UnresolvedIndirectRequire => self.unresolved_indirect_require += 1,
            OrphanReasonCode::FlatAttachedFallback      => self.flat_attached_fallback += 1,
        }
    }
}
```

**Rationale**: Single-pass O(N) with O(1) same-name lookup via pre-built index. FR-005 priority encoded via pattern match on `(ecosystem, has_reachable_sibling)`.

**Special-case: `flat-attached-fallback` preservation**: if the Go reader already emitted this code, the classifier skips (per R3). This preserves m061 backward-compat.

### E4 — `scan_fs::mod.rs` call-site (EDITED)

**Location**: `mikebom-cli/src/scan_fs/mod.rs` — the emission-pipeline entry point where `compute_graph_completeness` is invoked.

**Post-167** (schematic — verify exact insertion point at implementation time):

```rust
// (existing) Compute graph completeness — provides reachable_set.
let gc = compute_graph_completeness(&components, &relationships, &root);

// Milestone 167 (T005, implements m167 FR-001 through FR-008):
// classify orphans + stamp mikebom:orphan-reason.
let counts = crate::generate::orphan_reason::classify_orphans(
    &mut components,
    &gc.reachable_set,
);
tracing::info!(
    orphan_reason_stale_go_sum_entry           = counts.stale_go_sum_entry,
    orphan_reason_dead_lockfile_entry          = counts.dead_lockfile_entry,
    orphan_reason_hoisted_unused               = counts.hoisted_unused,
    orphan_reason_unresolved_indirect_require  = counts.unresolved_indirect_require,
    orphan_reason_flat_attached_fallback       = counts.flat_attached_fallback,
    "orphan-reason classification complete"
);
```

**Change surface**: ~15 lines added. Existing pipeline unchanged.

## Wire types

**None.** Milestone 167 changes only the SET of possible VALUES for the existing `mikebom:orphan-reason` annotation. The CDX 1.6 `properties[]` shape, SPDX 2.3 `annotations[]` shape, and SPDX 3.0.1 `Annotation` element shape are UNCHANGED (still single-string value). The C45 parity-catalog row is unchanged.

## Relationships

```text
scan_fs::mod.rs::do_scan (existing entry point)
    ↓
    ↓ (existing) compute_graph_completeness
    ↓         ← reachable_set now exposed in GraphCompletenessResult (E1)
    ↓
    ↓ (NEW T005) classify_orphans(&mut components, &gc.reachable_set)
    ↓         ← reads reachable_set + all_components
    ↓         ← writes mikebom:orphan-reason on each BFS-unreachable Go/npm component
    ↓         ← returns OrphanReasonCounts
    ↓
    ↓ (NEW T005) tracing::info!(...) with FR-008 fields
    ↓
    ↓ (existing) format-specific emission — CDX, SPDX 2.3, SPDX 3
    ↓         ← extra_annotations flows through unchanged into wire format
```

## State transitions

**None.** Milestone 167 is a pure post-processing pass at emission time.

## Data volume assumptions

- Per-scan: 100-5000 Go+npm components. podman-desktop = 2694 npm; K8s = 487 Go; ArgoCD = 1735 Go+npm combined.
- Orphan-classification complexity: O(N × K) where N = components, K = orphan count. On podman-desktop: 2694 × 12 = ~32K comparisons. Sub-millisecond.
- Memory: `by_name` HashMap ≈ 100 bytes/entry × 3000 entries ≈ 300 KB peak per scan.

## Validation rules (aggregated)

| Rule | Enforcement |
|------|-------------|
| Every Go/npm BFS-unreachable component receives an orphan-reason (FR-001) | Enforced by construction — loop iterates all components; match arm covers all combos. Verified by unit test T007. |
| Non-orphan components carry no orphan-reason (FR-006) | Enforced by `if reachable_set.contains → continue`. Verified by unit test T008. |
| FR-005 priority: most-specific wins | Enforced by pattern match ordering on `(ecosystem, has_reachable_sibling)`. Verified by unit tests T009-T012. |
| `flat-attached-fallback` preserved (m061 backward-compat) | Enforced by explicit skip if existing value == "flat-attached-fallback". Verified by unit test T013. |
| FR-008 log fires with correct per-ecosystem counts | Enforced by tracing::info! at scan_fs::mod.rs. Verified by integration test T014. |
| C45 wire format unchanged | Enforced by scope — annotation VALUE changes, KEY is unchanged. Verified by existing golden tests continuing to pass on non-Go/npm ecosystems (SC-005). |
