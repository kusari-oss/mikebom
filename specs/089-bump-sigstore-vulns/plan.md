# Implementation Plan: Clear sigstore-bundle transitive vulnerabilities

**Branch**: `089-bump-sigstore-vulns` | **Date**: 2026-05-09 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/089-bump-sigstore-vulns/spec.md`

## Summary

Phase 0 research turned up a much smaller scope than the spec's worst-case. Two findings drive the plan:

1. **The aws-lc-rs requirement was introduced in sigstore 0.12, not 0.13** as the existing pin comment claims. Sigstore **0.11.0** keeps `cert = []` (empty feature, no aws-lc-rs). Bumping `sigstore@0.10 → 0.11` is constitution-compatible.
2. **The `tough` dep is `optional = true` in both 0.10 and 0.11**, gated entirely behind the `sigstore-trust-root` feature. mikebom doesn't use sigstore's TUF client (the `TrustRootInvalid` enum at `attestation/verifier.rs:80` is mikebom's own, not sigstore's). **Dropping `sigstore-trust-root-rustls-tls` from mikebom's feature set eliminates `tough` entirely** — kills 6 of the 15 vulns (4 MED + 1 HIGH-pair + 1 LOW).

The bump 0.10 → 0.11 also brings `openidconnect 3.5 → 4.0` which modernizes the reqwest/rustls/rustls-webpki transitive stack — likely clearing the `rustls-webpki@0.101.7` entries (3 LOWs + 1 HIGH co-attribution path).

API surface impact: **zero**. Diff of `sigstore/src/crypto/signing_key/mod.rs` and `sigstore/src/crypto/mod.rs` between 0.10.0 and 0.11.0 is empty bytes. The `SigStoreSigner`, `SigStoreKeyPair`, `SigningScheme` types mikebom uses (verified by grep across `attestation/{verifier,signer,serializer}.rs`) are byte-identical between versions.

Expected post-fix residual vulns: 1–4 entries in `rustls-webpki@0.102.x` from sigstore's own non-optional `rustls-webpki = "0.102"` direct dep. The fix is in `rustls-webpki@0.103.13`, which sigstore upstream has not yet bumped to. mikebom doesn't process attacker-controlled CRLs or unusual cert-chain name constraints, so these are materially unexploitable in mikebom's use case. They'll be documented in `known-acceptances.md` per FR-001's OR clause, with an upstream-tracking link to a sigstore GitHub issue.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–088; no nightly required).
**Primary Dependencies**: bumping `sigstore = "0.10"` → `sigstore = "0.11"` in `mikebom-cli/Cargo.toml:141`. **Dropping** `sigstore-trust-root-rustls-tls` from the feature list (eliminates the `tough` transitive). Existing features kept: `bundle`, `cosign-rustls-tls`, `fulcio-rustls-tls`. No new direct Cargo deps. The transitively-promoted `pem = "3"` and `x509-parser = "0.16"` direct deps may need bumps if sigstore 0.11's openidconnect 4.0 forces newer versions; verified during smoke-test.
**Storage**: N/A — pure dep-bump milestone. No state, no persistence.
**Testing**: existing test suite. The `mikebom-cli/src/attestation/*` tests + `mikebom-cli/tests/*attestation*.rs` integration tests form the regression net for FR-005 + FR-006.
**Target Platform**: All platforms supported by mikebom (Linux, macOS).
**Project Type**: Single project — Rust workspace dep-bump + minor Cargo.toml feature-set edit.
**Performance Goals**: Build wall-time stays within ±10% of pre-089 baseline (no new heavy crypto deps; just a minor sigstore version bump). Trivy production-deps scan stays ≤30 s (SC-005).
**Constraints**: Constitution Principle I (Pure Rust, Zero C) — non-negotiable. Constitution Principle V (no `mikebom:*` properties without audit) — N/A for this milestone. The CVE-acceptance list (if any) is internal documentation, not SBOM-emitted metadata.
**Scale/Scope**: ~5 LOC of `Cargo.toml` edits + ~0–10 LOC of attestation-module API migration (likely zero based on byte-identical mod files) + 1 new doc file (`known-acceptances.md`) + 1 CI-workflow addition for production-deps trivy gate (FR-007) + audit-comment update at `Cargo.toml:137-138` + spec-doc updates. ~50 LOC total diff target.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Justification |
|-----------|--------|---------------|
| I. Pure Rust, Zero C | ✅ PASS | sigstore 0.11.0 keeps `cert = []` (empty feature, no aws-lc-rs). Verified by inspecting `sigstore-0.11.0/Cargo.toml` `[features]` block. The 0.12+ aws-lc-rs requirement is the upper bound, NOT 0.13 as the existing pin comment incorrectly states (will be corrected per FR-004). |
| II. eBPF-Only Observation | ✅ PASS | Not applicable — sigstore is a verify-side library, no discovery code path. |
| III. Fail Closed | ✅ PASS | No change to scan-failure semantics. Existing `attestation/verifier.rs::FailureMode` enum (including `TrustRootInvalid`) preserved. |
| IV. Type-Driven Correctness | ✅ PASS | The newtype-bounded crypto types (`SigStoreSigner`, `SigStoreKeyPair`, `SigningScheme`) are byte-identical between sigstore 0.10 and 0.11. No `String`-typed regressions. No new `.unwrap()` in production code. |
| V. Specification Compliance | ✅ PASS | No SBOM-emission code path changes. CycloneDX/SPDX 2.3/SPDX 3 outputs unaffected. |
| V — Standards-native precedence | ✅ PASS | No new `mikebom:*` properties / annotations / relationships introduced. The CVE-acceptance list is internal documentation, not SBOM-emitted. |
| VI. Three-Crate Architecture | ✅ PASS | No new crates. mikebom-cli, mikebom-common, xtask remain the workspace members; mikebom-ebpf untouched. |
| VII. Test Isolation | ✅ PASS | All attestation tests run unprivileged. No new eBPF dependencies. |
| VIII. Completeness | ✅ PASS | Verify-side library bump; no impact on discovery completeness. |
| IX. Accuracy | ✅ PASS | No phantom-component risk. |
| X. Transparency | ✅ PASS | The CVE-acceptance list (if any) is documented per spec FR-001's OR clause, providing maintainer-facing transparency about residual risk. |
| XI. Enrichment | ✅ PASS | No enrichment-source changes. |
| XII. External Data Source Enrichment | ✅ PASS | No new external data sources. |

**Strict Boundaries**:
- ✅ No lockfile-based dependency discovery — unchanged.
- ✅ No MITM proxy — unchanged.
- ✅ No C code — REINFORCED. The original 0.10.x pin's intent (avoid aws-lc-rs) is preserved by stopping the bump at 0.11.0.
- ✅ No `.unwrap()` in production — unchanged.

**Pre-PR Verification (mandatory)**: standard `./scripts/pre-pr.sh` gate. Both `cargo +stable clippy --workspace --all-targets -- -D warnings` and `cargo +stable test --workspace` must report clean.

**Gate verdict**: ✅ all gates pass. No constitution amendments required.

## Project Structure

### Documentation (this feature)

```text
specs/089-bump-sigstore-vulns/
├── plan.md              # This file
├── research.md          # Phase 0 output (sigstore feature audit + tough optionality + API delta findings)
├── data-model.md        # Phase 1 output (entities + validation rules)
├── quickstart.md        # Phase 1 output (maintainer recipes: smoke-test, vuln-rescan, residual-acceptance flow)
├── contracts/
│   └── sigstore-feature-set.md  # The new sigstore version + feature set + the constitution-compatibility claim
├── known-acceptances.md         # Created if any vulns remain post-bump (likely 1–4 rustls-webpki@0.102 entries)
├── checklists/
│   └── requirements.md  # Spec-quality checklist (already complete)
└── tasks.md             # Phase 2 output (/speckit.tasks command — NOT created here)
```

### Source Code (repository root)

```text
mikebom-cli/
├── Cargo.toml                    # MODIFIED: line 137-141 — version bump + feature drop + comment correction
├── src/
│   └── attestation/
│       ├── verifier.rs           # POSSIBLY MODIFIED: API migration if any (likely zero changes)
│       ├── signer.rs             # POSSIBLY MODIFIED: same
│       └── serializer.rs         # POSSIBLY MODIFIED: same
└── tests/
    └── (existing attestation tests — unchanged)

Cargo.lock                        # MODIFIED: regenerated for new sigstore + dropped tough closure

specs/006-sbomit-suite/
└── research.md                    # MODIFIED: R1 audit row updated to reflect 0.11.0 + the new pin rationale

.github/workflows/
└── ci.yml                         # MODIFIED: add a production-deps trivy gate (FR-007) — runs on Linux lane only
```

**Structure Decision**: Existing single-project Rust workspace. This milestone touches:
1. `mikebom-cli/Cargo.toml` (sigstore version + feature set + comment).
2. `Cargo.lock` (auto-regenerated).
3. `mikebom-cli/src/attestation/*` if API migration is needed (expected: zero changes given byte-identical sigstore mod files).
4. `specs/006-sbomit-suite/research.md` R1 audit row (constitution-rationale doc update).
5. `.github/workflows/ci.yml` for the production-deps trivy gate.
6. `specs/089-bump-sigstore-vulns/known-acceptances.md` (if residual vulns remain).

PR diff target: ~50 LOC across 4–6 files (one of which is `Cargo.lock` autoregenerated). Deliberately small.

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

No constitution violations. Complexity tracking N/A.
