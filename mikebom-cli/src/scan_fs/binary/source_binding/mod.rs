//! Source-tier ↔ binary-tier PURL attribution (milestone 109).
//!
//! Bridges the gap between mikebom's two SBOM-emission paths for the
//! same C/C++ library:
//! - **Source-tier**: the milestone-102/103 cmake reader emits
//!   `pkg:github/madler/zlib@v1.3.1` when it parses
//!   `FetchContent_Declare(zlib GIT_REPOSITORY ... GIT_TAG v1.3.1)`.
//! - **Binary-tier**: the milestone-099/108 symbol-fingerprint matcher
//!   emits `pkg:generic/zlib` when it sees zlib's exported-symbol
//!   set in a binary's dynamic symbol table.
//!
//! Pre-milestone-109, these two emissions don't equality-join — the
//! SBOM carries two components for the same library. This module
//! observes cmake's documented `_deps/<name>-build/` build-directory
//! layout to attribute the binary-tier fingerprint match to the
//! source-tier PURL.
//!
//! Architecture (per FR-012 forward-compat): the cmake-specific
//! path-observation logic lives in [`cmake_observer`]; the
//! attribution registry + matcher-rewrite plumbing in [`registry`]
//! are observer-agnostic. A future Bazel observer lands as a sibling
//! module (`bazel_observer.rs`) that implements the same
//! [`BuildDirObserver`] trait and feeds the same registry.
//!
//! See `specs/109-binary-source-purl-binding/`.

pub(crate) mod cmake_observer;
pub(crate) mod registry;

use std::path::{Path, PathBuf};

use super::super::package_db::PackageDbEntry;

pub(crate) use registry::BuildAttributionRegistry;

/// One per (cmake-project, cmake declaration) pair where the
/// corresponding `_deps/<name>-build/` directory exists. Computed
/// once per scan; consumed read-only by the per-binary matcher.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) struct CmakeBuildDirObservation {
    /// The library name VERBATIM as it appeared in the cmake
    /// declaration's first positional arg (e.g.,
    /// `FetchContent_Declare(zlib ...)` → `"zlib"`). NOT lowercased
    /// here; the lowercasing happens at registry-lookup time to keep
    /// the source-of-truth name faithful for diagnostics.
    pub library_name: String,

    /// The source-tier PURL the cmake reader emitted for this
    /// declaration. Drives the rewrite of the binary-tier match's
    /// PURL when attribution fires.
    pub source_tier_purl: String,

    /// The `mikebom:source-mechanism` enum value the cmake reader
    /// tagged this declaration with — one of `cmake-fetchcontent-git`
    /// / `cmake-fetchcontent-url`. Drives the merged component's
    /// source-mechanism annotation.
    pub source_mechanism: String,

    /// Absolute path of the `<cmake-project-build-dir>/_deps/<name>-build/`
    /// directory whose existence corroborates the binding. Confirmed
    /// to exist at observation time; MAY have disappeared by lookup
    /// time (rare race; the cached value still drives the rewrite).
    pub build_artifact_dir: PathBuf,

    /// Absolute path of the cmake-project build dir itself (the
    /// parent of `_deps/`). Used to constrain attribution to
    /// binaries under this project's build dir (per the multi-cmake-
    /// project scoping rule).
    pub cmake_project_build_root: PathBuf,
}

/// Observer-agnostic trait for future build-system extensions
/// (Bazel, Meson, etc.). The cmake observer is the only implementer
/// this milestone (per the Phase-2 clarification: ExternalProject,
/// Bazel, and Meson are all deferred). The trait is `pub(crate)`
/// because every implementer lives inside `mikebom-cli`.
#[allow(dead_code)]
pub(crate) trait BuildDirObserver {
    /// Walk `scan_root` and join the build-tree artifacts against
    /// the source-tier declarations the cmake / vcpkg / Conan / etc.
    /// readers already parsed. Each returned observation represents
    /// one verified source-tier ↔ build-artifact pairing.
    fn observe(
        &self,
        scan_root: &Path,
        source_declarations: &[PackageDbEntry],
    ) -> Vec<CmakeBuildDirObservation>;
}

/// Build the per-scan attribution registry from the cmake reader's
/// parsed declarations. Returns an empty registry when no cmake
/// projects exist in the scan root (the common no-cmake-project
/// case) — the matcher then falls through to the milestone-108
/// generic-PURL path naturally.
///
/// Called once at scan-start; the resulting registry is passed
/// by reference into the per-binary matcher loop.
#[allow(dead_code)]
pub(crate) fn build_attribution_registry(
    scan_root: &Path,
    source_declarations: &[PackageDbEntry],
) -> BuildAttributionRegistry {
    let observer = cmake_observer::CmakeFetchContentObserver;
    let observations = observer.observe(scan_root, source_declarations);
    BuildAttributionRegistry::from_observations(observations)
}
