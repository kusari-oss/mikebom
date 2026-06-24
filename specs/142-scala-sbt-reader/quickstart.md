# Quickstart — milestone 142 Scala/SBT reader

Operator-facing walkthrough of the scenarios this milestone surfaces.

## Scenario 1 — Scan an SBT project with a committed lockfile (US1 / SC-001)

```bash
mikebom --offline sbom scan --path . --output /tmp/app.cdx.json
```

```bash
# Main-module (when single-project layout):
jq '.metadata.component' /tmp/app.cdx.json
# {"bom-ref": "pkg:maven/com.example/my-app_2.13@1.2.3",
#  "name": "my-app", "version": "1.2.3",
#  "purl": "pkg:maven/com.example/my-app_2.13@1.2.3",
#  "properties": [
#    {"name": "mikebom:component-role", "value": "main-module"}
#  ]}

# Lockfile-derived deps:
jq '.components[] | select(.purl | startswith("pkg:maven/")) | .purl' /tmp/app.cdx.json | sort | head -10
# "pkg:maven/com.typesafe.akka/akka-actor_2.13@2.6.20"
# "pkg:maven/org.scala-lang/scala-library@2.13.12"
# "pkg:maven/org.typelevel/cats-core_2.13@2.10.0"
# ...
```

## Scenario 2 — Scala 2 vs Scala 3 vs pure-Java distinction (US2 / SC-002)

```bash
mikebom --offline sbom scan --path . --output /tmp/mixed.cdx.json

# Scala 2.13:
jq '.components[] | select(.purl | endswith("_2.13@2.10.0")) | .purl' /tmp/mixed.cdx.json
# "pkg:maven/org.typelevel/cats-core_2.13@2.10.0"

# Scala 3 (bare _3, NOT _3.3):
jq '.components[] | select(.purl | contains("_3@")) | .purl' /tmp/mixed.cdx.json
# "pkg:maven/org.typelevel/cats-core_3@2.10.0"

# Pure-Java (no suffix):
jq '.components[] | select(.purl == "pkg:maven/org.postgresql/postgresql@42.7.0") | .purl' /tmp/mixed.cdx.json
# "pkg:maven/org.postgresql/postgresql@42.7.0"
```

## Scenario 3 — Cross-built library emits TWO distinct components (SC-010)

```bash
# A lockfile containing both cats-core_2.13 AND cats-core_3:
jq '[.components[] | select(.name | startswith("cats-core"))] | length' /tmp/cross.cdx.json
# 2

jq '.components[] | select(.name | startswith("cats-core")) | .purl' /tmp/cross.cdx.json
# "pkg:maven/org.typelevel/cats-core_2.13@2.10.0"
# "pkg:maven/org.typelevel/cats-core_3@2.10.0"
# (They do NOT collapse — different artifactId → different PURL.)
```

## Scenario 4 — Inner SHA-256 hash emission from schema-v2 lockfile (FR-011)

```bash
jq '.components[] | select(.name == "cats-core_2.13") | .hashes' /tmp/app.cdx.json
# [
#   {"alg": "SHA-256", "content": "<inner_sha256_from_checksums_array>"}
# ]
```

Schema-v1 lockfiles (older sbt-dependency-lock releases) lack the `checksums` array; the reader emits ZERO hash entries for those per the FR-011 best-effort posture.

## Scenario 5 — Design-tier fallback when no lockfile (US3 / SC-003)

`build.sbt` only, no `*.sbt.lock`:

```bash
mikebom --offline sbom scan --path /tmp/scala-lib --output /tmp/lib.cdx.json

jq '.components[] | {name, purl, props: [.properties[] | {name, value}]}' /tmp/lib.cdx.json
# {
#   "name": "cats-core",
#   "purl": "pkg:maven/org.typelevel/cats-core_2.13@2.10.0",
#   "props": [
#     {"name": "mikebom:sbom-tier", "value": "design"},
#     {"name": "mikebom:requirement-range", "value": "2.10.0"},
#     {"name": "mikebom:evidence-kind", "value": "sbt-build"},
#     {"name": "mikebom:source-type", "value": "scala-sbt-design"},
#     {"name": "mikebom:scala-version-source", "value": "build-sbt-explicit"}
#   ]
# }
```

## Scenario 6 — Q1 default-fallback annotation (scalaVersion absent)

`build.sbt` without `scalaVersion`:

```bash
jq '.components[] | select(.name == "cats-core") | .properties' /tmp/no-version.cdx.json
# [
#   ...,
#   {"name": "mikebom:scala-version-source", "value": "default-fallback"}
# ]

jq '.components[] | select(.name == "cats-core") | .purl' /tmp/no-version.cdx.json
# "pkg:maven/org.typelevel/cats-core_2.13@2.10.0"
# (Default _2.13 applied per Q1 cascade rung 3.)
```

## Scenario 7 — Multi-project SBT build (SC-009 + Q2 union discovery)

```bash
ls /tmp/multi-project/
# build.sbt
# core/
# server/
# worker/

mikebom --offline sbom scan --path /tmp/multi-project --output /tmp/m.cdx.json

# Main-modules per subproject + root = 4 total:
jq '.components[] | select(.properties[]? | .name == "mikebom:component-role" and .value == "main-module") | .purl' /tmp/m.cdx.json
# "pkg:maven/com.example/multi-project_2.13@1.0.0"  ← root
# "pkg:maven/com.example/core_2.13@1.0.0"
# "pkg:maven/com.example/server_2.13@1.0.0"
# "pkg:maven/com.example/worker_2.13@1.0.0"

# Same-PURL deps across subprojects collapse to single entries:
jq '[.components[] | select(.name == "cats-core_2.13")] | length' /tmp/m.cdx.json
# 1
```

## Scenario 8 — Test-configuration → dev-scope filtering (SC-007)

```bash
# Default scan includes test deps:
jq '.components[] | select(.name == "scalatest_2.13") | .scope' /tmp/app.cdx.json
# "excluded"   (CDX native scope=excluded per milestone-052 dev-scope bridge)

# --exclude-scope dev (TOP-LEVEL flag BEFORE sbom subcommand) suppresses:
mikebom --offline --exclude-scope dev sbom scan --path . --output /tmp/prod.cdx.json
jq '.components[] | select(.name == "scalatest_2.13")' /tmp/prod.cdx.json
# (empty — scalatest_2.13 suppressed)
```

## Scenario 9 — Q3 content-shape validation (malformed-lockfile skip)

```bash
# /tmp/scala-with-broken-lock contains build.sbt + build.sbt.lock (corrupt JSON):
mikebom --offline sbom scan --path /tmp/scala-with-broken-lock --output /tmp/sbl.cdx.json 2>/tmp/scan.log

echo $?
# 0  (scan succeeded; design-tier fallback)

grep 'sbt.lock' /tmp/scan.log
# WARN ... scala: failed to parse *.sbt.lock; falling back to design-tier from build.sbt path=...
```

## Scenario 10 — No-op on non-Scala rootfs (SC-004)

```bash
mikebom --offline sbom scan --path /mnt/server-rootfs --output /tmp/server.cdx.json 2>/tmp/scan.log

jq '[.components[] | select(.properties[]? | .name == "mikebom:source-type" and (.value | startswith("scala-")))] | length' /tmp/server.cdx.json
# 0

grep -c 'scala\|sbt\.lock\|build\.sbt' /tmp/scan.log
# 0
```

## Verification commands

```bash
cargo test -p mikebom --test scala_sbt_baseline          # SC-001 baseline + FR-011 hash emission
cargo test -p mikebom --test scala_source_discriminators # SC-002 % vs %% vs %%% + SC-010 cross-built
cargo test -p mikebom --test scala_tier_fallbacks        # SC-003 design-tier + SC-007 dev-scope + SC-009 multi-project + Q1 cascade
cargo test -p mikebom --test scala_edge_cases            # SC-004 no-op + SC-005 malformed-lockfile + Q3 content-shape + main-module fallbacks
cargo test -p mikebom --test cdx_regression --test spdx_regression --test spdx3_regression  # SC-004 byte-identity preservation
```

## Cross-format byte-equivalence check

```bash
mikebom --offline sbom scan --path my_scala_app \
  --format cyclonedx-json --format spdx-2.3-json --format spdx-3-json \
  --output cyclonedx-json=/tmp/s.cdx.json \
  --output spdx-2.3-json=/tmp/s.spdx.json \
  --output spdx-3-json=/tmp/s.spdx3.json

jq '[.components[] | select(.purl | startswith("pkg:maven/")) | .purl] | sort' /tmp/s.cdx.json > /tmp/cdx-mvn.txt
jq '[.packages[].externalRefs[]? | select(.referenceType == "purl") | .referenceLocator | select(startswith("pkg:maven/"))] | sort' /tmp/s.spdx.json > /tmp/spdx-mvn.txt
jq '[.["@graph"][] | select(.software_packageUrl? | tostring | startswith("pkg:maven/")) | .software_packageUrl] | sort' /tmp/s.spdx3.json > /tmp/spdx3-mvn.txt

diff /tmp/cdx-mvn.txt /tmp/spdx-mvn.txt
diff /tmp/cdx-mvn.txt /tmp/spdx3-mvn.txt
# (no output = success)
```

## Known deferrals (spec Out-of-Scope)

- License extraction (deferred — Maven POM `licenses` blocks require resolved-jar enrichment).
- Per-package transitive dep edges from lockfile (v1.1).
- `%%%` triple-percent (Scala.js / Scala Native cross-platform) — warn-and-skip.
- `crossScalaVersions` matrix expansion (cross-published variants) — v1 emits primary `scalaVersion` only.
- `project/plugins.sbt` (build-time SBT plugins, not project deps).
- Mill / CBT / Bloop build tools — separate milestones.
- Pre-sbt-dependency-lock lockfile formats.
- Live `sbt` invocation.
