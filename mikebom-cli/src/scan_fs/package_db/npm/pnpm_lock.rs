//! pnpm-lock.yaml parser.
//!
//! Handles v6/v7 (single `packages:` section with inline
//! `dependencies` / `peerDependencies` / `optionalDependencies`)
//! AND v9 (`packages:` for identity + `snapshots:` for edges).
//! Milestone 157 (2026-07-03) added `snapshots:` support after the
//! team reported argo-cd's pnpm v9 lockfile emitting 1329 components
//! but only 110 dep-graph edges. Q1 clarification 2026-07-03 also
//! brought pnpm to parity with `package_lock.rs`'s milestone-147
//! behavior (walks all three non-dev dep sub-mappings — see
//! `PNPM_DEP_SECTIONS` const).

use std::path::Path;


use super::super::PackageDbEntry;
use super::{build_npm_purl, NpmIntegrity};

/// Milestone 157: pnpm dep-section names walked by both the snapshots
/// pre-scan (v9) AND the packages-inline path (v6/v7). Kept in one
/// place so the SC-011 pnpm/npm parity assertion has a stable code
/// anchor and so a future dep-section addition is a single edit.
///
/// NOT identical to `package_lock.rs`'s 4-section list — pnpm encodes
/// dev status via the per-package `dev: true` boolean at the entry
/// level (handled below in `parse_pnpm_lock`), NOT via a
/// `devDependencies:` sub-mapping. So `PNPM_DEP_SECTIONS` walks only
/// the three non-dev sections. The `dev: true` boolean continues to
/// gate whole-package filtering when `include_dev = false`.
const PNPM_DEP_SECTIONS: &[&str] = &[
    "dependencies",
    "peerDependencies",
    "optionalDependencies",
];

/// Milestone 157: walk the three dep sub-mappings inside a single
/// packages-entry table (v6/v7 inline path). Returns the sorted-
/// deduped union of the sub-mappings' KEYS (dep NAMES only — the
/// scan_fs dep-graph resolver at `scan_fs/mod.rs:700` keys
/// `name_to_purl` by `(ecosystem, name)`, matching every other npm
/// sub-reader's convention). Peer-dep suffixes on the KEY column
/// are irrelevant (dep-name column is a plain package name); we
/// only strip suffixes to filter out non-registry values in the
/// VERSION column (git URLs, tarballs, file paths) via
/// `parse_pnpm_key` on a synthesized `"<name>@<value>"` string.
fn walk_pnpm_dep_sections(entry_tbl: &serde_yaml::Mapping) -> Vec<String> {
    let mut deps: Vec<String> = Vec::new();
    for section in PNPM_DEP_SECTIONS {
        let Some(sub) = entry_tbl
            .get(serde_yaml::Value::String((*section).to_string()))
            .and_then(|v| v.as_mapping())
        else {
            continue;
        };
        for (dep_key, dep_value) in sub {
            let Some(dep_name) = dep_key.as_str() else { continue };
            let Some(dep_ver_raw) = dep_value.as_str() else { continue };
            // Validate that the VALUE is a registry-source string via
            // parse_pnpm_key round-trip; drop non-registry sources.
            let dep_pair_raw = format!("{dep_name}@{dep_ver_raw}");
            let stripped = dep_pair_raw
                .strip_prefix('/')
                .unwrap_or(&dep_pair_raw);
            let Some((canon_name, _canon_ver)) = parse_pnpm_key(stripped) else {
                tracing::debug!(dep = %dep_pair_raw, "pnpm-lock: skipping non-registry dep value");
                continue;
            };
            deps.push(canon_name);
        }
    }
    deps.sort();
    deps.dedup();
    deps
}

/// Milestone 157: pre-scan the top-level `snapshots:` section
/// (introduced in pnpm-lock.yaml v9) into a lookup table keyed by
/// canonical `name@version` (peer-dep suffix stripped via
/// `parse_pnpm_key`). Values are the sorted-deduped union of the
/// three sub-mappings' keys, each normalized to canonical form.
///
/// Returns empty HashMap when the top-level `snapshots:` key is
/// missing or not a mapping (v6/v7 lockfiles, or anomalous v9
/// lockfiles).
fn build_snapshots_lookup(
    root: &serde_yaml::Value,
) -> std::collections::HashMap<String, Vec<String>> {
    let mut out = std::collections::HashMap::new();
    let Some(snapshots) = root
        .get("snapshots")
        .and_then(|v| v.as_mapping())
    else {
        return out;
    };
    for (key, entry) in snapshots {
        let Some(key_str) = key.as_str() else { continue };
        let stripped = key_str.strip_prefix('/').unwrap_or(key_str);
        let Some((name, version)) = parse_pnpm_key(stripped) else {
            tracing::debug!(snapshot_key = %key_str, "pnpm-lock: skipping non-registry snapshot key");
            continue;
        };
        let canonical = format!("{name}@{version}");
        let Some(tbl) = entry.as_mapping() else { continue };
        let deps = walk_pnpm_dep_sections(tbl);
        out.insert(canonical, deps);
    }
    out
}

pub(super) fn read_pnpm_lock(rootfs: &Path, include_dev: bool) -> Option<Vec<PackageDbEntry>> {
    let path = rootfs.join("pnpm-lock.yaml");
    let text = std::fs::read_to_string(&path).ok()?;
    let parsed: serde_yaml::Value = serde_yaml::from_str(&text).ok()?;
    let source_path = path.to_string_lossy().into_owned();
    let out = parse_pnpm_lock(&parsed, &source_path, include_dev);
    if out.is_empty() { None } else { Some(out) }
}

/// Parse a deserialised `pnpm-lock.yaml` document. Handles v6/v7/v9
/// dialects per research.md R5. Milestone 157: v9's `snapshots:`
/// section is now the authoritative source for per-package dep-graph
/// edges (v6/v7 continued to use inline `packages:` entries). Both
/// paths now walk the union of the three non-dev sub-mappings
/// (`dependencies`, `peerDependencies`, `optionalDependencies`) per
/// Q1 clarification 2026-07-03, matching milestone-147 npm parity.
pub(crate) fn parse_pnpm_lock(
    root: &serde_yaml::Value,
    source_path: &str,
    include_dev: bool,
) -> Vec<PackageDbEntry> {
    let mut out = Vec::new();
    let mut fell_back_count: usize = 0;

    // Milestone 157: lockfileVersion detection for FR-007 / FR-008
    // diagnostic gating. Field may be quoted string ('9.0') or
    // unquoted number (6.0); accept both.
    let lock_version: String = root
        .get("lockfileVersion")
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| v.as_f64().map(|n| n.to_string()))
        })
        .unwrap_or_default();
    let is_v9_or_later: bool = lock_version
        .split('.')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .map(|major| major >= 9)
        .unwrap_or(false);

    // Milestone 157: pre-scan the v9 `snapshots:` section into a
    // lookup keyed by canonical name@version. Empty HashMap on
    // v6/v7 (no snapshots section) — the inline packages path
    // takes precedence via walk_pnpm_dep_sections.
    let snapshots_lookup = build_snapshots_lookup(root);

    // v6/v7 put per-package info under `packages:` keyed by
    // "/<name>@<version>" (or "/@scope/name@version"). v9 removes
    // the leading slash and moves dep edges to `snapshots:`; the
    // packages: side becomes identity + integrity metadata.
    let Some(packages) = root.get("packages").and_then(|v| v.as_mapping()) else {
        return out;
    };

    let mut keys: Vec<String> = packages
        .keys()
        .filter_map(|k| k.as_str().map(|s| s.to_string()))
        .collect();
    keys.sort();

    for key in keys {
        let Some(entry) = packages.get(serde_yaml::Value::String(key.clone())) else {
            continue;
        };
        let Some(tbl) = entry.as_mapping() else { continue };

        // v6/v7 key form: "/foo@1.0.0" or "/@scope/name@1.0.0"
        // v9 key form: "foo@1.0.0" (no leading slash)
        let stripped = key.strip_prefix('/').unwrap_or(&key);
        let (name, version) = parse_pnpm_key(stripped).unwrap_or_default();
        if name.is_empty() || version.is_empty() {
            continue;
        }

        let is_dev = tbl
            .get(serde_yaml::Value::String("dev".to_string()))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !include_dev && is_dev {
            continue;
        }

        let Some(purl) = build_npm_purl(&name, &version) else {
            continue;
        };

        let hashes = tbl
            .get(serde_yaml::Value::String("resolution".to_string()))
            .and_then(|res| res.as_mapping())
            .and_then(|m| m.get(serde_yaml::Value::String("integrity".to_string())))
            .and_then(|v| v.as_str())
            .and_then(NpmIntegrity::parse)
            .and_then(|i| i.to_content_hash())
            .map(|h| vec![h])
            .unwrap_or_default();

        // Milestone 157 depends construction — branched by lockfileVersion:
        // - v9 path: `packages:` entries carry ONLY identity + integrity
        //   metadata plus (for 225/1329 entries in the argo-cd testbed)
        //   a `peerDependencies:` sub-mapping whose VALUES ARE SEMVER
        //   SPECIFIERS (e.g. `^7.0.0`, `^7.4.0 || ^8.0.0-0 <8.0.0`), NOT
        //   resolved versions. Resolved dep-graph edges live EXCLUSIVELY
        //   in `snapshots:`. Reading inline `packages:` sub-mappings on
        //   v9 emits WRONG edges (the specifier's dep-NAME happens to
        //   match a real component so the graph looks plausible, but
        //   the 7-of-8 real edges from snapshots are silently dropped).
        //   Verified empirically 2026-07-03 on argo-cd/ui pre-fix:
        //   `@babel/helper-create-class-features-plugin@7.29.3` was
        //   emitting 1 edge (@babel/core, from the specifier) instead
        //   of 8 (from snapshots).
        // - v6/v7 path: `packages:` entries carry inline `dependencies:`
        //   with RESOLVED versions (matches milestone-147 npm parity).
        //   Walk the 3-section union directly.
        // - Empty on both sides = leaf semantics (FR-005).
        //
        // On v9 the snapshots_lookup HIT case (deps or empty leaf)
        // increments fell_back_count. On v6/v7 fell_back_count stays 0
        // (as expected — no fallback happened).
        let depends: Vec<String> = if is_v9_or_later {
            let canonical = format!("{name}@{version}");
            if let Some(snap_deps) = snapshots_lookup.get(&canonical) {
                fell_back_count += 1;
                snap_deps.clone()
            } else {
                Vec::new()
            }
        } else {
            walk_pnpm_dep_sections(tbl)
        };

        out.push(PackageDbEntry {
            build_inclusion: None,
            purl,
            name,
            version,
            arch: None,
            source_path: source_path.to_string(),
            depends,
            maintainer: None,
            licenses: Vec::new(),
            lifecycle_scope: if is_dev { Some(mikebom_common::resolution::LifecycleScope::Development) } else { Some(mikebom_common::resolution::LifecycleScope::Runtime) },
            requirement_range: None,
            source_type: None,
            buildinfo_status: None,
            evidence_kind: None,
            binary_class: None,
            binary_stripped: None,
            linkage_kind: None,
            detected_go: None,
            confidence: None,
            binary_packed: None,
            raw_version: None,
            parent_purl: None,
            npm_role: None,
            co_owned_by: None,
            hashes,
            sbom_tier: Some("source".to_string()),
            shade_relocation: None,
            extra_annotations: Default::default(),
            binary_role: None,
        });
    }

    // Milestone 157 FR-007 info-level diagnostic. Grep-friendly for
    // CI-log analysis. On v6/v7 lockfiles, fell_back_to_snapshots
    // will be 0 (inline path always populated); on well-formed v9
    // lockfiles, it approaches packages_count.
    tracing::info!(
        lockfile = %source_path,
        lockfile_version = %lock_version,
        packages_count = packages.len(),
        snapshots_count = snapshots_lookup.len(),
        fell_back_to_snapshots = fell_back_count,
        "pnpm-lock parsed"
    );

    // Milestone 157 FR-008 warn-level diagnostic — anomalous v9
    // lockfile shape. Fires when the operator's lockfile claims
    // v9 but has no snapshots section (pnpm's own tools would
    // refuse to install from it, but mikebom's fail-open posture
    // emits the identity-only graph with a diagnostic).
    if is_v9_or_later && snapshots_lookup.is_empty() {
        tracing::warn!(
            lockfile = %source_path,
            lockfile_version = %lock_version,
            "pnpm-lock v9 with no snapshots section — dep-graph will be empty for all non-root components. Check lockfile validity."
        );
    }

    out
}

/// Parse a pnpm package key — `<name>@<version>` or
/// `@<scope>/<name>@<version>` — into (name, version). The last `@`
/// is the version separator; everything before it is the name.
fn parse_pnpm_key(key: &str) -> Option<(String, String)> {
    // Strip any parenthesised peer-dep suffix (e.g. "(react@18.0.0)").
    let key = key.split('(').next().unwrap_or(key);
    // Find the LAST '@' that's after position 0 (position 0 is the
    // scope prefix for @scope/name).
    let search_start = if key.starts_with('@') { 1 } else { 0 };
    let at_idx = key[search_start..].rfind('@').map(|i| i + search_start)?;
    let name = key[..at_idx].to_string();
    let version = key[at_idx + 1..].to_string();
    if name.is_empty() || version.is_empty() {
        return None;
    }
    Some((name, version))
}

// -----------------------------------------------------------------------
// Tier B: flat node_modules walk
// -----------------------------------------------------------------------

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;
    #[test]
    fn pnpm_lock_v6_style_parses() {
        let yaml = r#"
lockfileVersion: '6.0'
packages:
  /lodash@4.17.21:
    resolution:
      integrity: sha512-MJ7MSJwS1utMxA9QyQLytNDtd+5RGnx+7fIK+4qg9hvLABzzXAIaFMqoD6YFUYaCQPkMInyGdz6TQEsE7bPdCg==
    dev: false
  /eslint@8.0.0:
    dev: true
"#;
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let out = parse_pnpm_lock(&parsed, "/pnpm-lock.yaml", false);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "lodash");
        assert_eq!(out[0].version, "4.17.21");
    }

    #[test]
    fn pnpm_lock_scoped_package_parses() {
        let yaml = r#"
lockfileVersion: '6.0'
packages:
  /@angular/core@16.0.0:
    resolution: {}
    dev: false
"#;
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let out = parse_pnpm_lock(&parsed, "/pnpm-lock.yaml", false);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "@angular/core");
        assert_eq!(out[0].version, "16.0.0");
        assert_eq!(out[0].purl.as_str(), "pkg:npm/%40angular/core@16.0.0");
    }

    #[test]
    fn pnpm_key_parser_handles_peer_suffix() {
        // v9 adds peer-dep suffixes: `react-dom@18.0.0(react@18.0.0)`.
        let (name, version) = parse_pnpm_key("react-dom@18.0.0(react@18.0.0)").unwrap();
        assert_eq!(name, "react-dom");
        assert_eq!(version, "18.0.0");
    }

    // ============================================================
    // Milestone 157 unit tests (SC-007 floor ≥8; 9 tests total
    // after F1 remediation added test #9 for SC-005 behavioral
    // verification). All 9 fn names begin with pnpm_v6_ / pnpm_v9_
    // / pnpm_walks_ for SC-007 grep compatibility.
    // ============================================================

    /// Helper: find first emitted entry by name.
    fn entry_by_name<'a>(entries: &'a [PackageDbEntry], name: &str) -> Option<&'a PackageDbEntry> {
        entries.iter().find(|e| e.name == name)
    }

    #[test]
    fn pnpm_v9_minimal_dependencies_only_emits_edge() {
        // Minimal v9 fixture: packages: identity + snapshots: single
        // dependencies edge. Assert the edge appears in depends[].
        let yaml = r#"
lockfileVersion: '9.0'

packages:
  foo@1.0.0:
    resolution: {integrity: sha512-aaaa}
  bar@2.0.0:
    resolution: {integrity: sha512-bbbb}

snapshots:
  foo@1.0.0:
    dependencies:
      bar: 2.0.0
  bar@2.0.0: {}
"#;
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let out = parse_pnpm_lock(&parsed, "/pnpm-lock.yaml", false);
        let foo = entry_by_name(&out, "foo").expect("foo emitted");
        assert_eq!(foo.depends, vec!["bar"]);
    }

    #[test]
    fn pnpm_v9_empty_snapshot_body_leaf_node() {
        // SC-004 + FR-005: snapshots entry with empty body → empty depends.
        let yaml = r#"
lockfileVersion: '9.0'

packages:
  foo@1.0.0:
    resolution: {integrity: sha512-aaaa}

snapshots:
  foo@1.0.0: {}
"#;
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let out = parse_pnpm_lock(&parsed, "/pnpm-lock.yaml", false);
        let foo = entry_by_name(&out, "foo").expect("foo emitted");
        assert!(foo.depends.is_empty(), "leaf node MUST have empty depends; got {:?}", foo.depends);
    }

    #[test]
    fn pnpm_v9_peer_dep_suffix_normalized_in_key_and_value() {
        // SC-003 + FR-003: peer-dep suffixes on snapshot KEY and dep VALUE
        // both normalize via parse_pnpm_key to canonical name@version.
        let yaml = r#"
lockfileVersion: '9.0'

packages:
  foo@1.0.0:
    resolution: {integrity: sha512-aaaa}
  baz@3.0.0:
    resolution: {integrity: sha512-cccc}

snapshots:
  foo@1.0.0(bar@2.0.0):
    dependencies:
      baz: 3.0.0(qux@4.0.0)
  baz@3.0.0: {}
"#;
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let out = parse_pnpm_lock(&parsed, "/pnpm-lock.yaml", false);
        let foo = entry_by_name(&out, "foo").expect("foo emitted");
        // Identity peer-suffix stripped from PURL.
        assert_eq!(foo.purl.as_str(), "pkg:npm/foo@1.0.0");
        // Value peer-suffix stripped from edge target.
        assert_eq!(foo.depends, vec!["baz"]);
    }

    #[test]
    fn pnpm_v9_orphaned_snapshot_skipped() {
        // FR-006: snapshot with no matching packages entry → skip.
        let yaml = r#"
lockfileVersion: '9.0'

packages: {}

snapshots:
  foo@1.0.0:
    dependencies:
      bar: 2.0.0
"#;
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let out = parse_pnpm_lock(&parsed, "/pnpm-lock.yaml", false);
        assert!(out.is_empty(), "orphan snapshot MUST NOT emit; got {:?}", out);
    }

    #[test]
    fn pnpm_v9_all_three_sub_mappings_union_with_dedup() {
        // FR-002 + Q1 clarification: union of dependencies +
        // peerDependencies + optionalDependencies with defensive dedup.
        let yaml = r#"
lockfileVersion: '9.0'

packages:
  foo@1.0.0:
    resolution: {integrity: sha512-aaaa}
  a@1.0.0:
    resolution: {}
  b@2.0.0:
    resolution: {}
  c@3.0.0:
    resolution: {}
  shared@5.0.0:
    resolution: {}

snapshots:
  foo@1.0.0:
    dependencies:
      a: 1.0.0
      shared: 5.0.0
    peerDependencies:
      b: 2.0.0
      shared: 5.0.0
    optionalDependencies:
      c: 3.0.0
"#;
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let out = parse_pnpm_lock(&parsed, "/pnpm-lock.yaml", false);
        let foo = entry_by_name(&out, "foo").expect("foo emitted");
        // Sorted + deduped union (shared@5.0.0 appears once).
        assert_eq!(
            foo.depends,
            vec!["a", "b", "c", "shared"]
        );
    }

    #[test]
    fn pnpm_v6_v7_inline_peer_and_optional_now_emit() {
        // FR-004 + Q1: v6/v7 inline path now walks 3 sub-mappings.
        // No snapshots section — pure v6-style fixture.
        let yaml = r#"
lockfileVersion: '6.0'

packages:
  /foo@1.0.0:
    resolution: {integrity: sha512-aaaa}
    dependencies:
      a: 1.0.0
    peerDependencies:
      b: 2.0.0
    optionalDependencies:
      c: 3.0.0
"#;
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let out = parse_pnpm_lock(&parsed, "/pnpm-lock.yaml", false);
        let foo = entry_by_name(&out, "foo").expect("foo emitted");
        assert_eq!(foo.depends, vec!["a", "b", "c"]);
    }

    #[test]
    fn pnpm_v9_snapshots_authoritative_ignoring_packages_specifiers() {
        // Empirical bug 2026-07-03 (post-T014 argo-cd/ui audit): on a
        // real pnpm v9 lockfile, `packages:` entries carry ONLY
        // identity/integrity metadata plus (for a subset) a
        // `peerDependencies:` sub-mapping whose values are SEMVER
        // SPECIFIERS, not resolved versions. Resolved dep-graph edges
        // live EXCLUSIVELY in `snapshots:`. Reading `packages:` inline
        // sub-mappings on v9 emits WRONG edges (the specifier's
        // dep-NAME may coincidentally match a real component). Correct
        // behavior: on v9, ALWAYS use snapshots and IGNORE inline
        // packages: sub-mappings.
        let yaml = r#"
lockfileVersion: '9.0'

packages:
  foo@1.0.0:
    resolution: {integrity: sha512-aaaa}
    peerDependencies:
      only-specifier: ^7.0.0

snapshots:
  foo@1.0.0:
    dependencies:
      only-snapshots: 2.0.0
"#;
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let out = parse_pnpm_lock(&parsed, "/pnpm-lock.yaml", false);
        let foo = entry_by_name(&out, "foo").expect("foo emitted");
        assert_eq!(
            foo.depends,
            vec!["only-snapshots"],
            "v9 MUST read edges from snapshots and MUST NOT emit \
             the specifier-only edge from packages:.peerDependencies"
        );
    }

    #[test]
    fn pnpm_walks_same_dep_sections_as_package_lock_non_dev() {
        // SC-011 parity: PNPM_DEP_SECTIONS matches the non-dev subset
        // of package_lock.rs's 4-section walk. The `dev: true` boolean
        // handles pnpm's dev-status axis; no devDependencies: sub-
        // mapping exists on individual pnpm entries by lockfile design.
        assert_eq!(
            PNPM_DEP_SECTIONS,
            &["dependencies", "peerDependencies", "optionalDependencies"],
            "SC-011: PNPM_DEP_SECTIONS drift from expected parity set"
        );
    }

    #[test]
    fn pnpm_v9_no_snapshots_scans_cleanly_with_empty_deps() {
        // F1 remediation, SC-005 behavioral verification: v9 lockfile
        // with no `snapshots:` key at all. Parser MUST return cleanly
        // (no panic) and emit components with empty depends. The
        // FR-008 tracing::warn! side effect is documented in FR-008
        // for operator grep; automated log-string capture is out of
        // scope per SC-005's downgraded automation claim.
        let yaml = r#"
lockfileVersion: '9.0'

packages:
  foo@1.0.0:
    resolution: {integrity: sha512-aaaa}
  bar@2.0.0:
    resolution: {integrity: sha512-bbbb}
"#;
        let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let out = parse_pnpm_lock(&parsed, "/pnpm-lock.yaml", false);
        assert_eq!(out.len(), 2);
        for entry in &out {
            assert!(
                entry.depends.is_empty(),
                "v9 with no snapshots MUST emit empty depends; {} got {:?}",
                entry.name,
                entry.depends
            );
        }
    }
}
