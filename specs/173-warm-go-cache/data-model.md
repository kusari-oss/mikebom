# Phase 1 Data Model: Go Cache Warming (m173)

**Feature**: 173-warm-go-cache
**Date**: 2026-07-08

Six entities gain new state. Two new annotations (C118 + C119) get emitted. Two new CLI flags get parsed. Everything else is plumbing that mirrors m172's C117 wiring verbatim.

## Entity 1 — `CacheWarmingMode` (new enum)

**Location**: `mikebom-cli/src/scan_fs/package_db/golang/warm_cache.rs` (new module).

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheWarmingMode {
    /// Default. No warming performed. C117 will reflect whatever
    /// state the operator's env is in.
    Off,
    /// One `go mod download` invocation per discovered `go.mod`
    /// workspace before the transitive resolver runs.
    PerWorkspace,
    /// Internal-only variant. Set when the operator requested
    /// `--warm-go-cache=per-workspace` but `--offline` is also set.
    /// The warmer skips all work; the annotation surfaces the
    /// override for operator awareness (FR-003 + FR-011).
    OfflineInhibited,
}

impl CacheWarmingMode {
    /// Wire-string value for the FR-011 doc-scope annotation.
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::PerWorkspace => "per-workspace",
            Self::OfflineInhibited => "offline-inhibited",
        }
    }
}
```

**Flow**: Set once at CLI parse time from the `--warm-go-cache` flag (`Off` or `PerWorkspace`). If `--offline` is also set + user picked `PerWorkspace`, upgrade the mode to `OfflineInhibited` at the pipeline entry point (before the warmer would have run). The final mode value flows through `ScanResult` → `ScanArtifacts` → `SbomEmission` → emitters exactly like `CacheWarmingResult`.

## Entity 2 — `WarmingFailureReason` (new closed enum)

**Location**: `warm_cache.rs`.

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WarmingFailureReason {
    /// `go` binary not found on PATH.
    GoBinaryAbsent,
    /// `Command::new("go")` spawn failed (permission denied,
    /// executable-format error, etc.).
    SpawnFailed,
    /// The subprocess exceeded the per-workspace timeout budget.
    Timeout,
    /// The subprocess exited with a non-zero status. Distinguished
    /// from `ParseError` because the go command signaled the
    /// failure itself.
    SubcommandFailed,
    /// The subprocess exited zero but its output couldn't be
    /// interpreted (extremely rare for `go mod download`; kept in
    /// the enum for future subcommands that may emit structured
    /// output).
    ParseError,
    /// The overall wall-clock budget was exhausted before this
    /// workspace could be attempted.
    BudgetExhausted,
}
```

Six variants match the FR-007 closed-enum requirement. The `#[serde(rename_all = "kebab-case")]` derive produces exactly the strings the wire contract expects (`"go-binary-absent"`, etc.).

## Entity 3 — `CacheWarmingResult` (new struct)

**Location**: `warm_cache.rs`.

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheWarmingResult {
    /// The mode value the warmer operated under. Duplicates
    /// `ScanArtifacts.go_cache_warming_mode` for emitter convenience;
    /// stays in sync because the warmer receives the mode as input
    /// and returns it back out.
    pub mode: CacheWarmingMode,
    /// Per-workspace outcomes, sorted alphabetically by
    /// `workspace_path` for byte-identity across regenerations.
    /// Successful workspaces are omitted; the vec contains only
    /// failures per FR-007 (aggregating "which workspaces failed").
    pub failures: Vec<WorkspaceFailure>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkspaceFailure {
    /// Reason class from the closed enum. Declared FIRST so serde's
    /// default emission order (struct declaration order) produces
    /// alphabetical JSON: `{"reason":..., "workspace":...}` —
    /// matches contracts/annotation-wire-shapes.md byte-identity
    /// requirement.
    pub reason: WarmingFailureReason,
    /// Workspace path RELATIVE to the scan root (for portability
    /// across environments and byte-identity of goldens).
    pub workspace: String,
}
```

**Serialization**: The C119 annotation value is `serde_json::to_string(&result.failures).unwrap()` — a JSON-encoded array of `{workspace, reason}` records. C119 is emitted iff `!failures.is_empty()`; when the failures vec is empty, the C119 annotation is entirely absent (FR-007 emission gate).

## Entity 4 — `CacheWarmingConcurrency` (new type alias)

**Location**: `warm_cache.rs`.

Semantically a `usize` in the range `1..=32`. Sourced from `--warm-go-cache-concurrency` (default 4). Value `0` from the CLI resolves to `min(std::thread::available_parallelism().map(NonZeroUsize::get).unwrap_or(4), 8)` at runtime — never left as `0` downstream. Values > 32 are clamped with a warn-level log (FR-014 anti-typo defense).

```rust
pub fn effective_concurrency(raw: u32) -> usize {
    if raw == 0 {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        cpus.min(8)
    } else if raw > 32 {
        tracing::warn!(
            requested = raw,
            "--warm-go-cache-concurrency clamped to 32 (per FR-014)"
        );
        32
    } else {
        raw as usize
    }
}
```

## Entity 5 — plumbing structs (extended fields)

Same 4-hop chain as m172's C117:

**`GoScanSignals`** (`mikebom-cli/src/scan_fs/package_db/golang/legacy.rs` line ~1384):

```rust
pub struct GoScanSignals {
    // ...existing fields (main_modules, production_imports, etc.)

    /// Milestone 173: aggregated Go cache-warming outcome for this
    /// scan. `None` iff no Go scan happened OR warming mode was
    /// `Off` (no attempt made). `Some(_)` when the warmer ran or
    /// was `OfflineInhibited`. Feeds C118 + C119 annotations.
    pub cache_warming: Option<CacheWarmingResult>,
}
```

**`ScanDiagnostics`** (`mikebom-cli/src/scan_fs/package_db/mod.rs` line ~308):

```rust
pub struct ScanDiagnostics {
    // ...existing fields

    /// Milestone 173: doc-scope Go cache-warming outcome.
    /// Mirror of `GoScanSignals.cache_warming` propagated through
    /// the aggregator. Consumed by the format emitters for the
    /// C118/C119 annotations.
    pub go_cache_warming: Option<CacheWarmingResult>,
}
```

**`ScanResult`** (`mikebom-cli/src/scan_fs/mod.rs` line ~98):

```rust
pub struct ScanResult {
    // ...existing fields (go_transitive_coverage, go_transitive_fallback_count, ...)

    /// Milestone 173: Go cache-warming outcome. `None` iff no Go
    /// scan happened OR mode was `Off`. Feeds the C118 mode
    /// annotation (unconditional when Some) + C119 failed
    /// annotation (conditional on failures.is_empty()).
    pub go_cache_warming: Option<CacheWarmingResult>,
}
```

**`ScanArtifacts`** (`mikebom-cli/src/generate/mod.rs` line ~50):

```rust
pub struct ScanArtifacts<'a> {
    // ...existing fields

    /// Milestone 173: doc-scope Go cache-warming outcome for the
    /// C118/C119 emissions. `None` iff no Go scan happened OR mode
    /// was `Off` (i.e., annotation-emission is gated on
    /// `Some(_)` presence, matching m172 C117's Option-gate pattern).
    pub go_cache_warming: Option<&'a CacheWarmingResult>,
}
```

## Entity 6 — CDX 1.6 / SPDX 2.3 / SPDX 3 wire shapes (annotations C118 + C119)

**Location**: emitted SBOM `metadata.properties[]` (CDX) / document annotations (SPDX 2.3) / `@graph[]` typed Annotation (SPDX 3).

### C118 — `mikebom:go-cache-warming-mode` (unconditional per Go scan)

**CDX**:
```json
{ "name": "mikebom:go-cache-warming-mode", "value": "off" }
```

**SPDX 2.3** (`MikebomAnnotationCommentV1` envelope):
```json
{
  "annotationType": "OTHER",
  "annotator": "Tool: mikebom-0.1.0-alpha.NN",
  "annotationDate": "1970-01-01T00:00:00Z",
  "comment": "{\"schema\":\"mikebom-annotation/v1\",\"field\":\"mikebom:go-cache-warming-mode\",\"value\":\"off\"}"
}
```

**SPDX 3**:
```json
{
  "type": "Annotation",
  "spdxId": "urn:mikebom:annotation:<content-hash>",
  "subject": "<SpdxDocument-root-IRI>",
  "statement": "{\"schema\":\"mikebom-annotation/v1\",\"field\":\"mikebom:go-cache-warming-mode\",\"value\":\"off\"}",
  "annotationType": "other",
  "creationInfo": "_:creationInfo0"
}
```

**Emitted iff**: `SbomEmission.go_cache_warming.is_some()` (i.e., Go was scanned; `None` for non-Go scans).

**Value shape**: exact regex `^(off|per-workspace|offline-inhibited)$`.

### C119 — `mikebom:go-cache-warming-failed` (conditional)

**CDX**:
```json
{
  "name": "mikebom:go-cache-warming-failed",
  "value": "[{\"reason\":\"parse-error\",\"workspace\":\"cmd/bar\"},{\"reason\":\"timeout\",\"workspace\":\"cmd/foo\"}]"
}
```

**SPDX 2.3** and **SPDX 3**: same envelope shapes as C118 with the payload's `value` field carrying the JSON-encoded array.

**Emitted iff**: `SbomEmission.go_cache_warming.map(|r| !r.failures.is_empty()).unwrap_or(false)`.

**Value shape**: JSON-encoded array of `{reason, workspace}` records. Records sorted alphabetically by `workspace` for byte-identity. `reason` is one of six closed-enum kebab-case values.

## Entity 7 — CLI flag surface

**Location**: `mikebom-cli/src/cli/scan_cmd.rs` (extended).

### `--warm-go-cache <MODE>`

```rust
/// Milestone 173: opt-in Go cache-warming mode. `off` (default)
/// preserves the milestone-172 `mikebom:go-transitive-fallback-count`
/// annotation as a signal; `per-workspace` invokes `go mod download`
/// in every discovered Go workspace before the transitive resolver
/// runs, so step 1 (`go mod graph`) can find every module locally
/// and produce true parent-child topology. No-op when `--offline`
/// is set (see `mikebom:go-cache-warming-mode = "offline-inhibited"`).
#[arg(
    long,
    value_enum,
    default_value = "off",
    require_equals = true,
    num_args = 1
)]
pub warm_go_cache: WarmGoCacheMode,
```

The `require_equals = true` matches FR-010's parseability constraint (no boolean shorthand). `value_enum` provides the auto-generated `off` / `per-workspace` completion.

### `--warm-go-cache-concurrency <N>`

```rust
/// Milestone 173: maximum concurrent `go mod download` invocations
/// during cache warming. Default: 4. Set to 1 for sequential
/// warming (CI shared-runner friendly). Set to 0 for auto
/// (min(logical_cpus, 8)). Values above 32 are clamped with a
/// warn-level log. No-op when `--warm-go-cache=off` or in offline
/// mode. Rationale: matches the m055/m091 `fetch_concurrency = 16`
/// posture; monorepos are the motivating use case.
#[arg(long, default_value_t = 4)]
pub warm_go_cache_concurrency: u32,
```

## Entity 8 — `AdvisoryContext` (FR-004 predicate)

**Location**: `mikebom-cli/src/cli/scan_cmd.rs` (used at the emission-tail advisory-log site).

```rust
#[derive(Debug)]
struct AdvisoryContext {
    /// From the emitted SBOM's C117 annotation.
    fallback_count: Option<usize>,
    /// Detected via `clap::ArgMatches::value_source(...)` !=
    /// `Some(ValueSource::DefaultValue)`.
    warm_flag_was_default: bool,
    /// The scan's --offline flag value.
    offline: bool,
    /// Whether the scan produced any Go components.
    scan_has_go_components: bool,
}

impl AdvisoryContext {
    fn should_advise(&self) -> bool {
        self.scan_has_go_components
            && !self.offline
            && self.warm_flag_was_default
            && self.fallback_count.map(|n| n > 0).unwrap_or(false)
    }
}
```

Advisory log line (stable substring per SC-002 + US2 Independent Test):

> `mikebom:go-transitive-fallback-count > 0 detected. Prime the cache with --warm-go-cache=per-workspace or 'go mod download' per workspace before scanning.`

Emitted at `tracing::info!` level exactly once per scan.

## Entity 9 — Parity catalog rows C118 + C119

**Location**: `mikebom-cli/src/parity/extractors/mod.rs` EXTRACTORS table + `docs/reference/sbom-format-mapping.md` Section C.

Both rows are `Directionality::SymmetricEqual` and `order_sensitive: false`. Both are `document`-scope for the parity-extractor macros.

### `mod.rs` entries

```rust
ParityExtractor {
    row_id: "C118",
    label: "mikebom:go-cache-warming-mode",
    cdx: c118_cdx,
    spdx23: c118_spdx23,
    spdx3: c118_spdx3,
    directional: Directionality::SymmetricEqual,
    order_sensitive: false,
},
ParityExtractor {
    row_id: "C119",
    label: "mikebom:go-cache-warming-failed",
    cdx: c119_cdx,
    spdx23: c119_spdx23,
    spdx3: c119_spdx3,
    directional: Directionality::SymmetricEqual,
    order_sensitive: false,
},
```

### Per-format helper stubs

```rust
// cdx.rs
cdx_anno!(c118_cdx, "mikebom:go-cache-warming-mode",   document);
cdx_anno!(c119_cdx, "mikebom:go-cache-warming-failed", document);

// spdx2.rs
spdx23_anno!(c118_spdx23, "mikebom:go-cache-warming-mode",   document);
spdx23_anno!(c119_spdx23, "mikebom:go-cache-warming-failed", document);

// spdx3.rs
spdx3_anno!(c118_spdx3, "mikebom:go-cache-warming-mode",   document);
spdx3_anno!(c119_spdx3, "mikebom:go-cache-warming-failed", document);
```

## Cross-entity invariants (post-173)

1. **Presence gate mode → any-Go-scan**: `mikebom:go-cache-warming-mode` present iff scan has at least one Go component. Matches m172 C117 gating pattern.
2. **Presence gate failed → some-workspace-failed**: `mikebom:go-cache-warming-failed` present iff `CacheWarmingResult.failures` non-empty. When mode is `Off` (no attempt), failures is trivially empty; when mode is `PerWorkspace`, failures may or may not be empty. When mode is `OfflineInhibited`, no work attempted, no failures possible → C119 always absent.
3. **Advisory-log triviality**: FR-004's `should_advise()` predicate returns false if `scan_has_go_components == false` (via FR-009). No advisory log lines fire on non-Go scans.
4. **Mode/offline consistency**: When `mode == OfflineInhibited`, the `mikebom:go-cache-warming-mode` value is emitted as `"offline-inhibited"`, and no per-workspace `go mod download` invocation is attempted (FR-003 hard gate).
5. **Concurrency-value-invariance**: The concurrency setting does NOT affect the emitted annotations (mode is mode, failures are failures). Two scans that differ only in `--warm-go-cache-concurrency` MUST produce byte-identical SBOMs (holds because warming failures classify per workspace, and workspaces are enumerated deterministically).

## State transitions

None. All state is per-scan in-process; the warmer runs once at the reader's pre-resolver entry point and its result is either present (fed into emitters) or absent (mode was `Off`). No lifecycle events beyond process exit.
