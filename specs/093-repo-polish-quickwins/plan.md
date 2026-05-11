# Implementation Plan: Repo polish — quick-wins cleanup pass

**Branch**: `093-repo-polish-quickwins` | **Date**: 2026-05-10 | **Spec**: [spec.md](spec.md)

## Summary

Five small repo-polish gaps surfaced after alpha.30 shipped (broken README git URL, missing SECURITY.md, missing CONTRIBUTING.md, missing issue/PR templates, cargo-binstall undocumented). Bundle them as one docs-and-metadata-only PR (~1 hour). Zero production code, zero golden regen, zero new deps. The Homebrew tap + curl-pipe installer items remain deferred to a separate follow-up milestone per spec's Out of Scope.

**Approach**: pure file additions / edits at the repo root and under `.github/`. The only `Cargo.toml` touch is an optional `[package.metadata.binstall.overrides]` table to make `cargo binstall mikebom` work against the existing release-tarball naming — verified at Phase 0 (research.md §1) to be a no-op if cargo-binstall's default autodiscovery already matches our naming, in which case the README documents the explicit invocation and the Cargo.toml stays untouched.

## Technical Context

**Language/Version**: N/A — this milestone touches Markdown + (potentially) Cargo.toml metadata only.
**Primary Dependencies**: None new. The existing `release.yml` artifact-naming convention is the only behavioral input that informs the cargo-binstall integration choice.
**Storage**: N/A — purely source-tree edits.
**Testing**: `./scripts/pre-pr.sh` (mandatory pre-PR gate; clippy + workspace tests) — must continue to pass. No new automated test added; SECURITY.md / CONTRIBUTING.md / templates are content-quality artifacts validated by human review against the spec's acceptance scenarios.
**Target Platform**: GitHub.com (renders `SECURITY.md`, the templates, the PR-checklist), plus any Markdown-rendering tooling (e.g., `cargo doc` doesn't touch these).
**Project Type**: Documentation + repo-metadata cleanup. Not a software feature.
**Performance Goals**: N/A. Spec's SC-005 (`cargo binstall mikebom` completes in 60 seconds) is bounded by network + the existing GitHub Releases CDN, not by anything this milestone adds.
**Constraints**: FR-007 (no production code changes), FR-008 (no golden regen), FR-009 (no new Cargo deps), FR-010 (pre-PR gate clean). Hard guardrails for scope creep.
**Scale/Scope**: ~5 new files at repo root + `.github/` (SECURITY.md, CONTRIBUTING.md, 2-3 issue templates, PR template), ~5 README edits, optionally one `Cargo.toml` table addition. Total diff target: <500 lines.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

- **I. Pure Rust, Zero C**: ✅ no code at all in this milestone; no FFI, no toolchain.
- **II. eBPF-Only Observation**: N/A — no scan/discovery behavior changes.
- **III. Fail Closed**: N/A — no runtime behavior changes.
- **IV. Type-Driven Correctness / no `.unwrap()` in production**: N/A — no Rust source touched.
- **V. Specification Compliance / standards-native precedence**: ✅ no `mikebom:*` properties involved; no SBOM emission affected.
- **VI. Three-Crate Architecture**: ✅ untouched. The optional `Cargo.toml` metadata addition is at the workspace member level (`mikebom-cli`) and doesn't change crate boundaries.
- **VII. Test Isolation**: N/A — no new tests.
- **VIII. Completeness**, **IX. Accuracy**, **X. Transparency**, **XI. Enrichment**, **XII. External Data Source Enrichment**: N/A — no SBOM output touched.

**No violations.** No Complexity Tracking entry needed.

## Project Structure

### Documentation (this feature)

```text
specs/093-repo-polish-quickwins/
├── plan.md                         # This file
├── research.md                     # Phase 0: cargo-binstall behavior audit + GitHub auto-detection paths
├── data-model.md                   # Phase 1: per-deliverable file shape (which sections each markdown file MUST contain)
├── contracts/
│   └── repo-surface-contracts.md   # Phase 1: section-level contracts for each new markdown file + the README edits
├── quickstart.md                   # Phase 1: maintainer recipes (apply, verify, ship)
├── checklists/
│   └── requirements.md             # 16/16 pass — already complete
└── spec.md                         # Feature spec
```

### Source Code (repository root)

```text
mikebom/                            # repo root
├── README.md                       # MODIFIED (FR-001 URL fix + FR-006 cargo-binstall path)
├── SECURITY.md                     # NEW (FR-002)
├── CONTRIBUTING.md                 # NEW (FR-003)
├── Cargo.toml                      # OPTIONALLY MODIFIED (FR-006 — only if cargo-binstall needs metadata overrides)
└── .github/
    ├── ISSUE_TEMPLATE/             # NEW directory
    │   ├── bug_report.md           # NEW (FR-004)
    │   └── feature_request.md      # NEW (FR-004)
    └── pull_request_template.md    # NEW (FR-005)
```

**Structure Decision**: Pure additive at repo root + `.github/`. No source files in `mikebom-cli/`, `mikebom-common/`, or `xtask/` are touched. The optional `Cargo.toml` edit is the only Cargo-mediated file change.

## Complexity Tracking

> Not applicable — no Constitution gate violations.

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|--------------------------------------|
| (none)    | (none)     | (none)                               |

## Phase Plan

### Phase 0 — Research (`research.md`)

Three decision points resolved at Phase 0:

1. **Does `cargo binstall mikebom` work today without `Cargo.toml` metadata?** — audit `cargo binstall`'s default URL-template logic against mikebom's release-tarball naming (`mikebom-vX.Y.Z-<arch>-<os>.tar.gz`).
2. **Where does GitHub auto-detect `SECURITY.md`?** — confirm repo-root vs `.github/` vs `docs/` precedence so the file lands in the right place for the Security-tab integration.
3. **Issue template format: classic `.md` with front-matter vs YAML forms (`.yml`)?** — pick one for FR-004.

### Phase 1 — Design (`data-model.md`, `contracts/`, `quickstart.md`)

- **data-model.md** — per-file section structure (SECURITY.md has 4 sections, CONTRIBUTING.md has 5, templates have ~4 each).
- **contracts/repo-surface-contracts.md** — what each section MUST contain.
- **quickstart.md** — maintainer recipes for applying the edits and verifying SC-001 through SC-007.

Re-evaluate Constitution Check post-design: still no violations expected (pure docs).

### Phase 2 — Tasks

Out-of-scope for `/speckit.plan`; will be generated by `/speckit.tasks`.

## Agent Context Update

The agent-context update script will be re-run after Phase 1; this milestone adds no new technology surface so the agent context delta should be empty or trivial.
