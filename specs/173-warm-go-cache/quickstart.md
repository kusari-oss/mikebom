# Quickstart: m173 Manual Verification

**Feature**: 173-warm-go-cache
**Date**: 2026-07-08

Three verification paths matching US1/US2/US3 + one bonus path exercising the concurrency knob.

## Path A — Monorepo warming: fallback-count 0 after opting in

**Setup**: use a real Go monorepo (or the milestone-055 `simple-module` fixture wrapped in a 2-workspace layout). Cold cache required — use `env -i HOME=/tmp/fake` to isolate.

```bash
# Baseline (no warming) — expect positive fallback count
env -i HOME=/tmp/fake-home-baseline PATH=/usr/bin:/bin \
  mikebom sbom scan --path /tmp/cobra-monorepo \
    --format cyclonedx-json \
    --output cyclonedx-json=/tmp/baseline.cdx.json \
    --no-deep-hash

jq '.metadata.properties[]? | select(.name == "mikebom:go-transitive-fallback-count") | .value' /tmp/baseline.cdx.json
# Expected: a positive integer, e.g. "11"

# Warmed (with the new flag) — expect "0"
env -i HOME=/tmp/fake-home-warmed PATH=/usr/bin:/bin \
  mikebom sbom scan --path /tmp/cobra-monorepo \
    --warm-go-cache=per-workspace \
    --format cyclonedx-json \
    --output cyclonedx-json=/tmp/warmed.cdx.json \
    --no-deep-hash

jq '.metadata.properties[]? | select(.name == "mikebom:go-transitive-fallback-count") | .value' /tmp/warmed.cdx.json
# Expected: "0"

jq '.metadata.properties[]? | select(.name == "mikebom:go-cache-warming-mode") | .value' /tmp/warmed.cdx.json
# Expected: "per-workspace"
```

**SC-001 verified**: single flag flips the fallback count from positive → 0.

## Path B — Advisory log fires exactly once in the baseline case

```bash
env -i HOME=/tmp/fake-home-baseline PATH=/usr/bin:/bin \
  mikebom sbom scan --path /tmp/cobra-monorepo \
    --format cyclonedx-json \
    --output cyclonedx-json=/tmp/baseline.cdx.json \
    --no-deep-hash 2> /tmp/baseline.stderr

grep -cF "Prime the cache with --warm-go-cache=per-workspace or 'go mod download' per workspace" /tmp/baseline.stderr
# Expected: 1
```

Now verify the suppression rule with explicit `--warm-go-cache=off`:

```bash
env -i HOME=/tmp/fake-home-baseline-explicit PATH=/usr/bin:/bin \
  mikebom sbom scan --path /tmp/cobra-monorepo \
    --warm-go-cache=off \
    --format cyclonedx-json \
    --output cyclonedx-json=/tmp/baseline-explicit.cdx.json \
    --no-deep-hash 2> /tmp/baseline-explicit.stderr

grep -cF "Prime the cache with --warm-go-cache=per-workspace" /tmp/baseline-explicit.stderr
# Expected: 0 (operator explicitly opted out; no advisory)
```

**SC-002 verified**: exactly one advisory line in the default-flag case; zero when explicit-off.

## Path C — Graceful degradation on malformed workspace

**Setup**: create a fixture with 3 Go workspaces where workspace 2's `go.mod` is intentionally malformed (first line is garbage instead of `module ...`).

```bash
mkdir -p /tmp/broken-monorepo/{ws1,ws2,ws3}
printf 'module example.com/ws1\n\ngo 1.21\n' > /tmp/broken-monorepo/ws1/go.mod
printf 'GARBAGE-first-line\nmodule example.com/ws2\ngo 1.21\n' > /tmp/broken-monorepo/ws2/go.mod
printf 'module example.com/ws3\n\ngo 1.21\n' > /tmp/broken-monorepo/ws3/go.mod

mikebom sbom scan --path /tmp/broken-monorepo \
  --warm-go-cache=per-workspace \
  --format cyclonedx-json \
  --output cyclonedx-json=/tmp/broken.cdx.json \
  --no-deep-hash

echo "exit-code: $?"
# Expected: 0 (scan completed despite ws2 failure)

jq '.metadata.properties[]? | select(.name == "mikebom:go-cache-warming-failed") | .value | fromjson' /tmp/broken.cdx.json
# Expected: [{"reason":"subcommand-failed","workspace":"ws2"}]
```

**SC-003 verified**: scan exits 0; C119 names exactly the failing workspace with the correct reason class.

## Bonus Path — Concurrency knob

```bash
# Sequential (concurrency=1) on a 4-workspace fixture — expect slower wall-clock
time mikebom sbom scan --path /tmp/four-workspace-monorepo \
  --warm-go-cache=per-workspace \
  --warm-go-cache-concurrency=1 \
  --format cyclonedx-json \
  --output cyclonedx-json=/tmp/seq.cdx.json \
  --no-deep-hash

# Parallel-4 default — expect faster
time mikebom sbom scan --path /tmp/four-workspace-monorepo \
  --warm-go-cache=per-workspace \
  --format cyclonedx-json \
  --output cyclonedx-json=/tmp/par.cdx.json \
  --no-deep-hash

# Both SBOMs MUST be byte-identical modulo timestamps (concurrency is
# not an observable emission-shape variable)
diff <(jq -S 'del(.metadata.timestamp,.serialNumber)' /tmp/seq.cdx.json) \
     <(jq -S 'del(.metadata.timestamp,.serialNumber)' /tmp/par.cdx.json)
# Expected: empty diff
```

**Concurrency-value-invariance** (data-model.md Cross-entity invariant 5) verified: parallel and sequential produce byte-identical annotations.

## Full success criteria table

| SC | Verification | Expected |
|---|---|---|
| SC-001 | Path A jq | `"0"` post-warming vs positive int pre-warming |
| SC-002 | Path B grep -c | `1` on default; `0` on explicit-off |
| SC-003 | Path C jq + exit code | Exit 0; C119 names the failing workspace |
| SC-004 | `git diff main --stat -- 'mikebom-cli/tests/fixtures/golden/**' \| grep -v golang` | No non-Go golden delta |
| SC-005 | Path A `time` | Wall-clock < 60s for the milestone's own test fixture |
| SC-006 | Any SBOM: `jq '.metadata.properties[]? \| select(.name == "mikebom:go-cache-warming-mode") \| .value'` | Returns one of the three enum values on any Go-containing SBOM |
