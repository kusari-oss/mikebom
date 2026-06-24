# Feature Specification: Scala/SBT ecosystem reader

**Feature Branch**: `142-scala-sbt-reader`
**Created**: 2026-06-24
**Status**: Draft
**Input**: User description: "426"

## Background

Scala is the dominant statically-typed functional/object-oriented language on the JVM. It powers some of the largest production systems in the data-engineering space (Apache Spark — written in Scala; Apache Kafka — Scala-derived; Akka — actor-model framework; Apache Flink — stream processing; Lihaoyi's full toolchain). SBT (Simple Build Tool) is the dominant Scala build tool by a wide margin; competitors (Mill, CBT, Bloop) exist but SBT is the de-facto standard for any non-trivial Scala project.

mikebom currently covers two JVM build ecosystems via dedicated readers: Maven (milestone 070 — `pom.xml` parsing) and Gradle (`build.gradle` + `build.gradle.kts` via milestones 106 + 122). But Scala projects use SBT and produce neither `pom.xml` nor `build.gradle` — they use `build.sbt` (Scala-DSL build definition) plus, when the sbt-dependency-lock plugin is installed, a `*.sbt.lock` JSON lockfile. Without an SBT reader, every Scala dep — Cats, Spark, Akka, Play Framework, ZIO, Scala stdlib — is invisible to mikebom scans of Scala source trees.

The Scala-on-Maven-Central convention is critical: SBT publishes all artifacts (Scala stdlib, Scala libraries, framework deps) to Maven Central with a Scala-version-suffixed artifactId. For example, `cats-core` published for Scala 2.13 lives at coordinate `org.typelevel:cats-core_2.13:2.10.0` — the `_2.13` is part of the artifactId, NOT a separate qualifier. The PURL emitted is therefore `pkg:maven/org.typelevel/cats-core_2.13@2.10.0` — the existing `pkg:maven/` PURL shape covers Scala artifacts without modification. Scala 3 artifacts use the suffix `_3` (not `_3.x`).

SBT's dependency declaration syntax distinguishes:

- **`%`** (single-percent): exact artifactId. `"org" % "artifact" % "1.0"` → coordinate `org:artifact:1.0`.
- **`%%`** (double-percent): Scala-version-suffixed. `"org" %% "artifact" % "1.0"` → coordinate `org:artifact_<scala>:1.0` where `<scala>` is `scalaVersion`'s major.minor (e.g., `_2.13`) for Scala 2 or just `_3` for Scala 3.
- **`%%%`** (triple-percent): Scala.js / Scala Native cross-platform suffix. Adds an additional platform suffix (`_sjs1`, `_native0.4`). Out of scope for v1; warn-and-skip.

This feature closes the Scala gap so an operator scanning any SBT-managed project gets a complete SBOM with every Scala dep represented, and the Scala-version suffix correctly encoded in the artifactId.

## Clarifications

### Session 2026-06-24

- Q: For `%%`-declared deps in design-tier mode, when `scalaVersion` is absent / unparseable, which fallback applies? → A: **Inference cascade with `_2.13` default**. Resolution order: (1) explicit `scalaVersion := "..."` in build.sbt → (2) infer from `project/build.properties` → (3) default to `_2.13` with `mikebom:scala-version-source = "default-fallback"` annotation. The earlier Edge Case wording about "no-suffix fallback + mikebom:scala-suffix-unknown" is REPLACED — that approach produced PURLs that don't exist on Maven Central, a worse failure mode than picking the dominant Scala-2.x version (>55% production share since 2023). Operator-visible signal preserved via the `mikebom:scala-version-source` annotation.
- Q: How should multi-project SBT builds be discovered — parse root `build.sbt` `lazy val` declarations, walk subdirs for `<subdir>/build.sbt`, or both? → A: **Union both surfaces**. Parse the root `build.sbt` for `lazy val <name> = project.in(file("<path>"))` blocks to enumerate operator-named subprojects with their declared paths, AND walk all subdirs under the scan root for `<subdir>/build.sbt` files (excluding `target/` / `project/` / `.git/`). Dedup the resulting set by canonicalized absolute directory path. Each unique subproject directory emits one main-module per FR-009. Matches the milestone-141 "discover what's there, don't assume layout" precedent; closes the gap that B (root-only) leaves for split-file conventions AND the gap that C (subdir-only) leaves for the dominant case where all subproject definitions live in the root `build.sbt`. When a `lazy val` declaration names a subproject AND a corresponding `<subdir>/build.sbt` exists, the `lazy val` name + path wins (preserves the operator's chosen subproject name).
- Q: For lockfile discovery — strict `build.sbt.lock`-only filename OR `*.sbt.lock` glob? → A: **Glob `*.sbt.lock` with content validation**. Walk all files ending in `.sbt.lock` under the scan root (subject to the standard excludes). At parse time, the reader requires the JSON contain a top-level `lockVersion` (integer) AND `modules` (array) key before treating the file as authoritative; non-matching files warn-and-skip per FR-007. The validation guard catches non-SBT-plugin files that happen to share the extension without admitting false-positive components. Closes the per-subproject lockfile variants + operator-rename gaps that strict `build.sbt.lock`-only matching would leave.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Operator scans an SBT-managed Scala project with `*.sbt.lock` (Priority: P1) 🎯 MVP

A Scala backend engineer runs `mikebom sbom scan --path .` on their SBT project source tree containing `build.sbt` + a `*.sbt.lock` JSON file (sbt 1.10+ with the sbt-dependency-lock plugin). They receive an SBOM containing one component per dep pinned in the lockfile. Each component carries a `pkg:maven/<groupId>/<artifactId>@<version>` PURL identity with the Scala-version suffix baked into the artifactId.

**Why this priority**: The headline use case. Production Scala projects using `.sbt.lock` are the highest-value scan target since the lockfile is fully self-contained and authoritative.

**Independent Test** (SC-001): Synthetic fixture with `build.sbt` declaring 3 direct deps + `build.sbt.lock` pinning those 3 plus 4 transitives (7 total). Run `mikebom sbom scan --path <tmp>`. Assert exactly 7 `pkg:maven/*` components emit with correct group/artifact/version per the lockfile's `modules[]` array.

**Acceptance Scenarios**:

1. **Given** a Scala project with `build.sbt.lock` pinning `cats-core_2.13 2.10.0`, `akka-actor_2.13 2.6.20`, `spark-core_2.13 3.5.0`, **When** the operator runs `mikebom sbom scan --path <project>`, **Then** the emitted SBOM contains components for each pinned dep with PURL `pkg:maven/<group>/<artifact>@<version>`.
2. **Given** the same project, **When** the operator inspects the emitted SBOM, **Then** transitive deps pinned in `.sbt.lock`'s `modules[]` array also appear as components — the lockfile is the authoritative dep set, not just `libraryDependencies` from `build.sbt`.
3. **Given** a source tree WITHOUT `build.sbt` or `*.sbt.lock` or `project/`, **When** the operator scans, **Then** no Scala components or annotations appear AND no warning fires (clean no-op).
4. **Given** a project whose `build.sbt` declares `name := "my-app"`, `version := "1.2.3"`, `organization := "com.example"`, `scalaVersion := "2.13.12"`, **When** the operator scans, **Then** a main-module component emits with PURL `pkg:maven/com.example/my-app_2.13@1.2.3` (or `pkg:maven/com.example/my-app@1.2.3` when `scalaVersion` is unparseable), `mikebom:component-role = "main-module"`, and `mikebom:sbom-tier = "source"` annotations.

---

### User Story 2 — Operator distinguishes Scala 2.x vs Scala 3 + `%` vs `%%` artifacts (Priority: P2)

The operator's Scala project mixes pure-Java deps (declared via `%`) with Scala-versioned deps (declared via `%%`) and pulls in both Scala 2.13 and Scala 3 cross-built artifacts. The SBOM must encode the Scala-version suffix correctly so downstream supply-chain risk tooling treats `cats-core_2.13` and `cats-core_3` as distinct components.

**Why this priority**: Scala-version mismatch is the #1 source of dep-resolution bugs in the Scala ecosystem. SBOMs that drop the suffix create false equivalences across Scala-2-vs-3 boundaries.

**Independent Test** (SC-002): Synthetic fixture with one pure-Java dep (`%`) + one Scala-2.13 dep (`%%` with `scalaVersion := "2.13.12"`) + one Scala-3 dep. Scan. Assert PURL shapes per FR-003.

**Acceptance Scenarios**:

1. **Given** a `build.sbt` declaring `"org.scala-lang" %% "scala-library" % "2.13.12"` with `scalaVersion := "2.13.12"`, **When** the operator scans the project (with sibling lockfile), **Then** the emitted PURL is `pkg:maven/org.scala-lang/scala-library_2.13@2.13.12` (Scala-major.minor suffix).
2. **Given** a `build.sbt` declaring `"org.typelevel" %% "cats-core" % "2.10.0"` with `scalaVersion := "3.3.1"`, **When** the operator scans, **Then** the emitted PURL is `pkg:maven/org.typelevel/cats-core_3@2.10.0` (Scala 3 uses bare `_3` suffix, not `_3.3`).
3. **Given** a `build.sbt` declaring `"org.postgresql" % "postgresql" % "42.7.0"` (single-percent — pure Java artifact), **When** the operator scans, **Then** the emitted PURL is `pkg:maven/org.postgresql/postgresql@42.7.0` (NO Scala suffix — `%` means literal artifactId).
4. **Given** the same project's lockfile contains both `cats-core_2.13` and `cats-core_3` entries (cross-built Scala library), **When** the operator scans, **Then** TWO distinct components emit (one per Scala-version variant); they do NOT collapse via dedup since the PURLs differ in artifactId.

---

### User Story 3 — Operator scans an SBT project WITHOUT a committed lockfile (Priority: P3)

Some Scala library projects do not use the sbt-dependency-lock plugin (it's optional and not yet universal in the Scala community circa 2026). Scanning such a project should produce SOME inventory rather than empty output, marked as `design`-tier. The reader regex-extracts the common `libraryDependencies` syntax from `build.sbt` and the convention `project/Dependencies.scala` sidecar.

**Why this priority**: Important for the Scala library ecosystem (many published libraries don't commit lockfiles). Best-effort regex extraction loses precision but recovers the dominant inventory.

**Independent Test** (SC-003): Synthetic fixture with `build.sbt` declaring 2 direct deps but NO `.sbt.lock`. Scan. Assert 2 components emit with `mikebom:sbom-tier = "design"` annotation.

**Acceptance Scenarios**:

1. **Given** a Scala library project with `build.sbt` only (no `.sbt.lock`), **When** the operator scans, **Then** components emit for declared deps from `libraryDependencies` blocks with `mikebom:sbom-tier = "design"` and the original version string preserved as evidence.
2. **Given** the same project, **When** the operator inspects the emitted SBOM, **Then** NO transitive deps appear (lockfile is required for transitive resolution).
3. **Given** a `build.sbt` declaring `"org.scalatest" %% "scalatest" % "3.2.18" % Test` (Test scope), **When** the operator scans in design-tier mode, **Then** the emitted `scalatest_2.13` component carries `mikebom:lifecycle-scope = "development"` annotation; downstream `--exclude-scope dev` filtering successfully suppresses it.

---

### Edge Cases

- **Mixed `%` / `%%` / `%%%` in one project**: a cross-built Scala project may use all three. v1 handles `%` and `%%`; `%%%` (Scala.js / Scala Native) warns-and-skips per Out-of-Scope.
- **Scala 3 suffix shape**: Scala 3 uses `_3` (just `_3`, no minor version) for ALL Scala-3.x releases. The PURL extractor MUST NOT append `_3.3` for `scalaVersion := "3.3.1"` — that would produce a coordinate that doesn't exist on Maven Central.
- **Scala 2 suffix is major.minor**: Scala 2 uses `_2.13`, `_2.12`, `_2.11` (NOT `_2.13.12`). Patch version is dropped.
- **Multi-project SBT builds**: A project with `lazy val core = project.in(file("core"))` + `lazy val server = project.in(file("server"))` has multiple subprojects, each with its own `libraryDependencies`. v1 emits one main-module per declared `project` (matches the milestone-140 umbrella + milestone-141 umbrella convention).
- **`build.sbt` Scala-DSL parsing is hard**: full Scala parsing requires a Scala compiler. v1 regex-extracts the documented `libraryDependencies += ...` and `libraryDependencies ++= Seq(...)` forms; complex programmatic dep generation (e.g., `libraryDependencies := computeDeps(env)`) is not statically resolvable and silently drops those entries per the best-effort convention (matches the milestone-140 Q1 conditional-flattened precedent).
- **`.sbt.lock` JSON schema variants**: the sbt-dependency-lock plugin's schema has evolved across versions; v1 targets schema versions 1 + 2 (both in production circa 2026). Per Q3 content validation, the reader requires top-level `lockVersion` + `modules` keys before treating a `*.sbt.lock`-glob match as authoritative. Unknown `lockVersion` values (≥3 or 0) warn-and-skip per FR-007; non-matching files (no `lockVersion` / `modules` keys) also warn-and-skip without polluting the SBOM.
- **`project/Dependencies.scala`**: Scala source containing dep declarations as `val` / `def` definitions. v1 regex-extracts the common `val foo = "group" %% "artifact" % "version"` pattern; complex computed deps are silently dropped.
- **Configuration suffix vs scope**: `% Test`, `% Provided`, `% Runtime`, `% Compile` (default) are configuration scopes. Map `Test` → `mikebom:lifecycle-scope = "development"` per FR-008. `Provided` deps are not bundled but required at compile-time — emit with default scope.
- **Evicted versions**: SBT resolves version conflicts by picking ONE version per coordinate. The `.sbt.lock` contains only the resolved version; evicted versions do NOT appear. v1 emits only what the lockfile contains.
- **Cross-built libraries**: `cats-core_2.13` + `cats-core_3` are DISTINCT components per FR-003 (different artifactId → different PURL → no dedup collapse). This matches Maven Central reality.
- **Bare `scalaVersion := "..."` typo or absent**: per Q1 clarification, the reader applies the inference cascade (build.sbt → `project/build.properties` → default `_2.13`). When the cascade reaches the default-fallback rung, `%%` deps emit with the `_2.13` suffix and carry `mikebom:scala-version-source = "default-fallback"` as transparency evidence so operators see the heuristic kicked in. The Maven Central coordinate produced is real, not a synthetic no-suffix placeholder.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST detect SBT-managed Scala projects by the presence of `build.sbt`, `*.sbt.lock`, OR a `project/` directory containing `build.properties` or `Dependencies.scala`. Any of these triggers reader activation.
- **FR-002**: System MUST discover lockfile candidates via the `*.sbt.lock` glob per Q3 (any file ending in `.sbt.lock` under the scan root, subject to the standard `target/` / `project/` / `.git/` excludes). Per Q3 content-shape validation, the reader MUST require each candidate's parsed JSON to contain a top-level `lockVersion` (integer) AND `modules` (array) key before treating it as an authoritative sbt-dependency-lock plugin lockfile. Files that pass validation are parsed for schema versions 1 + 2; for each entry, extract `org` (groupId), `name` (artifactId — already includes any Scala-version suffix), `version`, optional `configurations` arrays, and optional `checksums` arrays. The `name` field is authoritative — do NOT re-append Scala suffixes when the lockfile is present (the plugin has already resolved them). Files that fail validation warn-and-skip per FR-007.
- **FR-003**: System MUST emit one component per parsed lockfile entry with PURL `pkg:maven/<group>/<artifact>@<version>`. Cross-built libraries that appear as multiple `name`-distinct entries (e.g., `cats-core_2.13` + `cats-core_3`) emit as separate components per Acceptance Scenario 2.4.
- **FR-004**: System MUST emit dependency edges from each project's main-module (per FR-012) to each direct dep declared in `build.sbt`'s `libraryDependencies` block. Transitive components (lockfile entries not in the manifest's deps list) surface as standalone components but inter-package dependency edges are deferred to v1.1.
- **FR-005**: When `*.sbt.lock` is absent but `build.sbt` is present, system MUST emit components for direct deps declared in `libraryDependencies` blocks via regex extraction. For `%%` declarations, append the Scala-version suffix per the Q1 inference cascade: (1) explicit `scalaVersion := "..."` in build.sbt → (2) `project/build.properties`'s `sbt.version` and any embedded `scala.version` keys → (3) default `_2.13` with `mikebom:scala-version-source = "default-fallback"` annotation. Scala 2.x suffix uses major.minor (`_2.13`); Scala 3.x uses bare `_3` (no patch). Each design-tier component carries `mikebom:sbom-tier = "design"` annotation and the original version string as `mikebom:requirement-range` evidence.
- **FR-006**: System MUST treat a source tree containing none of `build.sbt` / `*.sbt.lock` / `project/build.properties` / `project/Dependencies.scala` as a clean no-op — no components emitted, no warnings logged.
- **FR-007**: System MUST tolerate per-file parse errors without aborting the whole scan — log a structured warning naming the affected file path and continue. When `*.sbt.lock` JSON is malformed AND a sibling `build.sbt` exists, fall back to design-tier emission per FR-005.
- **FR-008**: System MUST tag deps declared with the `Test` configuration in `build.sbt` (`"org" %% "artifact" % "version" % Test`) with `mikebom:lifecycle-scope = "development"` (matches the milestone-052 / 137-141 convention). `Provided`, `Runtime`, `Compile` (default) map to runtime-scope.
- **FR-009**: System MUST handle SBT multi-project builds via the Q2 union-discovery strategy: (a) parse the root `build.sbt` for every `lazy val <name> = project.in(file("<path>"))` block to enumerate operator-named subprojects with their declared paths, AND (b) walk all subdirs under the scan root for `<subdir>/build.sbt` files (excluding `target/`, `project/`, `.git/`, `node_modules/`). Dedup the resulting set of subproject directories by canonicalized absolute path. When a `lazy val` declaration names a subproject AND a corresponding `<subdir>/build.sbt` exists, the `lazy val` name + path wins (preserves the operator's chosen subproject identity). Emit one **main-module component per surfaced subproject**, plus the root project itself (the implicit "root project" surfaces from the root `build.sbt`). Each subproject's `libraryDependencies` block contributes its own dep declarations to its main-module's `depends` set. Same-PURL deps across subprojects collapse via standard `seen_purls` dedup.
- **FR-010**: System MUST NOT make any network calls during the scan — the lockfile is fully self-contained; the design-tier path uses on-disk `build.sbt` only.
- **FR-011**: System MUST preserve content-addressable hashes when present in the lockfile (the sbt-dependency-lock plugin's schema-v2 includes per-module SHA-256 hashes in the `checksums` field). Hashes flow into `PackageDbEntry.hashes` as `ContentHash::with_algorithm(HashAlgorithm::Sha256, hex)` entries. Per the milestone-141 best-effort posture, only hashes that are present + non-empty emit.
- **FR-012**: For each `build.sbt` file at the project root (and each `<subproject>/build.sbt` for multi-project builds), system MUST emit one **main-module component** with PURL `pkg:maven/<organization>/<name><scala-suffix>@<version>` (where `<organization>`, `<name>`, `<version>` come from the `organization`/`name`/`version` SBT settings, and `<scala-suffix>` is appended if `scalaVersion` parses cleanly — matching the `%%` semantics). The component MUST carry `mikebom:component-role = "main-module"` and `mikebom:sbom-tier = "source"` annotations. When any of `organization`/`name`/`version` is unparseable, fall back to: `organization → "unknown"`, `name → <parent-dir-basename>`, `version → "0.0.0-unknown"` (per the milestone-141 cascade pattern).

### Key Entities

- **`build.sbt`**: Scala-DSL build definition. Top-level settings include `name := "..."`, `version := "..."`, `organization := "..."`, `scalaVersion := "..."`, and `libraryDependencies += ...` / `libraryDependencies ++= Seq(...)`. May also declare multiple subprojects via `lazy val <name> = project.in(file("<path>"))` blocks.
- **`*.sbt.lock`**: JSON lockfile produced by the sbt-dependency-lock plugin. Top-level shape: `{"lockVersion": 1|2, "timestamp": "...", "configurations": ["compile", "test", ...], "modules": [{...}]}`. Each `modules[]` entry: `{"org": "...", "name": "...", "version": "...", "configurations": [...], "checksums": [{"name": "<group>:<artifact>:<version>.jar", "type": "SHA-256", "checksum": "..."}]}`.
- **`project/Dependencies.scala`**: Convention sidecar — a Scala source file declaring `val` / `def` named dep references (e.g., `val cats = "org.typelevel" %% "cats-core" % "2.10.0"`) so `build.sbt` can reference them by name. v1 regex-extracts the common pattern; computed deps are dropped.
- **`project/build.properties`**: Pins the SBT version (`sbt.version=1.10.0`). Used only as a project-detection signal per FR-001.
- **SBT configuration**: A named build context (`Compile` / `Test` / `Provided` / `Runtime` / `IntegrationTest`). Deps declared with `% Test` (etc.) suffix are scope-tagged per FR-008.
- **Scala-version suffix**: Appended to artifactId for `%%`-declared deps. Scala 2.x: major.minor (`_2.13`). Scala 3.x: bare `_3`. Pure-Java deps (declared with `%`) have NO suffix.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A scan of a synthetic SBT project with `build.sbt` (3 direct deps) + `build.sbt.lock` (those 3 plus 4 transitives = 7 total) produces a CDX SBOM whose Scala component count matches the lockfile entry count exactly (7) plus 1 main-module (= 8 total Scala-derived components); direct-dep edges target real bom-refs.
- **SC-002**: A scan of a fixture mixing one `%`-declared pure-Java dep + one `%%`-declared Scala-2.13 dep + one `%%`-declared Scala-3 dep produces correct PURLs per FR-003: pure-Java emits without suffix; Scala-2.13 emits with `_2.13` suffix; Scala-3 emits with bare `_3` suffix.
- **SC-003**: A scan of a Scala library project with `build.sbt` only (no `.sbt.lock`) produces components for declared `libraryDependencies` entries with `mikebom:sbom-tier = "design"` annotation; `%%` deps correctly carry the Scala-version suffix derived from `scalaVersion`.
- **SC-004**: A source tree containing no Scala files produces an SBOM byte-identical (modulo timestamps + serial numbers) to a pre-feature baseline scan. (No-op preservation invariant.)
- **SC-005**: A scan completes successfully (exit code 0, valid SBOM) on a fixture where one `*.sbt.lock` has corrupted JSON syntax alongside three valid Scala project subdirectories. The output contains components from the three valid projects plus a warning naming the corrupted lockfile; the corrupted project falls back to design-tier emission from its sibling `build.sbt`.
- **SC-006**: An external SBOM consumer reading the emitted CDX JSON can enumerate every Scala-derived component via the standard `components[]` array filtered on `purl =~ "^pkg:maven/"`. No Scala-specific consumer code is required (Scala lives natively on Maven Central).
- **SC-007**: A scan of a fixture with a `build.sbt` declaring `"org.scalatest" %% "scalatest" % "3.2.18" % Test` produces a component carrying `mikebom:lifecycle-scope = "development"` annotation; downstream `--exclude-scope dev` filtering successfully suppresses the component.
- **SC-008**: A scan of a project whose `build.sbt` declares `name := "my_app"`, `version := "1.2.3"`, `organization := "com.example"`, `scalaVersion := "2.13.12"` produces a main-module component with PURL `pkg:maven/com.example/my_app_2.13@1.2.3` carrying `mikebom:component-role = "main-module"` annotation.
- **SC-009**: A scan of a multi-project SBT build with 3 subprojects (each declared via `lazy val <subproj> = project.in(file("<path>"))` + per-subproject `libraryDependencies`) produces **4 main-module components — 3 per-subproject + 1 root project** per FR-009's union-discovery semantics (Q2 Phase C emits the implicit root in addition to the surfaced subprojects). Same-PURL deps across subprojects collapse to single component entries via standard dedup.
- **SC-010**: A scan of a fixture with a cross-built dep (`cats-core_2.13` + `cats-core_3` both present in the lockfile) produces TWO distinct components (one per Scala-version variant) that do NOT collapse via dedup — the PURLs differ in artifactId per the `%%` convention.

## Assumptions

- **SBT 1.10+ for `.sbt.lock` consumption**: pre-1.10 SBT versions predate the sbt-dependency-lock plugin's modern schema. Older lockfiles (schema v0 or pre-plugin formats) warn-and-skip.
- **`.sbt.lock` is the authoritative source when present**: prefer lockfile over `build.sbt` (design-tier fallback). The lockfile's `name` field is authoritative — do NOT re-append Scala suffixes (the plugin has already resolved them).
- **No live `sbt` invocation**: the reader parses on-disk metadata directly. It does NOT shell out to `sbt dependencyTree` or `sbt update` — the `sbt` binary (JVM-based, slow to start) isn't guaranteed to exist on the scan host and would defeat the offline-mode constraint.
- **Maven Central is the registry for all Scala artifacts**: SBT publishes to Maven Central (with `_2.13`/`_3` suffix in the artifactId). The `pkg:maven/` PURL type covers everything. No new PURL type is introduced.
- **Existing milestone-070 maven reader's PURL conventions apply verbatim**: groupId in PURL `namespace` slot, artifactId in `name` slot, version in `version` slot. Scala-version-suffixed artifactIds are literal artifactIds for PURL purposes — they go in the `name` slot exactly as resolved.
- **Regex parsing of `build.sbt`**: per the milestone-140 + milestone-141 precedent, Scala-DSL is regex-extractable for the dominant `libraryDependencies` syntax. Multi-line `Seq(...)` blocks handled via paren-counting (analogous to milestone-141's brace-counted tokenizer).
- **`build.sbt` is Scala source code, not a data format**: same posture as `mix.exs` (Elixir DSL parsed by regex in milestone 140) and `rebar.config` (Erlang source in milestone 141). Extract `libraryDependencies` blocks via regex; no Scala runtime evaluation.
- **`mikebom:source-type` value set**: uses the `scala-` prefix (`scala-sbt-lock` / `scala-sbt-design` / `scala-main-module`) per the milestone-122 / 137-141 prefixed convention. Distinguishes Scala-derived components from milestone-070's `maven-pom`-prefixed values even though both emit `pkg:maven/<group>/<artifact>@<version>` PURLs.
- **Scala-version inference order**: (a) explicit `scalaVersion := "..."` setting in `build.sbt`, (b) inferred from `project/build.properties` if absent, (c) default fallback `2.13` (the most-deployed Scala version circa 2026) when both are absent — emit `mikebom:scala-version-source = "default-fallback"` annotation to signal the heuristic.

## Out of Scope

- **Live invocation of `sbt` or any JVM toolchain binary**: read-only metadata parse only. JVM startup time is hostile to mikebom's sub-second scan budget.
- **`%%%` triple-percent (Scala.js / Scala Native cross-platform)**: warn-and-skip. Cross-compilation suffixes (`_sjs1`, `_native0.4`) are rare in production; deferred to v1.1.
- **`build.sbt` programmatic dep generation**: `libraryDependencies := computeDeps(env)` and similar non-statically-resolvable patterns silently drop those entries per the best-effort regex-extraction convention.
- **Pre-sbt-dependency-lock schemas**: older lockfile formats (some legacy SBT plugins produced JSON in idiosyncratic shapes). Out of scope; warn-and-skip.
- **Mill / CBT / Bloop build tools**: other Scala build tools have their own lockfile formats; deferred to separate milestones.
- **`project/plugins.sbt`**: declares SBT plugins (e.g., `addSbtPlugin("..." % "..." % "...")`). Plugins are build-time-only deps, not project deps; out of scope for the project SBOM.
- **Cross-version dep matrices** (`crossScalaVersions := Seq("2.12.18", "2.13.12", "3.3.1")`): a project that publishes cross-built artifacts for multiple Scala versions. v1 emits one main-module per the primary `scalaVersion`; cross-published variants are not separately surfaced.
- **License extraction**: deferred (matches the milestone-070 maven + milestone-141 erlang Out-of-Scope precedent — licenses live in per-package Maven POM `licenses` blocks, which require resolved-jar enrichment).
- **Per-package transitive dep edges from lockfile**: v1 emits standalone components; inter-package edges deferred to v1.1.

## Dependencies and Constraints

- **Builds on milestone 002** (initial language-reader architecture).
- **Builds on milestone 070** (Maven reader — PURL `pkg:maven/<group>/<artifact>@<version>` shape, the canonical Maven Central identity model).
- **Builds on milestone 106 + 122** (Gradle + Kotlin DSL — adjacent JVM-build-tool readers).
- **Builds on milestone 140** (Elixir/Mix — regex-extracted DSL parsing + brace-counted tokenizer pattern; closest in shape to `build.sbt` Scala-DSL extraction).
- **Builds on milestone 141** (Erlang/OTP — multi-tier emission with main-module + design-tier-fallback + profile-scope tagging + umbrella support).
- **Reuses the existing source-tree walker** (`scan_fs::walk::safe_walk`).
- **Does NOT touch existing language readers** — Scala support is strictly additive.
- **Does NOT introduce new external dependencies** — `regex` + `serde_json` are workspace deps.

## Related

- Closes: #426 (Add Scala/SBT ecosystem support (build.sbt + *.sbt.lock))
- Adjacent: milestone 070 (Maven), milestone 106 + 122 (Gradle + Kotlin DSL), milestone 140 (Elixir/Mix — DSL parsing precedent), milestone 141 (Erlang/OTP — multi-tier emission + umbrella precedent)
- Foundational reference: `mikebom-cli/src/scan_fs/package_db/maven.rs` (PURL shape source of truth for `pkg:maven/`), `mikebom-cli/src/scan_fs/package_db/elixir.rs` (DSL regex extraction template), `mikebom-cli/src/scan_fs/package_db/erlang.rs` (lockfile + design-tier dual-mode template)
