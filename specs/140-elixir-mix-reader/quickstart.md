# Quickstart — milestone 140 Elixir/Mix reader

Operator-facing walkthrough of the scenarios this milestone surfaces.

## Scenario 1 — Scan a Phoenix app (US1 / SC-001)

```bash
mikebom --offline sbom scan --path . --output /tmp/app.cdx.json
```

```bash
# Main-module:
jq '.metadata.component' /tmp/app.cdx.json
# {"bom-ref": "pkg:hex/my_app@0.5.2",
#  "name": "my_app", "version": "0.5.2",
#  "purl": "pkg:hex/my_app@0.5.2", ...
#  "properties": [
#    {"name": "mikebom:component-role", "value": "main-module"},
#    {"name": "mikebom:source-type", "value": "hex-main-module"}
#  ]}

jq '.components[] | select(.purl | startswith("pkg:hex/")) | .purl' /tmp/app.cdx.json | sort | head -10
# "pkg:hex/ecto@3.11.1"
# "pkg:hex/jason@1.4.4"
# "pkg:hex/mime@2.0.5"
# "pkg:hex/phoenix@1.7.10"
# "pkg:hex/plug@1.15.2"
# "pkg:hex/plug_crypto@2.0.0"
# "pkg:hex/telemetry@1.2.1"
```

## Scenario 2 — Source discriminator distinction (US2 / SC-002)

```bash
mikebom --offline sbom scan --path . --output /tmp/mixed.cdx.json

# Hex (default hexpm):
jq '.components[] | select(.properties[]? | .name == "mikebom:source-type" and .value == "hex-hex") | .purl' /tmp/mixed.cdx.json

# Hex (private organization) — note the namespace + repository_url qualifier:
jq '.components[] | select(.purl | contains("?repository_url=")) | .purl' /tmp/mixed.cdx.json
# "pkg:hex/acme/internal_lib@2.0.0?repository_url=https://repo.hex.pm"

# Git source — pkg:generic/ placeholder per Phase 0 correction:
jq '.components[] | select(.properties[]? | .name == "mikebom:source-type" and .value == "hex-git") | .purl' /tmp/mixed.cdx.json
# "pkg:generic/my_fork@eb39649a76b87e8451baf75d10ce82ca3a3d5601?vcs_url=git+https://github.com/foo/my-fork.git"

# Path source:
jq '.components[] | select(.properties[]? | .name == "mikebom:source-type" and .value == "hex-path") | .purl' /tmp/mixed.cdx.json
# "pkg:generic/shared_lib@unspecified"
```

## Scenario 3 — Inner + outer SHA-256 emission (Q3 / FR-011)

```bash
jq '.components[] | select(.name == "phoenix") | .hashes' /tmp/app.cdx.json
# [
#   {"alg": "SHA-256", "content": "<inner_sha256_from_4th_tuple_element>"},
#   {"alg": "SHA-256", "content": "<outer_sha256_from_8th_tuple_element>"}
# ]
```

Pre-Hex-2.0 entries lack the outer hash; mikebom emits ONE hash entry (Q3 best-effort).

## Scenario 4 — Design-tier with conditional extraction (US3 / Q1)

`mix.exs` only, no lockfile:

```bash
jq '.components[] | {name, purl, props: .properties}' /tmp/lib.cdx.json
# {
#   "name": "phoenix",
#   "purl": "pkg:hex/phoenix@~>_1.7",         # constraint preserved (sanitized for PURL)
#   "props": [
#     {"name": "mikebom:sbom-tier", "value": "design"},
#     {"name": "mikebom:requirement-range", "value": "~> 1.7"},
#     {"name": "mikebom:evidence-kind", "value": "mix-exs"},
#     {"name": "mikebom:source-type", "value": "hex-hex"}
#   ]
# },
# {
#   "name": "meck",
#   "purl": "pkg:hex/meck@~>_0.9",            # extracted from inside `if Mix.env() == :test`
#   "props": [
#     {"name": "mikebom:elixir-extraction-mode", "value": "conditional-flattened"}  # Q1 precision-loss signal
#   ]
# }
```

## Scenario 5 — Umbrella project (Q2 / SC-010)

```bash
# Apps under apps/:
ls /tmp/umbrella-project/apps/
# core/  web/  worker/

mikebom --offline sbom scan --path /tmp/umbrella-project --output /tmp/u.cdx.json

# Main-modules per sub-app + root = 4 total:
jq '.components[] | select(.properties[]? | .name == "mikebom:component-role" and .value == "main-module") | .purl' /tmp/u.cdx.json
# "pkg:hex/my_umbrella@0.1.0"     ← root (with mikebom:umbrella-root annotation)
# "pkg:hex/core@0.1.0"
# "pkg:hex/web@0.1.0"
# "pkg:hex/worker@0.1.0"

# Root's depends list = its own tooling + each sub-app main-module per Q2:
jq '.dependencies[] | select(.ref | contains("my_umbrella"))' /tmp/u.cdx.json
```

## Scenario 6 — Dev-scope filtering (SC-007)

```bash
# default scan includes dev/test deps:
jq '.components[] | select(.name == "credo") | .properties' /tmp/lib.cdx.json
# [{"name": "mikebom:lifecycle-scope", "value": "development"}]

# --exclude-scope dev suppresses (top-level flag BEFORE sbom subcommand):
mikebom --offline --exclude-scope dev sbom scan --path . --output /tmp/lib-prod.cdx.json
jq '.components[] | select(.name == "credo")' /tmp/lib-prod.cdx.json
# (empty — credo suppressed)
```

## Scenario 7 — No-op on non-Elixir rootfs (SC-004)

```bash
mikebom --offline sbom scan --path /mnt/server-rootfs --output /tmp/server.cdx.json 2>/tmp/scan.log

jq '[.components[] | select(.purl | startswith("pkg:hex/"))] | length' /tmp/server.cdx.json
# (matches pre-feature baseline — Elixir contributes zero)

grep -c 'elixir\|mix.lock\|mix.exs' /tmp/scan.log
# 0
```

## Scenario 8 — Malformed lockfile graceful degradation (SC-005)

```bash
mikebom --offline sbom scan --path /tmp/corrupted-elixir --output /tmp/corrupted.cdx.json 2>/tmp/scan.log

echo $?
# 0  (scan succeeded)

grep 'failed to parse mix.lock' /tmp/scan.log
# WARN mikebom::scan_fs::package_db::elixir: elixir: failed to parse mix.lock, falling back to design-tier from mix.exs path=...
```

## Verification commands

```bash
cargo test -p mikebom --test elixir_phoenix_baseline       # SC-001 + SC-007 + SC-008 + SC-009
cargo test -p mikebom --test elixir_source_discriminators  # SC-002
cargo test -p mikebom --test elixir_tier_fallbacks         # SC-003 + Q1 conditional-flatten + SC-010 umbrella
cargo test -p mikebom --test elixir_edge_cases             # SC-005 + multi-line tuples + dual SHA-256 + :github shortcut
cargo test -p mikebom --test cdx_regression --test spdx_regression --test spdx3_regression  # SC-004
```

## Cross-format byte-equivalence check

```bash
mikebom --offline sbom scan --path my_elixir_app \
  --format cyclonedx-json --format spdx-2.3-json --format spdx-3-json \
  --output cyclonedx-json=/tmp/e.cdx.json \
  --output spdx-2.3-json=/tmp/e.spdx.json \
  --output spdx-3-json=/tmp/e.spdx3.json

jq '[.components[] | select(.purl | startswith("pkg:hex/")) | .purl] | sort' /tmp/e.cdx.json > /tmp/cdx-hex.txt
jq '[.packages[].externalRefs[]? | select(.referenceType == "purl") | .referenceLocator | select(startswith("pkg:hex/"))] | sort' /tmp/e.spdx.json > /tmp/spdx-hex.txt
jq '[.["@graph"][] | select(.software_packageUrl? | tostring | startswith("pkg:hex/")) | .software_packageUrl] | sort' /tmp/e.spdx3.json > /tmp/spdx3-hex.txt

diff /tmp/cdx-hex.txt /tmp/spdx-hex.txt
diff /tmp/cdx-hex.txt /tmp/spdx3-hex.txt
# (no output = success)
```

## Known deferrals (spec Out-of-Scope)

- License extraction (cross-reader follow-up; lives in per-package `hex_metadata.config` under `deps/<pkg>/`).
- Transitive dep edges from lockfile tuple's 6th element (v1.1).
- Per-target attribution for umbrella sub-apps (v1.1).
- Private Hex API enrichment (license / owner / downloads).
- syft/trivy compatibility `mikebom:also-known-as` annotation (v1.1).
- Pre-Elixir-1.4 lockfile format (warn-and-skip).
- `deps/` directory walking for installed-tier scans.
