# Data Model — milestone 092

Single struct delta. No schema-level changes. No JSON-output shape changes.

## `PomXmlDocument` (existing struct)

**Path**: `mikebom-cli/src/scan_fs/package_db/maven.rs:530`

### Pre-092 shape

```rust
#[derive(Clone, Debug, Default)]
pub(crate) struct PomXmlDocument {
    pub self_coord: Option<(String, String, String)>, // (groupId, artifactId, version)
    pub parent_coord: Option<(String, String, String)>,
    pub properties: HashMap<String, String>,
    pub dependencies: Vec<PomDependency>,
    pub dependency_management: Vec<PomDependency>,
    /// Raw `<project>/<artifactId>` element value, even when the POM
    /// lacks a `<groupId>` or `<version>` ...
    pub self_artifact_id: Option<String>,
    pub modules: Vec<String>,
}
```

### Post-092 shape

```rust
#[derive(Clone, Debug, Default)]
pub(crate) struct PomXmlDocument {
    pub self_coord: Option<(String, String, String)>, // (groupId, artifactId, version)
    pub parent_coord: Option<(String, String, String)>,
    pub properties: HashMap<String, String>,
    pub dependencies: Vec<PomDependency>,
    pub dependency_management: Vec<PomDependency>,
    pub self_artifact_id: Option<String>,
    /// Raw `<project>/<version>` element value, even when the POM
    /// lacks a `<groupId>` (which forces `self_coord = None` because
    /// `self_coord` requires all three project-level GAV components).
    /// Populated whenever `<project>/<version>` is present, parallel
    /// to `self_artifact_id`. Sidecar readers prefer this over
    /// `parent_coord.2` when emitting main-module entries.
    /// Milestone 092 / FR-001.
    pub self_version: Option<String>,                 // NEW
    pub modules: Vec<String>,
}
```

### Field semantics

| Field | When populated | Used by |
|-------|---------------|---------|
| `self_coord` | All three project-level GAV components present (g, a, v) | `MavenInheritanceContext::by_coord`, `resolve_pom_property_value`, `resolve_maven_property` |
| `self_artifact_id` | `<project>/<artifactId>` present (regardless of g/v) | sidecar readers, milestone-007 child-POM identity |
| `self_version` (NEW) | `<project>/<version>` present (regardless of g/a) | `build_maven_main_module_entry` raw_version resolution; `resolve_pom_property_value`'s `project.version` arm |
| `parent_coord` | All three parent-block GAV components present | inheritance fallback, `MavenInheritanceContext::parent_doc` |

### Compatibility

- Strictly additive. No existing field removed or renamed.
- Default value `None` matches pre-existing behavior for any consumer not yet aware of the field.
- All existing call sites continue to work without modification; only `build_maven_main_module_entry` and `resolve_pom_property_value` are updated to read the new field.

## No other model changes

- `PomDependency`: unchanged.
- `MavenVersion`: unchanged.
- `MavenInheritanceContext`: unchanged (keys on `self_coord`, which is unaffected).
- `PackageDbEntry`: unchanged (only the `version` string content differs in fixed cases).
