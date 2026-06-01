# Contract: BitBake recipe walker (US4, FR-007, FR-008)

**New module**: `mikebom-cli/src/scan_fs/package_db/yocto/recipe.rs`

## Trigger

Any file matching `meta-*/recipes-*/<name>/<name>_<version>.bb` anywhere in the scan tree. The walker uses `walkdir` (workspace dep) bounded by a max-depth (8 — same convention as the other source-tree readers).

## Parsing

Filename-only — the `.bb` body is NOT parsed in this milestone (variable expansion is explicit out-of-scope per FR-008).

Regex applied to each candidate `.bb` filename (basename only):

```
^(?P<name>[a-zA-Z0-9_\-\+\.]+)_(?P<version>[a-zA-Z0-9_\-\+\.\~]+)\.bb$
```

`.bbappend` and `.bbclass` files are **not** matched and are silently skipped (they don't declare new components).

## Layer detection

Once a `.bb` is matched, the walker walks UP from the recipe's directory looking for the enclosing `meta-<name>/` directory (the layer root). The layer-root's basename becomes the `?layer=<layer_name>` qualifier on the emitted PURL.

If no `meta-*/` ancestor is found within the scan-target tree (the recipe lives outside any layer), the qualifier is omitted from the PURL and a `tracing::warn!` records the anomaly.

## PURL derivation

```
pkg:bitbake/<name>@<version>?layer=<layer_name>
```

Name and version percent-encoded via `encode_purl_segment`. Layer name passed verbatim.

## Unexpanded BitBake variables (FR-008)

Recipes whose filenames contain unexpanded BitBake expansion syntax (the literal sequence `${`) are skipped silently — `tracing::warn!` only, no component emitted, no `unresolved` sentinel. Examples that match the skip pattern:

```
${PN}_${PV}.bb
${PN}_1.0.bb
my-recipe_${PV}.bb
```

Per the 2026-06-01 clarification (Q2 Option A wins): emitting placeholder components pollutes downstream SBOMs without adding actionable signal.

## Annotations emitted (per component)

| Annotation | Value |
|---|---|
| `mikebom:source-files` | absolute path of the `.bb` recipe file |
| `mikebom:source-mechanism` | `"bitbake-recipe"` |
| `mikebom:layer-name` | the enclosing `meta-<name>/` directory's basename, if found |

## Edge cases

- Layer with no `meta-` prefix (vendor convention varies — Yocto's reference layer is `poky/meta/`, OpenSTLinux uses `meta-st-openstlinux/`, some shops use just `recipes-*/` at the repo root). The walker falls back: if no `meta-*/` ancestor found, the layer-name annotation is set to the path component immediately above the first `recipes-*/` directory.
- Recipes inside `bbappend` overlay dirs → NOT scanned (covered by the recipe with the same name in the original layer)
- Recipes with no `_<version>` segment (rare — e.g. `helloworld.bb`) → emit with `version: "unknown"` + `mikebom:version-status: "missing"` annotation
- Two recipes with the same name + version in different layers (a vendor layer that overrides an OE-core recipe) → BOTH emit as distinct components (different `?layer=` qualifiers); dedup pipeline does NOT collapse because the canonical PURL differs

## Tests

Per-module unit tests in `yocto/recipe.rs::tests`:
- `extracts_name_and_version_from_filename`
- `emits_layer_qualifier_from_meta_ancestor`
- `unexpanded_variables_skipped_silently`
- `version_only_filename_emits_unknown_version_annotation`
- `bbappend_and_bbclass_files_ignored`

Integration test at `mikebom-cli/tests/scan_yocto_recipe.rs`:
- End-to-end binary scan against `yocto_recipe_layer/` fixture (a tiny `meta-mikebom-fixture/` with a handful of `recipes-*/<name>/<name>_<version>.bb` files)
- Assertion: emitted CDX contains expected `pkg:bitbake/<name>@<version>?layer=meta-mikebom-fixture` PURLs
