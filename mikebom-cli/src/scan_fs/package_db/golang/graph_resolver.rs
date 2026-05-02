// Milestone 055 — Go transitive-edge resolver: 4-step ladder.
//
// Module-level `#[allow(dead_code)]`: the foundational scaffold (T004)
// lands the types ahead of the US1 wiring tasks (T021–T025) that
// actually consume them. The allow is removed in T025 once
// `legacy::read()` calls `GraphResolver::resolve()` per scan.
#![allow(dead_code)]

//
// This module is the orchestrator for spec FR-002's resolution ladder:
//
//   1. `go mod graph` (when `go` is on PATH and `--offline` not set)
//   2. `$GOMODCACHE` walk (existing 053 behavior; reuses
//      `legacy::cache_lookup_depends`)
//   3. Proxy fetch from `$GOPROXY` (per the Go module proxy protocol)
//   4. Graceful no-edges fallthrough (component still emits with empty
//      `depends`; FR-009 ladder summary names the count)
//
// All edges are intersected with the workspace's `go.sum` per FR-003
// (`go.sum` is canonical for what's installed). Workspace-level `replace`
// directives are applied per FR-006.
//
// See specs/055-go-transitive-edges/spec.md and
// specs/055-go-transitive-edges/contracts/resolver-api.md for the
// full contract.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use crate::scan_fs::package_db::golang::goprivate::{PrivatePatterns, ProxyChain};
use crate::scan_fs::package_db::golang::module_id::ModuleId;

// --------------------------------------------------------------------
// Resolution-step taxonomy
// --------------------------------------------------------------------

/// Which step of the 4-step ladder supplied this module's transitive
/// requires (per FR-002 / FR-009 ladder summary).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionStep {
    /// Step 1: `go mod graph` subprocess.
    GoModGraph,
    /// Step 2: `$GOMODCACHE` walk (the milestone 053 codepath).
    GoModCache,
    /// Step 3: HTTP fetch from `$GOPROXY`.
    Proxy,
    /// Step 4: graceful fallthrough — no edges produced for this module.
    None,
}

/// Per-module record after resolution. `requires` is post-replace, post-
/// intersection-with-go.sum.
#[derive(Clone, Debug)]
pub struct ModuleGraphEntry {
    pub module: ModuleId,
    pub requires: Vec<ModuleId>,
    pub source: ResolutionStep,
}

// --------------------------------------------------------------------
// Top-level resolver output
// --------------------------------------------------------------------

/// The complete module graph for a single scan. Keyed by `ModuleId`.
/// Consulted by `legacy::read()` to populate each `PackageDbEntry`'s
/// `depends` field.
#[derive(Clone, Debug, Default)]
pub struct ModuleGraphMap {
    entries: HashMap<ModuleId, ModuleGraphEntry>,
    summary: LadderSummary,
}

impl ModuleGraphMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn requires(&self, m: &ModuleId) -> &[ModuleId] {
        self.entries
            .get(m)
            .map(|e| e.requires.as_slice())
            .unwrap_or(&[])
    }

    pub fn entry(&self, m: &ModuleId) -> Option<&ModuleGraphEntry> {
        self.entries.get(m)
    }

    pub fn summary(&self) -> &LadderSummary {
        &self.summary
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ModuleId, &ModuleGraphEntry)> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    // --- mutating API used internally by GraphResolver ---

    pub(crate) fn insert(&mut self, entry: ModuleGraphEntry) {
        self.entries.insert(entry.module.clone(), entry);
    }

    pub(crate) fn contains(&self, m: &ModuleId) -> bool {
        self.entries.contains_key(m)
    }

    pub(crate) fn summary_mut(&mut self) -> &mut LadderSummary {
        &mut self.summary
    }

    pub(crate) fn entries_mut(&mut self) -> &mut HashMap<ModuleId, ModuleGraphEntry> {
        &mut self.entries
    }
}

// --------------------------------------------------------------------
// FR-009 ladder summary
// --------------------------------------------------------------------

/// Counters behind the FR-009 per-scan `tracing::info` summary line.
#[derive(Clone, Debug, Default)]
pub struct LadderSummary {
    pub graph_count: usize,
    pub cache_count: usize,
    pub proxy_count: usize,
    pub missing_count: usize,
    pub fetch_errors: HashMap<String, usize>,
}

impl fmt::Display for LadderSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "go transitive edges: ladder=[graph:{}, cache:{}, proxy:{}, missing:{}]",
            self.graph_count, self.cache_count, self.proxy_count, self.missing_count
        )
    }
}

// --------------------------------------------------------------------
// Workspace context
// --------------------------------------------------------------------

/// Inputs the resolver needs from the workspace + environment.
/// Constructed once per scan.
#[derive(Clone, Debug)]
pub struct WorkspaceContext {
    pub root_dir: PathBuf,
    pub go_sum_modules: HashSet<ModuleId>,
    pub replaces: HashMap<ModuleId, ModuleId>,
    pub excludes: HashSet<ModuleId>,
    pub offline: bool,
    pub gomodcache: PathBuf,
    pub goproxy: ProxyChain,
    pub goprivate: PrivatePatterns,
}

// --------------------------------------------------------------------
// Step-result + error taxonomy
// --------------------------------------------------------------------

/// Outcome of a single ladder-step invocation. The orchestrator decides
/// whether to fall through based on this value.
#[derive(Clone, Debug)]
pub enum StepResult<T> {
    /// Step succeeded; data attached.
    Ok(T),
    /// Step is unavailable (precondition not met) — fall through silently.
    /// e.g., `go` not on PATH, `--offline` set, `GOPROXY=off`.
    Unavailable,
    /// Step attempted and failed — fall through with a `tracing::warn`.
    Failed(StepError),
}

#[derive(Clone, Debug)]
pub struct StepError {
    pub class: ErrorClass,
    pub detail: String,
}

/// Operator-friendly error classification for `tracing::warn` lines per
/// research.md R14. Stable string names (`error_class="timeout"`, etc.)
/// are used in the summary's `fetch_errors` map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorClass {
    Timeout,
    Http4xx,
    Http404,
    Http5xx,
    Dns,
    Connection,
    Tls,
    Parse,
    Other,
}

impl ErrorClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Http4xx => "http_4xx",
            Self::Http404 => "http_404",
            Self::Http5xx => "http_5xx",
            Self::Dns => "dns",
            Self::Connection => "connection",
            Self::Tls => "tls",
            Self::Parse => "parse",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for ErrorClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --------------------------------------------------------------------
// Resolver config + error
// --------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct GraphResolverConfig {
    pub go_mod_graph_timeout: Duration,
    pub fetch_connect_timeout: Duration,
    pub fetch_total_timeout: Duration,
    pub fetch_concurrency: usize,
}

impl Default for GraphResolverConfig {
    fn default() -> Self {
        Self {
            go_mod_graph_timeout: Duration::from_secs(30), // FR-007
            fetch_connect_timeout: Duration::from_secs(10), // FR-008
            fetch_total_timeout: Duration::from_secs(30),  // FR-008
            fetch_concurrency: 16,                          // FR-008a
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum GraphResolverError {
    #[error("workspace go.sum missing or unreadable: {0}")]
    GoSumMissing(#[source] std::io::Error),

    #[error("workspace go.mod missing or unreadable: {0}")]
    GoModMissing(#[source] std::io::Error),
}

// --------------------------------------------------------------------
// Resolver
// --------------------------------------------------------------------

/// 4-step ladder orchestrator. `resolve()` is the single public entry
/// point and is consumed by `legacy::read()` once per scan.
///
/// In milestone 055, the body of `resolve()` is split into private
/// step-N functions implemented across this file and the sibling
/// `proxy_fetch` / `go_mod_graph` modules. The orchestration is
/// implemented incrementally by tasks T021–T024 (US1) and T031–T033 (US2).
pub struct GraphResolver {
    config: GraphResolverConfig,
}

impl GraphResolver {
    pub fn new(config: GraphResolverConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &GraphResolverConfig {
        &self.config
    }

    // The `resolve()` body is built up across tasks T021–T024 and T031–T033.
    // Stub left here so the type is constructible during foundational
    // scaffolding without panicking unreachable code.
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    #[test]
    fn ladder_summary_renders_canonical_format() {
        // FR-009: exact tracing line format.
        let s = LadderSummary {
            graph_count: 12,
            cache_count: 3,
            proxy_count: 27,
            missing_count: 1,
            ..Default::default()
        };
        assert_eq!(
            s.to_string(),
            "go transitive edges: ladder=[graph:12, cache:3, proxy:27, missing:1]"
        );
    }

    #[test]
    fn error_class_has_stable_string_repr() {
        // Used as a HashMap key in LadderSummary.fetch_errors and as the
        // `error_class` field in tracing::warn — stability matters.
        assert_eq!(ErrorClass::Timeout.as_str(), "timeout");
        assert_eq!(ErrorClass::Http404.as_str(), "http_404");
        assert_eq!(ErrorClass::Http4xx.as_str(), "http_4xx");
        assert_eq!(ErrorClass::Http5xx.as_str(), "http_5xx");
        assert_eq!(ErrorClass::Dns.as_str(), "dns");
        assert_eq!(ErrorClass::Connection.as_str(), "connection");
        assert_eq!(ErrorClass::Tls.as_str(), "tls");
        assert_eq!(ErrorClass::Parse.as_str(), "parse");
        assert_eq!(ErrorClass::Other.as_str(), "other");
    }

    #[test]
    fn resolver_config_defaults_match_spec() {
        // FR-007, FR-008, FR-008a hard-coded values.
        let cfg = GraphResolverConfig::default();
        assert_eq!(cfg.go_mod_graph_timeout, Duration::from_secs(30));
        assert_eq!(cfg.fetch_connect_timeout, Duration::from_secs(10));
        assert_eq!(cfg.fetch_total_timeout, Duration::from_secs(30));
        assert_eq!(cfg.fetch_concurrency, 16);
    }

    #[test]
    fn module_graph_map_default_is_empty() {
        let m = ModuleGraphMap::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
        assert_eq!(m.summary().graph_count, 0);
    }

    #[test]
    fn module_graph_map_requires_returns_empty_for_unknown() {
        let m = ModuleGraphMap::new();
        let unknown = ModuleId::new("github.com/never/seen", "v0.0.0");
        assert!(m.requires(&unknown).is_empty());
    }
}
