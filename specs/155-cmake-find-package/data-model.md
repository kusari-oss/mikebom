# Data Model — milestone 155

Phase 1 output. Types + wire shapes introduced or extended by the CMake `find_package` + `pkg_check_modules` extraction.

## 1. Internal parser types (`cmake.rs`, private to the module)

### `FindPackageHit` — one per successfully matched `find_package(<Name> [<Version>])` call site

```rust
struct FindPackageHit {
    /// Package name, lowercased for PURL emission (FR-008).
    lowercased_name: String,
    /// Original casing as it appeared in the CMake source file.
    /// Only preserved separately for later use in the
    /// `mikebom:cmake-find-package-name` annotation, and only when
    /// it differs from `lowercased_name`.
    original_casing: String,
    /// Declared version constraint from the second positional argument
    /// of `find_package`, if present. Verbatim string; not parsed.
    declared_version: Option<String>,
    /// Absolute path (as string) of the CMakeLists.txt or .cmake file
    /// where this call site was matched.
    source_path: String,
}
```

### `PkgCheckHit` — one per module in a `pkg_check_modules` / `pkg_search_module` module list

```rust
struct PkgCheckHit {
    /// pkg-config module name after stripping any `>=X.Y` / `<X.Y` /
    /// `=X.Y` version-comparator suffix. Lowercased for PURL emission.
    lowercased_module: String,
    /// Original casing as it appeared in the CMake source file.
    /// Only used for annotation attachment when differs from lowercased.
    original_casing: String,
    /// Absolute path of the CMake file where the pkg_check_modules /
    /// pkg_search_module call was matched.
    source_path: String,
}
```

**Lifetime**: created during the per-file loop in `read()`, accumulated across all discovered CMake files, then consumed by the two `emit_*_entries` functions. Discarded when `read()` returns.

## 2. `PackageDbEntry` extensions

**No struct-level changes.** The existing `PackageDbEntry` at `mikebom-cli/src/scan_fs/package_db/mod.rs` is unchanged. Milestone 155 populates it with new value combinations:

| Field | Value for `find_package` emissions | Value for `pkg_check_modules` emissions |
|-------|------------------------------------|------------------------------------------|
| `purl` | `Purl::new("pkg:generic/<lowercased_name>[@<highest_declared_version>]")` | `Purl::new("pkg:generic/<lowercased_module>")` |
| `name` | `lowercased_name` (the emitted PURL name segment) | `lowercased_module` |
| `version` | `highest_declared_version` (chosen per R3) OR empty string | empty string (pkg-config version constraints not preserved) |
| `arch` | `None` | `None` |
| `source_path` | The per-hit `source_path` (one entry per call site → one path per entry; downstream milestone-148 unions when merged) | The per-hit `source_path` |
| `depends` | `vec![]` (find_package doesn't declare edges) | `vec![]` |
| `maintainer` | `None` | `None` |
| `licenses` | `vec![]` | `vec![]` |
| `lifecycle_scope` | `None` (see §3.3 rationale) | `None` |
| `requirement_range` | `None` | `None` |
| `source_type` | `None` | `None` |
| `buildinfo_status` | `None` | `None` |
| `evidence_kind` | `Some("declared")` (R10) | `Some("declared")` |
| `binary_class` | `None` | `None` |
| `binary_stripped` | `None` | `None` |
| `linkage_kind` | `None` | `None` |
| `detected_go` | `None` | `None` |
| `confidence` | `None` | `None` |
| `binary_packed` | `None` | `None` |
| `raw_version` | `Some(<declared_version>)` when a version was declared but was NOT chosen as the highest (i.e., the site's version is smaller than the group's winning version); preserves per-site version for downstream forensics. `None` when the site's version equals the winning version. | `None` |
| `parent_purl` | `None` | `None` |
| `npm_role` | `None` | `None` |
| `co_owned_by` | `None` | `None` |
| `hashes` | `vec![]` | `vec![]` |
| `sbom_tier` | `Some("source")` (R11) | `Some("source")` |
| `shade_relocation` | `None` | `None` |
| `extra_annotations` | see §3 | see §3 |
| `binary_role` | `None` | `None` |

## 3. `extra_annotations` — key → value shape

### 3.1 — `mikebom:source-mechanism` (existing key, new open-enum values)

**For `find_package` emissions**:

```json
"mikebom:source-mechanism": "cmake-find-package"
```

**For `pkg_check_modules` + `pkg_search_module` emissions**:

```json
"mikebom:source-mechanism": "cmake-pkg-check-modules"
```

Both flow through the production `resolve::deduplicator` pass at `mikebom-cli/src/resolve/deduplicator.rs:28` unchanged. When two same-namespace entries (e.g., `cmake-find-package` + `cmake-fetchcontent-url`, both emitting `pkg:generic/openssl@1.1.0`) share the same canonical PURL, the deduplicator merges them into one `ResolvedComponent`. The winner's `mikebom:source-mechanism` value survives; the milestone-109 `extra_annotations` merge at `deduplicator.rs:190-209` folds non-conflicting loser annotations in but does NOT emit a `mikebom:also-detected-via` list (that list is a milestone-105 `scan_fs::dedup` construct which remains `#[allow(dead_code)]` at production emission time). Cross-namespace entries (e.g., `pkg:generic/openssl` from cmake vs `pkg:deb/debian/libssl3` from dpkg) do NOT merge under the current production dedup path — that requires milestone-111 `--pkg-alias-binding` OR a milestone-105 completion follow-up wiring `scan_fs::dedup` in.

### 3.2 — `mikebom:cmake-find-package-name` (NEW annotation key)

**Only emitted when** `original_casing != lowercased_name` — i.e., when the CMake file used mixed-case or all-caps for the package name. This keeps SBOMs from carrying redundant "OpenSSL" → "openssl" traceability when the CMake declaration was already all-lowercase.

**Shape**: JSON string, the original casing verbatim.

```json
"mikebom:cmake-find-package-name": "OpenSSL"
```

**Rationale for optional emission**: Constitution Principle V (standards-native first) — this annotation carries no equivalent construct in any of the three target formats. It's a parity-bridging annotation whose sole purpose is source-fidelity traceability. Emitting it universally would clutter SBOMs of all-lowercase-CMake-projects (which many are) with redundant `mikebom:cmake-find-package-name: "openssl"` entries.

**Emitted for**: `find_package` emissions only (FR-008). NOT emitted for `pkg_check_modules` emissions — pkg-config module names are conventionally lowercase-with-hyphens per the pkg-config spec, so the case-preservation traceability doesn't apply.

### 3.3 — No `mikebom:lifecycle-scope` annotation

**Decision**: Do not set `lifecycle_scope` for milestone-155 emissions. `find_package` declarations don't distinguish runtime vs build-time in the CMakeLists.txt itself (that's a CMake resolution-time concern based on which target the package is linked into). Emitting `lifecycle-scope: runtime` uniformly would be a factual overstatement; emitting `build` would be equally wrong for real runtime deps like OpenSSL.

**Consequence**: SPDX 2.3 emissions get no `DEV/BUILD/TEST_DEPENDENCY_OF` relationship for these deps; SPDX 3 emits `LifecycleScopeType::design` per the default fallback. Downstream consumers wanting scope classification for CMake-derived deps must layer their own heuristic (out of scope for milestone 155, natural follow-up if operator demand surfaces).

## 4. `SourceMechanism` open-enum extension

**File**: no dedicated enum type in the codebase — the mechanism is a raw string tag in `extra_annotations`. Milestone 155 adds two values to the informal enum documented at `cmake.rs:431-433`:

```
Existing values:
  cmake-fetchcontent-git
  cmake-fetchcontent-url
  cmake-externalproject
  cmake-vendored
  bazel-http-archive
  vcpkg-manifest
  conan-recipe

Added by milestone 155:
  cmake-find-package
  cmake-pkg-check-modules
```

The doc comment at `cmake.rs:431-433` is updated to reflect the two new values.

## 5. `evidence.source_file_paths` merge behavior (unchanged)

**No code change in milestone 155.** The milestone-148 union pass in `mikebom-common/src/resolution/*` handles multi-site source-path merging automatically when multiple `PackageDbEntry` instances share the same PURL. Milestone 155's emission strategy (one entry per call site with the group's chosen highest version) ensures this pipeline is exercised — one call site's `source_path` becomes one element in the merged `ResolvedComponent.evidence.source_file_paths` Vec.

**Verification**: R6 test #4 asserts this end-to-end (two files declaring OpenSSL at different versions → two emitted entries → milestone-148 union merges to one ResolvedComponent with two source paths).

## 6. CDX + SPDX 2.3 + SPDX 3 output shape (unchanged)

**No emitter code changes.** All three emitters read the merged `ResolvedComponent` and its `extra_annotations` bag. The new `cmake-find-package` / `cmake-pkg-check-modules` mechanism values flow through the existing property-emission path; the new `mikebom:cmake-find-package-name` annotation likewise. Wire-format-agnostic transformation is guaranteed by the emitter architecture (each emitter walks `extra_annotations` uniformly and emits `properties[]` / `annotations[]` per format convention).

### Example CDX output (for `find_package(OpenSSL 1.1.0)` in one file + `find_package(OpenSSL 3.0)` in another)

```json
{
  "type": "library",
  "bom-ref": "pkg:generic/openssl@3.0",
  "name": "openssl",
  "version": "3.0",
  "purl": "pkg:generic/openssl@3.0",
  "properties": [
    {"name": "mikebom:source-mechanism", "value": "cmake-find-package"},
    {"name": "mikebom:cmake-find-package-name", "value": "OpenSSL"}
  ],
  "evidence": {
    "occurrences": [
      {"location": "/scan/cmake/defs.cmake"},
      {"location": "/scan/cmake/modules/FindOpenSSL.cmake"}
    ]
  }
}
```

### Example SPDX 2.3 output

```json
{
  "SPDXID": "SPDXRef-Package-generic-openssl-3.0",
  "name": "openssl",
  "versionInfo": "3.0",
  "downloadLocation": "NOASSERTION",
  "filesAnalyzed": false,
  "externalRefs": [
    {"referenceCategory": "PACKAGE-MANAGER", "referenceType": "purl",
     "referenceLocator": "pkg:generic/openssl@3.0"}
  ],
  "annotations": [
    {"annotationType": "OTHER", "annotator": "Tool: mikebom",
     "comment": "{\"mikebom:source-mechanism\":\"cmake-find-package\",\"mikebom:cmake-find-package-name\":\"OpenSSL\"}"}
  ]
}
```

### Example SPDX 3 output (`@graph` element)

```json
{
  "type": "software_Package",
  "spdxId": "https://mikebom.kusari.dev/spdx3/doc-<hash>/pkg/generic-openssl-3.0",
  "name": "openssl",
  "packageVersion": "3.0",
  "externalIdentifier": [
    {"externalIdentifierType": "purl",
     "identifier": "pkg:generic/openssl@3.0"}
  ]
}
```

Plus an SPDX 3 `Annotation` element carrying the two `mikebom:*` properties.
