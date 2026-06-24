# Research — milestone 141 Erlang/OTP rebar reader (Phase 0)

Resolves all Technical Context unknowns before Phase 1 design. Decisions are
either inherited from the milestone-140 (Elixir/Mix) audit (Hex.pm is the
shared registry; the purl-spec audit and the brace-counted tokenizer carry
over identically) or are Erlang-specific (rebar.lock term shape variants,
`*.app.src` keyword family, OTP-version compatibility of `optional_applications:`).

## R1 — PURL spec audit for Erlang Hex deps (inherited verbatim from milestone 140)

**Decision**: The `hex` PURL type is purl-spec-blessed
([hex-definition.md](https://github.com/package-url/purl-spec/blob/main/types-doc/hex-definition.md)).
All Erlang Hex deps emit `pkg:hex/<lc-name>@<version>` for default-org
deps. Private-org deps emit `pkg:hex/<org>/<lc-name>@<version>?repository_url=https://repo.hex.pm`.
The git-source case uses `pkg:generic/<name>@<sha>?vcs_url=git+<url>` (purl-spec
does NOT bless `vcs_url=` on the `hex` type).

**Rationale**: Hex.pm serves BOTH Elixir AND Erlang packages (rebar3 publishes
to Hex via the `rebar3 hex publish` plugin). The same PURL type covers both
ecosystems — an Erlang project depending on an Elixir-published package or
vice versa emits identical PURLs. The milestone-140 Phase 0 correction
audited the purl-spec hex-definition exhaustively and codified the private-org
namespace-as-org + `repository_url=` qualifier pattern; reusing it verbatim
maintains cross-ecosystem identity consistency.

**Alternatives considered**:
- Introducing a separate `pkg:erlang/<name>@<version>` type — REJECTED. Not
  purl-spec-blessed; would split package identity across ecosystems for
  packages that intentionally serve both. Hex.pm is the registry of record.
- Inventing `mikebom:hex-repo` annotation for private orgs (the original
  pre-milestone-140-Phase-0 design) — REJECTED. The purl-spec already blesses
  the namespace-as-org + `repository_url=` qualifier shape.
- Emitting git-source deps as `pkg:hex/<name>@<sha>?vcs_url=git+<url>` —
  REJECTED. The `hex` type doesn't admit `vcs_url=` qualifiers per the
  spec definition. The `pkg:generic/<name>@<sha>?vcs_url=git+<url>` shape
  is the established cross-ecosystem fallback (used by composer git-source,
  cocoapods git-source, elixir git-source).

## R2 — rebar.lock term-syntax shape variants

**Decision**: The reader recognizes THREE pinned-dep shapes per `rebar.lock`
schema versions, dispatching by regex pattern match:

1. **Modern Hex (rebar3 3.7+, ~2018-present)**:
   ```erlang
   {<<"<name>">>, {pkg, <<"<name>">>, <<"<version>">>}, <depth>}.
   {<<"<name>">>, {pkg, <<"<name>">>, <<"<version>">>, <<"<sha256-hex>">>}, <depth>}.
   ```
   Extract: name (lowercased), version, optional inner SHA-256.

2. **Modern Hex with private-org map-form (rebar3 3.13+, ~2020-present)**:
   ```erlang
   {<<"<name>">>, {pkg, <<"<name>">>, <<"<version>">>, <<"<sha256-hex>">>, #{repo => <<"hexpm:<org>">>}}, <depth>}.
   ```
   Extract: name, version, inner SHA-256, repo string. The `hexpm:<org>` prefix
   identifies a private-Hex-org dep; bare `<org>` (without `hexpm:` prefix) is
   also accepted as a defensive fallback per rebar3's documented flexibility.

3. **Legacy Hex (pre-rebar3 3.7)**:
   ```erlang
   {<<"<name>">>, <<"<version>">>, <depth>}.
   ```
   Extract: name, version. No inner SHA-256.

4. **Git**:
   ```erlang
   {<<"<name>">>, {git, "<url>", {ref, "<sha>"}}, <depth>}.
   {<<"<name>">>, {git, "<url>", {tag, "<tag>"}}, <depth>}.
   {<<"<name>">>, {git, "<url>", {branch, "<branch>"}}, <depth>}.
   ```
   Extract: name, URL, resolved-ref (the `{ref, "..."}` form is the rebar3
   canonical pinned-form; `{tag, ...}` / `{branch, ...}` forms appear when
   the original `rebar.config` used them, but rebar3 may still resolve them
   to a SHA at lock-write time. The lockfile is the authoritative source
   for whatever ref shape it persists).

**Rationale**: rebar3's lockfile has accreted three shape variants over ~10 years
of releases. All three remain in the wild — modern projects on rebar3 3.13+
use the map-form; mid-vintage projects (2018–2020) use the 4-element `{pkg, ...}`
without map; legacy projects (some still-maintained libraries) use the
flat `{<<name>>, <<version>>}` form. The reader handles all three so
mikebom doesn't silently drop deps based on rebar3 version.

**Alternatives considered**:
- Recognizing only the modern shape (rebar3 3.13+ map-form) — REJECTED. Cuts
  out a meaningful share of in-the-wild rebar3 projects per Hex.pm
  package-publish telemetry.
- Recognizing only modern + legacy (no map-form) — REJECTED. Private-org
  identification requires the map-form's `repo` field; dropping it would
  silently misclassify private-org deps as default-Hex deps and emit
  incorrect PURLs.
- Implementing a full Erlang-term parser — REJECTED. Regex dispatch is
  sufficient for these shape-bounded patterns (each shape is uniquely
  identified by its tuple arity + position of literal markers like `pkg`/`git`);
  see R4 for tokenizer reuse from milestone 140. Full Erlang-term parsing
  would add complexity for zero accuracy gain on the bounded vocabulary
  rebar.lock uses.

## R3 — `*.app.src` keyword family + OTP-version compatibility

**Decision**: The reader extracts FOUR keyword lists from each `*.app.src`:

1. `{vsn, "<version>"}` — application version string (required).
2. `{applications, [<atom-list>]}` — REQUIRED dep atoms (OTP supervisor must
   start these before the app); present since OTP 1.0.
3. `{included_applications, [<atom-list>]}` — embedded sub-apps that share
   the parent's supervision tree; present since OTP R6 (early 2000s).
4. `{optional_applications, [<atom-list>]}` — soft deps the supervisor tries
   but does not fail on; ADDED IN OTP 26 (2023).

Per Q3, the main-module's `depends` set unions all three runtime lists +
`rebar.config::{deps, [...]}`. Each derived edge-target carries
`mikebom:erlang-app-dep-kind = "required" | "included" | "optional"`
(precedence required > included > optional when an atom appears in
multiple sets).

**Rationale**: A reader that only parses `applications:` would silently lose
real deps on OTP 25+ libraries using `optional_applications:` (a growing
pattern — e.g., libraries that integrate with `telemetry` when present
but don't require it) AND on embedded-app releases using `included_applications:`
(a long-standing OTP pattern for compile-time-merged sub-apps). Adding
the two extra keywords to the regex extractor is a zero-complexity addition
(the syntax is identical — atom-list inside a tuple). The
`mikebom:erlang-app-dep-kind` annotation discriminates which keyword
sourced the edge so operators filtering for "hard runtime deps only"
can do `mikebom:erlang-app-dep-kind == "required"` via the standard
property filter — a feature that mikebom is uniquely positioned to
emit (per R6: syft/trivy don't parse `*.app.src`).

**OTP version compatibility note**: The reader extracts `optional_applications:`
unconditionally; on OTP-25-and-earlier projects the keyword simply won't
be present in the descriptor, so the reader will see an empty union
contribution — no warning fires (absent keyword is a valid OTP application
descriptor). This makes the reader future-compatible without requiring
OTP-version sniffing.

**Alternatives considered**:
- Only parse `applications:` — REJECTED. See Q3 spec discussion.
- Parse `applications:` + `included_applications:` but defer `optional_applications:`
  to v1.1 (Option C in Q3) — REJECTED. The pattern is trivially uniform
  (atom-list inside tuple), there's no incremental cost, and OTP 26 has
  been generally available since 2023 — libraries using `optional_applications:`
  are already shipping in production.
- Treat `included_applications:` deps as NOT-separate-components (Option D
  in Q3 — embedded apps as parent-internal) — REJECTED. Embedded apps
  still have independent provenance (separate Hex.pm publish, separate
  license, separate version) and consumers need to see them in the SBOM
  for vulnerability scanning. The annotation `mikebom:erlang-app-dep-kind = "included"`
  preserves the semantic distinction without hiding the component.

## R4 — Brace-counted tokenizer reuse from milestone 140

**Decision**: Reuse the brace-counted-tokenizer pattern from
`mikebom-cli/src/scan_fs/package_db/elixir.rs::tokenize_mix_lock`. The Erlang
term syntax of `rebar.lock` uses identical `{...}` nesting; only the
top-level grammar differs (Elixir's `%{}` map literal wraps the
pinned-deps in mix.lock vs Erlang's nested-tuple list in rebar.lock).

**Rationale**: The brace-counting state machine — which tracks depth across
multi-line tuples so a top-level tuple containing nested `{git, "url", {ref, "sha"}}`
parses as ONE entry rather than multiple — is the exact same algorithm.
Milestone 140's tokenizer is battle-tested across the 22 unit tests in
`elixir.rs`; reusing it as a private helper inside `erlang.rs` (or
factoring it to a shared `package_db/brace_tokenizer.rs` if review
prefers — decision deferred to implementation review) avoids reinventing
the parser.

**Rationale (continued)**: The Erlang-vs-Elixir top-level grammar
distinction is shallow:
- mix.lock entries: `"<name>": {:hex, ...},` (Elixir map syntax)
- rebar.lock entries: `{<<"<name>">>, {pkg, ...}, depth}.` (Erlang term syntax)

The brace-counting logic is identical inside each entry; only the
top-level entry boundary detection differs (`,` between map-entries vs
`.` between Erlang term sentences).

**Alternatives considered**:
- Factor the brace-counter to `mikebom-cli/src/scan_fs/package_db/brace_tokenizer.rs`
  as a shared module immediately — DEFERRED. The current milestone scope is
  one-reader-add; shared-module extraction is a refactor that would touch
  both elixir.rs and erlang.rs. Defer until a third consumer materializes
  (e.g., a future Gleam-on-BEAM reader); meanwhile, copy the helper to
  `erlang.rs` with a `// Shape mirrors elixir.rs::tokenize_mix_lock; factor to shared module when a 3rd ecosystem needs it` comment.
- Use a full Erlang-term parser library — REJECTED. No pure-Rust Erlang-term
  parser exists with the required maintenance posture (zero new Cargo deps
  is a constraint per spec + plan). The bounded vocabulary of rebar.lock
  doesn't justify the dependency.

## R5 — `mikebom:source-type` prefix convention

**Decision**: Erlang-derived components carry `mikebom:source-type` annotation
values prefixed `erlang-`: `erlang-hex`, `erlang-git`, `erlang-otp-runtime`,
`erlang-main-module`. This distinguishes them from milestone-140's
`hex-hex`, `hex-git`, `hex-path`, `hex-main-module` Elixir-derived values
even when both readers emit `pkg:hex/<name>@<version>` for hex deps.

**Rationale**: Even though Erlang AND Elixir Hex packages share `pkg:hex/`
identity (per R1), the source-tier annotation needs to distinguish which
reader produced the component for traceability + debugging. Per the
established cross-milestone convention (`kmp-` milestone 122, `pub-`
milestone 137, `composer-` milestone 138, `cocoapods-` milestone 139,
`hex-` milestone 140), each new reader gets its own prefix. Erlang gets
`erlang-`.

**Cross-reader interaction**: when an operator's project has BOTH `mix.exs`
(Elixir/Mix) AND `rebar.config` (a "polyglot BEAM" project — rare but
legal), both readers fire. Same-PURL components dedupe via standard
`seen_purls` logic. The `mikebom:source-type` annotation reflects whichever
reader emitted the canonical component first (deterministic by alphabetical
reader order in `read_all` dispatcher — `elixir` < `erlang` so elixir
typically wins; this matches the milestone-140 dedup posture).

**Alternatives considered**:
- Use `hex-*` prefix for both readers — REJECTED. Loses provenance
  discrimination; operator-visible debugging signal degrades.
- Use ecosystem-specific prefix only for the source kind that differs
  (e.g., `erlang-otp-runtime` but bare `hex` for Hex.pm deps) — REJECTED.
  Inconsistent prefix style; the cross-milestone convention is "every
  source-type value uses the reader's prefix."

## R6 — syft/trivy parity surface

**Decision**: Neither syft nor trivy parses `*.app.src` keyword lists
or the OTP-runtime-libs surface. Both parse `rebar.lock` for Hex deps
and emit `pkg:hex/<name>@<version>` PURLs (with empty namespace +
no qualifiers — same posture as their Elixir support per milestone-140
R6). mikebom's emission for the Hex-package subset is structurally
identical (and more spec-correct re private-org `repository_url=`
qualifier).

**Mikebom-unique surfaces** (deferred compatibility annotations):
- OTP runtime libs from `*.app.src::applications:` — neither tool emits.
- `mikebom:erlang-app-dep-kind` (required/included/optional discrimination).
- `mikebom:otp-stdlib = "true"` allowlist supplementary marker.
- `*.app.src` main-module components (PURL `pkg:hex/<app>@<vsn>` derived
  from `{application, <atom>, [{vsn, "..."}, ...]}.`).

**Rationale**: Per the established cross-milestone convention, mikebom's
"more spec-correct than syft/trivy" posture is preserved. Compatibility
`mikebom:also-known-as` annotations for the Hex-package subset (so
downstream consumers can correlate mikebom output with syft/trivy output)
are deferred to v1.1 per the milestone-140 Out-of-Scope precedent.

**Memory note**: Per persistent memory `feedback_native_fields_first.md`
and Constitution Principle V, the `mikebom:erlang-app-dep-kind` annotation
was audited against existing standards-native constructs:
- CycloneDX `scope` field — values `required`/`optional`/`excluded` —
  REJECTED as the carrier because its semantic is "is this dep required
  for the application to function" (BUILD/RUNTIME orthogonal axis), NOT
  "which OTP application-descriptor keyword sourced this edge."
- SPDX 2.3 `DEV/BUILD/TEST_DEPENDENCY_OF` — REJECTED for the same reason
  (these are lifecycle-scope discriminators, not OTP-keyword-family).
- SPDX 3 `LifecycleScopeType` — REJECTED for the same reason.

The OTP `applications:` / `included_applications:` / `optional_applications:`
distinction is a runtime-startup-behavior axis (does the OTP supervisor
require this app to start? does it embed it into the parent's
supervision tree? does it try-but-not-fail?), which is orthogonal to
"is this dev/build/test/runtime scope." A future operator may legitimately
have an `optional_applications:` dep that is ALSO production-scope (the
`telemetry` integration example) — the two axes must coexist.

Per Principle V, the parity-bridge annotation `mikebom:erlang-app-dep-kind`
will be documented in `docs/reference/sbom-format-mapping.md` during
implementation with the justification clause above. (Implementation note:
this is one of the doc-touch items in tasks.md.)

**Alternatives considered**:
- Encode the discrimination via CycloneDX `scope` field instead of a
  custom annotation — REJECTED per the audit above.
- Skip the discrimination entirely and emit all three keyword families
  as undifferentiated `depends` edges — REJECTED. Loses
  operator-actionable signal (per Q3 — filter to hard-deps-only via
  `mikebom:erlang-app-dep-kind == "required"`).

## R7 — Regex compile-once via `std::sync::OnceLock`

**Decision**: All regex patterns used by `erlang.rs` are compiled once via
`std::sync::OnceLock<regex::Regex>` and reused across `parse_rebar_lock`
/ `parse_rebar_config` / `parse_app_src` invocations. Pattern compile is
amortized across every scan invocation.

**Rationale**: The reader is invoked at most once per language-reader pass
per scan, but its parse helpers are invoked once per `*.app.src` file
discovered (potentially many in an umbrella project) + once per
`rebar.config` block extraction + once per `rebar.lock` entry. Compiling
regex patterns on each call would be wasteful. The `OnceLock` pattern
is established across milestones 069 (gem), 137 (dart), 138 (composer),
139 (cocoapods), 140 (elixir).

**Alternatives considered**:
- Use `lazy_static!` macro — REJECTED. Workspace migrated to `std::sync::OnceLock`
  in milestone 094 era for std-only consistency.
- Recompile per-call — REJECTED. Wasteful for non-trivial regex patterns.

## R8 — Byte-identity SBOM golden preservation (SC-004)

**Decision**: A source tree containing no `rebar.lock` / `rebar.config` /
`*.app.src` files MUST produce an SBOM byte-identical (modulo timestamps
+ serial numbers) to a pre-feature baseline scan. The 12-ecosystem
golden test suite in `mikebom-cli/tests/cdx_regression.rs`,
`spdx_regression.rs`, and `spdx3_regression.rs` gates this invariant.

**Rationale**: The reader's no-op fast path is critical for non-Erlang
projects (the vast majority of scans). Any unintentional output drift
on non-Erlang trees would break every existing golden and surface
during the pre-PR gate.

**Test coverage**: The existing 12-ecosystem regression suite reruns
post-merge. If milestone 141 unexpectedly emits Erlang-specific output
on a non-Erlang fixture, the gate fails immediately with a golden diff.

**Memory note**: Per persistent memory `feedback_cross_host_goldens.md`,
the goldens are already cross-host-byte-identical (HOME isolated,
serial+timestamp masked, hashes stripped) so milestone 141's no-op
preservation is verified by re-running the existing suite. No new
golden files are introduced.

## R9 — Walker integration: `*.app.src` discovery

**Decision**: `safe_walk` from `mikebom-cli/src/scan_fs/walk.rs` (milestone
114) discovers `*.app.src` files anywhere under the scan root. The
reader filters by `path.extension() == Some("app.src")` to identify
candidates. `_build/` subdirectories are excluded by the walker per
the established milestone-002+ ignore-patterns convention; `*.app`
compiled-output files inside `_build/` are therefore not seen.

**Rationale**: The walker already supports arbitrary-depth discovery
of language-specific files (cargo's `Cargo.toml` may be nested under
any directory; npm's `package.json` likewise). Erlang's `*.app.src`
follows the same pattern — typically at `apps/<sub_app>/src/<sub_app>.app.src`
in umbrella projects, or `src/<app>.app.src` in flat projects.

**`_build/` exclusion**: rebar3's `_build/` directory holds compiled
artifacts (`.app`, `.beam`, `.boot`, `.script`). Including it would
double-emit components AND introduce non-source-tier artifacts the
reader has no use for. The exclusion is established practice (mirrors
node_modules/, target/, _build/).

**Alternatives considered**:
- Explicitly hardcode `apps/<sub_app>/src/<sub_app>.app.src` discovery
  path — REJECTED. Doesn't cover flat projects (non-umbrella `src/<app>.app.src`).
- Walk `_build/` and prefer compiled `.app` files when present —
  REJECTED. `_build/` artifacts may be stale (operator hasn't run
  `rebar3 compile` since last edit) and may carry build-host-specific
  artifacts (compile-time-generated info). `*.app.src` (source manifest,
  committed in git) is the authoritative source-tier truth.

## Summary table

| # | Decision | Inherits | Risk |
|---|----------|----------|------|
| R1 | `pkg:hex/` PURLs + private-org `repository_url=` | milestone 140 Phase 0 | none |
| R2 | Three rebar.lock shape variants (modern/map-form/legacy) | new | low — regex dispatch is bounded |
| R3 | Q3 keyword family + `mikebom:erlang-app-dep-kind` | new (Q3 clarification) | low — Principle V audit complete |
| R4 | Brace-counted tokenizer reuse | milestone 140 elixir.rs | none — battle-tested |
| R5 | `erlang-*` source-type prefix | milestone 122+137+138+139+140 | none |
| R6 | syft/trivy parity (deferred) | milestone 140 Out-of-Scope | none |
| R7 | OnceLock regex compile pattern | milestone 069+137-140 | none |
| R8 | Byte-identity SBOM golden | milestone 002+ (every reader) | low — gated by existing regression suite |
| R9 | safe_walk discovery + `_build/` exclusion | milestone 114 | none |

All NEEDS CLARIFICATION resolved. Phase 1 ready.
