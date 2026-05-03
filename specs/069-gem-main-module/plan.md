# Implementation Plan: gem source-tree main-module component for top-level *.gemspec roots

**Branch**: `069-gem-main-module` | **Date**: 2026-05-03 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/069-gem-main-module/spec.md`

## Summary

Extend the milestone 053+064+066+068+#127 main-module pattern to gem: emit one synthetic main-module per top-level `*.gemspec` (skipping install-state paths and application-style projects). Reuses existing `parse_gemspec_full` parser and `build_gem_purl` helper. Smallest #104 milestone — generator-side machinery is unchanged; reader-side adds a project-root walker (separate from the existing `find_gemspecs` install-state walker), an entry builder, and a dedup helper.

## Technical Context

**Language/Version**: Rust stable; no nightly.
**Primary Dependencies**: Existing only — no new crates. Reuses `parse_gemspec_full` (regex-based pure-Rust gemspec parser at `gem.rs:947`), `build_gem_purl` (PURL helper), `parse_gemspec_groups` (dep-section extractor for FR-007 edge classification).
**Storage**: N/A — in-process per scan.
**Testing**: `cargo +stable test --workspace`; new `gem-pyject-style` fixture; integration tests for the 4 acceptance scenarios.
**Target Platform**: Linux (x86_64 + aarch64) + macOS (aarch64).
**Performance Goals**: One additional `*.gemspec` parse per top-level project root. Sub-millisecond.
**Constraints**: Cross-host byte-identity goldens. Pure-Rust regex parsing only (no Ruby execution per A9).
**Scale/Scope**: One main-module per top-level `*.gemspec`; typical Ruby gem projects have 1.

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. Pure Rust, Zero C** | ✅ Pass | Regex parsing only; no Ruby interpreter. |
| **II. eBPF-Only Observation** | ✅ Pass | Main-module represents the implicit scan target. |
| **III. Fail Closed** | ✅ Pass | Non-literal version → `0.0.0-unknown` placeholder; non-parseable name → skip. Application-style projects skip with no synthetic identity. |
| **IV. Type-Driven Correctness** | ✅ Pass | Reuses `PackageDbEntry`, `Purl`. |
| **V. Specification Compliance** | ✅ Pass — **AUDIT PERFORMED** | Native CDX `metadata.component`, SPDX `primaryPackagePurpose: APPLICATION`, SPDX 3 `software_primaryPurpose: application`. PURL conforms to spec; existing `build_gem_purl` already percent-encodes non-PURL-safe chars. |
| **VI. Three-Crate Architecture** | ✅ Pass | All changes within `mikebom-cli/`. |
| **VII. Test Isolation** | ✅ Pass | Unit + integration; no eBPF, no privileges. |
| **VIII. Completeness** | ✅ Pass | Adds project-self component to gem SBOMs. |
| **IX. Accuracy** | ✅ Pass | Manifest-derived versions are authoritative; placeholder fallback is transparent. |
| **X. Transparency** | ✅ Pass | Same-PURL dedup `tracing::warn!`. |
| **XI. Enrichment** | ✅ Pass | LICENSE detection deferred to issue #103. |
| **XII. External Data Source Enrichment** | ✅ Pass | `*.gemspec` is read for the main-module's identity. |
| **Strict Boundary #1 (No lockfile-based dep discovery)** | ✅ Pass | Direct edges relocate from synthetic placeholder to main-module; no new components from manifest data. |

**Gate result**: Pass.

## Project Structure

### Documentation (this feature)

```text
specs/069-gem-main-module/
├── plan.md              # This file
├── spec.md              # Feature specification (no Clarifications needed)
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── gem-main-module-component.md
├── checklists/
│   └── requirements.md  # All-green
└── tasks.md             # Phase 2 output
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   └── scan_fs/
│       └── package_db/
│           └── gem.rs                # ⬅️ MAIN CHANGE — new
│                                     #     build_gem_main_module_entry()
│                                     #     + find_top_level_gemspecs() walker
│                                     #     (excludes vendor/, gems/,
│                                     #     specifications/, .bundle/)
│                                     #     + dedup_gem_main_modules_by_purl()
│                                     #     + Phase A wire-up in `read()`
│                                     #     after the existing dep-emission
│                                     #     loops, augment-existing pattern.
└── tests/
    ├── fixtures/
    │   └── gem-source-project/      # ⬅️ NEW FIXTURE — minimal gem project
    │       ├── foo.gemspec           #   s.name="foo" + s.version="1.0.0"
    │       │                         #   + s.add_dependency("rake")
    │       └── README.md
    └── scan_gem.rs                   # ⬅️ NEW or extend — 5 integration
                                       #   tests (US1 AS#1-4 + non-literal
                                       #   version case)

docs/
└── reference/
    └── sbom-format-mapping.md       # ⬅️ DOC UPDATE — extend C40 row's
                                      #   per-ecosystem matrix: Go ✅,
                                      #   cargo ✅, npm ✅, pip ✅, gem ✅;
                                      #   maven still in #104.

CHANGELOG.md                          # ⬅️ DOC UPDATE — `[Unreleased]` →
                                       #   `### Changed (BREAKING — SBOM
                                       #   output shape, milestone 069)`
```

**Structure Decision**: Single-crate (`mikebom-cli`) feature. The gem reader (`scan_fs/package_db/gem.rs`) gains the new helpers; generator-side machinery is unchanged.

## Phase 0: Outline & Research — COMPLETE (in-spec)

Phase 0 captured in spec Assumptions A1–A10. Key decisions:

- **Decision**: Pure-Rust regex parsing of `*.gemspec` literal-string assignments via existing `parse_gemspec_full`. **Rationale**: A9 — never execute Ruby code; cross-host determinism; zero new dependencies. **Alternatives**: shell out to Ruby interpreter (rejected — runtime dep + non-determinism + security implications).
- **Decision**: Non-literal version → `0.0.0-unknown` placeholder. **Rationale**: same convention as 053/064/066/068; common Ruby idiom is `s.version = Foo::VERSION` reading from `lib/<name>/version.rb`, which mikebom won't follow. **Alternatives**: scan `lib/<name>/version.rb` for the constant value (rejected — adds parsing complexity for a low-payoff edge case; placeholder is transparent).
- **Decision**: New top-level walker, separate from existing `find_gemspecs` (which descends into `specifications/` only). **Rationale**: existing walker is biased toward install-state paths; project-root manifests need a different traversal pattern. **Alternatives**: extend `find_gemspecs` with a mode flag (rejected — two distinct semantics in one function adds confusion).
- **Decision**: Application-style projects (Gemfile + Gemfile.lock, no `*.gemspec`) skip emission per FR-002. **Rationale**: Ruby explicitly distinguishes publishable gems (with `.gemspec`) from applications (Gemfile-only); emitting a synthetic identity for the latter would be a false signal. **Alternatives**: emit a synthetic `pkg:generic/...` placeholder (rejected — would diverge from cargo/npm/pip pattern where placeholder is "version unresolvable" not "no project identity").

## Phase 1: Design & Contracts

### 1. Data model

`data-model.md` — captures:

- **GemMainModuleEntry**: `PackageDbEntry` constrained to top-level `*.gemspec` emission. PURL `pkg:gem/<name>@<version>`; `parent_purl: None`; `sbom_tier: Some("source")`; C40 + `mikebom:component-role: "main-module"`; depends from `parse_gemspec_groups`.
- **DroppedDuplicate**: same shape as cargo (064) / npm (066) / pip (068).

### 2. Contracts

`contracts/gem-main-module-component.md` — per-format placement contract identical to cargo/npm/pip, only PURL prefix `pkg:gem/...` differs. Multi-main-module super-root behavior inherits from #127.

### 3. Quickstart

`quickstart.md` — three recipes: top-level `*.gemspec` single-project, non-literal version case, application-style skip.

### 4. Agent context update

Run `.specify/scripts/bash/update-agent-context.sh claude` post-commit. No new technologies.

### 5. Re-evaluate Constitution Check

Re-checked above table — no new violations.

**Phase 1 outputs**: this section + `data-model.md` + `contracts/gem-main-module-component.md` + `quickstart.md` (next run).

## Complexity Tracking

*No constitution violations to justify. Section intentionally empty.*
