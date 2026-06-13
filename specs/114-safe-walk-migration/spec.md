# Feature Specification: Shared `safe_walk` Helper Migration

**Feature Branch**: `114-safe-walk-migration`
**Created**: 2026-06-12
**Status**: Draft
**Input**: User description: "Issue #108 — migrate every hand-rolled filesystem walker in `mikebom-cli/src/scan_fs/` to a single shared `safe_walk` helper. Today ~13 walkers carry near-identical canonicalize-keyed visited-set + depth-limit + skip-list code; each new ecosystem reader copy-pastes the closest match and risks dropping a loop-protection invariant (the bug pattern milestone 054 originally hardened against). Extract `project_roots.rs`'s pattern into `scan_fs::walk::safe_walk`, port every existing walker. Pure internal refactor; no goldens regen; no behavior change."

## Clarifications

### Session 2026-06-12

- Q: What shape does the helper's discovery callback take? → A: Single unified callback invoked for every visited path (file or directory); the caller's closure filters for file-vs-directory and ecosystem-specific name/extension. One descent function in the helper.
- Q: What signature does the skip predicate take? → A: `(candidate_path, rootfs)` — the predicate receives the full candidate `&Path` and the scan rootfs `&Path`, letting it extract filename via `file_name()` AND compute candidate-relative-to-rootfs for milestone-113-style path matching in one closure.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A scanner author adds a new ecosystem reader without re-deriving symlink-safe descent (Priority: P1)

A contributor adds a new ecosystem reader (a hypothetical Swift Package Manager reader, an Erlang `rebar.lock` reader, a freshly-discovered C/C++ build-system tool). To find the ecosystem's manifest files anywhere under the scan root, they reach for a filesystem walker. Today they would copy-paste the closest existing walker (cargo? maven? gem?), inherit ~40 lines of canonicalize-keyed visited-set + depth-limit + skip-list boilerplate, and have a non-zero chance of dropping or weakening one of those invariants in the process (the original milestone-054 hang). After this migration, they import one helper, supply a small per-call configuration object, and write the ecosystem-specific predicate + emission logic — fewer lines, no chance of re-deriving the loop-protection wrong.

**Why this priority**: This is the entire motivating value of the migration. The tax this issue eliminates is paid every time a new walker is added — and we add 1–2 per quarter at the current milestone cadence (105 / 106 / 107 / 113 each touched walker code). Without this story the migration delivers nothing.

**Independent Test**: A contributor writes a hypothetical new reader (or an existing maintainer rewrites one of the smaller walkers from scratch using the shared helper). The new code uses the helper, contains zero hand-rolled `read_dir` recursion, contains no `canonicalize`-keyed visited-set bookkeeping at the call site, and produces the same set of discovered paths as the equivalent pre-migration hand-rolled walker against a representative fixture directory.

**Acceptance Scenarios**:

1. **Given** a scanner author opens a new ecosystem reader file, **When** they need to enumerate every directory matching an ecosystem-specific predicate under the scan root, **Then** they call the shared helper with the predicate and a small configuration object — no `std::fs::read_dir` recursion is written, no canonicalize-keyed visited-set is allocated at the call site.
2. **Given** the shared helper is invoked against any reasonable filesystem layout (single project, container image rootfs, monorepo with N nested workspaces), **When** the descent completes, **Then** every directory matching the caller's predicate is visited exactly once even when symlink loops exist in the tree.
3. **Given** a tree contains a symlink loop (e.g., `a/loop → a`), **When** the helper is invoked, **Then** descent terminates within bounded time without panicking, without exhausting the stack, and without revisiting the same canonical directory.

---

### User Story 2 — A maintainer auditing the codebase can prove the no-hand-rolled-walker invariant in one grep (Priority: P2)

A maintainer reviewing a PR or auditing the codebase needs a fast, mechanical way to confirm that no contributor has introduced a hand-rolled filesystem walker that bypasses the shared loop-protection invariants. They run a single `grep` against the source tree and see zero matches outside the shared helper's own module, giving them a one-liner audit gate (suitable for inclusion in a future CI hook) that catches regressions automatically.

**Why this priority**: The migration's long-term value depends on it staying done. Without an enforceable audit, contributors will re-introduce hand-rolled walkers and the per-walker drift returns. The grep-based audit gate is the cheap durability story.

**Independent Test**: `grep -rn 'fn walk' mikebom-cli/src/scan_fs/` (or an equivalent pattern documented in `design-notes.md`) returns matches only inside the shared helper's own file. Anywhere else, the pattern is a review-failing signal.

**Acceptance Scenarios**:

1. **Given** the codebase post-migration, **When** a maintainer runs the documented audit grep, **Then** the only matches reside in the shared helper's module file itself.
2. **Given** a PR introduces a new `fn walk_*` function outside the shared helper, **When** the audit grep runs, **Then** the new function shows up as a match and the maintainer can reject the PR on that basis with no further investigation.

---

### User Story 3 — Operators and CI pipelines see byte-identical SBOM output across the migration (Priority: P1)

Every operator using mikebom — local developers, CI pipelines, image-signing services downstream of an SBOM-emission step — receives byte-identical SBOM output from a scan against the same input pre- and post-migration. Operators do not need to re-pin or re-baseline any of the 33 committed byte-identity goldens, do not need to re-validate downstream consumers' parsing, and do not need to update any documented expected output. The migration is invisible at the user-visible layer.

**Why this priority**: Tied with US1 for P1 because a refactor that secretly changes output is a behavior change, not a refactor. The byte-identity guarantee is the entire contract on "internal cleanup" — without it, this becomes a feature with all the associated risk-management overhead.

**Independent Test**: Run the existing byte-identity golden suite (33 goldens across CDX / SPDX 2.3 / SPDX 3) pre-migration and post-migration. The output bytes match exactly across all 33 goldens. No `MIKEBOM_UPDATE_*_GOLDENS` regen is required as part of this work.

**Acceptance Scenarios**:

1. **Given** a scan of any committed fixture, **When** operated by a pre-migration and post-migration build, **Then** the emitted CDX, SPDX 2.3, and SPDX 3 JSON bytes are identical (modulo the version string and any run-to-run nondeterminism already masked by the existing test harness).
2. **Given** the existing test suite, **When** invoked post-migration without any update flags, **Then** every test passes — no committed golden requires regeneration as a consequence of the migration.

---

### Edge Cases

- What happens when the walker encounters a directory it cannot read (permissions denied, broken symlink, transient I/O error)? The walker silently skips and continues — matches the pre-migration behavior of every existing walker, which tolerates unreadable directories rather than aborting the whole scan.
- What happens when the walker hits its configured depth limit? Descent stops at that depth; directories already enqueued continue to be visited but their children are not. Matches the pre-migration `MAX_PROJECT_ROOT_DEPTH` / `MAX_BINARY_WALK_DEPTH` / per-walker depth-bound behavior.
- What happens when a walker has unique structural semantics that don't fit a generic directory-recursion model (e.g., the deep-hash whole-filesystem file walker that yields every file under a size cap; the npm walker that distinguishes `@scope` directories from regular packages by their parent name)? Those walkers either use the helper with a richer-capability configuration object (the design challenge) OR are explicitly documented as exceptions inside the shared helper's module so the audit grep accommodates them. The choice is per-walker.
- What happens when a walker emits results inside the walk (as `yocto/recipe.rs` does, building `PackageDbEntry` records on the fly rather than collecting paths)? The shared helper accepts a caller-supplied callback rather than building its own collection — the emit-inside-walk pattern naturally fits a callback model.
- What happens when a walker needs additional state propagated through the descent (e.g., the npm walker propagates `in_npm_internals: bool` flag through its recursive calls)? The shared helper provides the callback access to per-descent context — implementation detail of how the callback closes over state, but the helper must not constrain it.
- What happens when the user-supplied `--exclude-path` directory-exclusion mechanism from milestone 113 needs to interact with the shared helper? The shared helper continues to honor the `ExclusionSet` parameter exactly as `project_roots.rs` did pre-migration; every per-walker callsite that previously consulted `ExclusionSet` at descent now does so through the helper without duplication.
- What happens when a future ecosystem reader needs a behavior the current shared helper doesn't expose (a different skip predicate signature, follow-symlinks-but-cap-loops, breadth-first vs depth-first)? The shared helper's configuration object is the extension surface — adding new fields is the supported way; forking the helper is not.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The codebase MUST expose a single shared filesystem-walking helper used by every ecosystem-reader filesystem walker. Contributors writing a new ecosystem reader MUST be able to enumerate paths under the scan root by configuring this helper, not by writing their own `read_dir` recursion.
- **FR-002**: The shared helper MUST guarantee the loop-protection invariants every existing walker provides today: a canonicalize-keyed visited-set that prevents revisiting any directory whose canonical path was already seen, and a per-descent depth-bound that prevents unbounded recursion when canonicalization is unavailable or unreliable.
- **FR-003**: The shared helper MUST tolerate unreadable directories (permissions denied, broken symlinks, transient I/O errors) by silently skipping them and continuing — descent against a partially-restricted filesystem produces whatever results it can rather than aborting.
- **FR-004**: The shared helper MUST accept a single caller-supplied callback through which every visited path — directories AND files — flows to the caller's emission logic. The caller's closure discriminates between files and directories and applies its own ecosystem-specific name/extension filter. The helper MUST NOT build its own collection in cases where the caller emits inside the walk; collection is the caller's choice.
- **FR-005**: The shared helper MUST accept a caller-supplied skip predicate that is consulted before descent into each child directory. The predicate signature MUST take both the candidate path and the scan rootfs so it can extract the candidate's filename AND compute the candidate's path relative to the scan root within a single closure — collapsing the milestone-113 split where the directory-exclusion check lived as a separate config field outside the predicate.
- **FR-006**: The shared helper MUST honor the milestone-113 user-supplied directory-exclusion set (`ExclusionSet`) at every descent decision, replacing the duplicate per-walker exclusion-check blocks that exist today. Operator-visible behavior of `--exclude-path` is unchanged.
- **FR-007**: Every existing hand-rolled walker under the source tree's filesystem-scanning layer MUST be ported to delegate to the shared helper. After migration, no hand-rolled `read_dir` recursion that performs project-root or manifest-file discovery MAY remain outside the shared helper's module.
- **FR-008**: Walkers with unique structural semantics that do not fit the generic shape (e.g., the whole-filesystem deep-hash walker that yields every file under a size cap; the npm walker that propagates per-descent state and treats `@scope` directories specially) MUST be either (a) migrated to use the helper with whatever richer-capability configuration object the design ends up needing, or (b) explicitly documented inside the shared helper's module as known exceptions with a stated reason — the audit grep MUST accommodate any such exception.
- **FR-009**: The migration MUST be functionally equivalent. The set of paths each per-ecosystem reader collects post-migration MUST equal the set it collected pre-migration against the same input. No committed byte-identity golden may regenerate as a consequence of the migration.
- **FR-010**: A documented audit pattern (e.g., a documented grep) MUST exist such that running it against the source tree returns matches only inside the shared helper's module file. Reviewers MAY use this pattern as a one-liner gate on future PRs to reject re-introduction of hand-rolled walkers.
- **FR-011**: The shared helper MUST be std-only — no new third-party Cargo dependencies introduced solely to host this refactor. The project's minimal-dependency posture is preserved.
- **FR-012**: A new section in `docs/design-notes.md` MUST point future contributors at the shared helper as the sole entry point for ecosystem-reader filesystem walking, and MUST document the audit pattern from FR-010 and the known-exception list from FR-008.

### Key Entities

- **Shared filesystem-walking helper**: The new module exposing the single entry point that every ecosystem reader uses to enumerate paths under the scan root. Configured per-call by the caller; carries no state of its own beyond the per-call visited-set.
- **Per-call walk configuration**: A small caller-supplied object holding everything the helper needs to perform a single descent — at minimum the max-depth bound, the skip predicate, and the active user-supplied exclusion set. Future-extensible without forcing a re-fork of the helper.
- **Caller-supplied callback**: The mechanism through which discovered paths flow back to the caller. Each ecosystem reader writes this callback to filter for its target file type and to emit its own per-ecosystem records.
- **Known exception**: A walker that does not migrate to the shared helper because its structural semantics genuinely don't fit (whole-filesystem deep-hash walker; npm `@scope` walker). Documented inside the helper's module file with a stated reason so the audit pattern remains usable.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: After the migration ships, the audit pattern from FR-010 returns matches only inside the shared helper's module file. Any contribution that introduces a `read_dir` recursion outside the helper fails the audit and is visible to a reviewer in a single grep.
- **SC-002**: The 33 committed byte-identity goldens (CDX + SPDX 2.3 + SPDX 3) pass byte-identically post-migration with no `MIKEBOM_UPDATE_*_GOLDENS` regen required. The full workspace test suite passes post-migration without any test being marked `#[ignore]` solely because of the refactor.
- **SC-003**: For the next ecosystem reader added to the project after this migration ships, the per-walker code that performs filesystem enumeration is at least 60% shorter than the equivalent hand-rolled walker would have been pre-migration. Measured as lines-of-Rust strictly inside the walker function (excluding the ecosystem-specific predicate / emission logic).
- **SC-004**: A scan against a representative polyglot fixture completes in time no worse than 105% of the pre-migration time. The migration MUST NOT introduce a measurable performance regression.
- **SC-005**: An audit of every previously-listed hand-rolled walker shows it now delegates to the shared helper OR appears in the helper module's documented known-exception list. No walker is silently left out.
- **SC-006**: The new `docs/design-notes.md` section is discoverable from a single search for "filesystem walking" and contains the audit-pattern grep, the known-exception list, and a one-paragraph example of how a new ecosystem reader uses the helper.

## Assumptions

- The scope of this migration is the filesystem-scanning layer of the codebase (the directory tree that hosts the per-ecosystem readers — today `mikebom-cli/src/scan_fs/`). Walkers in unrelated parts of the codebase (the eBPF event-handling pipeline, the in-memory CDX/SPDX traversal at emission time, the parity-extractor walks over already-parsed SBOM trees) are out of scope. Only the on-disk filesystem walkers reachable through the existing ecosystem-reader chain are touched.
- The pre-migration walker semantics are correct as of milestone 054 — every existing walker provides the canonicalize-keyed visited-set + depth-bound invariants. The migration preserves those semantics; it does not relitigate whether they're the right invariants.
- "Hand-rolled" excludes walkers in the existing reference-implementation file (`project_roots.rs`); the migration extracts that file's pattern, so the helper itself is allowed to contain `read_dir` recursion (that's the new sole site).
- Walkers that don't fit the generic shape (whole-filesystem deep-hash walker; npm `@scope` walker) are explicitly anticipated as documented exceptions, not bugs in the migration. The Acceptance Scenarios cover them. The audit pattern accommodates them by being scoped to the helper's module.
- "Byte-identical" is measured against the existing deterministic-emission environment (`MIKEBOM_FIXED_TIMESTAMP`, serial-number masking) that the project's test harness already applies. The migration neither introduces nor aggravates run-to-run nondeterminism.
- No new third-party Cargo dependencies (no `walkdir`, no `ignore`, no `globwalk`). The project's minimal-dependency posture is part of the contract for this work.
- The migration ships as one or more PRs as the implementer judges — there is no operator-visible "release boundary" that requires the whole migration to land at once. The byte-identity guarantee is per-PR, not just per-final-state.
- A future CI hook MAY codify the audit pattern from FR-010 as a hard gate, but doing so is out of scope for this migration. The hook itself is a separate follow-up.
