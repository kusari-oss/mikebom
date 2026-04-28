# Spec Quality Checklist: Mach-O Codesign Metadata

**Checklist for** `/specs/030-macho-codesign-metadata/spec.md`

## Coverage

- [X] Background section explains why codesign metadata is missing
      today (deferred from milestone 024) + cites file:line evidence
      (`object/macho.rs:727` for LC_CODE_SIGNATURE = 0x1D; Apple's
      cs_blobs.h source for SuperBlob/CodeDirectory format).
- [X] User story has a P-priority (P1 — correctness) and a "why
      this priority" justification grounded in the supply-chain
      attestation use case.
- [X] Independent Test is concrete (specific tests + observable
      annotations + macOS CI lane SC-002 anchor).
- [X] Acceptance scenarios use Given/When/Then framing (6
      scenarios covering Apple-signed system binary, ad-hoc-
      signed, unsigned, multi-flag, fat binary, malformed
      SuperBlob).
- [X] Edge Cases section names corner cases (CD version <
      0x20200, empty identifier, detached signature, CMS
      envelope deferred, entitlements deferred, multiple
      LC_CODE_SIGNATURE, big-endian SuperBlob fields,
      unrecognized flag bits).
- [X] Functional Requirements numbered (FR-001 through FR-011).
- [X] Key Entities — `CodeDirectoryView` private type
      specified inline in FR-002.
- [X] Success Criteria measurable (SC-001 through SC-008), each
      with a verification mechanism.
- [X] Clarifications section captures the 5 scope decisions
      (3 signals only; flags as JSON array; unknown bits emit
      with hex; BE quirk; skip-on-empty contract).
- [X] Out of Scope explicitly names every adjacent concern
      (entitlements XML; CMS PKCS#7; designated requirements;
      hash-list verification; notarization; detached signatures;
      PE Authenticode).

## Tighter spec set rationale (4 files vs 8)

- [X] No `research.md` — recon answered every architectural
      question (9th use of the 4-file template after 021, 022,
      023, 024, 025, 026, 028, 029).
- [X] No `data-model.md` — only 3 BinaryScan field additions +
      1 private struct `CodeDirectoryView` specified inline.
- [X] No `contracts/` — no public API surface change beyond
      catalog rows C37/C38/C39 + 3 bag keys.
- [X] No `quickstart.md` — 4 short files self-explanatory.

This is the **9th use** of the 4-file template. Pattern fully
validated for contained binary-extraction milestones.

## Independence

- [X] Single user story self-contained.
- [X] Each per-commit deliverable (3 commits) independently
      verifiable (per FR-011 each commit's pre-PR passes).

## Concreteness

- [X] FRs cite specific file paths and line numbers
      (`binary/macho.rs::for_each_load_command` for the load-
      command walk; `object/macho.rs:727` for the LC_CODE_SIGNATURE
      const; existing `build_macho_identity_annotations` helper
      from milestone 024 as the extension point).
- [X] FR-001 names the 3 exact pub fn signatures.
- [X] FR-005 names the 3 exact bag keys.
- [X] FR-006 names exact catalog row IDs (C37/C38/C39).
- [X] SC-002 names the macOS CI lane anchor + Apple Team ID
      `EQHXZ8M8AV` (with the regex fallback in FR-009).
- [X] SC-004 quantifies the LOC ceiling (700 for macho.rs).
- [X] SC-008 (no new crate deps) is verifiable via `cargo tree`.

## Internal consistency

- [X] FR-001-005 (parsers + BinaryScan + scan.rs + entry.rs bag
      population) flow end-to-end.
- [X] FR-006 + FR-007 (catalog + parity) align with the
      holistic_parity regression gate.
- [X] Edge Case "CD version < 0x20200 doesn't carry teamOffset"
      aligns with FR-001's
      `parse_codesign_team_id` returns None for old CDs +
      Scenario 2 (ad-hoc has no team-id).
- [X] Edge Case "fat-binary first-slice" aligns with FR-004 +
      Scenario 5.
- [X] Determinism contract (sorted flags, skip-on-empty) aligns
      with the existing milestone 024 emission shape.

## Lessons from milestones 016-029

- [X] FR-011 carries the per-commit-clean discipline.
- [X] **6th amortization-proof consumer** — five identity-cohort
      milestones (023, 024, 025, 028, 029) and now this. Bag
      pattern conclusively validated.
- [X] Recon-first: every claim in the spec backed by a file:line
      reference from the pre-spec investigation.
- [X] R4 in plan.md (macOS CI lane SC-002 brittleness) anticipates
      the same kind of "real-world-data-may-shift" pattern that
      matters for cross-version compatibility.
- [X] Scope-deferral discipline: entitlements XML + CMS PKCS#7
      + designated requirements + notarization explicitly listed
      in Out of Scope with reasons. Same pattern as milestones
      024 (codesign deferred), 028 (Authenticode deferred), 026
      (hard cohort deferred).

## Pre-implementation

- [X] [PHASE-1] T001 reconnaissance done (2026-04-28).
- [ ] [PHASE-1] T002 baseline snapshot captured.
- [ ] [PHASE-2] Commit 1 (parsers + helpers + tests + dead_code)
      landed.
- [ ] [PHASE-3] Commit 2 (wire-up + macOS-lane test) landed.
- [ ] [PHASE-4] Commit 3 (parity rows) landed.
- [ ] [POLISH] SC-001-SC-008 verified.
- [ ] [POLISH] All 3 CI lanes green (macOS lane is SC-002 anchor).

## Post-merge

- [ ] [QUALITATIVE] Next time someone scans a macOS binary,
      mikebom answers "who signed this and how" via the
      CodeDirectory identifier + team ID + flags annotations.
      If yes, milestone delivered.
- [ ] [BAG STREAK] 6 consecutive amortization-proof bag
      consumers (023 → 024 → 025 → 028 → 029 → 030) — design
      pattern conclusively validated across 6 milestones.
