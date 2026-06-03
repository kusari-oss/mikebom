# Research: milestone 109 — binary-source PURL binding

Resolves the planning-phase open questions deferred from `/speckit-clarify`'s coverage summary (performance budget, walker depth, annotation-merge semantics).

## R1 — Build-dir walker discovery depth

**Decision**: walker walks the scan root to a max depth of **6** looking for `<dir>/CMakeCache.txt` AND `<dir>/_deps/` co-presence. Each match is recorded as one `CmakeBuildDirObservation`; iteration stops descending into already-matched cmake build dirs (no need to recurse INTO `_deps/` itself).

**Rationale**: typical project layouts put `build/` at the project root (depth 1) or `cmake/build/` (depth 2). Multi-target workspaces with `build-debug/` + `build-release/` are depth 1 each. Monorepos with per-subproject builds (e.g., `subprojects/foo/build/`) reach depth 3-4. Depth 6 covers the long tail with a hard floor against pathological deep trees. Matches mikebom's existing walker conventions (the milestone-054 fix-walker-symlink-hang work caps the per-scan walker similarly).

**Alternatives considered**:
- _Unbounded recursion_: rejected — pathological deep `node_modules/` or `target/`-like trees would blow up scan time. The existing walker already caps at similar depths.
- _Depth 2 only_: rejected — too restrictive for monorepos with `subprojects/<name>/build/`.
- _Configurable via env_: rejected — opinionated default is the right call for a Constitution Principle X (Transparency) capability; operators with unusual layouts can scan from the build dir directly.

## R2 — Per-scan walker overhead budget

**Decision**: target ≤10ms wall-clock per cmake build directory at typical scale (≤100 `_deps/` entries). Hard cap: ≤100ms even at pathological scale (10000 `_deps/` entries — unrealistic but bounded). Observable via the existing `MIKEBOM_TRACE_TIMING=1` env var.

**Rationale**: `std::fs::read_dir` on a single directory with ~100 entries on a modern SSD runs in well under 1ms. The observer reads the cmake build dir's `_deps/` listing exactly once + opens `CMakeCache.txt` to confirm validity. No recursive descent inside `_deps/` (the `<name>-build/` directory's CONTENTS aren't needed — only its existence). Net I/O: 1 `read_dir` + 1 stat per cmake project + 1 stat per cmake declaration (to confirm `_deps/<name>-build/` exists). At 100 deps that's 102 syscalls — microseconds.

**Alternatives considered**:
- _Memoize results across multi-scan workflows_: rejected — per-scan computation is sub-millisecond; caching adds invalidation complexity without measurable gain.
- _Async I/O via tokio_: rejected — the operations are all sync filesystem calls and there's no concurrency win at this scale. mikebom's binary walker is already sync; adding async here would mix paradigms.

## R3 — Annotation-merge semantics when attribution fires

**Decision**: the post-attribution flow rewrites the `SymbolFingerprintMatch.target_purl` field from `pkg:generic/<library>` to the source-tier PURL **before** the per-binary entry passes through the milestone-105 dedup pipeline. The pipeline then merges the rewritten binary-tier entry with the cmake reader's existing source-tier entry by PURL equality, producing ONE final component with:

- `mikebom:source-mechanism` = the cmake-derived value (`cmake-fetchcontent-git` or `cmake-fetchcontent-url`) — the source-tier mechanism wins.
- `mikebom:evidence-kind` annotations on the merged component carry BOTH `cmake-fetchcontent-*` (from the source-tier) AND `symbol-fingerprint` (from the binary-tier corroboration) per the existing milestone-105 multi-evidence pattern.
- `mikebom:fingerprint-corpus-sha` (from milestone 108) stays on the merged component since the binary-tier evidence contributed it.
- `mikebom:also-detected-via` populated when the binary-tier match overlapped with a different library record (the rare FR-013-style collision case from milestone 108).

**Rationale**: this rewrite-then-merge approach reuses the existing milestone-105 dedup pipeline verbatim. No new merge logic; no new annotation key. The cmake-source-mechanism wins because (a) it's the authoritative source of the PURL identity and (b) consumers filtering by source-mechanism for "what build-system mechanism declared this dep?" expect the source-tier answer. The binary-tier `symbol-fingerprint` evidence is preserved as corroboration.

**Alternatives considered**:
- _Add a NEW `mikebom:cross-tier-bound` annotation_: rejected — the milestone-105 source-mechanism enum already carries the signal; adding a parallel annotation would be redundant and consume a C-row slot in `sbom-format-mapping.md` for no operator benefit.
- _Source-tier and binary-tier remain as separate components, joined only by shared PURL_: rejected — fails SC-001 ("EXACTLY ONE zlib component"). The dedup pipeline's purpose is exactly this merge.
- _Binary-tier mechanism wins, source-tier becomes corroboration_: rejected — the PURL came from the source-tier, so labeling its mechanism as binary-tier would be misleading. Cmake declared the dep; the fingerprint matcher confirmed it ships in the binary.

## R4 — Multi-cmake-project scoping inside one scan root

**Decision**: each `CmakeBuildDirObservation` is scoped to ONE cmake project (one `CMakeCache.txt` at the project root + its sibling `_deps/`). When a scan root contains multiple cmake projects (e.g., `subprojects/A/build/` + `subprojects/B/build/`), each project's source declarations bind ONLY to fingerprint matches in binaries that live UNDER that project's build directory. Cross-project leakage (project A's libcurl declaration binding to a fingerprint match in project B's binary) is forbidden.

**Rationale**: cmake projects are independent build units; conflating their declarations would produce incorrect attributions (project A might pin libcurl 7.85 while project B pins 8.0, and we'd misattribute project B's binaries to project A's PURL). The scoping rule is: a binary at path `P` is attributable only via the `CmakeBuildDirObservation` whose build dir is a path-ancestor of `P`.

**Alternatives considered**:
- _Global scope across all projects_: rejected — version-collision risk is real (two projects can pin different versions of the same library).
- _Operator-configurable scoping via flag_: rejected — adds a flag for an edge case 95% of operators won't encounter. The deterministic path-ancestor rule is correct without configuration.

## R5 — Pluggability for future Bazel/Meson observers (FR-012)

**Decision**: the `BuildAttributionRegistry` accepts entries from any source via a small trait:

```rust
pub(crate) trait BuildDirObserver {
    /// Walk the scan root + emit observations. Each observation pairs a
    /// source-tier PURL with the path where the corresponding built
    /// artifact lives (if present).
    fn observe(&self, scan_root: &Path, source_declarations: &[PackageDbEntry]) -> Vec<CmakeBuildDirObservation>;
}
```

The cmake observer (`CmakeFetchContentObserver`) implements this trait. Future Bazel / Meson observers implement the same trait and produce `BuildDirObservation` values keyed on the SAME `(library_name_lc, build_dir_path_ancestor)` pair. The registry's lookup logic is observer-agnostic.

**Rationale**: keeps the cmake-specific path-walking logic isolated (per FR-012) while sharing the attribution-registry + matcher-rewrite plumbing across all future observers. The trait surface is intentionally small (one method, no associated types beyond `CmakeBuildDirObservation`) so future observers can land without architectural rework.

**Alternatives considered**:
- _Implement the trait now with `CmakeFetchContentObserver` AND a `NoopObserver` placeholder for Bazel/Meson_: rejected — premature scaffolding. The trait exists for the cmake observer's single implementer; adding a noop is dead-code clutter (Rust's `dyn Trait` requires zero placeholder impls to enable future extension).
- _Use an enum dispatch instead of a trait_: rejected — closed-enum dispatch would require updating the enum every time a new observer lands. Trait-based dispatch keeps the registry layer decoupled.

## R6 — Synthetic test fixture strategy (no cmake invocation at test time)

**Decision**: integration tests build the test fixtures by hand using `std::fs::create_dir_all` to construct the `_deps/<name>-build/` directory layout PLUS placeholder files mimicking the cmake-built static archive. The source-tree side is built by writing a tiny synthetic `CMakeLists.txt` that the existing milestone-102/103 cmake reader parses normally. No `cmake` invocation needed at test time; tests are hermetic and fast.

**Rationale**: invoking real cmake from tests would (a) require cmake on the CI host, (b) add wall-clock seconds per test (cmake configuration step), and (c) couple test correctness to cmake version differences across CI hosts. The fixture inputs we care about are FILESYSTEM STATE (paths, file presence) — easily reproducible with stdlib calls. The cmake reader's parsing is independently unit-tested at the milestone-102/103 level.

**Alternatives considered**:
- _Run real cmake + ninja against a tiny fixture project_: rejected — heavy, brittle, slow. Already covered by the live `mikebom-cmake-demo` end-to-end test.
- _Use the `mikebom-cmake-demo` artifacts directly as fixtures_: rejected — couples this milestone's tests to a sibling repo. Self-contained synthetic fixtures are preferred.

## R7 — Constitution Principle V (Specification Compliance) re-evaluation

**Decision**: this milestone introduces ZERO new SBOM annotation keys + ZERO new C-rows in `docs/reference/sbom-format-mapping.md`. The cross-attribution rides existing annotations: `mikebom:source-mechanism` (already C55), `mikebom:also-detected-via` (already C56), `mikebom:fingerprint-corpus-sha` (already C58). The post-merge component carries the union of source-tier + binary-tier evidence per the milestone-105 multi-evidence pattern.

**Rationale**: principle V's parity-bridging exception isn't needed here — the attribution is a per-component PURL-rewrite + dedup, not a new datum. The 71 catalog rows + their per-format extractors are unchanged. No `every_catalog_row_has_an_extractor` invariant churn.

**Alternatives considered**: a new `mikebom:cross-tier-binding-evidence` annotation was considered for explicit "this component crossed the cross-tier boundary" signaling; rejected per R3 (redundant with existing source-mechanism + evidence-kind).
