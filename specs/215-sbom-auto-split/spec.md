# Feature Specification: Auto-split monorepo SBOM into per-subproject SBOMs

**Feature Branch**: `215-sbom-auto-split`
**Created**: 2026-07-21
**Status**: Draft
**Input**: User description: "let's build a flag that will automatically split an sbom in a monorepo or a project with subrepos and whatnot into multiple sboms."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Monorepo owner emits per-workspace-member SBOMs in a single scan (Priority: P1)

A developer maintaining a monorepo runs `waybill sbom scan` with the new split flag pointed at the repository root. Today they get one large SBOM containing every component from every subproject. Post-feature they get one SBOM per detected workspace member — each SBOM contains only the components that member's dep-graph reaches, with a per-SBOM root component whose PURL identifies the specific member. Filenames encode the subproject identity so downstream tooling can route each SBOM to the right owner.

**Why this priority**: This is the core use case. Compliance pipelines routinely require one SBOM per shipping artifact (per binary, per service, per npm package). Handing them a single monorepo SBOM containing ten unrelated services is a hard block. The split flag is the difference between "the tool works for our compliance workflow" and "we hand-post-process every scan."

**Independent Test**: On a cargo workspace with 4 members (e.g., the m212 `two_binaries_diverge` fixture), running `waybill sbom scan --path <root> --split` produces exactly 4 SBOM files, each with a distinct root component PURL identifying its workspace member, and the set-union of components across the 4 SBOMs equals what the pre-feature single-SBOM scan would have emitted (no components dropped; shared transitive deps intentionally duplicated per per-member closure per US4).

**Acceptance Scenarios**:

1. **Given** a monorepo with N detected workspace members (each with a Cargo.toml / package.json / go.mod / pom.xml root), **When** the operator runs `waybill sbom scan --path <root> --split`, **Then** the tool emits N SBOM files — one per member — under a naming convention that includes the member's directory basename or package name.
2. **Given** the same scan, **When** the operator inspects any emitted SBOM's `metadata.component` (CDX) / `describes` (SPDX), **Then** the root component identifies that specific member (its PURL, not the workspace root's), and the component list is scoped to that member's dep-graph reachable set.
3. **Given** a scan with `--split` and no workspace members detected (e.g., single-package project), **When** the tool runs, **Then** it emits ONE SBOM identical to the pre-feature output plus a WARN log noting no split boundaries were found (fallback to non-split behavior).

---

### User Story 2 - Heterogeneous project with mixed ecosystems (Priority: P1)

A project layout has `frontend/` (npm), `backend/` (Python/pyproject.toml), and `mobile-ios/` (Swift Package.swift). Running `waybill sbom scan --split --path <root>` produces three SBOMs — one per ecosystem-scoped subdirectory — because the presence of an ecosystem-specific manifest at each directory constitutes a subproject boundary under each reader's existing main-module logic (per m176 workspace definition).

**Why this priority**: Multi-ecosystem monorepos are the second most common shape after single-ecosystem workspaces (React frontend + Python API + Go worker is a textbook pattern). Auto-splitting across ecosystems means the split flag "just works" without operators needing to hand-configure boundaries per project layout.

**Independent Test**: Construct a fixture with `frontend/package.json`, `backend/pyproject.toml`, and `mobile-ios/Package.swift`. Running `waybill sbom scan --path <fixture-root> --split` produces exactly 3 SBOM files, each rooted at the appropriate ecosystem's main-module PURL (`pkg:npm/…`, `pkg:pypi/…`, `pkg:swift/…`).

**Acceptance Scenarios**:

1. **Given** a directory tree with N distinct ecosystem manifests at N distinct subdirectories, **When** split scan runs, **Then** N SBOMs emit and each is rooted at the ecosystem-appropriate PURL type.
2. **Given** a subdirectory containing manifests for multiple ecosystems (e.g., a directory with BOTH `package.json` and `pyproject.toml`), **When** split scan runs, **Then** the tool emits TWO SBOMs for that directory (one per ecosystem root) OR a single SBOM containing both root components (behavior consistent within a release; see Assumptions).

---

### User Story 3 - Downstream consumer receives an index describing the split (Priority: P2)

A downstream tool ingesting the split output needs to know: which SBOMs are related, what the split boundaries were, and how to reassemble them into a whole-project view if needed. Waybill emits a manifest file alongside the individual SBOMs that lists each emitted file, its root component PURL, the source directory / workspace member it corresponds to, and shared-dependency counts.

**Why this priority**: Without a manifest, downstream tools can find the SBOMs (via a filename glob) but can't authoritatively know which subproject each represents or whether they're all part of the same scan. A manifest turns the split from "10 disconnected SBOMs on disk" into "a scan artifact with structure."

**Independent Test**: After a split scan, verify that `<output-dir>/split-manifest.json` exists and lists exactly the SBOMs that were written, that each entry names its source directory + root PURL + component count, and that the sum of component counts across entries corresponds to the pre-feature single-SBOM component count adjusted for shared-dep duplication.

**Acceptance Scenarios**:

1. **Given** a split scan that emits 4 SBOMs, **When** the operator inspects the emitted `split-manifest.json`, **Then** it lists exactly 4 entries, one per SBOM, each with `{path, root_purl, source_dir, component_count, shared_deps_count}`.
2. **Given** the same scan, **When** the operator sums each entry's `component_count`, **Then** the sum equals the number of components across all emitted SBOMs (including duplication of shared deps), and the manifest's document-level `total_unique_components` equals the pre-feature single-SBOM component count.

---

### User Story 4 - Shared transitive dependencies handled consistently (Priority: P2)

A monorepo's two subprojects both depend on `serde 1.0.219` (same version, same PURL). The split scan MUST NOT drop `serde` from either sub-SBOM (each needs its own dep closure for downstream analysis to work). Each sub-SBOM lists `serde 1.0.219` as a component, and the manifest reports the shared-dep count so downstream tools can dedup if they want.

**Why this priority**: Getting duplication semantics right is important but secondary to just getting the split working. The correct default is "duplicate — each sub-SBOM is self-contained" because that's what compliance tools expect. Dedup / shared-SBOM patterns can layer on later.

**Independent Test**: Two workspace members A and B both depending on `serde 1.0.219`. Post-split, both `A.cdx.json` and `B.cdx.json` list `serde` as a component. Manifest lists `shared_deps_count` reflecting the shared count (aggregate or per-pair depending on implementation choice within this spec).

**Acceptance Scenarios**:

1. **Given** two subprojects sharing K transitive dependencies, **When** split scan runs, **Then** both emitted SBOMs contain all K shared dependencies (each self-contained).
2. **Given** the same scan, **When** the operator inspects the manifest's shared-dep aggregate, **Then** it accurately reports the K shared dependencies (either as an aggregate count or as a list of shared PURLs — implementation choice within this spec).

---

### Edge Cases

- **No detectable subprojects**: Scan target is a single-package project (one `Cargo.toml` at root, no members) — split flag emits one SBOM (identical to non-split behavior) + WARN log line. Doesn't fail.
- **Nested workspaces**: Cargo workspace containing a nested Cargo workspace (or npm workspace inside a Cargo workspace). Split respects the OUTERMOST workspace boundary AND, within each outer member, honors any inner workspace boundaries (so a nested `apps/frontend/` with 3 npm packages produces 3 SBOMs under the outer member's split output).
- **Path conflicts**: Two subprojects with the same basename (e.g., `apps/frontend/` in two different top-level dirs). Filename generation MUST disambiguate — likely by including the relative path or the package-name field from the manifest.
- **Component with no clear subproject owner**: A binary at `/usr/lib/libfoo.so` scanned from an image-scan mode where no source manifest exists in a subproject subtree. Attributed to a "root" pseudo-subproject SBOM, OR emitted only in the manifest with a warning. Behavior consistent across formats.
- **Fixed-timestamp mode**: With `WAYBILL_FIXED_TIMESTAMP` set, all emitted SBOMs use the same timestamp for reproducibility. Individual SBOM serial numbers are deterministic per subproject (derived from the subproject identity, not fresh UUIDs).
- **Empty subproject**: A workspace member with no declared or observed dependencies (empty Cargo.toml except for `[package]`). Split emits an SBOM containing just the root component + a WARN annotation.
- **Non-`--split` invocation unchanged**: Existing `waybill sbom scan --path .` behavior is byte-identical to pre-feature. `--split` is strictly opt-in.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The `waybill sbom scan` command MUST accept a new opt-in flag (canonical name TBD in Assumptions; likely `--split` or `--split-by-workspace`). Without the flag, behavior is byte-identical to pre-feature scan output.
- **FR-002**: When the flag is set, the tool MUST detect subproject boundaries by reusing each ecosystem reader's existing workspace / main-module detection (per m176 workspace definition — cargo workspace members, npm workspaces, Go workspaces, Maven multi-module projects, gradle sub-projects, pyproject/setup.py directories, etc.).
- **FR-003**: For each detected subproject boundary, the tool MUST emit one SBOM containing only that subproject's dep-graph reachable set of components (root component + transitive dependencies).
- **FR-004**: Each emitted SBOM's `metadata.component` (CDX) / `describes` (SPDX 2.3) / root Element (SPDX 3) MUST identify the specific subproject (its PURL), not the whole-repo root.
- **FR-005**: Emitted SBOM filenames MUST encode the subproject identity in a way that survives filesystem-safe transformation (colons → underscores or similar) AND MUST be unique within the output directory. Naming convention should be predictable — e.g., `<subproject-name>.<format>.json` or `<sanitized-purl>.<format>.json`.
- **FR-006**: The tool MUST emit a split manifest file alongside the individual SBOMs listing each SBOM's path, root PURL, source directory, component count, and shared-dep summary. Manifest format is JSON with a stable schema.
- **FR-007**: Shared transitive dependencies MUST appear in every subproject SBOM that depends on them (each sub-SBOM is self-contained — no external references to sibling SBOMs required to interpret it). The manifest MUST report shared-dep counts as a diagnostic signal.
- **FR-008**: The flag MUST work across all three emitted SBOM formats (CycloneDX 1.6, SPDX 2.3, SPDX 3.0.1). When multiple formats are requested, the tool emits N × M files (N subprojects × M formats) plus one manifest.
- **FR-009**: When no subproject boundaries are detected, the tool MUST emit exactly one SBOM (identical to non-split behavior) plus a WARN log line indicating fallback. The command exits 0.
- **FR-010**: Nested workspace boundaries MUST be respected — a Cargo workspace member that itself contains an npm workspace produces N + M SBOMs (N outer cargo members + M inner npm packages under one of the members), not just N outer.
- **FR-011**: Filename collisions between subprojects with the same basename MUST be disambiguated by including a portion of the relative path or the package-name field from the manifest. Collision detection MUST be deterministic across scans.
- **FR-012**: When `WAYBILL_FIXED_TIMESTAMP` is set for reproducibility, all emitted SBOMs MUST use the same fixed timestamp, and serial numbers MUST be deterministic per subproject (derived from subproject identity, not fresh UUIDs).
- **FR-013**: A binary/component with no clear subproject owner (e.g., a system binary detected via image scan without a source subproject) MUST be attributed to a "root" pseudo-subproject SBOM (or explicitly noted in the manifest as unowned). Behavior consistent across the three SBOM formats.
- **FR-014**: The tool MUST emit one INFO-level log line at scan end summarizing the split — count of subprojects detected, total components across sub-SBOMs, count of shared deps across sub-SBOMs.
- **FR-015**: Each emitted sub-SBOM MUST retain the full-fidelity semantics of the non-split output for its subset — same annotations, same evidence blocks, same license expressions, same hash algorithms. Splitting is a scope-narrowing operation, not a fidelity-reduction operation.
- **FR-016**: The command exit code semantics are unchanged. Split-mode success = 0 exit + N sub-SBOMs + 1 manifest written. Split-mode failure of any sub-SBOM emission fails the whole invocation (no partial output on disk beyond what's already been written).

### Key Entities

- **Subproject boundary**: A detected root within the scan tree where an ecosystem reader identifies a main-module / workspace-member. Per m176, this comes from each reader's existing logic (cargo, npm, go, maven, gradle, pyproject, gem, etc.). One subproject = one emitted sub-SBOM.
- **Sub-SBOM**: A CycloneDX/SPDX document rooted at a subproject's identity, containing that subproject's dep-graph reachable set. Full-fidelity — same annotations, hashes, evidence as the non-split output for the subset.
- **Split manifest**: A JSON document listing each emitted sub-SBOM's path, root PURL, source directory, component count, and shared-dependency summary. Enables downstream tooling to reason about the split as a whole.
- **Shared dependency**: A component (identified by PURL equality) that appears in more than one sub-SBOM. Reported in the manifest as a diagnostic count; may be listed by PURL for machine consumption.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a fixture with N declared workspace members (all one ecosystem), running `waybill sbom scan --path <root> --split` emits exactly N sub-SBOMs plus one manifest, with zero flaky behavior across 10 consecutive scans.
- **SC-002**: On a heterogeneous fixture (3+ ecosystems in distinct subdirectories), the tool emits one sub-SBOM per detected ecosystem-root subdirectory. Each sub-SBOM's root PURL matches the ecosystem's PURL type (`pkg:npm/…`, `pkg:cargo/…`, `pkg:pypi/…`, etc.).
- **SC-003**: For any subproject A: components(A_sub_sbom) ⊆ components(pre-feature single SBOM). Sub-SBOMs never introduce components that weren't in the original — split is purely scope-narrowing.
- **SC-004**: The union of components across all sub-SBOMs equals the components in the pre-feature single SBOM (no components dropped by splitting). Verifiable via a golden test: `union(split.cdx.json[*].components[*])` == `single.cdx.json.components`.
- **SC-005**: The split manifest correctly reports (a) exactly the emitted files, (b) accurate component counts per sub-SBOM, (c) accurate shared-dep aggregate. Verifiable by parsing manifest + comparing to emitted SBOMs.
- **SC-006**: Each emitted sub-SBOM independently validates against its format's schema (CDX 1.6 JSON schema, SPDX 2.3 schema, SPDX 3.0.1 schema) — no sub-SBOM is malformed.
- **SC-007**: Non-`--split` invocations (`waybill sbom scan --path .`) produce byte-identical output to pre-feature scans. Split flag is strictly opt-in; unrelated behavior unchanged.
- **SC-008**: When `WAYBILL_FIXED_TIMESTAMP` is set, running the split scan twice produces byte-identical output on both runs (all N sub-SBOMs + manifest deterministic).
- **SC-009**: An operator following the documented usage pattern (`waybill sbom scan --path <monorepo> --split --output-dir <dir>`) can route each emitted sub-SBOM to the correct downstream consumer (e.g., a per-service SBOM registry) using only the manifest's `{path, root_purl, source_dir}` entries.

## Assumptions

- **Canonical flag name**: This spec assumes `--split` as the default flag name (short, discoverable, matches the user's word choice). If reviewers prefer `--split-by-workspace` or `--per-subproject` for clarity, that's a low-impact rename resolved during `/speckit.clarify`.
- **Boundary-detection strategy**: Reuse each ecosystem reader's existing main-module / workspace-member detection per m176. Zero new detection logic — the same directory that produces a main-module component in single-SBOM mode becomes a subproject boundary in split mode. This keeps the scope tight and avoids re-inventing workspace-boundary detection per ecosystem.
- **Output directory**: The flag pairs with `--output-dir <dir>` (existing today). Split mode writes N sub-SBOM files + 1 manifest into `<dir>`. Filenames auto-generated from subproject identity per FR-005. The existing `--output` flag (single-file target) is incompatible with `--split` and produces a friendly error suggesting `--output-dir` instead.
- **Shared-dep default**: DUPLICATE (each sub-SBOM is self-contained). Alternative "shared SBOM + per-sub overlays" is a common pattern in some tooling but adds significant complexity and requires cross-SBOM references that many downstream tools don't handle. Duplicate default matches operator expectations. If reviewers want a dedup mode, that's a future flag (e.g., `--split-mode=dedup-shared`) — this spec ships duplicate-only.
- **Manifest format**: JSON with a stable schema documented in the planning-phase contracts. Not a wire-format spec artifact (CDX/SPDX consumers don't parse it) — it's a Waybill-side operator-facing artifact under the `waybill:*` namespace.
- **Manifest filename**: `split-manifest.json` at the output-dir root. Distinct-enough name that it won't collide with a subproject SBOM.
- **Multi-format simultaneity**: When operator requests multiple output formats (e.g., `--format cyclonedx-json --format spdx-2.3-json`), split-mode emits N × M files. Manifest lists all of them grouped by subproject.
- **Reproducibility contract**: The existing `WAYBILL_FIXED_TIMESTAMP` reproducibility contract (per pre-feature single-SBOM scans) extends to split-mode. Serial numbers become deterministic hashes of the subproject identity (documented in contracts during `/speckit.plan`).
- **Full-fidelity per sub-SBOM**: Each sub-SBOM retains the same annotation set, evidence blocks, license expressions, and hash algorithms that the pre-feature single SBOM would emit for that subset. Splitting is a scope-narrowing operation, not a fidelity-reduction operation.
- **Existing tests unaffected**: The 34 golden test files that exercise non-split scan behavior remain byte-identical (single-file goldens). New golden files are added under `waybill-cli/tests/fixtures/golden/split/<fixture-name>/` covering split-mode output.
- **CI grep gate compatibility**: The m214 rename gate at ci.yml continues to pass — this feature adds only `waybill:*` annotations (if any new ones are needed) and no `MIKEBOM_*` references.
- **In-scope**: Automatic boundary detection via existing readers; deterministic filenames; JSON manifest; multi-format multiplication; opt-in via a single CLI flag.
- **Out of scope**: Manual boundary override (explicit `.waybill/split.yml` config file with hand-authored boundaries) — deferred to a follow-up spec once auto-detection ships. Cross-SBOM referencing / shared-SBOM overlay mode. Renaming the pre-existing single-SBOM output when `--split` produces zero boundaries (fallback keeps the existing filename convention).
