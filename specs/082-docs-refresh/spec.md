# Feature Specification: Documentation refresh and audit

**Feature Branch**: `082-docs-refresh`
**Created**: 2026-05-07
**Status**: Draft
**Input**: User description: "let's refresh docs and make sure they're clear, readable, and relatively completely and exhaustive"

## Overview

mikebom's documentation has grown organically across milestones 001–081. Each milestone tends to extend `docs/reference/identifiers.md` or add a new `docs/reference/<topic>.md` page; a few milestones add quickstart recipes; the README and `docs/user-guide/cli-reference.md` get touched less consistently. After 81 milestones, the cumulative state is:

- **28 markdown files in `docs/`** (~6,900 lines) plus the README (~625 lines).
- **Three-tier organization**: `user-guide/` (operator-facing onboarding), `reference/` (deep technical fields/wire formats/per-feature topics), `architecture/` (design rationale).
- **Currency drift**: a Phase 0 audit during /speckit.plan will enumerate exactly where stale claims live, but a quick survey shows `docs/user-guide/cli-reference.md` (527 lines) does NOT mention any of the operator-facing flags introduced in milestones 073–081 — `--root-name`, `--root-version`, `--scan-target-name`, `--sbom-type`, `--component-id`, `--creator`, `--annotator`, `--annotation-comment`, `--metadata-comment`, `--metadata-file`, `--strip-id-credentials`, `--component-id-allowed-schemes`. **All eleven flags are operator-visible but undocumented in the CLI reference**, meaning operators must read source code, milestone specs, or release notes to discover them.
- **Cross-reference inconsistency**: some reference docs (e.g., `sbom-types.md` from milestone 081) cross-link to other reference docs (e.g., `identifiers.md`) but most architecture docs don't link forward to relevant reference pages.
- **Voice/style drift**: voice, code-block conventions, milestone-reference handling, and link conventions vary across files (some pages use heavy milestone numbering as cross-references — useful for maintainers, distracting for operators reading for the first time).
- **Architecture docs are zero-milestone-reference** (per grep): suggests they describe stable foundational behavior, but means they may contain stale claims about behavior that has changed since the doc was last touched (specifically, milestones 047/052/072–081 likely affected behavior that architecture/scanning.md or architecture/generation.md describes).

The goal of this milestone is an **operator-facing docs refresh** that closes currency gaps, normalizes voice, and adds the cross-references that make the navigation surface usable. This is **not** a from-scratch docs rewrite — most files are accurate enough; the milestone is bounded to (a) audit + identify stale content, (b) close concrete gaps, (c) normalize style for operator discoverability.

The user's framing — "clear, readable, and relatively complete and exhaustive" — explicitly acknowledges that full exhaustiveness is impossible. The milestone targets **meaningful refresh**: every operator-visible CLI flag is documented; every reference doc cross-links its neighbors; every milestone 073–081 feature has at least one operator-facing entry point in `user-guide/` or `reference/`; the README accurately describes what mikebom does at alpha.22.

## Clarifications

### Session 2026-05-07

- Q: How aggressively should the milestone normalize voice/style across docs — touch every file, or only files already being edited for currency reasons? → A: **Aggressive — touch every file in `docs/` + README to apply the three style conventions uniformly.** All 28 markdown files in `docs/` plus the README receive a style pass for (a) milestone-reference handling, (b) code-block fence convention, (c) link convention. Larger diff (likely 25+ files), more thorough outcome — operators reading any doc encounter consistent voice + formatting. Style fixes ride alongside currency fixes in the same PR per the milestone's single-PR delivery cadence. Test-fixture READMEs, research notes, and binding-fixtures EXPECTED.md files remain out of scope per the spec's exclusions.
- Q: How deeply should US5 audit architecture docs against current source? → A: **Targeted spot-check.** For each architecture doc (`docs/architecture/*.md`), identify 5–10 testable behavioral claims (specific assertions that can be verified against alpha.22 source code, e.g., "scanning emits `mikebom:sbom-tier` per component" or "build-time eBPF observes file reads") and verify each. Fix surface-level staleness inline; file follow-up issues for deeper rewrites if any single claim requires more than ~30 lines of doc rewrite. Combined with Q1's aggressive style pass (which touches these docs anyway for formatting), produces a meaningful currency improvement without the scope creep of a full deep-verify pass.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — New operator can find every CLI flag in one canonical place (Priority: P1)

A new operator runs `mikebom sbom scan --help` and sees flag names but minimal context. They open `docs/user-guide/cli-reference.md` expecting the canonical reference for every flag with examples. Today they find a 527-line reference that omits eleven flags introduced in milestones 073–081. Post-milestone, every flag mikebom accepts at any level (`mikebom sbom scan`, `mikebom trace run`, etc.) is documented in `cli-reference.md` with: flag name, type, default, repeatable-or-not, valid value vocab where applicable, example invocation, cross-reference to the reference doc explaining the underlying feature.

**Why this priority**: This is the dominant operator-facing pain point. The CLI reference IS the doc operators reach for; missing flags = invisible features. P1 because the gap is concrete and the close-out is bounded (eleven flags to add).

**Independent Test**: an operator (or a test harness) runs `mikebom sbom scan --help` and `mikebom trace run --help`; for every flag listed by `--help`, the operator finds an entry in `docs/user-guide/cli-reference.md` with at least the flag name, brief description, type, and one example invocation.

**Acceptance Scenarios**:

1. **Given** the post-fix CLI reference, **When** the operator searches for any flag listed in `mikebom sbom scan --help` (alpha.22 set: `--path`, `--image`, `--format`, `--output`, `--root-name`, `--root-version`, `--scan-target-name`, `--sbom-type`, `--component-id`, `--creator`, `--annotator`, `--annotation-comment`, `--metadata-comment`, `--metadata-file`, `--strip-id-credentials`, `--component-id-allowed-schemes`, `--exclude-scope`, `--include-dev` (deprecated), `--offline`, `--max-file-size`, `--image-src`), **Then** an entry exists in the reference.
2. **Given** the post-fix CLI reference, **When** the operator searches for any flag listed in `mikebom trace run --help`, **Then** an entry exists.
3. **Given** any flag entry, **When** the operator reads it, **Then** the entry includes (a) flag name + value placeholder, (b) one-paragraph description, (c) at least one example invocation, (d) a cross-reference link to the deep-dive reference doc (e.g., `--sbom-type` links to `docs/reference/sbom-types.md`; `--component-id` links to `docs/reference/identifiers.md`).
4. **Given** a deprecated flag (`--include-dev`), **When** the operator reads its entry, **Then** the entry clearly marks it as deprecated, links to the replacement (`--exclude-scope`), and notes the removal milestone target if filed.

---

### User Story 2 — Reference docs cross-link cleanly so operators can navigate sideways (Priority: P1)

An operator reading `docs/reference/identifiers.md` (the milestone-073/074/075/076/077 identifier reference) wants to understand how the four-layer identity model interacts with the new milestone-080 `--creator` flag and the milestone-081 `--sbom-type` flag. Today these documents exist but cross-references between them are inconsistent — `sbom-types.md` (newest, milestone 081) links to `identifiers.md`, but `identifiers.md` doesn't link back; `cross-tier-binding.md` describes milestone 072 but doesn't reference how milestone-076 subjects affect binding semantics; `sbom-format-mapping.md` is a dense per-format mapping table that doesn't link out to the per-feature reference docs.

Post-milestone, every operator-facing reference doc has a "See also" section listing the closest neighbor docs. Operators reading any reference page can navigate to adjacent pages via in-doc links rather than guessing or grepping the docs/ directory.

**Why this priority**: Discoverability gap. The docs are mostly accurate; they just don't form a connected graph. P1 because the fix is small (add cross-reference sections) and high-leverage (operators using one doc surface adjacent docs for free).

**Independent Test**: an operator clicks through every reference doc's "See also" links from any starting point; within 3 clicks they can reach any other reference doc.

**Acceptance Scenarios**:

1. **Given** any reference doc in `docs/reference/`, **When** the operator looks at the bottom-of-page navigation, **Then** a "See also" section lists 2–5 closest-neighbor reference docs with one-line context.
2. **Given** the four key recently-added or recently-touched reference docs (`identifiers.md`, `sbom-types.md`, `cross-tier-binding.md`, `sbom-format-mapping.md`), **When** the operator clicks through their "See also" links, **Then** all four reach each other within 1–2 clicks.

---

### User Story 3 — Operator quickstart and configuration reflect current operator surface (Priority: P1)

A new operator following `docs/user-guide/quickstart.md` runs the documented commands. Today the quickstart's recipes were last comprehensively updated around milestone 010–015 era and may not reflect post-077 behavior (e.g., `--root-name` for renaming root components; `--sbom-type` for asserting SBOM type; the milestone-080 flag set for SBOM metadata). The configuration page (69 lines) doesn't mention `MIKEBOM_REQUIRE_SPDX3_VALIDATOR`, `MIKEBOM_UPDATE_*_GOLDENS` (test-side env vars but operators sometimes need them in CI), or any post-072 environment variables introduced.

Post-milestone, the quickstart includes at least one recipe demonstrating each major operator-control surface (`--sbom-type`, `--metadata-file`, `--component-id`); the configuration page documents every operator-visible environment variable mikebom reads at runtime.

**Why this priority**: Onboarding integrity. New operators following an outdated quickstart get a degraded first-impression of the tool. P1 because the gap is concrete and bounded.

**Independent Test**: a new operator (or test harness) follows `quickstart.md` recipes top-to-bottom against the alpha.22 binary; every documented command succeeds and produces output that matches the doc's expected snippets.

**Acceptance Scenarios**:

1. **Given** the post-fix `quickstart.md`, **When** the operator runs each documented recipe against alpha.22, **Then** every command exits zero and the produced output matches the doc's claimed shape (no doc says "you'll see X" while alpha.22 produces Y).
2. **Given** the post-fix `configuration.md`, **When** the operator searches for any environment variable mikebom reads at runtime (`MIKEBOM_REQUIRE_SPDX3_VALIDATOR`, `MIKEBOM_UPDATE_*_GOLDENS`, `MIKEBOM_PREPR_EBPF`, etc.), **Then** an entry exists with explanation + use case.

---

### User Story 4 — README accurately describes mikebom at alpha.22 (Priority: P1)

A potential operator visits the project README. Today the README is 625 lines and includes sections introduced milestone-by-milestone; some sections may describe behavior that has since changed (e.g., a "what mikebom emits" table that was current at milestone 050 but doesn't reflect milestones 072–081). The README is the single most-read doc; stale claims here mislead more operators than stale claims anywhere else.

Post-milestone, the README's "what mikebom does", "what it emits", "stability", and "getting started" sections accurately describe alpha.22 behavior. Outdated sections are either updated or removed; the "Recent milestones" section (if present) reflects the recent identity arc + SPDX 3 conformance + SBOM-type signaling work.

**Why this priority**: Highest-traffic doc. P1 because every other doc fix matters less if the entry-point misrepresents the tool.

**Independent Test**: a reviewer reads the README cover-to-cover with the alpha.22 binary handy; for every claim about behavior, they can verify it against the binary's actual output in <5 minutes per claim.

**Acceptance Scenarios**:

1. **Given** the post-fix README's "what mikebom emits" section, **When** the reviewer cross-checks against an alpha.22 SBOM, **Then** every described field is present + every described format is supported.
2. **Given** the post-fix README's "getting started" section, **When** a new operator follows the documented steps, **Then** they produce a valid SBOM in <5 minutes (matches `quickstart.md` per US3).
3. **Given** the README, **When** a reader searches for any milestone 072–081 feature by keyword (e.g., "scan target name", "creator", "annotator", "SBOM type", "identifier scheme"), **Then** at least one mention exists in the README with a link to the reference doc.

---

### User Story 5 — Architecture docs verified for currency (Priority: P2)

The architecture docs (`docs/architecture/scanning.md`, `generation.md`, `attestations.md`, etc.) describe design rationale + invariants, not per-flag operator surface. They're zero-milestone-reference per grep, suggesting they're stable. But milestones 047 (lifecycle phases), 052 (lifecycle dep scope), 072 (cross-tier binding), 078 (SPDX 3 conformance) likely changed behavior that these docs assert.

Post-milestone, each architecture doc has been read by an auditor (or LLM-equivalent grep+read pass) for stale claims. Docs are updated where claims are wrong; otherwise left alone.

**Why this priority**: Lower priority than user-facing docs. Architecture docs are read mostly by maintainers; staleness here matters less than staleness in user-guide. P2 because the audit may reveal zero changes are needed (they may genuinely be stable foundational descriptions).

**Independent Test**: a maintainer extracts the 5–10 testable behavioral claims per architecture doc identified during Phase 0 audit (per the 2026-05-07 Q2 clarification — targeted spot-check) and verifies each against current source code; ≥90% per-claim accuracy is expected post-fix.

**Acceptance Scenarios**:

1. **Given** each architecture doc, **When** the auditor verifies the 5–10 spot-check claims per the Q2 spot-check rule, **Then** stale claims are either fixed inline (≤30 lines per claim) OR filed as separate follow-up issues for deeper rewrites with the issue number recorded in research.md.
2. **Given** the architecture doc set as a whole, **When** an operator (not maintainer) reads it for design context, **Then** the docs accurately describe alpha.22's design philosophy on the verified-claim axes without misleading claims; minor inaccuracies on un-verified claims are acceptable for stable foundational docs.

---

### Edge Cases

- **Stale claims discovered during audit may be too large to fix in this milestone**: e.g., `docs/architecture/scanning.md` may describe pre-052 dep-scope semantics extensively. The milestone MUST resist scope creep — small fixes inline; large rewrites filed as separate follow-up issues.
- **Some flags are intentionally undocumented** (e.g., debugging flags, internal flags, sub-feature gates): the audit MUST distinguish "operator-facing flag missing from CLI reference" (FIX) from "internal-only flag intentionally absent" (LEAVE; document the omission policy in the CLI reference's intro).
- **`mikebom trace run` has experimental flags that change frequently**: documenting them at alpha.22 risks rapid drift. Mark trace-run flags with an "experimental" status banner; commit to maintaining them but acknowledge churn risk.
- **Voice/style normalization is subjective but applied broadly per Q1**: the milestone touches every in-scope file. The chosen conventions per dimension (milestone-reference handling, code-block format, link style) are picked once during Phase 0 design and applied consistently. Disputes over specific convention choices are handled in PR review, not by re-running the spec.
- **The Constitution Principle V audit-record entries** in `docs/reference/sbom-format-mapping.md` Section I (added by milestones 080 + 081) are durable historical records, not user-facing operator content. Preserve them as-is; they're signal for future maintainers.
- **Test-fixture READMEs** (`docs/reference/binding-fixtures/*/EXPECTED.md`, `docs/examples/cross-tier-walk/README.md`) are test artifacts, not operator-facing docs. Out of scope; do not refresh.
- **`docs/research/go-binary-scope.md`** is a research note, not a stable reference. Out of scope unless audit reveals it's actively misleading.
- **Multi-page consistency**: if the same operator-facing claim appears in three places (README + quickstart + a reference doc), the milestone picks ONE as the source-of-truth and links the others to it; avoids triplicate updates on every future flag addition.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The audit deliverable in `research.md` MUST enumerate every documentation file in `docs/` + `README.md`, classify each as `current` / `partially stale` / `materially stale` / `out-of-scope-for-refresh`, and identify the dominant currency gap (e.g., "missing 11 flags from milestones 073–081" for `cli-reference.md`).
- **FR-002**: `docs/user-guide/cli-reference.md` MUST document every operator-facing CLI flag accepted by `mikebom sbom scan` and `mikebom trace run` at alpha.22. For each flag: name, type, default, repeatable-or-not, valid value vocab where applicable, one-paragraph description, at least one example invocation, cross-reference to the deep-dive reference doc.
- **FR-003**: Each reference doc in `docs/reference/` MUST have a "See also" section listing 2–5 closest-neighbor reference docs with one-line context per link. The cross-reference graph MUST be connected: starting from any reference doc, an operator can reach any other reference doc within ≤3 clicks.
- **FR-004**: `docs/user-guide/quickstart.md` MUST contain at least one recipe demonstrating each of: source-tier scan, image scan, build-tier trace, `--sbom-type` operator override, `--metadata-file` sidecar, `--component-id` user-defined identifier. Each recipe's commands MUST succeed against the alpha.22 binary; documented output snippets MUST match alpha.22's actual output shape.
- **FR-005**: `docs/user-guide/configuration.md` MUST document every operator-visible environment variable mikebom reads at runtime, with at least one use case per variable. Includes `MIKEBOM_REQUIRE_SPDX3_VALIDATOR`, `MIKEBOM_UPDATE_CDX_GOLDENS`, `MIKEBOM_UPDATE_SPDX_GOLDENS`, `MIKEBOM_UPDATE_SPDX3_GOLDENS`, `MIKEBOM_PREPR_EBPF`, `MIKEBOM_NO_DEPRECATION_NOTICE`, plus any others surfaced by the audit.
- **FR-006**: The README MUST accurately describe what mikebom does, what it emits, and how to get started, at alpha.22. Stale claims MUST be removed or updated. The README's recipe count MUST stay ≤10 (operator-onboarding pages, not exhaustive references).
- **FR-007**: Per the 2026-05-07 Q1 clarification, voice and style MUST be normalized across **every in-scope markdown file** (all of `docs/user-guide/*.md`, `docs/reference/*.md` excluding test-fixture EXPECTED.md files, `docs/architecture/*.md`, `docs/index.md`, `docs/ecosystems.md`, `docs/design-notes.md`, and the root `README.md`) along three dimensions: (a) milestone-reference handling — pick one convention (inline `[milestone N]` vs. footnoted vs. omitted in operator-facing docs) and apply consistently; (b) code-block fence convention — language tags consistent (`bash`, `json`, `rust`, etc.) on every fenced block; (c) link convention — relative links to other docs/ pages, absolute links to external resources. The aggressive normalization scope produces a larger diff (~25+ files touched) but yields uniform voice across the doc surface for operators encountering any file first. Test-fixture READMEs (`docs/reference/binding-fixtures/*/EXPECTED.md`, `docs/examples/cross-tier-walk/README.md`) and research notes (`docs/research/*.md`) remain out of scope.
- **FR-008**: Per the 2026-05-07 Q2 clarification, architecture docs (`docs/architecture/*.md`) receive a **targeted spot-check audit**: per doc, the audit identifies 5–10 testable behavioral claims (specific assertions verifiable against alpha.22 source code) and verifies each. Stale spot-check claims MUST be fixed inline if the fix is ≤30 lines per claim; larger rewrites are filed as separate GitHub follow-up issues with the issue number recorded in research.md. Pure design-rationale prose that doesn't make verifiable behavioral claims is left unchanged. Q1's aggressive style pass independently touches each architecture doc for formatting + cross-reference fixes.
- **FR-009**: Documentation changes MUST NOT alter binary behavior, test outcomes, or any byte-identity goldens. The milestone is purely documentation; production code and tests stay untouched.
- **FR-010**: `docs/index.md` (the navigation entry point) MUST be updated to reflect any new files added or removed by this milestone. The "Two tracks" navigation pattern + the per-track file list MUST remain accurate.
- **FR-011**: Deprecated flags + features (e.g., `--include-dev` deprecated since milestone 052/part-3 per CLAUDE.md context) MUST be clearly marked as deprecated in the CLI reference with: deprecation date/milestone, replacement, removal target if scheduled. Operators reading the CLI reference MUST be able to identify deprecated flags at a glance.

### Key Entities

- **Documentation file**: A markdown file in `docs/` or the root README. Each file has: path, last-touched milestone (per git blame), classification (current/partially-stale/materially-stale/out-of-scope), audit findings, fix scope (small/large/follow-up).
- **CLI flag**: An operator-facing command-line option accepted by `mikebom sbom scan` or `mikebom trace run`. For this milestone's scope, "operator-facing" means listed by `--help` and not gated behind a `--debug-*` style internal namespace.
- **Cross-reference link**: A markdown relative-path link from one doc page to another. Cross-reference density is a measurable property of the doc graph (avg links per page).
- **Audit classification**: One of `current` (doc accurately reflects alpha.22) / `partially stale` (doc is mostly accurate but contains specific stale claims) / `materially stale` (doc describes pre-milestone-072 behavior with little post-072 updates) / `out-of-scope` (test fixture, research note, or other artifact not refreshed by this milestone).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of operator-facing CLI flags listed by every documented subcommand's `--help` output have entries in `docs/user-guide/cli-reference.md`. Subcommand coverage at alpha.22: `mikebom sbom scan`, `mikebom sbom verify`, `mikebom sbom enrich`, `mikebom trace run`, `mikebom trace capture`, `mikebom policy init`, `mikebom verify-binding`, `mikebom trace-binding`. Verified by `scripts/verify-docs-currency.sh` which diffs each subcommand's `--help` output against the doc's flag-name set; exit 0 = in sync.
- **SC-002**: 100% of `docs/reference/*.md` pages have a "See also" section. Verified by `grep -L '^## See also' docs/reference/*.md` returning empty.
- **SC-003**: From any reference doc, an operator can reach any other reference doc within ≤3 clicks. Verified by a graph-traversal smoke test (manual or scripted).
- **SC-004**: 100% of quickstart recipes succeed against the alpha.22 binary with output matching documented snippets. Verified by manual smoke against each recipe in the milestone's polish phase.
- **SC-005**: 100% of operator-visible environment variables mikebom reads at runtime are documented in `docs/user-guide/configuration.md`. Verified by `grep -roh 'MIKEBOM_[A-Z_]*' mikebom-cli/src/` against the doc's documented set.
- **SC-006**: README cover-to-cover review identifies zero stale claims about alpha.22 behavior. Verified by a maintainer (or self-review against the binary) during the milestone's polish phase.
- **SC-007**: Voice/style normalization applied consistently across user-guide + reference docs. Verified by a brief style-conformance checklist (3 dimensions × ≥80% conformance).
- **SC-008**: Per the 2026-05-07 Q2 clarification, architecture docs receive the targeted spot-check audit: the milestone's research.md lists 5–10 testable behavioral claims per architecture doc with per-claim verification status (verified / fixed inline / filed as follow-up issue #N). ≥90% per-claim accuracy post-fix.
- **SC-009**: Pre-PR gate stays clean (clippy + cargo test workspace — same gate as production milestones). Documentation changes MUST NOT cause test regressions, since the milestone is docs-only.
- **SC-010**: All deprecated flags clearly marked with deprecation milestone + replacement. Verified by reviewer cross-check against the CLI reference's deprecation table.
- **SC-011**: The audit deliverable (research.md) classifies every file in `docs/` + `README.md`. Verified by file-count match between the audit's list and `find docs -name '*.md' | wc -l` + 1 (for README).

## Assumptions

- Docs-only milestone. Production code, tests, and byte-identity goldens are NOT touched. Any code changes surfaced by audit findings are filed as separate GitHub issues for future milestones.
- Test-fixture READMEs (under `docs/reference/binding-fixtures/`, `docs/examples/cross-tier-walk/`) are out of scope. They're test artifacts; refreshing them dilutes the milestone's operator-facing focus.
- Research notes (`docs/research/go-binary-scope.md`) are out of scope unless audit reveals they're actively misleading operators.
- Architecture docs (`docs/architecture/*.md`) are read for currency but rewriting is out of scope; small inline fixes only.
- The audit deliverable in research.md is the single source-of-truth for what changes (vs. doesn't change). Reviewers + future maintainers reading the milestone can audit-trail every doc change against the audit's classification.
- Voice/style normalization is opinionated and applied across every in-scope file per the 2026-05-07 Q1 clarification. The milestone picks one convention per dimension during Phase 0 design and applies broadly; appeals are handled in PR review, not by re-running the spec.
- Milestone references in operator-facing docs (`user-guide/`) are minimized — operators don't care that `--root-name` came from milestone 077; they care what it does. Reference docs (`docs/reference/`) MAY retain milestone references as historical context for maintainers.
- Cross-reference graph density target: each reference doc has 2–5 "See also" links. Below 2 is too sparse; above 5 is link soup that loses signal.
- Quickstart recipes are bounded to ~10 (the README's stable-recipes count is the upper bound, mirroring the README); exhaustive coverage of every flag belongs in the CLI reference, not the quickstart.
- The milestone deliberately ships as a single PR. Splitting docs work by user-guide / reference / architecture would create transient states where some pages cross-reference others that haven't yet been refreshed.
- Existing milestones' deliverable docs (e.g., `docs/reference/sbom-types.md` from milestone 081, `docs/reference/sbom-format-mapping.md` Section I from milestones 080+081) are preserved — voice normalization may polish them, but their content is durable Principle V audit records, not refresh targets.
- The audit + refresh is bounded by the alpha.22 baseline. Future milestones (083+) refresh their own docs; this milestone doesn't pre-emptively cover hypothetical future surface.
