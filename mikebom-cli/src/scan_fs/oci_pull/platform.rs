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
pub(super) fn resolve_manifest_list_to_linux<I>(
    entries: I,
    target_arch: &str,
) -> Result<String>
where
    I: IntoIterator<Item = ManifestListEntry>,
{
    let entries: Vec<ManifestListEntry> = entries.into_iter().collect();
    if let Some(entry) = entries
        .iter()
        .find(|e| e.os == "linux" && e.architecture == target_arch)
    {
        return Ok(entry.digest.clone());
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

/// One entry in a manifest list / image index, in a form
/// decoupled from any specific crate's types. Owned strings —
/// the platform-resolver runs once per pull, so allocation cost
/// is negligible vs. the borrowed-view variant that complicated
/// the milestone-031 → 032 type-conversion path.
#[derive(Clone, Debug)]
pub(super) struct ManifestListEntry {
    pub digest: String,
    pub architecture: String,
    pub os: String,
}

#[cfg(test)]
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    fn entry(digest: &str, arch: &str, os: &str) -> ManifestListEntry {
        ManifestListEntry {
            digest: digest.to_string(),
            architecture: arch.to_string(),
            os: os.to_string(),
        }
    }

    #[test]
    fn picks_linux_amd64_when_present() {
        let entries = vec![
            entry("sha256:amd64", "amd64", "linux"),
            entry("sha256:arm64", "arm64", "linux"),
        ];
        let digest = resolve_manifest_list_to_linux(entries, "amd64").unwrap();
        assert_eq!(digest, "sha256:amd64");
    }

    #[test]
    fn picks_first_matching_when_multiples() {
        let entries = vec![
            entry("sha256:first", "amd64", "linux"),
            entry("sha256:second", "amd64", "linux"),
        ];
        let digest = resolve_manifest_list_to_linux(entries, "amd64").unwrap();
        assert_eq!(digest, "sha256:first");
    }

    #[test]
    fn errors_when_target_unavailable_and_lists_what_is() {
        let entries = vec![
            entry("sha256:arm64", "arm64", "linux"),
            entry("sha256:s390x", "s390x", "linux"),
        ];
        let err = resolve_manifest_list_to_linux(entries, "amd64").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("linux/amd64"), "missing target in message: {msg}");
        assert!(msg.contains("linux/arm64"), "missing available platform: {msg}");
        assert!(msg.contains("linux/s390x"), "missing available platform: {msg}");
    }

    #[test]
    fn skips_non_linux_os_entries() {
        let entries = vec![
            entry("sha256:darwin-arm64", "arm64", "darwin"),
            entry("sha256:linux-arm64", "arm64", "linux"),
        ];
        let digest = resolve_manifest_list_to_linux(entries, "arm64").unwrap();
        assert_eq!(digest, "sha256:linux-arm64");
    }
}
