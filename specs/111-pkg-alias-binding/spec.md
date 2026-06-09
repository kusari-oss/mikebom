# Feature Specification: Operator-supplied PURL alias for cross-tier binding

**Feature Branch**: `111-pkg-alias-binding`
**Created**: 2026-06-08
**Status**: Draft
**Input**: User description: "Operator-supplied PURL alias to bind binary-tier pkg:generic components to source-tier ecosystem PURLs (Option A of issue #225). Extends milestone 072 cross-tier binding."

## Clarifications

### Session 2026-06-09

- Q: How does the LHS PURL in `--pkg-alias` compare against scan-output component PURLs? → A: Strict equality on canonical PURL form (name + version + qualifiers + subpath, after canonicalization). No partial / pattern matches; no version-or-qualifier fuzzing.
- Q: Where does the applied-alias record live in the emitted SBOM, and what shape does it take? → A: Extend the existing milestone-072 binding-result property envelope with optional `alias_from` / `alias_to` fields. One property entry per component; existing verify-binding parser learns the new fields without new envelope plumbing.
- Q: When verify-binding reads an alias-bearing SBOM, how should its output surface that the binding was reached via an alias? → A: Output gains a sibling field `applied_alias: "<LHS> → <RHS>"` (single-string format) when alias was applied. `BindingStrength` enum unchanged (no breaking change); auditors get the mapping at a glance.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Bind a single primary binary to its source-tier ecosystem PURL (Priority: P1)

An operator builds a Rust application `baz` from `github.com/foo/bar`. The source-tier SBOM (produced by scanning the Rust workspace) has the project's main module emit as `pkg:cargo/baz@1.0.0`. CI builds a container image containing the compiled `baz` binary, and the image-tier SBOM (produced by scanning the image) identifies the binary as `pkg:generic/baz`. When the operator runs `mikebom sbom scan --image ... --bind-to-source baz-source.cdx.json` to bind image to source, the flagship `baz` component fails to bind because `pkg:generic/baz` and `pkg:cargo/baz@1.0.0` are not equal PURLs. The operator needs to declare that these two PURLs refer to the same logical component so cross-tier binding succeeds for the component they care most about.

**Why this priority**: This is the textbook source-to-image workflow that motivates issue #225. Until this case works, the binding feature is operator-hostile for the most-important component in every scan. P1 because no MVP exists without it.

**Independent Test**: Author a small Rust project with one binary; produce a source-tier SBOM; build a containerized version; produce an image-tier SBOM with the alias declaration; verify the binding strength of the binary component is `verified` or `weak`, never `unknown` with reason `source-not-found-in-bind-target`.

**Acceptance Scenarios**:

1. **Given** a source-tier SBOM containing a component with PURL `pkg:cargo/baz@1.0.0` and an image containing a binary that mikebom identifies as `pkg:generic/baz`, **When** the operator runs `mikebom sbom scan --image ... --bind-to-source baz-source.cdx.json --pkg-alias "pkg:generic/baz=pkg:cargo/baz@1.0.0"`, **Then** the emitted image-tier SBOM contains a component for the binary whose binding strength is `verified` (when hash-evidence matches) or `weak` (when hash-evidence is absent or partial), AND the reason field does not say `source-not-found-in-bind-target`.
2. **Given** the same scan as above, **When** an operator inspects the emitted SBOM later, **Then** the alias declaration is recorded on the affected component as a machine-readable property so a downstream consumer or auditor can see that an alias was applied to produce the binding result.
3. **Given** a scan with no `--bind-to-source` flag, **When** the operator supplies `--pkg-alias` anyway, **Then** mikebom emits a warning and proceeds without error (aliases are a binding-time concept; supplying them outside the binding workflow is harmless but should produce a clear operator signal that the flag had no effect).

---

### User Story 2 - Bind multiple primary binaries in a workspace project (Priority: P2)

An operator has a Cargo workspace containing two binaries `baz` and `baz-debug`. The source-tier SBOM emits two main-module components: `pkg:cargo/baz@1.0.0` and `pkg:cargo/baz-debug@1.0.0`. The image contains both compiled binaries. The operator needs to declare aliases for both so neither flagship component lands in the `unknown` binding state.

**Why this priority**: Workspace projects are common in Rust, Go, and monorepo setups; without N-alias support the feature only handles single-binary projects. P2 because P1 has the same UX shape — the additional surface is per-flag repetition, not new semantics.

**Independent Test**: Same setup as User Story 1 but with two binaries; verify both components bind correctly after declaring two `--pkg-alias` flags.

**Acceptance Scenarios**:

1. **Given** two binaries in an image and two corresponding source-tier components, **When** the operator passes `--pkg-alias` twice (once per binary), **Then** both image-tier components bind to their respective source-tier targets with strength `verified` or `weak`.
2. **Given** the same setup, **When** the operator instead supplies the aliases as one environment variable (comma-separated entries matching the per-flag syntax), **Then** mikebom honors the env-var form identically to the per-flag form for CI-pipeline ergonomics.

---

### User Story 3 - Verify-binding consumes alias-bearing SBOMs without re-supplying the alias (Priority: P2)

After a scan stamps the SBOM with the applied alias, an auditor running `mikebom verify-binding image.cdx.json baz-source.cdx.json` (or `mikebom trace-binding ...`) needs to reproduce the same binding result without having access to the original `--pkg-alias` flag value. The alias was operator intent at scan time; verification must honor that intent based purely on what's recorded in the artifacts.

**Why this priority**: Without persistence, the alias is operationally fragile — the scan succeeds but the SBOM doesn't survive a round-trip through verify-binding. Audit pipelines that re-verify SBOMs (without access to the original CLI invocation) lose the binding result. P2 because P1 unblocks the scan-time workflow; this completes the round-trip.

**Independent Test**: Run a scan with `--pkg-alias`, save the resulting SBOM, then run `verify-binding` on the saved SBOM against the same source-tier SBOM, with no CLI alias supplied; verify the binding strength matches the scan-time result.

**Acceptance Scenarios**:

1. **Given** an image-tier SBOM produced with `--pkg-alias` in User Story 1, **When** the operator runs `mikebom verify-binding image.cdx.json baz-source.cdx.json`, **Then** the verification result for the aliased component reports the same strength (`verified` or `weak`) as the scan-time result, AND identifies the alias as the source of the match.
2. **Given** the same SBOM but a different source-tier SBOM that does NOT contain the alias's target PURL, **When** the operator runs `verify-binding`, **Then** the result is `unknown` with a distinct reason indicating the alias target is missing from the bind source (separate from the existing `source-not-found-in-bind-target` reason).

---

### Edge Cases

- **Alias LHS does not appear in the scan output**: Operator supplies `--pkg-alias "pkg:generic/qux=pkg:cargo/qux@1.0.0"` but no `pkg:generic/qux` component is produced by the scan. Mikebom emits an info-level log noting the unused alias and proceeds; the unused alias is not recorded in the emitted SBOM. This protects against operator typos silently distorting the binding result.
- **Alias RHS not present in the bind-source SBOM**: Operator supplies an alias whose RHS PURL does not exist in `--bind-to-source`. Mikebom records the alias-application attempt on the affected component but reports binding strength `unknown` with reason `alias-target-not-found-in-bind-target` (distinct from the existing `source-not-found-in-bind-target` reason) so operators can debug their alias declaration separately from a missing-source-document state.
- **Same RHS targeted by multiple LHS aliases**: Operator declares `--pkg-alias A=X --pkg-alias B=X`. Both LHS components bind to the same RHS source-tier component. Mikebom does not collapse the two image-tier components into one; each retains its own identity while both bind to the same source.
- **Same LHS with multiple different RHS aliases**: Operator declares `--pkg-alias A=X --pkg-alias A=Y` (two RHSes for one LHS). Mikebom rejects the second alias declaration at flag-parse time with a clear error message; ambiguity is treated as operator error, not silent precedence.
- **Malformed PURL on either side of the alias**: Operator passes a syntactically invalid PURL string in the alias. Mikebom rejects the alias at flag-parse time with a clear error citing the malformed input, before any scan work begins.
- **Alias supplied without `--bind-to-source`**: As scenario 3 of User Story 1: a warning is emitted; the scan proceeds; the alias is not recorded in the SBOM (since it had no effect).
- **Alias whose LHS is not in the generic PURL namespace**: The flag accepts any LHS PURL, not just `pkg:generic/*`. Operators with hand-crafted scenarios (e.g., wanting to alias `pkg:deb/debian/foo@1.0` to `pkg:github/foo/foo@1.0` for a different source-tier representation) can do so.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Mikebom MUST accept a repeatable CLI flag `--pkg-alias LHS=RHS` on the `sbom scan` subcommand, where both LHS and RHS are PURL strings in canonical form. Match against scan-output components is strict equality of the canonical PURL form (name + version + qualifiers + subpath, after canonicalization); no partial, name-only, or version-pattern matches are performed.
- **FR-002**: Mikebom MUST accept an environment variable `MIKEBOM_PKG_ALIAS` containing one or more alias entries (entries separated by `,`) using the same `LHS=RHS` syntax as the CLI flag; the env-var form and the repeated-flag form MUST produce identical behavior.
- **FR-003**: When `--bind-to-source` is in effect AND a configured alias's LHS PURL matches a component produced by the scan, the binder MUST treat the matched component as if its PURL were the alias's RHS for the purpose of locating a counterpart in the bind-source SBOM.
- **FR-004**: When an alias is applied to a component, the binder MUST compute the binding strength (`verified`, `weak`, or `unknown`) using the same layered-evidence rules that apply to non-aliased components, with the RHS source-side evidence as input.
- **FR-005**: Mikebom MUST record the applied alias on the affected component in the emitted SBOM by extending the existing milestone-072 binding-result property envelope (one property entry per component) with two optional fields: `alias_from` carrying the LHS PURL the operator declared, and `alias_to` carrying the RHS PURL the binder matched against. The two fields MUST appear together (both populated when an alias was applied; both absent otherwise) so consumers can rely on their presence as a single signal. The format-parity layer emits the extended envelope in CDX 1.6, SPDX 2.3, and SPDX 3.0.1 with semantically equivalent content.
- **FR-006**: `verify-binding` and `trace-binding` subcommands MUST honor an alias recorded in the input SBOM and produce the same binding strength they would have produced at scan time, without requiring the original CLI alias to be re-supplied. When an alias was applied, the output MUST surface a sibling field `applied_alias: "<LHS> → <RHS>"` (single-string format) so auditors can recognize alias-driven results at a glance. The `BindingStrength` enum MUST NOT gain new variants for the aliased case — `verified`, `weak`, and `unknown` remain the only values.
- **FR-007**: When an alias's RHS does not appear in the bind-source SBOM, the binder MUST report `unknown` strength with reason `alias-target-not-found-in-bind-target`, distinct from the existing `source-not-found-in-bind-target` reason.
- **FR-008**: When the operator supplies the same LHS with two different RHSes, mikebom MUST reject the invocation at CLI-parse time with an actionable error message citing the conflicting declarations.
- **FR-009**: When the operator supplies a malformed PURL on either side of an alias, mikebom MUST reject the invocation at CLI-parse time with an actionable error message citing the malformed input.
- **FR-010**: When the operator supplies `--pkg-alias` without `--bind-to-source`, mikebom MUST emit a warning identifying that the alias has no effect and proceed with the scan; the unused alias MUST NOT appear in the emitted SBOM.
- **FR-011**: When a configured alias's LHS PURL does not match any component produced by the scan, mikebom MUST log the unused alias at info level and MUST NOT record it on any component in the emitted SBOM.
- **FR-012**: Existing milestone-072 binding behavior for components without a configured alias MUST be unchanged; component-level binding results without an applied alias MUST be byte-identical to pre-feature output, modulo the absence of the new alias-property entry.
- **FR-013**: The extended binding-result envelope MUST capture both the original LHS (in `alias_from`) and the RHS (in `alias_to`) so consumers can reconstruct what was rewritten without ambiguity; downstream tooling MUST be able to identify "this binding result was reached via an alias" by inspecting the presence of these fields alone, without consulting any external alias declaration source.

### Key Entities

- **PURL Alias**: A pair of PURL strings (LHS, RHS) declared by the operator. LHS is the component identifier produced by the binary-tier scan; RHS is the canonical ecosystem-tier PURL that LHS should be treated as during binding match.
- **Aliased Component**: A component in the emitted SBOM whose PURL is LHS, whose binding result was computed against the RHS source-tier component, and which carries a property recording the alias declaration that produced that binding.
- **Binding Strength**: One of `verified`, `weak`, or `unknown`, computed per the existing milestone-072 layered-evidence rules. The alias affects which RHS source-tier component is matched; it does not change the strength calculation itself.
- **Alias Failure Reason**: A string field accompanying an `unknown` binding result that identifies why binding failed. New value `alias-target-not-found-in-bind-target` is added for the case when an alias's RHS is not present in the bind-source SBOM; existing `source-not-found-in-bind-target` continues to fire for components without an applied alias.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An operator scanning their flagship source-to-image workflow (one primary binary, one source SBOM, one image SBOM) can move the binding strength of the primary binary component from `unknown` to `verified` or `weak` by adding exactly one `--pkg-alias` flag to their existing scan command. No other CLI surface or configuration change is required.
- **SC-002**: An auditor re-running `verify-binding` on an alias-bearing SBOM produced by a prior scan reproduces the same binding strength for every aliased component without re-supplying the alias at the command line.
- **SC-003**: When an operator misconfigures an alias (typo'd LHS, missing RHS in source, malformed PURL, conflicting declarations), mikebom produces an error or warning message that names both the misconfiguration and the corrective next step within a single output line, so the operator can fix the alias without consulting documentation.
- **SC-004**: A scan that does not supply any `--pkg-alias` flag produces an emitted SBOM byte-identical to the pre-feature baseline for every component (no new properties, no semantic drift, no new annotations). Regression-free for the OSS default-no-extras path.
- **SC-005**: A workspace project with five primary binaries can be fully aliased with five repeated `--pkg-alias` flags (or one env-var entry containing all five), without per-binary CLI ergonomics penalties (no required-config files, no per-binary subcommands, no separate scan invocations).
- **SC-006**: Aliased components round-trip cleanly through the existing format-parity layer: the alias-applied property emits in CDX 1.6, SPDX 2.3, and SPDX 3.0.1 with semantically equivalent content, and the cross-format parity gate accepts the new property without invariant violations.

## Assumptions

- Operators authoring `--pkg-alias` declarations know the source-tier PURL ahead of time (they have access to the source SBOM during scan invocation or can predict the main-module PURL from their build system). Out-of-band PURL lookup tooling is not part of this milestone.
- Aliases are scan-time configuration, not persisted state — operators re-supply the alias at every scan that needs it. Aliases ARE persisted in the emitted SBOM so that downstream consumers (verify-binding, trace-binding) can honor them without re-supply.
- The set of `unknown` reasons defined by milestone 072 is open for extension; adding `alias-target-not-found-in-bind-target` does not require external schema coordination.
- The format-parity layer (milestone 071) and the catalog-row infrastructure (existing) accept new component-level properties on the same terms as existing milestone-072 binding properties; no new parity infrastructure work is required.
- Operators who supply both `--pkg-alias` and the milestone-073 `--component-id` flag are responsible for understanding their distinct purposes; the spec does not need to enforce mutual exclusion. The two flags serve adjacent but non-overlapping goals — `--component-id` adds external identifiers as annotations, `--pkg-alias` rewrites the binding-match input — and operators may use them together without conflict.
- Aliases apply only at binding time. Out of scope: alias-driven rewriting of the component's emitted PURL itself (the LHS remains the emitted PURL; the RHS is referenced only via the alias-applied property). This preserves the milestone-104 binary-role-classification + milestone-096 binary-identity invariants that depend on the binary-tier PURL being computed from binary evidence alone.
- This milestone implements Option A from issue #225 only. Option B (source-side `mikebom:produces-binaries` annotation enabling automatic aliasing) is a deferred follow-on milestone. Option C (name-only heuristic match) is explicitly off the table.

## Out of Scope

- Wildcard alias matching (LHS = `pkg:generic/*` etc.) — start with literal LHS matches.
- Automatic aliasing via source-side annotations (issue #225 Option B).
- Name-only heuristic matching (issue #225 Option C).
- Aliasing across non-PURL identifiers (CPE, swid, gitoid, etc.) — PURL-to-PURL only.
- A configuration file format for aliases (e.g., `mikebom-aliases.toml`). Aliases are supplied via flag or env var; a config-file format may follow if operator usage demonstrates demand.
- Scoping an alias to a specific binary path within the image (e.g., apply only to `/usr/local/bin/baz`, not all `pkg:generic/baz`). Defer until operator usage shows demand.
- Reverse-direction alias application (treating the RHS as a synonym for the LHS) — the issue's problem statement is uni-directional.
