# Feature Specification: Fix Maven pom.xml version-extraction bug (closes #175 partially)

**Feature Branch**: `092-fix-maven-version-extract`
**Created**: 2026-05-10
**Status**: Draft
**Input**: User description: "175"

## Background

Issue #175 was filed during milestone 083's transitive-parity audit (`mikebom-cli/tests/transitive_parity_maven.rs`) against the `apache/commons-lang @ rel/commons-lang-3.14.0` fixture. It surfaces two parallel concerns in the Maven reader:

1. **Cache-empty zero-edge gap**: When `~/.m2/repository/` is empty, mikebom emits 0 transitive dep edges from `pom.xml` alone. trivy 0.69.3 also emits 0 in the same configuration; syft 1.27.0 emits 8 via DEPENDENCY_OF reverse-direction. Maven's parent POM inheritance + property substitution model means a single isolated `pom.xml` cannot self-resolve transitive deps without cached or fetched parent POMs. This is **a structural Maven-ecosystem limitation that affects every SBOM tool** — mikebom's behavior matches trivy's. **Out of scope for this milestone.**
2. **Version-extraction bug** (this milestone's scope): When `~/.m2/` IS populated, mikebom emits a single edge with a malformed version string — `pkg:maven/org.apache.commons/commons-lang3@64` instead of `@3.14.0`. **Root cause** (verified by inspecting the fixture's `pom.xml`): the project's pom.xml has two `<version>` elements at the top:

```xml
<project>
  <parent>
    <groupId>org.apache.commons</groupId>
    <artifactId>commons-parent</artifactId>
    <version>64</version>           <!-- parent POM's version -->
  </parent>
  <modelVersion>4.0.0</modelVersion>
  <artifactId>commons-lang3</artifactId>
  <version>3.14.0</version>          <!-- project's own version -->
  ...
```

mikebom's pom.xml parser at `mikebom-cli/src/scan_fs/package_db/maven.rs` extracts the FIRST `<version>` it encounters (the parent's `64`) rather than the project's own `<version>` element (`3.14.0`). The fix is to distinguish `/project/parent/version` from `/project/version` and use the latter as the project's component version.

This is a small, surgical fix to the existing pom.xml parser. The cache-empty fallback work (track 1) is left as a deliberate follow-up because (a) it's a much larger architectural addition (Maven Central HTTP fetch infrastructure), (b) trivy doesn't solve it either so mikebom isn't behind, and (c) operators using mikebom for Maven projects who care about transitive coverage can populate `~/.m2/` once before scanning.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Operator gets correct version on the project's main-module component (Priority: P1)

An operator scanning a Maven project with a `<parent>` element in `pom.xml` (the typical case for any project using a parent POM — Apache Commons projects, Spring Boot projects, anything using a corporate parent POM) sees the project's main-module component emitted with the **project's own version** (`pkg:maven/<group>/<artifact>@<project-version>`), NOT the parent POM's version.

**Why this priority**: This is the entire reason for the milestone. Pre-092, every Maven project that declares a `<parent>` (which is the vast majority of real-world Maven projects) emits a wrong-version PURL. Downstream consumers using mikebom output for vuln-scanning, SLSA-attestation, or compliance get false-negatives on the project's own component because the version doesn't match any registry record.

**Independent Test**: `target/release/mikebom sbom scan --path <commons-lang-fixture>` against the milestone-083 audit fixture. Confirm the emitted SBOM contains `pkg:maven/org.apache.commons/commons-lang3@3.14.0` (NOT `@64`).

**Acceptance Scenarios**:

1. **Given** the milestone-083 commons-lang fixture (a `pom.xml` with both `<parent><version>64</version></parent>` and a project-level `<version>3.14.0</version>`), **When** the operator runs `mikebom sbom scan --path <fixture>`, **Then** the emitted SBOM contains `pkg:maven/org.apache.commons/commons-lang3@3.14.0` as the project's main-module component PURL. The string `@64` does NOT appear in any commons-lang3-named PURL.
2. **Given** any Maven project's `pom.xml` that declares a `<parent>` element with a `<version>`, **When** mikebom parses the pom, **Then** the project's main-module version is extracted from the project-level `<version>` element (a direct child of `<project>`), not from `<project><parent><version>`.
3. **Given** a `pom.xml` that does NOT declare a `<parent>` element (uncommon but valid — top-level / corporate-root POMs), **When** mikebom parses it, **Then** the project's `<version>` element is the only `<version>` at the project level and gets extracted as today (no regression for parent-less POMs).

---

### User Story 2 - No regression for `<version>${property}` substitution (Priority: P2)

A Maven `pom.xml` may declare its own version as a property reference (`<version>${revision}</version>` with `<properties><revision>3.14.0</revision></properties>` elsewhere). mikebom's existing property-substitution path continues to resolve these correctly post-092.

**Why this priority**: Property substitution is the second-most-common pom.xml version pattern after literal versions. The fix in US1 must not weaken or break the existing property-resolution code path. P2 because property substitution isn't directly the bug being fixed; it's a regression boundary to honor.

**Independent Test**: All milestone-085 + milestone-070 maven tests pass post-092 with zero behavioral change. Specifically `scan_maven.rs::scan_maven_property_substitution_resolves` (or similar — exact test name verified at impl time).

**Acceptance Scenarios**:

1. **Given** a `pom.xml` with `<version>${revision}</version>` + `<properties><revision>1.2.3</revision></properties>`, **When** mikebom parses it, **Then** the emitted main-module PURL is `pkg:maven/<group>/<artifact>@1.2.3` (property resolution works as today).
2. **Given** the milestone-085 + milestone-070 test suites, **When** the maintainer runs `cargo +stable test -p mikebom`, **Then** every maven-related test reports `0 failed` with the same assertions.

---

### Edge Cases

- **`<parent>` with no `<version>` (`relativePath`-only parent reference)**: rare but valid — child POM inherits parent version from a sibling `pom.xml`. mikebom should ignore the missing parent version cleanly and use the project's own `<version>` (or skip the project if the project also lacks a version, which would be a malformed pom.xml).
- **Project lacks its own `<version>` and inherits from parent** (`<parent><version>1.0</version></parent>` + no project-level `<version>`): the inherited version semantically applies to the project. Pre-092, mikebom's "first version wins" heuristic accidentally returned the right answer for this case (parent's version IS the project's effective version when not overridden). Post-092, mikebom MUST handle this case explicitly: if no project-level `<version>` exists but a `<parent><version>` does, use the parent's version as the project's effective version. Documented behavior change: the heuristic is now "project version takes precedence; falls back to parent version when project version is absent."
- **Both `<parent><version>` and project `<version>` are property references**: should resolve via the existing property-substitution path. Property scope is per-document so resolutions don't bleed between parent + project.
- **Multi-module reactor builds** (parent POM with `<modules>`): per-module pom.xml files each get parsed independently; the version-extraction fix applies per-module without cross-module side effects. milestone 070's multi-module main-module emission stays unchanged.
- **Malformed pom.xml with multiple project-level `<version>` elements** (invalid XML by Maven schema, but mikebom tolerates malformed input per the existing `quick-xml` parser's permissive mode): take the first one; document the choice.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When parsing a `pom.xml` for project main-module emission, mikebom MUST extract the version from the project-level `<version>` element (a direct child of `<project>`), NOT from `<project><parent><version>`. The parent's version is metadata about the parent POM and MUST NOT be used as the project's component version when a project-level `<version>` is present.
- **FR-002**: When the project lacks a project-level `<version>` AND the `<parent>` declares a `<version>`, mikebom MUST use the parent's version as the project's effective version (Maven's documented inheritance semantics — the child inherits the parent's version when not overridden). This case is preserved-behavior post-092 (pre-092 already worked by accident; the new code path explicitly handles it).
- **FR-003**: When the project lacks BOTH a project-level `<version>` AND a `<parent><version>`, mikebom MUST skip emitting that project as a main-module component. No malformed PURL emission.
- **FR-004**: The fix MUST preserve all existing milestone-085 + milestone-070 test assertions. Zero test deletions; zero assertion weakenings.
- **FR-005**: The fix MUST extend the existing per-format scope (CDX 1.6 / SPDX 2.3 / SPDX 3) — emitted main-module PURLs change from `<group>/<artifact>@<parent-version>` to `<group>/<artifact>@<project-version>` for any fixture whose pom.xml has both. The relevant fixture (milestone-070's `maven/pom-three-deps`) MAY regenerate its 3 maven goldens IF it declares a `<parent>` element with its own `<version>`; if it has no `<parent>`, the goldens stay byte-identical.
- **FR-006**: The milestone-083 `transitive_parity_maven.rs` regression test MUST be updated post-092 to capture the corrected version. The pre-092 baseline pinned 1 emitted edge with the wrong version; post-092 the same 1 edge has the right version. Standard milestone-083 baseline-bump pattern.
- **FR-007**: No new Cargo dependencies. The existing `quick-xml` parser at `mikebom-cli/src/scan_fs/package_db/maven.rs` already supports XPath-style child-element traversal; the fix is a parser-logic adjustment, not a new dep.
- **FR-008**: This milestone does NOT address the cache-empty zero-edge gap (Track 1 of issue #175). That track requires a Maven Central HTTP fetch infrastructure analogous to milestone-055's Go proxy fetch — substantially larger scope, deferred to a future milestone. Issue #175 is closed PARTIALLY by this milestone; a follow-up issue MAY be filed for Track 1 if maintainers want it tracked separately.

### Key Entities

- **`pom.xml` document tree**: the XML data model the maven reader parses. The fix navigates to `/project/version` (project-level direct child) instead of any-`<version>`-anywhere.
- **Project main-module entry**: the `PackageDbEntry` emitted for the project itself by milestone-070's main-module logic. Its `version` field is the target of the FR-001 extraction.
- **Audit fixture**: `transitive_parity/maven/` from `mikebom-test-fixtures` repo (apache/commons-lang) — accessed via `MIKEBOM_FIXTURES_DIR` per milestone 090.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Operators scanning a Maven project with a `<parent>` element see the project's correct version in the emitted SBOM's main-module PURL. The commons-lang audit fixture emits `pkg:maven/org.apache.commons/commons-lang3@3.14.0` post-092 (vs `@64` pre-092).
- **SC-002**: 100% of pre-092 milestone-070 + milestone-085 maven tests pass post-092 with no test deletions, no assertion weakenings.
- **SC-003**: `cargo +stable test --workspace` post-092 reports `0 failed`. `./scripts/pre-pr.sh` clean.
- **SC-004**: Per-format scope: at most 3 maven goldens regenerate (CDX 1.6 / SPDX 2.3 / SPDX 3) IF the milestone-013 maven fixture declares a `<parent>`. All 24 non-maven goldens stay byte-identical.
- **SC-005**: Production-deps trivy CI gate (milestone 089) and milestone-090 fixture-cache CI step continue to pass.

## Assumptions

- The audit fixture (commons-lang) lives in the post-090 `mikebom-test-fixtures` repo at `transitive_parity/maven/`. Verified by inspection.
- mikebom's existing pom.xml parser uses `quick-xml` (per memory of milestone 005) and supports per-element traversal. The version-extraction fix lives in `mikebom-cli/src/scan_fs/package_db/maven.rs`.
- The milestone-083 `transitive_parity_maven.rs` regression test pins 1 emitted edge (pre-092 baseline). Post-092 the count stays 1; only the version string changes within that edge.
- Maven's pom.xml inheritance semantics (project version overrides parent version when both present; parent version applies when project version is absent) are the documented Maven behavior. mikebom's fix matches Maven's reference behavior.

## Dependencies

- Milestone 070 (Maven main-module emission) — the existing infrastructure this milestone fixes.
- Milestone 085 (Maven SPDX dep edges) — co-existing tests that must continue to pass.
- Milestone 083 (transitive-parity audit) — the regression-test scaffolding (`transitive_parity_maven.rs`) that pins the post-092 baseline.
- Milestone 090 (fixture-repo split) — the audit fixture lives in the new repo.

## Out of Scope

- **Cache-empty Maven transitive-edge fallback (Track 1 of #175)**: requires Maven Central HTTP fetch infrastructure analogous to milestone-055's Go proxy fetch. Substantially larger architectural scope; deferred to a future milestone.
- **Adding a `--maven-fetch-parents` flag**: out of scope for the same reason.
- **Refactoring the maven reader's parser engine**: minimal-scope edit to the existing version-extraction logic only.
- **Bumping the commons-lang fixture version**: same fixture as milestone 083; reusing it directly.
- **Per-component provenance annotation for the project version source**: not needed — the project version is now correct unambiguously, no fidelity downgrade signal required (unlike milestone 091's go-sum-fallback case).
- **Improving Maven main-module emission beyond the version field**: groupId / artifactId / metadata extraction stays unchanged.
