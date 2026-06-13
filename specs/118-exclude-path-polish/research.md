# Research — Milestone 113 `--exclude-path` polish bundle

**Feature**: 118-exclude-path-polish
**Date**: 2026-06-13
**Status**: Decisions resolved; no NEEDS CLARIFICATION markers remaining.

## Decision 1 — Suppressed-directory counter mechanism

**Decision**: Add a `pub(crate) suppressed_dirs: std::sync::atomic::AtomicUsize` field to `ExclusionSet` at `mikebom-cli/src/scan_fs/package_db/exclude_path.rs:108`. `safe_walk` at `walk.rs:227` calls `cfg.exclude_set.suppressed_dirs.fetch_add(1, Relaxed)` immediately after the `matches()` returns true. The scan-end `tracing::info!` at `scan_cmd.rs:1750` reads the count via `.load(Relaxed)`.

**Rationale**:
- **Zero churn to the `&self` borrow chain**: `ExclusionSet` is already threaded through every walker by reference. `fetch_add(&self, ...)` works through `&self` via interior mutability. No `safe_walk` signature change, no `&mut` propagation through ~20 call sites.
- **Free in the no-flag path**: `walk.rs:224`'s `if cfg.exclude_set.is_empty() { return }` short-circuit means the counter is never touched on scans without `--exclude-path`. Byte-identity preservation for non-exclusion scans (FR-010 emission gating) is structural.
- **Single-digit nanosecond cost per hit**: relaxed atomic increment on a non-contended cache line. On the kusari-cli fixture's measured workload (~10K dir entries), even worst-case 100% hit rate adds <10 µs total to scan wall time — well under the 10% budget from FR-011.
- **`Relaxed` ordering is sufficient**: the counter has no read/write dependency on other memory; it's accumulated once and read once at scan end. Sequential consistency / acquire-release would impose unnecessary fence costs.

**Alternatives considered**:
- **Thread-local register**: a `thread_local! { static COUNTER: Cell<usize> }` works but interacts poorly with future concurrent-scan modes and pollutes the per-thread state. Rejected.
- **Summary-struct return from `safe_walk`**: cleanest from a "no hidden state" purity standpoint, but requires updating every `safe_walk` call site to plumb the return value back to the scan-cmd code path. ~20 sites, all of which currently discard the return. Outsized refactor cost; rejected.
- **`Mutex<usize>`**: adds unnecessary mutex contention on a counter that never logically races (each scan owns its `ExclusionSet`). Rejected as over-synchronized.

## Decision 2 — Test-file consolidation

**Decision**: Append the four US1 tests (golang source, go binary, dep-edge, scan-root) AND the two US2 tests (multi-pattern, separator) to the existing `mikebom-cli/tests/exclude_path_integration.rs`. The new help-text test (FR-008) lives in a separate file `mikebom-cli/tests/exclude_path_help_text.rs` because its setup pattern (`Command::new(env!("CARGO_BIN_EXE_mikebom")).arg("sbom").arg("scan").arg("--help")`) is structurally distinct from the existing tests' `run_scan()` helper. The perf benchmark lives in its own file `mikebom-cli/tests/exclude_path_perf.rs` per the milestone-094 convention (one perf benchmark per file, opt-in `#[ignore]`d).

**Rationale**:
- **Test discoverability**: the existing 5 milestone-113 tests already establish `exclude_path_integration.rs` as the canonical location for `--exclude-path` integration coverage. A future contributor reading `cargo test --test exclude_path_integration` sees all 11 tests in one place.
- **Helper reuse**: `run_scan()`, `component_names()`, `envelope_property()`, `write_cargo_project()` are private to the existing test file and already cover the integration-test pattern. Splitting would require duplicating or hoisting helpers.
- **Perf-bench separation**: matches `dual_format_perf.rs` / `triple_format_perf.rs` / milestone-094's pattern. Perf benchmarks live in dedicated files because their `#[ignore]` annotation makes them no-ops in the default `cargo test` run, and reviewers expect to find them via filename grep.
- **Help-text separation**: the help-text test doesn't share any helper with the integration tests (no SBOM parsing, no fixture, no `--bind-to-source` flow). Putting it in its own file keeps `exclude_path_integration.rs`'s scope tight ("integration tests that scan something and assert on the SBOM output").

**Alternatives considered**:
- **One file per test class**: would produce `exclude_path_golang.rs`, `exclude_path_go_binary.rs`, `exclude_path_dep_edge.rs`, ... — six new files for six tests. Over-decomposed; the existing convention already groups by feature, not by test class.
- **Help-text test inside `exclude_path_integration.rs`**: works but mixes concerns; rejected for clarity.

## Decision 3 — Fixture vending (synthesized vs in-tree dirs)

**Decision**: All new integration-test fixtures are synthesized in-process via `tempfile::tempdir()`, following the existing milestone-113 pattern. NO new fixture directories are added under `mikebom-cli/tests/fixtures/`. The perf benchmark uses the kusari-cli fixture from milestone 090's external cache.

**Rationale**:
- **Per spec FR-012 + spec clarification Q1**: the "vendored in-tree" language in FR-012 is satisfied by `tempfile`-synthesized scaffolds. The five existing milestone-113 tests all use this pattern (`exclude_path_integration.rs:145, 182, 215, 246`). Adding new ones with the same pattern is the lowest-churn path.
- **No fixture-repo bump needed**: keeping the perf fixture as kusari-cli (already in `tests/fixtures.rev`) avoids any change to the milestone-090 fixture cache or its sibling-repo pin.
- **Hermetic test execution**: per-test `tempfile::tempdir()` materialization gives each test its own filesystem state, automatically cleaned up at test exit. Idiomatic Rust test practice.
- **Smaller diff**: zero new files under `mikebom-cli/tests/fixtures/`. The diff is concentrated in `exclude_path_integration.rs` where reviewer attention already lives.

**Alternatives considered**:
- **Vendor real per-ecosystem fixture dirs** under `mikebom-cli/tests/fixtures/exclude_path_polish/<ecosystem>/`: the literal reading of FR-012's "vendored in-tree" wording. Rejected: adds ~6 small subdirs of synthesized scaffold files that exactly mirror what `tempfile` produces at runtime, doubling the maintenance surface without measurable benefit. The spec's intent (per Assumptions) is "synthetic, not real-world corpora," which `tempfile` satisfies.
- **Pull the perf fixture inline (new vendored kusari-cli-like fixture)**: rejected per spec clarification Q1 — kusari-cli is already cached, already polyglot, zero new maintenance.

## Decision 4 — `tracing::info!` summary wording

**Decision**: The scan-end summary line emits as:

```rust
tracing::info!(
    components = components.len(),
    relationships = relationships.len(),
    excluded_entries = exclude_set.entries().len(),
    suppressed_dirs = exclude_set.suppressed_dirs.load(Relaxed),
    "scan complete"
);
```

ONLY when `!exclude_set.is_empty()`. When the set IS empty, the existing two-field shape (`components`, `relationships`) is preserved bit-for-bit per FR-010's emission gating.

Operators reading stderr see:
```text
INFO mikebom::cli::scan_cmd: scan complete components=42 relationships=58 excluded_entries=2 suppressed_dirs=137
```

**Rationale**:
- **Field-emission idiom** (vs string-formatted "exclude-path: applied N entries, suppressed M directories"): matches the existing `tracing::info!` field-emission shape at `scan_cmd.rs:1750-1754`. Tracing subscribers (`tracing-subscriber`, `tracing-tree`, `tracing-json`) parse fields structurally; a structured `excluded_entries=N` is grep-able AND machine-parseable, while a free-form string is neither.
- **No new message string**: keeps "scan complete" as the message; the new fields are additive. Operators using `RUST_LOG=info` see the new fields appear automatically. JSON-output users get them as separate JSON fields.
- **Backwards-compat preserved**: when the gate is unused, the existing two-field form is emitted — no operator-visible change for non-exclusion scans.

**Alternatives considered**:
- **Separate `tracing::info!` line dedicated to exclude-path**: works but adds a second log line, doubling the noise floor for scans that do use exclusions. Rejected: less compact, harder to grep.
- **Free-form message string** ("scan complete: ... excluded N, suppressed M..."): rejected per field-emission idiom rationale.

## Decision 5 — Perf-bench median-of-N sample size

**Decision**: Median-of-5 samples, matching the existing `dual_format_perf.rs:289-330` pattern. Five samples balance signal vs runtime cost: with a ~3s per-scan baseline on kusari-cli, five samples × 2 conditions (baseline + with-flag) ≈ 30 seconds total perf-bench runtime, which is reasonable for opt-in `--ignored` invocation.

**Rationale**:
- **Reuse milestone-094's pattern**: median-of-5 is what the existing dual_format_perf.rs uses. Pre-existing convention.
- **Variance robustness**: median of 5 trims one outlier per side, robust to JIT/cache warm-up + occasional OS scheduling hiccups. Single-sample timing on shared-CI runners is notoriously flaky per milestone 094.
- **Acceptable runtime**: ~30s opt-in benchmark is similar to existing perf benchmarks; contributors running `cargo test -- --ignored` once per perf-relevant PR is the expected cadence.

**Alternatives considered**:
- **Single sample**: cheaper but unreliable per milestone-094's documented variance.
- **Median-of-11**: more robust but doubles runtime for marginal precision gain on a 10%-budget assertion. Rejected.

## Decision 6 — Docs cross-link anchor placement in `docs/ecosystems.md`

**Decision**: Add ONE consolidated `## Directory exclusion (--exclude-path)` cross-cutting section between the `## Coverage matrix` (currently around line 19) and the per-ecosystem sections (starting `## apk` around line 22). The new section contains the canonical operator-facing explanation cross-linking to `docs/user-guide/cli-reference.md` § `--exclude-path`. Each per-ecosystem section gets a short pointer line in its "Known limitations" sub-section (or as a leading bullet if no Known limitations exists), worded as:

```markdown
**Path exclusion**: see [Directory exclusion (--exclude-path)](#directory-exclusion---exclude-path).
```

The pointer line is short, formulaic across ecosystems (single literal string + anchor), and identical wording everywhere so future grep / link-check tooling can verify presence uniformly.

**Rationale**:
- **Per spec clarification Q2**: the centralized-reference + per-section-pointer shape was committed in the spec. This decision finalizes the exact anchor format.
- **Anchor format `#directory-exclusion---exclude-path`**: GitHub-flavored Markdown auto-slug for `## Directory exclusion (--exclude-path)`. Verified by a one-shot local check (markdown link-fragment slugifier rules: lowercase, spaces → `-`, parens → empty, `--` from `--exclude-path` preserved as `---` after dash-joining).
- **"Known limitations" placement**: existing ecosystem sections already use this sub-section heading for caveats / operator-actionable notes. Putting the pointer there matches operator expectations ("things I should know before scanning this ecosystem").
- **Identical wording across ecosystems**: enables a one-line shell verification `grep -c 'Directory exclusion (--exclude-path)' docs/ecosystems.md` to assert presence per FR-007.

**Alternatives considered**:
- **Top-of-section pointer**: pollutes the canonical-detection-shape opening of each ecosystem section with cross-cutting noise. Rejected.
- **Footer-position pointer (last line of each ecosystem section)**: less discoverable; operators stop reading once they have what they need.
- **Per-ecosystem custom wording**: prevents the simple grep verification + adds writing burden. Rejected.

The current per-ecosystem section list (10 sections per the Explore agent's survey: `apk`, `cargo`, `deb`, `gem`, `golang`, `maven`, `npm`, `nuget`, `pip`, `rpm`, `yocto`) is the target set for the pointer lines. If new ecosystem sections land in `docs/ecosystems.md` between this PR's open and merge, they need the pointer too — a CI-level lint could enforce this in a future milestone (out of scope for #343).
