# Feature Specification: Build-tier identifier auto-detection

**Feature Branch**: `074-build-tier-id-autodetect`
**Created**: 2026-05-05
**Status**: Draft
**Input**: User description: "When `mikebom trace run` (build-tier) executes in a git checkout, auto-detect `repo:` and `git:` identifiers without operator intervention. Symmetric with milestone 073's source-tier auto-detection. No index, no resolver — just identifier emission. External tools that have all three tier SBOMs can correlate them by reading identifier fields directly."

## Overview

Milestone 073 shipped identifier auto-detection for source-tier and image-tier scans:

- Source-tier (`mikebom sbom scan --path`) auto-detects `repo:` from `git remote get-url origin` (with `upstream` and first-listed fallbacks).
- Image-tier (`mikebom sbom scan --image`) auto-detects `image:registry/name:tag@sha256:digest` from the resolved image reference.
- Build-tier (`mikebom trace run`) does **not** auto-detect anything. Operators must pass `--repo` and `--git-ref` manually on every invocation.

The asymmetry is a usability gap: when a build runs in a git checkout (the common case for both local development and CI), the build's cwd already carries the same `repo:` information that source-tier picks up automatically. Build-tier additionally has access to the specific commit-of-record (`git rev-parse HEAD`) which lets it emit a `git:<repo-url>#<sha>` identifier that source-tier scans typically can't (since source scans are not commit-anchored).

This milestone closes the gap: `mikebom trace run` invoked in a git checkout auto-detects both `repo:` and `git:` without operator intervention.

The deliverable is purely about identifier emission. External tools or operators holding all three tier SBOMs (source, build, image) can correlate them by reading the identifier fields. Mikebom does not perform the correlation in this milestone — that's reserved for a future milestone (local index, OCI referrers, or external-registry resolvers, scoped separately).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Auto-detected `repo:` on build-tier scans (Priority: P1)

A developer or CI runner invokes `mikebom trace run -- ./build.sh` inside a git checkout. The emitted build-tier SBOM carries a `repo:` identifier auto-detected from the same git remote that source-tier scans would pick up — no manual flag required.

**Why this priority**: This is the core user-visible value of the milestone. Without it, `mikebom trace run` continues to require manual `--repo` flags on every invocation, which is the friction the user explicitly called out. Symmetric with 073's source-tier behavior so operators don't have to remember "source auto-detects, build doesn't."

**Independent Test**: Initialize a git repo, set `origin` to a known URL, run `mikebom trace run -- /usr/bin/true` with no manual identifier flags, and verify the emitted build-tier SBOM contains a `repo:` identifier with the configured origin URL.

**Acceptance Scenarios**:

1. **Given** a build cwd that is a git checkout with `origin` set to `git@github.com:acme/foo.git`, **When** the operator runs `mikebom trace run -- ./build.sh` with no identifier flags, **Then** the emitted build-tier SBOM contains a `repo:git@github.com:acme/foo.git` identifier with `source_label` indicating it was auto-detected from the build-tier git remote.
2. **Given** a build cwd that is a git checkout with no `origin` but with `upstream` configured, **When** the operator runs `mikebom trace run -- ./build.sh`, **Then** the emitted SBOM picks up the `upstream` URL — same fallback algorithm as source-tier 073.
3. **Given** a build cwd that is not a git checkout, **When** the operator runs `mikebom trace run -- ./build.sh`, **Then** no `repo:` identifier is emitted, no error is raised, and an info-level log line records that detection was skipped.
4. **Given** a build cwd that is a git checkout, **When** the operator runs `mikebom trace run --repo git@github.com:acme/override.git -- ./build.sh`, **Then** the manual flag wins and the emitted SBOM carries `repo:git@github.com:acme/override.git`, not the auto-detected origin URL — same precedence as 073's source-tier.

---

### User Story 2 - Auto-detected commit-anchored `git:` on build-tier scans (Priority: P1)

The same `mikebom trace run` invocation additionally emits a `git:<repo-url>#<commit-sha>` identifier capturing the specific commit the build was performed against. This is the build-tier-specific addition: source-tier scans don't naturally carry a commit anchor (a working tree may have uncommitted changes), but a build-tier scan running in CI almost always corresponds to a specific HEAD commit.

**Why this priority**: The commit-anchored identifier is what makes build-tier SBOMs uniquely useful for cross-tier correlation. Two different builds of the same repo at different commits both carry `repo:git@github.com:acme/foo.git`, but their `git:` identifiers differ — so an operator holding all the SBOMs can tell "this image was built from commit X, that one from commit Y." Without this, build-tier scans add noise rather than signal.

**Independent Test**: In a git checkout with a known HEAD commit, run `mikebom trace run -- /usr/bin/true` and verify the emitted build-tier SBOM contains a `git:<remote-url>#<HEAD-sha>` identifier. The `<remote-url>` matches whatever `repo:` resolved to (FR-001); the `<HEAD-sha>` matches `git rev-parse HEAD`.

**Acceptance Scenarios**:

1. **Given** a git checkout at commit `abc1234567890abcdef1234567890abcdef1234`, **When** the operator runs `mikebom trace run -- ./build.sh`, **Then** the emitted SBOM contains `git:<remote-url>#abc1234567890abcdef1234567890abcdef1234`. The full 40-character commit sha is preserved (no abbreviation).
2. **Given** a git checkout in detached-HEAD state at a known commit, **When** the operator runs `mikebom trace run`, **Then** the emitted SBOM still carries the `git:` identifier with the detached commit sha — detached HEAD is a normal CI state, not an error condition.
3. **Given** a git checkout where `git rev-parse HEAD` fails (e.g., a freshly initialized repo with no commits yet), **When** the operator runs `mikebom trace run`, **Then** the `repo:` identifier still emits if a remote is configured, but the `git:` identifier is silently skipped with an info-level log line. The scan does not fail.
4. **Given** a build cwd that is not a git checkout at all, **When** the operator runs `mikebom trace run`, **Then** neither `repo:` nor `git:` is emitted; the build-tier SBOM contains zero auto-detected identifiers (manual flags can still be passed).

---

### Edge Cases

- **Build runs in a subdirectory of a git checkout**: The build cwd is, say, `myrepo/build/`, while the git root is `myrepo/`. Auto-detection must walk up to find the git root — same behavior as source-tier 073, which delegates to `git` itself for the walk.
- **Build runs in `/tmp` or some out-of-tree directory**: Build is in a non-git directory entirely. Auto-detection must skip silently (info-log only, no error). Source-tier 073 handles this same way.
- **Detached HEAD**: A typical CI state when the runner checks out by SHA rather than by branch. `git rev-parse HEAD` still returns the detached SHA. The `git:` identifier emits normally. This is not an error condition.
- **Empty repo with no commits**: `git init` was run but no commit was made. `git rev-parse HEAD` fails. `repo:` may still emit if a remote was configured before any commit; `git:` skips silently.
- **Multiple remotes with no `origin` and no `upstream`**: Same fallback as source-tier — pick the first-listed remote alphabetically, log which remote was selected.
- **Manual flag overrides**: Operator passes `--repo`, `--git-ref`, or `--id` flags. Manual values win and the auto-detected entry is dropped (matches 073's FR-006 manual-wins semantics). Mixed cases (e.g., manual `--repo` + auto-detected `git:`) are supported — manual flags only override the specific scheme they specify.
- **Build cwd vs wrapped-command cwd**: `mikebom trace run -- ./build.sh` invokes the wrapped command. If the wrapped script itself does `cd` to a different directory, the auto-detection still uses the cwd at the moment `mikebom trace run` was invoked — not the wrapped command's later cwd. This keeps the detection deterministic and matches the operator's mental model ("the directory I ran mikebom from").
- **Non-deterministic remote selection collision**: Remote configuration changes between two consecutive invocations (e.g., `origin` URL was updated). Each invocation uses its own snapshot of remote state — there's no global cache. This is consistent with 073's behavior.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `mikebom trace run` MUST auto-detect a `repo:` identifier from the git remote configured for the invocation cwd, using the same selection algorithm as milestone 073's source-tier auto-detection (`origin` → `upstream` → first-listed alphabetically).
- **FR-002**: `mikebom trace run` MUST auto-detect a `git:<repo-url>#<commit-sha>` identifier where `<repo-url>` matches the same remote URL selected by FR-001 and `<commit-sha>` is the full 40-character SHA returned by `git rev-parse HEAD` in the invocation cwd.
- **FR-003**: When the invocation cwd is not a git checkout, OR has no remote configured, OR `git rev-parse HEAD` fails, the affected identifier(s) MUST be silently skipped with an info-level log line. The scan MUST NOT fail. This mirrors 073's source-tier soft-fail behavior.
- **FR-004**: Manual identifier flags (`--repo`, `--git-ref`, `--id <scheme>=<value>`) MUST take precedence over auto-detected values. When a manual flag and an auto-detected value share the same `(scheme, value)` pair, deduplication treats them as a single entry attributed to the manual flag (mirrors 073's FR-006).
- **FR-005**: Auto-detection MUST execute exactly once per `mikebom trace run` invocation, before the wrapped command starts. The detected identifiers MUST be cached in the in-memory identifier vector and threaded through to all SBOM emitters — no per-format re-detection.
- **FR-006**: Auto-detected build-tier identifiers MUST set the `source_label` field to a human-readable string distinguishing build-tier from source-tier auto-detection (e.g., `"auto-detected from build-tier git remote `origin`"`). This is informational only — consumers that don't read `source_label` see no behavior change.
- **FR-007**: Cross-format byte-identity goldens for build-tier scans in non-git fixtures MUST remain identical to alpha.16 (no detection fires; no behavior change). Goldens for build-tier scans in git-tracked fixtures get one additive `repo:` slot and one additive `git:` slot per format — that is the expected golden regen for this milestone.
- **FR-008**: The auto-detection logic MUST share the same URL-discovery core as milestone 073's source-tier `auto_detect_repo_identifier` (the existing helper in the identifiers module). Source-tier and build-tier each call into a common URL-discovery function and attach their own tier-specific `source_label`. Build-tier extends, not duplicates. The shared core guarantees fallback semantics, error-handling, and logging stay identical across tiers.
- **FR-009**: Determinism: given a fixed git remote configuration and a fixed `HEAD` commit at invocation time, repeated invocations of `mikebom trace run` against the same wrapped command MUST produce byte-identical identifier slots in the emitted SBOMs. Identifier ordering follows 073's contract: auto-detected entries first, in the order `repo:` then `git:`, then manual flags in supply order.
- **FR-010**: When the build cwd's git remote is malformed (returns a string that fails 073's `repo:` value validator), the auto-detection MUST follow 073's soft-fail-to-opaque rule: emit the identifier under the `mikebom:identifiers` user-defined namespace with a `tracing::warn!` log, rather than rejecting the value. The build scan MUST NOT fail.

### Key Entities

- **BuildTierAutoDetectionResult**: The set of identifiers auto-detected for a single `mikebom trace run` invocation. Composed of: an optional `repo:` identifier (per FR-001), an optional `git:` identifier (per FR-002), each with a `source_label` describing the auto-detection origin. Result is computed once at invocation start and immutable for the duration of the trace.
- **InvocationCwd**: The directory `mikebom trace run` was invoked from. This is the input to auto-detection — *not* the wrapped command's later runtime cwd. Fixed at invocation start.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A developer or CI runner invoking `mikebom trace run -- ./build.sh` in a typical git checkout (any project that has `origin` set) gets a build-tier SBOM containing both `repo:` and `git:` identifiers with zero manual flags. Verified by integration test against a fixture project with a known git remote and commit.
- **SC-002**: An operator holding all three tier SBOMs (source from `mikebom sbom scan --path`, build from `mikebom trace run`, image from `mikebom sbom scan --image`) — all produced from the same git checkout at the same commit — can correlate them by reading the identifier fields directly. The source SBOM's `repo:` value matches the build SBOM's `repo:` value byte-for-byte. The build SBOM's `git:` commit sha matches `git rev-parse HEAD` of the source checkout. Verified end-to-end in a 3-tier integration test.
- **SC-003**: 100% of existing milestone-073 source-tier and image-tier byte-identity goldens remain unchanged after milestone 074 ships. Build-tier goldens for git-tracked fixtures gain the additive `repo:` and `git:` slots; build-tier goldens for non-git fixtures remain unchanged. Verified by the existing parity-check golden suite.
- **SC-004**: The auto-detection adds less than 100ms to `mikebom trace run` invocation time on a typical git repository (single `git remote get-url` + single `git rev-parse HEAD` invocation, both bounded by git's process-spawn cost). Verified by manual smoke timing during quickstart validation; CI-level regression is caught implicitly by the build-tier integration-test wall-time staying within its established envelope. (A dedicated benchmark fixture is deliberately out of scope — the auto-detection bottleneck is two `Command::spawn` calls, both of which `git`'s own performance contract bounds; instrumenting them with a microbenchmark is overkill for the cost-of-change.)
- **SC-005**: Operators do not need to learn any new flags or commands for milestone 074. The set of CLI flags is unchanged from 073 (`--repo`, `--git-ref`, `--image-id`, `--attestation`, `--id`); behavior changes only in the no-flag default. Verified by `mikebom trace run --help` output containing no new flag entries beyond 073's set.
- **SC-006**: When the invocation cwd is not a git checkout, `mikebom trace run` produces a build-tier SBOM with zero auto-detected identifiers and zero error/warning output beyond a single info-level log line. Verified by integration test against a tempdir-based non-git fixture.
- **SC-007**: Build-tier identifier auto-detection is deterministic across re-runs given a fixed git state. Re-running `mikebom trace run` against the same checkout at the same commit produces byte-identical identifier slots. Verified by golden-file equivalence testing on a git-tracked fixture.

## Assumptions

- The implementation reuses milestone 073's existing `auto_detect_repo_identifier` helper. Build-tier auto-detection is a new caller of an existing function plus a new sibling function for the `git:` commit-sha extraction. No duplication of the remote-selection algorithm.
- Build cwd is captured at the moment `mikebom trace run` is invoked — *not* whatever cwd the wrapped command later runs in. This is consistent with how operators think about "where am I running mikebom from."
- The `git:` identifier value uses the same `<repo-url>#<commit-sha>` format as 073's manual `--git-ref` flag. Format is documented in `docs/reference/identifiers.md` (alpha.16). No new format invented.
- Operators who want different behavior — e.g., disabling auto-detection entirely, or anchoring `git:` to a different commit (the parent commit, the merge-base) — can use the existing 073 manual flags. Auto-detection is a default; explicit flags always override.
- Network-based identifier resolution (the original draft of this milestone — local index + resolver + cross-machine binding) is out of scope. Whoever holds all three tier SBOMs correlates them externally by reading identifier fields. Automated correlation via index/registry/resolver is reserved for a future milestone (working name: 075+).
- `git` is available on `PATH` at `mikebom trace run` invocation time. This matches the assumption already made by milestone 073's source-tier auto-detection (and milestone 053's `git describe` ladder for Go module versioning).
- The `mikebom:sbom-tier` annotation already distinguishes build-tier from source-tier in emitted SBOMs (shipped in earlier milestones). Build-tier auto-detected identifiers do not need to carry tier information themselves — the surrounding SBOM context already provides it.
- Goldens for git-tracked fixtures will require a one-time additive regen as part of this milestone's PR — same kind of regen seen in milestone 073 when source-tier auto-detect was added. Non-git fixtures stay byte-identical.
