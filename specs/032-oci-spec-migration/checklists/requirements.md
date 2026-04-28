# Spec Quality Checklist: oci-client → oci-spec migration

**Checklist for** `/specs/032-oci-spec-migration/spec.md`

## Coverage

- [X] Background section explains the durability problem
      (oci-client 0.12 going stale; future versions force aws-lc-sys)
      with cross-link to issue #65.
- [X] User story has a P-priority (P1 — durability) with the
      "before 031.x to avoid double-migration" rationale.
- [X] Independent Test is concrete (cargo tree assertions; smoke
      test parity contract; existing regression test as the
      durability guardrail).
- [X] Acceptance scenarios use Given/When/Then framing (6
      scenarios covering output parity, bearer-token flow, direct
      anonymous, manifest-list resolution, ref parsing, digest
      verification).
- [X] Edge Cases section names corner cases (HTTP redirects,
      manifest list with no host arch, streaming vs buffered,
      connection reuse, manifest media-type negotiation, error
      mapping).
- [X] Functional Requirements numbered (FR-001 through FR-012).
- [X] Success Criteria measurable (SC-001 through SC-008).
- [X] Clarifications captures the 4 scope decisions
      (ref parsing rules; bearer-token anonymous-only;
      no-new-transitive-deps; behavior-parity primacy).
- [X] Out of Scope explicitly cross-references the existing
      tracking issues (#66 / #67 / #68 / indefinite-defer items).

## Tighter spec set rationale (4 files vs 8)

- [X] No `research.md` — recon answered every architectural
      question (11th use of the 4-file template).
- [X] No `data-model.md` — only one new struct
      (`ImageReference`) specified inline + reuses oci-spec types.
- [X] No `contracts/` — public CLI surface is unchanged. Only
      internal substrate swap.
- [X] No `quickstart.md` — 4 short files self-explanatory.

This is the **11th use** of the 4-file template (after 021 / 022 /
023 / 024 / 025 / 026 / 028 / 029 / 030 / 031). Pattern fully
validated for both feature-introducing AND substrate-swap milestones.

## Independence

- [X] Single user story self-contained.
- [X] Each per-commit deliverable (3 commits) independently
      verifiable per FR-012 dual-profile clean.
- [X] **Sub-scoping discipline**: scope is purely the substrate
      swap. Auth, multi-arch flag, layer caching, etc. all stay
      in their tracked follow-on issues.

## Concreteness

- [X] FRs cite specific file paths (Cargo.toml, oci_pull/mod.rs
      etc.).
- [X] FR-001 names exact dep entry shape.
- [X] FR-002 names the 5 submodule split.
- [X] FR-003 cross-references Spec Scenario 5's 7 ref shapes.
- [X] FR-004 names the exact endpoints + Accept header content.
- [X] SC-002 quantifies the durability bar (zero oci-client / aws-lc
      entries).
- [X] SC-008 quantifies the LOC ceiling (~700 total).

## Internal consistency

- [X] FR-001 (Cargo.toml swap) + FR-002 (module split) flow into
      FR-003-007 (per-submodule contracts) into FR-008-009
      (tests) into FR-012 (per-commit clean).
- [X] Behavior-parity contract is the SC-004 success criterion —
      and the smoke tests are the verification path.
- [X] Plan.md's R-risks (R1-R5) align with the FR-003 ref-parsing
      acceptance + FR-004 bearer-token shape + FR-005 digest
      verification.

## Lessons from milestones 016-031

- [X] FR-012 carries the per-commit-clean discipline,
      extended to dual-profile (default + feature-on).
- [X] **Pattern alignment**: this milestone follows the same
      shape as milestones 029 (cargo-auditable used flate2 +
      serde_json directly) and 011 (SPDX 3 emission written
      directly). When a wrapper crate's dep graph drifts, work
      with the format directly using primitives we trust.
- [X] **Durability guardrail**: the
      `no_c_dependencies_in_oci_registry_feature_tree` test
      (added in milestone 031) carries forward — it locks in
      the substrate choice across this milestone.
- [X] Recon-first: every claim grounded in pre-spec recon at
      file:line level (Cargo.toml:15 for reqwest; oci_pull.rs:408
      LOC at start of milestone; oci-spec 0.9 metadata).

## Pre-implementation

- [X] [PHASE-1] T001 reconnaissance done (2026-04-28).
- [ ] [PHASE-1] T002 baseline snapshot captured (default
      tree + feature-on tree + smoke baseline).
- [ ] [PHASE-2] Commit 1 (substrate skeleton) landed.
- [ ] [PHASE-3] Commit 2 (registry.rs + migration) landed.
- [ ] [PHASE-4] Commit 3 (drop oci-client dep) landed.
- [ ] [POLISH] SC-001-SC-008 verified.
- [ ] [POLISH] All 3 standard CI lanes green on default
      profile.
- [ ] [POLISH] Manual end-to-end smoke test against
      `alpine:3.19` and
      `gcr.io/distroless/static-debian12:latest` matches
      milestone-031 baseline output exactly.

## Post-merge

- [ ] [QUALITATIVE] The `oci-registry` feature now sits on
      a substrate where future security advisories can be
      addressed by updating mikebom's own code rather than
      waiting on backports that won't come. If yes, milestone
      delivered.
- [ ] [DEFERRED FOLLOW-ONS] 031.x (auth, #66) is the
      highest-priority next step; built on the new substrate
      from this milestone.
