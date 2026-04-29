# Spec Quality Checklist: dpkg `status.d/` reader

**Checklist for** `/specs/037-dpkg-status-d/spec.md`

## Coverage

- [X] Background cites the file:line root cause (`dpkg.rs:21`).
- [X] User story has P-priority (P1 — correctness, not polish).
- [X] Independent Test is concrete (specific image + specific
      package names).
- [X] 5 acceptance scenarios cover the cross product
      (status.d/ only, legacy only, both, companion files,
      status filter).
- [X] Edge Cases name empty dir, non-UTF-8 names, symlinks,
      companion files, multi-stanza files.
- [X] FR-001 through FR-006 numbered, each with file paths and
      function signatures.
- [X] SC-001 through SC-007 measurable with explicit verification
      commands.
- [X] Out of Scope names every adjacent concern (per-file deep
      hashing, apko apk variant, rpm variants, PURL changes).

## Tighter spec set rationale

- [X] No `research.md` — no open questions; recon nailed the seam.
- [X] No `data-model.md` — no new types.
- [X] No `contracts/` — public surface unchanged.
- [X] No `quickstart.md` — short.

Seventh use of the 4-file template. Pattern stable.

## Concreteness

- [X] FRs cite specific file paths and functions.
- [X] FR-002 names the exact filter pattern (extension-less files).
- [X] FR-004 names the exact test rename + new assertion.

## Internal consistency

- [X] FR-001 (read extension) aligns with FR-003 (dedup) aligns
      with FR-005 (test coverage).
- [X] Edge Case "companion files" aligns with FR-002's
      extension-skip rule.
- [X] FR-006 (no output-shape change) aligns with SC-003 (zero
      golden drift).

## Lessons from 016-036

- [X] Per-commit-clean discipline.
- [X] Reuse over reinvention: `parse_stanza` and `split_stanzas`
      are unchanged; only the source-discovery loop is new.
- [X] Synthetic-fixture pattern (tempdir-with-rootfs-shape) follows
      the existing test conventions in dpkg.rs.

## Pre-implementation

- [X] [PHASE-1] T001 reconnaissance done.
- [ ] [PHASE-1] T002 baseline snapshot.
- [ ] [PHASE-2] Commit 1 landed.
- [ ] [PHASE-3] Commit 2 landed.
- [ ] [POLISH] All SCs verified.
- [ ] [POLISH] CI green.

## Post-merge

- [ ] [QUALITATIVE] `mikebom sbom scan --image gcr.io/distroless/static-debian12:latest`
      reports 4+ components instead of 0. If yes, milestone delivered.
