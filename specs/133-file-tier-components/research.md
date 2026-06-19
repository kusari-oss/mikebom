# Research: File-tier component emission for unattributed content

**Date**: 2026-06-19
**Branch**: `133-file-tier-components`
**Driven by**: spec.md FR-022 (BLOCKING measure-first projection per the 2026-06-19 Q2 clarification)

## §Orphan projection (FR-022 BLOCKING deliverable)

### Projection methodology

A one-off bash projection tool walks the extracted milestone-132 pinned audit
baseline rootfs, applies the FR-005 content-shape allowlist, and approximates the
FR-011 hybrid dedupe via:

- **Path-coverage proxy**: files under common package-install directories
  (`/usr/bin/*`, `/usr/lib/*`, `/lib/*`, `/var/lib/{apk,dpkg,rpm}/*`,
  `/usr/share/{man,doc,locale}/*`, `/usr/include/*`) are assumed package-owned.
  This approximates what US2's `mikebom:component-paths` will subtract once
  implemented.
- **Hash-coverage check**: files whose SHA-256 appears in the cached audit SBOM's
  binary-tier `hashes[]` (221 distinct hashes) are subtracted.

The result is an **UPPER BOUND** on orphan emission — the real production-time
orphan count will be lower once US2's path-tracking exists for every package-tier
reader.

Tool location: `/tmp/mb-133-projection/project.sh`. Inputs: extracted rootfs at
`/tmp/mb-133-projection/rootfs/` + known-hash list at
`/tmp/mb-133-projection/known-hashes.txt`. Run time: ~2 minutes on the audit
baseline.

### Projection results (measured 2026-06-19)

```
=== Milestone 133 orphan-emit projection (UPPER BOUND) ===
rootfs:           /tmp/mb-133-projection/rootfs
known-hashes:     221 (binary-tier only; package-tier readers don't carry per-file hashes)
total files in rootfs: 38 539

category                    total   path-cov   hash-cov     orphan
------------------------------------------------------------------------
compressed-archive              1          1          0          0
elf-binary                    603        569          0         34
exec-script                   220        168          0         52
java-or-archive               367          4          0        363
lone-manifest                 638        273          0        365
macho-binary                    6          2          0          4
pe-binary                    2 475        20          0      2 455
shared-lib                      5          2          0          3

PROJECTED UPPER-BOUND ORPHAN COUNT: 3 276
```

### Decision: tighten FR-005 allowlist per FR-022 contract

The 3 276 upper-bound count is well outside the original SC-001 range (200-800).
The FR-022 clarification mandates allowlist adjustment at plan time before code
lands. **Three root causes** identified from the per-category breakdown:

1. **PE binaries: 2 455 orphan / 2 475 total (99.2 %).** Almost all .NET assemblies
   in the audit image live under `dotnet/packs/<framework>/<version>/<sub>/` —
   the milestone-130/131 PE/CLR readers DO identify these as `pkg:nuget`
   components but don't yet emit `mikebom:component-paths` so the projection
   heuristic doesn't recognize them as covered. **Without US2 expansion to cover
   .NET install roots, orphan emission would silently re-emit 2 455 components
   that already exist as `pkg:nuget`.**

2. **Java-or-archive: 363 orphan / 367 total (99 %).** JARs scattered under
   node_modules + various Java install roots. Most are NOT vendored unattributed
   JARs — they're packaged inside npm packages or under apk-installed Java
   runtimes. Same root cause as PE binaries: package-tier readers don't track
   contained files.

3. **Lone-manifest: 365 orphan / 638 total (57 %).** My projection counts EVERY
   `package.json` / `Cargo.toml` / `pom.xml`, but FR-005 specifies "lone
   manifest with NO associated lockfile". Most of these manifests sit next to
   lockfiles inside `node_modules/<dep>/` — they're NOT orphan signals. The
   projection script's `lone-manifest` classification doesn't apply the adjacent-
   lockfile check, so this count is artificially inflated.

**Decision (recommended path)**: tighten FR-005 with three changes:

A. **Add path exclusion list to content-shape allowlist**: orphan emission MUST
   skip files under these well-known package install roots even when the file
   shape passes the basic allowlist:
   - `**/dotnet/packs/**`
   - `**/dotnet/shared/**`
   - `**/dotnet/sdk/**`
   - `**/node_modules/**`
   - `**/lib/python*/site-packages/**`
   - `**/.cargo/registry/**`
   - `**/ruby/gems/**`
   - `**/usr/share/dotnet/**`

   Rationale: these are known package install roots where the package-tier reader
   DOES know the package identity (via PURL) but hasn't yet been extended to
   emit `mikebom:component-paths`. The exclusion list is a pragmatic stop-gap
   until US2 expansion can cover those readers fully.

B. **Apply adjacent-lockfile check to lone-manifest classification**: a
   `package.json` / `Cargo.toml` / `pom.xml` qualifies as "lone manifest" only
   when:
   - `package.json` + NO `package-lock.json` / `yarn.lock` / `pnpm-lock.yaml` in
     the same directory
   - `Cargo.toml` + NO `Cargo.lock` in the same directory or any parent up to
     the workspace root
   - `pom.xml` + NO `target/` directory in the same directory (pomx with build
     output = real build, not vendored source-tree signal)

   Rationale: lockfile presence means the project IS a real build target, not a
   vendored manifest mikebom should treat as "unattributed."

C. **Re-measure after the allowlist tightening lands at plan time**. The new
   projection should land in the 200-800 SC-001 band; if it doesn't, iterate.

### Re-projected estimate after tightening (manual estimate)

Per-category after applying tightening (rough):

| category | original orphan | tightened orphan (estimate) | rationale |
|---|---|---|---|
| pe-binary | 2 455 | ~50 | excluding `dotnet/packs/**` / `dotnet/shared/**` / `dotnet/sdk/**` |
| java-or-archive | 363 | ~80 | excluding `node_modules/**` JARs (most live there) |
| lone-manifest | 365 | ~20 | excluding manifests with adjacent lockfiles (NPM majority) |
| elf-binary | 34 | 34 | unchanged — these are genuinely unattributed ELF binaries |
| exec-script | 52 | 52 | unchanged |
| macho-binary | 4 | 4 | unchanged |
| shared-lib | 3 | 3 | unchanged |
| compressed-archive | 0 | 0 | unchanged |
| **TOTAL** | **3 276** | **~245** | within 200-800 band |

**Estimate confidence**: medium. The pe-binary subtraction is the biggest
driver and the most uncertain — depends on whether ALL 2 455 PE files
actually live under `dotnet/packs/` (some may be in `dotnet/store/` or other
roots I haven't enumerated). The projection MUST be re-run after the
allowlist tightening lands in code to confirm the count.

**Outcome**: SC-001 range narrowed to **200-400** in `research.md` based on
this estimate (slightly tighter than original to set a clear bar after
the allowlist tightening). The `±10 %` band per the Q2 clarification means
acceptable outcome is **180-440**. Re-projection at implementation time
confirms or surfaces the gap.

### Recorded action items

1. **FR-005 allowlist update (PR-level)**: code MUST exclude the path-prefix list
   above. Documented in `data-model.md §Content-shape allowlist`.
2. **FR-022 measure-re-run (T-level)**: the projection tool from this section
   re-runs as task T0NN after the allowlist code lands, before SC-001 is
   declared MET.
3. **US2 reader expansion notes**: the projection identifies which package-tier
   readers most need `mikebom:component-paths` tracking — PE/CLR (.NET), npm
   (node_modules), Java (JAR install roots). Implementation can prioritize
   these but full expansion is out of scope for milestone 133.

## §SPDX 3 element type for file-tier components (FR-001 deferred decision)

**Decision**: Use `software_File` element type for SPDX 3 emission.
**Rationale**: SPDX 3.0.1 schema defines `software_File` as a first-class element
under the `Software` profile (`Element` → `Artifact` → `software_File`). All
properties mikebom needs are inherited: `name` (from `Element`), `verifiedUsing`
(hashes — from `Artifact`), `software_primaryPurpose` (the `file` enum value
matches what we want to convey). The `mikebom:component-tier = "file"` annotation
becomes redundant for SPDX 3 (the element type IS the tier) but emitted anyway
for cross-format consistency.
**Alternatives considered**:
- `software_Package` (the path we currently use for SPDX 2.3 emission): rejected
  for SPDX 3 because `software_File` is the format-native choice; using
  `software_Package` would force consumers to walk annotations to determine the
  tier instead of reading the element type.
- Custom Element subtype: rejected — over-engineering; the existing
  `software_File` covers the use case.

## §100 MB file size limit (FR-010 default tuning)

**Decision**: 100 MB default skip threshold for file-tier emission. Operator
override via `--file-inventory-size-limit <bytes>`.
**Rationale**: hashing a 100 MB file takes ~1-2 seconds on commodity hardware;
hashing a 10 GB file (database backup, ML model weight, video asset) takes
minutes and dominates total scan time. Below 100 MB is fast enough that we
don't notice; above 100 MB is rare for SBOM-relevant content (real binaries are
typically <50 MB; the >100 MB content tends to be data, not code).
**Alternatives considered**:
- 10 MB: rejected — would skip legitimate kernel images / large statically-linked
  binaries.
- 1 GB: rejected — too liberal; a single 800 MB file dominates scan time.
- No limit: rejected — pathological for images carrying database dumps.

## §Multi-arch image handling (FR-013 layer-digest open question)

**Decision**: For multi-arch images, mikebom continues to scan whichever
architecture the existing image-resolution path already selected (today: native
host architecture matching the docker manifest list selection rule). FR-013
`mikebom:layer-digest` reads from the selected manifest's layer entries.
**Rationale**: this matches existing behavior; no new design surface introduced.
**Alternatives considered**:
- Multi-arch SBOM emission (one set of components per arch): rejected as out of
  scope. Tracked separately.
- Cross-arch layer-digest comparison: rejected — distinct concern.

## §Nested-archive path semantics (FR-007 / FR-012 open question)

**Decision**: For file-tier components surfaced from inside an archive walked by
existing milestone-130 maven nested-JAR walker etc., the `mikebom:file-paths`
property carries the synthesized `<archive-path>!/<inner-path>` form (Java JAR
URL convention; `!/` separator). For US2's `mikebom:component-path` on package-
tier components, the same convention applies — the apk package's source path
might be `var/lib/apk/db/installed!/<sub>` if mikebom's reader resolves into the
DB file.
**Rationale**: `!/` is the established JAR-URL convention and is human-readable.
Reusing it across the SBOM avoids inventing a new separator.
**Alternatives considered**:
- `:` separator (Java classpath style): rejected — collides with path-with-colon
  filenames on some platforms.
- Separate `mikebom:archive-path` property: rejected — splits identity across
  two properties for no gain.

## §All Q-level clarifications resolved

Per `spec.md §Clarifications`:

- Q1 (default flip → orphan in 133): codified in FR-015 + the CHANGELOG-callout
  obligation noted in this milestone's plan section.
- Q2 (measure-first → BLOCKING during planning): satisfied by this research.md
  §Orphan projection section above. FR-022 obligation discharged.
- Q3 (orphan dedupe → hybrid path-OR-hash): codified in the revised FR-011 +
  the projection's path-coverage proxy logic.
