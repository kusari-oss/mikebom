//! Milestone 215 — `split-manifest.json` on-wire schema.
//!
//! Operator-facing artifact emitted alongside sub-SBOMs when `--split`
//! is passed. Describes the split so downstream tooling can reason
//! about the emitted file set as a whole.
//!
//! See `specs/215-sbom-auto-split/contracts/split-manifest-schema.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Stable v1 schema URL. Bumps to `v2` on breaking schema changes.
pub const SPLIT_MANIFEST_SCHEMA_V1: &str =
    "https://waybill.dev/schema/split-manifest/v1.json";

/// Top-level manifest document. One per `--split` invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SplitManifest {
    #[serde(rename = "$schema")]
    pub schema_url: String,
    pub waybill_version: String,
    pub scan_root: String,
    pub generated_at: String,
    pub total_unique_components: u64,
    pub shared_dep_count: u64,
    pub entries: Vec<SplitEntry>,
}

/// One entry per detected workspace member.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SplitEntry {
    pub subproject_id: String,
    pub root_purl: String,
    pub source_dir: String,
    pub component_count: u64,
    pub shared_deps_count: u64,
    /// Format-id → relative-filename (e.g. `"cyclonedx-json" → "libsafe.cargo.cdx.json"`).
    /// `BTreeMap` guarantees deterministic key ordering across runs.
    pub files: BTreeMap<String, String>,
    /// Milestone 219 — additive-optional field. OMITTED (via
    /// `skip_serializing_if`) when the group covers exactly one
    /// main-module — preserves m215 wire-shape byte-identity per
    /// SC-005. PRESENT (sorted lex by `purl`) when the group covers
    /// ≥2 members (only possible under `--split=directory` today).
    /// See `specs/219-split-modes/contracts/manifest-additive-members.md`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub members: Option<Vec<SplitMember>>,
}

/// Milestone 219 — one contributing main-module in a multi-member
/// group. Populates `SplitEntry.members[]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SplitMember {
    /// PURL string of the contributing main-module. Purl-spec conformant.
    pub purl: String,
    /// Source directory of that main-module (relative to scan root).
    /// For multi-member groups under `--split=directory`, every
    /// member's source_dir is IDENTICAL to the group's source_dir;
    /// preserved per-member for consistency with future non-directory
    /// grouping modes (e.g., `--split=ecosystem`).
    pub source_dir: String,
}

impl SplitManifest {
    pub fn new(
        waybill_version: String,
        scan_root: String,
        generated_at: String,
    ) -> Self {
        Self {
            schema_url: SPLIT_MANIFEST_SCHEMA_V1.to_string(),
            waybill_version,
            scan_root,
            generated_at,
            total_unique_components: 0,
            shared_dep_count: 0,
            entries: Vec::new(),
        }
    }
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    fn sample_entry(id: &str, purl: &str) -> SplitEntry {
        let mut files = BTreeMap::new();
        files.insert(
            "cyclonedx-json".to_string(),
            format!("{id}.cdx.json"),
        );
        SplitEntry {
            subproject_id: id.to_string(),
            root_purl: purl.to_string(),
            source_dir: "crates/foo".to_string(),
            component_count: 42,
            shared_deps_count: 3,
            files,
            members: None,
        }
    }

    #[test]
    fn schema_url_matches_v1_contract() {
        let m = SplitManifest::new(
            "0.1.0".to_string(),
            "/tmp".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
        );
        assert_eq!(m.schema_url, SPLIT_MANIFEST_SCHEMA_V1);
        assert_eq!(
            m.schema_url,
            "https://waybill.dev/schema/split-manifest/v1.json"
        );
    }

    #[test]
    fn round_trip_serde_preserves_all_fields() {
        let mut m = SplitManifest::new(
            "0.1.0-alpha.66".to_string(),
            "/repo".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
        );
        m.total_unique_components = 100;
        m.shared_dep_count = 5;
        m.entries.push(sample_entry("libsafe.cargo", "pkg:cargo/libsafe@0.1.0"));

        let json = serde_json::to_string_pretty(&m).unwrap();
        let back: SplitManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn empty_entries_serializes_as_array_not_null() {
        let m = SplitManifest::new(
            "0.1.0".to_string(),
            "/tmp".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
        );
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"entries\":[]"));
        assert!(!json.contains("\"entries\":null"));
    }

    #[test]
    fn files_map_key_order_deterministic() {
        // Insert in arbitrary order; BTreeMap serializes sorted.
        let mut files = BTreeMap::new();
        files.insert("spdx-3-json".to_string(), "foo.spdx3.json".to_string());
        files.insert("cyclonedx-json".to_string(), "foo.cdx.json".to_string());
        files.insert("spdx-2.3-json".to_string(), "foo.spdx.json".to_string());
        let entry = SplitEntry {
            subproject_id: "foo.cargo".to_string(),
            root_purl: "pkg:cargo/foo@1.0.0".to_string(),
            source_dir: String::new(),
            component_count: 1,
            shared_deps_count: 0,
            files,
            members: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        // Alphabetical: cyclonedx-json < spdx-2.3-json < spdx-3-json.
        let cdx_pos = json.find("cyclonedx-json").unwrap();
        let spdx23_pos = json.find("spdx-2.3-json").unwrap();
        let spdx3_pos = json.find("spdx-3-json").unwrap();
        assert!(cdx_pos < spdx23_pos);
        assert!(spdx23_pos < spdx3_pos);
    }

    /// Milestone 219 — SC-005 byte-identity: single-member entries
    /// (`members: None`) MUST omit the field entirely from wire JSON,
    /// preserving m215 wire shape verbatim.
    #[test]
    fn m219_members_none_omitted_from_wire() {
        let entry = sample_entry("libsafe.cargo", "pkg:cargo/libsafe@0.1.0");
        assert!(entry.members.is_none());
        let json = serde_json::to_string(&entry).unwrap();
        assert!(
            !json.contains("\"members\""),
            "single-member entry MUST NOT emit `members` field; got: {json}"
        );
    }

    /// Milestone 219 — multi-member entries include a `members[]`
    /// array. Wire shape is JSON-canonical.
    #[test]
    fn m219_members_some_includes_sorted_array() {
        let mut entry = sample_entry("services-api.multi", "pkg:generic/services-api@0.0.0-unknown");
        entry.members = Some(vec![
            SplitMember {
                purl: "pkg:cargo/api@0.1.0".to_string(),
                source_dir: "services/api".to_string(),
            },
            SplitMember {
                purl: "pkg:npm/api@0.1.0".to_string(),
                source_dir: "services/api".to_string(),
            },
        ]);
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"members\""));
        assert!(json.contains("\"pkg:cargo/api@0.1.0\""));
        assert!(json.contains("\"pkg:npm/api@0.1.0\""));
    }

    /// Milestone 219 — m215 payloads (no `members` field) round-trip
    /// through m219 `SplitEntry` with zero data loss (per
    /// contracts/manifest-additive-members.md deserialize contract).
    #[test]
    fn m219_m215_payload_deserializes_cleanly() {
        let m215_json = r#"{"subproject_id":"libsafe.cargo","root_purl":"pkg:cargo/libsafe@0.1.0","source_dir":"crates/libsafe","component_count":42,"shared_deps_count":3,"files":{"cyclonedx-json":"libsafe.cargo.cdx.json"}}"#;
        let entry: SplitEntry = serde_json::from_str(m215_json).unwrap();
        assert!(entry.members.is_none());
        // Re-serialize; MUST be byte-identical to the input.
        let round_trip = serde_json::to_string(&entry).unwrap();
        assert_eq!(round_trip, m215_json);
    }

    #[test]
    fn dollar_sign_schema_field_serializes_correctly() {
        let m = SplitManifest::new(
            "0.1.0".to_string(),
            "/tmp".to_string(),
            "2026-01-01T00:00:00Z".to_string(),
        );
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("\"$schema\""));
    }
}
