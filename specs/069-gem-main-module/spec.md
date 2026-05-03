# Feature Specification: gem source-tree main-module component for top-level *.gemspec roots

**Feature Branch**: `069-gem-main-module`
**Created**: 2026-05-03
**Status**: Draft
**Input**: User description: "yes let's move on to gems" — pick up the gem slice of #104 next; maven remains as the final ecosystem.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Ruby project SBOMs identify the project itself (Priority: P1)

A developer or CI pipeline runs `mikebom sbom scan --path <ruby-project>` against a Ruby gem project. The resulting SBOM contains a component identifying the project itself — `pkg:gem/<name>@<version>` — alongside its dependencies. Today, scanning a Ruby project emits dependency components from `Gemfile.lock` but no component representing the project-being-scanned, so the SBOM cannot answer "what is this an SBOM for?" without falling back to filesystem path heuristics.

**Why this priority**: Issue #104's gem coverage closes the second-to-last gap before the per-ecosystem main-module suite is complete (maven follows). The pattern is well-trodden by 053+064+066+068+#127. Most Ruby gem projects ship with a top-level `*.gemspec` declaring `s.name` + `s.version`, so the manifest is reliably available.

**Independent Test**: Clone any gem project with a top-level `*.gemspec` declaring `s.name` and `s.version` literals (e.g., `git clone https://github.com/rails/thor`), run `mikebom sbom scan --path <project> --format spdx-2.3-json --output sbom.json --no-deep-hash`, and verify the output contains exactly one package whose PURL is `pkg:gem/<name>@<version>` derived from the gemspec's literal-string assignments.

**Acceptance Scenarios**:

1. **Given** a Ruby gem project with a top-level `foo.gemspec` containing `s.name = "foo"` and `s.version = "1.2.3"` (literal strings), **When** `mikebom sbom scan --path <project>` runs, **Then** the resulting SBOM contains exactly one component with PURL `pkg:gem/foo@1.2.3` placed in each format's standards-native "BOM subject" slot (CycloneDX `metadata.component`, SPDX 2.3 `documentDescribes` target, SPDX 3 `DESCRIBES` target).
2. **Given** a `*.gemspec` whose `s.version` is assigned from a constant (`s.version = Foo::VERSION` — common pattern, version lives in `lib/foo/version.rb`), **When** mikebom scans, **Then** the main-module emits with the literal `0.0.0-unknown` placeholder per the cross-host determinism convention from milestones 053/064/066/068.
3. **Given** a project with multiple `*.gemspec` files at the top level (rare; usually one), **When** mikebom scans, **Then** each gemspec emits its own main-module per FR-001; same-PURL collisions dedup with `tracing::warn!` per the established convention.
4. **Given** a Ruby project with no top-level `*.gemspec` (only `Gemfile` + `Gemfile.lock` — application-style projects that don't publish as gems), **When** mikebom scans, **Then** no main-module is emitted (per FR-002 — no `*.gemspec` means no project-self identity to declare). Existing `Gemfile.lock` dep emission is unaffected.

---

### User Story 2 - Main-module component is identifiable and excludable (Priority: P2)

Same use case as milestones 064/066/068 US2: downstream tools (sbomqs, vuln scanners, license-compliance tooling) can distinguish the synthetic main-module from real third-party deps via the C40 supplementary `mikebom:component-role: main-module` annotation.

**Why this priority**: Inherits the existing C40 wiring; same posture as Go + cargo + npm + pip.

**Independent Test**: Run a gem scan that produces the new main-module component; assert the C40 annotation is present in CDX `metadata.component.properties`, SPDX 2.3 annotations, and SPDX 3 native field.

**Acceptance Scenarios**:

1. **Given** a gem scan producing a main-module, **When** rendered as CycloneDX 1.6, **Then** the main-module is in `metadata.component` with `type: "application"` AND carries `properties[].name = "mikebom:component-role"` with `value = "main-module"`.
2. **Given** the same scan, **When** rendered as SPDX 2.3, **Then** the main-module has `primaryPackagePurpose: "APPLICATION"` AND a C40 annotation envelope.
3. **Given** the same scan, **When** rendered as SPDX 3.0.1, **Then** the main-module has `software_primaryPurpose: "application"` AND the C40-mapped native field.
4. **Given** the new main-module, **When** sbomqs runs, **Then** licensing-coverage doesn't degrade by more than 1pp vs. pre-069 baseline.

---

### User Story 3 - Document root points at gem main-module (Priority: P3)

Inherits the multi-DESCRIBES super-root behavior from milestone 064 + #127. Single-gem-project scans get length-1 `documentDescribes`; polyglot scans extend the existing super-root.

**Why this priority**: Cosmetic / tool-friendliness on top of US1.

**Independent Test**: Single-gem scan → `documentDescribes` length 1; polyglot scan (gem + cargo + Go + npm + pip) → all main-modules in `documentDescribes`, deterministically sorted.

**Acceptance Scenarios**:

1. **Given** a single Ruby gem project, **When** rendered as SPDX 2.3, **Then** `documentDescribes` is exactly `[<gem-main-module-spdxid>]` (length 1) and the corresponding DESCRIBES relationship exists.
2. **Given** a polyglot project (gem + cargo + Go), **When** mikebom scans, **Then** the SPDX `documentDescribes` extends to include the gem main-module alongside cargo and Go main-modules, deterministically sorted.

---

### Edge Cases

- **`s.version = SomeConstant` (non-literal)**: Most common Ruby idiom — version constant lives in `lib/<name>/version.rb`. Mikebom emits the main-module with the literal `0.0.0-unknown` placeholder rather than executing the gemspec or following the constant reference. Cross-host determinism preserved per FR-001 + Assumption A1.
- **`s.version = "1.2.3".freeze` / `s.version = "1.2.3"`**: Both are literal-string assignments and should resolve to `"1.2.3"`. The existing `parse_gemspec_full` regex helper handles `.freeze` chained calls.
- **Heredoc-assigned name (`s.name = <<~END\nfoo\nEND`)**: Out of scope — extremely rare. Falls through to no emission if name can't be parsed at all.
- **Multiple top-level `*.gemspec` files**: Each emits its own main-module (FR-001). Same-PURL collisions dedup with `tracing::warn!` per the established convention.
- **No top-level `*.gemspec` (only Gemfile + Gemfile.lock)**: Skip main-module emission per FR-002. Application-style projects that aren't publishable as gems don't have a project-self identity in the gem ecosystem. Existing `Gemfile.lock` dep emission is unaffected.
- **Project with `*.gemspec` AND its own `Gemfile.lock` listing the same gem name**: rare but possible (a gem that depends on its own previous published version for testing). The existing dep-emission path emits the Gemfile.lock-derived entry; the new milestone-069 main-module emission emits the project-self entry. Augment-existing-or-emit-new pattern (per cargo 064/npm 066/pip 068) handles the dedup: same PURL → augment existing entry with C40 + `parent_purl: None`; different versions → both emit.
- **`*.gemspec` inside `vendor/`, `gems/`, or `specifications/` directories**: skipped per FR-003. Those are install-state paths (handled by the existing dep-emission path), not project-internal manifests.
- **Same-PURL collision with the existing dep-tier `gemspec_to_entry` emission** (when a system-installed gemspec at `/usr/lib/ruby/gems/.../specifications/<name>.gemspec` declares the same `(name, version)` as the project's top-level `*.gemspec`): augment-existing wins — Phase A's C40 tag layered on top, install-state evidence preserved if present.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: For every `*.gemspec` discovered AT THE TOP LEVEL of a project root during a source-tree scan that contains literal-string assignments to `s.name` (or `spec.name`) and `s.version` (or `spec.version`), mikebom MUST emit a single component representing that gem, with PURL `pkg:gem/<name>@<version>`. Name + version resolution: (1) literal-string assignment via the existing `parse_gemspec_full` regex helper → use the parsed value verbatim; (2) non-literal version assignment (e.g., `s.version = Foo::VERSION`) → use the literal `0.0.0-unknown` placeholder for the version; (3) name unparseable → skip emission entirely (no fallback identity available).

- **FR-001a (placement)**: The gem main-module component MUST be emitted via each format's standards-native "BOM subject" construct, identical to cargo (064) / npm (066) / pip (068) wiring. Inherits the C40-tag-driven hooks established in milestones 053+064+#127:
  - **CycloneDX 1.6**: `metadata.component` (single) or `components[]` siblings under super-root (multi-project polyglot), `type: "application"`.
  - **SPDX 2.3**: `packages[]` entry with `primaryPackagePurpose: "APPLICATION"` + `documentDescribes[]` targeting.
  - **SPDX 3.0.1**: `software_primaryPurpose: "application"` + `DESCRIBES` Relationship.

- **FR-002**: For Ruby projects with NO top-level `*.gemspec` file (only `Gemfile` + `Gemfile.lock` — application-style projects), mikebom MUST NOT emit a gem main-module. Application-style Ruby projects don't have a project-self identity in the gem ecosystem; the existing `Gemfile.lock` dep emission is unaffected.

- **FR-003**: `*.gemspec` files discovered inside install-state paths (`vendor/`, `gems/`, `specifications/`, `.bundle/`) are NOT emitted as main-modules. The existing milestone-walker `find_gemspecs` already restricts itself to `specifications/` directory contents for the dep-emission path; the new milestone-069 walker explicitly excludes those paths to avoid double-counting installed gemspecs as project-self components.

- **FR-004**: The gem main-module component MUST also carry `mikebom:component-role: main-module` (catalog row C40) as a supplementary signal across all three formats. Inherits the existing C40 wiring; no new annotation infrastructure required.

- **FR-005**: The gem main-module component MUST emit with an empty `licenses` field. License detection (gemspec's `s.license` / `s.licenses` literal-string declarations, LICENSE-file content matching) is out of scope and tracked as a follow-up to issue #103.

- **FR-006**: The gem main-module component MUST carry `mikebom:sbom-tier: source` per the existing tier-classification convention.

- **FR-007**: Direct-dep edges from the gem main-module to its dependencies — gemspec's `s.add_dependency` / `s.add_runtime_dependency` / `s.add_development_dependency` — MUST originate from the main-module's PURL. Reuses the existing `parse_gemspec_groups` helper at `gem.rs:491` which already extracts dep-section keys for the milestone-051 dev/build classification path. The augment-existing-entry pattern from milestones 064/066/068 merges with `Gemfile.lock`-derived entries when same-PURL matches occur.

- **FR-008**: The SPDX 2.3 `documentDescribes[]` array (and SPDX 3 `rootElement[]`, CycloneDX `metadata.component`) MUST point at the gem main-module component(s). Polyglot scans extend the existing milestone-064-#127 multi-DESCRIBES super-root mechanism.

- **FR-009**: The gem main-module emission MUST NOT alter the existing dep-emission component count from `Gemfile.lock` or installed-gemspec walks. Total dependency-graph edge count from the project's own gem MUST be byte-equivalent to pre-069 modulo the placeholder→main-module identifier swap.

- **FR-010**: The new gem main-module component MUST be excluded from `mikebom:not-linked` annotation eligibility (milestone 050) — inherits the existing C40-tag-driven guard from milestone 064.

- **FR-011**: Same-PURL collisions across discovered `*.gemspec` files (rare given install-state path exclusion per FR-003) dedup with `tracing::warn!` per the established cargo/npm/pip Q1 convention.

### Key Entities

- **gem main-module component**: A synthetic SBOM component representing a single Ruby gem at a top-level `*.gemspec` discovered during scan. Identified by `pkg:gem/<name>@<version>`. Carries `primaryPackagePurpose: APPLICATION`, the C40 supplementary role tag, and `mikebom:sbom-tier: source`. Source of all `s.add_dependency` / `s.add_runtime_dependency` / `s.add_development_dependency` direct edges (via existing `parse_gemspec_groups`).

- **Top-level project gemspec**: A `*.gemspec` file at a project root (NOT inside `vendor/`, `gems/`, `specifications/`, `.bundle/`). Emits one main-module per FR-001.

- **Application-style Ruby project**: A directory containing `Gemfile` + `Gemfile.lock` but NO top-level `*.gemspec`. Skipped for main-module emission per FR-002; existing `Gemfile.lock` dep emission unaffected.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of Ruby gem project scans containing a top-level `*.gemspec` with literal-string `s.name` declared emit at least one gem main-module component in the resulting SBOM. Verified by an integration test scanning a gem-style fixture.

- **SC-002**: Application-style Ruby projects (Gemfile + Gemfile.lock, no `*.gemspec`) do NOT emit a main-module per FR-002. Verified by an integration test scanning the existing `tests/fixtures/gem/` (or equivalent) which exercises the application-style case.

- **SC-003**: sbomqs licensing-coverage score for the gem fixture does not regress by more than 1 percentage point vs. the pre-069 baseline.

- **SC-004**: Byte-identity goldens hold across hosts. The gem fixture's CycloneDX, SPDX 2.3, and SPDX 3 goldens regenerate identically on Linux x86_64, Linux aarch64, and macOS aarch64 runners.

- **SC-005**: SPDX 2.3 `documentDescribes` array for any gem-only single-project scan with a top-level `*.gemspec` contains the gem main-module's SPDXID directly.

## Assumptions

- **A1 (manifest authoritative)**: The `*.gemspec` file's `s.name` + `s.version` literal-string assignments are the canonical source for the main-module's PURL. Non-literal assignments (constants, expressions) fall through to the `0.0.0-unknown` placeholder per cross-host determinism convention. Mikebom does NOT execute Ruby code or follow constant references — pure-Rust regex parsing only.

- **A2 (existing parse helper reused)**: The existing `parse_gemspec_full` helper at `mikebom-cli/src/scan_fs/package_db/gem.rs:947` already handles literal-string `s.name = "..."` and `s.version = "..."` extraction for the dep-emission path. Milestone 069 reuses this helper; no new parser is added.

- **A3 (license deferred)**: License detection for the gem main-module is out of scope. The C40 carve-out from milestone 053 already protects sbomqs scoring. Real `s.license` / `s.licenses` field reading + LICENSE-file detection tracked as follow-up to issue #103.

- **A4 (install-state paths excluded)**: `*.gemspec` files inside `vendor/`, `gems/`, `specifications/`, `.bundle/` directories are NOT discovered for main-module emission. Those are install-state paths handled by the existing `find_gemspecs` walker for the dep-emission path. This matches cargo's `vendor/` exclusion (066 via npm pattern) and pip's `__pycache__/` / `.venv/` exclusion (068 A5).

- **A5 (no binary path interaction)**: Ruby doesn't have a binary-discovery path that emits main-modules. The gem main-module is unconditionally source-tree-derived. Mirrors cargo (064 A6) / npm (066 A6) / pip (068 A6).

- **A6 (existing scope filtering preserved)**: The milestone-052/part-3 `--exclude-scope` flag continues to filter gemspec dev/test edges identically; FR-007 only relocates the edge origin from synthetic placeholder to the new main-module.

- **A7 (other ecosystems unchanged)**: This milestone is gem-specific. Go, cargo, npm, pip, maven behaviors are untouched.

- **A8 (super-root reuse)**: The multi-main-module super-root + plural-DESCRIBES infrastructure from milestone 064 + #127 ships unchanged. gem main-modules slot in as additional describable elements.

- **A9 (Ruby code execution out of scope)**: Mikebom does NOT execute the gemspec as Ruby code. Constant references (`s.version = Foo::VERSION`), method calls, and dynamic computations all fall through to the `0.0.0-unknown` placeholder. Operators wanting precise versions for dynamic-version gems should pin the version literally OR rely on `Gemfile.lock`-derived entries (which already resolve everything).

- **A10 (non-PURL-safe characters in gem name)**: Gem names per [rubygems guidelines](https://guides.rubygems.org/name-your-gem/) are URL-safe alphanumerics + `-` and `_`. The existing `build_gem_purl` helper at `gem.rs:234` percent-encodes any non-allowed chars per the PURL spec.
