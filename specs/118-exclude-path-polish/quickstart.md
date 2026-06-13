# Quickstart — Reading and Verifying the Milestone 118 `--exclude-path` polish

**Feature**: 118-exclude-path-polish
**Audience**: an operator running mikebom scans who uses `--exclude-path`; a contributor adding a new test for the flag's behavior; a maintainer running the opt-in perf benchmark.

## The TL;DR

After this feature ships, `--exclude-path` is the same flag with three new affordances:

1. **Per-ecosystem regression coverage** — every ecosystem (Cargo, npm, pip, gem, Maven, Go source, Go binary) has an integration test asserting `--exclude-path` works for that ecosystem. Future changes to ecosystem readers can't silently break exclusion without CI catching it.
2. **Scan-end summary line** — operators reading scan stderr see a `tracing::info!` summary including how many entries were applied and how many directories were suppressed. Per-walker debug events emit at `MIKEBOM_LOG=debug` for drill-down.
3. **Opt-in perf benchmark** — maintainers can run `cargo test --test exclude_path_perf -- --ignored` to verify the flag's overhead is within 10% of the no-flag baseline on a polyglot fixture.

## Five-minute walkthrough — operator using `--exclude-path` after milestone 118

You run a polyglot project scan with two exclusion entries:

```bash
mikebom sbom scan --path . \
    --exclude-path tests/fixtures \
    --exclude-path '**/_archive' \
    --output myscan.cdx.json
```

Stderr emits (filtered to relevant lines):

```text
INFO mikebom::cli::scan_cmd: scan starting root=.
INFO mikebom::cli::scan_cmd: scan complete components=42 relationships=58 \
     excluded_entries=2 excluded_literals=1 excluded_patterns=1 suppressed_dirs=137
INFO mikebom::cli::scan_cmd: wrote SBOM artifact format=cyclonedx-json path=myscan.cdx.json
```

The new fields on the "scan complete" line tell you:
- **`excluded_entries=2`**: you supplied 2 entries (matches the CLI flags + any `MIKEBOM_EXCLUDE_PATH` env-var entries)
- **`excluded_literals=1`**: one of them was a literal path (`tests/fixtures`)
- **`excluded_patterns=1`**: one was a glob pattern (`**/_archive`)
- **`suppressed_dirs=137`**: the walkers skipped 137 directories total across the scan

The SBOM payload (`myscan.cdx.json`) is unchanged from pre-118 milestone-113 behavior; only the stderr summary gains the new fields.

### Drilling down with `MIKEBOM_LOG=debug`

If you want to see WHICH directories got skipped:

```bash
MIKEBOM_LOG=debug mikebom sbom scan --path . \
    --exclude-path '**/_archive' \
    --output myscan.cdx.json 2>&1 \
  | grep 'safe_walk: skipping'
```

You'll see per-directory lines like:

```text
DEBUG mikebom::scan_fs::walk: safe_walk: skipping directory matched by milestone-113 ExclusionSet \
      candidate=services/payment/_archive/legacy-v1 cause=exclude-path
```

(One line per skipped directory. The milestone-114 `safe_walk` helper centralizes this emission; every per-ecosystem walker that delegates to `safe_walk` produces the same shape.)

## Five-minute walkthrough — contributor adding a new integration test

You're adding a new ecosystem reader (say, an Elixir Mix project) and want to verify `--exclude-path` correctly suppresses its components. The existing milestone-113 tests + milestone-118 polish tests at `mikebom-cli/tests/exclude_path_integration.rs` are your template.

```rust
#[test]
fn elixir_fixture_suppressed_under_tests_fixtures() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // Write a real Mix project at <tmp>/real-app/
    write_mix_project(tmp.path(), "real-app", "real-app");
    // Write a fixture Mix project at <tmp>/tests/fixtures/mix-fixture/
    write_mix_project(tmp.path(), "tests/fixtures/mix-fixture", "fixture-app");

    // Baseline scan: both projects present.
    let (status, sbom) = run_scan(tmp.path(), &[]);
    assert!(status.success());
    let names = component_names(&sbom);
    assert!(names.contains("real-app"));
    assert!(names.contains("fixture-app"));

    // Excluded scan: only the real project present.
    let (status, sbom) = run_scan(tmp.path(), &["tests/fixtures"]);
    assert!(status.success());
    let names = component_names(&sbom);
    assert!(names.contains("real-app"));
    assert!(!names.contains("fixture-app"));
}
```

The helpers (`run_scan`, `component_names`) are private to `exclude_path_integration.rs` and already cover this pattern. No new helper modules needed; just use the existing ones.

### Verifying the scan-end summary in your test

If you want to assert the new tracing summary fires, capture stderr from the scan invocation and grep for the fields:

```rust
let stderr = String::from_utf8_lossy(&output.stderr);
assert!(stderr.contains("excluded_entries=1"));
assert!(stderr.contains("excluded_literals=1"));
assert!(stderr.contains("suppressed_dirs="));
```

(The exact `suppressed_dirs` count depends on the fixture's directory layout; assert the field is present rather than a specific value.)

## Five-minute walkthrough — maintainer running the opt-in perf benchmark

You're reviewing a PR that touches `walk.rs` or `exclude_path.rs` and want to verify it doesn't regress the `--exclude-path` overhead. Run:

```bash
cargo +stable test --test exclude_path_perf -- --ignored --nocapture
```

Expected output (Linux):

```text
running 1 test
test exclude_path_does_not_exceed_1_10x_baseline ... ok
        baseline_median=3.142s
        excluded_median=3.225s
        ratio=1.026

test result: ok. 1 passed; 0 failed; 0 ignored
```

The test passes when `excluded_median ≤ baseline_median × 1.10`. The actual ratio in healthy steady-state should be ≤1.05× per the `AtomicUsize`-based counter's nanosecond-cost-per-hit + the `is_empty()` short-circuit's no-flag-path skip.

**On macOS**: the strict assertion is downgraded to advisory-print per the milestone-094 thermal-noise rationale:

```text
running 1 test
test exclude_path_does_not_exceed_1_10x_baseline ... ok
        perf-bench measurement (macOS, advisory): baseline=3.142s excluded=3.225s ratio=1.026
        (strict assertion skipped on macOS per milestone-094)
```

The test still passes; it just doesn't enforce the budget on macOS.

## Five-minute walkthrough — operator discovering `--exclude-path` from scratch

A new operator running mikebom for the first time runs `mikebom sbom scan --help` and sees:

```text
--exclude-path <PATH_OR_PATTERN>
    Exclude directories from filesystem scans. Repeat the flag (or use
    MIKEBOM_EXCLUDE_PATH=...) to supply multiple entries. Each entry
    is either a literal directory path (e.g., 'tests/fixtures') or a
    glob pattern (e.g., '**/testdata'). See docs/user-guide/cli-reference.md
    § --exclude-path for the full troubleshooting matrix and worked
    examples.
```

The pointer to `docs/user-guide/cli-reference.md § --exclude-path` is what FR-008 verifies via the help-text integration test. Operators following that link find the comprehensive milestone-113 T029 reference content (already in the file from milestone 113; not re-written by this feature).

Cross-link from any ecosystem section in `docs/ecosystems.md` (e.g., `## cargo`'s "Known limitations" sub-section) reads:

> **Path exclusion**: see [Directory exclusion (--exclude-path)](#directory-exclusion---exclude-path).

The pointer lands the operator at the consolidated section in the same document, which then cross-links to the cli-reference deep-dive.

## When NOT to interact with the polish bundle

- **You're running mikebom without `--exclude-path`**: the new tracing fields don't emit; output is byte-identical to pre-118. No change for you.
- **Your PR doesn't touch any walker or the exclusion logic**: the perf benchmark is opt-in via `--ignored`. Run it only if you suspect a regression or have reason to verify.
- **You're a downstream SBOM consumer**: the SBOM payload is unchanged. Only stderr observability changed.

## Related docs

- [`spec.md`](./spec.md) — the user-visible contract
- [`research.md`](./research.md) — 6 implementation decisions (counter mechanism, test consolidation, fixture, tracing wording, perf-bench sampling, docs anchor)
- [`data-model.md`](./data-model.md) — `ExclusionSet` extension + tracing summary contract + perf-bench measurement protocol
- [`contracts/tracing-summary.md`](./contracts/tracing-summary.md) — the scan-end `tracing::info!` event shape
- [`contracts/perf-bench.md`](./contracts/perf-bench.md) — the opt-in benchmark's invocation + sampling + budget assertion
- Milestone 113 (`specs/113-exclude-path-flag/`) — the originating feature this polishes
- Milestone 114 (`specs/114-safe-walk-migration/`) — the `safe_walk` helper this feature instruments
- Milestone 094 (`specs/094-deflake-perf-tests/`) — the perf-bench `#[ignore]` + macOS-skip convention this feature follows
- Issue #343 — the motivating polish-bundle tracking issue
