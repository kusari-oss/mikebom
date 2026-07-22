# Phase 0 Research: SBOM auto-split

**Feature**: 215-sbom-auto-split
**Date**: 2026-07-22

## R1 — Boundary enumeration strategy (which components are "roots")

**Decision**: A workspace-root is any `ResolvedComponent` whose `extra_annotations` map contains `"waybill:is-workspace-root": true` (the m127/m201-established signal). Split-mode enumerates these into a list of `SubprojectRoot { purl, source_dir, extras }` structs; each becomes one sub-SBOM axis.

**Rationale**: m127 (root selector) + m201 (workspace-root disambiguation) already land the `waybill:is-workspace-root` annotation on every main-module component that a workspace-aware ecosystem reader identifies as a workspace member. Reusing this signal:
- Zero new detection logic (per m214 rename R2 principle — reuse existing readers)
- Consistent across all supported ecosystems (cargo workspaces, npm workspaces, Go workspaces, Maven multi-module, gradle sub-projects, pyproject dirs, gem sub-gems)
- Free correctness for edge cases the existing readers already handle (nested workspaces, workspace-root-that-is-also-a-package, mixed layouts)
- Test-fixture-friendly: any fixture that today produces multiple main-modules with `is_workspace_root=true` automatically becomes a split-scenario fixture

**Zero-boundary case (FR-009)**: If enumeration returns 0 workspace-root components, split-mode emits ONE SBOM identical to the pre-feature single output + WARN log line "no workspace boundaries detected — emitting single SBOM per --split fallback". The command exits 0.

**Alternatives considered**:
- (a) **Filesystem-walk-based detection** (scan for `Cargo.toml` / `package.json` / etc directly) — REJECTED. Duplicates every reader's existing logic; would drift over time. m127/m201 IS the source of truth.
- (b) **Operator-specified explicit list** (`--split-root=<purl>`) — REJECTED for v1 per Spec Assumptions (out-of-scope). May layer on later as a follow-up spec.

## R2 — Per-subproject dep-graph projection (which components go in each sub-SBOM)

**Decision**: BFS over the resolved `Vec<Relationship>` starting from each `SubprojectRoot.purl` as the seed. Include every component whose `bom-ref` is reachable via `DependsOn`/`RuntimeDependencyOn`/`BuildDependencyOn`/`OptionalDependencyOn`/`DevDependencyOn` edges (all `RelationshipType` variants that indicate "component X uses component Y").

**Rationale**: 
- `ResolvedComponent` + `Relationship` is Waybill's canonical in-memory model post-resolve. All existing emit code (CDX / SPDX 2.3 / SPDX 3) consumes these types unchanged. Split just narrows the sets before dispatch.
- BFS captures transitive closure. Direct + transitive deps of a workspace member are included; unrelated other-member deps are excluded.
- Multi-edge-type inclusion (Runtime + Build + Optional + Dev) matches the "self-contained per SC-004" requirement — a downstream vuln scanner needs to see build-time deps to catch supply-chain risks in dev toolchains.

**Algorithm sketch**:
```
fn project_for_root(root_purl: &Purl, all_components: &[ResolvedComponent],
                    all_relationships: &[Relationship]) -> (Vec<ResolvedComponent>, Vec<Relationship>) {
    let mut reached: BTreeSet<BomRef> = BTreeSet::from([root_bomref(root_purl)]);
    let mut queue: VecDeque<BomRef> = VecDeque::from([root_bomref(root_purl)]);
    while let Some(cur) = queue.pop_front() {
        for rel in all_relationships.iter().filter(|r| r.from == cur && is_dep_edge(&r.kind)) {
            if reached.insert(rel.to.clone()) { queue.push_back(rel.to.clone()); }
        }
    }
    let components = all_components.iter().filter(|c| reached.contains(&c.bomref)).cloned().collect();
    let relationships = all_relationships.iter()
        .filter(|r| reached.contains(&r.from) && reached.contains(&r.to))
        .cloned().collect();
    (components, relationships)
}
```

**Cost**: O(V + E) per subproject where V = component count, E = relationship count. For N subprojects: O(N × (V + E)) total. On a 5000-component 15000-edge monorepo with 20 subprojects: 400k ops per subproject × 20 = 8M ops. Sub-second in Rust.

**Alternatives considered**:
- (a) **Include all components regardless of reachability** (per-subproject SBOM = full component set with the root swapped) — REJECTED. Defeats the whole purpose; downstream vuln scan would see 5000 components per service when only 500 are relevant.
- (b) **Include only DIRECT deps of the root** — REJECTED. Compliance tools need transitive closure to catch deep supply-chain vulns.
- (c) **Bidirectional walk (transitive callers too)** — REJECTED. Confusing semantics; a shared lib doesn't "belong to" the service that calls it in the split sense.

## R3 — Filename convention + collision handling

**Decision**: `<slug>.<ecosystem>.<format>.json` where:
- `<slug>` = sanitized subproject-name (from PURL name field, with `/` → `-`, `@` → `at-`, other unsafe chars stripped)
- `<ecosystem>` = PURL type (`cargo`, `npm`, `pypi`, `maven`, `go`, `gem`, `swift`, `generic`, etc.)
- `<format>` = one of `cdx.json`, `spdx.json`, `spdx3.json`

Examples:
- `pkg:cargo/libsafe@0.1.0` → `libsafe.cargo.cdx.json`
- `pkg:npm/@myorg/frontend@1.0.0` → `at-myorg-frontend.npm.cdx.json`
- `pkg:generic/rootless@0.0.0` → `rootless.generic.cdx.json`

**Collision handling** (per FR-011): if two subprojects would produce the same `<slug>.<ecosystem>` prefix (rare — same package name in two workspace members), disambiguate by appending a directory-derived suffix: `<slug>.<ecosystem>-<relpath-hash>.<format>.json`. The `<relpath-hash>` is the first 8 hex chars of SHA-256 over the subproject's relative source dir. Deterministic across scans.

**Rationale**:
- Filename encodes ecosystem so operator-side globbing works: `find . -name '*.npm.cdx.json'` returns all npm-ecosystem sub-SBOMs
- Deterministic + filesystem-safe by construction — passes on Linux, macOS, Windows without additional sanitization
- Human-readable — matches operator "one SBOM = one service" mental model
- Collision-resistant via optional hash suffix (rare corner case; deterministic when triggered)

**Alternatives considered**:
- (a) **Full sanitized PURL** (`pkg-cargo-libsafe-0-1-0.cdx.json`) — REJECTED. Verbose; version-embedded means filename changes on every release.
- (b) **Directory-basename only** (`libsafe.cdx.json`) — REJECTED. Ambiguous when two subprojects share a basename; loses ecosystem info.
- (c) **Numeric suffix** (`sub-01.cdx.json`, `sub-02.cdx.json` — with manifest as the index) — REJECTED. Loses operator-facing identity; requires manifest lookup for any human action.

## R4 — split-manifest.json schema

**Decision**: JSON document with the following shape, stored as `<output-dir>/split-manifest.json`:

```json
{
  "$schema": "https://waybill.dev/schema/split-manifest/v1.json",
  "waybill_version": "0.1.0-alpha.66",
  "scan_root": "/path/to/monorepo",
  "generated_at": "2026-07-22T14:00:00Z",
  "total_unique_components": 1247,
  "shared_dep_count": 42,
  "entries": [
    {
      "subproject_id": "libsafe.cargo",
      "root_purl": "pkg:cargo/libsafe@0.1.0",
      "source_dir": "libsafe",
      "component_count": 87,
      "shared_deps_count": 12,
      "files": {
        "cyclonedx-json": "libsafe.cargo.cdx.json",
        "spdx-2.3-json": "libsafe.cargo.spdx.json",
        "spdx-3.0-json": "libsafe.cargo.spdx3.json"
      }
    },
    ...
  ]
}
```

**Fields**:
- `$schema`: URL identifying the manifest schema version. v1 pinned here; future breaking schema changes bump.
- `waybill_version`: which Waybill release emitted this manifest. Aids debugging if manifest shape drifts across releases.
- `scan_root`: absolute or repo-relative path the operator passed to `--path`.
- `generated_at`: RFC 3339 timestamp. Under `WAYBILL_FIXED_TIMESTAMP`, matches the fixed value (FR-012 reproducibility).
- `total_unique_components`: distinct component-PURL count across all sub-SBOMs (the pre-feature single-SBOM component count). Provides a SC-004 sanity check.
- `shared_dep_count`: distinct component-PURLs that appear in >1 sub-SBOM. Diagnostic; downstream tools may use to dedup at their level if desired.
- `entries[]`: one entry per subproject.
- `entries[].subproject_id`: `<slug>.<ecosystem>` — matches the filename prefix. Serves as the operator-visible primary key.
- `entries[].files{}`: map of format → filename (relative to manifest's containing dir). One entry per emitted format.

**Rationale**:
- JSON matches every other Waybill emit format — no new format spec.
- Format-multi-value under `files{}` handles N × M cleanly (per FR-008); single-format users see just one entry, multi-format users see all.
- `subproject_id` is stable across scans (deterministic function of PURL) so downstream automation can key on it.
- The `$schema` URL pin lets future consumers do version-aware parsing.

**Alternatives considered**:
- (a) **NDJSON one line per entry** — REJECTED. Requires downstream to know the aggregate shape (total counts) via a separate mechanism.
- (b) **YAML manifest** — REJECTED. Waybill's stack is JSON-native; adding YAML for one operator artifact adds a parser dep for consumers.
- (c) **Embedded in each sub-SBOM as a `waybill:split-siblings` annotation** — REJECTED. Bloats every sub-SBOM with sibling-listing data; couples the SBOMs to each other (against FR-007 self-contained).

## R5 — Reproducibility (deterministic serial numbers under WAYBILL_FIXED_TIMESTAMP)

**Decision**: Under `WAYBILL_FIXED_TIMESTAMP` (existing FR from pre-feature), each sub-SBOM's serial number becomes `urn:uuid:<sha256(subproject_root_purl + fixed_ts)>[..32]` — a UUIDv5-flavored deterministic hash. Manifest's `generated_at` matches the fixed timestamp.

**Rationale**:
- Pre-feature single-SBOM behavior already derives a deterministic serial under `WAYBILL_FIXED_TIMESTAMP` (the m212/m213 goldens depend on this). Split extends the pattern: each sub-SBOM's serial is deterministic per-subproject-identity.
- SHA-256 truncated to 32 hex chars (128 bits) is UUID-compatible-shape and collision-safe for the small subproject count in any realistic monorepo.
- Aids downstream deduplication + attestation-signing workflows where operators want the same signed artifact when the input hasn't changed.

**Alternatives considered**:
- (a) **Fresh random UUID per sub-SBOM** (default `uuid::Uuid::new_v4()`) — REJECTED for reproducibility mode; would break golden-diff testing.
- (b) **Sequential integer serial** (`sub-001`, `sub-002`) — REJECTED. Not spec-compliant (CDX/SPDX expect UUID-shaped serial numbers).

## R6 — Multi-format multiplication (N × M emit)

**Decision**: When operator passes `--format` multiple times (e.g., `--format cyclonedx-json --format spdx-2.3-json --format spdx-3-json`), split-mode emits N × M files (N subprojects × M formats). Each subproject's sub-SBOMs across formats share the same component projection (per R2) and root identity — they're the same SBOM expressed in different formats. The manifest lists all N × M under each `entries[].files{}` map.

**Rationale**:
- Existing scan-cmd already handles multi-format single-SBOM emission (each format runs through its own emit function against the same resolved data). Split reuses this exact fan-out per subproject: `for subproject in subprojects: for format in formats: emit(subproject.projection, format, filename)`.
- N × M is bounded (N < 50 realistic monorepo subs × M ≤ 3 formats = ≤ 150 files). Emit is CPU-cheap (~200 ms per format per subproject); total end-to-end < 30 sec even for large monorepos.
- Manifest's per-entry `files{}` map makes the multi-format shape discoverable to downstream tooling.

**Alternatives considered**:
- (a) **Multi-format = one format only per split invocation** (require operator to run --split three times for three formats) — REJECTED. Bad UX; operator wants one command that emits everything.
- (b) **Combine all formats into one sub-SBOM** — REJECTED. Not spec-valid (CDX and SPDX are distinct format wire shapes, cannot merge).

## R7 — Interaction with `--output` vs `--output-dir`

**Decision**: `--split` is INCOMPATIBLE with `--output <file>` (single-file target). Passing both is a hard error with a friendly diagnostic:

```
error: `--split` requires `--output-dir <dir>`; use `--output-dir` instead of `--output` when splitting.
```

When `--split` is set:
- `--output-dir <dir>` is REQUIRED. Emits N × M sub-SBOMs + 1 manifest into `<dir>`.
- `--output <file>` is REJECTED with the error above.
- If neither is set, default to `--output-dir=./waybill-split-<timestamp>/`.

When `--split` is NOT set (existing behavior):
- `--output <file>` behaves as pre-feature.
- `--output-dir <dir>` behaves as pre-feature.
- No breakage.

**Rationale**: A single file can't hold N SBOMs. Rather than silently pick one or emit a tar/zip archive, we fail loudly with a fix-suggestion. Matches Rust's "explicit is better than implicit" convention.

**Alternatives considered**:
- (a) **Emit sub-SBOMs as a single `.tar.gz`** — REJECTED. Introduces archive format ambiguity; downstream tools would need to know to extract. Directory output is simpler + universal.
- (b) **Auto-pick the first subproject when `--output <file>` is passed** — REJECTED. Silent data loss.
- (c) **Emit only the manifest to `--output <file>` when both flags present** — REJECTED. Confusing UX; operator expects `--output` to hold the SBOM data.

## R8 — Zero-boundary fallback (FR-009)

**Decision**: When boundary enumeration (R1) returns 0 workspace-root components, split-mode emits ONE SBOM (identical to pre-feature single-output) at either `<output-dir>/root.cdx.json` (auto-generated slug for single-file case) or `<output-file>` (if `--output` were somehow allowed — but per R7 it's rejected under `--split`, so this branch never triggers with `--output` set). No manifest is written (nothing to describe). WARN log line:

```
WARN waybill::generate::split: no workspace boundaries detected in scan_root=<path>; emitting single SBOM per --split fallback contract (FR-009).
```

Command exits 0.

**Rationale**:
- Operators shouldn't have to guard `--split` behind conditional logic in their CI ("only add --split if we detect a monorepo"). Flag should Just Work on single-package projects with a WARN + fallback.
- No manifest for single-output case avoids emitting a manifest that just points at one file — noise, not signal.
- Slug `root` for the auto-generated single-file name is a documented convention; predictable for CI script authors.

**Alternatives considered**:
- (a) **Hard error on zero-boundary** — REJECTED. Bad UX; scripts that opportunistically use `--split` shouldn't break on single-package trees.
- (b) **Still emit a manifest with one entry** — REJECTED. Manifest is meant to describe a set; one entry is degenerate.

## R9 — Interaction with existing golden tests

**Decision**: Pre-feature golden test files under `waybill-cli/tests/fixtures/golden/{cyclonedx,spdx-2.3,spdx-3}/` remain byte-identical (SC-007). Split-mode golden tests live in a NEW directory `waybill-cli/tests/fixtures/golden/split/<fixture-name>/{sub-sbom-files, split-manifest.json}` — no cross-contamination.

**Regeneration env var** for the new split goldens: `WAYBILL_UPDATE_SPLIT_GOLDENS=1` (matches the `WAYBILL_UPDATE_*_GOLDENS` pattern established by m212/m213/m214). The release-time golden-regen recipe extends to include the new env var; docs/migration/mikebom-to-waybill.md needs no update (the pattern is stable, only names change).

**Rationale**:
- Non-split scan behavior is unchanged; existing goldens verify SC-007.
- Split-mode goldens live in their own tree so a future rollback wouldn't touch pre-split goldens.
- New env var name follows the established naming convention — CI grep gate stays green.

**Alternatives considered**:
- (a) **Reuse existing golden dirs, add split output alongside** — REJECTED. Confuses which golden belongs to which mode; complicates regeneration invocation.
- (b) **No goldens; only smoke-test the CLI-flag surface** — REJECTED. Split correctness requires wire-shape stability (component set per sub-SBOM, manifest schema) that only golden diffs catch reliably.
