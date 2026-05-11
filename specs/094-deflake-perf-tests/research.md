# Research — milestone 094 deflake perf tests

Phase 0 investigation. Three decision points; all resolved without further clarification.

## §1 — Structural-signal mechanism: existing log surface, no production code change

**Finding**: `mikebom-cli/src/cli/scan_cmd.rs:1413` already emits:

```rust
tracing::info!(root = %root_path.display(), "scan starting");
```

This fires exactly once per `mikebom sbom scan` subprocess invocation, regardless of how many `--format` values are passed (the scan pipeline runs once and the format dispatch happens AFTER this log line; per `serialize_all` / format-dispatch code path). The default `tracing-subscriber` configuration in `mikebom-cli/src/main.rs` enables INFO-level emission to stderr.

**Decision**: the structural-correctness test captures stderr from `mikebom sbom scan ...` invocations and counts occurrences of the substring `"scan starting"`. Asserts:

- Triple-format invocation (`--format cdx,spdx,spdx3`): count = **1**.
- Three single-format sequential invocations: count = **3**.
- (Additionally: byte-equivalence between the triple-format outputs and the corresponding single-format outputs, after normalization, to catch dispatch-correctness regressions.)

**Rationale**:
- Zero production-code change (FR-008 compliant). The signal hook already exists.
- Deterministic by construction. `tracing::info!` fires unconditionally; the stderr capture is byte-stable regardless of CI runner thermal/scheduler state.
- The signal directly maps to the spec's user-facing intent: "single-invocation produces all 3 formats by running the scan pipeline once".

**Alternatives considered**:
- **New env-var-instrumented counter** (e.g., `MIKEBOM_SCAN_PIPELINE_COUNT=/tmp/foo`): would require new production code (the env-var-conditional emission), which violates FR-008. Rejected.
- **Resource-usage comparison** (CPU time via `resource::getrusage` or `/usr/bin/time -v`): less wall-clock-sensitive than total wall-time but still has variance. Still needs a threshold. Not deterministic. Rejected.
- **Output byte-equivalence alone**: if triple-format BREAKS single-pass internally (runs the scan pipeline 3 times but still emits 3 correct outputs), byte-equivalence would still pass. We'd lose the "single-pass" signal entirely. Rejected as sole signal — but kept as a complementary check alongside the log-count.
- **Process count via `ps`-style inspection**: complex, platform-specific, fragile. Rejected.

## §2 — Opt-in UX: `#[ignore]` + `cargo test -- --ignored`

**Finding**: Rust's standard convention for "this test exists but is skipped by default" is the `#[ignore]` attribute. Cargo's default `cargo test ...` invocation skips `#[ignore]`'d tests; `cargo test ... -- --ignored` runs ONLY them. This matches the project's existing usage (e.g., `#[test] #[ignore] fn requires_privilege() { ... }` in `mikebom-cli/tests/scan_walker_loops.rs`).

**Decision**: mark both perf tests with `#[ignore]`. The default lanes (and the pre-PR gate) skip them automatically. The dedicated perf lane invokes `cargo +stable test --workspace -- --ignored --test-threads=1` to run them.

**Rationale**:
- Standard Rust idiom; existing pattern in this project (`scan_walker_loops.rs`).
- No env var dance. No conditional compilation. No feature-flag gymnastics.
- `cargo test`'s built-in filtering handles the inclusion/exclusion split.
- `--test-threads=1` reduces inter-test CPU contention during the perf measurement (the perf lane runs the 2 perf tests serially, not in parallel with other tests).

**Alternatives considered**:
- **`#[cfg(feature = "perf-tests")]`**: requires adding a `perf-tests` Cargo feature. Workable but more ceremonious than `#[ignore]`; also non-standard for this project. Rejected.
- **Env-var conditional `return` at the top of each test**: skips at runtime instead of at filter time. Hides the test from `cargo test --list`'s output ambiguously. Worse UX than `#[ignore]`. Rejected.
- **Move tests into a separate crate (e.g., `mikebom-perf-tests`)**: adds workspace member; requires lifecycle plumbing; out-of-proportion to the goal. Rejected.

## §3 — Retry mechanism: `nick-fields/retry@v3` (SHA-pinned)

**Finding**: GitHub Actions doesn't have built-in retry-on-failure for steps. The industry-standard third-party action is [`nick-fields/retry`](https://github.com/nick-fields/retry), pinned to v3.0.2. Mature, ~2k+ GitHub stars, used widely (kubernetes, rust-lang/rust, many others).

**Decision**: use `nick-fields/retry@ce71cc2ab81d554ebbe88c79ab5975992d79ba08 # v3.0.2` in the new `perf.yml` workflow. SHA-pin per the milestone-094 / Kusari Inspector convention.

Wrap each perf-test invocation in a retry step with:
- `max_attempts: 3`
- `timeout_minutes: 5` (each perf test typically completes in <60s; 5min buffer absorbs slow runner cold-start)
- `command: cargo +stable test --workspace -- --ignored --test-threads=1 <test_name>` (one wrapped step per test, so failure attribution stays clean)

**Rationale**:
- 3 attempts × independent runner noise → P(all 3 flake on a 22% measured-reduction-vs-25%-threshold case) is ~negligible. Independent samples; the worst observed flake was a single CI run failure, not a sustained regression.
- SHA-pinned per the just-established repo convention (PR #200). Kusari Inspector will pass cleanly on the new workflow file.

**Alternatives considered**:
- **Custom bash retry loop in the step**: works but reinvents what `nick-fields/retry` already does cleanly. Less readable in the workflow YAML. Rejected.
- **GitHub Actions' built-in `continue-on-error: true`**: marks the step as soft-failed but doesn't retry. Wrong primitive. Rejected.
- **No retry at all**: the user's stated goal is "stop flaky failures from blocking work". Even on the opt-in lane, a flake would surface as a failed run and re-trigger user frustration. Retry is necessary. Rejected.

## §4 — Workflow trigger conditions

**Finding**: GitHub Actions supports three trigger types relevant to this use case:
- `pull_request: types: [labeled]` — fires when a maintainer adds a label to a PR.
- `workflow_dispatch:` — manual trigger via the Actions UI or `gh workflow run`.
- `schedule: - cron: '...'` — periodic execution.

**Decision**: configure perf.yml with all three triggers:

```yaml
on:
  pull_request:
    types: [labeled]
  workflow_dispatch:
  schedule:
    - cron: '0 6 * * *'  # 06:00 UTC daily (off-peak for both NA and EU)
```

For the `pull_request` trigger, the job's first step checks `if: github.event.label.name == 'perf'` so the workflow only runs on the `perf` label specifically, not on every label addition.

**Rationale**:
- `pull_request` + label gives maintainers an explicit opt-in for perf-touching PRs.
- `workflow_dispatch` gives a manual-trigger escape hatch (e.g., re-run after a known-flaky moment).
- `schedule` provides background regression-detection on main; daily cadence balances runner cost vs detection latency (24h max delay).

**Alternatives considered**:
- **`push: branches: [main]`**: runs on every commit to main. Too expensive given perf-test runtime + retry budget. Rejected in favor of daily schedule.
- **No scheduled trigger; only manual + label**: loses background regression detection. Maintainers might forget to label perf-touching PRs. Rejected.

## Coverage map

| Spec section | Resolution |
|--------------|------------|
| FR-001 (skip perf tests in default lane) | §2 → `#[ignore]` attribute. |
| FR-002 (pre-PR gate inherits default-skip) | §2 → `cargo test --workspace` skips ignored tests automatically. |
| FR-003 (opt-in mechanism for maintainers) | §2 → `cargo +stable test --workspace -- --ignored --test-threads=1`. |
| FR-004 (dedicated CI lane with retry, not required for merge) | §3 + §4 → new `perf.yml`; SHA-pinned `nick-fields/retry@v3`; 3 trigger types. |
| FR-005 (structural-correctness test in default lane) | §1 → stderr-grep `"scan starting"` count + byte-equivalence. |
| FR-006 (docs updates) | implementation detail; addressed in Phase 1's data-model / quickstart. |
| FR-007 (preserve existing assertion semantics) | §2 → only `#[ignore]` added; no `median_of_5` → `median_of_7` tuning. |
| FR-008 (no production code change) | §1 → log signal already exists; no instrumentation hook added. |
| FR-009 (no golden regen) | implicit — no source change emits goldens. |
| FR-010 (no new Cargo deps) | §3 → workflow YAML reference only, not a Cargo dep. |
| Constitution V audit | no `mikebom:*` properties; trivially satisfied. |
| Constitution VII (test isolation) | §1 + §2 → strengthened. |

All open spec questions resolved. Ready for Phase 1 (data-model + contracts + quickstart).
