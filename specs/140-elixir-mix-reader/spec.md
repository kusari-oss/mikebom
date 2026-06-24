# Feature Specification: Elixir/Mix ecosystem reader

**Feature Branch**: `140-elixir-mix-reader`
**Created**: 2026-06-24
**Status**: Draft
**Input**: User description: "422"

## Background

Elixir is a notable production ecosystem with disproportionate footprint in banking (Bleacher Report, Bank of America), telecom (WhatsApp's spiritual ancestor BEAM stack), real-time messaging (Discord), and high-availability systems where Erlang/OTP's actor model is the architectural foundation. The community is smaller than Ruby/Python by absolute headcount but dense and production-serious — every modern Elixir project commits a `mix.lock` for reproducible installs.

mikebom currently emits **zero** Elixir components when scanning a Phoenix web app, a Nerves embedded project, or any source tree containing `mix.lock`. Every Hex package pulled from `hex.pm` — plus the typically-rich graph of OTP-app dependencies (Plug, Ecto, Phoenix, Tesla, GenStage, Broadway, Oban) — is invisible to the scan.

The Elixir ecosystem has three discrimination surfaces a reader must handle:

- **Hex deps** (the dominant case): from `hex.pm` (the central package registry). PURL: `pkg:hex/<package>@<version>`.
- **Git deps**: directly-pinned git URLs (`{:foo, git: "https://github.com/owner/repo.git", ref: "abc123"}`). Identity is the resolved Git SHA.
- **Path deps**: local-filesystem deps (`{:my_lib, path: "../my-lib"}`) typically used for umbrella apps or in-flight library development.

The `mix.lock` file format is unique among lockfiles supported by mikebom: it's **Elixir source code** parsed by the Mix tooling, not a standardized data format. Each line is roughly `"<pkg_name>": {:hex, :<pkg_name>, "<version>", "<sha256>", [<build_tools>], [<deps>], "<repo>", "<hex_sha256_outer>"},` for Hex deps, with similar tuple-shapes for git/path. The file IS stable + regex-extractable (every prominent SBOM tool does this) — no Elixir runtime needed for parsing.

This feature closes the Elixir gap so an operator scanning any Mix-managed project gets a complete SBOM with every Hex-managed dep represented, source-type discriminators surfaced, and inner+outer SHA-256 hashes preserved.

## Clarifications

### Session 2026-06-24

- Q: When `mix.exs::deps/0` contains env-conditional blocks (`if Mix.env() == :prod do ... end`, multi-clause `deps(env)` dispatch, or routing through `deps(Mix.env())`), how should the regex extractor handle them? → A: Extract every `{:name, ...}` tuple regardless of conditional nesting (flatten all `if`/`unless`/multi-clause branches). Operator may see deps from a dev-only branch in a prod-context scan; design-tier mode is already best-effort and over-inclusion is safer than omission per Principle VIII Completeness. Components extracted from inside a conditional block carry an additional `mikebom:elixir-extraction-mode = "conditional-flattened"` annotation so consumers can detect the precision loss. Matches the milestone-139 CocoaPods Q3 / Podfile-conditional extraction posture.
- Q: For umbrella projects, what should the umbrella ROOT main-module's `depends` edges list — given that the root `mix.exs` typically declares only umbrella-wide tooling (`:dialyxir`, `:credo`) while sub-app `deps/0` declarations are the real production deps? → A: Root's `depends` = its own `deps/0` entries + each sub-app's main-module bom-ref (root → sub-apps + root-level tooling). Two-level topology: the umbrella root orchestrates the sub-apps; each sub-app's own main-module independently lists its production deps. Matches the operator-mental-model of umbrella architecture (root is the project orchestrator; sub-apps are the units of deployment).
- Q: When the inner SHA-256 (package contents) and/or outer SHA-256 (hex-archive wire-transport checksum) is missing or empty-string in a `mix.lock` entry (common for pre-Hex-2.0 entries which lack outer; rare empty-outer cases when the registry hasn't published it yet), what should the reader emit? → A: Emit only the hashes that are present AND non-empty in the lockfile. When inner is present + outer is missing/empty: emit one `ContentHash::sha256` entry. When both present + non-empty: emit two. Empty-string outers are skipped silently (they're a registry artifact, not operator-meaningful). Principle IX-aligned (no synthetic hashes). FR-011 below is updated to reflect this best-effort posture rather than the original "two separate entries" claim.

#### Phase 0 research corrections (post-clarification)

Plan-phase research against the [purl-spec `hex-definition.md`](https://github.com/package-url/purl-spec/blob/main/types-doc/hex-definition.md) and the canonical [elixir-lang/elixir Mix.Dep.Lock](https://github.com/elixir-lang/elixir/blob/main/lib/mix/lib/mix/dep/lock.ex) + [hexpm/hex SCM](https://github.com/hexpm/hex/blob/main/lib/hex/scm.ex) sources surfaced four corrections to initial spec guesses. These are CORRECTIONS to align with the authority, not scope changes:

- **Private Hex orgs use spec-blessed namespace + `repository_url=` qualifier** (NOT a custom `mikebom:hex-repo` annotation as initially proposed). purl-spec `hex-definition.md` defines `pkg:hex/<org>/<name>@<version>` (namespace = private org slug) PLUS `?repository_url=https://repo.hex.pm` for explicit registry pinning. The lockfile records the repo as `"hexpm"` (default) or `"hexpm:<org>"` (private org — colon-prefixed slug, NOT a URL). The reader MUST translate `"hexpm:<org>"` → `pkg:hex/<org>/<name>@<version>?repository_url=https://repo.hex.pm`. This is MORE correct than syft/trivy (both silently strip private-org info) and is Principle V-aligned (standards-native first).
- **Git-source hex deps use `pkg:generic/` placeholder, NOT `pkg:hex/`**. purl-spec hex-definition does NOT define a `vcs_url=` qualifier for the `hex` type, so emitting `pkg:hex/<name>@<sha>?vcs_url=git+<url>` is purl-spec-non-conformant. The honest emission is `pkg:generic/<name>@<resolved-sha>?vcs_url=git+<url>` + `mikebom:source-type = "hex-git"` annotation as the discriminator (path-source already uses this pattern; symmetric posture). The git-source dep has no Hex.pm provenance once it's been swapped to a git remote — `pkg:hex/` would falsely imply registry-resolution.
- **Default repository URL is `https://repo.hex.pm`** (NOT `hex.pm` — the latter is the user-facing web app; `repo.hex.pm` is the registry-API host that purl-spec records as the canonical default).
- **Lockfile tuple positions clarification**: per [hex/lib/hex/scm.ex](https://github.com/hexpm/hex/blob/main/lib/hex/scm.ex), the 5th tuple element is `managers` (atom list like `[:mix]` or `[:rebar3]`), not free-form "build_tools". The 8th element (outer SHA-256) IS truly optional per Q3. `:git` opts can include `ref:` / `branch:` / `tag:` / `submodules:` / `sparse:` / `subdir:` — broader than the initial spec's "ref: only" implication.

FR-002 + FR-003 + FR-011 (already updated) are amended below to reflect these corrections.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Operator scans a Phoenix or Nerves project (Priority: P1) 🎯 MVP

An Elixir backend developer runs `mikebom sbom scan --path .` on their Phoenix web app source tree containing `mix.exs` + `mix.lock`. They receive an SBOM containing one component per Hex package pinned in `mix.lock`. Each component carries a `pkg:hex/<package>@<version>` PURL identity and a dependsOn edge from the app's root component to each direct dep.

**Why this priority**: The headline use case. Every production Elixir project commits `mix.lock`; without this, the entire feature has no operator value.

**Independent Test** (SC-001): Synthetic fixture with `mix.exs` (3 direct deps: `phoenix`, `plug`, `ecto`) + `mix.lock` (5 entries — 3 direct + 2 transitives). Run `mikebom sbom scan --path <tmp>`. Assert exactly 5 `pkg:hex/*` components emit with correct names + versions; main-module + dep-edges wire correctly.

**Acceptance Scenarios**:

1. **Given** a Phoenix project with `mix.lock` pinning `phoenix 1.7.10`, `plug 1.15.2`, `ecto 3.11.1`, **When** the operator runs `mikebom sbom scan --path <project>`, **Then** the emitted SBOM contains components for each pinned dep with PURL `pkg:hex/<name>@<version>`.
2. **Given** the same project, **When** the operator inspects the emitted SBOM, **Then** transitive deps pinned in `mix.lock` (e.g., `telemetry`, `mime`, `plug_crypto`) also appear as components — the lockfile is the authoritative dep set, not just `mix.exs`'s `deps/0` list.
3. **Given** a source tree WITHOUT `mix.lock` or `mix.exs`, **When** the operator scans, **Then** no Elixir components or annotations appear AND no warning fires (clean no-op).
4. **Given** a project whose `mix.exs` declares `def project, do: [app: :my_app, version: "0.5.2", ...]`, **When** the operator scans, **Then** a main-module component emits with PURL `pkg:hex/my_app@0.5.2`, `mikebom:component-role = "main-module"`, and `mikebom:sbom-tier = "source"` annotations; dep edges flow from this main-module bom-ref to each direct dep's bom-ref.

---

### User Story 2 — Operator distinguishes Hex vs git / path deps (Priority: P2)

The operator's Elixir project's `mix.lock` mixes Hex deps (the default), a `:git` dep pinning a fork (`{:git, "https://github.com/foo/bar.git", "abc...", [ref: "main"]}`), and a `:path` dep pointing to an umbrella sibling app (`{:path, "apps/shared_lib", []}`). The SBOM must distinguish these sources so downstream supply-chain risk tooling can correctly assess each.

**Why this priority**: Important for supply-chain risk assessment but the headline value (US1) ships independently.

**Independent Test** (SC-002): Synthetic fixture with one each of `:hex`, `:git`, `:path` dep in `mix.lock`. Scan. Assert correct PURL shape and `mikebom:source-type` annotation per FR-003.

**Acceptance Scenarios**:

1. **Given** a `mix.lock` entry `"my_fork": {:git, "https://github.com/foo/my-fork.git", "abc123def456..."}`, **When** the operator scans, **Then** the emitted PURL embeds the resolved Git SHA per the purl-spec git-source convention (`pkg:hex/my_fork@<sha>?vcs_url=git+https://github.com/foo/my-fork.git`) AND carries `mikebom:source-type = "hex-git"` evidence.
2. **Given** a `mix.lock` entry `"shared_lib": {:path, "apps/shared_lib", []}`, **When** the operator scans, **Then** that dep emits with `mikebom:source-type = "hex-path"` evidence and a `pkg:generic/shared_lib` placeholder PURL.
3. **Given** an umbrella project (a Mix project containing `apps/<sub_app>/mix.exs` files), **When** the operator scans the umbrella root, **Then** sub-app `:path` deps emit with `mikebom:source-type = "hex-path"` AND the path string preserved as `mikebom:path` annotation.

---

### User Story 3 — Operator scans an Elixir library project WITHOUT a committed lockfile (Priority: P3)

Some Elixir library projects (especially those published to Hex.pm with permissive version constraints) deliberately do NOT commit `mix.lock` — only `mix.exs` with `deps/0` function returning constraint tuples like `{:phoenix, "~> 1.7"}`. Scanning such a project should produce SOME inventory rather than empty output, marked as `design`-tier.

**Why this priority**: Important for library publishers but smaller user base than app developers. Most production Elixir projects DO commit `mix.lock`.

**Independent Test** (SC-003): Synthetic fixture with `mix.exs` declaring 2 direct deps but NO `mix.lock`. Scan. Assert 2 components emit with `mikebom:sbom-tier = "design"` annotation, each carrying the declared constraint string as `mikebom:requirement-range` evidence.

**Acceptance Scenarios**:

1. **Given** an Elixir library project with `mix.exs` only (no `mix.lock`), **When** the operator scans, **Then** components emit for declared deps from `deps/0` with `mikebom:sbom-tier = "design"` and the original version constraint preserved as evidence.
2. **Given** the same project, **When** the operator inspects the emitted SBOM, **Then** NO transitive deps appear (lockfile is required for transitive resolution; design-tier captures only what's explicitly declared).
3. **Given** a `mix.exs` declaring `{:credo, "~> 1.7", only: [:dev, :test], runtime: false}`, **When** the operator scans in design-tier mode, **Then** the emitted `credo` component carries `mikebom:lifecycle-scope = "development"` annotation (per `only: [:dev, :test]` discriminator); downstream `--exclude-scope dev` filtering successfully suppresses it.

---

### Edge Cases

- **Mixed Hex/git/path in one project**: a Phoenix app with a forked Plug + an umbrella sub-app dep has all three source types. Each MUST surface with the correct `source-type` evidence; none MUST silently masquerade as a Hex dep.
- **Private Hex organization (`organization: "acme"` option)**: some organizations run a private Hex repository. The Hex lockfile entry can include an optional outer field naming the repo (`"hexpm"` for the default, `"acme"` for private orgs). When the repo isn't `"hexpm"`, emit a `repository_url=` qualifier on the PURL — though the spec is silent on what the URL should be (Hex private orgs resolve from `hex.pm/api/repos/acme` via authenticated download). v1 deferral: surface the repo name as `mikebom:hex-repo` evidence rather than synthesize a `repository_url=`.
- **`mix.lock` syntax edge cases**: Elixir tuples can wrap across lines. The regex-extraction approach MUST handle multi-line tuples (the entry's text is bounded by `{` ... `}` rather than newlines).
- **Pre-Elixir-1.4 lockfile format**: lacks the second `:hex` sigil in the tuple (`{:hex, :name, "version", ...}` vs older `{"version", "sha256"}`). Out of scope for v1 — Elixir 1.4+ (released 2017) is universal in 2026 production. Pre-1.4 detection → warn-and-skip.
- **Erlang `rebar3` deps via `:rebar` source type**: some Hex packages compile via rebar; the lockfile tuple's build_tools array carries `:rebar` instead of `:mix`. v1 treats this as identical to `:mix` Hex deps — the source-type distinction is build-tooling, not provenance.
- **Umbrella projects** (`apps/<sub_app>/mix.exs` under the umbrella root): the root `mix.exs` typically has `apps_path: "apps"` declaration; sub-apps each have their own `mix.exs`. v1 emits one main-module per sub-app's `mix.exs` PLUS one for the umbrella root (consistent with the milestone-064 / 138 monorepo pattern). Same-PURL deps across sub-apps collapse via standard `seen_purls` dedup.
- **Malformed `mix.lock`**: skip-the-file with `tracing::warn!`; when sibling `mix.exs` exists, fall back to design-tier emission per FR-005.
- **`mix.exs` is Elixir source code, not a data format**: parsing it requires either a real Elixir parser OR regex-extraction of the `deps/0` function body. v1 uses regex-extraction (matches the gem reader's posture from milestone 069 for Ruby `.gemspec`).
- **Build/dev/test scope classification**: `mix.exs` deps can carry `only: [:dev, :test]` or `runtime: false` keywords. These map to mikebom's `LifecycleScope::Development` (matches the cross-ecosystem convention from milestones 052 + 137 + 138).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST detect Elixir/Mix projects by the presence of `mix.lock` OR `mix.exs` files anywhere under the scan root. Either triggers reader activation.
- **FR-002**: System MUST parse `mix.lock` (Elixir-syntax tuple lockfile, Elixir 1.4+ schema) and extract from each top-level map entry: package name (the map key), source-type discriminator (`:hex`/`:git`/`:path` — the first tuple element after `{`), and source-specific identity payload:
  - **`:hex`** tuple shape: `{:hex, :<atom_name>, "<version>", "<inner_sha256>", [<managers>], [<deps>], "<repo>", "<outer_sha256>"}`. Per Phase 0 research, the 5th element is `managers` (atom list — `[:mix]` / `[:rebar3]` / `[:make]`), not free-form "build_tools"; the 8th element (outer SHA-256) IS truly optional (pre-Hex-2.0 entries lack it — handle as `Option<String>`). The `<repo>` string is `"hexpm"` for default registry OR `"hexpm:<org>"` (colon-prefixed slug) for private Hex organizations. NOT a URL.
  - **`:git`** tuple shape: `{:git, "<url>", "<resolved_sha>", [<opts>]}`. Extract URL, resolved 40-char SHA, optional opts keyword list. Opts may contain `ref:`/`branch:`/`tag:`/`submodules:`/`sparse:`/`subdir:` per Phase 0 research.
  - **`:path`** tuple shape: `{:path, "<path>", [<opts>]}`. Extract path string. Opts may contain `in_umbrella: true` for umbrella sub-app deps.
- **FR-003**: System MUST emit one component per parsed lockfile entry with PURL according to the source discriminator (shapes per the [purl-spec `hex-definition.md`](https://github.com/package-url/purl-spec/blob/main/types-doc/hex-definition.md)):
  - **hex (default `"hexpm"` repo)**: `pkg:hex/<name>@<version>`. Names lowercased per purl-spec canonical form (Hex.pm enforces lowercase at publish time, so this is typically a no-op).
  - **hex (private organization, repo `"hexpm:<org>"`)**: `pkg:hex/<org>/<name>@<version>?repository_url=https://repo.hex.pm` per purl-spec hex-definition (namespace = private-org slug; `repository_url=` qualifier carries the canonical hex registry host). The reader MUST split the lockfile's `"hexpm:<org>"` repo string on the first colon to extract the org slug. Per Phase 0 correction, this replaces the initial `mikebom:hex-repo` annotation proposal — purl-spec actually blesses both the namespace-as-org form AND the `repository_url=` qualifier.
  - **git**: `pkg:generic/<name>@<resolved-sha>?vcs_url=git+<url>` per Phase 0 correction — purl-spec hex-definition does NOT define a `vcs_url=` qualifier for the `hex` type, so emitting `pkg:hex/<name>@<sha>?vcs_url=git+<url>` would be non-conformant. `pkg:generic/` is honest about the lack of Hex.pm provenance once the dep is git-swapped. Plus `mikebom:source-type = "hex-git"` annotation as discriminator. The operator-declared opt (`ref:`/`branch:`/`tag:`/`commit:`) from the lockfile tuple is preserved as `mikebom:vcs-declared-ref` evidence when present.
  - **path**: `pkg:generic/<name>@<version-or-unspecified>` placeholder + `mikebom:source-type = "hex-path"` evidence + `mikebom:path = "<lockfile-path-value>"` annotation. Path-deps have no Hex.pm-addressable identity.
- **FR-004**: System MUST emit dependency edges from each project's main-module (per FR-012) to each direct dep declared in `mix.exs`'s `deps/0` function (lockfile mode: cross-reference lockfile entries against `mix.exs`'s direct-dep list; design-tier mode: use `deps/0` directly). Transitive components (lockfile entries not declared in `mix.exs`) surface as standalone components but their inter-package dependency edges (the per-entry `[<deps>]` array element) are deferred to v1.1.
- **FR-005**: When `mix.lock` is absent but `mix.exs` is present, system MUST emit components for direct deps declared in the `deps/0` function via regex extraction of `{:<name>, "<constraint>"[, <kw_opts>]}` tuples, with `mikebom:sbom-tier = "design"` annotation and the original version constraint string as `mikebom:requirement-range` evidence. Per Q1 clarification, extraction is conditional-flattened: every dep tuple found in the file is emitted regardless of `if`/`unless`/multi-clause function nesting (the regex extractor has no `Mix.env()` visibility). Components whose source line resides inside any conditional construct (`if Mix.env() == ...`, `unless ...`, `case ...`, `def deps(:dev)`, etc.) MUST additionally carry `mikebom:elixir-extraction-mode = "conditional-flattened"` annotation so consumers can detect the precision loss. No transitive deps emit in this design-tier mode.
- **FR-006**: System MUST treat a source tree containing no `mix.lock` AND no `mix.exs` as a clean no-op — no components emitted, no warnings logged. Existing scans on non-Elixir projects MUST stay byte-identical pre/post this feature.
- **FR-007**: System MUST tolerate per-file parse errors (malformed lockfile tuple syntax, missing required fields) without aborting the whole scan — log a structured warning naming the affected file path and continue. When a lockfile is malformed AND a sibling `mix.exs` exists, fall back to design-tier emission from the manifest per FR-005.
- **FR-008**: System MUST tag deps declared with `only: [:dev, :test, :doc]` or `runtime: false` in `mix.exs`'s `deps/0` function with `mikebom:lifecycle-scope = "development"` (matches dpkg/maven/npm/dart/composer/cocoapods precedent). When in lockfile mode, cross-reference the `mix.exs`-declared scope back to the lockfile entry (lockfile itself doesn't carry scope info; `mix.exs` is the authoritative scope source).
- **FR-009**: System MUST handle Elixir umbrella projects (a root `mix.exs` with `apps_path: "apps"` declaration, plus `apps/<sub_app>/mix.exs` for each member) by emitting **one main-module component per `mix.exs`** regardless of umbrella membership. Each sub-app's lockfile (when present) is parsed independently for the dep edges attributed to that sub-app's main-module. Same-PURL deps across sub-apps collapse via the standard cross-component `seen_purls` dedup. No synthetic umbrella-root component is emitted beyond the root `mix.exs`'s own main-module. Per Q2 clarification, the umbrella ROOT main-module's `depends` list is the union of (a) its own `deps/0` entries (typically umbrella-wide tooling like `:dialyxir`, `:credo`) and (b) each sub-app's main-module bom-ref — producing a two-level topology where the root orchestrates the sub-apps and each sub-app independently lists its production deps. Mirrors the cargo (milestone 064) + dart (milestone 137) workspace pattern with the umbrella-specific root-aggregation refinement.
- **FR-010**: System MUST NOT make any network calls during the scan — `mix.lock` is fully self-contained. Resolving a Hex package's `hex.pm` API metadata is out of scope.
- **FR-011**: System MUST preserve content-addressable hashes when present in the lockfile: each Hex entry's INNER SHA-256 (the package-contents hash; 4th tuple element) and OUTER SHA-256 (the hex-archive wire-transport checksum; 8th tuple element when present) flow into `PackageDbEntry.hashes` as separate `ContentHash::with_algorithm(HashAlgorithm::Sha256, hex)` entries. Per Q3 clarification, only hashes that are present AND non-empty are emitted (skip empty-string outers silently per Principle IX accuracy — don't synthesize hashes the lockfile doesn't carry): pre-Hex-2.0 entries with only an inner SHA-256 emit one hash entry; Hex 2.0+ entries with both populated emit two. Per the milestone-138 Composer SHA-1 + milestone-139 CocoaPods SHA-1 precedent, hashes flow via the standards-native CDX `hashes[]` array. Elixir is the first mikebom-supported ecosystem to potentially emit multiple hashes per single component.
- **FR-012**: For each `mix.exs` encountered under the scan root, system MUST emit one **main-module component** with PURL `pkg:hex/<app_name>@<version>` (where `<app_name>` is extracted from the `app: :<atom_name>` keyword in the `project/0` function's return-list, and `<version>` is extracted from the `version: "<version>"` keyword). The component MUST carry `mikebom:component-role = "main-module"` and `mikebom:sbom-tier = "source"` annotations. Dep edges MUST flow from this main-module to each direct dep declared in `deps/0`. When `mix.exs` lacks a parseable `app:` keyword, fall back to the parent-directory basename (per the milestone-138 + 139 Q1/Q3 cascade pattern). When `mix.exs` lacks a parseable `version:` keyword, use `0.0.0-unknown` as the placeholder per the cargo/dart/composer main-module convention.

### Key Entities

- **mix.lock**: Elixir-syntax map literal pinning each direct + transitive dep. Top-level shape: `%{ "<name>": {<tuple>}, ... }`. Each entry's tuple discriminates on the first atom (`:hex` / `:git` / `:path`) and carries source-specific fields. Format is Elixir source code parsed by the Mix tooling; regex-extractable for SBOM purposes.
- **mix.exs**: Elixir source file declaring the Mix project. Contains a `defmodule MyApp.MixProject` with two key functions: `project/0` (returns keyword list with `app:`, `version:`, `deps: deps()`) and `deps/0` (returns list of `{:dep_name, "version", kw_opts}` tuples). Lower fidelity than lockfile.
- **Mix project**: A directory containing `mix.exs`. The project's app name (`:my_app` atom from `app:` keyword) is the conventional Hex package name.
- **Hex package**: A package published to `hex.pm` (or a private Hex organization repo). Identified by `pkg:hex/<name>@<version>` per purl-spec; lowercase-required.
- **Umbrella project**: A Mix project containing `apps/<sub_app>/mix.exs` files; the root `mix.exs` declares `apps_path: "apps"`. Each sub-app is a peer Mix project with its own `deps/0`. Common in domain-decomposed Elixir architectures (e.g., separating `web`, `core`, `worker` apps).
- **Inner/outer SHA-256**: Hex lockfile tuples carry two distinct SHA-256 hashes — the INNER (4th tuple element) is the package contents hash; the OUTER (8th tuple element, optional) is the wire-transport hex-archive checksum. Both flow into mikebom's `hashes[]` per FR-011.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A scan of a synthetic Phoenix project with `mix.exs` (3 direct deps) + `mix.lock` (those 3 plus 2 transitives = 5 total) produces a CDX SBOM whose Elixir component count matches the lockfile entry count exactly (5) plus 1 main-module (= 6 total Elixir-derived components); direct-dep edges target real bom-refs.
- **SC-002**: A scan of a fixture mixing one `:hex`, one `:git`, and one `:path` dep produces correct PURLs per FR-003: hex as `pkg:hex/<name>@<version>` (or `pkg:hex/<org>/<name>@<version>?repository_url=https://repo.hex.pm` for private orgs per Phase 0 correction), git as `pkg:generic/<name>@<resolved-sha>?vcs_url=git+<url>` (per Phase 0 correction — purl-spec doesn't bless `vcs_url=` for hex; pkg:generic/ placeholder is honest), path as `pkg:generic/<name>@<version>` (placeholder). Each carries the correct `mikebom:source-type` evidence (`hex-hex` / `hex-git` / `hex-path`).
- **SC-003**: A scan of an Elixir library project with `mix.exs` only (no `mix.lock`) produces components for declared `deps/0` entries with `mikebom:sbom-tier = "design"` annotation and the constraint string preserved as `mikebom:requirement-range` evidence.
- **SC-004**: A source tree containing no Elixir files produces an SBOM byte-identical (modulo timestamps + serial numbers) to a pre-feature baseline scan. (No-op preservation invariant.)
- **SC-005**: A scan completes successfully (exit code 0, valid SBOM) on a fixture where one `mix.lock` has corrupted tuple syntax alongside three valid Elixir project subdirectories. The output contains components from the three valid projects plus a warning naming the corrupted lockfile; the corrupted project falls back to design-tier emission from its sibling `mix.exs`.
- **SC-006**: An external SBOM consumer reading the emitted CDX JSON can enumerate every Hex-managed dep via the standard `components[]` array filtered on `purl =~ "^pkg:hex/"`. No Elixir-specific consumer code is required.
- **SC-007**: A scan of a fixture with a `mix.exs` `deps/0` entry declared `{:credo, "~> 1.7", only: [:dev, :test]}` produces a component carrying `mikebom:lifecycle-scope = "development"` annotation; downstream `--exclude-scope dev` filtering on that property successfully suppresses the component.
- **SC-008**: A scan of a project whose `mix.exs` declares `app: :my_app` and `version: "0.5.2"` produces a main-module component with PURL `pkg:hex/my_app@0.5.2` carrying `mikebom:component-role = "main-module"` annotation; the SBOM's `dependencies[]` block contains an entry for the main-module's bom-ref with `dependsOn` targeting every direct dep's bom-ref.
- **SC-009**: A scan of a fixture with `mix.lock` entries containing both inner and outer SHA-256 hashes produces CDX `hashes[]` entries with `alg = SHA-256` and BOTH hash values present (two separate hash entries per component when both are available). A scan of a pre-Hex-2.0 fixture with only the inner SHA-256 produces ONE hash entry per component (per Q3 — Principle IX accuracy; no synthetic empty-string hashes).
- **SC-010**: A scan of an umbrella project with 3 sub-apps under `apps/` produces 3 main-module components (one per sub-app's `mix.exs`) plus 1 for the umbrella root — total 4 main-modules. Same-PURL deps across sub-apps collapse to single component entries via standard dedup.

## Assumptions

- **Modern Elixir 1.4+ lockfile format only**: pre-1.4 (released 2017) lockfiles are out of scope. Modern Mix is universal in 2026 production.
- **`mix.lock` is the authoritative source when present**: prefer lockfile over `mix.exs` (design-tier fallback).
- **No live `mix` invocation**: the reader parses on-disk metadata directly. It does NOT shell out to `mix deps.tree` or `mix deps.get` — the `mix` binary (Elixir/Erlang runtime) isn't guaranteed to exist on the scan host (mikebom is host-portable; scanned target may be a containerized Elixir release on Linux scanned from macOS).
- **The `hex` PURL type IS purl-spec-blessed**: the [purl-spec PURL-TYPES.rst](https://github.com/package-url/purl-spec/blob/main/PURL-TYPES.rst) defines the `hex` type explicitly. mikebom emits per the spec.
- **Existing milestone-138/139 language-reader pattern is the template**: the reader will share architectural shape with `composer.rs` (closest siblings — both parse lockfiles in source-tree-walked project directories with source-discriminator handling) and `cocoapods.rs` (multi-source PURL form + git-resolved-SHA + manifest regex-extraction pattern).
- **Regex parsing of `mix.lock`**: per spec Edge Cases, `mix.lock` is Elixir source code but its tuple shape is stable + regex-extractable. Matches the gem reader's posture (Ruby `.gemspec` regex extraction from milestone 069). Multi-line tuples handled via brace-counting.
- **Regex parsing of `mix.exs::deps/0`**: same posture as `mix.lock` — regex-extract the `deps/0` function body and parse each `{:name, "constraint", kw_opts}` tuple. No Elixir runtime needed.
- **Hex package names lowercased**: per purl-spec `hex-definition.md` (confirmed in Phase 0 research). Hex.pm enforces lowercase at publish time so this is typically a no-op, but the rule applies regardless.
- **Git-source PURL convention**: per purl-spec, git-sourced packages use `?vcs_url=git+<url>` qualifier; the resolved 40-char SHA from the lockfile tuple is preserved BOTH in the PURL version segment AND as `mikebom:vcs-ref` evidence (some downstream tools filter on `mikebom:vcs-ref`; others read the version segment).
- **`mikebom:source-type` value set**: uses the `hex-` prefix (`hex-hex` / `hex-git` / `hex-path` / `hex-main-module`) to avoid collision with cargo's existing C1 values + dart's `pub-` + composer's `composer-` + cocoapods's `cocoapods-`. Per the established convention.

## Out of Scope

- **Live invocation of `mix` or any Elixir/Erlang toolchain binary**: read-only metadata parse only.
- **Pre-Elixir-1.4 lockfile format**: deferred indefinitely (exceptionally rare in 2026).
- **Private Hex organization `repository_url=` qualifier**: v1 surfaces the repo name as `mikebom:hex-repo` evidence; emitting a true `repository_url=` would require knowing the operator's private-org URL (`hex.pm/api/repos/<org>` for hex.pm-hosted orgs; arbitrary for self-hosted Hex mirrors). Deferred.
- **`mix.exs` Elixir-runtime evaluation**: regex extraction of `deps/0` function body only. We do NOT execute Elixir code (matches the gem reader's posture from milestone 069 — Ruby `.gemspec` files are parsed lenient regex-only).
- **Constraint-resolution simulation**: when only `mix.exs` is present (no lockfile), we emit at design-tier with raw constraints preserved. We do NOT resolve constraints (`"~> 1.7"`, `">= 1.5.0 and < 2.0.0"`, etc.) into pinned versions.
- **Erlang `rebar.lock` format**: tracked separately as issue #428. This feature ONLY discovers Mix-managed deps; rebar-managed pure-Erlang projects are out of scope.
- **Per-dep transitive edges from individual lockfile tuple's `[<deps>]` element**: v1 emits main-module → direct deps only; transitive components surface but their inter-edges are deferred to v1.1.
- **`deps/` directory walking** for installed-tier scans: spec Out-of-Scope. lockfile is the v1 source of truth.
- **License extraction**: `mix.lock` does NOT carry license; license lives in each package's `mix.exs::package/0::licenses` (or in the published hex_metadata.config under `deps/<pkg>/`). Same shape as prior milestone deferrals.

## Dependencies and Constraints

- **Builds on milestone 002** (initial language-reader architecture).
- **Builds on milestone 137** (Dart — prefixed `mikebom:source-type` convention).
- **Builds on milestone 138** (Composer — multi-tier emission + SHA hash precedent).
- **Builds on milestone 139** (CocoaPods — git-source resolved-SHA pattern + main-module dir-basename cascade).
- **Builds on milestone 069** (gem — Ruby `.gemspec` regex extraction posture, which `mix.lock` + `mix.exs` parsing mirrors).
- **Reuses the existing source-tree walker** (`scan_fs::walk::safe_walk`).
- **Does NOT touch existing language readers** — Elixir support is strictly additive.
- **Does NOT introduce new external dependencies** — `regex` is already a workspace dep.

## Related

- Closes: #422 (Add Elixir/Mix ecosystem support (mix.lock))
- Adjacent: #428 (Erlang/OTP rebar.lock — closely related but distinct ecosystem)
- Foundational reference: milestone 069 (gem regex parsing precedent), milestone 137/138/139 (recent multi-tier language-reader convention)
