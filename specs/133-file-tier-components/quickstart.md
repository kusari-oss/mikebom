# Quickstart: Milestone 133 SC verification protocol

**Date**: 2026-06-19
**Branch**: `133-file-tier-components`
**Audience**: implementer claiming a milestone-133 SC; CI re-verification; code reviewers

End-to-end protocol for verifying each milestone-133 SC against the pinned
audit baseline. Built on milestone-132's verification harness.

## Step 0: Prerequisites

Pinned audit baseline unchanged from milestone 132:
`767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner@sha256:4e7b05811ce4885d8a7183819b4e0e209662784fe24b7553ceea3d149e3c719c`.

If the image is not in the local docker daemon, refresh ECR creds and pull:

```sh
aws sso login --sso-session kusari
AWS_PROFILE=AWSAdministratorAccess-204753367867 aws ecr get-login-password --region us-east-1 \
  | docker login --username AWS --password-stdin 767397973649.dkr.ecr.us-east-1.amazonaws.com
docker pull 767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner@sha256:4e7b05811ce4885d8a7183819b4e0e209662784fe24b7553ceea3d149e3c719c
docker tag 767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner@sha256:4e7b05811ce4885d8a7183819b4e0e209662784fe24b7553ceea3d149e3c719c rp-132-us3:pinned
```

## Step 1: SC-001 — orphan count in 180-440 band

Two ways to verify; use both for high confidence.

### 1a: Re-run the FR-022 projection tool (upper bound, offline)

```sh
# If /tmp/mb-133-projection/ already contains rootfs from planning, skip the
# extract; otherwise:
docker save rp-132-us3:pinned -o /tmp/mb-133-projection/image.tar
mkdir -p /tmp/mb-133-projection/layers /tmp/mb-133-projection/rootfs
tar -xf /tmp/mb-133-projection/image.tar -C /tmp/mb-133-projection/layers
for layer in $(jq -r '.[0].Layers[]' /tmp/mb-133-projection/layers/manifest.json); do
    tar -xf "/tmp/mb-133-projection/layers/$layer" -C /tmp/mb-133-projection/rootfs
done

# Re-export the current binary-tier hashes from the latest cached mikebom SBOM:
jq -r '[.. | objects | (.hashes // [])[]? | select(.alg=="SHA-256") | .content] | unique | .[]' \
    /tmp/mb-rp-132-us3-online.cdx.json > /tmp/mb-133-projection/known-hashes.txt

# Run the projection — should report ~245 after the FR-005 tightening lands in
# the production code path. If the projection script is updated to apply the
# milestone-133 FR-005 path-prefix exclusion list, the count is even closer to
# what production will emit.
/tmp/mb-133-projection/project.sh
# Look for "PROJECTED UPPER-BOUND ORPHAN COUNT" — should be in 180-440 band.
```

### 1b: Production scan + count file-tier components

```sh
# Build the milestone-133 release binary:
cargo build --release --bin mikebom

# Default-mode scan (orphan emission ON by default per FR-015):
./target/release/mikebom sbom scan \
  --image rp-132-us3:pinned \
  --output /tmp/mb-rp-133-orphan.cdx.json \
  --root-name 767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner \
  --offline

# Count file-tier components:
jq '[.components[] | select((.properties // [])[]? | select(.name == "mikebom:component-tier" and .value == "file"))] | length' \
   /tmp/mb-rp-133-orphan.cdx.json
# Expect: in 180-440 band (SC-001).
```

### 1c: Zero-duplicate gate (SC-002)

```sh
# Extract package-tier + binary-tier hash sets, plus file-tier hash set.
# SC-002 requires: file-tier ∩ package/binary-tier = ∅.
jq -r '[.components[] | select((.properties // [])[]? | select(.name == "mikebom:component-tier" and .value == "file")) | (.hashes // [])[] | select(.alg == "SHA-256") | .content] | unique | .[]' \
   /tmp/mb-rp-133-orphan.cdx.json > /tmp/mb-133-file-hashes.txt
jq -r '[.components[] | select(((.properties // []) | map(select(.name == "mikebom:component-tier")) | length) == 0 or ((.properties // []) | map(select(.name == "mikebom:component-tier" and .value != "file")) | length) > 0) | (.hashes // [])[] | select(.alg == "SHA-256") | .content] | unique | .[]' \
   /tmp/mb-rp-133-orphan.cdx.json > /tmp/mb-133-other-hashes.txt
comm -12 <(sort /tmp/mb-133-file-hashes.txt) <(sort /tmp/mb-133-other-hashes.txt) | wc -l
# Expect: 0 (no hash appears as both a file-tier and a package/binary-tier
# component).
```

## Step 2: SC-003 — full mode lifts Completeness to ≥4★

```sh
# Online-mode full scan (deps.dev on; --file-inventory=full):
./target/release/mikebom sbom scan \
  --image rp-132-us3:pinned \
  --output /tmp/mb-rp-133-full.cdx.json \
  --root-name 767397973649.dkr.ecr.us-east-1.amazonaws.com/remediation-planner \
  --file-inventory=full

# syft baseline against the same digest (re-use cached if available):
test -s /tmp/syft-rp-132-baseline.cdx.json || \
    DOCKER_HOST=unix:///Users/mlieberman/.colima/default/docker.sock \
    syft scan docker:rp-132-us3:pinned -o cyclonedx-json=/tmp/syft-rp-132-baseline.cdx.json

# Scorecard:
/Users/mlieberman/Projects/sbom-comparison/sbom-comparison \
  -format json -o /tmp/mb-rp-133-full.scorecard.json \
  /tmp/mb-rp-133-full.cdx.json /tmp/syft-rp-132-baseline.cdx.json

# SC-003 check:
jq '.completeness.starsA' /tmp/mb-rp-133-full.scorecard.json
# Expect: >= 4 (was 1 pre-milestone-133).
```

## Step 3: SC-004 — scan-time growth budgets

```sh
# Offline-vs-offline measurement isolates from deps.dev latency:
hyperfine --warmup 1 --runs 3 \
  --export-json /tmp/mb-rp-133-timing.json \
  "./target/release/mikebom sbom scan --image rp-132-us3:pinned --offline --output /tmp/discard.cdx.json --root-name foo --file-inventory=off" \
  "./target/release/mikebom sbom scan --image rp-132-us3:pinned --offline --output /tmp/discard.cdx.json --root-name foo --file-inventory=orphan" \
  "./target/release/mikebom sbom scan --image rp-132-us3:pinned --offline --output /tmp/discard.cdx.json --root-name foo --file-inventory=full"

jq '.results | map({name: .command, median: .median})' /tmp/mb-rp-133-timing.json
# Compute growth ratios:
#   orphan_median / off_median  → expect < 1.50  (SC-004 orphan budget <50%)
#   full_median   / off_median  → expect < 4.00  (SC-004 full   budget <300%)
```

## Step 4: SC-005 — byte-identity goldens

```sh
cargo +stable test --workspace --test cdx_regression --test spdx_regression --test spdx3_regression
# Expect: all pass without regen.
# Image-scan golden test (if any) MAY require regen from US2 path/layer property
# additions — that churn is declared intentional in SC-005.
```

## Step 5: SC-006 — Constitution version + reference doc

```sh
grep -c '^**Version**: 1.5.0' .specify/memory/constitution.md
# Expect: 1
grep -c '^### 5\. ' .specify/memory/constitution.md
# Expect: 1 (Strict Boundary §5)
test -f docs/reference/component-tiers.md && echo OK
# Expect: OK
```

## Step 6: SC-007 — full-mode ≥10× orphan-mode

```sh
orphan_count=$(jq '[.components[] | select((.properties // [])[]? | select(.name == "mikebom:component-tier" and .value == "file"))] | length' /tmp/mb-rp-133-orphan.cdx.json)
full_count=$(jq   '[.components[] | select((.properties // [])[]? | select(.name == "mikebom:component-tier" and .value == "file"))] | length' /tmp/mb-rp-133-full.cdx.json)
echo "orphan: $orphan_count; full: $full_count; ratio: $(echo "scale=2; $full_count / $orphan_count" | bc)"
# Expect: ratio >= 10.0
```

## Step 7: Pre-PR gate (mandatory per CLAUDE.md)

```sh
./scripts/pre-pr.sh
# Equivalent to:
#   cargo +stable clippy --workspace --all-targets  (zero errors)
#   cargo +stable test --workspace                  (every suite N passed; 0 failed)
```

Per the standing "Pre-PR gate: full output, don't grep" feedback, paste the
per-target `N passed; 0 failed` lines verbatim into the PR description.

## Step 8: Honest PR description template

```markdown
## Milestone 133 — what shipped

| SC | Target | Measured (pinned digest sha256:4e7b…) | Status |
|----|--------|--------------------------------------|--------|
| SC-001 | orphan count in 180-440 | <fill from Step 1b> | <MET / NOT MET> |
| SC-002 | zero-duplicate gate | <fill from Step 1c> | <MET / NOT MET> |
| SC-003 | full mode Completeness ≥4★ | <fill from Step 2> | <MET / NOT MET> |
| SC-004 | orphan <50%, full <300% growth | <fill from Step 3> | <MET / NOT MET> |
| SC-005 | byte-identity goldens preserved (except declared image-scan churn) | <fill from Step 4> | <MET / NOT MET> |
| SC-006 | Constitution 1.5.0 + ref doc | <fill from Step 5> | <MET / NOT MET> |
| SC-007 | full ≥10× orphan | <fill from Step 6> | <MET / NOT MET> |

Pinned digest: sha256:4e7b05811ce4885d8a7183819b4e0e209662784fe24b7553ceea3d149e3c719c
Pre-PR gate: <paste the two `N passed; 0 failed` lines verbatim>

## Behavior change callout (per Q1 clarification)

This PR flips the `--file-inventory` default from off (pre-133 implicit) to
`orphan`. Existing mikebom users will see ~245 new file-tier components per
image scan on upgrade — all carry `mikebom:component-tier = "file"` annotation
so consumers can filter client-side. Users wanting bit-for-bit pre-133
behavior set `--file-inventory=off` explicitly.
```
