//! Network-gated end-to-end smoke test for milestone 031.
//!
//! This test pulls a real public OCI image from a real registry,
//! runs it through the full mikebom pipeline (oci_pull →
//! docker_image::extract → scan → SBOM emission), and verifies the
//! output is well-formed. It runs ONLY when:
//!
//!   1. The crate is built with `--features oci-registry`, AND
//!   2. The `MIKEBOM_OCI_NETWORK_TESTS=1` env var is set.
//!
//! The default Linux + ebpf + macOS CI lanes do NOT set the env
//! var, so this test is silently skipped on every standard PR. A
//! follow-on milestone may add a dedicated `lint-and-test-oci-network`
//! job that flips it on; for milestone 031 it ships as opt-in only.
//!
//! To run locally:
//!
//! ```sh
//! MIKEBOM_OCI_NETWORK_TESTS=1 cargo +stable test \
//!     -p mikebom --features oci-registry --test oci_registry_smoke
//! ```

#![cfg(feature = "oci-registry")]

use std::process::Command;

fn network_tests_enabled() -> bool {
    std::env::var("MIKEBOM_OCI_NETWORK_TESTS").ok().as_deref() == Some("1")
}

#[test]
fn pulls_alpine_3_19_and_emits_apk_components() {
    if !network_tests_enabled() {
        eprintln!(
            "skipping: set MIKEBOM_OCI_NETWORK_TESTS=1 to run network-gated smoke tests"
        );
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let out_path = tmp.path().join("alpine.cdx.json");
    let output = Command::new(env!("CARGO_BIN_EXE_mikebom"))
        .arg("--offline") // VEX/CD enrichment off; pure registry pull
        .arg("sbom")
        .arg("scan")
        .arg("--image")
        .arg("alpine:3.19")
        .arg("--format")
        .arg("cyclonedx-json")
        .arg("--output")
        .arg(&out_path)
        .output()
        .expect("mikebom should run");
    assert!(
        output.status.success(),
        "mikebom failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let bytes = std::fs::read(&out_path).expect("read alpine.cdx.json");
    let sbom: serde_json::Value = serde_json::from_slice(&bytes).expect("valid CDX JSON");
    let components = sbom["components"]
        .as_array()
        .expect("CDX components array");
    // Alpine 3.19 base image has ~15-20 apk packages; we
    // intentionally don't pin the exact count to avoid breaking
    // when alpine bumps a minor version. Just assert non-empty
    // and at least one well-formed apk PURL.
    assert!(
        !components.is_empty(),
        "alpine:3.19 should yield ≥1 component; got 0"
    );
    let has_apk_purl = components.iter().any(|c| {
        c["purl"]
            .as_str()
            .is_some_and(|p| p.starts_with("pkg:apk/alpine/"))
    });
    assert!(
        has_apk_purl,
        "alpine:3.19 should yield at least one pkg:apk/alpine/* PURL; got components: {}",
        serde_json::to_string_pretty(&components).unwrap_or_default()
    );
}

#[test]
fn pulls_distroless_static_and_emits_well_formed_sbom_with_zero_components() {
    if !network_tests_enabled() {
        eprintln!(
            "skipping: set MIKEBOM_OCI_NETWORK_TESTS=1 to run network-gated smoke tests"
        );
        return;
    }

    // distroless static images intentionally ship with NO package
    // manager metadata. mikebom should produce a well-formed SBOM
    // with zero components — that's the correct behavior, not a
    // bug. Anything else (panic, error, hallucinated components)
    // is a regression.
    let tmp = tempfile::tempdir().expect("tempdir");
    let out_path = tmp.path().join("distroless.cdx.json");
    let output = Command::new(env!("CARGO_BIN_EXE_mikebom"))
        .arg("--offline")
        .arg("sbom")
        .arg("scan")
        .arg("--image")
        .arg("gcr.io/distroless/static-debian12:latest")
        .arg("--format")
        .arg("cyclonedx-json")
        .arg("--output")
        .arg(&out_path)
        .output()
        .expect("mikebom should run");
    assert!(
        output.status.success(),
        "mikebom failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let bytes = std::fs::read(&out_path).expect("read distroless.cdx.json");
    let sbom: serde_json::Value = serde_json::from_slice(&bytes).expect("valid CDX JSON");
    // SBOM is well-formed — the fields we care about are present.
    assert!(sbom["bomFormat"].as_str().is_some(), "missing bomFormat");
    assert!(
        sbom["specVersion"].as_str().is_some(),
        "missing specVersion"
    );
    assert!(
        sbom["serialNumber"].as_str().is_some(),
        "missing serialNumber"
    );
    // Zero components is the correct outcome here.
    let components = sbom["components"].as_array().map(|c| c.len()).unwrap_or(0);
    assert_eq!(
        components, 0,
        "distroless static image should yield 0 components (it ships no package metadata); got {components}"
    );
}
