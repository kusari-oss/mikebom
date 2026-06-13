# Research — Walker-Audit CI Gate

**Feature**: 115-walker-audit-ci
**Date**: 2026-06-13
**Status**: Decisions resolved; no NEEDS CLARIFICATION markers remaining.

This document records the implementation-level decisions the plan defers from the spec. Each decision lists the chosen approach, the rationale, and the alternatives evaluated.

## Decision 1 — CI lane to run the audit on

**Decision**: Linux only — slot the new step inside the existing `Lint + test (linux-x86_64)` job in `.github/workflows/ci.yml` (line 39). Do NOT replicate it to the macOS, Windows, or eBPF lanes.

**Rationale**: The audit pattern is `grep -rEn --include='*.rs' 'fn walk[_(]' mikebom-cli/src/scan_fs/` — text matching against checked-in source. The output is byte-identical regardless of the host OS (Linux, macOS, Windows) because (a) git normalizes line endings via the existing `.gitattributes` policy and (b) the matched files are pure Rust source under a single normalized path prefix. Running on three OSes triples cost and CI time for zero additional signal — a successful grep on Linux is a successful grep everywhere. The spec's FR-005 ("the audit pattern produces consistent output across CI runners") is satisfied by the pattern being host-agnostic, not by running it on every host.

The single-Linux-lane choice also satisfies SC-002 (≤5 seconds runtime) trivially: ubuntu-latest is the fastest GitHub Actions runner image; grep + sort + diff over ~50 source files completes in <500 ms.

**Alternatives considered**:
- **All three OS lanes** — rejected: 3× the cost and CI minutes for no marginal correctness; a flake on one lane (network blip during checkout) would block PRs unnecessarily.
- **macOS only** — rejected: macOS-latest runners are 10× more expensive than Linux on GitHub Actions billing; no benefit over Linux for a text-comparison check.
- **A dedicated workflow file** — rejected: a single-step audit doesn't justify a new top-level workflow with its own concurrency group, permissions, and checkout. Slotting into the existing job reuses the existing checkout and inherits the existing branch-protection requirement.

## Decision 2 — Diff tool

**Decision**: POSIX `diff -u expected actual`. Failure mode: non-zero exit (which fails the CI step) + the unified-diff output goes to stderr/stdout.

**Rationale**: `diff` is preinstalled on every GitHub Actions runner image (it's POSIX-mandated). `-u` produces unified-diff output that GitHub's job-log UI renders with familiar +/- prefixes — maintainers immediately see "this line was added, this line was removed." No new tool installation, no `apt-get install` step that could fail, no Cargo build of a Rust-based comparator.

The non-zero exit from `diff` naturally satisfies FR-002 (fail-the-build behavior) without requiring a wrapper script to interpret a "no differences" boolean.

**Alternatives considered**:
- **`comm`** — rejected: it works for set-difference but the output format is less readable than unified diff; harder for a reviewer to grok "what to do" from the failure log.
- **A Rust comparator binary** — rejected: pulls a Cargo build into the audit path; violates the spec's implicit "fast + dependency-free" intent. The point of this gate is to be trivially auditable shell.
- **`git diff --no-index`** — rejected: works but injects a git invocation where pure POSIX `diff` is simpler; the gate doesn't need git semantics.

## Decision 3 — Allow-list file location

**Decision**: `mikebom-cli/src/scan_fs/walk.audit-allowlist.txt`. Same directory as `walk.rs`. Plain `.txt` extension. LF line endings.

**Rationale**: Adjacency to `walk.rs` satisfies SC-005 ("a maintainer reviewing a walker PR sees the allow-list edit and the source edit in the same diff hunk"). Putting the file at the repository root (`/.walker-audit-allowlist.txt`) or under `.github/` would force a context switch every time a contributor consults it. The `walk.rs` comment block — which is the doc-of-record for known walker exceptions per milestone 114 — sits 10 lines away in the same directory.

`.txt` extension because the file IS plain text (one grep-output line per entry); a `.yaml` or `.json` extension would imply a schema the file doesn't have. The `walk.audit-allowlist.txt` prefix (`walk.`) groups it visually with `walk.rs` in directory listings, signaling its tight coupling.

LF (Unix) line endings because git normalizes line endings on commit per the repo's `.gitattributes` policy, and grep + diff produce LF-terminated output on the Linux CI runner. Mixed line endings between expected and actual would produce spurious diff failures.

**Alternatives considered**:
- **`.github/walker-audit-allowlist.txt`** — rejected: distant from the code it audits; SC-005 violated.
- **`tests/fixtures/walker-audit-allowlist.txt`** — rejected: implies it's a test fixture (mutable, regenerable). The allow-list is a policy file, not a fixture.
- **YAML or TOML schema** — rejected: overkill for a list of grep-output lines; introduces a parser dependency where none is needed.

## Decision 4 — Sort policy + canonicalization

**Decision**: Both the live grep output AND the committed allow-list are run through `LC_ALL=C sort -u` before `diff -u`. Output is byte-compared. `LC_ALL=C` pins lexicographic ordering deterministically across runners.

**Rationale**: `grep -rEn` walks the filesystem in directory-entry order, which is platform- and inode-dependent. Without sorting, the same set of matches could produce different diff output on consecutive runs of the same commit. Sorting normalizes both sides to a canonical order — the diff then reflects ACTUAL membership changes, not enumeration-order noise.

`LC_ALL=C` is critical: GNU sort's default locale (`en_US.UTF-8` on ubuntu-latest, varies elsewhere) treats `_` and `:` differently depending on collation rules; `LC_ALL=C` forces bytewise comparison which is portable and what every contributor sees when they run the audit locally.

`-u` (uniq) defends against the unlikely edge case of `grep -rEn` returning duplicate lines (e.g., a future grep that follows symlinks twice).

**Alternatives considered**:
- **No sorting (raw grep order)** — rejected: non-deterministic across runners and across consecutive PR re-runs; SC-001 violated.
- **`sort` without `LC_ALL=C`** — rejected: locale-dependent ordering; the committed allow-list (sorted on whoever's machine) would mismatch the live output (sorted on the CI runner's locale).
- **Sort + dedupe inside a Rust binary** — rejected: same Cargo-build objection as Decision 2.

## Decision 5 — Documentation home for the new-exception workflow

**Decision**: Primary doc: a new `## Walker-audit CI gate` section in `CONTRIBUTING.md`, between "Pre-PR gate (MANDATORY)" (line 48) and "Performance benchmarks (opt-in)" (line 78). Cross-link from the existing `docs/design-notes.md` § "Filesystem walking pattern (milestone 114)" section.

**Rationale**: `CONTRIBUTING.md` is where a first-time contributor looks when adding a new ecosystem reader. The milestone-114 design-notes section already enumerates the audit pattern + exception list as a design topic; it's the right place to LINK from but not the right place to host the contribution workflow (design-notes is for "why we did this," contributing is for "what to do"). A two-doc split with one as canonical avoids the rot pattern where two copies drift over time — every detail lives in CONTRIBUTING.md, and `design-notes.md` adds a one-line "see CONTRIBUTING.md § Walker-audit CI gate for the new-exception workflow."

The CONTRIBUTING.md section MUST include:
1. The audit pattern verbatim (the exact grep command, copy-pasteable).
2. The allow-list file path (`mikebom-cli/src/scan_fs/walk.audit-allowlist.txt`).
3. The two-step workflow for adding a new exception: (a) append the new grep-output line to the allow-list, sorted; (b) add a one-sentence reason in the comment block at the top of `walk.rs`. (The latter is reviewer-policed per the spec's Edge Case "comment-block omission is a soft norm.")
4. The failure-message contract: when CI fails, the diff output identifies the line(s) and the contributor follows the "see walk.rs comment block" pointer.

**Alternatives considered**:
- **Only in `docs/design-notes.md`** — rejected: contributors don't read design-notes when adding a new reader; they read CONTRIBUTING.
- **Only in `walk.rs` doc-comment** — rejected: not discoverable without already knowing the file exists; the audit-gate failure message would have to point at a Rust source file rather than a Markdown doc.
- **A new top-level `docs/walker-audit.md`** — rejected: yet another doc file to keep current; the contributor experience benefits from one canonical CONTRIBUTING.md entry.

## Decision 6 — Failure-message contract (FR-004)

**Decision**: When the diff is non-empty, the CI step prints (in order):
1. A one-line headline: `❌ Walker-audit allow-list mismatch — see mikebom-cli/src/scan_fs/walk.rs's module-level comment for the exception policy.` (No emoji in the source if the contributor preference is no-emoji; the literal `❌` here is illustrative — actual implementation will use plain ASCII `[FAIL]` to match the rest of `ci.yml`.)
2. The full `diff -u` output.
3. A trailing two-line pointer: `If your PR intentionally adds a new walker exception, see CONTRIBUTING.md § Walker-audit CI gate. If your PR did NOT intend to add a walker, remove the new fn walk_* function and use scan_fs::walk::safe_walk instead.`

Step then exits non-zero (inherited from `diff`'s exit code).

**Rationale**: The failure message is the contract with future contributors who triage a red CI. It must answer two questions in <30 seconds of reading: (a) "what went wrong," (b) "where do I go next." The diff hunks alone answer (a) but not (b) — a first-time contributor seeing only the diff might not realize the allow-list file even exists. The trailing pointer covers both the legitimate-new-walker case (CONTRIBUTING.md) and the accidental-introduction case (use safe_walk).

**Alternatives considered**:
- **Diff hunks only** — rejected: insufficient triage information for first-time contributors; SC-006 ("contributor resolves audit failure with one pass") not met.
- **A full how-to inline in the CI log** — rejected: too much text in CI output; the CONTRIBUTING.md link is the right level of indirection.

## Decision 7 — Allow-list bootstrap content

**Decision**: At PR-ship time, run `grep -rEn --include='*.rs' 'fn walk[_(]' mikebom-cli/src/scan_fs/ | LC_ALL=C sort -u > mikebom-cli/src/scan_fs/walk.audit-allowlist.txt` and commit the resulting file. The baseline reflects whatever the post-milestone-114 / post-milestone-113-polish tree contains at the moment this PR opens (expected: 28 entries per the spec's FR-010).

**Rationale**: Per the spec's Q1 clarification (strict-enforcement bootstrap), the gate MUST work on the very first CI run that includes it. Running the same command the gate runs and committing the output makes "the baseline matches the live output" trivially true at ship time — and any subsequent drift is exactly what the gate is supposed to catch.

The exact count (28 vs. some other number) is verified at PR-open time, not pre-declared in the spec; the spec's "28 entries" line in FR-010 reflects the post-milestone-114 state and may shift by ±1 if milestone 113 polish or this very PR's docs touch a `walk*` function.

**Alternatives considered**:
- **Silent-pass when allow-list missing** — explicitly rejected by the spec's Q1 clarification (FR-010).
- **Empty allow-list + ratchet-up on each future PR** — explicitly rejected: every existing walker entry would be treated as "new addition" on the bootstrap PR; the PR diff would be unreadable; reviewer cognitive load skyrockets.

## Decision 8 — Interaction with the existing `pre-pr.sh` script

**Decision**: The audit gate is a CI-only step. It is NOT added to `scripts/pre-pr.sh`. Contributors who want to check locally can run the one-liner manually; the documentation includes the snippet.

**Rationale**: `scripts/pre-pr.sh` exists to mirror the CRITICAL CI gates that block merging (clippy + workspace tests). The walker audit IS a blocking gate, so a case could be made to add it. However: the audit takes <500 ms to run, and the failure mode is uniquely easy to diagnose from the CI output alone (a unified diff naming the file + line). Contributors who hit the failure see it immediately on PR-open; they don't need a local pre-flight check the same way they need clippy. Adding it to pre-pr.sh increases local-iteration overhead with marginal value.

This decision is REVERSIBLE — if maintainers later observe that contributors are repeatedly hitting the gate in CI and pushing fix-up commits, adding it to `pre-pr.sh` is a one-line change.

**Alternatives considered**:
- **Add to `pre-pr.sh` from day one** — deferred: marginal value vs. added local-iteration time; revisit if CI flakiness emerges.
- **Make it a `cargo` check** — rejected: adds a Cargo-target hop where pure shell is faster and more transparent; runs through the test harness which is not the right home for a source-code policy check.
