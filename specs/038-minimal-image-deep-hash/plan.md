# Implementation Plan: Per-File Evidence for Minimal-Image Scans

**Branch**: `038-minimal-image-deep-hash` | **Date**: 2026-04-28 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/038-minimal-image-deep-hash/spec.md`

## Summary

Close the per-file evidence gap for minimal-image scans (distroless,
Bazel-built, chainguard apko). Milestone 037 made the package list
correct on these images; milestone 038 makes
`evidence.occurrences[]` match what full-fat-image scans produce.

**Technical approach**: extend
`mikebom-cli/src/scan_fs/package_db/file_hashes.rs::read_info_file`
to fall back to `var/lib/dpkg/status.d/<pkg>.md5sums` when the
legacy `var/lib/dpkg/info/<pkg>.list` is absent. The `.md5sums`
file's per-line `<md5hex>  <relpath>` format already encodes the
path list; the existing per-file hash loop in `hash_package_files`
consumes paths regardless of source. Total ~120 LOC of production
code + ~150 LOC of inline tests across two files; no new
dependencies.

For US2 (chainguard apko), Phase-0 research determines whether the
existing apk reader covers apko-built images out of the box (in
which case the milestone is a smoke-test verification + docs
exercise) or requires a small variant reader. Lower priority and
investigation-driven.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited
from milestones 001–037; no nightly required for user-space-only
work).

**Primary Dependencies**: existing only — `sha2` (per-file SHA-256,
already used by `file_hashes.rs`), `tempfile` (test fixtures, dev-
dep), stdlib `std::fs` / `std::io`. No new top-level deps; the
`no_c_dependencies` regression test continues to pass.

**Storage**: N/A — all state in-process per scan; reuses milestone
037's filesystem-read posture.

**Testing**: `cargo +stable test --workspace` (per Constitution
Pre-PR Verification gate). Inline tests in `dpkg.rs` and
`file_hashes.rs` for unit coverage; gated network smoke test in
`mikebom-cli/tests/oci_registry_smoke.rs` (under
`MIKEBOM_OCI_NETWORK_TESTS=1`) for end-to-end verification on real
distroless and chainguard images.

**Target Platform**: Linux containers (the scan target). The mikebom
CLI itself runs on Linux + macOS (already supported).

**Project Type**: CLI / library (Rust three-crate workspace per
Constitution Principle VI: `mikebom-cli`, `mikebom-common`,
`mikebom-ebpf`).

**Performance Goals**: equal-or-better than full-fat images.
Minimal-image scans have far fewer packages (4–~50 vs 100–500+) and
smaller per-package file lists, so deep-hash cost should be
proportionally lower. Inherit the existing 256 MB per-file hash cap
unchanged.

**Constraints**:

- Constitution Principle I (Pure Rust, Zero C) — verified via the
  `no_c_dependencies_in_tree` and
  `no_c_dependencies_in_oci_registry_feature_tree` regression
  tests; no new deps to introduce risk.
- Constitution Principle IV (no `.unwrap()` in production) — new
  helpers return `Option`/`Vec`; tests use the standard
  `cfg_attr(test, allow(clippy::unwrap_used))` envelope.
- Spec FR-005 — full-fat byte-identity goldens MUST regen with
  zero diff (no behavior change for the existing path).

**Scale/Scope**: ~270 LOC across 2 source files + 1 test file +
docs. Single PR in 2–3 atomic commits.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1
design.*

| Principle | Applicability | Status |
|---|---|---|
| I. Pure Rust, Zero C | Active | ✅ No new deps; regression test still green. |
| II. eBPF-Only Observation | Discovery-mode principle; this milestone touches the static-image scan path established in milestones 002–037. The per-image filesystem-scan surface is the explicitly accepted second face of mikebom (alongside the trace-mode face), and this milestone extends that existing surface only. | ✅ N/A to scan-path work. |
| III. Fail Closed | Discovery-mode principle. | ✅ N/A. |
| IV. Type-Driven Correctness | Active | ✅ New helpers return `Option`/`Vec`; no `.unwrap()` in production paths; `Purl` newtype already in use upstream. |
| V. Specification Compliance | Active | ✅ No SBOM output schema changes. `evidence.occurrences[]` already conforms to CycloneDX 1.6 / SPDX 2.3 / SPDX 3 — this milestone populates the same field with the same shape. |
| VI. Three-Crate Architecture | Active | ✅ Touches `mikebom-cli` only. |
| VII. Test Isolation | Active | ✅ All new tests run unprivileged; no eBPF gating. |
| VIII. Completeness | Active | ✅ This milestone is itself a completeness fix — closes a known false-negative (per-file evidence missing for minimal-image components). |
| IX. Accuracy | Active | ✅ File content hashes are observed-bytes truth; no new false-positive risk. |
| X. Transparency | Active | ✅ Existing per-file evidence already carries provenance; no new transparency concerns. |
| XI. Enrichment | Active | ✅ N/A — no external data sources used. |
| XII. External Data Source Enrichment | Active | ✅ N/A. |

**No constitution violations.** No entries in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/038-minimal-image-deep-hash/
├── plan.md              # This file
├── research.md          # Phase 0: source-of-truth confirmations + apko recon
├── data-model.md        # Phase 1: filelist-source discriminator + helper signatures
├── quickstart.md        # Phase 1: post-implementation verification recipe
├── contracts/           # (omitted — internal CLI tool, no external contracts)
├── checklists/
│   └── requirements.md  # /speckit.specify quality checklist (passing)
└── tasks.md             # Phase 2 output (/speckit.tasks - NOT created here)
```

### Source Code (repository root)

```text
mikebom-cli/                         # user-space CLI (touched)
├── src/
│   ├── scan_fs/
│   │   └── package_db/
│   │       ├── dpkg.rs              # source-discovery: legacy + status.d/ (037)
│   │       ├── file_hashes.rs       # ★ read_info_file fallback to status.d/<pkg>.md5sums
│   │       ├── apk.rs               # potentially touched if apko recon discovers a variant
│   │       └── …                    # (rpm, etc., untouched)
│   ├── scan_fs/binary/              # untouched
│   ├── parity/                      # untouched
│   ├── generate/                    # untouched
│   └── resolve/                     # untouched
└── tests/
    └── oci_registry_smoke.rs        # ★ new gated assertion: distroless evidence non-empty

mikebom-common/                      # untouched
mikebom-ebpf/                        # untouched
```

**Structure Decision**: Three-crate workspace (Constitution
Principle VI) is preserved unchanged. All source edits land in
`mikebom-cli/src/scan_fs/package_db/file_hashes.rs` (primary) and
optionally `mikebom-cli/src/scan_fs/package_db/apk.rs` (only if
US2 recon discovers a variant). Test edits land in
`mikebom-cli/tests/oci_registry_smoke.rs` (and inline next to
the modified production code).

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be
> justified**

No constitution violations. Section intentionally empty.
