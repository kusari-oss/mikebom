# Data Model: Resolver Trait + Chain (m209)

**Date**: 2026-07-18
**Purpose**: Enumerate the new types the refactor introduces + deltas to existing types. Every new struct / enum / trait has one entry.

## E1: `Resolver` trait (NEW)

**Location**: `mikebom-cli/src/resolve/resolver_trait.rs`.

**Shape**:

```rust
pub(crate) trait Resolver: Send + Sync {
    /// Stable identifier for logging + panic diagnostics. Snake-case,
    /// matches the entry in `RESOLVER_REGISTRY`. Never renamed after
    /// first release without a downstream-breaking-change note.
    fn name(&self) -> &'static str;

    /// Priority for chain ordering. Higher = runs earlier. MUST be
    /// unique across all registered resolvers per FR-017 (enforced
    /// at compile time via `RESOLVER_REGISTRY` const check).
    fn priority(&self) -> u32;

    /// Which technique this resolver reports via
    /// `ResolutionEvidence.technique` on emitted components. Preserves
    /// SC-005's downstream signal.
    fn technique(&self) -> ResolutionTechnique;

    /// Confidence attached to every component this resolver emits.
    /// Currently a per-resolver constant (matches pre-refactor); the
    /// interface accommodates future per-component confidence if
    /// needed by returning it from `resolve` instead.
    fn confidence(&self) -> f32;

    /// Cheap filter — returns `true` if this resolver applies to the
    /// given input type / shape. Called before `.await`ing `resolve`
    /// to skip clearly-inapplicable resolvers. Sync + O(1).
    fn handles(&self, input: &ResolveInput<'_>) -> bool;

    /// The actual resolution logic. Async to accommodate the deps.dev
    /// hash resolver's network call; sync resolvers' bodies never
    /// `.await` (compiler generates a no-op future).
    ///
    /// Per Q1 clarification: `Ok(Vec::new())` denotes a clean no-match
    /// (chain continues to the next resolver); `Err(...)` denotes a
    /// transient/internal failure (pipeline logs WARN + continues to
    /// the next resolver per FR-013).
    async fn resolve(
        &self,
        input: &ResolveInput<'_>,
        ctx: &ResolveContext<'_>,
    ) -> Result<Vec<ResolvedComponent>, ResolverError>;
}
```

**Validation rules**:
- `name()` MUST match one entry in `RESOLVER_REGISTRY` (assert at chain construction time).
- `priority()` MUST equal the priority declared in `RESOLVER_REGISTRY` for the matching name (assert at chain construction time).
- `technique()` MUST be one of the existing `ResolutionTechnique` variants OR a new variant added in the same PR (matches SC-005).
- Resolvers MUST be stateless per FR-014 — no mutable fields, no interior mutability.

## E2: `ResolveInput<'a>` (NEW)

**Location**: `mikebom-cli/src/resolve/resolver_trait.rs`.

**Shape**:

```rust
pub(crate) enum ResolveInput<'a> {
    /// A traced network connection. URL-family resolvers + the hash
    /// resolver + the hostname-fallback resolver consume this.
    Connection {
        connection: &'a Connection,
        /// Basename-to-content-hash correlation table built once per
        /// pipeline invocation from file-access events (per
        /// pipeline.rs:89 today). Passed by reference so resolvers
        /// don't need to rebuild it.
        basename_to_hash: &'a HashMap<&'a str, &'a ContentHash>,
    },
    /// A traced file-access operation. The path resolver consumes
    /// this (only ecosystem-neutral file-path resolution — the URL-
    /// family resolvers only see connections).
    FileOp(&'a FileAccessOperation),
}
```

## E3: `ResolveContext<'a>` (NEW)

**Location**: `mikebom-cli/src/resolve/resolver_trait.rs`.

**Shape**:

```rust
pub(crate) struct ResolveContext<'a> {
    /// Debian distro codename sampled from `/etc/os-release` (see
    /// pipeline.rs:74). Threaded to the Deb resolver as the PURL's
    /// `distro` qualifier; other resolvers ignore it. `None` on
    /// non-Debian hosts.
    pub deb_codename: Option<&'a str>,

    /// Whether the operator passed `--skip-purl-validation`.
    /// Consumed by the deps.dev-hash resolver's `handles()` — when
    /// true, that resolver returns `false` from `handles()` and is
    /// silently skipped for every input, preserving FR-011 semantics.
    pub skip_online_validation: bool,
}
```

## E4: `ResolverError` enum (NEW)

**Location**: `mikebom-cli/src/resolve/resolver_trait.rs`.

**Shape**:

```rust
#[derive(Debug, thiserror::Error)]
pub(crate) enum ResolverError {
    /// A transient network error occurred (deps.dev timeout,
    /// TCP reset, etc.). The pipeline logs WARN + continues.
    #[error("resolver `{resolver}` hit a transient network error: {source}")]
    Transient {
        resolver: &'static str,
        source: anyhow::Error,
    },

    /// The resolver's internal invariant was violated by the input
    /// (malformed URL, unexpected hash algorithm, etc.). The
    /// pipeline logs WARN + continues.
    #[error("resolver `{resolver}` rejected input as malformed: {reason}")]
    MalformedInput {
        resolver: &'static str,
        reason: String,
    },

    /// The resolver's dependency was unavailable (deps.dev
    /// unreachable). The pipeline logs WARN + continues; the
    /// operator sees enough context to know online-mode is degraded.
    #[error("resolver `{resolver}` dependency unavailable: {reason}")]
    Unavailable {
        resolver: &'static str,
        reason: String,
    },
}
```

**Validation rules**:
- Every variant carries the resolver `name` so the WARN log identifies the source.
- `Display` output is safe (no panics) and human-readable.

## E5: `ResolverChain` struct (NEW)

**Location**: `mikebom-cli/src/resolve/resolver_chain.rs`.

**Shape**:

```rust
pub(crate) struct ResolverChain {
    /// Registered resolvers, sorted by priority descending. Order
    /// captured at construction time; iteration order is stable
    /// across all invocations.
    resolvers: Vec<Box<dyn Resolver>>,
}

impl ResolverChain {
    /// Construct the default chain from `RESOLVER_REGISTRY`. Wires
    /// the compile-time-checked priority order into a runtime chain.
    /// Validates at construction that every registered name has a
    /// live implementation + that priorities match.
    pub(crate) fn new_default() -> Self { … }

    /// Dispatch an input through the chain, respecting first-match-
    /// wins semantic per R4. Returns the first non-empty
    /// `Vec<ResolvedComponent>` returned by a resolver whose
    /// `handles(input)` returned true, OR `Ok(vec![])` if every
    /// resolver returned an empty vec.
    ///
    /// Per FR-013, wraps every resolver.resolve invocation in a
    /// panic-catch (via `tokio::task::spawn` per R5). Both Err and
    /// panic emit a WARN naming the resolver + kind, then the chain
    /// continues.
    pub(crate) async fn run(
        &self,
        input: ResolveInput<'_>,
        ctx: &ResolveContext<'_>,
    ) -> Vec<ResolvedComponent> { … }
}
```

## E6: `RESOLVER_REGISTRY` const + compile-time uniqueness check (NEW)

**Location**: `mikebom-cli/src/resolve/resolver_chain.rs`.

**Shape**:

```rust
pub(crate) const RESOLVER_REGISTRY: &[(&str, u32)] = &[
    ("cargo",             100),
    ("pypi",               99),
    ("npm",                98),
    ("golang",             97),
    ("maven",              96),
    ("rubygems",           95),
    ("deb",                94),
    ("deps_dev_hash",      90),
    ("path",               70),
    ("hostname_fallback",  40),
];

const fn assert_registry_priorities_unique(reg: &[(&str, u32)]) {
    let mut i = 0;
    while i < reg.len() {
        let mut j = i + 1;
        while j < reg.len() {
            if reg[i].1 == reg[j].1 {
                panic!(
                    "resolver priority collision — two resolvers declared \
                     matching priorities in RESOLVER_REGISTRY. Give each \
                     resolver a unique priority in \
                     mikebom-cli/src/resolve/resolver_chain.rs"
                );
            }
            j += 1;
        }
        i += 1;
    }
}

const _: () = assert_registry_priorities_unique(RESOLVER_REGISTRY);
```

**Validation rules** (enforced by `cargo build`):
- No two entries may have the same priority — collision fails compilation with the panic message from R2.
- The name field is a string-slice; unique-name enforcement is a runtime check at `ResolverChain::new_default` construction (asserts each `RESOLVER_REGISTRY` entry maps to exactly one live implementation).

## E7: Per-ecosystem resolver structs (NEW; 7 of them)

**Location**: `mikebom-cli/src/resolve/resolvers/{cargo,pypi,npm,golang,maven,rubygems,deb}.rs`.

**Shape** (identical scaffold across all 7):

```rust
// resolvers/cargo.rs
pub(crate) struct CargoResolver;

impl Resolver for CargoResolver {
    fn name(&self) -> &'static str { "cargo" }
    fn priority(&self) -> u32 { 100 }
    fn technique(&self) -> ResolutionTechnique { ResolutionTechnique::UrlPattern }
    fn confidence(&self) -> f32 { 0.95 }

    fn handles(&self, input: &ResolveInput<'_>) -> bool {
        matches!(
            input,
            ResolveInput::Connection { connection, .. }
                if matches!(
                    connection.destination.hostname.as_deref(),
                    Some("crates.io") | Some("static.crates.io")
                )
        )
    }

    async fn resolve(
        &self,
        input: &ResolveInput<'_>,
        _ctx: &ResolveContext<'_>,
    ) -> Result<Vec<ResolvedComponent>, ResolverError> {
        // Extracted verbatim from url_resolver::resolve_cargo, with
        // the outer ResolvedComponent construction added (previously
        // in pipeline.rs).
        …
    }
}
```

Every resolver follows the same scaffold — only `name()`, `priority()`, `confidence()`, `handles()`'s hostname match, and `resolve()`'s ecosystem-specific extraction differ.

## E8: `DepsDevHashResolver` (NEW — wraps existing HashResolver)

**Location**: `mikebom-cli/src/resolve/resolvers/deps_dev_hash.rs`.

**Shape**:

```rust
pub(crate) struct DepsDevHashResolver {
    inner: super::super::hash_resolver::HashResolver,
}

impl Resolver for DepsDevHashResolver {
    fn name(&self) -> &'static str { "deps_dev_hash" }
    fn priority(&self) -> u32 { 90 }
    fn technique(&self) -> ResolutionTechnique { ResolutionTechnique::HashMatch }
    fn confidence(&self) -> f32 { 0.90 }

    fn handles(&self, input: &ResolveInput<'_>) -> bool {
        // FR-011 preservation: --skip-purl-validation short-circuits
        // by returning false from handles(), which makes the pipeline
        // skip this resolver entirely.
        if let ResolveInput::Connection { connection, .. } = input {
            !ctx_skip_online_validation()  // <-- via ResolveContext
                && connection
                    .response
                    .as_ref()
                    .and_then(|r| r.content_hash.as_ref())
                    .is_some()
        } else {
            false
        }
    }

    async fn resolve(
        &self,
        input: &ResolveInput<'_>,
        ctx: &ResolveContext<'_>,
    ) -> Result<Vec<ResolvedComponent>, ResolverError> {
        // Delegates to self.inner.resolve(hash).await; maps
        // anyhow::Error to ResolverError::Transient / Unavailable.
        …
    }
}
```

Note: `handles()` needs access to `skip_online_validation` from `ResolveContext`, so the actual signature threads `ctx` through `handles()` as well. Simpler alternative: put the `skip_online_validation` check inside `resolve()` and return `Ok(vec![])` when true. Design decision documented in the plan phase; either is acceptable.

## E9: `PathResolver` + `HostnameFallbackResolver` (NEW — thin wrappers)

**Location**: `mikebom-cli/src/resolve/resolvers/{path,hostname_fallback}.rs`.

Same shape pattern as E8 — wraps the existing `path_resolver::resolve_path_with_context` and `hostname_resolver::resolve_hostname` respectively. Priorities 70 and 40 per RESOLVER_REGISTRY.

## E10: Preserved legacy oracle (NEW test-only module)

**Location**: `mikebom-cli/src/resolve/pipeline_legacy_reference.rs` — `#[cfg(test)]`-gated.

Preserves the pre-refactor `pipeline.rs::resolve` implementation verbatim for the SC-001 byte-identity harness at `tests/resolver_chain_byte_identity.rs`. Scheduled for deletion in a follow-up cleanup milestone once byte-identity has been proven durable across multiple releases.

## E11: Deleted — `url_resolver.rs`

The 832-LOC monolith is fully removed. Its 7 ecosystem-specific `resolve_*` functions extract verbatim (with minor signature shifts) into the E7 resolvers. Any external references to `url_resolver::resolve_url_with_context` (there are none outside the pipeline today) MUST be updated to construct + invoke a `ResolverChain` instead.
