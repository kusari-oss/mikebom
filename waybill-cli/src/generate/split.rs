//! Milestone 215 — SBOM auto-split by workspace member.
//!
//! Post-resolve, pre-emit fan-out. When `--split` is passed to
//! `waybill sbom scan`, enumerate detected workspace-root components,
//! BFS-project each into its own reachable dep-graph subset, and
//! emit one SBOM per subproject (× each requested `--format`)
//! plus a sibling `split-manifest.json`.
//!
//! Boundary-enumeration signal: `waybill:is-workspace-root` annotation
//! set by the m127 root selector + m201 disambiguation. Reused
//! verbatim — no new detection logic.
//!
//! See `specs/215-sbom-auto-split/` for spec / plan / research.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use data_encoding::HEXLOWER;
use serde_json::Value;
use sha2::{Digest, Sha256};

use waybill_common::resolution::{Relationship, ResolvedComponent};
use waybill_common::types::purl::Purl;

use super::root_selector::IS_WORKSPACE_ROOT_KEY;

/// One detected workspace-root that becomes the axis for one sub-SBOM.
#[derive(Debug, Clone)]
pub(crate) struct SubprojectRoot {
    /// Canonical PURL identifying the subproject. Drives filename slug,
    /// manifest `subproject_id`, and serves as the BFS seed.
    pub purl: Purl,
    /// PURL string as it appears in `Relationship.from`/`.to` for graph
    /// traversal — Waybill's `Relationship` type keys on PURL strings,
    /// not bom-refs.
    pub purl_string: String,
    /// Subproject source directory relative to scan_root (empty when
    /// the subproject IS the scan root).
    pub source_dir: PathBuf,
    /// Ecosystem name (`cargo`, `npm`, `pypi`, …). Appears in filename.
    pub ecosystem: String,
}

impl SubprojectRoot {
    /// `<slug>.<ecosystem>` — matches the emitted-filename prefix and
    /// the manifest `subproject_id`. Deterministic function of the PURL.
    pub fn subproject_id(&self) -> String {
        format!("{}.{}", subject_slug(&self.purl), self.ecosystem)
    }
}

/// One BFS-projected subset — the components + relationships that end
/// up in a single sub-SBOM.
#[derive(Debug)]
pub(crate) struct SplitProjection {
    pub root: SubprojectRoot,
    pub components: Vec<ResolvedComponent>,
    pub relationships: Vec<Relationship>,
    /// Count of THIS projection's components that also appear in ≥ 1
    /// sibling projection. Populated post-hoc by [`compute_shared_deps`].
    pub shared_deps_count: usize,
}

// ---------- T008: enumerate_workspace_roots ----------

/// Return every workspace-root component projected into a
/// [`SubprojectRoot`], sorted lexicographically by `subproject_id` for
/// deterministic emit order.
///
/// Filters out any component whose PURL name is empty (m127's synthetic
/// placeholder path); those aren't split axes per research R1.
pub(crate) fn enumerate_workspace_roots(
    resolved_components: &[ResolvedComponent],
    scan_root: &std::path::Path,
) -> Vec<SubprojectRoot> {
    let mut roots: Vec<SubprojectRoot> = resolved_components
        .iter()
        .filter(|c| is_workspace_root(c))
        .filter(|c| !c.purl.name().is_empty())
        .map(|c| SubprojectRoot {
            purl: c.purl.clone(),
            purl_string: c.purl.to_string(),
            source_dir: source_dir_for(c, scan_root),
            ecosystem: c.purl.ecosystem().to_string(),
        })
        .collect();

    roots.sort_by(|a, b| a.subproject_id().cmp(&b.subproject_id()));
    roots
}

fn is_workspace_root(c: &ResolvedComponent) -> bool {
    matches!(
        c.extra_annotations.get(IS_WORKSPACE_ROOT_KEY),
        Some(Value::Bool(true))
    )
}

fn source_dir_for(
    c: &ResolvedComponent,
    scan_root: &std::path::Path,
) -> PathBuf {
    // Use the first evidence source_file_paths entry as the anchor;
    // strip the scan_root prefix + the manifest basename to get the
    // subproject directory.
    let raw = c.evidence.source_file_paths.first();
    let Some(raw) = raw else {
        return PathBuf::new();
    };
    let abs = PathBuf::from(raw);
    let rel = abs.strip_prefix(scan_root).unwrap_or(&abs);
    rel.parent().map(PathBuf::from).unwrap_or_default()
}

// ---------- T009: project_for_root (BFS) ----------

/// BFS from the root's PURL over dep-edge relationships. Returns the
/// reachable component set (including root) + all relationships whose
/// both endpoints are in that set (self-contained per FR-007).
pub(crate) fn project_for_root(
    root: &SubprojectRoot,
    all_components: &[ResolvedComponent],
    all_relationships: &[Relationship],
) -> SplitProjection {
    // Pre-build a `from → [to, ...]` adjacency map so BFS is O(V + E)
    // instead of O(V × E) with linear scans per node.
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for rel in all_relationships {
        if is_dep_edge(&rel.relationship_type) {
            adjacency
                .entry(rel.from.as_str())
                .or_default()
                .push(rel.to.as_str());
        }
    }

    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    reached.insert(root.purl_string.clone());
    queue.push_back(root.purl_string.clone());

    while let Some(cur) = queue.pop_front() {
        if let Some(next) = adjacency.get(cur.as_str()) {
            for &to in next {
                if reached.insert(to.to_string()) {
                    queue.push_back(to.to_string());
                }
            }
        }
    }

    // Preserve `all_components` ordering; place root first if present.
    let root_component = all_components
        .iter()
        .find(|c| c.purl.to_string() == root.purl_string)
        .cloned();

    let mut components: Vec<ResolvedComponent> = Vec::new();
    if let Some(rc) = root_component {
        components.push(rc);
    }
    for c in all_components {
        let s = c.purl.to_string();
        if s == root.purl_string {
            continue;
        }
        if reached.contains(&s) {
            components.push(c.clone());
        }
    }

    let relationships = all_relationships
        .iter()
        .filter(|r| reached.contains(&r.from) && reached.contains(&r.to))
        .cloned()
        .collect();

    SplitProjection {
        root: root.clone(),
        components,
        relationships,
        shared_deps_count: 0,
    }
}

fn is_dep_edge(kind: &waybill_common::resolution::RelationshipType) -> bool {
    use waybill_common::resolution::RelationshipType::*;
    matches!(
        kind,
        DependsOn
            | DevDependsOn
            | BuildDependsOn
            | TestDependsOn
            | OptionalDependsOn
    )
}

// ---------- T010: compute_shared_deps ----------

/// After all N projections exist, walk the union of every projection's
/// components. A PURL that appears in ≥ 2 projections is a shared dep.
/// Populate each projection's `shared_deps_count` (the count of ITS
/// components that overlap with ≥ 1 sibling) and return
/// `(total_unique_components, aggregate_shared_dep_count)` for manifest
/// document-level aggregates.
pub(crate) fn compute_shared_deps(
    projections: &mut [SplitProjection],
) -> (u64, u64) {
    // Count how many projections each PURL appears in.
    let mut occurrences: HashMap<String, usize> = HashMap::new();
    for p in projections.iter() {
        // Use HashSet per-projection so a self-repeat doesn't inflate.
        let mut seen: HashSet<String> = HashSet::new();
        for c in &p.components {
            seen.insert(c.purl.to_string());
        }
        for s in seen {
            *occurrences.entry(s).or_default() += 1;
        }
    }

    // Per-projection: count its components whose PURL appears in ≥ 2.
    for p in projections.iter_mut() {
        let mut n: usize = 0;
        let mut seen: HashSet<String> = HashSet::new();
        for c in &p.components {
            let s = c.purl.to_string();
            if seen.insert(s.clone()) {
                if occurrences.get(&s).copied().unwrap_or(0) >= 2 {
                    n += 1;
                }
            }
        }
        p.shared_deps_count = n;
    }

    let total_unique = occurrences.len() as u64;
    let shared_agg = occurrences.values().filter(|&&n| n >= 2).count() as u64;
    (total_unique, shared_agg)
}

// ---------- T011: filename_for + slug helpers ----------

/// Format-id → extension token (`cyclonedx-json` → `cdx`).
pub(crate) fn format_ext(format_id: &str) -> &'static str {
    match format_id {
        "cyclonedx-json" => "cdx",
        "spdx-2.3-json" => "spdx",
        "spdx-3-json" | "spdx-3-json-experimental" => "spdx3",
        _ => "sbom", // permissive fallback for unknown future formats
    }
}

/// Reserved-on-Windows base names (case-insensitive comparison).
const WINDOWS_RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul",
    "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9",
    "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Filenames the manifest itself owns — never produce one for a
/// sub-SBOM.
const RESERVED_SUB_SBOM_NAMES: &[&str] = &[
    "split-manifest.json",
    ".gitkeep",
    ".gitignore",
];

/// PURL → filesystem-safe slug per contracts/filename-convention.md.
///
/// Prefixes namespace when present (`@myorg/frontend` → `myorg-frontend`,
/// `com.example/my-lib` → `com.example-my-lib`), substitutes/strip
/// unsafe chars, truncates to 100 bytes, lowercases.
pub(crate) fn subject_slug(purl: &Purl) -> String {
    let mut s = if let Some(ns) = purl.namespace() {
        format!("{}-{}", ns, purl.name())
    } else {
        purl.name().to_string()
    };

    // 1) Character substitutions.
    s = s.replace('/', "-").replace('@', "at-");
    // 2) Strip URL/path-unsafe chars (backslash, colon, glob, wildcards,
    //    quotes, angle brackets, pipe). Whitespace also stripped.
    s.retain(|c| {
        !matches!(
            c,
            '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | ' ' | '\t' | '\n' | '\r'
        )
    });
    // 3) Non-ASCII → strip entirely (defensive; PURLs shouldn't carry
    //    non-ASCII but be conservative for cross-filesystem safety).
    s.retain(|c| c.is_ascii());
    // 4) Truncate to 100 chars.
    if s.len() > 100 {
        s.truncate(100);
    }
    // 5) Lowercase.
    s.to_ascii_lowercase()
}

/// Build the final filename `<slug>.<ecosystem>.<format-ext>.json` with
/// collision + reserved-name handling.
///
/// `collision_map` maps `subproject_id` → list of source_dirs of every
/// root sharing that id. When any list has len > 1, this root gets a
/// `-<8hex>` suffix on the slug (deterministic hash of its own source_dir).
pub(crate) fn filename_for(
    root: &SubprojectRoot,
    format_id: &str,
    collision_map: &BTreeMap<String, Vec<PathBuf>>,
) -> String {
    let mut slug = subject_slug(&root.purl);
    let ecosystem = &root.ecosystem;

    // Collision fallback: append SHA-8 of source_dir when the base
    // subproject_id collides with a sibling.
    let base_id = format!("{slug}.{ecosystem}");
    let colliding = collision_map
        .get(&base_id)
        .map(|paths| paths.len() > 1)
        .unwrap_or(false);
    if colliding {
        let hash = sha8_hex(&root.source_dir.to_string_lossy());
        slug = format!("{slug}-{hash}");
    }

    // Reserved-Windows-basename guard: prefix `wb-` if the slug matches
    // (case-insensitively) a reserved DOS device name.
    if WINDOWS_RESERVED
        .iter()
        .any(|w| w.eq_ignore_ascii_case(&slug))
    {
        slug = format!("wb-{slug}");
    }

    let ext = format_ext(format_id);
    let mut filename = format!("{slug}.{ecosystem}.{ext}.json");

    // Manifest-name collision guard: if the emitted name would clash
    // with a reserved sub-SBOM name (`split-manifest.json` etc.),
    // hash-suffix.
    if RESERVED_SUB_SBOM_NAMES.contains(&filename.as_str()) {
        let hash = sha8_hex(&root.source_dir.to_string_lossy());
        filename = format!("{slug}-{hash}.{ecosystem}.{ext}.json");
    }

    filename
}

fn sha8_hex(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let digest = hasher.finalize();
    HEXLOWER.encode(&digest[..4])
}

/// Build a collision map: `subproject_id` → list of source_dirs that
/// produce that same id. Used by [`filename_for`] to disambiguate.
pub(crate) fn build_collision_map(
    roots: &[SubprojectRoot],
) -> BTreeMap<String, Vec<PathBuf>> {
    let mut m: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for r in roots {
        m.entry(r.subproject_id()).or_default().push(r.source_dir.clone());
    }
    m
}

// ---------- Deterministic sub-SBOM serial (R5) ----------

/// Under `WAYBILL_FIXED_TIMESTAMP`, sub-SBOM serial becomes a
/// deterministic hash of the root PURL + fixed timestamp. Otherwise,
/// a fresh random UUIDv4-shaped serial.
pub(crate) fn sub_sbom_serial(
    root_purl: &Purl,
    fixed_ts: Option<&str>,
) -> String {
    match fixed_ts {
        Some(ts) => {
            let mut hasher = Sha256::new();
            hasher.update(root_purl.to_string().as_bytes());
            hasher.update(b"|");
            hasher.update(ts.as_bytes());
            let digest = hasher.finalize();
            // 32 hex chars = 128 bits; UUID-shaped.
            format!("urn:uuid:{}", &HEXLOWER.encode(&digest)[..32])
        }
        None => format!("urn:uuid:{}", uuid_v4_hex_32()),
    }
}

fn uuid_v4_hex_32() -> String {
    // Match Waybill's existing non-deterministic UUID shape without
    // pulling `uuid` crate here — the CDX path already uses `uuid`.
    // We format 32 hex chars from a random source for the split
    // fallback path (non-reproducible mode only).
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut hasher = Sha256::new();
    hasher.update(nanos.to_le_bytes());
    let digest = hasher.finalize();
    HEXLOWER.encode(&digest)[..32].to_string()
}

// ---------- Tests ----------

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;
    use waybill_common::resolution::{
        EnrichmentProvenance, RelationshipType, ResolutionEvidence,
        ResolutionTechnique,
    };

    fn mk_component(purl: &str, ws_root: bool) -> ResolvedComponent {
        let p = Purl::new(purl).unwrap();
        let mut ann = BTreeMap::new();
        if ws_root {
            ann.insert(
                IS_WORKSPACE_ROOT_KEY.to_string(),
                Value::Bool(true),
            );
        }
        ResolvedComponent {
            purl: p.clone(),
            name: p.name().to_string(),
            version: p.version().unwrap_or("").to_string(),
            evidence: ResolutionEvidence {
                technique: ResolutionTechnique::UrlPattern,
                confidence: 1.0,
                source_connection_ids: vec![],
                source_file_paths: vec![],
                deps_dev_match: None,
            },
            licenses: vec![],
            concluded_licenses: Vec::new(),
            hashes: vec![],
            supplier: None,
            cpes: vec![],
            advisories: vec![],
            occurrences: vec![],
            lifecycle_scope: None,
            build_inclusion: None,
            requirement_ranges: Vec::new(),
            source_type: None,
            sbom_tier: None,
            buildinfo_status: None,
            evidence_kind: None,
            binary_class: None,
            binary_stripped: None,
            linkage_kind: None,
            detected_go: None,
            confidence: None,
            binary_packed: None,
            npm_role: None,
            raw_version: None,
            parent_purl: None,
            co_owned_by: None,
            shade_relocation: None,
            external_references: Vec::new(),
            extra_annotations: ann,
            binary_role: None,
        }
    }

    fn mk_rel(from: &str, to: &str) -> Relationship {
        Relationship {
            from: from.to_string(),
            to: to.to_string(),
            relationship_type: RelationshipType::DependsOn,
            provenance: EnrichmentProvenance {
                source: "test".to_string(),
                data_type: "test".to_string(),
            },
        }
    }

    // -------- enumerate_workspace_roots --------

    #[test]
    fn enumerate_filters_on_is_workspace_root_annotation() {
        let comps = vec![
            mk_component("pkg:cargo/libsafe@0.1.0", true),
            mk_component("pkg:cargo/serde@1.0.0", false),
            mk_component("pkg:cargo/libvuln@0.1.0", true),
        ];
        let roots = enumerate_workspace_roots(&comps, std::path::Path::new("/"));
        assert_eq!(roots.len(), 2);
        // Sorted by subproject_id (lex): libsafe.cargo < libvuln.cargo.
        assert_eq!(roots[0].subproject_id(), "libsafe.cargo");
        assert_eq!(roots[1].subproject_id(), "libvuln.cargo");
    }

    #[test]
    fn enumerate_skips_empty_purl_name() {
        // Manually construct a synthetic-placeholder-shaped PURL that
        // parses but has an empty name — we can't; Purl::new rejects.
        // Instead, verify the filter path runs by not tripping on real
        // roots.
        let comps = vec![mk_component("pkg:cargo/x@0.1.0", true)];
        let roots = enumerate_workspace_roots(&comps, std::path::Path::new("/"));
        assert_eq!(roots.len(), 1);
    }

    // -------- project_for_root (BFS) --------

    #[test]
    fn project_bfs_reaches_transitive_closure() {
        let comps = vec![
            mk_component("pkg:cargo/root@0.1.0", true),
            mk_component("pkg:cargo/mid@1.0.0", false),
            mk_component("pkg:cargo/leaf@1.0.0", false),
            mk_component("pkg:cargo/unrelated@1.0.0", false),
        ];
        let rels = vec![
            mk_rel("pkg:cargo/root@0.1.0", "pkg:cargo/mid@1.0.0"),
            mk_rel("pkg:cargo/mid@1.0.0", "pkg:cargo/leaf@1.0.0"),
        ];
        let roots = enumerate_workspace_roots(&comps, std::path::Path::new("/"));
        assert_eq!(roots.len(), 1);
        let proj = project_for_root(&roots[0], &comps, &rels);
        assert_eq!(proj.components.len(), 3, "root + mid + leaf");
        assert_eq!(proj.relationships.len(), 2);
        assert_eq!(proj.components[0].purl.name(), "root", "root component first");
    }

    #[test]
    fn project_excludes_sibling_members() {
        let comps = vec![
            mk_component("pkg:cargo/a@1.0.0", true),
            mk_component("pkg:cargo/b@1.0.0", true),
            mk_component("pkg:cargo/dep-a@1.0.0", false),
            mk_component("pkg:cargo/dep-b@1.0.0", false),
        ];
        let rels = vec![
            mk_rel("pkg:cargo/a@1.0.0", "pkg:cargo/dep-a@1.0.0"),
            mk_rel("pkg:cargo/b@1.0.0", "pkg:cargo/dep-b@1.0.0"),
        ];
        let roots = enumerate_workspace_roots(&comps, std::path::Path::new("/"));
        let proj = project_for_root(&roots[0], &comps, &rels); // "a"
        assert_eq!(proj.components.len(), 2, "a + dep-a only");
        assert!(proj.components.iter().any(|c| c.purl.name() == "a"));
        assert!(proj.components.iter().any(|c| c.purl.name() == "dep-a"));
        assert!(!proj.components.iter().any(|c| c.purl.name() == "b"));
    }

    // -------- compute_shared_deps --------

    #[test]
    fn shared_deps_counts_correctly_across_three_projections() {
        let comps = vec![
            mk_component("pkg:cargo/a@1.0.0", true),
            mk_component("pkg:cargo/b@1.0.0", true),
            mk_component("pkg:cargo/c@1.0.0", true),
            mk_component("pkg:cargo/shared@1.0.0", false),
            mk_component("pkg:cargo/only-a@1.0.0", false),
        ];
        let rels = vec![
            mk_rel("pkg:cargo/a@1.0.0", "pkg:cargo/shared@1.0.0"),
            mk_rel("pkg:cargo/a@1.0.0", "pkg:cargo/only-a@1.0.0"),
            mk_rel("pkg:cargo/b@1.0.0", "pkg:cargo/shared@1.0.0"),
            mk_rel("pkg:cargo/c@1.0.0", "pkg:cargo/shared@1.0.0"),
        ];
        let roots = enumerate_workspace_roots(&comps, std::path::Path::new("/"));
        let mut projections: Vec<SplitProjection> = roots
            .iter()
            .map(|r| project_for_root(r, &comps, &rels))
            .collect();
        let (total, shared) = compute_shared_deps(&mut projections);
        assert_eq!(total, 5, "5 distinct PURLs across all projections");
        assert_eq!(shared, 1, "only `shared` appears in >1 projection");
        // Every projection sees shared → shared_deps_count = 1.
        for p in &projections {
            assert_eq!(
                p.shared_deps_count, 1,
                "projection {} should have shared_deps_count=1",
                p.root.subproject_id()
            );
        }
    }

    // -------- sub_sbom_serial --------

    #[test]
    fn sub_sbom_serial_deterministic_under_fixed_timestamp() {
        let p = Purl::new("pkg:cargo/foo@1.0.0").unwrap();
        let s1 = sub_sbom_serial(&p, Some("2026-01-01T00:00:00Z"));
        let s2 = sub_sbom_serial(&p, Some("2026-01-01T00:00:00Z"));
        assert_eq!(s1, s2, "same input → same serial");
        let s3 = sub_sbom_serial(&p, Some("2026-01-02T00:00:00Z"));
        assert_ne!(s1, s3, "timestamp change → serial change");
    }

    #[test]
    fn sub_sbom_serial_differs_across_purls() {
        let a = Purl::new("pkg:cargo/a@1.0.0").unwrap();
        let b = Purl::new("pkg:cargo/b@1.0.0").unwrap();
        let ts = Some("2026-01-01T00:00:00Z");
        assert_ne!(sub_sbom_serial(&a, ts), sub_sbom_serial(&b, ts));
    }

    // -------- filename_for + slug --------

    #[test]
    fn slug_simple_cargo_package() {
        let p = Purl::new("pkg:cargo/libsafe@0.1.0").unwrap();
        assert_eq!(subject_slug(&p), "libsafe");
    }

    #[test]
    fn slug_prefixes_npm_scope() {
        let p = Purl::new("pkg:npm/%40myorg/frontend@1.0.0").unwrap();
        // namespace() should return "@myorg" (or decoded form); the
        // slug prefixes it with a dash.
        let s = subject_slug(&p);
        assert!(s.ends_with("frontend"), "got {s}");
    }

    #[test]
    fn slug_lowercases() {
        let p = Purl::new("pkg:cargo/FooBar@1.0.0").unwrap();
        assert_eq!(subject_slug(&p), "foobar");
    }

    #[test]
    fn filename_cargo_no_collision() {
        let r = SubprojectRoot {
            purl: Purl::new("pkg:cargo/libsafe@0.1.0").unwrap(),
            purl_string: "pkg:cargo/libsafe@0.1.0".to_string(),
            source_dir: PathBuf::from("libsafe"),
            ecosystem: "cargo".to_string(),
        };
        let cm = BTreeMap::new();
        assert_eq!(
            filename_for(&r, "cyclonedx-json", &cm),
            "libsafe.cargo.cdx.json"
        );
        assert_eq!(
            filename_for(&r, "spdx-2.3-json", &cm),
            "libsafe.cargo.spdx.json"
        );
        assert_eq!(
            filename_for(&r, "spdx-3-json", &cm),
            "libsafe.cargo.spdx3.json"
        );
    }

    #[test]
    fn filename_windows_reserved_prefixes_wb() {
        let r = SubprojectRoot {
            purl: Purl::new("pkg:cargo/con@0.1.0").unwrap(),
            purl_string: "pkg:cargo/con@0.1.0".to_string(),
            source_dir: PathBuf::from("con-crate"),
            ecosystem: "cargo".to_string(),
        };
        let cm = BTreeMap::new();
        assert_eq!(
            filename_for(&r, "cyclonedx-json", &cm),
            "wb-con.cargo.cdx.json"
        );
    }

    #[test]
    fn filename_collision_appends_sha_suffix() {
        let a = SubprojectRoot {
            purl: Purl::new("pkg:cargo/foo@1.0.0").unwrap(),
            purl_string: "pkg:cargo/foo@1.0.0".to_string(),
            source_dir: PathBuf::from("libs/cli/foo"),
            ecosystem: "cargo".to_string(),
        };
        let b = SubprojectRoot {
            purl: Purl::new("pkg:cargo/foo@1.0.0").unwrap(),
            purl_string: "pkg:cargo/foo@1.0.0".to_string(),
            source_dir: PathBuf::from("libs/tools/foo"),
            ecosystem: "cargo".to_string(),
        };
        let cm = build_collision_map(&[a.clone(), b.clone()]);
        let fna = filename_for(&a, "cyclonedx-json", &cm);
        let fnb = filename_for(&b, "cyclonedx-json", &cm);
        assert_ne!(fna, fnb, "collision must yield distinct filenames");
        assert!(fna.starts_with("foo-"));
        assert!(fnb.starts_with("foo-"));
        // Deterministic: re-running produces same names.
        let fna2 = filename_for(&a, "cyclonedx-json", &cm);
        assert_eq!(fna, fna2);
    }

    #[test]
    fn slug_truncates_to_100_chars() {
        let long = "a".repeat(200);
        let raw = format!("pkg:cargo/{long}@1.0.0");
        let p = Purl::new(&raw).unwrap();
        let s = subject_slug(&p);
        assert!(s.len() <= 100);
    }

    // -------- format_ext --------

    #[test]
    fn format_ext_covers_registered_formats() {
        assert_eq!(format_ext("cyclonedx-json"), "cdx");
        assert_eq!(format_ext("spdx-2.3-json"), "spdx");
        assert_eq!(format_ext("spdx-3-json"), "spdx3");
        assert_eq!(format_ext("spdx-3-json-experimental"), "spdx3");
    }
}
