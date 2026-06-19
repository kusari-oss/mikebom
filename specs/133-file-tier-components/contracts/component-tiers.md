# Contract: Reference doc draft — `docs/reference/component-tiers.md`

**Driven by**: spec.md FR-020 (US4 reference doc).
**Target file**: `docs/reference/component-tiers.md` (new file shipped in
milestone 133).
**Change shape**: New top-level reference doc; ~250 lines; first cross-cutting
mikebom reference doc to acknowledge another tool (trivy) explicitly as informing
mikebom's design.

This contracts file is the BLUEPRINT for the actual `docs/reference/component-
tiers.md` file. The implementation task (T0NN in tasks.md) is a copy-paste with
minor formatting cleanup.

---

# mikebom component tiers

mikebom emits THREE component tiers in its SBOMs. Understanding the tier
distinction lets you query the SBOM efficiently and interpret what each
component represents.

## The three tiers

### Tier 1: Package-tier

A component representing a known package (cargo crate, npm package, deb/rpm/apk
package, NuGet assembly, etc.) identified by its PURL.

- **Identity**: Package URL (PURL) following the PURL specification.
- **Discovery**: package-DB readers (apk, dpkg, rpm, cargo-auditable),
  manifest/lockfile parsers (Cargo.lock, package-lock.json), and PE/CLR metadata
  reader (milestones 130 + 131).
- **CDX type**: `library` (or `application` / `operating-system` for OS-level
  packages).
- **SPDX 2.3**: `Package` with `externalRefs[purl]`.
- **SPDX 3**: `software_Package` with `software_packageUrl`.
- **Properties new in milestone 133**: `mikebom:component-path` (rootfs source
  path) and `mikebom:layer-digest` (OCI layer digest when applicable).

### Tier 2: Binary-tier

A component representing an identified binary artifact that doesn't map to a
known package, but whose identity has been derived via symbol fingerprinting,
embedded metadata, or content-hash matching.

- **Identity**: synthesized PURL (`pkg:generic/<name>@<version>`) OR no PURL
  with hash-based identity.
- **Discovery**: milestone-099 symbol fingerprinter; milestone-104 binary-role
  classifier; milestone-108 / -110 fingerprint corpus matchers.
- **CDX type**: `library`, `application`, or `file` depending on the binary
  role.
- **SPDX 2.3**: `Package`.
- **SPDX 3**: `software_Package` or `software_File`.
- **Hash**: per-file SHA-256 always populated (milestone 038 onward).

### Tier 3: File-tier (NEW in milestone 133)

A component representing content on the rootfs that survived both package-tier
and binary-tier readers. Identity is its SHA-256.

- **Identity**: SHA-256 hash. NO PURL (FR-009).
- **Discovery**: rootfs walker (`scan_fs::file_tier::walker`) applies the FR-005
  content-shape allowlist + FR-005 path-prefix exclusion list, then runs the
  FR-011 hybrid dedupe (path coverage from US2's `mikebom:component-paths` OR
  hash coverage from binary-tier per-file hashes).
- **CDX type**: `file`.
- **SPDX 2.3**: `Package` (no native File component type in 2.3) with
  `filesAnalyzed: false`.
- **SPDX 3**: `software_File` (native element type).
- **Annotation**: every file-tier component MUST carry
  `mikebom:component-tier = "file"` for unambiguous identification across
  formats (the SPDX 2.3 Package shape is otherwise indistinguishable from a
  package-tier Package).

## How the tiers compose

For a given file on a rootfs scan, the precedence is:

1. **Package-tier readers run first**. If a package-DB or manifest claims a path
   or contains the file's content, the package-tier component is emitted with
   its PURL identity. The file's path may be recorded via
   `mikebom:component-paths` (US2).
2. **Binary-tier readers run second**. For files surviving step 1 that pass
   binary-tier discovery criteria (ELF/PE/Mach-O magic + fingerprint match),
   a binary-tier component is emitted.
3. **File-tier walker runs last** in `default` (`orphan`) mode. For files
   surviving steps 1 and 2 AND passing the FR-005 content-shape allowlist AND
   failing the FR-011 hybrid dedupe (path NOR hash covered), a file-tier
   component is emitted.

In `--file-inventory=full` mode, step 3 emits per-unique-hash file-tier
components for every file passing the content-shape allowlist regardless of
package or binary coverage. Duplicates with package-tier components are
EXPECTED in this mode; the SBOM carries a document-level
`mikebom:file-inventory-mode = "full"` annotation so consumers can detect the
override at parse time.

## Orphan content-shape allowlist (FR-005)

Default-mode file-tier emission applies a content-shape allowlist to avoid
flooding SBOMs with source code, docs, and configs. Files qualify ONLY when
they match one of these shapes:

- **Unattributed ELF / Mach-O / PE binary** — magic-number probe (first 4
  bytes).
- **Unattributed shared library** — `.so` / `.so.*` / `.dylib` / `.dll`
  extension OR file-magic match.
- **Unattributed archive** — `.jar` / `.war` / `.ear` / `.deb` / `.rpm` / `.apk`
  / `.tar` / `.tgz` / `.tar.xz` / `.tar.bz2` / `.zip` extension.
- **Lone package manifest** — `Cargo.toml`, `package.json`, `pom.xml`,
  `requirements.txt`, `Gemfile`, `go.mod` WITH NO ADJACENT LOCKFILE in the same
  directory (or any parent up to the workspace root for `Cargo.toml`).
- **Executable script** — file with executable bit AND first 2 bytes = `#!`.

Files NOT matching any shape are skipped silently.

### Path-prefix exclusion (FR-005 post-tightening)

Even when a file passes the content-shape allowlist, file-tier emission is
SKIPPED when the file lives under one of these well-known package install
roots:

```
**/dotnet/packs/**
**/dotnet/shared/**
**/dotnet/sdk/**
**/dotnet/store/**
**/usr/share/dotnet/**
**/node_modules/**
**/lib/python*/site-packages/**
**/.cargo/registry/**
**/ruby/gems/**
**/jvm/openjdk*/lib/**
```

Rationale: these are install roots where a package-tier reader DOES know the
package identity (via PURL) but doesn't yet emit `mikebom:component-paths` to
prove it. The exclusion list is a pragmatic stop-gap until per-reader path
tracking expands. Surfaced via FR-022's measure-first projection during
milestone-133 planning — see milestone-133's `research.md §Orphan projection`
for the empirical justification.

## Content shapes EXPLICITLY excluded from orphan mode

These shapes are NEVER orphan-emitted, regardless of path:

- **Source code**: `.rs`, `.py`, `.go`, `.c`, `.cpp`, `.h`, `.cs`, `.java`,
  `.js`, `.ts`, `.rb`, `.php`, `.swift`, `.kt`
- **Plain text**: `.md`, `.txt`, `.rst`
- **Structured config (not archives)**: `.json`, `.yaml`, `.yml`, `.toml`,
  `.ini`, `.conf`, `.xml` (when not a known archive)
- **Documentation**: PDF, man pages outside well-known package install roots

Rationale: these shapes are pure noise for SBOM consumers. Vuln scanners don't
key on .md files; license auditors don't key on .py source. The exclusions
keep the orphan output signal-dense.

## Full mode (`--file-inventory=full`)

Opt-in via `--file-inventory=full`. Emits per-unique-hash file-tier component
for every file passing the content-shape allowlist (no path-prefix exclusion;
no hybrid dedupe). Targeted use cases:

- **Forensics**: "is `sha256:abc...` (a known IOC) present anywhere on this
  image?" — single component lookup answers the question.
- **Image diff**: full-mode SBOMs from two image versions show file-level
  deltas — newly added, modified, removed file hashes.
- **Malware detection**: a shared "bad" file appearing across multiple package
  contexts shows up as ONE file-tier component with all paths in
  `mikebom:file-paths`.

Full-mode SBOMs carry a document-level `mikebom:file-inventory-mode = "full"`
annotation. Consumers MAY use this annotation to detect that the SBOM contains
duplicate (file-tier × package-tier) coverage of the same content.

## Trivy lesson: path/layer context on package-tier (US2)

This milestone steals an idea from trivy's design: every package-tier component
identified from a rootfs path carries `mikebom:component-path` (the relative
rootfs path) and, for image scans, `mikebom:layer-digest` (the OCI layer digest
containing the path). Trivy proves these properties are low-cost / high-value
for forensic / diff / supply-chain queries.

This is a DIFFERENT choice than trivy's per-(package × path) component
duplication. mikebom continues to dedupe package-tier components by their
PURL identity; when a package is identified from multiple paths in a single
scan, the paths collapse into a single `mikebom:component-paths` (plural)
property carrying a sorted JSON array. Same shape as US1's file-tier
`mikebom:file-paths` for symmetry.

## Why mikebom rejected the alternative designs

Two adjacent industry designs were considered and rejected during milestone-132
close-out research. Their rationale lives here so future contributors don't
re-litigate.

### Syft model (per-(path × hash) file emission)

Syft emits one file-tier component per (path × hash) tuple — the same file at
two paths shows up as two components. This achieves 5★ Completeness on
the sbom-comparison scorecard but pumps SBOM size up dramatically (27 006
file entries vs ~3 770 package entries on the milestone-132 audit baseline).
mikebom chose per-unique-hash with paths-as-property to preserve the malware /
forensic query surface without the SBOM-bloat cost.

### Trivy model (per-(package × path) — no file-tier components)

Trivy emits package-tier components only, but DUPLICATES the package when it's
identified from multiple paths (581 components on the audit baseline; the same
`@smithy/is-array-buffer@2.2.0` appears twice with different `FilePath`
properties). Trivy bets that "the package is the unit of analysis"; mikebom
chose to surface BOTH (package-tier with `mikebom:component-paths` collection,
plus file-tier for unattributed content) so consumers can query by either.

## Related milestones

- Milestone 104 — Binary-tier component role classification. Established the
  binary-tier-vs-package-tier distinction this milestone extends.
- Milestone 130/131 — PE/CLR managed-assembly metadata + license-coverage
  backfill. Identified the dotnet/packs/ over-emission risk that motivated
  FR-005's path-prefix exclusion list.
- Milestone 132 — Closeout of milestone 131 SC misses. The Completeness 1★ vs
  5★ gap surfaced during 132's audit-baseline measurement; milestone 133 is
  the structural response.
- Milestone 132 SC-002 (deferred to milestone 134) — VERSION_MISMATCH `<50`.
  Independent of file-tier emission.
