# Quickstart — milestone 089 maintainer recipes

Five maintainer-facing recipes for the dep bump, validating the API delta, regenerating the lockfile, building the residual-vuln acceptance list, and verifying the post-fix vuln scan.

## Recipe 1 — Reproduce the pre-fix vuln baseline

```bash
trivy --quiet fs \
  --scanners vuln \
  --skip-dirs tests/fixtures \
  --skip-dirs mikebom-cli/tests/fixtures \
  --skip-dirs target \
  --format json \
  --output /tmp/pre-089.json \
  .

# Severity histogram for the workspace Cargo.lock target only:
jq -r '
  [.Results[]? | select(.Target == "Cargo.lock") | .Vulnerabilities[]?.Severity]
  | group_by(.) | map({severity: .[0], count: length})
' /tmp/pre-089.json
```

Expected (alpha.26 baseline): 5 HIGH + 5 MEDIUM + 5 LOW = 15 vulns.

## Recipe 2 — Apply the dep bump

Edit `mikebom-cli/Cargo.toml:141`:

```diff
-sigstore = { version = "0.10", default-features = false, features = ["sigstore-trust-root-rustls-tls", "cosign-rustls-tls", "fulcio-rustls-tls", "rekor-rustls-tls", "bundle"] }
+sigstore = { version = "0.11", default-features = false, features = ["cosign-rustls-tls", "fulcio-rustls-tls", "rekor-rustls-tls", "bundle"] }
```

Edit the pin comment at lines 137-138 per `data-model.md` VR-089-012/013/014:

```diff
-# Feature 006 — SBOMit compliance: DSSE envelope signing + Fulcio keyless +
-# Rekor transparency log. Pin to 0.10.x because 0.13+ forces aws-lc-rs via
-# the `cert` feature, violating Constitution Principle I (pure Rust, zero C).
-# rustls-tls variants chosen to match existing reqwest rustls posture. See
-# specs/006-sbomit-suite/research.md R1 for the audit.
+# Feature 006 — SBOMit compliance: DSSE envelope signing + Fulcio keyless +
+# Rekor transparency log. Pin to 0.11.x because 0.12+ forces aws-lc-rs via
+# the `cert` feature, violating Constitution Principle I (pure Rust, zero C).
+# rustls-tls variants chosen to match existing reqwest rustls posture.
+# `sigstore-trust-root-*` features dropped in milestone 089 — mikebom doesn't
+# use sigstore's TUF client, and the feature dragged in the vulnerable
+# `tough` transitive (see specs/089-bump-sigstore-vulns/research.md §2).
+# See specs/006-sbomit-suite/research.md R1 for the original audit.
```

Then regenerate `Cargo.lock`:

```bash
cargo +stable update -p sigstore
```

Or trigger a full re-resolve:

```bash
rm Cargo.lock && cargo +stable build
```

## Recipe 3 — Smoke-test the build + tests

```bash
# 1. Compile cleanly:
cargo +stable build --workspace
# Expected: success. If it fails, the byte-identical-API assumption from research §3 was wrong; check the error message + plan an API migration patch.

# 2. Run attestation tests:
cargo +stable test -p mikebom attestation
# Expected: every existing attestation test passes.

# 3. Confirm tough is gone:
! grep -q '^name = "tough"' Cargo.lock && echo "✓ tough eliminated" || echo "✗ tough still present"

# 4. Confirm no aws-lc-rs:
! grep -qE '^name = "aws-lc-(rs|sys)"' Cargo.lock && echo "✓ no aws-lc" || echo "✗ aws-lc detected"

# 5. Confirm sigstore at 0.11:
grep -A1 '^name = "sigstore"' Cargo.lock | head -2
```

## Recipe 4 — Re-scan + build the residual-vuln acceptance list

```bash
trivy --quiet fs \
  --scanners vuln \
  --skip-dirs tests/fixtures \
  --skip-dirs mikebom-cli/tests/fixtures \
  --skip-dirs target \
  --format json \
  --output /tmp/post-089.json \
  .

# Workspace Cargo.lock vulns post-fix:
jq -r '
  [.Results[]? | select(.Target == "Cargo.lock") | .Vulnerabilities[]?
   | {pkg: .PkgName, ver: .InstalledVersion, sev: .Severity, id: .VulnerabilityID, fix: (.FixedVersion // "—"), title: .Title}]
  | sort_by(.sev) | .[]
  | "[\(.sev)] \(.pkg)@\(.ver) → fix=\(.fix)  \(.id)  \(.title)"
'
```

Expected post-fix (rough): 1–4 entries in `rustls-webpki@0.102.x`, attributable to sigstore's own `rustls-webpki = "0.102"` direct dep that sigstore upstream hasn't yet bumped to 0.103.13.

For each remaining entry, create / append to `specs/089-bump-sigstore-vulns/known-acceptances.md`:

```markdown
## GHSA-82j2-j2ch-gfr8 — rustls-webpki@0.102.x — HIGH

- **Affected crate**: `rustls-webpki@0.102.x` (sigstore 0.11's direct dep).
- **Vuln**: Denial of service via panic on malformed CRL BIT STRING.
- **mikebom exposure**: mikebom does not process attacker-controlled CRLs. The `mikebom sbom verify` flow uses Cosign keyless verification (Fulcio + Rekor), not CRL-based revocation. Sigstore's webpki usage is internal to TLS handshake validation against trusted roots. The DoS panic requires a malformed CRL BIT STRING input which mikebom does not consume.
- **Upstream tracking**: sigstore-rs/sigstore-rs#XXXX (file an issue if none exists; link here).
- **Re-review date**: 2026-11-09 (6 months from acceptance).
```

Repeat for each residual entry. Save the file.

## Recipe 5 — Add the production-deps trivy CI gate

Edit `.github/workflows/ci.yml`. Add a step to the existing Linux lane (the lane that already has trivy installed for milestone-083's audit):

```yaml
- name: Production-deps vuln gate (milestone 089)
  run: |
    trivy --quiet fs \
      --scanners vuln \
      --skip-dirs tests/fixtures \
      --skip-dirs mikebom-cli/tests/fixtures \
      --skip-dirs target \
      --severity HIGH \
      --format json \
      --output /tmp/prod-vulns.json \
      .
    # Filter to Cargo.lock + cross-check known-acceptances.md.
    UNACCEPTED=$(jq -r --slurpfile acc <(grep -oE 'GHSA-[a-z0-9-]+|CVE-[0-9]+-[0-9]+' specs/089-bump-sigstore-vulns/known-acceptances.md 2>/dev/null | jq -R . | jq -s .) '
      [.Results[]?
       | select(.Target == "Cargo.lock")
       | .Vulnerabilities[]?
       | select(.Severity == "HIGH")
       | select(.VulnerabilityID as $id | ($acc[0] // []) | index($id) | not)
       | .VulnerabilityID
      ] | join(",")
    ' /tmp/prod-vulns.json)
    if [ -n "$UNACCEPTED" ]; then
      echo "::error::Unaccepted HIGH advisories in production deps: $UNACCEPTED"
      exit 1
    fi
    echo "✓ no unaccepted HIGH production-deps vulns"
```

## Recipe 6 — Final pre-PR gate

```bash
./scripts/pre-pr.sh
```

Expected: zero clippy warnings, every test suite `0 failed`. Standard CLAUDE.md mandatory gate.

Plus the new bonus check (run manually for visibility before pushing the PR):

```bash
git status --short mikebom-cli/tests/fixtures/golden/
# Expected: empty output (no goldens regenerated).
```

## When in doubt

- **`cargo build` fails post-bump**: the byte-identical-API assumption from research §3 was wrong. Check the error message:
  - If a `use sigstore::*` import broke, look for the new module path in `sigstore-0.11.0/src/` and update.
  - If a method signature changed, check the sigstore CHANGELOG between 0.10.0 and 0.11.0 (link from crates.io).
- **`tough` still appears in `Cargo.lock`**: feature drop didn't take. Verify `mikebom-cli/Cargo.toml:141` has `default-features = false` AND lacks any `sigstore-trust-root*` feature. Run `cargo update -p sigstore` again.
- **`rustls-webpki@0.103.12` HIGH still present**: it's the OCI-distribution co-attribution path. Verify with `cargo tree -i rustls-webpki@0.103.12` and decide whether to bump `oci-distribution` separately or add to `known-acceptances.md`.
- **Goldens regenerate when running pre-PR gate**: scope creep. This is a transitive-dep milestone — if the cargo goldens regenerate, the dep-graph change has somehow leaked into SBOM output. Investigate (likely a `creator/tool` field that includes a bumped crate version).
