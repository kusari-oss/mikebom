# Quickstart — milestone 137 Dart/Flutter pub reader

Operator-facing walkthrough of the scenarios this milestone surfaces.

## Scenario 1 — Scan a Flutter app project (US1 / SC-001)

A mobile developer's Flutter app source tree with `pubspec.yaml` + `pubspec.lock`:

```bash
mikebom --offline sbom scan --path . --output /tmp/app.cdx.json
```

Inspect main-module + deps:

```bash
# Main-module (the app itself):
jq '.metadata.component' /tmp/app.cdx.json
# {"bom-ref": "pkg:pub/my_flutter_app@1.2.3",
#  "name": "my_flutter_app", "version": "1.2.3",
#  "purl": "pkg:pub/my_flutter_app@1.2.3", ...
#  "properties": [
#    {"name": "mikebom:component-role", "value": "main-module"},
#    {"name": "mikebom:source-type", "value": "pub-main-module"}
#  ]}

# Direct + transitive deps:
jq '.components[] | select(.purl | startswith("pkg:pub/")) | .purl' /tmp/app.cdx.json | sort | head -10
# "pkg:pub/async@2.11.0"
# "pkg:pub/collection@1.18.0"
# "pkg:pub/http@1.1.0"
# "pkg:pub/http_parser@4.0.2"
# "pkg:pub/meta@1.10.0"
# "pkg:pub/provider@6.1.1"
# "pkg:pub/shared_preferences@2.2.2"
# ...
```

Count check against `dart pub deps`:

```bash
dart pub deps --json | jq '[.packages[] | select(.kind != "root")] | length'
# 47

jq '[.components[] | select(.purl | startswith("pkg:pub/")) | select(.properties[]? | .name == "mikebom:source-type" and .value != "pub-main-module")] | length' /tmp/app.cdx.json
# 47  ✓
```

## Scenario 2 — Source discriminator distinction (US2 / SC-002)

A Flutter app whose lockfile mixes hosted + git + path + sdk:

```bash
mikebom --offline sbom scan --path . --output /tmp/mixed.cdx.json
```

Filter by source type:

```bash
# Hosted deps only:
jq '.components[] | select(.properties[]? | .name == "mikebom:source-type" and .value == "pub-hosted") | .purl' /tmp/mixed.cdx.json
# "pkg:pub/http@1.1.0"
# "pkg:pub/provider@6.1.1"

# Git deps only:
jq '.components[] | select(.properties[]? | .name == "mikebom:source-type" and .value == "pub-git") | .purl' /tmp/mixed.cdx.json
# "pkg:pub/window_size@eb39649...3d5601?vcs_url=git+https://github.com/google/flutter-desktop-embedding.git#plugins/window_size"

# Path deps (operator's monorepo-local libs):
jq '.components[] | select(.properties[]? | .name == "mikebom:source-type" and .value == "pub-path") | .purl' /tmp/mixed.cdx.json
# "pkg:generic/my_local_lib@0.1.0"

# SDK pseudo-deps (Flutter framework + tooling):
jq '.components[] | select(.properties[]? | .name == "mikebom:source-type" and .value == "pub-sdk") | .purl' /tmp/mixed.cdx.json
# "pkg:pub/flutter@0.0.0"
# "pkg:pub/flutter_test@0.0.0"
```

## Scenario 3 — Self-hosted pub registry

A project pulling from a private pub mirror at `https://pub.acme.example.com`:

```bash
jq '.components[] | select(.purl | contains("repository_url=")) | .purl' /tmp/private.cdx.json
# "pkg:pub/internal_widget@2.0.0?repository_url=https://pub.acme.example.com"
```

Default-tap deps (from `pub.dev`) MUST NOT carry the qualifier:

```bash
jq '.components[] | select(.purl | startswith("pkg:pub/http@")) | .purl' /tmp/private.cdx.json
# "pkg:pub/http@1.1.0"     ← no ?repository_url= qualifier
```

## Scenario 4 — Library project, design-tier mode (US3 / SC-003)

A Dart library with `pubspec.yaml` but no `pubspec.lock`:

```bash
mikebom --offline sbom scan --path . --output /tmp/lib.cdx.json

# Components are design-tier (constraint preserved, not pinned):
jq '.components[] | {name, purl, props: .properties}' /tmp/lib.cdx.json | head -30
# {
#   "name": "http",
#   "purl": "pkg:pub/http@^1.0.0",                ← constraint not pinned
#   "props": [
#     {"name": "mikebom:sbom-tier", "value": "design"},
#     {"name": "mikebom:requirement-range", "value": "^1.0.0"},
#     {"name": "mikebom:evidence-kind", "value": "pubspec-yaml"},
#     {"name": "mikebom:source-type", "value": "pub-hosted"}
#   ]
# }
```

Dev deps tagged with lifecycle-scope (US3 acceptance scenario 3):

```bash
jq '.components[] | select(.properties[]? | .name == "mikebom:lifecycle-scope" and .value == "development") | .name' /tmp/lib.cdx.json
# "test"
# "build_runner"
# "lints"
```

`--include-dev=off` filtering works:

```bash
mikebom --offline sbom scan --path . --no-include-dev --output /tmp/lib-prod.cdx.json
jq '.components[] | select(.properties[]? | .name == "mikebom:lifecycle-scope" and .value == "development")' /tmp/lib-prod.cdx.json
# (empty — dev deps filtered)
```

## Scenario 5 — Workspace / monorepo (FR-009)

A Melos monorepo with multiple packages, each with its own `pubspec.yaml`:

```text
my_workspace/
├── packages/
│   ├── app/pubspec.yaml + pubspec.lock
│   ├── lib_a/pubspec.yaml + pubspec.lock
│   └── lib_b/pubspec.yaml + pubspec.lock
└── melos.yaml
```

```bash
mikebom --offline sbom scan --path my_workspace --output /tmp/workspace.cdx.json

# One main-module per pubspec.yaml — workspace structure is invisible:
jq '.components[] | select(.properties[]? | .name == "mikebom:component-role" and .value == "main-module") | .purl' /tmp/workspace.cdx.json
# "pkg:pub/app@1.0.0"
# "pkg:pub/lib_a@0.5.0"
# "pkg:pub/lib_b@0.3.0"
```

No synthetic workspace-root component emits (per Clarifications Q2). Same-PURL deps across the three lockfiles collapse via standard dedup.

## Scenario 6 — No-op on non-Dart rootfs (SC-004 regression invariant)

A pure Linux server rootfs (no `pubspec.yaml`, no `pubspec.lock`):

```bash
mikebom --offline sbom scan --path /mnt/server-rootfs --output /tmp/server.cdx.json 2>/tmp/scan.log

jq '[.components[] | select(.purl | startswith("pkg:pub/") or startswith("pkg:generic/"))] | length' /tmp/server.cdx.json
# (matches pre-feature baseline — Dart contributes zero)

grep -c 'dart\|pubspec' /tmp/scan.log
# 0
```

SBOM bytes are identical (modulo timestamps + serial numbers) to a pre-milestone-137 baseline for the same rootfs.

## Scenario 7 — Malformed lockfile graceful degradation (SC-005)

A monorepo where one `pubspec.lock` has corrupted YAML alongside three valid project subdirs:

```bash
mikebom --offline sbom scan --path /tmp/corrupted-dart --output /tmp/corrupted.cdx.json 2>/tmp/scan.log

# Scan exit code:
echo $?
# 0  (scan succeeded — partial output preserved)

# Components from the three valid projects emit:
jq '[.components[] | select(.purl | startswith("pkg:pub/"))] | length' /tmp/corrupted.cdx.json
# (sum of three valid projects' deps)

# Warn for the broken lockfile:
grep 'dart:.*parse failed\|pubspec' /tmp/scan.log
# WARN mikebom::scan_fs::package_db::dart: dart: failed to parse pubspec.lock, falling back to design-tier from pubspec.yaml path=/tmp/corrupted-dart/broken/pubspec.lock
```

## Verification commands

End-to-end SC validations:

```bash
# SC-001 — Flutter app baseline + dep edges
cargo test -p mikebom --test dart_flutter_app_baseline

# SC-002 — Source discriminator distinction
cargo test -p mikebom --test dart_source_discriminators

# SC-003 — Design-tier emission (no lockfile)
cargo test -p mikebom --test dart_design_tier

# SC-004 — Non-Dart byte-identity invariant
cargo test -p mikebom --test cdx_regression --test spdx_regression --test spdx3_regression

# SC-005 — Malformed-lockfile graceful degradation
cargo test -p mikebom --test dart_edge_cases -- malformed_lockfile

# SC-006 — Standard PURL filter usability
mikebom --offline sbom scan --path <project> --format cyclonedx-json --output /tmp/out.cdx.json
jq '.components[] | select(.purl | startswith("pkg:pub/"))' /tmp/out.cdx.json

# SC-007 — Dev-scope filterability
cargo test -p mikebom --test dart_flutter_app_baseline -- dev_scope

# SC-008 — Main-module emission
cargo test -p mikebom --test dart_flutter_app_baseline -- main_module
```

## Cross-format byte-equivalence check

Same scan, all three formats — Dart components must agree:

```bash
mikebom --offline sbom scan --path my_flutter_app \
  --format cyclonedx-json --format spdx-2.3-json --format spdx-3-json \
  --output cyclonedx-json=/tmp/dart.cdx.json \
  --output spdx-2.3-json=/tmp/dart.spdx.json \
  --output spdx-3-json=/tmp/dart.spdx3.json

jq '[.components[] | select(.purl | startswith("pkg:pub/")) | .purl] | sort' /tmp/dart.cdx.json > /tmp/cdx-pub.txt
jq '[.packages[].externalRefs[]? | select(.referenceType == "purl") | .referenceLocator | select(startswith("pkg:pub/"))] | sort' /tmp/dart.spdx.json > /tmp/spdx-pub.txt
jq '[.["@graph"][] | select(.software_packageUrl? | tostring | startswith("pkg:pub/")) | .software_packageUrl] | sort' /tmp/dart.spdx3.json > /tmp/spdx3-pub.txt

diff /tmp/cdx-pub.txt /tmp/spdx-pub.txt
diff /tmp/cdx-pub.txt /tmp/spdx3-pub.txt
# (no output = success)
```

## Known deferrals (documented in spec Out-of-Scope)

- **License emission**: `pubspec.lock` carries no license; extracting requires reading per-package `~/.pub-cache/hosted/pub.dev/<pkg>-<ver>/pubspec.yaml`. Cross-reader follow-up (parallels milestone-135 FR-012 + milestone-136 FR-011 deferrals).
- **Transitive dep edges**: v1 emits main-module → direct deps; transitive components surface but their inter-edges are deferred to v1.1.
- **`.dart_tool/package_config.json`** integration: lockfile-first design for v1; file-claim wiring is a separate spec.
- **Pre-Dart-2.0 lockfile format**: deferred indefinitely (rare in 2026).
