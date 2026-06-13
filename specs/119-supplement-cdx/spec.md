# Feature Specification: Developer-asserted source-of-truth supplement (v0.1, CDX 1.6 input)

**Feature Branch**: `119-supplement-cdx`
**Created**: 2026-06-13
**Status**: Draft
**Input**: Issue #326 — Option C v0.1 scope. mikebom today is purely an evidence-extracting scanner; there is no surface where a developer can assert ground truth the scanner cannot observe (license / supplier / copyright for an unknown component, vendored-without-package-manager libraries, SaaS dependencies with no on-disk footprint, aggregate "product" roll-ups). Per the issue's recommended starting point, ship Option A as v0.1 (`--supplement-cdx <PATH>` flag accepting hand-authored CDX 1.6 JSON); observe whether hand-authoring is the friction point users actually hit; layer the TOML manifest on top later if warranted. Critical safety property preserved: developer cannot suppress scanner detection of bytes-evident content.

## Clarifications

### Session 2026-06-13

- Q: When the operator passes both `--scan-as my-product@1.0.0` AND a supplement file whose `metadata.component` declares a different identity, which one wins? → A: `--scan-as` wins; supplement's `metadata.component` is IGNORED for v0.1 (matches FR-014's current commitment). Single override mechanism preserves milestone-110's mental model and avoids two competing override surfaces operators have to mentally rank.
- Q: For v0.1, does the supplement match-rule stay strict PURL exact-match, or relax to allow wildcards / name-based matching? → A: Strict PURL exact-match only (matches FR-010's current commitment). Operators get a clear contract ("exact PURL or no match") that's easy to reason about + easy to test. Wildcards / name-match use cases are real but introduce ambiguity (which scanner-side component does `pkg:cargo/log@*` match if 3 versions are present?) that's better resolved when operators request it in production, not pre-emptively.
- Q: For v0.1, does the conflict-justification enum stay minimal-mikebom (2 values), import OpenVEX verbatim (7 values), or something between? → A: Stay minimal (`developer-metadata-override` + `bytes-evident-detection-preserved`) for v0.1 (matches FR-009's current commitment). OpenVEX's vocabulary is designed for VULNERABILITY justifications — semantically wrong for component-identity conflicts. Starting minimal lets v0.1 ship clean; expanding later is a strict-superset extension that doesn't break existing consumers.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A developer declares SaaS dependencies and vendored libraries that the scanner cannot observe (Priority: P1)

A developer ships a project that depends on Stripe, Twilio, and a copy of `liberror` vendored from upstream at `third_party/liberror/` (no Cargo.toml, no package-lock.json, no manifest of any kind). They author a `supplement.cdx.json` file listing:
- Two service entries (`pkg:saas/stripe.com`, `pkg:saas/twilio.com`) under `services[]`
- One component entry (`pkg:generic/liberror@1.2.3`) under `components[]` with declared license `MIT`, declared supplier `Acme Open Source Foundation`
- A `dependencies[]` block linking the operator's own component to all three

CI runs `mikebom sbom scan --path . --supplement-cdx supplement.cdx.json --output sbom.cdx.json`. The emitted SBOM contains every scanner-discovered component PLUS the three declared entries from the supplement, each tagged so consumers can tell scanner-observed from developer-declared. The two SaaS services appear under `services[]` (a section scanner never populates today). The vendored library appears as a component with the declared license, supplier, and copyright text.

**Why this priority**: This is the textbook case the issue body opens with. Without it, mikebom's SBOMs are blind to SaaS dependencies and any library that ships without a recognizable manifest — exactly the "I know what's in my project but the scanner doesn't" gap operators repeatedly hit. P1 because no MVP exists without it; everything else (overrides, conflict resolution, justifications) is layered on top of this merge mechanic.

**Independent Test**: Author a 3-component supplement file; scan a project that has none of those three components on disk; verify the emitted SBOM contains all three in the right CDX sections (`services[]` for SaaS entries, `components[]` for the vendored library) with the declared metadata intact.

**Acceptance Scenarios**:

1. **Given** a scan target with one Cargo project AND a `supplement.cdx.json` declaring one `pkg:saas/stripe.com` service entry, **When** the operator runs `mikebom sbom scan --path . --supplement-cdx supplement.cdx.json --output sbom.cdx.json`, **Then** the emitted SBOM contains the scanner's Cargo components AND the declared Stripe service under `services[]`. No scanner-observed component is dropped; the supplement is purely additive in this case.
2. **Given** the same scan target with a supplement declaring a vendored library `pkg:generic/liberror@1.2.3` with `licenses[0].license.id = "MIT"`, `supplier.name = "Acme Open Source Foundation"`, and `copyright = "© 2026 Acme"`, **When** the scan runs, **Then** the emitted SBOM's `liberror` component carries the declared license + supplier + copyright verbatim, the `mikebom:source-tier = declared` annotation, and the path/sha256 provenance of the supplement file on `metadata.properties`.
3. **Given** a supplement file containing 0 components/services/dependencies (an empty array set), **When** the scan runs, **Then** the emitted SBOM is byte-identical to a scan with no `--supplement-cdx` flag (the merge is a no-op; FR-013 preserves byte-identity).
4. **Given** the operator passes NO `--supplement-cdx` flag, **When** the scan runs, **Then** the emitted SBOM is byte-identical to pre-feature mikebom (the flag is opt-in; absence is no-op per FR-013).

---

### User Story 2 — A developer's declared license overrides the scanner's empty/unknown value, but cannot suppress scanner-detected bytes-evident content (Priority: P2)

A developer's project includes a manifest-bearing library `pkg:cargo/opaque-lib@1.0.0` whose `Cargo.toml` carries no license field. The scanner emits the component with `licenses[] = []` (no license detected). The developer KNOWS the library is licensed `Apache-2.0` (from upstream documentation) and declares this in `supplement.cdx.json`. The same developer also has a project that statically links openssl; the scanner detects `pkg:generic/openssl@3.0.10` via symbol fingerprints. The developer attempts to declare a supplement entry "no openssl" — which mikebom MUST reject (the scanner's bytes-evident detection cannot be suppressed via developer assertion).

After scanning with the supplement, the emitted SBOM:
- Carries `Apache-2.0` on `opaque-lib` (declared license overrides scanner's empty value)
- Still carries `pkg:generic/openssl@3.0.10` (the bytes-evident detection stands; the supplement's contradicting assertion is annotated as a disagreement but does NOT remove the component)
- Both sides retained on every disagreement so consumers can audit

**Why this priority**: This is the trust-calibration centerpiece from the issue body. Without the hard/soft split, the supplement either lets developers silently hide bytes-evident dependencies (security disaster) OR lets the scanner override every metadata field operators care about (operator-hostile). P2 because US1's merge mechanic is independently valuable, but US2 is what makes the feature safe to actually use in security-sensitive contexts.

**Independent Test**: A supplement file declaring (a) a license override on a scanner-known component with an empty license field AND (b) a "no openssl" assertion against a component the scanner detected via symbol fingerprint. Verify: (a) the declared license appears as primary on the emitted component AND the scanner's empty value appears as a secondary annotation; (b) the openssl component is STILL in the emitted SBOM with a disagreement annotation explaining the conflict.

**Acceptance Scenarios**:

1. **Given** the scanner emits `pkg:cargo/opaque-lib@1.0.0` with `licenses[] = []` AND the supplement declares the same PURL with `licenses[0].license.id = "Apache-2.0"`, **When** the scan runs, **Then** the emitted SBOM's `opaque-lib` component carries `Apache-2.0` as its primary license field AND a `mikebom:scanner-discovered-licenses = []` annotation so consumers can see what the scanner observed independently.
2. **Given** the scanner detects `pkg:generic/openssl@3.0.10` via symbol fingerprint evidence AND the supplement contains a component asserting "the project contains no openssl" (e.g., declares the PURL with `confidence = 0` or similar suppression attempt), **When** the scan runs, **Then** the emitted SBOM still contains the openssl component, its scanner-derived evidence intact, AND a `mikebom:assertion-conflict` annotation explaining that a developer assertion contradicted the bytes-evident detection. The bytes-evident component is NEVER dropped on the basis of a developer assertion.
3. **Given** a scanner-derived sha256 hash on a component AND the supplement declares a different sha256 for the same PURL, **When** the scan runs, **Then** the scanner's sha256 wins (bytes-derived fact); the supplement's value is preserved as a `mikebom:declared-sha256` annotation alongside a `mikebom:assertion-conflict` annotation.
4. **Given** a scanner-derived `name` field on a component AND the supplement declares a different `name` for the same PURL, **When** the scan runs, **Then** the developer's declared `name` wins (metadata is developer-domain), the scanner's name appears as a `mikebom:scanner-discovered-name` annotation, and a `mikebom:assertion-conflict` annotation surfaces the disagreement.

---

### User Story 3 — A consumer reading the emitted SBOM can tell scanner-observed components from developer-declared ones (Priority: P3)

A consumer reading the emitted SBOM (security scanner, SBOM-quality dashboard, vulnerability triager) wants to assign different trust weights to scanner-observed vs developer-declared components. They look for a single annotation on every component that tells them which source it came from. They also want to know which supplement file contributed declared content, with provenance — file path + sha256 — recorded on `metadata.properties` so they can correlate against the supplement file in source control. After this feature, every declared component carries `mikebom:source-tier = declared`; every scanner-observed component carries an existing tier annotation (`installed`/`analyzed`/etc.); the supplement file's provenance is recorded once at document scope.

**Why this priority**: Without consumer-facing transparency, operators using `--supplement-cdx` to add ground truth produce SBOMs that downstream consumers cannot interpret correctly. P3 because US1 + US2 ship a usable feature; US3 is what makes the feature trustworthy for downstream interoperability.

**Independent Test**: A scan with both scanner-observed components AND supplement-declared components. Verify: (a) every supplement-declared component carries `mikebom:source-tier = declared`; (b) the emitted SBOM's `metadata.properties` contains `mikebom:supplement-cdx` with the supplement file's path + sha256; (c) consumer can shell-grep the SBOM to enumerate declared-vs-observed counts.

**Acceptance Scenarios**:

1. **Given** a scan with both scanner-observed and supplement-declared components, **When** a consumer parses the emitted CDX, **Then** every supplement-declared component carries a `properties[].name = "mikebom:source-tier" value = "declared"` entry distinguishing it from scanner-observed components (which carry `installed` / `analyzed` / equivalent).
2. **Given** any scan with `--supplement-cdx <path>`, **When** the emitted CDX is inspected, **Then** `metadata.properties[]` contains a `mikebom:supplement-cdx` entry with the value `<path>@sha256:<hex>` so consumers can verify which supplement file fed the merge.
3. **Given** any scan WITHOUT `--supplement-cdx`, **When** the emitted CDX is inspected, **Then** the `mikebom:supplement-cdx` property is ABSENT from `metadata.properties[]` and the output is byte-identical to pre-feature mikebom.

---

### Edge Cases

- What happens when the supplement file is missing, unreadable, or malformed JSON? mikebom MUST exit non-zero before any walker begins, matching the milestone-113 `--exclude-path` fail-closed pattern. The error message MUST name the supplement path and the parse error verbatim so the operator can diagnose without re-running with debug logs.
- What happens when the supplement file is valid JSON but not valid CDX 1.6 (e.g., missing `bomFormat`, missing `specVersion`, malformed `components[]` array)? mikebom MUST exit non-zero with a schema-validation error. Schema enforcement uses the existing CDX 1.6 validator already exercised by milestone-013 format-parity tests.
- What happens when the supplement declares a `services[]` entry but the emitted SBOM format is SPDX 2.3 (which has no native `services[]` section)? Per the issue's research summary, SPDX 3.0.1 derives the equivalent via `Bundle` + `Relationship` + `primaryPurpose`. For v0.1 the supplement-cdx flag emits SPDX 2.3 with the services entries projected onto `packages[]` carrying a `mikebom:component-role = saas-service` annotation per the milestone-049 C40 pattern. SPDX 3 emission follows the issue's research recommendation (Bundle + Relationship). The CDX path is the lossless native path.
- What happens when the supplement and a scanner-discovered component declare the SAME PURL but the supplement's `bom-ref` differs from what the scanner's emission would have used? The scanner's emission shape (bom-ref convention) wins. The supplement's `bom-ref` is treated as advisory for the supplement's own internal dependency-graph references; mikebom re-anchors the supplement's `dependencies[]` references to the scanner's bom-refs at merge time.
- What happens when the supplement declares a component with a PURL that conflicts with another supplement component's PURL (within the same supplement file)? The supplement file MUST contain no duplicate PURLs across `components[]` AND `services[]`. mikebom rejects schema-valid supplements with intra-file duplicates at parse time with a clear error.
- What happens when the supplement's `dependencies[]` block references a `bom-ref` that doesn't exist in either the supplement OR the scanner output? mikebom MUST exit non-zero with a "dangling dependency reference" error. Supplements with broken dependency graphs are operator errors; they fail closed.
- What happens when the operator passes `--supplement-cdx` AND the supplement's PURL match-rule (FR-010) does NOT find any scanner-derived component to merge against? The supplement's entries are emitted as standalone components/services (US1's basic case). Non-matching is the normal case for SaaS deps and vendored libraries that the scanner has no on-disk evidence of.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: mikebom MUST accept a new `--supplement-cdx <PATH>` flag on `mikebom sbom scan` that takes exactly one path to a CDX 1.6 JSON document. The flag is repeatable in principle but for v0.1 only the first occurrence is honored (multi-file support is deferred to a future milestone per Out of Scope).
- **FR-002**: At scan startup, mikebom MUST parse and schema-validate the supplement file BEFORE any walker begins. Schema-invalid supplements cause non-zero exit with a clear error naming the supplement path and the specific failure. Parse / I/O failures cause non-zero exit per Constitution Principle III (Fail Closed).
- **FR-003**: mikebom MUST merge supplement's `components[]` entries into the emitted SBOM's `components[]` array, preserving every scanner-discovered component. Match-rule for collision-vs-additive determination is PURL exact equality (FR-010); non-matching entries are emitted as standalone components.
- **FR-004**: mikebom MUST merge supplement's `services[]` entries into the emitted SBOM's `services[]` array (CDX-native section that mikebom does not currently populate from any scanner). Service entries have no scanner-observed counterpart so no conflict resolution applies; they are emitted as-declared.
- **FR-005**: mikebom MUST merge supplement's `dependencies[]` block into the emitted SBOM's `dependencies[]` array, re-anchoring any supplement-internal `bom-ref` references to the scanner-side bom-refs when the same PURL exists on both sides.
- **FR-006**: When the supplement and scanner output declare overlapping facts on the SAME PURL, scanner-discovered bytes-derived facts MUST take precedence. The bytes-derived fact set includes: `hashes[]` (sha256, sha1, etc.), `cpe`, `purl` (the canonical-form PURL itself, including version), embedded version strings, and binary-fingerprint-derived component identity. Developer-declared values for these fields are preserved as `mikebom:declared-<field>` annotations alongside `mikebom:assertion-conflict` for transparency.
- **FR-007**: When the supplement and scanner output declare overlapping facts on the SAME PURL, developer-declared metadata MUST take precedence on the metadata field set. This includes: `licenses[]`, `supplier`, `copyright`, `name` (display name, distinct from PURL), `description`, and `externalReferences[]` (all types — `website` / `documentation` / `distribution` / `vcs` / `mailing-list` / `issue-tracker` / etc. — operator-supplied URLs are operator-domain regardless of the reference type). Scanner-derived values for these fields are preserved as `mikebom:scanner-discovered-<field>` annotations.
- **FR-008**: For every merge-time conflict between supplement-declared and scanner-discovered facts (regardless of which side won per FR-006/FR-007), mikebom MUST emit a `mikebom:assertion-conflict` annotation on the resulting component naming the field where the conflict occurred. Consumers reading the emitted SBOM can grep for this annotation to enumerate every disagreement.
- **FR-009**: Every `mikebom:assertion-conflict` annotation MUST carry a `justification` field with a minimal mikebom-specific enum value (initial set: `developer-metadata-override`, `bytes-evident-detection-preserved`). Expansion to the full OpenVEX justification vocabulary is an open question deferred to clarify phase.
- **FR-010**: Match-rule for "same component across supplement + scanner output" is **PURL exact-match (canonical form, including version)**. Two entries with the same canonical PURL collide; entries with different PURLs (even by name or version) do not. Pattern / wildcard / `name+version` match-rules are deferred to clarify Q5.
- **FR-011**: Every supplement-declared component or service MUST carry `mikebom:source-tier = declared` annotation in the emitted SBOM. Scanner-observed components carry their existing tier annotation (`installed` / `analyzed` / `source` / etc.) per pre-feature mikebom. Consumers grep for this annotation to enumerate declared vs observed.
- **FR-012**: When `--supplement-cdx <path>` is in effect, mikebom MUST emit a `mikebom:supplement-cdx` document-scope annotation on `metadata.properties[]` with value `<path>@sha256:<hex>` recording the supplement file's path AND its sha256 hash for provenance. The hash is computed over the raw bytes of the supplement file.
- **FR-013**: When NO `--supplement-cdx` flag is supplied, mikebom's emitted SBOM MUST be byte-identical to pre-feature mikebom output (modulo the existing random `serialNumber` and timestamp fields). The flag is purely opt-in; absence preserves backwards compatibility.
- **FR-014**: The supplement file's `metadata.component` field, if present, MUST be IGNORED by v0.1. The milestone-110 `SelfIdentity` resolver continues to own scan-target identity. Whether the supplement should override `metadata.component` is an open question (issue's Q3) deferred to clarify phase.
- **FR-015**: The developer MUST NOT be able to suppress scanner detection of bytes-evident content. Specifically: a supplement entry asserting "absence" of a scanner-detected component (via any mechanism — confidence=0, explicit removal directive, contradicting fact) does NOT remove the component from the emitted SBOM. The component remains; the assertion is preserved as an annotated conflict.

### Key Entities

- **Supplement file**: A CDX 1.6 JSON document hand-authored (or generated by another tool) by the operator, supplied via `--supplement-cdx <PATH>`. Contains `components[]`, `services[]`, and `dependencies[]` arrays declaring ground truth the scanner cannot observe. Schema-validated at scan startup; parse failures fail closed.
- **Declared component / service**: An entry from the supplement file that contributes to the emitted SBOM. Carries `mikebom:source-tier = declared` annotation. May or may not have a scanner-side counterpart by PURL exact-match.
- **Bytes-derived fact set**: The set of component fields where scanner-discovered evidence always wins over developer assertion. Initial fixed set: `hashes[]`, `cpe`, canonical `purl`, embedded version strings, binary-fingerprint identity. Per FR-006.
- **Metadata field set**: The set of component fields where developer assertion always wins over scanner-derived heuristics. Initial fixed set: `licenses[]`, `supplier`, `copyright`, display `name`, `description`, `externalReferences[]` (ALL types). Per FR-007.
- **Assertion conflict**: A merge-time disagreement between supplement-declared and scanner-discovered facts on the same component. Recorded as `mikebom:assertion-conflict` annotation on the resulting component naming the conflicted field; both values preserved (declared as primary or annotation depending on which set the field belongs to).
- **Justification**: A minimal enum surfacing why a conflict resolved the way it did. v0.1 initial values: `developer-metadata-override`, `bytes-evident-detection-preserved`. Full OpenVEX import deferred.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: After this feature ships, a developer declaring a SaaS dependency (Stripe / Twilio / etc.) with no on-disk footprint in a `supplement.cdx.json` file sees that service appear in the emitted SBOM's `services[]` section after running `mikebom sbom scan --supplement-cdx <file>`. The scanner blindspot is closed.
- **SC-002**: A developer's declared license on a scanner-discovered component overrides the scanner's empty/unknown license value; the consumer reading the emitted SBOM sees the operator-authoritative license as the primary value AND can audit the scanner's original (empty) value via the `mikebom:scanner-discovered-licenses` annotation.
- **SC-003**: Scanner-detected bytes-evident dependencies (openssl detected via symbol fingerprints, log4j detected via embedded version strings, etc.) are NEVER suppressible by developer assertion. A supplement file asserting "no openssl" still produces an SBOM containing openssl; the assertion is preserved as an annotated conflict for transparency.
- **SC-004**: A consumer reading the emitted SBOM can enumerate scanner-observed vs developer-declared components with a single grep over the `mikebom:source-tier` property, AND can identify which supplement file fed the merge via the document-scope `mikebom:supplement-cdx` property's path + sha256 value.
- **SC-005**: A malformed supplement file (missing JSON brace, malformed CDX schema, dangling dependency reference) produces a non-zero exit BEFORE any walker begins, with an error message naming the supplement path and the specific failure. No partial SBOM is emitted on supplement-parse failure.
- **SC-006**: When `--supplement-cdx` is NOT supplied, the emitted SBOM is byte-identical to pre-feature mikebom (modulo random `serialNumber` and timestamp fields). The flag is opt-in; backwards compatibility is total.

## Assumptions

- The supplement file is CDX 1.6 JSON. SPDX 2.3 / SPDX 3.0.1 supplement formats are deferred to a future milestone. The CDX 1.6 path is the v0.1 surface because (a) the issue body's research summary identifies CDX as the format that ALREADY expresses ~80% of the user stories natively, (b) the milestone-013 format-parity infrastructure already validates CDX 1.6 schemas, (c) operators emit CDX 1.6 as the default mikebom output format.
- The supplement file is hand-authored or generated by another tool (CMake-SBOM-Builder, custom scripts, etc.). mikebom does NOT scaffold or generate it. A `mikebom supplement init` subcommand is explicit Out of Scope per the issue body.
- The hard/soft split (FR-006 bytes-derived vs FR-007 metadata) is initially FIXED. Tuning the split per-PR (per-flag, per-component, per-field) is out of scope for v0.1.
- Constitution Principle II (eBPF-only discovery) is preserved: the supplement is ENRICHMENT, not discovery. The scanner discovers via filesystem walks (per Principle II's clarification that filesystem walks are mikebom's discovery substitute for eBPF in non-trace mode); the supplement enriches with operator-authoritative metadata. The Principle XII External Data Source Enrichment carve-out covers operator-supplied input the same way it covers lockfiles and deps.dev queries.
- Constitution Principle III (Fail Closed) governs supplement-parse failure: any error in reading, parsing, or schema-validating the supplement file causes non-zero exit before any walker begins. No partial SBOM is emitted with "supplement was malformed; scanned anyway" hedging.
- Constitution Principle V (Spec Compliance) is preserved: declarations honor CDX 1.6 schema; the merged output validates against CDX 1.6, SPDX 2.3, and SPDX 3.0.1 schemas the same way pre-feature mikebom output does. The milestone-013 format-parity test framework MUST continue to pass on every emitted format.
- A small "negative test" — a scan with a deliberately malformed supplement file — verifies SC-005's fail-closed acceptance criterion. A full integration test asserting the US1 + US2 + US3 acceptance scenarios verifies the merge semantics. A perf benchmark on a polyglot fixture with `--supplement-cdx` (vs without) verifies negligible overhead.
- The six open questions from the issue body — `vendored_at` semantics, aggregate `contains` reference resolution, manifest vs `--scan-as` precedence, justification enum scope, override match-rule grammar, multi-file manifest discovery — are deferred to clarify phase. Their resolutions may shrink or expand the v0.1 scope.
- The CMake-SBOM-Builder ecosystem produces SPDX, not CDX. Bidirectional sync (ingesting CMake-SBOM-Builder's SPDX as a supplement source) is explicit Out of Scope per the issue body. v0.1 supports CDX 1.6 only as the supplement format.
- The TOML manifest layer (the issue's Option B) is deferred. If real-world hand-authoring CDX JSON proves to be the friction point operators hit, layering TOML on top is a separate milestone whose semantics inherit from this v0.1 work.
