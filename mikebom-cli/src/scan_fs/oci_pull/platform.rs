//! Manifest-list → platform-specific manifest selection
//! (milestone 031, extracted as a submodule by milestone 032).
//!
//! When a registry returns an image index (multi-arch manifest
//! list), we pick the entry matching `linux/<host-arch>`.
//! Cross-arch selection (overriding the host default) is deferred
//! to milestone 031.y / #67.

use anyhow::{anyhow, Result};

/// Select the digest of the manifest matching `linux/<target_arch>`
/// from a list of `(architecture, os, digest)` triples.
///
/// Returns the matching digest as a String. Errors when no entry
/// matches, listing the available platforms in the message so the
/// user can see what they could pass with `--image-platform` once
/// 031.y ships.
pub(super) fn resolve_manifest_list_to_linux<'a, I>(
    entries: I,
    target_arch: &str,
) -> Result<String>
where
    I: IntoIterator<Item = ManifestListEntry<'a>>,
{
    let entries: Vec<ManifestListEntry<'a>> = entries.into_iter().collect();
    if let Some(entry) = entries
        .iter()
        .find(|e| e.os == "linux" && e.architecture == target_arch)
    {
        return Ok(entry.digest.to_string());
    }
    let available: Vec<String> = entries
        .iter()
        .map(|e| format!("{}/{}", e.os, e.architecture))
        .collect();
    Err(anyhow!(
        "no manifest in image index matches linux/{target_arch}; \
         available: [{}]. Cross-arch image pulls (`--image-platform linux/<arch>`) \
         deferred to milestone 031.y.",
        available.join(", ")
    ))
}

/// Borrowed view of one entry in a manifest list / image index.
/// Decoupled from any specific crate's type so this submodule
/// works equally well with oci-client's
/// `manifest::ImageIndexEntry` (milestone 031) and oci-spec's
/// `image::Descriptor` (milestone 032+).
#[derive(Clone, Copy, Debug)]
pub(super) struct ManifestListEntry<'a> {
    pub digest: &'a str,
    pub architecture: &'a str,
    pub os: &'a str,
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    #[test]
    fn picks_linux_amd64_when_present() {
        let entries = vec![
            ManifestListEntry {
                digest: "sha256:amd64",
                architecture: "amd64",
                os: "linux",
            },
            ManifestListEntry {
                digest: "sha256:arm64",
                architecture: "arm64",
                os: "linux",
            },
        ];
        let digest = resolve_manifest_list_to_linux(entries, "amd64").unwrap();
        assert_eq!(digest, "sha256:amd64");
    }

    #[test]
    fn picks_first_matching_when_multiples() {
        // OCI spec doesn't promise unique platforms, but the
        // registry SHOULD only emit one per platform. If duplicates
        // appear we just pick the first match (deterministic
        // iteration over the input).
        let entries = vec![
            ManifestListEntry {
                digest: "sha256:first",
                architecture: "amd64",
                os: "linux",
            },
            ManifestListEntry {
                digest: "sha256:second",
                architecture: "amd64",
                os: "linux",
            },
        ];
        let digest = resolve_manifest_list_to_linux(entries, "amd64").unwrap();
        assert_eq!(digest, "sha256:first");
    }

    #[test]
    fn errors_when_target_unavailable_and_lists_what_is() {
        let entries = vec![
            ManifestListEntry {
                digest: "sha256:arm64",
                architecture: "arm64",
                os: "linux",
            },
            ManifestListEntry {
                digest: "sha256:s390x",
                architecture: "s390x",
                os: "linux",
            },
        ];
        let err = resolve_manifest_list_to_linux(entries, "amd64").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("linux/amd64"), "missing target in message: {msg}");
        assert!(msg.contains("linux/arm64"), "missing available platform: {msg}");
        assert!(msg.contains("linux/s390x"), "missing available platform: {msg}");
    }

    #[test]
    fn skips_non_linux_os_entries() {
        // darwin/arm64 entries (rare but possible in some indices)
        // are correctly NOT matched even when target_arch matches.
        let entries = vec![
            ManifestListEntry {
                digest: "sha256:darwin-arm64",
                architecture: "arm64",
                os: "darwin",
            },
            ManifestListEntry {
                digest: "sha256:linux-arm64",
                architecture: "arm64",
                os: "linux",
            },
        ];
        let digest = resolve_manifest_list_to_linux(entries, "arm64").unwrap();
        assert_eq!(digest, "sha256:linux-arm64");
    }
}
