# Phase 1 Data Model: Go transitive dependency edges

**Spec**: [spec.md](spec.md) · **Plan**: [plan.md](plan.md) · **Research**: [research.md](research.md)

Internal data structures for the resolver. None of these are SBOM-output types — output flows through the existing `PackageDbEntry::depends: Vec<String>` field per spec FR-010.

---

## `ModuleId`

Newtype wrapper for a `(module-path, version)` pair. Replaces ad-hoc `(String, String)` tuples per Constitution Principle IV (Type-Driven Correctness).

```rust
// mikebom-cli/src/scan_fs/package_db/golang/module_id.rs
#[derive(Clone, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub struct ModuleId {
    path: String,     // e.g., "github.com/Azure/azure-sdk-for-go"
    version: String,  // e.g., "v1.2.3", "v0.0.0-20211123-abcd1234abcd"
}

impl ModuleId {
    pub fn new(path: impl Into<String>, version: impl Into<String>) -> Self;
    pub fn path(&self) -> &str;
    pub fn version(&self) -> &str;
}

impl Display for ModuleId {
    // "<path>@<version>" — matches go.mod / go mod graph format
}
```

**Validation**: `path` MUST be a non-empty string with no whitespace; `version` MUST be either `vMAJOR.MINOR.PATCH[+SUFFIX]` or a Go pseudo-version (`v0.0.0-DATETIME-HASH`). Validation is `debug_assert!`-only (panics in debug, no-op in release) — the inputs come from already-parsed `go.sum` entries and `go.mod` files, so structural invariants are upstream-enforced.

**Lifecycle**: Created from parsing `go.sum` entries; cloned cheaply via `String` Clone. No mutation after construction.

---

## `ModuleGraphEntry`

The resolver's per-module record: a module-id, the list of modules it requires (after replace/exclude applied), and which ladder step supplied this data.

```rust
#[derive(Clone, Debug)]
pub struct ModuleGraphEntry {
    pub module: ModuleId,
    pub requires: Vec<ModuleId>,
    pub source: ResolutionStep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionStep {
    GoModGraph,   // step 1
    GoModCache,   // step 2
    Proxy,        // step 3
    None,         // step 4 (fallthrough; `requires` is empty)
}
```

**Validation**: `requires` is post-`go.sum`-intersection (FR-003) — every entry in `requires` MUST appear in the workspace's `go.sum`. Enforcement: the resolver applies the intersection filter as the last step before returning. Anything that violates this invariant is a resolver bug.

**Lifecycle**: Built once per scan, immutable after population.

---

## `ModuleGraphMap`

Top-level data structure returned by the resolver. Keyed by `ModuleId`; consulted by `golang.rs::read()` to populate each `PackageDbEntry::depends`.

```rust
pub struct ModuleGraphMap {
    entries: HashMap<ModuleId, ModuleGraphEntry>,
    summary: LadderSummary,
}

impl ModuleGraphMap {
    pub fn requires(&self, m: &ModuleId) -> &[ModuleId];
    pub fn summary(&self) -> &LadderSummary;
}
```

**Validation**: `entries` MUST cover every `(module, version)` pair from the workspace's `go.sum` (each gets either populated requires or an empty-requires `ResolutionStep::None` entry). Empty entries are a feature: a component with no edges is still a representable state.

**Lifecycle**: Constructed by `GraphResolver::resolve()`, consumed by `golang.rs::read()`, dropped at end of scan.

---

## `LadderSummary`

The data behind FR-009's per-scan `tracing::info` summary line.

```rust
#[derive(Clone, Debug, Default)]
pub struct LadderSummary {
    pub graph_count: usize,    // step 1 contributed N entries
    pub cache_count: usize,    // step 2 contributed N entries
    pub proxy_count: usize,    // step 3 contributed N entries
    pub missing_count: usize,  // step 4 fall-through count
    pub fetch_errors: HashMap<String, usize>,  // error_class -> count, per R14
}
```

**Display**: `format!("go transitive edges: ladder=[graph:{}, cache:{}, proxy:{}, missing:{}]", g, c, p, m)`. Per FR-009, emit at `tracing::info` once per scan that exercised the ladder.

**Lifecycle**: Mutated during resolution; flushed to a tracing event at scan end.

---

## `WorkspaceContext`

Inputs the resolver needs from the workspace. Built by `golang.rs::read()` before invoking the resolver.

```rust
pub struct WorkspaceContext {
    pub root_dir: PathBuf,                  // workspace root (where go.mod / go.sum live)
    pub go_sum_modules: HashSet<ModuleId>,  // canonical module set; intersection filter target
    pub replaces: HashMap<ModuleId, ModuleId>,  // workspace replace map (parsed from workspace go.mod)
    pub excludes: HashSet<ModuleId>,        // workspace exclude set (parsed from workspace go.mod)
    pub offline: bool,                      // global --offline flag
    pub gomodcache: PathBuf,                // resolved $GOMODCACHE (or $GOPATH/pkg/mod, or default)
    pub goproxy: ProxyChain,                // parsed $GOPROXY (R7)
    pub goprivate: PrivatePatterns,         // parsed $GOPRIVATE (R5)
}
```

**Validation**: `root_dir` MUST exist and contain `go.mod` + `go.sum`. Other fields parsed from environment / files; defaults documented in R7 / R5.

**Lifecycle**: Built once per scan, passed by reference to the resolver.

---

## `ProxyChain`

Parsed `$GOPROXY` value per R7.

```rust
pub struct ProxyChain {
    entries: Vec<ProxyEntry>,
}

pub enum ProxyEntry {
    Url { url: Url, fall_through_on_404_only: bool },  // separator was `,` (true) or `|` (false before this entry)
    Direct,  // out of scope for 055; treated as terminator
    Off,     // disable step 3 entirely
}
```

**Validation**: `Off` MUST appear alone (not chained). Multiple `Url` entries permitted (corporate proxy → public proxy fallback chain).

---

## `PrivatePatterns`

Parsed `$GOPRIVATE` value per R5.

```rust
pub struct PrivatePatterns {
    patterns: Vec<PrivatePattern>,
}

pub struct PrivatePattern {
    segments: Vec<PatternSegment>,
}

pub enum PatternSegment {
    Literal(String),  // exact match
    Glob(String),     // `*` matches any chars except `/`; pre-compiled to regex or kept as raw
}

impl PrivatePatterns {
    pub fn matches(&self, module_path: &str) -> bool;
}
```

**Validation**: An empty `patterns` vec means no module is private (no `GOPRIVATE` env or empty value).

---

## State transitions

The resolver is functionally pure: given a `WorkspaceContext`, it returns a `ModuleGraphMap`. No persistent state; nothing crosses scan boundaries.

Within a single resolution:

```text
WorkspaceContext  --[GraphResolver::resolve()]-->  ModuleGraphMap

internal sequence:
  1. probe go on PATH (R4) -> step1_available: bool
  2. if step1_available && !ctx.offline:
        run `go mod graph` (R3) -> populate entries from output
  3. for each ModuleId in ctx.go_sum_modules NOT yet in entries:
        try cache walk (existing cache_lookup_depends)
        if hit: insert as ResolutionStep::GoModCache
  4. for each ModuleId in ctx.go_sum_modules NOT yet in entries
     AND !ctx.offline AND !goprivate.matches(module.path)
     AND ctx.goproxy != Off:
        async-fetch via proxy_fetch (subject to 16-way semaphore)
        if hit: insert as ResolutionStep::Proxy
        if all proxy entries fail: leave as ResolutionStep::None
  5. for each remaining ModuleId in ctx.go_sum_modules:
        insert empty entry as ResolutionStep::None
  6. apply intersection filter: drop any require not in ctx.go_sum_modules
  7. apply ctx.replaces to every (parent, child) edge
     (workspace-level replaces only; transitive module replaces ignored per Go semantics, R12)
  8. emit summary tracing::info line
  9. return ModuleGraphMap
```

---

## Relationships to existing types

| Existing type | Where | Relationship to 055 |
|---------------|-------|---------------------|
| `GoModDocument` | `golang.rs:159` | Reused as-is for parsing fetched `.mod` bodies (R13) |
| `GoSumEntry` | `golang.rs:333` | Source of `WorkspaceContext::go_sum_modules` |
| `GoModRequire` | (sibling of `GoModDocument`) | Source of `ModuleGraphEntry::requires` |
| `PackageDbEntry::depends: Vec<String>` | (existing) | Output target — populated from `ModuleGraphMap::requires()` lookup |
| `cache_lookup_depends()` | `golang.rs:570` | **Deleted** in 055 — replaced by `GraphResolver` step 2; the cache-walk logic moves into `golang/proxy_fetch.rs` (no, sorry — into `golang/graph_resolver.rs::step2_cache`). The 053 codepath is preserved as a private helper called only from step 2. |
| `apply_replace_and_exclude()` | `golang.rs:475` | Reused — the resolver calls it once per scan to build `WorkspaceContext::replaces` + `excludes` |
| `build_main_module_entry()` | `golang.rs:610` | **Unchanged** — milestone 053's main-module emission path is independent of the transitive resolver. The main-module's direct edges are still built from the workspace `go.mod`'s `requires`, then merged into the same `ModuleGraphMap` in step 0 (before step 1). |

---

## Out-of-scope data structures

These would be needed for follow-up milestones, NOT for 055:

- `GoWorkDocument` — for `go.work` workspace-mode resolution (out of scope).
- `VendorModulesTxt` — for vendor-mode resolution (out of scope).
- `EdgeProvenance` — per-edge SBOM `evidence` annotation (deferred per R8).
- `SourceVcsClient` — for `GOPROXY=direct` source-code fetches (out of scope).
- `ModHashVerifier` — for verifying proxy-fetched `.mod` files against `go.sum`'s `h1:` hashes (out of scope per FR-004).
