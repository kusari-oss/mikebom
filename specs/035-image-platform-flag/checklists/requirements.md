# Spec Quality Checklist: `--image-platform` flag (031.y)

**Checklist for** `/specs/035-image-platform-flag/spec.md`

## Coverage

- [X] Background cites the file:line seam (`mod.rs:204-220` for
      `host_oci_arch`, `platform.rs:18-42` for the resolver).
- [X] User story has P-priority (P1 — closes a workflow gap that
      blocks cross-arch dev users).
- [X] Independent Test is concrete (specific commands + observable
      `arch=` qualifier checks).
- [X] 6 acceptance scenarios cover override, variant, no-match,
      single-platform-noop, tarball-rejection, regression.
- [X] Edge Cases name OS rejection, variant absence, variant
      mismatch, clap interactions.
- [X] FR-001 through FR-007 numbered, each with exact file paths
      and signatures.
- [X] SC-001 through SC-005 measurable with explicit verification
      commands.
- [X] Out of Scope names every adjacent concern.

## Tighter spec set rationale

- [X] No `research.md` — no open architectural questions.
- [X] No `data-model.md` — `ParsedPlatform` is fully specified
      inline in FR-002.
- [X] No `contracts/` — public surface unchanged beyond a new
      `Option<String>` arg on one fn + one CLI flag.
- [X] No `quickstart.md` — 4 short files self-explanatory.

This is the fifth use of the 4-file template. Pattern stable.

## Concreteness

- [X] FRs cite specific file paths and exported items.
- [X] FR-006 names both rejection paths.
- [X] SC-005 quantifies LOC ceiling.

## Internal consistency

- [X] FR-002 (ParsedPlatform) aligns with FR-003 (resolver match)
      aligns with FR-004 (pull_to_tarball signature).
- [X] FR-005 (variant population) aligns with FR-003's resolver
      match logic.
- [X] Scenario 5 (tarball + flag) aligns with FR-006's rejection.

## Lessons from 016-034

- [X] Per-commit-clean discipline carried through.
- [X] Resolver refactor uses `Option<&str>` to preserve compatibility
      with existing callers (mechanical caller updates only).
- [X] Smoke test follows the established gating pattern from 031/034.

## Pre-implementation

- [X] [PHASE-1] T001 reconnaissance done.
- [ ] [PHASE-1] T002 baseline snapshot.
- [ ] [PHASE-2] Commit 1 landed.
- [ ] [PHASE-3] Commit 2 landed.
- [ ] [POLISH] SC-001-SC-005 verified.
- [ ] [POLISH] All 3 CI lanes green.
