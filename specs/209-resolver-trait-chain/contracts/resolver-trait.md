# Contract: `Resolver` trait shape + `ResolverChain` registration point

**Date**: 2026-07-18
**Purpose**: Lock the interface every current + future resolver implements. Reviewers cite this doc when auditing per-resolver code.

## C-1: `Resolver` trait signature (LOCKED)

The trait signature is stable across the milestone. Adding a new required trait method is a breaking change; new capabilities MUST be added as default-implemented methods to preserve trait-object compatibility.

```rust
pub(crate) trait Resolver: Send + Sync {
    fn name(&self) -> &'static str;
    fn priority(&self) -> u32;
    fn technique(&self) -> mikebom_common::resolution::ResolutionTechnique;
    fn confidence(&self) -> f32;
    fn handles(&self, input: &ResolveInput<'_>) -> bool;
    async fn resolve(
        &self,
        input: &ResolveInput<'_>,
        ctx: &ResolveContext<'_>,
    ) -> Result<Vec<ResolvedComponent>, ResolverError>;
}
```

### Guarantees the trait provides to the pipeline

- **`name()` stable**: same string across every process invocation; snake-case; matches `RESOLVER_REGISTRY`.
- **`priority()` stable + unique**: same integer every invocation; different from every other registered resolver (compile-time enforced per C-3).
- **`handles()` cheap**: O(1) or O(few); no allocation, no `.await`. Called before every `resolve()` invocation to filter dispatch.
- **`resolve()` stateless**: no side effects observable across invocations. Two consecutive calls with identical `input` + `ctx` return identical `Result`.
- **Panic semantics**: `resolve()` MAY panic (unrecoverable programmer error) but SHOULD return `Err(ResolverError)` for anticipated failures. Both are caught at the pipeline layer per FR-013.

### Guarantees the pipeline provides to the resolver

- **Called only when `handles()` returned true**: resolvers do NOT need to re-check applicability inside `resolve()`.
- **Panic-safe outer wrapper**: even if `resolve()` panics, the process survives; a WARN log fires and the chain continues.
- **First-match-wins**: if a higher-priority resolver returned `Ok(components)` with `!components.is_empty()`, subsequent resolvers are NOT invoked for that input.

## C-2: `ResolveInput` + `ResolveContext` — resolver's inputs (LOCKED)

Every resolver receives exactly these two arguments to `resolve()`. Adding a new variant to `ResolveInput` is a breaking change (existing resolvers' `handles()` MUST be updated to handle the new variant — even if only to return `false`). Adding a new field to `ResolveContext` is source-compatible for existing resolvers (they ignore unread fields).

- **`ResolveInput`**: input type discriminant + payload. Current variants:
  - `Connection { connection, basename_to_hash }` — a traced network connection + the correlation table for hash-attaching.
  - `FileOp(&FileAccessOperation)` — a traced file-access operation.
- **`ResolveContext`**: read-only pipeline-wide context. Current fields:
  - `deb_codename: Option<&str>` — for the Deb resolver's `distro` qualifier.
  - `skip_online_validation: bool` — for the `DepsDevHashResolver`'s FR-011 short-circuit.

## C-3: `RESOLVER_REGISTRY` registration point (LOCKED shape; content grows over time)

The registry is a `const` array at `mikebom-cli/src/resolve/resolver_chain.rs`:

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
```

### Adding a new resolver

Per FR-010 + SC-002, a contributor adding a new resolver edits exactly **two files**:

1. **New file** at `mikebom-cli/src/resolve/resolvers/<name>.rs` — the resolver's implementation (matching E7 scaffold).
2. **One line** added to `RESOLVER_REGISTRY` (a `("<name>", <priority>)` tuple), plus one `pub mod <name>;` line in `resolvers/mod.rs`.

If the priority collides with an existing entry, `cargo build` fails with the panic message from E6. If the `<name>` doesn't match a `mod` declaration, `cargo build` fails with a missing-module error.

### Removing / renaming a resolver

Removing a resolver is a code-organization change; no external consumer sees the internal name.

Renaming a resolver requires updating both the `RESOLVER_REGISTRY` entry AND the resolver's `name()` return value in the same PR. The renamed `name()` becomes visible in WARN logs from FR-013; downstream log-consumers relying on the old name break — call it out in the PR description.

## C-4: `ResolutionEvidence.technique` signal preservation (LOCKED per SC-005)

The mapping from resolver → `ResolutionTechnique` enum value is byte-preserved across the refactor:

| Resolver | `technique()` returns |
|---|---|
| `CargoResolver` | `ResolutionTechnique::UrlPattern` |
| `PypiResolver` | `ResolutionTechnique::UrlPattern` |
| `NpmResolver` | `ResolutionTechnique::UrlPattern` |
| `GolangResolver` | `ResolutionTechnique::UrlPattern` |
| `MavenResolver` | `ResolutionTechnique::UrlPattern` |
| `RubyGemsResolver` | `ResolutionTechnique::UrlPattern` |
| `DebResolver` | `ResolutionTechnique::UrlPattern` |
| `DepsDevHashResolver` | `ResolutionTechnique::HashMatch` |
| `PathResolver` | `ResolutionTechnique::FilePathHeuristic` |
| `HostnameFallbackResolver` | `ResolutionTechnique::HostnameHeuristic` |

Adding a new technique variant to the `ResolutionTechnique` enum is a downstream-visible change; document in the PR description + update `docs/reference/sbom-format-mapping.md` if the technique carries into any `mikebom:*` annotation.

## C-5: Chain dispatch order (LOCKED)

Priority-descending. Explicitly:

1. Cargo (100) → PyPI (99) → npm (98) → Golang (97) → Maven (96) → RubyGems (95) → Deb (94) — the URL-family resolvers, first-match-wins per hostname
2. `deps_dev_hash` (90) — hash lookup via deps.dev; skipped if `handles()` returns false (either `--skip-purl-validation` OR input has no content hash)
3. `path` (70) — file-path heuristic; runs on both `ResolveInput::Connection` (extracting the URL's basename) and `ResolveInput::FileOp` inputs
4. `hostname_fallback` (40) — hostname-only heuristic; last-chance for connection inputs

Adding a new resolver in between existing tiers means picking an unused priority (e.g., 98-99 for a new URL-family resolver ranking between Cargo and PyPI). Picking a colliding priority fails `cargo build` per C-3.
