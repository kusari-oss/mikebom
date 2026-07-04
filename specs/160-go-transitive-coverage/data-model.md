# Data Model: Milestone 160 (Go transitive-edge coverage)

**Date**: 2026-07-04
**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Research**: [research.md](./research.md)

Phase-1 entity + type inventory. All entities are Rust types in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs` unless otherwise noted; wire-shape entities are per-format JSON constructs described in `contracts/annotations.md`.

## Rust types

### E1 — `ResolutionStep` (EXTENDED, existing enum)

**Location**: `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs:63`

Existing enum (unchanged). Milestone 160 adds only a new method:

```rust
pub enum ResolutionStep {
    GoModGraph,
    GoModCache,
    Proxy,
    GoSumFallback,
    None,
}

impl ResolutionStep {
    /// NEW in milestone 160: kebab-case wire string for the
    /// `mikebom:go-transitive-source` annotation value.
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::GoModGraph      => "go-mod-graph",
            Self::GoModCache      => "module-cache",
            Self::Proxy           => "proxy-fetch",
            Self::GoSumFallback   => "go-sum-fallback",
            Self::None            => "unresolved",
        }
    }
}
```

**Fields**: unchanged. **Relationships**: consumed by `E2::from(&StepError)` and by the emission code at `legacy.rs::read()` per R2.

**Validation rules**: none — enum is closed. Wire strings match the C108 parity-row expected values exactly.

### E2 — `UnresolvedReasonClass` (NEW enum)

**Location**: NEW type in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`

```rust
pub enum UnresolvedReasonClass {
    ProxyFetchTimeout,
    ProxyFetchNotFound,
    ProxyFetchForbidden,
    ProxyOffInChain,
    GoPrivateMatched,
    ModuleCacheMiss,
    UnknownError,
}

impl UnresolvedReasonClass {
    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::ProxyFetchTimeout    => "proxy-fetch-timeout",
            Self::ProxyFetchNotFound   => "proxy-fetch-not-found",
            Self::ProxyFetchForbidden  => "proxy-fetch-forbidden",
            Self::ProxyOffInChain      => "proxy-off-in-chain",
            Self::GoPrivateMatched     => "goprivate-matched",
            Self::ModuleCacheMiss      => "module-cache-miss",
            Self::UnknownError         => "unknown-error",
        }
    }
}

impl From<&StepError> for UnresolvedReasonClass {
    fn from(err: &StepError) -> Self {
        match err.class {
            ErrorClass::Timeout                       => Self::ProxyFetchTimeout,
            ErrorClass::Http404                       => Self::ProxyFetchNotFound,
            ErrorClass::Http4xx if err.is_forbidden() => Self::ProxyFetchForbidden,
            ErrorClass::Http4xx                       => Self::ProxyFetchNotFound,
            ErrorClass::Http5xx                       => Self::ProxyFetchTimeout,
            ErrorClass::Dns
            | ErrorClass::Connection
            | ErrorClass::Tls                         => Self::ProxyFetchTimeout,
            ErrorClass::Parse
            | ErrorClass::Other                       => Self::UnknownError,
        }
    }
}
```

**Fields**: enum-only. **Relationships**: mapped-from `StepError` at `graph_resolver.rs:282`; consumed by `legacy.rs::read()`'s emission code to populate the C109 per-component annotation.

**Validation rules**: closed 7-code vocabulary per FR-003. `is_forbidden()` on `StepError` is a small helper checking `class == Http4xx && detail.contains("403")` (implementation detail, single line).

### E3 — `GoTransitiveCoverage` (NEW enum)

**Location**: NEW type in `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs`

```rust
pub enum GoTransitiveCoverage {
    Complete,
    Partial(String),  // reason detail per FR-005 grammar
    Unknown(String),  // reason detail per FR-005 grammar
}

impl GoTransitiveCoverage {
    pub fn value_wire_str(&self) -> &'static str {
        match self {
            Self::Complete    => "complete",
            Self::Partial(_)  => "partial",
            Self::Unknown(_)  => "unknown",
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Complete           => None,
            Self::Partial(r)         => Some(r.as_str()),
            Self::Unknown(r)         => Some(r.as_str()),
        }
    }
}
```

**Fields**: enum with variant-attached string reason. **Relationships**: produced by `compute_coverage()` (R4); consumed by `scan_cmd.rs`'s doc-scope emission code.

**Validation rules**: `Partial` and `Unknown` variants MUST carry a non-empty reason string. Reason strings MUST follow FR-005 grammar `<code>: <detail>[; <code>: <detail>]*` where `<code>` is one of the closed 5-code vocab: `proxy-fetch-degraded`, `offline-mode`, `goproxy-off-in-chain`, `go-mod-graph-degraded`, `module-cache-empty-and-no-proxy`.

### E4 — `LadderSummary` (EXTENDED, existing struct)

**Location**: `mikebom-cli/src/scan_fs/package_db/golang/graph_resolver.rs:222`

Existing struct. Milestone 160 adds ONE field:

```rust
pub struct LadderSummary {
    pub graph_count: usize,
    pub cache_count: usize,
    pub proxy_count: usize,
    pub gosum_fallback_count: usize,
    pub missing_count: usize,
    pub fetch_errors: HashMap<String, usize>,
    /// NEW in milestone 160: true iff step 1 (`go mod graph` subprocess)
    /// either failed to launch OR returned partial output that the
    /// parser rejected. Populated by go_mod_graph.rs on error paths.
    /// Feeds compute_coverage()'s Q1 caution-first `unknown` gate.
    pub go_mod_graph_degraded: bool,
}

impl LadderSummary {
    /// NEW in milestone 160: total modules covered by the ladder
    /// (denominator for the FR-005 `<N> of <M>` reason detail).
    pub fn total_modules(&self) -> usize {
        self.graph_count + self.cache_count + self.proxy_count
            + self.gosum_fallback_count + self.missing_count
    }
}
```

**Fields**: one new bool + one derived helper. **Relationships**: populated by `GraphResolver::resolve()` at `graph_resolver.rs:397`; consumed by `compute_coverage()`.

### E5 — `ScanDiagnostics.go_transitive_coverage` (NEW field)

**Location**: `mikebom-cli/src/scan_fs/mod.rs:80` (in the existing `ScanDiagnostics` struct)

New field, sibling to the existing `go_graph_completeness` field at `scan_fs/mod.rs:94`:

```rust
pub struct ScanDiagnostics {
    // ... existing fields ...
    pub go_graph_completeness: Option<GraphCompleteness>,  // existing (C104)
    pub go_graph_completeness_reason: Option<String>,     // existing (C105)

    /// NEW in milestone 160: per-Q1 reason-code-driven signal reflecting
    /// the outcome of the milestone-055/091 transitive-edge ladder across
    /// all Go modules in the scan. See R1 for the semantic distinction
    /// vs `go_graph_completeness`.
    pub go_transitive_coverage: Option<GoTransitiveCoverage>,
}
```

Reused by CLI emission at `scan_cmd.rs` (near line 2585 where the existing `go_graph_completeness` is emitted; milestone 160's addition is a sibling emission block).

## Wire types

### W1 — `mikebom:go-transitive-source` (C108, per-component)

**Wire format**: raw kebab-case string. Value ∈ `{"go-mod-graph", "module-cache", "proxy-fetch", "go-sum-fallback", "unresolved"}`.

**Universality (per Q2)**: Emitted on EVERY Go component (`purl.starts_with("pkg:golang/")`).

**Per-format shape**: see `contracts/annotations.md` §C108.

### W2 — `mikebom:go-transitive-unresolved-reason` (C109, per-component, conditional)

**Wire format**: raw kebab-case string. Value ∈ 7 codes per E2.

**Conditional emission**: iff C108 value == `"unresolved"`.

**Per-format shape**: see `contracts/annotations.md` §C109.

### W3 — `mikebom:go-transitive-coverage` (C110, document-scope)

**Wire format**: raw string ∈ `{"complete", "partial", "unknown"}`.

**Universality**: Emitted iff the SBOM contains ≥1 Go component. When no Go components present, annotation is entirely absent.

**Per-format shape**: see `contracts/annotations.md` §C110.

### W4 — `mikebom:go-transitive-coverage-reason` (C111, document-scope, conditional)

**Wire format**: FR-005 grammar `<code>: <detail>[; <code>: <detail>]*`. Codes ∈ closed 5-code vocab per FR-005 (`proxy-fetch-degraded`, `offline-mode`, `goproxy-off-in-chain`, `go-mod-graph-degraded`, `module-cache-empty-and-no-proxy`).

**Conditional emission**: iff C110 value ∈ `{"partial", "unknown"}`.

**Per-format shape**: see `contracts/annotations.md` §C111.

## Relationships

```text
GraphResolver::resolve()
     │
     ├── produces → ModuleGraphMap (existing)
     │              └── entries[ModuleId] → ModuleGraphEntry {source: ResolutionStep}
     │
     └── produces → LadderSummary (extended: + go_mod_graph_degraded)
                    │
                    └── consumed by → compute_coverage(summary, ctx) → GoTransitiveCoverage
                                       │
                                       └── stored in → ScanDiagnostics.go_transitive_coverage
                                                       │
                                                       └── emitted by → cli/scan_cmd.rs
                                                                        │
                                                                        ├── C110 (doc-scope annotation)
                                                                        └── C111 (conditional reason annotation)

legacy::read() (per Go module)
     │
     └── consumes → ModuleGraphMap.entry(module_id).source
                    │
                    └── ResolutionStep::as_wire_str() → PackageDbEntry.extra_annotations[C108]

     iff source == ResolutionStep::None:
         └── UnresolvedReasonClass::from(&step_error).as_wire_str()
             → PackageDbEntry.extra_annotations[C109]
```

## State transitions

**`GoTransitiveCoverage` value determination** (per R4):

```text
Input: LadderSummary, WorkspaceContext

Step 1 (Unknown-fires-first per Q1 caution-first):
  ctx.offline                    → Unknown("offline-mode: ...")
  ctx.goproxy.contains_off()     → Unknown("goproxy-off-in-chain: ...")
  summary.go_mod_graph_degraded  → Unknown("go-mod-graph-degraded: ...")

Step 2 (Partial-fires-second):
  summary.missing_count > 0      → Partial("proxy-fetch-degraded: N of M modules unresolved")

Step 3 (Complete):
  else                           → Complete
```

**Idempotent**: same inputs always produce same output. No hidden state.

## Data volume assumptions

- **Typical Go closure**: 100–500 modules per scan (milestones 090 fixture: `test-podman` = ~300 modules).
- **Per-component annotation budget**: ~300 × 2 annotations (C108 universal + C109 conditional on 5-15% of modules) = ~360 annotations per test-podman scan. Milestone-158 precedent shows this is well within perf budget.
- **Reason-string length**: bounded by fixed prefix vocabulary + `<N> of <M>` count → ≤120 chars in worst case. No unbounded growth.

## Validation rules (aggregated)

| Rule | Enforcement |
|------|-------------|
| C108 value is one of 5 kebab-case codes | Enum-backed at emission (`ResolutionStep::as_wire_str()`); parity-catalog `Directionality::SymmetricEqual` verifies at test time. |
| C109 emitted iff C108 == `"unresolved"` | Guarded by `if entry.source == ResolutionStep::None` at emission site. |
| C110 emitted iff scan contains ≥1 Go component | Guarded by `if go_component_count > 0` in `scan_cmd.rs` doc-scope emission block. |
| C111 emitted iff C110 value ∈ `{partial, unknown}` | Guarded by `if coverage.reason().is_some()` at emission site. |
| Reason strings follow FR-005 grammar | Enforced by construction in `compute_coverage()`; unit-tested (SC-008). |
| Reason codes are within closed 5-code vocab | Enforced by `compute_coverage()` construction; extension requires spec-milestone bump per Q4. |
