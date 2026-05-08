# Feature Specification: Transitive dependency correctness audit per ecosystem

**Feature Branch**: `083-transitive-correctness`
**Created**: 2026-05-07
**Status**: Draft
**Input**: GitHub issue #111 — "Transitive dependency correctness audit across all ecosystems"

## Overview

After milestones 053 (Go direct edges) + 054 (walker correctness fix) + 055 (Go transitive 4-step ladder including module-proxy fallback) closed the headline Go gap, the remaining SBOM-quality risk in mikebom is **transitive dependency correctness across the other ecosystems**. Direct edges (project's own manifest → declared dependencies) are reliable. Transitive edges (one or more hops deep) have varying levels of completeness depending on the ecosystem's lockfile structure, the per-ecosystem reader's coverage, and whether the source format encodes the indirect-vs-direct distinction.

The umbrella audit exists because **components-without-edges is a flat list**, not a graph. Operators consuming SBOMs for vulnerability propagation, license-compatibility analysis, supply-chain risk modeling, and attack-path reasoning need the GRAPH — which means transitive edges have to be both present AND correct. An SBOM with components-but-no-edges is useful for vuln-name-matching only; an SBOM with WRONG edges is actively misleading.

This milestone is **audit-first**: produce a per-ecosystem findings report by running mikebom + trivy + syft against real-world fixtures, diffing the resulting `relationships[]` (SPDX 2.3) / `dependencies[]` (CDX 1.6) arrays, and classifying each edge difference. Findings drive follow-up work: per-ecosystem child issues filed for any gap, regression tests pinning current behavior so future milestones don't silently regress edge correctness.

The deliberate scope: audit + report + regression tests + child-issue filings. Code fixes for any gaps surfaced are **out of scope for this milestone** — they ship as separate per-ecosystem follow-up milestones. This keeps the audit's diff bounded and lets each gap close get its own focused review.

Per-ecosystem state as of 2026-05-07 (from issue #111's table, updated for current milestone state):

| Ecosystem | Direct edges | Transitive source | Pre-audit hypothesis |
|---|---|---|---|
| **Go** | go.mod requires (053) | 4-step ladder: `go mod graph` / `$GOMODCACHE` / proxy fetch / no-edges (055) | Closed; verify edges match `go mod graph` ground truth on the fixture |
| **Cargo** | Cargo.lock direct | Cargo.lock `dependencies = [...]` per `[[package]]` block | Likely OK — Cargo.lock encodes full closure structurally; verify reader emits every edge |
| **npm** | package-lock.json `packages.""` | package-lock.json `packages[].dependencies` | Likely OK for lockfile-v3; verify nested-edge reader |
| **Maven** | pom.xml `<dependencies>` | pom.xml + parent POM chain + dependencyManagement + deps.dev fallback | Partially complete — parent-POM resolution is complex; deps.dev fallback coverage uneven |
| **pip** | requirements.txt / pyproject.toml | None for plain requirements.txt; poetry.lock has edges; Pipfile.lock has edges | Plain pip without a lockfile has no transitive structure encoded — document the limitation |
| **gem** | Gemfile.lock direct | Gemfile.lock GEM block per-spec dependencies | Likely OK — verify reader |
| **dpkg / rpm / apk** | Depends field | Depends field | Likely OK — OS-package edges via Depends: are well-tested |

The audit confirms or refutes each hypothesis with primary-source evidence: real fixtures + side-by-side comparison against trivy + syft. Operators reading the audit's findings report can interpret mikebom's per-ecosystem accuracy guarantees without reading source code.

## Clarifications

### Session 2026-05-07

- Q: How are the per-ecosystem real-world fixtures stored — vendored in-tree, fetched at audit/test time via URL+commit-pin, or generated synthetically? → A: **Vendor minimal fixtures in-tree.** Each per-ecosystem fixture lives at `mikebom-cli/tests/fixtures/transitive_parity/<ecosystem>/` with the relevant manifest + lockfile only (e.g., for Cargo: `Cargo.toml` + `Cargo.lock`; for npm: `package.json` + `package-lock.json`; for Maven: `pom.xml` + parent POMs needed to satisfy `<parent>` resolution; not the source tree). Bounded growth (~10–50 KB per ecosystem × 9 ecosystems ≈ <500 KB total repo growth). Tests are reproducible offline. Each fixture's research.md row documents the source-of-truth URL + commit SHA the manifest was extracted from, so a maintainer can reconstruct the full tree if needed for trivy/syft execution against the same baseline.
- Q: When comparing mikebom's edge set against ground truth, what counts as "ground truth"? → A: **Trivy + syft as comparison baseline + source-format direct read as tiebreaker when the three SBOM tools disagree.** The per-ecosystem audit primarily compares 3 SBOM tools (mikebom + trivy + syft); when the three disagree on a specific edge, the audit re-derives ground truth from the source format directly (parse the lockfile/manifest ourselves; for OS package managers, query the native tool — `dpkg-query`, `rpm -q`, `apk info`). This catches BOTH the case where a peer tool has a known bug AND the case where mikebom has a known bug. Matches milestone 079's pattern (mikebom's own schema fixture validated decisions, not just peer-tool consensus). Audit-row format extends to: mikebom edge count, trivy edge count, syft edge count, source-format direct-read count (when invoked), tiebreaker resolution (when invoked).

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Maintainer can identify per-ecosystem transitive-edge gaps from a single findings report (Priority: P1)

A mikebom maintainer (or an operator evaluating mikebom for procurement) reads the audit's findings report and learns, per ecosystem, the answer to: (a) does mikebom emit transitive edges? (b) how does coverage compare against trivy + syft? (c) are there false-positive edges (mikebom emits edges trivy/syft doesn't)? (d) are there false-negative edges (mikebom misses edges trivy/syft find)? (e) what fixture was the audit run against (so future maintainers can re-run + re-verify)?

**Why this priority**: This IS the milestone's central deliverable. Without it, no informed decision on which ecosystem to prioritize for follow-up fixes. P1 because the audit's findings report is the input every other deliverable depends on.

**Independent Test**: a reviewer reads `specs/083-transitive-correctness/research.md` (the audit's findings appendix) for any of the 8 in-scope ecosystems and identifies (a) the fixture used, (b) the per-tool edge counts, (c) the diff classification (matches/missing/extra), (d) any follow-up GitHub issue filed for the ecosystem.

**Acceptance Scenarios**:

1. **Given** the audit report, **When** the reviewer searches for any in-scope ecosystem (Go, Cargo, npm, Maven, pip, gem, dpkg, rpm, apk — 9 ecosystems), **Then** an entry exists with the structure: fixture used, per-tool edge counts, diff classification, follow-up disposition.
2. **Given** a per-ecosystem entry classified as "matches expected", **When** a maintainer reads it, **Then** the entry cites at least one objective metric (e.g., "mikebom emits N edges, trivy emits N±k% on the same fixture") justifying the classification.
3. **Given** a per-ecosystem entry classified as "gap surfaced", **When** the maintainer reads it, **Then** the entry includes a follow-up GitHub issue number with a reference to the specific edge cases the gap covers.

---

### User Story 2 — Each per-ecosystem audit pins current behavior in a regression test (Priority: P1)

To prevent silent regressions in transitive-edge correctness as future milestones land, each ecosystem's audit produces a regression test under `mikebom-cli/tests/transitive_parity_<ecosystem>.rs` that pins the expected edge set against the real-world fixture. Future maintainers touching scan logic for that ecosystem MUST keep the test passing or explicitly bump the expected edge set with documented rationale.

**Why this priority**: Audit findings without regression tests rot — the next milestone touching the per-ecosystem reader silently breaks edge correctness without anyone noticing. P1 because the test pins the audit's value durably.

**Independent Test**: A maintainer modifies a per-ecosystem reader (e.g., changes `mikebom-cli/src/scan_fs/package_db/cargo.rs`'s edge-emission logic to drop transitive edges); the corresponding regression test fails with a clear "expected N edges, got M" diff.

**Acceptance Scenarios**:

1. **Given** the milestone post-fix, **When** `cargo +stable test --test transitive_parity_cargo` runs, **Then** the test passes against the pinned fixture's expected edge set.
2. **Given** a deliberately-introduced regression in any per-ecosystem reader (test scenario), **When** the corresponding `transitive_parity_<ecosystem>` test runs, **Then** the test fails with a clear edge-count + edge-list diff.
3. **Given** the milestone's regression-test set, **When** the developer runs `cargo test --workspace`, **Then** all per-ecosystem `transitive_parity_*` tests pass on the alpha.23 baseline.

---

### User Story 3 — Indirect-vs-direct dependency distinction decision per ecosystem (Priority: P2)

Some source formats distinguish indirect dependencies from direct ones (Go's `// indirect` marker in `go.mod`; npm's `devDependencies` vs `dependencies`; Cargo's `[dev-dependencies]` vs `[dependencies]`). mikebom currently emits all requires under root the same way. trivy tags `// indirect` distinctly via `RelationshipIndirect`. The audit decides per-ecosystem whether to (a) implement the distinction in mikebom, (b) document as deliberate divergence with rationale per Constitution Principle V, or (c) defer for a follow-up milestone.

**Why this priority**: Lower priority because mikebom's current "all-edges-under-root" emission is operator-comprehensible and isn't actively wrong — it just misses a finer-grained signal that some downstream tools (e.g., dependency-track filtering on `RelationshipIndirect`) can consume. P2 because the per-ecosystem decisions are bounded (1–2 lines of audit-record per ecosystem) and don't block US1 or US2.

**Independent Test**: The audit's findings report has a per-ecosystem column showing the indirect-vs-direct decision; future SBOM-consumer tools can read mikebom's documented stance.

**Acceptance Scenarios**:

1. **Given** the audit's per-ecosystem table, **When** a reviewer searches for the indirect-vs-direct decision for any ecosystem that supports the distinction natively (Go, npm, Cargo), **Then** an entry exists: implement / document-as-divergence / deferred.
2. **Given** an "implement" decision, **When** the maintainer reads it, **Then** the entry cites the milestone or follow-up issue scheduled to ship the distinction.
3. **Given** a "document-as-divergence" decision, **When** the maintainer reads it, **Then** the entry cites the rationale per Constitution Principle V's standards-native-precedence requirement.

---

### User Story 4 — Per-ecosystem follow-up issues filed for any gap surfaced (Priority: P2)

For each per-ecosystem gap the audit surfaces, a GitHub issue is filed with the gap's specifics: fixture used, per-tool edge counts, missing edges, suggested fix shape, scope estimate. Operators OR maintainers can pick up the follow-up later as standalone milestones.

**Why this priority**: P2 because filing issues is administrative; the audit's findings IS the value. The issue filings are the durable record of "this is what we know needs fixing."

**Independent Test**: For each per-ecosystem audit row classified as "gap surfaced", a corresponding GitHub issue exists with the audit's findings copied into the issue body.

**Acceptance Scenarios**:

1. **Given** the milestone PR's research.md, **When** a reviewer counts the per-ecosystem entries classified as "gap surfaced", **Then** each one has a corresponding GitHub issue linked.
2. **Given** any filed follow-up issue, **When** a future maintainer reads it cold, **Then** they can reproduce the gap by following the issue's "fixture used" + "reproduction" steps.

---

### Edge Cases

- **Vendored fixtures must remain bounded** per the 2026-05-07 Q1 clarification: only manifests + lockfiles are vendored, never the source tree. For npm specifically, this means `package.json` + `package-lock.json` only (NOT the `node_modules/` directory which would be tens of MB). Per-tool edge counts in the audit row reference re-running trivy/syft against the source-of-truth URL + commit, not against the vendored manifest-only fixture (since trivy/syft typically need the full tree to derive edges). The vendored manifest-only fixture is for **mikebom's own regression test**, not for trivy/syft side-by-side execution.
- **trivy or syft don't support an ecosystem mikebom does** (e.g., RPM source-tier scanning may not map cleanly to syft's package model): document the comparison gap explicitly. Audit row says "trivy/syft N/A for this ecosystem"; per the 2026-05-07 Q2 clarification, mikebom's edge correctness is verified against the source-format direct-read tiebreaker (the lockfile / manifest / native package-manager tool) used unconditionally when trivy/syft don't apply, rather than only as a tiebreaker between disagreeing SBOM tools.
- **Fixture edge count fluctuates between trivy/syft versions**: pin trivy + syft to specific versions in the audit's research.md. Future re-audits use the same pinned versions or document the change.
- **Pip plain requirements.txt has no transitive structure encoded** (issue #111 documents this): the audit row for pip plain notes this is not a mikebom gap but an upstream limitation. Pipfile.lock + poetry.lock cases are evaluated separately.
- **Indirect-vs-direct decision per ecosystem may not be unanimous**: e.g., implement the distinction for Go and npm (where the source format encodes it natively) but document-as-divergence for Cargo (where the distinction lives in the manifest's section labels, not in the lockfile, and emitting it requires re-reading sources). Per-ecosystem decisions are independent.
- **OS package managers (dpkg/rpm/apk) have a different edge-correctness model**: they're not source-language ecosystems with lockfiles; their edges come from binary package metadata. Per the 2026-05-07 Q2 clarification, the source-format direct-read tiebreaker for OS packages is the native tool's query output: `dpkg-query --show -f='${Package} ${Depends}\n'` for dpkg, `rpm -q --requires` for rpm, `apk info -R` for apk. The audit compares against trivy's OS-package output as the comparison baseline (syft's OS-package coverage is weaker and is N/A for many cases), with the native tool as tiebreaker.
- **A regression test pinned at alpha.23 may need to bump expected edge counts after a future milestone deliberately changes edges**: the test failure is the right gate; bumping the expected count is a documented, deliberate change in the future milestone's PR (mirrors the existing byte-identity-golden bump pattern from milestones 077–082).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The audit MUST produce a per-ecosystem findings appendix in `specs/083-transitive-correctness/research.md` covering all 9 in-scope ecosystems (Go, Cargo, npm, Maven, pip, gem, dpkg, rpm, apk). Each ecosystem's row MUST contain: fixture name + source URL + commit-pinned tag, mikebom edge count, trivy edge count, syft edge count, source-format direct-read edge count (when the three SBOM tools disagree per the 2026-05-07 Q2 clarification — invoked as a tiebreaker), diff classification (matches / minor differences / gap surfaced), tiebreaker resolution (when invoked: which tool was correct), follow-up disposition.
- **FR-002**: For each in-scope ecosystem, a real-world fixture MUST be selected with ≥50 components AND ≥100 edges. Per the 2026-05-07 Q1 clarification, fixtures are **vendored in-tree** at `mikebom-cli/tests/fixtures/transitive_parity/<ecosystem>/` containing only the manifest + lockfile content (no source tree). Each fixture's audit-record row in research.md MUST cite the source URL + commit SHA the manifest was extracted from, so the audit is reproducible AND so trivy/syft can be re-run against the original repo if a reviewer wants to re-verify per-tool edge counts. Total repo growth bounded to ~500 KB across the 9 fixtures.
- **FR-003**: Each per-ecosystem fixture MUST have a regression test at `mikebom-cli/tests/transitive_parity_<ecosystem>.rs` that pins the expected edge count + edge set (sample of 10–20 representative edges) against alpha.23 baseline. The regression test MUST fail when the per-ecosystem reader's emission changes the edge set without explicit test bumping.
- **FR-004**: For each ecosystem where the source format encodes the indirect-vs-direct distinction (Go, npm, Cargo), the audit MUST decide per-ecosystem: (a) implement the distinction in mikebom (file follow-up issue), (b) document as deliberate divergence with Principle V rationale, OR (c) defer with documented reason.
- **FR-005**: For each per-ecosystem gap surfaced (audit row classified as "gap surfaced"), a corresponding GitHub follow-up issue MUST be filed with: fixture used, per-tool edge counts, the specific missing/extra edges, suggested fix shape, scope estimate.
- **FR-006**: The audit MUST pin the trivy + syft versions used for the comparison. Future re-audits use the same pinned versions OR document the version change.
- **FR-007**: The milestone's regression test set MUST pass on the alpha.23 baseline as a precondition for merge. Failures indicate either (a) the audit's expected-edge-set is wrong (fix the test) or (b) mikebom's emission changed since the audit ran (investigate).
- **FR-008**: Pip's "plain requirements.txt" case MUST be documented as an upstream limitation (no transitive structure encoded in the source format) rather than a mikebom gap. Pipfile.lock + poetry.lock cases MUST be evaluated separately with their own per-tool comparisons.
- **FR-009**: OS package managers (dpkg, rpm, apk) MUST be evaluated against trivy's OS-package output rather than language-tool output. Audit rows reflect this with a different comparison column structure.
- **FR-010**: Code fixes for any gaps surfaced MUST be **out of scope** for this milestone. The audit produces findings + filings + regression tests; per-ecosystem fixes ship as separate follow-up milestones referenced from the filed issues.
- **FR-011**: The milestone's pre-PR gate MUST stay clean: clippy zero warnings; cargo test workspace all `0 failed` including the new `transitive_parity_*` regression tests; `cdx_regression`, `spdx_regression`, `spdx3_regression` all pass without their `MIKEBOM_UPDATE_*_GOLDENS` env vars (the audit doesn't change emission shape; goldens stay byte-identical).

### Key Entities

- **EcosystemAuditRow**: One row in `research.md` per in-scope ecosystem. Composed of: ecosystem name, fixture name + commit SHA + URL, mikebom edge count, trivy edge count, syft edge count, diff classification, indirect-vs-direct decision (if applicable), follow-up GitHub issue # (if a gap surfaced).
- **EdgeDiff**: A per-ecosystem comparison artifact — the set of edges emitted by mikebom XOR the set emitted by trivy XOR the set emitted by syft, classified per-edge as "agreement", "mikebom-only false-positive candidate", "trivy-only false-negative candidate", "syft-only false-negative candidate", etc.
- **TransitiveParityTest**: One regression test per ecosystem. Pins the alpha.23-baseline edge set against the per-ecosystem fixture so future milestones don't silently regress.
- **FollowUpIssue**: A GitHub issue filed per gap surfaced. Composed of: fixture reference, per-tool edge counts, specific missing/extra edges with examples, suggested fix shape, scope estimate.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the 9 in-scope ecosystems have an audit row in `specs/083-transitive-correctness/research.md`. Verified by file-count match between the audit's row count and the in-scope ecosystem list.
- **SC-002**: 100% of the 9 in-scope ecosystems have a regression test at `mikebom-cli/tests/transitive_parity_<ecosystem>.rs`. Verified by directory listing.
- **SC-003**: 100% of the regression tests pass on the alpha.23 baseline. Verified by `cargo +stable test --test 'transitive_parity_*'` exit 0.
- **SC-004**: Each ecosystem where the source format encodes the indirect-vs-direct distinction (Go, npm, Cargo) has an explicit per-ecosystem decision (implement / document-as-divergence / deferred). Verified by audit-row inspection.
- **SC-005**: Each per-ecosystem gap surfaced has a GitHub follow-up issue filed with the audit's findings copied into the issue body. Verified by issue-link presence in the audit row.
- **SC-006**: Trivy + syft versions are pinned in research.md. Verified by version-line presence.
- **SC-007**: Pip plain requirements.txt is documented as upstream limitation (not mikebom gap). Verified by audit-row inspection.
- **SC-008**: A maintainer reading the audit cold (no prior context) can identify within 60 seconds for any in-scope ecosystem: (a) fixture used, (b) per-tool edge counts, (c) diff classification, (d) follow-up disposition. Verified by audit-row inspection sample.
- **SC-009**: Pre-PR gate clean — clippy zero warnings; cargo test workspace all `0 failed`; cdx/spdx/spdx3 regression tests pass without their `MIKEBOM_UPDATE_*_GOLDENS` env vars (audit doesn't change emission shape). Verified by gate run.
- **SC-010**: A future maintainer modifying a per-ecosystem reader (`mikebom-cli/src/scan_fs/package_db/<ecosystem>.rs`) cannot silently regress transitive-edge correctness — the corresponding `transitive_parity_<ecosystem>` test fails on any edge-set delta. Verified by deliberate-regression smoke test for at least 1 ecosystem.

## Assumptions

- **Audit-first scope**: this milestone produces findings + regression tests + filings. Code fixes for any gaps surfaced are out-of-scope and ship as separate per-ecosystem follow-up milestones (analogous to milestone 081's runtime-tier-deferral pattern, where the audit names the gap but the fix lives elsewhere).
- **Real-world fixtures are vendored in-tree as manifest+lockfile only** per the 2026-05-07 Q1 clarification, with source-of-truth URL + commit SHA cited in each per-ecosystem audit row. The audit's value depends on the fixtures being representative of actual operator scans; the vendor strategy keeps repo growth bounded while preserving reproducibility.
- **Trivy + syft are installed and on PATH** during audit execution. The milestone's regression tests gracefully skip if either binary is missing (analogous to milestone 078's spdx3-validate graceful-skip pattern); CI runs install both unconditionally.
- **Trivy + syft versions are pinned at audit time** to the latest stable releases as of 2026-05-07. Future re-audits either use the same pins or document the version bump.
- **Edge-set comparison is done at the SBOM-content level**, not by re-running the per-ecosystem readers. mikebom emits SPDX 3 / CDX 1.6; trivy + syft emit the same formats; the audit diffs the relationships/dependencies arrays. Mikebom's internal data structures are not directly compared against trivy's or syft's internal data structures. Per the 2026-05-07 Q2 clarification, when the three SBOM tools disagree on a specific edge, the source-format direct-read tiebreaker (parse the lockfile/manifest ourselves, or query the native OS package-manager tool) determines correctness.
- **The 9-ecosystem in-scope list is exhaustive for the alpha.23 surface**: Go, Cargo, npm, Maven, pip, gem, dpkg, rpm, apk. Future ecosystems mikebom may add (e.g., Swift, .NET, conda) get audited in their own follow-up milestones.
- **Fixtures that fail to scan cleanly are filed as separate bugs** rather than silently excluded from the audit. If mikebom panics or errors on a real-world fixture, that's the gap — the audit row records "scan failed" + filed issue, and continues.
- **The milestone deliberately ships as a single PR**. Splitting per-ecosystem audits into separate PRs would create transient states where some ecosystems are audited and others aren't; reviewers benefit from seeing the comparative-completeness picture in one diff.
- **Indirect-vs-direct decisions are per-ecosystem and independent**: implementing it for Go and npm doesn't force the same for Cargo. Each decision is justified individually in the audit.
- **OS package manager comparison is a different shape**: dpkg/rpm/apk compare against trivy's OS-package output (which is well-tested) rather than syft (which has weaker OS-package support). Audit rows reflect this asymmetric comparison.
- **The audit may surface ZERO gaps** (best case): every ecosystem matches trivy + syft. In that case, the milestone's deliverable is the regression tests + the audit's positive-confirmation findings + filings of indirect-vs-direct decisions only. Still high-leverage because durable regression coverage now exists.
