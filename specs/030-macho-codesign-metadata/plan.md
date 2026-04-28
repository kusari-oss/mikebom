---
description: "Implementation plan — milestone 030 Mach-O codesign metadata"
status: plan
milestone: 030
---

# Plan: Mach-O codesign metadata

## Architecture

Pure additive scanning extension. Three new public parsers in
`binary/macho.rs` follow the established 024 LC_UUID / LC_RPATH /
min-os shape — byte-level walk of load commands, find
`LC_CODE_SIGNATURE` (cmd 0x1D), parse the `LinkeditDataCommand`
preamble to get dataoff/datasize, walk the SuperBlob's index to
find CSMAGIC_CODEDIRECTORY, decode the requested CodeDirectory
field. All three parsers share a private `parse_codesign_codedirectory`
helper that does the SuperBlob → CodeDirectory navigation once.

`BinaryScan` gains three new fields that flow through the same
bag-emission path as the existing 024 Mach-O identity fields. The
`build_macho_identity_annotations` helper (in `entry.rs`) extends
to emit the three new annotation keys. **6th amortization-proof
bag consumer.**

No new crate dependencies. No new types in `mikebom-common`. No
public-API surface change beyond catalog rows + bag keys. No
schema migration.

## Reuse inventory

These existing items handle the work; this milestone consumes them:

- `binary/macho.rs::for_each_load_command` (existing internal
  helper from milestone 024) — walks load commands; new parsers
  use it to find LC_CODE_SIGNATURE.
- `binary/macho.rs::decode_header` (existing) — detects 32 vs 64
  bit + endianness; reused for header preamble walking.
- `BinaryScan` struct — gains 3 new fields; 4 struct-literal sites
  updated (scan.rs non-fat ELF/PE arm, scan.rs non-fat Mach-O arm,
  scan.rs fat-Mach-O arm, entry.rs::tests::fake_binary_scan).
- `entry.rs::build_macho_identity_annotations` — extends to emit
  3 new annotation keys.
- `extra_annotations` bag — 6th consumer.
- `cdx_anno!`, `spdx23_anno!`, `spdx3_anno!` macros — 3 new
  invocations per format = 9 lines.
- C-section catalog pattern — 3 new rows (C37/C38/C39).

## Touched files

| File | Change | LOC |
|---|---|---|
| `mikebom-cli/src/scan_fs/binary/macho.rs` | + 3 public parsers + `parse_codesign_codedirectory` helper + flag-bit decoder + 8+ inline tests | +280 |
| `mikebom-cli/src/scan_fs/binary/entry.rs` | + 3 BinaryScan fields + extension to `build_macho_identity_annotations` | +35 |
| `mikebom-cli/src/scan_fs/binary/scan.rs` | + 3 fields populated in non-fat Mach-O arm + fat-Mach-O first-slice + 4 struct-literal sites | +20 |
| `mikebom-cli/src/parity/extractors/cdx.rs` | + 3 cdx_anno! invocations | +5 |
| `mikebom-cli/src/parity/extractors/spdx2.rs` | + 3 spdx23_anno! invocations | +5 |
| `mikebom-cli/src/parity/extractors/spdx3.rs` | + 3 spdx3_anno! invocations | +5 |
| `mikebom-cli/src/parity/extractors/mod.rs` | + 3 EXTRACTORS rows + 9 fn imports | +12 |
| `docs/reference/sbom-format-mapping.md` | + 3 C-section rows | +3 |
| `mikebom-cli/tests/scan_binary.rs` | + macOS-lane assertions (Apple Team ID + identifier + hardened-runtime flag) | +20 |

Total Rust source: ~385 LOC across 8 files.

## Phasing

Three atomic commits in dependency order:

### Commit 1: `030/parsers`
- 3 public parsers + private `parse_codesign_codedirectory`
  helper + private flag-bit decoder.
- 8+ inline tests in `macho.rs::tests` covering happy path +
  multi-flag + unknown-bit + old-CD-version + no-LC_CODE_SIGNATURE
  + malformed-SuperBlob.
- `#[allow(dead_code)]` on the new public parsers (lifted in
  commit 2).

### Commit 2: `030/wire-up-bag`
- `BinaryScan` gains 3 new fields.
- 4 BinaryScan struct-literal sites updated.
- `scan.rs::scan_binary` non-fat path calls the 3 parsers when
  `class == "macho"`.
- `scan.rs::scan_fat_macho` calls them against the first slice's
  bytes (per the 024 first-slice convention).
- `entry.rs::build_macho_identity_annotations` extends to emit
  the 3 new annotation keys via skip-on-empty.
- `tests/scan_binary.rs` macOS-lane assertions added.
- Lift `#[allow(dead_code)]` from commit 1's parsers.

### Commit 3: `030/parity-rows`
- 3 new C-section rows (C37/C38/C39) in
  `docs/reference/sbom-format-mapping.md`.
- 9 new `*_anno!` invocations across cdx/spdx2/spdx3.
- 3 new EXTRACTORS rows + 9 fn imports in `parity/extractors/mod.rs`.

Per FR-011, each commit's `./scripts/pre-pr.sh` is clean.

## Estimated effort

| Phase | Effort | Notes |
|---|---|---|
| Phase 1 (recon + baseline) | done | T001 done in scoping |
| Phase 2 (parsers) | 3 hr | byte-level SuperBlob walk; 8+ tests with hand-built fixtures |
| Phase 3 (wire-up + macOS-lane test) | 2 hr | 4 struct-literal sites + bag emission + integration test |
| Phase 4 (parity rows) | 30 min | mechanical |
| Phase 5 (verify + PR) | 1 hr | golden regen + CI watch |
| **Total** | **~6-7 hr** | comparable to 024. |

## Risks

- **R1: SuperBlob endianness quirk.** SuperBlob fields are
  big-endian regardless of host or Mach-O native endianness.
  Easy to misread if you assume LE. Mitigation: explicit
  `u32::from_be_bytes` calls + tests against fixtures with
  known-correct values.

- **R2: CodeDirectory version differences.** v1.0.x doesn't have
  teamOffset; v2.x does. Mitigation: check `cd_version >=
  0x20200` before reading teamOffset (per Apple's source).
  Tests cover both arms.

- **R3: Multiple LC_CODE_SIGNATURE commands.** Spec says at most
  one, but corruption could produce multiples. Mitigation:
  emit from the first; `tracing::warn!` if multiples found.

- **R4: macOS CI lane SC-002 brittleness.** Apple could rotate
  team IDs in future macOS releases. Mitigation: assert team-id
  matches `[A-Z0-9]{10}` rather than the literal `EQHXZ8M8AV`.
  If the regex fails, that's a real signal worth investigating
  at PR time.

## Constitution alignment

- **Principle I (Pure Rust, Zero C):** no new deps. ✓
- **Principle IV (no `.unwrap()` in production):** all parsers
  return `Option`/`Vec` on every failure path; no panics. ✓
- **Principle VI (Three-Crate Architecture):** untouched. ✓
- **Principle IX (Accuracy):** new evidence is build-time-truth
  (CodeDirectory bytes are what `codesign -dv` reads); same
  high-confidence tier framing as the existing 024 fields. ✓
- **Per-commit verification:** FR-011 enforced.
- **Recon-first:** every claim in the spec backed by file:line
  evidence (`object/macho.rs:727` for LC_CODE_SIGNATURE constant;
  Apple's `cscdefs.h.auto.html` source for the flag-bit names;
  existing macho.rs `for_each_load_command` for the load-command
  walk pattern).
- **Bag amortization:** SC-005 verifies zero churn outside
  `binary/` + `parity/extractors/`. **6th amortization-proof
  consumer.**

## What this milestone does NOT do

- Does not extract entitlements XML.
- Does not decode the CMS PKCS#7 cert chain.
- Does not validate code-directory page hashes.
- Does not parse designated-requirements expressions.
- Does not detect notarization tickets.
- Does not handle detached `.dSYM`-bundled signatures.
- Does not introduce any new crate dependency.
- Does not change CLI args, output flags, or schema.
