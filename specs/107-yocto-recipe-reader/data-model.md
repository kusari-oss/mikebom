# Data Model — Yocto / OpenEmbedded Reader (107)

All entities below live in-process for the lifetime of a single scan (matches every milestone since 002). No persistence, no caches.

---

## `OpkgStanza`

In-memory representation of one parsed stanza from `/var/lib/opkg/status`. Fields not directly used by mikebom's component emission are dropped during parsing.

| Field | Type | Source field in opkg DB | Notes |
|---|---|---|---|
| `name` | `String` | `Package:` | Required. Empty value → skip stanza with warn. |
| `version` | `String` | `Version:` | Required. Empty value → emit with `mikebom:version-status: "missing"`. |
| `arch` | `String` | `Architecture:` | Required. Passed verbatim into PURL `?arch=` qualifier. |
| `maintainer` | `Option<String>` | `Maintainer:` | Flows to CDX `component.supplier.name`. |
| `license_raw` | `Option<String>` | `License:` | Verbatim string; downstream SPDX-expression pipeline handles parsing. |
| `depends_raw` | `Vec<String>` | `Depends:` | Comma-separated; tokens become `PackageDbEntry.depends`. Version constraints in parens are stripped (same as dpkg's reader). |
| `claimed_files_path` | `PathBuf` | derived from `name` | Computed as `/usr/lib/opkg/info/<name>.list` (where opkg writes per-package install manifests). Read separately by `collect_claimed_paths`. |
| `feed_filename` | `Option<String>` | `Filename:` | The `.ipk` URL/filename if present; recorded for feed-traceability annotations but not used as identity. |

**Validation rules**:

- Stanza with empty `Package:` → skip + `tracing::warn!`
- Stanza missing `Version:` → emit with `mikebom:version-status: "missing"` annotation
- Stanza missing `Architecture:` → fallback to `all` (matches dpkg's "any architecture" semantics)
- Multi-line `Description:` continuation lines correctly merged via `control_file.rs` parser

---

## `YoctoImageManifest`

Parsed contents of one `<image>.manifest` file. Represented as a `Vec<ManifestLine>` plus the source path.

| `ManifestLine` field | Type | Source format | Notes |
|---|---|---|---|
| `name` | `String` | line.split_whitespace()[0] | Required, non-empty. |
| `arch` | `String` | line.split_whitespace()[1] | Required. |
| `version` | `String` | line.split_whitespace()[2] | Required. |

**Validation rules**:

- Lines with wrong token count → skip + `tracing::warn!` with line number
- Empty lines + lines starting with `#` (defensive) → skipped silently
- File-level read failure → return empty `Vec` + `tracing::warn!` (warn-and-continue)

---

## `BitbakeRecipeFile`

In-memory representation of one `.bb` file's *filename-derived* identity. Recipe body is NOT parsed.

| Field | Type | Source | Notes |
|---|---|---|---|
| `name` | `String` | filename regex group 1 | The `<name>` segment before the first `_`. |
| `version` | `String` | filename regex group 2 | Everything between the `_` and `.bb`. May contain `+git<sha>` suffix. |
| `recipe_path` | `PathBuf` | `walkdir` entry | Absolute path to the `.bb` file. |
| `layer_root` | `PathBuf` | derived | The enclosing `meta-<name>/` directory, walked up from `recipe_path`. Used for the PURL's `?layer=` qualifier. |

**Validation rules**:

- Filename matches `^<name>_<version>.bb$` regex → emit
- Filename contains `${...}` → skip silently + `tracing::warn!` (per FR-008)
- Recipe with no `_<version>` segment → emit with `version: "unknown"` + `mikebom:version-status: "missing"` annotation

---

## `ScanContext`

Result of the sysroot-vs-rootfs heuristic. One value per scan target — consumed by `opkg.rs` to decide whether to tag emitted entries with `LifecycleScope::Build`.

```rust
pub enum ScanContext {
    /// Confirmed sysroot — env-script present OR include + no init.d.
    Sysroot { primary_signal: bool, secondary_signal: bool },
    /// Confirmed runtime rootfs — neither signal fires.
    Rootfs,
    /// Both signals fire, but in conflicting ways (env-script present AND init.d also present).
    /// Treated as sysroot per FR-005a (primary signal wins) but emits a
    /// `mikebom:scan-ambiguity` annotation on the SBOM metadata.
    AmbiguousSysroot { reason: String },
}
```

Used by `opkg.rs::read` to decide lifecycle-scope tagging:
- `Sysroot { .. }` or `AmbiguousSysroot { .. }` → tag every emitted entry with `LifecycleScope::Build`
- `Rootfs` → no scope tag (runtime default)

---

## `SourceMechanism` enum extension

Extends the existing milestone-105 source-mechanism enum at `mikebom-cli/src/scan_fs/dedup/source_mechanism.rs` (or equivalent). Three new variants:

| Variant | `canonical_str()` | Emitted by |
|---|---|---|
| `OpkgInstalled` | `"opkg-installed"` | `opkg.rs` |
| `YoctoImageManifest` | `"yocto-image-manifest"` | `yocto/manifest.rs` |
| `BitbakeRecipe` | `"bitbake-recipe"` | `yocto/recipe.rs` |

**Precedence** (per FR-010, applied by the milestone-105 dedup pipeline):

```
OpkgInstalled  >  YoctoImageManifest  >  BitbakeRecipe
```

Rationale: opkg-installed-DB observes packages **actually installed** on the device (highest authority). Image-manifest observes packages **intended for installation** (high authority — BitBake itself recorded these). Bitbake-recipe observes packages **declared by a layer** but not necessarily ever built (lower authority — a recipe in a layer might never have been selected by any image).

Same posture as milestone 105's existing precedence: explicit declarations (vcpkg-manifest, conanfile, west.yml, idf_component.yml) outrank filesystem-derived signals (git-submodule, cmake-vendored).

---

## `PackageDbEntry` field-mapping

The four new readers all populate the existing `PackageDbEntry` struct. Per-reader field-mapping:

### opkg.rs

| `PackageDbEntry` field | Source | Notes |
|---|---|---|
| `purl` | `Purl::new("pkg:opkg/{name}@{version}?arch={arch}")` | |
| `name` | `stanza.name` | |
| `version` | `stanza.version` | |
| `arch` | `Some(stanza.arch)` | Drives the `?arch=` qualifier. |
| `source_path` | `<rootfs>/var/lib/opkg/status` | |
| `depends` | `stanza.depends_raw` (tokenized) | Same shape as dpkg's reader. |
| `maintainer` | `stanza.maintainer` | |
| `licenses` | empty (parsed downstream from `license_raw`) | License-resolution pipeline already handles SPDX expressions. |
| `lifecycle_scope` | `Some(LifecycleScope::Build)` if `ScanContext` ≠ `Rootfs`, else `None` | |
| `sbom_tier` | `Some("deployed")` | Matches dpkg/apk/rpm. |
| `evidence_kind` | `Some("package-database")` (or equivalent existing variant) | |
| `source_type` | `None` | Not a non-registry source. |
| `extra_annotations` | `{"mikebom:source-mechanism": "opkg-installed"}` | Plus optional `mikebom:version-status` and `mikebom:feed-filename` when applicable. |

### yocto/manifest.rs

| `PackageDbEntry` field | Source | Notes |
|---|---|---|
| `purl` | `Purl::new("pkg:opkg/{name}@{version}?arch={arch}")` | Same PURL form as opkg-installed — they identify the same packages from different sources. |
| `name` | `manifest_line.name` | |
| `version` | `manifest_line.version` | |
| `arch` | `Some(manifest_line.arch)` | |
| `source_path` | `<scan-root>/build/tmp/deploy/images/<machine>/<image>.manifest` | |
| `lifecycle_scope` | `None` (manifest captures what's intended for the device — runtime) | Build-time variants (nativesdk-*) tagged via FR-006 name-prefix check. |
| `sbom_tier` | `Some("source")` | Pre-deploy artifact, not yet installed. |
| `extra_annotations` | `{"mikebom:source-mechanism": "yocto-image-manifest"}` | |

### yocto/recipe.rs

| `PackageDbEntry` field | Source | Notes |
|---|---|---|
| `purl` | `Purl::new("pkg:bitbake/{name}@{version}?layer={layer_root_name}")` | Distinct ecosystem from opkg-installed/manifest — recipes are declarations, not installed packages. |
| `name` | `recipe.name` | |
| `version` | `recipe.version` | |
| `arch` | `None` | Recipes are arch-agnostic (recipe expands per-MACHINE). |
| `source_path` | `recipe.recipe_path` | |
| `lifecycle_scope` | `None` | |
| `sbom_tier` | `Some("design")` | Lowest tier per R13 — declared but not necessarily built. |
| `extra_annotations` | `{"mikebom:source-mechanism": "bitbake-recipe", "mikebom:layer-name": "<layer_root_name>"}` | |

---

## State transitions

None. All reads are single-pass per scan; no entry mutates after emission. The dedup pipeline (milestone 105) is the only post-emission collator and it operates on the assembled `Vec<PackageDbEntry>` after all readers run.

---

## Data-volume assumptions

| Scenario | Expected entry count |
|---|---|
| Yocto qemux86-64 reference image rootfs | ~250 |
| OpenSTLinux 6.6 SDK sysroot | ~400 |
| `core-image-sato` Yocto desktop image | ~1500 |
| Layer tree scan (mainline OE-core) | ~1200 recipes |
| Layer tree scan (single vendor layer) | ~50–150 recipes |

All linear in source-file count. No quadratic or higher scaling.
