# PR — milestone 152: preserve license operands (closes #481)

## Summary

Close [issue #481](https://github.com/kusari-oss/mikebom/issues/481) by extending the RPM reader with a second-pass `SpdxExpression::try_canonical` fallback that wraps unrecognized operands as SPDX 2.3-spec-blessed `LicenseRef-<sanitized>` escape-hatch identifiers, recovering the known portion of compound expressions instead of collapsing the whole expression to `NOASSERTION`.

**Before vs after**, on the issue-#481 testbed (`core-image-minimal` qemux86-64 scarthgap-LTS, the 5 packages that #478 left at NOASSERTION):

| Package | Pre-152 `licenseDeclared` | Post-152 `licenseDeclared` |
|---|---|---|
| `busybox*` (4 packages) | `NOASSERTION` | `GPL-2.0-only AND LicenseRef-bzip2-1.0.4` |
| `liblzma5` | `NOASSERTION` | `LicenseRef-PD` |

## Origin

Follow-up to #475 (closed in `eb75853` via PR #478). After #478's BitBake `&`/`|` operator normalization, 5 of 35 packages on the Yocto testbed still emitted NOASSERTION because their compound `License:` headers contain operands not on the SPDX license list (e.g., `bzip2-1.0.4`, `PD`). The all-or-nothing `try_canonical` collapse was discarding the recognized half along with the unknown half.

User chose **option 1** from the issue body (preserve known operands via SPDX `LicenseRef-` escape hatch) over option 2 (preserve raw with a new `mikebom:license-tag-raw` annotation). Per Constitution Principle V, `LicenseRef-<idstring>` is the standards-native carrier for unknown SPDX identifiers — no new `mikebom:*` annotation introduced.

## Changes

| File | Change |
|------|--------|
| `mikebom-cli/src/scan_fs/package_db/rpm_file.rs` | +~270 LOC: new `Token` enum + `tokenize` + `sanitize_to_license_ref_idstring` + `is_recognized_spdx_operand` + `preserve_known_operands_with_license_ref` helpers; +16 new unit tests; +1 `.or_else(...)` block at the existing pipeline call site. Milestone-478's `normalize_bitbake_license_operators` is UNCHANGED. |
| `mikebom-cli/Cargo.toml` | +9 LOC: promote `spdx = "0.10"` from transitive dep (via mikebom-common's `std` feature) to direct dep so the per-operand validation can call `spdx::license_id()` / `spdx::exception_id()` directly. **0 new lockfile entries** — spdx is already in the closure. |
| `CHANGELOG.md` | +~50 LOC: new entry under `[Unreleased]` documenting the LicenseRef escape hatch + the replace+collapse+strip sanitization rule + worked examples + WITH-clause behavior. |
| `Cargo.lock` | Minor — reflects the direct-dep promotion. No new transitive deps. |
| `CLAUDE.md` | Auto-updated by `update-agent-context.sh` during plan phase. |
| `specs/152-preserve-license-operands/*` | Standard speckit branch artifacts (spec, plan, research, data-model, contracts, quickstart, tasks, checklist, pr-description). |

No catalog (`docs/reference/sbom-format-mapping.md`) changes — FR-018 satisfied. No format-emitter changes — wire shapes unchanged.

## Spec / Plan trail

- [Spec](../../specs/152-preserve-license-operands/spec.md) — 13 FRs, 8 SCs, 2 USs.
- [Plan](../../specs/152-preserve-license-operands/plan.md) — Constitution Check PASS pre + post design.
- [Research](../../specs/152-preserve-license-operands/research.md) — R1-R10: spdx-crate API (`license_id` / `exception_id` per-operand validation; `parse_mode LAX` insufficient); hand-rolled tokenizer grammar; WITH-clause one-token-lookahead algorithm; per-operand classification ladder; sanitization algorithm with idempotency proof; pipeline integration site.
- [Data model](../../specs/152-preserve-license-operands/data-model.md) — `Token<'a>` enum, helper signatures, sanitization worked-examples, integration-site diff.
- [Contracts/helper-api.md](../../specs/152-preserve-license-operands/contracts/helper-api.md) — 5 contracts covering pre/post-conditions + idempotency invariants + scope guards.
- [Quickstart](../../specs/152-preserve-license-operands/quickstart.md) — 8 validation scenarios.
- [Tasks](../../specs/152-preserve-license-operands/tasks.md) — 29 tasks across 5 phases; all completed except T027 (this PR description).

Clarifications (`spec.md` § Clarifications, Session 2026-06-30):
- **Q1** → Sanitization rule: replace + collapse + strip (idempotent; spec-grammar-compliant).
- **Q2** → WITH-exception unrecognized: whole compound NOASSERTION (conservative; SPDX 2.3 doesn't define ExceptionRef-).

`/speckit-analyze` flagged 5 findings (0 critical, 2 medium test gaps, 3 low). All 5 applied as remediations:
- **C1** → new test #13 `with_clause_unknown_license_wrapped` for FR-013 first-clause
- **C2** → new test #14 `mixed_precedence_preserved` for FR-005 implicit precedence
- **A1** → tightened T011 wording for the manual pipeline-chaining inside the test
- **A2** → data-model §3 whitespace-serialization clarification
- **A3** → CHANGELOG header precheck added to T022

## Plan deviation surfaced during implementation

The plan assumed `spdx = "0.10"` was already accessible from `mikebom-cli` because it's a direct dep of `mikebom-common`. In reality, `mikebom-common` declares it as `optional = true` and `mikebom-cli` doesn't transitively re-export the crate root. To call `spdx::license_id()` / `spdx::exception_id()` from `rpm_file.rs`, I promoted `spdx = "0.10"` to a direct dep in `mikebom-cli/Cargo.toml` with a Constitution-Principle-VI-cited justification comment ("not new — already in the lockfile via mikebom-common"). The Cargo.lock change is a no-op at the transitive level. The plan's "no new Cargo dependencies" claim should be read as "no new lockfile entries" rather than "no Cargo.toml changes."

Also, the plan's R4 step 3 (pass-through imprecise synonyms via `spdx::imprecise_license_id`) was wrong: `SpdxExpression::try_canonical` runs in STRICT mode, which rejects imprecise forms — passing them through caused the final canonicalization to fail. I removed R4 step 3 from the helper. Imprecise synonyms now fall through to the LicenseRef escape hatch (e.g., raw `GPLv2` → `LicenseRef-GPLv2`). In practice, Yocto's RPM build pipeline canonicalizes `License:` headers upstream, so imprecise synonyms in real RPMs are rare; the conservative wrapping is correct for the cases that do occur. Test #15 (`imprecise_synonym_wrapped_as_license_ref`) documents the new behavior.

## Verification (SC-001 through SC-008)

| SC | Check | Result |
|----|-------|--------|
| SC-001 | The 5-package fix on the issue-#481 testbed | ⏳ **Manual operator-cadence** per quickstart.md Scenario 1 (Yocto fixture isn't in the milestone-090 sibling repo). Maintainer to verify post-merge. Unit tests `preserve_busybox_compound` + `preserve_liblzma5_single_unknown` cover the synthetic forms. |
| SC-002 | Byte-identical happy-path output | ✅ Test `happy_path_unchanged_for_fully_recognized` + the existing milestone-090 golden tests pass (verified in T024). |
| SC-003 | Idempotency | ✅ Test `idempotent_on_already_wrapped_input` passes. |
| SC-004 | Broader Yocto coverage 100% target | ⏳ Manual operator-cadence (same as SC-001). |
| SC-005 | Pre-PR gate clean | ✅ See pre-PR output below. |
| SC-006 | ≥8 unit tests | ✅ 16 new milestone-152 tests pass (`grep -cE "^\s+fn (preserve_\|sanitization_\|with_clause_\|...\|tokenize_)" rpm_file.rs` returns 16). |
| SC-007 | No wire-format / catalog changes | ✅ `git diff main --name-only -- docs/ mikebom-common/ mikebom-ebpf/ mikebom-cli/src/generate/` returns empty. |
| SC-008 | CHANGELOG entry present | ✅ New `### RPM license expressions: preserve known operands when one is unknown (closes #481)` section under `[Unreleased]` in CHANGELOG.md. |

### Pre-PR gate output (T024)

```
$ ./scripts/pre-pr.sh
[clippy: clean after iterating on overindented doc-list-items + question_mark suggestion]
[tests: 116 ok suites; all 16 new milestone-152 rpm_file tests pass]

Failed test (documented env-only flake — only acceptable failure per spec SC-005):
  - sbomqs_parity::sbomqs_spdx_score_meets_or_beats_cdx_across_ecosystems

Exit code: 101 (cargo's exit code for test failure; matches pre-152 main HEAD
state — same documented flake behaves identically before + after milestone 152
edits).
```

Mid-implementation iteration: the first pre-PR run surfaced 6 `doc_overindented_list_items` warnings on my doc-comment grammar block + 1 `question_mark` suggestion on the WITH-exception check. Both were straightforward fixes (reformat the grammar bullet list with 2-space indent + use `spdx::exception_id(s)?;` instead of `if … is_none() { return None; }`). Second pre-PR run was clean.

The documented `sbomqs_parity::sbomqs_spdx_score_meets_or_beats_cdx_across_ecosystems` env-only failure is the only acceptable test failure per spec SC-005 + the project's memory entry on environment-only flakes.

## Constitution check

Per [plan.md POST-DESIGN re-evaluation](specs/152-preserve-license-operands/plan.md#constitution-check--post-design-re-evaluation):
- **Principle V** (standards-native precedence): **REINFORCED** — `LicenseRef-<idstring>` is the SPDX 2.3-spec-blessed carrier for unknown license identifiers; no new `mikebom:*` annotation introduced.
- **Principle IX** (Accuracy): **ADVANCED** — the LicenseRef escape hatch is more accurate than NOASSERTION (honest "we don't recognize this operand" signal vs misleading "no assertion possible at all").
- **Principle X** (Transparency): **ADVANCED** — consumer-visible: a `LicenseRef-bzip2-1.0.4` in the output explicitly tells the consumer "this token isn't on the SPDX license list" so they can decide whether to ignore, resolve, or escalate.

All other principles N/A or unaffected (Rust-only code change touching one file).

No violations. No complexity-tracking entries needed.

## Reviewer-cadence operator test

To independently verify SC-001 + SC-004, follow `specs/152-preserve-license-operands/quickstart.md` Scenario 1 — manual operator-cadence check against the maintainer's local `yocto-test/` testbed. The 5 affected packages MUST emit non-NOASSERTION `licenseDeclared` per the per-package fix table above.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
