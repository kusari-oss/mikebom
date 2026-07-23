//! FR-003 tie-break rule — pure-function implementation per
//! `contracts/tie-break-rule.md`.
//!
//! Given a set of cross-ecosystem candidate matches for a single
//! bare dep name, decide how many DEPENDS_ON edges to emit and
//! which annotation(s) each carries:
//!
//! - **Exactly one candidate**: emit one edge with only C137
//!   (`waybill:cross-ecosystem-inference`); no ambiguity.
//! - **Multiple candidates but sibling-ecosystem tie-break narrows
//!   to exactly one**: emit that one edge with only C137.
//! - **Multiple candidates AND tie-break did NOT narrow to exactly
//!   one** (0 sibling matches OR 2+ sibling matches): emit ALL
//!   candidate edges, each carrying BOTH C137 AND C138
//!   (`waybill:cross-ecosystem-inference-ambiguous`). Consumers
//!   filter; waybill does not silently pick a single winner.

use std::collections::HashSet;

use super::{
    AlternateMatch, CrossEcosystemInferenceAmbiguousPayload,
    CrossEcosystemInferencePayload,
};

/// One resolved emission decision from the tie-break rule.
/// The caller pushes each variant's Relationship and annotation
/// payloads into the scan-scoped [`super::CrossEcosystemEdgesReport`].
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum EdgeEmission {
    /// Single-match / single-sibling-match resolution. Emit one
    /// edge with C137 only.
    Resolved(String, CrossEcosystemInferencePayload),
    /// Multi-match resolution where tie-break did not narrow to a
    /// single sibling. Emit this edge with C137 AND C138.
    Ambiguous(
        String,
        CrossEcosystemInferencePayload,
        CrossEcosystemInferenceAmbiguousPayload,
    ),
}

/// Given a `dep_name` that failed the same-ecosystem lookup for a
/// `pkg:generic/`-source main-module, decide which edges to emit.
///
/// `candidate_matches`: `(to_eco, target_purl)` pairs from the
/// resolver's cross-ecosystem search. Caller MUST have sorted this
/// slice lex by `(to_eco, target_purl)` before calling — the sort
/// is a determinism guarantee shared with parity-extractor
/// canonicalization.
///
/// `sibling_ecosystems`: precomputed set of ecosystems appearing in
/// the scan's non-generic main-modules (data-model E7). Passed by
/// reference; caller precomputes once per scan.
///
/// `lookup_via`: reader-registered identifier of the m216-alike
/// path that produced the source main-module. For m216 gem apps
/// this is `"gemfile-lock-dependencies"`.
///
/// Returns a `Vec<EdgeEmission>` — one entry per edge to emit.
/// Empty candidates panics (see doc-test note): the caller MUST
/// short-circuit to the FR-004 unresolved path when the resolver's
/// cross-ecosystem search returns zero matches, so this function is
/// never invoked with an empty vec.
pub fn resolve_cross_ecosystem(
    _dep_name: &str,
    _source_purl: &str,
    candidate_matches: Vec<(String, String)>,
    sibling_ecosystems: &HashSet<String>,
    lookup_via: &str,
) -> Vec<EdgeEmission> {
    debug_assert!(
        !candidate_matches.is_empty(),
        "resolve_cross_ecosystem must not be called with empty \
         candidates — the caller MUST short-circuit to the FR-004 \
         unresolved path when the R2 search returns zero matches",
    );

    // Fast path: exactly one candidate → no ambiguity possible.
    if candidate_matches.len() == 1 {
        // SAFETY: len() == 1 checked immediately above.
        let (to_eco, target_purl) = candidate_matches
            .into_iter()
            .next()
            .expect("len==1 checked above");
        return vec![EdgeEmission::Resolved(
            target_purl.clone(),
            CrossEcosystemInferencePayload {
                from_eco: "generic".to_string(),
                lookup_via: lookup_via.to_string(),
                target_purl,
                to_eco,
            },
        )];
    }

    // Multi-match: try to narrow via sibling-ecosystem intersection.
    let sibling_matches: Vec<(String, String)> = candidate_matches
        .iter()
        .filter(|(to_eco, _)| sibling_ecosystems.contains(to_eco))
        .cloned()
        .collect();

    if sibling_matches.len() == 1 {
        // Tie-break succeeded — single sibling match wins.
        // SAFETY: sibling_matches.len() == 1 checked immediately above.
        let (to_eco, target_purl) = sibling_matches
            .into_iter()
            .next()
            .expect("sibling_matches len==1 checked above");
        return vec![EdgeEmission::Resolved(
            target_purl.clone(),
            CrossEcosystemInferencePayload {
                from_eco: "generic".to_string(),
                lookup_via: lookup_via.to_string(),
                target_purl,
                to_eco,
            },
        )];
    }

    // Tie-break did NOT narrow to exactly one (0 or ≥2 sibling
    // matches). Emit ALL candidate edges per Q1 clarification.
    let alternates_full: Vec<AlternateMatch> = candidate_matches
        .iter()
        .map(|(to_eco, target_purl)| AlternateMatch {
            target_purl: target_purl.clone(),
            to_eco: to_eco.clone(),
        })
        .collect();

    candidate_matches
        .into_iter()
        .map(|(to_eco, target_purl)| {
            let mut alternates: Vec<AlternateMatch> = alternates_full
                .iter()
                .filter(|a| a.target_purl != target_purl)
                .cloned()
                .collect();
            alternates.sort();

            let base = CrossEcosystemInferencePayload {
                from_eco: "generic".to_string(),
                lookup_via: lookup_via.to_string(),
                target_purl: target_purl.clone(),
                to_eco: to_eco.clone(),
            };
            let ambiguous = CrossEcosystemInferenceAmbiguousPayload {
                alternates,
                from_eco: base.from_eco.clone(),
                lookup_via: base.lookup_via.clone(),
                target_purl: base.target_purl.clone(),
                to_eco: base.to_eco.clone(),
            };
            EdgeEmission::Ambiguous(target_purl, base, ambiguous)
        })
        .collect()
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    fn siblings(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn cand(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(e, p)| (e.to_string(), p.to_string()))
            .collect()
    }

    #[test]
    fn single_candidate_fast_path_resolves() {
        let out = resolve_cross_ecosystem(
            "fastlane",
            "pkg:generic/app@0",
            cand(&[("gem", "pkg:gem/fastlane@2.220.0")]),
            &siblings(&["gem"]),
            "gemfile-lock-dependencies",
        );
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], EdgeEmission::Resolved(_, _)));
    }

    #[test]
    fn single_candidate_with_no_siblings_still_resolves() {
        let out = resolve_cross_ecosystem(
            "fastlane",
            "pkg:generic/app@0",
            cand(&[("gem", "pkg:gem/fastlane@2.220.0")]),
            &siblings(&[]),
            "gemfile-lock-dependencies",
        );
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], EdgeEmission::Resolved(_, _)));
    }

    #[test]
    fn multi_candidate_one_sibling_match_narrows() {
        let out = resolve_cross_ecosystem(
            "json",
            "pkg:generic/app@0",
            cand(&[
                ("gem", "pkg:gem/json@2.7.1"),
                ("npm", "pkg:npm/json@1.0.0"),
                ("pypi", "pkg:pypi/json@0.1.1"),
            ]),
            &siblings(&["gem"]),
            "gemfile-lock-dependencies",
        );
        assert_eq!(out.len(), 1);
        match &out[0] {
            EdgeEmission::Resolved(target, payload) => {
                assert_eq!(target, "pkg:gem/json@2.7.1");
                assert_eq!(payload.to_eco, "gem");
            }
            EdgeEmission::Ambiguous(..) => panic!("expected Resolved"),
        }
    }

    #[test]
    fn multi_candidate_two_sibling_matches_fans_out() {
        let out = resolve_cross_ecosystem(
            "json",
            "pkg:generic/app@0",
            cand(&[
                ("gem", "pkg:gem/json@2.7.1"),
                ("npm", "pkg:npm/json@1.0.0"),
                ("pypi", "pkg:pypi/json@0.1.1"),
            ]),
            &siblings(&["gem", "npm"]),
            "gemfile-lock-dependencies",
        );
        // Both siblings AND pypi — sibling intersection has 2 matches,
        // so tie-break does not narrow. Emit ALL 3 candidates.
        assert_eq!(out.len(), 3);
        for e in &out {
            assert!(matches!(e, EdgeEmission::Ambiguous(..)));
        }
    }

    #[test]
    fn multi_candidate_zero_sibling_matches_fans_out() {
        let out = resolve_cross_ecosystem(
            "json",
            "pkg:generic/app@0",
            cand(&[
                ("gem", "pkg:gem/json@2.7.1"),
                ("npm", "pkg:npm/json@1.0.0"),
                ("pypi", "pkg:pypi/json@0.1.1"),
            ]),
            &siblings(&[]),
            "gemfile-lock-dependencies",
        );
        assert_eq!(out.len(), 3);
        for e in &out {
            assert!(matches!(e, EdgeEmission::Ambiguous(..)));
        }
    }

    #[test]
    fn ambiguous_alternates_exclude_self() {
        let out = resolve_cross_ecosystem(
            "json",
            "pkg:generic/app@0",
            cand(&[
                ("gem", "pkg:gem/json@2.7.1"),
                ("npm", "pkg:npm/json@1.0.0"),
            ]),
            &siblings(&[]),
            "gemfile-lock-dependencies",
        );
        assert_eq!(out.len(), 2);
        for e in &out {
            match e {
                EdgeEmission::Ambiguous(target, _, ambig) => {
                    assert_eq!(
                        ambig.alternates.len(),
                        1,
                        "ambiguous edge for {target} must have exactly 1 alternate"
                    );
                    assert!(
                        ambig.alternates.iter().all(|a| &a.target_purl != target),
                        "alternates MUST NOT include self"
                    );
                }
                EdgeEmission::Resolved(..) => panic!("expected Ambiguous"),
            }
        }
    }

    #[test]
    #[should_panic(expected = "must not be called with empty candidates")]
    fn empty_candidates_panics_via_debug_assert() {
        // Documents the contract: the caller MUST short-circuit to
        // the FR-004 unresolved path before invoking this function
        // with an empty candidate list. debug_assert! traps in tests
        // (release builds hit the fast path which returns an empty
        // Vec, harmlessly).
        let _ = resolve_cross_ecosystem(
            "nonexistent",
            "pkg:generic/app@0",
            vec![],
            &siblings(&[]),
            "gemfile-lock-dependencies",
        );
    }
}
