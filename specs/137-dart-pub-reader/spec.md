# Feature Specification: Dart/Flutter pub ecosystem reader

**Feature Branch**: `137-dart-pub-reader`
**Created**: 2026-06-22
**Status**: Draft
**Input**: User description: "let's now move to dart"

## Background

Dart/Flutter is the dominant cross-platform mobile framework — Flutter powers production apps at Google (Google Pay, Google Ads, Stadia), Alibaba, BMW, Tencent, and an increasingly visible portion of the indie/startup mobile market. Server-side Dart is also growing (Shelf, Serverpod) for backend services that share code with the Flutter client.

mikebom currently emits **zero** Dart/Flutter components when scanning a Flutter app, a Dart server project, or any source tree containing `pubspec.lock`. Every dep pulled from pub.dev — and the often-larger graph of transitive deps — is invisible to the scan.

The Dart ecosystem has four discrimination surfaces a reader must handle:

- **Hosted deps** (the common case): from `pub.dev` or a self-hosted pub server. PURL: `pkg:pub/<package>@<version>`.
- **Path deps**: local-filesystem deps (`path: ../my-lib`) typically used for monorepos. Identity is the path; the dep doesn't carry a pub.dev version.
- **Git deps**: directly-pinned git URLs (`git: https://github.com/foo/bar.git`). Identity is the resolved Git SHA.
- **SDK pseudo-deps**: `flutter`, `flutter_test`, `flutter_localizations` etc. resolve to the Flutter SDK shipped with the project's Flutter version, NOT to pub.dev. Surfacing these as `pkg:pub/flutter` is wrong — they have no pub.dev provenance.

This feature closes the gap so an operator scanning any Dart/Flutter project gets a complete SBOM with every pub-managed dep represented, the right provenance distinction surfaced, and SDK pseudo-deps handled correctly.

## Clarifications

### Session 2026-06-22

- Q: Should the Dart reader emit a `pkg:pub/<project-name>@<version>` component for each scanned `pubspec.yaml` with a `name:` field (matching the main-module emission pattern established by cargo / npm / pip / gem / maven)? → A: Yes — emit main-module per `pubspec.yaml`. The component is tagged `mikebom:component-role = "main-module"` and `mikebom:sbom-tier = "source"` per the milestone-064-through-070 pattern. Dep edges flow from the main-module to its declared direct deps; without this anchor the dep graph would have no root.
- Q: How should the reader treat multi-`pubspec.yaml` projects (Melos monorepos with N member packages, or pub workspaces with single root lockfile)? → A: One main-module per `pubspec.yaml` (Option A). Each member emits a `pkg:pub/<name>@<version>` regardless of monorepo membership. Workspace structure is invisible in the SBOM (no synthetic workspace-root); consumers see N independent packages. Matches cargo's milestone-064 + maven's milestone-070 pattern. For pub-workspace single-lockfile installs (Dart 3.6+), dep edges are attributed to each member's main-module via the `pubspec.yaml`'s declared direct deps.
- Q: In design-tier mode (no `pubspec.lock`, only `pubspec.yaml`), should the reader emit components for `dev_dependencies:` entries, and if so, tagged how? → A: Both, tagged (Option A). Emit components for BOTH `dependencies:` and `dev_dependencies:` entries; tag dev ones with `mikebom:lifecycle-scope = "development"` so `--include-dev=off` filters them out. Symmetric with FR-008's lockfile-mode behavior — operators get consistent results whether or not a lockfile is present.

#### Phase 0 research corrections (post-clarification)

Plan-phase research against the [purl-spec `pub` definition](https://github.com/package-url/purl-spec/blob/main/types-doc/pub-definition.md) surfaced three corrections to the initial PURL-shape guesses in FR-003 + FR-011. These are CORRECTIONS to align with the purl-spec authority, not scope changes:

- **Hosted qualifier name**: the canonical qualifier is `repository_url=<base-url-with-scheme>`, NOT `host=<bare-hostname>`. Omitted when `description.url` is `"https://pub.dev"` OR `"https://pub.dartlang.org"` (both default URLs; the latter is the legacy purl-spec-recorded default that redirects to `pub.dev`).
- **Git source PURL form**: per the purl-spec git-source cross-type convention, the `vcs_url` qualifier value carries a `git+` scheme prefix (e.g., `?vcs_url=git+https://github.com/foo/bar.git`), and `description.path` (subdirectory inside the repo) is preserved as a `#<subpath>` PURL fragment.
- **SDK pseudo-deps**: the purl-spec EXPLICITLY blesses `pkg:pub/flutter@0.0.0` as a canonical example. The initial FR-011 instruction to use `pkg:generic/<sdk-name>-sdk` instead was incorrect — SDK pseudo-deps MUST emit as `pkg:pub/<sdk-name>@0.0.0` per purl-spec, with `mikebom:source-type = "pub-sdk"` annotation surfacing the discriminator (the `pub-` prefix on all source-type values avoids collision with cargo's bare C1 values per research R3). The `0.0.0` version is the literal placeholder `pub` writes into the lockfile for SDK entries; preserving it matches purl-spec example output.

FR-003 + FR-011 are updated below to reflect these corrections.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Operator scans a Flutter app project (Priority: P1) 🎯 MVP

A mobile developer runs `mikebom sbom scan --path .` on their Flutter app source tree. They receive an SBOM containing one component per package pinned in `pubspec.lock`. Each component carries a `pkg:pub/<package>@<version>` PURL identity and a dependsOn edge from the app's root component to each of its direct deps.

**Why this priority**: The headline use case. Without it, the entire feature has no operator value — every Flutter app is the canonical target, and pubspec.lock is the universal artifact.

**Independent Test** (SC-001): Synthetic fixture with `pubspec.yaml` declaring 3 direct deps + `pubspec.lock` pinning those + their 2 transitive deps (5 total). Run `mikebom sbom scan --path <tmp>`. Assert exactly 5 `pkg:pub/*` components emit with correct versions and the project's direct-dep edges target the correct bom-refs.

**Acceptance Scenarios**:

1. **Given** a Flutter app project with `pubspec.lock` pinning `http 1.1.0`, `provider 6.1.1`, `shared_preferences 2.2.2`, **When** the operator runs `mikebom sbom scan --path <project>`, **Then** the emitted SBOM contains components for each pinned dep with PURL `pkg:pub/<name>@<version>`.
2. **Given** the same project, **When** the operator inspects the emitted SBOM, **Then** transitive deps pinned in `pubspec.lock` (e.g., `http_parser`, `meta`) also appear as components — the lockfile is the authoritative dep set, not just direct deps.
3. **Given** a source tree WITHOUT `pubspec.lock` or `pubspec.yaml`, **When** the operator scans, **Then** no Dart components or annotations appear AND no warning fires (clean no-op).
4. **Given** a project whose `pubspec.yaml` declares `name: my_flutter_app` and `version: 1.2.3`, **When** the operator scans, **Then** a main-module component emits with PURL `pkg:pub/my_flutter_app@1.2.3`, `mikebom:component-role = "main-module"`, and `mikebom:sbom-tier = "source"` annotations; dep edges flow from this main-module bom-ref to each direct dep's bom-ref.

---

### User Story 2 — Operator distinguishes hosted vs git/path/SDK deps (Priority: P2)

The operator's Flutter app's `pubspec.lock` mixes hosted deps (from pub.dev), a `path:` dep pointing to a shared in-monorepo package, a `git:` dep pinning a fork, and the standard Flutter SDK pseudo-deps. The SBOM must distinguish these sources so downstream supply-chain risk tooling can correctly assess each (path deps + git deps + SDK deps have meaningfully different risk profiles than hosted deps).

**Why this priority**: Important for supply-chain risk assessment but the headline value (US1) ships independently. Path/git/SDK handling is a refinement layered on top of the baseline hosted-dep extraction.

**Independent Test** (SC-002): Synthetic fixture with one each of hosted/path/git/sdk dep in `pubspec.lock`. Scan. Assert that:
- The hosted dep emits as `pkg:pub/<name>@<version>` (standard) with `mikebom:source-type = "pub-hosted"` evidence.
- The path dep emits with `mikebom:source-type = "pub-path"` evidence and a `pkg:generic/` PURL (not pub.dev's namespace).
- The git dep emits with `mikebom:source-type = "pub-git"` evidence; PURL carries the resolved git SHA per the purl-spec git-source convention.
- The SDK pseudo-dep (`flutter`) emits as `pkg:pub/flutter@0.0.0` per purl-spec canonical example; the `mikebom:source-type = "pub-sdk"` annotation distinguishes it from pub.dev-hosted deps via the standard property filter.

**Acceptance Scenarios**:

1. **Given** a `pubspec.lock` containing a `path:` dep entry, **When** the operator scans, **Then** that dep emits with `mikebom:source-type = "pub-path"` evidence; downstream filtering on the property surfaces only path-sourced deps.
2. **Given** a `pubspec.lock` containing a `git:` dep with a `resolved-ref:` SHA, **When** the operator scans, **Then** the emitted PURL embeds the resolved SHA as the version segment (per purl-spec's git-source convention) AND carries `mikebom:source-type = "pub-git"` evidence.
3. **Given** a `pubspec.lock` containing the `flutter` SDK pseudo-dep (`source: sdk`, `version: "0.0.0"`), **When** the operator scans, **Then** the emitted component carries PURL `pkg:pub/flutter@0.0.0` per purl-spec canonical example AND `mikebom:source-type = "pub-sdk"` annotation so downstream consumers can distinguish SDK pseudo-deps from pub.dev-hosted deps via the standard property filter.

---

### User Story 3 — Operator scans a Dart project WITHOUT a committed lockfile (Priority: P3)

Some Dart projects (especially libraries published to pub.dev) deliberately do NOT commit `pubspec.lock` — only `pubspec.yaml` with version constraints. Scanning such a project should produce SOME inventory (the declared direct deps with their constraint expressions) rather than empty output, marked as `design`-tier (not `source`-tier) to reflect the lower fidelity.

**Why this priority**: Important for libraries-and-packages publishers (smaller user base than app authors) but not blocking. The lockfile-driven slice in US1 is the headline.

**Independent Test** (SC-003): Synthetic fixture with `pubspec.yaml` declaring 2 direct deps but NO `pubspec.lock`. Scan. Assert: (a) 2 components emit with `mikebom:sbom-tier = "design"` annotation, (b) each carries the declared constraint string as a `mikebom:requirement-range` annotation so consumers see "what was declared" not "what was resolved".

**Acceptance Scenarios**:

1. **Given** a Dart library project with `pubspec.yaml` only (no `pubspec.lock`), **When** the operator scans, **Then** components emit for declared direct deps from BOTH `dependencies:` and `dev_dependencies:` blocks, each with `mikebom:sbom-tier = "design"` and the original version constraint preserved as evidence.
2. **Given** the same project, **When** the operator inspects the emitted SBOM, **Then** NO transitive deps appear (lockfile is required for transitive resolution; design-tier captures only what's explicitly declared).
3. **Given** a project with `dev_dependencies:` declaring `test: ^1.24.0`, **When** the operator scans in design-tier mode, **Then** the emitted `test` component carries `mikebom:lifecycle-scope = "development"` annotation; downstream `--include-dev=off` filtering on that property successfully suppresses it.

---

### Edge Cases

- **Mixed hosted/path/git/SDK in one project**: a Flutter app commonly has all four source kinds in one `pubspec.lock`. Each MUST surface with the correct `source-type` evidence; none MUST silently masquerade as a pub.dev hosted dep.
- **Self-hosted pub registry**: some organizations run an internal pub mirror. The `hosted` source-description in `pubspec.lock` carries a `url:` field. When `url` != `https://pub.dev` AND `url` != `https://pub.dartlang.org` (the two defaults), emit a `repository_url=<full-url-with-scheme>` qualifier on the PURL per the purl-spec authority (`pkg:pub/foo@1.0.0?repository_url=https://pub.acme.example.com`). See FR-003 + research R3 for the canonical form.
- **Workspace projects (Melos)**: a monorepo with multiple `pubspec.yaml` files (one per workspace member), each with its own sibling `pubspec.lock`. Each member's lockfile is parsed independently; same-PURL deps across multiple lockfiles collapse via the standard `seen_purls` dedup.
- **Pub-workspace (Dart 3.6+ single root lockfile)**: a workspace where the root `pubspec.yaml` declares `workspace:` field listing member packages, and ONE `pubspec.lock` lives at the root (members lack sibling lockfiles). v1 emits each member as a main-module + design-tier deps from THAT member's `pubspec.yaml`; the root-level lockfile is NOT walked to inherit pinned versions to each member. **Pub-workspace pinned-version inheritance is deferred to v1.1** — see research R6 for the v1.1 design sketch. v1 still produces a usable SBOM (unified-view: all member deps surface; only version-pinning fidelity is design-tier instead of source-tier).
- **Git deps without a resolved SHA**: malformed lockfile or interrupted `pub get`. Skip the dep with `tracing::warn!` rather than emit a placeholder PURL (would create a non-identifying component).
- **Pre-pub-2.0 lockfile format**: very old Dart projects use a different lockfile shape. Out of scope for v1 — modern Dart 2.0+ (released 2018) is universal in 2026 production.
- **Malformed `pubspec.lock`**: skip-the-file with `tracing::warn!`; downstream emission produces zero Dart components for that project rather than aborting the whole scan.
- **Dev dependencies vs regular dependencies**: `pubspec.lock` carries a `dependency: direct main|direct dev|transitive` field. Direct-dev deps SHOULD be tagged with `mikebom:lifecycle-scope = "development"` so the standard `--include-dev=off` filtering works (matches dpkg/maven/npm precedent).
- **Empty `pubspec.lock`**: a project with `pubspec.lock` but zero packages section (fresh `pub get` failure). Skip silently — no components emit, no warnings.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST detect Dart/Flutter projects by the presence of `pubspec.lock` or `pubspec.yaml` files anywhere under the scan root. Both file names participate; presence of either triggers reader activation.
- **FR-002**: System MUST parse `pubspec.lock` (YAML format) and extract: each package's name (top-level key under `packages:`), version, source discriminator (`hosted` / `path` / `git` / `sdk`), the source-specific identity payload (URL for hosted, path for path, repo + ref for git), and `dependency:` classification (`direct main`, `direct dev`, or `transitive`).
- **FR-003**: System MUST emit one component per parsed lockfile entry with PURL according to the source type (shapes confirmed against the [purl-spec `pub` definition](https://github.com/package-url/purl-spec/blob/main/types-doc/pub-definition.md)):
  - **hosted**: `pkg:pub/<package>@<version>[?repository_url=<base-url-with-scheme>]` (`repository_url=` qualifier omitted when `description.url` is `"https://pub.dev"` OR `"https://pub.dartlang.org"` — both default URLs; the latter is the legacy purl-spec-recorded default that redirects to `pub.dev`. Present for self-hosted mirrors.)
  - **git**: `pkg:pub/<package>@<resolved-sha>?vcs_url=git+<git-url>[#<subpath>]` per purl-spec git-source convention (`git+` scheme prefix MUST appear in the `vcs_url` value; `description.path` carries as the PURL `#<subpath>` fragment when non-trivial)
  - **path**: `pkg:generic/<package>@<version>` with `mikebom:source-type = "pub-path"` evidence — path-deps have no purl-spec-addressable identity, so the `pkg:generic/` placeholder + annotation surface the discriminator while preserving a usable bom-ref for dep-graph wiring
  - **sdk**: `pkg:pub/<sdk-name>@0.0.0` (the literal placeholder version `pub` writes for SDK entries) with `mikebom:source-type = "pub-sdk"` evidence. Per purl-spec canonical example (`pkg:pub/flutter@0.0.0`) — SDK pseudo-deps ARE addressable via the pub-type PURL; the annotation distinguishes them from pub.dev-hosted deps
- **FR-004**: System MUST emit dependency edges from each project's main-module (per FR-012) to each direct dep declared in `pubspec.yaml` (`dependencies:` + `dev_dependencies:` when `--include-dev=on`) OR — when a `pubspec.lock` is present — to each `dependency: direct main` / `direct dev` / `direct overridden` entry. Transitive components (lockfile `dependency: transitive`) surface as standalone components but their inter-package dependency edges (per-`LockfileEntry` `dependencies:` arrays) are deferred to v1.1 — scope-aligned with the milestone-064 / 066 / 068 / 069 / 070 v1 convention (main-module → direct deps in v1; full transitive edge graph in a follow-up).
- **FR-005**: When `pubspec.lock` is absent but `pubspec.yaml` is present, system MUST emit components for direct deps declared in BOTH `dependencies:` and `dev_dependencies:` blocks, with `mikebom:sbom-tier = "design"` annotation and the original constraint string as `mikebom:requirement-range` evidence. Components from `dev_dependencies:` MUST additionally carry `mikebom:lifecycle-scope = "development"` annotation so `--include-dev=off` filtering applies uniformly (symmetric with FR-008's lockfile-mode behavior). No transitive deps emit in this design-tier mode.
- **FR-006**: System MUST treat a source tree containing no `pubspec.lock` AND no `pubspec.yaml` as a clean no-op — no components emitted, no warnings logged. Existing scans on non-Dart projects MUST stay byte-identical pre/post this feature.
- **FR-007**: System MUST tolerate per-lockfile parse errors (malformed YAML, missing required fields, encoding issues) without aborting the whole scan — log a structured warning naming the affected lockfile path and continue.
- **FR-008**: System MUST tag direct-dev deps (lockfile `dependency: direct dev`) with `mikebom:lifecycle-scope = "development"` so the standard `--include-dev=off` filtering works (matches dpkg/maven/npm precedent).
- **FR-009**: System MUST handle Dart workspace projects (Melos monorepos with multiple `pubspec.yaml`/`pubspec.lock` files, OR pub workspaces with a single root lockfile per Dart 3.6+) by emitting **one main-module component per `pubspec.yaml`** regardless of monorepo membership. Each member's lockfile (when present) is parsed independently for the dep edges attributed to that member's main-module. Same-PURL deps across multiple lockfiles collapse via the standard cross-component `seen_purls` dedup at the orchestrator level. No synthetic workspace-root component is emitted — workspace structure is invisible at the SBOM level; consumers see N independent packages. Mirrors the cargo (milestone 064) + maven (milestone 070) workspace-member pattern.
- **FR-010**: System MUST NOT make any network calls during the scan — `pubspec.lock` is fully self-contained. Resolving a hosted-dep's registry URL via a remote query is out of scope.
- **FR-011**: For Flutter SDK pseudo-deps (`flutter`, `flutter_test`, `flutter_localizations`, `flutter_web_plugins`, etc. — detectable via the `source: sdk` discriminator in the lockfile), system MUST emit one component per entry with PURL `pkg:pub/<sdk-name>@0.0.0` (the `0.0.0` is the literal placeholder version `pub` writes for SDK entries; preserving it matches the purl-spec canonical example `pkg:pub/flutter@0.0.0`). The component MUST carry `mikebom:source-type = "pub-sdk"` annotation so consumers can distinguish toolchain-bundled deps from pub.dev-hosted ones via the standard property filter. (The `pub-` prefix on all `mikebom:source-type` values avoids collision with existing C1 parity-catalog row values like cargo's bare `git` / `path` / `registry`; per the milestone-122 Kotlin DSL `kmp-` precedent — see research R3.)
- **FR-012**: For each `pubspec.yaml` encountered under the scan root, system MUST emit one **main-module component** with PURL `pkg:pub/<name>@<version>` (where `<name>` and `<version>` come from the `pubspec.yaml`'s `name:` and `version:` fields). The component MUST carry `mikebom:component-role = "main-module"` and `mikebom:sbom-tier = "source"` annotations. Dep edges MUST flow from this main-module to each direct dep declared in `pubspec.yaml` (or, when a `pubspec.lock` is present, to each `dependency: direct main`/`direct dev` entry per FR-008). When the `pubspec.yaml` lacks a `version:` field (libraries-in-development), use `0.0.0-unknown` as the version placeholder per the existing cargo main-module convention (milestone 064). Mirrors the established milestone-064 (cargo) / 066 (npm) / 068 (pip) / 069 (gem) / 070 (maven) main-module emission pattern.

### Key Entities

- **pubspec.lock**: YAML lockfile pinning each direct + transitive dep of a Dart project to a specific version. Each `packages:` entry carries name (key), description (source-specific payload), `source:` discriminator (hosted / path / git / sdk), version, and a `dependency:` classification.
- **pubspec.yaml**: Declared dep manifest in the project root. Required for any Dart project; `pubspec.lock` is optional (libraries often omit it). Lower fidelity than lockfile (constraints not pinned versions).
- **`.dart_tool/package_config.json`**: Resolved package-path map written by `dart pub get` post-resolution. Not consumed by milestone 137 — the lockfile is the canonical source.
- **Package source**: Discriminator for where a dep came from. Four values: `hosted` (pub.dev or self-hosted mirror), `path` (filesystem-local), `git` (git repo + ref), `sdk` (Flutter / Dart SDK pseudo-deps).
- **Flutter SDK pseudo-dep**: A dep name like `flutter` that resolves to the Flutter framework shipped with the project's installed Flutter version, NOT to pub.dev. Has no pub.dev provenance; must NOT emit as `pkg:pub/flutter@...`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A scan of a synthetic Flutter app project with `pubspec.yaml` (3 direct deps) + `pubspec.lock` (those 3 plus 2 transitives = 5 total) produces a CDX SBOM whose Dart component count matches the lockfile package count exactly (5) and direct-dep edges target real bom-refs.
- **SC-002**: A scan of a fixture mixing one hosted, one path, one git, and one SDK dep produces correct PURLs for each per FR-003: hosted as `pkg:pub/<name>@<version>` (or with `?repository_url=` for self-hosted), git as `pkg:pub/<name>@<resolved-sha>?vcs_url=git+<url>` (with `#<subpath>` when applicable), path as `pkg:generic/<name>@<version>` (placeholder), sdk as `pkg:pub/<sdk-name>@0.0.0` (per purl-spec canonical example). Each carries the correct `mikebom:source-type` evidence (`pub-hosted` / `pub-git` / `pub-path` / `pub-sdk`).
- **SC-003**: A scan of a Dart library project with `pubspec.yaml` only (no `pubspec.lock`) produces components for declared direct deps with `mikebom:sbom-tier = "design"` annotation and the constraint string preserved as `mikebom:requirement-range` evidence.
- **SC-004**: A source tree containing no Dart files produces an SBOM byte-identical (modulo timestamps + serial numbers) to a pre-feature baseline scan. (No-op preservation invariant — protects every non-Dart scan.)
- **SC-005**: A scan completes successfully (exit code 0, valid SBOM) on a fixture where one `pubspec.lock` has corrupted YAML alongside three valid Dart project subdirectories. The output contains components from the three valid projects plus a warning naming the corrupted lockfile path; the corrupted project is silently dropped.
- **SC-006**: An external SBOM consumer reading the emitted CDX JSON can enumerate every Dart/Flutter dep via the standard `components[]` array filtered on `purl =~ "^pkg:pub/"`. No Dart-specific consumer code is required — the standard PURL filter works.
- **SC-007**: A scan of a fixture with one direct-dev dep (lockfile `dependency: direct dev`) produces a component carrying `mikebom:lifecycle-scope = "development"` annotation; downstream `--include-dev=off` filtering on that property successfully suppresses the component.
- **SC-008**: A scan of a project whose `pubspec.yaml` declares `name: my_flutter_app` and `version: 1.2.3` produces a main-module component with PURL `pkg:pub/my_flutter_app@1.2.3` carrying `mikebom:component-role = "main-module"` annotation; the SBOM's `dependencies[]` block contains an entry for the main-module's bom-ref with `dependsOn` targeting every direct dep's bom-ref.

## Assumptions

- **Modern Dart 2.0+ lockfile format only**: pre-2.0 (2018) lockfiles are out of scope. Modern Dart is universal in 2026 production.
- **`pubspec.lock` is the authoritative source when present**: the reader does NOT consult `.dart_tool/package_config.json` for v1. The lockfile is what `dart pub get` produces and is the build's source of truth.
- **No live `dart pub` invocation**: the reader parses on-disk metadata directly. It does NOT shell out to `dart pub deps` or `dart pub get` — the `dart` binary isn't guaranteed to exist on the scan host (mikebom is host-portable; scanned target may be a Dart project on a Linux server scanned from a macOS host).
- **The `pub` PURL type IS purl-spec-blessed**: the [purl-spec PURL-TYPES.rst](https://github.com/package-url/purl-spec/blob/main/PURL-TYPES.rst) defines the `pub` type explicitly. mikebom emits per the spec — no informal-type follow-up needed (unlike `brew` in milestone 136 and `yocto` in milestone 128).
- **Existing milestone-002 language-reader pattern is the template**: the reader will share architectural shape with `cargo.rs` / `npm/` (closest siblings — both parse lockfiles in source-tree-walked project directories). NOT the milestones-002/004/107/135/136 OS-reader pattern (those parse system-installed package DBs).
- **YAML parsing**: `serde_yaml` is already a workspace dep (per milestone 122's Kotlin DSL reader use); zero new Cargo deps.
- **Git-source PURL convention**: per purl-spec, git-sourced packages use the resolved Git SHA as the version segment + `vcs_url=<repo>` qualifier. Aligned with how the existing cargo reader handles `git+https://...` source-kind cargo deps (milestone 002).
- **Path-deps and SDK-deps use `pkg:generic/` placeholders**: these don't have pub.dev provenance so emitting under `pkg:pub/` would be wrong. The `pkg:generic/` placeholder + `mikebom:source-type` evidence properly signals their non-pub nature.

## Out of Scope

- **Live invocation of `dart pub` or any Dart toolchain binary**: read-only metadata parse only.
- **Pre-Dart-2.0 lockfile format**: deferred indefinitely (these are exceptionally rare in 2026).
- **`.dart_tool/package_config.json` parsing**: lockfile-first design; the resolved package-config is post-`dart pub get` evidence and out of scope for v1. File-claim integration (tracking which on-disk paths under `~/.pub-cache/` a project uses) is a separate concern.
- **Constraint-resolution simulation**: when only `pubspec.yaml` is present (no lockfile), we emit at design-tier with the raw constraints preserved. We do NOT attempt to resolve constraints (`^1.0.0`, `>=1.2.0 <2.0.0`, etc.) into pinned versions ourselves.
- **Self-hosted pub registry authentication**: scanning a project whose lockfile points at a private pub registry that requires auth is out of scope — we read the registry URL from the lockfile but never contact the registry.
- **Dart Hosted Pub format v2 future migrations**: the spec targets the current pubspec.lock v2 format (per Dart 3.x). Future Dart format changes would be addressed in follow-up milestones.
- **License extraction**: `pubspec.lock` does NOT carry license information — license lives in each package's own `pubspec.yaml` shipped under `~/.pub-cache/hosted/pub.dev/<pkg>-<ver>/pubspec.yaml`. Same shape as milestone-135 FR-012 (URL) and milestone-136 FR-011 (license) deferrals — out of scope for v1; tracked as cross-reader follow-up.
- **`.dart_tool/package_graph.json`** (Dart 3.6+): newer alternative to package_config.json; same out-of-scope rationale.

## Dependencies and Constraints

- **Builds on milestone 002** (initial language-reader architecture — cargo, npm, pip, etc.).
- **Builds on milestone 122** (Kotlin DSL reader — most recent YAML-parsing precedent using `serde_yaml`).
- **Reuses the existing source-tree walker** (`scan_fs::walk::safe_walk`) — no new walker logic.
- **Does NOT touch existing language readers** — Dart support is strictly additive.
- **Does NOT introduce new external dependencies** — `serde_yaml = "0.9"` is already a direct workspace dep.

## Related

- Closes: #420 (Add Dart/Flutter ecosystem support (pubspec.lock))
- Adjacent: #424 (CocoaPods reader — sibling mobile ecosystem)
- Adjacent: #422 (Elixir/Mix reader — another lockfile language ecosystem)
- Foundational reference: milestone 002 (cargo + npm + pip lockfile readers), milestone 122 (Kotlin DSL — `serde_yaml` precedent)
