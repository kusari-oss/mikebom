# Implementation Plan: Elixir/Mix ecosystem reader

**Branch**: `140-elixir-mix-reader` | **Date**: 2026-06-24 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/140-elixir-mix-reader/spec.md`

## Summary

Fourteenth language-ecosystem reader added to mikebom (joins cargo, npm, pip, gem, maven, golang, nuget, swift, kotlin, conan, dart, composer, cocoapods). Parses Elixir 1.4+ `mix.lock` (Elixir-syntax tuple map literal; regex-tokenized) + `mix.exs` (Elixir source; regex-extracted `deps/0` function body). Emits one main-module per `mix.exs` (FR-012) plus one component per lockfile entry. Three source-type discriminators: `:hex` (default) gets `pkg:hex/<name>@<version>` (or `pkg:hex/<org>/<name>@<version>?repository_url=https://repo.hex.pm` for private orgs per Phase 0 correction); `:git` gets `pkg:generic/<name>@<resolved-sha>?vcs_url=git+<url>` (per Phase 0 — purl-spec doesn't bless `vcs_url=` for hex); `:path` gets `pkg:generic/<name>@<version>` placeholder. Inner + outer SHA-256 hashes from `:hex` tuples flow into `PackageDbEntry.hashes` (Q3 best-effort — emit only what's present). Umbrella projects emit one main-module per sub-app + root, with root's `depends` listing each sub-app per Q2. Conditional `deps/0` blocks are flattened with `mikebom:elixir-extraction-mode = "conditional-flattened"` annotation per Q1. **Zero new Cargo dependencies** — `regex` is already a workspace dep.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–139; no nightly required).

**Primary Dependencies**: Existing only — `regex` (workspace dep, used by alpm/brew/yocto/cocoapods for line-format and DSL extraction), `mikebom_common::types::hash::{ContentHash, HashAlgorithm}` (FR-011 SHA-256 emission), `mikebom_common::types::purl::Purl` (PURL construction), `tracing` (warn-and-skip per FR-007), `anyhow`/`thiserror`, `serde_json` (annotation values), `std::sync::OnceLock` (regex compile-once). **No new Cargo dependencies.**

**Storage**: N/A — all state is in-process for the duration of a single scan.

**Testing**: `cargo +stable test --workspace`. Synthetic-fixture pattern via `tempfile::tempdir()` constructing minimal `mix.exs` + `mix.lock` trees. Four new integration test files at `mikebom-cli/tests/elixir_*.rs` mirroring the milestone-139 cocoapods_*.rs family. SC-004 byte-identity preservation guarded by the existing 11-ecosystem golden suite.

**Target Platform**: Cross-platform reader. Pure-Rust regex extraction; no Elixir/Erlang runtime required.

**Project Type**: CLI tool — extends the `mikebom sbom scan` pipeline via the `read_all` dispatcher.

**Performance Goals**: ≤2 ms overhead per `mix.lock` entry. Typical Phoenix app (~80 hex deps): ~5 ms. Heavy umbrella project (~250 deps across sub-apps): ~15 ms. No-Elixir-detected fast path adds ≤5 µs per non-Elixir scan.

**Constraints**:
- Byte-identical SBOM goldens when no Elixir project present (SC-004).
- Zero new Cargo deps.
- Per-file parse failures warn-and-skip; malformed lockfile → fall back to design-tier from sibling `mix.exs` per FR-007.
- The `hex` PURL type IS purl-spec-blessed (with namespace + `repository_url=` qualifier for private orgs per Phase 0 correction).
- Hex package names lowercased per purl-spec canonical form (typically no-op since Hex.pm enforces).
- `:git` source uses `pkg:generic/` per Phase 0 (purl-spec doesn't bless `vcs_url=` for hex).
- No `mix` / Elixir runtime invocation; regex-only parsing.
- Conditional `deps/0` blocks flattened per Q1 with `mikebom:elixir-extraction-mode = "conditional-flattened"` annotation.

**Scale/Scope**: Typical Phoenix app: 50–100 hex deps. Heavy LiveView/Ash project: ~150. Umbrella with 5 sub-apps × 50 deps each: ~250 unique components after dedup. Per-lockfile regex parse: ~2–5 ms warm-cache.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Verdict | Justification |
|---|---|---|
| I. Pure Rust, Zero C | ✓ | All new code is user-space Rust; no FFI, no C. Regex tokenization via the workspace `regex` crate. No Elixir runtime evaluation. |
| II. eBPF-Only Observation | N/A | Source-tree language reader; pre-existing discovery surface per every prior language-reader milestone. |
| III. Fail Closed | ✓ | A source tree without any of `mix.lock` / `mix.exs` is a clean no-op (FR-006). Per-file parse failures warn-and-skip (FR-007). |
| IV. Type-Driven Correctness | ✓ | Uses `Purl` newtype + `ContentHash` newtype; no stringly-typed identifiers. Lockfile tuple parsing uses an enum-discriminated `LockEntry` per data-model.md. Production code MUST NOT call `.unwrap()` — error propagation via `Result`. |
| V. Specification Compliance | ✓ | **`hex` IS a purl-spec-defined type** ([hex-definition.md](https://github.com/package-url/purl-spec/blob/main/types-doc/hex-definition.md)). Names lowercased per spec. Private orgs use spec-blessed namespace-as-org + `repository_url=` qualifier (Phase 0 correction — replaces the initial `mikebom:hex-repo` annotation proposal). Git-source pods use `pkg:generic/` placeholder per Phase 0 correction (purl-spec doesn't bless `vcs_url=` for hex). Path-source uses `pkg:generic/` + `mikebom:source-type = "hex-path"` annotation as parity-bridge per Principle V. `mikebom:source-type` annotation reuses C1 parity-catalog row. **syft/trivy divergence note**: syft emits empty namespace + no qualifiers (misses private-org info); trivy SKIPS `:git` and `:path` entries entirely. mikebom is more spec-correct than both. Compatibility `mikebom:also-known-as` annotations deferred to v1.1. Documented in research §R1 + spec Phase 0 corrections. |
| VI. Three-Crate Architecture | ✓ | All new code lives in `mikebom-cli`. No new workspace crate. |
| VII. Test Isolation | ✓ | Synthetic tempfile fixtures only; no host-state dependency. |
| VIII. Completeness | ✓ | Closes the Elixir gap entirely. Q1 (conditional-flattened extraction) chose completeness over silent omission; Q2 (umbrella root aggregation) preserves topology. |
| IX. Accuracy | ✓ | PURL identity from lockfile fields directly; no heuristic guesses. Hex names lowercased per spec. Private-org translation `"hexpm:<org>"` → spec-correct namespace + `repository_url=`. Q3 (best-effort SHA-256 emission) — don't synthesize hashes the lockfile doesn't carry. |
| X. Transparency | ✓ | Per-file parse failures emit `tracing::warn!`. Source-type via `mikebom:source-type`. Conditional-extraction precision-loss surfaces via `mikebom:elixir-extraction-mode = "conditional-flattened"` annotation per Q1. |
| XII. External Data Source Enrichment | ✓ | The lockfile + `mix.exs` ARE the discovery sources. No external enrichment (license + Hex API explicitly out of scope). |

**Verdict: PASS.** No violations.

## Project Structure

### Documentation (this feature)

```text
specs/140-elixir-mix-reader/
├── plan.md
├── spec.md              # corrected post-Phase 0
├── research.md          # Phase 0 — 8 sections
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── contracts/
│   └── elixir-component-purl.md
├── checklists/requirements.md  # 16/16 PASS
└── tasks.md             # Phase 2 via /speckit.tasks
```

### Source Code (repository root)

```text
mikebom-cli/src/
├── scan_fs/
│   ├── package_db/
│   │   ├── mod.rs                     # MODIFY: register elixir in read_all
│   │   ├── elixir.rs                  # NEW: mix.lock + mix.exs parsing,
│   │   │                              # main-module emission, source-type
│   │   │                              # discrimination, umbrella handling,
│   │   │                              # design-tier conditional-flattened
│   │   │                              # extraction
│   │   ├── cocoapods.rs               # REFERENCE: milestone 139 — main-module
│   │   │                              # + multi-source + regex-extracted DSL
│   │   ├── composer.rs                # REFERENCE: milestone 138 — SHA hash +
│   │   │                              # multi-tier precedent
│   │   └── gem.rs                     # REFERENCE: regex Ruby DSL parsing
│   └── (no other scan_fs changes — elixir is purely additive)
├── generate/cyclonedx/builder.rs       # MODIFY: extend mikebom:evidence-kind
│                                       # enum to include "mix-lock", "mix-exs"
└── (no changes to other generate/, parity/, common/)

mikebom-cli/tests/
├── elixir_phoenix_baseline.rs          # NEW: US1 — Phoenix app fixture
├── elixir_source_discriminators.rs     # NEW: US2 — hex + git + path + private-org
├── elixir_tier_fallbacks.rs            # NEW: US3 — design-tier + conditional-flat
└── elixir_edge_cases.rs                # NEW: malformed + umbrella + multi-line +
                                       # `:github` shortcut + dual SHA-256 edge cases
```

**Structure Decision**: New file `elixir.rs` is a peer of cargo/dart/composer/cocoapods/gem/maven/golang. Integration site is `read_all` dispatcher (placed alphabetically between `dpkg` and `exclude_path` — first reader starting with 'e'). Test files follow `<reader>_<scenario>.rs` convention. **No new workspace crate per Principle VI; no new Cargo deps.**

## Complexity Tracking

> No Constitution Check violations — no justifications required.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none)    | n/a        | n/a                                  |
