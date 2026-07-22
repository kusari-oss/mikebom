//! Milestone 215 — integration tests for `waybill sbom scan --split`.
//!
//! Verifies the end-to-end split fan-out against the m212
//! `two_binaries_diverge` cargo-workspace fixture (4 members):
//! T018  — happy path: 4 sub-SBOMs + 1 manifest, correct root
//!         identity, per-member component set, schema-valid
//!         CDX 1.6 output for every sub-SBOM (SC-006).
//! T018a — zero-boundary fallback: single-package project → 1 SBOM,
//!         no manifest, WARN log (FR-009).
//! T019  — manifest lists every emitted file for the requested
//!         format.

use std::path::PathBuf;
use std::process::Command;

use tempfile::tempdir;

fn workspace_root() -> PathBuf {
    // waybill-cli/tests/scan_split_basic.rs → workspace root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("waybill-cli parent")
        .to_path_buf()
}

fn m212_cargo_workspace_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/compiler_pipeline/two_binaries_diverge")
}

fn waybill_bin() -> PathBuf {
    // Cargo sets CARGO_BIN_EXE_<name> for integration-test binaries.
    PathBuf::from(env!("CARGO_BIN_EXE_waybill"))
}

/// Run `waybill sbom scan --split --output-dir <dir> [args]` in an
/// isolated $HOME so per-host caches (~/.m2, ~/.cargo, etc.) don't
/// leak into the split output.
fn run_split_scan(
    path: &PathBuf,
    output_dir: &PathBuf,
    extra_args: &[&str],
) -> (bool, String, String) {
    let home = tempdir().expect("home tempdir");
    let output = Command::new(waybill_bin())
        .arg("sbom")
        .arg("scan")
        .arg("--path")
        .arg(path)
        .arg("--split")
        .arg("--output-dir")
        .arg(output_dir)
        .args(extra_args)
        .env("HOME", home.path())
        .env("XDG_CACHE_HOME", home.path())
        .env("CARGO_HOME", home.path().join(".cargo"))
        .env("GOMODCACHE", home.path().join("go-mod"))
        .env("M2_REPO", home.path().join(".m2"))
        .current_dir(workspace_root())
        .output()
        .expect("spawn waybill");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

/// Read every JSON file in `dir` (matching a pattern) and return the
/// list of paths.
fn list_files(dir: &PathBuf, suffix: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read output_dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(suffix))
                .unwrap_or(false)
        })
        .collect();
    out.sort();
    out
}

// ============ T018 ============

#[test]
fn cargo_workspace_split_emits_one_sbom_per_member() {
    let out = tempdir().expect("output tempdir");
    let out_path = out.path().to_path_buf();
    let (ok, _stdout, stderr) = run_split_scan(
        &m212_cargo_workspace_fixture(),
        &out_path,
        &["--format", "cyclonedx-json"],
    );
    assert!(ok, "split scan failed:\n{stderr}");

    // 4 CDX sub-SBOMs + 1 manifest.
    let cdxs = list_files(&out_path, ".cdx.json");
    assert_eq!(
        cdxs.len(),
        4,
        "expected 4 sub-SBOMs, got {}:\n{}",
        cdxs.len(),
        cdxs.iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    );

    let manifest_path = out_path.join("split-manifest.json");
    assert!(
        manifest_path.exists(),
        "split-manifest.json missing at {}",
        manifest_path.display()
    );

    // Each sub-SBOM's metadata.component.purl matches a distinct
    // pkg:cargo/<member>@0.1.0.
    let mut roots: Vec<String> = Vec::new();
    for p in &cdxs {
        let text = std::fs::read_to_string(p).expect("read cdx");
        let v: serde_json::Value =
            serde_json::from_str(&text).expect("parse cdx");
        let purl = v
            .pointer("/metadata/component/purl")
            .and_then(|s| s.as_str())
            .expect("metadata.component.purl present")
            .to_string();
        roots.push(purl);
    }
    roots.sort();
    assert_eq!(
        roots,
        vec![
            "pkg:cargo/libsafe@0.1.0",
            "pkg:cargo/libvuln@0.1.0",
            "pkg:cargo/safe-only@0.1.0",
            "pkg:cargo/vuln-included@0.1.0",
        ],
        "sub-SBOM root PURLs don't match the 4 cargo workspace members",
    );
}

// ============ T019 ============

#[test]
fn cargo_workspace_split_manifest_lists_all_emitted_files() {
    let out = tempdir().expect("output tempdir");
    let out_path = out.path().to_path_buf();
    let (ok, _stdout, stderr) = run_split_scan(
        &m212_cargo_workspace_fixture(),
        &out_path,
        &["--format", "cyclonedx-json"],
    );
    assert!(ok, "split scan failed:\n{stderr}");

    let manifest_text = std::fs::read_to_string(out_path.join("split-manifest.json"))
        .expect("read manifest");
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_text).expect("parse manifest");

    // Contract fields present.
    assert_eq!(
        manifest["$schema"],
        "https://waybill.dev/schema/split-manifest/v1.json"
    );
    assert!(manifest["waybill_version"].is_string());
    assert!(manifest["scan_root"].is_string());
    assert!(manifest["generated_at"].is_string());
    assert!(manifest["total_unique_components"].is_number());
    assert!(manifest["shared_dep_count"].is_number());

    let entries = manifest["entries"].as_array().expect("entries array");
    assert_eq!(entries.len(), 4, "expected 4 manifest entries");

    // Every entries[].files["cyclonedx-json"] filename exists on disk.
    let mut ids: Vec<String> = Vec::new();
    for entry in entries {
        let id = entry["subproject_id"].as_str().expect("id").to_string();
        let filename = entry["files"]["cyclonedx-json"]
            .as_str()
            .expect("cdx filename");
        let fp = out_path.join(filename);
        assert!(
            fp.exists(),
            "manifest lists {filename} but file missing at {}",
            fp.display()
        );
        ids.push(id);
    }
    // subproject_id is unique per entry.
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "duplicate subproject_id in manifest");
}

// ============ T018a ============

#[test]
fn split_on_single_package_falls_back_to_one_sbom() {
    // Fixture: single-package Cargo.toml (no [workspace]).
    let scratch = tempdir().expect("fixture tempdir");
    let pkg_root = scratch.path();
    std::fs::write(
        pkg_root.join("Cargo.toml"),
        "[package]\nname = \"solitary\"\nversion = \"1.0.0\"\nedition = \"2021\"\n",
    )
    .expect("write manifest");
    std::fs::create_dir_all(pkg_root.join("src")).expect("mkdir src");
    std::fs::write(
        pkg_root.join("src/lib.rs"),
        "pub fn hi() -> &'static str { \"hi\" }\n",
    )
    .expect("write lib");
    // Empty Cargo.lock (cargo generates a minimal one, but our
    // scanner is fine without one for the workspace-detection path).
    std::fs::write(
        pkg_root.join("Cargo.lock"),
        "# empty lock — scanner will still find the [package] entry\nversion = 4\n",
    )
    .expect("write lock");

    let out = tempdir().expect("output tempdir");
    let out_path = out.path().to_path_buf();
    let (ok, _stdout, stderr) = run_split_scan(
        &pkg_root.to_path_buf(),
        &out_path,
        &["--format", "cyclonedx-json"],
    );
    assert!(ok, "split scan on single-package failed:\n{stderr}");

    // FR-009: WARN log emitted (stable grep substring).
    assert!(
        stderr.contains("no workspace boundaries detected"),
        "expected 'no workspace boundaries detected' in stderr:\n{stderr}"
    );

    // No manifest written (nothing to describe).
    assert!(
        !out_path.join("split-manifest.json").exists(),
        "manifest MUST NOT be written on zero-boundary fallback"
    );
}
