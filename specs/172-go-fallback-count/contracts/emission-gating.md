# Contract: Emission gating rules for C117

**Feature**: 172-go-fallback-count
**Date**: 2026-07-07

Codifies the per-scenario emission rules for `mikebom:go-transitive-fallback-count`. Reviewers use this as the truth-table for testing.

## Truth table

| Scan target | Go resolver ran? | Fallback count (N) | Annotation present? | Annotation value |
|---|---|---|---|---|
| Pure Rust (no Go files) | No | N/A | **No** | — |
| Pure npm (no Go files) | No | N/A | **No** | — |
| Go project, healthy env | Yes | 0 | **Yes** | `"0"` |
| Go project, degraded env (all step-5) | Yes | e.g., 73 | **Yes** | `"73"` |
| Go project, all modules unresolved | Yes | 0 (unresolved is step-6, not step-5) | **Yes** | `"0"` |
| Go project, `--offline` mode | Yes | ≥ 0 (depends on go.sum coverage) | **Yes** | `"N"` |
| `--path` where subdirs contain Go | Yes | 0-N | **Yes** | `"N"` |
| Empty scan target | No | N/A | **No** | — |
| Go project with `go.work` (multi-workspace) | Yes | sum across workspaces | **Yes** | `"total-N"` |

## Design rationale (per Q1 clarification)

**"Emit `\"0\"` explicitly on healthy scans"** was chosen over "omit when N=0" because:

1. **Affirmative-check ergonomics**: consumers writing "was this scan clean?" filters can rely on presence-with-value=`"0"`.
2. **Precedent alignment**: m134's `mikebom:purl-collisions-detected` emits `"0"` on clean scans; m158's universal `mikebom:graph-completeness` emits on every scan; m160's C110 emits when Go is present (regardless of Complete/Partial/Unknown status).
3. **Two-state signal**: consumers only need to distinguish (a) "annotation absent — no Go here" from (b) "annotation present — Go scanned; value tells you the count". Adding a third state ("annotation absent because N=0 despite Go being scanned") complicates consumer code without adding information.

## Coordination with other Go signals

Post-172, a Go-emitting SBOM contains this signal cluster at doc scope:

- `mikebom:graph-completeness` (m158 universal, C104) — reachability verdict
- `mikebom:graph-completeness-reason` (m158) — companion reason string (present iff verdict != Complete)
- `mikebom:go-transitive-coverage` (m160, C110) — ladder verdict
- `mikebom:go-transitive-coverage-reason` (m160, C111) — companion (present iff verdict != Complete)
- `mikebom:go-transitive-fallback-count` (m172, C117, **new**) — step-5 count
- `mikebom:go-workspace-mode` (m161, C112) — `go.work` mode
- Per-component `mikebom:go-transitive-source` (m160 T001, C108) — per-module resolution step

**Coordination invariant** (SC-005): the doc-scope C117 value equals the count of components tagged `mikebom:go-transitive-source == "go-sum-fallback"`. Consumers who trust one signal can derive the other; both are provided for ergonomics + easy CI filtering on either.

## Non-goals for m172 gating

- No `--strict` CLI flag that exits non-zero on N > 0. That's a separate future milestone if we decide it's worth the flag surface.
- No warning-level `tracing::warn!` when N > 0 during scan. The annotation is the signal; log-noise for a normal degraded scan would be too chatty. Trace-level (`tracing::debug!`) MAY log it as diagnostic aid, but not warn.
- No new `mikebom:go-transitive-fallback-modules` (plural, per-module list at doc scope). The per-component `mikebom:go-transitive-source` annotations already give per-module attribution; a doc-scope list would duplicate.
