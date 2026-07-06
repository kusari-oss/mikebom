# Implementation Plan: Extend `mikebom:orphan-reason` vocabulary

**Branch**: `167-orphan-reason-expand` | **Date**: 2026-07-06 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/167-orphan-reason-expand/spec.md`

## Summary

Extend the C45 `mikebom:orphan-reason` per-component annotation vocabulary from the existing 2 codes (`unresolved-indirect-require`, `flat-attached-fallback` — both Go-only, both emitted at Go-reader time in `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs:2091,2118`) to 5 codes covering Go + npm orphans per milestone-165 audit classifications:

| Code | Ecosystem | Semantic | Emission tier |
|---|---|---|---|
| `unresolved-indirect-require` | Go | Existing (m061). Preserved. | Go reader (Go-reader-time) |
| `flat-attached-fallback` | Go | Existing (m061 widened semantic). Preserved. | Go reader (Go-reader-time) |
| `stale-go-sum-entry` | Go | NEW (m167). Multi-version case. | Emit-time classifier |
| `dead-lockfile-entry` | npm | NEW (m167). npm multi-version case. | Emit-time classifier |
| `hoisted-unused` | npm | NEW (m167). npm no-sibling case. | Emit-time classifier |

**Fix approach**: two-tier emission per FR-005 priority:
1. **Go-reader-time** (pre-existing at `legacy.rs:2091,2118`): emits `unresolved-indirect-require` OR `flat-attached-fallback` per m061 semantics. Unchanged.
2. **Emit-time classifier** (NEW at `mikebom-cli/src/generate/orphan_reason.rs`): runs AFTER `compute_graph_completeness` (m158's BFS reachability pass). Iterates every `pkg:golang/*` and `pkg:npm/*` component. If BFS-unreachable (per Q1 clarification):
   - Check for same-name reachable sibling (multi-version case) → `stale-go-sum-entry` (Go) or `dead-lockfile-entry` (npm). OVERWRITES any Go-reader-time value (more specific).
   - Else check ecosystem → for Go, LEAVE the Go-reader-time value in place (`unresolved-indirect-require` OR `flat-attached-fallback`). For npm, emit `hoisted-unused`.

**Empirical target**: post-167 SBOMs for Kubernetes + ArgoCD show ≥ 35 Go orphan-reason emissions (K8s) + ≥ 28 orphan-reason emissions (ArgoCD) per SC-001/SC-002. All classifier-produced codes match m165 `analyze.py` classifications within a small margin.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–166; no nightly).
**Primary Dependencies**: Existing only — `serde_json` (annotation values), `std::collections::{HashSet, HashMap}`, `tracing`. Reuses `mikebom-cli/src/generate/graph_completeness/bfs.rs` (m158's multi-source BFS). **Zero new Cargo dependencies.**
**Storage**: N/A — all state in-process per scan.
**Testing**: 8+ unit tests (SC-008), 1 integration test (SC-009), existing m061 `flat-attached-fallback` + `unresolved-indirect-require` regression tests continue.
**Target Platform**: All mikebom-supported hosts.
**Project Type**: Rust CLI (mikebom-cli) — single-crate scope; 1 new module (`generate/orphan_reason.rs`) + 1 edited call-site (`scan_fs/mod.rs` or the emission pipeline entry point) + graph_completeness API extension.
**Performance Goals**: One additional BFS pass at emit-time per scan. Reuses milestone-158's `multi_source_bfs` — already O(V+E). Zero measurable overhead on scans of typical size.
**Constraints**: Constitution Principle IX (Accuracy — orphans classified correctly per FR-005 priority); Principle X (Transparency — FR-008 per-ecosystem counter log); SC-005 dual-side byte-identity for CDX + SPDX 2.3 (on non-Go/npm ecosystems only).
**Scale/Scope**: Kubernetes = 831 components, 39 Go orphans expected. ArgoCD = 1833 components, 31 orphans expected. podman-desktop = 2748 components, 12 npm orphans expected.

## Constitution Check

**GATE**: Pass before Phase 0 research. Re-check after Phase 1 design.

Constitution v1.5.0 principles evaluated against milestone 167's scope:

- **I. Pure Rust, Zero C**: PASS — Rust stable only, no new crates, no FFI.
- **II. Deterministic Scan Output**: PASS — same input produces same output. BFS is deterministic; same-name-sibling check is set-based (order-independent).
- **III. Attestation-First**: N/A — no attestation code touched.
- **IV. No `.unwrap()` in Production**: PASS — the classifier uses `Option::or` / `HashMap::get` patterns, no unwrap.
- **V. Specification Compliance (standards-native precedence)**: PASS — reuses existing C45 parity-catalog row per milestone 061's original justification (documented no-native-equivalent in `docs/reference/sbom-format-mapping.md`). No new `mikebom:*` prefix annotations added.
- **VI. Three-Crate Architecture**: PASS — only `mikebom-cli` touched.
- **VII. eBPF-Only Observation**: N/A — user-space code path.
- **VIII. Completeness — Never Silently Drop**: PASS — every BFS-unreachable Go/npm component receives a reason code. Nothing gets silently dropped.
- **IX. Accuracy — No Fake Versions**: PASS — reason codes reflect empirically-observed orphan patterns per milestone-165 audit.
- **X. Transparency — Explicit Signals**: PASS — FR-008 per-ecosystem counter log emits `orphan_reason_stale_go_sum_entry=<N>` etc. Grep-friendly per convention.
- **XI. Every Scan Produces an SBOM**: PASS — the classifier adds annotations post-emission-graph-assembly; no scan-termination path added.
- **XII. Ecosystem Coverage**: PASS — extends per-ecosystem orphan classification; doesn't remove existing coverage.

**Strict Boundaries** (v1.5.0):

- §1 (deterministic PURL): PASS.
- §2 (workspace layout): PASS.
- §3 (constitution amendment process): N/A.
- §4 (single source of truth): PASS — priority order in FR-005 documented and deterministic.
- §5 (no duplicate file-tier components): PASS — file-tier code path unchanged.

**Verdict**: All 12 principles + 5 boundaries clear. No violations, no Complexity Tracking entries needed.

## Project Structure

### Documentation (this feature)

```text
specs/167-orphan-reason-expand/
├── plan.md              # This file
├── research.md          # Phase 0 — BFS API extension + classifier priority order + emit-tier choice
├── data-model.md        # Phase 1 — classifier signature + call-site diff + vocab expansion
├── quickstart.md        # Phase 1 — how to reproduce + verify the fix
├── contracts/
│   └── README.md        # Empty stub — no new external contracts
├── checklists/
│   └── requirements.md  # /speckit.specify output
└── tasks.md             # /speckit.tasks output (NOT created here)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   ├── generate/
│   │   ├── graph_completeness/
│   │   │   └── mod.rs                # ← EDITED (T003): extend GraphCompletenessResult to expose reachable_set: HashSet<String>
│   │   └── orphan_reason.rs          # ← NEW (T004): emit-time classifier per FR-001 through FR-006
│   └── scan_fs/
│       └── mod.rs                    # ← EDITED (T005): call classifier after compute_graph_completeness; wire FR-008 log
└── tests/
    └── orphan_reason_expand.rs       # ← NEW (T014): SC-009 integration test — synthesized multi-ecosystem scan
```

**Structure Decision**: Two-tier emission preserved. Existing Go-reader-time emitter at `legacy.rs:2091,2118` (`unresolved-indirect-require`, `flat-attached-fallback`) is UNCHANGED. New emit-time classifier at `orphan_reason.rs` runs post-`compute_graph_completeness` and OVERWRITES more-general Go-reader-time values with more-specific classifications per FR-005 priority. This preserves m061 backward-compat for the 2 existing codes AND adds the 3 new codes without duplicate-emission conflicts.

Zero new external contracts. FR-008 tracing log field is the only new observable output beyond the extended vocabulary values on `@graph[]` / `properties[]` / `annotations[]`.

## Complexity Tracking

No entries required. All Constitution gates pass without justification. This is an extension of existing infrastructure with clear separation between pre-existing Go-reader-time emission (unchanged) and new emit-time classifier (additive).
