//! Integration test for the bun.lock reader (milestone 106 US2, issue #278).
//!
//! Companion to the unit tests in `scan_fs::package_db::npm::bun_lock::tests`
//! (which exercise `parse_bun_lock` directly). This test invokes the
//! `mikebom sbom scan --path <fixture>` binary against the in-repo
//! `bun_lock/basic/` fixture to verify the dispatcher integration —
//! `npm::read` actually calls `bun_lock::read_bun_lock`, the JSONC
//! comment stripper handles the top-of-file `// bun: lockfileVersion: 1`
//! marker correctly, and the emitted SBOM contains the expected
//! `pkg:npm/...` components including the URL-encoded scoped package.

use std::path::PathBuf;
use std::process::Command;

mod common;
use common::bin;
use common::normalize::apply_fake_home_env;

fn basic_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden_inputs")
        .join("bun_lock")
        .join("basic")
}

#[test]
fn bun_lock_basic_fixture_emits_npm_components() {
    let fixture = basic_fixture();
    assert!(
        fixture.is_dir(),
        "bun_lock/basic fixture missing at {}",
        fixture.display()
    );
    let workdir = tempfile::tempdir().expect("workdir tempdir");
    let fake_home = tempfile::tempdir().expect("fake-home tempdir");
    let out_path = workdir.path().join("sbom.cdx.json");

    let mut cmd = Command::new(bin());
    apply_fake_home_env(&mut cmd, fake_home.path());
    cmd.env("MIKEBOM_FIXED_TIMESTAMP", "2026-01-01T00:00:00Z");
    cmd.args([
        "--offline",
        "sbom",
        "scan",
        "--path",
        fixture.to_str().unwrap(),
        "--format",
        "cyclonedx-json",
        "--output",
        out_path.to_str().unwrap(),
    ]);
    let output = cmd.output().expect("spawn mikebom");
    assert!(
        output.status.success(),
        "bun.lock scan unexpectedly failed: stderr={}",
        String::from_utf8_lossy(&output.stderr),
    );

    let bytes = std::fs::read(&out_path).expect("read emitted SBOM");
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("parse JSON");

    let components = json["components"]
        .as_array()
        .expect("components array present");
    let npm_purls: Vec<&str> = components
        .iter()
        .filter_map(|c| c["purl"].as_str())
        .filter(|p| p.starts_with("pkg:npm/"))
        .collect();

    // Both fixture packages MUST appear, including the scoped name
    // with URL-encoded `@` (per the PURL spec).
    assert!(
        npm_purls.contains(&"pkg:npm/lodash@4.17.21"),
        "expected lodash in output; got: {npm_purls:?}",
    );
    assert!(
        npm_purls.contains(&"pkg:npm/%40types/node@22.5.0"),
        "expected URL-encoded @types/node in output; got: {npm_purls:?}",
    );
}
