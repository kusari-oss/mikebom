# Implementation Plan: Self-describe SBOM scope

**Branch**: `047-scope-self-description` | **Date**: 2026-04-30 | **Spec**: [spec.md](./spec.md)
**Input**: [spec.md](./spec.md)

## Summary

Two-prong milestone surfacing existing scope information through (a)
a new README "What kind of SBOM does mikebom emit?" explainer, and
(b) a new document-level scope hint in SPDX 2.3 + SPDX 3 output for
parity with CDX `metadata.lifecycles[]`. No new CLI flags. No new
component-level emissions. Reuses existing per-component
`sbom_tier` data and the existing
`--include-declared-deps`-derived scope-mode resolution.

The audit found mikebom already emits CDX `metadata.lifecycles[]`
and `compositions[]`; only the SPDX side lacks an equivalent
document-level slot, and only the README lacks a user-facing
mental-model explainer. This plan closes both gaps surgically.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited).
**Primary Dependencies**: existing only — `serde`, `serde_json`,
`chrono`. No new crates.
**Storage**: N/A.
**Testing**: byte-identity goldens regen for SPDX 2.3 + SPDX 3
(18 files); inline tests for the new comment-text builder; new
jq-shaped acceptance tests for SC-003 / SC-004; `holistic_parity`
must remain green.
**Target Platform**: cross-platform (any platform mikebom builds
on).
**Project Type**: code-modifying milestone (SPDX serializers) +
docs (README).
**Performance Goals**: N/A — adding a single string field per
document; no measurable runtime impact.
**Constraints**: zero CDX golden diff (FR-011 / SC-006), 18 SPDX
goldens regen with one comment-field delta + the SHA-derived
`SPDXID`/`documentNamespace` re-stamps that follow from content
change (already-accepted pattern from prior alpha bumps).
**Scale/Scope**: 5 source files modified, ~80 LOC of Rust + ~50
LOC of Markdown.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1
design.*

| Principle | Engaged? | Status |
|---|---|---|
| I. Pure Rust, Zero C | Yes — Rust-only edits | ✅ |
| II. eBPF-Only Observation | No — no discovery code change | ✅ vacuous |
| III–IV. (existing principles) | No — no behavior change to scan / resolve / enrich pipeline | ✅ vacuous |
| V. Specification Compliance | Yes — adds a `comment` field to SPDX 2.3 `creationInfo` and a `comment` key to SPDX 3 `SpdxDocument`. Both are spec-defined slots (SPDX 2.3 §6.13, SPDX 3.0.1 Element/comment). No format violation. | ✅ |
| VI. Three-crate architecture | No code in `mikebom-common` or `mikebom-ebpf` | ✅ untouched |
| Pre-PR Verification | All 4 commits' `./scripts/pre-pr.sh` clean | ✅ enforced by SC-007 |

No gate violations.

## Phase 0: Research (resolved inline)

The spec deferred one item to plan: which SPDX 3 slot carries the
document-level scope comment.

**Decision**: emit `comment` on the `SpdxDocument` Element (NOT
on `CreationInfo`).

**Rationale**:
- Semantically correct per SPDX 3.0.1 Core/Element property
  definitions: Element-level `comment` is "comments by the creator
  about the Element" — exactly what a scope hint is. `description`
  / `summary` are about the *artifact described*, not *the SBOM
  document itself*.
- `CreationInfo` is a shared sub-element referenced by every
  Element in the graph; a comment there reads as "note about how
  this batch of elements was created", not "note about the SBOM's
  scope." `SpdxDocument` is the singular document-identity
  Element; a comment there is unambiguously the document-level
  note this milestone wants.
- JSON-LD shape is plain `comment` — the `spdx-context.jsonld`
  already maps the unprefixed local name (mikebom uses the same
  pattern for `comment` on existing `ExternalRef` emission at
  `v3_document.rs:128`). No `@context` change needed.
- For SPDX 2.3, the natural slot is `creationInfo.comment` (SPDX
  2.3 §6.13). The two formats are not byte-identical here — they
  use the same JSON key `comment` in different parent objects,
  which matches each format's spec intent.

**Alternatives considered**:
- `CreationInfo.comment` for SPDX 3 (parallel to SPDX 2.3) —
  rejected because the SPDX 3 `CreationInfo` is shared, not
  document-scoped.
- `SpdxDocument.description` / `SpdxDocument.summary` — rejected
  because both describe the *artifact* per the SPDX 3 spec, not
  the document.
- New `mikebom:` annotation slot on the document — rejected
  because using the spec-native slot is more interoperable; any
  SPDX-aware tool that reads `comment` will see this hint without
  knowing about mikebom-specific extensions.

Sources consulted:
- https://spdx.github.io/spdx-spec/v3.0.1/model/Core/Classes/CreationInfo/
- https://spdx.github.io/spdx-spec/v3.0.1/model/Core/Classes/SpdxDocument/
- https://spdx.github.io/spdx-spec/v3.0.1/model/Core/Classes/Element/
- https://spdx.github.io/spdx-spec/v3.0.1/model/Core/Properties/comment/

## Approach

Four commits, ordered so the SPDX-side emission lands before the
README explainer (FR-002(c) lets the README authoritatively
reference the just-shipped SPDX field).

### Commit 1 — `feat(047/us2-1)` — SPDX 2.3 `creationInfo.comment` + scope-mode plumbing + shared phase utility

Touched files:
- `mikebom-cli/src/generate/mod.rs` — add `pub scope_mode: ScopeMode` field to `ScanArtifacts` (new enum `ScopeMode { Artifact, Manifest }`). Field populated at `ScanArtifacts` construction time.
- `mikebom-cli/src/cli/scan_cmd.rs` — at lines ~743–760, after `effective_include_declared_deps` resolves, set `scope_mode` on the `ScanArtifacts` builder: `Manifest` when `effective_include_declared_deps` is true, `Artifact` otherwise.
- New module `mikebom-cli/src/generate/lifecycle_phases.rs` — extract two functions out of `cyclonedx/metadata.rs` (lines 37–46 + 76–89):
  - `pub fn tier_to_phase(tier: &str) -> Option<&'static str>`
  - `pub fn aggregate_phases<'a>(components: impl Iterator<Item = &'a ResolvedComponent>) -> Vec<&'static str>` (returns sorted unique phase list)
  - cyclonedx/metadata.rs imports from this module instead of holding the source. Goldens unchanged (same algorithm, same sort order).
- `mikebom-cli/src/generate/spdx/document.rs:122-160` — add `pub comment: Option<String>` field to `CreationInfo` struct with `#[serde(skip_serializing_if = "Option::is_none")]`. In `build_document` (line 217+), call a new `build_scope_comment(scan, cfg)` helper that:
  - Reads `scan.scope_mode`
  - Aggregates phases via `lifecycle_phases::aggregate_phases(scan.components)`
  - Returns prose like: `"Scope: artifact (on-disk components only). Observed lifecycle phases: build, post-build, operations. Per-component scope detail in mikebom:sbom-tier annotations."`
  - Phase list is deterministic (sorted via the existing `BTreeSet` collector)
  - Returns `Some(comment)` always (per spec edge-case decision: always emit; degrade phases-list to "no lifecycle phases observed" if empty)
- Inline tests in `document.rs` for `build_scope_comment`: artifact-mode + manifest-mode, with-phases + empty-phases.
- Regen 9 SPDX 2.3 goldens: `MIKEBOM_UPDATE_SPDX_GOLDENS=1 cargo +stable test --test spdx_regression`.

Verification:
- `jq -r '.creationInfo.comment' <any-spdx-2.3-output>` returns non-empty containing `Scope:` and `mikebom:sbom-tier`.
- `git diff main..HEAD -- mikebom-cli/tests/fixtures/golden/cyclonedx/` empty (FR-011).
- `./scripts/pre-pr.sh` clean.

### Commit 2 — `feat(047/us2-2)` — SPDX 3 `SpdxDocument.comment` (reuses commit 1's helper)

Touched files:
- `mikebom-cli/src/generate/spdx/v3_document.rs:113–132` — at the `spdx_document` JSON-LD object construction, add a `comment` key populated by reusing `build_scope_comment` from commit 1 (or a thin wrapper that calls into the same lifecycle_phases utility). Same prose, same deterministic order, same emission policy (always emit).
- Inline tests in `v3_document.rs` mirroring commit 1's test cases.
- Regen 9 SPDX 3 goldens: `MIKEBOM_UPDATE_SPDX3_GOLDENS=1 cargo +stable test --test spdx3_regression`.

Verification:
- `jq -r '.[\"@graph\"][] | select(.type == \"SpdxDocument\") | .comment' <any-spdx-3-output>` returns non-empty containing `Scope:` and `mikebom:sbom-tier`.
- SPDX 3 byte-identity goldens reflect the comment field plus the standard SHA-derived `SPDXID` / `documentNamespace` re-stamps that follow from any content change (same pattern as the alpha-6 release commit).
- `holistic_parity` test still green (document-level metadata is outside the C-row catalog).

### Commit 3 — `docs(047/us1)` — README explainer

Touched file:
- `README.md` — insert a new section titled `## What kind of SBOM does mikebom emit?` between the existing "Why" section and "Install" section. ~50 lines covering:
  - Two-axis framing: scope-mode (artifact vs manifest) + per-component lifecycle tier.
  - The five `mikebom:sbom-tier` values with one-line definitions each: `design`, `source`, `build`, `deployed`, `analyzed`.
  - The CDX self-description trio: `metadata.lifecycles[]`, `compositions[]`, per-component `mikebom:sbom-tier` annotation.
  - The SPDX self-description: `creationInfo.comment` (SPDX 2.3) / `SpdxDocument.comment` (SPDX 3) plus the same per-component annotation.
  - One-paragraph mapping to industry / consumer terminology so readers comparing mikebom to trivy / syft don't assume the tools are answering the same question.
  - Default scope: artifact for `--image`, manifest for `--path` — name `--include-declared-deps` as the toggle.
  - Pointer to `docs/design-notes.md`'s "Scope: artifact vs manifest SBOM" for deeper rationale.

Verification:
- `grep -nE '^## .*[Ww]hat kind of SBOM' README.md` matches.
- Section enumerates all 5 sbom-tier values (grep for each).
- Section names `--include-declared-deps`.

### Commit 4 — `chore(047)` + `docs(046)` — CHANGELOG + spec scaffolding

Touched files:
- `CHANGELOG.md` — Unreleased entry naming the SPDX 2.3 / SPDX 3 document-level comment as new emission and the README explainer as docs.
- `specs/047-scope-self-description/` — spec.md + plan.md + tasks.md + checklists/requirements.md (matches milestone 046's convention of committing speckit artifacts).
- `CLAUDE.md` — auto-update if `update-agent-context.sh` produced changes.

## Touched files

| File | Commit | Purpose |
|---|---|---|
| `mikebom-cli/src/generate/mod.rs` | 1 | + `ScopeMode` enum + `ScanArtifacts.scope_mode` field |
| `mikebom-cli/src/cli/scan_cmd.rs` | 1 | populate `scope_mode` on `ScanArtifacts` from `effective_include_declared_deps` |
| `mikebom-cli/src/generate/lifecycle_phases.rs` (new) | 1 | extract `tier_to_phase` + `aggregate_phases` from `cyclonedx/metadata.rs` |
| `mikebom-cli/src/generate/cyclonedx/metadata.rs` | 1 | re-import phase utilities; goldens unchanged |
| `mikebom-cli/src/generate/spdx/document.rs` | 1 | + `CreationInfo.comment` field + `build_scope_comment` helper |
| `mikebom-cli/src/generate/spdx/v3_document.rs` | 2 | + `comment` key on `SpdxDocument` JSON-LD object |
| `mikebom-cli/tests/fixtures/golden/spdx-2.3/*.json` (9) | 1 | regen |
| `mikebom-cli/tests/fixtures/golden/spdx-3/*.json` (9) | 2 | regen |
| `README.md` | 3 | + "What kind of SBOM does mikebom emit?" section |
| `CHANGELOG.md` | 4 | Unreleased entry |
| `specs/047-scope-self-description/` | 4 | spec scaffolding |

Total: ~80 LOC Rust + ~50 LOC Markdown + 18 golden regens.

## Risks

- **R1: SPDX 3 goldens cascade re-stamp.** SPDX 3 emits content-addressed `SPDXID` / `documentNamespace` (SHA-derived from doc content). Adding a comment field changes the doc content → those identifiers shift. Same pattern alpha-bump release commits already accepted; mitigation is to inspect the diff post-regen and confirm only the comment-derived deltas + SHA cascade. No special handling beyond a careful goldens diff review.
- **R2: Comment text determinism.** The phase list MUST be sorted deterministically across runs (same fixture → same comment string). The existing `cyclonedx/metadata.rs` collector uses a `BTreeSet` for this exact reason; the extracted `aggregate_phases` preserves that. No new test infra needed beyond the byte-identity goldens which already enforce determinism.
- **R3: Empty-phases edge case.** When no component carries an `sbom_tier`, the comment degrades to "no lifecycle phases observed" rather than omitting the comment entirely. Spec edge-case section parked this as either-or; plan picks "always emit" for predictability (consumers can rely on the field being present). Inline test in commit 1 covers this case.
- **R4: Shared utility `lifecycle_phases.rs` introduces a new module.** Small module; matches mikebom's existing per-feature module convention. Both CDX and SPDX serializers import from it. No risk of circular imports — the new module has no dependencies on either format.
- **R5: README placement debate.** Inserting between "Why" and "Install" is the recommended placement; reviewer may prefer another location. Editorial discussion within the milestone, not a scope change.

## Phasing

| Phase | Commits | Effort |
|---|---|---|
| Setup + recon | done (audit + Phase 0 research above) | 0 |
| Commit 1 (SPDX 2.3 + plumb + utility) | 1 | 1 hr |
| Commit 2 (SPDX 3) | 1 | 30 min (reuses helper) |
| Commit 3 (README) | 1 | 30 min |
| Commit 4 (CHANGELOG + scaffold) | 1 | 10 min |
| Verify + PR | 0 | 15 min |
| **Total** | **4 commits** | **~2.5 hr** |

## What this milestone does NOT do

- Does not add a new CLI flag (e.g., `--sbom-scope` from the
  original recommendation). The spec's audit found the concept
  already exists via `--include-declared-deps`; this milestone
  surfaces existing state, doesn't add new state.
- Does not walk host `~/.m2/repository/` wholesale (the
  recommendation's "build scope = +.m2 walk"). Audit found this
  is deliberately not done today; whether to change is a separate
  conversation.
- Does not emit `--sbom-scope all`-style multi-doc SBOMs. Today
  users get both views by running mikebom twice; convenience
  flag is a follow-on if demand surfaces.
- Does not change CDX output. CDX already self-describes via
  `metadata.lifecycles[]` + `compositions[]`; this milestone
  closes the SPDX gap to match.
- Does not modify the per-component `mikebom:sbom-tier` annotation
  or the `holistic_parity` C-row catalog. Document-level
  metadata is outside that matrix.
- Does not introduce reachability analysis (the recommendation's
  "runtime scope"). Defer to a future milestone if scoped.

## Why no `data-model.md` / `contracts/` / `quickstart.md`

Same rationale milestones 021 / 022 / 023 / 042 / 046 used (the
project's tighter 4-file template):
- `data-model.md`: one new tiny enum (`ScopeMode { Artifact,
  Manifest }`) inline in `generate/mod.rs`. Not worth a separate
  doc.
- `contracts/`: the public-API change is the SPDX comment field's
  shape; spec.md FR-005 + FR-006 already specify it (scope mode +
  phases + pointer to per-component annotation). No external
  consumer needs a separate contract doc.
- `quickstart.md`: spec's User Stories include
  acceptance-scenario verifications that read like quickstart
  steps. Duplicating noise.

This is the sixth use of the tighter template — pattern stable.
