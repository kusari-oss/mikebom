# Data Model — milestone 074 build-tier identifier auto-detection

The milestone introduces zero new types in `mikebom-cli`. Every value flowing through the new code path is a composition of existing milestone-073 types. Constitution Principle IV is satisfied transitively: 073's `Identifier`, `SchemeName`, `IdentifierValue`, `BuiltinScheme`, `IdentifierKind` are all newtypes/enums, and the new functions return `Option<Identifier>` / `Vec<Identifier>` over those existing types.

## Entities

### `BuildTierAutoDetectionResult` (conceptual; not a struct)

The set of identifiers auto-detected for a single `mikebom trace run` invocation. Materialized in code as `Vec<Identifier>` returned from the new `auto_detect_build_tier_identifiers` function.

**Composition**:

- Zero or one auto-detected `repo:` identifier (per FR-001). Present if and only if the invocation cwd is a git checkout AND has at least one resolvable remote URL.
- Zero or one auto-detected `git:<repo-url>#<sha>` identifier (per FR-002). Present if and only if (a) `repo:` is present (because `git:` reuses its URL), AND (b) `git rev-parse HEAD` succeeds with a 40-char SHA.

**Invariants**:

- Order: when both are present, `repo:` is at index 0 and `git:` at index 1. Source-tier 073 produces `repo:` only; this ordering keeps source-tier's first slot stable when readers consume both tiers.
- Determinism: given fixed git remote configuration and fixed `HEAD` commit at invocation time, repeated calls produce byte-identical `Vec<Identifier>` content (per FR-009).
- Each `Identifier` carries `source_label = Some(...)` per FR-006, with the per-tier strings pinned in research §4.
- `kind` follows 073's soft-fail rule: well-formed remotes produce `Builtin(BuiltinScheme::Repo)` or `Builtin(BuiltinScheme::Git)`; malformed values downgrade to `UserDefined` with a `tracing::warn!` (per FR-010).

### `InvocationCwd` (conceptual; not a struct)

The directory `mikebom trace run` was invoked from. Materialized in code as `PathBuf` from `std::env::current_dir()` at the top of `run.rs::execute`.

**Captured ONCE**, before any subprocess work or trace capture. Wrapped-command later cwd changes have no effect (per spec edge-case bullet "Build cwd vs wrapped-command cwd").

**Failure mode**: `std::env::current_dir()` returns `io::Error` only if the cwd was unlinked under the running process — extremely rare. Propagated via `?` since `RunArgs::execute` already returns `anyhow::Result<()>`.

## Functions (public surface added by this milestone)

### `auto_detect_build_tier_identifiers`

```rust
pub fn auto_detect_build_tier_identifiers(invocation_cwd: &Path) -> Vec<Identifier>;
```

**Behavior**:

1. Try to discover a `repo:` URL via the shared `discover_repo_url` core (research §2). On success, construct an `Identifier` with build-tier `source_label` (research §4) and push to the result vec.
2. If step 1 produced a `repo:` identifier, attempt `git_rev_parse_head(invocation_cwd)`. On success (40-char hex SHA), construct a `git:<repo-url>#<sha>` identifier with build-tier `source_label` and push.
3. Return the vec. Length is 0, 1, or 2.

**Error model**: never panics, never returns a `Result`. All failure modes (no git, no remotes, no commits, `git` not on PATH, malformed values) collapse to "this identifier is omitted" with the appropriate `tracing::info!` or `tracing::warn!` per FR-003 / FR-010.

**Test surface** (in `mikebom-cli/tests/identifiers_build_tier_autodetect.rs`):

- Empty result on a non-git tempdir.
- Exactly `[repo:]` on a git tempdir with a remote but no commits.
- Exactly `[repo:, git:]` on a git tempdir with a remote and a commit.
- Exactly `[repo:, git:]` with the commit-anchored SHA matching `git rev-parse HEAD` byte-for-byte.
- Exactly `[repo:, git:]` in detached-HEAD state.
- Origin / upstream / first-listed fallback (parallel to 073's source-tier tests).

### `discover_repo_url` (private — refactor extract)

```rust
fn discover_repo_url(scan_root: &Path) -> Option<(String, String, bool)>;
//                                                  url      remote   fallback_used
```

**Behavior**: same 3-step fallback algorithm as the existing `auto_detect_repo_identifier` URL-discovery half — extracted verbatim per research §2. Returns the raw `(url, remote_name, fallback_used)` tuple so each tier's wrapper attaches its own `source_label`.

**Used by**:

- The refactored `auto_detect_repo_identifier` (source-tier entry point).
- The new `auto_detect_build_tier_identifiers` (build-tier entry point).

### `git_rev_parse_head` (private — new helper)

```rust
fn git_rev_parse_head(scan_root: &Path) -> Option<String>;
```

**Behavior**: shells out `git -C <scan_root> rev-parse HEAD`, returns `Some(sha)` if the result is a 40-char lowercase hex string, else `None` with `tracing::info!` per research §1.

**Failure modes**: pre-flight `.git` existence check is the caller's responsibility (already done via the `discover_repo_url` short-circuit). Empty repo (no commits) → `git` exits non-zero → return `None`. Detached HEAD → `git` exits zero with the SHA → return `Some(sha)`. SHA validation: bytes must be exactly 40 lowercase hex; otherwise treat as malformed and return `None`.

## Existing types (touched by reference, not modified)

The following milestone-073 types are consumed by the new code but unchanged:

- `Identifier` — return type of all new flows.
- `SchemeName` — used to construct `repo` and `git` scheme names via `SchemeName::new(...)`.
- `IdentifierValue` — used to wrap the URL and the `<url>#<sha>` strings.
- `IdentifierKind` — `Builtin(BuiltinScheme::Repo)` or `Builtin(BuiltinScheme::Git)` on happy paths, downgrading to `UserDefined` per FR-010 on malformed values.
- `BuiltinScheme::Repo`, `BuiltinScheme::Git` — variants used in classification.
- `validators::validate_for_scheme` — re-runs the value validator to catch malformed remotes.
- `Identifier::from_parts_with_label(scheme, value, kind, source_label)` — constructor used to attach the build-tier `source_label`.

## Relationships

```text
mikebom trace run
    │
    ├── std::env::current_dir() ──> InvocationCwd: PathBuf
    │
    ├── auto_detect_build_tier_identifiers(invocation_cwd)
    │         │
    │         ├── discover_repo_url(invocation_cwd)
    │         │       └─> Option<(url, remote_name, fallback_used)>
    │         │             ├─> validators::validate_for_scheme(BuiltinScheme::Repo, &url)
    │         │             └─> Identifier::from_parts_with_label(repo, url, kind, label)
    │         │
    │         └── git_rev_parse_head(invocation_cwd)
    │                 └─> Option<sha>
    │                       └─> Identifier::from_parts_with_label(git, "<url>#<sha>", kind, label)
    │
    └── resolve_identifiers(auto_detected_vec, manual_repo, manual_git_ref, manual_image,
                            manual_attestation, manual_id_flags)  // milestone-073 helper
              └─> Vec<Identifier>  // dedup + manual-wins precedence applied
                    └─> threaded into ScanArtifacts.identifiers and emitted per format
                          per milestone 073's existing per-format carriers
```

## Validation rules (extends milestone-073's set)

- **VR-074-001**: `auto_detect_build_tier_identifiers` MUST return at most ONE `repo:` and at most ONE `git:` identifier per invocation. Multiple matches at the discovery layer would violate the determinism contract (FR-009).
- **VR-074-002**: When `discover_repo_url` returns `None`, the resulting vec MUST NOT contain a `git:` identifier — `git:` requires a `<repo-url>` and reuses the same URL `repo:` would have used.
- **VR-074-003**: `git_rev_parse_head` MUST validate that its output is exactly 40 lowercase hex characters before returning `Some(sha)`. Anything else (e.g., abbreviated SHA, a ref name leaking through) returns `None` to preserve the wire-format invariant from 073's `git:` value validator.
- **VR-074-004**: The `source_label` on auto-detected build-tier identifiers MUST contain the substring `build-tier` (per research §4). Tested by the integration-test suite.
- **VR-074-005**: Before constructing each `Identifier`, `auto_detect_build_tier_identifiers` MUST call `validators::validate_for_scheme(builtin_scheme, &value)` and downgrade `kind` to `IdentifierKind::UserDefined` (with `tracing::warn!`) on validation failure. This implements FR-010's soft-fail-to-`UserDefined` rule for both the `repo:` and `git:` identifiers — including the documented case where 073's `validate_git` may reject SSH-form URLs (research §7). Mirrors the source-tier path at `auto_detect.rs:88-104`.

## Backward compatibility

- No new `Cargo.toml` deps; no MSRV change; no nightly required.
- Existing source-tier and image-tier auto-detection paths unchanged at the API boundary. The internal `discover_repo_url` extraction is a private refactor invisible to source-tier callers — `auto_detect_repo_identifier` keeps its existing public signature.
- Build-tier scans in non-git directories produce byte-identical SBOMs to alpha.16 (per FR-007).
- Build-tier scans in git-tracked fixtures gain one additive `repo:` slot and one additive `git:` slot per format. This is the expected golden regen, scoped per research §5.
- Existing manual identifier flags (`--repo`, `--git-ref`, `--image-id`, `--attestation`, `--id`) keep their precedence and shape (FR-004 + 073's FR-006 manual-wins).
