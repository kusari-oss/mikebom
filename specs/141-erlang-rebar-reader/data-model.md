# Data Model — milestone 141 Erlang/OTP rebar reader (Phase 1)

Defines the parsed representations of `rebar.lock`, `rebar.config`, and
`*.app.src` that the reader builds in-memory, and how they map onto
`PackageDbEntry` instances flowing through the existing `read_all`
pipeline.

## 1. Input artifacts

### 1.1 `rebar.lock`

Erlang-term-syntax lockfile, rebar3 1.0+ schema. Top-level is a two-element
tuple:

```erlang
{"<lock-version>", [<pinned-deps>]}.
```

Where `<lock-version>` is `"1.2.0"` for modern rebar3 (3.13+), `"1.1.0"`
for mid-vintage (3.7–3.12), or absent in legacy rebar3 (<3.7). The
inner list contains pinned-dep tuples per the shapes documented in
research §R2.

### 1.2 `rebar.config`

Erlang source code with the following top-level keywords (regex-extracted
per research §R4):

- `{deps, [<dep-decl-list>]}` — declared deps (BUILD-TIME).
- `{profiles, [{<env>, [<keyword-list>]}]}` — non-default profiles, each
  containing its own `{deps, [...]}` block. Environments `dev`, `test`,
  `doc` trigger `mikebom:lifecycle-scope = "development"` per FR-008.
- Other keywords (`erl_opts`, `plugins`, `relx`, etc.) — ignored.

### 1.3 `*.app.src`

OTP application descriptor. Shape:

```erlang
{application, <atom-name>, [
    {vsn, "<version>"},
    {applications, [<atom-list>]},
    {included_applications, [<atom-list>]},  % optional
    {optional_applications, [<atom-list>]},  % optional (OTP 26+)
    {description, "<text>"},
    {mod, {<module>, []}},
    % ... other keywords ignored
]}.
```

The `applications:` keyword is the AT-RUNTIME hard-dep list (what the OTP
supervisor must start first). `included_applications:` is the embedded-sub-app
list (sharing the parent's supervision tree). `optional_applications:` is
OTP-26+ soft-dep list.

## 2. Parsed intermediate types

### 2.1 `LockEntry` (private to `erlang.rs`)

```rust
enum LockEntry {
    HexModern {
        name: String,
        version: String,
        inner_sha256: Option<String>,
        repo: Option<String>,  // "hexpm:<org>" form when present; otherwise default (None == "hexpm")
        depth: u32,
    },
    HexLegacy {
        name: String,
        version: String,
        depth: u32,
    },
    Git {
        name: String,
        url: String,
        resolved_ref: String,         // SHA when persisted via {ref, ...}
        declared_ref_form: String,    // "ref" | "tag" | "branch" — original syntax preserved as evidence
        depth: u32,
    },
}
```

**Validation rules**:
- `name` is lowercased per purl-spec canonical hex form (Hex.pm typically
  enforces this at publish time, so the lowercasing is usually no-op).
- `version` strings preserved as-is (Hex.pm enforces SemVer; rebar.lock
  surfaces whatever the registry returned).
- `inner_sha256` only present when the lockfile's `{pkg, ...}` tuple
  includes the 4th element AND it's non-empty (per FR-011 best-effort).
- `repo == Some("hexpm:<org>")` → private-org dep; emit PURL with
  namespace-as-org + `repository_url=https://repo.hex.pm` qualifier.
- `repo == Some("<org>")` (bare org without `hexpm:` prefix) → also
  private-org dep per rebar3's documented flexibility; same emission.
- `repo == None` or `repo == Some("hexpm")` → default Hex.pm; emit
  `pkg:hex/<lc-name>@<version>` with no namespace, no qualifier.
- `Git.resolved_ref`: when the lockfile preserves the original `{tag, ...}`
  or `{branch, ...}` form (no SHA resolution at lock-write time), the
  reader uses the literal tag/branch as the version-position-substitute
  in the emitted PURL — purl-spec git-source allows arbitrary version
  strings, not just SHAs. The `declared_ref_form` value surfaces as
  `mikebom:vcs-declared-ref = "ref" | "tag" | "branch"` evidence.

### 2.2 `DeclaredDep` (private to `erlang.rs`)

Parsed from `rebar.config::{deps, [...]}` blocks for design-tier fallback
(FR-005) AND for the main-module-depends-union per Q2 (FR-012).

```rust
enum DeclaredDepSource {
    Hex,       // {<name>, "<version-constraint>"} or {<name>, {pkg, <name>, "<constraint>"}}
    Git,       // {<name>, {git, "<url>", {ref|tag|branch, "..."}}}
    Path,      // {<name>, {path, "<path>"}}  (less common but supported by rebar3)
}

struct DeclaredDep {
    name: String,
    source_kind: DeclaredDepSource,
    constraint: Option<String>,           // version constraint string (Hex case)
    git_url: Option<String>,              // Git case
    git_ref: Option<(String, String)>,    // (ref|tag|branch, value) for Git case
    path_target: Option<String>,          // Path case
    profile: Option<String>,              // None for default; Some("test"/"dev"/"doc") for profile-scoped
}
```

**Validation rules**:
- `name` is the bare atom from the dep tuple's first element (e.g., `cowboy`).
- `profile` carries the parent `{profiles, [{<env>, [...]}]}` block's name
  when the dep was extracted from inside one; `None` for top-level deps.
- Deps with `profile == Some("dev"|"test"|"doc")` get
  `mikebom:lifecycle-scope = "development"` per FR-008.

### 2.3 `AppSrcManifest` (private to `erlang.rs`)

Parsed from a single `*.app.src` file.

```rust
struct AppSrcManifest {
    app_name: String,                  // from {application, <atom>, ...}; fallback to parent-dir basename per FR-012
    version: String,                   // from {vsn, "..."}; fallback "0.0.0-unknown" per FR-012
    file_path: PathBuf,                // for evidence + warn diagnostics
    required_apps: Vec<String>,        // {applications, [<atom-list>]}
    included_apps: Vec<String>,        // {included_applications, [<atom-list>]}; empty when keyword absent
    optional_apps: Vec<String>,        // {optional_applications, [<atom-list>]}; empty when keyword absent
}
```

**Validation rules**:
- `app_name` regex-extract from `{application, <atom>, [` — the atom is
  bare (no `<<"...">>`-style binary-string in `*.app.src` source; that
  shape only appears in lockfiles).
- `version` regex-extract from `{vsn, "<text>"}` inside the keyword list.
  Fallback `"0.0.0-unknown"` when the keyword is absent or malformed
  (per FR-012, matches the cargo/dart/composer/cocoapods/elixir
  main-module convention).
- Each atom-list extract handles multi-line forms (e.g., one atom per
  line with trailing commas) via the brace-counted tokenizer.

### 2.4 `AppDepKind` (private)

Captures the Q3 keyword-family discrimination for each main-module
edge-target.

```rust
enum AppDepKind {
    Required,   // from {applications, [...]}
    Included,   // from {included_applications, [...]}
    Optional,   // from {optional_applications, [...]}
    BuildOnly,  // from rebar.config::{deps, [...]} only (not in any *.app.src list)
}
```

**Precedence rule** (per Q3): when the same atom appears in multiple
keyword lists for a single `*.app.src`, the kind annotation reflects
the highest-binding source: `Required > Included > Optional`.
`BuildOnly` (the rebar.config-only fallback) is unmarked by the
annotation (no `mikebom:erlang-app-dep-kind` property emitted) because
it has no application-graph kind — the dep was declared at build-time
in `rebar.config` but never surfaced in any runtime `applications:`
keyword list.

## 3. Output mapping → `PackageDbEntry`

### 3.1 Hex deps from `rebar.lock`

For each `LockEntry::HexModern { name, version, inner_sha256, repo, depth }`
or `LockEntry::HexLegacy { name, version, depth }`:

```rust
PackageDbEntry {
    purl: build_hex_purl(&name, &version, repo.as_deref()),
    // ^ default → pkg:hex/<lc-name>@<version>
    // ^ private-org → pkg:hex/<org>/<lc-name>@<version>?repository_url=https://repo.hex.pm
    name: name.clone(),
    version: Some(version.clone()),
    hashes: inner_sha256.map(|h| vec![ContentHash::with_algorithm(HashAlgorithm::Sha256, h)])
                       .unwrap_or_default(),
    depends: vec![],   // inter-package dep edges deferred to v1.1 per FR-004
    extra_annotations: btree_map! {
        "mikebom:source-type" => json!("erlang-hex"),
        "mikebom:evidence-kind" => json!("rebar-lock"),
    },
    // ... other PackageDbEntry fields default-initialized
}
```

### 3.2 Git deps from `rebar.lock`

```rust
PackageDbEntry {
    purl: Purl::new(format!(
        "pkg:generic/{name}@{resolved_ref}?vcs_url=git+{url}"
    ))?,
    name: name.clone(),
    version: Some(resolved_ref.clone()),
    hashes: vec![],
    depends: vec![],
    extra_annotations: btree_map! {
        "mikebom:source-type" => json!("erlang-git"),
        "mikebom:evidence-kind" => json!("rebar-lock"),
        "mikebom:vcs-declared-ref" => json!(declared_ref_form),  // "ref"/"tag"/"branch"
    },
}
```

### 3.3 OTP runtime placeholders from `*.app.src::applications:`

For each atom in `applications:` / `included_applications:` /
`optional_applications:` that doesn't match a `rebar.lock` entry:

```rust
PackageDbEntry {
    purl: Purl::new(format!("pkg:generic/{atom}@unspecified"))?,
    name: atom.clone(),
    version: Some("unspecified".to_string()),
    hashes: vec![],
    depends: vec![],
    extra_annotations: {
        let mut m = btree_map! {
            "mikebom:source-type" => json!("erlang-otp-runtime"),
            "mikebom:evidence-kind" => json!("app-src"),
        };
        if OTP_STDLIB_ALLOWLIST.contains(&atom.as_str()) {
            m.insert("mikebom:otp-stdlib".to_string(), json!("true"));
        }
        m
    },
}
```

Where `OTP_STDLIB_ALLOWLIST` is a `&'static [&'static str]` containing
the ~20 most-common Ericsson-distributed OTP runtime apps documented
in the spec Assumptions section. The list is informational only per Q1.

### 3.4 Main-module per `*.app.src`

For each parsed `AppSrcManifest`:

```rust
PackageDbEntry {
    purl: build_hex_purl(&manifest.app_name, &manifest.version, None),
    // ^ pkg:hex/<lc-name>@<version> (default Hex.pm, no private-org for main-modules)
    name: manifest.app_name.clone(),
    version: Some(manifest.version.clone()),
    hashes: vec![],
    depends: dep_atoms_union(&manifest, &declared_deps_for_this_app_src),
    // ^ NAMES, not bom-refs — name→bom-ref resolution happens at the
    //   orchestrator level (matches milestone-002 dpkg/apk convention;
    //   also matches milestone-140 elixir.rs main-module emission)
    extra_annotations: btree_map! {
        "mikebom:source-type" => json!("erlang-main-module"),
        "mikebom:component-role" => json!("main-module"),
        "mikebom:sbom-tier" => json!("source"),
        "mikebom:evidence-kind" => json!("app-src"),
        // edge-target annotations are emitted on the EDGE TARGETS' components
        // (not on the main-module itself); see §3.5 below
    },
}
```

### 3.5 `mikebom:erlang-app-dep-kind` annotation placement

The annotation is emitted on the EDGE-TARGET component (the dep), NOT
on the source main-module. This matches CycloneDX `scope` semantics
(scope-on-component, not scope-on-edge).

Resolution flow for each dep `<atom>` in the union per FR-012:
1. Find the `AppDepKind` for this atom per the Q3 precedence rule.
2. Look up the existing `PackageDbEntry` for `<atom>`:
   - First check `rebar.lock`-derived entries by name (Hex package case).
   - Then check OTP-runtime placeholder entries by atom (per §3.3).
   - If neither exists AND the kind is `BuildOnly` from `rebar.config` →
     this is a design-tier dep; emit a fresh `pkg:hex/<atom>@unspecified`
     with `mikebom:sbom-tier = "design"` per FR-005.
3. Set the annotation: `extra_annotations.insert("mikebom:erlang-app-dep-kind", json!(kind_str))`
   where `kind_str ∈ {"required", "included", "optional"}` per the
   AppDepKind enum (`BuildOnly` does not emit the annotation).

**Collision handling**: if the same atom appears in multiple `*.app.src`
files within an umbrella project with different kinds (e.g., required
in app-A's `applications:` AND optional in app-B's `optional_applications:`),
the precedence rule (required > included > optional) applies at the
SBOM-component level (since dedup collapses to one `PackageDbEntry`
per PURL). The annotation reflects the highest-binding kind across
all observed declarations.

## 4. Reader entry-point flow

```rust
pub fn read(scan_root: &Path) -> Result<Vec<PackageDbEntry>> {
    let mut entries = Vec::new();
    let lockfile_paths   = discover_rebar_locks(scan_root)?;
    let config_paths     = discover_rebar_configs(scan_root)?;
    let app_src_paths    = discover_app_src_files(scan_root)?;
    if lockfile_paths.is_empty() && config_paths.is_empty() && app_src_paths.is_empty() {
        return Ok(entries);  // FR-006 no-op
    }
    let mut lock_data: HashMap<PathBuf, Vec<LockEntry>> = HashMap::new();
    for p in &lockfile_paths {
        match parse_rebar_lock(p) {
            Ok(lock_entries) => { lock_data.insert(p.clone(), lock_entries); }
            Err(e) => warn!(path = ?p, error = %e, "erlang: failed to parse rebar.lock"),
        }
    }
    let mut config_data: HashMap<PathBuf, Vec<DeclaredDep>> = HashMap::new();
    for p in &config_paths {
        match parse_rebar_config(p) {
            Ok(deps) => { config_data.insert(p.clone(), deps); }
            Err(e) => warn!(path = ?p, error = %e, "erlang: failed to parse rebar.config"),
        }
    }
    let mut app_src_data: HashMap<PathBuf, AppSrcManifest> = HashMap::new();
    for p in &app_src_paths {
        match parse_app_src(p) {
            Ok(manifest) => { app_src_data.insert(p.clone(), manifest); }
            Err(e) => warn!(path = ?p, error = %e, "erlang: failed to parse *.app.src"),
        }
    }
    // 1. Emit lockfile-derived components per §3.1 + §3.2
    for (_, lock_entries) in &lock_data {
        for e in lock_entries {
            entries.push(lock_entry_to_pdb_entry(e)?);
        }
    }
    // 2. Emit main-modules per §3.4 (one per *.app.src)
    //    + union-derived OTP runtime placeholders per §3.3
    //    + edge-kind annotations per §3.5
    for (path, manifest) in &app_src_data {
        let nearby_config = find_nearest_rebar_config(path, &config_data);
        let main_mod_entry = emit_main_module(manifest, nearby_config, &lock_data, &mut entries)?;
        entries.push(main_mod_entry);
    }
    // 3. Design-tier emission (FR-005) — only if rebar.lock is absent
    //    AND rebar.config is present
    for (config_path, declared_deps) in &config_data {
        if !has_sibling_lockfile(config_path, &lock_data) {
            for dep in declared_deps {
                entries.push(design_tier_entry(dep)?);
            }
        }
    }
    Ok(entries)
}
```

**Performance note** (per Technical Context): ≤2 ms per lockfile entry.
HashMap lookups dominate the §3.5 annotation flow; for a heavy umbrella
project (~150 deps × 3 keyword lists), worst-case ~450 lookups, each
O(1) amortized.

## 5. Cross-format emission

### 5.1 CycloneDX 1.6

Each `PackageDbEntry` flows through the existing `mikebom-cli/src/generate/cyclonedx/builder.rs`
pipeline. The `mikebom:evidence-kind` value-set MUST be extended to
include `"rebar-lock"`, `"rebar-config"`, `"app-src"` (per the
builder's curated allowlist precedent).

If a main-module is promoted to `metadata.component`, the
`mikebom-cli/src/generate/cyclonedx/metadata.rs` curated property
allowlist MUST be extended to propagate `mikebom:erlang-app-dep-kind`
(and the milestone-140 `mikebom:umbrella-root` annotation IF the
reader chooses to emit one for umbrella roots — TBD during implementation;
defer to tasks.md whether umbrella-root annotation applies to rebar3
umbrellas, since rebar3 umbrella semantics differ slightly from Mix's).

### 5.2 SPDX 2.3

Per the milestone-071 annotation parity layer, the `mikebom:erlang-app-dep-kind`
annotation surfaces in SPDX 2.3 via the standard `annotations[]` shape
with `annotationType = "OTHER"` and `comment` carrying the structured
envelope per the existing `MikebomAnnotationCommentV1` shape.

### 5.3 SPDX 3.0.1

Per the milestone-079 SPDX 3 ID vocab work, annotations flow through
the document-scope `Annotation` shape with the appropriate
`software_*` discrimination. The same envelope shape applies.

## 6. Validation table

| Rule | Source | Enforcement site |
|---|---|---|
| Hex package names lowercased | research §R1 + Purl spec | `build_hex_purl` |
| Private-org `repository_url=` qualifier | research §R1 | `build_hex_purl` |
| `inner_sha256` only when present + non-empty | FR-011 | `LockEntry::HexModern` parse |
| Profile-scoped deps tagged `lifecycle-scope = "development"` | FR-008 | `DeclaredDep::profile` → `extra_annotations` |
| Q3 precedence required > included > optional | Q3 + research §R3 | `AppDepKind` resolution in §3.5 |
| Atoms not in lockfile emit OTP-runtime placeholder | Q1 + FR-003 | §3.3 emit step |
| `mikebom:otp-stdlib` annotation on allowlist members | Q1 + Assumptions | §3.3 emit step |
| Main-module fallback to dir basename | FR-012 | `parse_app_src` error path |
| Main-module version fallback to `0.0.0-unknown` | FR-012 | `parse_app_src` error path |
| Per-file warn-and-skip (don't abort scan) | FR-007 | top-level `read()` match arms |
| No-op on non-Erlang trees | FR-006 + SC-004 | top-level `read()` early return |
| No network access | FR-010 | (statically — no `reqwest`/`tokio` use in `erlang.rs`) |
