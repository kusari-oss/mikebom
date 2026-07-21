//! Milestone 213 (issue #616) — kernel-side trace-noise filter classifier.
//!
//! Pure-Rust `no_std`-compatible classifier that matches file-open path
//! bytes against four filter categories (System, UserCache, Ephemeral,
//! CargoFingerprint). Called from `mikebom-ebpf/src/programs/file_ops.rs`
//! kprobes to drop noise events BEFORE they enter the FILE_EVENTS ring
//! buffer, freeing capacity for the actual rustc + linker events the
//! operator cares about.
//!
//! The classifier is shared between the kernel-side eBPF programs and
//! userspace test code by living in `mikebom-common` — same crate that
//! defines `FileEvent`. This lets us test the substring / prefix
//! matching logic exhaustively in `cargo test -p mikebom-common` without
//! needing a kernel + eBPF loader. The eBPF side calls
//! `path_matches_filter_category(&path, widen_system)` from
//! `try_do_filp_open` + `try_openat2` (m213 T012 + T013).
//!
//! ## Verifier safety
//!
//! All internal helpers use bounded loops with constant limits (`while j
//! < 16`, `while offset < 128`). Milestone 211 (issue #611) established
//! that this pattern passes the eBPF verifier on kernels 5.15 / 6.1 /
//! 6.6 / 6.8. See specs/213-kernel-noise-filter/contracts/ebpf-verifier-
//! notes.md for the full recipe.

use crate::events::FilterCategoryTag;

/// The four filter categories (mirrors `FilterCategoryTag` variants).
/// Kept as internal u8 to match `FilterCategoryTag` discriminants
/// verbatim per contracts/filter-category-tag.md.
const CAT_SYSTEM: u8 = 0;
const CAT_USER_CACHE: u8 = 1;
const CAT_EPHEMERAL: u8 = 2;
const CAT_CARGO_FINGERPRINT: u8 = 3;

/// Pattern-match kind. `Prefix` means the pattern must appear at path
/// offset 0. `Contains` means the pattern may appear anywhere in the
/// first 128 bytes of the path (bounded scan; longer paths are truncated
/// by the 256-byte kernel scratch buffer and effectively skipped —
/// matches FR-016 fail-open-on-unknown-category).
const KIND_PREFIX: u8 = 0;
const KIND_CONTAINS: u8 = 1;

/// One filter pattern. `#[repr(C)]` + 8-byte alignment via `_pad` so
/// the const-array layout is verifier-friendly.
#[derive(Copy, Clone)]
#[repr(C)]
struct FilterPattern {
    /// NUL-padded pattern bytes. Max 16 bytes accommodates the longest
    /// pattern (`/.local/share/` at 14 bytes) with slack.
    bytes: [u8; 16],
    /// Effective pattern length; MUST be ≤ 16.
    len: u8,
    /// `KIND_PREFIX` or `KIND_CONTAINS`.
    kind: u8,
    /// `FilterCategoryTag` discriminant (0-3).
    category: u8,
    /// Padding to align to 8 bytes.
    _pad: [u8; 5],
}

/// The pattern catalog. Order within a category doesn't matter; overall
/// order matches the discriminant order (System, UserCache, Ephemeral,
/// CargoFingerprint) purely for readability.
///
/// Pattern semantics (per specs/213-kernel-noise-filter/spec.md
/// FR-001..FR-004):
///
/// - **System (prefix)**: paths starting with `/etc/`, `/proc/`,
///   `/sys/`, `/dev/`. Kernel-meta filesystems + device nodes; 0 signal
///   in a build trace.
/// - **UserCache (contains)**: any path containing `/.cache/` or
///   `/.local/share/` as a directory component. Matches per-user cache
///   dirs across arbitrary HOME layouts (`/root/.cache/`,
///   `/home/foo/.cache/`, etc.) without needing userspace-to-kernel
///   HOME resolution per spec Assumptions.
/// - **Ephemeral (prefix)**: `/tmp/`, `/var/tmp/`. Compiler scratch;
///   already ephemeral.
/// - **CargoFingerprint (contains)**: `/fingerprint/`, `/deps/`,
///   `/incremental/`. Matches cargo's out-of-band bookkeeping paths
///   across any cargo profile name (debug, release, or user-custom) —
///   per spec Assumptions, we don't hard-code profile literals.
const PATTERNS: [FilterPattern; 11] = [
    // System — 4 prefix patterns
    FilterPattern {
        bytes: *b"/etc/\0\0\0\0\0\0\0\0\0\0\0",
        len: 5,
        kind: KIND_PREFIX,
        category: CAT_SYSTEM,
        _pad: [0; 5],
    },
    FilterPattern {
        bytes: *b"/proc/\0\0\0\0\0\0\0\0\0\0",
        len: 6,
        kind: KIND_PREFIX,
        category: CAT_SYSTEM,
        _pad: [0; 5],
    },
    FilterPattern {
        bytes: *b"/sys/\0\0\0\0\0\0\0\0\0\0\0",
        len: 5,
        kind: KIND_PREFIX,
        category: CAT_SYSTEM,
        _pad: [0; 5],
    },
    FilterPattern {
        bytes: *b"/dev/\0\0\0\0\0\0\0\0\0\0\0",
        len: 5,
        kind: KIND_PREFIX,
        category: CAT_SYSTEM,
        _pad: [0; 5],
    },
    // UserCache — 2 contains patterns
    FilterPattern {
        bytes: *b"/.cache/\0\0\0\0\0\0\0\0",
        len: 8,
        kind: KIND_CONTAINS,
        category: CAT_USER_CACHE,
        _pad: [0; 5],
    },
    FilterPattern {
        bytes: *b"/.local/share/\0\0",
        len: 14,
        kind: KIND_CONTAINS,
        category: CAT_USER_CACHE,
        _pad: [0; 5],
    },
    // Ephemeral — 2 prefix patterns
    FilterPattern {
        bytes: *b"/tmp/\0\0\0\0\0\0\0\0\0\0\0",
        len: 5,
        kind: KIND_PREFIX,
        category: CAT_EPHEMERAL,
        _pad: [0; 5],
    },
    FilterPattern {
        bytes: *b"/var/tmp/\0\0\0\0\0\0\0",
        len: 9,
        kind: KIND_PREFIX,
        category: CAT_EPHEMERAL,
        _pad: [0; 5],
    },
    // CargoFingerprint — 3 contains patterns
    FilterPattern {
        bytes: *b"/fingerprint/\0\0\0",
        len: 13,
        kind: KIND_CONTAINS,
        category: CAT_CARGO_FINGERPRINT,
        _pad: [0; 5],
    },
    FilterPattern {
        bytes: *b"/deps/\0\0\0\0\0\0\0\0\0\0",
        len: 6,
        kind: KIND_CONTAINS,
        category: CAT_CARGO_FINGERPRINT,
        _pad: [0; 5],
    },
    FilterPattern {
        bytes: *b"/incremental/\0\0\0",
        len: 13,
        kind: KIND_CONTAINS,
        category: CAT_CARGO_FINGERPRINT,
        _pad: [0; 5],
    },
];

/// Maximum offset (exclusive) at which a `Contains` pattern is scanned
/// for. Paths longer than this within the 256-byte scratch buffer are
/// effectively skipped (FR-016 fail-open-on-unknown-category). Chosen to
/// bound the verifier's loop-state cost: 128 offsets × 16-byte pattern
/// check = 2048 iterations per Contains pattern per open.
const CONTAINS_SCAN_MAX_OFFSET: usize = 128;

/// Check if `path` starts with the pattern bytes.
#[inline(always)]
fn path_starts_with(path: &[u8; 256], pattern: &[u8; 16], plen: usize) -> bool {
    // Bounded scan over the first 16 bytes; `plen` is guaranteed ≤ 16.
    let mut mismatch = false;
    let mut j = 0usize;
    while j < 16 {
        // Only compare within the effective pattern length; positions
        // past `plen` are NUL-padded in the pattern and irrelevant.
        // The `path[j]` read is always safe because j < 16 < 256.
        if j < plen && path[j] != pattern[j] {
            mismatch = true;
        }
        j += 1;
    }
    !mismatch
}

/// Check if `path` contains the pattern bytes as a substring anywhere
/// within the first `CONTAINS_SCAN_MAX_OFFSET` (128) bytes.
#[inline(always)]
fn path_contains_pattern(path: &[u8; 256], pattern: &[u8; 16], plen: usize) -> bool {
    let mut offset = 0usize;
    while offset < CONTAINS_SCAN_MAX_OFFSET {
        // Check if path[offset..offset+plen] matches pattern[..plen].
        // The inner loop is fully bounded (< 16); `path[offset + j]`
        // read is safe because offset + j < 128 + 16 = 144 < 256.
        let mut mismatch = false;
        let mut j = 0usize;
        while j < 16 {
            if j < plen && path[offset + j] != pattern[j] {
                mismatch = true;
            }
            j += 1;
        }
        if !mismatch {
            return true;
        }
        offset += 1;
    }
    false
}

/// Classify a file-open path into one of the four filter categories,
/// or return `None` if no category matches.
///
/// When `widen_system` is `true`, the System category returns `None`
/// even on match — per m213 FR-010 the widening flag disables ONLY the
/// System category, leaving UserCache/Ephemeral/CargoFingerprint active.
///
/// The path is a fixed-size `[u8; 256]` matching `FileEvent.path`.
/// NUL-terminated shorter paths are handled correctly by both matchers
/// (they compare against `plen`-length prefix/pattern only, ignoring
/// trailing NULs in `path`).
pub fn path_matches_filter_category(path: &[u8; 256], widen_system: bool) -> Option<FilterCategoryTag> {
    let mut i = 0usize;
    while i < PATTERNS.len() {
        let p = &PATTERNS[i];
        let matched = if p.kind == KIND_PREFIX {
            path_starts_with(path, &p.bytes, p.len as usize)
        } else {
            path_contains_pattern(path, &p.bytes, p.len as usize)
        };
        if matched {
            // Widen-flag gate: if this is a System category match AND
            // the operator opted into system-read visibility, skip past
            // it and continue checking the remaining patterns. This
            // preserves UserCache/Ephemeral/CargoFingerprint behavior
            // per FR-010 even when the System filter is off.
            if p.category == CAT_SYSTEM && widen_system {
                i += 1;
                continue;
            }
            return Some(match p.category {
                CAT_SYSTEM => FilterCategoryTag::System,
                CAT_USER_CACHE => FilterCategoryTag::UserCache,
                CAT_EPHEMERAL => FilterCategoryTag::Ephemeral,
                CAT_CARGO_FINGERPRINT => FilterCategoryTag::CargoFingerprint,
                // Unreachable per PATTERNS catalog above; if this fires
                // someone added a new pattern without extending the map.
                _ => return None,
            });
        }
        i += 1;
    }
    None
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    fn to_path_buf(s: &str) -> [u8; 256] {
        let mut buf = [0u8; 256];
        let bytes = s.as_bytes();
        let n = core::cmp::min(bytes.len(), 256);
        buf[..n].copy_from_slice(&bytes[..n]);
        buf
    }

    // --- T007: US1 unit tests --------------------------------------------

    #[test]
    fn t007_system_paths_classified() {
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/etc/hostname"), false),
            Some(FilterCategoryTag::System)
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/proc/self/status"), false),
            Some(FilterCategoryTag::System)
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/sys/kernel/debug/tracing"), false),
            Some(FilterCategoryTag::System)
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/dev/null"), false),
            Some(FilterCategoryTag::System)
        );
    }

    #[test]
    fn t007_user_cache_paths_classified() {
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/root/.cache/pip/wheels"), false),
            Some(FilterCategoryTag::UserCache)
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/home/mike/.cache/npm/foo"), false),
            Some(FilterCategoryTag::UserCache)
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/home/mike/.local/share/gem"), false),
            Some(FilterCategoryTag::UserCache)
        );
    }

    #[test]
    fn t007_ephemeral_paths_classified() {
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/tmp/rustc-xxx"), false),
            Some(FilterCategoryTag::Ephemeral)
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/var/tmp/cache-abc"), false),
            Some(FilterCategoryTag::Ephemeral)
        );
    }

    #[test]
    fn t007_cargo_fingerprint_paths_classified() {
        // The canonical cargo fingerprint path from issue #616 investigation.
        assert_eq!(
            path_matches_filter_category(
                &to_path_buf("/root/mikebom/target/debug/build/foo-abc/fingerprint/dep-blah"),
                false
            ),
            Some(FilterCategoryTag::CargoFingerprint)
        );
        assert_eq!(
            path_matches_filter_category(
                &to_path_buf("/home/dev/proj/target/release/deps/libfoo.rlib"),
                false
            ),
            Some(FilterCategoryTag::CargoFingerprint)
        );
        assert_eq!(
            path_matches_filter_category(
                &to_path_buf("/home/dev/proj/target/debug/incremental/foo/abc"),
                false
            ),
            Some(FilterCategoryTag::CargoFingerprint)
        );
    }

    #[test]
    fn t007_non_matching_paths_return_none() {
        // FR-012: non-matching paths flow through unfiltered.
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/home/dev/proj/src/main.rs"), false),
            None
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/home/dev/proj/target/release/mikebom"), false),
            None
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/usr/local/bin/rustc"), false),
            None
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/opt/myapp/config.toml"), false),
            None
        );
    }

    #[test]
    fn t007_truncated_full_buffer_paths_return_none() {
        // FR-016: paths that fill the 256-byte buffer with no NUL
        // terminator and no matching pattern in the first 128 bytes are
        // treated as "unknown category" (fail-open). This test builds a
        // 256-byte path of just 'x' characters — no pattern matches.
        let mut buf = [b'x'; 256];
        buf[0] = b'/';
        assert_eq!(path_matches_filter_category(&buf, false), None);
    }

    #[test]
    fn t007_fingerprint_beyond_scan_window_missed() {
        // Deliberate FR-016 corollary: `/fingerprint/` appearing PAST
        // the 128-byte scan window is NOT caught (fail-open-on-truncated).
        // Build a padding of ≥ 128 bytes of directory noise, then the pattern.
        let padding = "a".repeat(130);
        assert!(padding.len() >= CONTAINS_SCAN_MAX_OFFSET);
        let path = format!("/{}/fingerprint/dep-blah", padding);
        assert_eq!(path_matches_filter_category(&to_path_buf(&path), false), None);
    }

    // --- T026: US3 widen-flag unit tests --------------------------------

    #[test]
    fn t026_widen_flag_disables_system_only() {
        // FR-010: --include-system-reads disables System category ONLY.
        // With widen_system = true, System paths pass through.
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/etc/hostname"), true),
            None
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/proc/self/status"), true),
            None
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/sys/kernel/debug/tracing"), true),
            None
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/dev/null"), true),
            None
        );

        // But UserCache/Ephemeral/CargoFingerprint remain active.
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/root/.cache/pip/wheels"), true),
            Some(FilterCategoryTag::UserCache)
        );
        assert_eq!(
            path_matches_filter_category(&to_path_buf("/tmp/rustc-xxx"), true),
            Some(FilterCategoryTag::Ephemeral)
        );
        assert_eq!(
            path_matches_filter_category(
                &to_path_buf("/root/mikebom/target/debug/build/foo/fingerprint/dep-x"),
                true
            ),
            Some(FilterCategoryTag::CargoFingerprint)
        );
    }

    // --- Pattern-catalog stability guards --------------------------------

    #[test]
    fn patterns_catalog_size_matches_declared_categories() {
        // Guards against future-you adding a category without adding
        // patterns for it (or vice versa). 4 System + 2 UserCache +
        // 2 Ephemeral + 3 CargoFingerprint = 11.
        assert_eq!(PATTERNS.len(), 11);
    }

    #[test]
    fn all_patterns_have_valid_category_discriminants() {
        for p in &PATTERNS {
            assert!(
                (p.category as usize) < FilterCategoryTag::ALL.len(),
                "pattern has out-of-range category discriminant: {}",
                p.category
            );
        }
    }

    #[test]
    fn all_pattern_lengths_within_bounds() {
        for p in &PATTERNS {
            assert!(
                (p.len as usize) <= 16,
                "pattern length {} exceeds 16-byte buffer",
                p.len
            );
            assert!(p.len > 0, "pattern length must be non-zero");
        }
    }

    #[test]
    fn all_patterns_have_valid_kind_discriminants() {
        for p in &PATTERNS {
            assert!(
                p.kind == KIND_PREFIX || p.kind == KIND_CONTAINS,
                "pattern has unknown kind: {}",
                p.kind
            );
        }
    }
}
