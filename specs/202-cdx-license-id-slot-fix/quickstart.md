# Quickstart: CDX License Splitter — LicenseRef Escape Hatch

**Date**: 2026-07-17
**Audience**: mikebom maintainer implementing or reviewing m202.

## Prerequisites

- Working mikebom checkout on branch `202-cdx-license-id-slot-fix`.
- `cargo +stable` toolchain (existing workspace toolchain).

## Reproducer 1 — Verify SC-001/SC-002/SC-003 against the m202 fixture

```bash
cargo test --manifest-path mikebom-cli/Cargo.toml --test ipk_license_splitter_m202 -- --nocapture 2>&1 | tail
```

**Expected**: `test result: ok. 2 passed; 0 failed; ...`.

## Reproducer 2 — End-to-end scan against a hand-built ipk

```bash
mkdir -p /tmp/m202-repro/{control,data}
cat > /tmp/m202-repro/control/control <<'CONTROL'
Package: test
Version: 1.0
Description: test
Section: base
Priority: optional
Maintainer: nobody
License: GPL-2.0-only & bzip2-1.0.4
Architecture: all
CONTROL
echo 2.0 > /tmp/m202-repro/debian-binary

(cd /tmp/m202-repro/control && tar -czf ../control.tar.gz control)
(cd /tmp/m202-repro/data && tar -czf ../data.tar.gz .)
(cd /tmp/m202-repro && ar cr test_1.0_all.ipk debian-binary control.tar.gz data.tar.gz)

mkdir -p /tmp/m202-ipks && mv /tmp/m202-repro/test_1.0_all.ipk /tmp/m202-ipks/
mikebom --offline sbom scan --format cyclonedx-json --path /tmp/m202-ipks --output /tmp/m202-out.cdx.json

jq '.components[] | .licenses' /tmp/m202-out.cdx.json
```

**Expected post-m202**:
```json
[
  {"license": {"id": "GPL-2.0-only", "acknowledgement": "declared"}},
  {"license": {"name": "LicenseRef-bzip2-1.0.4", "acknowledgement": "declared"}}
]
```

**Pre-m202 baseline** (for comparison):
```json
[
  {"license": {"id": "GPL-2.0-only", "acknowledgement": "declared"}},
  {"license": {"id": "bzip2-1.0.4", "acknowledgement": "declared"}}   ← BUG
]
```

## Reproducer 3 — Verify FR-004 regression guard (canonical operand unchanged)

```bash
# Any cargo/npm/etc. scan with canonical SPDX operands should not drift.
cargo test --manifest-path mikebom-cli/Cargo.toml --test cdx_regression 2>&1 | tail -3
cargo test --manifest-path mikebom-cli/Cargo.toml --test spdx_regression 2>&1 | tail -3
```

**Expected**: `test result: ok. N passed; 0 failed`. FR-004 preserves canonical-operand behavior.

## Reproducer 4 — Verify FR-002 CDX/SPDX 2.3 parity

```bash
mkdir -p /tmp/m202-parity
mikebom --offline sbom scan --format cyclonedx-json --path /tmp/m202-ipks --output /tmp/m202-parity/out.cdx.json
mikebom --offline sbom scan --format spdx-2.3-json --path /tmp/m202-ipks --output /tmp/m202-parity/out.spdx.json

cdx_name=$(jq -r '.components[0].licenses[] | .license.name // empty' /tmp/m202-parity/out.cdx.json | grep 'LicenseRef-')
spdx_licenseid=$(jq -r '.hasExtractedLicensingInfos[]?.licenseId' /tmp/m202-parity/out.spdx.json | grep 'LicenseRef-' | head -1)

echo "CDX name:  $cdx_name"
echo "SPDX id:   $spdx_licenseid"
[ "$cdx_name" = "$spdx_licenseid" ] && echo "PARITY ✓" || echo "PARITY FAILED"
```

**Expected**: both `LicenseRef-bzip2-1.0.4` (identical string). FR-002 verified.

## Reproducer 5 — Verify SC-006 pre-PR wall-clock delta

```bash
git checkout main
time ./scripts/pre-pr.sh 2>&1 | tail -3   # baseline

git checkout 202-cdx-license-id-slot-fix
time ./scripts/pre-pr.sh 2>&1 | tail -3   # post-m202
```

Delta MUST be ≤ 5s per SC-006. Expected delta ≈0s (small classifier extension + sanitizer extraction).

## Pre-PR gate

```bash
./scripts/pre-pr.sh
```

Both `cargo +stable clippy --workspace --all-targets` (zero errors, zero warnings) AND `cargo +stable test --workspace` (every suite `ok. N passed; 0 failed`) MUST pass green.

## Empirical re-verification at implement time (m199-m201 lesson)

Per `feedback_verify_research_empirical_claims` memory: before finalizing tasks.md, re-run:

```bash
# Post-implementation golden drift check:
git diff --stat mikebom-cli/tests/fixtures/ 2>&1 | tail

# If public-corpus goldens drift:
gh workflow run public-corpus.yml --field branch=main --field regen_goldens=true
```

**Expected**: only the new `ipk/license_licenseref_splitter_m202/` fixture dir. Any existing golden JSON drift signals unexpected reclassification — investigate.
