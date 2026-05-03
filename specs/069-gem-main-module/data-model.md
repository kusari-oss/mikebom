# Data Model: gem source-tree main-module component

## Entities

### GemMainModuleEntry (no new Rust type — constrained `PackageDbEntry`)

| Field | Value | Source | FR |
|-------|-------|--------|-----|
| `purl` | `pkg:gem/<name>@<version>` | `s.name` (literal) + `s.version` (literal or placeholder) via existing `build_gem_purl` | FR-001 |
| `name` | `s.name` literal | `parse_gemspec_full` | FR-001 |
| `version` | literal `s.version` or `"0.0.0-unknown"` placeholder | `parse_gemspec_full` with placeholder fallback | FR-001 |
| `source` | `Some("path+file://<absolute-gemspec-dir>")` | filesystem walker | (existing convention) |
| `lifecycle_scope` | `None` | n/a (Runtime by default) | (out of scope) |
| `sbom_tier` | `Some("source")` | constant | FR-006 |
| `extra_annotations` | BTreeMap with `mikebom:component-role: "main-module"` | constant | FR-004 |
| `parent_purl` | `None` | constant | FR-001a |
| `depends` | `Vec<String>` from `parse_gemspec_groups` (`s.add_dependency` / `s.add_runtime_dependency` / `s.add_development_dependency` keys) | gemspec dep sections | FR-007 |
| `licenses` | `vec![]` | constant | FR-005 |
| `hashes` | `vec![]` | constant | (synthetic) |

### DroppedDuplicate (private helper struct)

Same shape as cargo (064) / npm (066) / pip (068) — `purl`, `kept_path`, `dropped_path`. Returned from `dedup_gem_main_modules_by_purl`.

## Relationships

### Direct-dep edges from main-module to dep targets

```text
Relationship {
    from: <gem-main-module-purl>,           // pkg:gem/<name>@<version>
    to: <dep-target-purl>,                  // pkg:gem/<dep>@<version>
    relationship_type: DependsOn,
    provenance: {
        source: "<absolute-gemspec-path>",
        data_type: "gemspec-direct-dep",
    },
}
```

The existing `parse_gemspec_groups` helper extracts dep-section keys grouped by `runtime` / `development` / etc. Phase A populates `depends` with the union (post-scope-filter); the existing `name_to_purl` resolution emits edges to matching `Gemfile.lock`-derived components, dropping danglers.

### DESCRIBES relationship

Inherits multi-DESCRIBES wiring from milestone 064 + #127.

## State transitions

None.

## Validation rules

| Rule | Source | Failure mode |
|------|--------|--------------|
| `*.gemspec` MUST contain a literal `s.name` (or `spec.name`) assignment | FR-001 | Skip silently |
| Non-literal `s.version` (constant ref, expression) → `0.0.0-unknown` placeholder | FR-001 | Deterministic |
| `*.gemspec` inside `vendor/`, `gems/`, `specifications/`, `.bundle/` MUST NOT be discovered for main-module | FR-003 | Walker excludes these paths |
| Application-style projects (no `*.gemspec`) MUST NOT emit a main-module | FR-002 | No emission |
| Same-PURL collisions dedup, first-wins | FR-011 | `tracing::warn!` |

## Reuses from milestones 053+064+066+068+#127 + existing gem.rs

- `parse_gemspec_full` (existing `gem.rs:947`) — regex-based literal-string extractor
- `parse_gemspec_groups` (existing `gem.rs:491`) — dep-section name extractor
- `build_gem_purl` (existing `gem.rs:234`) — PURL builder with percent-encoding
- C40-tag-driven CDX `metadata.component` selector (milestone 064)
- C40-tag-driven SPDX `primaryPackagePurpose` predicate (milestone 053+064)
- Multi-root `documentDescribes` + per-root DESCRIBES (#127)
- Multi-root SPDX 3 `rootElement` + per-root describes (#127)
- Cargo's `dedup_main_modules_by_purl` pattern (milestone 064 T010) — copy-adapt to gem

## Does NOT introduce

- No new public Rust type
- No new crate dependency
- No new CLI flag
- No new SBOM annotation key
- No subprocess calls (no Ruby interpreter shellout per A9)
- No version-inheritance / workspace-context map
