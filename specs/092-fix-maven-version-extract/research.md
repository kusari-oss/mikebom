# Research — milestone 092 Maven version-extraction bug

Phase 0 investigation. Three decision points; all resolved without further clarification.

## §1 — Bug location: `PomXmlDocument` constructor in `parse_pom_xml`

**Finding**: the bug is in **the constructor** at `mikebom-cli/src/scan_fs/package_db/maven.rs:703-707`, NOT in the per-element walker (lines 612-629) and NOT in the property substitution path (lines 3318-3387) and NOT in `build_maven_main_module_entry` (lines 3395-3439).

The walker correctly distinguishes `<project>/<version>` (writes to local `self_v: Option<String>`) from `<project>/<parent>/<version>` (writes to local `parent_v: Option<String>`). When called against the commons-lang3 fixture pom.xml, by EOF the walker holds:

| local | value           | source                         |
|-------|-----------------|--------------------------------|
| `self_g` | `None`        | omitted at project level       |
| `self_a` | `Some("commons-lang3")` | `<project>/<artifactId>` |
| `self_v` | `Some("3.14.0")`        | `<project>/<version>`    |
| `parent_g` | `Some("org.apache.commons")` | `<parent>/<groupId>` |
| `parent_a` | `Some("commons-parent")`     | `<parent>/<artifactId>` |
| `parent_v` | `Some("64")`                  | `<parent>/<version>`   |

The constructor at line 703 is:

```rust
if let (Some(g), Some(a)) = (self_g.clone(), self_a.clone()) {
    let v = self_v.clone().or_else(|| parent_v.clone()).unwrap_or_default();
    doc.self_coord = Some((g, a, v));
}
doc.self_artifact_id = self_a.clone();  // line 711
if let (Some(g), Some(a), Some(v)) = (parent_g, parent_a, parent_v) {
    doc.parent_coord = Some((g, a, v));
}
```

Because `self_g = None`, the first `if let` fails. `self_v = Some("3.14.0")` is **discarded** — never written into `doc`. The doc-comment on `PomXmlDocument` at line 540-546 already acknowledged this gap for `<artifactId>` (resolved by adding `self_artifact_id` in milestone 007); the same gap exists for `<version>` and was never closed.

Then in `build_maven_main_module_entry` at line 3412:

```rust
let raw_version = doc
    .self_coord                                       // None
    .as_ref()
    .map(|c| c.2.clone())                             // None
    .or_else(|| doc.parent_coord.as_ref().map(|c| c.2.clone()))?;  // Some("64")
```

`raw_version = "64"`. Bug.

**Decision**: add a `self_version: Option<String>` field to `PomXmlDocument`, populated from local `self_v` regardless of `self_g`/`self_a` state — exactly parallel to `self_artifact_id`. Update `build_maven_main_module_entry`'s `raw_version` resolution to prefer `doc.self_version` over `parent_coord.2`.

**Rationale**:
- Strictly additive change to `PomXmlDocument`; cannot break any consumer keying on the existing fields.
- Mirrors the milestone-007 precedent for `self_artifact_id`. Same pattern, same justification.
- Keeps `self_coord` semantics intact: it remains "fully-resolved (g, a, v) tuple, all three project-level". Any consumer doing `doc.self_coord.as_ref()` (e.g., `MavenInheritanceContext::build_from_poms` keying `by_coord` on `self_coord`) sees no behavior change for the bug-trigger case (the fixture's child POM was already not in `by_coord` because `self_coord = None`; that's the parent inheritance lookup, which keys on the **child's** `parent_coord`, not on its own `self_coord`).
- The fix preserves the milestone-053-onward FR-005 "no regressions on populated POMs" by leaving `self_coord` semantics untouched.

**Alternatives considered**:
- **Relax the `self_coord` constructor to fall back to parent groupId**: would change `self_coord`'s definition from "all three project-level" to "all three resolved-via-inheritance". Touches `MavenInheritanceContext::build_from_poms` (line 3293) which inserts into `by_coord` keyed on `self_coord`; relaxing the gate could let child POMs claim the same `(g, a, v)` as their parents in `by_coord`, breaking parent-doc lookup for siblings. Higher blast radius. Rejected.
- **Inline `self_v` plumbing through `build_maven_main_module_entry`'s caller chain**: requires changing `parse_pom_xml`'s return type or threading an extra arg. Same effect as adding the field, but with broader API surface change. Rejected.
- **Recompute version from `self_coord.or_else(re-parse pom)`**: re-parses the file; performance regression and semantics drift. Rejected.

## §2 — Property substitution preserved

**Finding**: the existing `resolve_pom_property_value` path at lines 3318-3387 routes `${project.version}` through `self_doc.self_coord.as_ref().map(|c| c.2.clone()).or_else(|| self_doc.parent_coord.as_ref().map(|c| c.2.clone()))` (lines 3351-3357). When `self_coord = None` post-fix (because `self_g` is still `None`), this falls through to `parent_coord.2` — which is **wrong** for the same reason as the main-module case.

**Decision**: in `resolve_pom_property_value`'s `"project.version"` arm, prefer `self_doc.self_version` when present, falling back to `self_coord.2`, then `parent_coord.2`. Single-line change.

**Rationale**:
- US2 (preserve `${revision}` etc.) requires `${project.version}` to resolve correctly when used inside `<dependencies>/<dependency>/<version>`. Without this update, a child POM that omits project-level `<groupId>` and uses `${project.version}` for sibling-module deps would emit dependency edges with the parent's version — the same class of bug, one layer deeper.
- The property-substitution call site is only reached during dep-edge construction; the additional `self_version` lookup adds zero new failure modes (both fields are already populated by the same parser pass).

**Alternatives considered**:
- **Leave property substitution as-is and only fix the main-module path**: would create a second-order inconsistency where main-module-version is correct but `${project.version}`-resolved dep versions are stale. Rejected.

## §3 — Edge-case handling

**Decision**: handle the four edge cases listed in `spec.md` as follows:

| Edge case | Handling |
|-----------|----------|
| Project has no `<parent>` block at all | `parent_coord = None`; `self_version` populated; main-module emits with project version. No change vs. existing behavior. |
| Project has `<parent>` but no project-level `<version>` | `self_version = None`; `build_maven_main_module_entry` falls back to `parent_coord.2` per existing line 3416 logic. **Intentional inheritance**, not the bug. |
| Project has both `${revision}`-style version AND parent | `self_version = Some("${revision}")` (raw); `resolve_pom_property_value` resolves via `<properties>/<revision>` lookup; main-module emits the resolved value. |
| Multi-module reactor with mixed inheritance | each child POM is parsed independently; `parse_pom_xml` is per-file, so `self_version` is per-child. No cross-child interference. |
| Malformed pom.xml (parser fails mid-stream) | quick-xml's `Err(_)` arm at line 698 already breaks out; `self_version = None` if `</version>` was never reached. Existing behavior; main-module emission silently skipped per FR-001 step 4 (lines 3436-3438 `is_empty` guard). |

**Rationale**: the fix is purely additive — every prior-working case stays working because `self_version` only takes precedence over `parent_coord.2` when `self_version.is_some()`. When the project's own `<version>` is absent (intentional inheritance), the fallback chain matches the pre-092 behavior exactly.

## Coverage map

| Spec section | Resolution |
|--------------|------------|
| FR-001 (use project's own `<version>` when present) | §1 → `self_version` field + `build_maven_main_module_entry` precedence change. |
| FR-002 (fall back to parent's `<version>` when project-level missing) | §3 → existing `parent_coord.2` fallback chain unchanged. |
| FR-003 (no `<parent>` block) | §3 → `parent_coord = None`; main-module emits with project version. |
| FR-004 (`${revision}` property substitution) | §2 → `resolve_pom_property_value`'s `project.version` arm prefers `self_version`. |
| FR-005 (no regressions on populated POMs) | §1 → `self_coord` semantics unchanged; consumers untouched. |
| FR-006 (transitive_parity_maven baseline) | golden regen flips `@64` → `@3.14.0`; baseline already expected `@3.14.0` per milestone-083 spec. |
| FR-007 (no scope creep beyond the bug fix) | §1 → ~10 lines production change; struct field add + 2 reads. |
| FR-008 (no new dependencies) | §1+§2 → existing `quick-xml`, `serde`, `tracing` only. |
| Constitution V audit | no `mikebom:*` properties added; standards-native PURL string change only. |
| Constitution X transparency | improves transparency (correct version emitted); no opaque metadata. |

All open spec questions resolved. Ready for Phase 1 (data-model + contracts + quickstart).
