# Implementation Plan: Helm chart scanning (Chart.yaml + Chart.lock + charts/*.tgz + templates/*.yaml)

**Branch**: `188-helm-chart-scanning` | **Date**: 2026-07-13 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/188-helm-chart-scanning/spec.md`

## Summary

Adds Helm chart scanning to mikebom's package-DB reader family. Two-layer emission:

**Chart-level (US1)**: parse `Chart.yaml` + `Chart.lock` + `charts/*.tgz` recursively; emit one `pkg:helm/<repo>/<name>@<version>` component per declared/locked/packaged dep. `Chart.lock` is authoritative over `Chart.yaml` when both present (package-lock.json > package.json precedent).

**Template-level (US2)**: scan `templates/*.yaml` + `crds/*.yaml` for `image: <ref>` extraction using a permissive regex with Go-template tolerance. Unresolved `{{ .Values.image.tag }}` placeholders emit as `pkg:generic/<placeholder>` with `mikebom:image-ref-unresolved = "true"` property. Resolved refs emit as `pkg:docker/<name>@<tag>` (tagged) or `pkg:oci/<name>@sha256:<digest>` (digested).

**Optional `--helm-render` (US3)**: shells out to `helm template <chart-dir>` with a 60s timeout + `MIKEBOM_HELM_RENDER_TIMEOUT_SECS` env override. On success: extracts image refs from the fully-rendered YAML (higher fidelity, no placeholder markers). On failure (helm missing, exit non-zero, timeout): WARN + fall back to US2 unrendered extraction. Zero external-binary calls in the default flow (FR-013).

**Technical approach**: New file `mikebom-cli/src/scan_fs/package_db/helm.rs` (~500-700 LOC + tests). Wired into `package_db::read_all` dispatcher (line 1227 of `mod.rs`) as a new reader called alongside npm/cargo/etc. — composability preserved per Clarifications Q1. New CLI flag `--helm-render` (bool) + `--helm-chart <path>` (PathBuf) on ScanArgs; when `<path>` ends in `.tgz`, mikebom extracts to a tempdir and treats the extracted dir as the scan target (reuses the m031 tarball-extract pattern). Auto-detect via existing `--path <dir>` when `<dir>/Chart.yaml` exists.

Zero new Cargo dependencies. Existing `serde_yaml` (workspace, dart/cocoapods/haskell readers), `tar = 0.4` (workspace, ipk-file reader), `flate2` (workspace), `regex = "1"` (workspace, cmake/vcpkg/alpm readers), `mikebom_common::types::purl::Purl` (existing), `tempfile` (workspace dev + prod). No new subprocess calls in the default flow; the opt-in `--helm-render` uses `std::process::Command` matching the m053 `git describe` + m055 `go mod graph` precedents.

## Technical Context

**Language/Version**: Rust stable (workspace toolchain inherited from milestones 001–187; no nightly required for this user-space-only work).

**Primary Dependencies**: Existing only — `serde_yaml = "0.9"` (workspace; already used by `dart.rs`, `cocoapods.rs`, `haskell.rs`, `pnpm_lock.rs`, `yarn_lock.rs`), `tar = "0.4"` (workspace; already used by `ipk_file.rs`), `flate2` (workspace; gzip decompression), `regex = "1"` (workspace; already used by cmake, vcpkg, alpm, brew, elixir, erlang, cocoapods readers), `mikebom_common::types::purl::{Purl, encode_purl_segment}` (workspace type), `tracing` (WARN/INFO logs), `anyhow` / `thiserror` (error propagation), `serde` / `serde_json` (annotation values), `tempfile` (workspace; used by every tarball-extraction reader), `std::process::Command` (opt-in `--helm-render` shell-out; matches m053 + m055 patterns). **Zero new Cargo dependencies.**

**Storage**: N/A — all state in-process per scan. The recursive chart-dep enumeration accumulates in a stack-allocated `Vec<HelmComponent>` for the duration of one scan; dropped at return. Matches every milestone since 002.

**Testing**: `cargo +stable test --workspace` (unit + integration tests), `cargo +stable clippy --workspace --all-targets -- -D warnings` (lint). New integration test file `mikebom-cli/tests/helm_reader.rs` fabricates synthetic Helm charts at test time (using stdlib file-write ops + `tar` + `flate2` for the `.tgz` cases) — self-contained, no split-fixtures repo dep. Matches m187's `ipk_yocto_reader_fixes.rs` pattern.

**Target Platform**: Linux + macOS user-space (unchanged from prior milestones). Windows support piggybacks on the `#[cfg(unix)]` guards already present in adjacent readers for inode-based dedup; helm parsing itself is byte-level and platform-agnostic.

**Project Type**: CLI (Rust binary + shared common crate). Existing three-crate architecture: `mikebom-cli`, `mikebom-common`, `xtask`.

**Performance Goals**: Chart parsing is O(chart-deps + template-files) — for a stock ingress-nginx chart (~10 chart deps + ~15 template files) the total parse cost is <100ms. The optional `--helm-render` shell-out is the operator's known-latency opt-in (typical `helm template` on a moderate chart: 500ms–3s).

**Constraints**: FR-016 + SC-005 byte-identity guard on default scans (no `Chart.yaml` present + no `--helm-chart` flag). The helm reader MUST NOT introduce any drift on existing golden fixtures. FR-019 / SC-007 zero-new-deps gate (`cargo tree --workspace | wc -l` identical pre/post m188). FR-013 zero-external-binary-calls-in-default: `std::process::Command` invoked ONLY when `--helm-render` is passed.

**Scale/Scope**: 3 user stories (2 P1 + 1 P2). One new file (`helm.rs`) at ~500-700 LOC + inline unit tests. One new integration test file. Two CLI flags (`--helm-render` bool, `--helm-chart <PathBuf>`). Estimated ~24-28 tasks across 6 phases.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

**Principle I (Pure Rust, Zero C)** — PASS. Zero new Cargo dependencies. YAML parsing via existing `serde_yaml` (pure Rust). ar-format `.tgz` extraction via existing `tar` + `flate2` (both pure Rust). Regex via existing `regex` crate (already a direct workspace dep for cmake/vcpkg/alpm/brew/elixir/erlang/cocoapods). The `helm` binary shell-out is opt-in only (US3 / FR-011) and follows the m053 `git describe` precedent for the analogous "opt-in external tool integration" pattern.

**Principle II (eBPF-Only Observation)** — N/A. m188 is user-space package-DB reader work; `mikebom-ebpf` untouched.

**Principle III (Fail Closed)** — PASS. Every parse-error class (malformed YAML, missing Chart.yaml, tarball extraction failure, helm-binary absence, helm-render timeout, helm-render non-zero exit) surfaces a specific WARN log with the underlying reason. Fall-through hierarchy: US3 render → US2 unrendered → NO helm components emitted (but scan still succeeds — helm absence isn't a fatal error since other readers may have produced output). Zero silent drops per FR-006/FR-012/FR-017.

**Principle IV (Type-Driven Correctness)** — PASS. `ChartMetadata` + `ChartLock` + `ChartDep` + `ImageRef` are compile-time-typed structs (deserialized via serde). `ImageRefKind` enum (Tagged / Digested / TemplatePlaceholder) drives PURL construction. `HelmRenderMode` enum (Off / OptIn) drives dispatch. A new module-private `HelmParseError` enum wraps the failure classes.

**Principle V (Specification Compliance + Native-first)** — PASS. `pkg:helm/...`, `pkg:oci/...`, `pkg:docker/...` are all PURL-spec-native. Two new `mikebom:*` annotations added (`mikebom:image-ref-unresolved`, `mikebom:image-extraction-completeness`) — required because CDX 1.6 / SPDX 2.3 / SPDX 3 have no native construct for "this image reference contains an unresolved template placeholder" or "the image-extraction pass ran in reduced-fidelity mode." Documented in `docs/reference/sbom-format-mapping.md` per Constitution Principle V process (native-field audit result cited in the addendum).

**Principle VI (Three-Crate Architecture)** — PASS. All production changes confined to `mikebom-cli/src/scan_fs/package_db/helm.rs` (new) + `mikebom-cli/src/scan_fs/package_db/mod.rs` (dispatcher wire-up, ~10 lines) + `mikebom-cli/src/cli/scan_cmd.rs` (two new flags + tarball-unlock plumbing, ~30 lines). Zero changes to `mikebom-common`, `mikebom-ebpf`, or `xtask`.

**Principle VII (Test Isolation)** — PASS. Unit tests colocated in `helm.rs` `#[cfg(test)]` block (chart-yaml parsing, chart-lock precedence, image-ref regex extraction, tarball recursion). Integration tests via new `mikebom-cli/tests/helm_reader.rs` — synthetic charts fabricated at test time (no root/CAP_BPF requirement; matches m187 pattern).

**Principle VIII (Completeness)** — PASS. m188 CLOSES a "no SBOM scanner handles Helm well" gap in the ecosystem (per issue #455 rationale). On typical open-source charts (ingress-nginx, cert-manager, prometheus-operator) mikebom will emit 5-50x more helm-relevant components than the current pass-through-generic-YAML behavior.

**Principle IX (Accuracy)** — PASS. Templated image refs are explicitly surfaced with `mikebom:image-ref-unresolved = "true"` so consumers don't accept placeholder PURLs as concrete container image identities (FR-008). Chart.lock's authoritative version resolution (FR-004) prevents version drift between declared and locked.

**Principle X (Transparency)** — PASS. Document-scope `mikebom:image-extraction-completeness = "partial"|"full"` annotation surfaces whether extraction ran in reduced or full-fidelity mode (FR-015). Per-component `mikebom:image-ref-raw` preserves the pre-resolution raw string for auditability.

**Principle XI (Enrichment)** — N/A. No external-data enrichment for m188.

**Principle XII (External Data Source Enrichment)** — N/A. Same as XI.

**Result**: All 12 principles PASS. No violations to justify. No Complexity Tracking table needed.

## Project Structure

### Documentation (this feature)

```text
specs/188-helm-chart-scanning/
├── plan.md                    # This file
├── research.md                # Phase 0 output (5 decisions — auto-detect posture, PURL type disambiguation, tarball tempdir lifecycle, regex vs YAML-walk extraction, helm-render subprocess pattern)
├── data-model.md              # Phase 1 output (Chart/ChartLock/ChartDep/ImageRef structs, ImageRefKind enum, HelmRenderMode enum, HelmParseError variants, dispatch matrix)
├── quickstart.md              # Phase 1 output (operator + contributor worked examples)
├── contracts/
│   ├── cli-flags.md           # --helm-chart + --helm-render semantics + composability matrix
│   └── extraction-pipeline.md # Chart parse → dep enumeration → template scan → image emission contract
├── checklists/
│   └── requirements.md        # 16/16 PASS from /speckit-specify
├── spec.md                    # Feature specification
└── tasks.md                   # Phase 2 output (/speckit-tasks — NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
mikebom-cli/
├── src/
│   ├── cli/
│   │   └── scan_cmd.rs                       # US1/US2/US3 — new `--helm-chart <path>` + `--helm-render` flags on ScanArgs; tarball-unlock plumbing for `.tgz` inputs (~30 lines)
│   └── scan_fs/
│       └── package_db/
│           ├── mod.rs                        # US1/US2/US3 — wire `helm::read` into `read_all()` dispatcher near npm/cargo callsite (~10 lines)
│           └── helm.rs                       # US1/US2/US3 — NEW FILE — full helm reader (~500-700 LOC + inline unit tests)
└── tests/
    └── helm_reader.rs                        # NEW — US1/US2/US3 integration tests (synthetic Chart.yaml + Chart.lock + templates fabricated at test time)

docs/reference/
└── sbom-format-mapping.md                    # US1/US2 — new §Milestone 188 addendum documenting `mikebom:image-ref-unresolved` + `mikebom:image-extraction-completeness` per Constitution Principle V native-field audit
```

**Structure Decision**: Single-file scope for the production reader inside `mikebom-cli/src/scan_fs/package_db/`. Follows the m140 (elixir), m141 (erlang), m143 (haskell), m187 (ipk-file update) precedent — a single self-contained `<ecosystem>.rs` file with the reader + its `#[cfg(test)]` unit tests. One new integration test file. Two `mod.rs` touches (dispatcher wire-up + module declaration). One `scan_cmd.rs` touch (CLI flags + tarball-unlock).

## Complexity Tracking

*No violations to justify — all 12 constitution principles PASS.*
