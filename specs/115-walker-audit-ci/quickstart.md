# Quickstart — Triaging a Red Walker-Audit Gate

**Feature**: 115-walker-audit-ci
**Audience**: a contributor whose PR just hit a red `Walker-audit allow-list check` step in CI; a maintainer reviewing a PR that touches the gate.

## The TL;DR

If your PR is red on `Walker-audit allow-list check`:

1. Open the failing CI log. The diff hunks identify the offending lines.
2. Decide: did you ADD a new `fn walk_*` on purpose, or by accident?
3. By accident: refactor to use `scan_fs::walk::safe_walk` (see `mikebom-cli/src/scan_fs/walk.rs` module-level comment).
4. On purpose: append the new grep-output line to `mikebom-cli/src/scan_fs/walk.audit-allowlist.txt` (sorted) AND add a one-sentence reason to `walk.rs`'s comment block. Both in the same PR.

## Five-minute walkthrough — Scenario A: accidental new walker

You added a new ecosystem reader at `mikebom-cli/src/scan_fs/package_db/elixir/mix_lock.rs` and the CI failed:

```text
[FAIL] Walker-audit allow-list mismatch — see mikebom-cli/src/scan_fs/walk.rs's module-level comment for the exception policy.

--- mikebom-cli/src/scan_fs/walk.audit-allowlist.txt (expected)
+++ live: grep -rEn --include='*.rs' 'fn walk[_(]' mikebom-cli/src/scan_fs/ | sort -u (actual)
@@ -27,3 +27,4 @@
 mikebom-cli/src/scan_fs/walk.rs:142:pub(crate) fn safe_walk<F: FnMut(&Path)>(
 mikebom-cli/src/scan_fs/walker.rs:55:fn walk(...)
+mikebom-cli/src/scan_fs/package_db/elixir/mix_lock.rs:88:fn walk_for_mix_locks(root: &Path) -> Vec<PathBuf> {
 mikebom-cli/src/scan_fs/package_db/npm/walk.rs:73:fn walk_node_modules(...)
```

Diagnosis: you wrote a hand-rolled recursion when you could have used the shared helper.

Fix:

```rust
use crate::scan_fs::walk::{safe_walk, WalkConfig};

pub fn read(rootfs: &Path, exclude_set: &ExclusionSet) -> Vec<PackageDbEntry> {
    let mut out = Vec::new();
    let cfg = WalkConfig {
        max_depth: 6,
        should_skip: &|p, _| should_skip_default_descent(p.file_name().and_then(|s| s.to_str()).unwrap_or("")),
        exclude_set,
    };
    safe_walk(rootfs, &cfg, |path| {
        if path.file_name() == Some(OsStr::new("mix.lock")) {
            if let Some(entry) = parse_mix_lock(path) {
                out.push(entry);
            }
        }
    });
    out
}
```

The audit gate goes green automatically — your new file's only `fn walk*` match is GONE because you no longer have one.

## Five-minute walkthrough — Scenario B: legitimate new exception

You're adding a Swift Package Manager reader. The Swift manifest semantics genuinely can't fit the generic `safe_walk` (e.g., you need to recurse into specific sub-trees while pruning others mid-descent — the per-descent state can't be expressed in the `should_skip` closure).

You write `fn walk_swift_packages(...)` in `mikebom-cli/src/scan_fs/package_db/swift_pm.rs`. CI fails.

Fix (TWO edits in your PR):

**Edit 1**: Add the new line to the allow-list. Run the regenerator locally:

```bash
grep -rEn --include='*.rs' 'fn walk[_(]' mikebom-cli/src/scan_fs/ | LC_ALL=C sort -u > mikebom-cli/src/scan_fs/walk.audit-allowlist.txt
git add mikebom-cli/src/scan_fs/walk.audit-allowlist.txt
git diff --cached mikebom-cli/src/scan_fs/walk.audit-allowlist.txt
```

The diff should show ONE added line. If it shows more than one, you're either (a) regenerating against unrelated upstream drift — rebase first — or (b) you accidentally added more than one new walker.

**Edit 2**: Add a sentence to `mikebom-cli/src/scan_fs/walk.rs`'s module-level comment block, in the "Documented known exceptions" subsection:

```rust
//! ## Documented known exceptions
//! ...
//! - `scan_fs/package_db/swift_pm.rs::walk_swift_packages` — Swift package
//!   manifests carry pre-declared sub-directory targets that prune mid-descent
//!   based on per-package-state; the generic should_skip can't express
//!   per-descent stateful pruning. See PR #NNN.
```

Both edits commit-together. The CI gate goes green. Reviewers see both the source-tree edit AND the policy edit AND the doc edit in one PR — exactly what SC-005 promises.

## Maintainer triage — Reviewing a PR that adds an allow-list entry

When you see `walk.audit-allowlist.txt` in the diff list of an incoming PR, your evaluation checklist:

1. **One new entry, not several** — multiple new entries are a code smell. Push back: "Can these consolidate to one walker that calls safe_walk for the common case?"
2. **Comment-block entry in `walk.rs`** — verify the contributor added the one-sentence reason. If missing, request it. The gate doesn't enforce this (per the spec's Edge Cases) but reviewers do.
3. **The reason is concrete** — "doesn't fit safe_walk" is not a reason; "per-descent stateful pruning" is. Push back on vague reasons.
4. **The "Why exempt" claim is testable** — could a reasonable refactor of `safe_walk` accommodate this? If yes, ask the contributor to evaluate the refactor instead of growing the exception list. The spec's "ten exceptions = abstraction is failing" threshold is your aspirational ceiling.
5. **The entry is sorted in correctly** — verify `LC_ALL=C sort -u mikebom-cli/src/scan_fs/walk.audit-allowlist.txt | diff - mikebom-cli/src/scan_fs/walk.audit-allowlist.txt` produces empty output.

## Negative test — Verifying the gate works

If you need to convince yourself the gate is wired up correctly (e.g., before merging the bootstrap PR), this is the runbook:

```bash
# 1. Note the current state.
git status
git log -1 --oneline

# 2. Introduce a synthetic violation in a throwaway branch.
git checkout -b verify-walker-audit-gate
cat >> mikebom-cli/src/scan_fs/synthetic_negative_test.rs <<'EOF'
// THIS FILE EXISTS ONLY TO VERIFY THE WALKER-AUDIT GATE BLOCKS UNEXPECTED ADDITIONS.
// DO NOT MERGE. Delete this file after verifying the gate fails red on the PR.
fn walk_synthetic_negative(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    vec![root.to_path_buf()]
}
EOF
git add mikebom-cli/src/scan_fs/synthetic_negative_test.rs
git commit -m "test: synthetic walker-audit negative test (DO NOT MERGE)"
git push -u origin verify-walker-audit-gate

# 3. Open the PR. Expected: the Walker-audit allow-list check step fails red,
#    diff hunk shows the synthetic file's fn walk_synthetic_negative line as the
#    sole +entry. The clippy + test steps SHOULD short-circuit before running
#    because the audit step runs first.

# 4. Verify the failure-message contract:
#    - Headline starts with [FAIL].
#    - Diff hunks identify the synthetic file + line.
#    - Trailing pointer references CONTRIBUTING.md § Walker-audit CI gate.

# 5. Close the PR + delete the branch.
gh pr close --comment "verifying gate behavior; closing" --delete-branch
```

If the PR turns green, the gate is broken — escalate before merging the bootstrap PR.

## When NOT to interact with the gate

You don't touch the allow-list when:

- You're refactoring inside an existing walker file (renaming variables, extracting helpers) — as long as no NEW `fn walk*` shows up, the grep output is unchanged.
- You're adding a non-walker function that happens to be named `fn walker_*` or `fn walking_*` — the regex `'fn walk[_(]'` matches only `fn walk_*` or `fn walk(` exactly; `walker_` / `walking_` don't match. Verify with `grep -rEn --include='*.rs' 'fn walk[_(]' your-new-file.rs`.
- You're working in a different directory (`mikebom-common/`, `mikebom-ebpf/`) — the gate scopes to `mikebom-cli/src/scan_fs/` only.

## When the gate's own infrastructure changes

If a future PR moves `walk.rs` itself, or relocates `scan_fs/` to a different module path, the audit pattern must change in lock-step with the allow-list paths. The grep command in `ci.yml`, the allow-list path, AND the documentation in CONTRIBUTING.md all reference `mikebom-cli/src/scan_fs/` — keep them aligned.

## Related docs

- `CONTRIBUTING.md § Walker-audit CI gate` — the contributor-facing doc-of-record (FR-006 / FR-007)
- `mikebom-cli/src/scan_fs/walk.rs` module-level comment block — the per-exception reason list (milestone 114 / this milestone's reviewer policing)
- `docs/design-notes.md § Filesystem walking pattern (milestone 114)` — the design context + cross-link to the contributor doc
- This feature's [contracts/ci-step.md](./contracts/ci-step.md) — the step's externally observable contract
