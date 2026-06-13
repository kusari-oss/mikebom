# Feature Specification: Walker-Audit CI Gate

**Feature Branch**: `115-walker-audit-ci`
**Created**: 2026-06-13
**Status**: Draft
**Input**: User description: "Issue #342 — codify the milestone-114 `safe_walk` audit grep as a CI gate. Today the no-hand-rolled-walker invariant lives only in `scan_fs/walk.rs` comment block + `design-notes.md` and depends on reviewer discipline. Add a CI step that runs the audit grep and diffs against a committed allow-list, failing the build on any unexpected match. Includes a CONTRIBUTING update for the new-exception workflow."

## Clarifications

### Session 2026-06-13

- Q: How does the very first version of the gate ship — with a baseline allow-list, or auto-bootstrapping? → A: Ship a baseline allow-list as part of this PR. The implementing PR commits the 28-line snapshot of the current `grep -rEn 'fn walk[_(]'` output, sorted for stable diffing. After merge, the gate is in strict-enforcement mode from day one — a missing or empty allow-list fails the build (no silent-pass mode).

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A maintainer reviewing a PR gets the audit verdict automatically (Priority: P1)

A maintainer opens a pull request from a contributor who has added new code under the filesystem-scanning layer of the codebase. The maintainer reads the diff to evaluate the contribution's design, correctness, and test coverage. The maintainer does NOT remember to run any specific shell command to verify whether the contribution introduced a new hand-rolled filesystem walker that bypasses the shared helper. Continuous Integration runs automatically and either flashes a green check (the milestone-114 invariant holds — no new hand-rolled walker outside the shared helper or the documented exception list) or fails red with a clear message naming the offending file and pointing the contributor at the authoritative documentation for how to either migrate the new code to the shared helper OR document it as a new exception in the same PR. The maintainer makes a merge decision informed by the gate's verdict instead of by their own discipline at remembering to run a grep.

**Why this priority**: The milestone-114 migration's entire durability depends on this gate. Without it, the no-hand-rolled-walker invariant — documented today but unenforced — silently drifts as soon as one contributor adds a new walker and one reviewer doesn't run the audit grep. This is the contractual foundation of the feature; everything else is polish.

**Independent Test**: A test PR adds a new function whose name matches the audit pattern in a source file inside the filesystem-scanning layer but is NOT in the documented allow-list. The CI step exits with a non-zero status and the build is marked failed. A second test PR makes the same change AND also updates the allow-list (with a new entry naming the file + the one-sentence reason). The CI step exits with success and the build is marked green.

**Acceptance Scenarios**:

1. **Given** the post-114 baseline (28 audit-pattern matches across the documented file set), **When** CI runs against any branch with no further changes to the filesystem-scanning layer, **Then** the audit gate passes silently with no maintainer action required.
2. **Given** a PR that introduces a new hand-rolled walker in a file outside the documented allow-list, **When** CI runs, **Then** the gate fails with a stderr message naming the file + line + function name AND pointing the contributor at the authoritative documentation (the comment block at the top of the shared helper's module file).
3. **Given** the same PR as scenario 2 with the addition of an allow-list entry naming the new file + a one-sentence reason for the new exception, **When** CI runs, **Then** the gate passes — the maintainer sees the allow-list change in the diff and reviews the new exception's justification at the same time as the code change.
4. **Given** a PR that DELETES an entry from the documented allow-list AND removes the corresponding walker function (a contributor migrating an existing exception to the shared helper), **When** CI runs, **Then** the gate passes — the allow-list shrinks and the invariant strengthens.

---

### User Story 2 — A contributor adding a new ecosystem reader knows the workflow before they start (Priority: P2)

A contributor is adding a new ecosystem reader (a Swift Package Manager reader, an Erlang `rebar.lock` reader, a new C/C++ build-system tool, …) and reads the project's contribution documentation before opening the PR. The documentation explicitly tells them: "if you need a filesystem walker, use the shared helper at `<path>`. If your walker's semantics genuinely don't fit (rare — see the existing exception list for the four cases that don't), add an entry to `<allow-list-file>` AND a one-sentence reason at the top of `<helper-module-file>` IN THE SAME PR. The CI gate will reject the PR otherwise." The contributor reads this BEFORE writing code, so they make an informed choice from the start instead of getting a CI rejection after writing the PR.

**Why this priority**: The CI gate's value is amplified when contributors know the workflow up front. Without documentation, the gate is a stick that surprises people; with documentation, it's a guardrail that they expect. Discoverability is the difference between a feature that's helpful and a feature that's perceived as bureaucratic.

**Independent Test**: A contributor unfamiliar with milestone 114 reads only the contribution guide. They can correctly answer: (a) where the shared helper lives, (b) when it's appropriate to add a new exception vs migrate to the helper, (c) the exact files they need to edit to make the CI gate pass when adding an exception.

**Acceptance Scenarios**:

1. **Given** the project's contribution guide post-feature, **When** a contributor reads the section on adding a new ecosystem reader, **Then** they find a one-paragraph explanation of the helper + a pointer at the authoritative reference + a copy-pasteable workflow for the new-exception path.
2. **Given** a contributor's PR that documents a new exception, **When** the maintainer reviews it, **Then** the maintainer sees the allow-list change AND the comment-block reason in the same diff and can evaluate both with a single read.

---

### Edge Cases

- What happens when the audit grep returns a different ordering or whitespace shape on different platforms (Linux vs macOS vs Windows CI runners)? The comparison must be order-stable and whitespace-stable so the gate doesn't false-positive due to platform differences in `grep` output formatting.
- What happens when a contributor adds an entry to the allow-list but forgets to add the one-sentence reason to the helper module's comment block? The CI gate passes (the allow-list is satisfied) but the reviewer notices the asymmetry in the diff. This is acceptable — the gate enforces the procedural invariant ("new entries are intentional"), not the human-readable justification, which is the maintainer's review responsibility.
- What happens when a contributor adds the one-sentence reason but forgets the allow-list entry? The CI gate fails. The contributor reads the gate's error message, sees the workflow pointer, and adds the allow-list entry.
- What happens when the audit grep's output changes due to a refactor unrelated to walker code (e.g., a contributor renames a non-walker function whose name coincidentally matches the pattern)? The gate fails. The contributor adds the new line to the allow-list, OR renames the function to avoid the pattern. Either is a legitimate resolution.
- What happens when an allow-list entry is left in the file pointing at a deleted source file? The CI gate fails because the audit grep won't produce that line anymore — the allow-list contains a line that doesn't appear in actual code. The contributor removes the stale entry as part of their cleanup PR. This is a feature, not a bug — stale exceptions can't accumulate.
- What happens when a contributor's PR adds a test function whose name matches the audit pattern (e.g., a new `fn walks_*` test for a new walker scenario)? The gate fails until the contributor adds the test function to the allow-list. This is mildly annoying for test authors but enforces the principle that EVERY match is intentional — no silent drift.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A Continuous Integration step MUST run on every pull request and on every push to the main branch that exercises the milestone-114 audit pattern across the filesystem-scanning layer of the codebase. The gate's pass/fail verdict MUST be visible alongside other CI checks in the platform's pull-request user interface.
- **FR-002**: The audit gate MUST compare the actual audit-pattern output against a committed allow-list file checked into the source tree. The allow-list is the single source of truth for "intentional audit-pattern matches"; entries are added or removed by contributors as part of the PR that changes the underlying code.
- **FR-003**: The audit gate MUST fail the build with a non-zero exit status when the audit-pattern output contains any line not present in the allow-list, OR when the allow-list contains any line not present in the audit-pattern output. Both directions are enforced — the gate detects new walkers added without intent AND stale exceptions left behind after a walker is deleted.
- **FR-004**: The audit gate's failure message MUST identify the specific offending line(s) AND point the contributor at the authoritative documentation for how to resolve the failure (either migrate to the shared helper, or add an allow-list entry plus a one-sentence reason in the helper module's comment block).
- **FR-005**: The audit-pattern output and the allow-list MUST be order-stable and whitespace-stable across the project's supported CI runners. The gate MUST NOT false-positive due to filesystem ordering differences, line-ending differences, or platform-specific tool output variations.
- **FR-006**: The contribution documentation MUST describe the new-exception workflow: which file the contributor edits to add an allow-list entry, which file they edit to add the one-sentence reason, and the principle that both edits land in the SAME pull request as the new walker code. The documentation MUST be discoverable from the project's standard contribution-guide entry point.
- **FR-007**: The audit gate MUST run quickly — substantially faster than the existing test suite — so the maintainer's pull-request feedback latency is not materially affected by the new check.
- **FR-008**: The allow-list file's location, name, and format MUST be consistent with the project's existing convention for source-tree-tracked configuration files (no new file-type conventions introduced). Contributors editing the allow-list use the same editing workflow they already use for source code.
- **FR-009**: A maintainer with no prior knowledge of milestone 114 MUST be able to evaluate a contributor's allow-list change AND the corresponding code change in a single diff view without consulting external documentation, by reading the inline diff alone.
- **FR-010**: The pull request that introduces this feature MUST commit the baseline allow-list at the same time as the CI step. The baseline reflects the post-milestone-114 audit-pattern output (28 entries at feature ship time). The gate operates in strict-enforcement mode from the moment it ships — there is NO silent-pass mode for "allow-list file missing" or "allow-list file empty." A missing or empty allow-list fails the build the same way an unauthorized walker addition does. This guarantees SC-001's "zero PRs merge without explicit acknowledgment" outcome holds even when a future PR accidentally or maliciously deletes the allow-list file.

### Key Entities

- **Allow-list file**: A committed source-tree file enumerating every line of the audit-pattern output that the maintainers have intentionally accepted as a non-violation. Each entry corresponds to a specific file path + line number + function name combination. Adding or removing an entry is a deliberate maintainer-reviewable decision.
- **Audit-pattern output**: The set of lines produced when the documented walker-audit pattern is executed against the filesystem-scanning layer of the codebase. Each line names a file + line number + function declaration whose name matches the milestone-114 walker convention.
- **CI step**: The automated check that compares the live audit-pattern output against the committed allow-list and emits a pass/fail verdict. Runs as part of every pull-request build and every main-branch push.
- **New-exception workflow**: The documented procedure a contributor follows when they have determined that their new code legitimately cannot delegate to the shared helper. Edits two files (allow-list + helper module's comment block) in the same pull request.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: After this feature ships, every pull request that introduces a new hand-rolled walker outside the shared helper without a corresponding allow-list entry fails Continuous Integration. Zero such pull requests can merge to the main branch without explicit maintainer acknowledgment via an allow-list edit.
- **SC-002**: The CI step that runs the audit gate completes in under five seconds on every supported CI runner. The gate's runtime is not a meaningful contributor to overall pull-request feedback latency.
- **SC-003**: A contributor adding a new ecosystem reader can read only the project's contribution guide (no other documentation, no source-code archaeology) and correctly know whether they should use the shared helper, OR document a new exception, OR rename a non-walker function whose name accidentally matches the pattern.
- **SC-004**: After this feature ships, the count of intentional walker-audit allow-list entries decreases over time as future milestones migrate existing documented exceptions into the shared helper. The allow-list is a shrinking surface, not a growing one. (Verified at the time of every future milestone touching walker code.)
- **SC-005**: A maintainer reviewing a pull request that touches walker code can evaluate the proposed allow-list change AND the underlying code change with one diff read, with no need to switch tabs to the helper's comment block or to external documentation. The two artifacts are spatially close in the diff view.

## Assumptions

- The feature's scope is enforcement of the existing milestone-114 audit pattern. Designing a different audit pattern, generalizing the gate to other source-code invariants (e.g., a no-`.unwrap()`-in-production check), or porting the gate to a custom lint rule are all out of scope.
- The CI gate runs on a single Linux CI runner (slotted into the existing `Lint + test (linux-x86_64)` job). The audit pattern is OS-agnostic by design — `grep` over checked-in source produces byte-identical output regardless of host OS, so multi-OS execution would triple cost for zero additional signal. FR-005's stability guarantee is preserved by the pattern's OS-agnosticism, not by per-OS replication. No new CI runner provisioning is required.
- The allow-list format is human-editable plain text. JSON / YAML / TOML schemas are over-engineering for what is fundamentally a list of "line of grep output, signed off as intentional."
- The new-exception workflow lives in the same documentation file family as the existing contribution guidance. Creating a new top-level documentation tree or a separate "milestone-114 governance" document is unnecessary overhead.
- Walker-audit allow-list entries are stable across the supported CI runners. If a future change introduces platform-dependent grep output (different line endings, different file-path separators), the gate's normalization step handles it at runtime — it does NOT require per-platform allow-list files.
- The "audit pattern" is the documented one from milestone 114 (`fn walk[_(]` regex over the filesystem-scanning layer). Extending the pattern to additional regexes or additional directory trees is out of scope.
- A small "negative test" that proves the gate fails when a new unauthorized walker is introduced is the principal validation mechanism. A full unit-test suite for the gate's internal logic is unnecessary — the gate is a single shell pipeline.
