# Implementation Plan: Transitive dep correctness audit per ecosystem

**Branch**: `083-transitive-correctness` | **Date**: 2026-05-07 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/083-transitive-correctness/spec.md`

## Summary

Audit-first milestone: run mikebom + trivy + syft against per-ecosystem real-world fixtures, diff the resulting `relationships[]` (SPDX 2.3) / `dependencies[]` (CDX 1.6) arrays, and produce a per-ecosystem findings report classifying each ecosystem as `matches expected` / `minor differences` / `gap surfaced`. Per the 2026-05-07 Q1 clarification, fixtures are vendored in-tree as manifest+lockfile-only (~500 KB total). Per Q2, trivy + syft are the comparison baseline; source-format direct read (lockfile parse OR native OS package-manager query) is the tiebreaker when the three SBOM tools disagree.

The milestone produces:
- **Per-ecosystem audit findings** in `research.md` covering all 9 ecosystems (Go, Cargo, npm, Maven, pip, gem, dpkg, rpm, apk).
- **Regression tests** at `mikebom-cli/tests/transitive_parity_<ecosystem>.rs` × 9 pinning the alpha.23 baseline edge counts so future milestones can't silently regress.
- **Per-ecosystem indirect-vs-direct decisions** (US3): implement / document-as-divergence / defer.
- **Per-ecosystem follow-up issues** filed for any gap surfaced (US4).

Code fixes for gaps surfaced are **explicitly out of scope** per FR-010. The milestone produces the diagnosis; treatment ships as separate per-ecosystem follow-up milestones.

The Phase 0 audit (this `/speckit.plan` setup verified): trivy 0.69.3 + syft 1.27.0 are installed locally and on the developer's PATH. Both binaries support all 9 in-scope ecosystems. mikebom's per-ecosystem readers exist at `mikebom-cli/src/scan_fs/package_db/{cargo,dpkg,gem,golang/,maven,npm/,pip/,rpm,apk}.rs`. Existing test fixtures cover most ecosystems at small synthetic scale; this milestone adds vendored real-world manifest+lockfile fixtures meeting the ≥50/≥100 thresholds at `mikebom-cli/tests/fixtures/transitive_parity/<ecosystem>/`.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–082; no nightly).
**Primary Dependencies**: Existing only — `serde`/`serde_json` (parsing emitted SBOM JSON), `tracing`, `anyhow`. The audit harness shells out to `trivy` and `syft` as external CLI tools (similar to milestone 078's `spdx3-validate` shell-out pattern). Source-format direct readers use existing per-ecosystem parsers in `mikebom-cli/src/scan_fs/package_db/` for tiebreaker comparisons (we don't re-implement; we re-invoke). For OS package managers, native tools (`dpkg-query`, `rpm`, `apk`) are shelled out. **No new Cargo dependencies.**
**Storage**: N/A — purely test infrastructure. The 9 vendored fixtures (~500 KB total) live in `mikebom-cli/tests/fixtures/transitive_parity/<ecosystem>/`. No caches, no persistence beyond test fixtures.
**Testing**: `cargo +stable test --workspace` continues as the primary gate. Adds 9 new integration test files (`transitive_parity_<ecosystem>.rs`) with the same graceful-skip pattern as milestone 078's `spdx3_conformance.rs` — when `trivy` / `syft` / native package-manager binaries aren't on PATH, tests skip with a clear diagnostic; when `MIKEBOM_REQUIRE_TRANSITIVE_PARITY=1` env var is set (CI lane), tests fail-on-absent.
**Target Platform**: Linux + macOS for developer workstations. Linux (CI primary) for `MIKEBOM_REQUIRE_TRANSITIVE_PARITY=1` strict mode. The OS-package-manager tests (dpkg/rpm/apk) only run on Linux where the native tools exist; macOS skips them gracefully.
**Project Type**: Audit + regression test infrastructure. No production code changes; mikebom emission paths are observed, not modified.
**Performance Goals**: Audit harness wall-time <60s end-to-end (9 ecosystems × ~5s per side-by-side comparison); per-test wall-time <10s including any tool shell-outs. Negligible compared to the existing milestone-078 conformance gate.
**Constraints**: Per FR-010 — code fixes for gaps surfaced are out of scope. Per FR-011 — pre-PR gate stays clean (no production code changes; existing byte-identity goldens preserved without `MIKEBOM_UPDATE_*_GOLDENS`). Per Q1 — fixtures vendored in-tree as manifest+lockfile-only. Per Q2 — trivy + syft as comparison baseline + source-format direct-read tiebreaker.
**Scale/Scope**: Audit harness ~150 LOC (one-time test infrastructure). Per-ecosystem regression test ~80–120 LOC × 9 = ~900 LOC. Fixtures ~500 KB total across 9 ecosystems. Audit-record entries in `research.md` ~30–60 lines per ecosystem × 9 = ~400 lines + summary. Total milestone diff: ~1500–2000 lines of new test infrastructure + audit documentation. **Zero production code changes.**

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Constitution v1.4.0 (last amended 2026-05-01). All 12 principles + 4 strict boundaries reviewed:

| Principle | Status | Justification |
|-----------|--------|---------------|
| I. Pure Rust, Zero C | ✅ Pass | Audit harness is Rust integration tests; no C, no FFI. Trivy + syft + native package-manager binaries are external CLI tools shelled out via `std::process::Command` (same pattern as milestone-078's `spdx3-validate`). |
| II. eBPF-Only Observation | ✅ Pass / N/A | Audit observes per-ecosystem readers' output; doesn't touch the eBPF trace path. |
| III. Fail Closed | ✅ Pass | Regression tests fail closed: when expected edge count diverges from baseline, the test fails with a clear diff. Graceful-skip when external tools absent (matching milestone-078 pattern); strict-fail under `MIKEBOM_REQUIRE_TRANSITIVE_PARITY=1` env var. |
| IV. Type-Driven Correctness | ✅ Pass | New types: `AuditEcosystemRow`, `EdgeDiff`, `TransitiveParityFixture` — pub(crate) test-only types. Production code untouched. Test code uses `#[cfg_attr(test, allow(clippy::unwrap_used))]` per CLAUDE.md. |
| V. Specification Compliance | ✅ Pass / N/A | Audit doesn't change SBOM emission shape. The `relationships[]` / `dependencies[]` arrays already conform; this milestone verifies they're CORRECT, not just well-formed. |
| VI. Three-Crate Architecture | ✅ Pass | All Rust changes inside `mikebom-cli/tests/`. No new crates. |
| VII. Test Isolation | ✅ Pass | Tests run without elevated privileges. Trivy + syft + native package-manager tools run unprivileged. The graceful-skip pattern preserves the unprivileged-CI invariant; CI's strict mode requires the tools to be installed. |
| VIII. Completeness | ✅ Pass | The audit IS Principle VIII work — verifying mikebom's transitive-edge completeness against ground truth. Findings inform whether mikebom's edge emission has false negatives. |
| IX. Accuracy | ✅ Pass | The audit IS Principle IX work — verifying mikebom's transitive-edge accuracy against ground truth. Findings inform whether mikebom's edge emission has false positives. |
| X. Transparency | ✅ Pass | The audit's findings are durable, structured, and discoverable — operators reading the per-ecosystem audit-record entry know exactly what mikebom's per-ecosystem accuracy guarantees are. |
| XI. Enrichment | ✅ Pass / N/A | Audit-only; no enrichment changes. |
| XII. External Data Source Enrichment | ✅ Pass / N/A | Trivy + syft are comparison baselines, not enrichment data sources for mikebom-emitted SBOMs. |

| Strict Boundary | Status |
|-----------------|--------|
| 1. No lockfile-based dependency discovery | ✅ Pass — source-format direct-read tiebreakers parse lockfiles for AUDIT verification only, not as a discovery source for SBOM emission |
| 2. No MITM proxy | ✅ Pass |
| 3. No C code | ✅ Pass — Rust audit harness; external tools (trivy, syft, dpkg, rpm, apk) are pre-existing |
| 4. No `.unwrap()` in production | ✅ Pass — all changes are in `tests/`; production code untouched |

**Gate result: PASS.** No violations; no Complexity Tracking entries needed. The audit aligns with Principles VIII (completeness) + IX (accuracy) + X (transparency) — verifying and durably documenting mikebom's per-ecosystem edge correctness.

## Project Structure

### Documentation (this feature)

```text
specs/083-transitive-correctness/
├── plan.md                         # This file
├── spec.md                         # /speckit.specify + /speckit.clarify (Q1 + Q2 integrated)
├── research.md                     # Phase 0 — per-ecosystem fixture selections + tool versions + audit harness design
├── data-model.md                   # Phase 1 — AuditEcosystemRow + EdgeDiff entities
├── quickstart.md                   # Phase 1 — maintainer recipe for re-running audit, picking new fixtures, bumping baselines
├── contracts/
│   └── audit-harness.md            # Phase 1 — audit harness contract + audit-row format
├── checklists/
│   └── requirements.md             # Already passing
└── tasks.md                        # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

The milestone touches ONLY test infrastructure under `mikebom-cli/tests/`. Production code in `mikebom-cli/src/` is NOT modified per FR-010.

```text
mikebom-cli/
├── tests/
│   ├── fixtures/
│   │   └── transitive_parity/                 # NEW — vendored real-world fixtures per Q1
│   │       ├── go/
│   │       │   ├── go.mod                     # extracted from a real Go project (e.g., kubernetes/cri-tools tag-pinned)
│   │       │   ├── go.sum
│   │       │   └── README.md                  # source-of-truth URL + commit SHA
│   │       ├── cargo/
│   │       │   ├── Cargo.toml
│   │       │   ├── Cargo.lock
│   │       │   └── README.md
│   │       ├── npm/
│   │       │   ├── package.json
│   │       │   ├── package-lock.json
│   │       │   └── README.md
│   │       ├── maven/
│   │       │   ├── pom.xml
│   │       │   └── README.md
│   │       ├── pip-poetry/                    # poetry.lock case
│   │       │   ├── pyproject.toml
│   │       │   ├── poetry.lock
│   │       │   └── README.md
│   │       ├── pip-pipfile/                   # Pipfile.lock case
│   │       │   ├── Pipfile
│   │       │   ├── Pipfile.lock
│   │       │   └── README.md
│   │       ├── pip-plain/                     # plain requirements.txt — documents upstream limitation
│   │       │   ├── requirements.txt
│   │       │   └── README.md
│   │       ├── gem/
│   │       │   ├── Gemfile
│   │       │   ├── Gemfile.lock
│   │       │   └── README.md
│   │       ├── dpkg/                          # Debian-style metadata fixture
│   │       │   ├── status                     # /var/lib/dpkg/status format
│   │       │   └── README.md
│   │       ├── rpm/                           # RPM rpmdb sample
│   │       │   ├── packages.db                # extracted from a real container layer
│   │       │   └── README.md
│   │       └── apk/                           # APK installed-database sample
│   │           ├── installed
│   │           └── README.md
│   ├── transitive_parity_common.rs            # NEW (~150 LOC) — shared audit harness:
│   │                                            # - run_mikebom(fixture) -> Vec<Edge>
│   │                                            # - run_trivy(fixture) -> Vec<Edge>
│   │                                            # - run_syft(fixture) -> Vec<Edge>
│   │                                            # - run_source_format_direct(fixture, ecosystem) -> Vec<Edge>
│   │                                            #   (tiebreaker per Q2; invokes lockfile parser or native OS tool)
│   │                                            # - diff_edge_sets(mikebom, trivy, syft, source_truth) -> EdgeDiff
│   │                                            # - graceful_skip_when_tools_absent(env_var: &str)
│   │                                            #   (mirrors milestone-078 spdx3_conformance.rs pattern)
│   ├── transitive_parity_go.rs                # NEW (~100 LOC each — 9 ecosystems)
│   ├── transitive_parity_cargo.rs
│   ├── transitive_parity_npm.rs
│   ├── transitive_parity_maven.rs
│   ├── transitive_parity_pip_poetry.rs        # poetry.lock case
│   ├── transitive_parity_pip_pipfile.rs       # Pipfile.lock case
│   ├── transitive_parity_pip_plain.rs         # plain requirements.txt — documents upstream limitation
│   ├── transitive_parity_gem.rs
│   ├── transitive_parity_dpkg.rs              # Linux-only; macOS skips
│   ├── transitive_parity_rpm.rs               # Linux-only
│   └── transitive_parity_apk.rs               # Linux-only

mikebom-cli/src/                                 # NOT MODIFIED. Per FR-010, code fixes for any gap
                                                 # surfaced ship as separate per-ecosystem follow-up
                                                 # milestones referenced from filed GitHub issues.

scripts/                                         # NOT MODIFIED. The new audit harness lives in
                                                 # tests/transitive_parity_common.rs (Rust), not as
                                                 # a standalone shell script.

.github/workflows/                               # MODIFY (small) — extend the linux-x86_64 lint+test
                                                 # job to install trivy + syft, set
                                                 # MIKEBOM_REQUIRE_TRANSITIVE_PARITY=1. macOS lane
                                                 # intentionally does NOT install trivy/syft (per
                                                 # the milestone-078 macOS exclusion pattern —
                                                 # graceful-skip preserves dev experience without
                                                 # forcing tool install on developer workstations).
```

**Structure Decision**: Pure test infrastructure milestone. 9 vendored fixtures (~500 KB) + 1 shared audit harness + 11 per-ecosystem regression tests (note: pip splits into 3 cases — poetry, Pipfile, plain — for FR-008's documented-limitation case). Reuses milestone-078's `MIKEBOM_REQUIRE_*` env-var pattern + graceful-skip-when-tools-absent.

## Phase 0 — Research questions

Six implementation-level decisions to pin in `research.md`. Two highest-impact decisions (Q1 fixture vendoring; Q2 comparison-base policy) were locked during /speckit.clarify; this phase pins per-ecosystem fixture selections + tool versions + audit-harness implementation details.

1. **Trivy + syft version pinning**: trivy 0.69.3 + syft 1.27.0 confirmed installed on developer workstation during /speckit.plan setup. Pin these versions in research.md. CI's tool-install step pins to the same versions. Future re-audits document any version bumps.

2. **Per-ecosystem fixture selection**: pick a real-world repo per ecosystem with ≥50 components / ≥100 edges. Candidates per ecosystem (commit-pin TBD at fixture-extraction time):
   - **Go**: `kubernetes/cri-tools` (300+ deps in go.sum)
   - **Cargo**: a workspace project with ~80 deps (e.g., a rust-analyzer subset, or `clap-rs/clap`)
   - **npm**: `expressjs/express` (200+ deps in package-lock.json)
   - **Maven**: `apache/commons-lang` (with parent POM chain)
   - **pip-poetry**: a real poetry.lock-using project (e.g., `pypa/poetry` itself)
   - **pip-pipfile**: a real Pipfile.lock-using project
   - **pip-plain**: a small `requirements.txt`-only project (documents the limitation; expected zero transitive edges)
   - **gem**: `rubocop/rubocop` Gemfile.lock
   - **dpkg/rpm/apk**: extract from a real container image's installed database (e.g., debian:12, fedora:39, alpine:3.20)

3. **Audit harness design (`tests/transitive_parity_common.rs`)**: shared helper module. Functions:
   - `run_mikebom(fixture_path) -> Result<Vec<Edge>>` — invokes `cargo run -p mikebom-cli -- sbom scan --path <fixture> --format spdx-3-json` and parses `relationships[]`.
   - `run_trivy(fixture_path) -> Result<Vec<Edge>>` — invokes `trivy fs --format spdx-json <fixture>` (note: trivy emits SPDX 2.3 by default; spdx-3-json may not be supported, so SPDX 2.3 + JSON conversion).
   - `run_syft(fixture_path) -> Result<Vec<Edge>>` — invokes `syft <fixture> -o spdx-json` and parses.
   - `run_source_format_direct(fixture_path, ecosystem) -> Result<Vec<Edge>>` — per-ecosystem dispatch:
     - Go: `go mod graph` if `go` on PATH; else parse go.mod direct-require lines (lossy — we already do this in mikebom-cli/src/scan_fs/package_db/golang.rs; re-invoke that helper as a library for tiebreaker).
     - Cargo: parse `Cargo.lock` `dependencies = [...]` lines.
     - npm: parse `package-lock.json` `packages[].dependencies` field.
     - Maven: shell out to `mvn dependency:tree -DoutputType=text`.
     - pip-poetry: parse `poetry.lock`'s `[[package]]` blocks + `dependencies` subkeys.
     - pip-pipfile: parse `Pipfile.lock`'s `default` + `develop` sections.
     - gem: parse `Gemfile.lock`'s GEM block per-spec dependencies.
     - dpkg: shell out to `dpkg-query --show -f='${Package} ${Depends}\n'` (linux only).
     - rpm: shell out to `rpm -q --requires --all` or `rpm -qpR <package>` (linux only).
     - apk: shell out to `apk info -R` (linux only).
   - `diff_edge_sets(mikebom, trivy, syft, source_truth) -> EdgeDiff` — computes the per-edge classification per Q2.
   - `assert_graceful_skip(env_var: &str)` — returns early with `Skipped` variant when external tools absent + env var unset; returns `Err` when env var set + tools absent (CI strict mode).

4. **Per-ecosystem regression test structure**: each `transitive_parity_<ecosystem>.rs` follows the same pattern (~80–120 LOC):
   ```rust
   const FIXTURE_PATH: &str = "tests/fixtures/transitive_parity/<ecosystem>/";
   const EXPECTED_EDGE_COUNT: usize = 142;  // Pinned at alpha.23 baseline
   const EXPECTED_REPRESENTATIVE_EDGES: &[(&str, &str)] = &[
       ("pkg:cargo/serde@1.0.193", "pkg:cargo/serde_derive@1.0.193"),
       // ... 10–20 representative edges
   ];

   #[test]
   #[cfg_attr(test, allow(clippy::unwrap_used))]
   fn transitive_edges_match_baseline() { ... }

   #[test]
   fn cross_tool_parity_check() { ... }  // Compares mikebom vs trivy vs syft + tiebreaker
   ```

5. **Indirect-vs-direct decision rubric (US3 / FR-004)**: per-ecosystem decision matrix:
   - **Go**: `// indirect` marker in go.mod is structurally explicit; emit as `RelationshipIndirect` per trivy's pattern OR document as deliberate divergence (mikebom's "all-edges-under-root-the-same-way" is operator-comprehensible). Audit-decision: defer to follow-up; not a P1/P2 milestone gate.
   - **npm**: `dependencies` vs `devDependencies` in package.json maps to mikebom's milestone-052 lifecycle scope (already implemented). Audit confirms; no new work.
   - **Cargo**: `[dependencies]` vs `[dev-dependencies]` maps to milestone-052 lifecycle scope (already implemented). Same.
   - Other ecosystems: no native distinction; N/A.

6. **CI workflow integration**: extend `.github/workflows/ci.yml` Linux job with two new steps before `cargo test`:
   ```yaml
   - name: Install trivy
     run: |
       sudo apt-get install -y wget
       wget -qO - https://aquasecurity.github.io/trivy-repo/deb/public.key | sudo apt-key add -
       echo deb https://aquasecurity.github.io/trivy-repo/deb $(lsb_release -sc) main | sudo tee -a /etc/apt/sources.list.d/trivy.list
       sudo apt-get update
       sudo apt-get install -y trivy=0.69.3
   - name: Install syft
     run: curl -sSfL https://raw.githubusercontent.com/anchore/syft/main/install.sh | sh -s -- -b /usr/local/bin v1.27.0
   - name: Set MIKEBOM_REQUIRE_TRANSITIVE_PARITY
     run: echo "MIKEBOM_REQUIRE_TRANSITIVE_PARITY=1" >> $GITHUB_ENV
   ```
   The macOS-latest job intentionally skips both installs (per milestone-078 macOS exclusion pattern — graceful-skip preserves dev experience without forcing trivy/syft on developer workstations).

## Phase 1 — Design & contracts

### data-model.md

Three new internal Rust types in `mikebom-cli/tests/transitive_parity_common.rs`:
- `AuditEcosystemRow` — one entry per ecosystem in research.md with mikebom/trivy/syft/source-format edge counts.
- `Edge` — `(from_purl: String, to_purl: String)` newtype for cross-tool comparison.
- `EdgeDiff` — set-theoretic comparison output: `agreement`, `mikebom_only`, `trivy_only`, `syft_only`, `source_truth_says_X` per-edge classifications.

### contracts/

One contract: `audit-harness.md`. Documents:
- Per-tool invocation contract (how each external tool is called)
- Edge-extraction algorithm per format (SPDX 2.3 relationships → Edge tuples; SPDX 3 software_dependsOn → Edge tuples; CDX dependencies[] → Edge tuples)
- Tiebreaker dispatch table (per-ecosystem source-format direct-read invocation)
- Audit-row JSON schema (the structured output research.md consumes)

### quickstart.md

Maintainer-facing recipes:
1. **Re-running the audit** — invoke `cargo test --test 'transitive_parity_*'` with `MIKEBOM_REQUIRE_TRANSITIVE_PARITY=1`; expected output: all 11 tests pass against alpha.23 baseline.
2. **Picking a new fixture for an ecosystem** — guide for vendoring a new manifest+lockfile fixture (≥50/≥100 thresholds, source-of-truth URL+commit pinning, README.md format).
3. **Bumping the expected baseline after a deliberate edge-emission change** — guide for updating `EXPECTED_EDGE_COUNT` + `EXPECTED_REPRESENTATIVE_EDGES` in the per-ecosystem test (mirrors the existing byte-identity-golden bump pattern).
4. **Reading the audit-record findings** — how to interpret per-ecosystem audit rows (matches / minor differences / gap surfaced + follow-up issue link).
5. **Filing a follow-up issue for a surfaced gap** — issue-body template per the spec's FollowUpIssue entity.

### Agent context update

Run `.specify/scripts/bash/update-agent-context.sh claude` after Phase 1 docs land.

## Phase 2 — Out of scope for this command

`/speckit.plan` ends here. `/speckit.tasks` consumes plan.md + spec.md + Phase 1 docs and emits `tasks.md`. Estimated task count: **~18–22** — one task per ecosystem (11 — pip splits into 3 cases) + the shared audit harness + the 9 vendored fixtures + the audit-record per-ecosystem entries + the CI workflow update + polish.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified.**

Not applicable — Constitution Check passes on all 12 principles + 4 strict boundaries with zero violations. The audit aligns with Principles VIII (completeness audit) + IX (accuracy audit) + X (transparency findings).
