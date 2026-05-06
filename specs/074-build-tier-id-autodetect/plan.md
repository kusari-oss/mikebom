# Implementation Plan: Build-tier identifier auto-detection

**Branch**: `074-build-tier-id-autodetect` | **Date**: 2026-05-05 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/074-build-tier-id-autodetect/spec.md`

## Summary

Close the asymmetry left by milestone 073: when `mikebom trace run` (build-tier) executes in a git checkout, auto-detect the same `repo:` identifier that source-tier already detects, and additionally a commit-anchored `git:<repo-url>#<sha>` identifier sourced from `git rev-parse HEAD`. No new CLI flags, no new dependencies, no new modules — purely extends the milestone-073 identifier substrate at one new call site (`mikebom-cli/src/cli/run.rs`) using a small refactor of the existing `auto_detect_repo_identifier` helper.

The work is mechanical because every behavioral contract — selection algorithm, soft-fail rules, manual-flag precedence, `source_label` shape, ordering, determinism — is inherited directly from milestone 073. The only genuinely new logic is shelling out to `git rev-parse HEAD` and stitching its result into the existing identifier-emission flow.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–073; no nightly required for this user-space-only work).
**Primary Dependencies**: Existing only — `std::process::Command` for the new `git rev-parse HEAD` shell-out (same pattern as the existing `git remote get-url` calls at `auto_detect.rs:32`+ and as milestone 053's `git describe` ladder), `tracing` for info/warn logs, `anyhow` for error propagation. **No new `Cargo.toml` deps.**
**Storage**: N/A — identifiers live in emitted SBOMs only; no caches, no persistence.
**Testing**: `cargo +stable test --workspace` for unit + integration tests. New build-tier integration tests reuse the existing `tempfile`-based git-fixture pattern from milestone-073's `auto_detect.rs` test module (`auto_detect.rs:267-340`).
**Target Platform**: Linux (primary CI lane) and macOS (developer workstations). The auto-detection logic itself is OS-agnostic — `git` CLI is the only external assumption, already implicit in mikebom's broader contract per milestones 053 and 073.
**Project Type**: CLI tool — single workspace, three crates (`mikebom-cli` is the only one touched).
**Performance Goals**: Auto-detection adds <100ms to `mikebom trace run` invocation latency on a typical git repo (per SC-004). Two `git` subprocess invocations bound the worst case (`git remote get-url <name>` + `git rev-parse HEAD`).
**Constraints**: Determinism per FR-009 (re-run produces byte-identical identifier slots). Soft-fail per FR-003 (auto-detection failures never abort the scan). No regression on milestone-073 source-tier or image-tier goldens per SC-003.
**Scale/Scope**: One new function (`auto_detect_build_tier_identifiers`) of estimated ~80 LOC; one small refactor of `auto_detect_repo_identifier` to extract a tier-agnostic core; one call site addition in `run.rs::execute`; one new integration-test file (~200 LOC); golden regen for build-tier git-tracked fixtures.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Constitution v1.4.0 ratified 2026-04-15, last amended 2026-05-01. All twelve principles + four strict boundaries reviewed against this milestone:

| Principle | Status | Justification |
|-----------|--------|---------------|
| I. Pure Rust, Zero C | ✅ Pass | Pure-Rust extension of an existing pure-Rust helper. No FFI, no C, no new toolchain. |
| II. eBPF-Only Observation | ✅ Pass / N/A | Identifier emission is metadata, not dependency discovery. The eBPF trace itself is unchanged; auto-detection runs once at invocation start, before the wrapped command. |
| III. Fail Closed | ✅ Pass | Auto-detection follows 073's documented soft-fail rule (info-log, return None). The trace + scan operation that produces the actual SBOM continues to fail closed exactly as before — milestone 074 doesn't touch that contract. |
| IV. Type-Driven Correctness | ✅ Pass | Reuses existing `Identifier`, `SchemeName`, `IdentifierValue`, `BuiltinScheme`, `IdentifierKind` newtypes from milestone 073. Production code uses `anyhow::Result`/`IdentifierError`. Tests already use `#[cfg_attr(test, allow(clippy::unwrap_used))]` per the established convention. No new raw `String` boundaries introduced. |
| V. Specification Compliance | ✅ Pass | **Native-first audit (constitution v1.4.0 5th bullet):** The auto-detected identifiers ride milestone 073's already-vetted standards-native carriers — CDX `metadata.component.externalReferences[type:vcs]`, SPDX 2.3 `Package.externalRefs[PERSISTENT-ID]` + `creationInfo.creators` redundant text line, SPDX 3 `Element.externalIdentifier[]`. Milestone 074 introduces zero new `mikebom:*` annotations and zero new fields. The existing `mikebom:identifiers` annotation (catalog row C47, milestone 073) is reused only for user-defined-namespace identifiers — and only when an operator explicitly passes `--id`, which is unaffected by this milestone. No native-precedence rule applies because no new field is introduced. |
| VI. Three-Crate Architecture | ✅ Pass | All changes inside `mikebom-cli`. No new crates. |
| VII. Test Isolation | ✅ Pass | Auto-detection logic is pure user-space subprocess + path arithmetic. Unit tests need no privilege. The eBPF integration tests are unaffected (build-tier auto-detection runs *before* eBPF attach, in the user-space pre-flight phase). |
| VIII. Completeness | ✅ Pass / N/A | Doesn't affect dependency discovery — identifier auto-detection is metadata enrichment of the SBOM document itself, not the component graph. Completeness contract is unchanged. |
| IX. Accuracy | ✅ Pass | The 073 soft-fail-to-`UserDefined` rule applies (FR-010): malformed git remotes (e.g., URLs that fail the `repo:` validator) downgrade to user-defined classification rather than producing a falsely-labeled `Builtin`. No phantom Built-in classifications. |
| X. Transparency | ✅ Pass | The `source_label` field on auto-detected identifiers explicitly discloses the auto-detection origin (per FR-006: distinct from source-tier so consumers can tell tiers apart). Detection-skip events log at `tracing::info!` per FR-003. |
| XI. Enrichment | ✅ Pass / N/A | Not enrichment — the identifier set is metadata about the scan invocation, not external-source-derived data. |
| XII. External Data Source Enrichment | ✅ Pass / N/A | `git` is a local CLI tool, not an external data source. The trace remains the authoritative source for dependency discovery; identifiers are scan-time metadata, not components. No relationship to Principle XII or Strict Boundary 1 (no lockfile-based discovery). |

| Strict Boundary | Status |
|-----------------|--------|
| 1. No lockfile-based dependency discovery | ✅ Pass — milestone doesn't touch dependency discovery |
| 2. No MITM proxy | ✅ Pass — no network observation surface modified |
| 3. No C code | ✅ Pass — pure Rust only |
| 4. No `.unwrap()` in production | ✅ Pass — extending production code that already complies; tests use the standard `#[cfg_attr(test, allow(clippy::unwrap_used))]` guard |

**Gate result: PASS.** No constitution violations; no Complexity Tracking entries required.

## Project Structure

### Documentation (this feature)

```text
specs/074-build-tier-id-autodetect/
├── plan.md                         # This file (/speckit.plan output)
├── spec.md                         # /speckit.specify output
├── research.md                     # Phase 0 output
├── data-model.md                   # Phase 1 output
├── quickstart.md                   # Phase 1 output
├── contracts/
│   └── build-tier-autodetect.md    # Phase 1 output — single CLI/lib contract
├── checklists/
│   └── requirements.md             # /speckit.specify output
└── tasks.md                        # Phase 2 output (/speckit.tasks; not produced here)
```

### Source Code (repository root)

The milestone touches three production files and adds one integration-test file. No new modules, no new crates.

```text
mikebom-cli/
├── src/
│   ├── binding/
│   │   └── identifiers/
│   │       └── auto_detect.rs              # MODIFY — small refactor:
│   │                                       #   1. extract tier-agnostic core
│   │                                       #      that returns the (url,
│   │                                       #      remote_name, fallback_used)
│   │                                       #      tuple without label.
│   │                                       #   2. add `auto_detect_build_tier_
│   │                                       #      identifiers(invocation_cwd:
│   │                                       #      &Path) -> Vec<Identifier>`
│   │                                       #   3. add `git_rev_parse_head`
│   │                                       #      private helper.
│   └── cli/
│       └── run.rs                          # MODIFY — call
│                                           # `auto_detect_build_tier_identifiers`
│                                           # at invocation start, thread the
│                                           # result into the existing
│                                           # `assembled_ids` resolution
│                                           # pipeline (which already
│                                           # implements 073's manual-wins
│                                           # precedence).
└── tests/
    └── identifiers_build_tier_autodetect.rs # NEW — integration tests for
                                              # the SC-001/SC-002/SC-006/SC-007
                                              # acceptance scenarios:
                                              # - happy path in a git fixture
                                              # - non-git directory (skip)
                                              # - manual override
                                              # - cross-tier correlation
                                              # - detached HEAD
                                              # - empty repo (no commits)
                                              # - first-listed fallback

mikebom-cli/tests/snapshots/                 # MODIFY — golden regen for
└── (existing build-tier git-tracked          # build-tier git-tracked
     fixture goldens)                         # fixtures only. Non-git
                                              # fixtures stay byte-identical.

docs/reference/identifiers.md                 # MODIFY (small) — replace the
                                              # paragraph that says
                                              # "build-tier scans don't
                                              # auto-detect; pass --repo and
                                              # --git-ref manually" with a
                                              # paragraph documenting build-
                                              # tier auto-detection symmetry.
```

**Structure Decision**: Single project. Extends `mikebom-cli` with no new modules. The minimal-surface-change pattern matches milestone 073's own deliberate design (4 new types, 1 module) and the broader project's posture: prefer extending the existing identifier substrate over introducing parallel scaffolding.

## Phase 0 — Research questions

The following decisions need pinning in `research.md` before Phase 1 design. Each is a small implementation-level decision (not a feature-level clarification — those were resolved during `/speckit.specify`).

1. **`git rev-parse HEAD` invocation pattern** — same `Command::new("git")` shell-out as the existing `git_remote_get_url`/`git_remote_list` helpers in `auto_detect.rs`. Confirm the failure modes (no commits → exit 128; not a git repo → exit 128; SHA truncation → never on rev-parse HEAD; wrapping in info-log per FR-003).
2. **Tier-agnostic core refactor** — exact signature for the extracted core (probably `fn auto_detect_repo_url(scan_root: &Path) -> Option<(String, String, bool)>` returning `(url, remote_name, fallback_used)`). Source-tier and build-tier each wrap this with their own label-formatting code.
3. **Invocation cwd capture** — `std::env::current_dir()` at the top of `run.rs::execute` before any subprocess work. Confirm this matches operator mental model (the cwd they ran `mikebom trace run` from, not whatever the wrapped command later changes to).
4. **`source_label` shape for build-tier** — exact strings to use for `repo:` and `git:` identifiers, given FR-006 wants build-tier distinguishable from source-tier. Document the chosen strings so they appear in goldens predictably.
5. **Golden regen strategy** — which existing build-tier fixtures will see additive identifier slots, and whether any non-git build-tier fixtures need adjustment to *prevent* spurious detection (e.g., by ensuring tempdirs have no `.git` directory). Reuse milestone-073's golden-regen pattern.
6. **Manual-wins precedence at the new call site** — FR-004 says manual overrides win. The existing `assembled_ids` flow at `run.rs:226` already implements this for manual-only flags via `Identifier::parse` and dedup logic. Confirm whether to call the existing 073 `resolve_identifiers` helper (used in `scan_cmd.rs`) for parity, or assemble inline.

## Phase 1 — Design & contracts

### data-model.md

One new entity (`BuildTierAutoDetectionResult` per spec) and one trivially-named ephemeral entity (`InvocationCwd`). Both compose existing milestone-073 types — no new newtypes, no new enums. The data model is essentially "a vector of `Identifier` with documented ordering invariants."

### contracts/

One contract: `build-tier-autodetect.md`. Documents the new public function signature, its return-shape invariants (ordering, dedup), its observable behavior (info-log on skip, warn-log on soft-fail), and its integration-boundary assumption (called from `run.rs::execute` before the wrapped command starts; result threaded into the identifier emission pipeline). No new CLI flags, so no new CLI contract surface.

### quickstart.md

Operator-facing recipes:

1. Zero-config build-tier in a git checkout (the headline "no flags needed" recipe — produces both `repo:` and `git:` automatically).
2. Build-tier in a non-git directory (no auto-detection; demonstrates soft-fail UX).
3. Manual override (`--repo` wins over auto-detect).
4. Detached HEAD (typical CI state — `git:` still emits with the detached SHA).
5. Cross-tier correlation walk-through (the SC-002 storyline: same `repo:` value byte-for-byte across source and build SBOMs; build's `git:` SHA matches `git rev-parse HEAD` of the source checkout).

### Agent context update

Run `.specify/scripts/bash/update-agent-context.sh claude` after Phase 1 docs land. Expected behavior: appends a `074-build-tier-id-autodetect` row to the "Active Technologies" table in `CLAUDE.md` capturing the dependency posture summarized in this plan's Technical Context.

## Phase 2 — Out of scope for this command

`/speckit.plan` ends at the close of Phase 1. The next command (`/speckit.tasks`) consumes plan.md + spec.md + the Phase 1 docs and emits `tasks.md` as a strictly-ordered task list. Estimated task count: ~12-15 (much smaller than milestone 073's 25 because no new types, no new module, no new CLI flags).

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified.**

Not applicable — the Constitution Check above passes on all twelve principles + all four strict boundaries with zero violations. No complexity-tracking entries required.
