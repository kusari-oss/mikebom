# Feature Specification: Erlang/OTP ecosystem reader

**Feature Branch**: `141-erlang-rebar-reader`
**Created**: 2026-06-24
**Status**: Draft
**Input**: User description: "428"

## Background

Erlang/OTP is the runtime substrate for some of the largest fault-tolerant systems in production: telecom infrastructure (Ericsson, Cisco), messaging (WhatsApp pre-Meta-acquisition), distributed databases (Riak, CouchDB, RabbitMQ, MongoDB-derivatives), and is the BEAM-compatible foundation Elixir compiles to (closed by milestone 140). Pure-Erlang projects use **rebar3** as the dominant build tool (replacing the legacy rebar2 around 2015-2017); rebar3 commits a `rebar.lock` for reproducible installs and reads `rebar.config` for declared deps.

mikebom shipped milestone-140 (Elixir/Mix) one milestone ago — that reader covers `mix.lock` + `mix.exs`. But pure-Erlang projects (those NOT using Mix) emit nothing today; every Hex package pulled via `rebar3` — Cowboy, Mnesia, Phoenix LiveView's compile-time core, Ranch, Lager, Jiffy, Telemetry — is invisible to the scan when the project is rebar3-managed.

The Erlang ecosystem has three discrimination surfaces a reader must handle:

- **Hex deps** (the dominant case): from `hex.pm` — Hex serves BOTH Elixir AND Erlang packages (rebar3 publishes to Hex via the `rebar3 hex publish` plugin). PURL: `pkg:hex/<package>@<version>` — identical type to milestone 140.
- **Git deps**: directly-pinned git URLs via `{git, "https://github.com/owner/repo.git", {tag, "1.0"}}` in `rebar.config`. Identity is the resolved git SHA.
- **OTP applications** (`*.app` and `*.app.src`): standard OTP application descriptors that declare runtime/build deps via the `applications:` key. These describe deps a running OTP system needs at startup — DISTINCT from `rebar.config` build-time deps (an OTP app may depend on `kernel`, `stdlib`, `crypto`, etc. — runtime libs that come with the Erlang runtime, NOT Hex packages).

Critically, the rebar3 ecosystem shares **Hex.pm** as the package registry with Elixir. This means `pkg:hex/<package>@<version>` is the correct PURL type for all rebar3 Hex deps — mikebom can reuse the milestone-140 `hex-` source-type prefix conventions + the purl-spec hex-definition + the private-org `repository_url=` qualifier pattern verbatim.

This feature closes the Erlang gap so an operator scanning any rebar3-managed project gets a complete SBOM with every Hex- and git-managed dep represented, with OTP application descriptors surfaced as supplementary evidence for the runtime application graph.

## Clarifications

### Session 2026-06-24

- Q: How should the reader handle apps in `*.app.src::applications:` lists that are neither in `rebar.lock` nor in the hardcoded OTP runtime allowlist (custom OTP applications + potential typos + silent-lockfile-parse-failure regressions)? → A: Emit ALL `applications:` entries not in the lockfile as `pkg:generic/<lib>@unspecified` with `mikebom:source-type = "erlang-otp-runtime"` annotation. The hardcoded allowlist is informational only — apps in the allowlist carry an additional `mikebom:otp-stdlib = "true"` annotation; apps outside the allowlist still emit per Principle VIII (Completeness — over-emission is safer than silent drop; operator-visible discrimination via the standard property filter). Catches custom OTP applications + potential typos + silent-lockfile-parse-failure regressions as well-defined SBOM evidence rather than holes.
- Q: For each `*.app.src` main-module, should its `depends` aggregate from BOTH `rebar.config::{deps, [...]}` AND `*.app.src::{applications, [...]}` (or only one of these sources)? → A: Union both lists. The main-module `depends` set = NAMES in `rebar.config::{deps, [...]}` ∪ atoms in `*.app.src::{applications, [...]}`. Hex packages typically appear in both (build- AND run-time) and dedupe naturally by name; OTP runtime libs surface only via `applications:`; build-only `rebar.config` deps surface even if absent from the runtime application graph. Operator mental model: "everything the app needs at build OR runtime." Both build-time-only and runtime-only edges are preserved as direct edges from the main-module bom-ref. Edges to OTP runtime libs target the `pkg:generic/<lib>@unspecified` placeholder components from Q1 (FR-003 otp-runtime branch).
- Q: Should the `*.app.src` keyword family include `included_applications:` (embedded sub-apps) and `optional_applications:` (OTP 26+ soft deps), or only `applications:`? → A: Union ALL THREE keywords (`applications:` ∪ `included_applications:` ∪ `optional_applications:`) into the main-module `depends` set per Q2. Each derived edge-target component carries a `mikebom:erlang-app-dep-kind` annotation valued `"required"` (from `applications:`) / `"included"` (from `included_applications:`) / `"optional"` (from `optional_applications:`) so operators can filter to hard-deps-only via the standard property filter. Closes the OTP 25+ gap for libraries using `optional_applications:` (e.g., libraries that integrate with `telemetry` when present but don't require it) AND embedded-app releases using `included_applications:`. Names that appear in multiple keyword sets dedupe naturally; the precedence order for `mikebom:erlang-app-dep-kind` when a name appears in multiple sets is required > included > optional (most-binding wins). Principle VIII (Completeness) + Principle X (Transparency via the new annotation).

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Operator scans a rebar3-managed Erlang/OTP project (Priority: P1) 🎯 MVP

An Erlang backend developer runs `mikebom sbom scan --path .` on their rebar3 project source tree containing `rebar.config` + `rebar.lock` + one or more `*.app.src` files. They receive an SBOM containing one component per Hex package pinned in `rebar.lock`. Each component carries a `pkg:hex/<package>@<version>` PURL identity and a dependsOn edge from the app's root component to each direct dep.

**Why this priority**: The headline use case. Every modern rebar3 project commits `rebar.lock`; without this, the entire feature has no operator value.

**Independent Test** (SC-001): Synthetic fixture with `rebar.config` declaring 3 direct deps (`cowboy`, `jiffy`, `lager`) + `rebar.lock` pinning those + their 2 transitive deps (5 total). Run `mikebom sbom scan --path <tmp>`. Assert exactly 5 `pkg:hex/*` components emit with correct names + versions; main-module + dep-edges wire correctly.

**Acceptance Scenarios**:

1. **Given** an Erlang project with `rebar.lock` pinning `cowboy 2.10.0`, `jiffy 1.1.1`, `lager 3.9.2`, **When** the operator runs `mikebom sbom scan --path <project>`, **Then** the emitted SBOM contains components for each pinned dep with PURL `pkg:hex/<name>@<version>`.
2. **Given** the same project, **When** the operator inspects the emitted SBOM, **Then** transitive deps pinned in `rebar.lock` (e.g., `ranch`, `cowlib`, `goldrush`) also appear as components — the lockfile is the authoritative dep set, not just `rebar.config`.
3. **Given** a source tree WITHOUT `rebar.lock` or `rebar.config` or `*.app.src`, **When** the operator scans, **Then** no Erlang components or annotations appear AND no warning fires (clean no-op).
4. **Given** a project whose `*.app.src` file declares `{application, my_app, [{vsn, "1.2.3"}, ...]}`, **When** the operator scans, **Then** a main-module component emits with PURL `pkg:hex/my_app@1.2.3`, `mikebom:component-role = "main-module"`, and `mikebom:sbom-tier = "source"` annotations; dep edges flow from this main-module bom-ref to each direct dep's bom-ref.

---

### User Story 2 — Operator distinguishes Hex vs git deps + OTP runtime apps (Priority: P2)

The operator's rebar3 project's `rebar.config` mixes Hex deps (the default), a `{git, ...}` dep pinning a fork, and the `*.app.src` files declare `applications:` lists that include OTP runtime libs (`kernel`, `stdlib`, `crypto`, `ssl`) — NOT Hex packages but runtime dependencies that the OTP application requires at startup. The SBOM must distinguish these three discrimination axes so downstream supply-chain risk tooling can correctly classify each.

**Why this priority**: Important for supply-chain risk assessment. OTP runtime libs are part of the Erlang/OTP distribution itself (delivered by Ericsson) and have different licensing + provenance than Hex.pm packages.

**Independent Test** (SC-002): Synthetic fixture with one Hex dep + one git dep in `rebar.lock` + one `*.app.src` declaring an `applications:` list including both. Scan. Assert correct PURL shape and `mikebom:source-type` annotation per FR-003.

**Acceptance Scenarios**:

1. **Given** a `rebar.lock` entry `{<<"my_fork">>,{git,"https://github.com/foo/my-fork.git", {ref, "abc123..."}}, 0}`, **When** the operator scans, **Then** the emitted PURL embeds the resolved Git SHA per the purl-spec git-source convention (`pkg:generic/my_fork@<sha>?vcs_url=git+https://github.com/foo/my-fork.git`) AND carries `mikebom:source-type = "erlang-git"` evidence.
2. **Given** a `*.app.src` whose `applications:` list contains `[kernel, stdlib, crypto, cowboy]`, **When** the operator scans, **Then** OTP runtime libs (`kernel`, `stdlib`, `crypto`) emit as `pkg:generic/<lib-name>@<otp-version-or-unspecified>` components with `mikebom:source-type = "erlang-otp-runtime"` evidence so consumers can filter them out of "Hex package" views (these are NOT Hex.pm packages — they ship with the Erlang/OTP runtime distribution).
3. **Given** the same `*.app.src` file, **When** the operator scans, **Then** Hex-package entries in `applications:` (like `cowboy`) DO NOT double-emit — the `rebar.lock` entry takes precedence and emits the canonical `pkg:hex/cowboy@<version>` component; the `*.app.src::applications:` entry is informational evidence and surfaces as `mikebom:otp-applications-listed-in = "<.app.src path>"` annotation on the lockfile-derived component.

---

### User Story 3 — Operator scans an Erlang library project WITHOUT a committed lockfile (Priority: P3)

Some Erlang library projects (especially those published to Hex.pm with permissive version constraints) deliberately do NOT commit `rebar.lock` — only `rebar.config` with constraint tuples like `{cowboy, "~> 2.10"}`. Scanning such a project should produce SOME inventory rather than empty output, marked as `design`-tier.

**Why this priority**: Important for library publishers but smaller user base than app developers. Most production Erlang projects DO commit `rebar.lock`.

**Independent Test** (SC-003): Synthetic fixture with `rebar.config` declaring 2 direct deps but NO `rebar.lock`. Scan. Assert 2 components emit with `mikebom:sbom-tier = "design"` annotation, each carrying the declared constraint string as `mikebom:requirement-range` evidence.

**Acceptance Scenarios**:

1. **Given** an Erlang library project with `rebar.config` only (no `rebar.lock`), **When** the operator scans, **Then** components emit for declared deps from the `{deps, [...]}` block with `mikebom:sbom-tier = "design"` and the original version constraint preserved as evidence.
2. **Given** the same project, **When** the operator inspects the emitted SBOM, **Then** NO transitive deps appear (lockfile is required for transitive resolution).
3. **Given** a `rebar.config` declaring `{profiles, [{test, [{deps, [{meck, "~> 0.9"}]}]}]}` (deps under a profile block), **When** the operator scans in design-tier mode, **Then** the emitted `meck` component carries `mikebom:lifecycle-scope = "development"` annotation (per `:test` profile discriminator); downstream `--exclude-scope dev` filtering successfully suppresses it.

---

### Edge Cases

- **Mixed Hex/git/path in one project**: a Cowboy-using Erlang app with a forked Ranch + a path-source dev pod has all three source surfaces. Each MUST surface with the correct `source-type` evidence.
- **rebar.lock binary-string atom encoding**: `rebar.lock` uses binary-string literals like `<<"cowboy">>` for package names (Erlang term syntax). The regex extractor MUST handle both bare-atom (`cowboy`) AND binary-string (`<<"cowboy">>`) name forms.
- **Hex deps shape variations**: Hex deps in `rebar.lock` can appear as `{<<"name">>, {pkg, <<"name">>, <<"version">>}, 0}` OR the older `{<<"name">>, <<"version">>}` shape. v1 handles both via regex dispatch.
- **OTP runtime libs vs Hex packages**: every `*.app.src` `applications:` list starts with `[kernel, stdlib, ...]`. These are NOT Hex packages and MUST NOT emit as `pkg:hex/kernel@...` — they emit as `pkg:generic/kernel@unspecified` with `mikebom:source-type = "erlang-otp-runtime"` per FR-003.
- **Pre-rebar3 (rebar2) lockfile format**: lacks the unified term format that rebar3 introduced. Out of scope for v1 — rebar3 (released 2015) is universal in 2026 production.
- **Multi-app `apps/` umbrella**: an Erlang umbrella project has `apps/<sub_app>/src/<sub_app>.app.src` for each member, with a shared root `rebar.config` + `rebar.lock`. v1 emits one main-module per `*.app.src` file (matches the Elixir umbrella pattern from milestone 140).
- **`.app` vs `.app.src` distinction**: `.app.src` is the source manifest (committed in git); `.app` is the compiled output (under `_build/`). v1 reads `*.app.src` only; `*.app` files under `_build/` are skipped by the walker.
- **`included_applications:` + `optional_applications:` keywords**: per Q3 clarification, the reader unions atoms from `applications:` + `included_applications:` + `optional_applications:` (in addition to `rebar.config::{deps, ...}` per Q2). Each emitted edge-target carries `mikebom:erlang-app-dep-kind = "required" | "included" | "optional"`; when the same atom appears in multiple sets the precedence is required > included > optional. Closes the OTP 25+ soft-deps gap (e.g., libraries that integrate with `telemetry` when present) AND the embedded-app release pattern.
- **Malformed `rebar.lock`**: skip-the-file with `tracing::warn!`; when sibling `rebar.config` exists, fall back to design-tier emission per FR-005.
- **Build/test scope from `rebar.config` profile blocks**: deps declared inside `{profiles, [{test, ...}]}` or `{profiles, [{dev, ...}]}` blocks map to `mikebom:lifecycle-scope = "development"` (matches the cross-ecosystem convention).
- **Private Hex orgs**: rebar3 supports private Hex orgs via `{deps, [{name, {pkg, name, "1.0", #{repo => "hexpm:acme"}}}]}` shape. v1 handles this by reusing the milestone-140 private-org PURL form (`pkg:hex/acme/<name>@<version>?repository_url=https://repo.hex.pm`).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST detect Erlang/rebar3 projects by the presence of `rebar.lock`, `rebar.config`, OR `*.app.src` files anywhere under the scan root. Any of the three triggers reader activation.
- **FR-002**: System MUST parse `rebar.lock` (Erlang-term-syntax lockfile, rebar3 1.0+ schema). The file is a list of two-element tuples, each containing a list of pinned-dep tuples. Extract each pinned-dep entry's name + source-specific identity:
  - **Hex dep (modern shape)**: `{<<"<name>">>, {pkg, <<"<name>">>, <<"<version>">>}, <depth>}` (or `{<<"<name>">>, {pkg, <<"<name>">>, <<"<version>">>, <<"<sha256-or-empty>">>}, <depth>}` with optional 4th SHA-256 element). Extract name, version, optional inner SHA-256.
  - **Hex dep (legacy shape)**: `{<<"<name>">>, <<"<version>">>, <depth>}` (rebar3 pre-3.7 entries lack the `{pkg, ...}` wrapper). Extract name + version.
  - **Hex dep (private org)**: `{<<"<name>">>, {pkg, <<"<name>">>, <<"<version>">>, <<"<sha>">>, #{repo => <<"hexpm:<org>">>}}, <depth>}` (rebar3 3.13+ map-form). Detect private-org repo string.
  - **git dep**: `{<<"<name>">>, {git, "<url>", {ref, "<sha>"}}, <depth>}` (or `{tag, "..."}` / `{branch, "..."}`). Extract URL, resolved SHA, declared ref form.
- **FR-003**: System MUST emit one component per parsed lockfile entry with PURL according to the source discriminator (Hex deps reuse milestone-140's purl-spec `hex-definition.md` shapes):
  - **hex (default `"hexpm"` repo)**: `pkg:hex/<lc-name>@<version>`. Hex.pm enforces lowercase at publish time so this is typically no-op.
  - **hex (private org, repo `"hexpm:<org>"`)**: `pkg:hex/<org>/<lc-name>@<version>?repository_url=https://repo.hex.pm` per the milestone-140 + Phase 0 Elixir precedent.
  - **git**: `pkg:generic/<name>@<resolved-sha>?vcs_url=git+<url>` per the milestone-140 git-source convention (purl-spec doesn't bless `vcs_url=` for hex). The operator-declared ref form (`{ref, ...}` / `{tag, ...}` / `{branch, ...}`) is preserved as `mikebom:vcs-declared-ref` evidence.
  - **otp-runtime** (libs from `*.app.src::applications:` that don't match any `rebar.lock` entry): `pkg:generic/<lib-name>@unspecified` placeholder + `mikebom:source-type = "erlang-otp-runtime"` evidence. Per Q1 clarification, ALL such entries emit (not just those in the hardcoded allowlist) — covers Ericsson-distributed OTP standard libs (`kernel`/`stdlib`/`crypto`/`ssl`/etc.), custom operator-written OTP applications, and any silent lockfile-parse-failure regressions. Entries in the hardcoded allowlist additionally carry `mikebom:otp-stdlib = "true"` annotation so consumers can filter to "Ericsson stdlib only" via the standard property filter.
- **FR-004**: System MUST emit dependency edges from each project's main-module (per FR-012) to each direct dep declared in `rebar.config`'s `{deps, [...]}` block. Transitive components (lockfile entries not in the manifest's deps list) surface as standalone components but inter-package dependency edges are deferred to v1.1.
- **FR-005**: When `rebar.lock` is absent but `rebar.config` is present, system MUST emit components for direct deps declared in the `{deps, [...]}` block via regex extraction of `{<atom_name>, ...}` tuples, with `mikebom:sbom-tier = "design"` annotation and the original version constraint string as `mikebom:requirement-range` evidence. Deps declared inside `{profiles, [{dev, ...}]}` / `{profiles, [{test, ...}]}` blocks carry `mikebom:lifecycle-scope = "development"`. No transitive deps emit in this design-tier mode.
- **FR-006**: System MUST treat a source tree containing none of `rebar.lock` / `rebar.config` / `*.app.src` as a clean no-op — no components emitted, no warnings logged.
- **FR-007**: System MUST tolerate per-file parse errors without aborting the whole scan — log a structured warning naming the affected file path and continue. When `rebar.lock` is malformed AND a sibling `rebar.config` exists, fall back to design-tier emission per FR-005.
- **FR-008**: System MUST tag deps declared inside `rebar.config`'s `{profiles, [{<env>, [{deps, [...]}]}]}` blocks where `<env>` is `dev` / `test` / `doc` with `mikebom:lifecycle-scope = "development"` (matches milestone-052/137/138/139/140 convention).
- **FR-009**: System MUST handle Erlang umbrella projects (a root `rebar.config` plus `apps/<sub_app>/src/<sub_app>.app.src` for each member) by emitting **one main-module component per `*.app.src` file** regardless of umbrella membership. Each main-module's `applications:` list contributes the OTP-runtime-libs analysis per FR-003 otp-runtime branch. Same-PURL deps across sub-apps collapse via standard `seen_purls` dedup.
- **FR-010**: System MUST NOT make any network calls during the scan — `rebar.lock` is fully self-contained.
- **FR-011**: System MUST preserve content-addressable hashes when present in the lockfile: each Hex entry's inner SHA-256 (4th element of the `{pkg, ...}` tuple when present) flows into `PackageDbEntry.hashes` as a `ContentHash::with_algorithm(HashAlgorithm::Sha256, hex)` entry. Per the milestone-140 Q3 best-effort posture, only hashes that are present + non-empty emit.
- **FR-012**: For each `*.app.src` file encountered under the scan root, system MUST emit one **main-module component** with PURL `pkg:hex/<app_name>@<version>` (where `<app_name>` is the atom in the `{application, <atom>, [...]}` outer tuple and `<version>` is extracted from the `{vsn, "<version>"}` keyword inside the inner list). The component MUST carry `mikebom:component-role = "main-module"` and `mikebom:sbom-tier = "source"` annotations. Per Q2 + Q3 clarifications, the main-module's direct-`depends` set is the **union** of (a) names in `rebar.config::{deps, [...]}`, (b) atoms in `*.app.src::{applications, [...]}`, (c) atoms in `*.app.src::{included_applications, [...]}`, and (d) atoms in `*.app.src::{optional_applications, [...]}`. Names that appear in multiple sources dedupe naturally; build-only `rebar.config` deps surface even if absent from the runtime `applications:` graph; OTP runtime libs (Q1 fallback class) surface even if absent from `rebar.config`. Each derived edge-target component carries a `mikebom:erlang-app-dep-kind` annotation valued `"required"` (from `applications:`), `"included"` (from `included_applications:`), or `"optional"` (from `optional_applications:`); when a name appears in multiple keyword sets the precedence is required > included > optional (most-binding wins). Edges from the main-module bom-ref target each dep's bom-ref — Hex packages resolve via shared name dedup against `rebar.lock`-derived `pkg:hex/<name>@<version>` components; OTP runtime libs target the `pkg:generic/<lib>@unspecified` placeholder components per the FR-003 otp-runtime branch. When `*.app.src` lacks a parseable `{vsn, "..."}` keyword, use `0.0.0-unknown` as the placeholder per the cargo/dart/composer/cocoapods/elixir main-module convention. When `*.app.src` lacks a parseable application name, fall back to the parent-directory basename (milestone-139 Q1 + milestone-140 Q1 cascade pattern).

### Key Entities

- **rebar.lock**: Erlang-term-syntax lockfile. Top-level is a two-element tuple `{"<lock-version>", [<pinned-deps>]}` (where `<lock-version>` is `"1.2.0"` for modern rebar3); the inner list contains pinned-dep tuples. Each pinned-dep entry's first element is the name (as binary-string `<<"<name>">>`); subsequent elements encode source-specific identity.
- **rebar.config**: Declared dep manifest. Erlang source code with `{deps, [...]}` top-level + optional `{profiles, [{<env>, [...]}]}` blocks. Lower fidelity than lockfile.
- **`*.app.src` / `*.app`**: OTP application descriptors. `.app.src` is the source-tree manifest committed in git; `.app` is the compiled output under `_build/`. Shape: `{application, <atom_name>, [{vsn, "<version>"}, {applications, [<atom_list>]}, {included_applications, [<atom_list>]}, {optional_applications, [<atom_list>]}, {description, "..."}, ...]}.`. Three closely-related keyword lists carry dep atoms (per Q3 clarification): `applications:` (hard deps the OTP supervisor MUST start first), `included_applications:` (embedded sub-apps that share the parent's supervision tree), and `optional_applications:` (OTP 26+ soft deps the supervisor tries but doesn't fail on). All three are AT-RUNTIME dep lists, distinct from `rebar.config::{deps, [...]}` which is the BUILD-TIME dep list. Per FR-012, the main-module's `depends` union sources atoms from ALL FOUR (the three runtime keyword lists + `rebar.config::{deps, [...]}`), each tagged with a `mikebom:erlang-app-dep-kind` annotation discriminating required/included/optional/(build-time-only is unmarked since it has no application-graph kind).
- **OTP runtime application**: An application that ships with the Erlang/OTP distribution (Ericsson-maintained) — `kernel`, `stdlib`, `crypto`, `ssl`, `inets`, `mnesia`, `runtime_tools`, etc. NOT a Hex package. Surfaces in `applications:` lists alongside Hex deps; distinguished by being absent from `rebar.lock`.
- **rebar3 profile**: A named build configuration in `rebar.config` (`{profiles, [{<name>, [...]}]}`). Standard profiles: `default` (omitted), `test`, `dev`, `prod`, `doc`, `bench`. Deps inside non-default profiles are scope-discriminated per FR-008.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A scan of a synthetic Erlang project with `rebar.config` (3 direct deps) + `rebar.lock` (those 3 plus 2 transitives = 5 total) produces a CDX SBOM whose Erlang component count matches the lockfile entry count exactly (5) plus 1 main-module (= 6 total Erlang-derived components); direct-dep edges target real bom-refs.
- **SC-002**: A scan of a fixture mixing one Hex dep + one git dep + one `*.app.src` declaring OTP-runtime libs (`kernel`, `stdlib`) produces correct PURLs per FR-003: Hex as `pkg:hex/<name>@<version>` (or `pkg:hex/<org>/<name>@<version>?repository_url=...` for private orgs), git as `pkg:generic/<name>@<sha>?vcs_url=git+<url>`, OTP runtime as `pkg:generic/<lib-name>@unspecified` (per FR-003 otp-runtime branch). Each carries correct `mikebom:source-type` evidence (`erlang-hex` / `erlang-git` / `erlang-otp-runtime` / `erlang-main-module`).
- **SC-003**: A scan of an Erlang library project with `rebar.config` only (no `rebar.lock`) produces components for declared `{deps, [...]}` entries with `mikebom:sbom-tier = "design"` annotation and the constraint string preserved as `mikebom:requirement-range` evidence.
- **SC-004**: A source tree containing no Erlang files produces an SBOM byte-identical (modulo timestamps + serial numbers) to a pre-feature baseline scan. (No-op preservation invariant.)
- **SC-005**: A scan completes successfully (exit code 0, valid SBOM) on a fixture where one `rebar.lock` has corrupted Erlang term syntax alongside three valid Erlang project subdirectories. The output contains components from the three valid projects plus a warning naming the corrupted lockfile; the corrupted project falls back to design-tier emission from its sibling `rebar.config`.
- **SC-006**: An external SBOM consumer reading the emitted CDX JSON can enumerate every Hex-managed Erlang dep via the standard `components[]` array filtered on `purl =~ "^pkg:hex/"`. No Erlang-specific consumer code is required.
- **SC-007**: A scan of a fixture with a `rebar.config` `{profiles, [{test, [{deps, [{meck, "~> 0.9"}]}]}]}` block produces a component carrying `mikebom:lifecycle-scope = "development"` annotation; downstream `--exclude-scope dev` filtering successfully suppresses the component.
- **SC-008**: A scan of a project whose `*.app.src` declares `{application, my_app, [{vsn, "1.2.3"}, {applications, [kernel, stdlib, cowboy]}, ...]}` produces a main-module component with PURL `pkg:hex/my_app@1.2.3` carrying `mikebom:component-role = "main-module"` annotation; the SBOM's `dependencies[]` block contains an entry for the main-module's bom-ref with `dependsOn` targeting `cowboy`'s bom-ref (the Hex-package entry from `applications:`) — OTP runtime libs like `kernel` and `stdlib` also emit as separate `pkg:generic/` components.
- **SC-009**: A scan of an umbrella project with 3 sub-apps under `apps/` (each with own `*.app.src`) produces 3 main-module components — one per sub-app. Same-PURL deps across sub-apps collapse to single component entries via standard dedup.
- **SC-010**: A scan of a fixture whose `*.app.src` declares `{application, my_app, [{vsn, "1.0.0"}, {applications, [kernel, stdlib, cowboy]}, {included_applications, [config_app]}, {optional_applications, [telemetry]}, ...]}.` produces FIVE main-module dep-edge targets (`kernel`, `stdlib`, `cowboy`, `config_app`, `telemetry`). The `cowboy` / `config_app` / `telemetry` components each carry a `mikebom:erlang-app-dep-kind` annotation valued `"required"` / `"included"` / `"optional"` respectively. The `kernel` + `stdlib` components carry `"required"` plus the FR-003 otp-runtime `pkg:generic/<lib>@unspecified` PURL shape. Operators filtering on `mikebom:erlang-app-dep-kind == "optional"` retrieve exactly `telemetry`.

## Assumptions

- **rebar3 3.0+ only** (released 2015): pre-rebar3 (rebar2) lockfile format is out of scope. rebar3 is universal in 2026 production Erlang development.
- **`rebar.lock` is the authoritative source when present**: prefer lockfile over `rebar.config` (design-tier fallback). When neither exists but `*.app.src` files do, emit just the main-modules + their OTP runtime libs (per FR-003 otp-runtime branch).
- **No live `rebar3` invocation**: the reader parses on-disk metadata directly. It does NOT shell out to `rebar3 tree` or `rebar3 deps` — the `rebar3` binary (Erlang escript) isn't guaranteed to exist on the scan host.
- **Hex.pm is the same registry for Erlang AND Elixir**: the `hex` PURL type covers both. Milestone-140's purl-spec audit (canonical form, lowercasing, private-org `repository_url=`) applies verbatim. Cross-ecosystem deps (an Erlang project depending on an Elixir-published Hex package, or vice versa) emit identical PURLs.
- **Existing milestone-140 reader pattern is the template**: the reader will share architectural shape with `elixir.rs` (closest sibling — same Hex.pm registry, same git-source discriminator, similar regex-extraction posture). Reuse the brace-counted tokenization for Erlang's nested tuple syntax (mix.lock's `{...}` and rebar.lock's `{...}` are syntactically similar).
- **Regex parsing of `rebar.lock`**: per the milestone-140 + milestone-069 gem precedent, Erlang lockfile term syntax is regex-extractable. Multi-line tuples handled via brace-counting.
- **`rebar.config` is Erlang source code, not a data format**: same posture as `mix.exs` (Ruby DSL parsed by regex in milestone 140) and `Podfile` (Ruby DSL in milestone 139). Extract `{deps, [...]}` block via regex; no Erlang runtime evaluation.
- **`*.app.src` is also Erlang source code**: term-syntax with simple key-value list inside `{application, <name>, [...]}` outer tuple. Regex-extract `{vsn, "..."}` + `{applications, [...]}` + `{description, "..."}` keywords.
- **OTP runtime libs allowlist**: a hardcoded set of the most common Ericsson-distributed OTP runtime apps (`kernel`, `stdlib`, `crypto`, `ssl`, `inets`, `mnesia`, `runtime_tools`, `sasl`, `os_mon`, `tools`, `compiler`, `syntax_tools`, `xmerl`, `public_key`, `asn1`, `ftp`, `tftp`, etc.). Per Q1 clarification, the allowlist is INFORMATIONAL ONLY — apps in `applications:` lists NOT in the lockfile are emitted regardless of allowlist membership (per FR-003 otp-runtime branch). Allowlisted entries additionally carry `mikebom:otp-stdlib = "true"` annotation; non-allowlisted entries (custom operator-written OTP apps, typos, silent-lockfile-parse-failure regressions) emit without it. Operators filter the two classes via the standard property filter.
- **`mikebom:source-type` value set**: uses the `erlang-` prefix (`erlang-hex` / `erlang-git` / `erlang-otp-runtime` / `erlang-main-module`) to distinguish from milestone-140's `hex-*` Elixir-derived values. Even though both readers emit `pkg:hex/<name>@<version>` for hex deps, the source-type annotation differentiates which ecosystem-reader produced the component (Erlang vs Elixir-from-Mix). Per the established milestone-122 `kmp-` + milestone-137 `pub-` + milestone-138 `composer-` + milestone-139 `cocoapods-` + milestone-140 `hex-` precedent.

## Out of Scope

- **Live invocation of `rebar3` or any Erlang toolchain binary**: read-only metadata parse only.
- **Pre-rebar3 (rebar2) lockfile format**: deferred indefinitely.
- **`rebar.config` Erlang-runtime evaluation**: regex extraction of `{deps, [...]}` block + `{profiles, [...]}` blocks only.
- **Constraint-resolution simulation**: design-tier mode preserves raw constraints.
- **Compiled `*.app` files under `_build/`**: out of scope — `_build/` is skipped by the walker. Only `*.app.src` (source-tree, committed) is consumed.
- **`escript` archives + standalone OTP releases**: out of scope.
- **Per-app transitive dep edges from per-application `applications:` lists**: v1 emits main-module → direct deps only; transitive components surface but their inter-edges are deferred to v1.1.
- **Mixed Erlang/Elixir umbrella projects** (umbrella with both `mix.exs` sub-apps AND `*.app.src` sub-apps): each reader handles its own sub-apps; cross-reader coordination deferred to v1.1.
- **License extraction**: `rebar.lock` does NOT carry license; license lives in each package's hex_metadata. Same shape as prior milestone deferrals.

## Dependencies and Constraints

- **Builds on milestone 002** (initial language-reader architecture).
- **Builds on milestone 140** (Elixir/Mix — Hex registry + private-org `repository_url=` + git `pkg:generic/` pattern + brace-counted tokenizer + Q1 conditional-flattened + Q3 best-effort hash emission). The closest sibling reader.
- **Builds on milestone 069** (gem regex extraction posture).
- **Builds on milestone 137-139** (multi-tier emission, Q1 fallbacks, prefixed `mikebom:source-type` convention).
- **Reuses the existing source-tree walker** (`scan_fs::walk::safe_walk`).
- **Does NOT touch existing language readers** — Erlang support is strictly additive.
- **Does NOT introduce new external dependencies** — `regex` is a workspace dep.

## Related

- Closes: #428 (Add Erlang/OTP ecosystem support (rebar.lock + *.app))
- Adjacent: #422 (Elixir/Mix — closed by milestone 140; sibling Hex.pm ecosystem)
- Foundational reference: milestone 140 (Elixir/Mix reader — purl-spec hex audit + brace-counted tokenizer template), milestone 137-139 (multi-tier emission + Q-cascade fallback patterns)
