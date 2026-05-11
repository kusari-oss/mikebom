# Contract — Deflake perf tests deliverables

Behavioral contracts for every new or modified file. Each contract names: (a) what the file MUST contain, (b) how to verify (a) — typically a `grep`, `cargo test --list`, or local 100×-loop check.

## Contract 1 — `triple_format_perf.rs` is skipped by default (FR-001 / FR-002 / SC-001)

**File**: `mikebom-cli/tests/triple_format_perf.rs`

**Post-094 state**: the test fn `triple_format_is_at_least_25_percent_faster_than_three_sequential_scans` carries a `#[ignore = "..."]` attribute. The body is unchanged (FR-007).

**Verification**:

```bash
# Test is listed but flagged as ignored:
cargo +stable test -p mikebom --test triple_format_perf -- --list \
  | grep "triple_format_is_at_least_25_percent_faster" \
  | grep -q "ignored"
# Expected: exit 0 (grep finds the "ignored" annotation).

# Default invocation skips it:
cargo +stable test -p mikebom --test triple_format_perf 2>&1 \
  | grep -E "test result: ok.*0 passed.*0 failed.*1 ignored"
# Expected: 1 ignored, 0 passed, 0 failed.

# Opt-in invocation runs it:
cargo +stable test -p mikebom --test triple_format_perf -- --ignored \
  | grep -E "test result: ok.*1 passed"
# Expected: 1 passed (or, on macOS, possibly the original flake — which is OK,
# the perf-lane retry handles that).
```

## Contract 2 — `dual_format_perf.rs` is skipped by default (FR-001)

**File**: `mikebom-cli/tests/dual_format_perf.rs`

**Post-094 state**: the dual_format perf test fn carries a `#[ignore = "..."]` attribute. Body unchanged.

Same verification as Contract 1, applied to the dual-format test fn name and binary. ALSO verify the test stays skipped when included as a submodule of `holistic_parity`:

```bash
cargo +stable test -p mikebom --test holistic_parity 2>&1 \
  | grep -E "test result: ok.*(N) passed.*(M) ignored" \
  | grep -v "0 ignored"
# Expected: at least 1 ignored test in the holistic_parity binary (from the
# included dual_format_perf submodule).
```

## Contract 3 — `triple_format_structural.rs` deterministic + binary pass/fail (FR-005 / SC-006)

**File**: `mikebom-cli/tests/triple_format_structural.rs`

**Post-094 state**: 3 test fns, all without `#[ignore]`, all running in the default lane:

- `triple_format_invokes_scan_pipeline_exactly_once` — captures stderr of a triple-format invocation; counts `"scan starting"`; asserts == 1.
- `three_sequential_invocations_emit_three_pipeline_starts` — sanity-check that the signal works in both directions.
- `triple_format_outputs_byte_match_three_sequential` — normalized byte-equivalence across the 3 emitted formats.

**Verification (deterministic):**

```bash
# Sanity: tests are listed (not ignored):
cargo +stable test -p mikebom --test triple_format_structural -- --list \
  | grep -c "triple_format_invokes_scan_pipeline_exactly_once\|three_sequential_invocations_emit_three_pipeline_starts\|triple_format_outputs_byte_match_three_sequential"
# Expected: 3

# Default invocation runs them (no --ignored flag needed):
cargo +stable test -p mikebom --test triple_format_structural 2>&1 \
  | grep -E "test result: ok.*3 passed.*0 failed"

# 100-iteration determinism check (SC-006):
for i in $(seq 1 100); do
  cargo +stable test -p mikebom --test triple_format_structural --quiet \
    || { echo "FAIL on iteration $i"; exit 1; }
done
echo "100/100 passed — deterministic"
```

## Contract 4 — `.github/workflows/perf.yml` (FR-004 / SC-002 / SC-004)

**File**: `.github/workflows/perf.yml`

**Post-094 state**: workflow file exists; defines 3 triggers (`pull_request` labeled, `workflow_dispatch`, `schedule` daily); runs on `ubuntu-latest` + `macos-latest` matrix; wraps each perf test in a `nick-fields/retry@v3` step with `max_attempts: 3, timeout_minutes: 5`.

**Verification**:

```bash
# File exists + parses as valid YAML:
test -f .github/workflows/perf.yml && python3 -c "import yaml; yaml.safe_load(open('.github/workflows/perf.yml'))"
# Expected: exit 0.

# Required triggers present:
grep -E "pull_request:|workflow_dispatch:|schedule:" .github/workflows/perf.yml | wc -l
# Expected: >= 3.

# Retry action SHA-pinned (Kusari Inspector pass per milestone-094 conventions):
grep -E "nick-fields/retry@[0-9a-f]{40}" .github/workflows/perf.yml
# Expected: at least 2 matches (one per perf test).

# `perf` label gate present:
grep -q "github.event.label.name == 'perf'" .github/workflows/perf.yml
# Expected: exit 0.
```

**Behavioral verification (post-merge, manual)**:

- Add `perf` label to a test PR → expect `Performance benchmarks` workflow to start within ~1 minute.
- `gh workflow run perf.yml` → expect a manual run to appear in `gh run list`.
- Wait until 06:00 UTC the day after merge → expect a scheduled run to appear.
- A deliberate single-pass-dispatch regression on a `perf`-labeled PR → expect at least one retry attempt to fail (the structural test in the default lane should ALSO fail and catch it earlier).

## Contract 5 — `.github/pull_request_template.md` (FR-006)

**File**: `.github/pull_request_template.md`

**Post-094 state**: the existing Pre-PR checklist gains one new item referencing the `perf` label.

**Verification**:

```bash
grep -c "perf.yml\|perf label\|perf benchmarking lane" .github/pull_request_template.md
# Expected: >= 1.
```

## Contract 6 — `CONTRIBUTING.md` (FR-006)

**File**: `CONTRIBUTING.md`

**Post-094 state**: a new H3 subsection titled "Performance benchmarks (opt-in)" appears under "Pre-PR gate (MANDATORY)". Documents the default-skip behavior, the `-- --ignored` opt-in, and the perf.yml lane.

**Verification**:

```bash
grep -c "^### Performance benchmarks" CONTRIBUTING.md
# Expected: 1.

grep -q "cargo +stable test --workspace -- --ignored" CONTRIBUTING.md
# Expected: exit 0.
```

## Contract 7 — Diff scope guard (FR-008 / FR-009 / FR-010 / SC-007)

**Verification**:

```bash
# Only allowed paths:
git diff --name-only main | grep -vE '^(mikebom-cli/tests/(triple_format_perf|dual_format_perf|triple_format_structural)\.rs|\.github/(workflows/perf|pull_request_template)\.(yml|md)|CONTRIBUTING\.md|specs/094-deflake-perf-tests/.+)$' | grep -v '^$' | wc -l
# Expected: 0.

# No source-tree changes:
git diff --name-only main | grep -E '^(mikebom-cli/src|mikebom-common/src|xtask/src)/' | wc -l
# Expected: 0.

# No golden regen:
git diff --name-only main | grep -E '^mikebom-cli/tests/fixtures/golden/' | wc -l
# Expected: 0.

# No Cargo.lock churn:
git diff --name-only main | grep -E '^Cargo\.(lock|toml)$' | wc -l
# Expected: 0.
```

## Contract 8 — Pre-PR gate clean (FR-002 / SC-005)

**Verification**:

```bash
./scripts/pre-pr.sh
# Expected: prints `>>> all pre-PR checks passed.`; exit 0.
```

The pre-PR gate's `cargo +stable test --workspace` invocation:
- ✅ skips both perf tests (they carry `#[ignore]`)
- ✅ runs the 3 new structural tests in `triple_format_structural.rs`
- ✅ runs every other existing test as today

Net effect: pre-PR gate becomes faster (saves ~20s of perf-test runtime per local invocation) and more reliable (no perf-flake false fails).

## Contract 9 — Default-lane CI reliability (SC-001)

**Behavioral verification (post-merge, ambient observation)**:

Over the 10 PRs following this milestone's merge, count failure events on the three default lanes (`Lint + test (linux-x86_64)`, `Lint + test (macos-latest)`, `Lint + test (linux-x86_64, --features ebpf-tracing)`) attributable to perf-test wall-clock flakes.

```bash
# Sample query for the 10 most recent PRs post-merge:
gh pr list --state merged --base main --limit 10 --json number,statusCheckRollup --jq '.[] | "#\(.number) " + ((.statusCheckRollup // []) | map(select((.name // .context // "" | contains("Lint + test")) and (.conclusion == "FAILURE"))) | length | tostring)'
# Expected: every line ends in "0" (zero default-lane failures). Allow ≤1
# failure across all 10 PRs as a noise budget (covers genuinely-real
# regressions, not perf flakes).
```

If ANY perf-flake failure surfaces on the default lane post-merge, the milestone's architectural change has NOT delivered FR-001. Root-cause that failure and patch in a follow-up.
