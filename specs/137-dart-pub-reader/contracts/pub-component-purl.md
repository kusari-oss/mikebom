# Contract — `pkg:pub/*` and `pkg:generic/*` component PURL

The only wire-format contract this feature introduces. Per Constitution Principle V audit (research §R1):

- `pkg:pub/` is **purl-spec-blessed** ([pub-definition.md](https://github.com/package-url/purl-spec/blob/main/types-doc/pub-definition.md)) — used for hosted, git, and sdk source types.
- `pkg:generic/` is the placeholder for path source type (purl-spec does not define a `pub-path` type).
- The source-type discriminator surfaces via the existing parity-catalog C1 row (`mikebom:source-type` annotation) — no new C-row added; Dart contributes new VALUES (`pub-hosted` / `pub-git` / `pub-path` / `pub-sdk` / `pub-main-module`) to C1's value set.

## Wire shapes per source

### Hosted (default pub.dev)

```text
pkg:pub/<package-name>@<version>
```

### Hosted (self-hosted mirror)

```text
pkg:pub/<package-name>@<version>?repository_url=<base-url-with-scheme>
```

### Git

```text
pkg:pub/<package-name>@<resolved-40-char-sha>?vcs_url=git+<git-remote-url>[#<subpath>]
```

- `vcs_url` MUST carry the `git+` scheme prefix (cross-type purl-spec convention).
- `#<subpath>` is the `description.path` from the lockfile when non-trivial (not `.` and not empty).

### Path

```text
pkg:generic/<package-name>@<version>
```

(plus `mikebom:source-type = "pub-path"` annotation as discriminator)

### SDK pseudo-deps

```text
pkg:pub/<sdk-name>@0.0.0
```

(the `0.0.0` is literal — purl-spec canonical example. Plus `mikebom:source-type = "pub-sdk"` annotation.)

### Main-module (per FR-012)

```text
pkg:pub/<pubspec.yaml.name>@<pubspec.yaml.version-or-"0.0.0-unknown">
```

(plus `mikebom:component-role = "main-module"` + `mikebom:source-type = "pub-main-module"` annotations)

## Examples

| Scan input | Emitted PURL |
|---|---|
| `pubspec.yaml`: `name: my_flutter_app`, `version: 1.2.3` | `pkg:pub/my_flutter_app@1.2.3` (main-module) |
| `pubspec.lock`: `http` from `pub.dev`, version `1.1.0` | `pkg:pub/http@1.1.0` |
| `pubspec.lock`: `internal_lib` from `https://pub.acme.example.com`, version `2.0.0` | `pkg:pub/internal_lib@2.0.0?repository_url=https://pub.acme.example.com` |
| `pubspec.lock`: `window_size`, git from `github.com/google/flutter-desktop-embedding.git`, `description.path = "plugins/window_size"`, resolved-ref `eb39649...3d5601` | `pkg:pub/window_size@eb39649...3d5601?vcs_url=git+https://github.com/google/flutter-desktop-embedding.git#plugins/window_size` |
| `pubspec.lock`: `my_local_lib`, path `"../packages/my_local_lib"`, version `0.1.0` | `pkg:generic/my_local_lib@0.1.0` (with `mikebom:source-type = "pub-path"`) |
| `pubspec.lock`: `flutter`, source sdk, version `"0.0.0"` | `pkg:pub/flutter@0.0.0` (with `mikebom:source-type = "pub-sdk"`) |
| `pubspec.lock`: `flutter_test`, source sdk, version `"0.0.0"` | `pkg:pub/flutter_test@0.0.0` (with `mikebom:source-type = "pub-sdk"`) |

## Per-format emission

### CycloneDX 1.6

Location: `.components[].purl` (native).

```json
{
  "type": "library",
  "name": "http",
  "version": "1.1.0",
  "purl": "pkg:pub/http@1.1.0",
  "hashes": [
    {"alg": "SHA-256", "content": "<lowercase-hex-from-lockfile-description.sha256>"}
  ],
  "properties": [
    {"name": "mikebom:source-type", "value": "pub-hosted"},
    {"name": "mikebom:evidence-kind", "value": "pubspec-lock"},
    {"name": "mikebom:sbom-tier", "value": "source"}
  ]
}
```

The `mikebom:source-type` / `mikebom:evidence-kind` / `mikebom:sbom-tier` properties are existing per-component annotations (milestones 002 / 004 / 002 respectively). The PURL is the only new wire-format addition.

For casks-style discrimination — git source ALSO includes `mikebom:vcs-ref` (preserves the lockfile's `ref:` field, distinct from `resolved-ref`). Path source includes `mikebom:path` (the relative path string). SDK source includes `mikebom:sdk-name`.

### SPDX 2.3

Location: `.packages[].externalRefs[]` with `referenceCategory: PACKAGE-MANAGER`.

```json
{
  "name": "http",
  "versionInfo": "1.1.0",
  "externalRefs": [
    {
      "referenceCategory": "PACKAGE-MANAGER",
      "referenceType": "purl",
      "referenceLocator": "pkg:pub/http@1.1.0"
    }
  ],
  "checksums": [
    {"algorithm": "SHA256", "checksumValue": "<hex>"}
  ],
  "annotations": [
    { /* mikebom:source-type / evidence-kind / sbom-tier envelopes via existing annotation pattern */ }
  ]
}
```

### SPDX 3.0.1

Location: `software_Package.software_packageUrl` + `Element.externalIdentifier[]`.

```json
{
  "type": "software_Package",
  "spdxId": "...",
  "name": "http",
  "software_packageVersion": "1.1.0",
  "software_packageUrl": "pkg:pub/http@1.1.0",
  "externalIdentifier": [
    {
      "type": "ExternalIdentifier",
      "externalIdentifierType": "packageUrl",
      "identifier": "pkg:pub/http@1.1.0"
    }
  ]
}
```

## Determinism

For a given `pubspec.lock` / `pubspec.yaml`, the emitted PURL set MUST be identical across runs:

- Lockfile entries processed in `BTreeMap` insertion order (lex-sorted by package name — `serde_yaml`'s default).
- Main-module components processed in walker discovery order (sorted directory entries per `safe_walk` convention from milestone 114).

## Absence semantics

When the scanned root contains no `pubspec.yaml` AND no `pubspec.lock`:

- Zero `pkg:pub/*` and zero `pkg:generic/*` Dart-derived components emit.
- No warnings fire (per FR-006).
- SBOM bytes are identical (modulo timestamps + serial numbers) to a pre-feature scan (SC-004 invariant).

## Parity-catalog note

Because the wire-format addition is the native PURL field (not a new `mikebom:*` annotation), no new C-row is added to `docs/reference/sbom-format-mapping.md`. The PURL surfaces via the existing A1 row ("PURL"). The `mikebom:source-type` annotation reuses C1 (introduced in milestone 002 for cargo's path/git/registry discrimination); Dart contributes new VALUES (`pub-hosted` / `pub-git` / `pub-path` / `pub-sdk` / `pub-main-module`) to C1's value set without altering wire shape.
