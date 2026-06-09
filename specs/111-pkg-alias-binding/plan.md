# Implementation Plan: Operator-supplied PURL alias for cross-tier binding

**Branch**: `111-pkg-alias-binding` | **Date**: 2026-06-09 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/111-pkg-alias-binding/spec.md`

## Summary

A new repeatable CLI flag `--pkg-alias LHS=RHS` (plus `MIKEBOM_PKG_ALIAS` env-var equivalent) on `mikebom sbom scan` declares that the binary-tier PURL `LHS` should be treated as the source-tier PURL `RHS` when computing the milestone-072 cross-tier binding. Aliased components reach `Verified` / `Weak` strength instead of `Unknown { reason: "source-not-found-in-bind-target" }`. The alias is persisted by extending the existing milestone-072 `SourceDocumentBinding` envelope with two optional fields (`alias_from`, `alias_to`) so `verify-binding` and `trace-binding` reproduce the same result without re-supplying the flag. Output also gains a sibling `applied_alias: "<LHS> → <RHS>"` field for at-a-glance auditor visibility. The `BindingStrength` enum is unchanged.

Technical approach: PURL match is strict equality on canonical form (clarification Q1); persistence via additive `Option<Purl>` fields on `SourceDocumentBinding` with `#[serde(default, skip_serializing_if)]` so the envelope wire format remains backward-compatible (clarification Q2); verify-binding output extension via a sibling `Option<String>` field rather than a new strength variant (clarification Q3).

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–110; no nightly required for this user-space-only feature).
**Primary Dependencies**: Existing only — `clap` (the new flag via `ArgAction::Append` derive), `serde`/`serde_json` (additive envelope round-trip), `Purl` newtype from `mikebom-common` (milestone 005 — canonicalization + equality), milestone-072 `SourceDocumentBinding` (the envelope this extends), `tracing` (warn/info logs), `anyhow` (error propagation), `thiserror` (alias-parse error variants). **No new Cargo dependencies.**
**Storage**: N/A — alias declarations are in-process per scan; persisted only inside the emitted SBOM via the extended envelope. No caches, no databases.
**Testing**: `cargo test --workspace` (CI gate). Unit tests in `mikebom-cli/src/binding/`, `mikebom-cli/src/cli/scan_cmd.rs`. Integration tests in `mikebom-cli/tests/` reusing the existing milestone-072 + milestone-096 fixtures plus three new alias-specific scenarios.
**Target Platform**: Linux / macOS / Windows host (matches the milestone-101 Windows smoke matrix). The feature is pure user-space; no OS-specific code paths.
**Project Type**: CLI tool feature extension. No new crates.
**Performance Goals**: O(N_aliases × N_components) per binding pass. With typical N_aliases ≤ 10 and N_components ≤ 10k, the additional cost is microsecond-scale per scan. SC-005 requires workspace-of-5 aliases without ergonomic penalty; performance not on the critical path.
**Constraints**:
- **SC-004 byte-identity**: scans without any `--pkg-alias` flag MUST produce output byte-identical to the pre-feature baseline. The additive `Option<Purl>` envelope fields with `skip_serializing_if` plus the regression test against the existing milestone-072 + milestone-096 goldens enforce this.
- **Principle IV no-`.unwrap()`**: all alias parsing returns `Result<_, AliasError>`; the `Purl` constructor returns `Result<Purl, PurlError>` and the CLI `value_parser` propagates.
- **Principle V standards-native audit**: confirmed in research.md §1 — CDX 1.6 and SPDX 2.3/3.0.1 have no native cross-tier SBOM-to-SBOM binding construct, so the milestone-072 envelope extension is the only viable carrier.
**Scale/Scope**: ~3 PRs (research → implementation → polish). Estimated LOC: ~600 lines of production code + ~400 lines of tests.

## Constitution Check

*Gate evaluated against mikebom Constitution v1.4.0. Re-checked after Phase 1 design (see post-design re-check at the bottom).*

| Principle | Applicability | Status | Notes |
|---|---|---|---|
| I. Pure Rust, Zero C | Yes | PASS | All new code in Rust; no C bindings, no new build-script logic. |
| II. eBPF-Only Observation | N/A | PASS | This milestone touches post-discovery binding logic only. No dependency-discovery changes; the eBPF-trace authority over component existence is unaffected. |
| III. Fail Closed | Yes | PASS | Malformed PURLs and conflicting alias declarations reject at CLI parse time with actionable errors (FR-008/009). Aliases without `--bind-to-source` produce a warning, not silent loss (FR-010). |
| IV. Type-Driven Correctness | Yes | PASS | Alias parsing returns `Result<PurlAlias, AliasError>`; `PurlAlias` is a newtype around `(Purl, Purl)`; no `.unwrap()` in production code paths. The CLI `value_parser` propagates parse errors via `anyhow`. |
| V. Specification Compliance | Yes | PASS WITH AUDIT | Standards-native audit cited: CDX 1.6 + SPDX 2.3/3.0.1 have no native cross-tier SBOM-to-SBOM binding construct (research.md §1). The existing milestone-072 `SourceDocumentBinding` envelope is the only viable carrier; extending it with two optional fields preserves the parity-bridging justification documented in `docs/reference/sbom-format-mapping.md` C56 (binding rows). No new top-level `mikebom:*` property is introduced; only the existing envelope grows two additive fields. The envelope's CDX/SPDX 2.3/SPDX 3 emission shape is unchanged. |
| VI. Three-Crate Architecture | Yes | PASS | All changes confined to `mikebom-cli/`. No new crates. |
| VII. Test Isolation | Yes | PASS | All tests run in unprivileged CI. No eBPF / root requirements. |
| VIII. Completeness | N/A | PASS | No discovery surface changes. |
| IX. Accuracy | Yes | PASS | Aliases improve binding-result accuracy by removing the systemic `Unknown` for flagship components. Aliases are operator-asserted; the existing milestone-072 layered-evidence rules still apply to the RHS-side match (verified-via-hash-evidence vs weak-via-purl-only). |
| X. Transparency | Yes | PASS | The extended envelope's `alias_from` / `alias_to` fields explicitly surface that an alias was applied; the `applied_alias` sibling field in verify-binding output gives auditors at-a-glance visibility. |
| XI. Enrichment | N/A | PASS | No external enrichment. |
| XII. External Data Source Enrichment | N/A | PASS | No external data sources. |
| Strict Boundary 1: No lockfile-based discovery | N/A | PASS |
| Strict Boundary 2: No MITM proxy | N/A | PASS |
| Strict Boundary 3: No C code | N/A | PASS |
| Strict Boundary 4: No `.unwrap()` in production | Yes | PASS | Enforced by `clippy::unwrap_used` deny at `mikebom-cli` crate root. New code follows the existing pattern. |

**Gate result**: All principles pass. No violations to track. Complexity Tracking section below is therefore empty.

## Project Structure

### Documentation (this feature)

```text
specs/111-pkg-alias-binding/
├── plan.md              # This file
├── research.md          # Phase 0 — standards audit + design decisions
├── data-model.md        # Phase 1 — type extensions
├── quickstart.md        # Phase 1 — operator walkthrough
├── contracts/           # Phase 1 — CLI + envelope schemas
│   ├── cli-flags.md
│   └── binding-envelope-v1.1.md
├── checklists/
│   └── requirements.md  # Already created by /speckit-specify
├── spec.md              # Spec with embedded clarifications
└── tasks.md             # Phase 2 (/speckit.tasks)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   ├── binding/
│   │   ├── mod.rs                  # MODIFY: add optional alias_from / alias_to fields to SourceDocumentBinding
│   │   ├── verify.rs               # MODIFY: per-component binding builder honors recorded aliases
│   │   └── alias.rs                # NEW: PurlAlias newtype + AliasMap + AliasError + parser
│   ├── cli/
│   │   ├── scan_cmd.rs             # MODIFY: --pkg-alias flag + env-var propagation + parser wiring
│   │   ├── verify_binding_cmd.rs   # MODIFY: applied_alias sibling field in output JSON
│   │   └── trace_binding_cmd.rs    # MODIFY: applied_alias sibling field in output JSON
│   └── parity/extractors/
│       ├── cdx.rs                  # MODIFY: extractor for the extended envelope (alias_from / alias_to)
│       ├── spdx2.rs                # MODIFY: same — SPDX 2.3 path
│       └── spdx3.rs                # MODIFY: same — SPDX 3 path
├── tests/
│   ├── pkg_alias_binding_us1.rs    # NEW: User Story 1 integration test (single primary binary)
│   ├── pkg_alias_binding_us2.rs    # NEW: User Story 2 integration test (workspace with multiple binaries)
│   ├── pkg_alias_binding_us3.rs    # NEW: User Story 3 integration test (verify-binding round-trip)
│   └── fixtures/
│       └── pkg_alias_binding/
│           ├── source-baz.cdx.json    # NEW: source-tier fixture with pkg:cargo/baz@1.0.0
│           ├── image-baz.cdx.json     # NEW: image-tier fixture pre-binding
│           └── workspace-multi.cdx.json # NEW: two-binary workspace fixture
docs/reference/
└── sbom-format-mapping.md         # MODIFY: extend C56 (binding) row with alias_from/alias_to note
```

**Structure Decision**: Extension of an existing milestone feature, no architectural change. New `binding/alias.rs` submodule keeps the alias-specific types isolated from the broader binding logic. Three integration tests (one per user story) reuse existing milestone-072 + milestone-096 fixture patterns. Parity extractors gain the two new fields in the same envelope, no new C-rows.

## Phase 0 — Research

See [research.md](./research.md) for:
- **§1 Standards-native audit (Principle V mandatory)**: confirms no CDX/SPDX-native cross-tier-binding construct; documents the existing milestone-072 parity-bridging justification this milestone extends.
- **§2 Envelope wire-compatibility**: the additive `Option<Purl>` field approach with `#[serde(default, skip_serializing_if = "Option::is_none")]`. Decision NOT to bump the envelope's `algo` field from `v1` to `v2` — consumers ignore unknown fields by spec, and a forced version bump would break pre-feature SBOM consumers unnecessarily.
- **§3 Match semantics**: confirms clarification Q1's strict-canonical-equality choice. `Purl::canonical()` from milestone 005 is the comparison primitive on both sides (alias LHS at CLI parse time, scan-output component PURL at binding-time).
- **§4 Conflict resolution**: same-LHS-different-RHS rejected at CLI parse time per FR-008. Same-RHS-multiple-LHS allowed per Edge Cases.
- **§5 CLI ergonomics survey**: reviewed existing `--component-id` (milestone 073) for the `LHS=RHS` flag-value-parse pattern; reuse via a shared parser helper.

## Phase 1 — Design

See:
- [data-model.md](./data-model.md) — `PurlAlias`, `AliasMap`, extended `SourceDocumentBinding`, `AliasError` enum.
- [contracts/cli-flags.md](./contracts/cli-flags.md) — `--pkg-alias` syntax, env-var aliasing, error-message contract.
- [contracts/binding-envelope-v1.1.md](./contracts/binding-envelope-v1.1.md) — extended `SourceDocumentBinding` schema with `alias_from` / `alias_to` fields; CDX / SPDX 2.3 / SPDX 3 emission shapes.
- [quickstart.md](./quickstart.md) — operator walkthrough of the issue #225 scenario.

## Constitution Re-Check (Post-Design)

Re-evaluated after Phase 1 artifacts written. All gates from the initial check remain green:

- The envelope-extension approach (Phase 1 contracts) introduces no new top-level `mikebom:*` property; the standards-native audit (Phase 0 research.md §1) holds.
- The `PurlAlias` newtype in data-model.md satisfies Principle IV's type-driven correctness mandate.
- The CLI `value_parser` design in contracts/cli-flags.md fails closed on malformed input (Principle III).
- No design choice introduces a new dependency, a new crate, or a privilege requirement.

No constitution amendments required. No complexity-tracking entries needed.

## Complexity Tracking

*Empty — Constitution Check produced no violations.*
