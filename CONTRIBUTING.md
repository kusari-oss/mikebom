# Contributing to mikebom

Thanks for your interest in contributing! mikebom is pre-1.0 alpha; we
encourage a quick discussion on non-trivial changes before you open a
PR so we can align on direction.

## Workflow overview (the speckit lifecycle)

For non-trivial changes (new features, behavior changes, large
refactors, ecosystem additions), mikebom uses the spec-kit lifecycle:

1. `/speckit.specify` — write the feature spec (what + why)
2. `/speckit.clarify` (optional) — resolve open questions
3. `/speckit.plan` — produce `research.md`, `data-model.md`, `contracts/`
4. `/speckit.tasks` — break work into a checklist
5. `/speckit.analyze` (optional) — cross-check spec ↔ plan ↔ tasks
6. `/speckit.implement` — execute the task list

Each milestone lives at `specs/<NNN>-<short-name>/`. See an existing one
(e.g., [`specs/092-fix-maven-version-extract/`](specs/092-fix-maven-version-extract/))
for a complete example.

Per-skill references are under `.claude/skills/speckit-*/SKILL.md`.

**Small drive-by fixes** (typo corrections, single-line bug fixes,
doc tweaks) skip the lifecycle — just open a PR.

## Local development setup

```bash
git clone https://github.com/kusari-sandbox/mikebom.git
cd mikebom
cargo +stable build --release
```

The `sbom`, `policy`, `attestation`, and related subcommands build
under the **stable** toolchain. The eBPF-based `trace` subcommands
additionally need nightly + bpf-linker — see
[`docs/user-guide/installation.md`](docs/user-guide/installation.md)
for the full setup, including the `mikebom-dev` container and Lima VM
options for macOS.

Test fixtures live in a sibling repo (`kusari-sandbox/mikebom-test-fixtures`)
and are cloned automatically by `build.rs` on first build into a
per-host cache at `~/.cache/mikebom/fixtures/<pinned-sha>/`. The
pinned SHA lives in `tests/fixtures.rev`.

## Pre-PR gate (MANDATORY)

Before opening any PR, **both** of these MUST exit clean:

```bash
./scripts/pre-pr.sh
```

This single script runs, in order:

1. `cargo +stable clippy --workspace --all-targets -- -D warnings` — zero
   clippy warnings (warnings become errors).
2. `cargo +stable test --workspace` — every test suite must report
   `N passed; 0 failed`.

Both gates are what CI enforces; running the script locally first saves
a CI round-trip.

For PRs that touch SBOM emission or output formats, also opt-in to the
SPDX-3 conformance validator:

```bash
MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1 ./scripts/pre-pr.sh
```

This requires the JPEWdev `spdx3-validate` Python package pinned in
`.venv/spdx3-validate/`. If the validator isn't installed locally,
the gate skips silently — but CI runs it strictly on release branches,
so test locally before release-bump PRs.

### Performance benchmarks (opt-in)

Wall-clock perf benchmarks (`triple_format_perf.rs`, `dual_format_perf.rs`)
do NOT run in the default pre-PR gate or in the per-PR CI lanes — they
inherit shared-CI-runner thermal/scheduler noise that false-fails
intermittently on macOS-latest at ~14–22% measured-reduction vs the
25% gate. Blocking PR merges on those flakes hurts more than the perf
signal helps (see [`specs/094-deflake-perf-tests/`](specs/094-deflake-perf-tests/)
for the architectural rationale).

Instead, [`.github/workflows/perf.yml`](.github/workflows/perf.yml) runs
them:

- **Daily at 06:00 UTC on `main`** — catches background regressions
  within ~24h.
- **On manual `workflow_dispatch`** — `gh workflow run perf.yml`.
- **On PRs labeled `perf`** — opt-in for PRs that touch the scan
  pipeline, output dispatch, or per-format emission.

The perf lane uses `nick-fields/retry@v3` with 3 attempts per test to
absorb runner-noise spikes. It is NOT required for PR merge.

To run perf benchmarks locally:

```bash
cargo +stable test --workspace -- --ignored --test-threads=1
```

`./scripts/pre-pr.sh` and the default `cargo +stable test --workspace`
skip `#[ignore]`'d tests automatically, matching CI default-lane
behavior.

A deterministic structural-correctness sibling test
([`mikebom-cli/tests/triple_format_structural.rs`](mikebom-cli/tests/triple_format_structural.rs))
DOES run in the default lane. It catches single-pass dispatch
regressions binary pass/fail via stderr log-line counting of the
existing `"scan starting"` info-line, plus triple-vs-sequential output
byte-equivalence — no wall-clock semantics, no thresholds, no
flakiness.

## Project principles + where to find them

The canonical source-of-truth for project principles is
[`.specify/memory/constitution.md`](.specify/memory/constitution.md).
Twelve principles to be aware of:

- **I. Pure Rust, Zero C** — no FFI, no `libbpf` bindings, no C
  toolchains in the build pipeline. `aya` provides the eBPF stack.
- **II. eBPF-Only Observation** — eBPF tracing is the trust-rooted
  dependency-discovery path; external sources (lockfiles, registries)
  only ENRICH what was observed.
- **III. Fail Closed** — never gap-fill with heuristics when the
  trace loses data; exit non-zero and surface the gap.
- **IV. Type-Driven Correctness** — newtype wrappers for PURL,
  hashes, license expressions; no `.unwrap()` in production code
  (use `anyhow` / `thiserror`).
- **V. Specification Compliance** — CycloneDX 1.6 + SPDX 2.3 +
  SPDX 3.x conformance is non-negotiable. **Standards-native fields
  take precedence over `mikebom:*` properties** — every new
  `mikebom:*` field MUST first audit each target format for an
  existing native construct.
- **VI. Three-Crate Architecture** — `mikebom-ebpf/` (no_std kernel
  programs), `mikebom-common/` (shared structs), `mikebom-cli/`
  (user-space). Additional crates require a constitution amendment.
- **VII. Test Isolation** — privilege-dependent tests (eBPF) gated
  behind CAP_BPF; unprivileged unit tests run on every CI lane.
- **VIII. Completeness** — minimize false negatives; every observed
  fetch must appear in the SBOM.
- **IX. Accuracy** — minimize false positives; flag low-confidence
  matches via spec-native confidence/evidence fields.
- **X. Transparency** — surface every limitation (overflow events,
  inferred edges, heuristic matches) via spec-native mechanisms.
- **XI. Enrichment** — license, VEX, supplier metadata when
  available; never block SBOM emission on unavailable enrichment.
- **XII. External Data Source Enrichment** — lockfiles / registries /
  hash databases MAY enrich observed components but MUST NOT
  introduce components the eBPF trace didn't observe.

If your change touches any principle, link to it from your PR
description and explain how the change preserves the principle. The
mandatory pre-PR template (`.github/pull_request_template.md`) has
a checkbox for this.

## Pull request etiquette

- Open one PR per logical change. The PR title should match the
  format `<type>(<scope>): <subject>` (e.g., `fix(092): Maven pom.xml
  version-extraction`).
- Include a `## Test plan` section in the PR description with the
  commands you ran locally.
- Run `./scripts/pre-pr.sh` clean before requesting review.
- For changes that regenerate byte-identity goldens, mention the
  expected diff symmetry in the PR description (e.g., "+1521/-1521
  tool-version churn only").

## Reporting issues + security

- Bugs / feature requests: use the structured templates at
  https://github.com/kusari-sandbox/mikebom/issues/new/choose.
- Vulnerabilities: see [`SECURITY.md`](SECURITY.md) — do **not**
  open a public issue.

## License

By contributing, you agree your contributions are licensed under
Apache-2.0 (the project's license — see [`LICENSE`](LICENSE)).
