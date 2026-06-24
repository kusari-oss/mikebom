# Quickstart — milestone 141 Erlang/OTP rebar reader

Operator-facing walkthrough of the scenarios this milestone surfaces.

## Scenario 1 — Scan a rebar3 OTP app (US1 / SC-001)

```bash
mikebom --offline sbom scan --path . --output /tmp/app.cdx.json
```

```bash
# Main-module:
jq '.metadata.component' /tmp/app.cdx.json
# {"bom-ref": "pkg:hex/my_app@1.2.3",
#  "name": "my_app", "version": "1.2.3",
#  "purl": "pkg:hex/my_app@1.2.3",
#  "properties": [
#    {"name": "mikebom:component-role", "value": "main-module"},
#    {"name": "mikebom:source-type", "value": "erlang-main-module"}
#  ]}

jq '.components[] | select(.purl | startswith("pkg:hex/")) | .purl' /tmp/app.cdx.json | sort | head -10
# "pkg:hex/cowboy@2.10.0"
# "pkg:hex/cowlib@2.12.1"
# "pkg:hex/goldrush@0.1.9"
# "pkg:hex/jiffy@1.1.1"
# "pkg:hex/lager@3.9.2"
# "pkg:hex/ranch@1.8.0"
```

## Scenario 2 — Source discriminator distinction (US2 / SC-002)

```bash
mikebom --offline sbom scan --path . --output /tmp/mixed.cdx.json

# Hex (default Hex.pm):
jq '.components[] | select(.properties[]? | .name == "mikebom:source-type" and .value == "erlang-hex") | .purl' /tmp/mixed.cdx.json

# Hex (private organization) — namespace + repository_url qualifier:
jq '.components[] | select(.purl | contains("?repository_url=")) | .purl' /tmp/mixed.cdx.json
# "pkg:hex/acme/internal_lib@2.0.0?repository_url=https://repo.hex.pm"

# Git source — pkg:generic/ placeholder per the Hex git-source convention:
jq '.components[] | select(.properties[]? | .name == "mikebom:source-type" and .value == "erlang-git") | .purl' /tmp/mixed.cdx.json
# "pkg:generic/my_fork@eb39649a76b87e8451baf75d10ce82ca3a3d5601?vcs_url=git+https://github.com/foo/my-fork.git"

# OTP runtime placeholders:
jq '.components[] | select(.properties[]? | .name == "mikebom:source-type" and .value == "erlang-otp-runtime") | .purl' /tmp/mixed.cdx.json
# "pkg:generic/kernel@unspecified"
# "pkg:generic/stdlib@unspecified"
# "pkg:generic/crypto@unspecified"
```

## Scenario 3 — Inner SHA-256 emission (FR-011)

```bash
jq '.components[] | select(.name == "cowboy") | .hashes' /tmp/app.cdx.json
# [
#   {"alg": "SHA-256", "content": "<inner_sha256_from_4th_tuple_element>"}
# ]
```

Pre-rebar3-3.7 entries (legacy `{<<name>>, <<version>>}` shape) lack the
inner hash; mikebom emits ZERO hash entries for those per the FR-011
best-effort posture.

## Scenario 4 — Design-tier when no rebar.lock present (US3 / SC-003)

`rebar.config` only, no `rebar.lock`:

```bash
mikebom --offline sbom scan --path /tmp/erlang-lib --output /tmp/lib.cdx.json

jq '.components[] | {name, purl, props: [.properties[] | {name, value}]}' /tmp/lib.cdx.json
# {
#   "name": "cowboy",
#   "purl": "pkg:hex/cowboy@~>%202.10",            # constraint URL-encoded
#   "props": [
#     {"name": "mikebom:sbom-tier", "value": "design"},
#     {"name": "mikebom:requirement-range", "value": "~> 2.10"},
#     {"name": "mikebom:evidence-kind", "value": "rebar-config"},
#     {"name": "mikebom:source-type", "value": "erlang-hex"}
#   ]
# },
# {
#   "name": "meck",
#   "purl": "pkg:hex/meck@~>%200.9",                # test-profile-scoped
#   "props": [
#     {"name": "mikebom:sbom-tier", "value": "design"},
#     {"name": "mikebom:lifecycle-scope", "value": "development"}   # FR-008 profile discriminator
#   ]
# }
```

## Scenario 5 — Q3 keyword family discrimination (SC-010)

```bash
# *.app.src declares applications + included + optional:
cat apps/my_app/src/my_app.app.src
# {application, my_app, [
#     {vsn, "1.0.0"},
#     {applications, [kernel, stdlib, cowboy]},
#     {included_applications, [config_app]},
#     {optional_applications, [telemetry]},
#     {description, "App with all three keyword families"}
# ]}.

mikebom --offline sbom scan --path . --output /tmp/q3.cdx.json

# Filter to hard-deps-only (Q3 operator workflow):
jq '.components[] | select(.properties[]? | .name == "mikebom:erlang-app-dep-kind" and .value == "required") | .name' /tmp/q3.cdx.json
# "kernel"
# "stdlib"
# "cowboy"

# Optional-only (e.g., telemetry integrations the operator might want to swap):
jq '.components[] | select(.properties[]? | .name == "mikebom:erlang-app-dep-kind" and .value == "optional") | .name' /tmp/q3.cdx.json
# "telemetry"

# Embedded sub-apps:
jq '.components[] | select(.properties[]? | .name == "mikebom:erlang-app-dep-kind" and .value == "included") | .name' /tmp/q3.cdx.json
# "config_app"
```

## Scenario 6 — Umbrella project (SC-009)

```bash
# Apps under apps/:
ls /tmp/umbrella-project/apps/
# my_app/  my_lib/  my_worker/

mikebom --offline sbom scan --path /tmp/umbrella-project --output /tmp/u.cdx.json

# Main-modules per sub-app = 3 total:
jq '.components[] | select(.properties[]? | .name == "mikebom:component-role" and .value == "main-module") | .purl' /tmp/u.cdx.json
# "pkg:hex/my_app@1.0.0"
# "pkg:hex/my_lib@1.0.0"
# "pkg:hex/my_worker@1.0.0"

# Same-PURL deps across sub-apps collapse to single component entries:
jq '[.components[] | select(.purl | contains("pkg:hex/cowboy@"))] | length' /tmp/u.cdx.json
# 1   (even if cowboy declared in all 3 sub-app .app.src files)
```

## Scenario 7 — Dev-scope filtering (SC-007)

```bash
# default scan includes test-profile deps:
jq '.components[] | select(.name == "meck") | .properties' /tmp/lib.cdx.json
# [..., {"name": "mikebom:lifecycle-scope", "value": "development"}, ...]

# --exclude-scope dev suppresses (top-level flag BEFORE sbom subcommand):
mikebom --offline --exclude-scope dev sbom scan --path . --output /tmp/lib-prod.cdx.json
jq '.components[] | select(.name == "meck")' /tmp/lib-prod.cdx.json
# (empty — meck suppressed)
```

## Scenario 8 — No-op on non-Erlang rootfs (SC-004)

```bash
mikebom --offline sbom scan --path /mnt/server-rootfs --output /tmp/server.cdx.json 2>/tmp/scan.log

jq '[.components[] | select(.properties[]? | .name == "mikebom:source-type" and (.value | startswith("erlang-")))] | length' /tmp/server.cdx.json
# 0  (Erlang reader contributes zero on non-Erlang trees)

grep -c 'erlang\|rebar\.\|\.app\.src' /tmp/scan.log
# 0
```

## Scenario 9 — Malformed lockfile graceful degradation (SC-005)

```bash
mikebom --offline sbom scan --path /tmp/corrupted-erlang --output /tmp/corrupted.cdx.json 2>/tmp/scan.log

echo $?
# 0  (scan succeeded)

grep 'failed to parse rebar.lock' /tmp/scan.log
# WARN mikebom::scan_fs::package_db::erlang: erlang: failed to parse rebar.lock, falling back to design-tier from rebar.config path=...
```

## Verification commands

```bash
cargo test -p mikebom --test erlang_rebar_baseline           # SC-001 + SC-007 + SC-008
cargo test -p mikebom --test erlang_source_discriminators    # SC-002
cargo test -p mikebom --test erlang_tier_fallbacks           # SC-003 + SC-009 umbrella + SC-010 Q3 keyword family
cargo test -p mikebom --test erlang_edge_cases               # SC-005 + binary-string atoms + legacy shape + map-form + main-module fallback
cargo test -p mikebom --test cdx_regression --test spdx_regression --test spdx3_regression  # SC-004 byte-identity preservation
```

## Cross-format byte-equivalence check

```bash
mikebom --offline sbom scan --path my_erlang_app \
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

- License extraction (cross-reader follow-up; lives in each Hex package's
  `hex_metadata` under `deps/<pkg>/`).
- Per-package transitive dep edges from lockfile (v1.1).
- Per-app transitive dep edges across the umbrella supervision-tree (v1.1).
- Private Hex API enrichment (license / owner / downloads).
- syft/trivy compatibility `mikebom:also-known-as` annotation (v1.1).
- Pre-rebar3 (rebar2) lockfile format (out of scope indefinitely).
- `escript` archives + standalone OTP releases.
- Compiled `*.app` files under `_build/` (walker excludes `_build/`).
- Mixed Erlang/Elixir umbrella projects (each reader handles its own
  sub-apps; cross-reader coordination deferred to v1.1).
