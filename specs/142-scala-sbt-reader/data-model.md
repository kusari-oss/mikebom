# Data Model — milestone 142 Scala/SBT reader (Phase 1)

Defines the parsed representations of `build.sbt`, `*.sbt.lock`, `project/Dependencies.scala`, and `project/build.properties` that the reader builds in-memory, and how they map onto `PackageDbEntry` instances flowing through the existing `read_all` pipeline.

## 1. Input artifacts

### 1.1 `build.sbt`

Scala-DSL build definition. Top-level setting assignments (`:=` operator) plus dep declarations (`+=` / `++=` operators on `libraryDependencies`). Optional `lazy val <name> = project.in(file("<path>"))` blocks for multi-project builds.

Regex-extracted via patterns documented in research §R4.

### 1.2 `*.sbt.lock`

JSON lockfile produced by the sbt-dependency-lock plugin. Discovered via the `*.sbt.lock` glob with Q3 content-shape validation gate (top-level `lockVersion` + `modules` keys required). Schema versions 1 (older plugin releases) and 2 (current) both supported per research §R2.

### 1.3 `project/Dependencies.scala`

Convention sidecar. Scala source file with `val`/`def` named dep references. v1 regex-extracts the common `val foo = "group" %% "artifact" % "version"` pattern; complex computed forms drop silently.

### 1.4 `project/build.properties`

Java-properties-format file pinning the SBT version (`sbt.version=1.10.0`). May embed `scala.version=...` (rare); used only for the Q1 inference cascade fallback rung when `build.sbt` doesn't declare `scalaVersion`.

## 2. Parsed intermediate types

### 2.1 `SbtLockEntry` (private to `scala.rs`)

```rust
#[derive(Debug, Clone)]
struct SbtLockEntry {
    org: String,
    name: String,        // includes Scala-version suffix (already baked in by the plugin)
    version: String,
    configurations: Vec<String>,
    sha256: Option<String>,  // FR-011 best-effort — only when v2 checksums present
}
```

**Validation rules**:
- All three `org` / `name` / `version` strings preserved verbatim from the lockfile JSON.
- `configurations` typically `["compile"]` or `["compile", "runtime"]`; `"test"` triggers FR-008 dev-scope tagging.
- `sha256` only populated when v2 schema's `checksums` array contains an entry with `"type": "SHA-256"` (case-insensitive on the algorithm name).

### 2.2 `DeclaredSbtDep` (private to `scala.rs`)

Parsed from `libraryDependencies` blocks in `build.sbt` for design-tier fallback (FR-005) AND for main-module-depends-union per FR-009 + FR-012.

```rust
#[derive(Debug, Clone)]
struct DeclaredSbtDep {
    group: String,
    artifact: String,      // BARE artifactId (no Scala suffix appended yet)
    declaration_kind: DeclKind,  // SinglePercent | DoublePercent | TriplePercent
    version: String,       // raw version string (constraint preserved)
    configuration: Option<String>,  // Some("Test") / Some("Provided") / None (Compile default)
    subproject: Option<String>,     // owning subproject name (None = root)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclKind {
    SinglePercent,   // "group" % "artifact" % "ver" — NO suffix
    DoublePercent,   // "group" %% "artifact" % "ver" — suffix from Q1 cascade
    TriplePercent,   // "group" %%% "artifact" % "ver" — warn-and-skip (Out-of-Scope)
}
```

**Validation rules**:
- `artifact` is the BARE artifactId as written in `build.sbt`. The Scala-version suffix is appended at PURL-build time via `apply_scala_suffix(declaration_kind, artifact, scala_version)`.
- `configuration` extracted from the trailing `% Test` / `% Provided` / etc. suffix. Defaults to `None` (Compile).
- `subproject` populated when the declaration was extracted from inside a `.settings(...)` block tied to a specific `lazy val` subproject, OR when the declaration came from a `<subdir>/build.sbt` file (subproject = `<subdir>` basename or matching `lazy val` name).

### 2.3 `SbtSubproject` (private to `scala.rs`)

Per Q2 union discovery, one entry per surfaced subproject (whether via `lazy val` parsing or filesystem walk).

```rust
#[derive(Debug, Clone)]
struct SbtSubproject {
    name: String,           // from lazy val identifier OR subdir basename fallback
    project_dir: PathBuf,   // canonicalized absolute path to subproject root
    build_sbt_path: Option<PathBuf>,  // some when on-disk file exists
    declared_in_root: bool, // true when lazy val block in root build.sbt declared it
}
```

**Validation rules**:
- `project_dir` always canonicalized via `std::fs::canonicalize` for dedup correctness.
- When both `declared_in_root == true` AND `build_sbt_path.is_some()`, the `lazy val` name + path wins per Q2 precedence rule (operator-chosen identity preserved).

### 2.4 `SbtMainModule` (private to `scala.rs`)

Per-subproject main-module candidate.

```rust
#[derive(Debug, Clone)]
struct SbtMainModule {
    subproject: SbtSubproject,
    organization: Option<String>,  // from `organization := "..."`
    name_setting: Option<String>,  // from `name := "..."`
    version_setting: Option<String>,  // from `version := "..."`
    scala_version: Option<String>, // from `scalaVersion := "..."`
}
```

**Validation rules** (per FR-012 cascade):
- `organization` fallback: `"unknown"` when not declared.
- `name_setting` fallback: parent-directory basename when not declared.
- `version_setting` fallback: `"0.0.0-unknown"` when not declared.
- `scala_version`: drives the Q1 inference cascade for the main-module's own PURL when declared via `%%` style. Main-module PURL ALWAYS uses `%%` semantics (the SBT-published artifact is Scala-version-suffixed if the project is Scala-2.x or Scala-3.x).

### 2.5 `ScalaVersionSource` (private to `scala.rs`)

Tracks the Q1 inference cascade's outcome per subproject. Used to annotate emitted components with `mikebom:scala-version-source`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalaVersionSource {
    BuildSbtExplicit,        // (1) explicit scalaVersion := "..." in build.sbt
    BuildPropertiesEmbedded, // (2) project/build.properties carried scala.version=
    DefaultFallback,         // (3) hardcoded _2.13 — emits annotation per Q1
}
```

## 3. Output mapping → `PackageDbEntry`

### 3.1 Lockfile-derived components

For each `SbtLockEntry`:

```rust
PackageDbEntry {
    purl: Purl::new(&format!("pkg:maven/{org}/{name}@{version}",
        org = entry.org, name = entry.name, version = entry.version))?,
    // ^ entry.name ALREADY includes the Scala-version suffix per FR-002
    name: entry.name.clone(),
    version: Some(entry.version.clone()),
    hashes: entry.sha256.as_ref().map(|h|
        vec![ContentHash::with_algorithm(HashAlgorithm::Sha256, h)])
        .unwrap_or_default(),
    depends: vec![],  // inter-package edges deferred to v1.1 per FR-004
    extra_annotations: btree_map! {
        "mikebom:source-type" => json!("scala-sbt-lock"),
        "mikebom:evidence-kind" => json!("sbt-lock"),
    },
    lifecycle_scope: if entry.configurations.iter().any(|c| c.eq_ignore_ascii_case("test"))
        { Some(LifecycleScope::Development) } else { Some(LifecycleScope::Runtime) },
    // ... other PackageDbEntry fields default-initialized
}
```

### 3.2 Design-tier components (from `build.sbt` when no lockfile present)

For each `DeclaredSbtDep` (where `declaration_kind != TriplePercent` — `%%%` warns-and-skips):

```rust
let artifactid_with_suffix = apply_scala_suffix(
    declaration_kind,
    &dep.artifact,
    &subproject_scala_version,  // from Q1 cascade per §2.5
);
let purl = Purl::new(&format!("pkg:maven/{group}/{artifact}@{version}",
    group = dep.group,
    artifact = artifactid_with_suffix,
    version = sanitize_purl_version(&dep.version),
))?;

PackageDbEntry {
    purl,
    name: dep.artifact.clone(),   // PackageDbEntry.name uses BARE artifact (without suffix)
    version: Some(dep.version.clone()),
    extra_annotations: {
        let mut m = btree_map! {
            "mikebom:source-type" => json!("scala-sbt-design"),
            "mikebom:evidence-kind" => json!("sbt-build"),
            "mikebom:requirement-range" => json!(dep.version.clone()),
        };
        if declaration_kind == DeclKind::DoublePercent {
            // Per Q1: surface the Scala-version-source for transparency.
            m.insert("mikebom:scala-version-source".to_string(),
                     json!(scala_version_source.to_annotation_value()));
            // Returns "build-sbt-explicit" | "build-properties-embedded" | "default-fallback"
        }
        m
    },
    sbom_tier: Some("design".to_string()),
    lifecycle_scope: match dep.configuration.as_deref() {
        Some("Test") => Some(LifecycleScope::Development),
        _ => Some(LifecycleScope::Runtime),
    },
    // ...
}
```

### 3.3 Main-module per subproject (FR-012)

For each `SbtMainModule`:

```rust
let organization = main.organization.clone().unwrap_or_else(|| "unknown".to_string());
let name = main.name_setting.clone()
    .or_else(|| main.subproject.project_dir.file_name()
        .and_then(|s| s.to_str()).map(String::from))
    .unwrap_or_else(|| "unknown".to_string());
let version = main.version_setting.clone()
    .unwrap_or_else(|| "0.0.0-unknown".to_string());

// Main-module artifact uses %% semantics (Scala-version-suffixed when scalaVersion parses).
let scala_version_source_for_main = derive_scala_version(&main, &project_build_properties);
let artifactid = match scala_version_source_for_main {
    Some((suffix, _source)) => format!("{name}_{suffix}"),
    None => name.clone(),  // Pure-Java SBT project (rare); no suffix
};

let purl = Purl::new(&format!("pkg:maven/{organization}/{artifactid}@{version}"))?;

PackageDbEntry {
    purl,
    name,
    version: Some(version),
    extra_annotations: {
        let mut m = btree_map! {
            "mikebom:component-role" => json!("main-module"),
            "mikebom:source-type" => json!("scala-main-module"),
        };
        // Per F6 remediation: surface the Scala-version-source on the
        // main-module so operators can distinguish explicit vs cascaded
        // suffix derivation (matches the design-tier %% deps convention).
        if let Some((_suffix, source)) = scala_version_source_for_main {
            m.insert(
                "mikebom:scala-version-source".to_string(),
                json!(source.to_annotation_value()),
            );
        }
        m
    },
    sbom_tier: Some("source".to_string()),  // OR "design" when no lockfile (mode-aware)
    depends: <subproject's DeclaredSbtDep names>,  // populated per FR-004
    // ...
}
```

## 4. `apply_scala_suffix` algorithm (drives Q1 + R3)

```rust
fn apply_scala_suffix(
    kind: DeclKind,
    bare_artifact: &str,
    scala_version: Option<&str>,
) -> String {
    match kind {
        DeclKind::SinglePercent => bare_artifact.to_string(),  // No suffix
        DeclKind::TriplePercent => bare_artifact.to_string(),  // Warned-and-skipped upstream
        DeclKind::DoublePercent => {
            let suffix = match scala_version {
                Some(v) if v.starts_with("3.") || v == "3" => "_3".to_string(),
                Some(v) => {
                    // Scala 2.x: major.minor; drop patch.
                    let mut iter = v.split('.');
                    let major = iter.next().unwrap_or("2");
                    let minor = iter.next().unwrap_or("13");
                    format!("_{major}.{minor}")
                }
                None => "_2.13".to_string(),  // Q1 default fallback
            };
            format!("{bare_artifact}{suffix}")
        }
    }
}
```

**Test coverage**: unit tests in `scala.rs` exercise each branch:
- `apply_scala_suffix(SinglePercent, "postgresql", Some("2.13.12"))` → `"postgresql"`
- `apply_scala_suffix(DoublePercent, "cats-core", Some("2.13.12"))` → `"cats-core_2.13"`
- `apply_scala_suffix(DoublePercent, "cats-core", Some("3.3.1"))` → `"cats-core_3"`
- `apply_scala_suffix(DoublePercent, "cats-core", None)` → `"cats-core_2.13"` (Q1 default)

## 5. Reader entry-point flow

```rust
pub fn read(rootfs: &Path, _include_dev: bool, exclude_set: &ExclusionSet) -> Vec<PackageDbEntry> {
    let mut out: Vec<PackageDbEntry> = Vec::new();
    let mut seen_purls: HashSet<String> = HashSet::new();

    // Phase A — discover all artifacts.
    let lockfile_candidates = discover_sbt_locks(rootfs, exclude_set);
    let build_sbt_paths = discover_build_sbts(rootfs, exclude_set);
    let build_properties_paths = discover_build_properties(rootfs, exclude_set);
    let dependencies_scala_paths = discover_dependencies_scala(rootfs, exclude_set);

    // FR-006: no-op when no Scala artifacts present.
    if lockfile_candidates.is_empty()
        && build_sbt_paths.is_empty()
        && build_properties_paths.is_empty()
        && dependencies_scala_paths.is_empty() {
        return out;
    }

    // Phase B — parse all build.sbt files into per-subproject SbtBuildInfo,
    // running Q2 union discovery across root + subdir surfaces.
    let subprojects = discover_subprojects(&build_sbt_paths);  // Q2

    // Phase C — parse all *.sbt.lock files (with Q3 content-shape validation).
    let lock_data = parse_lockfiles(&lockfile_candidates);

    // Phase D — emit lockfile-derived components (per §3.1).
    for entry in lock_data.iter() {
        let component = build_lockfile_component(entry);
        let purl_key = component.purl.as_str().to_string();
        if seen_purls.insert(purl_key) {
            out.push(component);
        }
    }

    // Phase E — for each subproject: emit main-module (per §3.3)
    //   + design-tier deps when no sibling lockfile (per §3.2).
    for subproj in subprojects.iter() {
        let has_lockfile = lock_data.iter().any(|e| /* matches subproj */);
        let main = build_main_module(subproj, &lock_data);
        if seen_purls.insert(main.purl.as_str().to_string()) {
            out.push(main);
        }
        if !has_lockfile {
            for design_dep in collect_design_tier_deps(subproj) {
                if seen_purls.insert(design_dep.purl.as_str().to_string()) {
                    out.push(design_dep);
                }
            }
        }
    }

    out
}
```

**Performance note** (per Technical Context): ≤2 ms per lockfile entry. HashMap/HashSet lookups dominate the seen_purls dedup; for ~400-component heavy multi-project builds, ~400 lookups O(1) amortized.

## 6. Cross-format emission

### 6.1 CycloneDX 1.6

Each `PackageDbEntry` flows through the existing `mikebom-cli/src/generate/cyclonedx/builder.rs` pipeline. The `mikebom:evidence-kind` value-set MUST be extended to include `"sbt-lock"` and `"sbt-build"` (per the builder's curated allowlist precedent — same shape as milestone-141's `"rebar-lock"` / `"rebar-config"` / `"app-src"` extensions).

The `mikebom:source-type` allowlist (if curated) extends with `"scala-sbt-lock"` / `"scala-sbt-design"` / `"scala-main-module"`. If `mikebom:scala-version-source` needs explicit allowlisting in the builder, add it — verify by examining how milestone-141's `mikebom:erlang-app-dep-kind` was wired (in practice the builder is permissive about `mikebom:*` namespace; only `evidence-kind` is strict-allowlist-validated).

### 6.2 SPDX 2.3

Per the milestone-071 annotation parity layer, the `mikebom:scala-version-source` annotation surfaces in SPDX 2.3 via the standard `annotations[]` shape with the `MikebomAnnotationCommentV1` envelope. No special handling needed.

### 6.3 SPDX 3.0.1

Same as 2.3 — flows through the document-scope `Annotation` shape with the standard envelope.

## 7. Validation table

| Rule | Source | Enforcement site |
|---|---|---|
| `pkg:maven/` PURL shape | research §R1 + Purl spec | `build_lockfile_component` + `build_main_module` |
| Scala-version suffix in `name` slot (NOT qualifier) | research §R1 + Maven Central reality | `apply_scala_suffix` |
| `_3` for Scala 3.x (NOT `_3.x`) | research §R3 | `apply_scala_suffix` |
| Q1 inference cascade for design-tier `%%` deps | spec FR-005 + research §R3 | `derive_scala_version` |
| Q3 content-shape validation on `*.sbt.lock` | spec FR-002 + research §R2 | `parse_lockfiles` |
| Test configuration → lifecycle-scope = dev | spec FR-008 | §3.1 + §3.2 emit steps |
| `%%%` warn-and-skip | spec Out-of-Scope | §3.2 (skip branch) |
| Main-module fallback (organization=unknown, name=dir basename, version=0.0.0-unknown) | spec FR-012 | `build_main_module` |
| Q2 union discovery dedup by canonicalized path | spec FR-009 + research §R5 | `discover_subprojects` |
| Per-file warn-and-skip (don't abort scan) | spec FR-007 | `parse_lockfiles` + per-file match arms |
| No-op on non-Scala trees | spec FR-006 + SC-004 | top-level `read()` early return |
| No network access | spec FR-010 | static — no `reqwest`/`tokio` use in `scala.rs` |
