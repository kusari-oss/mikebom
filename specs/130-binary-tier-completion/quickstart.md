# Quickstart — milestone 130

Three operator-facing scenarios. Each is a one-command repro of the corresponding user story.

## Scenario 1 — Cargo crate enumeration on cargo-auditable binaries (US1)

Astral's `uv` is statically linked from ~200 Rust crates, declared via `cargo auditable` in the
binary's `.dep-v0` ELF section. Pre-130 mikebom silently suppressed these because `/usr/bin/uv` is
apk-claimed in the audit image. Post-130 the suppression is removed.

```sh
mikebom sbom scan \
    --image 767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner:latest \
    --output cyclonedx-json=/tmp/rp-130.cdx.json \
    --root-name 767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner \
    --offline
```

**Expected output**:

```sh
jq -r '.components[] | select(.purl // "" | startswith("pkg:cargo")) | .purl' /tmp/rp-130.cdx.json | sort -u | wc -l
# Expected: ≥900 (pre-130: 58)
```

## Scenario 2 — Maven dependencies inside Spring Boot uber JAR (US2)

A Spring Boot uber JAR carries dozens of dependency JARs nested in `BOOT-INF/lib/`. After milestone
130, mikebom descends into them.

```sh
# Clone spring-petclinic and build the uber JAR
git clone https://github.com/spring-projects/spring-petclinic /tmp/petclinic
cd /tmp/petclinic && ./mvnw clean package -DskipTests

mikebom sbom scan \
    --path /tmp/petclinic/target \
    --output cyclonedx-json=/tmp/petclinic-130.cdx.json
```

**Expected output**:

```sh
jq -r '.components[] | select(.purl // "" | startswith("pkg:maven"))
                    | .properties[]?
                    | select(.name == "mikebom:source-mechanism")
                    | .value' /tmp/petclinic-130.cdx.json | sort | uniq -c
# Expected (post-130):
#   1  maven-jar           (top-level uber JAR itself)
#   ~50  maven-jar-nested  (nested deps in BOOT-INF/lib/)
```

## Scenario 3 — NuGet packages from PE/CLR managed-assembly metadata (US3)

A Microsoft-published .NET runtime image ships managed `.dll` files. Some are covered by `.deps.json`
sidecars (which milestone 129 already parses), but others — reference assemblies under
`/usr/share/dotnet/packs/`, MSBuild task DLLs, CLI host extensions — are NOT. Post-130 mikebom
extracts each managed assembly's `(name, version)` from CLR metadata.

```sh
mikebom sbom scan \
    --image mcr.microsoft.com/dotnet/runtime:8.0-alpine \
    --output cyclonedx-json=/tmp/dotnet-runtime-130.cdx.json \
    --root-name dotnet-runtime
```

**Expected output**:

```sh
jq -r '.components[] | select(.purl // "" | startswith("pkg:nuget"))
                    | .properties[]?
                    | select(.name == "mikebom:source-mechanism")
                    | .value' /tmp/dotnet-runtime-130.cdx.json | sort | uniq -c
# Expected (post-130):
#   ~50   dotnet-deps-json           (existing milestone 129 US1A)
#   ~400  dotnet-assembly-metadata   (NEW, milestone 130 US3)
```

For multi-culture resource assemblies (e.g. `Microsoft.AspNetCore.Localization` with cultures
de/fr/ja/ko/zh-Hans/zh-Hant), expect ONE component per `(name, version)` with the cultures listed
in `mikebom:assembly-cultures`:

```sh
jq -r '.components[] | select(.name == "Microsoft.AspNetCore.Localization")
                    | {name, version, cultures: ([.properties[]? | select(.name == "mikebom:assembly-cultures") | .value][0])}' /tmp/dotnet-runtime-130.cdx.json
# Expected: one entry with cultures = "de,fr,ja,ko,zh-Hans,zh-Hant"
```

---

## How to verify mikebom didn't regress (SC-005 byte-identity)

For any image where the new readers find no applicable inputs, the emitted SBOM MUST be
byte-identical to the post-milestone-129 output:

```sh
./scripts/regen-goldens.sh
git status --short mikebom-cli/tests/fixtures/
# Expected: no .cdx.json or .spdx.json files in the diff
```

## How to verify each SC

| SC | Verification command |
|---|---|
| SC-001 (cargo ≥900) | `jq -r '.components[].purl' /tmp/rp-130.cdx.json \| grep -c '^pkg:cargo'` returns ≥900 |
| SC-002 (nuget PE ≥400) | filter for `mikebom:source-mechanism = "dotnet-assembly-metadata"`; count ≥400 |
| SC-003 (maven nested ≥1 per nested JAR) | `binary_tier_completion_us2_maven_nested_jar.rs` integration test |
| SC-004 (sbom-comparison weighted ≥4.5) | `/Users/mlieberman/Projects/sbom-comparison/sbom-comparison /tmp/rp-130.cdx.json ~/Downloads/remediation-planner-syft-image-sbom.json` |
| SC-005 (byte-identity 33 goldens) | `./scripts/regen-goldens.sh` produces zero `.cdx.json` / `.spdx.json` churn |
| SC-006 (scan time +30% cap) | `time` Scenario 1 pre/post; assert wall-clock <1.3× |
| SC-007 (independently shippable) | Per-PR landings — US1 alone passes SC-001 + SC-004 (cargo coverage already 90% of weighted-score completeness gain) |
