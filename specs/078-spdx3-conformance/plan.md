# Implementation Plan: SPDX 3.0.1 conformance pass

**Branch**: `078-spdx3-conformance` | **Date**: 2026-05-06 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/078-spdx3-conformance/spec.md`

## Summary

Establish operational ground truth for "100% SPDX 3.0.1 conformant" by integrating the JPEWdev `spdx3-validate` Python tool as the authoritative conformance validator, fixing every issue it surfaces against mikebom's existing SPDX 3 fixtures, and adding the validator as a CI gate to prevent regression. The minimum-viable hotfix (the user-reported `createdBy` type-mismatch — `Tool` IRI in a slot that requires `Agent`) ships in the same PR; per the 2026-05-06 clarification, the fix replaces the `createdBy` reference with an `Organization` element (`name: "mikebom contributors"`, matching the existing CDX `metadata.tools[0].publisher` value) and routes the existing `Tool` to a new `createdUsing` field.

The exact list of additional fixes is **research-time work**: visual inspection surfaces several suspicious patterns (`dataLicense` license-expression shape, `externalIdentifierType` controlled-vocabulary values, `software_*` property naming convention, `_:creation-info` blank-node identifier), but only running the validator establishes which patterns actually violate the spec versus which are spec-conformant alternatives mikebom already chose. Phase 0 runs the validator and enumerates the real fix list before Phase 1 design starts.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–077; no nightly). **Plus Python 3.10+** in CI/test for the JPEWdev `spdx3-validate` tool — Python is NEW for this milestone but only at the CI/test layer; the mikebom production binary stays pure Rust per Constitution Principle I.
**Primary Dependencies**: Existing only on the Rust side — `serde`/`serde_json` (existing JSON-LD round-tripping), `tracing`, `anyhow`, plus a small `Command::new("spdx3-validate")` shell-out helper for the integration tests. Dev/CI side adds the **JPEWdev `spdx3-validate`** Python package, version-pinned per FR-008. **No additions to the Rust `Cargo.toml` deps.**
**Storage**: N/A — purely a metadata transform on the SPDX 3 emission code path; no caches, no persistence.
**Testing**: `cargo +stable test --workspace` continues as the primary gate. New integration test `mikebom-cli/tests/spdx3_conformance.rs` shells out to `spdx3-validate` against every existing SPDX 3 golden fixture + at least 3 freshly-emitted fixtures (source/image/synthetic-build per SC-002). The test gracefully skips with a clear error message when the validator binary isn't installed (so local dev without Python doesn't break) but the CI lane installs it unconditionally and asserts presence + zero-error exit.
**Target Platform**: Linux (CI primary), macOS (developer workstations). The validator is pure Python — runs identically on both. The integration test that shells out to it works on either OS once `pip install spdx3-validate` succeeds.
**Project Type**: CLI tool — single workspace, three crates (`mikebom-cli` is the only one touched).
**Performance Goals**: Validator runs in <30s against the existing 9-fixture golden suite (per the JPEWdev tool's documented per-file complexity — it's a single-pass JSON-LD validator, fast enough for CI). Integration test wall-time <60s end-to-end including pip install if not cached.
**Constraints**: Determinism per FR-009 (validator at pinned version + pinned Python version → byte-identical output across re-runs). Backward compatibility per FR-007 (CDX 1.6 + SPDX 2.3 byte-identity goldens stay byte-identical; SPDX 3 goldens regenerate as the expected operator-visible change of this milestone). No regression on existing 073/074/075/076/077 byte-identity goldens for CDX or SPDX 2.3.
**Scale/Scope**: Bug-fix scope dominated by Phase 0 discovery work. ~50 LOC for the `Organization` element + `createdUsing` slot fix. Additional fixes per Phase 0 audit — could be 50 LOC (small fixes only) or 500 LOC (if `software_*` property naming requires wholesale convention change). Plus ~150 LOC integration test + ~30 LOC CI workflow update. SPDX 3 golden regen across 9 fixtures (likely substantive per-file diff; not symmetric like a version-string bump because conformance fixes can be structural).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Constitution v1.4.0 (last amended 2026-05-01). All 12 principles + 4 strict boundaries reviewed:

| Principle | Status | Justification |
|-----------|--------|---------------|
| I. Pure Rust, Zero C | ✅ Pass | mikebom binary stays pure Rust. Python is added only at the CI/test layer for output validation — analogous to existing `jq` invocations in `realistic-projects.yml` and other test-side tooling. The "build pipeline" produces the mikebom binary; this milestone's Python addition is in the test/CI lane, not in the binary's compilation path. No C, no FFI. |
| II. eBPF-Only Observation | ✅ Pass / N/A | This milestone touches SPDX 3 emission metadata; the eBPF trace is unchanged. |
| III. Fail Closed | ✅ Pass | Validator integration fails closed: when validator reports any conformance violation, the integration test fails AND the CI gate fails. No silent fallback to "looks good enough." |
| IV. Type-Driven Correctness | ✅ Pass | New `Organization` element is constructed via the existing `serde_json::Value` JSON-LD pipeline (not a new newtype layer — that's overkill for one new element type). Existing newtype contracts in `mikebom-cli/src/binding/identifiers/` and `generate/` are preserved. Production code uses `anyhow::Result`. Test code uses `#[cfg_attr(test, allow(clippy::unwrap_used))]` per the established convention. No new raw-`String` boundary crossings. |
| V. Specification Compliance | ✅ Pass | **This milestone IS the Principle V audit.** Establishes that mikebom's SPDX 3 output passes the JPEWdev validator (the operational definition of "conformant" per FR-002). Native-first audit confirmed: zero new `mikebom:*` annotations introduced; all changes are at standards-native field positions inside SPDX 3 emission (`Organization`, `Tool`, `createdBy`, `createdUsing`, plus whatever else Phase 0 surfaces — all per the SPDX 3 Core/Software model). |
| VI. Three-Crate Architecture | ✅ Pass | All Rust changes inside `mikebom-cli`. No new Rust crates. The Python validator is an external CI/test tool, not a workspace member. |
| VII. Test Isolation | ✅ Pass | Conformance tests run without elevated privileges. `spdx3-validate` is a userspace Python tool; no kernel privileges needed. The graceful-skip behavior when the validator binary is absent (local dev) preserves the unprivileged-CI invariant; CI installs the validator deterministically and asserts presence. |
| VIII. Completeness | ✅ Pass / N/A | Doesn't affect dependency discovery. |
| IX. Accuracy | ✅ Pass | Validator-driven fixes by definition improve accuracy of mikebom's SPDX 3 output relative to the spec. The user-reported `createdBy` bug is exactly an accuracy issue (mikebom claimed a `Tool` was an `Agent`). |
| X. Transparency | ✅ Pass | Conformance violations are surfaced with clear validator-output error messages in CI logs (FR-010). When mikebom disagrees with a validator-flagged issue (FR-008), the disagreement is documented with spec citation rather than silently ignored — preserving transparency. |
| XI. Enrichment | ✅ Pass / N/A | Not enrichment. |
| XII. External Data Source Enrichment | ✅ Pass / N/A | The validator is a CI/test tool, not an external data source for SBOM content. Validator failure during CI surfaces as a CI failure (degrades the gate, not the SBOM). |

| Strict Boundary | Status |
|-----------------|--------|
| 1. No lockfile-based dependency discovery | ✅ Pass |
| 2. No MITM proxy | ✅ Pass |
| 3. No C code | ✅ Pass — Python tooling, not C |
| 4. No `.unwrap()` in production | ✅ Pass — extending production code that already complies; tests use the standard `#[cfg_attr(test, allow(clippy::unwrap_used))]` guard |

**Gate result: PASS.** No violations; no Complexity Tracking entries needed. The Python addition is a test-lane integration, not a constitutionally-restricted change.

## Project Structure

### Documentation (this feature)

```text
specs/078-spdx3-conformance/
├── plan.md                         # This file
├── spec.md                         # /speckit.specify + /speckit.clarify output (corrected)
├── research.md                     # Phase 0 — validator-driven discovery + decisions
├── data-model.md                   # Phase 1 — Organization element, CreationInfo schema
├── quickstart.md                   # Phase 1 — operator-facing recipes
├── contracts/
│   └── spdx3-conformance.md        # Phase 1 — wire-format contract per SPDX 3 model
├── checklists/
│   └── requirements.md             # Already passing
└── tasks.md                        # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

The milestone touches the SPDX 3 emission file + adds one new integration test file + small CI workflow updates.

```text
mikebom-cli/
├── src/
│   └── generate/
│       └── spdx/
│           └── v3_document.rs              # MODIFY — bulk of milestone work:
│                                           # 1. Add `Organization` element with
│                                           #    name "mikebom contributors" and
│                                           #    deterministic spdxId IRI.
│                                           # 2. Replace `createdBy: [tool_iri]`
│                                           #    with `createdBy: [org_iri]`.
│                                           # 3. Add `createdUsing: [tool_iri]`
│                                           #    referencing the existing Tool.
│                                           # 4. Apply additional fixes per Phase 0
│                                           #    audit (likely candidates:
│                                           #    `dataLicense` shape;
│                                           #    `externalIdentifierType` enum
│                                           #    values; `software_*` property
│                                           #    naming convention; possibly
│                                           #    `@id` vs `spdxId` consistency).
└── tests/
    └── spdx3_conformance.rs                # NEW — integration test that shells
                                              # out to `spdx3-validate` against:
                                              # (a) all 9 existing golden fixtures,
                                              # (b) freshly emitted fixtures from
                                              #     ≥3 representative scan targets.
                                              # Gracefully skips when validator
                                              # binary is absent locally (logs a
                                              # clear "validator not installed,
                                              # skipping — see scripts/install-
                                              # spdx3-validate.sh"); CI lane
                                              # installs unconditionally and
                                              # treats absence as test failure.

mikebom-cli/tests/fixtures/golden/spdx-3/    # MODIFY — regenerate every fixture:
                                              # apk, cargo, deb, gem, golang,
                                              # maven, npm, pip, rpm.

scripts/
└── install-spdx3-validate.sh                # NEW — small shell helper to install
                                              # spdx3-validate at the pinned
                                              # version. Used by both CI and the
                                              # integration test's "validator
                                              # absent" diagnostic message.

.github/workflows/
└── ci.yml                                   # MODIFY — add a step in the linux-
                                              # x86_64 lint+test job that runs
                                              # `bash scripts/install-spdx3-
                                              # validate.sh` before `cargo test`.
                                              # macOS lane intentionally skips
                                              # the install (Python toolchain
                                              # variability on macOS runners is
                                              # well-documented; Linux is the
                                              # authoritative gate).

docs/reference/identifiers.md                # MODIFY (small) — note the SPDX 3
                                              # createdBy/createdUsing distinction
                                              # in the existing wire-mapping
                                              # table for the `subject:` and
                                              # related schemes (clarifies that
                                              # the Tool reference moved from
                                              # CreationInfo.createdBy to
                                              # CreationInfo.createdUsing).
```

**Structure Decision**: Single project. Extends `mikebom-cli` with no new modules; touches one production source file + one new integration test + one new shell helper + one CI workflow update. Smallest-possible-surface-change consistent with milestones 074/075/077.

## Phase 0 — Research questions

Six implementation-level decisions to pin in `research.md`. The most important is **#1**: actually run the validator against the existing fixtures and enumerate the real fix list.

1. **Run `spdx3-validate` against existing 9 golden fixtures** — establishes ground truth. Document every issue surfaced + the corresponding spec citation. The output of this step IS the fix list for the milestone. Without this, we'd be guessing at fixes; with it, we know exactly what changes Phase 1 designs need to specify.
2. **JPEWdev `spdx3-validate` install + version pinning strategy** — pip install vs pipx vs git+SHA. Recommend pip with a pinned version in `scripts/install-spdx3-validate.sh`. Decide the actual pin (latest stable as of 2026-05-06; prefer a tagged release if available). Document the bump policy.
3. **Validator output shape + machine-readable surface** — `spdx3-validate` produces stderr text by default. Decide how the integration test parses it (full-text match for "0 errors" vs structured JSON output if the tool supports `--format json`). Pin the parsing strategy.
4. **`Organization` element IRI scheme** — should the IRI be `<doc_iri>/agent/mikebom-contributors` (path-style, matches existing `<doc_iri>/tool/mikebom`) or a stable global IRI like `https://mikebom.kusari.dev/spdx3/agent/mikebom-contributors` (mirrors the publisher identity, not document-scoped)? Recommend path-style for now (matches existing Tool pattern); a future milestone can promote to stable global IRI if downstream tools want cross-document deduplication.
5. **Test-side graceful-skip behavior** — when the validator binary is absent locally, the test prints a clear diagnostic + skips (returns success). When CI runs, the install script runs first; if install fails the CI step fails BEFORE the test runs (so the test never sees an absent validator in CI). Pin the exact stderr message + the env-var hook (e.g., `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1` to fail-on-absent for CI explicitness).
6. **Determinism contract for the Organization element** — its `spdxId` IRI must be deterministic across re-runs. Either a hash-derived IRI (e.g., SHA-256 of "mikebom contributors" → BASE32 prefix, matching the existing pattern at `v3_document.rs:435`) or a stable static suffix (`mikebom-contributors-publisher`). Pin the choice + verify byte-identity goldens stay stable across re-runs.

## Phase 1 — Design & contracts

### data-model.md

One new entity (`Organization` element, with the SPDX 3 Core/Organization shape) + one new field on the existing CreationInfo emission (`createdUsing: [tool_iri]`). Per-fixture documentation of the shape change. Plus a section enumerating any additional conformance fixes Phase 0 surfaces.

### contracts/

One contract: `spdx3-conformance.md`. Documents:
- The SPDX 3 wire-format expectations per the JPEWdev validator's coverage (mapped back to the SPDX 3 Core + Software model)
- The exact emitted shape of `CreationInfo`, `Organization`, `Tool` post-fix
- Per-format wire-mapping for any other fields Phase 0 touches
- The CI gate's behavior contract (when does it fail; what do operators see in the error log)
- The graceful-skip behavior for local dev

### quickstart.md

Operator-facing recipes:
1. **Inspect the new SPDX 3 wire shape** — `jq` snippets showing `createdBy` (Organization) + `createdUsing` (Tool) post-fix.
2. **Run the validator locally** — `pip install spdx3-validate@<pinned-version>` + `spdx3-validate <fixture>`.
3. **Validate a freshly-emitted SBOM** — `mikebom sbom scan --path . --output out.spdx3.json` + `spdx3-validate out.spdx3.json`.
4. **Cross-check with the Java SPDX library** — for operators who hit the original error, demonstrate the post-fix output passes their previous failure case.
5. **Pre-PR gate behavior** — what to expect when the local dev environment doesn't have Python installed.

### Agent context update

Run `.specify/scripts/bash/update-agent-context.sh claude` after Phase 1 docs land.

## Phase 2 — Out of scope for this command

`/speckit.plan` ends here. `/speckit.tasks` consumes plan.md + spec.md + Phase 1 docs and emits `tasks.md`. Estimated task count: **~14-18** depending on Phase 0 audit findings (more issues surfaced → more fix tasks; same Phase 1 + Phase 6 task count). Smaller than 076 because no per-format work (only SPDX 3 touched); larger than 077 because validator integration + CI workflow update + multi-fixture regen add real work.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified.**

Not applicable — Constitution Check passes on all 12 principles + 4 strict boundaries with zero violations. The Python toolchain addition is a test-lane integration, treated identically to existing test-lane tooling (jq, etc.) and not constitutionally restricted.
