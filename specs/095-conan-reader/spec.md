# Feature Specification: Conan source-side reader — first C/C++ ecosystem in mikebom

**Feature Branch**: `095-conan-reader`
**Created**: 2026-05-12
**Status**: Draft
**Input**: User description: "let's look at C/C++ features. I want to start simple for now"

## Background

mikebom currently has 9 ecosystem readers (apk, cargo, deb, gem, golang, maven, npm, pip, rpm) plus a generic ELF/Mach-O/PE binary scanner with `DT_NEEDED` linkage extraction. For **deployed** C/C++ — Linux containers, compiled binaries — mikebom already produces reasonable SBOMs via the OS-package readers + binary linkage.

The gap is **source-side**: there's no reader for any C/C++ build-system manifest. Scanning a C/C++ source repo before/without a build yields zero source-language-specific components, even when the repo declares dependencies through a structured manifest.

"Start simple" — pick ONE C/C++ source-side dep-manager reader rather than tackling the full matrix (CMake `find_package`, Conan, vcpkg, Meson wrap, Bazel `MODULE.bazel`, autoconf). Among the candidates, **Conan** is the highest-value first reader because:

1. **Has a proper lockfile** (`conan.lock`) — matches mikebom's existing lockfile-first pattern (cargo, npm, pip, golang). Lockfile-first means deterministic version resolution without running the package manager.
2. **Has a clean PURL spec** (`pkg:conan/<name>@<version>`, per the packageurl-spec).
3. **Parseable manifests without running the tool** — `conanfile.txt` is INI-like; `conan.lock` is JSON. No need to invoke `conan install` to extract dep info.
4. **Active enterprise adoption** — Bloomberg, JFrog, many enterprise C++ shops use Conan. Conan Center provides versioned packages with consistent metadata.
5. **Matches the milestone-003 ecosystem-expansion pattern** — same shape as adding pip/npm/gem readers. Small surface area, well-bounded scope.

vcpkg is a close second and a natural follow-up milestone (`vcpkg.json` manifest mode + baseline-pinned versions, `pkg:vcpkg/<name>@<version>` PURL spec). CMake / Meson / Bazel / autoconf are deferred indefinitely — they don't have clean dep-manifest formats and would require running build-system tooling.

This milestone is the first in what could be a C/C++ ecosystem track. Scope is deliberately narrow: parse the two Conan v2 manifest files, emit components with PURLs + direct-dep edges when the lockfile is present. No `conanfile.py` (Python AST parsing), no Conan v1 (Conan 2.x replaced v1 in 2023), no Conan Center HTTP lookups, no transitive resolution beyond what's in the lockfile.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Operator scans a Conan-managed C/C++ source repo and sees declared dependencies (Priority: P1)

An operator scanning a C/C++ source tree managed by Conan v2 (most modern Conan-based projects post-2023) sees the project's declared dependencies emitted as components in the SBOM with Conan-spec-conformant PURLs.

**Why this priority**: This is the entire reason for the milestone. Without source-side coverage, mikebom currently produces zero components for a pre-build C/C++ source tree even when the tree has a fully-specified dep manifest.

**Independent Test**: `target/release/mikebom sbom scan --path <conan-test-fixture>` against a synthetic test fixture with `conanfile.txt` + `conan.lock`. Confirm the emitted SBOM contains a `pkg:conan/<name>@<version>` component for each entry in the lockfile (or each `[requires]` entry when no lockfile exists).

**Acceptance Scenarios**:

1. **Given** a directory with a `conanfile.txt` declaring `[requires]` entries (e.g., `zlib/1.3.1`, `openssl/3.2.0`), **When** the operator runs `mikebom sbom scan --path .`, **Then** the emitted SBOM contains components `pkg:conan/zlib@1.3.1` and `pkg:conan/openssl@3.2.0` with `type=library` and `evidence.identity[].technique = manifest-analysis`.
2. **Given** a directory with both `conanfile.txt` AND `conan.lock`, **When** mikebom scans it, **Then** the SBOM uses the lockfile as the authoritative version source (lockfile pins win over manifest version ranges). Components include both direct-`[requires]` entries AND lockfile-pinned transitive dependencies.
3. **Given** a directory with only `conan.lock` (no `conanfile.txt` — uncommon but valid for some CI workflows), **When** mikebom scans it, **Then** the SBOM contains all lockfile-pinned components. No error; lockfile-only scan is supported.
4. **Given** a directory with `conanfile.py` (no `conanfile.txt`), **When** mikebom scans it, **Then** mikebom logs an info-level message naming the Python-manifest gap and emits zero conan components for this milestone. No crash; clean fallthrough.

---

### User Story 2 — Conan direct-dep edges land as relationships in the SBOM (Priority: P2)

When a `conan.lock` is present, the SBOM's dependency-relationship graph reflects which conan components are direct deps of the project (the main-module ↔ direct-`[requires]` edges). Operators using vulnerability scanners can distinguish "we depend on libssl directly" from "libssl transitively pulled in zlib".

**Why this priority**: dependency-relationship correctness is what makes the SBOM useful to downstream consumers. P2 (not P1) because the v1 spec for FR-001 produces a flat component list — that's already useful for inventory; the dep graph is the next layer of fidelity.

**Independent Test**: scan a fixture with `conanfile.txt` listing 2 direct deps and `conan.lock` showing 5 total components (2 direct + 3 transitive). Confirm the emitted SBOM has DEPENDS_ON edges from the main-module component to the 2 direct-dep components, and edges from those direct deps to their respective transitive dependencies as recorded in the lockfile.

**Acceptance Scenarios**:

1. **Given** a fixture with `conanfile.txt` declaring 2 direct `[requires]` (e.g., `zlib/1.3.1`, `boost/1.84.0`) and `conan.lock` pinning those plus 3 transitive deps (e.g., `bzip2/1.0.8`, `libbacktrace/...`), **When** mikebom scans it, **Then** the SBOM contains DEPENDS_ON edges: main → zlib, main → boost, boost → bzip2, boost → libbacktrace, etc. — matching the lockfile's recorded dep graph.
2. **Given** the same fixture without `conan.lock` (only `conanfile.txt`), **When** mikebom scans it, **Then** the SBOM contains only the direct-dep edges (main → zlib, main → boost). No false transitive edges — the lockfile is the only authoritative transitive source.

---

### Edge Cases

- **`conanfile.txt` declares both `[requires]` and `[tool_requires]` (build-only deps)**: emit both, but mark `tool_requires` entries with the standards-native build-scope mechanism in each output format — CDX `scope`, SPDX 2.3 `BUILD_DEPENDENCY_OF`, SPDX 3 `LifecycleScopeType` per milestone-052's conventions. Build-only deps don't appear in deployed binaries; downstream consumers may want to filter.
- **`conanfile.txt` uses version ranges (e.g., `zlib/[>=1.2 <2.0]`) without a lockfile**: the manifest's version constraint is a range, not a concrete version. Emit the component with the range string verbatim as the PURL version segment — same approach as pip's range handling. Operators with no lockfile get a less-precise PURL but still a valid one.
- **`conan.lock` schema version mismatch**: Conan 2.x uses a specific JSON schema (typically `version: "0.5"` or similar). If the lockfile version is unrecognized (e.g., a future Conan release or the legacy Conan 1 format), log a warn and skip lockfile parsing — fall back to manifest-only emission.
- **`conanfile.py` only (Python-manifest project)**: out of scope for this milestone. Log info-level "skipping Python-format Conan manifest"; emit zero conan components from this directory. Future milestone can add a partial Python AST reader.
- **Conan packages with `user/channel` qualifiers** (e.g., `zlib/1.3.1@conan/stable`): preserve the `@user/channel` suffix in the PURL via the PURL spec for `pkg:conan`. Phase 0 research will verify the exact qualifier shape — `user` and `channel` are well-known PURL spec qualifiers for the Conan ecosystem.
- **Conflicting versions across manifest and lockfile**: lockfile wins. Document this in the data-model so operators understand the resolution order.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When mikebom encounters a directory containing `conanfile.txt`, it MUST parse the `[requires]` section and emit one `pkg:conan/<name>@<version>` component per entry. Component metadata MUST include `type=library`, `evidence.identity[].technique = manifest-analysis`, and `confidence = 0.85` (matching the existing per-ecosystem manifest-analysis convention).
- **FR-002**: When `conan.lock` is present alongside (or instead of) `conanfile.txt`, mikebom MUST use the lockfile as the authoritative version-resolution source. Lockfile-pinned versions override `conanfile.txt`'s declared ranges; lockfile-recorded transitives are emitted in addition to the direct deps.
- **FR-003**: mikebom MUST emit DEPENDS_ON / depends-on relationships per the lockfile's recorded dep graph when present. When only `conanfile.txt` exists, emit only the main-module → direct-`[requires]` edges (no false transitive edges).
- **FR-004**: mikebom MUST distinguish `[requires]` (runtime/library deps) from `[tool_requires]` (build-only deps) via the standards-native scope mechanism in each output format — CDX `scope`, SPDX 2.3 `BUILD_DEPENDENCY_OF`, SPDX 3 `LifecycleScopeType` per milestone-052's standards-native conventions. No new `mikebom:*` property for this — Constitution V audit required.
- **FR-005**: PURL emission MUST conform to the packageurl-spec for `pkg:conan`. Specifically: lowercase namespace = `conan`, lowercase name, version segment per the spec's encoding rules (`+` → `%2B`, etc., per the existing `Purl` newtype's encoding). If a Conan package uses `user/channel` qualifiers, the PURL MUST encode them per the packageurl-spec's `pkg:conan` qualifier definition — verified in Phase 0 research.
- **FR-006**: mikebom MUST handle `conanfile.py` (Python-format) gracefully. For this milestone, the reader logs an info-level message naming the file and emits zero components from it — no crash, no malformed output. A future milestone may add Python AST parsing.
- **FR-007**: The Conan reader MUST handle missing lockfile gracefully. When only `conanfile.txt` exists (no `conan.lock`), emit direct-`[requires]` entries with whatever version string the manifest declares (literal version OR version-range string). Range strings are emitted verbatim in the PURL — same approach as pip's range handling.
- **FR-008**: The Conan reader MUST handle lockfile-only inputs gracefully. When only `conan.lock` exists (no `conanfile.txt`), emit all lockfile-pinned components. No error; this matches the CI-cache-clone workflow where only the lockfile is preserved.
- **FR-009**: The Conan reader MUST detect `conanfile.txt` and `conan.lock` files at directory scan time using the existing `scan_fs` walker. No special CLI flag required — `mikebom sbom scan --path <dir>` discovers Conan manifests in the same pass as it discovers other ecosystem manifests.
- **FR-010**: This milestone MUST NOT add a Conan Center HTTP fetch fallback. Operators with neither lockfile nor cached deps get only what the manifest declares. Network-fetch fallback is out of scope (parallel to milestone 091's "cache-empty fallback is a separate milestone" decision for Go).
- **FR-011**: This milestone MUST NOT add new Cargo dependencies. The existing `serde` / `serde_json` (already in workspace) handles `conan.lock` JSON; an INI-like parser for `conanfile.txt` can be implemented in pure Rust without new crates (the format is simple enough — `[section]` headers + line entries). If a robust INI parser is unavoidable, propose at planning time.
- **FR-012**: This milestone MUST NOT regenerate non-Conan goldens. The existing 27 byte-identity goldens (CDX 1.6 / SPDX 2.3 / SPDX 3 × 9 ecosystems) MUST stay byte-identical. Adding the Conan reader means **3 new goldens** for the new conan test fixture (`golden/cyclonedx/conan.cdx.json`, `golden/spdx-2.3/conan.spdx.json`, `golden/spdx-3/conan.spdx3.json`), plus updates to the ecosystem-count constants if those are pinned anywhere.

### Key Entities

- **`conanfile.txt`**: the operator-authored Conan v2 dep manifest. INI-like format with `[requires]`, `[tool_requires]`, `[generators]`, `[options]`, `[layout]` sections. Only `[requires]` and `[tool_requires]` are in scope for this milestone.
- **`conan.lock`**: the generated lockfile (`conan lock create`). JSON format. Contains a flat list of resolved package references plus a graph structure recording direct vs transitive dep relationships.
- **Conan package reference**: shape `<name>/<version>[@<user>/<channel>][#<recipe-revision>]`. Example: `zlib/1.3.1`, `openssl/3.2.0@bincrafters/stable`, `boost/1.84.0#abcdef123`. Milestone scope: parse `<name>/<version>`; preserve `@user/channel` if present; ignore `#recipe-revision` for v1 (Conan-internal; not security-relevant).
- **`pkg:conan/<name>@<version>` PURL**: the emitted component identifier per the packageurl-spec for the Conan ecosystem.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An operator scanning a synthetic `conanfile.txt` + `conan.lock` fixture sees ≥1 `pkg:conan/<name>@<version>` component in the emitted SBOM. The exact component count matches the lockfile's component count (for fixtures with lockfiles) OR the `[requires]` line count (for manifest-only fixtures).
- **SC-002**: SBOM dependency-relationship graph reflects the Conan lockfile's structure. For a 2-direct + 3-transitive lockfile, the SBOM contains the same 5 components AND ≥4 DEPENDS_ON relationships (main → 2 direct + ≥2 transitive edges from the direct deps to their resolved transitives).
- **SC-003**: 100% of pre-095 milestone test suites pass post-implementation. No regressions in the 9 existing ecosystem readers' assertions, no regressions in the binary scanner, no regressions in the SBOM format parity tests (CDX 1.6 / SPDX 2.3 / SPDX 3).
- **SC-004**: PURL conformance: every emitted `pkg:conan/...` PURL round-trips through the existing `Purl` newtype's parse-and-re-encode validation (already used by every other ecosystem reader). Zero malformed PURLs.
- **SC-005**: `./scripts/pre-pr.sh` clean post-implementation — zero clippy warnings, every test target reports `0 failed`. `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1` opt-in also passes (the new Conan components don't trip SPDX 3 conformance).
- **SC-006**: The new Conan reader's test suite includes coverage for: (a) manifest-only scan, (b) lockfile-only scan, (c) manifest + lockfile combined scan, (d) tool_requires → build-scope mapping, (e) version-range handling without a lockfile, (f) `conanfile.py`-only graceful fallthrough, (g) `user/channel` qualifier preservation. At least one fixture per case.
- **SC-007**: The new Conan fixture is added to the `transitive_parity` audit harness from milestone 083 (or marked deferred-to-future-milestone with a documented reason). Either way, the milestone-083 audit infrastructure is extended to recognize Conan-managed projects as a 10th ecosystem.

## Assumptions

- Conan v2 is the canonical version. Conan v1 (deprecated 2023) is out of scope unless it's already widespread in the user's test corpus — initial scope assumes v2.
- `conanfile.txt` is INI-like enough that a hand-rolled parser is robust enough for v1. If we discover real-world manifests with unexpected syntax (multiline values, escaped section headers), the planning phase can revisit adding a proper INI crate.
- `conan.lock` is JSON. The schema may evolve across Conan minor releases; the parser handles a known-version range and logs a warn for unrecognized versions.
- PURL spec for `pkg:conan` follows the packageurl-spec at the upstream package-url/purl-spec repository's `PURL-TYPES.rst#conan` section. Phase 0 research will verify the exact qualifier shape for `user/channel`.
- The Conan reader integrates into the existing `scan_fs::package_db` module pattern. No new crate, no new binary, no new CLI flag.
- Test fixtures will be added to the milestone-090 fixture-repo (`kusari-sandbox/mikebom-test-fixtures`) under `conan/<scenario-name>/` paths. The fixture-cache mechanism (build.rs / `MIKEBOM_FIXTURES_DIR`) picks them up automatically. SHA pin in `tests/fixtures.rev` is bumped at implementation time.

## Dependencies

- Milestone 003 (multi-ecosystem expansion) — the existing pattern this milestone replicates for Conan.
- Milestone 052 (lifecycle-scope work) — the standards-native `BUILD_DEPENDENCY_OF` mapping for `tool_requires`.
- Milestone 083 (transitive-correctness audit) — the Conan fixture should be added here as a 10th ecosystem audit row.
- Milestone 090 (fixture-repo split) — new Conan fixtures live in the sibling repo, not in mikebom main.
- The packageurl-spec for `pkg:conan` — external dependency; well-established; no version churn risk.

## Out of Scope

- **`conanfile.py` (Python-format) parsing**: deferred to a future milestone. Requires Python AST parsing or shelling out to Python — significant scope. v1 emits zero components from `.py` manifests (with a clean log message).
- **Conan v1 lockfile format**: deprecated since 2023; only Conan v2 is supported.
- **Conan Center HTTP fetch fallback**: when manifest has only `name/[version_range]` and no lockfile, mikebom does NOT contact Conan Center to resolve the latest matching version. Range strings emit verbatim. Network-fetch fallback is a separate future milestone (parallel to milestone 091's Go proxy decision).
- **CMake `find_package()` / `FetchContent` parsing**: deferred. CMake doesn't have a structured dep-manifest format; would require running CMake or running a heuristic-based pattern scanner. Out of scope for "start simple".
- **vcpkg `vcpkg.json` parsing**: a natural follow-up milestone but out of scope here. Same shape as Conan reader once Conan lands; pattern is reusable.
- **Meson `subprojects/*.wrap` parsing**: same — separate future milestone.
- **Bazel `MODULE.bazel` parsing**: separate future milestone.
- **autoconf `configure.ac` parsing**: low signal; not planned.
- **DWARF debug-info dep extraction from compiled C/C++ binaries**: separate milestone. The current binary scanner reads `DT_NEEDED` (runtime linkage); DWARF would add compile-time source-file paths and the libraries that contributed them. Useful but architecturally distinct.
- **eBPF trace mode for C/C++ builds**: already exists experimentally (`mikebom trace run` + `ebpf-tracing` feature). This milestone doesn't change the trace path; the Conan reader complements it (source-side manifest reader) but doesn't depend on it.
- **Conan Center / ConanCenterIndex enrichment** (license, vendor, upstream Git URL): not planned. mikebom emits what the lockfile + manifest declare; downstream enrichment is operator's choice.
- **Conan profile / options propagation into PURLs** (e.g., `pkg:conan/zlib@1.3.1?profile=msvc`): not in the packageurl-spec; out of scope.
- **`recipe-revision` (`#abcdef`) suffix in package references**: parsed-and-discarded for v1. Conan-internal; not security-relevant to SBOM consumers.
