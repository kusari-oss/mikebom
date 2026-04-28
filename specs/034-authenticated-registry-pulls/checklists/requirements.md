# Spec Quality Checklist: Authenticated OCI Registry Pulls (031.x)

**Checklist for** `/specs/034-authenticated-registry-pulls/spec.md`

## Coverage

- [X] Background section names what's missing today (anonymous-only) and
      cites the file:line auth seam (`registry.rs::fetch_with_bearer_retry`
      line 107, `fetch_bearer_token` line 166).
- [X] User story has a P-priority (P1) and a "why this priority"
      justification (highest-value follow-on; closes the workflow gap that
      forces `docker pull && docker save`).
- [X] Independent Test is concrete (specific commands + observable
      outcomes + secret-leak audit).
- [X] Acceptance scenarios use Given/When/Then framing (5 scenarios, each
      naming a different cred source).
- [X] Edge Cases section names the corner cases (identitytoken, helper
      failure, helper stdin contract, registry name normalization, Debug
      impl, empty auth field, ECR).
- [X] Functional Requirements numbered (FR-001 through FR-008), each with
      a concrete file path and signature.
- [X] Key Entities — `DockerConfig`, `Credential`, `AuthEntry` introduced
      with field-level detail in the FRs.
- [X] Success Criteria measurable (SC-001 through SC-007), each with an
      explicit verification command.
- [X] Clarifications section captures the 4 scope decisions (bearer+creds
      vs Basic-direct, no new base64 dep, no `dirs` crate, helper timeout).
- [X] Out of Scope names every adjacent concern (push, mirror configs,
      OAuth refresh, native ECR SDK, --registry-auth flag, 031.y, 031.z).

## Tighter spec set rationale (4 files vs 8)

- [X] No `research.md` — recon answered every architectural question; no
      open "should we do X or Y" decisions.
- [X] No `data-model.md` — FR-001 fully specifies the new types inline.
- [X] No `contracts/` — public surface unchanged; FR-008 / SC-003 enforce.
- [X] No `quickstart.md` — 4 short files self-explanatory.

This is the fourth use of the 4-file template (after 021, 022, 023). The
pattern is stable for genuinely contained milestones.

## Independence

- [X] Single user story self-contained.
- [X] Each per-commit deliverable independently verifiable (per FR / SC
      mapping each commit's `./scripts/pre-pr.sh` passes).

## Concreteness

- [X] FRs cite specific file paths and exported items.
- [X] FR-001 names the exact struct + function signatures.
- [X] FR-003 / FR-004 name the exact registry.rs surgery points.
- [X] SC-004 quantifies the LOC ceiling (500 for auth.rs).
- [X] SC-005 / SC-006 name the verification commands verbatim.

## Internal consistency

- [X] FR-001 (auth.rs surface) aligns with FR-003 (registry.rs uses it)
      aligns with FR-004 (basic_auth on realm fetch).
- [X] FR-006 (no secret leak) aligns with SC-006 (grep audit) and the
      `Credential::Debug` redaction in FR-001.
- [X] Edge Case "helper exit code != 0" aligns with FR-002's "fall through
      to anonymous; no helper stderr captured".
- [X] FR-007 (no_c_dependencies regression test) aligns with the milestone
      032 substrate inheritance — guardrail still active.

## Lessons from milestones 016-033

- [X] FR-008 carries the per-commit-clean discipline.
- [X] R3 in plan.md (`#[cfg(unix)]` shim test) anticipates Windows-CI
      behavior for the cred-helper subprocess test.
- [X] The auth-on-bearer-token-fetch design reuses the existing
      `fetch_bearer_token` shape from milestone 032 — no new abstractions.
- [X] Recon-first: every claim in the spec backed by file:line refs from
      the just-read registry.rs.
- [X] Secret-handling discipline (FR-006 + SC-006) follows Constitution
      Principle X transparency BUT explicitly excludes secrets — codifying
      the lesson that "transparency does not extend to credentials".

## Pre-implementation

- [X] [PHASE-1] T001 reconnaissance done (2026-04-26).
- [ ] [PHASE-1] T002 baseline snapshot captured.
- [ ] [PHASE-2] Commit 1 (auth module) landed.
- [ ] [PHASE-3] Commit 2 (wire-up) landed.
- [ ] [PHASE-4] Commit 3 (docs + smoke) landed.
- [ ] [POLISH] SC-001-SC-007 verified.
- [ ] [POLISH] All 3 CI lanes green.

## Post-merge

- [ ] [QUALITATIVE] Next time someone tries `mikebom sbom scan --image
      ghcr.io/<priv>/<repo>:tag` with a working `~/.docker/config.json`,
      the scan succeeds without a "private registries not yet supported"
      hint. If yes, milestone delivered.
- [ ] [FOLLOW-ON] Re-evaluate whether 031.y `--image-platform` (#67) or
      031.z layer caching (#68) is the next priority, or if a different
      surface area (e.g., #64 dpkg `status.d/`) jumps ahead.
