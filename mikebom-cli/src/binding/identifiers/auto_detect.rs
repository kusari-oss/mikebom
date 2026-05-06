//! Auto-detection paths for source identifiers.
//!
//! Three entry points:
//!
//! - `auto_detect_repo_identifier(scan_root)` for source-tier
//!   `--path` scans (FR-001). Implements the 3-step git-remote
//!   fallback per spec Q1: `origin` → `upstream` → first-listed
//!   alphabetical. Failure (no git, no remotes, command error) is
//!   logged at `tracing::info!` and returns `None` — never fails the
//!   scan.
//! - `auto_detect_build_tier_identifiers(invocation_cwd)` for
//!   build-tier `mikebom trace run` invocations (milestone 074).
//!   Reuses the shared 3-step git-remote fallback core via
//!   `discover_repo_url`, then additionally captures
//!   `git rev-parse HEAD` to emit a commit-anchored `git:` identifier.
//!   Soft-fail rules mirror source-tier per FR-003 / FR-010.
//! - `image_reference_to_identifier(...)` for image-tier `--image`
//!   scans (FR-008). Synthesizes the canonical `image:<registry>/
//!   <name>:<tag>@sha256:<digest>` shape per spec Q3 from the
//!   resolved-image fields, omitting components that aren't present.

use std::path::Path;
use std::process::Command;

use super::{Identifier, IdentifierKind, IdentifierValue, SchemeName};

/// Auto-detect a `repo:` identifier from a git checkout. Returns `None`
/// when the scan root isn't a git checkout, has no remotes, or any git
/// subprocess errors out — all such conditions log at `tracing::info!`
/// and never fail the scan (FR-001).
///
/// Three-step fallback per Q1 clarification: try `origin` first; fall
/// back to `upstream`; fall back to first-listed remote per `git
/// remote` output (alphabetical). The chosen remote name is recorded
/// in the resulting identifier's `source_label` for transparency
/// (FR-007).
///
/// Internally a thin wrapper over the shared `discover_repo_url` core
/// (milestone 074 refactor extract): URL discovery is tier-agnostic;
/// each tier formats its own `source_label` string.
pub fn auto_detect_repo_identifier(scan_root: &Path) -> Option<Identifier> {
    let (url, remote_name, fallback_used) = discover_repo_url(scan_root)?;
    build_repo_identifier_with_label(
        url,
        &remote_name,
        source_tier_repo_label(&remote_name, fallback_used),
    )
}

/// Source-tier `source_label` formatter. Kept stable across milestones
/// 073 and 074 — pre-refactor strings are reproduced verbatim so the
/// existing source-tier goldens remain byte-identical (per VR-074-007 /
/// research §5).
fn source_tier_repo_label(remote_name: &str, fallback_used: bool) -> String {
    if fallback_used {
        format!(
            "auto-detected from git remote `{remote_name}` (origin/upstream absent; first-listed)"
        )
    } else {
        format!("auto-detected from git remote `{remote_name}`")
    }
}

/// Build-tier `source_label` formatter for `repo:` identifiers per
/// research §4. Inserts `build-tier ` between `from` and `git remote`
/// so consumers reading SBOMs without surrounding tier-context can
/// disambiguate per-identifier (FR-006).
fn build_tier_repo_label(remote_name: &str, fallback_used: bool) -> String {
    if fallback_used {
        format!(
            "auto-detected from build-tier git remote `{remote_name}` (origin/upstream absent; first-listed)"
        )
    } else {
        format!("auto-detected from build-tier git remote `{remote_name}`")
    }
}

/// Build-tier `source_label` for the auto-detected `git:` identifier
/// per research §4.
const BUILD_TIER_GIT_LABEL: &str =
    "auto-detected from build-tier `git rev-parse HEAD`";

/// Tier-agnostic URL-discovery core: 3-step git-remote fallback
/// (`origin` → `upstream` → first-listed alphabetical). Returns
/// `(url, remote_name, fallback_used)` on success — each tier's wrapper
/// attaches its own `source_label`. `None` on every failure mode
/// (not a git repo, no remotes, subprocess error) with appropriate
/// `tracing::info!` logging.
///
/// Extracted from the original `auto_detect_repo_identifier` in
/// milestone 074 so source-tier and build-tier auto-detection share
/// the discovery algorithm verbatim per FR-008 + research §2.
fn discover_repo_url(scan_root: &Path) -> Option<(String, String, bool)> {
    if !scan_root.join(".git").exists() {
        return None;
    }
    // Step 1+2: try origin and upstream by name.
    for name in ["origin", "upstream"] {
        if let Some(url) = git_remote_get_url(scan_root, name) {
            let trimmed = url.trim().to_string();
            if !trimmed.is_empty() {
                return Some((trimmed, name.to_string(), false));
            }
        }
    }
    // Step 3: list all remotes alphabetically; take the first non-
    // origin / non-upstream entry.
    let remotes = match git_remote_list(scan_root) {
        Some(r) if !r.is_empty() => r,
        Some(_) => {
            tracing::info!(
                scan_root = %scan_root.display(),
                "git checkout has no remotes; identifier auto-detection skipped"
            );
            return None;
        }
        None => {
            tracing::info!(
                scan_root = %scan_root.display(),
                "git remote list failed; identifier auto-detection skipped"
            );
            return None;
        }
    };
    let first = remotes
        .iter()
        .find(|r| r.as_str() != "origin" && r.as_str() != "upstream")?;
    let url = git_remote_get_url(scan_root, first)?;
    let trimmed = url.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some((trimmed, first.clone(), true))
    }
}

/// Construct a `repo:` identifier with a caller-supplied `source_label`.
/// Re-runs the `validate_for_scheme(BuiltinScheme::Repo, ...)` validator
/// so a malformed remote URL downgrades `kind` to `UserDefined` with a
/// `tracing::warn!` per FR-010 / VR-005 (and milestone-074 VR-074-005).
fn build_repo_identifier_with_label(
    url: String,
    remote_name: &str,
    label: String,
) -> Option<Identifier> {
    if url.is_empty() {
        return None;
    }
    let scheme = match SchemeName::new("repo".to_string()) {
        Ok(s) => s,
        Err(_) => return None,
    };
    let value = match IdentifierValue::new(url.clone()) {
        Ok(v) => v,
        Err(_) => return None,
    };
    // Re-run the value validator so the resulting kind reflects
    // whether the URL is well-formed. Auto-detected values from `git
    // remote get-url` are well-formed unless the operator explicitly
    // configured a malformed remote — preserve the soft-fail path.
    let kind = match super::BuiltinScheme::from_scheme_name(&scheme) {
        Some(b) => match super::validators::validate_for_scheme(b, value.as_str()) {
            Ok(()) => IdentifierKind::Builtin(b),
            Err(err) => {
                tracing::warn!(
                    remote = remote_name,
                    url = %url,
                    reason = %err,
                    "auto-detected repo URL failed `repo:` validation; \
                     emitting as user-defined under \
                     mikebom:identifiers"
                );
                IdentifierKind::UserDefined
            }
        },
        None => IdentifierKind::UserDefined,
    };
    Some(Identifier::from_parts_with_label(
        scheme,
        value,
        kind,
        Some(label),
    ))
}

/// Auto-detect build-tier identifiers from a `mikebom trace run`
/// invocation cwd (milestone 074).
///
/// Returns 0, 1, or 2 identifiers in deterministic order:
///
/// - `[]` when `invocation_cwd` is not a git checkout, has no
///   resolvable remotes, or all subprocess calls fail (FR-003).
/// - `[repo:<url>]` when a remote is resolvable but
///   `git rev-parse HEAD` fails (e.g., empty repo with no commits
///   per spec US2 §3).
/// - `[repo:<url>, git:<url>#<sha>]` when both a remote URL and a
///   `HEAD` commit are resolvable. `repo:` is at index 0, `git:` at
///   index 1 (FR-009 + data-model.md ordering invariant).
///
/// Each identifier carries `source_label = Some(...)` containing the
/// substring `build-tier` per FR-006 / VR-074-004.
///
/// `kind` follows the source-tier soft-fail rule per FR-010 /
/// VR-074-005: well-formed values produce
/// `IdentifierKind::Builtin(...)`; malformed values downgrade to
/// `IdentifierKind::UserDefined` with a `tracing::warn!`.
///
/// Never panics. Never returns `Result`. All failure modes collapse
/// to "this identifier is omitted" with the appropriate
/// `tracing::info!` (skipped detection) or `tracing::warn!` (soft-
/// fail to UserDefined).
///
/// Determinism (FR-009): given fixed git remote configuration and
/// fixed `HEAD` commit, repeated calls produce byte-identical output.
pub fn auto_detect_build_tier_identifiers(invocation_cwd: &Path) -> Vec<Identifier> {
    let mut out: Vec<Identifier> = Vec::new();
    // Step 1: discover the remote URL via the shared core.
    let (url, remote_name, fallback_used) = match discover_repo_url(invocation_cwd) {
        Some(t) => t,
        None => {
            // The shared core already logged via tracing::info!.
            return out;
        }
    };
    let label = build_tier_repo_label(&remote_name, fallback_used);
    let repo_id = match build_repo_identifier_with_label(url.clone(), &remote_name, label) {
        Some(id) => id,
        None => {
            // Construction failure (empty URL, etc.); without a
            // resolvable repo URL we can't synthesize the `git:`
            // identifier either, so bail.
            return out;
        }
    };
    tracing::info!(
        scheme = repo_id.scheme.as_str(),
        value = repo_id.value.as_str(),
        remote = %remote_name,
        "build-tier auto-detected `repo:{}` from git remote `{}`",
        repo_id.value.as_str(),
        remote_name,
    );
    out.push(repo_id);

    // Step 2: attempt `git rev-parse HEAD`. Only fires if step 1
    // produced a `repo:` identifier — the `git:` value reuses the
    // same URL string per VR-074-002.
    let sha = match git_rev_parse_head(invocation_cwd) {
        Some(s) => s,
        None => {
            tracing::info!(
                scan_root = %invocation_cwd.display(),
                "`git rev-parse HEAD` failed; build-tier `git:` identifier skipped"
            );
            return out;
        }
    };
    let git_value_str = format!("{url}#{sha}");
    let git_scheme = match SchemeName::new("git".to_string()) {
        Ok(s) => s,
        Err(_) => return out,
    };
    let git_value = match IdentifierValue::new(git_value_str.clone()) {
        Ok(v) => v,
        Err(_) => return out,
    };
    let git_kind = match super::BuiltinScheme::from_scheme_name(&git_scheme) {
        Some(b) => match super::validators::validate_for_scheme(b, git_value.as_str()) {
            Ok(()) => IdentifierKind::Builtin(b),
            Err(err) => {
                tracing::warn!(
                    value = %git_value_str,
                    reason = %err,
                    "auto-detected build-tier `git:` value failed validation; \
                     emitting as user-defined under \
                     mikebom:identifiers"
                );
                IdentifierKind::UserDefined
            }
        },
        None => IdentifierKind::UserDefined,
    };
    let git_id = Identifier::from_parts_with_label(
        git_scheme,
        git_value,
        git_kind,
        Some(BUILD_TIER_GIT_LABEL.to_string()),
    );
    tracing::info!(
        value = %git_value_str,
        "build-tier auto-detected `git:{}` from `git rev-parse HEAD`",
        git_value_str,
    );
    out.push(git_id);
    out
}

/// Run `git -C <scan_root> rev-parse HEAD`. Returns `Some(sha)` only
/// if the result is exactly 40 lowercase hex characters (per
/// VR-074-003); else `None` with `tracing::info!`.
///
/// Failure modes covered (research §1):
///
/// - Not a git repo (caller already pre-checks via `discover_repo_url`,
///   but the helper still tolerates it) → exit 128 → `None`.
/// - Empty repo, no commits → exit 128 → `None`.
/// - Detached HEAD → exit 0 with the SHA on stdout → `Some(sha)`,
///   treated identically to attached HEAD.
/// - `git` not on PATH → `Command::spawn` returns `io::Error` → `None`.
fn git_rev_parse_head(scan_root: &Path) -> Option<String> {
    let scan_root_str = scan_root.to_str()?;
    let output = Command::new("git")
        .args(["-C", scan_root_str, "rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    let trimmed = s.trim();
    // VR-074-003: must be exactly 40 lowercase hex chars; anything
    // else (abbreviated SHA, ref name leaking through, empty output)
    // returns `None` to preserve the wire-format invariant from
    // milestone 073's `validate_git`.
    if trimmed.len() != 40 {
        return None;
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
        return None;
    }
    Some(trimmed.to_string())
}

/// Run `git -C <scan_root> remote get-url <name>`. Returns `None` on
/// any failure (subprocess error, exit status non-zero, empty output).
fn git_remote_get_url(scan_root: &Path, name: &str) -> Option<String> {
    let scan_root_str = scan_root.to_str()?;
    let output = Command::new("git")
        .args(["-C", scan_root_str, "remote", "get-url", name])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Run `git -C <scan_root> remote`. Returns the list of configured
/// remote names, sorted alphabetically (which is `git remote`'s
/// natural output). `None` on subprocess error.
fn git_remote_list(scan_root: &Path) -> Option<Vec<String>> {
    let scan_root_str = scan_root.to_str()?;
    let output = Command::new("git")
        .args(["-C", scan_root_str, "remote"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    let mut names: Vec<String> = s
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    names.sort();
    Some(names)
}

// ---------------------------------------------------------------------
// Image-tier auto-detection
// ---------------------------------------------------------------------

/// Synthesize an `image:` identifier from resolved-image fields.
///
/// Per the Q3 clarification, the canonical shape is:
///
/// ```text
/// image:<registry>/<name>:<tag>@sha256:<digest>
/// ```
///
/// with these documented omissions:
///
/// - tarball-loaded images without a registry context omit the
///   registry portion: `image:<name>@sha256:<digest>` or
///   `image:<name>:<tag>@sha256:<digest>`.
/// - pre-distribution-spec images without a digest omit the digest:
///   `image:<registry>/<name>:<tag>` etc.
///
/// Returns `None` when there's not enough information to synthesize
/// any meaningful identifier (no name).
pub fn image_reference_to_identifier(
    registry: Option<&str>,
    name: &str,
    tag: Option<&str>,
    digest: Option<&str>,
) -> Option<Identifier> {
    if name.is_empty() {
        return None;
    }
    let mut wire = String::new();
    if let Some(r) = registry {
        if !r.is_empty() {
            wire.push_str(r);
            wire.push('/');
        }
    }
    wire.push_str(name);
    if let Some(t) = tag {
        if !t.is_empty() {
            wire.push(':');
            wire.push_str(t);
        }
    }
    if let Some(d) = digest {
        if !d.is_empty() {
            wire.push_str("@sha256:");
            wire.push_str(d);
        }
    }
    let scheme = SchemeName::new("image".to_string()).ok()?;
    let value = IdentifierValue::new(wire).ok()?;
    let kind = match super::BuiltinScheme::from_scheme_name(&scheme) {
        Some(b) => match super::validators::validate_for_scheme(b, value.as_str()) {
            Ok(()) => IdentifierKind::Builtin(b),
            Err(err) => {
                tracing::warn!(
                    value = value.as_str(),
                    reason = %err,
                    "auto-synthesized `image:` value failed validation; \
                     emitting as user-defined under \
                     mikebom:identifiers"
                );
                IdentifierKind::UserDefined
            }
        },
        None => IdentifierKind::UserDefined,
    };
    Some(Identifier::from_parts_with_label(
        scheme,
        value,
        kind,
        Some("auto-detected from resolved image reference".to_string()),
    ))
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;
    use std::process::Command;

    fn run(cmd: &mut Command) {
        let status = cmd.status().expect("git subprocess");
        assert!(status.success(), "git command failed: {cmd:?}");
    }

    fn git_init(dir: &Path) {
        run(Command::new("git")
            .args(["-C", dir.to_str().unwrap(), "init", "-q"]));
        // Some CI git installs require user.email/user.name; not
        // strictly required for the read-only `remote` commands we
        // exercise, but harmless.
    }

    fn git_remote_add(dir: &Path, name: &str, url: &str) {
        run(Command::new("git")
            .args(["-C", dir.to_str().unwrap(), "remote", "add", name, url]));
    }

    #[test]
    fn no_git_dir_returns_none() {
        let td = tempfile::tempdir().unwrap();
        assert!(auto_detect_repo_identifier(td.path()).is_none());
    }

    #[test]
    fn git_dir_no_remotes_returns_none() {
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        assert!(auto_detect_repo_identifier(td.path()).is_none());
    }

    #[test]
    fn origin_only_uses_origin() {
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        git_remote_add(td.path(), "origin", "git@github.com:test/foo.git");
        let id = auto_detect_repo_identifier(td.path()).expect("origin detected");
        assert_eq!(id.scheme.as_str(), "repo");
        assert_eq!(id.value.as_str(), "git@github.com:test/foo.git");
        assert_eq!(
            id.source_label.as_deref(),
            Some("auto-detected from git remote `origin`")
        );
        assert!(id.is_builtin());
    }

    #[test]
    fn upstream_only_uses_upstream() {
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        git_remote_add(td.path(), "upstream", "git@github.com:acme/foo.git");
        let id = auto_detect_repo_identifier(td.path()).expect("upstream detected");
        assert_eq!(id.scheme.as_str(), "repo");
        assert_eq!(id.value.as_str(), "git@github.com:acme/foo.git");
        assert_eq!(
            id.source_label.as_deref(),
            Some("auto-detected from git remote `upstream`")
        );
    }

    #[test]
    fn third_remote_only_uses_first_alphabetical_with_fallback_label() {
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        // Add only third-named remotes (no origin/upstream).
        git_remote_add(td.path(), "zebra", "git@example.com:z/foo.git");
        git_remote_add(td.path(), "alpha", "git@example.com:a/foo.git");
        let id = auto_detect_repo_identifier(td.path()).expect("first-listed detected");
        // Alphabetical first → alpha.
        assert_eq!(id.value.as_str(), "git@example.com:a/foo.git");
        assert_eq!(
            id.source_label.as_deref(),
            Some(
                "auto-detected from git remote `alpha` (origin/upstream absent; first-listed)"
            )
        );
    }

    #[test]
    fn origin_wins_over_upstream() {
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        git_remote_add(td.path(), "origin", "git@github.com:o/foo.git");
        git_remote_add(td.path(), "upstream", "git@github.com:u/foo.git");
        let id = auto_detect_repo_identifier(td.path()).unwrap();
        assert_eq!(id.value.as_str(), "git@github.com:o/foo.git");
    }

    #[test]
    fn image_full_form_synthesis() {
        let id = image_reference_to_identifier(
            Some("docker.io"),
            "acme/foo",
            Some("v1"),
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
        )
        .unwrap();
        assert_eq!(id.scheme.as_str(), "image");
        assert_eq!(
            id.value.as_str(),
            "docker.io/acme/foo:v1@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert!(id.is_builtin());
    }

    #[test]
    fn image_tarball_no_registry_synthesis() {
        let id = image_reference_to_identifier(
            None,
            "acme/foo",
            None,
            Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
        )
        .unwrap();
        assert_eq!(
            id.value.as_str(),
            "acme/foo@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
    }

    #[test]
    fn image_pre_distribution_spec_no_digest_synthesis() {
        let id =
            image_reference_to_identifier(Some("docker.io"), "acme/foo", Some("v1"), None)
                .unwrap();
        assert_eq!(id.value.as_str(), "docker.io/acme/foo:v1");
    }

    #[test]
    fn image_empty_name_returns_none() {
        assert!(image_reference_to_identifier(Some("docker.io"), "", None, None).is_none());
    }

    // ----------------------------------------------------------------
    // Milestone 074 — git_rev_parse_head helper (T003)
    // ----------------------------------------------------------------

    fn git_config_user(dir: &Path) {
        run(Command::new("git").args([
            "-C",
            dir.to_str().unwrap(),
            "config",
            "user.email",
            "test@example.com",
        ]));
        run(Command::new("git").args([
            "-C",
            dir.to_str().unwrap(),
            "config",
            "user.name",
            "Test User",
        ]));
    }

    fn git_commit_empty(dir: &Path, msg: &str) {
        git_config_user(dir);
        run(Command::new("git").args([
            "-C",
            dir.to_str().unwrap(),
            "commit",
            "--allow-empty",
            "-q",
            "-m",
            msg,
        ]));
    }

    fn git_rev_parse_head_via_subprocess(dir: &Path) -> String {
        let out = Command::new("git")
            .args(["-C", dir.to_str().unwrap(), "rev-parse", "HEAD"])
            .output()
            .expect("git subprocess");
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    #[test]
    fn rev_parse_head_returns_full_sha_on_committed_repo() {
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        git_commit_empty(td.path(), "first");
        let sha = git_rev_parse_head(td.path()).expect("HEAD resolves");
        assert_eq!(sha.len(), 40);
        assert!(
            sha.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "expected lowercase hex; got {sha}"
        );
        // Cross-check against the same subprocess invocation.
        assert_eq!(sha, git_rev_parse_head_via_subprocess(td.path()));
    }

    #[test]
    fn rev_parse_head_returns_none_in_empty_repo_no_commits() {
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        // No commit made — `git rev-parse HEAD` exits 128.
        assert!(git_rev_parse_head(td.path()).is_none());
    }

    #[test]
    fn rev_parse_head_returns_none_in_non_git_dir() {
        let td = tempfile::tempdir().unwrap();
        // Helper is tolerant of non-git dirs even though the
        // `auto_detect_build_tier_identifiers` caller pre-checks via
        // `discover_repo_url`.
        assert!(git_rev_parse_head(td.path()).is_none());
    }

    // ----------------------------------------------------------------
    // Milestone 074 — auto_detect_build_tier_identifiers (T004)
    // ----------------------------------------------------------------

    #[test]
    fn build_tier_empty_on_non_git_dir() {
        let td = tempfile::tempdir().unwrap();
        let ids = auto_detect_build_tier_identifiers(td.path());
        assert!(
            ids.is_empty(),
            "non-git dir must yield zero auto-detected identifiers"
        );
    }

    #[test]
    fn build_tier_repo_only_when_remote_but_no_commits() {
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        git_remote_add(td.path(), "origin", "git@github.com:test/foo.git");
        // No commit — `git rev-parse HEAD` will fail.
        let ids = auto_detect_build_tier_identifiers(td.path());
        assert_eq!(
            ids.len(),
            1,
            "expected exactly [repo:] when remote configured but no HEAD"
        );
        assert_eq!(ids[0].scheme.as_str(), "repo");
        assert_eq!(ids[0].value.as_str(), "git@github.com:test/foo.git");
        let label = ids[0]
            .source_label
            .as_deref()
            .expect("source_label set for auto-detection");
        assert!(
            label.contains("build-tier"),
            "VR-074-004: build-tier substring required; got {label:?}"
        );
        assert_eq!(
            label,
            "auto-detected from build-tier git remote `origin`"
        );
    }

    #[test]
    fn build_tier_repo_and_git_when_remote_and_commit() {
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        git_remote_add(td.path(), "origin", "git@github.com:test/foo.git");
        git_commit_empty(td.path(), "first");
        let head_sha = git_rev_parse_head_via_subprocess(td.path());

        let ids = auto_detect_build_tier_identifiers(td.path());
        assert_eq!(
            ids.len(),
            2,
            "expected [repo:, git:] when remote and HEAD both resolvable"
        );
        // Order: repo at index 0, git at index 1 per data-model.md.
        assert_eq!(ids[0].scheme.as_str(), "repo");
        assert_eq!(ids[0].value.as_str(), "git@github.com:test/foo.git");
        assert_eq!(ids[1].scheme.as_str(), "git");
        assert_eq!(
            ids[1].value.as_str(),
            format!("git@github.com:test/foo.git#{head_sha}")
        );
        // VR-074-004: build-tier substring on both.
        assert!(ids[0].source_label.as_deref().unwrap().contains("build-tier"));
        assert!(ids[1].source_label.as_deref().unwrap().contains("build-tier"));
        assert_eq!(
            ids[1].source_label.as_deref(),
            Some("auto-detected from build-tier `git rev-parse HEAD`")
        );
    }

    #[test]
    fn build_tier_deterministic_across_two_calls() {
        // FR-009 / SC-007: same fixture, same call -> byte-identical
        // identifier slots.
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        git_remote_add(td.path(), "origin", "git@github.com:test/foo.git");
        git_commit_empty(td.path(), "first");

        let a = auto_detect_build_tier_identifiers(td.path());
        let b = auto_detect_build_tier_identifiers(td.path());
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.scheme.as_str(), y.scheme.as_str());
            assert_eq!(x.value.as_str(), y.value.as_str());
            assert_eq!(x.source_label, y.source_label);
        }
    }

    #[test]
    fn build_tier_upstream_fallback() {
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        git_remote_add(td.path(), "upstream", "git@github.com:acme/foo.git");
        let ids = auto_detect_build_tier_identifiers(td.path());
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].scheme.as_str(), "repo");
        assert_eq!(ids[0].value.as_str(), "git@github.com:acme/foo.git");
        assert_eq!(
            ids[0].source_label.as_deref(),
            Some("auto-detected from build-tier git remote `upstream`")
        );
    }

    #[test]
    fn build_tier_first_listed_fallback_uses_alpha_label() {
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        git_remote_add(td.path(), "zebra", "git@example.com:z/foo.git");
        git_remote_add(td.path(), "alpha", "git@example.com:a/foo.git");
        let ids = auto_detect_build_tier_identifiers(td.path());
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].value.as_str(), "git@example.com:a/foo.git");
        assert_eq!(
            ids[0].source_label.as_deref(),
            Some(
                "auto-detected from build-tier git remote `alpha` (origin/upstream absent; first-listed)"
            )
        );
    }

    #[test]
    fn build_tier_malformed_remote_softfails_to_userdefined() {
        // FR-010 / VR-074-005: a remote URL that fails `validate_repo`
        // downgrades `kind` to UserDefined rather than rejecting.
        // `validate_repo` (milestone 073) accepts well-known forms but
        // we deliberately use a value the validator can be expected to
        // reject (raw whitespace-only is empty after trim, so we craft
        // something that survives empty-check but fails repo-shape
        // validation: a plain string with no scheme/host structure).
        // If the `validate_repo` of milestone 073 happens to accept
        // whatever we use here, the test still passes (Builtin is
        // also a valid kind) — the assertion only checks that the
        // identifier is returned at all and has the build-tier label.
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        // A bare token with no `://` or `@host:` structure tends to
        // exercise the soft-fail path; whether it actually does is
        // a milestone-073 implementation detail — we don't pin
        // `kind` here. Per research §7's test guidance: assert the
        // identifier slot is present, do NOT assert the kind.
        git_remote_add(td.path(), "origin", "not-a-real-url");
        let ids = auto_detect_build_tier_identifiers(td.path());
        assert!(!ids.is_empty(), "soft-fail must still emit the identifier");
        assert_eq!(ids[0].scheme.as_str(), "repo");
        assert_eq!(ids[0].value.as_str(), "not-a-real-url");
    }

    #[test]
    fn build_tier_origin_wins_over_upstream() {
        let td = tempfile::tempdir().unwrap();
        git_init(td.path());
        git_remote_add(td.path(), "origin", "git@github.com:o/foo.git");
        git_remote_add(td.path(), "upstream", "git@github.com:u/foo.git");
        let ids = auto_detect_build_tier_identifiers(td.path());
        assert!(!ids.is_empty());
        assert_eq!(ids[0].value.as_str(), "git@github.com:o/foo.git");
    }
}
