# Research: ipk reader — modern ar-format extraction + filename-fallback arch fix (m187)

**Feature**: [spec.md](./spec.md) · **Plan**: [plan.md](./plan.md)

## Decisions

### Decision 1 — ar-format parser implementation approach (hand-rolled, no new crate)

**Decision**: Hand-roll a ~100-line ar-archive member enumerator directly inside `ipk_file.rs` as a private helper. Return `Result<Vec<ArMember>, ArError>` where `ArMember { name: String, data: Vec<u8> }` and `ArError` covers the three malformed classes (truncated header, non-ASCII size, size overrun body).

**Rationale**:
- BSD ar format is trivially simple: 8-byte magic (`!<arch>\n`), then per-member 60-byte fixed-size header (16-byte name + 12-byte mtime + 6-byte uid + 6-byte gid + 8-byte mode + 10-byte size + 2-byte end-marker), then data padded to even byte boundary. No compression, no encryption, no signatures. Hand-rolling is ~100 lines and covers 100% of what opkg-build produces.
- opkg-build uses ONLY short names (`debian-binary`, `control.tar.gz`, `data.tar.gz`) — all fit in the 16-byte name field without GNU-ar-long-name extensions (`//` name-table or `#1/N` inline-length prefixes). We don't need to handle those.
- Zero new Cargo dependencies (Constitution I + FR-015 + SC-007). The `ar` crate (v0.9) exists on crates.io but adds ~200 lines of transitive dep-tree churn for functionality we can hand-roll in the same space.
- Matches Constitution's stated preference (m152/m185 both hand-rolled parsers vs pulling crates). The precedent is well-established.

**Alternatives considered**:
- **Add `ar = "0.9"` crate** — Rejected. Costs `cargo tree` line drift (~5-8 lines of transitive additions); benefit is minimal (~50 LOC saved) vs. cost (Constitution I violation + SC-007 gate broken).
- **Reuse a `.deb` reader crate** (`.deb` files are ar-format) — Rejected. `debpkg` and similar crates conflate `.deb`-specific semantics (Debian-vs-Ubuntu control-file variants, DEBIAN dir naming conventions) with the ar container. m187 wants the ar-container-only slice.
- **Parse via `std::process::Command::new("ar")`** — Rejected. Fails on macOS where `ar` is Apple's proprietary `ar(1)` with different flag semantics + not guaranteed on all Linux distros. Constitution I "Zero C" also implicitly prefers pure-Rust over shelling out.

---

### Decision 2 — Member-selection order + tolerance for missing `debian-binary`

**Decision**: The `parse_ar_archive` helper returns members in the order they appear in the archive, with no filtering. The caller (`parse_ipk_file`) then:
1. Scans for a member named `control.tar.gz` OR `control.tar` (uncompressed inner tar variant).
2. Scans for a member named `data.tar.gz` OR `data.tar`.
3. If `debian-binary` is present, reads its content (should be `2.0\n`); logs a WARN if any other value but proceeds.
4. If `debian-binary` is ABSENT, proceeds with extraction (WARN log flags the anomaly).
5. If BOTH `control.tar[.gz]` are absent, returns `IpkParseError::ControlMissing` (existing variant, reused).

**Rationale**:
- Spec's Edge Cases (line 60-63) explicitly requires tolerance for anomalies: missing `debian-binary`, member reorderings, uncompressed inner tar. Encoding member-order-independence in the parser (return `Vec` unfiltered; scan for names) is the simplest way to satisfy all three.
- Reuses existing `IpkParseError::ControlMissing` variant — no enum-shape churn on the primary failure class. Downstream code (the caller's `match` in `read()`) already handles this branch correctly.
- The `debian-binary` value is diagnostic-only per spec.md Edge Cases; not gating.

**Alternatives considered**:
- **Strict member ordering + strict `debian-binary` presence** — Rejected. Would produce a legitimate ipk failure surface for real-world builds that omit `debian-binary` (some non-conforming vendor builds do). Overly rigid.
- **Return an ordered enum `ArMemberKind::Control | Data | DebianBinary | Other`** — Rejected. Overengineered for a private helper called from one site. `Vec<ArMember>` is Vec of `(name, data)` — the caller iterates once.

---

### Decision 3 — Suffix-match implementation for FR-010 parent-dir arch source

**Decision**: The suffix-match rule (from Clarifications Q1) is implemented as a pure string operation on the pre-`.ipk` filename portion, byte-for-byte case-sensitive:

```rust
fn parent_dir_arch_match<'a>(filename_no_ext: &'a str, parent_dir_name: &str) -> Option<&'a str> {
    // Returns Some(name_and_version_portion) IFF the filename ends with
    // `_<parent_dir_name>`. The returned string is everything BEFORE the
    // matched `_<parent-dir-name>` suffix.
    let suffix = format!("_{parent_dir_name}");
    filename_no_ext.strip_suffix(&suffix)
}
```

Callers then apply the existing `<name>_<version>` LEFT split on the returned prefix, treating the parent-dir name as the authoritative arch.

**Rationale**:
- Byte-for-byte case-sensitive matches Yocto's actual behavior: Yocto emits arch dirs + filename arch suffixes with identical casing (both lowercase for `qemux86_64`, both mixed for `x86-64` when applicable). Case-insensitive matching would slightly loosen the gate at negligible correctness benefit.
- Returning the prefix (as `Option<&str>`) enables reuse of the existing `<name>_<version>` LEFT-split logic without duplication. The parent-dir case + the filename-heuristic case both end up calling `split_once('_')` on their respective prefixes.
- Pure string operation — no regex, no allocation beyond the transient `format!` for the suffix. Fits Constitution I posture.

**Alternatives considered**:
- **Case-insensitive suffix match** — Rejected. Yocto doesn't produce case-mismatched arches; loosening the gate here just increases the false-positive surface.
- **Regex-based match** — Rejected. `regex` crate is already in the workspace but string-suffix comparison is stdlib and clearer.
- **Fuzzy match** (edit distance, longest-common-suffix) — Rejected. Way over-engineered. The Yocto convention is exact-match; anything else is genuinely different.

---

### Decision 4 — `IpkParseError::LegacyArFormat` variant rename scope

**Decision**: Rename the variant to `LegacyGzipTarFallbackFailed` AND simultaneously add a new variant `ArMalformed(String)` for the case where ar-format is detected but the ar-container parser fails. Both variants have distinct `Display` impls that name the actual failure mode.

The old `LegacyArFormat` name is dropped entirely (no deprecation alias). Any downstream test that matches on the variant by name will fail to compile; the compiler error surfaces the sites that need updating.

**Rationale**:
- FR-016 mandates the rename (`MUST be renamed (or replaced)`). The Assumption gave scope-flexibility but not permission to skip the rename entirely.
- The variant is `pub(crate)`-scoped so its blast radius is `mikebom-cli` internal only. No external consumers exist.
- Adding `ArMalformed(String)` is necessary anyway — the new primary ar-format path needs a failure class distinct from `LegacyGzipTarFallbackFailed`. Doing both renames in one milestone avoids a partial-migration state where the enum is half-updated.
- Compile-time detection of stale variant references (via `cargo build` failure on any `LegacyArFormat` reference) is safer than a runtime "silent match arm fires the wrong branch" outcome.

**Alternatives considered**:
- **Keep `LegacyArFormat` as a deprecated alias for one milestone** — Rejected. No downstream external consumers exist. The deprecation-alias overhead has zero benefit.
- **Rename to `Legacy` or similar shorter name** — Rejected. `LegacyGzipTarFallbackFailed` is verbose but precisely describes what failed and when. Post-m187, the pre-2015 `gzip(tar)` path is the actual "legacy" path; the name reflects that.

---

## Bug Discovery

None so far. m187 is a pure fix-forward milestone with clear symptoms documented in #542 + #543 and root causes identified during Phase 0. No latent issues surfaced during code survey.

## Related Reading

- **spec.md**: full US1 + US2 acceptance scenarios + 10 edge cases.
- **Issue #542**: filename-fallback arch regression, with concrete `kernel_qemux86_64.ipk` example showing `?arch=64` symptom.
- **Issue #543**: full stock Yocto `busybox` control-file example + `ar tv` output showing the modern ar-format shape.
- **`mikebom-cli/src/scan_fs/package_db/ipk_file.rs`**: current implementation with the misclassification bug at line 316 (`LegacyArFormat` return) + the rsplit filename-parser at line 629.
- **`mikebom-cli/src/scan_fs/package_db/rpm_file.rs`**: reference implementation for the extraction-then-license-normalize-then-emit pipeline; m187 mirrors this shape for the ar-format code path.
- **BSD ar(5) format spec**: https://man7.org/linux/man-pages/man5/ar.5.html — the 60-byte-header shape used by opkg-build.
