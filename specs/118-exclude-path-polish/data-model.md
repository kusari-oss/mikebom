# Data Model — Milestone 113 `--exclude-path` polish bundle

**Feature**: 118-exclude-path-polish
**Date**: 2026-06-13

This feature extends ONE existing struct (`ExclusionSet`) with ONE new field (`AtomicUsize` counter) and ONE existing tracing-event shape (the scan-end `tracing::info!` summary) with two new field columns. There are no new persisted entities — the counter lives in process memory for the duration of one scan, then dies with the `ExclusionSet`. The tracing summary is stderr-only; nothing it emits flows into the SBOM payload.

## Entity 1 — `ExclusionSet.suppressed_dirs` (counter extension)

**Location**: New field on the existing `ExclusionSet` struct at `mikebom-cli/src/scan_fs/package_db/exclude_path.rs:108-228`.

**Field definition**:

```rust
pub(crate) struct ExclusionSet {
    entries: Vec<ExclusionEntry>,
    pattern_set: Option<GlobSet>,
    literal_paths: Vec<String>,
    // Milestone 118 — counter incremented by safe_walk every time a
    // candidate directory matches an exclusion entry. Read at scan
    // end by scan_cmd.rs:1750 to populate the FR-010 tracing summary.
    // Reset implicitly per-scan via ExclusionSet's one-per-scan lifetime.
    suppressed_dirs: std::sync::atomic::AtomicUsize,
}
```

**Lifecycle**:

1. **Construction** (`exclude_path.rs:130-150` constructor): counter initialized to 0 via `AtomicUsize::new(0)`. Done implicitly when the `ExclusionSet` is built from CLI args + env-var at `main.rs:292-301`.
2. **Increment** (`walk.rs:227-230` modified): every call to `cfg.exclude_set.matches(&rel_str)` that returns `true` is immediately followed by `cfg.exclude_set.suppressed_dirs.fetch_add(1, Relaxed)`. The increment lives next to the existing `tracing::debug!` skip-event emission for visibility.
3. **Read** (`scan_cmd.rs:1750-1754` modified): post-scan, `exclude_set.suppressed_dirs.load(Relaxed)` returns the final count. Used as the `suppressed_dirs` field on the `tracing::info!` line.
4. **Destruction**: implicit when the `ExclusionSet` goes out of scope at scan-cmd exit. No persistence.

**Invariants**:

1. **Reset per-scan**: each scan invocation constructs a fresh `ExclusionSet`; the counter starts at 0. There is no global / static state.
2. **Monotonic non-decreasing during a scan**: `fetch_add(1, Relaxed)` only increases the counter; no decrement path exists.
3. **Free in the no-flag path**: `walk.rs:224`'s `if cfg.exclude_set.is_empty() { return }` short-circuit means the counter is never incremented when no exclusions are supplied. Combined with FR-010's emission gating (`tracing::info!` only includes the new fields when `!is_empty()`), the no-flag scan produces byte-identical stderr to pre-118 mikebom.
4. **Relaxed ordering is sufficient**: the counter has no read/write dependency on other memory. Per Decision 1 in research.md.

## Entity 2 — `ExclusionSet::count_literals()` + `count_patterns()` (accessor helpers)

**Location**: Two new methods on the same struct.

```rust
impl ExclusionSet {
    /// Milestone 118 — count of LITERAL entries (`tests/fixtures` form)
    /// in the set. Used by FR-010 tracing summary to surface the entry
    /// breakdown. O(1) — wraps `self.literal_paths.len()`.
    pub(crate) fn count_literals(&self) -> usize {
        self.literal_paths.len()
    }

    /// Milestone 118 — count of PATTERN entries (`**/testdata` form)
    /// in the set. O(N) over `self.entries` filtering for `Pattern`
    /// variant; N ≤ 100 in practice (operator-supplied list).
    pub(crate) fn count_patterns(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, ExclusionEntry::Pattern(_)))
            .count()
    }
}
```

**Rationale**: the FR-010 tracing summary distinguishes "applied N entries" from "suppressed M directories." Operators reading stderr want to know BOTH the input size (entries) AND the effect size (suppressed dirs). The two accessors are mechanical wrappers; no new data fields beyond the existing `entries` + `literal_paths` arrays.

**Invariants**: each accessor is a pure function over the immutable fields populated at construction; values never change post-construction.

## Entity 3 — Scan-end `tracing::info!` summary (event-shape extension)

**Location**: The existing `tracing::info!` call at `mikebom-cli/src/cli/scan_cmd.rs:1750-1754`.

**Current shape** (pre-118):

```rust
tracing::info!(
    components = components.len(),
    relationships = relationships.len(),
    "scan complete"
);
```

**Post-118 shape**:

```rust
if !exclude_set.is_empty() {
    tracing::info!(
        components = components.len(),
        relationships = relationships.len(),
        excluded_entries = exclude_set.entries().len(),
        excluded_literals = exclude_set.count_literals(),
        excluded_patterns = exclude_set.count_patterns(),
        suppressed_dirs = exclude_set.suppressed_dirs.load(std::sync::atomic::Ordering::Relaxed),
        "scan complete"
    );
} else {
    tracing::info!(
        components = components.len(),
        relationships = relationships.len(),
        "scan complete"
    );
}
```

**Stderr output examples**:

| Scenario | Emitted line |
|---|---|
| No `--exclude-path` | `INFO mikebom::cli::scan_cmd: scan complete components=42 relationships=58` |
| `--exclude-path tests/fixtures` (1 literal, 137 dirs suppressed) | `INFO mikebom::cli::scan_cmd: scan complete components=42 relationships=58 excluded_entries=1 excluded_literals=1 excluded_patterns=0 suppressed_dirs=137` |
| `--exclude-path '**/testdata' --exclude-path '**/_archive'` (0 literals, 2 patterns, 42 dirs suppressed) | `INFO mikebom::cli::scan_cmd: scan complete components=42 relationships=58 excluded_entries=2 excluded_literals=0 excluded_patterns=2 suppressed_dirs=42` |

**Invariants** (per FR-010):

1. **Byte-identity preserved when `is_empty()`**: the no-exclusion branch emits the exact same two-field shape as pre-118. Operators running scans without `--exclude-path` see no change in stderr.
2. **Field names are stable**: `excluded_entries`, `excluded_literals`, `excluded_patterns`, `suppressed_dirs` are the contract field names. Renaming them is a contract change.
3. **Subscriber compatibility**: tracing-subscriber, tracing-tree, tracing-json all handle additive field-emission gracefully. No new tracing dependency.
4. **No SBOM-payload effect**: this summary is stderr-only. The SBOM payload (CDX JSON, SPDX 2.3, SPDX 3) is unaffected. The existing milestone-113 `mikebom:exclude-path` envelope annotation remains the SBOM-payload signal.

## Entity 4 — Perf-bench measurement protocol (test infrastructure)

**Location**: `mikebom-cli/tests/exclude_path_perf.rs` (new file).

**Sampling**: median-of-5 per measurement condition (baseline + with-flag). Per Decision 5 in research.md.

**Wall-time measurement**:

```rust
fn time_scan(fixture: &Path, exclude_paths: &[&str]) -> Duration {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mikebom"));
    cmd.arg("--offline").arg("sbom").arg("scan").arg("--path").arg(fixture)
        .arg("--output").arg("/dev/null").arg("--no-deep-hash");
    for ep in exclude_paths {
        cmd.arg("--exclude-path").arg(ep);
    }
    let start = Instant::now();
    let out = cmd.output().expect("mikebom runs");
    let elapsed = start.elapsed();
    assert!(out.status.success(), "scan failed: {}", String::from_utf8_lossy(&out.stderr));
    elapsed
}
```

**Budget assertion** (≤1.10×):

```rust
let baseline_median = median(&[time_scan(&fixture, &[]); 5]);
let excluded_median = median(&[time_scan(&fixture, &["**/testdata"]); 5]);
let max_allowed = baseline_median.mul_f64(1.10);
assert!(
    excluded_median <= max_allowed,
    "perf: --exclude-path scan ({excluded_median:?}) exceeded {:.0}% of baseline ({baseline_median:?})",
    max_allowed.as_secs_f64() / baseline_median.as_secs_f64() * 100.0,
);
```

**Fixture**: kusari-cli, resolved via `env!("MIKEBOM_FIXTURES_DIR")/kusari-cli` (the milestone-090 cache).

**Invariants** (per FR-011):

1. **`#[ignore]` annotation**: the test does NOT run in the default `cargo test` invocation. Opt-in only via `cargo +stable test --test exclude_path_perf -- --ignored`.
2. **Linux-only CI gate**: per milestone-094's macOS thermal-noise exemption, the test's strict assertion is skipped on macOS (the measurement is still emitted to stderr for observation, but no CI failure).
3. **No persisted measurement**: each invocation re-times from scratch. No baseline file checked into the repo; the budget is computed per-run from that invocation's own baseline measurement.

## Validation rules summary

| Rule | Source | Where enforced |
|---|---|---|
| Counter is reset per-scan | Decision 1 / data-model invariant 1 | `ExclusionSet` constructor at `exclude_path.rs:130-150` |
| Counter increment uses Relaxed ordering | Decision 1 / data-model invariant 4 | `walk.rs:227-230` modified code |
| Tracing summary preserves byte-identity when empty | FR-010 / data-model invariant 1 (entity 3) | `scan_cmd.rs:1750-1754` modified branch |
| Field names match contract | data-model invariant 2 (entity 3) | `scan_cmd.rs:1750-1754` modified line; reviewed by integration test if added |
| Perf bench is `#[ignore]`d | FR-011 / data-model invariant 1 (entity 4) | `exclude_path_perf.rs` test attributes |
| Perf bench skips strict assertion on macOS | Decision 5 + milestone-094 convention | `exclude_path_perf.rs` conditional skip |
| Fixture is kusari-cli (not a new vendored fixture) | Spec clarification Q1 / Decision 3 | `exclude_path_perf.rs` fixture resolution |

No state transitions (no lifecycle FSM); the counter is monotonic-non-decreasing during a scan and dies at scan exit.
