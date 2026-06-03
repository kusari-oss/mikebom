# Data model: milestone 109 — binary-source PURL binding

## CmakeBuildDirObservation

One per (cmake-project root, cmake declaration) pair encountered during the build-dir walk.

```rust
pub(crate) struct CmakeBuildDirObservation {
    /// The library name as it appeared in the cmake declaration's
    /// FIRST positional arg (e.g., `FetchContent_Declare(zlib ...)` →
    /// "zlib"). NOT lowercased here; lowercasing happens at the
    /// registry-lookup callsite to keep the source-of-truth name
    /// faithful for diagnostics.
    pub library_name: String,

    /// The source-tier PURL the cmake reader emitted for this
    /// declaration (e.g., "pkg:github/madler/zlib@v1.3.1"). Drives
    /// the rewrite of the binary-tier match's PURL when attribution
    /// fires.
    pub source_tier_purl: String,

    /// The source-mechanism enum value the cmake reader tagged this
    /// declaration with — one of `cmake-fetchcontent-git` /
    /// `cmake-fetchcontent-url`. Drives the merged component's
    /// `mikebom:source-mechanism` annotation.
    pub source_mechanism: String,

    /// Absolute path of the `<cmake-project-build-dir>/_deps/<name>-build/`
    /// directory whose existence corroborates the binding. Confirmed
    /// to exist at observation time (`Path::is_dir()`); MAY have
    /// disappeared by the time the matcher reads the registry (rare
    /// — only if a concurrent process is cleaning up the build dir).
    pub build_artifact_dir: PathBuf,

    /// Absolute path of the cmake-project build dir itself (the parent
    /// of `_deps/`). Used to constrain attribution to binaries under
    /// this project's build dir (per R4 scoping rule).
    pub cmake_project_build_root: PathBuf,
}
```

**Validation rules**:
- `library_name` MUST be non-empty (cmake declarations always have a non-empty first positional arg; the reader rejects malformed declarations upstream).
- `source_tier_purl` MUST start with `pkg:` and parse via `mikebom_common::types::purl::Purl::new`.
- `source_mechanism` MUST be one of the closed enum values `cmake-fetchcontent-git` / `cmake-fetchcontent-url`. `cmake-externalproject` is deliberately NOT a valid value this milestone per the Phase-2 clarification.
- `build_artifact_dir` MUST exist as a directory at observation time.
- `cmake_project_build_root` MUST be a path-ancestor of `build_artifact_dir`.

**Lifecycle**: computed exactly once per scan, before the binary walker iterates files. Stored in the `BuildAttributionRegistry` (below). Read-only after construction.

## BuildAttributionRegistry

In-process per-scan lookup table. ONE instance per scan invocation.

```rust
pub(crate) struct BuildAttributionRegistry {
    /// Lookup: (library_name_lowercased, scope_path_ancestor) →
    /// CmakeBuildDirObservation. The scope path is the
    /// `cmake_project_build_root` from the observation; lookup
    /// callers pass the binary's path and the registry walks
    /// ancestors to find the matching scope.
    by_library: BTreeMap<String, Vec<CmakeBuildDirObservation>>,
}
```

**Validation rules**:
- `by_library` keys MUST be lowercase ASCII (the join key per the Phase-1 clarification).
- A single key MAY map to multiple observations (one per cmake project in the scan root); the lookup helper picks the observation whose `cmake_project_build_root` is the closest path-ancestor of the binary being scanned (per R4 scoping).

**Lifecycle**: built once at scan-start by `source_binding::build_attribution_registry(scan_root, cmake_declarations)`. Passed by reference to the per-binary loop. Dropped at scan end.

**Lookup contract**: `registry.lookup(library_name: &str, binary_path: &Path) -> Option<&CmakeBuildDirObservation>` returns:
- The matching observation when (lowercased) `library_name` is a key AND one of the observations under that key has a `cmake_project_build_root` that's a path-ancestor of `binary_path`.
- `None` otherwise (the matcher falls back to milestone-108 generic-PURL behavior).

## SymbolFingerprintMatch (existing — extended)

The milestone-099/108 `SymbolFingerprintMatch` struct (already in `symbol_fingerprint.rs`) gains NO new fields. Its existing `target_purl: String` field is rewritten in place when attribution fires (by `scan_with_corpus` after the matcher loop, before returning).

```rust
// Existing (unchanged):
pub struct SymbolFingerprintMatch {
    pub library: String,
    pub matched_count: usize,
    pub total_count: usize,
    pub target_purl: String,                       // ← rewritten in-place when attribution fires
    pub corpus_sha_annotation: Option<String>,
    pub also_detected_via: Vec<String>,
}
```

**Validation rules for the rewrite**:
- The rewrite happens ONLY when `scan_with_corpus` is called with `Some(&registry)` AND the registry's `lookup(match.library, current_binary_path)` returns `Some(_)`.
- Before-rewrite value: `pkg:generic/<library>` (the milestone-108 emission).
- After-rewrite value: `observation.source_tier_purl` (the cmake-derived PURL).
- The match's `library` field is NOT changed (keeps the fingerprint library-name source-of-truth intact for diagnostics).

## Relationship summary

```
scan-start:
  cmake_reader::read(scan_root)       → Vec<PackageDbEntry> (source-tier components)
                                          │
                                          ▼
  source_binding::build_attribution_registry(scan_root, cmake_declarations)
                                          │
                                          ▼
                                BuildAttributionRegistry
                                          │
per-binary loop:                          │
  scan_binary(binary_path)        ─────────────► symbol_names: Vec<String>
                                          │            │
                                          │            ▼
                                          └──► symbol_fingerprint::scan_with_corpus(
                                                  symbol_names,
                                                  corpus,
                                                  stamp_corpus_sha,
                                                  Some(&registry),  ← NEW optional param
                                                  binary_path,      ← NEW for scope lookup
                                              ) → Vec<SymbolFingerprintMatch>
                                                          │
                                                          │  match.target_purl rewritten
                                                          │  when registry.lookup() hits
                                                          ▼
                                              entry::symbol_match_to_entry(&match, ...)
                                                  → PackageDbEntry (with attributed PURL)
                                                          │
                                                          ▼
scan-end:                       per-component dedup pipeline (milestone 105)
                                  merges source-tier + binary-tier
                                  components sharing the same PURL
                                          │
                                          ▼
                                  emit final SBOM
```

## State transitions

None. All state is immutable after construction. The `BuildAttributionRegistry` is built once, read many times, dropped at scan end.

## Cardinality summary

| Entity | Cardinality per scan |
|---|---|
| `CmakeBuildDirObservation` | 0..N, where N = number of cmake `FetchContent_Declare` declarations whose `_deps/<name>-build/` exists |
| `BuildAttributionRegistry` | exactly 1 (always — empty when no cmake declarations exist) |
| `SymbolFingerprintMatch` (with rewritten PURL) | 0..M, where M = number of binary fingerprint matches whose library name + binary path satisfy `registry.lookup()` |
