# Contract: Opt-in perf benchmark

**Feature**: 118-exclude-path-polish
**Date**: 2026-06-13
**Consumed by**: maintainer running `cargo +stable test --test exclude_path_perf -- --ignored`; future CI perf-lane invocations (out of scope for milestone 118)
**Spec mapping**: FR-011, SC-006

This contract defines the externally observable behavior of the opt-in perf benchmark for `--exclude-path` overhead.

## Test location

- **File**: `mikebom-cli/tests/exclude_path_perf.rs` (new)
- **Test function**: `exclude_path_does_not_exceed_1_10x_baseline`
- **Test attributes**: `#[test] #[ignore = "wall-clock perf test — opt in via `cargo test -- --ignored`; runs on Linux CI lane only per milestone-094"]`

## Invocation

**Maintainer-side opt-in**:

```bash
cargo +stable test --test exclude_path_perf -- --ignored --nocapture
```

The `--nocapture` flag is recommended so the median measurements emit to stdout for human review.

**Default `cargo test` invocation** (e.g., the pre-PR gate, the CI `Lint + test` job):

```bash
cargo +stable test --workspace
```

The perf test does NOT run in this invocation per the `#[ignore]` attribute. Its presence in the file is invisible to default runs.

## Fixture

**Source**: kusari-cli, resolved via `env!("MIKEBOM_FIXTURES_DIR")/kusari-cli`. This is the milestone-090 cached fixture, pinned via `tests/fixtures.rev`.

**Why this fixture** (per spec clarification Q1):
- Polyglot (Rust workspace + Go modules + Python tooling + Cargo + npm + pip surfaces) — exercises every walker that participates in `--exclude-path` enforcement
- Already cached locally; zero per-run clone cost
- Already maintained; pinned via the existing `tests/fixtures.rev` mechanism
- Matches the existing realistic-projects CI lane fixture set

## Measurement protocol

**Sampling**: median-of-5 per measurement condition. Per Decision 5 in research.md.

**Conditions**:

1. **Baseline**: scan kusari-cli with no `--exclude-path` flag.
2. **With-flag**: scan kusari-cli with `--exclude-path '**/testdata'`.

**Per-sample timing**:

```rust
fn time_scan(fixture: &Path, exclude_paths: &[&str]) -> Duration {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mikebom"));
    cmd.arg("--offline").arg("sbom").arg("scan")
        .arg("--path").arg(fixture)
        .arg("--output").arg("/dev/null")
        .arg("--no-deep-hash");
    for ep in exclude_paths {
        cmd.arg("--exclude-path").arg(ep);
    }
    let start = std::time::Instant::now();
    let out = cmd.output().expect("mikebom runs");
    let elapsed = start.elapsed();
    assert!(out.status.success(), "scan failed: {}", String::from_utf8_lossy(&out.stderr));
    elapsed
}
```

**Median computation**:

```rust
fn median(samples: &mut [Duration]) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}
```

**Budget assertion**:

```rust
let max_allowed = baseline_median.mul_f64(1.10);
assert!(
    excluded_median <= max_allowed,
    "perf: --exclude-path scan ({excluded_median:?}) exceeded 1.10× baseline ({baseline_median:?})",
);
```

## Performance contract

- **Budget**: with-flag median ≤ 1.10 × baseline median (SC-006)
- **Expected ratio in practice**: ≤1.05× given the `is_empty()` short-circuit's structural skip for non-matching candidates AND the `AtomicUsize` increment's nanosecond-cost-per-hit. Per-scan wall time on kusari-cli is ~3s; the new instrumentation adds <10 µs per scan.
- **Runtime**: ~30 seconds total (5 samples × 2 conditions × ~3s per scan)

## Out-of-band overrides

- **macOS skip pattern**: per the milestone-094 thermal-noise rationale, the strict budget assertion on macOS produces unreliable results. The test detects the host via `cfg!(target_os = "macos")` and downgrades the assertion to a measurement-only print on macOS hosts. Linux + Windows hosts run the strict assertion.

  ```rust
  if cfg!(target_os = "macos") {
      eprintln!("perf-bench measurement (macOS, advisory): baseline={baseline_median:?} excluded={excluded_median:?} ratio={ratio:.2}");
      return; // skip strict assertion
  }
  ```

- **No CI default-on**: the test does NOT run in the per-PR `Lint + test (linux-x86_64)` job. Future opt-in lane integration (a dedicated `perf.yml` workflow akin to `dual_format_perf.rs`'s lane) is out of scope for milestone 118.

## Failure modes

| Symptom | Diagnosis | Action |
|---|---|---|
| `cargo test` doesn't run the test | Expected — `#[ignore]` attribute. Run with `-- --ignored`. | — |
| `MIKEBOM_FIXTURES_DIR` env-var not set | Fixture cache wasn't bootstrapped (rare; build.rs handles this). | Run `cargo build` once to populate the cache. |
| Test fails on macOS | Thermal noise; expected per milestone 094. | Re-run on Linux; the macOS lane reports measurement-only. |
| Test fails on Linux with ratio < 1.10 but consistently flakes | Could indicate genuine regression OR runner contention. | Re-run the test 3 times; if 2/3 fail, investigate the recent commits touching `walk.rs` / `exclude_path.rs` / `scan_cmd.rs`. |
| Test fails on Linux with ratio >> 1.10 | Genuine regression. The `AtomicUsize` increment shouldn't add >>1% even at 100% hit rate; verify the `is_empty()` short-circuit at `walk.rs:224` is still in place. | Revert the offending commit OR escalate. |

## Out of scope (for milestone 118)

- **Dedicated CI perf lane**: a follow-up issue could add a `perf.yml` workflow that runs this benchmark nightly or on perf-relevant PR labels. Not blocking the milestone-118 ship.
- **Multi-pattern budget**: the current contract tests one pattern (`**/testdata`). Multi-pattern overhead could be measured in a follow-up but doesn't change the structural conclusion (per-hit cost is identical regardless of which pattern matched).
- **Cold-cache measurement**: all measurements are post-warm (the kusari-cli fixture is on local SSD; OS page cache eats the I/O cost after sample 1). Cold-cache measurement is out of scope; the budget assumes warm-cache operation, matching how operators run mikebom in CI.
