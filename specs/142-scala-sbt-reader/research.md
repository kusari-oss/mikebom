# Research — milestone 142 Scala/SBT reader (Phase 0)

Resolves all Technical Context unknowns before Phase 1 design. Decisions either inherit from milestone 070 (Maven — PURL shape) and milestone 140/141 (Elixir/Erlang — DSL regex extraction + multi-tier emission template), or are Scala-specific (the `%` / `%%` / `%%%` declaration syntax, the Scala-version-suffix-in-artifactId convention, the `lazy val` multi-project syntax, the sbt-dependency-lock plugin's JSON schema).

## R1 — PURL spec audit (inherited verbatim from milestone 070)

**Decision**: All Scala/SBT-derived components emit PURL `pkg:maven/<group>/<artifact>@<version>`. The `maven` type is purl-spec-blessed; Maven Central is the registry for the entire Scala ecosystem. The Scala-version suffix (`_2.13` / `_3`) is part of the artifactId — it goes in the `name` slot of the PURL exactly as it appears in the resolved Maven coordinate. No new PURL type, no new qualifier conventions.

**Rationale**: SBT publishes all Scala artifacts (stdlib, libraries, framework deps) to Maven Central under coordinates like `org.typelevel:cats-core_2.13:2.10.0`. The `_2.13` is part of the artifactId because Maven Central treats the cross-built variants as distinct artifacts — they have different POMs, different jars, different SHAs. mikebom mirrors this: `cats-core_2.13` and `cats-core_3` are two different PURLs and must NOT collapse via dedup. The milestone-070 maven reader already encodes this convention; the SBT reader is a thin shell on top of the same PURL construction logic.

**Alternatives considered**:
- Emit Scala-version suffix as a qualifier (`pkg:maven/org.typelevel/cats-core@2.10.0?scala_version=2.13`) — REJECTED. Doesn't match Maven Central reality; consumers would have to denormalize. Cross-built variants would collapse via PURL dedup, losing operator-actionable information.
- Introduce a separate `pkg:scala/` PURL type — REJECTED. Not purl-spec-blessed; splits identity unnecessarily for artifacts that already live on Maven Central.

## R2 — `*.sbt.lock` JSON schema (sbt-dependency-lock plugin)

**Decision**: The reader parses schema versions 1 + 2 of the sbt-dependency-lock plugin's output. Per Q3, candidate files (`*.sbt.lock` glob hits) MUST pass a content-shape validation gate requiring top-level `lockVersion` (integer) AND `modules` (array) keys before being treated as authoritative. Schema-version-specific differences:

**Schema v1** (sbt-dependency-lock 0.x):
```json
{
  "lockVersion": 1,
  "timestamp": "2024-01-15T12:34:56Z",
  "configurations": ["compile", "test", ...],
  "modules": [
    {"org": "org.typelevel", "name": "cats-core_2.13", "version": "2.10.0", "configurations": ["compile"]}
  ]
}
```

**Schema v2** (sbt-dependency-lock 1.x+):
Adds per-module `checksums` array per FR-011:
```json
{
  "lockVersion": 2,
  ...
  "modules": [
    {"org": "org.typelevel", "name": "cats-core_2.13", "version": "2.10.0",
     "configurations": ["compile"],
     "checksums": [
       {"name": "cats-core_2.13.jar", "type": "SHA-256", "checksum": "abc123..."}
     ]
    }
  ]
}
```

**Rationale**: Both schema versions remain in the wild circa 2026. The plugin's GitHub repo documents v2 as the stable schema but v1 lockfiles persist in projects that haven't regenerated. Supporting both costs ~20 lines of conditional logic in the JSON parser.

**Alternatives considered**:
- Only support v2 — REJECTED. Cuts out a meaningful share of in-the-wild lockfiles; the v1→v2 differences are purely additive (v2 adds `checksums`), so handling both is trivial.
- Use a typed `serde_json` `Deserialize` derive — VIABLE but more brittle than indexed field access. The plan keeps it as `serde_json::Value` field-access to gracefully handle minor schema evolution within a major version.

## R3 — Scala-version-suffix algorithm (`%` / `%%` / `%%%`)

**Decision**: The Scala-version-suffix appending logic operates only in design-tier mode (FR-005) — in lockfile mode (FR-002) the suffix is already baked into the `name` field by the plugin's resolver. In design-tier mode, for each `%%`-declared dep, the reader determines the Scala-version suffix via the Q1 inference cascade:

1. Explicit `scalaVersion := "..."` setting in the surrounding `build.sbt` (within the same subproject scope) — preferred.
2. `project/build.properties`'s embedded `scala.version` key (uncommon but legal).
3. Default `_2.13` with `mikebom:scala-version-source = "default-fallback"` annotation (the dominant deployed Scala version circa 2026; >55% production share since 2023).

Suffix shape rules:
- Scala 2.x: `_<major>.<minor>` (e.g., `_2.13`, `_2.12`). Patch version is DROPPED — Maven Central uses `_2.13` for all of `2.13.0` through `2.13.12+`.
- Scala 3.x: bare `_3` (NOT `_3.3`). Maven Central uses one suffix for all Scala-3.x cross-built artifacts.
- `%`-declared deps: NO suffix appended (literal artifactId).
- `%%%`-declared deps: warn-and-skip per Out-of-Scope (Scala.js / Scala Native cross-platform variants are rare and require an additional platform-suffix layer).

The `mikebom:scala-version-source` annotation is a NEW parity-bridge per Principle V audit:
- **CycloneDX 1.6 audit**: no native field carries "this artifactId suffix was inferred via heuristic." `component.scope` / `evidence` fields don't fit.
- **SPDX 2.3 audit**: no native carrier; `Annotation` / `Relationship` types don't have a "heuristic-derived attribute" axis.
- **SPDX 3 audit**: same gap. `LifecycleScopeType` is orthogonal.
- **Outcome**: `mikebom:scala-version-source` is a durable parity-bridge specific to this reader. Will be documented in `docs/reference/sbom-format-mapping.md` Section I, milestone 142 row.

**Rationale**: Scala-version mismatch is the most common Scala-ecosystem dep-resolution bug — emitting a bare `cats-core` (no suffix) when the operator's `build.sbt` says `%%` produces a PURL that points at a non-existent Maven Central coordinate. Defaulting to `_2.13` with operator-visible signal is strictly better than emitting a broken PURL.

**Alternatives considered**:
- Default to `_3` (the newer Scala version) — REJECTED. Scala 3 adoption is still <40% as of 2026; defaulting to it would produce wrong PURLs more often than `_2.13`.
- Skip `%%` deps entirely when `scalaVersion` is unparseable — REJECTED per Q1 Option C analysis. Loses too much completeness; the dominant case (`scalaVersion` IS parseable) is well-served by the cascade.
- Emit both `_2.13` and `_3` variants of every `%%` dep (over-emission) — REJECTED per Q1 Option D. Inflates the SBOM with phantom components.

## R4 — `build.sbt` Scala-DSL extraction strategy

**Decision**: Regex-extract the documented `libraryDependencies` syntax. Three concrete patterns:

1. **Single-add form**: `libraryDependencies += "<group>" %{1,3} "<artifact>" % "<version>" [% <Config>]`
2. **Multi-add seq form**: `libraryDependencies ++= Seq( <dep1>, <dep2>, ... )` — parsed via paren-counted tokenization (analogous to milestone-141's brace-counted tokenizer) to handle multi-line `Seq(...)` blocks.
3. **Single-line shorthand inside a project's settings**: `.settings(libraryDependencies += "<group>" %% "<artifact>" % "<version>")`

The same regex set extracts:
- `name := "<value>"` for main-module name (FR-012).
- `version := "<value>"` for main-module version.
- `organization := "<value>"` for main-module groupId.
- `scalaVersion := "<value>"` for the Q1 inference cascade.
- `lazy val <ident> = project.in(file("<path>")) [.settings(...)]` for Q2 multi-project subproject enumeration.

**Rationale**: Per the milestone-140 + milestone-141 precedent, DSL parsing via regex is sufficient for the dominant declaration patterns. Programmatic dep generation (`libraryDependencies := computeDeps(env)` and similar) is statically unresolvable without a Scala compiler and silently drops per FR-005's best-effort posture. The cost of full Scala parsing (embedding a Scala parser, JVM startup, accept-anything-Scala-syntax complexity) is not justified for the marginal completeness gain on programmatic cases.

**Specific patterns documented in data-model.md §2** (with named-capture-group regex bodies for each).

**Alternatives considered**:
- Use the `scala-parser-combinators` Rust crate or similar — DOES NOT EXIST as a Rust crate; would require shelling out to a Scala compiler or porting one.
- Embed a JS-based Scala AST parser via wasm — REJECTED per Constitution Principle I (Pure Rust, Zero C — and no embedded scripting parsers).
- Use a full PEG grammar for Scala syntax — REJECTED. The maintenance burden vastly exceeds the regex-based approach's coverage gap.

## R5 — Multi-project SBT discovery (Q2 union strategy)

**Decision**: Per Q2, multi-project subprojects are discovered via union of two surfaces:

**Surface A — `lazy val` parsing in root `build.sbt`**:
```scala
lazy val core = project.in(file("core")).settings(libraryDependencies ++= Seq(...))
lazy val server = project.in(file("server")).dependsOn(core)
```
Regex: `lazy\s+val\s+(\w+)\s*=\s*project\s*\.\s*in\s*\(\s*file\s*\(\s*"([^"]+)"\s*\)\s*\)`. Captures: 1 = subproject identifier, 2 = file-path string.

**Surface B — Filesystem walk for `<subdir>/build.sbt`**:
The existing `safe_walk` discovers any `build.sbt` file under the scan root. Subprojects identified via surface A are matched by directory path; orphan `<subdir>/build.sbt` files (present on disk but NOT declared via `lazy val`) emit as standalone main-modules with the subdir basename as the subproject name.

**Dedup**: by canonicalized absolute directory path. When a `lazy val` declaration matches an on-disk subdir, the `lazy val` name + declared path wins (preserves operator-chosen identity).

**Rationale**: Real-world SBT projects mix both conventions — some keep all subproject definitions in the root `build.sbt` (Spark-style), others split per-subproject (Play Framework-style). Union discovery covers both without forcing operators to follow a specific layout convention.

**Rationale (continued)**: The Q2 clarification specifically rejected B (root-only) and C (subdir-only) for this completeness reason. The decision is durable.

**Alternatives considered**:
- Per Q2 Options B/C/D — REJECTED in the clarification phase.
- Parse `Build.scala` (legacy SBT 0.13 multi-project syntax) — REJECTED as Out-of-Scope. Pre-1.0 SBT is rare in 2026 production code.

## R6 — `mikebom:source-type` prefix convention

**Decision**: Scala-derived components carry `mikebom:source-type` annotation values prefixed `scala-`:
- `scala-sbt-lock` — derived from a `*.sbt.lock` entry.
- `scala-sbt-design` — derived from `build.sbt` `libraryDependencies` (design-tier).
- `scala-main-module` — per-subproject root component (one per `lazy val` or per-subdir `build.sbt`).
- (Hypothetical `scala-deps-scala` for `project/Dependencies.scala` extraction — deferred; v1 emits any extracted `Dependencies.scala` deps as `scala-sbt-design` to keep the value-set tight.)

**Rationale**: Per the established cross-milestone convention (`kmp-` milestone 122, `pub-` milestone 137, `composer-` milestone 138, `cocoapods-` milestone 139, `hex-` milestone 140, `erlang-` milestone 141), each new reader gets its own prefix. Scala gets `scala-`.

**Cross-reader interaction**: when an operator's project has BOTH `build.sbt` (Scala/SBT) AND `pom.xml` (Maven) at the same root — a polyglot JVM project — both readers fire. Same-PURL components dedupe via standard `seen_purls` logic. The `mikebom:source-type` annotation reflects whichever reader emitted the canonical component first (deterministic by alphabetical reader order in `read_all` — `maven` < `scala` so maven typically wins on shared coords). This matches the milestone-141 cross-reader interaction policy.

**Alternatives considered**:
- Use `maven-*` prefix for both readers — REJECTED. Loses provenance discrimination.
- Skip the prefix and emit a bare `sbt` value — REJECTED. Inconsistent with the cross-milestone convention.

## R7 — `project/Dependencies.scala` extraction (v1 best-effort)

**Decision**: The reader regex-extracts the common `val foo = "group" %% "artifact" % "version"` pattern from any `project/Dependencies.scala` files found alongside `build.sbt`. Extracted deps are emitted as design-tier components per FR-005. Complex computed deps (`def foo(v: String) = "group" %% "artifact" % v`) are silently dropped per the best-effort posture.

**Rationale**: The `project/Dependencies.scala` convention is widespread in larger Scala projects (separates dep declarations from build settings for readability). Recovering the dominant pattern adds completeness for free. The Scala-version-suffix algorithm from R3 applies identically to extracted Dependencies.scala deps.

**Alternatives considered**:
- Skip `Dependencies.scala` entirely — REJECTED. Misses dep declarations in a meaningful fraction of large Scala projects.
- Parse `Dependencies.scala` as full Scala — REJECTED per R4 alternatives.

## R8 — Regex compile-once via `std::sync::OnceLock`

**Decision**: All regex patterns used by `scala.rs` are compiled once via `static REGEX_NAME: OnceLock<Regex> = OnceLock::new();` at module scope (or function scope when used in a single function). Pattern compile is amortized across every scan invocation per the milestone-141 R7 precedent.

**Rationale**: Same as milestone 141 — the reader's parse helpers are invoked once per discovered artifact (potentially many in multi-project builds), and per-call regex compilation is wasteful. The pattern is established across milestones 069, 137, 138, 139, 140, 141.

**Critical reminder**: regex declarations inside loops or inside functions called from loops trigger `clippy::regex_creation_in_loops` (caught empirically in milestone 141). Always hoist `OnceLock<Regex>` to the function's top-level OR module-level static.

## R9 — Byte-identity SBOM golden preservation (SC-004)

**Decision**: A source tree containing no `build.sbt` / `*.sbt.lock` / `project/build.properties` / `project/Dependencies.scala` files MUST produce an SBOM byte-identical (modulo timestamps + serial numbers) to a pre-feature baseline scan. Gated by the existing 13-ecosystem regression suite (`mikebom-cli/tests/cdx_regression.rs`, `spdx_regression.rs`, `spdx3_regression.rs`).

**Rationale**: Same as milestone 141 R8. Non-Scala projects are the vast majority of scans; any unintentional output drift would break every existing golden.

**Memory note**: Per persistent memory `feedback_cross_host_goldens.md`, goldens are already cross-host-byte-identical (HOME isolated, serial+timestamp masked, hashes stripped) so milestone 142's no-op preservation is verified by re-running the existing suite. No new golden files are introduced.

## R10 — Walker integration: `build.sbt` + `*.sbt.lock` + `project/` discovery

**Decision**: `safe_walk` from `mikebom-cli/src/scan_fs/walk.rs` (milestone 114) discovers the four artifact types. The reader filters by:
- file name `build.sbt`
- file extension `.sbt.lock` (per Q3 glob)
- file name `build.properties` AND parent directory name `project`
- file name `Dependencies.scala` AND parent directory name `project`

Standard excludes apply: `target/`, `project/target/`, `_build/`, `.git/`, `node_modules/`. The reader's own `should_skip_descent` helper extends these with Scala-specific dirs as needed (none currently — SBT's `target/` is already in the universal exclude list).

**Rationale**: The walker already supports arbitrary-depth artifact discovery (cargo's `Cargo.toml`, npm's `package.json`, maven's `pom.xml` all use this pattern). SBT's artifacts follow the same convention — typically at project root for single-module, OR under `<subproj>/` for multi-project layouts.

**Alternatives considered**:
- Hardcode discovery at `./build.sbt` + `./build.sbt.lock` only — REJECTED. Doesn't cover subdir layouts or per-subproject lockfiles.

## Summary table

| # | Decision | Inherits | Risk |
|---|---|---|---|
| R1 | `pkg:maven/<group>/<artifact>@<version>` PURL shape | milestone 070 | none |
| R2 | `*.sbt.lock` v1 + v2 schemas + Q3 content-shape gate | new | low — additive evolution between versions |
| R3 | Q1 Scala-version-suffix inference cascade | new (Q1 clarification) | low — Principle V audit complete |
| R4 | `build.sbt` regex extraction (DSL parsing) | milestone 140/141 precedent | medium — programmatic dep generation drops silently (documented Out-of-Scope) |
| R5 | Q2 multi-project union discovery | new (Q2 clarification) | low — dedup is straightforward |
| R6 | `scala-*` source-type prefix | milestone 122+137-141 | none |
| R7 | `project/Dependencies.scala` best-effort extraction | new | low — same posture as R4 |
| R8 | OnceLock regex compile pattern | milestone 069+137-141 | none — empirical lesson from 141 informs hoisting placement |
| R9 | Byte-identity SBOM golden | milestone 002+ (every reader) | low — gated by existing regression suite |
| R10 | safe_walk discovery + standard excludes | milestone 114 | none |

All NEEDS CLARIFICATION resolved. Phase 1 ready.
