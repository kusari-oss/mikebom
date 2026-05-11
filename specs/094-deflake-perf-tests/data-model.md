# Data Model — milestone 094

Per-file structure for every deliverable. No JSON / no YAML schema-level changes outside the new workflow file. This is a test-infrastructure milestone; the "data model" here is just the section structure each file MUST adopt.

## File inventory

| File | State | Owner FRs |
|------|-------|-----------|
| `mikebom-cli/tests/triple_format_perf.rs` | MODIFIED (1-line `#[ignore]` add) | FR-001 |
| `mikebom-cli/tests/dual_format_perf.rs` | MODIFIED (1-line `#[ignore]` add) | FR-001 |
| `mikebom-cli/tests/triple_format_structural.rs` | NEW | FR-005 |
| `.github/workflows/perf.yml` | NEW | FR-004 |
| `.github/pull_request_template.md` | MODIFIED (add 1 checklist line) | FR-006 |
| `CONTRIBUTING.md` | MODIFIED (add ~10-line subsection on perf-test split) | FR-006 |

## `triple_format_perf.rs` — modification

Add `#[ignore]` attribute to the single test fn at line 199:

```rust
#[test]
#[ignore = "wall-clock perf test — opt in via `cargo test -- --ignored`, runs in dedicated perf.yml lane"]
fn triple_format_is_at_least_25_percent_faster_than_three_sequential_scans() {
    // ... existing body unchanged ...
}
```

The `ignore = "..."` form documents WHY the test is ignored; `cargo test --verbose` surfaces this reason. Existing body (fixture build, median-of-5, assertion) stays untouched (FR-007).

## `dual_format_perf.rs` — modification

Same pattern. The test fn name (and module-inclusion quirk via `mod dual_format_perf;` in `holistic_parity.rs`) doesn't affect the `#[ignore]` semantics — both inclusion contexts will skip the test in default `cargo test` runs.

## `triple_format_structural.rs` — NEW

Three test functions, all deterministic (no wall-clock measurement, no thresholds):

### Test 1 — `triple_format_invokes_scan_pipeline_exactly_once`

Asserts: a single `mikebom sbom scan --format cdx,spdx,spdx3` invocation emits exactly ONE `"scan starting"` log line on stderr.

```rust
#[test]
fn triple_format_invokes_scan_pipeline_exactly_once() {
    let (_guard, image) = build_synthetic_fixture();  // reuse helper from triple_format_perf
    let tmp = tempfile::tempdir().expect("tempdir");
    let fake_home = tempfile::tempdir().expect("fake-home");
    let mut cmd = Command::new(bin());
    apply_fake_home_env(&mut cmd, fake_home.path());
    cmd.arg("--offline").arg("sbom").arg("scan")
        .arg("--image").arg(&image)
        .arg("--format").arg("cyclonedx-json,spdx-2.3-json,spdx-3-json")
        .arg("--output").arg(format!("cyclonedx-json={}", tmp.path().join("out.cdx.json").display()))
        .arg("--output").arg(format!("spdx-2.3-json={}", tmp.path().join("out.spdx.json").display()))
        .arg("--output").arg(format!("spdx-3-json={}", tmp.path().join("out.spdx3.json").display()))
        .arg("--no-deep-hash");
    let out = cmd.output().expect("mikebom runs");
    assert!(out.status.success(), "scan failed: stderr={}", String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    let count = stderr.matches("scan starting").count();
    assert_eq!(
        count, 1,
        "triple-format MUST invoke the scan pipeline exactly once (single-pass dispatch). \
         Saw {count} `scan starting` log lines. stderr:\n{stderr}",
    );
}
```

### Test 2 — `three_sequential_invocations_emit_three_pipeline_starts`

Asserts: three separate single-format invocations emit `"scan starting"` exactly 3 times total. (Sanity check that the signal mechanism works in both directions; catches log-message-rename regressions.)

```rust
#[test]
fn three_sequential_invocations_emit_three_pipeline_starts() {
    let (_guard, image) = build_synthetic_fixture();
    let mut total = 0;
    for fmt in &["cyclonedx-json", "spdx-2.3-json", "spdx-3-json"] {
        let tmp = tempfile::tempdir().expect("tempdir");
        let fake_home = tempfile::tempdir().expect("fake-home");
        let mut cmd = Command::new(bin());
        apply_fake_home_env(&mut cmd, fake_home.path());
        cmd.arg("--offline").arg("sbom").arg("scan")
            .arg("--image").arg(&image)
            .arg("--format").arg(fmt)
            .arg("--output").arg(format!("{}={}", fmt, tmp.path().join("out").display()))
            .arg("--no-deep-hash");
        let out = cmd.output().expect("mikebom runs");
        assert!(out.status.success(), "scan failed: stderr={}", String::from_utf8_lossy(&out.stderr));
        let stderr = String::from_utf8_lossy(&out.stderr);
        total += stderr.matches("scan starting").count();
    }
    assert_eq!(total, 3, "expected 3 `scan starting` lines across 3 invocations; got {total}");
}
```

### Test 3 — `triple_format_outputs_byte_match_three_sequential` (correctness sibling)

Asserts: each of the 3 outputs from a triple-format invocation byte-matches (after normalization) the output from a single-format invocation. Catches dispatch-bug regressions that produce wrong output even when single-pass works.

```rust
#[test]
fn triple_format_outputs_byte_match_three_sequential() {
    let (_guard, image) = build_synthetic_fixture();
    let workspace = workspace_root();
    let formats = &[
        ("cyclonedx-json",  "cdx.json",   normalize_cdx_for_golden  as fn(&str, &Path) -> String),
        ("spdx-2.3-json",   "spdx.json",  normalize_spdx23_for_golden),
        ("spdx-3-json",     "spdx3.json", normalize_spdx3_for_golden),
    ];

    // Single triple-format invocation.
    let triple_tmp = tempfile::tempdir().expect("tempdir");
    let triple_home = tempfile::tempdir().expect("fake-home");
    let mut triple_cmd = Command::new(bin());
    apply_fake_home_env(&mut triple_cmd, triple_home.path());
    triple_cmd.arg("--offline").arg("sbom").arg("scan").arg("--image").arg(&image)
        .arg("--format").arg("cyclonedx-json,spdx-2.3-json,spdx-3-json")
        .arg("--no-deep-hash");
    for (fmt, ext, _) in formats {
        triple_cmd.arg("--output").arg(format!("{fmt}={}", triple_tmp.path().join(format!("out.{ext}")).display()));
    }
    assert!(triple_cmd.output().unwrap().status.success());

    // Three single-format invocations.
    for (fmt, ext, normalize) in formats {
        let single_tmp = tempfile::tempdir().expect("tempdir");
        let single_home = tempfile::tempdir().expect("fake-home");
        let mut single_cmd = Command::new(bin());
        apply_fake_home_env(&mut single_cmd, single_home.path());
        single_cmd.arg("--offline").arg("sbom").arg("scan").arg("--image").arg(&image)
            .arg("--format").arg(fmt)
            .arg("--output").arg(format!("{fmt}={}", single_tmp.path().join(format!("out.{ext}")).display()))
            .arg("--no-deep-hash");
        assert!(single_cmd.output().unwrap().status.success());

        let triple_out = std::fs::read_to_string(triple_tmp.path().join(format!("out.{ext}"))).unwrap();
        let single_out = std::fs::read_to_string(single_tmp.path().join(format!("out.{ext}"))).unwrap();
        assert_eq!(
            normalize(&triple_out, &workspace),
            normalize(&single_out, &workspace),
            "triple-format `{fmt}` output MUST byte-match single-format `{fmt}` output after normalization",
        );
    }
}
```

The new file imports `common::normalize::{apply_fake_home_env, normalize_cdx_for_golden, normalize_spdx23_for_golden, normalize_spdx3_for_golden}` and `common::workspace_root` per the existing `tests/common/mod.rs` shape. Fixture-build helper is duplicated locally from `triple_format_perf.rs` (only 2 functions, ~40 lines).

## `.github/workflows/perf.yml` — NEW

```yaml
name: Performance benchmarks

on:
  pull_request:
    types: [labeled]
  workflow_dispatch:
  schedule:
    - cron: '0 6 * * *'  # 06:00 UTC daily

permissions:
  contents: read

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  perf:
    name: Wall-clock perf benchmarks
    # Gate the pull_request trigger to only fire on the `perf` label.
    if: github.event_name != 'pull_request' || github.event.label.name == 'perf'
    strategy:
      fail-fast: false
      matrix:
        runner: [ubuntu-latest, macos-latest]
    runs-on: ${{ matrix.runner }}
    steps:
      - uses: actions/checkout@de0fac2e4500dabe0009e67214ff5f5447ce83dd # v6.0.2

      - name: Install stable Rust
        uses: dtolnay/rust-toolchain@29eef336d9b2848a0b548edc03f92a220660cdb8 # stable (2026-04-25)

      - name: Cache cargo + build
        uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32 # v2.9.1
        with:
          key: perf-${{ matrix.runner }}

      - name: Run triple_format_perf (with retry)
        uses: nick-fields/retry@ce71cc2ab81d554ebbe88c79ab5975992d79ba08 # v3.0.2
        with:
          max_attempts: 3
          timeout_minutes: 5
          command: cargo +stable test --workspace -- --ignored --test-threads=1 triple_format_is_at_least_25_percent_faster_than_three_sequential_scans

      - name: Run dual_format_perf (with retry)
        uses: nick-fields/retry@ce71cc2ab81d554ebbe88c79ab5975992d79ba08 # v3.0.2
        with:
          max_attempts: 3
          timeout_minutes: 5
          command: cargo +stable test --workspace -- --ignored --test-threads=1 dual_format_is_at_least_30_percent_faster_than_two_sequential_scans
```

(Test fn name for `dual_format_perf` is approximate; pinned at implementation time by reading `dual_format_perf.rs`.)

## `.github/pull_request_template.md` — modification

Add ONE checklist item to the existing Pre-PR checklist (after the SPDX-3-validator line):

```markdown
- [ ] If this PR touches the scan pipeline, output dispatch, or per-format emission, I added the `perf` label to trigger the dedicated perf benchmarking lane (`.github/workflows/perf.yml`).
```

## `CONTRIBUTING.md` — modification

Add a new subsection under "Pre-PR gate (MANDATORY)" titled "Performance benchmarks (opt-in)":

```markdown
### Performance benchmarks (opt-in)

Wall-clock perf benchmarks (`triple_format_perf.rs`, `dual_format_perf.rs`)
do not run in the default pre-PR gate or in the per-PR CI lanes — they
gate the per-PR merge button on a wall-clock measurement which inherits
shared-CI-runner noise (thermal throttling, scheduler jitter) and false-fails
intermittently.

The dedicated `.github/workflows/perf.yml` lane runs them:

- **On every nightly schedule** (06:00 UTC daily on `main`) — catches background
  regressions within 24 hours.
- **On manual workflow_dispatch** — `gh workflow run perf.yml`.
- **On PRs labeled `perf`** — for PRs that touch the scan pipeline / output
  dispatch / per-format emission.

The perf lane uses retry-on-failure (3 attempts per test) to absorb runner
noise. It is NOT required for PR merge.

To run perf benchmarks locally:

```bash
cargo +stable test --workspace -- --ignored --test-threads=1
```

The default `cargo +stable test --workspace` and `./scripts/pre-pr.sh` skip
ignored tests automatically, matching CI default-lane behavior.

A deterministic structural-correctness sibling test
(`triple_format_structural.rs`) DOES run in the default lane. It asserts
single-pass dispatch via stderr log-line counting and triple-vs-sequential
output byte-equivalence — no wall-clock semantics, binary pass/fail.
```

## Compatibility

- **Render targets**: every new Markdown file renders correctly in GitHub UI; the new YAML file is GitHub-Actions-schema-valid.
- **Backward compatibility**: 100% additive. The two perf tests still exist and still assert what they always did — just not in the default lane. Any maintainer running `cargo test ... -- --ignored` gets the same signal as today.
- **Cargo.lock**: zero change. No new dep. (FR-010)
- **Goldens**: zero regen. No source-tree change. (FR-009)

## No SBOM-emission changes

Zero `mikebom:*` properties involved. Zero output-shape changes. Zero spec-conformance impact.
