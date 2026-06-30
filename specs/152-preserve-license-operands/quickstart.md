# Quickstart — milestone 152

Validation walkthrough for the RPM LicenseRef-fallback fix. Mirrors the milestone-478 (#475 close) operator-cadence verification pattern — the SC-001 testbed is local to the maintainer's machine (Yocto isn't in the milestone-090 sibling-fixture repo), so the full end-to-end check is manual; the unit-level coverage is automated.

## Scenario 1 — SC-001 issue-#481 testbed verification (MANUAL operator-cadence)

After the milestone-152 PR merges, the maintainer (or a Yocto-equipped reviewer) runs:

```bash
# 1. Build mikebom at milestone-152 HEAD:
cargo +stable build --release -p mikebom

# 2. Rebuild the issue-#481 testbed (or reuse cached artifacts if still valid):
#    - yocto-test/ local repo, scarthgap LTS, poky 802e4c1
#    - core-image-minimal, qemux86-64 MACHINE
#    bitbake invocation per the yocto-test/ README.

# 3. Re-run the issue-#481 mikebom command against the rebuilt rootfs:
./target/release/mikebom sbom scan \
    --path /path/to/yocto-build/tmp/work/qemux86_64-poky-linux/core-image-minimal/.../rootfs \
    --format spdx-2.3-json \
    --output /tmp/mikebom-m152/core-image-minimal.spdx.json

# 4. Inspect the 5 affected packages' licenseDeclared fields:
for pkg in busybox busybox-hwclock busybox-syslog busybox-udhcpc liblzma5; do
  echo "=== $pkg ==="
  jq -r ".packages[] | select(.name == \"$pkg\") | .licenseDeclared" \
    /tmp/mikebom-m152/core-image-minimal.spdx.json
done
```

**Expected output** (per SC-001 + spec's reference table):
```
=== busybox ===
GPL-2.0-only AND LicenseRef-bzip2-1.0.4
=== busybox-hwclock ===
GPL-2.0-only AND LicenseRef-bzip2-1.0.4
=== busybox-syslog ===
GPL-2.0-only AND LicenseRef-bzip2-1.0.4
=== busybox-udhcpc ===
GPL-2.0-only AND LicenseRef-bzip2-1.0.4
=== liblzma5 ===
LicenseRef-PD
```

(Or the canonical form `try_canonical` produces — minor whitespace/ordering normalization is acceptable.)

If all 5 packages emit non-`NOASSERTION` values → ✅ SC-001 PASS. Report PASS in the PR comments + close issue #481 on merge.

## Scenario 2 — SC-002 happy-path regression check (automated)

```bash
# The milestone-090 sibling-fixture cache should already be populated:
ls ~/.cache/mikebom/fixtures/*/transitive_parity/cargo/

# Run the standard workspace test suite — the existing golden tests
# verify SBOM byte-identity for the fully-canonicalizable happy path:
cargo +stable test --workspace
```

**Expected**: every test passes except the documented `sbomqs_parity::sbomqs_spdx_score_meets_or_beats_cdx_across_ecosystems` env-only flake. If any milestone-090 golden test fails, that's a SC-002 regression — investigate (likely cause: an unintended pipeline-ordering bug that re-canonicalizes a happy-path expression to a different but equivalent form).

## Scenario 3 — SC-003 idempotency check (automated via test #7)

```bash
cargo +stable test --workspace --package mikebom \
    -- idempotent_on_already_wrapped_input
```

**Expected**: 1 test passed.

The test asserts that feeding `GPL-2.0-only AND LicenseRef-bzip2-1.0.4` (the milestone-152 output shape) BACK as input to `preserve_known_operands_with_license_ref` produces the same string unchanged — no double-wrapping, no operator drift.

## Scenario 4 — SC-006 unit-test count + coverage (automated)

```bash
# Count new tests in rpm_file.rs that exercise the milestone-152 code path:
grep -cE "^\s+fn (preserve_|sanitize_|with_clause|happy_path_unchanged|empty_input|opaque_garbage|idempotent_on_already_wrapped|imprecise_synonym|parens_preserved)" \
    mikebom-cli/src/scan_fs/package_db/rpm_file.rs
```

**Expected**: ≥ 8 (per SC-006 floor; data-model §7 enumerates 12 tests).

## Scenario 5 — SC-005 + SC-008 pre-PR gate

```bash
./scripts/pre-pr.sh
```

**Expected**: green except the documented `sbomqs_parity::sbomqs_spdx_score_meets_or_beats_cdx_across_ecosystems` env-only flake.

## Scenario 6 — SC-007 wire-format guard (manual diff check)

```bash
# Verify no wire-format / catalog / annotation-key changes shipped:
git diff main --name-only -- \
    docs/reference/sbom-format-mapping.md \
    mikebom-cli/src/generate/cyclonedx/ \
    mikebom-cli/src/generate/spdx/ \
    mikebom-cli/src/generate/spdx/v3_*.rs
# Expected output: (empty)

git diff main --name-only -- mikebom-common/ mikebom-ebpf/
# Expected output: (empty — only mikebom-cli/src/scan_fs/package_db/rpm_file.rs + CHANGELOG.md)

git diff main --name-only -- mikebom-cli/src/scan_fs/
# Expected output: mikebom-cli/src/scan_fs/package_db/rpm_file.rs (the only changed file)
```

## Scenario 7 — SC-008 CHANGELOG entry presence

```bash
# Confirm the CHANGELOG.md entry was added under [Unreleased] / ### Fixed:
sed -n '/^## \[Unreleased\]/,/^## \[v/p' CHANGELOG.md | grep -A1 "LicenseRef"
```

**Expected**: a 4–8 line entry naming `LicenseRef-<sanitized>`, the sanitization rule, worked examples, and issue #481.

## Scenario 8 — Per-format wire-shape spot check (optional ad-hoc verification)

After the SC-001 PASS, the maintainer MAY re-emit the SC-001 testbed in all three formats and confirm the LicenseRef value rides correctly in each:

```bash
for fmt in cyclonedx-json spdx-2.3-json spdx-3-json; do
  ./target/release/mikebom sbom scan \
      --path /path/to/yocto-rootfs \
      --format $fmt \
      --output /tmp/mikebom-m152/core-image-minimal.$fmt.json
done

# CDX:
jq '.components[] | select(.name == "busybox") | .licenses' \
    /tmp/mikebom-m152/core-image-minimal.cyclonedx-json.json
# Expected: licenses entry carrying "GPL-2.0-only AND LicenseRef-bzip2-1.0.4" as
# either .license.id (for single-id) or .expression (for compound).

# SPDX 2.3:
jq '.packages[] | select(.name == "busybox") | .licenseDeclared' \
    /tmp/mikebom-m152/core-image-minimal.spdx-2.3-json.json
# Expected: "GPL-2.0-only AND LicenseRef-bzip2-1.0.4"

# SPDX 3:
jq '.["@graph"][] | select(.name == "busybox" and .type == "software_Package") | .software_packageLicenseDeclared' \
    /tmp/mikebom-m152/core-image-minimal.spdx-3-json.json
# Expected: same string (or null if SPDX 3 emits via a different path — verify).
```

**Rationale**: per research.md §R10, the format emitters all share the `SpdxExpression::as_str()` accessor — the LicenseRef value flows through unchanged. This spot check confirms no per-format special-casing surfaced a bug. Optional because the upstream `SpdxExpression` carrier is already format-agnostic.

## Known deferrals (spec Out of Scope)

- No deb_file / apk_file / gem / npm license-fallback work — separate milestone if those readers exhibit the same NOASSERTION collapse.
- No `mikebom:*` annotation introduced (Constitution V — `LicenseRef-` is standards-native).
- No `DocumentRef-` form emission (RPM reader lacks Yocto-recipe context per FR-011).
- No `licenseConcluded` changes per FR-012.
- No automated Yocto fixture in the milestone-090 sibling repo — SC-001 stays manual operator-cadence.
- No CLI opt-out flag — the behavior is a strict improvement; no operator would rationally want NOASSERTION back.
