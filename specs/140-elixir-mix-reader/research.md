# Research — milestone 140 Elixir/Mix reader

Resolves the Phase 0 open items from `plan.md`'s Technical Context: Constitution Principle V audit + purl-spec authority audit, `mix.lock` tuple shape, `mix.exs::deps/0` regex extraction, umbrella detection, integration site, per-file error posture.

## R1: Constitution Principle V audit — `hex` PURL type is purl-spec-blessed

**Decision**: Emit per [purl-spec `hex-definition.md`](https://github.com/package-url/purl-spec/blob/main/types-doc/hex-definition.md). For private Hex orgs, use spec-blessed `namespace = org` + `?repository_url=https://repo.hex.pm` qualifier (NOT a custom annotation). For `:git` source, use `pkg:generic/` placeholder (purl-spec doesn't bless `vcs_url=` for hex). For `:path` source, same `pkg:generic/` pattern as path-deps in milestones 137/138/139.

**Key purl-spec rules**:

- **Namespace**: OPTIONAL. Used for private Hex orgs (`pkg:hex/<org>/<name>@<version>`). Default-hexpm packages omit it.
- **Default repository URL**: `https://repo.hex.pm` (NOT `hex.pm` — that's the web app; `repo.hex.pm` is the registry-API host).
- **Name case-sensitivity**: lowercased per spec (Hex.pm enforces at publish time so this is typically no-op).
- **`repository_url=` qualifier**: defined; canonical for private orgs.
- **`vcs_url=` for git-source**: NOT defined for `hex` type. Use `pkg:generic/` placeholder instead.

**syft/trivy divergence**:
- syft (`elixir/package.go`) — emits `pkg:hex/<name>@<version>` with EMPTY namespace + EMPTY qualifiers. Drops private-org info.
- trivy (`hex/mix/parse.go`) — SKIPS `:git` and `:path` entries entirely. Hex-only.
- Both are purl-spec-non-conformant in different ways. mikebom emits the spec-conformant form per Principle V; syft/trivy compatibility `mikebom:also-known-as` annotations deferred to v1.1.

**Source-discriminator handling**:

| Source kind | `mikebom:source-type` value | PURL form |
|---|---|---|
| Default hexpm | `hex-hex` | `pkg:hex/<name>@<version>` |
| Private org (`hexpm:<org>`) | `hex-hex` (repo distinguished via PURL namespace + `repository_url=`) | `pkg:hex/<org>/<name>@<version>?repository_url=https://repo.hex.pm` |
| Git | `hex-git` | `pkg:generic/<name>@<resolved-sha>?vcs_url=git+<url>` |
| Path | `hex-path` | `pkg:generic/<name>@<version>` |
| Main-module | `hex-main-module` | `pkg:hex/<app_name>@<version>` |

## R2: `mix.lock` tuple format (Elixir 1.4+)

**Decision**: Regex-tokenize the Elixir map literal. Per-entry dispatch on the first atom (`:hex` / `:git` / `:path`). Multi-line tuples handled via brace-counting (the tuple shape isn't line-bounded).

**Canonical shapes** (per [elixir/lib/mix/lib/mix/dep/lock.ex](https://github.com/elixir-lang/elixir/blob/main/lib/mix/lib/mix/dep/lock.ex) + [hex/lib/hex/scm.ex](https://github.com/hexpm/hex/blob/main/lib/hex/scm.ex)):

```elixir
%{
  "phoenix": {:hex, :phoenix, "1.7.10", "<inner_sha256>", [:mix], [{:plug, ...}], "hexpm", "<outer_sha256>"},
  "my_fork": {:git, "https://github.com/foo/my-fork.git", "abc123...", [ref: "main"]},
  "shared_lib": {:path, "apps/shared_lib", []},
  "private_pkg": {:hex, :private_pkg, "2.0.0", "<inner>", [:mix], [], "hexpm:acme", "<outer>"}
}
```

**`:hex` tuple field positions**:
1. `:hex` (discriminator atom)
2. `:<atom_name>` — package name as Elixir atom; matches the top-level map key
3. `"<version>"` — version string
4. `"<inner_sha256>"` — package-contents SHA-256 (lowercase hex; 64 chars)
5. `[<managers>]` — Mix build managers atom list (e.g., `[:mix]`, `[:rebar3]`, `[:make]`); not consumed v1
6. `[<deps>]` — transitive dep references (e.g., `[{:plug, "~> 1.15", [hex: :plug, repo: "hexpm", optional: false]}]`); not consumed v1 (deferred to v1.1)
7. `"<repo>"` — repo identifier; `"hexpm"` default OR `"hexpm:<org>"` private. NOT a URL.
8. `"<outer_sha256>"` — OPTIONAL. Outer hex-archive checksum. Pre-Hex-2.0 entries lack this; handle as `Option<String>`.

**`:git` tuple field positions**:
1. `:git`
2. `"<url>"` — git remote URL
3. `"<resolved_sha>"` — resolved 40-char SHA (always present in lock)
4. `[<opts>]` — keyword list; may contain `ref:` / `branch:` / `tag:` / `submodules:` / `sparse:` / `subdir:`

**`:path` tuple field positions**:
1. `:path`
2. `"<path>"` — relative path string
3. `[<opts>]` — keyword list; may contain `in_umbrella: true`

**Other source types**: NONE. No `:rebar` or `:bare` lockfile entries per Phase 0 research. Rebar-built packages still come through `:hex` with `[:rebar3]` in the `managers` field.

**Top-level wrapping**: Plain `%{ ... }` Elixir map literal — no module wrapper, no preamble.

## R3: `mix.exs::deps/0` regex extraction

**Decision**: Regex-extract the `deps/0` function body. Match every `{:<name>, ...}` tuple within the file, flattening any conditional nesting per Q1.

**Canonical tuple shapes** (per [Mix.Tasks.Deps docs](https://hexdocs.pm/mix/Mix.Tasks.Deps.html)):

```elixir
defp deps do
  [
    {:phoenix, "~> 1.7"},                                # Hex with version
    {:plug, "~> 1.15", optional: false},                 # Hex with opts
    {:credo, "~> 1.7", only: [:dev, :test], runtime: false},  # Dev/test scope
    {:my_fork, git: "https://github.com/foo/my-fork.git"},  # git source, no version
    {:my_lib, github: "owner/repo", branch: "main"},     # :github shortcut → expands to :git
    {:shared, in_umbrella: true},                        # umbrella sub-app
    {:local_lib, path: "../my-lib"}                      # path source
  ]
end
```

**Regex patterns** (compiled once via `std::sync::OnceLock`):

- `app:` (project name): `(?m)^\s*app:\s*:([a-zA-Z_][a-zA-Z0-9_]*)\b`
- `version:` (project version): `(?m)^\s*version:\s*"([^"]+)"`
- Dep tuple (full sig): `\{\s*:([a-zA-Z_][a-zA-Z0-9_]*)\s*,([^}]*)\}` — capture name + opts blob; post-process the opts to detect `git:` / `github:` / `path:` / `in_umbrella:` / `only:` / `runtime:` markers.
- The `\{...\}` regex is multi-line-safe via `(?s)` flag where needed; tuple bodies that contain nested `[...]` lists are handled via brace-counting if regex matches truncate.

**Conditional detection**: when a dep tuple is found inside an `if Mix.env() ... do ... end` / `unless ...` / `case ... do ... end` block, emit with `mikebom:elixir-extraction-mode = "conditional-flattened"` annotation per Q1. Detection: scan the file line-by-line; track an `in_conditional: bool` stack via simple keyword counting.

**Scope detection from `only:` / `runtime:` keywords** (FR-008):
- `only: [:dev, :test]` OR `only: :dev` OR `only: :test` OR `runtime: false` → `LifecycleScope::Development`
- Otherwise → `LifecycleScope::Runtime`

**`:github` shortcut**: `github: "owner/repo"` expands to `git: "https://github.com/owner/repo.git"` per Phase 0. Combinable with `branch:`/`tag:`/`ref:` identically to `:git`. v1 design-tier extraction recognizes the shortcut; lockfile mode doesn't see it (the resolved entry is already `:git` shape).

**Alternatives considered**:
- Real Elixir AST parser (`tree-sitter-elixir` crate). Rejected: significantly increases dep tree for marginal accuracy gain over regex; matches gem/cocoapods reader posture.
- Shell out to `mix deps`. Rejected: Principle I + host-portability.

## R4: Umbrella project detection

**Decision**: Detect umbrella by KEY PRESENCE of `apps_path:` in root `mix.exs::project/0` return list. Per Phase 0 research, the default value is `"apps"` BUT user-configurable — don't string-match the value. Sub-app indicators (`build_path: "../../_build"`, `lockfile: "../../mix.lock"`) are advisory conventions, not authoritative detection.

**Regex**: `(?m)^\s*apps_path:\s*("[^"]*"|:[a-zA-Z_]+)` — capture the value verbatim (the value matters less than the key presence).

Sub-app `mix.exs` files are detected by walking under `apps_path` directory + finding `<sub_app>/mix.exs`. Each sub-app emits its own main-module + deps; no special handling beyond the standard "one main-module per `mix.exs`" rule.

Per Q2, the umbrella root's `depends` list = root's own `deps/0` entries + each sub-app's main-module bom-ref.

## R5: purl-spec `hex` canonical form (post-corrections)

Already documented in R1 above. Reference table:

| Source | PURL |
|---|---|
| Default hexpm | `pkg:hex/jason@1.4.4` |
| Private org | `pkg:hex/acme/internal_lib@2.0.0?repository_url=https://repo.hex.pm` |
| Git | `pkg:generic/my_fork@<resolved-sha>?vcs_url=git+https://github.com/foo/my-fork.git` |
| Path | `pkg:generic/shared_lib@<version-or-unspecified>` |
| Main-module | `pkg:hex/my_app@0.5.2` |

Hex names lowercased per purl-spec (no-op for Hex.pm-published packages; matters for case-mixed `:github`-shortcut deps in design-tier mode).

## R6: Integration site within `read_all`

**Decision**: register `pub mod elixir;` in `mikebom-cli/src/scan_fs/package_db/mod.rs` (placed alphabetically between `pub mod dpkg;` and `pub mod exclude_path;`) and add a call site in `read_all` after `dpkg::read` and before `exclude_path` (which isn't a reader — first reader 'e' would be standalone).

Looking at actual current ordering: existing readers include `alpm`, `apk`, `bazel`, `brew`, `cargo`, `cmake`, `cocoapods`, `composer`, `conan`, `dart`, `dpkg`... So `elixir` goes alphabetically after `dpkg`. Call site placement in `read_all`: between `dpkg::read(...)` invocation and the next reader's call. (Note: actual `dpkg::read` may not be a peer in the same dispatch chain — verify at impl time and pick the closest alphabetical neighbor.)

No `collect_claimed_paths` integration. Signature: `pub fn read(rootfs: &Path, include_dev: bool, exclude_set: &ExclusionSet) -> Vec<PackageDbEntry>`.

## R7: Multi-project / umbrella handling + per-file error posture

**Decision**: three walker passes, mirroring milestones 138/139.

Algorithm:

1. **Walker pass A** — `mix.lock` walker via `safe_walk`. For each:
   1. Parse via `regex` + brace-counted tokenizer. On error → `tracing::warn!` + skip project (or fall back to sibling-`mix.exs` design-tier per FR-007).
   2. Look for sibling `mix.exs` → parse via regex for `app:` / `version:` (FR-012 main-module name + version) + `deps/0` body (for scope cross-reference per FR-008 + conditional-extraction annotation per Q1).
   3. Emit main-module per FR-012.
   4. Emit lockfile components per FR-002 + FR-003 + FR-008 (cross-reference `mix.exs::deps/0` for scope tags) + FR-011 (SHA-256 hashes).

2. **Walker pass B** — `mix.exs` design-tier walker. For each `mix.exs` whose parent dir has NO sibling `mix.lock`: emit main-module + design-tier components per FR-005.

3. **Umbrella detection** — for any `mix.exs` whose `project/0` carries `apps_path:` key, treat as umbrella root. After Pass A + B, populate root's `depends` with each sub-app main-module bom-ref per Q2.

**Same-PURL dedup**: standard orchestrator `seen_purls` HashSet across all three sources.

**Per-file error matrix**:

| Condition | Behavior | Justification |
|---|---|---|
| No `mix.lock` / `mix.exs` | Return `Vec::new()` | Clean no-op (FR-006) |
| `mix.lock` parses; sibling `mix.exs` parseable | Standard source-tier + cross-ref scope | Common case |
| `mix.lock` parses; no sibling `mix.exs` | Lockfile emit; main-module from dir-basename per milestone-139 Q1 fallback | Lockfile-only commits |
| `mix.lock` malformed; sibling `mix.exs` present | Warn + design-tier per FR-007 | Best-effort preservation |
| Per-entry tuple malformed | Warn + skip single entry | Forward compat |
| `mix.exs` Elixir syntax error breaks regex | Warn + skip entry | Best-effort regex |
| `apps_path:` value not string/atom | Warn but still detect umbrella by key presence per R4 | Standards-compatible |

## R8: Performance considerations

**Decision**: no performance budget violations expected.

- Per-`mix.lock`: read ~10–30 KB typical, regex-tokenize. ~2–8 ms warm-cache.
- Typical Phoenix app (~80 hex deps): ~5 ms total.
- Heavy umbrella with 5 sub-apps × 80 deps: ~25 ms.
- Source-tree walker: sub-millisecond on typical repos.

The no-Elixir-detected fast path: walker finds no relevant files; reader returns empty Vec; statistically free.

---

## Summary of Phase 0 resolutions

| Unknown | Decision | Reference |
|---|---|---|
| Principle V audit | `hex` is purl-spec-blessed; private orgs use namespace + `repository_url=`; git source uses `pkg:generic/` (Phase 0 correction) | R1 |
| `mix.lock` schema | 3-source enum dispatch; brace-counted regex; outer SHA-256 is `Option<String>` | R2 |
| `mix.exs::deps/0` extraction | Regex per-tuple; conditional-flattened per Q1; `:github` shortcut → expand to `:git` | R3 |
| Umbrella detection | `apps_path:` KEY presence (not value match) in root `mix.exs::project/0` | R4 |
| purl-spec hex canonical form | Lowercase names; default URL `https://repo.hex.pm`; namespace for private orgs | R5 |
| Integration site | `read_all` dispatcher alphabetically between `dpkg` and next reader | R6 |
| Multi-project + error posture | Three walker passes; warn-and-skip; design-tier fallback on lockfile error | R7 |
| Performance | ~25 ms on heavy umbrella; no budget concerns | R8 |

All Phase 0 unknowns resolved. Ready for Phase 1.
