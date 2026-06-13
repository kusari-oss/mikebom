# Contract: Scan-end `tracing::info!` summary

**Feature**: 118-exclude-path-polish
**Date**: 2026-06-13
**Consumed by**: operator reading scan stderr; tracing-subscriber backends (text / JSON / tree); future structured-log aggregators
**Spec mapping**: FR-009, FR-010

This contract defines the externally observable shape of the scan-end `tracing::info!` summary line. Per FR-010's emission gating, the contract has TWO arms: one for scans with zero `--exclude-path` entries (byte-identical to pre-118 mikebom) and one for scans with one or more entries.

## Event location

- **Module**: `mikebom::cli::scan_cmd`
- **Source line**: `mikebom-cli/src/cli/scan_cmd.rs:1750-1754` (pre-118 baseline; post-118 the branch extends as documented below)
- **Trigger**: emitted exactly once per scan, immediately after walker discovery and dedup complete, before enrichment phase
- **Severity**: `INFO` — visible at `RUST_LOG=info` or higher

## Arm 1 — Zero `--exclude-path` entries

**Trigger condition**: `exclude_set.is_empty()` returns `true` at scan-end.

**Emitted shape** (byte-identical to pre-118 milestone-110 emission):

```rust
tracing::info!(
    components = components.len(),
    relationships = relationships.len(),
    "scan complete"
);
```

**Stderr example**:
```text
2026-06-13T22:45:01.123456Z INFO mikebom::cli::scan_cmd: scan complete components=42 relationships=58
```

## Arm 2 — One or more `--exclude-path` entries

**Trigger condition**: `!exclude_set.is_empty()` at scan-end.

**Emitted shape**:

```rust
tracing::info!(
    components = components.len(),
    relationships = relationships.len(),
    excluded_entries = exclude_set.entries().len(),
    excluded_literals = exclude_set.count_literals(),
    excluded_patterns = exclude_set.count_patterns(),
    suppressed_dirs = exclude_set.suppressed_dirs.load(Ordering::Relaxed),
    "scan complete"
);
```

**Field reference**:

| Field | Type | Source | Notes |
|---|---|---|---|
| `components` | `usize` | `components.len()` | Pre-existing field; unchanged from pre-118 |
| `relationships` | `usize` | `relationships.len()` | Pre-existing field; unchanged from pre-118 |
| `excluded_entries` | `usize` | `exclude_set.entries().len()` | Total entries supplied (CLI `--exclude-path` + `MIKEBOM_EXCLUDE_PATH` env-var, deduplicated) |
| `excluded_literals` | `usize` | `exclude_set.count_literals()` | Subset of `excluded_entries` that are literal directory paths (e.g., `tests/fixtures`) |
| `excluded_patterns` | `usize` | `exclude_set.count_patterns()` | Subset of `excluded_entries` that are glob patterns (e.g., `**/testdata`) |
| `suppressed_dirs` | `usize` | `exclude_set.suppressed_dirs.load(Relaxed)` | Count of times `safe_walk` matched a candidate against an exclusion entry. Counted per-match-attempt; a single directory matched by two entries counts twice (rare in practice given operator typically supplies non-overlapping patterns) |

**Stderr example** (operator passed `--exclude-path tests/fixtures --exclude-path '**/testdata'`):

```text
2026-06-13T22:45:01.123456Z INFO mikebom::cli::scan_cmd: scan complete components=42 relationships=58 excluded_entries=2 excluded_literals=1 excluded_patterns=1 suppressed_dirs=137
```

## Invariants

1. **`excluded_entries == excluded_literals + excluded_patterns`** at all times. The two subsets partition the total. Verified by the `ExclusionSet` constructor's classification logic at `exclude_path.rs:130-150`.
2. **`suppressed_dirs >= 0`** always (counter is `AtomicUsize`; underflow impossible).
3. **`suppressed_dirs == 0` is valid**: it means the operator supplied entries but no candidate directory matched any of them. The summary still emits. Operators reading this signal know their entries were "supplied but unused" — useful for debugging stale exclusion configs.
4. **Subscriber compatibility**: structured field-emission works under tracing-subscriber's text formatter, JSON formatter, and tracing-tree. No field is required by any subscriber; missing fields render as omitted-by-format.
5. **No SBOM-payload effect**: this contract describes stderr output ONLY. The SBOM payload (CDX JSON, SPDX 2.3, SPDX 3) is unaffected. The existing milestone-113 `mikebom:exclude-path` envelope annotation remains the SBOM-payload signal for exclusion context.

## Out-of-band considerations

- **`MIKEBOM_LOG=debug`**: at debug level, per-walker `tracing::debug!` skip-event emission from `safe_walk:228-232` (already in place since milestone 114) provides per-directory granularity. The scan-end summary is the per-scan aggregate; the per-walker debug events are the per-directory breakdown.
- **JSON output mode**: tracing-subscriber-json renders the line as `{"timestamp": ..., "level": "INFO", "target": "mikebom::cli::scan_cmd", "message": "scan complete", "fields": {"components": 42, "relationships": 58, "excluded_entries": 2, ...}}`. Operators aggregating logs in ELK / Splunk / Datadog see the new fields appear automatically.
- **`--quiet` flag** (if added in a future milestone): would suppress the INFO line at the subscriber level, not at the emission level. The contract above describes the EMITTED shape; subscriber-side filtering is orthogonal.

## Backwards compatibility

Pre-merge of milestone 118: only Arm 1 exists. Post-merge: Arm 1 is byte-identical (no operator-visible change for non-exclusion scans). Arm 2 is additive — operators who currently use `--exclude-path` see new fields in the existing INFO line, but the message string ("scan complete") and the two pre-existing fields are unchanged.

There is no transition mode; the change is total at merge time. Operators reading the summary line via structured-log aggregation tools that pin field schemas should be aware that the four new fields may appear starting at milestone-118 merge.
