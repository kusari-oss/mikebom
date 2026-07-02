# Contract — `mikebom:cmake-find-package-name` (NEW annotation key)

**Feature**: milestone 155 — CMake `find_package` extraction
**Scope**: introduces one new `mikebom:*`-prefixed property/annotation key.

## Summary

Milestone 155 adds one new annotation key: `mikebom:cmake-find-package-name`. It carries the original casing of a package name extracted from a CMake `find_package(<Name>)` call, preserved as a JSON string.

Its purpose is **source-fidelity traceability** — the PURL emission lowercases the name per PURL convention (FR-008), which loses information about the original casing in the CMakeLists.txt. Preserving it as a separate annotation lets operators pattern-match against the exact CMake declaration form during audit.

## Constitution Principle V audit

Per Constitution Principle V (standards-native fields take precedence over `mikebom:*` properties):

| Format | Native construct for "original casing of a name we normalized"? | Result |
|--------|---------------------------------------------------------------|--------|
| CycloneDX 1.6 | No — `component.name` is a single normalized string; no case-preservation field exists. | `mikebom:*` annotation required |
| SPDX 2.3 | No — `Package.name` is a single normalized string. | `mikebom:*` annotation required |
| SPDX 3.0.1 | No — `Package.name` is a single normalized string; `software_ContentIdentifier` covers hashes, not case-preservation. | `mikebom:*` annotation required |

**Conclusion**: this annotation is a **parity-bridging annotation** — no native construct in any target format carries the semantic. Emission is permitted per Principle V's parity-bridging exception.

**Catalog documentation deferral**: `docs/reference/sbom-format-mapping.md` is NOT updated in milestone 155 per FR-015 + SC-007. The catalog row addition is a natural follow-up docs-refresh milestone, matching prior additive-annotation precedent (e.g., milestone 105's `mikebom:source-mechanism` catalog row was added in a follow-up docs-refresh).

## Emission rules

### When emitted

Milestone 155 emits `mikebom:cmake-find-package-name` **only when**:

1. The extraction came from `find_package(<Name> [<Version>])` (NOT from `pkg_check_modules` — pkg-config module names are conventionally lowercase-with-hyphens per the pkg-config spec, so case-preservation traceability doesn't apply).
2. AND the original casing differs from the lowercased PURL name segment — i.e., the CMake declaration used mixed-case (`OpenSSL`), all-caps (`BOOST`), or any non-all-lowercase form.

### When NOT emitted

- Declarations where the CMake source already used all-lowercase for the package name (e.g., `find_package(openssl 1.1.0)` — no case-preservation traceability needed).
- All `pkg_check_modules` / `pkg_search_module` emissions (see above).

### Rationale for conditional emission

Emitting universally would clutter SBOMs of all-lowercase-CMake-projects with redundant `mikebom:cmake-find-package-name: "openssl"` entries. The conditional emission keeps the annotation load-bearing — its presence signals "this name was normalized; here's the original."

## Value shape

**Type**: JSON string.
**Content**: the exact original substring matched by the `find_package` regex's capture group 1 (see research §R2.1).

**No escaping needed** — the CMake identifier alphabet `[A-Za-z0-9_:.+-]+` produces strings that are valid JSON string values without special-character escaping.

## Consumer contract

Consumers may rely on:

1. **String-valued**: the annotation value is always a JSON string, never a number, boolean, object, or array.
2. **Non-empty**: when emitted, the value is at least 1 character (extraction requires a non-empty capture).
3. **Matches PURL name (case-insensitive)**: `lowercased(annotation_value) == purl.name` always holds — the annotation IS the source of the PURL's name after lowercasing.
4. **No newlines**: the CMake identifier alphabet excludes whitespace + newlines.

## Consumer non-contract

Consumers MUST NOT rely on:

1. Absence-of-annotation implying "name was originally lowercase" vs "name was normalized to lowercase" — in future milestones the emission rule may be tightened or loosened. Absence is safest interpreted as "casing information not preserved for this component."
2. Value length bounds — CMake identifiers can be arbitrarily long.
3. Character-set bounds beyond `[A-Za-z0-9_:.+-]` — the underlying capture regex may be broadened in future milestones (e.g., to accept `<` and `>` in identifier names if some future CMake grammar extension permits).

## Wire example — CDX

```json
{
  "properties": [
    {"name": "mikebom:source-mechanism", "value": "cmake-find-package"},
    {"name": "mikebom:cmake-find-package-name", "value": "OpenSSL"}
  ]
}
```

## Wire example — SPDX 2.3

```json
{
  "annotationType": "OTHER",
  "annotator": "Tool: mikebom",
  "comment": "{\"mikebom:source-mechanism\":\"cmake-find-package\",\"mikebom:cmake-find-package-name\":\"OpenSSL\"}"
}
```

## Wire example — SPDX 3

```json
{
  "type": "Annotation",
  "annotationType": "other",
  "subject": "<pkg-element-spdxId>",
  "statement": "{\"mikebom:cmake-find-package-name\":\"OpenSSL\"}"
}
```

## Verification

- R6 test #2 (`find_package_with_version_emits_at_version`) asserts the annotation is present for a `find_package(OpenSSL 1.1.0)` declaration with value `"OpenSSL"`.
- R6 test #3 (`find_package_case_normalization`) asserts the annotation is present for `find_package(BOOST 1.75.0)` with value `"BOOST"`.
- R6 test #1 (`find_package_simple_no_version_emits_pkg_generic`) asserts the annotation IS present (input `Foo`, capitalized → normalized).
- A negative test (implicit in test #9's assertions) asserts the annotation is NOT emitted for `pkg_check_modules` extractions.
- An additional negative test: `find_package_all_lowercase_no_annotation` — input `find_package(zlib 1.2.11)` → no `mikebom:cmake-find-package-name` annotation on the emitted entry.
