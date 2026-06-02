//! Cache-first JSON loader for the fingerprint corpus.
//!
//! Reads `<cache-dir>/<sha>/corpus/index.json` to enumerate per-library
//! files, then loads each `corpus/<library>.json` into a
//! `FingerprintRecord`. Records that fail individual validation are
//! skipped with `tracing::warn!` per FR-010; other records still load.
//! Missing or corrupt index returns a typed error so the caller can
//! decide whether to trigger a fetch (Phase 4) or fall back to bundled.

use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

use super::cache;
use super::record::FingerprintRecord;
use super::source_sha::CorpusSha;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub(crate) enum LoaderError {
    #[error("cache miss: no `corpus/index.json` at {path}")]
    CacheNotFound { path: String },
    #[error("cache corrupt: {reason} (at {path})")]
    CacheCorrupt { path: String, reason: String },
}

#[derive(Debug, Deserialize)]
struct CorpusIndex {
    version: u32,
    entries: Vec<IndexEntry>,
}

#[derive(Debug, Deserialize)]
struct IndexEntry {
    library: String,
    path: String,
    #[serde(default)]
    #[allow(dead_code)]
    digest: Option<String>,
}

/// Load the corpus from the per-SHA cache directory. Returns a vector
/// of validated records or a typed error for the caller to handle.
#[allow(dead_code)]
pub(crate) fn load_corpus_from_cache(
    sha: &CorpusSha,
) -> Result<Vec<FingerprintRecord>, LoaderError> {
    let dir = cache::cache_dir_for_sha(sha);
    let corpus_dir = dir.join("corpus");
    let index_path = corpus_dir.join("index.json");

    let index_text = std::fs::read_to_string(&index_path).map_err(|_| {
        LoaderError::CacheNotFound {
            path: index_path.display().to_string(),
        }
    })?;

    let index: CorpusIndex = serde_json::from_str(&index_text).map_err(|e| {
        LoaderError::CacheCorrupt {
            path: index_path.display().to_string(),
            reason: format!("index.json parse failed: {e}"),
        }
    })?;

    if index.version != 1 {
        return Err(LoaderError::CacheCorrupt {
            path: index_path.display().to_string(),
            reason: format!("unsupported index version: {}", index.version),
        });
    }

    let records = load_per_library_records(&corpus_dir, &index);
    Ok(records)
}

fn load_per_library_records(
    corpus_dir: &Path,
    index: &CorpusIndex,
) -> Vec<FingerprintRecord> {
    let mut out = Vec::with_capacity(index.entries.len());
    for entry in &index.entries {
        let path = corpus_dir.join(&entry.path);
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    library = %entry.library,
                    path = %path.display(),
                    error = %e,
                    "fingerprint corpus record file unreadable; skipping",
                );
                continue;
            }
        };
        let record: FingerprintRecord = match serde_json::from_str(&text) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    library = %entry.library,
                    path = %path.display(),
                    error = %e,
                    "fingerprint corpus record JSON malformed; skipping",
                );
                continue;
            }
        };
        if let Err(e) = record.validate() {
            tracing::warn!(
                library = %entry.library,
                path = %path.display(),
                error = %e,
                "fingerprint corpus record failed validation; skipping",
            );
            continue;
        }
        out.push(record);
    }
    out
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;
    // Shared env-mutation lock with cache::tests — see fingerprints/mod.rs.
    use super::super::test_env_lock as env_lock;

    const SAMPLE_SHA: &str = "fff39c6ad22ce8420b506323ce1d5cce4b628d5c";

    fn write_valid_record(corpus_dir: &Path, library: &str) -> std::path::PathBuf {
        let path = corpus_dir.join(format!("{library}.json"));
        std::fs::write(
            &path,
            format!(
                r#"{{
                    "library": "{library}",
                    "target_purl": "pkg:generic/{library}",
                    "symbols": ["sym1", "sym2", "sym3"],
                    "min_symbols": 2
                }}"#
            ),
        )
        .unwrap();
        path
    }

    fn write_index(corpus_dir: &Path, libraries: &[&str]) {
        let entries: Vec<String> = libraries
            .iter()
            .map(|l| format!(r#"{{"library":"{l}","path":"{l}.json"}}"#))
            .collect();
        let json = format!(
            r#"{{"version":1,"entries":[{}]}}"#,
            entries.join(",")
        );
        std::fs::write(corpus_dir.join("index.json"), json).unwrap();
    }

    fn setup_cache(libraries: &[&str]) -> (tempfile::TempDir, CorpusSha) {
        let tmp = tempfile::tempdir().unwrap();
        let sha = CorpusSha::from_hex(SAMPLE_SHA).unwrap();
        let corpus_dir = tmp.path().join(sha.to_full_hex()).join("corpus");
        std::fs::create_dir_all(&corpus_dir).unwrap();
        for lib in libraries {
            write_valid_record(&corpus_dir, lib);
        }
        write_index(&corpus_dir, libraries);
        (tmp, sha)
    }

    #[test]
    fn loads_valid_cache_to_corpus() {
        let _g = env_lock();
        let (tmp, sha) = setup_cache(&["openssl", "zlib"]);
        let path_str = tmp.path().to_string_lossy().into_owned();
        unsafe {
            std::env::set_var("MIKEBOM_FINGERPRINTS_CACHE_DIR", &path_str);
        }
        let records = load_corpus_from_cache(&sha).unwrap();
        assert_eq!(records.len(), 2);
        unsafe {
            std::env::remove_var("MIKEBOM_FINGERPRINTS_CACHE_DIR");
        }
    }

    #[test]
    fn returns_cache_not_found_when_index_absent() {
        let _g = env_lock();
        let tmp = tempfile::tempdir().unwrap();
        let path_str = tmp.path().to_string_lossy().into_owned();
        unsafe {
            std::env::set_var("MIKEBOM_FINGERPRINTS_CACHE_DIR", &path_str);
        }
        let sha = CorpusSha::from_hex(SAMPLE_SHA).unwrap();
        assert!(matches!(
            load_corpus_from_cache(&sha),
            Err(LoaderError::CacheNotFound { .. })
        ));
        unsafe {
            std::env::remove_var("MIKEBOM_FINGERPRINTS_CACHE_DIR");
        }
    }

    #[test]
    fn returns_cache_corrupt_on_malformed_index_json() {
        let _g = env_lock();
        let tmp = tempfile::tempdir().unwrap();
        let sha = CorpusSha::from_hex(SAMPLE_SHA).unwrap();
        let corpus_dir = tmp.path().join(sha.to_full_hex()).join("corpus");
        std::fs::create_dir_all(&corpus_dir).unwrap();
        std::fs::write(corpus_dir.join("index.json"), "{ not valid json").unwrap();
        let path_str = tmp.path().to_string_lossy().into_owned();
        unsafe {
            std::env::set_var("MIKEBOM_FINGERPRINTS_CACHE_DIR", &path_str);
        }
        assert!(matches!(
            load_corpus_from_cache(&sha),
            Err(LoaderError::CacheCorrupt { .. })
        ));
        unsafe {
            std::env::remove_var("MIKEBOM_FINGERPRINTS_CACHE_DIR");
        }
    }

    #[test]
    fn skips_malformed_records_warns_continues() {
        let _g = env_lock();
        let (tmp, sha) = setup_cache(&["openssl"]);
        let corpus_dir = tmp.path().join(sha.to_full_hex()).join("corpus");
        // Add a malformed record file to the corpus dir + index.
        std::fs::write(corpus_dir.join("broken.json"), "{ invalid").unwrap();
        write_index(&corpus_dir, &["openssl", "broken"]);
        let path_str = tmp.path().to_string_lossy().into_owned();
        unsafe {
            std::env::set_var("MIKEBOM_FINGERPRINTS_CACHE_DIR", &path_str);
        }
        let records = load_corpus_from_cache(&sha).unwrap();
        // Only the valid record loaded; the broken one was skipped.
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].library, "openssl");
        unsafe {
            std::env::remove_var("MIKEBOM_FINGERPRINTS_CACHE_DIR");
        }
    }

    #[test]
    fn parses_index_with_optional_digest_field() {
        let _g = env_lock();
        let tmp = tempfile::tempdir().unwrap();
        let sha = CorpusSha::from_hex(SAMPLE_SHA).unwrap();
        let corpus_dir = tmp.path().join(sha.to_full_hex()).join("corpus");
        std::fs::create_dir_all(&corpus_dir).unwrap();
        write_valid_record(&corpus_dir, "openssl");
        std::fs::write(
            corpus_dir.join("index.json"),
            r#"{"version":1,"entries":[{"library":"openssl","path":"openssl.json","digest":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}]}"#,
        )
        .unwrap();
        let path_str = tmp.path().to_string_lossy().into_owned();
        unsafe {
            std::env::set_var("MIKEBOM_FINGERPRINTS_CACHE_DIR", &path_str);
        }
        let records = load_corpus_from_cache(&sha).unwrap();
        assert_eq!(records.len(), 1);
        unsafe {
            std::env::remove_var("MIKEBOM_FINGERPRINTS_CACHE_DIR");
        }
    }
}
