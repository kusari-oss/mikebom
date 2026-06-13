# Implementation Plan: Walker-Audit CI Gate

**Branch**: `115-walker-audit-ci` | **Date**: 2026-06-13 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/115-walker-audit-ci/spec.md`

## Summary

Add a CI step to `.github/workflows/ci.yml`'s existing `Lint + test (linux-x86_64)` job that runs `grep -rEn 'fn walk[_(]' mikebom-cli/src/scan_fs/` and `diff`s the sorted output against a committed allow-list at `mikebom-cli/src/scan_fs/walk.audit-allowlist.txt`. The PR ships the 28-line post-114 baseline as that file. Failure prints the offending diff hunks AND a one-paragraph "how to resolve" pointer to `scan_fs/walk.rs`'s comment block. Documentation lands in `CONTRIBUTING.md` (new section) + a cross-link from `docs/design-notes.md`'s existing milestone-114 section. The new-exception workflow requires two edits in the same PR — append to the allow-list + add a one-sentence reason in `walk.rs`'s comment block — though only the allow-list edit is mechanically enforced (the comment-block edit is reviewer-policed per the spec's Edge Cases).

**Technical approach**: POSIX shell + `grep` + `sort` + `diff` (already on every GitHub Actions runner). Linux-only execution per the spec's Deferred decision (cheapest lane, audit-pattern is OS-agnostic by design). No new tools, no new Cargo dependencies. The CI step runs BEFORE the existing clippy step at `ci.yml:216` (and therefore also before `cargo test --workspace`) so a fast audit failure short-circuits both the lint suite and the slow test suite.

## Technical Context

**Language/Version**: POSIX shell (bash) inside GitHub Actions YAML; no Rust code change.
**Primary Dependencies**: existing tools — `grep` (GNU/BSD; the `-rEn` flags are POSIX-compatible), `sort` (POSIX), `diff` (POSIX). All preinstalled on every GitHub Actions runner image. **Zero new tool installations.**
**Storage**: A single source-tree-committed plain-text file: `mikebom-cli/src/scan_fs/walk.audit-allowlist.txt`. Sorted lex; one `<file>:<line>:<content>` entry per line; LF line endings.
**Testing**: The gate's correctness is exercised by the negative test path documented in `quickstart.md` — a contributor opens a PR adding `fn walk_synthetic_negative_test_DO_NOT_MERGE()` to a non-allowlisted file, observes CI fail red. No new unit-test infrastructure; the gate IS a single shell pipeline.
**Target Platform**: GitHub Actions `ubuntu-latest` runner (existing `Lint + test (linux-x86_64)` job). The spec's FR-005 cross-platform stability guarantee is satisfied by the audit pattern being OS-agnostic; running on Linux-only is a cost choice per the spec's Deferred section, not a correctness choice.
**Project Type**: CI/CD configuration + one tracked data file + documentation updates.
**Performance Goals**: ≤5 seconds wall time per SC-002. The grep runs in <1s on the post-114 source tree (~50 files under `mikebom-cli/src/scan_fs/`), `sort` is O(n log n) on ~28 lines, `diff -u` is O(n + m) on two ~28-line files. Total expected: <500 ms.
**Constraints**: No new Cargo dependencies (matches every milestone since 001); the gate MUST run on every PR and every push to main (FR-001); allow-list MUST be order-stable across runners (FR-005); failure message MUST point at `scan_fs/walk.rs`'s comment block (FR-004).
**Scale/Scope**: One CI step, one allow-list file, one `CONTRIBUTING.md` section. Diff size: ~80 lines YAML + ~30 lines text file + ~50 lines docs.

## Constitution Check

| Principle | Status | Notes |
|---|---|---|
| I. Pure Rust, Zero C | N/A | This feature ships YAML + plain text + Markdown; no source code. |
| II. eBPF-Only Observation | N/A | This affects CI, not the trace or scan codepaths. |
| III. Fail Closed | ✓ | The gate IS a fail-closed mechanism. A missing/empty allow-list, an unauthorized new walker, OR a stale allow-list entry all produce non-zero exit (FR-010 confirms strict-enforcement bootstrap). |
| IV. Type-Driven Correctness | N/A | No Rust code. |
| V. Specification Compliance | N/A | No SBOM emission change. |
| VI. Three-Crate Architecture | N/A | No new crates. |
| VII. Test Isolation | ✓ | The gate runs as a standalone CI step, not as part of the Rust test suite. Privilege requirements: none. |
| VIII. Completeness | N/A | No discovery layer change. |
| IX. Accuracy | N/A | |
| X. Transparency | ✓ | Gate failure prints the diff hunks AND a documented resolution pointer (FR-004). Maintainer reading the CI log sees exactly which line is offending + where to look for the resolution doc. |
| XI. Enrichment | N/A | |
| XII. External Data Source Enrichment | N/A | |
| Strict Boundary 1 (no lockfile-based discovery) | N/A | |
| Strict Boundary 2 (no MITM) | N/A | |
| Strict Boundary 3 (no C code) | ✓ | Shell + Markdown only. |
| Strict Boundary 4 (no `.unwrap()` in production) | N/A | |

**Result**: Constitution Check PASSES. No violations. (Most principles are N/A because this is CI infrastructure, not source code.)

## Project Structure

### Documentation (this feature)

```text
specs/115-walker-audit-ci/
├── plan.md              # This file
├── research.md          # Phase 0 — implementation decisions: shell pipeline shape, allow-list format, file location, failure-message contract
├── data-model.md        # Phase 1 — allow-list entry format + invariants
├── quickstart.md        # Phase 1 — "how to triage an audit-gate failure" runbook
├── contracts/
│   └── ci-step.md       # The single CI-step contract (YAML shape, exit codes, stdout/stderr)
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
.github/
└── workflows/
    └── ci.yml                                # +1 step in the linux-x86_64 lane
mikebom-cli/
└── src/
    └── scan_fs/
        ├── walk.rs                           # UNCHANGED (the helper module; comment block already lists exceptions)
        └── walk.audit-allowlist.txt          # NEW — 28-line baseline snapshot, sorted lex
CONTRIBUTING.md                                # +1 section: "Walker-audit CI gate"
docs/
└── design-notes.md                            # +1 paragraph in the existing milestone-114 section
                                               # linking to CONTRIBUTING.md for the workflow
```

**Structure Decision**: The allow-list lives ADJACENT to `walk.rs` (same directory, sibling file). This satisfies SC-005 ("maintainer evaluates allow-list change + corresponding code change with one diff read") — `git diff` against a PR that adds both a walker AND an allow-list entry shows them ~10 lines apart in the same directory. Putting the allow-list elsewhere (e.g., `tests/` or `.github/`) would force a context switch. The text-file format (no `.yaml`/`.json`/`.toml` schema) matches the spec's Assumption that the allow-list is "fundamentally a list of grep-output lines."

## Complexity Tracking

No constitution violations. No complexity to justify.
