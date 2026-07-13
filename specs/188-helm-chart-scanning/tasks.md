---
description: "Task list for m188 Helm chart scanning"
---

# Tasks: Helm chart scanning (m188)

**Input**: Design documents from `/specs/188-helm-chart-scanning/`
**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/ ✓, quickstart.md ✓

**Tests**: Included — data-model.md §7 enumerates 16 unit tests + 16 integration tests. TDD ordering within each phase (unit tests colocated in `helm.rs`, integration tests in `mikebom-cli/tests/helm_reader.rs`).

**Organization**: Three user stories (US1 P1 chart-level, US2 P1 template-level unrendered, US3 P2 render mode). US1 and US2 both share the new `helm.rs` file + `mod.rs` dispatch; US3 adds the opt-in `--helm-render` shell-out on top. Sequential development recommended even though stories are conceptually independent.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story (US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- Rust workspace root at repository root; all production changes in `mikebom-cli/src/scan_fs/package_db/helm.rs` (NEW) + `mikebom-cli/src/scan_fs/package_db/mod.rs` (dispatcher) + `mikebom-cli/src/cli/scan_cmd.rs` (CLI flags); new integration test file at `mikebom-cli/tests/helm_reader.rs`; docs addendum in `docs/reference/sbom-format-mapping.md`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: SC-007 zero-new-deps baseline anchoring.

- [X] T001 Capture pre-m188 `cargo tree --workspace | wc -l` line count baseline — 1136 lines persisted.

**Checkpoint**: Baseline captured.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: New file scaffold + type definitions + enum stubs + CLI flag surface that all three stories depend on. Type-driven correctness gate per Principle IV.

**⚠️ CRITICAL**: US1, US2, US3 all block on Phase 2 completion.

- [ ] T002 Create new module file `mikebom-cli/src/scan_fs/package_db/helm.rs` with:
  - Module-level doc comment describing US1/US2/US3 scope + native-field audit rationale (linking to `docs/reference/sbom-format-mapping.md` §Milestone 188 addendum)
  - `use` statements: `std::path::Path`, `std::process::Command`, `flate2::read::GzDecoder`, `tar::Archive`, `regex::Regex`, `serde::Deserialize`, `mikebom_common::types::purl::{Purl, encode_purl_segment}`
  - Empty `pub fn read(rootfs: &Path, render_mode: HelmRenderMode) -> anyhow::Result<Vec<PackageDbEntry>>` STUB returning `Ok(Vec::new())` (implementation lands in T014).
  - Empty `#[cfg(test)] #[cfg_attr(test, allow(clippy::unwrap_used))] mod tests { use super::*; ... }` block ready for T008/T017/T024 unit tests.
- [ ] T003 Add `pub(super) mod helm;` to `mikebom-cli/src/scan_fs/package_db/mod.rs` module declaration list. Add corresponding `pub use helm::HelmRenderMode;` re-export so `scan_cmd.rs` can consume it.
- [ ] T004 Add all core types per data-model.md §1 to `mikebom-cli/src/scan_fs/package_db/helm.rs`:
  - `ChartMetadata` (§1.1) with `Deserialize` derive + `serde` field renames (`type`, `appVersion`) + `default_chart_type` helper
  - `ChartDep` (§1.2) with `Deserialize` derive
  - `ChartLock` (§1.3) with `Deserialize` derive
  - `ImageRef` (§1.4) with `Debug/Clone/PartialEq/Eq/Hash` derive (needed for dedup)
  - `ImageRefKind` (§1.5) enum
  - `HelmRenderMode` (§1.6) enum + `Default = Off` + `pub` scope (for scan_cmd.rs consumption)
  - `HelmParseError` (§1.7) with `thiserror::Error` derive
  - `HelmComponent` (§3) intermediate enum
  - `ChartDepSource` (§3) enum
- [ ] T005 [P] Add `--helm-chart` + `--helm-render` flags to `ScanArgs` in `mikebom-cli/src/cli/scan_cmd.rs` per data-model.md §2. Both flags include the full help text from contracts/cli-flags.md. Add `conflicts_with = "path"` on `--helm-chart` per contracts/cli-flags.md composability matrix (both are top-level scan targets).
- [ ] T006 Run `cargo +stable build --workspace --all-targets` — verify types + CLI flags compile clean. Only dead-code warnings for unwired stubs expected; cleared in Phase 3+.

**Checkpoint**: US1/US2/US3 tests can now compile against the new types. Type-driven correctness gate closed.

---

## Phase 3: User Story 1 — Chart-level dependency enumeration (Priority: P1) 🎯 MVP

**Goal**: Operators scanning Helm charts get one component per declared/locked/packaged chart dep. Closes the first layer of #455.

**Independent Test**: spec.md §User Story 1 Acceptance 1-5. Verifiable via `mikebom-cli/tests/helm_reader.rs::us1_*` (6 tests) with synthetic Chart.yaml + Chart.lock + charts/*.tgz fabricated at test time.

### Tests for User Story 1 (write FIRST, ensure they FAIL before implementation) ⚠️

- [ ] T007 [P] [US1] Add unit tests to the `#[cfg(test)] mod tests` block in `mikebom-cli/src/scan_fs/package_db/helm.rs` per data-model.md §7.1:
  - `chart_yaml_parses_minimal_shape` — happy path Chart.yaml with just `name`, `version`
  - `chart_yaml_parses_full_shape` — includes dependencies, appVersion, keywords, home
  - `chart_lock_takes_precedence_over_chart_yaml` — FR-004 verification (declared 1.0.0, locked 1.0.5 → emit 1.0.5)
  - `chart_dep_with_url_repo_produces_correct_purl` — `https://charts.bitnami.com/bitnami` → `pkg:helm/charts.bitnami.com/nginx@13.0.0`
  - `chart_dep_with_alias_repo_uses_alias` — `@bitnami` → `pkg:helm/@bitnami/nginx@13.0.0`
  - `chart_dep_with_no_repo_falls_back_to_generic` — WARN + `pkg:generic/<name>@<version>`
  Guard `.unwrap()` per CLAUDE.md pre-PR clippy convention.

### Implementation for User Story 1

- [ ] T008 [US1] Implement `parse_chart_yaml(chart_dir: &Path) -> Result<ChartMetadata, HelmParseError>` in `helm.rs`. Reads `<chart_dir>/Chart.yaml` via `std::fs::read_to_string` + `serde_yaml::from_str`. Wraps I/O errors as `HelmParseError::ChartYamlRead`; parse errors as `HelmParseError::ChartYamlParse`.
- [ ] T009 [US1] Implement `parse_chart_lock(chart_dir: &Path) -> Option<ChartLock>` in `helm.rs`. Returns `None` when `Chart.lock` is absent (this is normal — no chart-lock file). On parse failure, log WARN + return `None` per contracts/extraction-pipeline.md §Phase A. Never propagates the error (Chart.lock is best-effort).
- [ ] T010 [US1] Implement `resolve_chart_dep(dep: &ChartDep, lock: Option<&ChartLock>) -> (String, ChartDepSource)` in `helm.rs`. Returns `(version, source_kind)` per FR-004 precedence: if `lock` has an entry with the same `name` + `repository`, use lock's `version` + `ChartDepSource::ChartLock`; else use dep's `version` + `ChartDepSource::ChartYaml`.
- [ ] T011 [US1] Implement `build_helm_purl(name: &str, version: &str, repo_or_alias: Option<&str>) -> Purl` in `helm.rs` per data-model.md §5:
  - `Some(alias)` starting with `@` → `pkg:helm/<alias>/<name>@<version>`
  - `Some(url)` — parse URL, use host as namespace → `pkg:helm/<host>/<name>@<version>`
  - `None` — fall back to `pkg:generic/<name>@<version>` with WARN log (no PURL type inference possible)
  All values URL-encoded via `encode_purl_segment` from `mikebom-common`.
- [ ] T012 [US1] Implement `process_subchart_tgz(tgz_path: &Path, depth: usize) -> Vec<HelmComponent>` in `helm.rs`. Extracts the `.tgz` to a `tempfile::TempDir`, recursively runs `parse_chart_yaml` + `parse_chart_lock` on the extracted content. Bail at `depth > 12` per m114 walker convention. Errors bubble as `HelmParseError::SubchartTarballFailed` (caller WARNs + skips).
- [ ] T013 [US1] Implement top-level chart enumeration in `helm.rs::read_chart_at(chart_dir: &Path) -> Result<Vec<HelmComponent>, HelmParseError>`. Sequence: (a) parse Chart.yaml; (b) emit `HelmComponent::Chart` for the root; (c) parse Chart.lock (best-effort); (d) for each `Chart.yaml.dependencies` entry, resolve + emit `HelmComponent::ChartDep`; (e) enumerate `charts/*.tgz` via `safe_walk` + call `process_subchart_tgz` for each; extend the result with subchart HelmComponents.
- [ ] T014 [US1] Implement `helm::read` top-level entry point in `helm.rs`. Sequence: (a) auto-detect gate — return `Ok(vec![])` if `<rootfs>/Chart.yaml` doesn't exist; (b) call `read_chart_at`; (c) FOR NOW skip Phase B/C (US2/US3 in later phases — leave a TODO comment); (d) call `components_to_package_db_entries` with `ExtractionMode::Unrendered` (Phase B addition in T023).
- [ ] T015 [US1] Implement `components_to_package_db_entries(components: Vec<HelmComponent>, extraction_mode: ExtractionMode) -> Vec<PackageDbEntry>` in `helm.rs` per data-model.md §6 property matrix. For each Chart / ChartDep component: build PURL via T011; construct `PackageDbEntry` with `evidence_kind`, `mikebom:*` annotations per matrix. Image emission wired in T023 (US2).
- [ ] T016 [US1] Wire `helm::read` into `read_all()` dispatcher in `mikebom-cli/src/scan_fs/package_db/mod.rs` per data-model.md §4. Insert near the existing dpkg/npm/cargo callsites (around line 1300). Extend `read_all` signature with new `helm_render_mode: HelmRenderMode` + `helm_diagnostics: &mut ScanDiagnostics` parameters (see T023 for the ScanDiagnostics side). **Enumerate every existing `read_all(` call site** via `grep -rEn 'read_all\(' mikebom-cli/src/` — as of pre-m188 there is at least one production caller (`scan_cmd.rs::execute`) and at least one test caller (`package_db/mod.rs:1990` — `read_all_falls_back_to_debian_namespace_when_id_missing`). Update every call site to pass `HelmRenderMode::Off` (preserves pre-m188 behavior). Extend the `--helm-chart` handling in `scan_cmd.rs::execute` to (a) reject `--helm-chart` + `--path` combo per contracts/cli-flags.md; (b) if `<path>.tgz`, extract to tempdir before treating as scan target.
- [ ] T017 [US1] Add integration test file `mikebom-cli/tests/helm_reader.rs` with US1 tests per data-model.md §7.2:
  - `us1_chart_yaml_only_produces_expected_components`
  - `us1_chart_lock_overrides_chart_yaml_versions`
  - `us1_charts_tgz_subchart_deps_emit_recursively`
  - `us1_helm_chart_flag_with_tarball_extracts_and_scans`
  - `us1_helm_chart_flag_with_invalid_tarball_exits_nonzero`
  - `us1_composability_with_npm_reader` — Chart directory containing `package.json` → both helm AND npm components emitted per Clarifications Q1
  Reuse the tempdir + `Command::new(env!("CARGO_BIN_EXE_mikebom"))` pattern from `mikebom-cli/tests/ipk_yocto_reader_fixes.rs` (m187 precedent). Fabricate charts inline via `std::fs::write` + `tar` + `flate2` builders. Guard `.unwrap()` per CLAUDE.md.

**Checkpoint**: US1 fully functional. Ship as MVP. `mikebom sbom scan --path <chart-dir>` emits helm components for the first time.

---

## Phase 4: User Story 2 — Template-level image-reference extraction (unrendered) (Priority: P1)

**Goal**: Operators get image-ref components from `templates/*.yaml` + `crds/*.yaml` extraction, with Go-template placeholder tolerance. Closes the second layer of #455.

**Independent Test**: spec.md §User Story 2 Acceptance 1-5. Verifiable via `mikebom-cli/tests/helm_reader.rs::us2_*` (5 tests). The T017 test file already exists; T024 appends US2 tests to it.

### Tests for User Story 2 (write FIRST) ⚠️

- [ ] T018 [P] [US2] Add unit tests to the `#[cfg(test)] mod tests` block in `helm.rs` per data-model.md §7.1:
  - `image_ref_regex_extracts_tagged` — `image: nginx:1.27.0` → Tagged
  - `image_ref_regex_extracts_digested` — `image: nginx@sha256:...` → Digested
  - `image_ref_regex_extracts_placeholder` — `image: "{{ .Values.image }}"` → TemplatePlaceholder
  - `image_ref_regex_extracts_mixed` — mixed placeholder+literal → TemplatePlaceholder (any placeholder taints)
  - `image_ref_regex_handles_quoted_and_unquoted` — both `image: nginx:1.27.0` and `image: "nginx:1.27.0"` extract
  - `image_ref_regex_handles_comments` — trailing `# comment` stripped
  - `library_prefix_added_for_dockerhub_unqualified` — `nginx:1.27.0` → `pkg:docker/library/nginx@1.27.0`
  - `library_prefix_not_added_for_registry_prefixed` — `ghcr.io/foo/bar:v1` → `pkg:docker/ghcr.io/foo/bar@v1`
  - `dedup_collapses_same_ref_from_multiple_templates` — same image in 2 files → 1 component + `evidence.occurrences[]` with 2 paths

### Implementation for User Story 2

- [ ] T019 [US2] Add `static IMAGE_REGEX: OnceLock<Regex>` in `helm.rs` per contracts/extraction-pipeline.md §Phase B — `(?m)^(\s*)image:\s*['"]?([^'"\s{]+(?:\{\{[^}]+\}\}[^'"\s]*)*)['"]?\s*(#.*)?$`. Include a doc comment linking to spec.md's regex specification.
- [ ] T020 [US2] Implement `classify_image_ref(raw: &str) -> ImageRefKind` in `helm.rs`:
  - Contains `{{ ... }}` → `TemplatePlaceholder { slug }` where slug replaces each `{{ ... }}` block with `__PLACEHOLDER_N__` (N incrementing per unique block)
  - Contains `@sha256:` → `Digested { image, digest }`
  - Contains `:<tag>` → `Tagged { image, tag }` (add `library/` prefix for unqualified names per Docker Hub convention)
  - Otherwise (bare `nginx` without tag) → `Tagged { image, tag: "latest" }` (Helm's default)
- [ ] T021 [US2] Implement `extract_image_refs_unrendered(chart_dir: &Path) -> Vec<ImageRef>` in `helm.rs` per contracts/extraction-pipeline.md §Phase B. Enumerate `templates/**/*.yaml`, `templates/**/*.yml`, `crds/*.yaml`, `crds/*.yml` via `safe_walk` (m114); for each file, read + apply IMAGE_REGEX line-by-line; classify each ref. Emit ONE `PackageDbEntry` per (image, source-file) tuple with distinct `source_path` values — do NOT collapse duplicates inside the reader. The resolver at `mikebom-common/src/resolution.rs:53` (`ResolutionEvidence.occurrences: Vec<FileOccurrence>`) is responsible for post-emission dedup by PURL + occurrence-merge; matches the existing convention for every other package-DB reader that emits multi-file evidence. Verify this end-to-end via T025's `dedup_collapses_same_ref_from_multiple_templates` fixture (same image referenced from 2 template files → 1 resolved component + 2 occurrence entries in the emitted CDX).
- [ ] T022 [US2] Extend `components_to_package_db_entries` in `helm.rs` to also emit `HelmComponent::Image` variants per data-model.md §5. Build PURL via kind-dispatch:
  - `Digested` → `pkg:oci/<image>@<digest>`
  - `Tagged` → `pkg:docker/<image>@<tag>`
  - `TemplatePlaceholder` → `pkg:generic/<slug>` + `mikebom:image-ref-unresolved = "true"` + `mikebom:image-ref-raw = "<raw>"` properties
- [ ] T023 [US2] Plumb the extraction outcome through `ScanDiagnostics` (at `package_db/mod.rs:308`) so the CDX + SPDX emitters can surface it at document scope. Steps:
  1. Add a new field `pub helm_extraction_mode: Option<HelmExtractionMode>` on `ScanDiagnostics` in `package_db/mod.rs` (default `None` — set only when a Helm chart was scanned).
  2. Define `pub enum HelmExtractionMode { Unrendered, Rendered }` in `package_db/mod.rs` (public so emitters can match on it).
  3. Update `helm::read` to accept `&mut ScanDiagnostics` and set `helm_extraction_mode = Some(HelmExtractionMode::Unrendered)` on the auto-detect success path. US3's T032 will conditionally set `Rendered`.
  4. Update `read_all` to thread `&mut ScanDiagnostics` into the `helm::read` call (T016).
  Reader-level `partial`/`full` marker emission is handled at document scope by the format emitters (T023a/b/c). Reader itself emits ONLY the state, not the annotation.
- [ ] T023a [US2] Extend `mikebom-cli/src/generate/cyclonedx/metadata.rs` to emit `mikebom:image-extraction-completeness = "partial"` OR `"full"` as a document-scope `metadata.properties[]` entry, driven by `ScanDiagnostics.helm_extraction_mode`. When `helm_extraction_mode == None`, emit nothing (FR-016 byte-identity: non-Helm scans see zero drift). Follows the C110/C111/C112/C118/C119 document-scope-annotation precedent already in place at `generate/cyclonedx/metadata.rs:75-240`.
- [ ] T023b [US2] Extend `mikebom-cli/src/generate/spdx/` document-scope `Annotation` emission to emit `mikebom:image-extraction-completeness` for SPDX 2.3. Uses the same `ScanDiagnostics.helm_extraction_mode` state. When `None`, emit nothing (byte-identity).
- [ ] T023c [US2] Extend `mikebom-cli/src/generate/spdx/v3_*.rs` (or the SPDX 3 annotation emitter) to emit `mikebom:image-extraction-completeness` as an SPDX 3 `Annotation` element at document scope. Same driver: `ScanDiagnostics.helm_extraction_mode`. When `None`, emit nothing.
- [ ] T024 [US2] Wire the image-ref extraction into `helm::read`'s top-level pipeline (replacing the T014 TODO). Sequence extends: (a) auto-detect gate; (b) `read_chart_at` (Phase A); (c) `extract_image_refs_unrendered` (Phase B unrendered); (d) build components + emit `partial` completeness annotation.
- [ ] T025 [US2] Append US2 integration tests to `mikebom-cli/tests/helm_reader.rs` per data-model.md §7.2:
  - `us2_templated_image_ref_emits_unresolved_property`
  - `us2_concrete_image_refs_emit_normal_purl`
  - `us2_crds_yaml_scanned_alongside_templates`
  - `us2_extraction_survives_go_template_broken_yaml` — template with a `{{ if .Values.enabled }}...{{ end }}` block spanning multiple YAML docs (YAML-invalid); assert `image:` refs are still extracted by the line-based regex (no "fall back" — line-based regex is the only path per FR-006 remediated)
  - `us2_document_scope_completeness_partial_annotation_present` — assert `mikebom:image-extraction-completeness = "partial"` on document scope
- [ ] T026 [US2] Verify byte-identity for the pre-m188 default path via new integration test `default_scan_without_chart_yaml_is_byte_identical` in `mikebom-cli/tests/helm_reader.rs`. Scan a directory with NO `Chart.yaml`; assert output is byte-identical to pre-m188 (no helm components, no `mikebom:image-extraction-completeness` annotation).

**Checkpoint**: US2 fully functional. Template image refs (resolved AND placeholder) now emit alongside chart deps. Default scan for non-Helm targets unchanged (SC-005 byte-identity).

---

## Phase 5: User Story 3 — Rendered-image extraction via `--helm-render` (Priority: P2)

**Goal**: Operators with `helm` installed get high-fidelity image-ref extraction with all placeholders resolved. Opt-in only.

**Independent Test**: spec.md §User Story 3 Acceptance 1-4. Verifiable via `mikebom-cli/tests/helm_reader.rs::us3_*` (4 tests). US3-specific tests gated behind `MIKEBOM_HELM_INTEGRATION=1` env var when real helm binary is required.

### Tests for User Story 3 (write FIRST) ⚠️

- [ ] T027 [P] [US3] Add unit test `helm_render_mode_off_never_invokes_helm` to `helm.rs::tests` — verifies via mock subprocess that `HelmRenderMode::Off` never spawns a `helm` process. FR-013 verification. Can use a helper that captures whether `Command::new("helm")` was constructed at all.

### Implementation for User Story 3

- [ ] T028 [US3] Add `HelmRenderError` enum + `Display` impl to `helm.rs` per contracts/extraction-pipeline.md §Phase C:
  - `BinaryNotFound` — `Command::spawn` returned `ENOENT`
  - `NonZeroExit { code: i32, stderr_head: String }` — subprocess exited non-zero (stderr limited to first 20 lines per FR-018)
  - `Timeout` — subprocess exceeded configured timeout
  - `IoError(std::io::Error)` — pipe read / process wait failure
- [ ] T029 [US3] Implement `resolve_render_timeout() -> u64` helper in `helm.rs`. Reads `MIKEBOM_HELM_RENDER_TIMEOUT_SECS` env var; parses as `u64`; defaults to `60` per FR-011.
- [ ] T030 [US3] Implement `run_helm_template(chart_dir: &Path, timeout_secs: u64) -> Result<Vec<u8>, HelmRenderError>` in `helm.rs`. Spawn `Command::new("helm").args(["template", chart_dir_str])` with piped stdout/stderr; enforce timeout via `std::thread::spawn` + `std::sync::mpsc::channel` (m055 `go_mod_graph.rs:81-158` pattern); kill child on timeout expire; return stdout bytes on success. Stderr head capture (first 20 lines) for `NonZeroExit` variant.
- [ ] T031 [US3] Implement `extract_image_refs_from_rendered(rendered_yaml: &[u8]) -> Vec<ImageRef>` in `helm.rs`. Runs the same IMAGE_REGEX from T019 on the rendered stdout (bytes → `str::from_utf8` → line scan). Post-rendered content should have zero `TemplatePlaceholder` refs (rendered values are concrete); if any remain, log DEBUG (they represent legitimate unbound values like `{{ include "..." }}` outputs).
- [ ] T032 [US3] Extend `helm::read` in `helm.rs` to dispatch on `render_mode` per contracts/extraction-pipeline.md §Phase C. On `HelmRenderMode::OptIn`: call `run_helm_template`. On success → run `extract_image_refs_from_rendered` + set `extraction_mode = Rendered` + emit `mikebom:image-extraction-completeness = "full"`. On any failure → WARN log with reason + fall back to `extract_image_refs_unrendered` + emit `partial`.
- [ ] T033 [US3] Wire `--helm-render` flag from `ScanArgs` (Phase 2 T005) into `read_all` call in `mikebom-cli/src/cli/scan_cmd.rs::execute`. Pass `HelmRenderMode::OptIn` when `args.helm_render`, else `HelmRenderMode::Off`.
- [ ] T034 [US3] Append US3 integration tests to `mikebom-cli/tests/helm_reader.rs` per data-model.md §7.2:
  - `us3_helm_render_missing_binary_falls_back` — set `PATH=""` in test environment or use a nonexistent binary path; assert WARN + fallback + scan succeeds
  - `us3_helm_render_timeout_falls_back` — write a shell script that sleeps 65s, place it as `helm` in a tempdir, prepend tempdir to PATH; assert timeout WARN + fallback
  - `us3_helm_render_env_var_timeout_override` — `MIKEBOM_HELM_RENDER_TIMEOUT_SECS=5` + slow stub; assert 5s timeout enforced
  - `us3_helm_render_success_produces_no_unresolved_markers` — GATED behind `MIKEBOM_HELM_INTEGRATION=1` env var (requires real helm binary). When enabled, real helm renders a synthesized chart; assert zero `mikebom:image-ref-unresolved` properties + `mikebom:image-extraction-completeness = "full"` annotation.

**Checkpoint**: US3 fully functional. `--helm-render` opt-in path delivers high-fidelity extraction with graceful fallback. All three P1/P2 stories complete.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: SC-007 verification, byte-identity guard, pre-PR gate, docs, PR.

- [ ] T035 [P] Update `docs/reference/sbom-format-mapping.md` with §Milestone 188 addendum documenting the two new `mikebom:*` properties:
  - `mikebom:image-ref-unresolved` — per-component boolean flag for templated image refs. Native-field audit: no CDX / SPDX 2.3 / SPDX 3 construct exists for "this image reference contains an unresolved template placeholder" — the placeholder is a mikebom-specific concept.
  - `mikebom:image-extraction-completeness` — document-scope enum (`partial` | `full`) surfacing whether extraction ran unrendered (partial) or rendered (full). Native-field audit: no format has a "coverage confidence" axis at document scope.
  Include a paragraph justifying each per Constitution Principle V per data-model.md §9.
- [ ] T036 Verify SC-007 zero-new-deps gate: capture post-m188 `cargo tree --workspace | wc -l` line count; diff against `specs/188-helm-chart-scanning/artifacts/cargo-tree-pre.txt` from T001 — MUST be identical. If differs, root-cause and remove transitive additions before opening the PR.
- [ ] T037 Verify FR-016 / SC-005 byte-identity for existing golden fixtures — since no fixture contains `Chart.yaml`, byte-identity is preserved by construction. Explicitly verify by running the CDX + SPDX regression suites: `cargo +stable test -p mikebom --test cdx_regression --test spdx_regression --test spdx3_regression --test pkg_alias_binding_us1 --test oci_pull_backward_compat --test optional_dep_classification`. Expected: all pass with zero drift on emitted output. Per memory `feedback_release_bump_regen_all_golden_tests` — the 6 golden-writing tests are the safety net.
- [ ] T038 Walker-audit CI-gate self-check per memory `feedback_walker_audit_local_check`: run `grep -rEn --include='*.rs' 'fn walk[_(]' mikebom-cli/src/scan_fs/` and confirm helm.rs does NOT introduce any new `fn walk_*` functions (use `safe_walk` for filesystem enumeration). If unavoidable naming, add `// walker-audit: false-positive — <reason>` comment on the preceding line.
- [ ] T039 Run `./scripts/pre-pr.sh` — both `cargo +stable clippy --workspace --all-targets -- -D warnings` and `cargo +stable test --workspace --no-fail-fast` MUST report `0 failed` on every target per CLAUDE.md. Capture the per-target `N passed; 0 failed` enumeration per memory `feedback_prepr_gate_full_output`.
- [ ] T040 Open PR from `188-helm-chart-scanning` → `main` with PR title `impl(188): Helm chart scanning — Chart.yaml + Chart.lock + charts/*.tgz + templates/*.yaml (#455)` and body linking to spec.md + plan.md + tasks.md. Close #455 in the PR body.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001. No dependencies.
- **Foundational (Phase 2)**: Depends on Setup. T002 first (creates file); T003 wires module; T004 adds types (needs T002); T005 CLI flags (parallelizable with T004); T006 build check last. **BLOCKS all US work.**
- **US1 (Phase 3)**: Depends on Foundational.
- **US2 (Phase 4)**: Depends on Foundational + US1 (shares `helm.rs`, `read_all` wire-up). Some US2 unit tests can be authored in parallel with US1 impl.
- **US3 (Phase 5)**: Depends on Foundational + US2 (US3 is the render-mode dispatch on top of US2's Phase B extraction).
- **Polish (Phase 6)**: Depends on US1 + US2 + US3.

### User Story Dependencies

- **US1 (P1 MVP)**: Foundational only. Independently shippable.
- **US2 (P1)**: Foundational + US1 for wire-up; conceptually independent (image-ref extraction doesn't need chart-dep enumeration to work), but they share `helm::read` orchestration.
- **US3 (P2)**: Foundational + US2's Phase B extractor (US3 shell-out replaces Phase B with a rendered variant).

### Within Each User Story

- **US1**: T007 tests FIRST → T008-T016 impl → T017 integration tests
- **US2**: T018 tests FIRST → T019-T024 impl → T025-T026 integration tests (+ byte-identity guard)
- **US3**: T027 test FIRST → T028-T033 impl → T034 integration tests

### Parallel Opportunities

- **Phase 2**: T004 (types) || T005 (CLI flags) once T002/T003 done.
- **Phase 3 US1 unit tests**: T007 is one task with 6 sub-tests; single contributor.
- **Phase 4 US2 unit tests**: T018 is one task with 9 sub-tests; single contributor.
- **Phase 6 Polish**: T035 (docs) || T036 (cargo tree) || T037 (byte-identity) || T038 (walker-audit).

---

## Parallel Example: Foundational Phase 2

```bash
# Sequential (module scaffolding):
Task: "T002 create helm.rs skeleton"

# Then parallel:
Task: "T003 wire mod.rs pub(super) mod helm"
Task: "T004 add all core types per data-model §1"
Task: "T005 add --helm-chart + --helm-render flags on ScanArgs"

# Then sequential:
Task: "T006 cargo build --all-targets — verify clean"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001) — 5 minutes.
2. Complete Phase 2: Foundational (T002-T006) — 1 hour. **CRITICAL: blocks all US work.**
3. Complete Phase 3: US1 (T007-T017) — 4-5 hours.
4. **STOP and VALIDATE**: `cargo +stable test --workspace --no-fail-fast` — verify US1 tests pass + no drift on m169 ipk regression or m186 oci regression.
5. Demoable: `mikebom sbom scan --path <chart-dir>` emits helm chart-dep components for the first time.

### Incremental Delivery

1. Setup + Foundational → foundation ready.
2. Add US1 → validate chart-dep extraction end-to-end. **Ship-able as standalone feature for the "helm dep audit" use case.**
3. Add US2 → validate image-ref extraction (unrendered). Complete Helm chart scanning coverage.
4. Add US3 → validate `--helm-render` opt-in path with real helm binary.
5. Polish (Phase 6) → SC-007 + byte-identity + walker-audit + pre-PR + PR.

### Sequential Team Strategy

One developer:
- Day 1: Phase 1 + Phase 2 + Phase 3 US1 (setup + scaffolding + chart-level).
- Day 2: Phase 4 US2 (template extraction).
- Day 3 morning: Phase 5 US3 (helm-render).
- Day 3 afternoon: Phase 6 (docs + pre-PR + PR).

---

## Notes

- All 40 tasks strictly follow the `- [ ] T### [P?] [Story?] Description with file path` format.
- Each task has a concrete file path + specific instruction.
- Tests before implementation per data-model.md §7 test-contract commitment.
- Commit after each task or logical group. Avoid amending; use new commits per CLAUDE.md.
- The `#[cfg(test)] mod tests { #[cfg_attr(test, allow(clippy::unwrap_used))] }` guard is REQUIRED in `helm.rs` — the crate root denies `clippy::unwrap_used` per Constitution Principle IV.
- Do NOT skip T037 (byte-identity check on the 6 golden-writing tests) — SC-005 gate; per memory `feedback_release_bump_regen_all_golden_tests`, easy to miss.
- Do NOT skip T038 (walker-audit self-check) — memory `feedback_walker_audit_local_check`; the CI gate isn't in `pre-pr.sh` so it only trips on CI otherwise.
- Do NOT skip T036 (cargo-tree zero-drift verification) — SC-007 zero-new-deps is a Constitution Principle I gate.
