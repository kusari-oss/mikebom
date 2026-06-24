# Implementation Plan: Erlang/OTP ecosystem reader

**Branch**: `141-erlang-rebar-reader` | **Date**: 2026-06-24 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/141-erlang-rebar-reader/spec.md`

## Summary

Fifteenth language-ecosystem reader added to mikebom (joins cargo, npm, pip, gem, maven, golang, nuget, swift, kotlin, conan, dart, composer, cocoapods, elixir). Parses rebar3 `rebar.lock` (Erlang-term-syntax tuple-list literal; regex-tokenized with brace counting) + `rebar.config` (Erlang source; regex-extracted `{deps, [...]}` block + `{profiles, [...]}` blocks) + `*.app.src` (OTP application descriptor; regex-extracted `{vsn, ...}` + `{applications, ...}` + `{included_applications, ...}` + `{optional_applications, ...}` keywords). Emits one main-module per `*.app.src` (FR-012) plus one component per lockfile entry. Four source-type discriminators per FR-003: `pkg` (default Hex.pm) → `pkg:hex/<name>@<version>`; private-org `#{repo => hexpm:<org>}` → `pkg:hex/<org>/<name>@<version>?repository_url=https://repo.hex.pm`; `git` → `pkg:generic/<name>@<sha>?vcs_url=git+<url>`; OTP-runtime placeholder → `pkg:generic/<lib>@unspecified` (Q1: ALL non-lockfile entries from `applications:` emit; allowlisted carry `mikebom:otp-stdlib = "true"`). Main-module `depends` set unions FOUR sources per Q2+Q3: `rebar.config::{deps, ...}` ∪ `*.app.src::{applications, ...}` ∪ `{included_applications, ...}` ∪ `{optional_applications, ...}`; each edge-target carries `mikebom:erlang-app-dep-kind` annotation (`"required"`/`"included"`/`"optional"`) with precedence required > included > optional. Inner SHA-256 hashes (4th element of `{pkg, ...}` tuple when present) flow into `PackageDbEntry.hashes` per FR-011. **Zero new Cargo dependencies** — `regex` is already a workspace dep.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–140; no nightly required).

**Primary Dependencies**: Existing only — `regex` (workspace dep, used by gem/alpm/brew/yocto/cocoapods/elixir for line-format and DSL extraction), `mikebom_common::types::hash::{ContentHash, HashAlgorithm}` (FR-011 SHA-256 emission), `mikebom_common::types::purl::Purl` (PURL construction), `tracing` (warn-and-skip per FR-007), `anyhow`/`thiserror`, `serde_json` (annotation values), `std::sync::OnceLock` (regex compile-once). **No new Cargo dependencies.**

**Storage**: N/A — all state is in-process for the duration of a single scan.

**Testing**: `cargo +stable test --workspace`. Synthetic-fixture pattern via `tempfile::tempdir()` constructing minimal `rebar.config` + `rebar.lock` + `*.app.src` trees. Four new integration test files at `mikebom-cli/tests/erlang_*.rs` mirroring the milestone-140 `elixir_*.rs` family. SC-004 byte-identity preservation guarded by the existing 12-ecosystem golden suite.

**Target Platform**: Cross-platform reader. Pure-Rust regex extraction; no Erlang/OTP runtime required on the scan host.

**Project Type**: CLI tool — extends the `mikebom sbom scan` pipeline via the `read_all` dispatcher.

**Performance Goals**: ≤2 ms overhead per `rebar.lock` entry. Typical rebar3 OTP app (~30 hex deps): ~3 ms. Heavy multi-app umbrella (~150 deps across sub-apps): ~10 ms. No-Erlang-detected fast path adds ≤5 µs per non-Erlang scan.

**Constraints**:

- Byte-identical SBOM goldens when no Erlang project present (SC-004).
- Zero new Cargo deps.
- Per-file parse failures warn-and-skip; malformed lockfile → fall back to design-tier from sibling `rebar.config` per FR-007.
- The `hex` PURL type IS purl-spec-blessed (with namespace + `repository_url=` qualifier for private orgs per milestone-140 Phase 0 correction).
- Hex package names lowercased per purl-spec canonical form (typically no-op since Hex.pm enforces).
- `git` source uses `pkg:generic/` per the milestone-140 git-source convention (purl-spec doesn't bless `vcs_url=` for hex).
- No `rebar3` / Erlang runtime invocation; regex-only parsing.
- OTP runtime libs emit per Q1 — allowlist informational only; ALL `applications:` entries absent from `rebar.lock` emit as `pkg:generic/<lib>@unspecified` regardless of allowlist membership.
- Main-module `depends` unions four sources per Q2+Q3 with `mikebom:erlang-app-dep-kind` annotation per edge-target.

**Scale/Scope**: Typical rebar3 OTP app: 20–50 hex deps (smaller than typical Phoenix Elixir app's 80–100 because the Erlang ecosystem is more minimalist). Heavy multi-app umbrella with 3–5 sub-apps: ~100–150 unique components after dedup. Per-lockfile regex parse: ~2–5 ms warm-cache.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Verdict | Justification |
|---|---|---|
| I. Pure Rust, Zero C | ✓ | All new code is user-space Rust; no FFI, no C. Regex tokenization via the workspace `regex` crate. No Erlang/OTP runtime evaluation. |
| II. eBPF-Only Observation | N/A | Source-tree language reader; pre-existing discovery surface per every prior language-reader milestone. |
| III. Fail Closed | ✓ | A source tree without any of `rebar.lock` / `rebar.config` / `*.app.src` is a clean no-op (FR-006). Per-file parse failures warn-and-skip (FR-007). |
| IV. Type-Driven Correctness | ✓ | Uses `Purl` newtype + `ContentHash` newtype; no stringly-typed identifiers. Lockfile tuple parsing uses an enum-discriminated `LockEntry` per data-model.md. Production code MUST NOT call `.unwrap()` — error propagation via `Result`. |
| V. Specification Compliance | ✓ | **`hex` IS a purl-spec-defined type** ([hex-definition.md](https://github.com/package-url/purl-spec/blob/main/types-doc/hex-definition.md)). Names lowercased per spec. Private orgs use spec-blessed namespace-as-org + `repository_url=` qualifier (inherited from milestone-140 Phase 0 correction). Git-source uses `pkg:generic/` placeholder per milestone-140 correction (purl-spec doesn't bless `vcs_url=` for hex). OTP-runtime placeholders use `pkg:generic/<lib>@unspecified` + `mikebom:source-type = "erlang-otp-runtime"` annotation as a parity-bridge per Principle V (no purl-spec type for "OTP runtime libs ship with Ericsson distribution, not a registry"). `mikebom:source-type` annotation reuses C1 parity-catalog row from milestone 140. `mikebom:erlang-app-dep-kind` is a NEW annotation per Q3; documented in research.md as a parity-bridging extension because none of CycloneDX `scope`, SPDX 2.3 `DEV/BUILD/TEST_DEPENDENCY_OF`, or SPDX 3 `LifecycleScopeType` express the runtime keyword-family discriminator (required/included/optional in OTP's supervisor-startup model is orthogonal to dev/test scope). **syft/trivy divergence note**: neither tool parses `*.app.src` `applications:` keywords, so the OTP runtime discrimination is mikebom-unique; compatibility `mikebom:also-known-as` annotations deferred to v1.1. Documented in research §R1+R3. |
| VI. Three-Crate Architecture | ✓ | All new code lives in `mikebom-cli`. No new workspace crate. |
| VII. Test Isolation | ✓ | Synthetic tempfile fixtures only; no host-state dependency. |
| VIII. Completeness | ✓ | Closes the pure-Erlang/OTP gap entirely. Q1 chose over-emission of OTP runtime libs (allowlist informational only) to catch custom OTP apps + typos + silent-parse-failure regressions. Q2+Q3 unioned all four dep sources to surface build-only deps + runtime soft-deps that single-source extraction would lose. |
| IX. Accuracy | ✓ | PURL identity from lockfile fields directly; no heuristic guesses. Hex names lowercased per spec. Private-org translation `"hexpm:<org>"` → spec-correct namespace + `repository_url=`. Inner SHA-256 best-effort emission per FR-011 — don't synthesize hashes the lockfile doesn't carry. OTP runtime libs use `@unspecified` placeholder rather than guessing OTP versions. |
| X. Transparency | ✓ | Per-file parse failures emit `tracing::warn!`. Source-type via `mikebom:source-type`. OTP-allowlist membership surfaces via `mikebom:otp-stdlib = "true"` (Q1). App-dep-kind discrimination via `mikebom:erlang-app-dep-kind` (Q3). Design-tier fallback emits `mikebom:sbom-tier = "design"` + `mikebom:requirement-range` evidence. |
| XII. External Data Source Enrichment | ✓ | The lockfile + `rebar.config` + `*.app.src` files ARE the discovery sources. No external enrichment (license + Hex API explicitly out of scope per spec Out-of-Scope). |

**Verdict: PASS.** No violations.

## Project Structure

### Documentation (this feature)

```text
specs/141-erlang-rebar-reader/
├── plan.md                        # THIS FILE
├── spec.md                        # with Q1+Q2+Q3 clarifications
├── research.md                    # Phase 0 — Erlang/OTP-specific decisions
├── data-model.md                  # Phase 1 — lockfile shape variants, app.src parsed shape
├── quickstart.md                  # Phase 1 — operator scenarios
├── contracts/
│   └── erlang-component-purl.md   # PURL shape contract per FR-003
├── checklists/requirements.md     # 16/16 PASS (from /speckit-specify)
└── tasks.md                       # Phase 2 (created by /speckit-tasks)
```

### Source Code (repository root)

```text
mikebom-cli/src/
├── scan_fs/
│   ├── package_db/
│   │   ├── mod.rs                     # MODIFY: register erlang in read_all
│   │   ├── erlang.rs                  # NEW: rebar.lock + rebar.config + *.app.src
│   │   │                              # parsing, main-module emission, source-type
│   │   │                              # discrimination, umbrella handling,
│   │   │                              # design-tier fallback, Q3 keyword family,
│   │   │                              # Q1 over-emission of OTP runtime libs
│   │   ├── elixir.rs                  # REFERENCE: milestone 140 — closest sibling
│   │   │                              # (Hex registry + brace-counted tokenizer +
│   │   │                              # private-org PURL pattern + git-source
│   │   │                              # pkg:generic/)
│   │   ├── cocoapods.rs               # REFERENCE: milestone 139 — main-module
│   │   │                              # + multi-source + regex-extracted DSL
│   │   ├── gem.rs                     # REFERENCE: regex Ruby DSL parsing pattern
│   │   └── (no other scan_fs changes — erlang is purely additive)
│   └── walk.rs                        # UNCHANGED — safe_walk discovers
│                                       # *.app.src + rebar.{lock,config}
├── generate/cyclonedx/
│   ├── builder.rs                     # MODIFY: extend mikebom:evidence-kind
│   │                                  # enum to include "rebar-lock",
│   │                                  # "rebar-config", "app-src"
│   └── metadata.rs                    # POSSIBLY MODIFY: if main-module
│                                      # promoted to metadata.component
│                                      # carries new mikebom:* annotations
│                                      # (mikebom:erlang-app-dep-kind),
│                                      # extend the curated allowlist
│                                      # mirroring milestone-140's umbrella-root
│                                      # propagation
└── (no changes to other generate/, parity/, common/)

mikebom-cli/tests/
├── erlang_rebar_baseline.rs           # NEW: US1 — rebar3 OTP app fixture
├── erlang_source_discriminators.rs    # NEW: US2 — hex + git + OTP runtime
├── erlang_tier_fallbacks.rs           # NEW: US3 — design-tier + umbrella + Q3
│                                       # keyword family (applications +
│                                       # included_applications +
│                                       # optional_applications)
└── erlang_edge_cases.rs               # NEW: malformed lockfile + binary-string
                                       # atoms + legacy {<<name>>, version}
                                       # shape + private-org map-form shape +
                                       # main-module version fallback
```

**Structure Decision**: New file `erlang.rs` is a peer of cargo/dart/composer/cocoapods/gem/maven/golang/elixir. Integration site is `read_all` dispatcher (placed alphabetically between `dpkg` and `exclude_path` — second reader starting with 'e', after `elixir` from milestone 140). Test files follow `<reader>_<scenario>.rs` convention. **No new workspace crate per Principle VI; no new Cargo deps.**

## Complexity Tracking

> No Constitution Check violations — no justifications required.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none)    | n/a        | n/a                                  |
