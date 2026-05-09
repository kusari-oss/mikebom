# Data Model — milestone 089 sigstore vuln-bump

This is a dep-bump milestone with no new domain types and no SBOM-emission impact. The "model" is the workspace dependency graph + the residual-vuln acceptance list.

## Entities

### Sigstore feature set (declared in `mikebom-cli/Cargo.toml:141`)

The set of cargo features mikebom enables on its `sigstore` direct dep. Constrains which sigstore optional deps end up in `Cargo.lock`.

**Pre-089 state** (5 features):
- `sigstore-trust-root-rustls-tls`
- `cosign-rustls-tls`
- `fulcio-rustls-tls`
- `rekor-rustls-tls`
- `bundle`

**Post-089 state** (4 features):
- `cosign-rustls-tls`
- `fulcio-rustls-tls`
- `rekor-rustls-tls`
- `bundle`

**Validation rules**:
- VR-089-001: post-089 `Cargo.toml` MUST drop `sigstore-trust-root-rustls-tls`.
- VR-089-002: post-089 `Cargo.toml` MUST keep all 4 of `cosign-rustls-tls`, `fulcio-rustls-tls`, `rekor-rustls-tls`, `bundle` (removing any of these would break `mikebom sbom verify` / `mikebom attestation sign|verify`).
- VR-089-003: post-089 `Cargo.toml` MUST NOT add any sigstore feature containing `cert` standalone, `*-native-tls`, or `default` — these would either (a) drag in C deps via aws-lc-rs in a future minor bump, or (b) reverse the rustls-only posture documented in milestone 006's R1 audit.

### Workspace `Cargo.lock`

Auto-regenerated when `Cargo.toml` changes. The post-089 graph MUST satisfy:

**Validation rules**:
- VR-089-004: post-089 `Cargo.lock` MUST NOT contain any entry for `tough` (any version). Verified by `! grep -q '^name = "tough"' Cargo.lock`.
- VR-089-005: post-089 `Cargo.lock` MUST NOT contain any entry for `aws-lc-rs` or `aws-lc-sys`. Verified by `! grep -qE '^name = "aws-lc-(rs|sys)"' Cargo.lock`.
- VR-089-006: post-089 `Cargo.lock` MUST contain `sigstore = "0.11.0"` (or any 0.11.x patch).
- VR-089-007: post-089 `Cargo.lock` MAY contain `rustls-webpki = "0.102.x"` entries — these are residual vulns documented in `known-acceptances.md`. The trivy CI gate fails ONLY on NEW HIGH advisories, not existing accepted ones.

### Sigstore API call sites (`mikebom-cli/src/attestation/`)

The set of mikebom source files that import from `sigstore::*`.

**Pre-089 inventory** (verified by grep):
- `verifier.rs:18` — `use sigstore::crypto::{...};`
- `verifier.rs:614, 615` — `use sigstore::crypto::signing_key::SigStoreSigner; use sigstore::crypto::SigningScheme;`
- `serializer.rs:122` — `let scheme = sigstore::crypto::SigningScheme::ECDSA_P256_SHA256_ASN1;`
- `signer.rs:17` — `use sigstore::crypto::signing_key::{SigStoreKeyPair, SigStoreSigner};`
- `signer.rs:18` — `use sigstore::crypto::SigningScheme;`

**Validation rules**:
- VR-089-008: each pre-089 import path MUST resolve identically post-089 (`use sigstore::crypto::{...}` → same module + same exported symbol). Verified by successful `cargo build`.
- VR-089-009: zero new `use sigstore::*` imports added by this milestone (Out-of-Scope: not refactoring).

### CVE-acceptance list (`specs/089-bump-sigstore-vulns/known-acceptances.md`)

Maintainer-curated list of advisories that remain flagged post-089 with documented justifications. Created if and only if residual vulns exist post-fix.

**Schema** (per entry):
- `id`: CVE ID or GHSA ID.
- `crate`: name + version of affected dep.
- `severity`: HIGH / MEDIUM / LOW.
- `mikebom_exposure`: prose describing why the vuln is unexploitable in mikebom's use case (e.g., "does not process attacker-controlled CRLs").
- `upstream_tracking`: link to the sigstore (or other upstream) issue/PR where the fix is being driven, with status.
- `re_review_date`: target date for re-evaluating the acceptance (e.g., 6 months out).

**Validation rules**:
- VR-089-010: every advisory listed under `target == "Cargo.lock"` in the post-089 trivy output MUST have a corresponding entry in `known-acceptances.md`, OR be a NEW HIGH not yet seen pre-089 (which would fail CI per FR-007).
- VR-089-011: each entry MUST cite a tracked-resolution path (upstream issue link). Acceptances with no resolution path are not allowed.

### Pin comment (`mikebom-cli/Cargo.toml:137-138`)

The maintainer-facing audit comment explaining why sigstore is pinned to a specific minor version.

**Pre-089 text**:
```text
# Feature 006 — SBOMit compliance: DSSE envelope signing + Fulcio keyless +
# Rekor transparency log. Pin to 0.10.x because 0.13+ forces aws-lc-rs via
# the `cert` feature, violating Constitution Principle I (pure Rust, zero C).
# rustls-tls variants chosen to match existing reqwest rustls posture. See
# specs/006-sbomit-suite/research.md R1 for the audit.
```

**Post-089 text** (corrects the version threshold + reflects the feature drop):
```text
# Feature 006 — SBOMit compliance: DSSE envelope signing + Fulcio keyless +
# Rekor transparency log. Pin to 0.11.x because 0.12+ forces aws-lc-rs via
# the `cert` feature, violating Constitution Principle I (pure Rust, zero C).
# rustls-tls variants chosen to match existing reqwest rustls posture.
# `sigstore-trust-root-*` features dropped in milestone 089 — mikebom doesn't
# use sigstore's TUF client, and the feature dragged in the vulnerable
# `tough` transitive (see specs/089-bump-sigstore-vulns/research.md §2).
# See specs/006-sbomit-suite/research.md R1 for the original audit.
```

**Validation rules**:
- VR-089-012: post-089 comment MUST cite "0.12+ forces aws-lc-rs" (the corrected version cliff per research §1).
- VR-089-013: post-089 comment MUST mention milestone 089 + the `tough` feature drop rationale.
- VR-089-014: post-089 comment MUST preserve the "rustls-tls variants chosen" sentence + the milestone-006 R1 audit reference (continuity with prior maintainer decision-making).
