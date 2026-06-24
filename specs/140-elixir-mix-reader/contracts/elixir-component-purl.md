# Contract — `pkg:hex/*` and `pkg:generic/*` component PURL

The only wire-format contract this feature introduces. Per Constitution Principle V audit (research §R1):

- `pkg:hex/` is **purl-spec-blessed** ([hex-definition.md](https://github.com/package-url/purl-spec/blob/main/types-doc/hex-definition.md)) — used for `:hex` source-type entries (both default-hexpm and private-org).
- `pkg:generic/` is the placeholder for `:git` (per Phase 0 correction — purl-spec doesn't bless `vcs_url=` for hex) and `:path` sources.
- Private Hex orgs use the spec-blessed namespace-as-org form + `?repository_url=https://repo.hex.pm` qualifier per Phase 0 correction (replaces the initial `mikebom:hex-repo` annotation proposal).
- Hex package names lowercased per purl-spec canonical form (Hex.pm enforces at publish time so typically no-op).

## Wire shapes per source

### Hex (default `"hexpm"` repo)

```text
pkg:hex/<lc-name>@<version>
```

### Hex (private organization `"hexpm:<org>"`)

```text
pkg:hex/<org>/<lc-name>@<version>?repository_url=https://repo.hex.pm
```

Reader splits the lockfile's `"hexpm:<org>"` repo string on the first colon; org slug becomes the PURL namespace.

### Git

```text
pkg:generic/<name>@<resolved-sha>?vcs_url=git+<git-url>
```

Per Phase 0 correction: `pkg:generic/` placeholder because purl-spec hex-definition does NOT define a `vcs_url=` qualifier for hex. Once a dep is git-swapped it has no Hex.pm provenance — `pkg:hex/` would imply registry-resolution.

Plus `mikebom:source-type = "hex-git"` annotation + `mikebom:vcs-declared-ref = "<opt-value>"` when present (preserves the operator's `ref:`/`branch:`/`tag:` declaration from `mix.exs`).

### Path

```text
pkg:generic/<name>@<version-or-unspecified>
```

Plus `mikebom:source-type = "hex-path"` annotation + `mikebom:path = "<path-string>"` annotation. For umbrella sub-app deps (`in_umbrella: true` opt), also `mikebom:in-umbrella = "true"` annotation.

### Main-module (per FR-012)

```text
pkg:hex/<app_name>@<version-or-"0.0.0-unknown">
```

Plus `mikebom:component-role = "main-module"` + `mikebom:source-type = "hex-main-module"` annotations. For umbrella roots: additional `mikebom:umbrella-root = "true"` annotation. App-name derivation cascade:
1. `mix.exs::project/0::app:` atom (lowercased verbatim).
2. Parent-directory basename fallback when `mix.exs` lacks `app:` or no `mix.exs` exists (matches milestone-139 Q1 pattern).

## Examples

| Scan input | Emitted PURL |
|---|---|
| `mix.exs` `app: :my_app`, `version: "0.5.2"` | `pkg:hex/my_app@0.5.2` (main-module) |
| Lockfile `"phoenix": {:hex, :phoenix, "1.7.10", "abc...", [:mix], [...], "hexpm", "def..."}` | `pkg:hex/phoenix@1.7.10` (hashes: `[sha256:abc..., sha256:def...]`) |
| Lockfile `"my_lib": {:hex, :my_lib, "2.0.0", "abc...", [:mix], [], "hexpm:acme", "def..."}` (private-org `acme`) | `pkg:hex/acme/my_lib@2.0.0?repository_url=https://repo.hex.pm` |
| Lockfile `"my_fork": {:git, "https://github.com/foo/my-fork.git", "eb39649...3d5601", [ref: "main"]}` | `pkg:generic/my_fork@eb39649...3d5601?vcs_url=git+https://github.com/foo/my-fork.git` (annotations: `mikebom:source-type = "hex-git"`, `mikebom:vcs-declared-ref = "ref: main"`) |
| Lockfile `"shared_lib": {:path, "apps/shared_lib", []}` | `pkg:generic/shared_lib@unspecified` (annotations: `mikebom:source-type = "hex-path"`, `mikebom:path = "apps/shared_lib"`) |
| Umbrella root `mix.exs` with `apps_path: "apps"` | `pkg:hex/<root_app_name>@<version>` (annotations: `mikebom:component-role = "main-module"`, `mikebom:umbrella-root = "true"`) |
| `mix.exs::deps/0` entry `{:credo, "~> 1.7", only: [:dev, :test], runtime: false}` (design-tier mode) | `pkg:hex/credo@~>_1.7` (annotations: `mikebom:sbom-tier = "design"`, `mikebom:requirement-range = "~> 1.7"`, `mikebom:lifecycle-scope = "development"`) |
| Conditional `if Mix.env() == :test do {:meck, "~> 0.9"} end` | `pkg:hex/meck@~>_0.9` (annotations: `mikebom:sbom-tier = "design"`, `mikebom:elixir-extraction-mode = "conditional-flattened"` per Q1) |

## Per-format emission

### CycloneDX 1.6

Location: `.components[].purl` (native).

```json
{
  "type": "library",
  "name": "phoenix",
  "version": "1.7.10",
  "purl": "pkg:hex/phoenix@1.7.10",
  "hashes": [
    {"alg": "SHA-256", "content": "<inner-sha256-from-4th-tuple-element>"},
    {"alg": "SHA-256", "content": "<outer-sha256-from-8th-tuple-element>"}
  ],
  "properties": [
    {"name": "mikebom:source-type", "value": "hex-hex"},
    {"name": "mikebom:evidence-kind", "value": "mix-lock"},
    {"name": "mikebom:sbom-tier", "value": "source"}
  ]
}
```

The `mikebom:source-type` / `mikebom:evidence-kind` / `mikebom:sbom-tier` properties are existing per-component annotations. The PURL + dual SHA-256 hashes (when both present per Q3) + `mikebom:vcs-declared-ref` / `mikebom:path` / `mikebom:elixir-extraction-mode` / `mikebom:umbrella-root` (source-type/scenario-specific) are the only new wire-format additions.

### SPDX 2.3

Location: `.packages[].externalRefs[]` with `referenceCategory: PACKAGE-MANAGER`.

```json
{
  "name": "phoenix",
  "versionInfo": "1.7.10",
  "externalRefs": [
    {
      "referenceCategory": "PACKAGE-MANAGER",
      "referenceType": "purl",
      "referenceLocator": "pkg:hex/phoenix@1.7.10"
    }
  ],
  "checksums": [
    {"algorithm": "SHA256", "checksumValue": "<inner-hex>"},
    {"algorithm": "SHA256", "checksumValue": "<outer-hex>"}
  ]
}
```

### SPDX 3.0.1

Location: `software_Package.software_packageUrl` + `Element.externalIdentifier[]`.

```json
{
  "type": "software_Package",
  "spdxId": "...",
  "name": "phoenix",
  "software_packageVersion": "1.7.10",
  "software_packageUrl": "pkg:hex/phoenix@1.7.10",
  "externalIdentifier": [
    {
      "type": "ExternalIdentifier",
      "externalIdentifierType": "packageUrl",
      "identifier": "pkg:hex/phoenix@1.7.10"
    }
  ]
}
```

## Determinism

For a given `mix.lock` / `mix.exs`, the emitted PURL set MUST be identical across runs:

- Lockfile entries processed in their YAML-map order (preserved by regex-tokenization — top-down).
- Main-module components processed in walker discovery order (sorted directory entries per `safe_walk` convention from milestone 114).
- `extra_annotations` `BTreeMap` ensures deterministic property emission order.

## Absence semantics

When the scanned root contains none of `mix.lock` / `mix.exs`:

- Zero `pkg:hex/*` and zero Elixir-derived `pkg:generic/*` components emit.
- No warnings fire (per FR-006).
- SBOM bytes byte-identical (modulo timestamps + serial numbers) to a pre-feature scan (SC-004).

## Parity-catalog note

Because the wire-format addition is the native PURL field (including the spec-blessed namespace-as-org and `repository_url=` qualifier for private Hex orgs), no new C-row is added to `docs/reference/sbom-format-mapping.md` for identity. The PURL surfaces via the existing A1 row.

The `mikebom:source-type` annotation reuses C1; Elixir contributes new VALUES (`hex-hex` / `hex-git` / `hex-path` / `hex-main-module`) to C1's value set without altering wire shape.

NEW annotations introduced by this milestone:
- `mikebom:vcs-declared-ref` (operator-declared `ref:`/`branch:`/`tag:`/`commit:` from lockfile `:git` opts) — reuses milestone-139's annotation name (CocoaPods used it identically; same semantics).
- `mikebom:path` (path-source path string) — reuses milestone-137 + 138 + 139 precedent.
- `mikebom:in-umbrella` (path-source `in_umbrella: true` flag preservation) — NEW. Deferred parity-catalog refresh.
- `mikebom:umbrella-root` (umbrella-root main-module marker) — NEW. Deferred parity-catalog refresh.
- `mikebom:elixir-extraction-mode = "conditional-flattened"` (design-tier extraction precision-loss marker per Q1) — NEW. Deferred parity-catalog refresh.

## syft/trivy divergence (deferred to v1.1)

Per Phase 0 research, both syft and trivy emit purl-spec-non-conformant PURLs for hex:
- syft: empty namespace + no qualifiers (drops private-org info).
- trivy: skips `:git` and `:path` entries entirely (hex-only).

mikebom emits the spec-conformant form per Principle V. Emitting `mikebom:also-known-as` annotations for syft/trivy compatibility is **deferred to v1.1** — operators who need that ecosystem's compatibility can use those tools directly in the interim.
