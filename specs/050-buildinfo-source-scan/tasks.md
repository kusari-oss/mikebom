---
description: "Task list — milestone 050 BuildInfo-vs-go.sum scope hint"
---

# Tasks: BuildInfo-vs-go.sum scope hint for source-tree Go scans

**Input**: spec.md ✅, plan.md ✅. (No checklists/, research.md,
data-model.md, contracts/, quickstart.md — same 4-file tighter
template milestones 047/048/049 used.)

**Tests**: integration test in `mikebom-cli/tests/scan_go.rs`
asserting hint fires/doesn't-fire based on binary presence.

**Organization**: One user story (US1 — diagnostic) plus US2
(docs). US2 is co-bundled into the same commit since it's
~10 lines and shares the milestone narrative.

## Format: `[ID] [P?] [Story?] Description`

---

## Phase 1: Setup

- [X] T001 Branch `050-buildinfo-source-scan` created (via /speckit.specify auto-allocation).
- [X] T002 Spec.md + plan.md authored, recon complete (R1: G3 already correct; R2: detection signals identified; R3: hint message style; R4: README target).

---

## Phase 2: Foundational

(No foundational tasks. Every change in this milestone lives in
`mikebom-cli/src/scan_fs/package_db/mod.rs` post-G5, plus
README.md / CHANGELOG.md edits, plus one new test in
`tests/scan_go.rs`. The shared infrastructure — G3 filter,
go_binary::read, GoScanSignals, ScanMode threading — is
already in place.)

---

## Phase 3: Commit `feat(050)` — scope hint + docs

**Goal**: Emit a `tracing::info` hint when `mikebom sbom scan
--path <go-project>` finds `go.mod` but no built Go binary.
Document the BuildInfo intersection workflow in README.

**Independent test**: SC-001 (hint fires when binary absent),
SC-002 (no hint when binary present), SC-003 (goldens unchanged),
SC-004 (scan_go integration tests pass), SC-005
(holistic_parity 11/11), SC-006 (new integration test passes),
SC-007 (pre-pr clean), SC-008 (CI green).

### Hint emission

- [ ] T003 [US1] Read `mikebom-cli/src/scan_fs/package_db/mod.rs`
      around the existing G3/G4/G5 chain (lines ~622-706). Identify
      the exact line BEFORE `out.extend(go_binary_entries)`
      (currently line 629) where `go_binary_entries.len()` can be
      captured into a local variable.
- [ ] T004 [US1] Verify `ScanMode` value at `read_all` invocation:
      `--path` flow vs `--image` flow. Read
      `mikebom-cli/src/cli/scan_cmd.rs` to find where `ScanMode::Path`
      vs `ScanMode::Image` is set. Confirm the value at `read_all`
      reflects the original CLI flag, not the post-extraction
      rootfs type.
- [ ] T005 [US1] Add the hint-emission block in `mod.rs` after the
      G5 main-module filter completes. Conditions: at least one Go
      module parsed (`!go_signals.main_modules.is_empty()`), zero
      Go-binary entries (`go_binary_entries_count == 0`), source-tree
      scan mode (`scan_mode == ScanMode::Path`). The hint computes
      `source_tier_count` via a fold on `out` (Go ecosystem +
      `sbom_tier == "source"`), emits a single
      `tracing::info` line with `go_modules`, `go_sum_components`
      structured fields, and a human-readable message naming
      `go build` + the BuildInfo intersection.

### Test

- [ ] T006 [US1] Add an integration test `scan_go_source_only_emits_buildinfo_scope_hint` in `mikebom-cli/tests/scan_go.rs`. Synthetic rootfs with `go.mod` + `go.sum` + `main.go` (no binary). Run mikebom via the test's `Command` helper, capture stderr, assert it contains a substring like `"no Go binary found alongside go.mod"` (or whatever final wording lands in T005). Counterpart: existing `scan_go_source_plus_binary_*` tests stay green AND don't emit the hint (assert on stderr absence, optional).

### Docs

- [ ] T007 [US2] Read `README.md` to find the Go ecosystem section. Append a paragraph naming the BuildInfo intersection workflow with the concrete `apigatewayv2/config` numbers (63 from `go.sum` vs ~41 from BuildInfo). Add the one-liner: `go build && mikebom sbom scan --path .`.
- [ ] T008 [US2] CHANGELOG.md `[Unreleased]` → `### Added` entry: name the new diagnostic, note no behavior change, no new flag, no goldens churn.

### Verify

- [ ] T009 [US1] [US2] Build debug mikebom: `cargo +stable build -p mikebom`. Real-world smoke: `mikebom sbom scan --path ~/Projects/iac/app-code/apigatewayv2/config` (binary absent) → confirm hint fires (visible in stderr trace). Then `cd ~/Projects/iac/app-code/apigatewayv2/config && go build .` if not already done; copy `apigatewayv2-config` binary into dir; re-scan; confirm hint absent and SBOM ≤ 41 components.
- [ ] T010 [US1] [US2] `cargo +stable test -p mikebom --test scan_go` — all 14 existing tests + new test pass.
- [ ] T011 [US1] [US2] `cargo +stable test -p mikebom --test holistic_parity` — 11/11 ok.
- [ ] T012 [US1] [US2] `cargo +stable test -p mikebom --test cdx_regression --test spdx_regression --test spdx3_regression -- --test-threads=1` — 27/27 byte-identity goldens unchanged (the diagnostic doesn't touch SBOM output).
- [ ] T013 [US1] [US2] `./scripts/pre-pr.sh` clean.

### Commit

- [ ] T014 [US1] [US2] Commit: `feat(050): BuildInfo-scope hint when source-tree Go scan finds no binary`. Include all touched files (mod.rs, scan_go.rs, README.md, CHANGELOG.md, specs/050 scaffolding) in one commit per the 4-file pattern.

---

## Phase 4: PR

- [ ] T015 [US1] [US2] Push branch: `git push -u origin 050-buildinfo-source-scan`.
- [ ] T016 [US1] [US2] Open PR titled `feat(050): BuildInfo-scope hint for source-tree Go scans`. Body covers: 1-commit summary, link to G3's existing implementation in `mod.rs:458`, the audit-grounded 63→41 numbers, the SC-001 through SC-008 verification commands.
- [ ] T017 [US1] [US2] Verify SC-008: 3 CI lanes green on the PR.
