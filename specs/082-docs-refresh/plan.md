# Implementation Plan: Documentation refresh and audit

**Branch**: `082-docs-refresh` | **Date**: 2026-05-07 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/082-docs-refresh/spec.md`

## Summary

Audit-first docs refresh covering all 22 in-scope markdown files (`docs/architecture/*.md` × 9, `docs/reference/*.md` × 5, `docs/user-guide/*.md` × 4, `docs/{index,design-notes,ecosystems}.md` × 3, `README.md`). Per the 2026-05-07 Q1 clarification, voice/style normalization is applied aggressively across every in-scope file (three dimensions: milestone-reference handling, code-block fence convention, inter-doc link convention). Per the Q2 clarification, architecture docs receive a targeted spot-check (5–10 testable claims per doc, ≤30-line inline fixes, larger rewrites filed as follow-up issues).

The Phase 0 audit (executed during /speckit.plan setup) surfaced concrete signal:
- **CLI-reference gap (real)**: `docs/user-guide/cli-reference.md` (527 lines) omits all 11 operator-facing flags from milestones 073–081. This is the single largest currency gap in the corpus.
- **Code-block tagging gaps (small)**: `cli-reference.md` (9/12 tagged), `architecture/overview.md` (0/1), `README.md` (17/18). Other files at 100%.
- **Milestone-reference style is inconsistent**: 104 `milestone N` mentions across docs with mixed inline/parenthetical/footnoted forms.
- **Inter-doc links are mostly relative** (good) but a few absolute links + a few `[label](./other.md)` vs `[label](other.md)` style differences.
- **Cross-reference graph is sparse**: only `sbom-types.md` (newest) has a "See also" section. Other reference docs end without forward navigation.

The milestone is **docs-only**: production code, tests, and byte-identity goldens are not touched. Pre-PR gate is the standard clippy + test sweep but should produce zero diff in code paths.

## Technical Context

**Language/Version**: N/A — pure documentation work. No code paths touched. The Rust workspace continues to compile + test identically pre/post merge.
**Primary Dependencies**: None new. The pre-PR gate uses the existing `cargo +stable clippy` + `cargo +stable test --workspace` pipeline; both remain stable across docs-only changes.
**Storage**: N/A.
**Testing**: `cargo +stable test --workspace` continues as the stability gate (must remain `0 failed` post-merge — proves docs changes haven't accidentally touched code). Plus a new lightweight verification script (`scripts/verify-docs-currency.sh`) added to the milestone that diffs `mikebom sbom scan --help` + `mikebom trace run --help` flag sets against `docs/user-guide/cli-reference.md` and reports any missing flags. The script is invocable manually + serves as the SC-001 verifier.
**Target Platform**: Markdown (CommonMark + GitHub-flavored extensions). Renders identically on GitHub web, IDE markdown previews, and `pandoc`-based static-site generators.
**Project Type**: Documentation refresh.
**Performance Goals**: N/A.
**Constraints**: Per FR-009 + SC-009: no production code, no test code, no goldens touched. The single allowed Rust touch is `scripts/verify-docs-currency.sh` (a new shell script, not Rust). Per Q1: aggressive style application across all 22 in-scope files. Per Q2: targeted spot-check on architecture docs.
**Scale/Scope**: ~22 markdown files touched. Per-file diff size: small (≤30 lines) for most; one file (`cli-reference.md`) gets a substantial expansion (~150–200 lines added for the 11 missing flags + their cross-references). Total milestone diff: ~500–800 lines net new docs, plus ~200–300 lines of style normalization edits (whitespace, code-block tagging, milestone-reference rewrites).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Constitution v1.4.0 (last amended 2026-05-01). All 12 principles + 4 strict boundaries reviewed:

| Principle | Status | Justification |
|-----------|--------|---------------|
| I. Pure Rust, Zero C | ✅ Pass / N/A | Docs-only milestone. No code touched. |
| II. eBPF-Only Observation | ✅ Pass / N/A | Architecture docs describing eBPF discovery may be edited for currency under Q2 spot-check rules; the principle is not modified. |
| III. Fail Closed | ✅ Pass / N/A | Documentation cannot fail open or closed; the principle applies to runtime behavior. |
| IV. Type-Driven Correctness | ✅ Pass / N/A | No new types; no code edited. |
| V. Specification Compliance | ✅ Pass | The milestone STRENGTHENS Principle V's audit-trail discoverability: existing audit-record entries in `docs/reference/sbom-format-mapping.md` Section I (added by milestones 080 + 081) are preserved verbatim; cross-references from other reference docs make them more discoverable. No new `mikebom:*` properties; no behavioral changes. |
| VI. Three-Crate Architecture | ✅ Pass / N/A | No code touched. |
| VII. Test Isolation | ✅ Pass / N/A | The new `scripts/verify-docs-currency.sh` is a shell script (not Rust); it runs without elevated privileges; it doesn't depend on eBPF; it doesn't affect the `cargo test` test isolation contract. |
| VIII. Completeness | ✅ Pass / N/A | Doesn't affect dependency discovery. |
| IX. Accuracy | ✅ Pass | The milestone IMPROVES accuracy by removing stale claims from architecture docs (Q2 spot-check) + ensuring CLI reference describes alpha.22 behavior. |
| X. Transparency | ✅ Pass | The milestone EXTENDS transparency by making the Principle V audit trail (sbom-format-mapping.md Section I) more discoverable via cross-references; by documenting deprecated flags clearly per FR-011. |
| XI. Enrichment | ✅ Pass / N/A | Not enrichment. |
| XII. External Data Source Enrichment | ✅ Pass / N/A | Not external data. |

| Strict Boundary | Status |
|-----------------|--------|
| 1. No lockfile-based dependency discovery | ✅ Pass / N/A |
| 2. No MITM proxy | ✅ Pass / N/A |
| 3. No C code | ✅ Pass — new shell script, no C |
| 4. No `.unwrap()` in production | ✅ Pass / N/A — no Rust code touched |

**Gate result: PASS.** No violations; no Complexity Tracking entries needed. The milestone aligns with Principles V/IX/X by improving discoverability + accuracy of operator-facing documentation.

## Project Structure

### Documentation (this feature)

```text
specs/082-docs-refresh/
├── plan.md                         # This file
├── spec.md                         # /speckit.specify + /speckit.clarify (Q1 + Q2 integrated)
├── research.md                     # Phase 0 — full per-file audit + style-convention decisions
├── data-model.md                   # Phase 1 — audit-classification entity + cross-reference graph entity
├── quickstart.md                   # Phase 1 — maintainer-facing recipe for keeping docs current going forward
├── contracts/
│   └── docs-style.md               # Phase 1 — the three style conventions Phase 0 picks
├── checklists/
│   └── requirements.md             # Already passing
└── tasks.md                        # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

The milestone touches every in-scope markdown file. Production code, test code, and byte-identity goldens are NOT touched. One new shell script lives in `scripts/`.

```text
docs/                                              # ALL in-scope files refreshed:
├── architecture/                                   # 9 files — Q2 targeted spot-check + Q1 style
│   ├── overview.md
│   ├── scanning.md
│   ├── generation.md
│   ├── attestations.md
│   ├── enrichment.md
│   ├── licenses.md
│   ├── purls-and-cpes.md
│   ├── resolution.md
│   └── signing.md
├── reference/                                     # 5 files — currency verify + "See also" added per FR-003
│   ├── identifiers.md
│   ├── sbom-types.md                             # Already has "See also" (milestone 081)
│   ├── cross-tier-binding.md
│   ├── sbom-format-mapping.md                    # Section I audit-record preserved verbatim
│   └── conformance-harness-guide.md
├── user-guide/                                    # 4 files — major content addition for cli-reference
│   ├── installation.md
│   ├── quickstart.md                             # Recipes refreshed per US3 + FR-004
│   ├── cli-reference.md                          # Major addition: 11 flags from milestones 073–081
│   └── configuration.md                          # Env var coverage per FR-005
├── design-notes.md                                # Style normalization
├── ecosystems.md                                  # Style normalization
└── index.md                                       # Navigation update per FR-010

README.md                                          # Currency + style per US4 + FR-006

scripts/
└── verify-docs-currency.sh                       # NEW — diff `mikebom <sub> --help` flag set
                                                   # against docs/user-guide/cli-reference.md;
                                                   # exit 0 when no flags missing; exit 1 with
                                                   # diff output otherwise. Used as SC-001 verifier.

docs/reference/binding-fixtures/                   # OUT OF SCOPE per spec — test artifacts
docs/examples/                                     # OUT OF SCOPE per spec — example artifacts
docs/research/                                     # OUT OF SCOPE per spec — research notes
```

**Structure Decision**: Pure documentation refresh + one new verification shell script. No Rust changes. Every in-scope file (22 total) gets touched for at least the Q1 aggressive style pass; subset gets larger content edits per US1–US5.

## Phase 0 — Research questions

Six implementation-level decisions to pin in `research.md`. The two highest-impact decisions (Q1 aggressive style scope; Q2 targeted spot-check architecture audit) were locked during /speckit.clarify; this phase pins the per-decision specifics + builds the per-file audit deliverable.

1. **Per-file audit classification (definitive)**: enumerate every in-scope file (22 total per the in-scope list above) with: path, last-touched milestone (per `git log --oneline -- <path> | head -1`), classification (`current` / `partially stale` / `materially stale`), dominant currency gap, fix scope (small inline / large rewrite / file follow-up). The audit table IS the work plan for Phase 5–7 tasks.

2. **Style convention decisions (the three dimensions)**:
   - **Milestone-reference handling**: pick one convention from {inline `[milestone N]`, parenthetical `(milestone N)`, footnote-style, omitted in operator-facing docs + retained in reference docs}. **Recommend**: omitted in `user-guide/*.md` + `README.md` (operators don't care which milestone a feature came from); retained as parenthetical `(milestone N)` in `reference/*.md` + `architecture/*.md` (maintainer-facing). Single source of truth for milestone history is git log.
   - **Code-block fence convention**: language tag REQUIRED on every fenced block. Acceptable language tags from the existing corpus survey: `bash`, `json`, `rust`, `text` (for plain output), `yaml`, `toml`, `markdown`. Untagged blocks (currently 4 across 3 files per Phase 0 audit) MUST be tagged.
   - **Inter-doc link convention**: relative paths from the file's own directory, NO leading `./`. Examples: `[CLI ref](../user-guide/cli-reference.md)` from `docs/reference/`; `[Quickstart](user-guide/quickstart.md)` from `docs/index.md`. Inconsistent leading `./` forms (`[CLI](./cli-reference.md)` vs `[CLI](cli-reference.md)`) MUST be normalized to no-`./`.

3. **CLI-reference document structure (FR-002)**: pick the per-flag entry layout. **Recommend**: per-subcommand sections (`### sbom scan` / `### trace run` / `### sbom verify` / `### policy init` / etc.), each with a flag-by-flag table summary at the top + per-flag detailed sections below. Per-flag detail block includes: name + value placeholder, type, default, repeatable-or-not, valid value vocab, one-paragraph description, ≥1 example invocation, cross-reference to deep-dive doc. The 11 missing flags from milestones 073–081 land in their respective subcommand sections.

4. **Cross-reference graph design (FR-003)**: pick the "See also" link structure. **Recommend**: bottom-of-page section titled exactly "See also" with bullet list, each bullet `[Doc Title](relative-path) — one-line context.` Limit: 2–5 bullets per doc. The graph is curated by the milestone author + verified by SC-003 (≤3-click reachability).

5. **Audit deliverable structure (research.md)**: pick the format. **Recommend**: per-file audit table with columns `Path | Last-touched milestone | Classification | Currency gap | Fix scope`. Plus a per-architecture-doc spot-check appendix listing the 5–10 testable claims per doc with verification status. Total research.md target length: 200–300 lines (concise, scannable).

6. **`scripts/verify-docs-currency.sh` design (SC-001 verifier)**: pick the verification approach. **Recommend**: a bash script that (a) runs `mikebom sbom scan --help 2>&1 | grep -oE '^\s+--[a-z][a-z-]*' | sort -u` to get the flag set; (b) extracts flag names from `docs/user-guide/cli-reference.md` via `grep -oE -- '--[a-z][a-z-]*' | sort -u`; (c) computes `comm -23` to surface flags in the binary but missing from docs; (d) exits 0 when the diff is empty, exits 1 with the diff printed otherwise. Same approach for `mikebom trace run`. Adds ~40 LOC; runs in <1s.

## Phase 1 — Design & contracts

### data-model.md

Two entities (no new Rust types — these are documentation-process entities):
- `AuditedDoc { path, last_touched_milestone, classification, currency_gap, fix_scope }` — one row per in-scope file in research.md's audit table.
- `CrossReferenceLink { from_path, to_path, context_text }` — captured during Phase 0 to validate the SC-003 ≤3-click reachability claim.

### contracts/

One contract: `docs-style.md`. Documents the three style conventions chosen during Phase 0 §2 + the cli-reference structure picked during Phase 0 §3 + the "See also" link convention picked during Phase 0 §4. Future milestones reference this contract when touching docs to avoid re-relitigating style choices.

### quickstart.md

Maintainer-facing recipe for keeping docs current going forward:
1. **When adding a new CLI flag**: update `docs/user-guide/cli-reference.md` per the per-subcommand structure + add a deep-dive entry in the appropriate `docs/reference/<topic>.md` if the flag introduces a new operator-facing concept. Run `bash scripts/verify-docs-currency.sh` to confirm flag set is current.
2. **When adding a new operator-visible env var**: update `docs/user-guide/configuration.md`.
3. **When changing operator-visible behavior**: review `docs/user-guide/quickstart.md` recipes for breakage.
4. **When making any docs change**: apply the three style conventions per `contracts/docs-style.md`. Aggressive normalization for any newly-touched file; piggy-back style fixes on currency edits going forward.
5. **When deprecating a flag**: mark it deprecated in the CLI reference per FR-011; document replacement; cite removal-target milestone.

### Agent context update

Run `.specify/scripts/bash/update-agent-context.sh claude` after Phase 1 docs land.

## Phase 2 — Out of scope for this command

`/speckit.plan` ends here. `/speckit.tasks` consumes plan.md + spec.md + Phase 1 docs and emits `tasks.md`. Estimated task count: **~14–18** — larger than 081 because the milestone touches 22 files but smaller than 080 because most files get a small style-only edit (1 task per cluster of files); a few files (cli-reference.md, README.md, configuration.md) get larger content edits with their own tasks.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified.**

Not applicable — Constitution Check passes on all 12 principles + 4 strict boundaries with zero violations. The milestone aligns with Principles V (audit-trail discoverability), IX (accuracy improvement), and X (transparency improvement).
