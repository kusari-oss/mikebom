# Implementation Plan: Developer-asserted source-of-truth supplement (v0.1, CDX 1.6 input)

**Branch**: `119-supplement-cdx` | **Date**: 2026-06-13 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/119-supplement-cdx/spec.md`

## Summary

A new `--supplement-cdx <PATH>` clap flag on `mikebom sbom scan` reads a hand-authored CDX 1.6 JSON document, validates it structurally at scan startup, and merges its `components[]` / `services[]` / `dependencies[]` arrays with the scanner-discovered set BEFORE the CDX/SPDX builders run. Merge happens at one point in the pipeline (post-dedup, post-enrichment, immediately before serializer entry) so every emission format inherits the merged set identically.

**Pipeline slot**: insert a new `supplement::merge` step between the existing deduplicator and the CDX builder's `build_components()` call at `mikebom-cli/src/generate/cyclonedx/builder.rs:355`. The merge mutates the `Vec<ResolvedComponent>` and `Vec<RelationshipEdge>` in place, plus produces a new `Vec<ServiceEntry>` that the builder picks up via a new `build_services()` function paralleling `dependencies.rs`.

**Conflict resolution** (FR-006/FR-007) lives in a dedicated `supplement::conflict::resolve_component()` helper that takes a `(scanner_side, supplement_side)` pair and produces a merged `ResolvedComponent` plus a `Vec<ConflictRecord>`. Each conflict record becomes a `mikebom:assertion-conflict` annotation on the resulting component via the existing `extra_annotations` channel (same pattern milestone 116 + milestone 118 used).

**Validation** (FR-002) is a hand-rolled structural check — NOT a full JSON Schema validation — that asserts the file's `bomFormat == "CycloneDX"`, `specVersion == "1.6" | "1.5" | "1.4"` (accept earlier versions for compatibility), and that `components[]` / `services[]` / `dependencies[]` are arrays whose entries carry the keys mikebom actually consumes (PURL on components, name on services, ref+dependsOn on dependencies). Per Decision 1 in research.md, this avoids adding `jsonschema` as a runtime dep. Future v2 layering can swap to full schema validation without breaking the contract.

**Constitution Principle V audit** completed in research.md § Decision 8: three new annotation keys (`mikebom:source-tier = declared` as a new value on the existing C5 row; `mikebom:supplement-cdx` as a new envelope-level C-row; `mikebom:assertion-conflict` as a new per-component C-row). All three are justified per Principle V bullet 5 — no native CDX 1.6 / SPDX 2.3 / SPDX 3.0.1 field expresses "this component came from operator declaration" / "this SBOM merged supplement file X" / "scanner observed Y but operator declared Z."

**Technical approach**: greenfield additions only — no changes to existing per-ecosystem readers, no changes to dedup logic, no changes to enrichment. The flag is opt-in per FR-013; absence preserves byte-identity with pre-feature mikebom output.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–118; no nightly required).
**Primary Dependencies**: Existing only — `serde`/`serde_json` (CDX JSON parse), `clap` (the new flag via derive), `sha2` + `data-encoding` (the supplement file's sha256 for FR-012 provenance — both already in the workspace), `tracing`, `anyhow`, `thiserror`. **Zero new Cargo dependencies.** Per research §1, the supplement file validation is a hand-rolled structural check — no `jsonschema` runtime dep added.
**Storage**: N/A — the supplement file is read once at scan startup; its parsed representation lives in-process for the duration of the scan; no caching, no persistence.
**Testing**: `cargo +stable test --workspace` (existing harness). New tests: integration tests in `mikebom-cli/tests/supplement_cdx_integration.rs` exercising the three user stories' acceptance scenarios + the seven edge cases from spec.md. CDX 1.6 schema validation for the merged OUTPUT continues to run via the existing `tests/sbom_user_metadata.rs:1482-1516` precedent (the supplement-fed merged SBOM MUST still validate against the bundled CDX 1.6 schema at `tests/fixtures/schemas/cyclonedx-1.6.json`).
**Target Platform**: Linux x86_64 + macOS aarch64 + Windows x86_64 (same matrix as every milestone since 001).
**Project Type**: Single-project Rust CLI (`mikebom-cli/`).
**Performance Goals**: Supplement parse + structural validation: <10 ms for typical operator-authored files (~10–50 component/service entries). Merge step: O(N+M) where N = scanner component count, M = supplement component count; HashMap lookup keyed by canonical PURL. Total contribution to scan wall time: <50 ms even for pathologically large supplement files.
**Constraints**: Backwards-compat is TOTAL per FR-013 / SC-006 — emitted SBOMs without `--supplement-cdx` are byte-identical to pre-118 output (modulo random `serialNumber` and timestamp fields). The bytes-derived vs metadata field sets (FR-006/FR-007) are FIXED in v0.1; tunability is out of scope. Per spec clarification Q1: `--scan-as` always wins; supplement's `metadata.component` is IGNORED.
**Scale/Scope**: 1 new flag in clap; 1 new module `mikebom-cli/src/supplement/` (~400 LoC across parser, merge, conflict); 3 new file edits in the CDX builder + SPDX builders; 3 new C-rows in `docs/reference/sbom-format-mapping.md` + parity-extractors; 1 new integration-test file (~300 LoC, 11 tests covering acceptance scenarios + edge cases). Diff size estimate: ~700 LoC production + ~300 LoC tests + ~150 LoC docs.

## Constitution Check

| Principle | Status | Notes |
|---|---|---|
| I. Pure Rust, Zero C | ✓ | std-only feature; no new C deps. |
| II. eBPF-Only Observation | N/A → ✓ | The supplement is ENRICHMENT, not discovery. The scanner discovers via filesystem walks (per Principle II's clarification — filesystem walks are mikebom's discovery substitute outside trace mode); the supplement enriches with operator-authoritative metadata. Per Principle XII's External Data Source Enrichment carve-out, operator-supplied input is permitted enrichment. |
| III. Fail Closed | ✓ | Supplement-parse failure exits non-zero BEFORE any walker begins per FR-002 + SC-005. No partial SBOM is emitted on supplement-parse failure. |
| IV. Type-Driven Correctness | ✓ | New `Supplement` struct uses the existing `Purl` newtype from `mikebom-common` for canonical PURL identity; new `ConflictRecord` enum has explicit variants for field-set membership; zero new `.unwrap()` in production code (errors propagate via `?` / `anyhow::Result`). |
| V. Specification Compliance | ✓ | Three new `mikebom:*` annotation keys (`source-tier = declared` value, `supplement-cdx`, `assertion-conflict`) audited per Principle V bullet 5 in research.md § Decision 8. No native CDX 1.6 / SPDX 2.3 / SPDX 3.0.1 field expresses any of the three semantics; all three are justified parity-bridging annotations. Three new rows added to `docs/reference/sbom-format-mapping.md` as the docs-of-record. The merged output SBOM continues to validate against the existing CDX 1.6 + SPDX 2.3 + SPDX 3.0.1 schema gates. |
| VI. Three-Crate Architecture | ✓ | Lives entirely in `mikebom-cli/`; no new crates. |
| VII. Test Isolation | ✓ | Pure-logic + filesystem-fixture tests via `tempfile::tempdir()`; no eBPF, no privileged operations. |
| VIII. Completeness | ✓ | The supplement is PURELY ADDITIVE on the "what's in the SBOM" axis. FR-015 forbids any suppression mechanism. The merge never removes scanner-discovered components. |
| IX. Accuracy | ✓ | FR-006 preserves bytes-derived facts when scanner and supplement disagree; the supplement cannot blur the scanner's accuracy guarantees. |
| X. Transparency | ✓✓ | Three new annotations (`mikebom:source-tier = declared`, `mikebom:supplement-cdx`, `mikebom:assertion-conflict`) make every operator-declared input AND every conflict resolution decision machine-readable for consumers. This feature STRENGTHENS Principle X. |
| XI. Enrichment | ✓ | The supplement IS enrichment per Principle XI's positive carve-out for "operator-supplied data the scanner cannot observe." |
| XII. External Data Source Enrichment | ✓ | The supplement file is an external data source per Principle XII bullet 1. **Important distinction**: Principle XII constraint 1 was written about LOCKFILES + DATABASES (deps.dev / hash-to-PURL databases) — where the constraint forbids EXTRAPOLATING components from tool inference. Operator-asserted supplement files are a DIFFERENT category: the operator has authoritative knowledge that the scanner cannot observe (SaaS deps, vendored libraries without manifests). The standing `sbom scan` precedent (milestones 110 SelfIdentity `--scan-as` + 113 exclude-path) treats operator-supplied authoritative input as enrichment regardless of whether the scanner saw bytes-evident evidence. The FR-011 `mikebom:source-tier = declared` annotation makes the operator-vs-scanner provenance visible to consumers per Principle X (Transparency), preserving the trust-calibration centerpiece. Per constraint 2, every supplement-introduced component is annotated with `mikebom:source-tier = declared` provenance. Per constraint 3, the supplement-file unavailable case fails closed per Principle III (no graceful degradation). Per constraint 4, the scanner remains authoritative on bytes-evident facts (FR-006); supplement is enrichment context, not discovery substitute. |
| Strict Boundary 1 (no lockfile-based discovery) | ✓ | The supplement is NOT a lockfile read for component DISCOVERY — it's an operator-authoritative declaration. Components without scanner-observed evidence appear in the SBOM as `mikebom:source-tier = declared`, distinguishable from scanner-observed entries. The semantic distinction (operator-asserted vs lockfile-extrapolated) is consumer-visible per FR-011. |
| Strict Boundary 2 (no MITM) | N/A | |
| Strict Boundary 3 (no C code) | ✓ | |
| Strict Boundary 4 (no `.unwrap()` in production) | ✓ | All new error paths use `?` + `anyhow::Result`. The supplement parser uses `serde_json::from_str(...)?` + manual field validation. |

**Result**: Constitution Check PASSES. The feature STRENGTHENS Principle X (Transparency) by surfacing operator-declared input + conflict-resolution decisions as machine-readable annotations. The Principle XII enrichment-vs-discovery distinction is preserved by the FR-011 source-tier annotation + FR-015 no-suppression safety property.

## Project Structure

### Documentation (this feature)

```text
specs/119-supplement-cdx/
├── plan.md              # This file
├── research.md          # Phase 0 — 8 implementation decisions: validation mechanism (hand-rolled vs jsonschema dep); merge pipeline slot; field-set membership (FR-006/FR-007 commitments); SPDX 2.3 services-as-packages projection; assertion-conflict annotation shape; supplement-cdx provenance shape; supplement module layout; Constitution Principle V audit for three new annotation keys
├── data-model.md        # Phase 1 — Supplement struct + ConflictRecord enum + MergedSet + lifecycle diagram (load → validate → merge → emit)
├── quickstart.md        # Phase 1 — "operator authors supplement.cdx.json + invokes scan" walkthrough + "contributor adds a new conflict-resolution field" runbook + consumer-side "how to read mikebom:assertion-conflict" guide
├── contracts/
│   ├── supplement-format.md       # CDX 1.6 subset mikebom accepts; structural validation rules; rejected shapes
│   ├── merge-pipeline.md          # Pre/post invariants of supplement::merge; conflict-resolution algorithm; field-set membership table
│   └── annotation-shape.md        # The 3 new mikebom:* annotation keys' value shapes + emission gating
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   ├── supplement/                                  # NEW MODULE
│   │   ├── mod.rs                                   # Public Supplement struct + merge() entry point (~150 LoC)
│   │   ├── parser.rs                                # Hand-rolled structural CDX 1.6 subset validator (~150 LoC)
│   │   ├── merge.rs                                 # PURL-keyed merge of scanner + supplement Vec<ResolvedComponent> (~120 LoC)
│   │   ├── conflict.rs                              # FR-006/FR-007 field-set resolution + ConflictRecord emission (~80 LoC)
│   │   └── annotation.rs                            # Helpers for the 3 new mikebom:* annotation keys (~50 LoC)
│   ├── main.rs                                      # EXTENDED — clap derive: new `--supplement-cdx <PATH>` flag (~5 LoC)
│   ├── cli/
│   │   └── scan_cmd.rs                              # EXTENDED — parse flag, load supplement, thread into pipeline (~15 LoC)
│   ├── generate/
│   │   ├── cyclonedx/
│   │   │   ├── builder.rs                           # EXTENDED — call supplement::merge between dedup + build_components; build_services() new (~30 LoC)
│   │   │   ├── metadata.rs                          # EXTENDED — mikebom:supplement-cdx document-scope property (~10 LoC)
│   │   │   └── services.rs                          # NEW FILE — build_services() parallel to dependencies.rs (~80 LoC)
│   │   └── spdx/
│   │       ├── packages.rs                          # EXTENDED — supplement services project as Package entries with mikebom:component-role = "saas-service" (~30 LoC)
│   │       └── v3_packages.rs                       # EXTENDED — supplement services emit as Bundle + Relationship per spec edge case 3 (~30 LoC)
│   └── parity/
│       └── extractors/                              # EXTENDED — three new parity catalog rows (cdx.rs + spdx2.rs + spdx3.rs + mod.rs)
├── tests/
│   ├── supplement_cdx_integration.rs                # NEW — 11 integration tests covering acceptance scenarios + edge cases (~300 LoC)
│   └── fixtures/
│       └── supplement_cdx/                           # NEW — per-test scaffold templates (rare; most tests use tempfile per Decision 3)
docs/
└── reference/
    └── sbom-format-mapping.md                       # EXTENDED — 3 new C-rows (source-tier=declared value addition; supplement-cdx; assertion-conflict) with Principle V audit citations
mikebom-common/                                       # UNCHANGED
mikebom-ebpf/                                         # UNCHANGED
```

**Structure Decision**: Single-project layout (every milestone since 001). The new `supplement/` module lives at `mikebom-cli/src/supplement/` as a sibling to `scan_fs/`, `binding/`, and `generate/`. The placement reflects that supplement is conceptually parallel to scan_fs (a separate input source) rather than nested inside it. The module has five files (~550 LoC total) split by concern: parser, merge, conflict, annotation helpers, and a thin `mod.rs` entry. Tests live in one consolidated integration-test file mirroring the milestone-113 / milestone-118 `exclude_path_integration.rs` precedent.

## PR-split strategy

Single PR. The feature is intentionally scoped per the issue body's "~400 lines, 1 PR" estimate (slightly exceeded by the new `build_services()` infrastructure but still single-PR sized). If review feedback pushes back on diff size, FR-009's justification enum (just 2 values today) and the SPDX 2.3/3 services projection (FR-004 paths) are the natural cut-points — both deferrable to follow-up PRs without changing the v0.1 user-visible contract for the CDX 1.6 output path.

## Complexity Tracking

No constitution violations. The one design tension worth noting is the **validation mechanism choice** (Decision 1 in research.md): hand-rolled structural check vs. adding the `jsonschema` crate as a runtime dep. The hand-rolled path is chosen because (a) the supplement is a SUBSET of CDX 1.6 (we only care about a handful of fields), (b) adding `jsonschema` adds ~5 MB to the binary + a transitive dep tree, (c) the hand-rolled check is testable and stays in sync with what the merge code actually reads. The alternative is documented for v2 layering if operator feedback surfaces real-world supplement files that need stricter validation.
