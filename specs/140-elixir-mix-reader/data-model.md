# Data Model — milestone 140

The Elixir/Mix reader does NOT introduce new types in `mikebom-common`. Reader-private serde-deserializing structs aren't applicable here because `mix.lock` is Elixir source code, not standardized data — instead the reader uses regex-tokenized intermediate enums.

## LockEntry (reader-private, lockfile)

Enum-discriminated representation of one parsed lockfile entry:

```rust
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum LockEntry {
    Hex {
        /// Top-level map key (the lockfile's `"<name>"` string).
        name: String,
        /// 3rd tuple element.
        version: String,
        /// 4th tuple element (always present in Elixir 1.4+).
        inner_sha256: String,
        /// 5th tuple element — atom list. Not consumed v1 (informational).
        #[allow(dead_code)]
        managers: Vec<String>,
        /// 7th tuple element. `"hexpm"` (default) OR `"hexpm:<org>"`
        /// (private org — colon-prefixed slug, NOT a URL).
        repo: String,
        /// 8th tuple element. OPTIONAL — pre-Hex-2.0 entries lack it.
        outer_sha256: Option<String>,
    },
    Git {
        /// Top-level map key.
        name: String,
        /// 2nd tuple element.
        url: String,
        /// 3rd tuple element — always present in lockfile (40-char SHA).
        resolved_sha: String,
        /// 4th tuple element. May contain `ref:`/`branch:`/`tag:`/etc.
        /// Stored as a single string for simplicity (e.g., `ref: "main"`).
        declared_ref: Option<String>,
    },
    Path {
        /// Top-level map key.
        name: String,
        /// 2nd tuple element.
        path: String,
        /// 4th tuple element. May contain `in_umbrella: true` flag.
        in_umbrella: bool,
    },
}
```

### Validation rules

- `name` (map key) MUST be non-empty.
- `:hex` `version` MUST be non-empty.
- `:hex` `inner_sha256` MUST be 64-char lowercase hex; warn-and-skip otherwise.
- `:hex` `outer_sha256` when present MUST be 64-char lowercase hex; non-conformant values become `None` per Q3 best-effort emission.
- `:hex` `repo` MUST be one of `"hexpm"` OR matches regex `^hexpm:[a-zA-Z0-9_-]+$` (private-org slug). Unknown shapes warn-and-skip.
- `:git` `resolved_sha` MUST be 40-char lowercase hex; warn-and-skip otherwise.
- `:git` `url` MUST be non-empty.
- `:path` `path` MUST be non-empty.

## MixExsInfo (reader-private, manifest)

Regex-extracted intermediate from `mix.exs`:

```rust
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
struct MixExsInfo {
    /// `app:` keyword in `project/0`. Drives FR-012 main-module name.
    app_name: Option<String>,
    /// `version:` keyword in `project/0`. Drives FR-012 main-module version.
    version: Option<String>,
    /// `apps_path:` keyword presence in `project/0` (umbrella sentinel
    /// per R4). Value not consumed.
    is_umbrella: bool,
    /// Direct deps extracted from `deps/0` function body.
    deps: Vec<DeclaredDep>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DeclaredDep {
    /// `{:<name>, ...}` atom name.
    name: String,
    /// First positional string after the atom: `"constraint"` for hex
    /// deps; absent for non-hex (`git:`/`path:` deps lack the
    /// positional version).
    constraint: Option<String>,
    /// Set when `only: [:dev, ...]` / `only: :dev` / `only: :test` /
    /// `runtime: false` keyword present. Drives FR-008 LifecycleScope.
    dev_scope: bool,
    /// Set when the dep tuple's source line resides inside an
    /// `if Mix.env() ... do` / `unless ...` / `case ... do` /
    /// multi-clause `def deps(env)` block. Drives Q1
    /// `mikebom:elixir-extraction-mode = "conditional-flattened"` annotation.
    in_conditional: bool,
    /// Source-kind discriminator extracted from the opts blob per C2
    /// remediation. T018 dispatches on this to emit the correct PURL
    /// shape per FR-003 (design-tier git/path deps emit as
    /// `pkg:generic/`, not `pkg:hex/`). The `:github` shortcut is
    /// expanded to `Git` at extraction time (T008).
    source_kind: DeclaredDepSource,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum DeclaredDepSource {
    /// Default — hex source; emit as `pkg:hex/<name>@<constraint>`.
    Hex,
    /// `git:` or `github:` shortcut in opts. `url` is the resolved git
    /// remote URL (`:github` shortcut expanded to
    /// `https://github.com/<owner>/<repo>.git` at extraction time per
    /// research R3).
    Git { url: String, declared_ref: Option<String> },
    /// `path:` opt. `path` is the relative path string.
    Path { path: String, in_umbrella: bool },
}
```

### Validation rules

- `app_name` is extracted via regex `(?m)^\s*app:\s*:([a-zA-Z_][a-zA-Z0-9_]*)\b` — atom name MUST match Elixir identifier syntax.
- `version` is extracted via regex `(?m)^\s*version:\s*"([^"]+)"` — string-literal form only (no compile-time expression evaluation).
- `is_umbrella` set when regex `(?m)^\s*apps_path:` matches anywhere in the file (key-presence detection per R4, value ignored).
- `deps[*].name` MUST match Elixir atom syntax.

## PackageDbEntry field mapping (per source type)

### Common fields (all source types)

| Field | Source | Notes |
|---|---|---|
| `name` | Lockfile map key (verbatim) | Lowercase enforced by Hex.pm publish-time; preserved as-is |
| `version` | `:hex` tuple element 3, OR `:git` resolved SHA (40-char), OR `:path` from sibling `mix.exs::version` lookup (or `"unspecified"` placeholder when absent) | |
| `arch` | `None` | N/A |
| `source_path` | Absolute path to owning `mix.lock` / `mix.exs` | Drives `ResolutionEvidence.source_file_paths` |
| `maintainer` | `None` | Not consumed v1 |
| `lifecycle_scope` | `Runtime` default; `Development` when sibling `mix.exs::deps/0` entry has `only: [:dev,...]` / `runtime: false` per FR-008 | Cross-reference via `DeclaredDep.dev_scope` |
| `requirement_range` | `None` for lockfile; `Some(constraint)` for design-tier per FR-005 | |
| `evidence_kind` | `"mix-lock"` (source-tier) / `"mix-exs"` (design-tier) | NEW values added to cyclonedx/builder.rs enum |
| `sbom_tier` | `"source"` (lockfile-derived) / `"design"` (FR-005) | |
| `binary_class` through `build_inclusion` | `None` | N/A |
| `licenses` | `Vec::new()` | License deferred per spec Out-of-Scope |
| `extra_annotations` | Source-type discriminator + source-specific extras | |

### Per-source-type fields

| Source | `purl` | `source_type` | `extra_annotations` extras | `hashes` |
|---|---|---|---|---|
| **hex (default `"hexpm"`)** | `pkg:hex/<lc-name>@<version>` | `Some("hex-hex")` | `mikebom:source-type = "hex-hex"` | Inner SHA-256 always; outer SHA-256 when present + non-empty per Q3. Both as `ContentHash::with_algorithm(HashAlgorithm::Sha256, hex)` |
| **hex (private org `"hexpm:<org>"`)** | `pkg:hex/<org>/<lc-name>@<version>?repository_url=https://repo.hex.pm` | `Some("hex-hex")` | `mikebom:source-type = "hex-hex"` (org distinguishable via PURL namespace) | Same as default hex |
| **git** | `pkg:generic/<name>@<resolved-sha>?vcs_url=git+<url>` (per Phase 0: pkg:hex/ would be non-conformant since purl-spec doesn't bless vcs_url for hex) | `Some("hex-git")` | `mikebom:source-type = "hex-git"`; `mikebom:vcs-declared-ref = "<opt-value>"` when present (e.g., `"ref: main"` or `"branch: develop"`) | Empty (git source has no SHA-256 in lockfile) |
| **path** | `pkg:generic/<name>@<version-or-unspecified>` | `Some("hex-path")` | `mikebom:source-type = "hex-path"`; `mikebom:path = "<path-string>"`; `mikebom:in-umbrella = "true"` when opts contain that flag | Empty |

### Main-module field mapping (per FR-012)

For each `mix.exs` discovered:

| Field | Value |
|---|---|
| `purl` | `pkg:hex/<app_name>@<version-or-"0.0.0-unknown">` (lowercase app_name) OR fallback to dir-basename when `mix.exs` lacks parseable `app:` |
| `name` | `app_name` per derivation cascade (preserved verbatim — display) |
| `version` | `version` per `mix.exs::project/0::version:` keyword OR `"0.0.0-unknown"` fallback |
| `source_path` | Absolute path to `mix.exs` (preferred) or `mix.lock` |
| `evidence_kind` | `Some("mix-exs")` |
| `sbom_tier` | `Some("source")` |
| `source_type` | `Some("hex-main-module")` |
| `extra_annotations` | `mikebom:component-role = "main-module"` + `mikebom:source-type = "hex-main-module"`; for umbrella roots: `mikebom:umbrella-root = "true"` |
| `depends` | Lockfile mode: cross-reference lockfile entries against `mix.exs::deps/0` to enumerate direct deps + (for umbrella roots) each sub-app's main-module bom-ref per Q2. Design-tier mode: `mix.exs::deps/0` declared names + (umbrella) sub-app main-module bom-refs. |

### Conditional-extraction annotation (per Q1)

Components emitted from `mix.exs::deps/0` where `DeclaredDep.in_conditional == true`:
- Additional `mikebom:elixir-extraction-mode = "conditional-flattened"` annotation.

This annotation only applies to design-tier (FR-005) components; lockfile-derived components are post-resolution and don't carry the precision-loss signal.

## Validation invariants (per spec FR-* + Constitution)

- `purl.as_str().starts_with("pkg:hex/")` OR `purl.as_str().starts_with("pkg:generic/")` for every emitted Elixir entry.
- `source_type` value MUST be one of `hex-hex` / `hex-git` / `hex-path` / `hex-main-module`.
- `evidence_kind` MUST be one of `mix-lock` / `mix-exs`.
- For hex entries: `purl.as_str().starts_with("pkg:hex/")` AND vendor segment (when present) matches `^[a-zA-Z0-9_-]+$` (org slug).
- For private-org hex entries: `purl.as_str().contains("?repository_url=")`.
- For git entries: `purl.as_str().contains("?vcs_url=git+")`.
- SHA-256 hashes (when present) MUST be 64-char lowercase hex.

## Out-of-scope data shapes

- License extraction from per-package `mix.exs::package/0::licenses` field (cross-reader follow-up — `hex_metadata.config` for installed packages).
- `deps/` directory walking for installed-tier scans (spec Out-of-Scope).
- Per-package transitive dep edges from the lockfile tuple's 6th element (deferred to v1.1).
- Hex.pm API enrichment (license, owner, downloads — spec Out-of-Scope).
- syft/trivy compatibility `mikebom:also-known-as` annotation (deferred to v1.1).
- Pre-Elixir-1.4 lockfile format (spec Out-of-Scope; warn-and-skip on detection — missing `:hex` discriminator atom in tuples).
