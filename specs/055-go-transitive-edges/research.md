# Phase 0 Research: Go transitive dependency edges

**Spec**: [spec.md](spec.md) · **Plan**: [plan.md](plan.md) · **Date**: 2026-05-02

Resolves all `NEEDS CLARIFICATION` from Technical Context and the Principle XII Constraint #2 question flagged in Constitution Check. Each entry follows the format **Decision → Rationale → Alternatives considered**.

---

## R1 — HTTP client for proxy `.mod` fetches

**Decision**: Reuse the workspace `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }` pin already declared in `Cargo.toml` (verified via grep at planning time). New code calls `reqwest::Client::new()` once per scan, configures connect/total timeouts per FR-008, and uses async `.get(url).send().await`.

**Rationale**: Already a workspace dep — no new crate. `rustls-tls` feature aligns with Principle I (no C / no OpenSSL). Async HTTP via `tokio` matches the concurrency model needed for the 16-way semaphore (R2). Building on the existing pin avoids accidental version-skew.

**Alternatives considered**:
- `ureq` (sync, smaller): rejected because it forces sync I/O which fights the tokio runtime already used by the rest of `mikebom-cli`. Sync inside async = blocking the runtime; not acceptable for a 500-module workspace.
- `curl` subprocess: rejected — adds an external dependency on the user's `curl` binary, defeats Principle I's "single supply chain," and opens a fork-bomb-shaped attack surface for nothing in return.
- `hyper` directly: rejected — `reqwest` already wraps `hyper`; reaching past it adds boilerplate without changing capability.

---

## R2 — Concurrent fetches: tokio semaphore

**Decision**: Use `tokio::sync::Semaphore::new(16)` as a single global semaphore in `graph_resolver`. Each fetch acquires a permit before issuing the HTTP request and drops it on completion. Fetches are spawned as `tokio::task::spawn`'d futures and joined via `tokio::task::JoinSet` for clean error aggregation.

**Rationale**: Pure-tokio primitive; no extra crate. `JoinSet` is the modern (tokio 1.30+) idiom for "fan out N tasks, await all, collect results." Existing `tokio = { features = ["full"] }` in workspace already includes `sync` and `task::JoinSet`.

**Alternatives considered**:
- `futures::stream::FuturesUnordered`: works but adds the `futures` crate to the call site for one type. Rejected for unnecessary surface.
- A bounded `mpsc` channel (worker pool): more code, no benefit. The semaphore + JoinSet pattern is idiomatic.
- Per-host rate-limiting (Q2 Option E from `/speckit.clarify`): explicitly rejected during clarification. A flat 16 cap is the spec's commitment.

---

## R3 — `go mod graph` subprocess invocation

**Decision**: `tokio::process::Command::new("go").args(["mod", "graph"]).current_dir(&workspace_root).output().await`, wrapped in `tokio::time::timeout(Duration::from_secs(30), ...)` per FR-007. Non-zero exit code or timeout → fall through to step 2 with a `tracing::warn`.

**Rationale**: Async-first to integrate with the resolver's tokio runtime. The existing `git describe` invocation at `mikebom-cli/src/scan_fs/package_db/golang.rs:733` uses **synchronous** `std::process::Command` — this is fine in 053's blocking context, but the 055 resolver runs inside an async function (it's invoked from a chain that is already `async fn`), and using sync `Command` would block the executor.

**Alternatives considered**:
- `std::process::Command` (matching 053's pattern): rejected because the surrounding code in `golang.rs::read()` is already async — sync `Command` blocks the executor for up to 30 s on a worst-case `go mod graph`.
- Spawn `std::process::Command` on `tokio::task::spawn_blocking`: works but adds a layer of indirection for no benefit over `tokio::process::Command`.
- Skip step 1 entirely: rejected — it's the most reliable step and is free when `go` is present.

---

## R4 — Detecting whether `go` is on PATH

**Decision**: Probe-and-fail: attempt `tokio::process::Command::new("go").arg("version").output().await`. If `Err(io::Error)` with kind `NotFound`, `go` is not on PATH; record this in a one-shot lazy `OnceCell<bool>` for the scan and skip step 1 unconditionally.

**Rationale**: Zero new dependencies. Simpler than adding the `which` crate. `go version` is the canonical "is Go installed?" probe and exits in <50 ms; running it once per scan is free.

**Alternatives considered**:
- `which` crate: rejected because it requires adding a new dep for a one-line probe. The spec explicitly favors no-new-crate (Assumptions section).
- Read `$PATH` and search manually: re-implements `which` badly; cross-platform PATHEXT handling on Windows is a footgun.
- Cache the result across scans: rejected — `go` could be installed mid-session (rare, but harmless to recheck).

---

## R5 — `GOPRIVATE` glob matching

**Decision**: Implement Go's glob algorithm in `goprivate.rs` (~50 LOC). Algorithm per Go's `cmd/go/internal/module/module.go::MatchPrefixPatterns`:

1. Split `GOPRIVATE` on commas; each entry is a pattern.
2. For each pattern, split on `/` into segments. The pattern matches a module path `M` if:
   - `M` begins with the pattern's literal prefix (segments without `*`), AND
   - any pattern segment containing `*` matches the corresponding `M` segment via shell-glob semantics (`*` matches any characters except `/`).
3. The module is private if ANY pattern matches.

Pure-Rust implementation; unit tests cover Go's documented examples (`*.corp.example.com`, `github.com/our-org/*`, `gopkg.in/foo.*`).

**Rationale**: No existing Rust crate replicates Go's specific glob semantics for `GOPRIVATE` (this is a Go-internal algorithm, not standard shell globbing). Implementing it ourselves is ~50 LOC and a unit test matrix; depending on a third-party globbing crate (`glob`, `globset`) doesn't help because Go's semantics differ slightly from POSIX globs.

**Alternatives considered**:
- `globset` crate: rejected — POSIX-glob semantics differ from Go's pattern semantics in edge cases (e.g., `*` matching across `/` boundaries differs).
- Shell out to `go env GOPRIVATE` and parse: rejected — tautologically requires `go` to be installed, which defeats the whole point of the offline-without-go path.
- Treat `GOPRIVATE` as a comma-separated literal-prefix list (no globs): rejected — would silently misbehave for the common `*.corp.example.com` pattern.

---

## R6 — Module-path proxy escape rules

**Decision**: Implement Go's escape algorithm in `proxy_fetch.rs::escape_module_path()`. Algorithm per `cmd/go/internal/module/module.go::EscapePath`:

- Each lowercase ASCII letter, digit, `.`, `-`, `_`, `~`, `/`, `+` passes through.
- Each uppercase ASCII letter `X` becomes `!x` (lowercase, prefixed with `!`).
- Other characters cause an error (return `Err`).

Examples (covered by unit tests):
- `github.com/Azure/azure-sdk-for-go` → `github.com/!azure/azure-sdk-for-go`
- `gopkg.in/yaml.v2` → `gopkg.in/yaml.v2` (unchanged)
- `github.com/SAP/go-hdb` → `github.com/!s!a!p/go-hdb`

The version is escaped with the same algorithm (e.g., `v2.0.0+incompatible` → `v2.0.0+incompatible` unchanged; pseudo-versions like `v0.0.0-20211123-abcd1234abcd` unchanged).

**Rationale**: ~30 LOC pure mechanical transformation. The rule is documented and stable since Go modules launched (1.11). Implementing it ourselves is the canonical approach; depending on a `go`-binary call to escape is circular.

**Alternatives considered**:
- `urlencoding` / `percent-encoding` crates: rejected — Go's escape is NOT URL-encoding; uppercase-`!` is a Go-specific scheme that no general crate implements.
- Using `cargo`'s URL-escape utilities: same problem.

---

## R7 — `GOPROXY` parsing

**Decision**: Parse `$GOPROXY` per Go's documented semantics:

1. Default value when unset: `https://proxy.golang.org,direct`.
2. Split on `,` (fall-through on HTTP 404 or 410 only) and `|` (fall-through on any error). Both separators may appear in the same string.
3. Special tokens:
   - `direct`: source-VCS fetch — out of scope for 055 (per spec Assumptions). Treat as "skip remaining proxies, fall through to ladder step 4."
   - `off`: no fetching allowed. Disable step 3 entirely for the scan.
4. For each proxy URL, attempt the fetch; on a fall-through error per the separator semantics, advance to the next entry.

**Rationale**: Honors the user's environment. A user who configures `$GOPROXY=https://internal-proxy.corp,direct` for their `go` toolchain expects mikebom to use the same chain. Reusing this convention is the "no surprise" path.

**Alternatives considered**:
- Hardcode `proxy.golang.org` only: rejected — breaks any user with a corporate proxy or `direct` requirement.
- Read via `go env GOPROXY` subprocess: rejected (circular: requires `go` installed) — `std::env::var("GOPROXY")` is the right zero-dep call.

---

## R8 — Per-edge SBOM provenance (Principle XII Constraint #2)

**Decision**: **Align with milestone 053 precedent — no per-edge SBOM `evidence` annotations in 055.** The FR-009 `tracing::info` ladder summary is the documented provenance mechanism. SBOM consumers see uniform `dependsOn` edges regardless of which ladder step produced them; operators and auditors get per-scan provenance via the structured log line.

Rationale for this reading of Constraint #2:
- Constraint #2's text reads: "Data from external sources MUST be annotated with its provenance (e.g., 'relationship from Cargo.lock', 'license from deps.dev')." The example uses *kind-of-source* granularity ("Cargo.lock", "deps.dev"), not *per-edge* granularity. A scan-level "ladder summary" line satisfies the spirit at the same granularity.
- Milestone 053 (`spec.md` FR-005, shipped) emits cache-derived direct edges with no per-edge SBOM annotation. The 053 PR (#105) merged with the constitution at v1.4.0 in effect. This establishes that scan-mode static-analysis paths satisfy Constraint #2 via tracing logs, not per-edge SBOM properties.
- Adding per-edge `evidence` fields to potentially thousands of edges per SBOM would bloat output 5–10× for marginal information gain; the same provenance is reconstructable from the scan log.
- If a future consumer requirement emerges for per-edge provenance (e.g., a regulatory requirement that supply-chain edges be source-attributed), a follow-up milestone can add CDX `evidence` carrying `bom-ref`-anchored provenance — at that point the work touches 049/053/055 uniformly, not 055 alone.

**Alternatives considered**:
- Add `evidence` per edge naming the ladder step: rejected for the bloat reason above and for inconsistency with 053.
- Add `evidence` ONLY for proxy-fetched (step 3) edges: tempting (it's the "external network" step), but creates asymmetric SBOM output where some edges have provenance and others don't — confusing for consumers, and the asymmetry implies step-1/2 edges are "more trustworthy" which isn't true (all three steps converge on the same answer for a given module-version).
- Defer entirely to a follow-up milestone: rejected — the question must be answered before 055 ships, even if the answer is "no change." Documenting the answer here is the answer.

---

## R9 — Wiremock for hermetic test fixtures

**Decision**: Add `wiremock = "0.6"` as a `[dev-dependencies]` entry on `mikebom-cli/Cargo.toml` (NOT a workspace dep — keep its blast radius minimal). Use it in:
- New unit tests in `proxy_fetch.rs::tests` covering: 200-OK happy path, 404 fall-through, 5xx retry-then-fail, timeout, malformed `.mod` body.
- New integration test `mikebom-cli/tests/go_transitive_edges.rs` (FR-012) — start a `MockServer`, populate with synthesized `.mod` responses for the `argo-style-no-cache/argo-workflows/` fixture's `go.sum` entries, point `$GOPROXY` at the mock URL, run a full `mikebom sbom scan`, assert on the resulting SBOM's edge set.

**Rationale**: `wiremock` is the de-facto Rust equivalent of WireMock (Java); it's mature (4+ years), tokio-native, requires no extra setup, and integrates with `cargo test` cleanly. Pure-Rust (Principle I), pure-rustls (Principle I), no C deps. Dev-dep status means it never ships in the production binary.

**Alternatives considered**:
- `httpmock`: similar capability; `wiremock` chosen for slightly broader use in the Rust async ecosystem and clearer "mount on path" API.
- Hand-rolled `tokio::net::TcpListener` HTTP stub: ≥150 LOC of test infra that re-implements wiremock badly. Rejected on maintenance grounds.
- `mockito`: older, less idiomatic with tokio. Rejected.
- Real `proxy.golang.org` calls in tests: VIOLATES Principle VII (test isolation) and is flake-prone. Hard rejection.

If reviewers reject the new dev-dep during PR review, we fall back to a hand-rolled `tokio::net::TcpListener` mock. This is documented in the plan's Complexity Tracking row.

---

## R10 — Per-scan dedup of in-flight resolutions

**Decision**: The resolver maintains a `HashMap<ModuleId, ModuleGraphEntry>` keyed by `(module-path, version)`. Step 1 (`go mod graph`) populates it in one shot. Steps 2–3 are invoked only for `(module, version)` pairs missing from the map AND present in `go.sum`. A given pair is fetched at most once per scan even if it appears as a `require` of many parents.

**Rationale**: Linear in `|go.sum|`; no quadratic blowup. The same pair appears as a require in many parents' `go.mod` files (a popular utility module gets required by dozens of others) — without dedup we'd issue dozens of identical fetches. With dedup, every pair is fetched at most once.

**Alternatives considered**:
- Per-parent fetch (no dedup): rejected on perf grounds — for large workspaces this is 10–100× more requests.
- Persistent on-disk dedup cache: explicitly rejected by spec Q3 clarification (no on-disk cache).

---

## R11 — Integration test fixture choice

**Decision**: Reuse `tests/fixtures/go/argo-style-no-cache/argo-workflows/` (committed in milestone 053 specifically because its `go.sum` has 14 direct requires + non-trivial transitive closure and was the original #102 reproduction case). The new integration test:

1. Sets `PATH` to a directory not containing `go` (or stubs `go` to a script that exits non-zero) — exercises the no-`go` path.
2. Sets `GOMODCACHE` to a fresh empty `tempfile::tempdir()` — exercises the empty-cache path.
3. Starts a `wiremock::MockServer`, populates it with `.mod` responses derived from a small local seed directory under `tests/fixtures/go/argo-style-no-cache/proxy-mock/` (synthesized minimal `go.mod` files for the modules in `argo-workflows/go.sum`).
4. Sets `GOPROXY` to the mock server's URL.
5. Runs `mikebom sbom scan --path <fixture>` (no `--offline`).
6. Asserts: SBOM contains transitive edges between `go.sum` modules; ratio ≥ 90% per SC-001.

**Rationale**: The fixture already exists (committed, schema-stable). Adding a sibling `proxy-mock/` directory is the smallest delta. Synthesizing minimal `go.mod` files for the proxy mock is preferable to checking in real third-party project files (size, license, drift).

**Alternatives considered**:
- Use `tests/fixtures/go/simple-module/`: too small (5 direct + 5 indirect) for SC-001's 90% ratio assertion to be statistically meaningful.
- Clone knative/func live (per milestone 054 pattern): heavyweight for a unit-level integration test; FR-012 is for the smaller hermetic case, knative/func is for the SC-003 realistic-project gate (US3).
- Use real proxy.golang.org: violates Principle VII.

---

## R12 — Replace directive transitive application

**Decision**: Apply the workspace `go.mod`'s `replace` directives globally during edge resolution, not just for the main-module's direct edges (the 053 behavior).

Concrete: when looking up `(M, V)`'s `go.mod` (steps 2–3), first check the workspace replace map: if `(M, V)` is replaced by `(M', V')`, fetch `(M', V')`'s `go.mod` instead, and treat that file as the source of `(M, V)`'s requires. Step 1 (`go mod graph`) handles this natively — Go applies replaces before emitting graph output.

**Rationale**: Spec FR-006 mandates this. The 053 implementation already has the replace map at hand (`apply_replace_and_exclude` at `golang.rs:475`). Threading it through the resolver is straightforward.

**Alternatives considered**:
- Skip transitive replace: rejected — directly violates FR-006 and would produce wrong edges in any workspace using replaces (which is most large workspaces).
- Apply replace per-module by re-reading the replaced module's own go.mod's replaces: rejected — Go semantics: ONLY the main module's replaces apply to the build. Transitive modules' own replaces are ignored. Mikebom matches Go's behavior.

---

## R13 — `go.mod` parsing reuse

**Decision**: Reuse the existing `parse_go_mod()` parser at `mikebom-cli/src/scan_fs/package_db/golang.rs:159` (which produces `GoModDocument` with `module_path`, `requires`, `replaces`, `excludes`). The proxy-fetched `.mod` files have the same format; the parser handles them as-is.

**Rationale**: Zero new parsing code. The existing parser has been hardened over milestones 049, 053, 054.

**Alternatives considered**:
- Use a third-party `gomod` Rust crate: none mature enough; the existing in-house parser already covers our needs.

---

## R14 — Error class taxonomy for FR-008 fetch failures

**Decision**: Classify each proxy fetch result into one of:

| Class | Trigger | `tracing::warn` field name |
|-------|---------|----------------------------|
| `Timeout` | reqwest's connect or total timeout fired | `error_class="timeout"` |
| `Http4xx` | HTTP status 400–499 (other than 404) | `error_class="http_4xx"`, `status=<code>` |
| `Http404` | HTTP status 404 (special — fall-through per `,` semantics) | `error_class="http_404"` |
| `Http5xx` | HTTP status 500–599 | `error_class="http_5xx"`, `status=<code>` |
| `Dns` | DNS resolution failure | `error_class="dns"` |
| `Connection` | TCP connect refused / reset | `error_class="connection"` |
| `Tls` | TLS handshake failure | `error_class="tls"` |
| `Parse` | `.mod` body fails to parse | `error_class="parse"` |
| `Other` | anything else | `error_class="other"`, `error=<display>` |

**Rationale**: Operator-friendly tracing. A flood of `error_class=connection` says "your proxy is unreachable"; a flood of `error_class=parse` says "the proxy is returning HTML or junk"; a flood of `error_class=http_404` for `GOPRIVATE`-shaped paths says "you forgot to set GOPRIVATE." Structured categories beat opaque error messages.

**Alternatives considered**:
- One opaque `error=<display>` field: harder to grep/aggregate at scale.
- More granular classes (e.g., split http_5xx by code): premature; we can refine if the data tells us to.

---

## Open questions (NONE)

All Technical Context unknowns and the Constitution Check XII flag are resolved above. Plan Phase 1 may proceed.
