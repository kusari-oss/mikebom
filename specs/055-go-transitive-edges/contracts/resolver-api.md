# Contract: GraphResolver internal API

**Spec**: [../spec.md](../spec.md) · **Plan**: [../plan.md](../plan.md) · **Data Model**: [../data-model.md](../data-model.md)

This is mikebom's only "interface contract" for milestone 055. There is no external API surface change — no new CLI flag, no new SBOM field, no new public Rust API. The `dependsOn` channel is unchanged (CDX `dependencies[]`, SPDX 2.3 `relationships[type=DEPENDS_ON]`, SPDX 3 `relationship[type=dependsOn]`). The contract below is internal to `mikebom-cli` and exists to keep the resolver's pieces composable and testable.

---

## `GraphResolver` — top-level resolver

```rust
// mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs

pub struct GraphResolver { /* private */ }

impl GraphResolver {
    /// Construct a resolver. Inexpensive; mostly stores config.
    pub fn new(config: ResolverConfig) -> Self;

    /// Resolve the module graph for a workspace. Single async entry point.
    /// Performs steps 1-4 of the ladder per spec FR-002.
    pub async fn resolve(
        &self,
        ctx: &WorkspaceContext,
    ) -> Result<ModuleGraphMap, GraphResolverError>;
}

pub struct ResolverConfig {
    pub go_mod_graph_timeout: Duration,   // default: 30s (FR-007)
    pub fetch_connect_timeout: Duration,  // default: 10s (FR-008)
    pub fetch_total_timeout: Duration,    // default: 30s (FR-008)
    pub fetch_concurrency: usize,         // default: 16 (FR-008a)
}

#[derive(thiserror::Error, Debug)]
pub enum GraphResolverError {
    #[error("workspace go.sum missing or unreadable: {0}")]
    GoSumMissing(#[source] std::io::Error),

    #[error("workspace go.mod missing or unreadable: {0}")]
    GoModMissing(#[source] std::io::Error),
    // No other variants — every fetch / subprocess / parse failure is internal,
    // logged via tracing, and falls through. The resolver itself only errors
    // on missing-input contracts.
}
```

**Contract**:
- `resolve()` MUST return `Ok` if `go.sum` and `go.mod` are readable in `ctx.root_dir`, regardless of which ladder steps succeed.
- `resolve()` MUST honor `ctx.offline`: when true, NO subprocess invocations and NO HTTP calls are made.
- `resolve()` MUST emit exactly one `tracing::info` summary line per invocation, naming the LadderSummary counts.
- `resolve()` is `Send + Sync`-safe — internal state is owned by the function or scoped via `Arc`.

---

## Step 1: `go mod graph` invocation

```rust
// mikebom-cli/src/scan_fs/package_db/golang/go_mod_graph.rs

pub async fn run_go_mod_graph(
    workspace_root: &Path,
    timeout: Duration,
) -> StepResult<HashMap<ModuleId, Vec<ModuleId>>>;

pub enum StepResult<T> {
    /// Step succeeded; data attached.
    Ok(T),
    /// Step is unavailable (precondition not met) — fall through silently.
    /// e.g., `go` not on PATH, `--offline` set.
    Unavailable,
    /// Step attempted and failed — fall through with warn.
    Failed(StepError),
}

pub struct StepError {
    pub class: ErrorClass,    // R14 taxonomy
    pub detail: String,       // human-readable for logs
}
```

**Contract**:
- `Ok(map)`: keys cover the workspace root + every transitive module. Values are the parent's direct requires.
- `Unavailable`: returned when `go version` probe fails OR `--offline` is set.
- `Failed`: returned when `go mod graph` exited non-zero, hit the timeout, or produced unparseable output.
- MUST NOT panic on any output; malformed lines are logged at `tracing::debug` and skipped.

**Output format parsing**: each non-empty line is `parent[@version] child@version` (whitespace-separated). The main-module's parent has no `@version`. Ambiguity: a few `go mod graph` versions emit additional whitespace in pseudo-version timestamps; the parser splits on `\s+` and validates exactly 2 fields after split.

---

## Step 2: `$GOMODCACHE` walk

```rust
// mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs (private fn)

async fn step2_cache_walk(
    target: &ModuleId,
    gomodcache: &Path,
) -> StepResult<Vec<ModuleId>>;
```

**Contract**:
- Returns the module's direct `requires` (post-replace, pre-intersection) if a cached `go.mod` exists.
- `Unavailable`: returned when `gomodcache` directory doesn't exist.
- `Failed`: returned for I/O errors or `go.mod` parse errors.

**Reuse**: this is a thin async wrapper over the existing `cache_lookup_depends()` logic at `golang.rs:570`. The 053 implementation is preserved as the body; only the call site changes (now invoked from `GraphResolver::step2_for_each_missing()`).

---

## Step 3: Proxy fetch

```rust
// mikebom-cli/src/scan_fs/package_db/golang/proxy_fetch.rs

pub async fn fetch_module_mod(
    client: &reqwest::Client,
    proxy_chain: &ProxyChain,
    target: &ModuleId,
) -> StepResult<String>;  // returns raw go.mod body

/// Public for unit testing.
pub fn escape_module_path(path: &str) -> Result<String, EscapeError>;

/// Public for unit testing.
pub fn build_proxy_url(
    proxy_base: &Url,
    target: &ModuleId,
) -> Result<Url, EscapeError>;
```

**Contract for `fetch_module_mod`**:
- Walks the `proxy_chain` per R7 separator semantics (`,` falls through on 404 only; `|` falls through on any error).
- `Ok(body)`: the body is a valid UTF-8 string; SHA-256 verification against `go.sum` is NOT performed in 055 (deferred per FR-004).
- `Unavailable`: returned when `proxy_chain` is `Off` or starts with `Direct`, OR when called for a `GOPRIVATE`-matched module.
- `Failed(StepError { class, ... })`: classified per R14.

**Contract for `escape_module_path`**:
- Pure function; no I/O.
- Lowercase ASCII, digits, `.-_~/+` pass through unchanged.
- Uppercase ASCII `X` → `!x`.
- Any other byte → `Err(EscapeError::InvalidByte { byte, position })`.

---

## Step 4: Empty entry insertion (no-edges fallthrough)

No public function — handled inline in `GraphResolver::resolve()`. After steps 1–3, any `ModuleId` in `ctx.go_sum_modules` not yet in the `entries` map gets a `ModuleGraphEntry { requires: vec![], source: ResolutionStep::None }`. The `LadderSummary::missing_count` is incremented per such entry.

---

## Intersection filter

```rust
// mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs (private fn)

fn intersect_with_go_sum(
    entries: &mut HashMap<ModuleId, ModuleGraphEntry>,
    go_sum_modules: &HashSet<ModuleId>,
);
```

**Contract**:
- Mutates each entry's `requires` to drop any element not in `go_sum_modules`.
- Implements FR-003 (go.sum is canonical).
- Logging: each dropped require emits `tracing::debug` with parent + dropped child (high-volume, debug-level only).

---

## Replace application

```rust
fn apply_replaces(
    entries: &mut HashMap<ModuleId, ModuleGraphEntry>,
    replaces: &HashMap<ModuleId, ModuleId>,
);
```

**Contract**:
- For every edge `(parent, child)` where `child` is replaced, rewrite `child` to its replacement target.
- Workspace-level replaces ONLY (R12); transitive modules' own replaces are ignored, matching Go semantics.
- Implements FR-006.

---

## `GOPRIVATE` parser

```rust
// mikebom-cli/src/scan_fs/package_db/golang/goprivate.rs

pub fn parse_private_patterns(env_value: &str) -> PrivatePatterns;

impl PrivatePatterns {
    pub fn matches(&self, module_path: &str) -> bool;
}
```

**Contract for `parse_private_patterns`**:
- Empty input → empty `PrivatePatterns` (matches no module).
- Comma-separated entries.
- Each entry: split on `/`; `*` in a segment becomes a glob.
- MUST NOT panic on malformed input — bad patterns become non-matchers (fail-open is acceptable here because the consequence of a bad pattern is "module fetched from public proxy when user wanted privacy" — but mikebom emits `tracing::warn` for any pattern that fails to parse, so the user can fix their env).

**Contract for `matches`**:
- Returns true if ANY pattern matches `module_path`.
- Pure function; const-time per pattern.

---

## `GOPROXY` parser

```rust
// mikebom-cli/src/scan_fs/package_db/golang/goprivate.rs (same module — env var parsing is grouped)

pub fn parse_proxy_chain(env_value: Option<&str>) -> Result<ProxyChain, ProxyParseError>;
```

**Contract**:
- `None` or empty input → default chain `[Url(https://proxy.golang.org, fall_through_on_404_only=true), Direct]`.
- `"off"` → `ProxyChain { entries: vec![Off] }`.
- `"direct"` → `ProxyChain { entries: vec![Direct] }`.
- Mixed: `"https://internal,direct"` → `[Url(internal, ,), Direct]`.
- Bad URL → `Err(ProxyParseError::InvalidUrl)`.
- The default scheme is HTTPS; `http://` URLs MUST emit `tracing::warn` once per scan (insecure proxy, leaks module list in cleartext).

---

## Test hooks

The resolver MUST be testable without real network. To support FR-011/FR-012:

- `ResolverConfig` exposes timeout fields so tests can use 100 ms timeouts for fast failure.
- `WorkspaceContext::goproxy` accepts arbitrary URLs — tests point it at `wiremock::MockServer::uri()`.
- `WorkspaceContext::gomodcache` accepts arbitrary `PathBuf` — tests point it at empty `tempfile::tempdir()`.
- Step 1's `go` discovery can be forced "unavailable" by pointing `PATH` at an empty directory or by injecting a `step1_disabled` field on `ResolverConfig` for unit tests.

---

## Non-contract: SBOM output

**Restated for clarity (FR-010)**:

- 055 does NOT add new SBOM fields, types, or properties.
- 055 does NOT introduce any `mikebom:*` annotations.
- 055 ONLY populates the existing `PackageDbEntry::depends: Vec<String>` field.
- The downstream emitters (CDX, SPDX 2.3, SPDX 3) consume `depends` exactly as they do today (post-053).

If reviewers later determine that per-edge provenance is required (Constitution Principle XII Constraint #2 strict reading — currently flagged in research.md R8 as resolved-by-precedent), that work is a separate spec. 055's contract is data-population only.
