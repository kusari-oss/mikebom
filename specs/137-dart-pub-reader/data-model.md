# Data Model — milestone 137

The Dart reader does NOT introduce any new types in `mikebom-common`. Every Dart-derived component flows through the existing `PackageDbEntry` → `ResolvedComponent` pipeline. The new types are reader-private serde-deserializing structs that mirror the subset of `pubspec.yaml` + `pubspec.lock` the reader consumes.

## PubspecYaml (reader-private, manifest)

The manifest parser's per-project intermediate representation. Lives inside `mikebom-cli/src/scan_fs/package_db/dart.rs` only.

```rust
// Lives in mikebom-cli/src/scan_fs/package_db/dart.rs only.
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct PubspecYaml {
    /// `name:` — required for any valid pubspec. PURL-segment source.
    name: String,
    /// `version:` — optional (libraries-in-development may omit).
    /// Falls back to `"0.0.0-unknown"` per the cargo (milestone 064)
    /// main-module convention when absent.
    #[serde(default)]
    version: Option<String>,
    /// `description:` — informational; not consumed for emission.
    #[serde(default)]
    description: Option<String>,
    /// `dependencies:` — declared direct runtime deps (constraint
    /// strings). Used in design-tier mode (no lockfile) per FR-005.
    #[serde(default)]
    dependencies: std::collections::BTreeMap<String, serde_yaml::Value>,
    /// `dev_dependencies:` — declared dev-only deps. Used in
    /// design-tier mode + tagged with lifecycle-scope=development
    /// per FR-005's analysis-clarified behavior.
    #[serde(default)]
    dev_dependencies: std::collections::BTreeMap<String, serde_yaml::Value>,
    /// `dependency_overrides:` — informational v1. The lockfile's
    /// `direct overridden` entry classification (when present) is the
    /// authoritative discriminator; pubspec.yaml's override block is
    /// not separately consumed in design-tier mode.
    #[serde(default)]
    dependency_overrides: std::collections::BTreeMap<String, serde_yaml::Value>,
    /// `environment:` — SDK constraints; informational only (not
    /// consumed v1).
    #[serde(default)]
    environment: Option<serde_yaml::Value>,
}
```

### Validation rules

- `name` MUST be non-empty for the project to emit a main-module component.
- `version` MAY be absent (libraries-in-development); falls back to `"0.0.0-unknown"`.
- Dep-constraint values (`dependencies[key]`) are heterogeneous YAML scalars or maps (string for simple constraint, map for `path: ...` / `git: ...` / `sdk:` directives). Stored as `serde_yaml::Value` for permissive design-tier extraction; the dep NAME (key) is what matters for the design-tier emission.

## PubspecLock + LockfileEntry (reader-private, lockfile)

```rust
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct PubspecLock {
    /// `packages:` — every direct + transitive dep keyed by name.
    #[serde(default)]
    packages: std::collections::BTreeMap<String, LockfileEntry>,
    /// `sdks:` — host SDK constraint strings; informational only
    /// (not consumed v1 per research §R2).
    #[serde(default)]
    sdks: Option<serde_yaml::Value>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct LockfileEntry {
    /// `dependency:` — required. One of: "direct main" | "direct dev"
    /// | "transitive" | "direct overridden".
    dependency: String,
    /// `description:` — polymorphic per `source:`. String for
    /// `source: sdk` entries (the SDK name); map for hosted/git/path.
    /// Use `#[serde(untagged)]` to handle both shapes.
    description: LockfileDescription,
    /// `source:` — discriminator. Required. One of: hosted | git |
    /// path | sdk.
    source: String,
    /// `version:` — required. For hosted: upstream version string.
    /// For git: typically `"0.0.0"` or upstream-recorded version.
    /// For path: the local package's declared version. For sdk:
    /// always literal `"0.0.0"` per pub convention.
    version: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
#[allow(dead_code)]
enum LockfileDescription {
    /// `source: sdk` entries — `description: flutter` (bare scalar).
    /// The scalar IS the SDK name.
    Sdk(String),
    /// All other source types — a map of source-specific fields.
    Map(LockfileDescriptionMap),
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct LockfileDescriptionMap {
    /// hosted: package name (typically matches top-level key but
    /// preserved for safety). git: not present.
    /// path: not present.
    #[serde(default)]
    name: Option<String>,
    /// hosted: download SHA-256 (lowercase hex, no prefix).
    /// Universally present for Dart 2.19+ lockfiles per research §R2.
    #[serde(default)]
    sha256: Option<String>,
    /// hosted: bare base URL with scheme (e.g., "https://pub.dev").
    /// git: git remote URL.
    #[serde(default)]
    url: Option<String>,
    /// git: user-supplied ref (branch name, tag, or "HEAD").
    #[serde(default)]
    ref_: Option<String>,
    /// git: resolved 40-char SHA. Required for git PURL construction.
    #[serde(default, rename = "resolved-ref")]
    resolved_ref: Option<String>,
    /// git: subdirectory inside the repo where the package lives.
    /// "." when at repo root; always present for git source.
    /// path: filesystem path (relative if `relative: true`).
    #[serde(default)]
    path: Option<String>,
    /// path: whether `path` is workspace-relative.
    #[serde(default)]
    relative: Option<bool>,
}
```

### Validation rules

- `dependency` MUST be one of the four documented values; unknown values trigger warn-and-skip per FR-007.
- `source` MUST be one of `hosted` / `git` / `path` / `sdk`; unknown values trigger warn-and-skip per research §R7.
- For `source: hosted`, `description.url` defaults to `"https://pub.dev"` if absent (matches pub's own fallback).
- For `source: git`, `description.resolved-ref` MUST be present + 40 hex chars; absent triggers warn-and-skip per spec Edge Cases ("Git deps without a resolved SHA").
- For `source: sdk`, `description` is a bare string; the parser must NOT fail when it's not a map. The `version` field is always literal `"0.0.0"` per purl-spec example (preserved verbatim).

## PackageDbEntry field mapping (per source type)

### Common fields (all source types)

| Field | Source | Notes |
|---|---|---|
| `name` | Lockfile entry's top-level key (== `description.name` for hosted) | Verbatim |
| `version` | `LockfileEntry.version` | Verbatim per source-type semantics |
| `arch` | `None` | Dart components are architecture-independent at the lockfile layer |
| `source_path` | Absolute path to the owning `pubspec.lock` (or `pubspec.yaml` for design-tier) | Drives `ResolutionEvidence.source_file_paths` |
| `maintainer` | `None` | Not in lockfile |
| `lifecycle_scope` | `Runtime` for `direct main` / `transitive` / `direct overridden`; `Development` for `direct dev` | Per FR-008 |
| `requirement_range` | `None` for lockfile entries; `Some(constraint-string)` for design-tier per FR-005 | |
| `evidence_kind` | `Some("pubspec-lock")` for lockfile-derived; `Some("pubspec-yaml")` for design-tier | NEW values added to cyclonedx/builder.rs enum |
| `sbom_tier` | `Some("source")` for lockfile-derived; `Some("design")` for design-tier (FR-005) | |
| `binary_class` / `binary_stripped` / `linkage_kind` / `detected_go` / `confidence` / `binary_packed` | `None` | N/A for source-tree language reader |
| `raw_version` | `None` | |
| `parent_purl` | `None` | Top-level (lockfile entries are flat) |
| `npm_role` | `None` | N/A |
| `co_owned_by` | `None` | N/A |
| `shade_relocation` | `None` | N/A |
| `binary_role` | `None` | N/A |
| `build_inclusion` | `None` | N/A |
| `licenses` | `Vec::new()` | License NOT in lockfile per spec Out-of-Scope (mirrors milestone-135 FR-012 + milestone-136 FR-011 deferrals) |
| `extra_annotations` | Source-type discriminator (see per-source-type rows) | See below |

### Per-source-type fields

| Source | `purl` | `source_type` (in `PackageDbEntry.source_type`) | `extra_annotations` extras | `hashes` |
|---|---|---|---|---|
| **hosted** | `pkg:pub/<name>@<version>[?repository_url=<url>]` (qualifier omitted when url is `pub.dev` or `pub.dartlang.org`) | `Some("pub-hosted")` | `mikebom:source-type = "pub-hosted"` | When `description.sha256` present: `vec![ContentHash::sha256(<hex>)]`; else empty |
| **git** | `pkg:pub/<name>@<resolved-sha>?vcs_url=git+<url>[#<subpath>]` (subpath omitted when path is `"."` or empty) | `Some("pub-git")` | `mikebom:source-type = "pub-git"`; `mikebom:vcs-ref = "<user-ref>"` (preserves the `ref:` field for evidence) | Empty (git source has no download hash) |
| **path** | `pkg:generic/<name>@<version>` (placeholder per R1) | `Some("pub-path")` | `mikebom:source-type = "pub-path"`; `mikebom:path = "<lockfile-relative-path>"` | Empty |
| **sdk** | `pkg:pub/<sdk-name>@0.0.0` (purl-spec canonical example) | `Some("pub-sdk")` | `mikebom:source-type = "pub-sdk"`; `mikebom:sdk-name = "<sdk>"` (the SDK family — `flutter`, `dart`, etc.) | Empty |

### Main-module field mapping (per FR-012)

For each scanned `pubspec.yaml`, one additional `PackageDbEntry` emits with:

| Field | Value |
|---|---|
| `purl` | `pkg:pub/<pubspec.yaml.name>@<pubspec.yaml.version-or-"0.0.0-unknown">` |
| `name` | `pubspec.yaml.name` |
| `version` | `pubspec.yaml.version` (or `"0.0.0-unknown"` fallback) |
| `source_path` | Absolute path to `pubspec.yaml` |
| `evidence_kind` | `Some("pubspec-yaml")` |
| `sbom_tier` | `Some("source")` |
| `source_type` | `Some("pub-main-module")` |
| `extra_annotations` | `mikebom:component-role = "main-module"` + `mikebom:source-type = "pub-main-module"` |
| `depends` | Names of direct deps from the project's lockfile (or pubspec.yaml `dependencies:` + `dev_dependencies:` in design-tier mode) |

### Dep edges

- **Lockfile mode**: each `LockfileEntry`'s dependencies come from the lockfile's per-entry `dependencies:` sub-array (not modeled in the struct above for v1 — extracted via post-parse pass through the raw `serde_yaml::Value` map). When absent in v1's typed parse, dep edges are derived from the project main-module → lockfile entries where `dependency == "direct main" | "direct dev" | "direct overridden"`. Transitive edges deferred to v1.1.
- **Design-tier mode**: main-module → each top-level key in `dependencies:` + `dev_dependencies:`. No transitive edges (no lockfile available).

## Validation invariants (per spec FR-* + Constitution)

- `purl.as_str().starts_with("pkg:pub/")` OR `purl.as_str().starts_with("pkg:generic/")` for every emitted Dart entry.
- `source_type` value MUST be one of `pub-hosted` / `pub-git` / `pub-path` / `pub-sdk` / `pub-main-module`.
- `evidence_kind` MUST be one of `pubspec-lock` / `pubspec-yaml` (the cyclonedx/builder.rs enum extension).
- For SDK entries: `purl.as_str().ends_with("@0.0.0")` (literal placeholder per purl-spec).
- For git entries: `purl.as_str().contains("?vcs_url=git+")`.

## Out-of-scope data shapes

- License extraction from `~/.pub-cache/hosted/pub.dev/<pkg>-<ver>/pubspec.yaml` (cross-reader follow-up; mirrors milestone-135 FR-012 + milestone-136 FR-011 deferrals).
- Per-package homepage / VCS URL from the upstream `pubspec.yaml` (same deferral).
- Pre-Dart-2.0 lockfile format (rare in 2026; explicitly out of spec scope).
- `.dart_tool/package_config.json` parsing (file-claim follow-up).
- Transitive dep edges from individual lockfile entries' `dependencies:` arrays — v1 emits main-module → direct deps only; transitive components surface but their inter-edges are deferred to v1.1.
- pub-workspace single-lockfile per-member attribution (operators in pub-workspace mode see unified view in v1).
