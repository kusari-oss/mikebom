# Contract — Scala/SBT component PURL shapes

Authoritative output-shape contract for components emitted by the milestone-142 Scala/SBT reader. Test fixtures in `mikebom-cli/tests/scala_*.rs` MUST assert exact PURL strings matching the shapes below.

## 1. Hex-style Scala-2.x dep from `*.sbt.lock`

**Input** (`build.sbt.lock`):
```json
{
  "lockVersion": 1,
  "modules": [
    {"org": "org.typelevel", "name": "cats-core_2.13", "version": "2.10.0",
     "configurations": ["compile"]}
  ]
}
```

**Output**:
- `purl`: `pkg:maven/org.typelevel/cats-core_2.13@2.10.0`
- `name`: `cats-core_2.13` (verbatim from lockfile; suffix is part of artifactId)
- `version`: `2.10.0`
- `hashes`: `[]`
- `properties[]`:
  - `mikebom:source-type = "scala-sbt-lock"`
  - `mikebom:evidence-kind = "sbt-lock"`

## 2. Scala-3 dep from `*.sbt.lock`

**Input**:
```json
{"modules": [
    {"org": "org.typelevel", "name": "cats-core_3", "version": "2.10.0",
     "configurations": ["compile"]}
]}
```

**Output**:
- `purl`: `pkg:maven/org.typelevel/cats-core_3@2.10.0`
  (NOT `cats-core_3.3` — Maven Central uses bare `_3` for all Scala-3.x cross-builds)
- `name`: `cats-core_3`

## 3. Pure-Java dep from `*.sbt.lock`

**Input**:
```json
{"modules": [
    {"org": "org.postgresql", "name": "postgresql", "version": "42.7.0",
     "configurations": ["compile"]}
]}
```

**Output**:
- `purl`: `pkg:maven/org.postgresql/postgresql@42.7.0` (NO Scala suffix)
- `name`: `postgresql`

## 4. Schema-v2 dep with SHA-256 checksum

**Input**:
```json
{
  "lockVersion": 2,
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

**Output**:
- `purl`: `pkg:maven/org.typelevel/cats-core_2.13@2.10.0`
- `hashes`: `[{"alg": "SHA-256", "content": "abc123..."}]`
- (other fields per §1)

## 5. Cross-built library — distinct components, no dedup

**Input** (one lockfile, two `name`-distinct entries for the same logical library):
```json
{"modules": [
    {"org": "org.typelevel", "name": "cats-core_2.13", "version": "2.10.0", "configurations": ["compile"]},
    {"org": "org.typelevel", "name": "cats-core_3",    "version": "2.10.0", "configurations": ["compile"]}
]}
```

**Output**: TWO distinct components:
- `pkg:maven/org.typelevel/cats-core_2.13@2.10.0`
- `pkg:maven/org.typelevel/cats-core_3@2.10.0`

(They do NOT collapse — the PURLs differ in `name` segment per FR-003. Matches Maven Central reality: distinct artifacts with distinct POMs, jars, and SHAs.)

## 6. Test-configuration dep → development scope

**Input**:
```json
{"modules": [
    {"org": "org.scalatest", "name": "scalatest_2.13", "version": "3.2.18",
     "configurations": ["test"]}
]}
```

**Output**:
- `purl`: `pkg:maven/org.scalatest/scalatest_2.13@3.2.18`
- CDX native field `scope`: `"excluded"` (per milestone-052 lifecycle-scope-as-native-field bridge for Development scope)
- (other fields per §1)

## 7. Design-tier dep from `build.sbt` only (Q1 cascade, scalaVersion declared)

**Input** (`build.sbt` only, no `.sbt.lock`):
```scala
scalaVersion := "2.13.12"
libraryDependencies ++= Seq(
  "org.typelevel" %% "cats-core" % "2.10.0",
  "org.postgresql" % "postgresql" % "42.7.0"
)
```

**Output**:

`cats-core` (Scala-suffixed via Q1 cascade rung 1 — explicit `scalaVersion`):
- `purl`: `pkg:maven/org.typelevel/cats-core_2.13@2.10.0`
- `name`: `cats-core` (BARE — Q1 cascade applies to PURL slot only)
- `properties[]`:
  - `mikebom:source-type = "scala-sbt-design"`
  - `mikebom:evidence-kind = "sbt-build"`
  - `mikebom:sbom-tier = "design"`
  - `mikebom:requirement-range = "2.10.0"`
  - `mikebom:scala-version-source = "build-sbt-explicit"`

`postgresql` (no suffix — `%` declaration):
- `purl`: `pkg:maven/org.postgresql/postgresql@42.7.0`
- `properties[]`:
  - `mikebom:source-type = "scala-sbt-design"`
  - `mikebom:evidence-kind = "sbt-build"`
  - `mikebom:sbom-tier = "design"`
  - (NO `mikebom:scala-version-source` annotation — only emitted on `%%` deps)

## 8. Design-tier dep with Q1 default fallback (scalaVersion missing)

**Input** (`build.sbt` only, scalaVersion ABSENT):
```scala
libraryDependencies += "org.typelevel" %% "cats-core" % "2.10.0"
```

**Output**:
- `purl`: `pkg:maven/org.typelevel/cats-core_2.13@2.10.0`
  (Default `_2.13` applied per Q1 cascade rung 3)
- `properties[]`:
  - `mikebom:scala-version-source = "default-fallback"`
  - (plus standard design-tier annotations per §7)

## 9. Main-module from `build.sbt` (Scala 2.x project)

**Input**:
```scala
name := "my-app"
version := "1.2.3"
organization := "com.example"
scalaVersion := "2.13.12"
```

**Output (main-module)**:
- `purl`: `pkg:maven/com.example/my-app_2.13@1.2.3`
- `name`: `my-app` (bare — suffix applied to PURL slot only)
- `version`: `1.2.3`
- `properties[]`:
  - `mikebom:component-role = "main-module"`
  - `mikebom:source-type = "scala-main-module"`
  - `mikebom:scala-version-source = "build-sbt-explicit"` (per F6 — Q1 cascade rung 1 hit)

## 10. Main-module from `build.sbt` (Scala 3 project)

**Input**:
```scala
name := "my-app"
version := "1.2.3"
organization := "com.example"
scalaVersion := "3.3.1"
```

**Output (main-module)**:
- `purl`: `pkg:maven/com.example/my-app_3@1.2.3`
  (Bare `_3` — Scala 3 patch is dropped)
- `properties[]` (additionally):
  - `mikebom:scala-version-source = "build-sbt-explicit"` (per F6)

## 11. Main-module with cascade fallbacks (all settings missing)

**Input** (`build.sbt` at `/tmp/orphaned_app/build.sbt`):
```scala
libraryDependencies += "org.typelevel" %% "cats-core" % "2.10.0"
```

**Output (main-module)**:
- `purl`: `pkg:maven/unknown/orphaned_app_2.13@0.0.0-unknown`
  (`organization → "unknown"`, `name → dir basename "orphaned_app"`, `version → "0.0.0-unknown"`, scalaVersion → Q1 default `_2.13`)
- `properties[]` (additionally):
  - `mikebom:scala-version-source = "default-fallback"` (per F6 — Q1 cascade rung 3 hit)

## 12. Multi-project main-modules (Q2 union discovery)

**Input** (`build.sbt` at project root):
```scala
ThisBuild / organization := "com.example"
ThisBuild / version := "1.0.0"
ThisBuild / scalaVersion := "2.13.12"

lazy val core = project.in(file("core"))
  .settings(libraryDependencies += "org.typelevel" %% "cats-core" % "2.10.0")

lazy val server = project.in(file("server"))
  .dependsOn(core)
  .settings(libraryDependencies += "com.typesafe.akka" %% "akka-actor" % "2.6.20")
```

**Output**: THREE main-modules:
- `pkg:maven/com.example/<root-name>_2.13@1.0.0` (the root project — typically named after the repo dir)
- `pkg:maven/com.example/core_2.13@1.0.0`
- `pkg:maven/com.example/server_2.13@1.0.0`

(All three carry `mikebom:component-role = "main-module"` + `mikebom:source-type = "scala-main-module"`.)

## 13. Cross-format byte-equivalence

For every emission in §1–§12, the same `purl` value MUST appear in:
- CycloneDX 1.6 output's `components[].purl`
- SPDX 2.3 output's `packages[].externalRefs[].referenceLocator` (where `referenceType == "purl"`)
- SPDX 3.0.1 output's `@graph[].software_packageUrl`

Per the milestone-013 format-parity-enforcement work, the `parity-check` subcommand verifies this invariant for milestone 142 test fixtures.

## 14. SBOM-format property name mapping

| Field | CycloneDX 1.6 | SPDX 2.3 | SPDX 3.0.1 |
|---|---|---|---|
| `mikebom:source-type` | `properties[].name = "mikebom:source-type"` | `annotations[]` with comment envelope | document-scope `Annotation` with envelope |
| `mikebom:evidence-kind` | `properties[]` | `annotations[]` | `Annotation` |
| `mikebom:sbom-tier` | `properties[]` | `annotations[]` | `Annotation` |
| `mikebom:requirement-range` | `properties[]` | `annotations[]` | `Annotation` |
| `mikebom:lifecycle-scope` | NATIVE: `components[].scope` | NATIVE: `relationships[].relationshipType = "DEV_DEPENDENCY_OF"` etc. | NATIVE: `LifecycleScopeType` |
| `mikebom:component-role` | `properties[]` | `annotations[]` | `Annotation` |
| `mikebom:scala-version-source` | `properties[]` | `annotations[]` | `Annotation` |

Per Constitution Principle V, `mikebom:lifecycle-scope` flows through the milestone-052 native-field path (CDX `scope` / SPDX 2.3 `DEV_DEPENDENCY_OF` / SPDX 3 `LifecycleScopeType`). The other `mikebom:*` properties remain as standalone annotations because no spec-native carrier exists for the semantic per research §R3.
