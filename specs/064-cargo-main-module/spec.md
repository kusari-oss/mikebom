# Feature Specification: Cargo source-tree main-module component for crate / workspace-member roots

**Feature Branch**: `064-cargo-main-module`
**Created**: 2026-05-02
**Status**: Draft
**Input**: User description: "yes" — confirming start of issue #104 with cargo as the first ecosystem (per /speckit-specify recommendation: dogfood-friendly, simple manifest, sets the pattern for npm/pip workspace handling)

## Clarifications

### Session 2026-05-02

- Q: How should mikebom resolve same-PURL collisions across discovered `Cargo.toml` files (vendored copies, `examples/` mirrors, `target/package/` extractions)? → A: Dedup the component to a single row keyed by PURL; relationships pointing at it accumulate naturally per the standard SBOM relationship-graph model (DESCRIBES emitted once per (document, target) pair via existing relationship-dedup; incoming dependsOn edges from many parents stack on the same component as today). Emit `tracing::warn!` listing dropped duplicates so operators have visibility without mutating SBOM bytes. Divergent-deps case — same PURL but DIFFERENT outgoing direct dependencies, plausibly indicating two crates claim the same identity while their content hashes differ — is **out of scope** for milestone 064 and tracked in follow-up issue #125. The realistic collision cases (vendor/, examples/) have identical dep sets, so first-discovered's dep set wins and the divergent-deps detection is left to the follow-up to spec properly (it's a supply-chain signal, not just a dedup detail).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Cargo project SBOMs identify the project itself (Priority: P1)

A developer or CI pipeline runs `mikebom sbom scan --path <cargo-project>` against a Rust crate or cargo workspace. The resulting SBOM contains a component identifying the project itself — `pkg:cargo/<crate-name>@<version-from-Cargo.toml>` — alongside its dependencies. Today, scanning a Rust crate emits dependency components from `Cargo.lock` but no component representing the crate-being-scanned, so the SBOM cannot answer "what is this an SBOM for?" without falling back to filesystem path heuristics.

**Why this priority**: This is the dominant value of issue #104 for cargo. Every Rust crate's SBOM today is missing its own row, so vuln-intersection tools, dependency-graph visualizers, and `documentDescribes`-following consumers all see a placeholder root instead of the project. Adding the main-module component fixes the most user-visible omission and aligns cargo SBOM output with the Go behavior that shipped in milestone 053. mikebom is itself a cargo workspace, so the change is dogfood-verifiable on the very next scan.

**Independent Test**: Clone any crate with `[package].name` and `[package].version` declared in `Cargo.toml` (e.g., `git clone https://github.com/clap-rs/clap`), run `mikebom sbom scan --path <crate> --format spdx-2.3-json --output sbom.json --no-deep-hash`, and verify the output contains exactly one package whose PURL is `pkg:cargo/<crate-name>@<crate-version>` derived verbatim from `Cargo.toml`. Independently delivers the issue's primary value (project-self identification in cargo SBOMs).

**Acceptance Scenarios**:

1. **Given** a single-crate cargo project with `[package].name = "foo"` and `[package].version = "1.2.3"` in `Cargo.toml`, **When** `mikebom sbom scan --path <project>` runs, **Then** the resulting SBOM contains exactly one component with PURL `pkg:cargo/foo@1.2.3` placed in each format's standards-native "BOM subject" slot (CycloneDX `metadata.component`, SPDX 2.3 `documentDescribes` target, SPDX 3 `DESCRIBES` target).
2. **Given** a cargo workspace whose root `Cargo.toml` declares only `[workspace]` (no `[package]`) with members `["a", "b", "c"]` (each member having its own `[package].name` and `[package].version`), **When** mikebom scans, **Then** the SBOM contains exactly three main-module components (one per member), the workspace root itself emits NO main-module, and the document's `DESCRIBES` relationship targets all three members in deterministic name-sorted order via the existing polyglot super-root pattern from milestone 053 (FR-008).
3. **Given** a cargo project where `[package].version` is inherited from `[workspace.package].version` via `version.workspace = true`, **When** mikebom scans, **Then** the main-module's PURL version reflects the resolved value from the workspace `Cargo.toml`, not the literal string `"workspace = true"`.
4. **Given** the mikebom workspace itself, **When** scanned by `mikebom sbom scan --path .`, **Then** the resulting SBOM contains main-module components for `mikebom`, `mikebom-common`, and `xtask` (the three workspace members) — each carrying the correct `pkg:cargo/<member-name>@0.1.0-alpha.11` PURL — plus a separate `mikebom-ebpf` main-module discovered by the walker as an excluded-but-present crate.

---

### User Story 2 - Main-module component is identifiable and excludable (Priority: P2)

A consumer of the resulting SBOM (sbomqs, an internal license-compliance tool, a vuln-intersection tool, etc.) needs to distinguish the synthetic main-module component from real third-party dependencies — for licensing-coverage scoring (the project's own crate has no upstream `crates.io` license metadata fetched, so counting it against coverage would skew scores), for vulnerability lookup (the project itself is not in vuln databases), and for visualization (the main module is the root of the dep tree, not a leaf).

**Why this priority**: Without an explicit signal, downstream tools either count the main-module as a dep with missing metadata (unfair sbomqs licensing-coverage penalty) or surface it as an item to vuln-scan (false-positive lookups, no hits, wasted CI time). The C40 catalog row (`mikebom:component-role: main-module`) already exists from milestone 048; we just need to emit it on the new component, exactly as Go did in milestone 053.

**Independent Test**: Run a cargo scan that produces the new main-module component, parse the resulting SBOM, and verify the main-module component carries `mikebom:component-role: main-module` (CycloneDX property) AND the equivalent SPDX 2.3 annotation AND the SPDX 3 native field as defined by the C40 catalog row. Independently testable via parity-extractor C40 in `tests/holistic_parity.rs` (which already covers Go).

**Acceptance Scenarios**:

1. **Given** a cargo scan producing a main-module component, **When** the SBOM is rendered as CycloneDX 1.6, **Then** the main-module carries `properties[].name = "mikebom:component-role"` with `value = "main-module"` and is placed in `metadata.component` with `type: "application"`, NOT in the top-level `components[]` array.
2. **Given** the same scan, **When** rendered as SPDX 2.3, **Then** the main-module package carries the equivalent `mikebom:component-role: main-module` annotation per C40 AND `primaryPackagePurpose: APPLICATION`.
3. **Given** the same scan, **When** rendered as SPDX 3.0.1, **Then** the main-module element carries `software_primaryPurpose: "application"` and the C40-mapped native field representing the main-module role.
4. **Given** the new main-module component, **When** sbomqs runs against the SBOM, **Then** the licensing-coverage score is not degraded by more than 1 percentage point relative to the pre-064 baseline for the cargo fixture (the main-module component is excluded from "components requiring license" denominator via the C40 carve-out).

---

### User Story 3 - Document root points at the cargo main-module(s) (Priority: P3)

Consumers reading the SPDX `documentDescribes` / CycloneDX `metadata.component` slot expect a single subject component (or a deterministic set, for workspaces) that represents "what was scanned." Today mikebom emits a synthetic `SPDXRef-DocumentRoot-...` placeholder for cargo scans. Post-064, the document root targets the cargo main-module(s) directly so SPDX-tree-walking tools surface the project's own crate(s) as the SBOM subject.

**Why this priority**: Cosmetic and tool-friendliness improvement — most consumers don't follow the documentDescribes pointer for actionable data, but tools that DO (sbomqs root-resolution scoring, GitHub dep-tree visualizations, GUAC ingest) get a more accurate root. Lower priority than US1/US2 because the project-identification value is already delivered by US1; this is a polish layer that completes parity with milestone 053's Go behavior.

**Independent Test**: Run a cargo scan, inspect the output's `documentDescribes[]` (SPDX 2.3) / `metadata.component` (CDX) / `DESCRIBES` relationship (SPDX 3), and verify it points at the cargo main-module(s) — not at a synthetic `DocumentRoot-*` placeholder. For a workspace, verify all members are described, sorted deterministically.

**Acceptance Scenarios**:

1. **Given** a single-crate cargo project, **When** the SBOM is rendered as SPDX 2.3, **Then** `documentDescribes` contains exactly the SPDXID of the cargo main-module, and the document's `DESCRIBES` relationship targets that same SPDXID.
2. **Given** a cargo workspace with three members, **When** mikebom scans, **Then** the document root is a synthetic super-root component whose `DESCRIBES` relationship targets all three cargo main-modules in deterministic crate-name-sorted order, alongside any other ecosystems' main-modules from milestone 053 (Go) or future milestones.
3. **Given** a polyglot project (cargo + Go + npm), **When** mikebom scans, **Then** the synthetic super-root from milestone 053 / FR-008 extends to DESCRIBES every cargo member's main-module alongside the Go main-module and the npm placeholder root, in deterministic ecosystem-then-name order.

---

### Edge Cases

- **Workspace-only `Cargo.toml` (no `[package]` table, only `[workspace]`)**: Emit no main-module for the root. Each `[workspace.members]` entry is discovered separately by the walker and emits its own main-module per US1 AS#2.
- **`Cargo.toml` with BOTH `[package]` and `[workspace]` (root crate that is also a workspace root)**: Emit one main-module for the root crate AND one per member. Both are valid publishable crates with their own `[package]` tables. Documented as Assumption A4.
- **`version.workspace = true` inheritance**: The main-module's PURL version MUST reflect the resolved value (from `[workspace.package].version` in the workspace root), not the literal string. If resolution fails (workspace root not found, key not declared), fall back to the literal `0.0.0-unknown` placeholder for cross-host golden determinism — same convention as Go's FR-001 step 3.
- **Crate name with hyphens vs underscores**: cargo crates may declare a hyphenated `name = "foo-bar"` while Rust import paths use `foo_bar`. The PURL uses the manifest name verbatim (`pkg:cargo/foo-bar@1.0.0`). No normalization.
- **Renamed dependencies (`package = "..."` in `[dependencies]`)**: Out of scope — main-module emission only reads the project's OWN `[package]` table. Dependency renames are unaffected.
- **Path-only dependencies (`{ path = "../foo" }`) inside a workspace**: Within a workspace, a member depending on another member by path produces a dep edge from the depending member's main-module to the depended-on member's main-module. Both endpoints are real `pkg:cargo/...` components in the workspace's main-module set. No synthetic / orphan handling required.
- **Crates excluded from the workspace via `exclude = ["..."]` (e.g., `mikebom-ebpf`)**: The filesystem walker discovers each `Cargo.toml` independently of `[workspace]` declarations, so excluded-but-present crates DO emit their own main-module components. This is correct: an excluded crate is still a real publishable crate in the source tree.
- **No `Cargo.lock` present (library crate, application without lockfile committed)**: Main-module emission is independent of `Cargo.lock` — the manifest provides everything needed for the component's own row. Dependency edges from main-module to direct deps still emit (the `[dependencies]` / `[dev-dependencies]` / `[build-dependencies]` tables are authoritative for direct edges, mirroring Go's `go.mod` direct-require treatment in milestone 053 FR-002).
- **Pre-release versions (e.g., `0.1.0-alpha.11`)**: The manifest version is used verbatim; PURL semantics already accept SemVer pre-release strings. Mikebom's own workspace exercises this edge case — the `mikebom` crate publishes as `pkg:cargo/mikebom@0.1.0-alpha.11`.
- **Build metadata (e.g., `0.1.0+build.1`)**: Same as pre-release — verbatim from the manifest, no transformation.
- **Same-PURL collisions across discovered `Cargo.toml` files**: When the walker discovers two-or-more `Cargo.toml` files yielding identical `pkg:cargo/<name>@<version>` PURLs (e.g., a workspace member at `crates/foo/Cargo.toml` AND a vendored copy at `vendor/foo-1.2.3/Cargo.toml`), mikebom emits exactly one main-module component (deduped by PURL) and uses the first-discovered crate's outgoing direct-dep edges per FR-001. Other relationships (incoming dependsOn from other crates, outgoing edges from the deduped component, DESCRIBES from the document) accumulate per the standard relationship-graph model — a single component can be the target of arbitrarily many relationships. A `tracing::warn!` lists dropped duplicate paths for operator visibility. The divergent case (same PURL, different content hashes — potentially indicating typosquatting or accidental crate-name collision) is out of scope here; tracked separately.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: For every `Cargo.toml` discovered during a source-tree scan that contains a `[package]` table with declared `name` and `version`, mikebom MUST emit a single component representing that crate, with PURL `pkg:cargo/<package.name>@<resolved-version>`. Version resolution: if `version` is a literal string, use it verbatim; if `version.workspace = true`, resolve from the nearest enclosing `Cargo.toml` containing `[workspace.package].version`; if resolution fails, use the literal placeholder `0.0.0-unknown` (matches Go FR-001 step 3 convention for cross-host determinism). When two or more discovered `Cargo.toml` files resolve to the same PURL (vendored copies, `examples/` mirrors, `target/package/` extractions), mikebom MUST dedup to a single component row, retain the first-discovered crate's outgoing direct-dep edges (FR-007), and emit `tracing::warn!` listing the dropped duplicate paths. Divergent-deps detection (same PURL with content-hash mismatch — a potential supply-chain signal) is out of scope for milestone 064 and tracked in follow-up issue #125.

- **FR-001a (placement)**: The cargo main-module component MUST be emitted via each format's standards-native "BOM subject" construct, not as a sibling of regular dependency components. Specifically:
  - **CycloneDX 1.6**: emit each cargo main-module as `metadata.component` (when there is exactly one) or as the children of a `metadata.component` super-root (when there are multiple — i.e., workspace member crates), with `type: "application"`. The cargo main-module(s) MUST NOT also appear in the top-level `components[]` array.
  - **SPDX 2.3**: emit each cargo main-module as a regular `packages[]` entry, with `primaryPackagePurpose: "APPLICATION"`, and ensure `documentDescribes` (and the corresponding `SPDXRef-DOCUMENT DESCRIBES <main-module>` relationship) targets it.
  - **SPDX 3.0.1**: set `software_primaryPurpose: "application"` on the main-module element, and add a `DESCRIBES` (or v3-equivalent) relationship from the SBOM document to the main-module element.
  This satisfies Constitution Principle V (native fields take precedence). The `mikebom:component-role: main-module` property/annotation per FR-004 remains as a supplementary signal but is NOT the primary placement mechanism.

- **FR-002**: For workspace `Cargo.toml` files (containing `[workspace]` with `members`), mikebom MUST NOT emit a main-module for the workspace root unless the same `Cargo.toml` ALSO declares `[package]`. Each member crate is discovered as a separate `Cargo.toml` and produces its own main-module per FR-001.

- **FR-003**: Excluded crates declared via `[workspace].exclude = [...]` are NOT special-cased: the filesystem walker's normal `Cargo.toml` discovery is authoritative. If the walker finds a `Cargo.toml` with `[package]` inside an excluded directory, it emits a main-module for that crate exactly as if it were a standalone single-crate project.

- **FR-004**: The cargo main-module component MUST also carry `mikebom:component-role: main-module` (catalog row C40) as a **supplementary** signal, emitted via the format-appropriate construct — CycloneDX `properties`, SPDX 2.3 annotation envelope, SPDX 3 native field — exactly as already wired for the C40 row by milestone 053 (Go). This is layered on top of FR-001a's native-field emission so consumers reading either signal recognize the main-module. Per Principle V the native construct is authoritative; the C40 tag exists for backwards-compat with consumers that already read C40, and to enable sbomqs licensing-coverage carve-out (FR-005).

- **FR-005**: The cargo main-module component MUST emit with an empty `licenses` field. Coverage parity with sbomqs is achieved via the C40 role tag (FR-004), which excludes the component from the licensing-coverage denominator. LICENSE-file content detection (`SPDX-License-Identifier` header scan, askalono content matching, plus reading cargo's own `[package].license` and `[package].license-file` keys) is **out of scope** for milestone 064 and is tracked as a follow-up to issue #103 (which currently covers Go; will be extended to cargo). If post-merge sbomqs verification (per SC-003) shows >1pp regression vs. the pre-064 baseline, the implementer MUST halt the PR, file a follow-up issue, and request maintainer guidance — milestone 064 does NOT in-line patch licensing detection in response to a regression.

- **FR-006**: The cargo main-module component MUST carry `mikebom:sbom-tier: source` (the `Cargo.toml` is the authoritative source of the crate's own identity; this matches the existing tier-classification convention).

- **FR-007**: Existing direct-edge emission from `[dependencies]`, `[dev-dependencies]`, and `[build-dependencies]` tables MUST originate from the cargo main-module rather than from the synthetic `DocumentRoot-*` placeholder used pre-064. When BOTH the new main-module direct-edge emission AND any existing transitive-edge emission (Cargo.lock-resolved) produce an edge to the same target, the edge emits exactly once (deduplicated by the existing edge-dedup pipeline). Edges respect existing scope filtering (e.g., `--exclude-scope dev` from milestone 052/part-3).

- **FR-008**: The SPDX 2.3 `documentDescribes` array (and the SPDX 3 / CycloneDX equivalents) MUST point at the cargo main-module component(s) for cargo-only scans. For polyglot scans, mikebom MUST extend the existing milestone 053 / FR-008 super-root to include cargo main-modules alongside any Go main-module and other-ecosystem placeholder roots, in deterministic order. No per-ecosystem precedence tie-break is required: every described element is a sibling.

- **FR-009**: The cargo main-module emission MUST NOT alter the existing `[dependencies]` direct-edge fanout's component count (other than removing the synthetic root placeholder when one would have been emitted). The total dependency-graph edge count from the project's own crate MUST be byte-equivalent to the pre-064 edge count from the placeholder root, modulo the placeholder→main-module identifier swap.

- **FR-010**: The new cargo main-module component MUST be excluded from `mikebom:not-linked` annotation eligibility (milestone 050) — the project's own crate is by definition the linker root, never a non-linked dep. This mirrors Go's FR-010.

- **FR-011**: Workspace-member main-modules that depend on other workspace members via `{ path = "..." }` MUST emit `dependsOn` edges to the depended-on member's main-module component (using the same `pkg:cargo/<member>@<version>` identifier). No synthetic, separate, or orphan handling is required: both endpoints are real components emitted by FR-001 / FR-002.

### Key Entities

- **Cargo main-module component**: A synthetic SBOM component representing a single cargo crate at a `Cargo.toml` discovered during scan. Identified by `pkg:cargo/<package.name>@<resolved-version>`. Carries `primaryPackagePurpose: APPLICATION` (or CDX `type: "application"`), the C40 supplementary role tag, and `mikebom:sbom-tier: source`. Source of all `[dependencies]` / `[dev-dependencies]` / `[build-dependencies]` direct edges, replacing the pre-064 synthetic `DocumentRoot-*` placeholder.

- **Cargo workspace root**: A `Cargo.toml` containing `[workspace]` with `members`. Does not itself emit a main-module unless the same file also declares `[package]`. Provides resolution context for `version.workspace = true` member-crate fields.

- **Workspace member crate**: A `Cargo.toml` with `[package]` whose path appears in the parent's `[workspace.members]`. Emits its own main-module per FR-001; depends on other members via the FR-011 path-dep edge convention.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of cargo project scans containing at least one `Cargo.toml` with `[package].name` and `[package].version` declared emit at least one cargo main-module component in the resulting SBOM (CDX, SPDX 2.3, and SPDX 3 outputs all consistent). Verified by an integration test that scans a single-crate fixture, a workspace fixture, and the mikebom workspace itself, asserting `pkg:cargo/...@...` PURL presence in each output.

- **SC-002**: For the mikebom workspace itself (dogfood), `mikebom sbom scan --path .` produces exactly four cargo main-module components corresponding to `mikebom` (the cli), `mikebom-common`, `xtask`, and `mikebom-ebpf` (the excluded-but-present crate). Each PURL version matches the workspace `Cargo.toml`'s resolved `[workspace.package].version` (or `mikebom-ebpf`'s independently-declared version). The SBOM is byte-stable across two consecutive scans on the same checkout.

- **SC-003**: sbomqs licensing-coverage score for the cargo fixture does not regress by more than 1 percentage point vs. the pre-064 baseline. The C40 role tag (FR-004) excludes the new main-module from the denominator; sbomqs verification is performed manually before PR merge and recorded in the PR description.

- **SC-004**: Byte-identity goldens hold across hosts. The cargo fixture's CycloneDX, SPDX 2.3, and SPDX 3 goldens regenerate identically on Linux x86_64, Linux aarch64, and macOS aarch64 runners (per `feedback_cross_host_goldens.md` playbook: workspace-path rewrite, hash strip, HOME isolation, serial/timestamp masking, all simultaneously). Verified by the existing `holistic_byte_identity` test suite extended to include the cargo fixture's new main-module rows.

- **SC-005**: SPDX 2.3 `documentDescribes` array for any cargo-only scan contains the cargo main-module's SPDXID as exactly one of its members (one-per-member for workspaces, exactly-one for single-crate projects). The synthetic `DocumentRoot-*` placeholder no longer appears in cargo-only outputs.

## Assumptions

- **A1 (manifest authoritative)**: `Cargo.toml`'s `[package].name` and `[package].version` are the canonical source for the main-module's PURL. No git introspection (à la Go's `git describe` ladder) is needed because cargo's manifest is always present and version-bearing — this is a deliberate simplification vs. milestone 053's Go ladder.

- **A2 (workspace inheritance)**: When a member's `version.workspace = true`, the workspace root's `[workspace.package].version` is the resolved value. If the resolution chain is broken (member declares workspace inheritance but no parent workspace is found within the scan's filesystem boundary, or the workspace root lacks `[workspace.package].version`), the literal `0.0.0-unknown` placeholder is used to preserve cross-host golden determinism.

- **A3 (license deferred)**: License detection for the cargo main-module is out of scope. The C40 role-tag carve-out from milestone 053 already protects sbomqs licensing-coverage scoring. Real LICENSE-file detection (cargo's `[package].license`, `[package].license-file`, SPDX-License-Identifier headers, askalono content matching) is tracked as a follow-up to issue #103.

- **A4 (root-with-workspace)**: A `Cargo.toml` declaring BOTH `[package]` and `[workspace]` is treated per FR-001 / FR-002: emit one main-module for the root crate AND one per member. Both are valid publishable crates in the cargo model.

- **A5 (excluded crates)**: Crates inside `[workspace].exclude` directories that contain their own `Cargo.toml` with `[package]` ARE emitted as main-modules (FR-003). The filesystem walker is authoritative; the workspace exclusion list does not affect SBOM coverage.

- **A6 (binary path interaction)**: Cargo source-tree main-module emission does NOT interact with binary-path scanning the way Go's milestone 053 FR-009 does. Cargo binaries do not embed BuildInfo-style metadata, so there is no parallel "binary main-module" emission to dedup with. The cargo main-module is unconditionally source-tree-derived.

- **A7 (existing scope filtering preserved)**: All direct-edge filtering established by milestones 052/part-3 (`--exclude-scope`), 049 (Go source-tree dev edges), and the deprecated `--include-dev` shim continue to behave identically; FR-007 only relocates the edge origin from the placeholder root to the new main-module, no scope-filtering semantics change.

- **A8 (other ecosystems unchanged)**: This milestone is cargo-specific. npm, pip, maven, gem, and Go-binary scans are untouched — their existing component emission, edge emission, and root-placeholder behavior continue working as in alpha.11. Future milestones (issue #104 follow-up) extend the pattern to those ecosystems.

- **A9 (super-root extension is additive)**: The polyglot synthetic super-root from milestone 053 / FR-008 already DESCRIBES per-ecosystem placeholder roots. This milestone replaces the cargo placeholder with one-or-more cargo main-modules; the super-root structure does not change. SPDX 2.3 `documentDescribes` and CDX `metadata.component` extend their member set; no new top-level structure is introduced.
