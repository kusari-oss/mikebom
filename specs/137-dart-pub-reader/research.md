# Research — milestone 137 Dart/Flutter pub reader

Resolves the Phase 0 open items from `plan.md`'s Technical Context: Constitution Principle V audit, on-disk `pubspec.lock` schema, purl-spec `pub` canonical form, integration site within `read_all`, the language-reader pattern selection, and per-source error posture.

## R1: Constitution Principle V audit — `pub` PURL type is purl-spec-blessed

**Decision**: Emit `pkg:pub/<name>@<version>` per [purl-spec PURL-TYPES.rst `pub-definition.md`](https://github.com/package-url/purl-spec/blob/main/types-doc/pub-definition.md). No `mikebom:*` annotation introduced for identity. Source-discriminator surfaces via the existing `mikebom:source-type` annotation (parity-catalog C1, introduced in milestone 002) — same wire shape as cargo's `path` / `git` / `registry` discrimination.

**Rationale**:

The purl-spec defines `pub` explicitly:

- **Namespace**: prohibited (no `pkg:pub/<vendor>/<name>` form).
- **Default repository URL**: `https://pub.dartlang.org` (legacy; `pub.dev` redirects).
- **Name normalization**: lowercase-snake-case enforced by pub.dev's publishing rules; canonical form is no-op for any modern package.
- **Canonical example**: `pkg:pub/characters@1.2.0` AND `pkg:pub/flutter@0.0.0` (the spec explicitly blesses the `0.0.0` placeholder for Flutter SDK pseudo-deps).

This is fundamentally different from milestone 136's `brew` situation — `pub` is upstream-blessed, no follow-up purl-spec extension needed.

**Source-discriminator handling**:

For hosted / git / sdk: emit under `pkg:pub/` per spec. The `mikebom:source-type` annotation reuses the existing C1 parity-catalog row (milestone 002 introduced it for cargo's path/git/registry discrimination). Dart contributes new VALUES to that row's value set (`pub-hosted`, `pub-git`, `pub-sdk`) but does NOT alter wire shape — same as how cargo, npm, pip etc. already populate it.

For path-sourced deps: the purl-spec doesn't define an addressable PURL for filesystem-local packages (path identity has no globally-unique resolvable form). The reader emits `pkg:generic/<name>@<version>` as a placeholder + `mikebom:source-type = "pub-path"` annotation as the discriminator. This is a **parity-bridge** per Principle V's escape clause (annotation surfaces information the standard doesn't express). No new C-row in the parity catalog because the annotation reuses C1.

**Alternatives considered**:

- **`pkg:pub/<name>?host=<bare-hostname>` for self-hosted**. Rejected: purl-spec uses `repository_url=` as the cross-type qualifier name (with full scheme), not `host=`. Wrong qualifier name vs spec authority.
- **Skip path deps entirely**. Rejected: they're real source-tree deps the operator's project relies on. Surfacing them with an annotated placeholder is more transparent than silently dropping.
- **Custom `pkg:pub-path/` informal type**. Rejected: violates Principle V (don't invent type-names; the `pkg:generic/` + annotation pattern is the established mikebom convention for non-addressable identities).
- **Emit SDK deps as `pkg:generic/flutter-sdk`**. Rejected: the purl-spec EXPLICITLY uses `pkg:pub/flutter@0.0.0` as a canonical example. Honor upstream spec rather than invent a placeholder.

## R2: `pubspec.lock` v2 schema (Dart 2.0+, current as of Dart 3.x)

**Decision**: Parse a small typed subset; treat every per-package field as `Option<T>` except `dependency`, `description`, `source`, `version` (universally present in modern lockfiles). Use `#[serde(untagged)]` for `description` to handle string-vs-map polymorphism.

**Schema** (per [`dart-lang/pub` source](https://github.com/dart-lang/pub) + real-world `flutter/gallery/pubspec.lock`):

Top-level keys (only two):

```yaml
packages:
  <package_name>: <LockfileEntry>
  ...
sdks:
  dart: ">=3.11.0 <3.999.0"   # informational; not consumed v1
  flutter: ">=3.41.2"
```

The `sdks:` block carries **constraint strings** (not resolved versions). Out of scope for v1 — describes host toolchain requirement, not a runtime/build component. Skip.

**Per-package entry** (always 4 fields):

```yaml
<name>:
  dependency: "direct main"      # | "direct dev" | "transitive" | "direct overridden"
  description: <see below>
  source: hosted                  # | git | path | sdk
  version: "<string>"
```

**`description:` polymorphism** — varies by `source:`:

| `source:` | `description:` shape | Fields |
|---|---|---|
| `hosted` | Map | `name`, `sha256` (lowercase hex, no prefix), `url` (bare base URL with scheme, no trailing slash) |
| `git` | Map | `url` (git remote), `ref` (user-supplied — branch/tag/`HEAD`), `resolved-ref` (40-char SHA), `path` (subdir; `"."` when package is at repo root — always present) |
| `path` | Map | `path` (string — absolute or relative), `relative` (bool) |
| `sdk` | **Scalar string** (e.g., `"flutter"`) | SDK name; same scalar shared across multiple entries from same SDK |

**`dependency:` enum** (four values per [`dart-lang/pub#4047`](https://github.com/dart-lang/pub/issues/4047)):

- `"direct main"` — `dependencies:` in pubspec.yaml
- `"direct dev"` — `dev_dependencies:`
- `"transitive"` — pulled in by another package
- `"direct overridden"` — `dependency_overrides:` in pubspec.yaml (rare but real)

Lifecycle mapping per FR-008:
- `direct main` → `Runtime`
- `direct dev` → `Development`
- `direct overridden` → treat as `Runtime` (overrides preserve runtime semantics; the override fact is annotation-worthy but doesn't change scope)
- `transitive` → `Runtime` (lockfile doesn't distinguish transitive-from-runtime vs transitive-from-dev; mikebom's existing language-reader convention treats transitives uniformly)

**Pre-Dart-2.0 lockfile format**: per spec Out-of-Scope. Modern Dart 2.0+ (2018) is universal in 2026 production.

**Alternatives considered**:

- **Shell out to `dart pub deps`**. Rejected: `dart` not guaranteed on scan host; cross-platform reader principle. Matches cargo/npm/pip posture.
- **Parse `.dart_tool/package_config.json` instead of `pubspec.lock`**. Rejected: package_config.json is post-`dart pub get` evidence, lockfile is the input authority. Out of scope per spec.
- **Strongly-typed `LockfileDescription` enum with `tag: source`**. Rejected: the YAML doesn't carry a typed discriminator inside `description`; only the sibling `source:` field acts as the tag. Use `#[serde(untagged)]` with explicit variant probing or post-parse re-interpretation.

## R3: purl-spec `pub` canonical form

**Decision**: Honor purl-spec verbatim. Specifically:

- **Base form**: `pkg:pub/<name>@<version>` (no namespace).
- **Self-hosted registry**: `?repository_url=<base-url-with-scheme>` (NOT `host=`). Value preserves scheme (e.g., `https://pub.acme.example.com`). Omitted when `description.url == "https://pub.dev"` or `https://pub.dartlang.org`.
- **SHA-256 download hash**: surfaced via the `PackageDbEntry.hashes` field (existing milestone-002 convention), NOT as a PURL `checksums=` qualifier. PackageDbEntry.hashes flows to `components[].hashes[]` in CDX / `Package.checksums[]` in SPDX — the standards-native field. Bypassing it for a PURL qualifier would be Principle V regression.
- **Git source**: `pkg:pub/<name>@<resolved-sha>?vcs_url=git+<git-url>[#<subpath>]`. The `git+` scheme prefix on `vcs_url` is the purl-spec git-source cross-type convention. `description.path` (subdir) becomes the PURL `#<subpath>` fragment when not `.` or empty. The user-supplied `ref` (branch/tag) is preserved as a `mikebom:source-type` evidence sub-field, not in the PURL.
- **Path source**: `pkg:generic/<name>@<version>` placeholder + `mikebom:source-type = "pub-path"` annotation (see R1).
- **SDK source**: `pkg:pub/<sdk-name>@0.0.0` + `mikebom:source-type = "pub-sdk"` annotation. The `0.0.0` is the literal placeholder pub writes; preserve verbatim per purl-spec canonical example.

**`mikebom:source-type` value set extension** (reuses parity-catalog C1):

| Source kind | `mikebom:source-type` value |
|---|---|
| Hosted (default `pub.dev`) | `pub-hosted` |
| Hosted (self-hosted mirror) | `pub-hosted` (the `repository_url=` PURL qualifier carries the distinguishing URL) |
| Git | `pub-git` |
| Path | `pub-path` |
| SDK | `pub-sdk` |

(The `pub-` prefix avoids collision with existing C1 values like cargo's `git`/`path`/`registry`. Per milestone-122 Kotlin DSL's `kmp-` prefix convention.)

## R4: Integration site within `read_all`

**Decision**: register `pub mod dart;` in `mikebom-cli/src/scan_fs/package_db/mod.rs` and add a call site in `read_all` alongside the existing language readers (cargo / gem / npm / pip / golang / maven / nuget / swift / kotlin_dsl).

Existing pattern (cargo example, line ~1436):

```rust
let cargo_out = cargo::read(rootfs, include_dev, exclude_set)?;
out.extend(cargo_out.entries);
diagnostics.divergence_records.extend(cargo_out.divergences);
```

Dart adds (placed alphabetically between `conan::read` and `gem::read`):

```rust
out.extend(dart::read(rootfs, include_dev, exclude_set));
```

No `collect_claimed_paths` integration — language readers don't claim binary paths (file-claim is an OS-reader concern; the binary walker classifies non-OS-claimed binaries as `pkg:generic/*` regardless of source-tree language-reader output).

**`include_dev` flag**: existing language-reader convention is to accept `include_dev: bool` and filter out dev-scope components before returning. Dart respects this (FR-008): when `include_dev == false`, components with `dependency: direct dev` are dropped at the reader level.

**`exclude_set`**: existing `ExclusionSet` (milestone 113) for the safe-walk filter. Standard plumbing.

## R5: Language-reader pattern selection

**Decision**: Use **cargo.rs** (milestone 002 + 064) as the architectural template. Closest semantic match:

- Both parse a YAML/TOML lockfile in a source-tree project directory
- Both emit a main-module component per workspace member's manifest (`Cargo.toml` ↔ `pubspec.yaml`)
- Both handle multi-source dep classification (cargo: registry/path/git; dart: hosted/path/git/sdk)
- Both support multi-version-of-same-package coexistence
- Both use lifecycle-scope annotation for dev vs runtime distinction

**`serde_yaml` precedent**: `npm/yarn_lock.rs` + `npm/pnpm_lock.rs` already use `serde_yaml = "0.9"` from the workspace dep tree. Confirms the dep is available and battle-tested for lockfile parsing.

**Source-tree walker**: reuse `scan_fs::walk::safe_walk` (milestone 114). Pattern: walk for `pubspec.yaml` files; for each, look for sibling `pubspec.lock`; emit per-project components keyed off the manifest's `name:`+`version:`.

**Alternatives considered**:

- **Use maven.rs as template**. Rejected: maven is more complex (BOM imports, parent-POM walking, JAR archive descent). Cargo is simpler and closer.
- **Custom YAML parser via line-format extraction**. Rejected: `serde_yaml` is in the workspace; no reason to hand-roll.

## R6: Multi-project / workspace handling

**Decision**: per Clarifications Q2, one main-module per `pubspec.yaml`. No synthetic workspace-root component.

Implementation: the source-tree walker finds every `pubspec.yaml` under the scan root. For each, the reader:

1. Parses the manifest's `name:` + `version:` for the main-module emission.
2. Looks for a sibling `pubspec.lock` in the same directory.
3. If lockfile present → emit `source`-tier components per FR-002 (lockfile-driven, all transitive deps).
4. If lockfile absent → emit `design`-tier components per FR-005 (manifest-only, direct + dev deps).
5. Attribute dep edges to THAT manifest's main-module bom-ref.

Same-PURL dep collisions across multiple lockfiles (e.g., monorepo where two members both pin `http 1.1.0`) collapse via the standard cross-component `seen_purls` dedup at the orchestrator level — milestone-002 pattern, no special handling needed.

**Pub-workspace (Dart 3.6+ `workspace:` field in root `pubspec.yaml`, single lockfile at root)** — **DEFERRED TO v1.1**: walker finds each member's `pubspec.yaml` and emits one main-module per member per FR-009; member dep edges from `pubspec.yaml`'s `dependencies:` / `dev_dependencies:` (design-tier fallback per FR-005) are emitted in v1, but inheriting pinned versions from an enclosing root-level lockfile via parent-directory walk is **out of scope for v1** to keep the implementation small and the test surface tractable. In v1, members without their own sibling `pubspec.lock` use design-tier emission from `pubspec.yaml` only — even when a root lockfile is present. This is a v1.1 follow-up: extend the walker to discover root-level lockfiles and attribute their entries to each member's main-module via the lockfile's per-entry `dependency:` classification. The unified-view behavior (single SBOM containing all members + their deps) STILL works in v1 because each member's pubspec.yaml carries the same dep names — only the version-pinning fidelity differs (design-tier constraint vs source-tier pinned version).

## R7: Per-lockfile error posture

**Decision**: warn-and-skip per FR-007.

| Condition | Behavior | Justification |
|---|---|---|
| Source tree has no `pubspec.yaml` and no `pubspec.lock` | Return `Vec::new()` immediately | Clean no-op (FR-006) |
| `pubspec.yaml` exists, no `pubspec.lock` | Emit design-tier components per FR-005 | Library publishers / pre-`pub get` workflows |
| `pubspec.yaml` malformed YAML | `tracing::warn!`, skip the project, continue walking other projects | FR-007 |
| `pubspec.yaml` missing `name:` field | `tracing::warn!`, skip main-module emission for that project | Cannot synthesize PURL |
| `pubspec.lock` malformed YAML | `tracing::warn!`, fall back to design-tier emission from `pubspec.yaml` | Best-effort preservation |
| Per-entry `version:` missing or empty | `tracing::warn!`, skip that single entry | Cannot synthesize PURL |
| Per-entry `source:` discriminator unknown (not hosted/git/path/sdk) | `tracing::warn!`, skip that single entry | Future pub format changes — warn-and-skip preserves forward compat |
| Empty `packages:` block | Emit just the main-module; no dep components; no warning | Fresh `pub get` failure or library with zero deps |

## R8: Performance considerations

**Decision**: no performance budget violations expected; per-scan cost is bounded by lockfile size.

- Per-project: read `pubspec.yaml` (~500 bytes) + `pubspec.lock` (5–20 KB typical), parse via `serde_yaml`, build N `PackageDbEntry` instances. Estimated 1–3 ms per project on warm cache.
- Typical Flutter app (~50 deps): ~3 ms total. Heavy app (~200 deps): ~10 ms. Monorepo with 5 members (~250 deps): ~15 ms.
- Source-tree walker discovery cost (find all `pubspec.yaml` under scan root): same as cargo's existing `find_cargo_manifests` walker — sub-millisecond on typical repos.

The no-Dart-detected fast path: walker finds no `pubspec.yaml`; reader returns empty Vec; statistically free.

---

## Summary of Phase 0 resolutions

| Unknown | Decision | Reference |
|---|---|---|
| Principle V audit | `pub` is purl-spec-blessed; reuse C1 for source-type discriminator | R1 |
| `pubspec.lock` schema | Parse 4-field subset; `serde(untagged)` for description polymorphism | R2 |
| purl-spec `pub` canonical form | `repository_url=`, `vcs_url=git+...#<subpath>`, `0.0.0` for SDK per spec example | R3 |
| Integration site | `read_all` dispatcher alongside other language readers | R4 |
| Reader pattern template | cargo.rs (milestone 002 + 064) — closest semantic match | R5 |
| Multi-project handling | One main-module per `pubspec.yaml`; no synthetic root | R6 |
| Per-lockfile error posture | Warn-and-skip; never fail-the-scan | R7 |
| Performance | ~15 ms on heavy monorepo; no budget concerns | R8 |

All Phase 0 unknowns resolved. Ready for Phase 1 (data-model + contracts + quickstart).
