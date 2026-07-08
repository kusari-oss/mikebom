---

description: "Tasks for milestone 173 — opt-in Go cache warming (`--warm-go-cache=<off|per-workspace>` + concurrency knob + advisory log + two new doc-scope annotations)"
---

# Tasks: Opt-in Go cache warming for accurate transitive graphs

**Input**: Design documents from `/specs/173-warm-go-cache/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/{annotation-wire-shapes,cli-surface}.md, quickstart.md

**Tests**: 1 new integration test file at `mikebom-cli/tests/warm_go_cache.rs` covering US1 (before/after C117 delta), US2 (advisory-log fires/suppresses), US3 (graceful degradation on malformed workspace + `go` binary absent).

**Organization**: 3 user stories from spec.md (US1 P1 MVP + US2 P2 + US3 P2) + setup + foundational + polish. ~35 tasks total. All LLM-executable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm branch state + verify current pre-fix output shape (no C118/C119 annotation).

- [X] T001 Confirm current branch is `173-warm-go-cache` via `git rev-parse --abbrev-ref HEAD`. If not, `git checkout 173-warm-go-cache`.

- [X] T002 Verify pre-173 state — annotations `mikebom:go-cache-warming-mode` AND `mikebom:go-cache-warming-failed` MUST NOT already be emitted. Verified via source-grep (`grep -rn "go-cache-warming" mikebom-cli/src/ mikebom-common/src/` → 0 hits) + golden-grep (`grep -rl "go-cache-warming" mikebom-cli/tests/fixtures/golden/` → 0 hits). Baseline confirmed.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Create the warmer module + full plumbing chain so US1/US2/US3 tasks have data to route through. BLOCKS Phase 3, 4, and 5.

- [X] T003 Create new module file `mikebom-cli/src/scan_fs/package_db/golang/warm_cache.rs` with the type definitions from data-model.md Entities 1–3: `CacheWarmingMode` enum (3 variants: `Off`, `PerWorkspace`, `OfflineInhibited`) with `as_wire_str(&self) -> &'static str`; `WarmingFailureReason` closed enum (6 variants: `GoBinaryAbsent`, `SpawnFailed`, `Timeout`, `SubcommandFailed`, `ParseError`, `BudgetExhausted`) with `#[serde(rename_all = "kebab-case")]` AND an `as_wire_str(&self) -> &'static str` method returning the kebab-case wire strings (`"go-binary-absent"`, `"spawn-failed"`, `"timeout"`, `"subcommand-failed"`, `"parse-error"`, `"budget-exhausted"`) — the method is used by T011's FR-005 warn log to render the reason class as a bare string in the log fields; the serde derive still handles the JSON emission for C119; the two produce the same strings; a unit test `warming_failure_reason_wire_str_matches_serde` in the same `#[cfg(test)] mod tests` block verifies parity by round-tripping each variant through both paths; `WorkspaceFailure` struct — declare fields in the order `pub reason: WarmingFailureReason,` first, `pub workspace: String,` second, so serde's default emission produces alphabetical JSON (`{"reason":..., "workspace":...}`) matching contracts/annotation-wire-shapes.md byte-identity requirement; `CacheWarmingResult` struct `{mode: CacheWarmingMode, failures: Vec<WorkspaceFailure>}`. All derive `Debug`, `Clone`, `serde::Serialize`. Add file-level module doc comment referencing the m055/m091 subprocess+concurrency precedent per research §R2 + §R3.

- [X] T004 Implement `effective_concurrency(raw: u32) -> usize` helper in `warm_cache.rs` per data-model.md Entity 4. Logic: `raw == 0` → `min(std::thread::available_parallelism().map(NonZeroUsize::get).unwrap_or(4), 8)`; `1..=32` → `raw as usize`; `>32` → clamp to `32` and emit `tracing::warn!(requested = raw, "--warm-go-cache-concurrency clamped to 32 (per FR-014)")`. Unit test `effective_concurrency_bounds` in the same file's `#[cfg(test)] mod tests` block — verifies auto/passthrough/clamp behavior.

- [X] T005 Add `pub mod warm_cache;` line to `mikebom-cli/src/scan_fs/package_db/golang/mod.rs` so the new module compiles into the crate. Also re-export the public types: `pub use warm_cache::{CacheWarmingMode, CacheWarmingResult, WarmingFailureReason, WorkspaceFailure};` (mirrors the m160 GoTransitiveCoverage re-export style at the golang module boundary).

- [X] T006 Add `pub cache_warming: Option<CacheWarmingResult>` field to `GoScanSignals` struct in `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs` (~line 1384, sibling of the m172 `gosum_fallback_count` field). Add doc comment matching data-model.md Entity 5's `GoScanSignals` shape.

- [X] T007 Add `pub go_cache_warming: Option<golang::CacheWarmingResult>` field to `ScanDiagnostics` struct in `mikebom-cli/src/scan_fs/package_db/mod.rs` (~line 348, sibling of the m172 `gosum_fallback_count` field). Add doc comment. In `read_all` (~line 1460 area, where `diagnostics.go_transitive_coverage = go_signals.go_transitive_coverage` fires), also assign `diagnostics.go_cache_warming = go_signals.cache_warming.clone();` — no Option-gate needed here (the None case is idempotent).

- [X] T008 Add `pub go_cache_warming: Option<golang::graph_resolver::...CacheWarmingResult>` field to `ScanResult` struct in `mikebom-cli/src/scan_fs/mod.rs` (~line 98 area, sibling of the m172 `go_transitive_fallback_count` field — verify the exact use-path; probably `crate::scan_fs::package_db::golang::CacheWarmingResult`). Around line 270 area (sibling of m172's `go_transitive_fallback_count` local), declare `let mut go_cache_warming: Option<CacheWarmingResult> = None;`. Around line 307 area (sibling of m172's `go_transitive_fallback_count` assignment), also assign `go_cache_warming = scan_result.diagnostics.go_cache_warming.clone();`. At line ~825 area (the `Ok(ScanResult { ... })` return literal), add the field.

- [X] T009 Add `pub go_cache_warming: Option<&'a crate::scan_fs::package_db::golang::CacheWarmingResult>` field to `ScanArtifacts` struct in `mikebom-cli/src/generate/mod.rs` (~line 78 area, sibling of the m172 `go_transitive_fallback_count` field). Add doc comment.

- [X] T010 Wire `ScanArtifacts.go_cache_warming` at every construction site. Enumerate via `grep -rn "go_transitive_fallback_count:" mikebom-cli/src/generate/ mikebom-cli/src/cli/`. Real sites (value derives from scan/artifacts): `mikebom-cli/src/cli/scan_cmd.rs:2614` area (`go_cache_warming: scan_result.go_cache_warming.as_ref()`); `mikebom-cli/src/generate/spdx/v3_document.rs:99` area (`go_cache_warming: scan.go_cache_warming`); `mikebom-cli/src/generate/spdx/document.rs:462 + :490` (BOTH sites use `artifacts.go_cache_warming`). Test-harness stub sites (all `None`): `openvex/mod.rs:246`, `spdx/mod.rs:388`, `spdx/packages.rs:724`, `spdx/relationships.rs:345`, `spdx/document.rs:1167`, `cyclonedx/builder.rs:139` — mirror the m172 T009 pattern. Let the compiler drive; run `cargo +stable build -p mikebom --all-targets 2>&1 | grep -E "^\s+-->"` to enumerate any missed sites.

**Checkpoint**: T003–T010 build clean via `cargo +stable build -p mikebom --all-targets`. Phase 3+ can start.

---

## Phase 3: User Story 1 — Monorepo warms in one flag (Priority: P1) 🎯 MVP

**Goal**: `--warm-go-cache=per-workspace` runs `go mod download` in every discovered Go workspace before the resolver ladder, flipping the C117 fallback count from positive → 0 on a cold-cache monorepo scan. C118 `mikebom:go-cache-warming-mode` doc-scope annotation emitted across all 3 formats.

**Independent Test**: quickstart.md Path A — baseline scan (no flag) shows positive C117; scan with `--warm-go-cache=per-workspace` shows `"0"` + `mikebom:go-cache-warming-mode = "per-workspace"`.

- [X] T011 [US1] Implement `warm_workspaces(workspace_paths: &[PathBuf], mode: CacheWarmingMode, concurrency: usize, per_workspace_timeout: Duration, overall_budget: Duration) -> CacheWarmingResult` in `mikebom-cli/src/scan_fs/package_db/golang/warm_cache.rs`. Structure (per research §R2 + §R3):
  - Early-return `CacheWarmingResult { mode: CacheWarmingMode::Off, failures: Vec::new() }` when `mode == Off` (belt-and-braces; caller should not invoke).
  - Early-return the same with mode `OfflineInhibited` when the caller passed that (caller responsibility to set correctly).
  - Probe `go version` once before spawning workers — if it fails with `NotFound`, return a result whose `failures` names EVERY workspace with `WarmingFailureReason::GoBinaryAbsent` (per US3 Acceptance Scenario 2 — behavior converges to no-warming for all workspaces).
  - Worker pool per m055/m091 `parallel_fetch` pattern at `graph_resolver.rs:1001-1050`: `let workers = concurrency.max(1).min(workspace_paths.len())`; `mpsc::sync_channel::<PathBuf>(workers)` job queue; `mpsc::channel()` result collector; `std::thread::spawn` × workers.
  - Each worker pulls a workspace path, invokes `run_go_mod_download(path, per_workspace_timeout) -> Result<(), WarmingFailureReason>` (helper function mirroring `run_go_mod_graph` from `go_mod_graph.rs:81-158`), sends `(path, result)` back on the result channel.
  - Feeder thread iterates `workspace_paths`, drops each onto `job_tx`, checks the overall wall-clock budget between iterations — when exhausted, drop the tx (workers see channel-closed and exit) and mark all remaining workspaces `BudgetExhausted`.
  - Collector loop drains the result channel until all workers exit, records failures into a `Vec<WorkspaceFailure>` sorted alphabetically by workspace path (per data-model.md Entity 3's byte-identity requirement).
  - **FR-005 per-failure warn log**: for each recorded failure, emit exactly one `tracing::warn!(workspace = %failure.workspace, reason = failure.reason.as_wire_str(), "go mod download failed for workspace")` line at the point of recording (BEFORE aggregation into the result vec). Reason class rendered via a new `as_wire_str(&self) -> &'static str` helper on `WarmingFailureReason` producing the kebab-case wire values (`"timeout"`, `"go-binary-absent"`, etc.). This log fires in addition to (not instead of) the C119 doc-scope aggregation — the log is real-time operator feedback while warming runs; the C119 annotation is post-scan audit trail.
  - Return `CacheWarmingResult { mode, failures }`.
  - Add `run_go_mod_download` helper mirroring `run_go_mod_graph` — `Command::new("go").args(["mod", "download"]).current_dir(path)` + `rx.recv_timeout(timeout)` → maps to `Ok(())` / `Err(Timeout)` / `Err(SubcommandFailed)` / `Err(SpawnFailed)`.

- [X] T012 [US1] Add `WarmGoCacheMode` clap `ValueEnum` + the `warm_go_cache: WarmGoCacheMode` field to the scan-command `Args` struct in `mikebom-cli/src/cli/scan_cmd.rs`. Attributes per contracts/cli-surface.md: `#[arg(long, value_enum, default_value = "off", require_equals = true, num_args = 1)]`. Add a `From<WarmGoCacheMode> for crate::scan_fs::package_db::golang::CacheWarmingMode` impl so the CLI-side enum lifts into the domain enum (keeps the two boundaries independent — CLI enum only has `Off`/`PerWorkspace`; internal enum adds the `OfflineInhibited` variant that's set later by the pipeline).

- [X] T013 [US1] Add `warm_go_cache_concurrency: u32` field to the scan-command `Args` struct in `mikebom-cli/src/cli/scan_cmd.rs`. Attributes: `#[arg(long, default_value_t = 4)]`. Doc comment per data-model.md Entity 7. Value is a raw `u32` at parse time; runtime resolution via `warm_cache::effective_concurrency` (from T004) happens at the pipeline entry, not at parse.

- [X] T014 [US1] Thread the two new flag values from the CLI `Args` struct into the scan pipeline: extend the internal config/plumbing (mirror how `--offline` is threaded — search for `offline_mode` in `scan_cmd.rs` and add sibling parameters `warm_go_cache_mode: CacheWarmingMode` and `warm_go_cache_concurrency: usize`). Compute the effective mode with the offline-inhibited rule: `let effective_mode = if offline_mode && cli_mode == CacheWarmingMode::PerWorkspace { CacheWarmingMode::OfflineInhibited } else { cli_mode.into() };`. Emit exactly one `tracing::warn!` line if `effective_mode == OfflineInhibited` naming the conflict (per FR-003). Pass both values into `scan_fs::scan_path` (extend its signature to accept them — mirrors the m172 pattern).

- [X] T015 [US1] Integrate the warmer call in `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs` immediately before the existing `graph_map.coverage()` aggregation at ~line 1670. Logic:
  ```rust
  // Milestone 173: pre-resolver cache warming (opt-in).
  // Skipped when mode is `Off` or `OfflineInhibited`; runs one
  // `go mod download` per discovered workspace at bounded concurrency.
  if matches!(cache_warming_mode, CacheWarmingMode::PerWorkspace) {
      let concurrency = warm_cache::effective_concurrency(concurrency_raw);
      let per_workspace_timeout = Duration::from_secs(60);  // per research §R4
      let overall_budget = Duration::from_secs(300);         // per research §R4
      let workspace_paths: Vec<PathBuf> = /* collected from the same walker that feeds the resolver */;
      signals.cache_warming = Some(warm_cache::warm_workspaces(
          &workspace_paths,
          cache_warming_mode,
          concurrency,
          per_workspace_timeout,
          overall_budget,
      ));
  } else if matches!(cache_warming_mode, CacheWarmingMode::OfflineInhibited | CacheWarmingMode::Off) {
      // Emit the mode annotation carrier so C118 fires with the correct value.
      signals.cache_warming = Some(CacheWarmingResult { mode: cache_warming_mode, failures: Vec::new() });
  }
  ```
  Guard the entire block on `!workspace_paths.is_empty()` — nothing to warm when there are zero Go workspaces (matches FR-002).

- [X] T016 [P] [US1] Emit C118 in CDX at `mikebom-cli/src/generate/cyclonedx/metadata.rs`. Add function parameter `go_cache_warming: Option<&CacheWarmingResult>` (mirror m172's C117 parameter shape). Immediately BEFORE the existing C110/C111 emission block (alphabetic-sort places `cache-warming-*` before `transitive-*`), add:
  ```rust
  if let Some(cw) = go_cache_warming {
      properties.push(json!({
          "name": "mikebom:go-cache-warming-mode",
          "value": cw.mode.as_wire_str(),
      }));
  }
  ```
  Thread the new param from the caller (`cyclonedx/builder.rs` → `mod.rs` `.with_go_cache_warming(scan.go_cache_warming.cloned())` mirroring m172 `.with_go_transitive_fallback_count(...)`).

- [X] T017 [P] [US1] Emit C118 in SPDX 2.3 at `mikebom-cli/src/generate/spdx/annotations.rs`. Add block BEFORE the existing C110 emission using the envelope helper. Preserve the C118-before-C110 alphabetic ordering.

- [X] T018 [P] [US1] Emit C118 in SPDX 3 at `mikebom-cli/src/generate/spdx/v3_annotations.rs`. Add block BEFORE the existing C110 emission using the typed-Annotation graph-element push helper.

- [X] T019 [US1] Add C118 parity extractor row + helpers. In `mikebom-cli/src/parity/extractors/mod.rs`, add `ParityExtractor { row_id: "C118", label: "mikebom:go-cache-warming-mode", cdx: c118_cdx, spdx23: c118_spdx23, spdx3: c118_spdx3, directional: Directionality::SymmetricEqual, order_sensitive: false }` in alphabetical position BEFORE the m160 C110 row (since `cache-warming` sorts before `transitive-coverage`). Add `c118_cdx`/`c118_spdx23`/`c118_spdx3` to the three import lists. Add helpers in `parity/extractors/{cdx,spdx2,spdx3}.rs` using the standard `cdx_anno!` / `spdx23_anno!` / `spdx3_anno!` macros with `document` scope.

- [X] T020 [US1] Regenerate the 3 Go goldens with the new C118 annotation emitted at value `"off"` (goldens run in default-flag mode): `MIKEBOM_UPDATE_CDX_GOLDENS=1 cargo +stable test -p mikebom --test cdx_regression 2>&1 | tail -3 && MIKEBOM_UPDATE_SPDX_GOLDENS=1 cargo +stable test -p mikebom --test spdx_regression 2>&1 | tail -3 && MIKEBOM_UPDATE_SPDX3_GOLDENS=1 cargo +stable test -p mikebom --test spdx3_regression 2>&1 | tail -3`. Expected diff: +4 lines (CDX), +5-6 (SPDX 2.3), +7-8 (SPDX 3) per Go golden, ALL with `value = "off"`. Non-Go goldens MUST show zero delta per SC-004: `git diff main --stat -- 'mikebom-cli/tests/fixtures/golden/**' | grep -v golang` MUST return empty.

- [X] T021 [US1] Add integration test at `mikebom-cli/tests/warm_go_cache.rs` covering US1 scenarios. Structure mirrors m172's `tests/go_fallback_count.rs`:
  - `mod common; use common::{bin, normalize::apply_fake_home_env};`
  - Helper `scan(path, warm_mode: Option<&str>, offline: bool) -> serde_json::Value` — invokes `Command::new(bin())` with fake HOME + optional `--warm-go-cache=<mode>` + optional `--offline`.
  - `t021_us1_healthy_go_scan_default_mode_off` — scan the m055 `simple-module` fixture without the flag; assert `mikebom:go-cache-warming-mode = "off"`.
  - `t021_us1_explicit_off_matches_default` — scan with explicit `--warm-go-cache=off`; assert same value and byte-identity of the emitted C118 vs the default case.
  - `t021_us1_offline_inhibited_mode` — scan with both `--offline` and `--warm-go-cache=per-workspace`; assert `mikebom:go-cache-warming-mode = "offline-inhibited"` AND stderr contains a warn-level conflict-log line.
  - `t021_us1_per_workspace_mode_annotation_present` — scan with `--warm-go-cache=per-workspace` (without offline; test requires network — skip via `#[ignore]` if no network, run in CI with proxy fixture); assert value is `"per-workspace"`. **SC-005 wall-clock guard**: at the top of this test, capture `let started = std::time::Instant::now();`; after the scan completes and assertions run, add `assert!(started.elapsed() < std::time::Duration::from_secs(60), "SC-005: warming test-fixture scan must complete in <60s, elapsed={:?}", started.elapsed());`. Regression signal if fixture size or warming path ever regresses past the ceiling.
  - Non-Go scan test: `t021_us1_non_go_scan_omits_c118_annotation` — scan a pure Rust/npm fixture with `--warm-go-cache=per-workspace`; assert C118 is absent (FR-011 gate).

**Checkpoint**: US1 delivered — every Go SBOM emits C118 across CDX/SPDX 2.3/SPDX 3; the `--warm-go-cache=per-workspace` flag flips the C117 count when network + go binary present.

---

## Phase 4: User Story 2 — Advisory log (Priority: P2)

**Goal**: When the operator's env is degraded (C117 > 0 in non-offline mode) AND they didn't explicitly set `--warm-go-cache`, exactly one INFO-level log line names the flag + manual remediation.

**Independent Test**: quickstart.md Path B — grep -c on captured stderr equals `1` in the default case; equals `0` on explicit `--warm-go-cache=off`.

- [X] T022 [US2] Add `AdvisoryContext` struct + `should_advise(&self) -> bool` method in `mikebom-cli/src/cli/scan_cmd.rs` per data-model.md Entity 8. Struct fields: `fallback_count: Option<usize>`, `warm_flag_was_default: bool`, `offline: bool`, `scan_has_go_components: bool`. `should_advise` returns true iff ALL four predicates hold (FR-004 + FR-009).

- [X] T023 [US2] Detect the "flag was explicitly set vs default" state using clap `ArgMatches::value_source("warm_go_cache")`. If the caller uses `Args`-derive (which we do), obtain the match struct via `Args::group_id()` isn't reliable; instead thread the `bool` through the CLI `Args` struct as `warm_go_cache_was_default: bool` populated pre-scan. Precise approach: in `mikebom-cli/src/main.rs` or the scan-command handler, after `Cli::parse()`, call `cli.warm_go_cache_was_default = matches.get_one_source::<WarmGoCacheMode>("warm_go_cache") == Some(&ValueSource::DefaultValue)` — the exact API is `matches!(matches.value_source("warm_go_cache"), Some(clap::parser::ValueSource::DefaultValue))`. Add a `pub fn warm_go_cache_was_default(matches: &ArgMatches) -> bool` helper to make this reusable.

- [X] T024 [US2] Emit the advisory log line at the emission-tail site in `mikebom-cli/src/cli/scan_cmd.rs`. Immediately after the SBOM write succeeds (before final `tracing::info!` scan-complete line), compute:
  ```rust
  let ctx = AdvisoryContext {
      fallback_count: scan_result.go_transitive_fallback_count,
      warm_flag_was_default,
      offline: offline_mode,
      scan_has_go_components: components.iter().any(|c| c.purl.as_ref().is_some_and(|p| p.starts_with("pkg:golang/"))),
  };
  if ctx.should_advise() {
      tracing::info!(
          "mikebom:go-transitive-fallback-count > 0 detected. Prime the cache with --warm-go-cache=per-workspace or 'go mod download' per workspace before scanning."
      );
  }
  ```
  The literal string MUST match the substring in contracts/cli-surface.md verbatim (SC-002 grep).

- [X] T025 [US2] Add integration tests in `mikebom-cli/tests/warm_go_cache.rs`:
  - `t025_us2_advisory_fires_once_in_default_case` — scan a Go fixture that produces C117>0 with default flag settings, non-offline; grep stderr for the stable substring; assert exactly `1` match.
  - `t025_us2_advisory_suppressed_on_explicit_off` — same scan with `--warm-go-cache=off`; assert `0` matches.
  - `t025_us2_advisory_suppressed_in_offline_mode` — same scan with `--offline`; assert `0` matches (FR-003 + Acceptance Scenario 3).
  - `t025_us2_advisory_suppressed_on_non_go_scan` — scan a pure Rust fixture; assert `0` matches (FR-009).

**Checkpoint**: US2 delivered — advisory log fires exactly in the four-predicate-satisfied case and is suppressed otherwise.

---

## Phase 5: User Story 3 — Graceful degradation + C119 (Priority: P2)

**Goal**: Cache-warming failures (malformed `go.mod`, subprocess exits non-zero, timeout, absent `go` binary) NEVER abort the scan; instead they surface via the C119 `mikebom:go-cache-warming-failed` doc-scope annotation naming affected workspaces + reason class.

**Independent Test**: quickstart.md Path C — 3-workspace fixture with workspace 2 malformed; scan exits 0; C119 names ws2 with `parse-error` (or `subcommand-failed` — depends on how `go` classifies the failure).

- [X] T026 [P] [US3] Emit C119 in CDX at `mikebom-cli/src/generate/cyclonedx/metadata.rs`. Add block immediately BEFORE the C118 block (alphabetic: `failed` sorts before `mode`):
  ```rust
  if let Some(cw) = go_cache_warming {
      if !cw.failures.is_empty() {
          properties.push(json!({
              "name": "mikebom:go-cache-warming-failed",
              "value": serde_json::to_string(&cw.failures).unwrap_or_default(),
          }));
      }
  }
  ```
  Value is the JSON-encoded array string per contracts/annotation-wire-shapes.md.

- [X] T027 [P] [US3] Emit C119 in SPDX 2.3 at `mikebom-cli/src/generate/spdx/annotations.rs`. Same conditional gate; envelope-wrap the JSON-encoded array string.

- [X] T028 [P] [US3] Emit C119 in SPDX 3 at `mikebom-cli/src/generate/spdx/v3_annotations.rs`. Same conditional gate; typed `Annotation` graph element with the JSON-string in `statement`.

- [X] T029 [US3] Add C119 parity extractor row + helpers. In `mikebom-cli/src/parity/extractors/mod.rs`, add `ParityExtractor { row_id: "C119", label: "mikebom:go-cache-warming-failed", cdx: c119_cdx, spdx23: c119_spdx23, spdx3: c119_spdx3, directional: Directionality::SymmetricEqual, order_sensitive: false }` immediately BEFORE the C118 row (alphabetic). Add `c119_*` to imports. Add three helpers in `parity/extractors/{cdx,spdx2,spdx3}.rs` (mirroring T019).

- [X] T030 [US3] Add integration tests in `mikebom-cli/tests/warm_go_cache.rs`:
  - `t030_us3_malformed_workspace_records_failure` — synthesize a 3-workspace fixture with `ws2/go.mod` containing garbage; scan with `--warm-go-cache=per-workspace`; assert exit 0 AND `mikebom:go-cache-warming-failed` value contains an entry `{"reason": "subcommand-failed", "workspace": "ws2"}` (the `go mod download` command exits non-zero for malformed go.mod).
  - `t030_us3_go_binary_absent_degrades` — scan with `env -i PATH=/nonexistent` so the `go` binary can't be found; with `--warm-go-cache=per-workspace`; assert exit 0 AND C119 value contains `{"reason": "go-binary-absent", ...}` for every workspace (converges to no-warming per FR-005).
  - `t030_us3_c119_absent_on_healthy_scan` — scan the m055 `simple-module` fixture with warming on; assert C119 is ABSENT (no failures → annotation not emitted, per FR-007 gate). This is the byte-identity gate that healthy scans don't get a spurious empty-array C119.

**Checkpoint**: US3 delivered — failures never abort the scan; C119 annotation surfaces failure details across all 3 formats.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Docs enrichment, walker-audit, pre-PR gate, diff-scope verification.

- [X] T031 [P] Add rows C118 + C119 to `docs/reference/sbom-format-mapping.md`. Insert BEFORE the m160 C110 row (alphabetic). Use exact row content from data-model.md Entity 9 — each row includes the Constitution Principle V "KEEP-NO-NATIVE" audit citing rejected alternatives (CDX `component.evidence.identity[].confidence` is per-component, not doc-scope; SPDX `Package.filesAnalyzed` is per-package with unrelated semantics; SPDX 3 `CreationInfo.comment` loses machine-parseability).

- [X] T032 [P] Enrich `docs/reference/reading-a-mikebom-sbom.md` with a new subsection for the m173 cache-warming signals. Position it alongside the m172 C117 section — either as its own top-level `#### mikebom:go-cache-warming-mode + mikebom:go-cache-warming-failed` block or as an enrichment of the existing C117 section explaining "priming the cache before scanning." Include:
  1. The 5-step ladder recap (link back to m172 C117 section).
  2. What `go mod download` does + why it's the appropriate tool (mikebom does NOT run `tidy`/`build`/`test`/`generate` per Constitution).
  3. The three C118 values (`off`, `per-workspace`, `offline-inhibited`) + their operator meanings.
  4. C119 wire shape + jq recipe (from contracts/annotation-wire-shapes.md Recipe 2).
  5. The advisory log line stable substring + when it fires.
  6. Cross-reference to the depth-coverage table row for C118 (add row) + annotation-key alphabetical list entries + milestone-changes table entry.
  Target length: ~80 lines added.

- [X] T033 Run walker-audit CI-gate locally per memory `feedback_walker_audit_local_check`. m173 touches no walker code — the audit should PASS with zero drift. Reproduce the CI check locally: `ALLOWLIST="mikebom-cli/src/scan_fs/walk.audit-allowlist.txt" && STRIP_LINE_NUMBERS='s/^\([^:]*\):[0-9]*:/\1:/' && EXPECTED=$(grep -v '^#' "$ALLOWLIST" | grep -v '^$' | /usr/bin/sed "$STRIP_LINE_NUMBERS" | LC_ALL=C sort -u) && LIVE=$(...) && diff -u <(printf '%s\n' "$EXPECTED") <(printf '%s\n' "$LIVE") && echo "OK"`. Expected: PASS.

- [X] T034 Run `./scripts/pre-pr.sh` per SC-004 + SC-005. Verify green — `>>> all pre-PR checks passed.` Enumerate any `^---- .+ stdout ----` failure lines before claiming green per memory `feedback_prepr_gate_bails_on_first_failure` + `feedback_prepr_gate_full_output`. This exercises the m071 parity gate (T019 + T029 correctness), the golden regression checks, the C118/C119 emission tests, US1/US2/US3 integration tests.

- [X] T035 Diff the working tree against `main` per SC-004. Expected paths changed:
  - `mikebom-cli/src/scan_fs/package_db/golang/warm_cache.rs` (T003–T004, T011 — new file)
  - `mikebom-cli/src/scan_fs/package_db/golang/mod.rs` (T005)
  - `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs` (T006, T015)
  - `mikebom-cli/src/scan_fs/package_db/mod.rs` (T007)
  - `mikebom-cli/src/scan_fs/mod.rs` (T008)
  - `mikebom-cli/src/generate/mod.rs` (T009)
  - `mikebom-cli/src/generate/cyclonedx/{metadata,builder,mod}.rs` (T010, T016, T026)
  - `mikebom-cli/src/generate/spdx/{annotations,v3_annotations,document,v3_document,mod,packages,relationships}.rs` (T010, T017, T018, T027, T028)
  - `mikebom-cli/src/generate/openvex/mod.rs` (T010 stub)
  - `mikebom-cli/src/cli/scan_cmd.rs` (T012–T014, T022–T024)
  - `mikebom-cli/src/parity/extractors/{cdx,mod,spdx2,spdx3}.rs` (T019 + T029)
  - `mikebom-cli/tests/fixtures/golden/{cyclonedx,spdx-2.3,spdx-3}/golang.*` (T020 — exactly 3 files)
  - `mikebom-cli/tests/warm_go_cache.rs` (T021, T025, T030 — new file)
  - `docs/reference/sbom-format-mapping.md` (T031)
  - `docs/reference/reading-a-mikebom-sbom.md` (T032)
  - `CLAUDE.md` (auto-updated by /speckit-plan)
  - `specs/173-warm-go-cache/**` (new)
  Verify SC-004 explicitly: `git diff main --stat -- 'mikebom-cli/tests/fixtures/golden/**' | grep -v golang` MUST return empty (no non-Go goldens changed).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: T001–T002. No prerequisites.
- **Foundational (Phase 2)**: T003–T010 — depends on Phase 1. **BLOCKS Phase 3/4/5**. All user stories need the warmer types + plumbing chain to exist.
- **User Story 1 (Phase 3, P1 MVP)**: T011–T021 — depends on Phase 2 complete.
- **User Story 2 (Phase 4, P2)**: T022–T025 — depends on Phase 3 T014 (CLI flag machinery) + T020 (goldens so tests can diff).
- **User Story 3 (Phase 5, P2)**: T026–T030 — depends on Phase 3 T011 (warmer runs) + T015 (integration point wires signals).
- **Polish (Phase 6)**: T031–T035 — depends on Phases 3+4+5 complete.

### Within User Story 1

Order matters because plumbing depends on struct fields existing:

1. **T011** → **T012**+**T013** sequential (both edit scan_cmd.rs) → **T014** (threads them together)
2. **T015** depends on T011 + T014 (needs the warmer function + the piped-through mode value)
3. **T016** + **T017** + **T018** parallel [P] (three different files, all format emitters — depend on T015 having populated `signals.cache_warming`)
4. **T019** depends on T016–T018 (needs the emission code to exist)
5. **T020** golden regen depends on T016–T019
6. **T021** integration test depends on T020 (goldens don't fight the test)

### Within User Story 2

Sequential:
1. **T022** → **T023** → **T024** (each depends on the previous struct + logic existing in scan_cmd.rs)
2. **T025** integration test depends on all three

### Within User Story 3

1. **T026** + **T027** + **T028** parallel [P] (three format emitters, all depend on the T011 + T015 machinery from US1)
2. **T029** depends on T026–T028 (needs emission code)
3. **T030** integration test depends on T029

### Cross-Story Parallel Windows

- After Phase 3 US1 completes, Phase 4 US2 (T022–T025) and Phase 5 US3 (T026–T030) can run fully in parallel — they touch disjoint code paths.
- Phase 6 polish tasks T031 + T032 are [P] parallel (different docs files); T033 + T035 are [P] parallel (different concerns); T034 sequential last.

### Parallel Opportunities Summary

- **Phase 3 US1**: T016+T017+T018 parallel [P]; T014 sequential prereq.
- **Phase 4 + Phase 5**: fully parallel after Phase 3 (touch disjoint files).
- **Phase 5 US3**: T026+T027+T028 parallel [P] after Phase 3 T015.
- **Phase 6**: T031+T032 parallel [P]; T033+T035 parallel [P].

### Independent Test Criteria per User Story

- **US1**: quickstart.md Path A — before/after C117 delta; C118 value = `"per-workspace"` post-flag.
- **US2**: quickstart.md Path B — `grep -c` on stderr equals `1` in default case, `0` on explicit-off.
- **US3**: quickstart.md Path C — scan exits 0 despite malformed workspace; C119 names the failing workspace with the correct reason class.

### MVP Scope

**Suggested MVP**: US1 alone (T003–T021 + T033–T035). This delivers the primary monorepo ergonomics story from the milestone. US2 (advisory) and US3 (graceful degradation) are P2 polish that ships alongside for the full milestone but could technically be separate PRs.

**Recommended**: land all three stories in one PR. Estimated ~350 lines source + ~80 lines docs + 3 golden updates + 1 integration test file with ~8 test functions. Splitting adds process overhead disproportionate to size (matches the m172 rationale).
