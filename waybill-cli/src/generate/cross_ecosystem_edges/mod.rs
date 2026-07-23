//! Milestone 218 (closes waybill#633): cross-ecosystem dep-name edge
//! resolution — payload types + scan-scoped report.
//!
//! Under the FR-000 `--experimental-cross-ecosystem-edges` flag, when
//! the graph-dep-name resolver at `waybill-cli/src/scan_fs/mod.rs:794`
//! encounters a `pkg:generic/`-source main-module whose bare dep name
//! doesn't resolve within the `generic` ecosystem, it iterates every
//! other ecosystem present in the resolver's `name_to_purl` index and
//! bridges the lookup — emitting a DEPENDS_ON edge with the payload
//! types defined here.
//!
//! Per spec Q3 clarification: canonical wire form for every payload
//! is `serde_json::to_string(&payload)` on structs whose fields are
//! declared in alphabetic order (serde emits fields in declaration
//! order). No runtime sort step. Matches the m134 `DivergenceRecord`
//! canonicalization contract.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub mod normalize;
pub mod tie_break;

/// Per-edge base payload for the `waybill:cross-ecosystem-inference`
/// annotation (data-model E1).
///
/// Emitted on every DEPENDS_ON edge produced by the FR-001 fallback.
/// Fields declared in alphabetic order so `serde_json::to_string`
/// produces canonical bytes without an explicit sort step.
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct CrossEcosystemInferencePayload {
    /// PURL type identifier of the source main-module's ecosystem.
    /// At v1 this is always `"generic"` (only ecosystem that triggers
    /// FR-001).
    pub from_eco: String,
    /// Stable machine-readable identifier of the reader path that
    /// produced the source main-module. m216 registers
    /// `"gemfile-lock-dependencies"`. Future m216-alike readers
    /// register their own identifiers.
    pub lookup_via: String,
    /// PURL of the target component the cross-ecosystem lookup
    /// resolved to. Purl-spec conformant.
    pub target_purl: String,
    /// PURL type identifier of the target component's ecosystem.
    /// e.g. `"gem"`, `"pypi"`, `"npm"`, `"cargo"`, `"golang"`.
    pub to_eco: String,
}

/// Sibling record for the ambiguous-variant payload's `alternates[]`
/// field (data-model E2). Sorted lex by `target_purl` for
/// byte-identity.
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct AlternateMatch {
    pub target_purl: String,
    pub to_eco: String,
}

/// Per-edge variant payload for the
/// `waybill:cross-ecosystem-inference-ambiguous` annotation
/// (data-model E2). Extends the base payload with an `alternates[]`
/// field enumerating sibling matches that were ALSO considered but
/// couldn't be narrowed by the FR-003 tie-break rule.
///
/// Every edge annotated with this variant ALSO carries the base
/// `waybill:cross-ecosystem-inference` annotation (Invariant 1 per
/// contracts/annotation-payloads.md).
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct CrossEcosystemInferenceAmbiguousPayload {
    /// Sibling matches (excludes self). Sorted lex by `target_purl`.
    /// `alternates.len() >= 1` when this variant is emitted.
    pub alternates: Vec<AlternateMatch>,
    pub from_eco: String,
    pub lookup_via: String,
    pub target_purl: String,
    pub to_eco: String,
}

/// Element of the document-scope
/// `waybill:cross-ecosystem-inference-unresolved` annotation
/// (data-model E3). Recorded when a `pkg:generic/`-source
/// main-module's `depends[]` entry matches no component in ANY
/// ecosystem's resolver index.
///
/// Doc annotation value is `Vec<Self>` serialized to canonical JSON,
/// sorted lex by `(source_purl, unresolved_name)`.
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct CrossEcosystemInferenceUnresolvedRecord {
    pub source_purl: String,
    pub unresolved_name: String,
}

/// Scan-scoped aggregate threaded through `ScanArtifacts` per the
/// m134/m173/m204/m217 propagation pattern (data-model E4).
///
/// - `crossed_edges` — every crossed edge emitted this scan, keyed
///   by `(source_purl, target_purl)`.
/// - `ambiguous_edges` — subset of `crossed_edges` that additionally
///   carry the C138 ambiguous annotation.
/// - `unresolved` — sorted vec of unresolved records for the C139
///   doc-scope annotation. Absent from the SBOM when empty per
///   FR-011.
/// - `summary` — three counters consumed by the FR-013 INFO log.
///
/// When the FR-000 flag is OFF, this report is `None` on
/// `ScanArtifacts`. When the flag is ON but the scan contains no
/// `pkg:generic/` main-modules, the report is `Some(default())` —
/// FR-008 guarantees byte-identity anyway because no cross-eco
/// lookups fire.
#[derive(Debug, Default, Clone)]
pub struct CrossEcosystemEdgesReport {
    pub crossed_edges: BTreeMap<(String, String), CrossEcosystemInferencePayload>,
    pub ambiguous_edges:
        BTreeMap<(String, String), CrossEcosystemInferenceAmbiguousPayload>,
    pub unresolved: Vec<CrossEcosystemInferenceUnresolvedRecord>,
    pub summary: CrossEcosystemEdgesSummary,
}

/// Per-scan counters for the FR-013 INFO summary log (data-model
/// E4). Populated at resolver-exit time by counting the report's
/// three collections.
#[derive(Debug, Default, Clone, Copy)]
pub struct CrossEcosystemEdgesSummary {
    pub edges_resolved: usize,
    pub edges_ambiguous: usize,
    pub names_unresolved: usize,
}

impl CrossEcosystemEdgesReport {
    /// Convenience: recompute `summary` from the three collections.
    /// Called at resolver-exit before threading the report into
    /// `ScanArtifacts`.
    pub fn recompute_summary(&mut self) {
        self.summary = CrossEcosystemEdgesSummary {
            edges_resolved: self
                .crossed_edges
                .len()
                .saturating_sub(self.ambiguous_edges.len()),
            edges_ambiguous: self.ambiguous_edges.len(),
            names_unresolved: self.unresolved.len(),
        };
    }
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    #[test]
    fn payload_serializes_alphabetically() {
        let p = CrossEcosystemInferencePayload {
            from_eco: "generic".into(),
            lookup_via: "gemfile-lock-dependencies".into(),
            target_purl: "pkg:gem/fastlane@2.220.0".into(),
            to_eco: "gem".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(
            json,
            r#"{"from_eco":"generic","lookup_via":"gemfile-lock-dependencies","target_purl":"pkg:gem/fastlane@2.220.0","to_eco":"gem"}"#
        );
    }

    #[test]
    fn ambiguous_payload_places_alternates_first() {
        let p = CrossEcosystemInferenceAmbiguousPayload {
            alternates: vec![
                AlternateMatch {
                    target_purl: "pkg:npm/json@1.0.0".into(),
                    to_eco: "npm".into(),
                },
                AlternateMatch {
                    target_purl: "pkg:pypi/json@0.1.1".into(),
                    to_eco: "pypi".into(),
                },
            ],
            from_eco: "generic".into(),
            lookup_via: "gemfile-lock-dependencies".into(),
            target_purl: "pkg:gem/json@2.7.1".into(),
            to_eco: "gem".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.starts_with(r#"{"alternates":["#));
        assert!(json.contains(r#""from_eco":"generic""#));
        assert!(json.contains(r#""to_eco":"gem""#));
    }

    #[test]
    fn recompute_summary_derives_from_collections() {
        let mut r = CrossEcosystemEdgesReport::default();
        r.crossed_edges.insert(
            ("s".into(), "t1".into()),
            CrossEcosystemInferencePayload {
                from_eco: "generic".into(),
                lookup_via: "x".into(),
                target_purl: "t1".into(),
                to_eco: "gem".into(),
            },
        );
        r.crossed_edges.insert(
            ("s".into(), "t2".into()),
            CrossEcosystemInferencePayload {
                from_eco: "generic".into(),
                lookup_via: "x".into(),
                target_purl: "t2".into(),
                to_eco: "gem".into(),
            },
        );
        r.ambiguous_edges.insert(
            ("s".into(), "t2".into()),
            CrossEcosystemInferenceAmbiguousPayload {
                alternates: vec![AlternateMatch {
                    target_purl: "t3".into(),
                    to_eco: "npm".into(),
                }],
                from_eco: "generic".into(),
                lookup_via: "x".into(),
                target_purl: "t2".into(),
                to_eco: "gem".into(),
            },
        );
        r.unresolved.push(CrossEcosystemInferenceUnresolvedRecord {
            source_purl: "s".into(),
            unresolved_name: "missing".into(),
        });
        r.recompute_summary();
        assert_eq!(r.summary.edges_resolved, 1);
        assert_eq!(r.summary.edges_ambiguous, 1);
        assert_eq!(r.summary.names_unresolved, 1);
    }
}
