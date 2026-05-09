# Feature Specification: Clear sigstore-bundle transitive vulnerabilities

**Feature Branch**: `089-bump-sigstore-vulns`
**Created**: 2026-05-09
**Status**: Draft
**Input**: User description: "Bump sigstore 0.10 → 0.13 to clear 15 transitive vulns (tough + rustls-webpki)"

## Background

A trivy scan against mikebom's workspace `Cargo.lock` (production deps only; test fixtures excluded) surfaced **15 transitive vulnerabilities** from a single root: the `sigstore = "0.10"` dependency.

Severity breakdown:
- **5 HIGH**: 2 in `tough@0.18.0` (TUF signature-threshold bypass `CVE-2026-6966`, missing delegated-metadata validation `CVE-2026-6967`); 3 in `rustls-webpki@{0.101.7, 0.102.8, 0.103.12}` (DoS panic on malformed CRL BIT STRING `GHSA-82j2-j2ch-gfr8`).
- **5 MEDIUM**: 4 in `tough@0.18.0` (TUF root-version sequence check, terminating delegations, snapshot rollback, timestamp caching — `CVE-2025-2885` through `CVE-2025-2888`); 1 in `rustls-webpki@0.102.8` (CRL distribution-point matching `GHSA-pwjx-qhcg-rvj4`).
- **5 LOW**: 1 in `tough@0.18.0` (cyclic delegation graphs); 4 in `rustls-webpki` (URI/wildcard name-constraint accepts).

**14 of 15 vulns route through `sigstore@0.10.0`**. Only the modern `rustls-webpki@0.103.12` HIGH-severity entry has a co-attribution path through `oci-distribution@0.11.0`'s reqwest stack.

**Constraint (already documented in code)**: `mikebom-cli/Cargo.toml:137-138` carries an explicit pin to sigstore 0.10 with the rationale: *"0.13+ forces aws-lc-rs via the `cert` feature, violating Constitution Principle I (pure Rust, zero C). rustls-tls variants chosen to match existing reqwest rustls posture. See specs/006-sbomit-suite/research.md R1 for the audit."* Constitution Principle I (Pure Rust, Zero C) is non-negotiable.

This milestone reconciles the security-update goal with the constitution: drive the transitive vuln count to zero (or to a justified-and-tracked exception list) while keeping the user-space binary pure Rust.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Operators see zero HIGH transitive vulns from mikebom (Priority: P1)

A security-conscious operator runs a vuln scanner (trivy / syft / cargo-audit / Snyk) against mikebom's release binary or against the workspace `Cargo.lock` and sees zero HIGH-severity vulnerabilities attributable to mikebom's production dependency closure.

**Why this priority**: Two HIGH-severity TUF integrity bugs (`CVE-2026-6966`, `CVE-2026-6967`) are flagged in the very library mikebom uses to verify Cosign/Fulcio bundles — the integrity foundation of the `mikebom sbom verify` and `mikebom attestation verify` flows. An operator looking at mikebom for supply-chain integrity work sees its own deps flagging integrity-bypass CVEs in TUF; this is reputationally and substantively the most important class of finding to clear.

**Independent Test**: `trivy --quiet fs --scanners vuln --skip-dirs tests/fixtures --skip-dirs target --skip-dirs mikebom-cli/tests/fixtures --format json --output /tmp/post-089.json . && jq '[.Results[]? | select(.Target == "Cargo.lock") | .Vulnerabilities[]? | select(.Severity == "HIGH")] | length' /tmp/post-089.json` returns `0`.

**Acceptance Scenarios**:

1. **Given** mikebom's workspace `Cargo.lock` post-089, **When** an operator runs trivy with the production-dep-only invocation above, **Then** zero HIGH-severity vulns are reported under target `Cargo.lock`.
2. **Given** mikebom's workspace `Cargo.lock` post-089, **When** an operator runs `cargo audit` (with a CVSS-4.0-aware build), **Then** zero HIGH-severity advisories are reported.

---

### User Story 2 - Operators see zero MEDIUM/LOW transitive vulns from mikebom (Priority: P2)

Same operator runs the same scanner; zero MEDIUM- and LOW-severity vulns are flagged from mikebom's production dep closure (or any remaining non-zero count is documented with a per-finding justification + tracked CVE-acceptance entry).

**Why this priority**: MEDIUM/LOW findings are noise-level for most consumers but accumulate FUD. Closing them lets `mikebom audit` consumers see a clean run. Lower than P1 because the integrity-bypass HIGHs have material exploit potential while name-constraint LOWs are theoretical attacks against unusual cert chains.

**Independent Test**: same trivy invocation as US1; `[.Results[]? | select(.Target == "Cargo.lock") | .Vulnerabilities[]? | select(.Severity == "MEDIUM" or .Severity == "LOW")] | length` returns either `0`, OR a value `N` matched by the same `N` entries documented in `specs/089-bump-sigstore-vulns/known-acceptances.md` (if the spec creates such a file).

**Acceptance Scenarios**:

1. **Given** the post-089 dep closure, **When** the operator filters for MEDIUM+LOW, **Then** the count is zero, OR every flagged advisory has a corresponding entry in a maintainer-curated acceptance list with a justification + tracked-bump-target CVE.

---

### User Story 3 - mikebom attestation features remain functional (Priority: P1)

A maintainer running `mikebom sbom verify <bundle>` against a Cosign-signed in-toto attestation bundle sees identical pass/fail behavior pre-vs-post-089. `mikebom attestation sign` and `mikebom attestation verify` (used in the milestone-006 SBOMit suite) produce byte-identical outputs for identical inputs.

**Why this priority**: Tied with US1 — a security update that breaks the security feature it's meant to protect is a net regression. The attestation flows are the most security-sensitive code paths in the project.

**Independent Test**: every existing test in `mikebom-cli/src/attestation/` and every integration test under `mikebom-cli/tests/` that exercises the `attestation/` module passes post-089 with the same assertions. Specifically the milestone-006 SBOMit acceptance tests + the milestone-072 cross-tier-binding tests.

**Acceptance Scenarios**:

1. **Given** the existing `mikebom-cli/tests/sbom_user_metadata.rs` + `attestation_*.rs` test suite, **When** the maintainer runs `cargo +stable test -p mikebom`, **Then** every attestation-related test reports `0 failed`.
2. **Given** a Cosign-signed bundle that verifies pre-089, **When** an operator runs `mikebom sbom verify <bundle>` post-089, **Then** the verification succeeds with the same exit code + same printed output.
3. **Given** an operator signs an in-toto envelope via `mikebom attestation sign` pre-089 and verifies it via post-089 mikebom, **Then** the verification succeeds (cross-version compat).

---

### Edge Cases

- **Sigstore 0.11 / 0.12 also pull aws-lc-rs**: if true, the simple bump path is closed across all 0.10+ versions and we'd need to either (a) accept the vulns + document the rationale, or (b) override the transitive deps via `[patch]` at the workspace level, or (c) fork sigstore to revert the aws-lc-rs change. Plan-level decision.
- **`tough` and `rustls-webpki` are reachable independently of sigstore**: the modern `rustls-webpki@0.103.12` flag has a dual path through `oci-distribution@0.11.0` + `reqwest@0.12.28`. Bumping sigstore alone won't necessarily clear the `oci-distribution` co-attribution; that path may need its own dep bump.
- **API breakage in sigstore 0.11+**: if a new sigstore version is viable, the `attestation/{verifier,signer,serializer}.rs` modules use `sigstore::crypto::{SigStoreSigner, SigStoreKeyPair, SigningScheme}` types that may be renamed/moved. Plan-level migration work.
- **Goldens stability**: signing flows can produce non-deterministic outputs (random nonces). Existing tests should already use deterministic mocks; if any golden uses real signing output, the bump may regen unexpectedly. Need to verify scope is golden-stable.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Post-089 production dep closure (workspace `Cargo.lock`, excluding test fixtures) MUST emit zero HIGH-severity transitive vulnerabilities under a trivy + cargo-audit scan, OR every remaining HIGH MUST be documented in a maintainer-curated CVE-acceptance list with a justification and a tracked next-step (upstream PR, fork, or patch override).
- **FR-002**: Post-089 production dep closure MUST emit zero MEDIUM+LOW vulnerabilities, OR each remaining vuln MUST be documented per FR-001's acceptance pattern.
- **FR-003**: The user-space mikebom binary MUST remain pure Rust (Constitution Principle I). The fix MUST NOT introduce `aws-lc-rs`, `openssl-sys`, `libgit2-sys`, or any other crate that links a C library.
- **FR-004**: The `mikebom-cli/Cargo.toml:137-138` pin comment MUST be updated to reflect the post-089 sigstore version + feature set. If the pin remains at 0.10.x with vuln acceptances, the comment MUST cite the acceptance list.
- **FR-005**: All existing `mikebom-cli/src/attestation/*` tests MUST pass post-089 with zero behavioral changes (no test deletions, no assertion weakening). Migration to a new sigstore API MAY require code changes inside `attestation/{verifier,signer,serializer}.rs`, but the externally observable test outcomes stay identical.
- **FR-006**: `mikebom sbom verify` MUST produce byte-identical CLI output (exit code + stdout + stderr) pre-vs-post-089 against a fixed Cosign-signed bundle test input.
- **FR-007**: The fix MUST add OR maintain a CI gate that runs the production-deps trivy scan and fails CI on any new HIGH vuln in the production closure. Test fixtures (intentionally vulnerable lockfiles for ecosystem testing) MUST remain excluded from this gate via path-based skip, not by allow-listing.
- **FR-008**: Existing 27 SBOM goldens (CDX, SPDX 2.3, SPDX 3 across 9 ecosystems) MUST stay byte-identical post-089 unless a sigstore-emitted field appears in the goldens (which would be unexpected — sigstore's role is verify-side, not emit-side). If goldens regenerate, scope has crept and the spec MUST document why.

### Key Entities

- **Workspace `Cargo.lock`**: the single resolved dep graph for the mikebom user-space binary. Test-fixture `Cargo.lock` files are NOT in scope and MUST stay separately scannable.
- **Production-deps trivy scan**: the audit command run for FR-001/FR-002 (`trivy --quiet fs --skip-dirs tests/fixtures --skip-dirs mikebom-cli/tests/fixtures --skip-dirs target --format json .`).
- **Attestation modules**: `mikebom-cli/src/attestation/{verifier,signer,serializer}.rs` — the only files that import from `sigstore::*`.
- **CVE-acceptance list** (created if needed): `specs/089-bump-sigstore-vulns/known-acceptances.md` documenting any deliberately-unfixed advisories with justification + tracked-resolution.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Operators running a vuln scanner against a mikebom release binary or `Cargo.lock` see zero HIGH advisories from mikebom's production dependencies (or a documented per-finding justification list of length `N` matching exactly `N` flagged advisories).
- **SC-002**: 100% of `mikebom-cli/src/attestation/*` tests + every milestone-006 SBOMit acceptance test + every milestone-072 cross-tier-binding test pass post-089. Zero test deletions; zero assertion weakenings.
- **SC-003**: `cargo +stable test --workspace` post-089 reports `0 failed` for every test suite. `cargo +stable clippy --workspace --all-targets -- -D warnings` post-089 reports zero warnings.
- **SC-004**: The mikebom binary remains pure Rust — `cargo tree -e features --no-default-features` against a fresh build shows zero `*-sys` crates that link C libraries (excluding `libc` which is a Rust shim, and `linux-raw-sys` which is also Rust).
- **SC-005**: Production dep scan wall-time stays ≤30 s on standard developer hardware (matches the existing trivy invocation cost).

## Assumptions

- Sigstore 0.11.0 OR 0.12.x are available on crates.io (verified: yes — 0.11.0 released 2025-02-06, 0.12.0 released 2025-05-09, 0.12.1 released 2025-05-28, 0.13.0 released 2025-10-16). Whether 0.11 or 0.12 avoid the aws-lc-rs requirement is a plan-level investigation.
- mikebom uses sigstore for crypto primitives only (`SigStoreSigner`, `SigStoreKeyPair`, `SigningScheme` — verified by grep). It does NOT use sigstore's TUF client (`tough` integration) directly. The `tough` vulns are dragged in by sigstore's optional features even though mikebom doesn't exercise the TUF code paths. Whether disabling those features in 0.10 clears the `tough` dep is a plan-level investigation.
- The `oci-distribution@0.11.0`-rooted co-attribution to `rustls-webpki@0.103.12` (1 HIGH) may resolve as a side-effect of any reqwest/rustls bumps performed during sigstore migration, OR may need a separate `oci-distribution` bump.
- No new external systems (no new MCP servers, no new GitHub Actions, no new release-pipeline steps).

## Dependencies

- Constitution Principle I (Pure Rust, Zero C) — non-negotiable; constrains the bump target.
- `mikebom-cli/Cargo.toml:137-138` — pin comment must be updated post-fix.
- `specs/006-sbomit-suite/research.md R1` — the original audit that established the 0.10.x pin. Must be referenced in the post-089 research.md.
- Milestone 087's release (alpha.26) as the baseline. Post-089 may or may not warrant an alpha.27 release depending on user-observable behavior change scope.

## Out of Scope

- Fixing test-fixture vulnerabilities. The 38 vulns in `tests/fixtures/*` are intentional (the fixtures are real-world projects pinned at versions where vuln-detection differs across SBOM tools).
- Adding new attestation features. This is a security maintenance milestone, not a feature addition.
- Refactoring `attestation/{verifier,signer,serializer}.rs` beyond what's needed for sigstore API migration.
- Switching off `sigstore` to a different crypto library entirely (e.g., `k256`, `ed25519-dalek`). That's a much larger scope change worth its own milestone if maintainability becomes an issue, but isn't required for this fix.
