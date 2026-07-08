# Quickstart: m172 Manual Verification

**Feature**: 172-go-fallback-count
**Date**: 2026-07-07

Three verification paths (one per FR-006 scenario) plus a bonus guac-reproduction path for the incident that motivated m172.

## Path A — Healthy Go scan → value=`"0"`

**Setup**: a well-known Go project + a warm module cache + working GOPROXY. The mikebom repo itself qualifies (has go.mod artifacts in fixtures) — but for a real Go project:

```bash
git clone --depth 1 https://github.com/spf13/cobra /tmp/cobra
mikebom sbom scan --path /tmp/cobra \
    --format cyclonedx-json \
    --output cyclonedx-json=/tmp/cobra.cdx.json \
    --no-deep-hash
```

**Assertion**:

```bash
jq '.metadata.properties[]? | select(.name == "mikebom:go-transitive-fallback-count") | .value' /tmp/cobra.cdx.json
```

**Expected**: `"0"` — annotation present with value `"0"` because all Go modules resolved via steps 1-3 of the ladder.

## Path B — Degraded Go scan → value > 0

**Setup**: force step-5 fallback by disabling the proxy and running with no cache.

```bash
env -i HOME=/tmp/nohome PATH=/usr/bin:/bin GOPROXY=off GOMODCACHE=/tmp/nonexistent-cache \
    mikebom sbom scan --path /tmp/cobra \
    --format cyclonedx-json \
    --output cyclonedx-json=/tmp/cobra-degraded.cdx.json \
    --no-deep-hash
```

**Assertion**:

```bash
jq '.metadata.properties[]? | select(.name == "mikebom:go-transitive-fallback-count") | .value | tonumber' /tmp/cobra-degraded.cdx.json
```

**Expected**: an integer > 0 — probably ~all-Go-modules count. Reflects step 5 (go.sum fallback) resolving every module because steps 1-3 all failed.

**Cross-verify** with per-component annotations (SC-005 invariant):

```bash
jq '[.components[]?.properties[]? | select(.name == "mikebom:go-transitive-source" and .value == "go-sum-fallback")] | length' /tmp/cobra-degraded.cdx.json
```

Should equal the C117 count exactly.

## Path C — Non-Go scan → annotation absent

**Setup**: any pure non-Go project.

```bash
git clone --depth 1 https://github.com/expressjs/express /tmp/express
mikebom sbom scan --path /tmp/express \
    --format cyclonedx-json \
    --output cyclonedx-json=/tmp/express.cdx.json \
    --no-deep-hash
```

**Assertion**:

```bash
jq '.metadata.properties[]? | select(.name == "mikebom:go-transitive-fallback-count")' /tmp/express.cdx.json
```

**Expected**: empty output (nothing) — annotation is absent because no Go components in the scan.

## Bonus Path — Reproduce the guac incident that motivated m172

```bash
# Fresh clone at the specific commit
git clone https://github.com/guacsec/guac /tmp/guac-verify
git -C /tmp/guac-verify checkout ebb808e

# Simulate the reporter's degraded env
env -i HOME=/tmp/nohome PATH=/usr/bin:/bin GOPROXY=off GOMODCACHE=/tmp/nonexistent-cache \
    mikebom sbom scan --path /tmp/guac-verify --root-name guac --root-version ebb808e \
    --format cyclonedx-json \
    --output cyclonedx-json=/tmp/guac-verify.cdx.json \
    --no-deep-hash

# The single annotation that would have told the reporter exactly what happened:
jq '.metadata.properties[]? | select(.name | test("go-transitive-fallback-count|go-transitive-coverage"))' /tmp/guac-verify.cdx.json
```

**Expected output**:

```json
{
  "name": "mikebom:go-transitive-coverage",
  "value": "unknown"
}
{
  "name": "mikebom:go-transitive-coverage-reason",
  "value": "goproxy-off-in-chain: GOPROXY chain contains 'off'"
}
{
  "name": "mikebom:go-transitive-fallback-count",
  "value": "N"
}
```

Where N is the count of modules that fell to go.sum. The reporter would have seen this + the reading-guide entry + known immediately: "my scan had N degraded modules due to GOPROXY=off, not a mikebom regression".

## Full success criteria table

| SC | Verification | Expected |
|---|---|---|
| SC-001 | Path A jq | integer output |
| SC-002 | Path C jq | empty |
| SC-003 | Path A jq | `"0"` |
| SC-004 | Path B jq | positive integer |
| SC-005 | Path B cross-verify jq | doc-count == per-component-count |
| SC-006 | grep reading guide post-172 for "step 5" + "fallback-count" | matches found |
| SC-007 | `./scripts/pre-pr.sh` | `>>> all pre-PR checks passed.` |
| SC-008 | `git diff main -- 'mikebom-cli/tests/fixtures/golden/**' \| grep '^[+-]' \| grep -v golang` | no non-Go golden changes |
| SC-009 | 5-minute stopwatch: consumer diagnoses "my SBOM has degraded transitive graph" | resolution within 5 min |
