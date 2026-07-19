# Quickstart: Resolver Trait + Chain (m209)

**Date**: 2026-07-18
**Purpose**: The shortest path from `cargo build` to seeing the chain work + adding a new resolver. Also serves as the manual QA script during implementation.

## Prerequisites

- Rust stable (workspace toolchain).
- `mikebom-cli` built: `cargo +stable build -p mikebom --release`.

## Path 1 — Verify no user-visible regression (SC-001)

The refactor is net-zero on operator surface. Any existing invocation MUST produce the same output byte-for-byte:

```sh
# Pre-refactor baseline (already captured; do NOT regen)
cat mikebom-cli/tests/fixtures/resolver_chain/byte_identity_reference.json | jq '.components | length'
# e.g. 43 components

# Post-refactor
mikebom sbom generate \
    --attestation mikebom-cli/tests/fixtures/attestations/nginx-1.27.0.dsse.json \
    --format cyclonedx-json \
    --output /tmp/post-refactor.cdx.json

jq '.components | length' /tmp/post-refactor.cdx.json
# MUST equal 43
```

Byte-identity harness runs automatically in the test suite:

```sh
cargo +stable test -p mikebom --test resolver_chain_byte_identity
# Expected: passed
```

## Path 2 — Add a new resolver (SC-002)

Follow this recipe to add support for a new packaging ecosystem. Estimated time: 30–60 minutes for the resolver logic + 10 minutes for wiring.

### Step 1 — Create the resolver file

```sh
touch mikebom-cli/src/resolve/resolvers/nuget.rs
```

Populate it with the standard scaffold:

```rust
// mikebom-cli/src/resolve/resolvers/nuget.rs
use mikebom_common::resolution::{ResolutionTechnique, ResolvedComponent};

use crate::resolve::resolver_trait::{
    ResolveContext, ResolveInput, Resolver, ResolverError,
};

pub(crate) struct NugetResolver;

impl Resolver for NugetResolver {
    fn name(&self) -> &'static str { "nuget" }
    fn priority(&self) -> u32 { 93 }   // between deb (94) and deps_dev_hash (90)
    fn technique(&self) -> ResolutionTechnique { ResolutionTechnique::UrlPattern }
    fn confidence(&self) -> f32 { 0.95 }

    fn handles(&self, input: &ResolveInput<'_>) -> bool {
        matches!(
            input,
            ResolveInput::Connection { connection, .. }
                if connection
                    .destination
                    .hostname
                    .as_deref()
                    == Some("api.nuget.org")
        )
    }

    async fn resolve(
        &self,
        input: &ResolveInput<'_>,
        _ctx: &ResolveContext<'_>,
    ) -> Result<Vec<ResolvedComponent>, ResolverError> {
        // Ecosystem-specific extraction logic:
        // parse `path` (e.g., "/v3-flatcontainer/newtonsoft.json/13.0.3/newtonsoft.json.13.0.3.nupkg")
        // into a pkg:nuget/newtonsoft.json@13.0.3 PURL.
        todo!("implement NuGet URL pattern match")
    }
}
```

### Step 2 — Register in the chain

Add ONE line to `resolvers/mod.rs`:

```rust
pub(crate) mod nuget;
```

And add ONE tuple to `RESOLVER_REGISTRY` at `resolver_chain.rs`:

```rust
pub(crate) const RESOLVER_REGISTRY: &[(&str, u32)] = &[
    ("cargo",             100),
    ("pypi",               99),
    ("npm",                98),
    ("golang",             97),
    ("maven",              96),
    ("rubygems",           95),
    ("deb",                94),
    ("nuget",              93),   // <-- new
    ("deps_dev_hash",      90),
    ("path",               70),
    ("hostname_fallback",  40),
];
```

Also wire the new resolver into `ResolverChain::new_default()` (one line adding `Box::new(nuget::NugetResolver)` to the `Vec`).

### Step 3 — Verify

```sh
cargo +stable build -p mikebom
# If you accidentally reused an existing priority (e.g., typed 94 instead of 93):
#   error[E0080]: evaluation of constant value failed
#     |
#     | const _: () = assert_registry_priorities_unique(RESOLVER_REGISTRY);
#     |               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ …
#     | panicked at: resolver priority collision — two resolvers declared
#     | matching priorities in RESOLVER_REGISTRY. Give each resolver a
#     | unique priority in mikebom-cli/src/resolve/resolver_chain.rs
```

Compile-time collision detection per FR-017 caught the mistake before any runtime invocation.

### Step 4 — Test in isolation (SC-003)

```sh
# Run only the new resolver's tests — no fixtures from other resolvers load
cargo +stable test -p mikebom -- resolve::resolvers::nuget
# Expected: sub-100ms wall-clock
```

## Path 3 — Verify per-resolver panic-safety (FR-013)

Inject a panic into a resolver to verify the pipeline catches it:

```rust
// In a test-only variant of NugetResolver:
async fn resolve(&self, _input: &ResolveInput<'_>, _ctx: &ResolveContext<'_>)
    -> Result<Vec<ResolvedComponent>, ResolverError>
{
    panic!("deliberate test panic")
}
```

Run the pipeline; verify:
- Process does NOT abort.
- Stderr contains `WARN` line naming `nuget` + kind=`panic`.
- Other resolvers still produce their expected components.

## Path 4 — Verify perf regression cap (SC-004)

```sh
cargo +stable test -p mikebom --release --ignored --test resolver_chain_perf
# Prints wall-clock: baseline=NNN ms, post-refactor=MMM ms, ratio=X.XX
# Asserts ratio <= 1.05
```

## Non-goals surfaced in quickstart

- **Not a dynamic plugin loader** — the resolver registration is compile-in; adding a resolver requires rebuilding mikebom. External plugin loading is tracked separately at #453.
- **Not a per-invocation resolver toggle** — operators cannot disable a resolver at runtime via CLI. The one exception is `--skip-purl-validation`, which disables `DepsDevHashResolver` per FR-011 (preserved from pre-refactor). Adding more per-resolver toggles is a follow-up if operators surface a need.
- **Not exposing the trait publicly** — the `Resolver` trait is `pub(crate)`. Third-party crates cannot implement resolvers without a public-API milestone (again #453).
