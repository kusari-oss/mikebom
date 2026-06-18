# Implementation Plan: Binary-tier completion (milestone 130)

**Branch**: `130-binary-tier-completion` | **Date**: 2026-06-18 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/130-binary-tier-completion/spec.md`

## Summary

Three follow-up tracks from milestone 129, prioritized debug-first:

1. **US1 (P1)** — `cargo-auditable` binary reader fix. **Root cause already identified during planning**
   (see research R1): the milestone-029 `parse_dep_v0` reader IS functioning correctly, but the call
   site at `mikebom-cli/src/scan_fs/binary/mod.rs:700` is gated by `!skip_secondary_evidence`, which
   becomes `true` for any binary claimed by a package-db reader (apk/dpkg/rpm). On the audit image,
   `/usr/bin/uv` and `/usr/bin/uvx` are apk-claimed (Wolfi package), so the cargo-auditable emission
   is suppressed. **The fix is ~5 LOC**: remove the `skip_secondary_evidence` gate around the
   `if let Some(ref manifest) = scan.cargo_auditable { ... }` block at lines 700-708; keep all OTHER
   `skip_secondary_evidence`-gated blocks unchanged (those are correct — they handle version-string
   shadows that DO duplicate package-db claims). Per-crate emissions from `cargo-auditable` are NOT
   shadows of the apk-claimed binary — they're a SEPARATE tier of truth (transitive build closure vs
   installed-package identity), so the gate was conceptually wrong from the start.
2. **US2 (P2)** — Maven nested-JAR recursion. Extends the existing milestone-009 `package_db/maven.rs`
   reader with depth-bounded recursive archive descent per the deferred milestone-129 US3 design.
   ~300-400 LOC of bounded new code; SHA-256-keyed visited set for cycle detection; 1 GB
   decompressed-size cap; depth limit 8; `.jar`/`.war`/`.ear` only per milestone-129 clarification Q2.
3. **US3 (P3)** — PE/CLR managed-assembly metadata reader. New module at
   `mikebom-cli/src/scan_fs/binary/dotnet_pe_clr.rs`. ECMA-335 §II.22 metadata-table hand-roll on top
   of `object` 0.36's PE primitives. ~800-1000 LOC. Closes ~451 unique NuGet packages on the audit
   image. **Resource-assembly dedup** per the 2026-06-18 clarification: one component per
   `(name, version)`, with `mikebom:assembly-cultures` listing the set of detected non-"neutral"
   cultures.

Per SC-007, each user story is independently shippable. The plan supports both a single bundled PR
(if all three fit one push) AND a three-PR split (US1 first as the smallest landing → US2 → US3).
**Zero new Cargo dependencies.**

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–129; no nightly
required for this user-space-only work).

**Primary Dependencies**: Existing only — `object = "0.36"` (workspace; ELF section reading for the
US1 fix-verification path, PE COFF + CLR `IMAGE_COR20_HEADER` data-directory parsing for US3), `zip`
(already a direct dep at `mikebom-cli/Cargo.toml` per milestone-009; reused for US2 nested-JAR
descent), `flate2 = "1"` (workspace; reused for US1 — the existing reader uses `flate2::read::ZlibDecoder`
unchanged), `sha2` + `data-encoding` (workspace; SHA-256-keyed visited set for US2 cycle detection,
matching the milestone-128 include-chain convention), `serde`/`serde_json` (workspace; reused for the
existing cargo-auditable manifest decode + new annotation-emission paths), `tracing`, `anyhow`,
`thiserror`. **No new Cargo dependencies.**

The CLR metadata-table reader for US3 is a small enough scope to hand-roll on `object`'s PE primitives
(the table format is documented in ECMA-335 §II.22). No `pelite` or `clrmetadata` crate added — for
our scope (four string fields per managed DLL: AssemblyName + Version 4-tuple + FileVersion +
InformationalVersion + Culture), hand-rolling is ~800 LOC of straight-line code matching the existing
binary-tier convention. See research R3 for the metadata-table walk's exact wire-format details.

**Storage**: N/A — all state in-process per scan; no caches, no persistence. Matches every milestone
since 002.

**Testing**: Standard `cargo +stable test --workspace`. Three new integration test files (one per
user story) + unit tests inside each new/modified module. Synthetic fixtures vendored in
`mikebom-cli/tests/fixtures/binary_tier_completion/` per the milestone-128 "stay-set" rule. For US3,
PE fixtures are hand-crafted minimal-managed-assembly byte arrays embedded via `include_bytes!` — no
real Microsoft DLLs committed (size concerns + provenance-tracking burden); the real-image acceptance
test runs against a public `mcr.microsoft.com/dotnet/runtime:8.0-alpine` pull.

**Target Platform**: Linux rootfs (`mikebom sbom scan --image` always targets a Linux container per
the existing `--image-platform` constraint). The PE/CLR reader operates on `.dll` files INSIDE a Linux
container's rootfs (e.g. `/usr/share/dotnet/sdk/8.0.127/...`); mikebom does not target Windows hosts
itself.

**Project Type**: CLI tool (the `mikebom sbom scan` polyglot-scanner pipeline). NOT the eBPF trace
pipeline; the relevant Principle II vs polyglot-scanner clarification from milestone 129 plan §R9
carries forward verbatim.

**Performance Goals**:
- US1 cargo-auditable fix: zero added overhead (it's a gate removal, not new work).
- US2 per-nested-archive parse: <50ms for a typical Spring Boot uber JAR's nested JAR (~1 MB
  uncompressed).
- US3 per-DLL PE/CLR parse: <20ms (small fixed-size metadata table read; no JIT, no method body
  parsing).
- Total scan time growth on the audit image: <30% relative to milestone 129 (per SC-006).

**Constraints**:
- Zero new Cargo dependencies (verified via `cargo tree -p mikebom`).
- Byte-identity preservation across the 33 committed alpha.48 goldens (verified via
  `./scripts/regen-goldens.sh` producing zero `.cdx.json` / `.spdx.json` churn).
- All three reader paths MUST respect `--offline` (no network), `--exclude-path` (milestone 113),
  and `safe_walk` paths (milestone 114).
- US2 per-nested-archive decompressed-size cap: 1 GB (FR-014).
- US2 nested-archive depth limit: 8 levels (FR-012, matching milestone-128 `INCLUDE_DEPTH_LIMIT`).

**Scale/Scope**:
- ~900-1000 new `pkg:cargo` components emitted per typical cargo-auditable-tool-bearing image
  (the audit image's `/usr/bin/uv` + `/usr/bin/uvx` ≈ 200 + ~700 unique crates).
- ~300 new `pkg:maven` components from US2 nested-JAR recursion per typical fat-JAR-bearing image.
- ~450 new `pkg:nuget` components from US3 PE/CLR metadata per typical .NET-SDK-bearing image.
- 4 new `mikebom:*` annotation keys catalogued + parity-extracted (C-row range C92..C95 expected,
  final range pinned at implementation time):
  - `mikebom:assembly-version-informational` (US3)
  - `mikebom:assembly-version-file` (US3)
  - `mikebom:assembly-version-runtime` (US3)
  - `mikebom:assembly-cultures` (US3 — plural per the 2026-06-18 clarification)
- US1 + US2 introduce NO new `mikebom:*` annotation keys. US1 reuses the existing
  `cargo-auditable-binary` source-mechanism string. US2 introduces the new
  `maven-jar-nested` source-mechanism string but routes via the EXISTING
  `mikebom:source-mechanism` annotation channel — no new annotation key, just a new value variant.

## Constitution Check

Audit against `mikebom Constitution v1.4.0` (`.specify/memory/constitution.md`):

| Principle | Verdict | Notes |
|---|---|---|
| I. Pure Rust, Zero C | ✓ Pass | All new code Rust; zero new transitive C deps (`object`, `zip`, `flate2`, `sha2`, `serde_json` all pure Rust). |
| II. eBPF-Only Observation | ✓ N/A | This milestone targets the `mikebom sbom scan` polyglot pipeline. Principle II governs the SEPARATE `mikebom trace` eBPF pipeline. (Same clarification as milestone 129 plan §R9.) |
| III. Fail Closed | ✓ Pass | Parse failures emit a single `warn`-level log + a `mikebom:parse-failure` component-scope annotation; scan continues on sibling files (FR-005). No silent omission. The US1 fix REMOVES a silent suppression (the `skip_secondary_evidence` gate was wrong, not failsafe) — this strengthens the principle. |
| IV. Type-Driven Correctness | ✓ Pass | All new components flow through `mikebom_common::types::purl::Purl`. CLR metadata parsing uses typed enums for table-token / heap-handle / blob-prolog discrimination; no stringly-typed dispatch. No `.unwrap()` in production code — `anyhow` for application errors, `thiserror` for module-internal error variants. |
| V. Specification Compliance | ⚠ Audit pending per-key | **Four new `mikebom:*` keys** (`assembly-version-{informational,file,runtime}`, `assembly-cultures`) all parity-bridging — neither CDX 1.6 nor SPDX 2.3 nor SPDX 3 has native multi-version preservation OR multi-culture set storage. Catalogued in `contracts/annotation-schema.md` with full Principle V audit narratives. Same shape as milestone-128's catalogue posture. |
| VI. Three-Crate Architecture | ✓ Pass | All new code lives in `mikebom-cli`. No new crate. |
| VII. Test Isolation | ✓ Pass | All tests run unprivileged. Synthetic byte-array fixtures embedded via `include_bytes!` for PE; in-memory ZIP builder helper for maven nested-JAR per the 2026-06-18 clarification Q2. |
| VIII. Completeness | ✓ Pass + improves | US1 fix CLOSES a completeness regression (cargo-auditable was silently suppressed by `skip_secondary_evidence`). US2 + US3 add coverage where mikebom previously emitted nothing. |
| IX. Accuracy | ✓ Pass | All PURLs derived directly from the source artifact (cargo-auditable wire format / pom.properties / CLR Assembly table). No heuristic matching. The milestone-105 dedup pipeline handles collisions correctly. |
| X. Transparency | ✓ Pass | `mikebom:assembly-cultures` is exactly the kind of transparency annotation Principle X envisions (per-component, structured, audit-trail-preserving). Parse failures surface via `mikebom:parse-failure`; depth-limit hits via existing nested-archive warn-logs. |
| XI. Enrichment | ✓ Pass | CPE candidates per the milestone-097 channel reused for new components. |
| XII. External Data Source Enrichment | ✓ N/A | No external data sources consulted. `--offline` honored (FR-004). |

**Strict Boundaries**:

1. **No lockfile-based dependency discovery** — N/A; this is the polyglot-scanner pipeline.
2. **No MITM proxy** — ✓ N/A.
3. **No C code** — ✓ Zero new C deps.
4. **No `.unwrap()` in production** — ✓ All new modules carry the standard
   `#[cfg_attr(test, allow(clippy::unwrap_used))]` guard on `mod tests`.

**Gate verdict**: PASS with the catalog-narrative requirement folded into the plan (contracts/annotation-schema.md
covers all four new keys before implementation).

## Project Structure

```text
specs/130-binary-tier-completion/
├── plan.md              # This file
├── spec.md              # Feature spec (exists)
├── research.md          # Phase 0 output (this command)
├── data-model.md        # Phase 1 output (this command)
├── quickstart.md        # Phase 1 output (this command)
├── contracts/           # Phase 1 output (this command)
│   ├── annotation-schema.md     # 4 new mikebom:* keys (US3) with Principle V audits
│   └── reader-behavior.md       # Per-reader I/O / side-effect / observability contract
├── checklists/
│   └── requirements.md  # Spec-quality checklist (exists from /speckit-specify)
└── tasks.md             # Phase 2 output (/speckit-tasks; NOT created by this command)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   ├── scan_fs/
│   │   ├── binary/
│   │   │   ├── mod.rs                   # MODIFIED — US1: remove skip_secondary_evidence
│   │   │   │                            #          gate around the cargo_auditable emission
│   │   │   │                            #          block at lines 700-708 ONLY; other gated
│   │   │   │                            #          blocks (version-string scan, linkage,
│   │   │   │                            #          ELF-note) stay gated.
│   │   │   ├── cargo_auditable.rs       # UNCHANGED — existing milestone-029 reader is correct
│   │   │   ├── dotnet_pe_clr.rs         # NEW — US3 PE/CLR managed-assembly metadata reader
│   │   │   └── scan.rs                  # UNCHANGED — section discovery already correct
│   │   └── package_db/
│   │       ├── maven.rs                 # MODIFIED — US2: add walk_nested_archives helper
│   │       │                            #          + cycle detection + size cap + emission
│   │       └── nuget/
│   │           └── deps_json.rs         # UNCHANGED — milestone 129 US1A reader; US3 emissions
│   │                                    #          dedup against these via milestone-105
│   ├── parity/
│   │   └── extractors/
│   │       ├── cdx.rs                   # +4 cdx_anno! entries (C92..C95)
│   │       ├── spdx2.rs                 # +4 spdx23_anno! entries
│   │       ├── spdx3.rs                 # +4 spdx3_anno! entries
│   │       └── mod.rs                   # +4 ParityExtractor slice entries + matching `use`
│   └── generate/                        # UNCHANGED — emission paths unchanged
└── tests/
    ├── common/
    │   └── maven_jar_builder.rs         # NEW — synthetic ZIP builder helper per 2026-06-18 Q2
    ├── binary_tier_completion_us1_cargo_auditable.rs  # NEW — US1 acceptance scenarios
    ├── binary_tier_completion_us2_maven_nested_jar.rs # NEW — US2 acceptance scenarios
    ├── binary_tier_completion_us3_dotnet_pe_clr.rs    # NEW — US3 acceptance scenarios
    └── fixtures/binary_tier_completion/
        ├── cargo_auditable_regression/
        │   └── claimed_binary_with_dep_v0.elf   # Synthetic ELF: cargo-auditable + apk-style claim
        ├── maven_nested_jar/
        │   └── (built at test time via maven_jar_builder.rs)
        └── dotnet_pe_clr/
            ├── valid_clr_no_culture.dll         # AssemblyName=Foo.Bar, Version=1.2.3.4, Culture=""
            ├── valid_clr_with_cultures/        # Resource-assembly fan-out test
            │   ├── Foo.Bar.dll                  # Culture=""
            │   ├── de/Foo.Bar.resources.dll     # Culture="de"
            │   └── fr/Foo.Bar.resources.dll     # Culture="fr"
            ├── native_pe_no_clr.dll             # PE without CLR header — must skip silently
            └── corrupt_clr_metadata.dll         # PE with CLR header but corrupt tables

docs/reference/sbom-format-mapping.md            # +4 C-rows (C92..C95) with Principle V audits
CHANGELOG.md                                     # +1 milestone-130 entry under [Unreleased]
```

**Structure Decision**: Three reader paths, three (or four) locations:

- **`scan_fs/binary/mod.rs`** (US1) — the fix is a small targeted gate removal, NOT a new file.
- **`scan_fs/package_db/maven.rs`** (US2) — extends the existing milestone-009 reader with a recursive
  helper. No new module.
- **`scan_fs/binary/dotnet_pe_clr.rs`** (US3) — new module living alongside `pe.rs` (the existing PE
  identity reader for PDB-id / machine / subsystem) but with a separate concern (CLR metadata vs PE
  identity). Naming follows the milestone-096..099 convention.

The US3 reader is the only NEW module. US1 and US2 are surgical extensions to existing modules.

## Phase 0: Outline & Research

See [research.md](./research.md).

Research topics covered:

1. **US1 root cause** — confirmed during planning. `skip_secondary_evidence` gate at
   `mikebom-cli/src/scan_fs/binary/mod.rs:700` suppresses cargo-auditable emission for any binary
   claimed by a package-db reader. The gate's design intent ("don't double-emit shadows") is correct
   for version-string scanning but WRONG for cargo-auditable (per-crate transitive build closure is
   not a shadow of the file-level binary identity).
2. **US2 nested-JAR walker design** — depth-limited recursive `zip::ZipArchive` descent with
   SHA-256-keyed visited set + 1 GB per-archive size cap. Direct reuse of the milestone-128
   include-chain conventions and the milestone-129 deferred US3 design.
3. **US3 CLR metadata-table layout (ECMA-335 §II.22)** — Assembly table row 0 carries
   `MajorVersion`/`MinorVersion`/`BuildNumber`/`RevisionNumber`/`Flags`/`PublicKey`/`Name`/`Culture`.
   CustomAttribute table carries `AssemblyFileVersionAttribute` and `AssemblyInformationalVersionAttribute`
   custom-attribute blob references.
4. **US3 resource-assembly dedup mechanics** — milestone-105 `SourceMechanism`-keyed dedup pipeline
   already handles cross-mechanism collisions; the new dimension (culture set) needs an additive
   merge step.
5. **Audit-image cargo unique count baseline** — confirmed during planning that syft's 986 cargo
   count is the unique-`(name,version)` count from `cargo-auditable-binary-cataloger` (not
   per-binary-multi-counted). SC-001 ≥900 is achievable from the US1 fix alone.
6. **PE/CLR fixture provenance** — synthetic hand-crafted PE byte arrays (~5 KB each) embedded via
   `include_bytes!`. No real Microsoft DLLs committed (provenance burden + repo bloat).

## Phase 1: Design & Contracts

See [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md).

Phase 1 artifacts:

- **data-model.md** — 3 entities: `CargoAuditableEmissionDecision` (US1; the decision-table
  documenting the gate-removal fix's behavioral surface — not a new Rust type, but the regression
  scope the fix opens up), `NestedArchiveWalker` (US2; matches the milestone-129 deferred design
  verbatim), `ManagedPeAssembly` (US3; per the milestone-129 deferred design + the 2026-06-18
  culture-set addition).
- **contracts/annotation-schema.md** — 4 new `mikebom:*` keys with Principle V audit narratives.
  C-rows C92..C95 expected; final numbering at implementation time.
- **contracts/reader-behavior.md** — Per-reader I/O / side-effect / observability contracts. US1
  contract documents the BEHAVIORAL DIFFERENCE the gate removal introduces (cargo-auditable now fires
  on claimed binaries) and the regression-test surface (FR-008).
- **quickstart.md** — Three operator-facing scenarios mirroring milestone-129 quickstart shape.

### Agent context update

After Phase 1, the plan invokes `.specify/scripts/bash/update-agent-context.sh claude` which appends
a milestone-130 entry to CLAUDE.md.

## Complexity Tracking

> Nothing requiring complexity-tracking entries. The US1 fix is a 5-LOC gate removal. US2 and US3
> are bounded per-story implementations following the milestone-128 / milestone-129 conventions for
> recursive walkers and binary readers.
