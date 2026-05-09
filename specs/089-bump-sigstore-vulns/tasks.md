---
description: "Tasks: Clear sigstore-bundle transitive vulnerabilities (sigstore 0.10 → 0.11 + drop sigstore-trust-root feature)"
---

# Tasks: Clear sigstore-bundle transitive vulnerabilities

**Input**: Design documents from `/specs/089-bump-sigstore-vulns/`
**Prerequisites**: spec.md ✅, plan.md ✅, research.md ✅, data-model.md ✅, contracts/sigstore-feature-set.md ✅, quickstart.md ✅

**Organization**: Small dep-bump milestone with regression-net testing. Phase 1 captures pre-fix evidence. Phase 2 = the actual bump (single Cargo.toml edit). Phase 3 = US3 regression net (verify attestation tests pass post-bump — must run BEFORE US1/US2 because if tests break, the bump is wrong). Phase 4 = US1 (HIGH vuln verification). Phase 5 = US2 (MED/LOW + acceptance list). Phase 6 = Polish (CI gate, audit-doc, pin comment, pre-PR gate).

## Path Conventions

Repository-relative paths from `/Users/mlieberman/Projects/mikebom/`:
- Cargo manifest (sigstore dep + pin comment): `mikebom-cli/Cargo.toml` (lines 137-141)
- Resolved dep graph: `Cargo.lock`
- Attestation modules (regression net): `mikebom-cli/src/attestation/{verifier,signer,serializer}.rs`
- Milestone-006 audit doc (rationale update): `specs/006-sbomit-suite/research.md` R1 row
- CI workflow (production-deps trivy gate): `.github/workflows/ci.yml`
- CVE acceptance list: `specs/089-bump-sigstore-vulns/known-acceptances.md` (created if needed)

---

## Phase 1: Setup (pre-fix evidence)

- [X] T001 [P] Capture the pre-fix vuln baseline per quickstart Recipe 1: run trivy with the production-dep-only invocation (`--skip-dirs tests/fixtures --skip-dirs mikebom-cli/tests/fixtures --skip-dirs target`), save to `/tmp/pre-089.json`. Confirm 5 HIGH + 5 MEDIUM + 5 LOW vulns in the workspace `Cargo.lock` target. Records the baseline for FR-001 / FR-002 verification.
- [X] T002 [P] Verify the byte-identical-API claim from research §3 by re-confirming `diff sigstore-0.10.0/src/crypto/signing_key/mod.rs sigstore-0.11.0/src/crypto/signing_key/mod.rs` returns empty AND `diff sigstore-0.10.0/src/crypto/mod.rs sigstore-0.11.0/src/crypto/mod.rs` returns empty. (Cached crates from Phase 0 research already in `/tmp/sigstore-0.10.0/` and `/tmp/sigstore-0.11.0/`.)

## Phase 2: Foundational (the bump)

- [X] T003 Edit `mikebom-cli/Cargo.toml:141` per quickstart Recipe 2: change `sigstore = { version = "0.10", ... features = ["sigstore-trust-root-rustls-tls", "cosign-rustls-tls", "fulcio-rustls-tls", "rekor-rustls-tls", "bundle"] }` to `sigstore = { version = "0.11", ... features = ["cosign-rustls-tls", "fulcio-rustls-tls", "rekor-rustls-tls", "bundle"] }` (drop `sigstore-trust-root-rustls-tls`). Verifies VR-089-001, VR-089-002, VR-089-003.
- [X] T004 Regenerate `Cargo.lock` via `cargo +stable update -p sigstore`. If sigstore 0.11 transitively bumps openidconnect 3.5 → 4.0 forces other dep churn beyond what `cargo update -p sigstore` resolves, run `rm Cargo.lock && cargo +stable build` for a full re-resolve.
- [X] T005 Verify `cargo +stable build --workspace` compiles cleanly. If any `attestation/{verifier,signer,serializer}.rs` import or method call fails to resolve, investigate per quickstart "When in doubt" section — check sigstore CHANGELOG between 0.10 and 0.11 for the affected symbol. Expected: zero compilation errors based on the byte-identical-API finding from T002.
- [X] T006 Verify `Cargo.lock` post-bump satisfies VR-089-004 (no `tough` entries), VR-089-005 (no `aws-lc-rs` / `aws-lc-sys`), VR-089-006 (sigstore at 0.11.x): run the assertions from quickstart Recipe 3 steps 3–5.

## Phase 3: US3 — Attestation features remain functional (Priority: P1) — regression net

**Goal**: Confirm `cargo +stable test -p mikebom` reports zero failures in attestation-related tests post-bump. Byte-identical CLI behavior for `mikebom sbom verify` against a Cosign-signed bundle.

**Independent Test**: `cargo +stable test -p mikebom attestation` — every attestation-related test reports `0 failed` with the same assertions. Plus a manual `mikebom sbom verify <fixture-bundle>` smoke test producing identical exit code + stdout + stderr.

**Why first**: if tests break, the bump is wrong and US1/US2 are moot. This phase gates Phases 4 + 5.

### Implementation for User Story 3

- [X] T007 [US3] Run `cargo +stable test -p mikebom attestation` and confirm every test passes. Verifies FR-005 (no test-deletion / assertion-weakening).
- [X] T008 [US3] Run `cargo +stable test --workspace` (full test suite, not just attestation). Confirms no cross-module regression from the dep-graph change. Verifies SC-003.
- [X] T009 [US3] Smoke-test `mikebom sbom verify` end-to-end against an existing Cosign-signed bundle test fixture (find one via `find mikebom-cli/tests -name "*cosign*" -o -name "*bundle*" 2>/dev/null`). Compare exit code + stdout + stderr against pre-fix output captured in T001's session. Verifies FR-006.

## Phase 4: US1 — Zero HIGH transitive vulns (Priority: P1)

**Goal**: trivy production-deps scan reports zero HIGH-severity vulns under target `Cargo.lock`, OR every remaining HIGH is documented in `known-acceptances.md` with a justification + upstream-tracking link.

**Independent Test**: `jq '[.Results[]? | select(.Target == "Cargo.lock") | .Vulnerabilities[]? | select(.Severity == "HIGH")] | length' /tmp/post-089.json` returns 0, OR returns N matching exactly N entries in `known-acceptances.md`.

### Implementation for User Story 1

- [X] T010 [US1] Re-run trivy per quickstart Recipe 4 against the post-bump workspace. Save to `/tmp/post-089.json`. Compare HIGH-severity counts: pre-fix had 5 HIGH (2 tough + 3 rustls-webpki); post-fix expected ≤2 HIGH (the 2 tough HIGHs are eliminated by the feature drop; rustls-webpki HIGHs may remain).
- [X] T011 [US1] If any HIGH remains in `/tmp/post-089.json` under target `Cargo.lock`, create `specs/089-bump-sigstore-vulns/known-acceptances.md` and add an entry per data-model.md "CVE-acceptance list" schema. Each entry MUST cite: id, crate@version, severity, mikebom_exposure prose, upstream_tracking link, re_review_date. Verifies VR-089-010 + VR-089-011.

## Phase 5: US2 — Zero MED/LOW transitive vulns (Priority: P2)

**Goal**: trivy production-deps scan reports zero MEDIUM- or LOW-severity vulns under target `Cargo.lock`, OR every remaining MED/LOW is documented in `known-acceptances.md`.

**Independent Test**: same trivy invocation as US1; `[.Results[]? | select(.Target == "Cargo.lock") | .Vulnerabilities[]? | select(.Severity == "MEDIUM" or .Severity == "LOW")] | length` returns 0, OR returns N matching N entries in `known-acceptances.md`.

### Implementation for User Story 2

- [X] T012 [US2] Filter `/tmp/post-089.json` for MED + LOW under target `Cargo.lock`. Pre-fix had 5 MED + 5 LOW; post-fix expected: 4 MED tough entries eliminated (feature drop), 1 LOW tough entry eliminated, leaving residual rustls-webpki MED/LOWs (1 MED + 4 LOW pre-fix; some may persist via sigstore's `rustls-webpki = "0.102"` direct dep).
- [X] T013 [US2] For each remaining MED/LOW entry not yet on the acceptance list, append to `specs/089-bump-sigstore-vulns/known-acceptances.md` with the same schema as T011. Verifies VR-089-010.

## Phase 6: Polish

- [X] T014 Update the pin comment at `mikebom-cli/Cargo.toml:137-138` per quickstart Recipe 2 / data-model.md VR-089-012/013/014: change "0.13+ forces aws-lc-rs" → "0.12+ forces aws-lc-rs"; add a sentence explaining the milestone-089 feature drop. Preserve the milestone-006 R1 audit reference + the "rustls-tls variants chosen" sentence.
- [X] T015 Update `specs/006-sbomit-suite/research.md` R1 audit row to reflect sigstore 0.11.0 + the corrected aws-lc-rs cliff (0.12+, not 0.13+). Cross-link to milestone 089's research.md §1 for the corrected analysis.
- [X] T016 Add the production-deps trivy CI gate to `.github/workflows/ci.yml` per quickstart Recipe 5. Use the existing Linux lane that already has trivy 0.69.3 installed for milestone-083's audit. The gate fails the lane on any HIGH vuln in `Cargo.lock` not on `known-acceptances.md`. Verifies FR-007.
- [X] T017 Verify zero golden regenerations: `git status --short mikebom-cli/tests/fixtures/golden/` returns empty. Verifies FR-008. ANY golden regen indicates scope creep — narrow the diff.
- [X] T018 Run `./scripts/pre-pr.sh`: zero clippy warnings + every test suite reports `0 failed`. Verifies SC-002 + SC-003 + the standard CLAUDE.md mandatory gate.
- [X] T019 Update CLAUDE.md "Recent Changes" if the speckit infrastructure didn't auto-update it (verify with `grep "089-bump-sigstore-vulns" CLAUDE.md`). Speckit-plan typically auto-adds; this is a verify step.

---

## Dependencies & Execution Order

- T001 + T002 (Phase 1 evidence) — both `[P]`, can run in parallel.
- T003 → T004 → T005 → T006 (Phase 2 sequential, same files).
- T007 → T008 → T009 (Phase 3 US3 sequential — fast tests first; T008 strictly supersedes T007 but explicit ordering helps debugging).
- **Phase 3 MUST complete before Phase 4 + 5** — if tests fail, the bump is wrong and vuln-count work is wasted.
- T010 → T011 (Phase 4 US1 sequential — scan then acceptance-list-update).
- T012 → T013 (Phase 5 US2 sequential — same shape as Phase 4).
- T014, T015, T016 (Polish doc/comment/CI work) — independent files, can run in parallel after Phases 3-5 complete.
- T017 → T018 → T019 (Polish verification — sequential because pre-PR gate must run last).

## Parallel Opportunities

- T001 + T002 (Phase 1 evidence) — independent trivy scan vs. file diff.
- T014 + T015 + T016 (Polish doc edits) — different files, no dependencies. Can parallelize.

## Notes

- **No new Cargo dependencies** at the lockfile level beyond the natural sigstore 0.10 → 0.11 closure (which SHRINKS by removing tough). Verifies FR-003 + SC-004.
- **Zero golden regenerations** (verify-side library bump; no SBOM-emission code path touched). Verifies FR-008. T017 enforces this explicitly.
- **PR diff target**: ~50 LOC across 4–6 files (`mikebom-cli/Cargo.toml`, `Cargo.lock`, `specs/006-sbomit-suite/research.md`, `.github/workflows/ci.yml`, optionally `attestation/*` if API migration is needed, plus this milestone's spec dir).
- **Suggested MVP scope**: Phases 1 + 2 + 3 + 4 (the dep bump + regression net + HIGH-vuln clearance). US2 (MED/LOW) and Polish are the polish-pass tail; could ship as a follow-up if scope balloons.
- **Tasks T007 + T008 ordering**: T008 strictly includes T007's test surface (running --workspace runs every test, including attestation). The split is for fast-feedback debugging — if T007 fails, the failure is contained to attestation and T008 isn't worth running yet.
