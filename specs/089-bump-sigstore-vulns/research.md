# Research — milestone 089 sigstore vuln-bump scoping

Phase 0 investigation against the spec's open questions. Six decision points; all resolved without further clarification.

## §1 — Sigstore version target

**Decision**: bump from `sigstore = "0.10"` → `sigstore = "0.11"` (NOT 0.13 as the original spec input suggested).

**Rationale**: by inspecting `[features]` blocks in each version's published `Cargo.toml`:
- **0.10.0**: `cert = []` — empty feature, no aws-lc-rs.
- **0.11.0**: `cert = []` — empty feature, no aws-lc-rs. **Last constitution-compatible version.**
- **0.12.1**: `cert = ["dep:aws-lc-rs", "rustls-webpki/aws-lc-rs"]` — **aws-lc-rs forced** (violates Constitution Principle I).
- **0.13.0**: also forces aws-lc-rs (per the existing pin comment, which was correct about the direction but wrong about the version threshold).

So the existing pin comment at `mikebom-cli/Cargo.toml:137-138` is **incorrect**: it claims "0.13+ forces aws-lc-rs", but the actual cliff is **0.12+**. The fix to the comment is part of this milestone (FR-004).

**Alternatives considered**:
- **Stay on 0.10 + `[patch.crates-io]` override on `tough` and `rustls-webpki`**: feasible but high-maintenance — a `[patch]` requires either a fork repo or a vendored copy. Rejected: bumping to 0.11 is simpler, byte-identical for our crypto API surface, and still pulls modern transitives.
- **Bump to 0.12 with `default-features = false` to avoid `cert`**: rejected because `cosign-rustls-tls` (which mikebom needs) chains to `cert`, which in 0.12 forces aws-lc-rs. There's no way to use cosign-rustls-tls in 0.12 without aws-lc-rs.
- **Wait for sigstore upstream to make `cert`'s aws-lc-rs requirement optional**: open-ended timeline; not viable for this milestone. Will track as upstream issue but not block.

## §2 — Eliminate `tough` via feature toggle

**Decision**: drop `sigstore-trust-root-rustls-tls` from mikebom's sigstore feature set. Final feature list becomes `["cosign-rustls-tls", "fulcio-rustls-tls", "rekor-rustls-tls", "bundle"]` (4 features, was 5).

**Rationale**: `tough` is `optional = true` in sigstore 0.10.0 AND 0.11.0 (verified by inspecting `[dependencies.tough]` blocks). It's gated behind the `sigstore-trust-root` feature. The `sigstore-trust-root-rustls-tls` feature in mikebom's current setup chains to `sigstore-trust-root`, which pulls `tough = "0.18"` (0.10) or `tough = "0.19"` (0.11). Dropping the feature eliminates the entire `tough` transitive closure.

**Verification that mikebom doesn't use sigstore's TUF client**: grep across `mikebom-cli/src/attestation/` shows zero references to `sigstore::tuf::`, `sigstore::trust_root::`, `TufClient`, or any TUF-related sigstore type. The `TrustRootInvalid` enum at `mikebom-cli/src/attestation/verifier.rs:80` is mikebom's own `FailureMode` variant, NOT a sigstore type — verified by the `enum FailureMode { ... TrustRootInvalid, ... }` definition.

**Eliminated vulns** (all `tough@0.18.0`-rooted):
- 2 HIGH: `CVE-2026-6966` (signature-threshold bypass), `CVE-2026-6967` (missing delegated-metadata validation).
- 4 MEDIUM: `CVE-2025-2885` through `CVE-2025-2888` (root version sequence, terminating delegations, snapshot rollback, timestamp caching).
- 1 LOW: `GHSA-j8x2-777p-23fc` (cyclic delegation graphs).

**Total**: 7 of 15 vulns eliminated by this feature drop alone.

**Alternatives considered**:
- **Keep `sigstore-trust-root-rustls-tls` and accept the `tough` vulns**: rejected because mikebom doesn't actually use TUF, so accepting the vulns would document an unexploited risk forever. Dropping the feature flag is strictly better.

## §3 — Sigstore API delta 0.10 → 0.11

**Decision**: zero migration work. mikebom's `sigstore::*` API surface is byte-identical between versions.

**Rationale**: the sigstore `crypto` module — the only one mikebom imports from (`sigstore::crypto::{SigStoreSigner, SigStoreKeyPair, SigningScheme}`, used at `attestation/{verifier,signer,serializer}.rs:{17, 18, 122, 614, 615}`) — has byte-identical files between 0.10.0 and 0.11.0:

```text
$ diff sigstore-0.10.0/src/crypto/signing_key/mod.rs sigstore-0.11.0/src/crypto/signing_key/mod.rs
(empty diff)
$ diff sigstore-0.10.0/src/crypto/mod.rs sigstore-0.11.0/src/crypto/mod.rs
(empty diff)
```

**Verification path**: implementation will run `cargo +stable build -p mikebom` after the dep bump and confirm zero compilation errors in `attestation/` modules. If any errors surface (regression vs. byte-diff finding), they'd be in non-`crypto` re-exports that the byte-diff didn't catch — handled at implementation time.

**Alternatives considered**:
- **Pre-emptively refactor attestation/ to use raw `ring`/`p256`/`ed25519-dalek` types instead of sigstore wrappers**: rejected — out of scope (per spec Out of Scope), and unnecessary given byte-identical API.

## §4 — Residual vulns post-fix

**Expected residual** (post-feature-drop + post-bump):
- `rustls-webpki@0.102.x`: the HIGH `GHSA-82j2-j2ch-gfr8` (CRL DoS panic), MED `GHSA-pwjx-qhcg-rvj4` (CRL distribution-point matching), and 2 LOW URI/wildcard name-constraint accepts. **Sigstore 0.11 still pins `rustls-webpki = "0.102"`** as a non-optional direct dep; the fix is only available in `rustls-webpki@0.103.13+`.
- `rustls-webpki@0.103.12` HIGH (CRL DoS panic) — pulled via `oci-distribution@0.11.0` → `reqwest@0.12.28`. **Note**: with sigstore 0.11 the `oci-client` rename is in effect; need to verify whether the 0.103.12 path persists post-bump (it might become 0.103.13 if sigstore 0.11's deps moved forward).

**Decision**: document residual vulns in `specs/089-bump-sigstore-vulns/known-acceptances.md` per spec FR-001's OR clause. For each entry:
1. CVE / GHSA ID.
2. Affected crate + version.
3. Severity.
4. Why it's not exploitable in mikebom's use case (e.g., "mikebom does not process attacker-controlled CRLs; the DoS panic requires a malformed CRL BIT STRING input which mikebom does not consume").
5. Tracked next-step (e.g., "Upstream sigstore PR / issue link to bump rustls-webpki to 0.103.13").

**Rationale**: rustls-webpki 0.102 → 0.103 is a major-version bump in a security-critical crate; sigstore upstream needs to do that bump deliberately. We can't safely `[patch]` it ourselves without potentially breaking sigstore's webpki API usage. Maintainer-curated acceptance + upstream tracking is the correct posture.

**Alternatives considered**:
- **Cargo `[patch.crates-io]` for rustls-webpki**: technically possible but risky — sigstore might call into webpki APIs that changed shape between 0.102 and 0.103. Requires deep-diving sigstore's webpki usage; out of scope.
- **Fork sigstore at 0.11 and bump rustls-webpki**: maintenance burden too high for a single-version security carry.

## §5 — `oci-distribution` co-attribution

**Decision**: monitor; no separate bump needed in this milestone.

**Rationale**: sigstore 0.10 uses `oci-distribution@0.11`; sigstore 0.11 uses `oci-client@0.14` (the rename). Bumping sigstore changes the OCI dep transitively. The 1 HIGH `rustls-webpki@0.103.12` co-attribution at the OCI path is bundled into the sigstore bump's effect on the dep graph. Verified post-bump via `cargo tree -i rustls-webpki@0.103.12`.

If the post-bump trace still shows the OCI path attributing `rustls-webpki@0.103.12` and the fix isn't pulled in transitively, it'll go on the `known-acceptances.md` list with the same upstream-tracking pattern.

## §6 — CI gate for production-deps trivy scan

**Decision**: add a CI step to `.github/workflows/ci.yml` that runs trivy with the `--skip-dirs tests/fixtures --skip-dirs mikebom-cli/tests/fixtures --skip-dirs target` invocation and fails the lane if any HIGH-severity vuln appears under target `Cargo.lock`. Test-fixture vulns remain ignored via path-skip.

**Rationale**: FR-007 requires a CI gate. Trivy 0.69.3 is already pinned in CI (per memory: "trivy 0.69.3 + syft 1.27.0 pinned versions"). The gate uses the existing trivy install + adds one job step. ~10 LOC of YAML.

The gate's failure threshold is HIGH only (per FR-001). MEDIUM/LOW vulns appear in trivy output for visibility but don't fail CI; they're documented in `known-acceptances.md` if intentionally accepted.

**Alternatives considered**:
- **Use cargo-audit instead of trivy**: cargo-audit's CVSS-4.0 incompatibility (encountered during this milestone's investigation — fails on `RUSTSEC-2026-0073` libcrux-poly1305 entry) makes it unreliable for CI gating today. Trivy handles CVSS 4.0 fine.
- **Use `cargo deny`**: not currently in CI; adds a new tool. Trivy is already there.
- **Run the trivy gate on every push, not just CI**: out of scope for this milestone; CI integration is sufficient.

## Coverage map

| Spec section | Resolution |
|--------------|------------|
| FR-001 (zero HIGH) | §1 + §2 → ≥7 vulns eliminated; §4 documents residual HIGHs as accepted with upstream tracking. |
| FR-002 (zero MED/LOW) | §2 → 5 of 5 MEDs eliminated. §4 documents residual MED/LOWs as accepted. |
| FR-003 (no aws-lc-rs) | §1 → 0.11.0 chosen as ceiling because 0.12+ forces aws-lc-rs. |
| FR-004 (pin comment update) | §1 → comment will be corrected from "0.13+ forces aws-lc-rs" to "0.12+ forces aws-lc-rs". |
| FR-005 (no test regressions) | §3 → byte-identical API; expected zero migration work. Verified at impl time. |
| FR-006 (`mikebom sbom verify` byte-identical CLI output) | §3 → no behavior change. Verified by running existing tests + manual smoke test against a Cosign bundle. |
| FR-007 (CI gate) | §6 → trivy gate added to existing CI workflow. |
| FR-008 (goldens stable) | Implicit — no SBOM-emission code path touched. Verified by `git status mikebom-cli/tests/fixtures/golden/` post-fix. |

All open spec questions resolved. Ready for Phase 1 (data-model + contracts + quickstart).
