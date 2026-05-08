# Feature Specification: Maven SPDX dep-edge emission (milestone-070 follow-up)

**Feature Branch**: `085-maven-spdx-dep-edges`
**Created**: 2026-05-08
**Status**: Draft
**Input**: User description: "Fix maven SPDX dep edges from milestone-070 gap" — Maven main-module emits no `DEPENDS_ON` relationships in SPDX 2.3 / SPDX 3 (and CDX relies on the primary-dep fallback rather than real `Relationship` entries). Pre-existing milestone-070 gap surfaced by milestone-084 closing the CDX orphan-ref bug.

## Overview

Milestone 070 (Maven main-module) added the project's main-module to `metadata.component` and `components[]`, with `entry.depends` populated from the `<dependencies>` block as `"groupId:artifactId"` strings (per `mikebom-cli/src/scan_fs/package_db/maven.rs:3451-3455`). The intent was that the existing edge-emission machinery in `mikebom-cli/src/scan_fs/mod.rs:548-564` would resolve those depends names against `name_to_purl` and emit `Relationship { from: main-module-PURL, to: dep-PURL, type: DependsOn }` entries — same as cargo / npm / pip / gem / go.

That resolution **silently fails for maven**:

- `name_to_purl` is keyed by `(ecosystem, normalize_dep_name(ecosystem, e.name))` at `scan_fs/mod.rs:373-379`. For maven dep components, `e.name` is the **artifact-id** (e.g., `"guava"`), so the key is `("maven", "guava")`.
- Maven main-module's `depends` list contains **`"groupId:artifactId"`** strings (e.g., `"com.google.guava:guava"`).
- Lookup at `scan_fs/mod.rs:549` constructs key `("maven", "com.google.guava:guava")` — which is NOT in `name_to_purl`.
- Every maven dep lookup misses → zero `Relationship` entries emitted from the maven main-module → SPDX 2.3 / SPDX 3 emit zero `DEPENDS_ON` for maven.

The CDX side compensated via the `dependencies.rs:78-91` primary-dep fallback (synthesizing `target_ref → roots-of-component-graph`), which produced correct dep-graph edges in CDX `dependencies[]`. Milestone 084's closure-invariant test exposed the gap: the CDX side now correctly emits 3 edges from the demo-app PURL, but SPDX 2.3 + SPDX 3 still emit zero. Parity test `parity_maven` for row B1 (`dependency edge (runtime)`) failed; milestone 084 added a `KNOWN_PARITY_GAPS` allowlist entry as a temporary measure pointing at this milestone for the real fix.

Empirical baseline (alpha.23 + milestone-084 fix):

| Format | Maven main-module → guava/junit/commons-lang3 edges |
|---|---|
| CDX 1.6 | 3 (via fallback synthesis at `dependencies.rs:78-91`) |
| SPDX 2.3 | 0 |
| SPDX 3 | 0 |

Post-085 target:

| Format | Maven main-module → guava/junit/commons-lang3 edges |
|---|---|
| CDX 1.6 | 3 (via real `Relationship` entries; fallback no longer fires for maven) |
| SPDX 2.3 | 3 (via the existing `DEPENDS_ON` emission at `spdx/relationships.rs:122-169`) |
| SPDX 3 | 3 (via the existing emission at `spdx/v3_relationships.rs`) |

## Why this matters — root cause

One symbol mismatch in `mikebom-cli/src/scan_fs/mod.rs:373-379`:

```rust
for e in &db_entries {
    let ecosystem = e.purl.ecosystem().to_string();
    name_to_purl.insert(
        (ecosystem, normalize_dep_name(e.purl.ecosystem(), &e.name)),
        e.purl.as_str().to_string(),
    );
}
```

`e.name` for maven entries is just the artifact-id. But maven dep names are conventionally `"groupId:artifactId"` (the canonical disambiguation format). Cargo / npm / pip / gem don't have this issue because their dep names are package names that match `e.name` directly.

The fix: for maven entries, ALSO insert a key under `"groupId:artifactId"` form. Both keys point at the same PURL. The maven main-module's `depends` lookups now resolve correctly.

## Why this matters — semantic impact

Per Constitution Principle V (Specification Compliance + standards-native precedence):

> "**Standards-native fields take precedence over `mikebom:`-prefixed properties.**"

The CDX primary-dep fallback at `dependencies.rs:78-91` is a `mikebom`-internal synthesis that compensates for missing real relationships. It produces correct CDX output but provides no signal to the SPDX path. SPDX 2.3 and SPDX 3 both have native `DEPENDS_ON` / `software_dependsOn` constructs that mikebom emits when it has real relationships (per cargo/gem/golang/npm/pip evidence). Maven's gap means the SPDX side can't represent the dep-graph edges that the CDX side has — a per-format vocabulary divergence that the format-parity gate (milestone 013) is designed to catch.

This was masked pre-milestone-084 because:
- The CDX-side parity extractor at `parity/extractors/cdx.rs:253` couldn't resolve the orphan `<short-name>@0.0.0` ref and skipped its dep edges in the comparison.
- Both sides reported empty edge sets → `parity_maven` row B1 SymmetricEqual passed (false positive).

Post-milestone-084: CDX correctly emits 3 edges, SPDX still has zero, parity test fails. The KNOWN_PARITY_GAPS allowlist in `holistic_parity.rs` papers over the divergence; this milestone removes the allowlist entry by closing the underlying gap.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — SPDX 2.3 maven document carries main-module dep edges (Priority: P1)

A regulatory-compliance pipeline consuming a mikebom-emitted SPDX 2.3 document for a Maven project needs `DEPENDS_ON` relationships from the project Package to its direct deps so the dep graph is reconstructable from SPDX alone. Today these edges are missing for maven (zero `DEPENDS_ON` in any maven SPDX 2.3 golden); the consumer either falls back to the CDX-only pipeline (defeating SPDX parity) or treats the maven SBOM as having zero deps.

**Why this priority**: Headline operator pain — SPDX 2.3 is the dominant deployed SPDX version per Constitution Principle V's prose, and federal procurement / sbomqs / syft|grype|trivy interop all expect it. A maven SPDX 2.3 with zero dep edges fails dep-tree-reconstruction expectations.

**Independent Test**: Run `mikebom sbom scan --path tests/fixtures/maven/pom-three-deps --format spdx-2.3-json --output -` and assert the resulting document has 3 `DEPENDS_ON` relationships from the demo-app SPDXID to guava / junit / commons-lang3 SPDXIDs. Equivalently: assert `[.relationships[] | select(.relationshipType == "DEPENDS_ON")] | length == 3`.

**Acceptance Scenarios**:

1. **Given** a maven scan with `<dependencies>` declaring guava + junit + commons-lang3, **When** mikebom emits SPDX 2.3, **Then** 3 `DEPENDS_ON` relationships exist with `spdxElementId` = main-module SPDXID and `relatedSpdxElement` = each dep's SPDXID.
2. **Given** the same scan, **When** mikebom emits SPDX 3, **Then** the equivalent edges exist via `software_dependsOn` per the existing milestone-079 ID-vocab work.

---

### User Story 2 — Format-parity gate is clean without an allowlist (Priority: P1)

The `KNOWN_PARITY_GAPS` allowlist entry in `mikebom-cli/tests/holistic_parity.rs` (added by milestone 084) papers over the SPDX-side divergence for maven. Post-085, the allowlist entry is removed; `parity_maven` passes for row B1 against unmodified holistic_parity infrastructure.

**Why this priority**: A clean parity gate is more durable than an allowlist. Allowlists tend to grow over time and silently mask drift. P1 because removing the allowlist is the load-bearing milestone-085 deliverable that "closes the loop" on the milestone-084 → 085 chain.

**Independent Test**: Remove the `("maven", "B1", _)` entry from `KNOWN_PARITY_GAPS` in `holistic_parity.rs`; run `cargo +stable test -p mikebom --test holistic_parity parity_maven`; assert exit 0.

**Acceptance Scenarios**:

1. **Given** the post-085 fix, **When** `holistic_parity::parity_maven` runs without the allowlist entry, **Then** the test passes.
2. **Given** the same post-085 fix, **When** the closure-invariant test from milestone 084 (`cdx_ref_closure_invariant`) runs, **Then** it continues to pass for maven (no regression).

---

### User Story 3 — CDX maven output stays byte-identical post-085 (Priority: P2)

The CDX side already has the 3 dep edges via the primary-dep fallback at `dependencies.rs:78-91`. Post-085 the same 3 edges come through real `Relationship` entries instead, but the CDX `dependencies[]` array contents are identical. The maven CDX golden MUST stay byte-identical pre/post 085.

**Why this priority**: P2 because byte-identity is a property of the fix shape (relationships flow through the same `target_ref` entry whether from real edges or fallback synthesis), not a separate requirement. Verifying it explicitly catches over-correction.

**Independent Test**: Run `cargo +stable test -p mikebom --test cdx_regression cdx_regression_maven` post-fix; assert exit 0 without `MIKEBOM_UPDATE_CDX_GOLDENS` env var.

**Acceptance Scenarios**:

1. **Given** the post-085 maven scan, **When** mikebom emits CDX 1.6, **Then** the resulting document is byte-identical to the milestone-084-regenerated maven golden.
2. **Given** the same post-085 fix, **When** the closure-invariant test runs across all 6 ecosystems, **Then** maven continues to satisfy the closure invariant + reverse-walk + compositions-anchor assertions.

---

### Edge Cases

- **Multiple maven entries sharing an artifact-id but different groupIds** (e.g., `org.foo:utils` and `org.bar:utils`): the post-085 fix inserts BOTH `("maven", "utils")` AND `("maven", "org.foo:utils")` keys. The artifact-id-only key collides between the two utils components — last-write-wins (existing behavior). The disambiguated `groupId:artifactId` key is unambiguous. Maven main-module depends are looked up via the disambiguated key, so they resolve correctly regardless of artifact-id collisions.
- **Maven dep with property-substituted GAV** (e.g., `<groupId>${groupId.placeholder}</groupId>`): the maven reader at `maven.rs:3417-3434` already resolves properties via `resolve_pom_property_value`. If a property is unresolvable, the dep emission fails with a warn — no relationship emitted (current behavior; not regressed).
- **Maven workspace / reactor with multi-module POM** (`<modules>` block): each child module emits its own main-module entry via `build_maven_main_module_entry`. Each module's `<dependencies>` produces its own depend-name list. No cross-module dep lookups beyond what milestone 070 already supports.
- **Empty `<dependencies>` block**: maven main-module's `depends` list is empty; the edge-emission loop iterates zero times; zero relationships emitted (current behavior; correct).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: For every `db_entry` whose ecosystem is `"maven"`, `name_to_purl` MUST contain BOTH a key `("maven", artifact-id)` AND a key `("maven", "groupId:artifactId")` — both pointing at the entry's PURL string.
- **FR-002**: Maven main-module's `depends` list (containing `"groupId:artifactId"` strings) MUST resolve via the `name_to_purl` lookup at `scan_fs/mod.rs:548-564`, producing one `Relationship { from: main-module-PURL, to: dep-PURL, type: DependsOn }` entry per direct dep.
- **FR-003**: SPDX 2.3 maven emission MUST contain N `DEPENDS_ON` relationships where N = number of resolved direct deps from the maven main-module's pom.xml `<dependencies>` block. Verified by inspecting the regenerated maven SPDX 2.3 golden.
- **FR-004**: SPDX 3 maven emission MUST contain N equivalent `software_dependsOn` edges via the existing milestone-079 ID-vocab path.
- **FR-005**: CDX 1.6 maven emission MUST be byte-identical to the milestone-084-regenerated maven golden — the same 3 dep edges flow through real relationships rather than the primary-dep fallback, but the resulting `dependencies[]` array is unchanged.
- **FR-006**: The `KNOWN_PARITY_GAPS` allowlist entry `("maven", "B1", _)` in `mikebom-cli/tests/holistic_parity.rs` MUST be removed; `holistic_parity::parity_maven` MUST pass without it.
- **FR-007**: Existing milestone-070 main-module emission tests MUST pass unmodified (no behavioral change to the main-module component itself; only the relationships from it are added).
- **FR-008**: Existing milestone-013 / 071 / 084 parity + closure-invariant tests MUST pass post-regen.
- **FR-009**: SPDX 2.3 + SPDX 3 maven goldens MUST regenerate cleanly with diffs containing only the newly-added `DEPENDS_ON` / `software_dependsOn` edges (no other field changes). Other ecosystems' SPDX goldens MUST stay byte-identical.

### Key Entities

- **`name_to_purl` map** (existing internal at `scan_fs/mod.rs:371`): `HashMap<(String, String), String>` keyed by `(ecosystem, normalized-name)`, value is PURL string. Post-085: maven entries get TWO keys per entry (artifact-id + groupId:artifactId), both pointing at the same PURL.
- **Maven main-module's `depends` field** (existing internal): `Vec<String>` of `"groupId:artifactId"` strings, populated at `maven.rs:3451-3455`. Unchanged by this milestone — it's already correct; the fix is on the consumer side.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: SPDX 2.3 maven golden post-085 contains exactly 3 `DEPENDS_ON` relationships from the demo-app Package to guava / junit / commons-lang3 Packages.
- **SC-002**: SPDX 3 maven golden post-085 contains exactly 3 equivalent `software_dependsOn` edges.
- **SC-003**: CDX 1.6 maven golden post-085 is byte-identical to the milestone-084-regenerated golden.
- **SC-004**: `holistic_parity::parity_maven` passes after removing the `KNOWN_PARITY_GAPS` allowlist entry.
- **SC-005**: Other ecosystems' goldens (CDX, SPDX 2.3, SPDX 3) all stay byte-identical post-085.
- **SC-006**: Pre-PR gate clean (`./scripts/pre-pr.sh`): zero clippy warnings, every test suite `0 failed`.
- **SC-007**: Closure-invariant test from milestone 084 (`cdx_ref_closure_invariant`) continues to pass for maven (no regression).

## Assumptions

- The existing maven dep-name extraction at `maven.rs:3451-3455` correctly produces `"groupId:artifactId"` strings. (Verified empirically — already shipping behavior.)
- The existing SPDX 2.3 / SPDX 3 dep-edge emission at `spdx/relationships.rs:122-169` and `spdx/v3_relationships.rs` correctly converts mikebom-internal `Relationship` entries into format-native edges. (Verified — works for cargo / gem / golang / npm / pip with non-trivial DEPENDS_ON counts in their goldens.)
- Maven artifact-ids are unique enough within a typical fixture that the additional `groupId:artifactId` key doesn't introduce collisions in real workloads. The test fixture `pom-three-deps` has no collision; future fixtures with intentional collisions would be caught by the new SPDX 2.3 maven golden's diff scope (3 expected edges; collisions would change the count).
- No new Cargo dependencies needed.
- The fix is safe to ship in a single PR. The change site is one block in `scan_fs/mod.rs`; no API surface change.

## Out of scope

- **SPDX-side per-ecosystem audit beyond maven**: cargo, gem, golang, npm, pip already emit SPDX `DEPENDS_ON` (per their non-zero golden counts). dpkg / apk / rpm don't have main-module promotion (fallback path); they emit zero `DEPENDS_ON` legitimately. No other ecosystem has a similar gap.
- **Refactoring `name_to_purl` to a smarter per-ecosystem strategy**: the fix is the smallest change (one extra insert for maven entries). Refactoring `name_to_purl` to use a per-ecosystem dep-name-format adapter is a code-quality concern outside scope.
- **Adding a CDX-side closure-invariant assertion that the primary-dep fallback no longer fires for maven**: byte-identity (FR-005 / SC-003) covers this implicitly — if the fallback fires the same way pre/post, byte-identity holds; if the fallback's behavior changes, the goldens drift and the audit catches it.
- **Maven `<dependencyManagement>` resolution beyond what milestone 070 already supports**: the dep-name extraction inherits whatever GAV resolution milestone-070 produced. If a `<dependency>` has a property-substituted GAV that fails to resolve, no relationship is emitted (current behavior).

## Dependencies

- Milestone 070 (Maven main-module emission) — must continue to work; the fix is downstream of it.
- Milestone 084 (CDX 1.6 main-module super-root collapse) — the trigger for this fix; PR #166 must merge before this milestone can land cleanly (the `KNOWN_PARITY_GAPS` allowlist entry that this milestone removes was added by 084).
- Milestone 013 format-parity gate — gates the SPDX maven golden regen.
- Milestone 071 annotation parity — unaffected by this fix (different parity rows).
- Milestone 079 SPDX 3 ID-vocab — provides the `software_dependsOn` IRI form used by SPDX 3 dep edges.
