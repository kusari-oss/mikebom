# Data Model — Walker-Audit CI Gate

**Feature**: 115-walker-audit-ci
**Date**: 2026-06-13

This feature has exactly one data artifact: the allow-list text file. There are no domain entities, no persisted state, no schema migrations — this section documents the single file's format + invariants so future maintainers can edit it confidently.

## Entity: Allow-list Entry

**File**: `mikebom-cli/src/scan_fs/walk.audit-allowlist.txt`
**Encoding**: UTF-8 (ASCII subset in practice)
**Line endings**: LF (Unix)
**Final-newline policy**: file ends with a single LF after the last entry (POSIX text-file convention; required for `diff -u` to compare cleanly)

### Line format

Each non-blank, non-comment line is one allow-list entry, byte-equal to a single line of `grep -rEn --include='*.rs' 'fn walk[_(]' mikebom-cli/src/scan_fs/` output:

```text
<relative-path>:<line-number>:<matched-line-content>
```

| Field | Type | Source | Notes |
|---|---|---|---|
| `<relative-path>` | string | grep | Relative to repo root, forward-slash separators. Always starts with `mikebom-cli/src/scan_fs/`. |
| `<line-number>` | positive int | grep | 1-based; identifies the line in the source file where `fn walk[_(]` matched. |
| `<matched-line-content>` | string | grep | The full source line text where the match occurred, including leading whitespace and trailing comments. Verbatim. |

**Example entry** (from the expected post-114 baseline):

```text
mikebom-cli/src/scan_fs/walk.rs:142:pub(crate) fn safe_walk<F: FnMut(&Path)>(
```

### Sort order

The file is sorted with `LC_ALL=C sort -u` (locale-independent bytewise lexicographic, deduped). Entries thus appear in a deterministic order across all hosts that follow Decision 4 in [research.md](./research.md).

### Blank lines + comments

The file MAY contain blank lines (zero or more LFs in a row) and comment lines (starting with `#`) for human readability. These lines are filtered out before the audit comparison runs — they exist purely for maintainer convenience and do not participate in the diff.

**Filter logic (informal pseudocode)**:
```
keep_for_diff(line) = line is non-empty AND line does NOT start with '#'
```

The committed allow-list as shipped in this PR has NO comments or blank lines (it's the raw grep output sorted) — the comment-line provision is a forward-looking affordance for future maintainers who may want to group entries by category (e.g., `# Helper module`, `# Documented exceptions`, `# Known non-walker false positives`). Adopting comments is a future-PR decision; the initial commit doesn't take a position.

### Invariants

The file MUST satisfy these invariants at every commit; the CI gate enforces them implicitly:

1. **Coverage**: The set of non-blank-non-comment lines is EXACTLY equal to `LC_ALL=C sort -u` of the live `grep -rEn --include='*.rs' 'fn walk[_(]' mikebom-cli/src/scan_fs/` output. Any deviation fails CI.
2. **No duplicates**: The `-u` step in the sort guarantees this; entries are unique line-by-line.
3. **Sort-stability**: The file content matches `LC_ALL=C sort -u file > file.tmp && diff file file.tmp` cleanly. A future PR that hand-edits the file out of order will fail the diff (the live side is freshly sorted; the committed side will be canonically re-sorted via the CI step's pipeline, so the comparison only succeeds if the committed file was already sorted).
4. **Final-newline**: file ends with exactly one trailing LF.
5. **Non-empty**: file has at least one entry. (FR-010: empty allow-list fails CI under strict-enforcement bootstrap.)

### Lifecycle

The allow-list is born at PR ship time (this feature's PR) populated with the 28-line post-milestone-114 baseline. Subsequent edits happen ONLY in PRs that legitimately add a new walker exception, per the workflow documented in `CONTRIBUTING.md § Walker-audit CI gate`. The expected long-term direction is downward: every milestone that migrates an existing exception walker to `safe_walk` removes its entry from the allow-list (SC-004's "shrinks over time" measurement).

There is no automatic regeneration. Any PR that wants to update the file must edit it explicitly — running the grep one-liner and replacing the file content is the documented workflow.

## Entity: CI Step (Comparison Pipeline)

The audit step itself is an ephemeral entity — it has inputs (the source tree at the PR's HEAD) and outputs (a zero/non-zero exit code + stdout/stderr). Documented here for completeness:

### Inputs
1. The repo source tree at PR HEAD (checked out by the existing `actions/checkout@v4` step at line 14 of `ci.yml`).
2. `mikebom-cli/src/scan_fs/walk.audit-allowlist.txt` (read from the checkout).

### Pipeline
1. Run `grep -rEn --include='*.rs' 'fn walk[_(]' mikebom-cli/src/scan_fs/ | LC_ALL=C sort -u > /tmp/live.txt`.
2. Run `grep -v '^#' mikebom-cli/src/scan_fs/walk.audit-allowlist.txt | grep -v '^$' | LC_ALL=C sort -u > /tmp/expected.txt`. (The two `grep -v` invocations strip blank lines + comments per the format-section filter.)
3. Run `diff -u /tmp/expected.txt /tmp/live.txt`.
4. If diff is empty → step exits 0 (success).
5. If diff is non-empty → print the FR-004 failure-message payload (headline + diff + pointer) and exit with diff's non-zero exit code.

### Outputs
- **Exit code**: 0 (allow-list matches live) or non-zero (drift detected; FR-002).
- **stdout/stderr**: per FR-004 failure-message contract; see [research.md § Decision 6](./research.md).

### Performance contract
- p95 wall time ≤5 s per SC-002.
- Measured: <500 ms expected on ubuntu-latest given ~50 source files in `scan_fs/`.

There is no persisted state from this entity — it runs once per CI invocation and is discarded.
