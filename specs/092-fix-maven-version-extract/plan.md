# Implementation Plan: Fix Maven pom.xml version-extraction bug

**Branch**: `092-fix-maven-version-extract` | **Date**: 2026-05-08 | **Spec**: [spec.md](spec.md)

## Summary

Maven main-module entries can emit the **parent** POM's version instead of the project's own — observed concretely in the milestone-083 transitive_parity fixture as `pkg:maven/org.apache.commons/commons-lang3@64` (commons-parent's version) instead of `@3.14.0` (commons-lang3's project-level version).

**Root cause** (Phase 0 confirmed): `parse_pom_xml` at `mikebom-cli/src/scan_fs/package_db/maven.rs:703-707` only populates `PomXmlDocument.self_coord` when **both** project-level `<groupId>` AND project-level `<artifactId>` are present. Maven's POM inheritance routinely permits omitting `<groupId>` (inherited from `<parent>`); the commons-lang3 fixture exercises exactly this. With `self_g = None`, the entire `self_coord` block is skipped — the parsed `self_v = "3.14.0"` is **discarded**. Then `build_maven_main_module_entry` at line 3412 falls back to `parent_coord.2` (= "64").

**Fix shape**: parallel to the existing `self_artifact_id: Option<String>` field (added in milestone 007 for the same inheritance pattern), add a `self_version: Option<String>` field that's populated from `self_v` regardless of `self_g`/`self_a` state. Update `build_maven_main_module_entry`'s `raw_version` resolution to prefer `doc.self_version` over `doc.parent_coord.2`. Surgical change; bug-fix-only milestone.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–091; no nightly required for this user-space-only bug fix).
**Primary Dependencies**: existing only — `quick-xml = "0.31"` (already used by `parse_pom_xml`), `serde`/`serde_json`, `tracing`, `anyhow`. **No new crates.**
**Storage**: N/A — pure metadata transform on the maven main-module emission code path; no caches, no persistence.
**Testing**: `cargo +stable test --workspace` (existing). New unit test in `mikebom-cli/src/scan_fs/package_db/maven.rs` for the `<groupId>`-omitted parsing case; new integration test in `mikebom-cli/tests/maven_pom_version_extraction.rs` for the regression and a `${revision}` property-substitution case (FR-004 / SC-005).
**Target Platform**: same as workspace — Linux x86_64, macOS aarch64, Linux aarch64.
**Project Type**: Rust workspace, single binary (mikebom-cli).
**Performance Goals**: zero per-component cost change. Adding one `Option<String>` field to `PomXmlDocument` is constant-overhead.
**Constraints**: byte-stable golden output for non-Maven fixtures; the only allowed delta is the maven golden where the version corrects from a parent-version to the project-version. The milestone-083 `transitive_parity_maven` baseline becomes binding (it expected `@3.14.0`; this fix delivers it).
**Scale/Scope**: ~10 lines of production code change + ~30 lines of tests. Single ecosystem (maven). Surgical fix.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

- **I. Pure Rust, Zero C**: ✅ no FFI, no C deps, no toolchain changes.
- **II. eBPF-Only Observation**: N/A — this is a filesystem-mode parser; eBPF discovery path unchanged.
- **III. Fail Closed**: ✅ no behavior change to error handling. The fix replaces wrong-version emission with correct-version emission; no new failure modes.
- **IV. Strict Workspace Hygiene**: ✅ pre-PR gate (`./scripts/pre-pr.sh`) unchanged.
- **V. Specification Compliance / standards-native precedence**: ✅ no new `mikebom:*` properties. Same emission shape pre/post fix; only the version string changes.
- **X. Transparency**: ✅ surfacing the project's own version is *more* transparent, not less; no new opaque metadata.
- **XII. External Data Source Enrichment**: N/A — pom.xml is filesystem-discovered manifest data, same source class as before.

**No violations**. No Complexity Tracking entry needed.

## Project Structure

### Documentation (this feature)

```text
specs/092-fix-maven-version-extract/
├── plan.md                         # This file
├── research.md                     # Phase 0: bug investigation + fix-shape decision
├── data-model.md                   # Phase 1: PomXmlDocument struct delta
├── contracts/
│   └── maven-version-extraction.md # Phase 1: parser contract + main-module emission contract
├── quickstart.md                   # Phase 1: maintainer recipes
├── checklists/
│   └── requirements.md             # 16/16 pass — already complete
└── spec.md                         # Feature spec
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   └── scan_fs/
│       └── package_db/
│           └── maven.rs            # PRODUCTION CHANGE: lines ~530-554 (struct field)
│                                   #                    lines ~703-715 (parser wiring)
│                                   #                    lines ~3395-3439 (build_maven_main_module_entry)
└── tests/
    └── maven_pom_version_extraction.rs  # NEW: integration tests for FR-001 / FR-004
```

**Structure Decision**: Single-crate fix in `mikebom-cli`. No new files in production code path; one new integration-test file. The maven sidecar reader at `maven.rs` is the sole production touchpoint.

## Complexity Tracking

> Not applicable — no Constitution gate violations.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|--------------------------------------|
| (none)    | (none)     | (none)                               |

## Phase Plan

### Phase 0 — Research (this directory's `research.md`)

Status: **complete**. The Phase 0 investigation traced the bug to the exact line numbers and resolved the fix-shape decision (add `self_version` field; do not relax the `self_coord` constructor — that would risk breaking dependent code paths in `MavenInheritanceContext::build_from_poms` that key on the full `(g, a, v)` tuple). See `research.md` for the three resolved decision points.

### Phase 1 — Design (this directory's `data-model.md`, `contracts/`, `quickstart.md`)

Output:

- **data-model.md**: minimal — one new field on `PomXmlDocument`; no schema-level changes.
- **contracts/maven-version-extraction.md**: precise behavioral contract for the two affected functions.
- **quickstart.md**: maintainer recipes (reproduce, fix, verify, regenerate goldens).

Re-evaluate Constitution Check post-design: still no violations expected (the fix is a pure-additive `Option<String>` field).

### Phase 2 — Tasks

Out-of-scope for `/speckit.plan`; will be generated by `/speckit.tasks`.

## Agent Context Update

The agent context file (`CLAUDE.md`) will be updated by the agent-context script to reflect milestone 092's stack (rust stable, no new deps, single ecosystem).
