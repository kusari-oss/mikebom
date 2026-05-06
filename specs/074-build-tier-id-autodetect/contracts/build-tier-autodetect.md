# Contract — milestone 074 build-tier auto-detect

This is the milestone's only contract. The change at the CLI level is "no flags, behavior change in the no-flag default for `mikebom trace run`." The change at the library level is "one new public function plus a small refactor extract."

## CLI surface

**No new flags.** Per SC-005:

- `mikebom trace run --help` output MUST contain no new flag entries beyond the milestone-073 set (`--repo`, `--git-ref`, `--image-id`, `--attestation`, `--id`).
- The behavior change is observable only in the no-flag default: invoking `mikebom trace run -- ./build.sh` in a git checkout now produces a build-tier SBOM with auto-detected `repo:` and `git:` identifiers; previously (alpha.16) such an invocation produced a build-tier SBOM with zero identifiers.

**Backward compatibility**: every existing invocation form continues to work. Manual flags continue to override (FR-004). `mikebom trace run` outside a git checkout continues to produce identifier-free build-tier SBOMs.

## Library surface (`mikebom-cli` crate)

### New public function

```rust
// In mikebom-cli/src/binding/identifiers/auto_detect.rs

/// Auto-detect build-tier identifiers from a `mikebom trace run`
/// invocation cwd.
///
/// Returns 0, 1, or 2 identifiers:
/// - `[]` when `invocation_cwd` is not a git checkout, has no remotes,
///   or all subprocess calls fail.
/// - `[repo:<url>]` when a remote is resolvable but `git rev-parse HEAD`
///   fails (e.g., empty repo with no commits).
/// - `[repo:<url>, git:<url>#<sha>]` (in this order) when both a remote
///   URL and a `HEAD` commit are resolvable.
///
/// Identifiers carry `source_label = Some(...)` per FR-006 with build-
/// tier-distinguishable strings per research §4.
///
/// Never panics. Never returns `Result`. All failure modes collapse to
/// "this identifier is omitted" with `tracing::info!` (skipped detection)
/// or `tracing::warn!` (soft-fail to UserDefined per FR-010).
///
/// Determinism: given fixed git remote configuration and fixed `HEAD`
/// commit, repeated calls produce byte-identical output.
pub fn auto_detect_build_tier_identifiers(
    invocation_cwd: &Path,
) -> Vec<Identifier>;
```

### Refactored existing function

```rust
// In mikebom-cli/src/binding/identifiers/auto_detect.rs

/// (UNCHANGED public signature)
///
/// Auto-detect a `repo:` identifier from a git checkout. Source-tier
/// entry point. Internally now calls `discover_repo_url` for the URL
/// discovery half and attaches the source-tier `source_label` string.
pub fn auto_detect_repo_identifier(scan_root: &Path) -> Option<Identifier>;
```

The public signature does not change. Internal implementation is a thin wrapper over the new shared `discover_repo_url` core. Source-tier callers see no behavior change.

### New private helpers (not exported)

```rust
// In mikebom-cli/src/binding/identifiers/auto_detect.rs

/// Shared 3-step git-remote-fallback URL discovery. Returns the
/// `(url, remote_name, fallback_used)` tuple. Each tier's auto-detect
/// wrapper attaches its own `source_label`.
fn discover_repo_url(scan_root: &Path) -> Option<(String, String, bool)>;

/// Shells out `git -C <scan_root> rev-parse HEAD`. Returns `Some(sha)`
/// only on a 40-char lowercase hex result, else `None` with info-log.
fn git_rev_parse_head(scan_root: &Path) -> Option<String>;
```

## Integration boundary

### Where the new function is called

```rust
// In mikebom-cli/src/cli/run.rs::execute(args: RunArgs)

pub async fn execute(args: RunArgs) -> anyhow::Result<()> {
    // STEP 1 (NEW) — capture invocation cwd ONCE, before any other work.
    let invocation_cwd = std::env::current_dir()?;

    // STEP 2 (NEW) — auto-detect build-tier identifiers BEFORE the
    // wrapped command starts. The wrapped command may cd into subdirs
    // or modify the working tree; auto-detection runs against the
    // original invocation state for determinism.
    let auto_detected =
        mikebom::binding::identifiers::auto_detect::auto_detect_build_tier_identifiers(
            &invocation_cwd,
        );

    // STEP 3 (UNCHANGED) — Phase 1: capture the trace → attestation.
    let scan_args = ScanArgs { /* ... */ };
    super::scan::execute(scan_args).await?;

    // STEP 4 (REFACTORED) — assemble identifiers via the shared
    // milestone-073 resolver, passing both auto-detected entries and
    // manual-flag-derived entries. Manual-wins precedence per
    // 073's FR-006 + 074's FR-004.
    let assembled_ids = resolve_identifiers(
        auto_detected,            // <-- new: was always Vec::new() pre-074
        args.repo.as_deref(),
        args.git_ref.as_deref(),
        args.image_id.as_deref(),
        args.attestation.as_deref(),
        &args.id,
    );

    // STEP 5 (UNCHANGED-ish) — emit per-format using assembled_ids.
    /* existing emission flow */
}
```

### Calling order invariants

- `current_dir()` MUST be called before `super::scan::execute(scan_args).await?` so the cwd snapshot reflects the operator's invocation state, not whatever the wrapped command has done by the time we're assembling identifiers.
- `auto_detect_build_tier_identifiers(...)` MUST be called once per invocation. Re-calling it later (after the wrapped command finishes) would be both wasteful and risk capturing changed state.
- Identifier emission per format (CDX / SPDX 2.3 / SPDX 3) MUST consume the post-resolve `assembled_ids` vec, not the raw `auto_detected` vec — otherwise manual flags wouldn't override.

## Observable contract from outside the binary

### What the operator sees

When invoking `mikebom trace run` in a git checkout:

```
$ mikebom trace run -- ./build.sh
INFO build-tier auto-detected `repo:git@github.com:acme/foo.git` from git remote `origin`
INFO build-tier auto-detected `git:git@github.com:acme/foo.git#abc1234...` from `git rev-parse HEAD`
... (rest of trace flow unchanged)
```

When invoking outside a git checkout:

```
$ cd /tmp && mikebom trace run -- ./build.sh
INFO not a git checkout; build-tier identifier auto-detection skipped
... (rest of trace flow unchanged; no identifiers emitted)
```

When invoking with manual override:

```
$ mikebom trace run --repo git@github.com:acme/override.git -- ./build.sh
INFO manual --repo flag overrides build-tier auto-detected `repo:git@github.com:acme/foo.git`
... (emitted SBOM contains `repo:git@github.com:acme/override.git`, not the auto-detected origin)
```

### What appears in the emitted SBOM

The auto-detected identifiers ride milestone-073's already-vetted carriers:

- **CDX 1.6**: `metadata.component.externalReferences[]` with `type` = `"vcs"` for both `repo:` and `git:`. The `comment` field carries the `source_label` for transparency.
- **SPDX 2.3**: dual carrier per 073 — `Package.externalRefs[].referenceCategory = "PERSISTENT-ID"` for the main module + `creationInfo.creators[]` redundant text line for document-level discoverability.
- **SPDX 3.0.1**: `Element.externalIdentifier[]` with `type` = the scheme name (`"repo"` / `"git"`).

No new fields. No new `mikebom:*` annotations. Constitution Principle V's native-precedence audit was satisfied by milestone 073; this milestone consumes the existing carriers unchanged.

## Test contract (extends `mikebom-cli/tests/identifiers_*` patterns)

A new integration-test file `mikebom-cli/tests/identifiers_build_tier_autodetect.rs` MUST cover:

| Test | Acceptance scenario | Validates |
|------|--------------------|-----------|
| `build_tier_autodetect_repo_in_git_checkout` | US1 §1 | FR-001, SC-001 |
| `build_tier_autodetect_upstream_fallback` | US1 §2 | FR-001 fallback |
| `build_tier_skips_outside_git` | US1 §3, US2 §4 | FR-003, SC-006 |
| `build_tier_manual_repo_overrides_autodetect` | US1 §4 | FR-004 |
| `build_tier_autodetect_git_with_full_sha` | US2 §1 | FR-002 (40-char SHA) |
| `build_tier_autodetect_git_in_detached_head` | US2 §2 | FR-002 detached-HEAD edge case |
| `build_tier_skips_git_in_empty_repo` | US2 §3 | FR-003 partial-skip path |
| `build_tier_cross_tier_correlation` | SC-002 | end-to-end source/build identifier byte-equality |
| `build_tier_autodetect_is_deterministic` | SC-007 | FR-009 |
| `build_tier_autodetect_first_listed_fallback` | edge case | parity with source-tier 073's first-listed fallback |

Test fixtures MUST be tempdir-based (no real network, no real cloning). Each test that needs a git remote configures it via `git -C <tempdir> remote add origin <fake-url>`. Each test that needs a commit makes one with `git -C <tempdir> commit --allow-empty -m test` so the SHA is reproducible across runs (with `GIT_AUTHOR_*` / `GIT_COMMITTER_*` pinning if needed for byte-identity).

## Performance contract (per SC-004)

- Two `git` subprocess invocations bound the worst case: one `git remote get-url <name>` (or `git remote` for the fallback path) plus one `git rev-parse HEAD`.
- On the happy path (git checkout with `origin` configured), exactly two `Command::spawn`s.
- On the non-git path, zero `Command::spawn`s — the `.git` existence pre-check short-circuits at the filesystem level.
- Target: <100ms total auto-detection latency on a typical repo. Measured via integration-test benchmark; CI lane time impact <1s on the build-tier test suite.

## Determinism contract (per FR-009)

- Same git config + same `HEAD` commit + same invocation cwd → byte-identical `Vec<Identifier>` output.
- Identifier ordering is fixed: `repo:` at index 0, `git:` at index 1 when both present.
- The post-resolve `assembled_ids` ordering follows 073's contract: auto-detected entries first (in this fixed order), then manual flags in supply order. Output ordering in emitted SBOMs follows each format's existing 073 sort rules.

## Logging contract

| Event | Level | Format |
|-------|-------|--------|
| `repo:` auto-detected | `tracing::info!` | `"build-tier auto-detected `repo:<url>` from git remote `<name>`"` |
| `git:` auto-detected | `tracing::info!` | `"build-tier auto-detected `git:<url>#<sha>` from `git rev-parse HEAD`"` |
| Skipped detection (non-git) | `tracing::info!` | `"not a git checkout; build-tier identifier auto-detection skipped"` |
| Skipped `git:` (empty repo) | `tracing::info!` | `"`git rev-parse HEAD` failed; build-tier `git:` identifier skipped"` |
| Soft-fail (malformed remote) | `tracing::warn!` | `"auto-detected build-tier repo URL failed `repo:` validation; emitting as user-defined under mikebom:identifiers"` |
| Manual flag override | `tracing::info!` | `"manual --repo flag overrides build-tier auto-detected `repo:<url>`"` |
