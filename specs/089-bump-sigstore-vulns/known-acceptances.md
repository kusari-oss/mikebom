# Known CVE Acceptances — milestone 089

This file documents transitive vulnerabilities that remain in mikebom's production dep closure post-milestone-089 with maintainer-curated justifications. Per spec FR-001 / FR-002's OR clause, each entry represents a deliberately-unfixed advisory with a tracked-resolution path.

The CI gate at `.github/workflows/ci.yml` (added in milestone 089) reads this file at lane time: any HIGH-severity advisory under target `Cargo.lock` not listed here fails CI.

**Last reviewed**: 2026-05-09
**Re-review cadence**: every 6 months OR when sigstore upstream cuts a new minor version.

## Common context

All four entries below have the same root cause: **sigstore 0.11.0's `Cargo.toml` declares `rustls-webpki = "0.102"` as a non-optional direct dep**. The semver-compatible `0.102.x` patch line has no security-fix release for any of these advisories — fixes are only available in `0.103.x` (a major-version bump for the rustls-webpki crate). Sigstore upstream needs to bump its own `rustls-webpki` requirement, which is tracked at:

- **Upstream tracking**: file an issue at <https://github.com/sigstore/sigstore-rs/issues> requesting a bump to `rustls-webpki = "0.103"`. (Filed: TODO post-merge of milestone 089's PR.)

mikebom cannot safely `[patch.crates-io]` rustls-webpki@0.102 → 0.103 because 0.103 is a major-version-incompatible API change for rustls-webpki, and sigstore's webpki call sites would need migration alongside.

---

## GHSA-82j2-j2ch-gfr8 — rustls-webpki: Denial of service via panic on malformed CRL BIT STRING

- **id**: GHSA-82j2-j2ch-gfr8
- **crate**: `rustls-webpki@0.102.8`
- **severity**: HIGH
- **mikebom_exposure**: mikebom does not consume attacker-controlled Certificate Revocation Lists (CRLs). The `mikebom sbom verify` flow uses Cosign keyless verification (Fulcio + Rekor), not CRL-based revocation. Sigstore's webpki usage is internal to TLS handshake validation against trusted public roots. The DoS panic requires a malformed CRL `BIT STRING` input, which mikebom does not parse from any external source. **Materially unexploitable** in mikebom's threat model.
- **upstream_tracking**: <https://github.com/sigstore/sigstore-rs/issues> (sigstore-rs needs to bump `rustls-webpki` from 0.102 → 0.103, picking up the fix in 0.103.13).
- **re_review_date**: 2026-11-09

## GHSA-pwjx-qhcg-rvj4 — webpki: CRLs not considered authoritative by Distribution Point due to faulty matching logic

- **id**: GHSA-pwjx-qhcg-rvj4
- **crate**: `rustls-webpki@0.102.8`
- **severity**: MEDIUM
- **mikebom_exposure**: Same root cause as GHSA-82j2-j2ch-gfr8. mikebom does not configure or consume CRL Distribution Points; the `mikebom sbom verify` flow does not perform CRL-based revocation checks. The advisory describes a logic error in CRL Distribution Point matching that affects relying parties who DO use CRL-based revocation, which mikebom does not. **Materially unexploitable** in mikebom's threat model.
- **upstream_tracking**: <https://github.com/sigstore/sigstore-rs/issues> (same bump path as GHSA-82j2-j2ch-gfr8).
- **re_review_date**: 2026-11-09

## GHSA-965h-392x-2mh5 — webpki: Name constraints for URI names were incorrectly accepted

- **id**: GHSA-965h-392x-2mh5
- **crate**: `rustls-webpki@0.102.8`
- **severity**: LOW
- **mikebom_exposure**: The advisory affects relying parties who validate X.509 cert chains containing `nameConstraints` extensions with URI-typed `GeneralName` constraints. mikebom's verify path uses Fulcio's standard cert chain (no URI-typed name constraints in the deployed Fulcio root) and operates against Sigstore's public good instance. The vulnerability requires a crafted certificate chain with URI name constraints to be presented during validation, which mikebom does not encounter in normal operation. **Theoretical attack only**; not exploitable in mikebom's threat model.
- **upstream_tracking**: <https://github.com/sigstore/sigstore-rs/issues> (same bump path).
- **re_review_date**: 2026-11-09

## GHSA-xgp8-3hg3-c2mh — webpki: Name constraints were accepted for certificates asserting a wildcard name

- **id**: GHSA-xgp8-3hg3-c2mh
- **crate**: `rustls-webpki@0.102.8`
- **severity**: LOW
- **mikebom_exposure**: Same root cause as GHSA-965h-392x-2mh5 — affects relying parties validating X.509 cert chains with name constraints + wildcard SANs. mikebom's verify path operates against Fulcio's standard cert chain, not arbitrary cert chains with custom name-constraint configurations. **Theoretical attack only**; not exploitable in mikebom's threat model.
- **upstream_tracking**: <https://github.com/sigstore/sigstore-rs/issues> (same bump path).
- **re_review_date**: 2026-11-09

---

## Closed by milestone 089 (no longer in residual list)

For audit-trail purposes, these advisories were eliminated by milestone 089's combined `sigstore 0.10 → 0.11` bump + `sigstore-trust-root-rustls-tls` feature drop + opportunistic `rustls-webpki@0.103.12 → 0.103.13` patch update:

- ✅ `tough@0.18.0` — CVE-2026-6966 (HIGH), CVE-2026-6967 (HIGH), CVE-2025-2885 (MED), CVE-2025-2886 (MED), CVE-2025-2887 (MED), CVE-2025-2888 (MED), GHSA-j8x2-777p-23fc (LOW). Eliminated entirely by dropping the `sigstore-trust-root-rustls-tls` feature; mikebom never used sigstore's TUF client.
- ✅ `rustls-webpki@0.101.7` — GHSA-82j2-j2ch-gfr8 (HIGH), GHSA-965h-392x-2mh5 (LOW), GHSA-xgp8-3hg3-c2mh (LOW). Eliminated by the openidconnect 3.5 → 4.0 modernization that came with the sigstore 0.11 bump (replaces the old reqwest 0.11 → rustls 0.21 → rustls-webpki 0.101.7 transitive stack with reqwest 0.12 → rustls 0.23 → rustls-webpki 0.103).
- ✅ `rustls-webpki@0.103.12` — GHSA-82j2-j2ch-gfr8 (HIGH). Eliminated by `cargo update -p rustls-webpki@0.103.12 → 0.103.13` patch bump (no semver-incompat change required; just took the fix from the active 0.103 line).

**Total**: 11 of the 15 pre-089 advisories eliminated. 4 residual (1 HIGH, 1 MED, 2 LOW) all rooted at sigstore's own `rustls-webpki = "0.102"` dep, awaiting the upstream bump.
