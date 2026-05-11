# Quickstart — milestone 094 maintainer recipes

Seven maintainer-facing recipes to apply the deflake architecture and verify each contract.

## Recipe 1 — Add `#[ignore]` to `triple_format_perf.rs` (FR-001)

```bash
# Find the test fn (currently around line 199):
grep -n "fn triple_format_is_at_least_25_percent_faster" mikebom-cli/tests/triple_format_perf.rs
```

Edit the function attribute block above the fn to:

```rust
#[test]
#[ignore = "wall-clock perf test — opt in via `cargo test -- --ignored`, runs in dedicated perf.yml lane"]
fn triple_format_is_at_least_25_percent_faster_than_three_sequential_scans() {
    // ... existing body unchanged (FR-007) ...
}
```

Verify:

```bash
cargo +stable test -p mikebom --test triple_format_perf -- --list \
  | grep "triple_format_is_at_least_25_percent_faster"
# Expected output contains "ignored".

cargo +stable test -p mikebom --test triple_format_perf 2>&1 | tail -3
# Expected: "test result: ok. 0 passed; 0 failed; 1 ignored; ..."
```

## Recipe 2 — Add `#[ignore]` to `dual_format_perf.rs` (FR-001)

Same pattern. The exact fn name to locate:

```bash
grep -n "^fn dual_format_is_at_least\|^fn .*dual.*_faster" mikebom-cli/tests/dual_format_perf.rs
```

Apply the same `#[test] #[ignore = "..."]` pattern. Verify via the same commands, plus an additional check that the test stays skipped when included as a submodule of `holistic_parity`:

```bash
cargo +stable test -p mikebom --test holistic_parity -- --list 2>&1 \
  | grep -E "dual_format.*ignored"
# Expected: at least 1 ignored test surfaces from the included submodule.
```

## Recipe 3 — Add the structural-correctness sibling test (FR-005)

Create `mikebom-cli/tests/triple_format_structural.rs` with the three test functions from `data-model.md §triple_format_structural.rs`:

- `triple_format_invokes_scan_pipeline_exactly_once` — counts `"scan starting"` log lines == 1.
- `three_sequential_invocations_emit_three_pipeline_starts` — counts == 3.
- `triple_format_outputs_byte_match_three_sequential` — normalized byte-equivalence across the 3 formats.

The fixture-build helper (`build_synthetic_fixture`) is copied from `triple_format_perf.rs` (~40 lines: `ImageFile` struct, `build_synthetic_image()` tar-builder, `build_benchmark_fixture()` deb + npm populator). The duplicated body is acceptable scope per FR-008 (test code only); a future refactor could move the helper into `tests/common/mod.rs`.

Verify the deterministic 100-run loop (SC-006):

```bash
for i in $(seq 1 100); do
  cargo +stable test -p mikebom --test triple_format_structural --quiet \
    >/dev/null 2>&1 || { echo "FAIL on iteration $i"; exit 1; }
done && echo "100/100 deterministic passes — SC-006 satisfied"
```

This takes ~10–20 minutes locally (each test invocation is ~4s). Run once during implementation; subsequent runs don't need this loop unless the test changes.

## Recipe 4 — Create `.github/workflows/perf.yml` (FR-004)

Copy the workflow body from `data-model.md §perf.yml`. Key elements:

- Triggers: `pull_request: types: [labeled]` + `workflow_dispatch:` + `schedule: cron: '0 6 * * *'`.
- Label gate: `if: github.event_name != 'pull_request' || github.event.label.name == 'perf'`.
- Matrix: `runner: [ubuntu-latest, macos-latest]`.
- Each perf-test step wrapped in `nick-fields/retry@ce71cc2ab81d554ebbe88c79ab5975992d79ba08 # v3.0.2` with `max_attempts: 3, timeout_minutes: 5`.
- All other actions SHA-pinned per the milestone-094 / PR #200 convention.

Verify the YAML parses + has the required triggers:

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/perf.yml'))" && echo "YAML OK"
grep -E "pull_request:|workflow_dispatch:|schedule:" .github/workflows/perf.yml | wc -l
# Expected: >= 3.
grep -E "nick-fields/retry@[0-9a-f]{40}" .github/workflows/perf.yml | wc -l
# Expected: >= 2.
```

## Recipe 5 — Update docs (FR-006)

Edit `.github/pull_request_template.md` — add ONE checklist item to the Pre-PR checklist:

```markdown
- [ ] If this PR touches the scan pipeline, output dispatch, or per-format emission, I added the `perf` label to trigger the dedicated perf benchmarking lane (`.github/workflows/perf.yml`).
```

Edit `CONTRIBUTING.md` — add a new H3 subsection under "Pre-PR gate (MANDATORY)" titled "Performance benchmarks (opt-in)". Body per `data-model.md §CONTRIBUTING.md`.

Verify:

```bash
grep -q "perf.yml\|perf label\|perf benchmarking lane" .github/pull_request_template.md && echo "PR template OK"
grep -q "^### Performance benchmarks" CONTRIBUTING.md && grep -q -- "-- --ignored" CONTRIBUTING.md && echo "CONTRIBUTING OK"
```

## Recipe 6 — Pre-PR gate (FR-002 / FR-010 / SC-005)

```bash
./scripts/pre-pr.sh 2>&1 | grep -E "FAILED|warning:|error\[|^>>>" | tail -5
# Expected: only the trailing ">>> ... passed." line; no failures.
```

The pre-PR gate now:
- skips both wall-clock perf tests (they carry `#[ignore]`)
- runs the 3 new structural tests in `triple_format_structural.rs`
- runs every other existing test as today

## Recipe 7 — Diff scope audit (FR-008 / FR-009 / FR-010 / SC-007)

```bash
git diff --name-only main | sort
# Expected exact set (or close to it):
#   .github/pull_request_template.md
#   .github/workflows/perf.yml
#   CONTRIBUTING.md
#   mikebom-cli/tests/dual_format_perf.rs
#   mikebom-cli/tests/triple_format_perf.rs
#   mikebom-cli/tests/triple_format_structural.rs
#   specs/094-deflake-perf-tests/...

# Verify zero source-tree changes:
git diff --name-only main | grep -E '^(mikebom-cli/src|mikebom-common/src|xtask/src)/' \
  && echo "SCOPE CREEP" || echo "Source tree untouched — FR-008 satisfied."

# Verify zero golden regen:
git diff --name-only main | grep -E '^mikebom-cli/tests/fixtures/golden/' \
  && echo "GOLDEN CHURN" || echo "No goldens regenerated — FR-009 satisfied."

# Verify zero Cargo.lock/Cargo.toml change:
git diff --name-only main | grep -E '^Cargo\.(lock|toml)$' \
  && echo "DEP CHURN" || echo "Cargo.lock + Cargo.toml untouched — FR-010 satisfied."
```

## When in doubt

- **Pre-PR gate fails**: shouldn't happen — no Rust production source touched. Audit `git diff` for accidental source-tree changes.
- **The 100-iter determinism loop reports a flake**: the new structural test is NOT deterministic; investigate which assertion fired. Likely culprits: (a) the stderr-grep is timing-sensitive (race between subprocess buffer flush and parent read — solved by `cmd.output()` which waits for EOF), (b) the normalize helpers don't strip a volatile field. Fix before merging.
- **perf.yml `pull_request` trigger doesn't fire on labeled PR**: confirm the `if:` gate references `github.event.label.name` correctly. GitHub Actions only delivers `labeled` events when the `pull_request` event type is enabled.
- **`nick-fields/retry@<SHA>` flagged by Kusari Inspector**: confirm the SHA matches a known v3.x tag — check `gh api repos/nick-fields/retry/tags?per_page=20`. Kusari accepts SHA-pinned forms cleanly per the milestone-094 / PR #200 baseline.
- **The structural test's `build_synthetic_fixture` duplicates triple_format_perf's helper**: that's intentional per FR-008 ("test code only"); both files are independent test binaries. A future refactor (out of scope here) could move it to `tests/common/mod.rs`.
- **Default `cargo test` accidentally runs the perf tests**: confirm `#[ignore]` is on the test fn directly (not on a `mod` block). `cargo test -- --list` should show "ignored" annotation.
