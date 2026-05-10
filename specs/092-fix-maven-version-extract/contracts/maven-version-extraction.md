# Contract — Maven version-extraction

Behavioral contract for the two production functions affected by milestone 092. Tests in `mikebom-cli/tests/maven_pom_version_extraction.rs` enforce these contracts.

## Contract 1 — `parse_pom_xml` populates `self_version` independently of `self_coord`

**Function**: `mikebom-cli/src/scan_fs/package_db/maven.rs:577 parse_pom_xml(bytes: &[u8]) -> PomXmlDocument`

**Pre-092 behavior**:
- `self_coord` populated iff project-level `<groupId>` AND `<artifactId>` both present.
- Project-level `<version>` was only persisted when `self_coord` was populated.
- A pom omitting `<groupId>` at project level would have its project-level `<version>` **silently discarded**.

**Post-092 behavior**:
- `self_coord` semantics **unchanged** (still requires all three).
- New: `self_version: Option<String>` field populated whenever the parser observed `<project>/<version>` (regardless of project-level `<groupId>` state).

**Test cases**:

| Input pom shape | `self_coord` | `self_version` |
|----------------|--------------|----------------|
| `<groupId>g</groupId><artifactId>a</artifactId><version>v</version>` (all three) | `Some(("g", "a", "v"))` | `Some("v")` |
| `<artifactId>a</artifactId><version>v</version>` (no groupId; inherits from parent) | `None` | `Some("v")` |
| `<artifactId>a</artifactId>` (no groupId, no version; full inheritance) | `None` | `None` |
| `<groupId>g</groupId><artifactId>a</artifactId>` (no version; intentional inheritance) | `None` (pre-092 path) — see note | `None` |
| `<artifactId>a</artifactId><version>${revision}</version>` (raw placeholder) | `None` | `Some("${revision}")` |

> **Note on row 4**: pre-092 the `if let (Some(g), Some(a)) = ...` block falls through to `unwrap_or_default() = ""`, producing `self_coord = Some(("g", "a", ""))`. Post-092 this is unchanged because `self_v = None`. The empty-string version case is already handled downstream by the `is_empty()` guard at line 3436. This row is not affected by the fix.

## Contract 2 — `build_maven_main_module_entry` prefers `self_version` over parent's version

**Function**: `mikebom-cli/src/scan_fs/package_db/maven.rs:3395 build_maven_main_module_entry(pom_path: &Path, doc: &PomXmlDocument, ctx: &MavenInheritanceContext) -> Option<PackageDbEntry>`

**Pre-092 resolution chain for `raw_version`**:

```rust
let raw_version = doc
    .self_coord
    .as_ref()
    .map(|c| c.2.clone())
    .or_else(|| doc.parent_coord.as_ref().map(|c| c.2.clone()))?;
```

**Post-092 resolution chain for `raw_version`**:

```rust
let raw_version = doc
    .self_coord
    .as_ref()
    .map(|c| c.2.clone())
    .or_else(|| doc.self_version.clone())                          // NEW step
    .or_else(|| doc.parent_coord.as_ref().map(|c| c.2.clone()))?;
```

**Behavioral guarantee**: for any pom where `<project>/<version>` is present, the emitted main-module PURL's version segment matches that value (after property substitution, per Contract 3).

**Test cases**:

| Input pom shape | Expected emitted PURL |
|----------------|----------------------|
| `<parent>/<groupId>org.apache.commons</groupId>/<version>64</version>` + `<artifactId>commons-lang3</artifactId><version>3.14.0</version>` | `pkg:maven/org.apache.commons/commons-lang3@3.14.0` |
| Same parent, project omits `<version>` entirely | `pkg:maven/org.apache.commons/commons-lang3@64` (intentional inheritance per FR-002) |
| Project has both `<groupId>` and `<version>` (no inheritance gap) | unchanged from pre-092 |

## Contract 3 — `resolve_pom_property_value` resolves `${project.version}` correctly when `self_coord = None`

**Function**: `mikebom-cli/src/scan_fs/package_db/maven.rs:3318 resolve_pom_property_value(raw: &str, self_doc: &PomXmlDocument, parent_doc: Option<&PomXmlDocument>) -> PropertyResolution`

**Pre-092 `"project.version"` arm**:

```rust
"project.version" => self_doc
    .self_coord
    .as_ref()
    .map(|c| c.2.clone())
    .or_else(|| {
        self_doc.parent_coord.as_ref().map(|c| c.2.clone())
    }),
```

**Post-092 `"project.version"` arm**:

```rust
"project.version" => self_doc
    .self_coord
    .as_ref()
    .map(|c| c.2.clone())
    .or_else(|| self_doc.self_version.clone())                     // NEW step
    .or_else(|| {
        self_doc.parent_coord.as_ref().map(|c| c.2.clone())
    }),
```

**Behavioral guarantee**: for a pom that omits project-level `<groupId>` but has project-level `<version>`, any dependency whose `<version>` is `${project.version}` resolves to the project's own version (not the parent's).

**Test case**:

| Input pom shape | Dep with `${project.version}` | Resolved dep version |
|----------------|------------------------------|----------------------|
| Project: omitted groupId, version=3.14.0; Parent: version=64; Dep: `<version>${project.version}</version>` | `pkg:maven/.../<dep>@3.14.0` | `3.14.0` |

## Contract 4 — `resolve_maven_property` (sibling helper)

**Function**: `mikebom-cli/src/scan_fs/package_db/maven.rs:723 resolve_maven_property(raw: &str, doc: &PomXmlDocument) -> MavenVersion`

**Decision**: in scope to mirror Contract 3 — the function's `"project.version"` arm at line 738 reads `doc.self_coord.as_ref().map(|(_, _, v)| v.clone())` and would return `None` (then `Placeholder` per line 743) for the bug-trigger case. Update to fall back to `doc.self_version`.

**Post-092 `"project.version"` arm**:

```rust
"project.version" => doc
    .self_coord
    .as_ref()
    .map(|(_, _, v)| v.clone())
    .or_else(|| doc.self_version.clone()),                         // NEW
```

**Test case**:

| Input | Expected |
|-------|----------|
| pom with omitted project-level groupId, version=3.14.0; raw=`${project.version}` | `MavenVersion::Resolved("3.14.0")` |

## Contract — non-regression

| Test fixture | Pre-092 emission | Post-092 emission |
|--------------|------------------|-------------------|
| `mikebom-cli/tests/fixtures/maven/maven-source-project/pom.xml` (existing — has groupId) | unchanged | unchanged (golden byte-stable) |
| `mikebom-cli/tests/fixtures/transitive_parity/maven/pom.xml` (commons-lang3 — omitted groupId) | `pkg:maven/org.apache.commons/commons-lang3@64` (BUG) | `pkg:maven/org.apache.commons/commons-lang3@3.14.0` (FIX) |
| Any other Maven-using fixture in the workspace | unchanged | unchanged |

## Out-of-scope (deferred to future milestones per spec.md "Out of Scope")

- Maven cache-empty fallback (Track A of issue #175).
- Variable-substitution from external property files (`${env.X}`, settings.xml).
- Reactor parent-aggregation logic that walks `<modules>` with cross-module property inheritance beyond the existing Phase-1 implementation.
- BOM-import resolution for `dependencyManagement/<dependency>/<scope>import</scope>`.
