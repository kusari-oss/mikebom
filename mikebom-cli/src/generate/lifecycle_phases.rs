//! Lifecycle-phase aggregation shared between CDX and SPDX
//! serializers (milestone 047).
//!
//! Maps the per-component `mikebom:sbom-tier` value (one of
//! `design`, `source`, `build`, `deployed`, `analyzed`) to the
//! corresponding CycloneDX 1.6 / SPDX-comment phase name, and
//! aggregates the observed phase set across a scan's components.
//!
//! Source-of-truth for CDX `metadata.lifecycles[]` and the SPDX
//! 2.3 `creationInfo.comment` / SPDX 3 `SpdxDocument.comment`
//! fields. Both serializers MUST call into this module so the
//! phase set is identical regardless of output format.
//!
//! Sort order is deterministic (lexicographic via `BTreeSet`)
//! so byte-identity goldens regen cleanly.
//!
//! Coverage: end-to-end byte-identity goldens
//! (`mikebom-cli/tests/cdx_regression.rs`,
//! `mikebom-cli/tests/spdx_regression.rs`,
//! `mikebom-cli/tests/spdx3_regression.rs`) exercise both
//! functions through the live serializer pipeline. Any change
//! to phase mapping or aggregation order surfaces as a goldens
//! regression.

use std::collections::BTreeSet;

use mikebom_common::resolution::ResolvedComponent;

/// Map a `mikebom:sbom-tier` string to its corresponding
/// CycloneDX 1.5+ `lifecycles[].phase` value. Returns `None` for
/// unrecognised tier strings so unknown tiers don't pollute the
/// aggregated phase set.
pub fn tier_to_phase(tier: &str) -> Option<&'static str> {
    match tier {
        "build" => Some("build"),
        "deployed" => Some("operations"),
        "analyzed" => Some("post-build"),
        "source" => Some("pre-build"),
        "design" => Some("design"),
        _ => None,
    }
}

/// Aggregate the unique set of CDX phase names observed across
/// the given components' `sbom_tier` values. Returns the phase
/// list sorted lexicographically (deterministic for byte-identity
/// goldens).
pub fn aggregate_phases<'a>(
    components: impl IntoIterator<Item = &'a ResolvedComponent>,
) -> Vec<&'static str> {
    let mut phases: BTreeSet<&'static str> = BTreeSet::new();
    for c in components {
        if let Some(ref tier) = c.sbom_tier {
            if let Some(phase) = tier_to_phase(tier) {
                phases.insert(phase);
            }
        }
    }
    phases.into_iter().collect()
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    #[test]
    fn tier_to_phase_maps_known_tiers() {
        assert_eq!(tier_to_phase("design"), Some("design"));
        assert_eq!(tier_to_phase("source"), Some("pre-build"));
        assert_eq!(tier_to_phase("build"), Some("build"));
        assert_eq!(tier_to_phase("deployed"), Some("operations"));
        assert_eq!(tier_to_phase("analyzed"), Some("post-build"));
    }

    #[test]
    fn tier_to_phase_returns_none_for_unknown() {
        assert_eq!(tier_to_phase("unknown-tier"), None);
        assert_eq!(tier_to_phase(""), None);
    }
}
