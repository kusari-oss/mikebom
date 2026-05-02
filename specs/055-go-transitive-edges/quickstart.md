# Quickstart: Go transitive dependency edges

**Spec**: [spec.md](spec.md) · **Plan**: [plan.md](plan.md)

How to develop, test, and verify milestone 055 work locally.

---

## Run the integration test (FR-012)

The headline test reproduces the issue #102 residual gap: `go` not on PATH, empty `$GOMODCACHE`, scan still produces transitive edges via the proxy ladder.

```bash
# From repo root
cargo +stable test -p mikebom --test go_transitive_edges -- --nocapture
```

Expected output: scan produces a SBOM whose `dependencies[].dependsOn` (CDX) or equivalent (SPDX) contains transitive edges between `go.sum` modules. The test asserts ≥ 90% of `go.sum` modules with non-empty `requires` get at least one outgoing edge.

The test is hermetic — it spins up a local `wiremock::MockServer` and points `$GOPROXY` at it. No real network calls.

---

## Manually exercise each ladder step

### Step 1: `go mod graph` (preferred path, when `go` installed)

```bash
cd tests/fixtures/go/argo-style-no-cache/argo-workflows
go mod graph | head -20
```

Verify mikebom matches:

```bash
RUST_LOG=mikebom_cli::scan_fs::package_db::golang=debug \
  cargo +stable run -p mikebom --release -- sbom scan \
  --path tests/fixtures/go/argo-style-no-cache/argo-workflows \
  --format spdx-2.3-json --output /tmp/argo.spdx.json
```

Look for the `tracing::info` summary line: `go transitive edges: ladder=[graph:N1, cache:N2, proxy:N3, missing:N4]`. With `go` installed and the project's go module cache populated, expect `graph:>0`, `cache:0`, `proxy:0`, `missing:0`.

### Step 2: `$GOMODCACHE` walk (current 053 path)

Force step 1 unavailable by hiding `go`:

```bash
PATH=/usr/bin:/bin \  # adjust if go is in /usr/local/bin or similar
  RUST_LOG=mikebom_cli=debug \
  cargo +stable run -p mikebom --release -- sbom scan \
  --path tests/fixtures/go/argo-style-no-cache/argo-workflows \
  --format spdx-2.3-json --output /tmp/argo.spdx.json
```

Pre-populate the cache with `go mod download` against the fixture (one-time):

```bash
cd tests/fixtures/go/argo-style-no-cache/argo-workflows
GOMODCACHE="$(pwd)/.mikebom-test-cache" go mod download
```

Then point mikebom at it:

```bash
GOMODCACHE="$(pwd)/.mikebom-test-cache" \
  PATH=/usr/bin:/bin \
  cargo +stable run -p mikebom --release -- sbom scan \
  --path tests/fixtures/go/argo-style-no-cache/argo-workflows \
  --offline --format spdx-2.3-json --output /tmp/argo.spdx.json
```

Expect: `ladder=[graph:0, cache:N>0, proxy:0, missing:0]` (or low `missing` for a complete cache).

### Step 3: Proxy fetch (the 055 differentiator)

Start the wiremock server (or use real proxy.golang.org for ad-hoc verification — NOT for CI):

```bash
# Real proxy (interactive verification only — never in CI):
PATH=/usr/bin:/bin \
  GOMODCACHE=/tmp/empty-mikebom-cache \
  RUST_LOG=mikebom_cli=debug \
  cargo +stable run -p mikebom --release -- sbom scan \
  --path tests/fixtures/go/argo-style-no-cache/argo-workflows \
  --format spdx-2.3-json --output /tmp/argo.spdx.json
```

Expect: `ladder=[graph:0, cache:0, proxy:N>0, missing:M]` where M is the count of modules whose proxy fetch failed (private modules, retracted versions, transient errors).

### Step 4: Graceful no-edges fallthrough

```bash
PATH=/usr/bin:/bin \
  GOMODCACHE=/tmp/empty-mikebom-cache \
  cargo +stable run -p mikebom --release -- sbom scan \
  --path tests/fixtures/go/argo-style-no-cache/argo-workflows \
  --offline \
  --format spdx-2.3-json --output /tmp/argo.spdx.json
```

Expect: `ladder=[graph:0, cache:0, proxy:0, missing:N>0]`. Components emit but transitive edges are empty (the main-module's direct edges from 053 are still present). The scan completes without error per FR-005.

---

## Inspect the output

```bash
# Count transitive edges in the SPDX 2.3 output
jq '[.relationships[] | select(.relationshipType == "DEPENDS_ON")] | length' /tmp/argo.spdx.json

# Spot-check a specific module's outgoing edges
jq '.relationships[] | select(.relationshipType == "DEPENDS_ON" and (.spdxElementId | contains("argoproj")))' /tmp/argo.spdx.json
```

For CDX output:

```bash
jq '[.dependencies[] | .dependsOn // [] | length] | add' /tmp/argo.cdx.json
```

---

## Pre-PR verification (MANDATORY per Constitution v1.4.0)

Before opening or updating the PR for 055:

```bash
./scripts/pre-pr.sh
```

This runs both required gates in order:

1. `cargo +stable clippy --workspace --all-targets -- -D warnings` — zero errors AND zero warnings
2. `cargo +stable test --workspace` — every suite reports `ok. N passed; 0 failed`

Both MUST pass. A passing per-crate `cargo test -p mikebom` is NOT sufficient evidence of CI-readiness.

---

## Run the milestone's specific test set

```bash
# Unit tests for the resolver pieces
cargo +stable test -p mikebom golang::module_id
cargo +stable test -p mikebom golang::go_mod_graph
cargo +stable test -p mikebom golang::proxy_fetch
cargo +stable test -p mikebom golang::goprivate
cargo +stable test -p mikebom golang::graph_resolver

# Integration test
cargo +stable test -p mikebom --test go_transitive_edges
```

---

## Common debugging hooks

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| `ladder=[graph:0, cache:0, proxy:0, missing:N]` with `go` installed | `--offline` is set unintentionally; or `go mod graph` exited non-zero | Drop `--offline`; check `cd <fixture> && go mod graph 2>&1 \| head` for upstream error |
| Many `error_class=http_404` warnings | Module path escape rule mismatch (uppercase letters not lowercased correctly) | Add a unit test in `proxy_fetch::tests::escape_module_path` for the offending module |
| `error_class=parse` warnings | Proxy returning HTML (corporate proxy login redirect) | Check `$GOPROXY`; the user may need `$GOPROXY=https://proxy.golang.org,direct` |
| Test hangs on `cargo test --test go_transitive_edges` | Mock server not started or mocks not registered for some `go.sum` entry | Run with `RUST_LOG=wiremock=debug,mikebom_cli=debug` and look for unmatched requests |
| Scan time regressed >15% on existing fixtures | Sequential proxy fetches (semaphore not wired) or `go mod graph` runs even when irrelevant | Profile with `--release` and `tracing` enabled; check the `ladder=` summary on each fixture |

---

## What NOT to do (constitutional / spec hard limits)

- **Do NOT add a `--go-resolve-online` or `--go-fetch-concurrency` flag** — Q1 and Q2 clarifications locked these as no-flag commitments. Add them in a follow-up if needed.
- **Do NOT cache fetched `.mod` files to disk** — Q3 clarification.
- **Do NOT add `mikebom:*` properties to the SBOM output** — FR-010 + Constitution Principle V.
- **Do NOT use `.unwrap()` in production code** — Constitution Principle IV; pre-PR clippy gate enforces.
- **Do NOT skip `--all-targets` in clippy** — pre-PR gate uses `--workspace --all-targets`; this catches `unwrap_used` inside test modules too.
- **Do NOT contact real `proxy.golang.org` from any test** — Principle VII; CI runs would flake. Use `wiremock`.
