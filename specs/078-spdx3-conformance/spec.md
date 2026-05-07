# Feature Specification: SPDX 3.0.1 conformance pass

**Feature Branch**: `078-spdx3-conformance`
**Created**: 2026-05-06
**Status**: Draft
**Input**: User description: "Let's make sure we're 100% conformant to SPDX 3. I'm seeing 'Analysis exception processing SPDX file: Incompatible type for property https://spdx.org/rdf/3.0.1/terms/Core/createdBy: class org.spdx.library.model.v3_0_1.core.Agent' from an external Java SPDX library when consuming our output. The JPEWdev `spdx3-validate` Python tool and the official SPDX 3 JSON-LD validation rules at https://github.com/spdx/spdx-3-model/blob/develop/serialization/jsonld/validation.md are the authoritative validators. I really don't know if this is accurate or if we're accurate."

## Overview

Today, mikebom's SPDX 3.0.1 emission path (introduced in milestone 011 + 017, last touched by 077) is gated only by JSON Schema validation against the SPDX 3.0.1 JSON schema. JSON Schema enforces structural shape (required fields, value types at the JSON-primitive level, array vs. object) but cannot enforce **class-hierarchy semantics** that the SPDX 3 model requires. The SPDX 3 model is RDF-flavored: properties carry a declared `range` (the class an IRI value must belong to), and the JSON-LD serialization expresses references-by-IRI that point at typed elements elsewhere in the `@graph`. JSON Schema doesn't follow those references; class-hierarchy validators do.

The reported failure is exactly this gap. The Java SPDX library's range-checking caught that `CreationInfo.createdBy` is declared with range `Agent` (abstract, with concrete subclasses `Person` / `Organization` / `SoftwareAgent`), but mikebom emits an IRI in `createdBy` that resolves to a `Tool` element — a separate class hierarchy from `Agent`. JSON Schema couldn't catch this because the JSON-level shape (an array of strings) is fine; the violation is semantic.

The user's honest framing — "I really don't know if this is accurate or if we're accurate" — is the right one. Without external validator integration, we don't actually know what other conformance issues mikebom's SPDX 3 output has. Visual inspection of a fixture surfaces several suspicious patterns (e.g., `dataLicense: "https://spdx.org/licenses/CC0-1.0"` as a bare URL; `externalIdentifierType: "cpe23"` / `"packageUrl"` as enum values without verification against the SPDX 3 controlled vocabulary; `software_packageUrl` namespaced property casing). Some may be conformant; some may not. Only running an authoritative validator establishes ground truth.

This milestone establishes that ground truth and fixes whatever it surfaces. Three deliverables:

1. **Fix the reported `createdBy` bug**. Add an `Organization` element representing the mikebom publisher (`mikebom contributors`); route `createdBy` to that Agent; add a new `createdUsing` property on `CreationInfo` pointing at the existing `Tool` element. This is the minimum-viable hotfix that resolves the user-visible error.
2. **Run an authoritative validator against every existing SPDX 3 fixture**. Use the JPEWdev `spdx3-validate` tool (Python, actively maintained, follows `validation.md` rules) as the primary validator. Catalog every conformance issue it surfaces. Fix them all.
3. **Add validator-based conformance verification to CI**. Prevent regression by failing the build on conformance violations. The exact CI integration shape (Python sidecar, Rust wrapper, GitHub Action) is a research/plan-phase decision, but the gate must run on every PR and against the existing golden fixture set.

The deliberate scope: SPDX 3.0.1 only. CDX 1.6 + SPDX 2.3 conformance are separately tracked (CDX has a published JSON Schema we already validate against; SPDX 2.3 has its own validator ecosystem). Both formats will see no behavior change from this milestone — only SPDX 3 emission will change to fix conformance issues.

## Clarifications

### Session 2026-05-06

- Q: Which `Agent` subclass should mikebom emit for `CreationInfo.createdBy` — `SoftwareAgent`, `Organization`, or both? → A: `Organization` with `name: "mikebom contributors"` (matching the publisher identity already used in CDX `metadata.tools[0].publisher`). Conforms to the SPDX 3 spec's intent for the `createdBy` slot (Person or Organization — the human/legal entity responsible for the analysis), aligns with the convention used by syft / trivy / cdxgen, and plays well with downstream tools that filter SBOMs by organization name. The Tool element stays in `createdUsing` — that's the slot SPDX 3 designates for "what tool ran the analysis." `SoftwareAgent` is reserved for autonomous software agents (AI assistants etc.) per the spec, not for "the runtime instance of an SBOM-generation tool."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Fix the user-reported `createdBy` type mismatch (Priority: P1)

The operator who reported the issue is consuming mikebom's SPDX 3 output via a Java SPDX library that performs RDF-flavored range checking. The library throws "Incompatible type for property `Core/createdBy`: class `core.Agent`" because mikebom emits an IRI in `createdBy` that resolves to a `Tool` element (separate class hierarchy from `Agent`). Post-fix, mikebom emits an `Organization` element for `createdBy` (per the 2026-05-06 clarification, with `name: "mikebom contributors"`) and routes the existing `Tool` to a new `createdUsing` field.

**Why this priority**: Direct user-reported failure with concrete, immediate consumer-side breakage. Fixing it unblocks the operator's downstream tooling immediately. P1 because (a) it's the headline issue from the original message and (b) the fix is small and bounded — it doesn't depend on the broader audit completing.

**Independent Test**: Take the post-fix `cargo.spdx3.json` golden, run the JPEWdev `spdx3-validate` tool with `--check class-hierarchy` (or whatever the equivalent flag is), and verify `Core/createdBy` no longer fails the range check. Equally: run the Java SPDX library against the post-fix output and verify no "Incompatible type" exception.

**Acceptance Scenarios**:

1. **Given** the post-fix `cargo.spdx3.json` golden, **When** the operator runs the JPEWdev validator on it, **Then** the validator reports zero `createdBy` range violations.
2. **Given** the same post-fix golden, **When** the Java SPDX library processes it, **Then** no "Incompatible type for property … Core/createdBy" exception is thrown.
3. **Given** the post-fix output, **When** an operator inspects `metadata.creationInfo` (or the equivalent JSON-LD `CreationInfo` graph entry), **Then** `createdBy` references an `Organization` element with `name: "mikebom contributors"`, AND a separate `createdUsing` field references the `Tool` element, AND both elements are present in `@graph`.

---

### User Story 2 — Pass the JPEWdev `spdx3-validate` tool against every existing SPDX 3 fixture (Priority: P1)

Beyond the user-reported `createdBy` bug, mikebom's SPDX 3 output likely has additional conformance issues that visual inspection can't catch reliably. Running the JPEWdev `spdx3-validate` tool against every existing SPDX 3 golden fixture surfaces the full list. Every issue it reports gets fixed in this milestone.

**Why this priority**: This is what "100% conformant" means operationally — pass the authoritative validator. Fixing only the user-reported issue (US1 alone) leaves mikebom in a state where the next operator using a different validator hits a different issue, and the same fire drill repeats. P1 because the milestone's stated goal explicitly requires it.

**Independent Test**: For each existing SPDX 3 golden fixture (`cargo.spdx3.json`, `apk.spdx3.json`, `deb.spdx3.json`, `gem.spdx3.json`, `golang.spdx3.json`, `maven.spdx3.json`, `npm.spdx3.json`, `pip.spdx3.json`, `rpm.spdx3.json`), run `spdx3-validate <fixture>` and verify it exits zero with no errors reported.

**Acceptance Scenarios**:

1. **Given** every existing SPDX 3 golden fixture (9 files across the supported ecosystems), **When** `spdx3-validate` is run on each, **Then** every fixture exits with zero errors.
2. **Given** an SPDX 3 file that mikebom emits for any in-scope scan target (source-tier source tree, image-tier image scan, build-tier trace), **When** `spdx3-validate` is run on the emitted output, **Then** the validator exits zero. Verified by integration test that runs the validator against fresh emissions for at least 3 representative scan targets.
3. **Given** a fixture that fails validation pre-fix, **When** the milestone's conformance fixes are applied, **Then** the validator's specific error code that fired pre-fix is gone post-fix. Verified per-issue with a test harness that re-runs the validator and asserts the absence of the previously-reported error message.

---

### User Story 3 — Add SPDX 3 conformance verification to CI (Priority: P2)

The conformance fixes from US1 + US2 prevent today's known issues. CI integration prevents tomorrow's regressions. After this milestone ships, any PR that introduces a conformance violation in mikebom's SPDX 3 emission MUST fail CI before it can merge.

**Why this priority**: Lower-priority because the immediate user-reported breakage is fixed by US1, and the full audit is fixed by US2. CI integration is the hardening layer. P2 because the absence of CI gating is acceptable for a single milestone-completion bug-fix; it becomes essential only if mikebom commits to ongoing SPDX 3 conformance over many future milestones (which the project does, given Constitution Principle V).

**Independent Test**: Open a PR that intentionally regresses one SPDX 3 conformance rule (e.g., reverts the US1 createdBy fix), push it, and verify CI fails with a clear validator-output error pointing at the violation.

**Acceptance Scenarios**:

1. **Given** a PR that introduces a conformance violation in mikebom's SPDX 3 emission, **When** the PR's CI run completes, **Then** at least one CI check fails with the validator's error output visible in the failure log.
2. **Given** the existing pre-PR-gate workflow (`./scripts/pre-pr.sh`), **When** a developer runs it locally, **Then** the SPDX 3 conformance check runs as part of the gate and reports clean for fully-conformant emission.
3. **Given** a PR that doesn't touch SPDX 3 emission, **When** its CI runs, **Then** the conformance check still runs (so we catch regressions in code paths that indirectly affect SPDX 3 emission, like changes to `ScanArtifacts` or shared helpers).

---

### Edge Cases

- **Validator unavailable in CI**: if the JPEWdev `spdx3-validate` tool fails to install (Python toolchain hiccup, network outage on `pip install`), CI must surface this as an infrastructure failure (clear "validator install failed" error message), not silently skip the conformance check.
- **Validator reports a false positive**: if a downstream version of the JPEWdev tool surfaces a "violation" that's actually a tool bug, mikebom needs an escape hatch. Document the policy: pin to a known-good validator version in CI; bumping is a deliberate PR with proof the new version isn't producing false positives.
- **A required SPDX 3 conformance fix would break SPDX 3 byte-identity goldens for existing milestones**: expected. The byte-identity contract for SPDX 3 goldens (FR-009 and equivalents in milestones 073-077) is replaced by "SPDX 3 goldens conform to spec." Existing goldens get regenerated as part of this milestone — that's the expected change.
- **A required SPDX 3 conformance fix would also affect SPDX 2.3 or CDX**: not in scope. SPDX 2.3 goldens stay byte-identical. CDX 1.6 goldens stay byte-identical. Only SPDX 3 goldens change.
- **`software_*` namespaced property names**: validator may flag the `software_` prefix style as non-canonical (the actual SPDX 3 spec might prefer `softwarePackageUrl` camelCase, or `software:packageUrl` namespaced, or something else entirely). If so, every SPDX 3 fixture's property naming changes — that's a substantial regen but it's the right outcome.
- **`@id` vs `spdxId` field naming**: validator may flag inconsistent use of `@id` (JSON-LD canonical) vs `spdxId` (SPDX 3 spec property name) for element identifiers. If so, normalize across the emission code paths.
- **License-expression IRIs**: `dataLicense: "https://spdx.org/licenses/CC0-1.0"` is suspect — SPDX 3 may expect a `SimpleLicensingExpression` element reference instead of a bare URL. If validator flags it, refactor.
- **External identifier type vocabulary**: `externalIdentifierType: "cpe23"` / `"packageUrl"` use string values whose membership in the SPDX 3 controlled vocabulary needs verification. If validator flags them, fix to canonical values.
- **CreationInfo blank node identifier**: `_:creation-info` is JSON-LD-correct but the validator may have specific expectations about whether CreationInfo gets a stable URI vs a blank node. If flagged, refactor.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: mikebom's SPDX 3 emission MUST resolve the user-reported `createdBy` type mismatch by emitting an `Organization` element (per the 2026-05-06 clarification) with `name: "mikebom contributors"` (matching the publisher identity already in CDX `metadata.tools[0].publisher`), and pointing `CreationInfo.createdBy` at that Agent element. The existing `Tool` element MUST be referenced from a new `CreationInfo.createdUsing` field. The `Tool` element's existing identity (name `mikebom-<version>`, current `spdxId` IRI) is preserved unchanged — only its CreationInfo-slot reference moves from `createdBy` to `createdUsing`.
- **FR-002**: mikebom's SPDX 3 emission MUST pass the JPEWdev `spdx3-validate` tool with zero errors when run against every existing golden fixture under `mikebom-cli/tests/fixtures/golden/spdx-3/`.
- **FR-003**: mikebom's SPDX 3 emission MUST pass the JPEWdev `spdx3-validate` tool with zero errors when run against fresh emissions for at least three representative scan targets (one source-tier, one image-tier, one synthetic-build-tier — pattern matching milestones 074/075/076's representative-fixture coverage).
- **FR-004**: A pinned-version installation of the JPEWdev `spdx3-validate` tool MUST be invocable from the project's CI workflow. CI MUST fail on any non-zero validator exit, with the validator's error output visible in the build log.
- **FR-005**: A pinned-version installation of the JPEWdev `spdx3-validate` tool MUST be invocable from the project's pre-PR gate script (`./scripts/pre-pr.sh`) so developers catch conformance issues before opening a PR.
- **FR-006**: Every existing SPDX 3 golden fixture (9 files: apk, cargo, deb, gem, golang, maven, npm, pip, rpm) MUST be regenerated to reflect whatever conformance fixes are required. The regen is expected and is the operator-visible change of this milestone.
- **FR-007**: Existing milestone-073/074/075/076/077 byte-identity goldens for **CDX 1.6** and **SPDX 2.3** MUST stay byte-identical. Only SPDX 3 fixtures regen as part of this milestone. Verified by the existing parity-check golden suite continuing to pass for non-SPDX-3 formats.
- **FR-008**: When `spdx3-validate` reports a violation that mikebom's authors believe is a validator bug rather than a real conformance issue, the project MUST document the disagreement (with a citation to the SPDX 3 model spec confirming mikebom's reading) and pin the validator to a version that doesn't produce the false positive. No silent suppression.
- **FR-009**: The validator integration MUST be deterministic — the same emission against the same validator version produces the same result across re-runs. If the validator has any nondeterministic behavior (e.g., random IRI generation in error messages), the integration normalizes the output before comparison.
- **FR-010**: The validator integration MUST surface errors in a machine-readable + human-readable form. Machine-readable for CI to programmatically gate on; human-readable so developers can quickly understand what's wrong.
- **FR-011**: Conformance fixes MUST NOT introduce semantic ambiguity. If a fix has multiple valid spec-conformant choices (e.g., model `createdBy` as `SoftwareAgent` vs `Organization` — both are valid Agent subclasses), the choice MUST be documented in this milestone's research with a rationale.

### Key Entities

- **ConformanceViolation**: An issue surfaced by the JPEWdev `spdx3-validate` tool against a mikebom-emitted SPDX 3 file. Composed of: file path, validator rule ID (or message), affected JSON path within the document, severity (error vs warning — only errors block; warnings get triaged but don't block). Each violation is either fixed in this milestone OR documented as a deliberate disagreement with rationale per FR-008.
- **ValidatedFixture**: A mikebom-emitted SPDX 3 file (golden or freshly-emitted) that has been validated zero-error by the JPEWdev tool at a pinned version. The CI gate asserts ValidatedFixture status for every golden + every test-emitted file.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Running `spdx3-validate` (JPEWdev's tool, at the pinned version) against every existing SPDX 3 golden fixture (9 files) reports zero errors for every fixture. Verified by integration test that loops over the fixture set and asserts on the validator's exit code + stderr.
- **SC-002**: Running `spdx3-validate` against fresh SPDX 3 emissions from at least 3 representative scan targets (source-tier, image-tier, synthetic-build-tier) reports zero errors. Verified by integration test that emits-then-validates.
- **SC-003**: A test harness that asserts the absence of the user-reported error ("Incompatible type for property … Core/createdBy: class … core.Agent") confirms the specific failure mode is resolved post-fix. Verified by the test asserting (a) `CreationInfo.createdBy` is a single-element array referencing an element whose `type` is `Organization` and whose `name` is `"mikebom contributors"`; (b) `CreationInfo.createdUsing` is a single-element array referencing an element whose `type` is `Tool`; (c) both elements are present in `@graph` and are distinct (different `spdxId` IRIs); (d) the JPEWdev validator reports zero `createdBy`/`createdUsing` range violations against the post-fix output.
- **SC-004**: CI fails when a PR introduces a conformance violation. Verified by submitting a deliberate-regression PR (manual-test step or scripted in this milestone's polish phase) and confirming the CI check turns red with the validator's error output visible.
- **SC-005**: The pre-PR gate script (`./scripts/pre-pr.sh`) catches conformance violations before PR submission. Verified by running the script locally with a deliberate-regression and confirming non-zero exit + error message.
- **SC-006**: 100% of CDX 1.6 + SPDX 2.3 byte-identity goldens stay byte-identical pre/post milestone. Verified by the existing `cdx_regression_*` and `spdx_regression_*` golden suites continuing to pass without regen.
- **SC-007**: Every SPDX 3 conformance fix is reproducible — given the same scan target, mikebom emits a byte-identical SPDX 3 document across re-runs that pass the validator identically. Verified by determinism integration test.
- **SC-008**: Documentation update — `docs/reference/identifiers.md` (SPDX 3 portion) reflects the new emission shape (Agent + Tool element split, plus any other conformance changes). Verified by manual inspection during the milestone's polish phase.

## Assumptions

- The JPEWdev `spdx3-validate` tool (https://github.com/JPEWdev/spdx3-validate) is the **primary** validator. The official SPDX 3 JSON-LD validation rules at https://github.com/spdx/spdx-3-model/blob/develop/serialization/jsonld/validation.md are the spec these rules trace back to. The Java SPDX library that produced the user's original error is treated as a secondary cross-check, not the primary gate.
- The project will install the JPEWdev tool via `pip install` in CI. Python is already used elsewhere in the project's tooling (per the docs/examples/cross-tier-walk/ Python walker shipped in PR #150), so adding a Python dependency for CI is incremental work, not a new ecosystem.
- The validator version is pinned in CI (e.g., a specific git SHA or PyPI version of `spdx3-validate`). Version bumps are deliberate PRs with proof the new version doesn't produce false positives. Per FR-008.
- This milestone's scope is fix + validate + gate. It does NOT include broader SPDX 3 feature additions (e.g., emitting Build elements per the SPDX 3 Build profile, emitting AI/Dataset profile content) — those are separate milestones if/when operator demand emerges.
- All existing SPDX 3 golden fixtures (9 files across the supported ecosystems) get regenerated in this milestone's PR. The regen is symmetric where possible (only the conformance-fix-related lines change) but may include structural rearrangement if validator-driven fixes touch widely-used fields like property naming or class hierarchies. Per-file diff sizes will vary.
- Existing CDX 1.6 + SPDX 2.3 goldens stay byte-identical. The conformance work is scoped strictly to the SPDX 3 emission code path; no shared types or helpers that affect other formats are modified.
- The user's specific Java SPDX library version that produced the original error is documented but not directly pinned in CI. The JPEWdev tool's coverage is broader and more actively maintained; passing JPEWdev validation is a sufficient condition for passing the Java library's range checks (the rules trace back to the same spec).
- This milestone deliberately ships as a single PR. Splitting US1 (createdBy hotfix) from US2 + US3 (full audit + CI gate) into separate PRs is technically possible but creates a transient state where the CI gate isn't yet in place to prevent further drift; the audit fixes might re-introduce subtle issues that gate-less CI would miss. Single-PR delivery is the safer cadence given the small total scope.
- The milestone treats "pass JPEWdev validator zero-error" as the operational definition of "100% SPDX 3.0.1 conformant." If subsequent operator reports surface validator gaps (issues no available tool catches, but the spec actually requires), those are tracked as follow-up GitHub issues, not regressions of this milestone.
