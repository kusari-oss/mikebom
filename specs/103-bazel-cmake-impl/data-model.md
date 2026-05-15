# Data Model — milestone 103

Per-file shape of every deliverable.

## File inventory

| File | State | Owner FRs |
|------|-------|-----------|
| `mikebom-cli/src/scan_fs/package_db/bazel.rs` | MODIFY (stub → real) | FR-001, FR-002, FR-003, FR-004 |
| `mikebom-cli/src/scan_fs/package_db/cmake.rs` | MODIFY (stub → real) | FR-005, FR-006, FR-007, FR-008, FR-009 |
| `mikebom-cli/tests/scan_bazel.rs` | NEW | US1 integration test |
| `mikebom-cli/tests/scan_cmake.rs` | NEW | US2 integration test |
| `mikebom-cli/tests/scan_cmake_vendored.rs` | NEW | US3 vendored opt-in test |
| `mikebom-cli/tests/scan_cmake_findpackage_negative.rs` | NEW | FR-007 negative-emission test |
| `mikebom-cli/tests/cdx_regression.rs` | MODIFY | +2 ecosystem test fns (bazel, cmake) |
| `mikebom-cli/tests/spdx_regression.rs` | MODIFY | +2 |
| `mikebom-cli/tests/spdx3_regression.rs` | MODIFY | +2 |
| `mikebom-cli/tests/fixtures/bazel/{MODULE.bazel,WORKSPACE.bazel}` | NEW (2 files) | Bazel fixture |
| `mikebom-cli/tests/fixtures/cmake/{CMakeLists.txt,cmake/third_party.cmake,third_party/foo/version.txt}` | NEW (3 files) | CMake fixture |
| `mikebom-cli/tests/fixtures/golden/{cyclonedx,spdx-2.3,spdx-3}/{bazel,cmake}.*` | NEW (6 files) | Byte-identity goldens |
| `docs/user-guide/cli-reference.md` | MODIFY | `--include-vendored` flag docs per FR-014 |

Total: 2 modified production files + 4 new integration tests + 3 modified test-suite files + 5 new fixture files + 6 new goldens + 1 modified doc = matches SC-007 envelope.

## `bazel.rs` — MODIFY (stub → real)

PR-A signature kept unchanged: `pub fn read(scan_root: &Path) -> Vec<PackageDbEntry>`.

Top-level structure:

```rust
//! Bazel source-tree reader (milestone 102 US1 / milestone 103 implementation).
//! Parses MODULE.bazel (Bzlmod) + WORKSPACE.bazel (legacy http_archive
//! / http_file / git_repository). Per spec FR-001..FR-004.
//!
//! Cross-platform (no `#[cfg(unix)]`). Zero new Cargo deps — uses
//! workspace `regex` + std. Parse errors emit `tracing::warn!` and
//! return zero components per FR-013.

use std::path::Path;
use mikebom_common::resolution::LifecycleScope;
use mikebom_common::types::purl::{encode_purl_segment, Purl};
use regex::Regex;
use super::PackageDbEntry;

pub fn read(scan_root: &Path) -> Vec<PackageDbEntry> {
    let mut entries = Vec::new();

    // MODULE.bazel (Bzlmod, Bazel 6+) — preferred per FR-002.
    let module_path = scan_root.join("MODULE.bazel");
    if module_path.is_file() {
        entries.extend(parse_module_bazel(&module_path));
    }

    // WORKSPACE.bazel + WORKSPACE (legacy) per FR-003.
    for ws_name in &["WORKSPACE.bazel", "WORKSPACE"] {
        let ws_path = scan_root.join(ws_name);
        if ws_path.is_file() {
            entries.extend(parse_workspace_bazel(&ws_path));
            break; // Bazel only loads one; prefer .bazel suffix
        }
    }

    // MODULE.bazel wins on name conflicts per Edge Cases + research §1.
    dedup_module_wins(entries)
}

fn parse_module_bazel(path: &Path) -> Vec<PackageDbEntry> {
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e,
                "failed to read MODULE.bazel (FR-013)");
            return Vec::new();
        }
    };
    let re = match Regex::new(
        r#"(?ms)bazel_dep\s*\(\s*name\s*=\s*"([^"]+)"\s*,\s*version\s*=\s*"([^"]+)"(?:\s*,\s*dev_dependency\s*=\s*(True|False))?\s*\)"#,
    ) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let source_path = path.to_string_lossy().to_string();
    re.captures_iter(&content)
        .filter_map(|c| {
            let name = c.get(1)?.as_str();
            let version = c.get(2)?.as_str();
            let dev = c.get(3).map(|m| m.as_str() == "True").unwrap_or(false);
            build_bazel_entry(name, version, &source_path, None, None, dev, false)
        })
        .collect()
}

fn parse_workspace_bazel(path: &Path) -> Vec<PackageDbEntry> {
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { tracing::warn!(path = %path.display(), error = %e,
            "failed to read WORKSPACE.bazel (FR-013)"); return Vec::new(); }
    };
    let source_path = path.to_string_lossy().to_string();
    let mut entries = Vec::new();

    // http_archive + http_file (research §2)
    let http_re = Regex::new(
        r#"(?ms)(http_archive|http_file)\s*\(\s*name\s*=\s*"([^"]+)"\s*,(.*?)\)"#
    ).ok();
    let urls_re = Regex::new(r#"urls\s*=\s*\[\s*"([^"]+)""#).ok();
    let url_re = Regex::new(r#"\burl\s*=\s*"([^"]+)""#).ok();
    let sha_re = Regex::new(r#"sha256\s*=\s*"([0-9a-fA-F]+)""#).ok();

    if let (Some(http), Some(urls), Some(url), Some(sha)) =
        (http_re, urls_re, url_re, sha_re)
    {
        for c in http.captures_iter(&content) {
            let name = c.get(2).map(|m| m.as_str()).unwrap_or("");
            let body = c.get(3).map(|m| m.as_str()).unwrap_or("");
            let archive_url = urls.captures(body)
                .or_else(|| url.captures(body))
                .and_then(|m| m.get(1))
                .map(|m| m.as_str().to_string());
            let sha256 = sha.captures(body).and_then(|m| m.get(1))
                .map(|m| m.as_str().to_string());
            let version = archive_url.as_deref()
                .and_then(parse_version_from_url)
                .unwrap_or_else(|| "unknown".to_string());
            if let Some(entry) = build_bazel_entry(
                name, &version, &source_path,
                archive_url.as_deref(), sha256.as_deref(),
                false, false,
            ) {
                entries.push(entry);
            }
        }
    }

    // git_repository (research §2)
    let git_re = Regex::new(
        r#"(?ms)git_repository\s*\(\s*name\s*=\s*"([^"]+)"\s*,(.*?)\)"#
    ).ok();
    let remote_re = Regex::new(r#"remote\s*=\s*"([^"]+)""#).ok();
    let commit_re = Regex::new(r#"commit\s*=\s*"([^"]+)""#).ok();
    let tag_re = Regex::new(r#"tag\s*=\s*"([^"]+)""#).ok();
    if let (Some(git), Some(remote), Some(commit), Some(tag)) =
        (git_re, remote_re, commit_re, tag_re)
    {
        for c in git.captures_iter(&content) {
            let name = c.get(1).map(|m| m.as_str()).unwrap_or("");
            let body = c.get(2).map(|m| m.as_str()).unwrap_or("");
            let remote_url = remote.captures(body).and_then(|m| m.get(1))
                .map(|m| m.as_str().to_string());
            let version_full = commit.captures(body)
                .or_else(|| tag.captures(body))
                .and_then(|m| m.get(1))
                .map(|m| m.as_str().to_string());
            // Short-SHA truncation per research §8: first 7 chars of commit.
            let version = version_full.as_deref()
                .map(|v| if v.len() >= 40 { v[..7].to_string() } else { v.to_string() })
                .unwrap_or_else(|| "unknown".to_string());
            if let Some(entry) = build_bazel_entry(
                name, &version, &source_path,
                remote_url.as_deref(), None,
                false, false,
            ) {
                entries.push(entry);
            }
        }
    }

    entries
}

fn build_bazel_entry(
    name: &str,
    version: &str,
    source_path: &str,
    download_url: Option<&str>,
    sha256: Option<&str>,
    dev_dependency: bool,
    _placeholder: bool,
) -> Option<PackageDbEntry> {
    let purl = Purl::new(&format!(
        "pkg:bazel/{}@{}",
        encode_purl_segment(name),
        encode_purl_segment(version),
    )).ok()?;
    let mut extra_annotations = std::collections::BTreeMap::new();
    if let Some(url) = download_url {
        extra_annotations.insert("mikebom:download-url".to_string(),
            serde_json::json!(url));
        extra_annotations.insert("mikebom:bazel-archive-name".to_string(),
            serde_json::json!(name));
    }
    let hashes = if let Some(s) = sha256 {
        vec![/* ContentHash { algorithm: "SHA-256", value: s.to_string() } */
             // The exact struct shape matches existing readers like gem.rs
             // (verified in PR-A planning); copy that pattern verbatim.
        ]
    } else {
        Vec::new()
    };
    let lifecycle_scope = if dev_dependency {
        Some(LifecycleScope::Development)
    } else {
        None
    };
    Some(PackageDbEntry {
        purl, name: name.to_string(), version: version.to_string(),
        arch: None, source_path: source_path.to_string(),
        depends: Vec::new(), maintainer: None,
        licenses: Vec::new(), lifecycle_scope,
        requirement_range: None, source_type: None,
        buildinfo_status: None, evidence_kind: None,
        binary_class: None, binary_stripped: None, linkage_kind: None,
        detected_go: None, confidence: None, binary_packed: None,
        raw_version: None, parent_purl: None, npm_role: None,
        co_owned_by: None, hashes,
        sbom_tier: Some("source".to_string()), shade_relocation: None,
        extra_annotations,
    })
}

fn parse_version_from_url(url: &str) -> Option<String> {
    // Match `<name>-<version>.tar.gz` or `<name>_<version>.zip` etc.
    let re = Regex::new(r"[-_]([0-9]+\.[0-9]+(?:\.[0-9]+)?)").ok()?;
    re.captures(url)?.get(1).map(|m| m.as_str().to_string())
}

fn dedup_module_wins(entries: Vec<PackageDbEntry>) -> Vec<PackageDbEntry> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    // Preserve MODULE.bazel order first (assumes module entries come first per
    // the parsing order in `read`).
    for entry in entries {
        let key = entry.name.clone();
        if seen.insert(key) {
            out.push(entry);
        }
    }
    out
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;
    // 5 unit tests covering MODULE.bazel single-dep, dev_dependency,
    // WORKSPACE http_archive + http_file + git_repository, dedup
    // module-wins, malformed-file skip-with-warn.
}
```

**Total**: ~280-300 lines including doc-comments + tests.

**Implementation notes** for the implementer:
- `ContentHash` struct: copy the exact construction from `gem.rs` or `cargo.rs` (the inline comment placeholder above marks where it lands; verified at implementation time).
- The `_placeholder` param on `build_bazel_entry` exists as documentation that future fields (e.g., `mikebom:bazel-rule-type`) could be added here; remove or use as needed.

## `cmake.rs` — MODIFY (stub → real)

PR-A signature kept unchanged: `pub fn read(scan_root: &Path, include_vendored: bool) -> Vec<PackageDbEntry>`.

Top-level structure mirrors bazel.rs:

```rust
//! CMake source-tree reader (milestone 102 US2 / milestone 103
//! implementation). Parses CMakeLists.txt + cmake/*.cmake +
//! Modules/*.cmake + third_party/*.cmake for FetchContent_Declare +
//! ExternalProject_Add directives. Per spec FR-005..FR-008.
//!
//! `find_package(X)` calls are NOT parsed per FR-007 (system-resolved;
//! double-counts otherwise). `add_subdirectory(third_party/...)`
//! vendored deps only emit when `include_vendored = true` per FR-008.

use std::path::{Path, PathBuf};
use mikebom_common::types::purl::{encode_purl_segment, Purl};
use regex::Regex;
use super::PackageDbEntry;

pub fn read(scan_root: &Path, include_vendored: bool) -> Vec<PackageDbEntry> {
    let mut entries = Vec::new();
    let cmake_files = discover_cmake_files(scan_root);
    for path in &cmake_files {
        let content = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e,
                    "failed to read CMake file (FR-013)");
                continue;
            }
        };
        let source_path = path.to_string_lossy().to_string();
        entries.extend(parse_fetch_block(&content, &source_path,
            "FetchContent_Declare"));
        entries.extend(parse_fetch_block(&content, &source_path,
            "ExternalProject_Add"));
        if include_vendored {
            entries.extend(parse_vendored(&content, &source_path, scan_root));
        }
    }
    entries
}

fn discover_cmake_files(scan_root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let top = scan_root.join("CMakeLists.txt");
    if top.is_file() { out.push(top); }
    for subdir in &["cmake", "Modules", "third_party"] {
        let dir = scan_root.join(subdir);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("cmake")
                   || p.file_name().and_then(|s| s.to_str()) == Some("CMakeLists.txt")
                {
                    out.push(p);
                }
            }
        }
    }
    out
}

fn parse_fetch_block(content: &str, source_path: &str, rule_name: &str)
    -> Vec<PackageDbEntry>
{
    let outer = match Regex::new(&format!(
        r"(?ms){}\s*\(\s*(\S+)(.*?)\)", regex::escape(rule_name)
    )) {
        Ok(r) => r, Err(_) => return Vec::new(),
    };
    let git_repo = Regex::new(r"GIT_REPOSITORY\s+(\S+)").ok();
    let git_tag = Regex::new(r"GIT_TAG\s+(\S+)").ok();
    let url_re = Regex::new(r"\bURL\s+(\S+)").ok();
    let url_hash = Regex::new(r"URL_HASH\s+SHA256=([\dA-Fa-f]+)").ok();
    let github_repo = Regex::new(r"^https?://github\.com/([^/]+)/([^/\.]+)").ok();

    let mut out = Vec::new();
    for c in outer.captures_iter(content) {
        let name = c.get(1).map(|m| m.as_str()).unwrap_or("");
        let body = c.get(2).map(|m| m.as_str()).unwrap_or("");
        let git_url = git_repo.as_ref().and_then(|r| r.captures(body))
            .and_then(|m| m.get(1)).map(|m| m.as_str());
        let tag = git_tag.as_ref().and_then(|r| r.captures(body))
            .and_then(|m| m.get(1)).map(|m| m.as_str());
        let url = url_re.as_ref().and_then(|r| r.captures(body))
            .and_then(|m| m.get(1)).map(|m| m.as_str());
        let sha = url_hash.as_ref().and_then(|r| r.captures(body))
            .and_then(|m| m.get(1)).map(|m| m.as_str());

        let (purl_str, download_url, version) = if let (Some(g), Some(t)) = (git_url, tag) {
            let purl = github_repo.as_ref().and_then(|r| r.captures(g))
                .and_then(|m| Some((m.get(1)?.as_str(), m.get(2)?.as_str())))
                .map(|(owner, repo)| format!(
                    "pkg:github/{}/{}@{}",
                    encode_purl_segment(owner),
                    encode_purl_segment(repo),
                    encode_purl_segment(t),
                ))
                .unwrap_or_else(|| format!(
                    "pkg:generic/{}@{}",
                    encode_purl_segment(name),
                    encode_purl_segment(t),
                ));
            (purl, Some(g.to_string()), t.to_string())
        } else if let Some(u) = url {
            let version = parse_version_from_url(u).unwrap_or_else(|| "unknown".to_string());
            (format!("pkg:generic/{}@{}",
                encode_purl_segment(name),
                encode_purl_segment(&version)),
                Some(u.to_string()), version)
        } else {
            continue;
        };

        if let Ok(purl) = Purl::new(&purl_str) {
            out.push(build_cmake_entry(
                name, &version, source_path, purl,
                download_url.as_deref(), sha, false,
            ));
        }
    }
    out
}

fn parse_vendored(content: &str, source_path: &str, scan_root: &Path)
    -> Vec<PackageDbEntry>
{
    let re = Regex::new(r"(?ms)add_subdirectory\s*\(\s*(third_party|vendor)/([^)\s]+)\s*\)").ok();
    let Some(re) = re else { return Vec::new(); };
    let mut out = Vec::new();
    for c in re.captures_iter(content) {
        let prefix = c.get(1).map(|m| m.as_str()).unwrap_or("");
        let name = c.get(2).map(|m| m.as_str()).unwrap_or("");
        // Version backfill from version.txt per research §6.
        let version_path = scan_root.join(prefix).join(name).join("version.txt");
        let version = std::fs::read_to_string(&version_path).ok()
            .and_then(|s| s.lines().find(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string()));
        let purl_str = match &version {
            Some(v) => format!("pkg:generic/{}@{}",
                encode_purl_segment(name), encode_purl_segment(v)),
            None => format!("pkg:generic/{}", encode_purl_segment(name)),
        };
        if let Ok(purl) = Purl::new(&purl_str) {
            let mut entry = build_cmake_entry(
                name, version.as_deref().unwrap_or(""), source_path, purl,
                None, None, true,
            );
            entry.extra_annotations.insert("mikebom:vendored".to_string(),
                serde_json::json!(true));
            out.push(entry);
        }
    }
    out
}

fn parse_version_from_url(url: &str) -> Option<String> {
    let re = Regex::new(r"[-_]([0-9]+\.[0-9]+(?:\.[0-9]+)?)").ok()?;
    re.captures(url)?.get(1).map(|m| m.as_str().to_string())
}

fn build_cmake_entry(
    name: &str, version: &str, source_path: &str, purl: Purl,
    download_url: Option<&str>, sha256: Option<&str>, vendored: bool,
) -> PackageDbEntry {
    // Same field-fill pattern as bazel.rs::build_bazel_entry and PR-A's
    // vcpkg.rs / conan.rs. download_url + sha256 + vendored flag set
    // appropriate annotations.
    todo!("inline like build_bazel_entry; verbose by design")
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    // 6 unit tests: FetchContent_Declare GIT + URL forms,
    // ExternalProject_Add same, find_package not parsed,
    // included-file walk, vendored opt-in gated, malformed-file
    // skip-with-warn.
}
```

**Total**: ~350-400 lines including doc-comments + tests.

## Test fixtures

See `research.md §9` for full fixture content. Summary:

- **`tests/fixtures/bazel/MODULE.bazel`** — 2 `bazel_dep` calls (abseil-cpp normal, googletest with `dev_dependency = True`).
- **`tests/fixtures/bazel/WORKSPACE.bazel`** — 1 `http_archive` + 1 `git_repository`.
- **`tests/fixtures/cmake/CMakeLists.txt`** — 1 `FetchContent_Declare(GIT_REPOSITORY)` + 1 `ExternalProject_Add(URL+URL_HASH)` + 1 `find_package(OpenSSL REQUIRED)` (negative-emission anchor) + `include(cmake/third_party.cmake)` + `add_subdirectory(third_party/foo)`.
- **`tests/fixtures/cmake/cmake/third_party.cmake`** — 1 `FetchContent_Declare(URL+URL_HASH)` for boost.
- **`tests/fixtures/cmake/third_party/foo/version.txt`** — `1.2.3` for vendored test.

## Integration tests

| File | Tests |
|------|-------|
| `scan_bazel.rs` | 4: pkg_bazel_purls + native_scope, http_archive_with_url+sha, git_repository_commit_as_version, malformed-skip |
| `scan_cmake.rs` | 4: fetchcontent_github → pkg_github, externalproject_url → sha256+url, cmake_subdir_walk, find_package_not_emitted (FR-007) |
| `scan_cmake_vendored.rs` | 3: default-off zero emission, with-flag pkg_generic+version.txt, third_party-prefix-gate (no first-party src/) |
| `scan_cmake_findpackage_negative.rs` | 1: dedicated FR-007 test — fixture with ONLY `find_package(...)`, no FetchContent — assert zero components |

## Goldens regen

Extend the existing `CASES` array in each regression test file:

```rust
// cdx_regression.rs / spdx_regression.rs / spdx3_regression.rs
const CASES: &[(&str, &str)] = &[
    ("apk", ...),
    ("cargo", ...),
    ("conan", ...),         // PR-A added
    ("deb", ...),
    ("gem", ...),
    ("golang", ...),
    ("maven", ...),
    ("npm", ...),
    ("pip", ...),
    ("rpm", ...),
    ("vcpkg", ...),         // PR-A added
    ("bazel", "tests/fixtures/bazel"),    // NEW this milestone
    ("cmake", "tests/fixtures/cmake"),    // NEW this milestone
];
```

Plus one `#[test] fn` per new ecosystem per format. Run with `MIKEBOM_UPDATE_*_GOLDENS=1` once to generate 6 committed goldens; subsequent runs verify byte-identity.

## Docs update

**`docs/user-guide/cli-reference.md`**: add a `--include-vendored` section per FR-014:

```markdown
### `--include-vendored`

Include vendored C/C++ dependencies declared via CMake
`add_subdirectory(third_party/<name>)` or `add_subdirectory(vendor/<name>)`.

**Default**: OFF. Opt-in via the flag OR `MIKEBOM_INCLUDE_VENDORED=1`
env var.

**Why default-off**: CMake's `add_subdirectory` is also used for
first-party project sub-modules (`src/`, `tests/`, `examples/`).
Default-on would inflate the SBOM with phantom components. The
`third_party/` or `vendor/` path prefix is the trigger — only
those subdirectories are considered.

**Version backfill**: when a co-located `version.txt` (or `.version`)
file exists at `<third_party_or_vendor>/<name>/version.txt`, its
first non-empty line is used as the component version. Otherwise
the component is emitted with no version segment in its PURL.

**Annotation**: every vendored component carries
`mikebom:vendored = true` so downstream consumers can filter
them in or out independently.
```

## Compatibility

- No `Cargo.lock` change — pure in-source body replacement.
- No production-code change outside bazel.rs + cmake.rs.
- No Linux/macOS/Windows CI behavior change beyond the new tests + goldens.
- Existing 11 ecosystems' goldens stay byte-identical per SC-005.

## No JSON / no YAML schema additions

Zero new fields. The `mikebom:*` properties (`download-url`, `bazel-archive-name`, `vendored`) all use the existing `extra_annotations: BTreeMap<String, serde_json::Value>` pattern that PR-A's vcpkg/conan readers already use.
