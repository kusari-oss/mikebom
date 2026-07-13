# Quickstart: ipk reader — modern ar-format extraction + filename-fallback arch fix (m187)

**Feature**: [spec.md](./spec.md) · **Plan**: [plan.md](./plan.md) · **Contracts**: [ipk-parse-pipeline.md](./contracts/ipk-parse-pipeline.md)

## Operator worked examples

### Example 1 — Yocto build output scan (post-m187 primary flow)

**Use case**: You've built a Yocto image (`bitbake core-image-minimal`) and want mikebom to enumerate every ipk artifact with full metadata (license, deps, section, maintainer).

```bash
mikebom sbom scan \
  --path /path/to/build/tmp/deploy/ipk \
  --format cyclonedx-json \
  --output /tmp/yocto.cdx.json
```

**Expected outcomes**:
- Every ipk in `tmp/deploy/ipk/<arch>/*.ipk` is parsed via the ar-format primary path.
- Every emitted component carries `mikebom:source-mechanism = "ipk-file-archive-extraction"` + `mikebom:arch-source = "control-file"`.
- Every component has `licenses[]` populated when the control file's `License:` field is non-empty (post-m152/m185 canonicalization).
- Every `Depends:` and `Recommends:` entry surfaces as a dependency edge.
- `?arch=` qualifiers match the control file's `Architecture:` field exactly — including multi-underscore arches like `qemux86_64`.

**INFO log excerpt** (verbose mode):
```
INFO mikebom::scan_fs::package_db::ipk_file: extracted ar-format ipk
  path=/build/tmp/deploy/ipk/qemux86_64/kernel-6.6.127-yocto-standard_...ipk
  name=kernel-6.6.127-yocto-standard
  version=6.6.127+git0+45f69741c7_70af2998be-r0
  arch=qemux86_64
  license=GPL-2.0-only & bzip2-1.0.4
```

### Example 2 — Malformed ar body → filename fallback with parent-dir arch

**Use case**: You're scanning a partially-corrupted Yocto build (mid-build interruption; some ipks have truncated ar bodies). mikebom should still emit correct PURLs for the corrupted files, using the parent-directory arch signal.

```bash
mikebom sbom scan \
  --path /path/to/corrupted-build/tmp/deploy/ipk \
  --format cyclonedx-json \
  --output /tmp/corrupted.cdx.json
```

**Expected outcomes**:
- Well-formed ipks: normal ar-format extraction (Example 1 path).
- Corrupted ipks (e.g., `qemux86_64/foo_1.0-r0_qemux86_64.ipk` with truncated ar body): mikebom logs a WARN naming the specific ar-parse failure, then salvages via filename fallback with parent-dir arch signal.
- Corrupted components emit `?arch=qemux86_64` (correct — from parent-dir suffix match), `version=1.0-r0` (correct — no `_qemux86` gluing), `mikebom:arch-source = "parent-directory"`, `mikebom:source-mechanism = "ipk-file-filename-fallback"`.

**WARN log excerpt**:
```
WARN mikebom::scan_fs::package_db::ipk_file: salvaging .ipk via filename fallback
  path=/build/tmp/deploy/ipk/qemux86_64/foo_1.0-r0_qemux86_64.ipk
  reason=ar-format: truncated header at offset 68 (expected 60 bytes, got 12)
```

### Example 3 — Legacy pre-2015 ipk (backward compat)

**Use case**: You have a pre-2015 opkg-build ipk lying around (e.g., from an archived OpenWrt release). mikebom should still parse it via the `gzip(tar)` fallback.

```bash
mikebom sbom scan \
  --path /path/to/legacy-openwrt \
  --format cyclonedx-json \
  --output /tmp/legacy.cdx.json
```

**Expected outcomes**:
- ar-magic probe fails (file bytes 0..7 ≠ `!<arch>\n`).
- Fall through to the `gzip(tar)` outer parser (pre-m187 behavior, unchanged).
- Emitted component carries `mikebom:source-mechanism = "ipk-file"` (existing value, byte-identical to pre-m187).
- No `mikebom:arch-source` property emitted (only relevant for the filename-fallback path).

### Example 4 — Loose ipk file with unrelated parent directory

**Use case**: You've downloaded a bunch of ipks into `~/downloads/` for offline inspection. The parent directory (`downloads`) is not an arch name.

```bash
mikebom sbom scan \
  --path ~/downloads \
  --format cyclonedx-json \
  --output /tmp/downloads.cdx.json
```

**Expected outcomes**:
- Each ipk parsed via ar-format primary path (assuming well-formed).
- For any ipk that falls through to the filename-fallback (malformed body), the parent-dir suffix match FAILS (`downloads` doesn't match the filename's arch suffix) → falls back to the filename rsplit heuristic.
- Emits `mikebom:arch-source = "filename-heuristic"` — operators can filter these low-confidence emissions in downstream tooling if needed.

## Developer worked example (contributor flow)

### Adding a new inner-tar compression format (e.g., `control.tar.zst`)

Currently deferred per spec.md §Deferred to Future Milestones. To land it in a follow-up milestone:

1. Extend the `control_tar_member_names` scan in `parse_ipk_file` (around the FR-002 loop) to include `control.tar.zst`.
2. Add a `zstd` decompression branch (mirroring the existing `flate2` branch for `.gz`).
3. Add a new unit test in `ipk_file.rs::tests::parse_ar_archive_handles_zstd_inner_tar`.
4. Extend the integration test suite in `ipk_yocto_reader_fixes.rs::us1_ar_format_extracts_zstd_control`.
5. Update `docs/reference/component-tiers.md` (or wherever the ipk-format-support matrix lives) to reflect the new compression.

Note: adding `zstd` support introduces a new Cargo dep (the `zstd` crate); this changes the SC-007 zero-new-deps posture. The follow-up milestone will need to explicitly justify the dep addition in its Constitution Check.

### Running the m187 tests locally

```bash
# Unit tests only (fast; ~1s):
cargo +stable test -p mikebom --bin mikebom scan_fs::package_db::ipk_file::tests

# Integration tests (~10s; wiremock-free — synthesizes ipk bytes at test time):
cargo +stable test -p mikebom --test ipk_yocto_reader_fixes

# Existing m169 ipk-reader integration test (verifies no drift on legacy gzip-tar path):
cargo +stable test -p mikebom --test ipk_reader
cargo +stable test -p mikebom --bin mikebom scan_fs::package_db::ipk_file

# Full pre-PR gate:
./scripts/pre-pr.sh
```

### Verification checklist for merge

Before opening a PR:
- [ ] `cargo +stable clippy --workspace --all-targets -- -D warnings` — zero warnings.
- [ ] `cargo +stable test --workspace` — every suite passes with `0 failed`.
- [ ] `./scripts/pre-pr.sh` runs to green.
- [ ] Zero drift in `cargo tree --workspace` output (SC-007 zero-new-deps gate).
- [ ] Existing pre-2015 gzip-tar-format fixture components produce byte-identical output (FR-014 / SC-005; check via targeted golden diff).
- [ ] The renamed `IpkParseError::LegacyGzipTarFallbackFailed` variant compiles clean across `mikebom-cli/` (no stale `LegacyArFormat` references).

## FAQ

**Q: Does m187 change the emitted output for pre-2015 gzip-tar-format ipks?**
A: No. The `gzip(tar)` code path is preserved verbatim. `mikebom:source-mechanism = "ipk-file"` (existing) is unchanged; no new properties emitted for that path. Byte-identity per SC-005.

**Q: What happens if an ipk has BOTH an ar-format shape AND a valid `gzip(tar)` shape (e.g., adversarial nesting)?**
A: mikebom picks the ar-format path via magic-byte detection at offset 0. Truly-ambiguous shapes are extremely unusual — no known real-world producer emits both. If encountered, the ar-format parser wins (matches the OCI spec's "first-8-bytes-magic" convention for other formats).

**Q: Can I disable the ar-format path if I only want the old behavior?**
A: No. The `LegacyGzipTarFallbackFailed` fallback is triggered automatically when ar-parse fails. There's no CLI flag to force the gzip-tar path — but the codepath is preserved for pre-2015 ipks that legitimately need it.

**Q: What if my ipk has `control.tar.zst` (zstd-compressed) instead of `control.tar.gz`?**
A: Not supported in m187 (see spec.md §Deferred). Extract manually via `zstd -d` and repack as `control.tar.gz`, OR wait for the follow-up milestone. mikebom's error message will name `control.tar.gz` (or `control.tar`) as the expected member.

**Q: How does m187 handle signed ipks (e.g., PGP signatures via `.ipk.sig` sidecars)?**
A: Signatures are ignored per spec.md §Deferred. mikebom emits the component without verifying the signature. A future signed-verification milestone can add this.
