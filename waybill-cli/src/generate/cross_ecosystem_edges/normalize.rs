//! FR-012 helper — target-ecosystem's normalization rule applies
//! when the cross-ecosystem fallback searches for a bare dep name in
//! a candidate ecosystem's slot of the resolver index.
//!
//! Delegates to the existing `crate::scan_fs::normalize_dep_name` at
//! `waybill-cli/src/scan_fs/mod.rs:1460` — this module exists only
//! to give the fallback code a clear callsite the reader can grep
//! for.

/// Return the dep name normalized as the TARGET ecosystem would
/// index it. Cross-ecosystem lookup MUST use the target's rules
/// (not the source's) because the target is what the name is
/// stored under in `name_to_purl`.
///
/// Delegates verbatim to the existing same-ecosystem normalization
/// helper — FR-012 requires the two paths agree bit-for-bit.
pub fn target_normalized_name(target_eco: &str, name: &str) -> String {
    crate::scan_fs::normalize_dep_name(target_eco, name)
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    #[test]
    fn delegates_to_scan_fs_normalize_dep_name() {
        // Invariant: the cross-ecosystem helper produces the same
        // output as the same-ecosystem path. Sampling one canonical
        // case per representative ecosystem is enough to catch a
        // divergence regression — the underlying implementation is
        // unit-tested in scan_fs/mod.rs::tests.
        assert_eq!(
            target_normalized_name("pypi", "Requests-OAuth"),
            crate::scan_fs::normalize_dep_name("pypi", "Requests-OAuth"),
        );
        assert_eq!(
            target_normalized_name("gem", "fastlane"),
            crate::scan_fs::normalize_dep_name("gem", "fastlane"),
        );
        assert_eq!(
            target_normalized_name("cargo", "clap_builder"),
            crate::scan_fs::normalize_dep_name("cargo", "clap_builder"),
        );
    }
}
