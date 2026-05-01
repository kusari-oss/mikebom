# Feature Specification: Self-describe SBOM scope (README explainer + SPDX creationInfo.comment)

**Feature Branch**: `047-scope-self-description`
**Created**: 2026-04-30
**Status**: Draft
**Input**: User description: "Add a 'What kind of SBOM does mikebom emit?' README section explaining the artifact/manifest scope axis + per-component sbom-tier system + CDX lifecycles aggregation, plus SPDX creationInfo.comment document-level scope hint for parity with CDX metadata.lifecycles"

## Background

A pre-spec audit verified that mikebom **already self-describes
lifecycle scope** through three independent channels:

- CDX `metadata.lifecycles[]` (CDX 1.6 native, aggregated from
  per-component tiers; phases observed in goldens: `pre-build`,
  `operations`).
- CDX `compositions[]` (CDX 1.6 native, with
  `aggregate: complete | incomplete_first_party_only` plus
  `assemblies[]` and `dependencies[]` per aggregate).
- The `mikebom:sbom-tier` annotation on every component, in all
  three formats (CDX property + SPDX 2.3 annotation + SPDX 3
  annotation; `tier` values: `design`, `source`, `build`,
  `deployed`, `analyzed`).

What's missing is **not** another emission channel — it's:

1. **A reader-facing explanation** of how those signals map to
   the question "what kind of SBOM is this?" Today users have to
   reverse-engineer the intent from the component list, comparing
   counts to other tools (trivy, syft) without knowing whether
   they're asking the same question. The README has no section
   that names mikebom's scope axes or how to read the lifecycle /
   composition / tier signals.
2. **SPDX-side parity for document-level scope hint.** SPDX 2.3's
   `creationInfo` struct currently emits only `created`,
   `creators`, and (sometimes) `licenseListVersion`. The
   `creationInfo.comment` field is unused. SPDX consumers parsing
   only metadata (without walking the per-component
   `mikebom:sbom-tier` annotation) get strictly less scope
   information than CDX consumers reading
   `metadata.lifecycles[]`. SPDX 3 has the same gap on its
   `CreationInfo` element.

This milestone closes both.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — README explainer for "what kind of SBOM is this?" (Priority: P1) 🎯 MVP

An operator runs `mikebom sbom scan --image alpine:3.19` and gets
a CycloneDX with N components. The same image, scanned with trivy
or syft, produces a different N. Without a project-side
explanation of mikebom's scope choices, the operator can't tell
whether the difference is mikebom undercounting, the other tool
overcounting, or both tools answering different questions
correctly.

The README needs a short, opinionated section that names the two
axes mikebom uses today and tells the reader how to interpret the
signals it emits.

**Why this priority**: closes a documentation gap that's been
audibly costing user trust ("is mikebom undercounting?"). Pure
docs work; cheap; high leverage.

**Independent Test**: After the milestone ships, README contains
a section whose heading matches `what kind of SBOM` (case-insensitive
grep), naming at minimum:

- The artifact-vs-manifest scope axis and how
  `--include-declared-deps` toggles it (auto-defaults differ by
  `--path` vs `--image`).
- The per-component `mikebom:sbom-tier` system and what each
  tier means.
- The CDX `metadata.lifecycles[]` aggregation and CDX
  `compositions[]` self-description signals — and the SPDX
  parity slots for both.
- A short mapping to NTIA / industry lifecycle terminology so
  readers comparing mikebom against other tools have a shared
  vocabulary.

**Acceptance Scenarios**:

1. **Given** an operator who just scanned an image and is
   confused why mikebom's component count differs from trivy's,
   **When** they read the README's "What kind of SBOM does
   mikebom emit?" section, **Then** they can name (a) which
   scope mikebom emits by default for `--image` scans, (b) how
   to switch scope, and (c) which CDX/SPDX field tells a
   downstream tool what scope this document represents.
2. **Given** a developer integrating mikebom into a pipeline
   that already uses trivy or syft, **When** they read the
   README, **Then** the section explicitly names how mikebom's
   "artifact" scope corresponds to common lifecycle terminology
   (post-build / runtime / deployment) so the developer can
   choose between tools rather than treating them as
   interchangeable.

---

### User Story 2 — SPDX document-level scope hint via `creationInfo.comment` (Priority: P2)

A consumer parses an SPDX 2.3 document mikebom produced and reads
only `creationInfo` to identify the document's provenance. Today
that block carries `created` (timestamp), `creators` (`Tool:
mikebom-<version>` plus `Organization: …`), and sometimes
`licenseListVersion`. It carries **nothing** about which scope
the document represents — that information lives on per-component
annotations the consumer must walk to discover.

The `creationInfo.comment` field exists in the SPDX 2.3 spec for
exactly this kind of document-level hint. SPDX 3 has an analogous
slot on its `CreationInfo` element. Populating both with a short
structured note closes the metadata-only-consumer gap.

**Why this priority**: SPDX-side parity with CDX
`metadata.lifecycles[]`. Smaller user surface than US1 (most
operators use CDX); not blocking. Bundles cleanly with US1
because the README explainer can name `creationInfo.comment` as
the SPDX equivalent in its cross-format mapping.

**Independent Test**: After the milestone ships:

- `jq -r '.creationInfo.comment // empty' <any-spdx-2.3-output>`
  returns a non-empty string.
- The string contains, at minimum: (a) the scope mode (artifact
  or manifest), (b) the lifecycle phases observed (mirroring CDX
  `metadata.lifecycles[]`), and (c) a short pointer to the
  per-component `mikebom:sbom-tier` annotation for the
  full per-component breakdown.
- SPDX 3 has equivalent self-description on its `CreationInfo`
  element (slot TBD in plan; either `comment` if SPDX 3 has one,
  or the document's top-level `Element/comment`).

**Acceptance Scenarios**:

1. **Given** a consumer parsing only `creationInfo` from an
   SPDX 2.3 document mikebom produced, **When** they read
   `creationInfo.comment`, **Then** they learn the scope mode
   and the lifecycle phases without needing to walk every
   `packages[].annotations[]` entry.
2. **Given** a CDX consumer who already reads
   `metadata.lifecycles[]` for scope context, **When** they
   compare the same scan's SPDX 2.3 output, **Then**
   `creationInfo.comment` carries equivalent (not byte-identical)
   information — phases agree, scope mode agrees.
3. **Given** an SPDX 3 consumer, **When** they read the
   document's `CreationInfo`, **Then** the same scope hint is
   present (in whichever slot SPDX 3 designates for free-text
   metadata commentary).

---

### Edge Cases

- **Empty per-component tiers**: if no component carries an
  `sbom_tier` value (atypical — most ecosystems set one), the
  CDX `metadata.lifecycles[]` array is omitted today. The SPDX
  `creationInfo.comment` should also degrade gracefully —
  either omit the comment, or emit a comment that names the
  scope mode without phase info. Goal: the comment is never
  misleading; absence is acceptable.
- **README + per-format docs duplication**: the README
  explainer is the canonical user-facing description. Avoid
  duplicating it verbatim in `docs/user-guide/cli-reference.md`
  — link instead, so updates have one source of truth.
- **NTIA terminology mapping**: NTIA's "minimum elements"
  document doesn't itself define lifecycle phases (it defines
  data fields). The recommendation cited NTIA loosely; the
  README explainer should use CDX 1.6's phase names (which
  align with the broader industry convention)
  rather than inventing a parallel taxonomy.
- **Parity-extractor framework**: the existing per-component
  parity tests (`mikebom-cli/tests/holistic_parity.rs`) cover
  C-row catalog annotations. Document-level metadata isn't in
  the catalog; this milestone's emissions are intentionally
  outside the parity matrix. No new catalog rows; no
  `holistic_parity` regression risk.

## Requirements *(mandatory)*

### Functional Requirements

#### US1 — README explainer

- **FR-001**: `README.md` MUST contain a section whose heading
  matches `what kind of SBOM` (case-insensitive). The section
  is reachable from the README's documentation table of
  contents.
- **FR-002**: The section MUST cover (in any reasonable order):
  (a) the artifact-vs-manifest scope axis, naming
  `--include-declared-deps` and the auto-default rule
  (`--path` → manifest, `--image` → artifact); (b) the
  per-component `mikebom:sbom-tier` system listing all five
  current tier values (`design`, `source`, `build`,
  `deployed`, `analyzed`) with a one-line definition each;
  (c) the CDX `metadata.lifecycles[]` aggregation and SPDX
  `creationInfo.comment` SPDX equivalent (after US2 lands);
  (d) at least a one-paragraph mapping to industry / consumer
  lifecycle terminology so cross-tool comparisons aren't
  miscalibrated.
- **FR-003**: The section MUST explicitly state that mikebom's
  default scope for `--image` is artifact (on-disk components
  only) so operators comparing component counts to other
  tools have a clear "we deliberately answer a tighter
  question" framing — without claiming the other tools are
  wrong.

#### US2 — SPDX document-level scope hint

- **FR-004**: SPDX 2.3 output MUST carry a non-empty
  `creationInfo.comment` field describing the scope.
- **FR-005**: The comment MUST name (a) the scope mode
  (artifact or manifest, derived from the resolution of
  `--include-declared-deps` for that scan), (b) the
  lifecycle phases observed (mirroring the CDX
  `metadata.lifecycles[]` set the same scan emits), (c) a
  pointer to the per-component `mikebom:sbom-tier`
  annotation for finer-grained per-component scope.
- **FR-006**: SPDX 3 output MUST carry equivalent
  document-level self-description in its `CreationInfo` (slot
  decision deferred to the plan; the plan picks
  `comment` if it exists in SPDX 3 or the top-level
  document's `Element/comment` field, whichever the SPDX 3
  spec designates as the canonical free-text metadata slot).
- **FR-007**: Comment text format SHOULD be human-readable
  prose (not JSON-as-string) so operators reading raw SPDX
  output can absorb the hint at a glance. Structured
  machine-parseable form is out of scope; if downstream tools
  need structured data, they walk the existing
  per-component `mikebom:sbom-tier` annotations.
- **FR-008**: The comment MUST NOT carry per-component
  detail. It's strictly a document-level summary; the
  per-component breakdown is the job of the existing
  `mikebom:sbom-tier` annotation.

#### Cross-cutting

- **FR-009**: No new CLI flags. No changes to `--include-declared-deps`
  semantics. No changes to per-component
  `mikebom:sbom-tier` emission.
- **FR-010**: SPDX 2.3 + SPDX 3 byte-identity goldens regen
  cleanly. The added `creationInfo.comment` is the only
  semantic delta in the affected goldens; no other field
  shifts.
- **FR-011**: CDX goldens DO NOT change. CDX already
  self-describes via `metadata.lifecycles[]` and
  `compositions[]`; this milestone adds nothing to CDX.
- **FR-012**: `holistic_parity` tests remain green. Document-
  level metadata is outside the per-component catalog matrix.
- **FR-013**: Pre-PR gate (`./scripts/pre-pr.sh`) clean.

### Key Entities

- **Scope mode**: a binary attribute of any single mikebom
  scan, derived from the resolution of
  `--include-declared-deps`. Values: `artifact` (on-disk
  presence required for emission) or `manifest` (declared
  transitives + .pom-cached coords also emitted). Already
  computed today; this milestone surfaces it in human-readable
  metadata.
- **Lifecycle phases set**: the union of CDX 1.6 phase names
  observed across the scan's components (today: subset of
  `design`, `pre-build`, `build`, `post-build`, `operations`).
  Already aggregated for `metadata.lifecycles[]`; this
  milestone reuses the same aggregation for the SPDX comment.

## Success Criteria *(mandatory)*

### Measurable Outcomes

#### US1

- **SC-001**: A grep for `[Ww]hat kind of SBOM` in `README.md`
  matches at least one heading (`#`-style heading line).
- **SC-002**: The new section names all five sbom-tier values
  AND names the scope-mode toggle flag explicitly.

#### US2

- **SC-003**: After running `mikebom sbom scan
  --format spdx-2.3-json --path tests/fixtures/<any>`,
  `jq -r '.creationInfo.comment' <output>` returns a string
  containing the substring `scope:` (or equivalent
  designator) AND the substring `mikebom:sbom-tier`.
- **SC-004**: Equivalent assertion holds for SPDX 3 output
  (location TBD per FR-006).
- **SC-005**: All 9 SPDX 2.3 goldens regen with the new
  comment field present and otherwise byte-identical to
  pre-milestone output.

#### Cross-cutting

- **SC-006**: `git diff main..HEAD -- mikebom-cli/tests/fixtures/golden/cyclonedx/`
  is empty (CDX goldens unchanged per FR-011).
- **SC-007**: `./scripts/pre-pr.sh` clean. All 3 CI lanes
  green on the milestone PR.
- **SC-008**: A second pass against the audit findings finds
  zero remaining SPDX-comment-emission gaps in the
  scope-self-description category.

## Assumptions

- The audit's findings about current self-describing
  emissions (`metadata.lifecycles[]`, `compositions[]`,
  `mikebom:sbom-tier`) are accurate and don't need
  re-verification before this milestone ships. The audit
  cited specific files (`generate/cyclonedx/metadata.rs:218`,
  `generate/cyclonedx/compositions.rs`,
  `generate/spdx/annotations.rs:141`) and confirmed via
  goldens.
- The artifact-vs-manifest scope axis is the right primary
  framing in the README explainer. The five
  `mikebom:sbom-tier` values are the secondary
  per-component framing. The CDX phase names are the tertiary
  "where does this end up in the SBOM document" framing. The
  README explainer threads all three into a single
  consistent narrative; if a reviewer prefers a different
  primary framing, that's an editorial discussion within
  this milestone, not a scope change.
- SPDX 3's free-text comment slot is on `CreationInfo`
  similarly to SPDX 2.3, OR on the top-level
  `SpdxDocument` element as `comment`. Plan phase will
  resolve which.
- No CHANGELOG entry needed for the README explainer (US1) —
  pure docs. US2 (the SPDX comment) IS user-visible (SPDX
  consumers will see new metadata), so it warrants a small
  CHANGELOG entry.

## Out of scope

- Adding a new CLI flag (e.g., `--sbom-scope` or
  `--sbom-scope all`). The existing `--include-declared-deps`
  already controls scope; this milestone surfaces existing
  state, doesn't add new state.
- Walking host `~/.m2/repository/` wholesale to emit every
  cached JAR as a component. The recommendation that
  prompted this milestone proposed this as a "build scope"
  emission; the audit found mikebom deliberately does not
  do this today (rootfs `.m2/` is walked, host `~/.m2/` is
  used only for resolution data). Whether to change that is
  a separate conversation.
- Multi-document emission (`--sbom-scope all` style: emit
  artifact + manifest in one run). Could be a follow-on if
  user demand surfaces; today users get both views by
  running mikebom twice.
- Reachability analysis (the recommendation's "runtime"
  scope). Defer; not scoped to anything concrete yet.
- Conformance-suite changes (per-fixture `expected_sbom_scope`
  ground-truth, `MISMATCHED_SBOM_SCOPE` finding kind). These
  belong in the sbom-conformance repo, not this one. Mark as
  follow-on for that repo's maintainer.
- CDX-side document-level scope-mode property
  (`metadata.properties[]` entry for `mikebom:scan-mode`).
  CDX already has `metadata.lifecycles[]`; adding a parallel
  property would duplicate existing self-description. If
  later experience shows lifecycles isn't enough, scope a
  separate milestone.
