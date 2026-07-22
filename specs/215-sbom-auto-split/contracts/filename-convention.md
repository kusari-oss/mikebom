# Contract: Sub-SBOM filename convention

**Feature**: 215-sbom-auto-split
**Kind**: Filesystem contract
**Consumers**: operators + CI scripts that glob for emitted sub-SBOMs; the split manifest's `entries[].files{}` map.

## Naming format

```
<slug>.<ecosystem>.<format-ext>.json
```

- `<slug>`: sanitized PURL name field
- `<ecosystem>`: PURL type
- `<format-ext>`: `cdx` | `spdx` | `spdx3`

## Slug derivation

Given a subproject PURL `pkg:<type>/<namespace?>/<name>@<version>`:

1. Start with `<name>`.
2. If `<namespace>` is present (scoped npm, Maven groupId), prepend as `<namespace>-<name>`:
   - `pkg:npm/@myorg/frontend@1.0.0` → intermediate slug `myorg-frontend`
   - `pkg:maven/com.example/my-lib@2.0.0` → `com.example-my-lib`
3. Apply character substitutions:
   - `/` → `-`
   - `@` → `at-` (only in edge cases where `@` survives step 2)
   - `\/:*?"<>|` → strip entirely
   - Whitespace → strip
   - Any non-ASCII → strip
4. Truncate to 100 chars (defensive against pathological input).
5. Lowercase.

## Ecosystem name mapping

Direct 1:1 from `Purl::ecosystem()`:

| PURL type | `<ecosystem>` value |
|---|---|
| `cargo` | `cargo` |
| `npm` | `npm` |
| `pypi` | `pypi` |
| `maven` | `maven` |
| `golang` | `go` |
| `gem` | `gem` |
| `swift` | `swift` |
| `nuget` | `nuget` |
| `composer` | `composer` |
| `hex` | `hex` |
| `hackage` | `hackage` |
| `pub` | `pub` |
| `cocoapods` | `cocoapods` |
| `deb` | `deb` |
| `rpm` | `rpm` |
| `apk` | `apk` |
| `alpm` | `alpm` |
| `bitbake` | `bitbake` |
| `opkg` | `opkg` |
| `brew` | `brew` |
| `generic` (fallback) | `generic` |

## Format extension mapping

| `--format` value | `<format-ext>` |
|---|---|
| `cyclonedx-json` | `cdx` |
| `spdx-2.3-json` | `spdx` |
| `spdx-3-json` | `spdx3` |

## Collision handling (per FR-011)

Two subprojects that would produce the same `<slug>.<ecosystem>` prefix (rare but possible — e.g., same-named packages in different workspace members) are disambiguated:

**Detection**: after computing all N slugs, scan for duplicates (case-sensitive, exact-prefix).

**Resolution**: append `-<8-hex-chars>` where the 8 hex chars are the first 8 characters of `SHA-256(source_dir_relative_to_scan_root)`. Applied to ALL colliding entries (not just the second one) so ordering doesn't affect which entry gets the suffix.

**Example**:
- Two subprojects: `libs/cli/foo/Cargo.toml` and `libs/tools/foo/Cargo.toml`, both `pkg:cargo/foo@1.0.0`
- Base slugs: both `foo.cargo`
- Collision detected
- SHA-256(`libs/cli/foo`) = `abc12345...` → slug becomes `foo-abc12345.cargo`
- SHA-256(`libs/tools/foo`) = `def67890...` → slug becomes `foo-def67890.cargo`
- Result: `foo-abc12345.cargo.cdx.json`, `foo-def67890.cargo.cdx.json`

Deterministic across scans (SHA is a function of the path).

## Example filenames

| PURL | Slug | Filename (cdx) |
|---|---|---|
| `pkg:cargo/libsafe@0.1.0` | `libsafe` | `libsafe.cargo.cdx.json` |
| `pkg:cargo/vuln-included@0.1.0` | `vuln-included` | `vuln-included.cargo.cdx.json` |
| `pkg:npm/@myorg/frontend@1.0.0` | `myorg-frontend` | `myorg-frontend.npm.cdx.json` |
| `pkg:pypi/my-service@2.5.0` | `my-service` | `my-service.pypi.cdx.json` |
| `pkg:golang/github.com%2Fkusari-oss%2Fapi@v1.2.3` | `github.com-kusari-oss-api` | `github.com-kusari-oss-api.go.cdx.json` |
| `pkg:maven/com.example/my-lib@1.0.0` | `com.example-my-lib` | `com.example-my-lib.maven.cdx.json` |

## Zero-boundary fallback filename (per R8)

When no workspace boundaries detected + `--split` is set, emit ONE sub-SBOM with slug `root`:

- `root.generic.cdx.json` (if the scan produced a synthetic-PURL root — m127 placeholder)
- `<detected-root-slug>.<detected-ecosystem>.<format>.json` (if the scan produced ONE main-module component but it wasn't tagged as a workspace root — e.g., single-package project)

No manifest is written in the zero-boundary case.

## Reserved names

The following filenames MUST NOT be generated as sub-SBOM outputs:
- `split-manifest.json` (reserved for the manifest itself)
- `.gitkeep`, `.gitignore` (reserved for operator conventions)

If a slug derivation happens to produce one of these names, apply the collision-resolution SHA-8-char suffix per the standard collision path.

## Filesystem safety

Filenames MUST be safe on Linux, macOS, and Windows filesystems:
- ASCII-only after sanitization
- No path separators (`/`, `\`) in the filename component
- No reserved Windows names (`CON`, `PRN`, `AUX`, `NUL`, `COM1..9`, `LPT1..9`) — detected by uppercased-basename comparison and, if matched, prefixed with `wb-` (e.g., `con.cargo.cdx.json` → `wb-con.cargo.cdx.json`)
- Length capped per slug rule (100 chars slug + ~20 chars ecosystem+format extension = ~120 chars total; well within all filesystem limits)

## Contract stability

- Filename format is a public interface — operators write CI scripts globbing on it.
- Adding new ecosystem types is non-breaking (existing filenames unchanged, new ecosystem gets a new suffix).
- Changing the slug-derivation algorithm is a BREAKING change; existing scripts that key on the derived name would fail. Requires a MAJOR version bump per constitution amendment procedure.
- Collision-resolution algorithm (SHA-8-char) is stable within v1; changing hash length or algorithm is breaking.
