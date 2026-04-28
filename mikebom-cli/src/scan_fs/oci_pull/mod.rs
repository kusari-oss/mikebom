//! OCI registry image pull (milestone 031, restructured into a
//! submodule directory by milestone 032).
//!
//! This module is gated behind the default-off `oci-registry`
//! Cargo feature. When enabled, the `--image <ref>` CLI argument
//! accepts an OCI image reference (e.g. `alpine:3.19`,
//! `gcr.io/foo/bar@sha256:...`) in addition to the existing
//! docker-save tarball path. The reference is parsed, the manifest
//! plus layer blobs are pulled, gzipped layers are decompressed,
//! and a docker-save-format tarball is written to a tempdir before
//! being routed through the existing
//! `scan_fs::docker_image::extract` path.
//!
//! Sub-scope (milestone 031):
//!   * Anonymous public registries only.
//!   * Host-arch image selection only (no `--image-platform` flag).
//!   * Gzipped layers only (zstd → clear "not yet supported" error).
//!
//! Deferred:
//!   * 031.x — authenticated pulls (Docker keychain + cred helpers).
//!   * 031.y — `--image-platform linux/arch` flag.
//!   * 031.z — layer caching.
//!
//! Substrate evolution:
//!   * Milestone 031 used `oci-client = "0.12"` for the full pull
//!     pipeline (manifest fetch, blob fetch, digest verification,
//!     manifest-list resolution).
//!   * Milestone 032 (this commit's parent module split) is the
//!     **first phase** of replacing oci-client with `oci-spec`
//!     (types-only) + a thin custom HTTP client. This commit
//!     restructures the file into submodules; oci-client is still
//!     the substrate. Subsequent commits in milestone 032 swap
//!     in the new substrate and drop the oci-client dep.

mod platform;
mod reference;
mod tarball;

use std::path::Path;

use anyhow::{Context, Result};

use oci_client::client::{ClientConfig, ImageData};
use oci_client::manifest::{
    ImageIndexEntry, IMAGE_CONFIG_MEDIA_TYPE, IMAGE_DOCKER_CONFIG_MEDIA_TYPE,
    IMAGE_DOCKER_LAYER_GZIP_MEDIA_TYPE, IMAGE_DOCKER_LAYER_TAR_MEDIA_TYPE,
    IMAGE_LAYER_GZIP_MEDIA_TYPE, IMAGE_LAYER_MEDIA_TYPE, IMAGE_MANIFEST_MEDIA_TYPE,
    OCI_IMAGE_MEDIA_TYPE,
};
use oci_client::secrets::RegistryAuth;
use oci_client::{Client, Reference};

/// Pull an OCI image reference and write a docker-save-format
/// tarball to a tempdir. Returns the TempDir handle so the
/// caller can keep it alive through the subsequent
/// `docker_image::extract` call. The tarball lives at
/// `<tempdir>/image.tar`.
///
/// Multi-arch image indexes resolve to `linux/<host-arch>` via
/// a custom platform resolver (mikebom only scans Linux
/// containers regardless of the host OS — the default oci-client
/// resolver would mismatch on macOS/Windows hosts).
///
/// Anonymous pulls only in milestone 031. Auth handling lives in
/// the deferred 031.x follow-on.
///
/// Async by design — mikebom's CLI is `#[tokio::main]`-bootstrapped,
/// so callers `.await` this directly without bridging.
pub async fn pull_to_tarball(image_ref: &str) -> Result<tempfile::TempDir> {
    let reference: Reference = image_ref
        .parse()
        .with_context(|| format!("parsing OCI image reference `{image_ref}`"))?;
    tracing::info!(
        registry = %reference.resolve_registry(),
        repository = %reference.repository(),
        tag = ?reference.tag(),
        digest = ?reference.digest(),
        "pulling OCI image"
    );

    let host_arch = host_oci_arch()
        .context("mapping host architecture to OCI platform name")?;
    let config = ClientConfig {
        platform_resolver: Some(Box::new(move |entries: &[ImageIndexEntry]| {
            // Adapter from oci-client's ImageIndexEntry shape to
            // platform.rs's borrowed-view type. Lets the resolver
            // stay decoupled from any specific crate's types.
            let mapped: Vec<platform::ManifestListEntry<'_>> = entries
                .iter()
                .filter_map(|e| {
                    let p = e.platform.as_ref()?;
                    Some(platform::ManifestListEntry {
                        digest: e.digest.as_str(),
                        architecture: p.architecture.as_str(),
                        os: p.os.as_str(),
                    })
                })
                .collect();
            platform::resolve_manifest_list_to_linux(mapped, host_arch).ok()
        })),
        ..ClientConfig::default()
    };
    let client = Client::new(config);
    let auth = RegistryAuth::Anonymous;
    // Tell the registry which media types we accept. We accept
    // both Docker v2 and OCI manifest types, plus tar and gzipped
    // tar layer types. zstd-compressed layers (a separate OCI
    // media type) are NOT accepted — registries that prefer
    // those will return them anyway, in which case
    // `tarball::assert_layers_supported` catches it with a clear
    // "not yet supported" error.
    let accepted = vec![
        IMAGE_MANIFEST_MEDIA_TYPE,
        OCI_IMAGE_MEDIA_TYPE,
        IMAGE_LAYER_MEDIA_TYPE,
        IMAGE_LAYER_GZIP_MEDIA_TYPE,
        IMAGE_DOCKER_LAYER_TAR_MEDIA_TYPE,
        IMAGE_DOCKER_LAYER_GZIP_MEDIA_TYPE,
        IMAGE_CONFIG_MEDIA_TYPE,
        IMAGE_DOCKER_CONFIG_MEDIA_TYPE,
    ];
    let image: ImageData = client
        .pull(&reference, &auth, accepted)
        .await
        .with_context(|| format!("pulling image `{image_ref}`"))?;

    tarball::assert_layers_supported(&image)?;

    let tempdir = tempfile::Builder::new()
        .prefix("mikebom-oci-pull-")
        .tempdir()
        .context("creating tempdir for OCI pull tarball")?;
    let tarball_path = tempdir.path().join("image.tar");
    tarball::assemble_docker_save_tarball(&image, image_ref, &tarball_path)
        .context("assembling docker-save-format tarball from pulled image")?;
    Ok(tempdir)
}

/// Distinguish a `--image` argument as either a path on disk
/// (existing tarball-extract path) or an OCI image reference
/// (the registry-pull path).
///
/// Detection rules (priority order):
///  1. If a file exists at the given path → treat as tarball.
///  2. Else if the string parses via the new
///     [`reference::parse_reference`] grammar → treat as ref.
///  3. Else → return `Invalid`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageArgKind {
    /// Path to a docker-save-format tarball on disk.
    Path,
    /// OCI image reference (e.g. `alpine:3.19`).
    OciRef,
    /// Neither — error.
    Invalid,
}

pub fn detect_image_arg_kind(arg: &Path) -> ImageArgKind {
    if arg.is_file() {
        return ImageArgKind::Path;
    }
    let s = match arg.to_str() {
        Some(s) => s,
        None => return ImageArgKind::Invalid,
    };
    match reference::parse_reference(s) {
        Ok(_) => ImageArgKind::OciRef,
        Err(_) => ImageArgKind::Invalid,
    }
}

/// Map `std::env::consts::ARCH` to an OCI platform-arch name.
///
/// The OCI image-spec uses Go's GOARCH naming (`amd64`, `arm64`,
/// `arm`, `riscv64`, etc.) which differs from Rust's `ARCH`
/// constant (`x86_64`, `aarch64`, etc.).
///
/// Returns an error for unmapped host architectures so the
/// caller can surface a clear "host arch X not supported, please
/// use --image-platform <linux/...> when 031.y ships" message.
pub fn host_oci_arch() -> Result<&'static str> {
    Ok(match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "arm" => "arm",
        "riscv64" => "riscv64",
        "powerpc64" => "ppc64le", // typical OCI naming
        "s390x" => "s390x",
        other => {
            anyhow::bail!(
                "host architecture `{other}` not mapped to an OCI platform name; \
                 milestone 031 supports x86_64/aarch64/arm/riscv64/powerpc64/s390x. \
                 Cross-arch image pulls (`--image-platform linux/<arch>`) deferred to milestone 031.y."
            );
        }
    })
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    #[test]
    fn host_oci_arch_returns_a_known_value_for_typical_hosts() {
        let arch = host_oci_arch();
        assert!(arch.is_ok(), "host_oci_arch failed: {arch:?}");
        let arch = arch.unwrap();
        assert!(
            ["amd64", "arm64", "arm", "riscv64", "ppc64le", "s390x"].contains(&arch),
            "unexpected OCI arch `{arch}` for std::env::consts::ARCH = {}",
            std::env::consts::ARCH,
        );
    }

    #[test]
    fn detect_image_arg_kind_recognizes_existing_file_as_path() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        assert_eq!(detect_image_arg_kind(tmp.path()), ImageArgKind::Path);
    }

    #[test]
    fn detect_image_arg_kind_recognizes_typical_image_refs() {
        let cases = &[
            "alpine:3.19",
            "library/alpine:3.19",
            "docker.io/library/alpine:3.19",
            "gcr.io/distroless/static-debian12:latest",
            "ghcr.io/foo/bar@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        ];
        for case in cases {
            let p = Path::new(case);
            assert_eq!(
                detect_image_arg_kind(p),
                ImageArgKind::OciRef,
                "expected OciRef for `{case}`",
            );
        }
    }

    #[test]
    fn detect_image_arg_kind_rejects_garbage() {
        let p = Path::new("");
        assert_eq!(detect_image_arg_kind(p), ImageArgKind::Invalid);
    }
}
