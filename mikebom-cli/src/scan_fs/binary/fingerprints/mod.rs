//! External symbol-fingerprint corpus subsystem (milestone 108).
//!
//! This module provides:
//! - A typed corpus loader (`load_corpus`) that returns either a
//!   cached external corpus OR the bundled in-source fallback,
//!   depending on operator opt-in and cache state.
//! - A typed `FingerprintRecord` shape that both the bundled and
//!   external paths produce, so the matcher in
//!   `super::symbol_fingerprint::scan` consumes a unified slice
//!   regardless of source.
//! - A `CorpusSource` enum tracking provenance for the
//!   `mikebom:fingerprint-corpus-sha` SBOM annotation (FR-005).
//!
//! Phase 2B/2C scope: types + loader + bundled-fallback path. Phase 4
//! adds the network-fetch path (`fetch.rs`); Phase 4 also wires
//! `--fingerprints-corpus` and stamps the annotation. Until then,
//! `load_corpus(LoadOptions::default())` returns the bundled corpus
//! and `symbol_fingerprint::scan` calls into it without behavioral
//! change.
//!
//! See `specs/108-fingerprint-corpus/`.

pub(crate) mod cache;
pub(crate) mod loader;
pub(crate) mod record;
pub(crate) mod source_sha;

use std::sync::OnceLock;

pub(crate) use record::FingerprintRecord;
pub(crate) use source_sha::CorpusSha;

/// Provenance tag for the corpus that produced a match. Surfaces as
/// the value of the `mikebom:fingerprint-corpus-sha` SBOM annotation
/// (FR-005): the 12-hex SHA for cached/fetched paths, the literal
/// `"bundled"` for the fallback path.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) enum CorpusSource {
    /// In-source bundled fallback (the seeded 7-library corpus from
    /// milestone 099, frozen at milestone-108 ship time).
    Bundled,
    /// External corpus loaded from a populated cache (no network fetch
    /// was needed during this scan).
    Cached { sha: CorpusSha },
    /// External corpus loaded after a successful cache-miss fetch.
    Fetched { sha: CorpusSha },
}

#[allow(dead_code)]
impl CorpusSource {
    /// SBOM annotation value per FR-005. 12-hex truncation for
    /// `Cached`/`Fetched`; literal `"bundled"` for `Bundled`.
    pub fn annotation_value(&self) -> String {
        match self {
            CorpusSource::Bundled => "bundled".to_string(),
            CorpusSource::Cached { sha } | CorpusSource::Fetched { sha } => sha.to_short_hex(),
        }
    }
}

/// Container the matcher consumes. Holds the validated records + the
/// source tag for downstream annotation emission.
#[allow(dead_code)]
pub(crate) struct FingerprintCorpus {
    pub records: Vec<FingerprintRecord>,
    pub source: CorpusSource,
}

/// Options accepted by `load_corpus`. Phase 2B/2C only exposes the
/// `external_enabled` field; Phase 4 adds `offline` + `override_sha`.
#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub(crate) struct LoadOptions {
    /// True when the operator passed `--fingerprints-corpus` (or set
    /// `MIKEBOM_FINGERPRINTS_CORPUS=1`). When false, the bundled
    /// fallback is returned unconditionally — no cache access.
    pub external_enabled: bool,
}

/// Return the bundled in-source 7-library corpus. Memoized via
/// `OnceLock` so the 7 owned-string allocations happen exactly once
/// per process.
#[allow(dead_code)]
pub(crate) fn load_bundled() -> &'static FingerprintCorpus {
    static BUNDLED: OnceLock<FingerprintCorpus> = OnceLock::new();
    BUNDLED.get_or_init(|| FingerprintCorpus {
        records: super::symbol_fingerprint::bundled_records(),
        source: CorpusSource::Bundled,
    })
}

/// Resolve the active corpus for this scan.
///
/// Phase 2B/2C behavior: always returns the bundled fallback. Phase 4
/// will add the cache-first / fetch-on-miss / fall-back-to-bundled
/// flow per FR-004 when `opts.external_enabled` is true.
#[allow(dead_code)]
pub(crate) fn load_corpus(_opts: LoadOptions) -> &'static FingerprintCorpus {
    // Phase 2B/2C stub: external path not wired yet. Bundled fallback
    // satisfies FR-001 (no regression for non-opt-in operators).
    load_bundled()
}

/// Process-wide mutex for tests that mutate the
/// `MIKEBOM_FINGERPRINTS_CACHE_DIR` env var. cargo runs tests in
/// parallel by default; without a shared lock, `cache::tests` and
/// `loader::tests` race for the same env var. Shared here (not
/// per-module) so any test in either module serializes against
/// the others.
#[cfg(test)]
pub(super) fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::Mutex;
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}
