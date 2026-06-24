# Implementation Plan: Scala/SBT ecosystem reader

**Branch**: `142-scala-sbt-reader` | **Date**: 2026-06-24 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/142-scala-sbt-reader/spec.md`

## Summary

Sixteenth language-ecosystem reader added to mikebom (joins cargo, npm, pip, gem, maven, golang, nuget, swift, kotlin, conan, dart, composer, cocoapods, elixir, erlang). Parses `*.sbt.lock` (JSON lockfile from the sbt-dependency-lock plugin, schema versions 1 + 2; discovered via the `*.sbt.lock` glob with mandatory top-level `lockVersion` + `modules` content-shape validation per Q3) + `build.sbt` (Scala-DSL build definition; regex-extracted `libraryDependencies` blocks + `name`/`version`/`organization`/`scalaVersion` settings + `lazy val ... = project.in(file("<path>"))` subproject declarations) + `project/Dependencies.scala` (Scala source sidecar; regex-extracted `val foo = "group" %% "artifact" % "version"` patterns) + `project/build.properties` (`sbt.version` pin, used for SBT-version inference per Q1 fallback cascade).

Emits one main-module per subproject per FR-009 + Q2 union-discovery strategy (parse root `build.sbt` `lazy val` blocks AND walk subdirs for `<subdir>/build.sbt`, dedup by canonicalized path) plus one component per lockfile entry. All components use PURL `pkg:maven/<group>/<artifact>@<version>` per FR-003 — Scala lives on Maven Central; the Scala-version suffix (`_2.13` / `_3`) is baked into the artifactId (the `name` PURL slot). The `%%` declaration form drives Scala-version-suffix appending in design-tier mode per the Q1 inference cascade (build.sbt → `project/build.properties` → default `_2.13` with `mikebom:scala-version-source = "default-fallback"`). Cross-built libraries (`cats-core_2.13` + `cats-core_3`) emit as distinct components per FR-003 since the PURLs differ in artifactId.

`mikebom:source-type` value-set: `scala-sbt-lock` (lockfile-derived), `scala-sbt-design` (build.sbt-derived), `scala-main-module` (per-subproject root). Distinguishes Scala-derived components from milestone-070's `maven-pom`-prefixed values even though both emit `pkg:maven/<group>/<artifact>@<version>` PURLs.

Inner SHA-256 hashes from `*.sbt.lock` schema-v2 `checksums` arrays flow into `PackageDbEntry.hashes` per FR-011 best-effort posture. **Zero new Cargo dependencies** — `regex` + `serde_json` are workspace deps.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–141; no nightly required).

**Primary Dependencies**: Existing only — `regex` (workspace dep, used by gem/alpm/brew/yocto/cocoapods/elixir/erlang for line-format and DSL extraction), `serde_json` (workspace dep, used by every JSON-format reader), `mikebom_common::types::hash::{ContentHash, HashAlgorithm}` (FR-011 SHA-256 emission), `mikebom_common::types::purl::Purl` (PURL construction), `tracing` (warn-and-skip per FR-007), `anyhow`/`thiserror`, `std::sync::OnceLock` (regex compile-once). **No new Cargo dependencies.**

**Storage**: N/A — all state is in-process for the duration of a single scan.

**Testing**: `cargo +stable test --workspace`. Synthetic-fixture pattern via `tempfile::tempdir()` constructing minimal `build.sbt` + `*.sbt.lock` + `project/` trees. Four new integration test files at `mikebom-cli/tests/scala_*.rs` mirroring the milestone-141 `erlang_*.rs` family. SC-004 byte-identity preservation guarded by the existing 13-ecosystem golden suite.

**Target Platform**: Cross-platform reader. Pure-Rust regex extraction; no SBT or JVM runtime required on the scan host.

**Project Type**: CLI tool — extends the `mikebom sbom scan` pipeline via the `read_all` dispatcher.

**Performance Goals**: ≤2 ms overhead per `.sbt.lock` entry. Typical SBT project (~80 hex deps): ~5 ms. Heavy multi-project SBT build (~250 deps across 5 subprojects): ~12 ms. No-Scala-detected fast path adds ≤5 µs per non-Scala scan.

**Constraints**:

- Byte-identical SBOM goldens when no Scala project present (SC-004).
- Zero new Cargo deps.
- Per-file parse failures warn-and-skip; malformed `*.sbt.lock` JSON → fall back to design-tier from sibling `build.sbt` per FR-007.
- The `maven` PURL type is purl-spec-blessed and inherited verbatim from milestone 070; Scala-version-suffixed artifactIds go in the `name` slot exactly as resolved.
- `%%` declarations in design-tier mode follow the Q1 inference cascade: explicit `scalaVersion` → `project/build.properties` → default `_2.13` with `mikebom:scala-version-source = "default-fallback"`.
- Lockfile discovery uses `*.sbt.lock` glob with Q3 content-shape validation (`lockVersion` + `modules` required); non-matching files warn-and-skip.
- Multi-project discovery unions root `build.sbt` `lazy val` declarations AND subdir `<subdir>/build.sbt` walking per Q2; dedup by canonicalized path; `lazy val` declarations win when both surfaces hit the same dir.
- No `sbt` / JVM toolchain invocation; regex-only DSL parsing.

**Scale/Scope**: Typical single-module SBT project: 40–80 deps. Phoenix-equivalent web-app SBT: ~100–150. Heavy multi-project SBT (Spark-style): ~250–400 deps across 5–10 subprojects. Per-lockfile JSON parse: ~3–8 ms warm-cache.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Verdict | Justification |
|---|---|---|
| I. Pure Rust, Zero C | ✓ | All new code is user-space Rust; no FFI, no C. Regex tokenization via workspace `regex`. JSON parsing via workspace `serde_json`. No SBT or JVM runtime evaluation. |
| II. eBPF-Only Observation | N/A | Source-tree language reader; pre-existing discovery surface per every prior language-reader milestone. |
| III. Fail Closed | ✓ | A source tree without any of `build.sbt` / `*.sbt.lock` / `project/build.properties` / `project/Dependencies.scala` is a clean no-op (FR-006). Per-file parse failures warn-and-skip (FR-007). |
| IV. Type-Driven Correctness | ✓ | Uses `Purl` newtype + `ContentHash` newtype; no stringly-typed identifiers. Lockfile JSON parsing into typed `SbtLockEntry` per data-model.md. Production code MUST NOT call `.unwrap()` — error propagation via `Result`. |
| V. Specification Compliance | ✓ | **`maven` IS a purl-spec-defined type** (inherited from milestone 070); used verbatim. Scala-version-suffixed artifactIds are literal artifactIds for PURL purposes per Maven Central reality — the `_2.13` / `_3` lives in the `name` slot, not as a separate qualifier. `mikebom:source-type` annotation (`scala-sbt-lock` / `scala-sbt-design` / `scala-main-module`) follows the milestone-122/137-141 prefixed convention. `mikebom:scala-version-source = "default-fallback"` (Q1) is a NEW transparency annotation per Principle X — audited against standards-native carriers in research §R3 (no existing CDX/SPDX field carries "the Scala-version suffix was inferred via heuristic"; the annotation is a parity-bridge specific to this reader). Documented in `docs/reference/sbom-format-mapping.md` Section I, milestone 142 row. **syft/trivy divergence note**: existing scala cataloging in those tools parses limited shapes; mikebom is more spec-correct re cross-built dedup. Compatibility `mikebom:also-known-as` annotations deferred to v1.1. |
| VI. Three-Crate Architecture | ✓ | All new code lives in `mikebom-cli`. No new workspace crate. |
| VII. Test Isolation | ✓ | Synthetic tempfile fixtures only; no host-state dependency. |
| VIII. Completeness | ✓ | Closes the Scala/SBT gap entirely. Q1 (default `_2.13` fallback) chose completeness over emit-broken-PURLs; Q2 (union discovery) chose completeness over single-source-discovery; Q3 (glob + content validation) chose completeness over strict-filename. |
| IX. Accuracy | ✓ | PURL identity from lockfile fields directly when present; no heuristic guesses. `%%` Scala-version inference uses explicit cascade with operator-visible heuristic-fallback annotation. Cross-built libraries preserve distinct identity (no dedup collapse) per Maven Central reality. Inner SHA-256 best-effort emission per FR-011. |
| X. Transparency | ✓ | Per-file parse failures emit `tracing::warn!`. Source-type via `mikebom:source-type`. Heuristic-fallback Scala-version surfaces via `mikebom:scala-version-source` (Q1). Design-tier mode emits `mikebom:sbom-tier = "design"` + `mikebom:requirement-range` evidence. Content-shape validation failures (Q3) log warn-and-skip per FR-007. |
| XII. External Data Source Enrichment | ✓ | The lockfile + `build.sbt` + `project/` files ARE the discovery sources. No external enrichment (license + Maven Central API explicitly out of scope per spec Out-of-Scope). |

**Verdict: PASS.** No violations.

## Project Structure

### Documentation (this feature)

```text
specs/142-scala-sbt-reader/
├── plan.md                        # THIS FILE
├── spec.md                        # with Q1+Q2+Q3 clarifications
├── research.md                    # Phase 0 — Scala/SBT-specific decisions
├── data-model.md                  # Phase 1 — lockfile JSON shape, build.sbt parsed shape
├── quickstart.md                  # Phase 1 — operator scenarios
├── contracts/
│   └── scala-component-purl.md    # PURL shape contract per FR-003
├── checklists/requirements.md     # 16/16 PASS (from /speckit-specify)
└── tasks.md                       # Phase 2 (created by /speckit-tasks)
```

### Source Code (repository root)

```text
mikebom-cli/src/
├── scan_fs/
│   ├── package_db/
│   │   ├── mod.rs                     # MODIFY: register scala in read_all
│   │   ├── scala.rs                   # NEW: *.sbt.lock + build.sbt + Dependencies.scala
│   │   │                              # parsing, main-module emission, source-type
│   │   │                              # discrimination, multi-project union-discovery,
│   │   │                              # design-tier fallback, Q1 inference cascade,
│   │   │                              # Q3 content-shape validation
│   │   ├── erlang.rs                  # REFERENCE: milestone 141 — closest sibling
│   │   │                              # (multi-tier emission + main-module + design-tier
│   │   │                              # fallback + brace-counted tokenizer + umbrella)
│   │   ├── elixir.rs                  # REFERENCE: milestone 140 — DSL regex extraction
│   │   │                              # template (mix.exs Elixir DSL → build.sbt Scala DSL)
│   │   ├── maven.rs                   # REFERENCE: milestone 070 — pkg:maven/ PURL shape
│   │   │                              # source of truth
│   │   └── (no other scan_fs changes — scala is purely additive)
│   └── walk.rs                        # UNCHANGED — safe_walk discovers
│                                       # *.sbt.lock + build.sbt + project/
├── generate/cyclonedx/
│   ├── builder.rs                     # MODIFY: extend mikebom:evidence-kind
│   │                                  # allowlist to include "sbt-lock",
│   │                                  # "sbt-build", "sbt-dependencies-scala"
│   └── metadata.rs                    # (verify if main-module propagation
│                                      # needs new entries — likely not since
│                                      # we reuse mikebom:component-role +
│                                      # mikebom:source-type which the
│                                      # existing curation handles)
└── (no changes to other generate/, parity/, common/)

mikebom-cli/tests/
├── scala_sbt_baseline.rs              # NEW: US1 — sbt-lock baseline fixture
├── scala_source_discriminators.rs     # NEW: US2 — % vs %% vs %%% + Scala 2 vs 3 +
│                                       # cross-built distinct components
├── scala_tier_fallbacks.rs            # NEW: US3 — design-tier + Q1 cascade + Q2
│                                       # multi-project + dev-scope
└── scala_edge_cases.rs                # NEW: malformed lockfile + Q3 content-shape
                                       # validation + main-module fallback paths
```

**Structure Decision**: New file `scala.rs` is a peer of cargo/dart/composer/cocoapods/gem/maven/golang/elixir/erlang. Integration site is `read_all` dispatcher (placed alphabetically after `rpm*` and before `swift` — first reader starting with 's'). Test files follow `<reader>_<scenario>.rs` convention. **No new workspace crate per Principle VI; no new Cargo deps.**

## Complexity Tracking

> No Constitution Check violations — no justifications required.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| (none)    | n/a        | n/a                                  |
