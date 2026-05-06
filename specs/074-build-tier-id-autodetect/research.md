# Research — milestone 074 build-tier identifier auto-detection

Six implementation-level decisions to pin before Phase 1 design. Each follows the Decision / Rationale / Alternatives format. Every decision either reuses milestone 073's contract verbatim or extends 073 in a documented, narrow way.

## §1 — `git rev-parse HEAD` invocation pattern

**Decision**: Add a private helper `fn git_rev_parse_head(scan_root: &Path) -> Option<String>` in `auto_detect.rs` that mirrors the existing `git_remote_get_url` / `git_remote_list` shell-out style. Subprocess: `Command::new("git").arg("-C").arg(scan_root).arg("rev-parse").arg("HEAD")`. Capture stdout, trim trailing newline, return `Some(sha)` if 40-char hex, else `None` with `tracing::info!`.

**Rationale**: Identical pattern to the existing helpers — same crate, same imports, same failure-handling discipline, same logging idiom. Reading the existing helpers (`auto_detect.rs:32`+ for `git_remote_get_url`, the parallel helper for `git_remote_list`) gives the new helper its shape for free. Using `-C <dir>` rather than spawning with `current_dir()` matches the existing pattern and lets the helper accept a path argument cleanly.

**Failure modes covered**:
- Not a git repo → `git -C <non-git-dir> rev-parse HEAD` exits 128 with `fatal: not a git repository`. The pre-flight `.git` existence check that the source-tier helper already does (line 32) prevents reaching this case.
- Empty repo, no commits yet → `git -C <empty-repo> rev-parse HEAD` exits 128 with `fatal: ambiguous argument 'HEAD'`. We check exit status and log info-level, return `None`.
- Detached HEAD → `git -C <detached> rev-parse HEAD` exits 0 with the detached SHA on stdout. Treated identically to attached HEAD (FR-002 + edge case in spec).
- `git` not on PATH → `Command::spawn` returns `io::Error`; mapped to `None` with info-log (same pattern as existing helpers).

**Alternatives considered**:
- `gitoxide` (`gix`) crate for in-process git access — adds a new workspace dependency and parses `.git/HEAD` itself. Rejected: the project's stated posture (per CLAUDE.md) is "shell out to `git`" (the same pattern milestones 053 and 073 use). Adding an in-process git crate would deviate without benefit at this scale.
- Reading `.git/HEAD` directly via `std::fs::read_to_string` — works for attached HEAD (returns `ref: refs/heads/...`) but requires walking the ref chain to resolve the SHA. Detached HEAD is simpler (file contains the SHA directly). Rejected: re-implementing the part of `git rev-parse` we need would add ~30 LOC plus its own test surface, while the subprocess shell-out is a 5-line helper.

## §2 — Tier-agnostic core refactor

**Decision**: Extract the URL-discovery half of `auto_detect_repo_identifier` into a new private function `fn discover_repo_url(scan_root: &Path) -> Option<(String, String, bool)>` returning `(url, remote_name, fallback_used)`. Both source-tier and build-tier callers call this core then attach their own `source_label`-formatting wrapper.

**Rationale**: The 3-step fallback algorithm (origin → upstream → first-listed) is identical for both tiers. Only the `source_label` string differs (per FR-006). Extracting the core eliminates duplication and guarantees the algorithm stays in lock-step. Source-tier `auto_detect_repo_identifier` becomes a thin wrapper that calls `discover_repo_url` and formats the source-tier label string. Build-tier gets a sibling wrapper.

The chosen signature returns the raw tuple rather than a richer struct because (a) the consumers' use cases are simple (label-format + identifier-construct), (b) the tuple-shape `(String, String, bool)` is self-documenting in context, and (c) introducing a new struct would expand the module's public surface for negligible benefit.

**Alternatives considered**:
- Pass a `tier: Tier` enum into the existing function and switch on it inside — Rejected: makes the existing function more complex than necessary; adds an enum that has only two variants; duplicates the label-format inside the existing function.
- Pass a `label_fn: impl Fn(&str, bool) -> String` closure — Rejected: harder to read at call sites than two named wrappers; closure-typing complicates the function signature for zero generality benefit (only two label formats exist).
- Don't refactor — duplicate the 3-step fallback at the build-tier call site — Rejected: violates DRY and creates a maintenance hazard if the fallback algorithm ever changes.

## §3 — Invocation cwd capture

**Decision**: Capture `let invocation_cwd = std::env::current_dir()?;` at the very top of `run.rs::execute(args: RunArgs)`, before any subprocess work or trace capture. The auto-detection function takes this `&Path` as input. Wrapped-command cwd changes (e.g., the script does `cd build/`) have no effect.

**Rationale**: Matches the operator's mental model ("the directory I ran mikebom from"). Source-tier 073 already takes `&Path` (`scan_root`) for the same reason. Failures in `current_dir()` (very rare — basically only happens if the directory was unlinked under us mid-invocation) propagate via `?` since `RunArgs::execute` already returns `anyhow::Result<()>`.

**Alternatives considered**:
- Use `args.path` (a CLI-supplied scan root) — Rejected: `mikebom trace run` doesn't have a `--path` flag; the build cwd is implicit. Inventing one for this milestone would add CLI surface that contradicts SC-005 ("zero new flags").
- Pass cwd through to the wrapped command's cwd → use that — Rejected: wrapped commands frequently `cd` into subdirs (`cd build && make`); using the wrapped command's later cwd would produce non-deterministic auto-detection.
- Default to `std::env::var("PWD")` then fall back to `current_dir()` — Rejected: `PWD` isn't reliably set in CI environments; `current_dir()` is the canonical Rust API.

## §4 — `source_label` shape for build-tier

**Decision**: For an auto-detected `repo:` identifier on build-tier, use `"auto-detected from build-tier git remote `<remote_name>`"` (and the fallback variant `"auto-detected from build-tier git remote `<remote_name>` (origin/upstream absent; first-listed)"`). For an auto-detected `git:` identifier, use `"auto-detected from build-tier `git rev-parse HEAD`"`.

**Rationale**: Symmetrically extends the source-tier strings already shipped at `auto_detect.rs:109-115` (`"auto-detected from git remote `<remote_name>`"`). Inserting `build-tier ` between `from` and `git remote` keeps the human-readable shape parseable by people skimming SBOMs. The `git:` label uses a different verb ("from `git rev-parse HEAD`") because the data source is different — distinguishing the two so a reader knows which subprocess produced which identifier.

These exact strings get pinned now (in research and in goldens) because they appear in cross-format byte-identity goldens and changing them later would force another regen.

**Alternatives considered**:
- Reuse the source-tier label verbatim and rely on the surrounding `mikebom:sbom-tier` annotation to disambiguate — Rejected: violates FR-006 (which explicitly calls for distinguishable labels). A consumer reading just the identifier without surrounding annotations should still know which tier produced it.
- Use a structured label format like `"tier=build remote=origin"` — Rejected: not consistent with the existing free-prose shape; nothing in the 073 contract reads `source_label` programmatically (it's purely for humans).
- Drop `source_label` for auto-detected build-tier identifiers — Rejected: violates Principle X (Transparency) and FR-006.

## §5 — Golden regen strategy

**Decision**: One additive regen of build-tier git-tracked fixture goldens. Specifically: any `mikebom trace run` integration-test fixture whose tempdir has a `.git` directory and a configured remote will gain (a) a `repo:` identifier slot and (b) a `git:` identifier slot in its emitted CDX/SPDX 2.3/SPDX 3 outputs. Existing non-git build-tier fixtures stay byte-identical.

**Rationale**: Milestone 073's golden regen for source-tier auto-detect followed the same shape — additive identifier slots, no other format changes. Goldens for non-git fixtures stay alpha-15-identical because no detection fires. The test plan needs to enumerate which existing fixtures are git-tracked and confirm each one's golden becomes additive only.

**Auditing fixtures pre-regen**:
1. Identify all `mikebom trace run` integration test fixtures by grepping for `trace::run` / `RunArgs` test invocations.
2. For each fixture, check whether its tempdir setup includes `git init` + remote configuration.
3. For each git-tracked fixture, predict the additive identifier set: at minimum `repo:`; `git:` only if a commit was made.
4. For each non-git fixture, confirm the absence of `.git` so we don't accidentally pick up unrelated state.

**Alternatives considered**:
- Force-disable build-tier auto-detect in test environments via an env var — Rejected: the whole point of integration tests is to exercise the new code path. Hiding it would defeat verification.
- Rebuild every build-tier fixture with deterministic git remote/HEAD values — Rejected for non-affected fixtures (would regen things that don't actually need to change). Affected fixtures get this naturally as part of golden regen.

## §6 — Manual-wins precedence at the new call site

**Decision**: Refactor the assembly logic at `run.rs:226-260+` to call milestone 073's existing `resolve_identifiers(...)` helper (currently used at `scan_cmd.rs:1471`) — passing the auto-detected `Vec<Identifier>` and the manual-flag-derived entries. The helper already implements 073's FR-006 manual-wins precedence + dedup-by-(scheme, value) rule.

**Rationale**: 073 already shipped the resolution logic; calling it preserves the same precedence semantics across both tiers and avoids re-implementing dedup. The current `run.rs` build-tier flow is "manual entries only" and intentionally simpler than scan-tier (per pre-074 FR-008 in the 073 spec, build-tier "doesn't auto-detect"). Once auto-detect lands, build-tier's flow becomes structurally identical to source-tier's, and the natural simplification is "use the same helper."

This refactor has a small risk: the helper signature may need a tiny tweak if it bakes in source-tier-specific assumptions. Verify in Phase 1 that `resolve_identifiers` is tier-agnostic (likely true — it operates on `Vec<Identifier>` and `Option<Identifier>` for the auto-detected entry, no tier metadata).

**Alternatives considered**:
- Inline the dedup logic at the build-tier call site — Rejected: duplicates 073's resolution rules (FR-006 in 073's spec); creates two places that must stay in sync if the precedence rule ever changes.
- Push the auto-detected entries into `args.id` before assembly so the existing build-tier flow handles them — Rejected: `args.id` is operator-supplied user-defined identifiers; mixing auto-detected entries in there would lose the auto-detected/manual distinction needed for dedup tie-breaking.

## §7 — URL form for the auto-detected `git:` identifier

**Decision**: The auto-detected `git:` identifier reuses the verbatim `repo:` URL — exactly whatever `git remote get-url <name>` returned, including SSH-form URLs like `git@github.com:acme/foo.git`. No URL normalization is performed. If milestone 073's `validate_git` value validator rejects the SSH form, the resulting `git:` identifier soft-fails to `IdentifierKind::UserDefined` per FR-010 — same soft-fail rule as for `repo:`. This produces the documented per-identifier classification behavior: `repo:` and `git:` are independently validated, and either may end up `Builtin` or `UserDefined` based on its own validator's verdict.

**Rationale**: Three reasons stack:
1. **Consistency with `repo:`**. Whatever URL form the operator's `git config` produced is what gets emitted. A normalization step here would create asymmetry — the source-tier `repo:` would emit `git@github.com:...` verbatim while build-tier `git:` would emit `https://github.com/...`. Operators reading both SBOMs would see two different URL strings for the same logical repository.
2. **Soft-fail is the documented contract**. FR-010 already covers the case where a value fails its scheme's validator. Per-identifier downgrade to `UserDefined` is the established 073 behavior. If `validate_git` doesn't accept SSH form today, the soft-fail path is the correct outcome — and a separate milestone can broaden `validate_git` later without changing 074's contract.
3. **Minimal surface**. A normalization helper would need its own test surface (SSH→HTTPS mapping for `github.com`, `gitlab.com`, `bitbucket.org`, etc.) and would cement assumptions about which Git hosting providers are first-class. None of that complexity earns its keep at this milestone's scope.

**Test coverage requirement**: T008 (US2 integration tests) MUST include a test confirming the chosen behavior on an SSH-form remote — i.e., emitted SBOM contains a `git:git@github.com:fake/repo.git#<sha>` identifier whose `kind` reflects whatever `validate_git` returns (`Builtin` if accepted, `UserDefined` if not). The test asserts the wire-format slot is present with the verbatim URL; it does NOT assert which `IdentifierKind` it carries (that's a 073-validator implementation detail to discover empirically at T004 time).

**Alternatives considered**:
- **Normalize SSH → HTTPS** before constructing the `git:` value (e.g., `git@github.com:acme/foo.git` → `https://github.com/acme/foo.git`) — Rejected: introduces hosting-provider-specific knowledge into a generic `auto_detect_repo_identifier` module. Either it's complete (covers GitHub, GitLab, Bitbucket, self-hosted Gitea, custom domains) and over-scoped for this milestone, or it's incomplete and surprises operators on uncovered hosts.
- **Reject SSH-form remotes outright at auto-detection time** — Rejected: source-tier 073 doesn't reject SSH-form `repo:` values, so build-tier rejecting SSH-form `git:` values would create a new asymmetry. FR-010's soft-fail is the right disposition.
- **Make URL normalization an opt-in flag** — Rejected: adds CLI surface, contradicts SC-005 (zero new flags), and re-litigates a decision that should follow 073's contract.
