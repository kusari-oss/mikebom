# Contract — milestone 089 sigstore feature set + CI vuln gate

The milestone's two contracts: (1) the new sigstore version + feature set + the constitution-compatibility claim, and (2) the production-deps trivy CI gate behavior.

## CLI surface

**No new operator-facing CLI flags.** This is a transitive-dep cleanup milestone. `mikebom sbom verify`, `mikebom attestation sign|verify` keep their existing flag sets and behaviors.

## Library surface (`mikebom-cli` crate)

**No new public Rust API.** No internal API changes either. The byte-identical `sigstore::crypto::*` API surface between 0.10.0 and 0.11.0 (verified by file diff) means the existing imports at `attestation/{verifier,signer,serializer}.rs` resolve identically post-bump.

## Sigstore version + feature set contract

`mikebom-cli/Cargo.toml:141` MUST be:

```toml
sigstore = { version = "0.11", default-features = false, features = ["cosign-rustls-tls", "fulcio-rustls-tls", "rekor-rustls-tls", "bundle"] }
```

NOT:
- `version = "0.12"` or higher (0.12+ forces aws-lc-rs).
- Any feature list that includes `sigstore-trust-root-*`, `cert`, `*-native-tls`, or omits `default-features = false`.

The pin comment at `mikebom-cli/Cargo.toml:137-138` MUST be updated per data-model.md VR-089-012 through VR-089-014.

This contract is enforced by VR-089-001 + VR-089-002 + VR-089-003 + VR-089-006 + VR-089-012 + VR-089-013 + VR-089-014.

## Production-deps trivy CI gate contract

`.github/workflows/ci.yml` MUST contain a job step (or new job) that runs:

```bash
trivy --quiet fs \
  --scanners vuln \
  --skip-dirs tests/fixtures \
  --skip-dirs mikebom-cli/tests/fixtures \
  --skip-dirs target \
  --severity HIGH \
  --exit-code 1 \
  --format json \
  --output /tmp/prod-vulns.json \
  .
```

And then post-process the JSON to:
1. Filter to `target == "Cargo.lock"` entries only.
2. Filter to `Severity == "HIGH"`.
3. Cross-check each remaining entry against `specs/089-bump-sigstore-vulns/known-acceptances.md` IDs.
4. Fail the lane (exit non-zero) if any HIGH entry is NOT on the acceptance list.

This contract is enforced by FR-007 + VR-089-010 + VR-089-011.

**Test fixtures stay excluded via path-skip**, NOT via allow-listing. Operators investigating fixture vulns can run trivy without `--skip-dirs` for visibility, but CI gates only on production deps.

## CVE-acceptance list contract

`specs/089-bump-sigstore-vulns/known-acceptances.md` (created if and only if residual vulns remain post-fix) MUST follow the schema documented in data-model.md "CVE-acceptance list" entity. Each accepted advisory:

- Cites a CVE/GHSA ID.
- Names the affected crate + version.
- Records severity.
- Explains why the vuln is unexploitable in mikebom's use case (REQUIRED — vague justifications like "low impact" don't count).
- Links to an upstream issue or PR where the fix is being driven (REQUIRED — acceptances without a tracked path aren't allowed).
- Sets a re-review date (typically 6 months out).

This contract is enforced by VR-089-010 + VR-089-011.

## Per-format scope contract

| Format | Affected? | Verification |
|---|---|---|
| **CDX 1.6** (all 9 ecosystems) | NO — sigstore is verify-side, not emit-side | All 9 cdx goldens byte-identical |
| **SPDX 2.3** (all 9 ecosystems) | NO — same | All 9 spdx goldens byte-identical |
| **SPDX 3** (all 9 ecosystems) | NO — same | All 9 spdx3 goldens byte-identical |

This is a transitive-dep cleanup milestone. ZERO golden regenerations. If goldens regenerate, scope has crept and the PR must be narrowed.

## Test invocation contract

```bash
# Confirm sigstore bump compiles cleanly:
cargo +stable build --workspace
# Expected: success.

# Confirm attestation tests pass with byte-identical assertions:
cargo +stable test -p mikebom attestation
# Expected: every existing attestation test passes; zero deletions; zero assertion weakenings.

# Confirm goldens stay byte-identical:
git status --short mikebom-cli/tests/fixtures/golden/
# Expected: empty output.

# Confirm tough is gone from the dep graph:
! grep -q '^name = "tough"' Cargo.lock
# Expected: exit 0 (tough absent).

# Confirm no aws-lc:
! grep -qE '^name = "aws-lc-(rs|sys)"' Cargo.lock
# Expected: exit 0.

# Confirm sigstore is at 0.11:
grep -A1 '^name = "sigstore"' Cargo.lock | head -2
# Expected: version = "0.11.x" (any patch).

# Run production-deps trivy scan:
trivy --quiet fs --scanners vuln \
  --skip-dirs tests/fixtures \
  --skip-dirs mikebom-cli/tests/fixtures \
  --skip-dirs target \
  --format json --output /tmp/post-089.json .
# Expected: zero HIGH advisories under target=Cargo.lock NOT on known-acceptances.md.

# Standard pre-PR gate:
./scripts/pre-pr.sh
# Expected: zero clippy warnings, every test suite reports `0 failed`.
```

## Performance contract

- Build wall-time: within ±10% of pre-089 baseline. The dep graph SHRINKS post-089 (fewer transitive crates due to `tough` drop), so cold builds may speed up slightly.
- Test wall-time: identical (no test changes).
- Trivy production-deps scan: ≤30 s on standard developer hardware (matches pre-089 wall-time observed during the initial vuln audit).

## Backward-compatibility contract

- Operators of `mikebom sbom verify` against Cosign-signed bundles see zero behavior change. Pre-089 verify-pass bundles still verify post-089; pre-089 verify-fail bundles still fail post-089 with the same `FailureMode` exit codes.
- Operators of `mikebom attestation sign|verify` see zero behavior change. The `SigStoreSigner`, `SigStoreKeyPair`, `SigningScheme` types are byte-identical between sigstore 0.10.0 and 0.11.0 — signing produces identical envelopes for identical inputs (modulo intentional non-determinism like nonces).
- Operators using TUF-rooted Cosign trust-root verification flows are NOT supported pre-089 OR post-089 (mikebom never exercised that code path; the feature drop is removing dead transitive weight, not removing functionality).
- The trivy CI gate is a NEW gate, not a modification of existing CI behavior. PRs that introduce HIGH vulns in production deps will fail CI; PRs that don't are unaffected.
