//! CMake source-tree reader (milestone 102 US2 / milestone 103 implementation).
//!
//! Parses `CMakeLists.txt` + included `.cmake` files for:
//! - `FetchContent_Declare(<name> GIT_REPOSITORY ... GIT_TAG ...)` — emits
//!   `pkg:github/<owner>/<repo>@<tag>` for GitHub-hosted URLs, otherwise
//!   `pkg:generic/<name>@<tag>` with `mikebom:download-url`.
//! - `FetchContent_Declare(<name> URL ... URL_HASH SHA256=...)` and
//!   `ExternalProject_Add(<name> URL ...)` — emits `pkg:generic/<name>@<version>`
//!   with URL + SHA-256.
//! - `add_subdirectory(third_party|vendor/<name>)` — opt-in via the
//!   `include_vendored` parameter (wired from PR-A's CLI flag); emits
//!   `pkg:generic/<name>@<version-from-version.txt>` with the JSON
//!   boolean `mikebom:vendored = true` annotation.
//!
//! `find_package(X)` declarations are NOT parsed per FR-007 — they
//! resolve to system-installed packages and would double-count
//! against OS-package readers + vcpkg + Conan.
//!
//! Walks at depth 1: scan root for `CMakeLists.txt`; `cmake/`,
//! `Modules/`, `third_party/` for `*.cmake` files. Per FR-005.
//!
//! Cross-platform; no `#[cfg(unix)]` gates. Zero new Cargo deps —
//! uses workspace `regex` + std.

use std::path::{Path, PathBuf};

use mikebom_common::types::hash::ContentHash;
use mikebom_common::types::purl::{encode_purl_segment, Purl};
use regex::Regex;

use super::PackageDbEntry;

pub fn read(scan_root: &Path, include_vendored: bool) -> Vec<PackageDbEntry> {
    let cmake_files = discover_cmake_files(scan_root);
    let mut entries = Vec::new();
    for path in &cmake_files {
        let content = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "failed to read CMake file (FR-013)"
                );
                continue;
            }
        };
        let source_path = path.to_string_lossy().to_string();
        entries.extend(parse_fetch_block(
            &content,
            &source_path,
            "FetchContent_Declare",
        ));
        entries.extend(parse_fetch_block(
            &content,
            &source_path,
            "ExternalProject_Add",
        ));
        if include_vendored {
            entries.extend(parse_vendored(&content, &source_path, scan_root));
        }
    }
    entries
}

/// Discover CMake files: top-level `CMakeLists.txt` + any `*.cmake`
/// (and `CMakeLists.txt`) at depth 1 of `cmake/`, `Modules/`,
/// `third_party/`. Non-recursive per FR-005.
fn discover_cmake_files(scan_root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let top = scan_root.join("CMakeLists.txt");
    if top.is_file() {
        out.push(top);
    }
    for subdir in &["cmake", "Modules", "third_party"] {
        let dir = scan_root.join(subdir);
        if let Ok(read_dir) = std::fs::read_dir(&dir) {
            for entry in read_dir.flatten() {
                let p = entry.path();
                let is_cmake_module = p
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("cmake"))
                    .unwrap_or(false);
                let is_cmakelists = p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.eq_ignore_ascii_case("CMakeLists.txt"))
                    .unwrap_or(false);
                if (is_cmake_module || is_cmakelists) && p.is_file() {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// Parse a `FetchContent_Declare(...)` or `ExternalProject_Add(...)`
/// block. Parameterized over `rule_name` so the same body handles both.
/// Returns one `PackageDbEntry` per matched rule per research §3+§4.
fn parse_fetch_block(content: &str, source_path: &str, rule_name: &str) -> Vec<PackageDbEntry> {
    // Outer envelope: rule_name + first whitespace-separated token
    // (the dep name) + everything until the matching `)`. Non-greedy
    // dotall.
    let outer_pattern = format!(
        r"(?ms){}\s*\(\s*(\S+)(.*?)\)",
        regex::escape(rule_name)
    );
    let outer = match Regex::new(&outer_pattern) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let github_re = Regex::new(r"^https?://github\.com/([^/]+)/([^/\.\s]+)").ok();
    let mut out = Vec::new();
    for c in outer.captures_iter(content) {
        let name = c.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        let body = c.get(2).map(|m| m.as_str()).unwrap_or("");
        if name.is_empty() {
            continue;
        }

        let git_repo = extract_keyword(body, "GIT_REPOSITORY");
        let git_tag = extract_keyword(body, "GIT_TAG");
        let url = extract_keyword(body, "URL");
        let url_hash_sha256 = extract_url_hash_sha256(body);

        let (purl_str, download_url, version) = if let (Some(g), Some(t)) =
            (git_repo.as_deref(), git_tag.as_deref())
        {
            // GIT form — check for GitHub URL → pkg:github/<owner>/<repo>@<tag>;
            // otherwise pkg:generic/<name>@<tag>.
            let github_pair = github_re
                .as_ref()
                .and_then(|r| r.captures(g))
                .and_then(|m| {
                    let owner = m.get(1)?.as_str();
                    let repo = m.get(2)?.as_str();
                    Some((owner.to_string(), repo.to_string()))
                });
            let purl = match github_pair {
                Some((owner, repo)) => format!(
                    "pkg:github/{}/{}@{}",
                    encode_purl_segment(&owner),
                    encode_purl_segment(&repo),
                    encode_purl_segment(t)
                ),
                None => format!(
                    "pkg:generic/{}@{}",
                    encode_purl_segment(name),
                    encode_purl_segment(t)
                ),
            };
            (purl, Some(g.to_string()), t.to_string())
        } else if let Some(u) = url.as_deref() {
            // URL form — parse version from filename.
            let version = parse_version_from_url(u).unwrap_or_else(|| "unknown".to_string());
            (
                format!(
                    "pkg:generic/{}@{}",
                    encode_purl_segment(name),
                    encode_purl_segment(&version)
                ),
                Some(u.to_string()),
                version,
            )
        } else {
            continue;
        };

        if let Ok(purl) = Purl::new(&purl_str) {
            // Distinguish source-mechanism by rule name + presence
            // of GIT_REPOSITORY (FetchContent_Declare with git form)
            // vs URL form. ExternalProject_Add only supports URL
            // form in our parser.
            let source_mechanism = if rule_name == "ExternalProject_Add" {
                "cmake-externalproject"
            } else if git_repo.is_some() {
                "cmake-fetchcontent-git"
            } else {
                "cmake-fetchcontent-url"
            };
            out.push(build_cmake_entry(
                name,
                &version,
                source_path,
                purl,
                download_url.as_deref(),
                url_hash_sha256.as_deref(),
                false,
                source_mechanism,
            ));
        }
    }
    out
}

/// Extract a CMake keyword-value pair. CMake syntax: `KEYWORD value`
/// (whitespace-separated, no `=`). Value is the next non-whitespace
/// token. Returns None when keyword not found.
fn extract_keyword(body: &str, keyword: &str) -> Option<String> {
    let pattern = format!(r"\b{}\s+(\S+)", regex::escape(keyword));
    let re = Regex::new(&pattern).ok()?;
    re.captures(body)?.get(1).map(|m| m.as_str().to_string())
}

/// Extract `URL_HASH SHA256=<hex>` — CMake's compound-keyword form.
fn extract_url_hash_sha256(body: &str) -> Option<String> {
    let re = Regex::new(r"URL_HASH\s+SHA256\s*=\s*([0-9a-fA-F]+)").ok()?;
    re.captures(body)?.get(1).map(|m| m.as_str().to_string())
}

/// Parse the vendored-dep block — `add_subdirectory(third_party/<name>)`
/// or `add_subdirectory(vendor/<name>)`. Per FR-008. Only called when
/// `include_vendored = true`. Reads `<scan_root>/<prefix>/<name>/version.txt`
/// for version backfill per FR-009 + research §6.
fn parse_vendored(
    content: &str,
    source_path: &str,
    scan_root: &Path,
) -> Vec<PackageDbEntry> {
    let re = match Regex::new(
        r"(?ms)add_subdirectory\s*\(\s*(third_party|vendor)/([^)\s]+)\s*\)",
    ) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for c in re.captures_iter(content) {
        let prefix = c.get(1).map(|m| m.as_str()).unwrap_or("");
        let name = c.get(2).map(|m| m.as_str()).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        // Version backfill from <scan_root>/<prefix>/<name>/version.txt
        // first non-empty line.
        let version_path = scan_root.join(prefix).join(name).join("version.txt");
        let version = std::fs::read_to_string(&version_path).ok().and_then(|s| {
            s.lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string())
        });
        let purl_str = match &version {
            Some(v) => format!(
                "pkg:generic/{}@{}",
                encode_purl_segment(name),
                encode_purl_segment(v)
            ),
            None => format!("pkg:generic/{}", encode_purl_segment(name)),
        };
        if let Ok(purl) = Purl::new(&purl_str) {
            let mut entry = build_cmake_entry(
                name,
                version.as_deref().unwrap_or(""),
                source_path,
                purl,
                None,
                None,
                true,
                "cmake-vendored",
            );
            // FR-009: JSON boolean `true` per the milestone-009
            // `mikebom:shade-relocation` precedent.
            entry.extra_annotations.insert(
                "mikebom:vendored".to_string(),
                serde_json::json!(true),
            );
            out.push(entry);
        }
    }
    out
}

/// Parse a semver-ish version from an archive URL filename.
/// Same regex as bazel.rs's helper; copied here to keep modules
/// independent.
fn parse_version_from_url(url: &str) -> Option<String> {
    let re = Regex::new(r"[-_/]v?([0-9]+\.[0-9]+(?:\.[0-9]+)?)").ok()?;
    re.captures(url)?.get(1).map(|m| m.as_str().to_string())
}

#[allow(clippy::too_many_arguments)]
fn build_cmake_entry(
    name: &str,
    version: &str,
    source_path: &str,
    purl: Purl,
    download_url: Option<&str>,
    sha256_hex: Option<&str>,
    _vendored: bool,
    source_mechanism: &str,
) -> PackageDbEntry {
    let mut extra_annotations: std::collections::BTreeMap<String, serde_json::Value> =
        std::collections::BTreeMap::new();
    if let Some(url) = download_url {
        extra_annotations.insert(
            "mikebom:download-url".to_string(),
            serde_json::json!(url),
        );
    }
    // C/C++ provenance: explicit `mikebom:source-mechanism` annotation
    // so operators can grep/filter components by origin without
    // reverse-engineering the PURL prefix + per-reader annotations.
    // Closed enum across cmake / vcpkg / conan / bazel:
    //   cmake-fetchcontent-git, cmake-fetchcontent-url,
    //   cmake-externalproject, cmake-vendored,
    //   bazel-http-archive, vcpkg-manifest, conan-recipe.
    extra_annotations.insert(
        "mikebom:source-mechanism".to_string(),
        serde_json::json!(source_mechanism),
    );
    let hashes = sha256_hex
        .and_then(|hex| ContentHash::sha256(hex).ok())
        .map(|h| vec![h])
        .unwrap_or_default();

    PackageDbEntry {
        purl,
        name: name.to_string(),
        version: version.to_string(),
        arch: None,
        source_path: source_path.to_string(),
        depends: Vec::new(),
        maintainer: None,
        licenses: Vec::new(),
        lifecycle_scope: None,
        requirement_range: None,
        source_type: None,
        buildinfo_status: None,
        evidence_kind: None,
        binary_class: None,
        binary_stripped: None,
        linkage_kind: None,
        detected_go: None,
        confidence: None,
        binary_packed: None,
        raw_version: None,
        parent_purl: None,
        npm_role: None,
        co_owned_by: None,
        hashes,
        sbom_tier: Some("source".to_string()),
        shade_relocation: None,
        extra_annotations,
        binary_role: None,
    }
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    #[test]
    fn empty_when_no_files() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read(tmp.path(), false).is_empty());
    }

    #[test]
    fn fetchcontent_github_emits_pkg_github() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("CMakeLists.txt"),
            r#"FetchContent_Declare(googletest GIT_REPOSITORY https://github.com/google/googletest.git GIT_TAG release-1.14.0)"#,
        )
        .unwrap();
        let entries = read(tmp.path(), false);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].purl.as_str(),
            "pkg:github/google/googletest@release-1.14.0"
        );
    }

    #[test]
    fn fetchcontent_url_emits_pkg_generic_with_sha256() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("CMakeLists.txt"),
            r#"FetchContent_Declare(zlib URL https://zlib.net/zlib-1.3.1.tar.gz URL_HASH SHA256=9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23)"#,
        )
        .unwrap();
        let entries = read(tmp.path(), false);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].purl.as_str(), "pkg:generic/zlib@1.3.1");
        assert_eq!(entries[0].hashes.len(), 1);
        assert_eq!(
            entries[0]
                .extra_annotations
                .get("mikebom:download-url")
                .and_then(|v| v.as_str()),
            Some("https://zlib.net/zlib-1.3.1.tar.gz")
        );
    }

    #[test]
    fn externalproject_add_url_form() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("CMakeLists.txt"),
            r#"ExternalProject_Add(boost URL https://example.com/boost_1_84_0.tar.gz)"#,
        )
        .unwrap();
        let entries = read(tmp.path(), false);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].purl.as_str().starts_with("pkg:generic/boost@"));
    }

    #[test]
    fn find_package_does_not_emit_components() {
        let tmp = tempfile::tempdir().unwrap();
        // ONLY find_package, no FetchContent_Declare or ExternalProject_Add.
        std::fs::write(
            tmp.path().join("CMakeLists.txt"),
            r#"find_package(zlib REQUIRED)"#,
        )
        .unwrap();
        let entries = read(tmp.path(), false);
        assert!(
            entries.is_empty(),
            "find_package(X) MUST NOT emit components per FR-007; got {entries:?}"
        );
    }

    #[test]
    fn vendored_emits_only_when_include_vendored_set() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("CMakeLists.txt"),
            r#"add_subdirectory(third_party/foo)"#,
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("third_party/foo")).unwrap();
        std::fs::write(
            tmp.path().join("third_party/foo/version.txt"),
            "1.2.3",
        )
        .unwrap();

        // Default off: no emission.
        assert!(read(tmp.path(), false).is_empty());

        // With include_vendored = true: 1 component with vendored annotation.
        let entries = read(tmp.path(), true);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].purl.as_str(), "pkg:generic/foo@1.2.3");
        assert_eq!(
            entries[0].extra_annotations.get("mikebom:vendored"),
            Some(&serde_json::json!(true))
        );
    }

    #[test]
    fn vendored_path_prefix_gate_rejects_first_party() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("CMakeLists.txt"),
            r#"add_subdirectory(src)
add_subdirectory(tests)"#,
        )
        .unwrap();
        // Even with include_vendored=true, first-party `src/`/`tests/`
        // sub-modules MUST NOT emit per FR-008's path-prefix gate.
        let entries = read(tmp.path(), true);
        assert!(
            entries.is_empty(),
            "first-party add_subdirectory(src) MUST NOT emit; got {entries:?}"
        );
    }

    // --- C/C++ provenance: source-mechanism annotation ---------------------

    #[test]
    fn source_mechanism_annotation_fetchcontent_git() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("CMakeLists.txt"),
            r#"FetchContent_Declare(googletest GIT_REPOSITORY https://github.com/google/googletest.git GIT_TAG release-1.14.0)"#,
        )
        .unwrap();
        let entries = read(tmp.path(), false);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0]
                .extra_annotations
                .get("mikebom:source-mechanism")
                .and_then(|v| v.as_str()),
            Some("cmake-fetchcontent-git"),
            "FetchContent_Declare GIT form should be `cmake-fetchcontent-git`; got: {:?}",
            entries[0].extra_annotations.get("mikebom:source-mechanism"),
        );
    }

    #[test]
    fn source_mechanism_annotation_fetchcontent_url() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("CMakeLists.txt"),
            r#"FetchContent_Declare(zlib URL https://zlib.net/zlib-1.3.1.tar.gz URL_HASH SHA256=9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23)"#,
        )
        .unwrap();
        let entries = read(tmp.path(), false);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0]
                .extra_annotations
                .get("mikebom:source-mechanism")
                .and_then(|v| v.as_str()),
            Some("cmake-fetchcontent-url"),
        );
    }

    #[test]
    fn source_mechanism_annotation_externalproject() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("CMakeLists.txt"),
            r#"ExternalProject_Add(boost URL https://example.com/boost_1_84_0.tar.gz)"#,
        )
        .unwrap();
        let entries = read(tmp.path(), false);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0]
                .extra_annotations
                .get("mikebom:source-mechanism")
                .and_then(|v| v.as_str()),
            Some("cmake-externalproject"),
        );
    }

    #[test]
    fn source_mechanism_annotation_vendored() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("CMakeLists.txt"),
            r#"add_subdirectory(third_party/foo)"#,
        )
        .unwrap();
        // Vendored dir needs a version source — use third_party/foo/version.txt
        std::fs::create_dir_all(tmp.path().join("third_party/foo")).unwrap();
        std::fs::write(tmp.path().join("third_party/foo/version.txt"), "1.2.3").unwrap();
        let entries = read(tmp.path(), true);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0]
                .extra_annotations
                .get("mikebom:source-mechanism")
                .and_then(|v| v.as_str()),
            Some("cmake-vendored"),
        );
    }
}
