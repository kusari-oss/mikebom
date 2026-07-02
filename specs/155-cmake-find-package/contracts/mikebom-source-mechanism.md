# Contract — `mikebom:source-mechanism` open-enum extension

**Feature**: milestone 155 — CMake `find_package` + `pkg_check_modules` extraction
**Scope**: extending the informal open-enum documented at `mikebom-cli/src/scan_fs/package_db/cmake.rs:431-433`.

## Summary

Milestone 155 adds two new values to the existing open-enum string tag `mikebom:source-mechanism`:

- `cmake-find-package` — for `find_package(<Name> [<Version>])` extractions.
- `cmake-pkg-check-modules` — for `pkg_check_modules(<TARGET> <modules>)` AND `pkg_search_module(<TARGET> <modules>)` extractions (both mapped to the same value per FR-004).

Both values are:
- **Additive**: no existing value is renamed, deprecated, or removed.
- **Open-enum-compliant**: the milestone-105 dedup pipeline treats `mikebom:source-mechanism` as an open-enum string; unknown values are preserved verbatim in the merged component's `mikebom:also-detected-via` list.

## Consumer contract

Consumers filtering by `mikebom:source-mechanism` can rely on:

1. **Value stability**: `cmake-find-package` + `cmake-pkg-check-modules` are permanent as of milestone 155. Any future rename would be a breaking wire-contract change requiring a follow-up milestone with explicit backwards-compat handling.

2. **Same-PURL cross-mechanism dedup**: when a `cmake-find-package` component and a `cmake-fetchcontent-url` (or another same-`pkg:generic/`-namespace mechanism) share the same canonical PURL string, the production `resolve::deduplicator` pipeline produces exactly ONE merged component. The surviving `mikebom:source-mechanism` value is determined by the deduplicator's confidence-based tie-break — this contract does NOT prescribe which mechanism "wins" a tie. Consumers wanting the multi-mechanism transparency signal should watch for a future milestone-105-completion release that wires the `scan_fs::dedup` open-enum pipeline into production emission (adding `mikebom:also-detected-via`).

3. **Cross-namespace merging is NOT provided**: `cmake-find-package` emits `pkg:generic/<name>@<ver>`. Other-namespace PURLs like `pkg:deb/debian/<pkg>@<ver>` (from dpkg) or `pkg:rpm/<distro>/<pkg>@<ver>` (from rpm) will NOT merge with cmake-find-package emissions under the current production dedup path. Operators wanting cross-namespace dedup should use the milestone-111 `--pkg-alias-binding` CLI flag to declare aliases explicitly, or wait for the milestone-105 `scan_fs::dedup` completion follow-up.

4. **Multi-file same-mechanism**: when the same package is declared via `find_package` in multiple CMake files (same mechanism), the resulting merged component surfaces every source-file path in `evidence.occurrences[].location` (CDX) / `evidence.source_file_paths` (SPDX 2.3 + SPDX 3) via the milestone-148 union pass — with only ONE `mikebom:source-mechanism` value.

5. **PURL type is `pkg:generic/<lowercased-name>`**: neither value implies a more specific PURL type. Downstream vulnerability + license enrichment tools should treat `pkg:generic/openssl@3.0` from `cmake-find-package` identically to `pkg:generic/openssl@3.0` from any other source.

## Provider contract (milestone 155 emissions)

Milestone 155 guarantees:

1. **Every entry emitted by `parse_find_package_calls`** carries exactly one `mikebom:source-mechanism` property with value `cmake-find-package`.

2. **Every entry emitted by `parse_pkg_check_modules_calls`** carries exactly one `mikebom:source-mechanism` property with value `cmake-pkg-check-modules`. This value covers BOTH `pkg_check_modules` and `pkg_search_module` origins — consumers cannot distinguish the two from the emitted SBOM (by design; the two macros are semantic siblings per CMake's pkg-config module docs).

3. **No other `mikebom:source-mechanism` values are emitted** by this milestone's new code paths. The existing values (`cmake-fetchcontent-git`, `cmake-fetchcontent-url`, `cmake-externalproject`, `cmake-vendored`) continue to be emitted unchanged by the pre-milestone-155 code paths.

## Wire example — CDX

```json
{
  "properties": [
    {"name": "mikebom:source-mechanism", "value": "cmake-find-package"}
  ]
}
```

## Wire example — SPDX 2.3

Serialized inside the JSON blob of an `annotations[].comment` value (mikebom's SPDX 2.3 annotation format):

```json
{
  "annotationType": "OTHER",
  "annotator": "Tool: mikebom",
  "comment": "{\"mikebom:source-mechanism\":\"cmake-find-package\"}"
}
```

## Wire example — SPDX 3

Emitted as a separate `Annotation` element in the document `@graph`:

```json
{
  "type": "Annotation",
  "annotationType": "other",
  "subject": "<pkg-element-spdxId>",
  "statement": "{\"mikebom:source-mechanism\":\"cmake-find-package\"}"
}
```

## Verification

SC-004's integration test (`mikebom-cli/tests/cmake_find_package_kamailio_shape_integration.rs`) exercises the wire contract by:

1. Synthesizing a minimal CMakeLists.txt + `cmake/defs.cmake` fixture tree.
2. Running `mikebom sbom scan --format cyclonedx-json,spdx-2.3-json,spdx-3-json`.
3. Asserting each output format contains a `mikebom:source-mechanism = cmake-find-package` property/annotation for the emitted OpenSSL component.

SC-003's integration test (`mikebom-cli/tests/cmake_find_package_dedup_integration.rs`) verifies the same-PURL cross-mechanism dedup behavior by synthesizing a scan target with a CMakeLists.txt containing `find_package(openssl 1.1.0)` AND a `cmake/deps.cmake` containing `FetchContent_Declare(openssl URL ...openssl-1.1.0.tar.gz)`. Both mechanisms produce identical PURLs (`pkg:generic/openssl@1.1.0`) and the production `resolve::deduplicator` merges them into ONE component with either mechanism value (winner is confidence-tie-break-dependent, not prescribed).
