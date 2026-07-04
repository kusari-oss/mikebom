# Tasks: pnpm/yarn npm-alias syntax support (milestone 159)

**Input**: Design documents from `/specs/159-pnpm-yarn-alias/`
**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/), [quickstart.md](./quickstart.md)

**Tests**: Included per spec (SC-007 requires ≥12 unit tests + SC-008 requires integration test).

**Organization**: Tasks grouped by user story from spec.md (US1 P1, US2 P2, US3 P3).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies).
- **[Story]**: Which user story (US1 / US2 / US3). Setup + Foundational + Polish have no story tag.
- Every task includes an exact file path.

## Path Conventions

Rust workspace (`Cargo.toml` at repo root). Source under `mikebom-cli/src/`, tests under `mikebom-cli/tests/` (integration) or inline `#[cfg(test)] mod tests` (unit). All paths absolute from `/Users/mlieberman/Projects/mikebom/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Baseline capture + pre-159 snapshots for SC-001–SC-004 empirical + SC-003 byte-identity guard.

- [ ] T001 Verify baseline state: `git log -1 --oneline` on branch `159-pnpm-yarn-alias`; confirm milestone 158 (PR #497, commit `661a27d`) is at or near main tip; capture pre-159 `mikebom-cli/src/scan_fs/package_db/npm/` LOC counts for delta reporting at PR close.

- [ ] T002 Snapshot pre-159 CDX SBOMs for the 3 alias-relevant test repos into `/tmp/159-pre-snapshot/`:
  ```bash
  mkdir -p /tmp/159-pre-snapshot
  for repo in test-podman-desktop test-guac-visualizer test-rails; do
    ./target/release/mikebom sbom scan --path /tmp/kusari-audit/$repo \
      --no-deep-hash --format cyclonedx-json \
      --output cyclonedx-json=/tmp/159-pre-snapshot/${repo}.cdx.json
  done
  ```
  Drives SC-001 + SC-002 delta comparison at T028–T029 (spot-checks against milestone-157 audit findings).

- [ ] T003 Snapshot pre-159 golden fixtures via `git show HEAD:mikebom-cli/tests/fixtures/golden/<format>/<ecosystem>.<ext>` into `/tmp/159-pre-goldens/` for all 33 files (11 ecosystems × 3 formats). Drives SC-003 byte-identity guard at T027 (matches milestone 157/158 pattern).

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Build the `alias_mapping.rs` submodule (types + detection functions) — shared foundation for US1 + US2.

**⚠️ CRITICAL**: No user story task (T010+) can begin until T004–T009 land.

- [ ] T004 [P] Create `mikebom-cli/src/scan_fs/package_db/npm/alias_mapping.rs` with:
  - `pub(crate) struct AliasResolution` (5 fields per data-model.md).
  - `pub(crate) enum AliasEcosystem { Pnpm, YarnV1 }` + `impl AliasEcosystem { pub(crate) fn annotation_name(&self) -> &'static str }`.
  - `pub(crate) struct AliasedIdentity { aliased_name, aliased_version }`.
  - `pub(crate) type AliasMap = std::collections::HashMap<String, AliasedIdentity>`.
  - Wire the new module into `mikebom-cli/src/scan_fs/package_db/npm/mod.rs` by adding `pub mod alias_mapping;`.

- [ ] T005 [P] Add `pub(crate) fn detect_pnpm_alias(local_name: &str, raw_value: &str) -> Option<AliasResolution>` in `alias_mapping.rs` per contracts/pnpm-alias-grammar.md. Reuses the existing `pnpm_lock::parse_pnpm_key` helper — canonical name inequality against `local_name` is the alias signal. **Include the FR-010 warn-log emission on malformed values** (message: `"npm-alias parse failed (skipping entry)"`).

- [ ] T006 [P] Add `pub(crate) fn detect_yarn_v1_alias(key_line: &str, resolved_version: &str) -> Option<AliasResolution>` in `alias_mapping.rs` per contracts/yarn-alias-grammar.md. Splits the comma-joined key line, checks each spec for the `npm:` marker in the version-spec portion. **Include the FR-010 warn-log emission on malformed keys**.

- [ ] T007 [P] Add `pub(crate) fn rewrite_dep_names(dep_names: &[String], alias_map: &AliasMap) -> Vec<String>` in `alias_mapping.rs`. For each dep-name in `dep_names`, look up in `alias_map`; if found, replace with the aliased-name; else preserve byte-identically. Preserves ordering.

- [ ] T008 [US1] Unit tests (inline `#[cfg(test)] mod tests` in `alias_mapping.rs`) covering the 10 SC-007 contract-test cases (T1-T10 from contracts/pnpm-alias-grammar.md + contracts/yarn-alias-grammar.md):
  - T1: pnpm quoted-form alias with scoped aliased-name (real test-podman-desktop shape).
  - T2: pnpm unquoted-form alias.
  - T3: pnpm alias with scoped LOCAL name.
  - T4: pnpm no-alias case (must return `None`).
  - T5: pnpm malformed-value (must return `None` AND emit warn log — assert via `tracing_test` crate OR by capturing output).
  - T6: yarn v1 key with `npm:` alias, both scoped.
  - T7: yarn v1 key with `npm:` alias, unscoped local + scoped aliased.
  - T8: yarn v1 key with `npm:` alias, scoped local + unscoped aliased.
  - T9: yarn v1 no-alias case (must return `None`).
  - T10: yarn v1 malformed-key (must return `None` AND emit warn log).

- [ ] T009 [US1] Unit tests for `rewrite_dep_names`:
  - Empty `dep_names` → empty result.
  - No matches → byte-identical passthrough.
  - Partial matches → mixed rewrite + passthrough with preserved ordering.
  - Multi-alias case (2 different dep names both aliased).

- [ ] T009a [US1] Unit test in `mikebom-cli/src/scan_fs/package_db/npm/alias_mapping.rs` (inline `#[cfg(test)]`) for FR-007 byte-identity guard on `AliasResolution.local_name`. Synthesize 4 edge-shape aliases:
  - Unscoped-with-hyphens (`string-width-cjs`) — validates hyphen preservation.
  - Scoped-with-hyphens (`@scope/name-with-hyphens`) — validates scope `@` + hyphen preservation.
  - Underscores (`some_alias_name`) — npm-legal chars per registry spec.
  - Mixed-case (`ReactHelmetAsync`) — no lowercase normalization.
  For each, call `detect_pnpm_alias` (or `detect_yarn_v1_alias`) and assert `result.local_name` is byte-identical to the input string. No URL-encoding, no case normalization, no quote-stripping side effects. (Closes analyze finding A1 — FR-007 coverage.)

**Checkpoint**: At end of Phase 2, `alias_mapping.rs` compiles + all 12+ unit tests pass. No wire-up to parsers yet — that's US1.

---

## Phase 3: User Story 1 (P1) — SBOM consumer sees resolved-package identity for aliased deps

**Story Goal**: mikebom's CDX/SPDX 2.3/SPDX 3 output for repos containing pnpm-lock v9 or yarn v1 alias syntax emits components at the ALIASED canonical PURL and dep-graph edges pointing at the aliased identity — closing the false-negative gap that the milestone-157 Round-2 audit measured.

**Independent Test**: Scan a synthetic pnpm-lock fixture with an alias entry; assert the emitted CDX has the aliased-canonical component AND the depender's `dependsOn` includes the aliased PURL. Delivers value INDEPENDENTLY of US2 (annotations) and US3 (byte-identity).

- [ ] T010 [US1] Modify `mikebom-cli/src/scan_fs/package_db/npm/pnpm_lock.rs::collect_pnpm_dep_names` (at line ~46) to call `detect_pnpm_alias(local_name, raw_value)` on every dep entry. When an alias is detected: (a) return the ALIASED-NAME as the canonical dep-name (not the local-name); (b) populate a caller-visible `Vec<AliasResolution>` accumulator for downstream consumption. Change the function signature to `fn collect_pnpm_dep_names(entry_tbl, aliases: &mut Vec<AliasResolution>) -> Vec<String>` per data-model.md.

  **T010 caller-safety note (verified 2026-07-04 during /speckit-analyze)**: `grep -rn 'collect_pnpm_dep_names' mikebom-cli/src/` returns 3 hits — all inside `pnpm_lock.rs` (at lines 46 definition, 104 packages loop, 239 snapshots loop). No cross-file callers exist. The signature change is safe; all 3 call sites are updated as part of T010–T012.

- [ ] T011 [US1] Modify `mikebom-cli/src/scan_fs/package_db/npm/pnpm_lock.rs::build_snapshots_lookup` (at line ~85) to also call `detect_pnpm_alias` on each snapshot's inner-dep entries. Accumulate the resulting `Vec<AliasResolution>` into a `PerLockfileAliases` local; return it alongside the snapshots lookup so `parse_pnpm_lock` can consume the alias-map for edge rewriting.

- [ ] T012 [US1] Modify `mikebom-cli/src/scan_fs/package_db/npm/pnpm_lock.rs::parse_pnpm_lock` (at line ~126) to:
  1. Build `alias_map: AliasMap` from the accumulated `Vec<AliasResolution>` (T010 + T011 outputs) — key = local_name, value = AliasedIdentity{aliased_name, aliased_version}.
  2. For each `PackageDbEntry.depends: Vec<String>` about to be constructed, call `rewrite_dep_names(depends, &alias_map)` to re-target local-name refs to aliased identities per FR-005.
  3. Add every alias-resolved component's `local_name` to a NEW `extra_annotations` entry keyed `mikebom:pnpm-alias` (raw-string value = local_name per Q1) on the aliased `PackageDbEntry` per FR-006 — used by US2's emission tasks.
  4. Emit the FR-011 info-level tracing log at the end: `"npm-alias resolution completed"` with `lockfile_path`, `alias_count`, `alias_ecosystem = "pnpm"`.

- [ ] T013 [US1] Modify `mikebom-cli/src/scan_fs/package_db/npm/yarn_lock.rs::parse_v1` (at line ~139) analogously:
  1. Extend `v1_header_to_name` (at line ~229) OR add a parallel `v1_header_to_alias` helper that calls `detect_yarn_v1_alias(key_line, resolved_version)`.
  2. When alias detected: emit the `PackageDbEntry` under the aliased-name identity (not local-name), populate `extra_annotations` with `mikebom:yarn-alias = local_name`.
  3. Build `alias_map: AliasMap` from all detected aliases in the lockfile.
  4. Re-run `rewrite_dep_names` over every emitted `PackageDbEntry.depends` so local-name refs from OTHER entries resolve to the aliased identity (FR-005 yarn variant).
  5. Emit the FR-011 info-level tracing log at end: `"npm-alias resolution completed"` with `lockfile_path`, `alias_count`, `alias_ecosystem = "yarn"`.

- [ ] T014 [US1] Unit test in `mikebom-cli/src/scan_fs/package_db/npm/pnpm_lock.rs` (inline `#[cfg(test)]`) — synthesize a pnpm-lock v9 snapshot with the 4 real test-podman-desktop alias entries (`react-helmet-async`, `react-loadable`, `string-width-cjs`, `strip-ansi-cjs`). Parse and assert:
  - Emitted `PackageDbEntry` records include the 4 ALIASED-canonical identities (`@slorber/react-helmet-async@1.3.0`, `@docusaurus/react-loadable@6.0.0`, `string-width@4.2.3`, `strip-ansi@6.0.1`).
  - The 4 local-name identities do NOT appear as `PackageDbEntry.name`.
  - Each aliased entry's `extra_annotations["mikebom:pnpm-alias"]` value equals the correct local-name.
  - The depender entry's `depends` list contains the ALIASED canonical name (not the local-name).

- [ ] T015 [US1] Unit test in `mikebom-cli/src/scan_fs/package_db/npm/yarn_lock.rs` (inline `#[cfg(test)]`) — synthesize a yarn v1 lockfile with the test-guac-visualizer alias entry (`"@cosmograph/cosmos@^1.1.1", "@cosmograph/cosmos@npm:@cosmos.gl/graph":`). Parse and assert:
  - Emitted `PackageDbEntry` has `name = "@cosmos.gl/graph"` and `version = "2.6.4"`.
  - No `PackageDbEntry` has `name = "@cosmograph/cosmos"`.
  - `extra_annotations["mikebom:yarn-alias"] = "@cosmograph/cosmos"`.
  - A different entry (`"hosted-server-mgmt@^0.5.0"`) with `dependencies: {"@cosmograph/cosmos": "^1.1.1"}` gets its `depends` rewritten so it lists `@cosmos.gl/graph` (not `@cosmograph/cosmos`).

- [ ] T015a [US1] Unit test in `mikebom-cli/src/scan_fs/package_db/npm/pnpm_lock.rs` for FR-012 multi-alias emission (analyze finding A2). Synthesize a pnpm-lock v9 where TWO different snapshot entries alias to the same resolved package under DIFFERENT local-names — e.g. workspace peer A depends on `helmet-shim: '@slorber/react-helmet-async@1.3.0'` AND workspace peer B depends on `react-helmet-async: '@slorber/react-helmet-async@1.3.0'`. Parse and assert:
  - Exactly ONE `PackageDbEntry` emitted for `@slorber/react-helmet-async@1.3.0` (dedup by canonical identity).
  - The entry carries BOTH local-name values in `extra_annotations` — implementation may use `Value::Array` OR two BTreeMap entries with numeric-suffixed keys. Inspect existing multi-value `extra_annotations` patterns before choosing the shape at impl time (T010–T012). Test asserts BOTH values are recoverable via a single lookup path.
  - The downstream CDX emission at `builder.rs:1185-1205` produces TWO `properties[]` entries with `name = "mikebom:pnpm-alias"` and distinct `value` fields (verified end-to-end via T026 integration test on a similarly-shaped synthesized fixture).

  **Note**: `extra_annotations: BTreeMap<String, Value>` is single-value per key. This test may surface a data-model constraint that needs a Phase-3 resolution — using `Value::Array` (comma-joined-decoded) OR a per-emitter loop over an array-shaped value. Either resolution is a T010–T012 impl decision documented in the checklist notes at close.

- [ ] T016 [US1] Unit test — pnpm no-alias regression guard. Synthesize a pnpm-lock v9 with ZERO alias entries (just plain `foo@1.0.0: bar@2.0.0` where both are canonical). Assert:
  - No `extra_annotations["mikebom:pnpm-alias"]` on any entry.
  - `PackageDbEntry.depends` byte-identical to pre-159 behavior.
  - No FR-011 info log emitted (alias_count == 0 → skip the log).

- [ ] T017 [US1] Unit test — yarn no-alias regression guard. Same pattern as T016 but for yarn v1.

**Checkpoint**: End of US1. The alias-resolution + edge-rewriting works end-to-end at the parser layer. The 4 pnpm-side + 4 yarn-side unit tests pass. At this checkpoint, the `mikebom:*-alias` marker sits in `extra_annotations` on each aliased `PackageDbEntry`, ready for US2 to project into wire-format annotations.

---

## Phase 4: User Story 2 (P2) — SBOM consumer sees alias-provenance annotation for auditability

**Story Goal**: Every alias-resolved component in every emitted SBOM (CDX / SPDX 2.3 / SPDX 3) carries the `mikebom:pnpm-alias` OR `mikebom:yarn-alias` component-scope annotation per FR-006/007/012.

**Independent Test**: On any of the 3 test repos, `jq '.components[] | select(.properties[]?.name == "mikebom:pnpm-alias" or .properties[]?.name == "mikebom:yarn-alias")' sbom.cdx.json` returns the expected N alias-resolved components.

- [ ] T018 [P] [US2] Register parity catalog rows C106 (`mikebom:pnpm-alias`) + C107 (`mikebom:yarn-alias`) in `mikebom-cli/src/parity/extractors/mod.rs`. Insert after milestone-158's C104/C105 entries (line ~452). Both use `Directionality::SymmetricEqual`, `order_sensitive: false`. Add the imports `c106_cdx, c106_spdx23, c106_spdx3, c107_cdx, c107_spdx23, c107_spdx3` to the extractor-name lists at the top of the file (be careful NOT to insert them into the ParityExtractor struct-literal line — milestone-158 hit that bug).

- [ ] T019 [P] [US2] Add the 2 CDX extractors in `mikebom-cli/src/parity/extractors/cdx.rs` using the `cdx_anno!` macro with `scope = component`:
  ```rust
  cdx_anno!(c106_cdx, "mikebom:pnpm-alias", component);
  cdx_anno!(c107_cdx, "mikebom:yarn-alias", component);
  ```
  Insert after milestone-158's C104/C105 macro invocations.

- [ ] T020 [P] [US2] Add the 2 SPDX 2.3 extractors in `mikebom-cli/src/parity/extractors/spdx2.rs`:
  ```rust
  spdx23_anno!(c106_spdx23, "mikebom:pnpm-alias", component);
  spdx23_anno!(c107_spdx23, "mikebom:yarn-alias", component);
  ```

- [ ] T021 [P] [US2] Add the 2 SPDX 3 extractors in `mikebom-cli/src/parity/extractors/spdx3.rs`:
  ```rust
  spdx3_anno!(c106_spdx3, "mikebom:pnpm-alias", component);
  spdx3_anno!(c107_spdx3, "mikebom:yarn-alias", component);
  ```

- [ ] T022 [US2] Add C106 + C107 rows to `docs/reference/sbom-format-mapping.md` after milestone-158's C105 row (insert before the `## Section D — Evidence` boundary). Follow the milestone-158 row format: `| C10X | \`mikebom:XXX-alias\` | <description> | Annotation on Package with MikebomAnnotationCommentV1 envelope | Annotation element with same envelope | **KEEP-NO-NATIVE**. <rejection audit>. |`. Reference the milestone-158 C104/C105 rows as the shape template.

- [ ] T023 [US2] CDX component-scope emission wire-up: **verified during /speckit-analyze 2026-07-04** that `builder.rs:1185-1205` iterates `component.extra_annotations` and emits per-key `properties[]` entries with the value pass-through pattern `Value::String(s) => s.clone()` at line 1198 — bare-string values are emitted directly, no envelope wrap. Consequence: `mikebom:pnpm-alias` and `mikebom:yarn-alias` entries flow through the existing loop and land in the emitted CDX `properties[]` with the correct raw-string shape. No new emission code required for CDX. Task action: read-only verification via `grep -A25 'for (key, value) in &component.extra_annotations' mikebom-cli/src/generate/cyclonedx/builder.rs` and confirm the pass-through pattern still lives there (regression-detector for a future refactor).

- [ ] T024 [US2] SPDX 2.3 per-package emission wire-up at `mikebom-cli/src/generate/spdx/annotations.rs`: verify the milestone-127+ pattern iterates `extra_annotations` and emits per-key `Annotation` with `comment = "<key>=<value>"`. This is the same established shape as the milestone-158 `mikebom:demoted-from-main-module` annotation. Read-only verification task; no code changes needed.

- [ ] T025 [US2] SPDX 3 per-package emission wire-up at `mikebom-cli/src/generate/spdx/v3_annotations.rs`: verify the milestone-127+ pattern iterates `extra_annotations` and emits per-key `Annotation` elements with `statement = "<key>=<value>"`. Read-only verification task; no code changes needed.

- [ ] T026 [US2] SC-008 integration test at `mikebom-cli/tests/npm_alias_resolution.rs`. Synthesize a mixed pnpm+yarn workspace via `tempfile::tempdir()` + `std::fs::write` (mirrors milestone-157/158 pattern):
  ```
  <tmp>/
    package.json         (workspace root)
    pnpm-lock.yaml       (with 2 aliases: react-helmet-async + string-width-cjs)
    packages/
      web/
        package.json
        yarn.lock        (with 1 alias: @cosmograph/cosmos → @cosmos.gl/graph)
  ```
  Invoke the release binary; parse the emitted CDX + SPDX 2.3 + SPDX 3 outputs; assert:
  - All 3 alias-resolved canonical PURLs (`@slorber/react-helmet-async@1.3.0`, `string-width@4.2.3`, `@cosmos.gl/graph@2.6.4`) exist as components in each format.
  - Local-name PURLs (`react-helmet-async@`, `string-width-cjs@4.2.3`, `@cosmograph/cosmos@2.6.4`) do NOT exist.
  - Each aliased component carries the correct `mikebom:pnpm-alias` OR `mikebom:yarn-alias` annotation in each format.
  - Milestone-071 parity check passes symmetrically across the 3 formats via `cargo test parity_symmetric`.

**Checkpoint**: End of US2. Every alias-resolved component in every emitted SBOM carries the correct annotation. Parity catalog gate green.

---

## Phase 5: User Story 3 (P3) — Non-alias repos byte-identical to pre-159

**Story Goal**: Milestone-090 non-alias goldens (11 ecosystems × 3 formats = 33 goldens) byte-identical vs pre-159. Zero fabricated annotations, zero PURL rewrites, zero regression bytes.

**Independent Test**: `diff /tmp/159-pre-goldens/<eco>.cdx.json mikebom-cli/tests/fixtures/golden/cyclonedx/<eco>.cdx.json | grep -cE '^[<>]' == 0` for every one of the 33 goldens.

- [ ] T027 [US3] SC-003 byte-identity guard: run `./scripts/regen-goldens.sh` from repo root. Confirm the 33 goldens under `mikebom-cli/tests/fixtures/golden/` are byte-identical to `/tmp/159-pre-goldens/`. If ANY golden shows diff bytes, STOP and investigate — this signals an unintended byte change per SC-003.

**Checkpoint**: End of US3. All 33 non-alias goldens byte-identical.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Empirical verification, CHANGELOG, pre-PR gate, and PR closure.

- [ ] T028a SC-004 universal-presence check on test-podman-desktop (analyze finding A3). Parse the emitted CDX and enumerate all alias-resolved components via `jq`. Assert the COUNT of `mikebom:pnpm-alias` values equals the milestone-157-audit-measured count of 6:
  ```bash
  ACTUAL=$(jq '[.components[] | select(.properties[]? | .name == "mikebom:pnpm-alias") | .properties[] | select(.name == "mikebom:pnpm-alias") | .value] | length' /tmp/159-test-podman-desktop.cdx.json)
  EXPECTED=6
  test "$ACTUAL" -eq "$EXPECTED" || echo "SC-004 mismatch: expected $EXPECTED alias annotations, got $ACTUAL"
  ```
  If ACTUAL differs from EXPECTED, log the actual count and revise SC-004's implicit-expected number inline per milestone-156/157/158 empirical-revision pattern.

- [ ] T028 SC-001 empirical: scan `test-podman-desktop` with the built binary. For the 4 real alias entries, verify via `jq -e` (per quickstart.md Scenario 1) that:
  - `@docusaurus/core@3.10.1` dependsOn includes `@slorber/react-helmet-async@1.3.0`.
  - `@docusaurus/core@3.10.1` dependsOn includes `@docusaurus/react-loadable@6.0.0`.
  - `@isaacs/cliui@8.0.2` dependsOn includes `string-width@4.2.3` AND `strip-ansi@6.0.1`.
  - No local-name PURLs (`pkg:npm/react-helmet-async@`, `pkg:npm/string-width-cjs@4.2.3`, etc.) appear in `.components[]`.
  Record actual counts.

- [ ] T029 SC-002 empirical: scan `test-guac-visualizer` + `test-rails`. Verify per quickstart.md Scenarios 2 + 3:
  - test-guac-visualizer: `pkg:npm/%40cosmos.gl/graph@2.6.4` exists as a component; `pkg:npm/%40cosmograph/cosmos@2.6.4` does NOT; the aliased component carries `mikebom:yarn-alias = "@cosmograph/cosmos"`.
  - test-rails: 3 yarn alias-affected entries (`string-width-cjs`, `strip-ansi-cjs`, `wrap-ansi-cjs`) all resolve to their aliased canonicals.

- [ ] T030 SC-005 BFS reachability check on test-podman-desktop per quickstart.md Scenario 6. Target ≥708 reachable npm components (+10 from milestone-158's 698 baseline). Record actual measurement; if the actual number differs, revise SC-005 inline per milestone-156/157/158 pattern.

- [ ] T031 SC-006 pre-PR gate: run `./scripts/pre-pr.sh`. MUST pass `cargo +stable clippy --workspace --all-targets -- -D warnings` AND `cargo +stable test --workspace`. Per the milestone-155 `feedback_prepr_gate_bails_on_first_failure` memory: use `--no-fail-fast` if invoking `cargo test` manually.

- [ ] T032 SC-009 CHANGELOG entry: add a new `[Unreleased]` section entry to `CHANGELOG.md` following the milestone-158 template + research §R9 shape. Include:
  - Bug summary + Q1/Q2 clarification bullets.
  - SC-001 / SC-002 empirical numbers from T028 + T029.
  - Consumer jq recipe (per R9) for filtering by `mikebom:pnpm-alias` / `mikebom:yarn-alias`.
  - Wire-format-cleanliness note (parity catalog rows C106/C107 + no new Cargo deps).

- [ ] T033 SC-011 issue closure: verify the `impl(159)` commit message includes `closes #493`. Update the milestone-159 requirements checklist at `specs/159-pnpm-yarn-alias/checklists/requirements.md` with implementation-completion notes (mirrors milestone-157's T015 + milestone-158's T039 pattern): measured SC-001 + SC-002 spot-check results from T028 + T029; SC-005 reachability delta from T030; SC-003 diff-line counts (should be 0 across all 33) from T027; any surprises encountered during impl.

- [ ] T034 Commit `impl(159)`: `alias_mapping.rs` + `pnpm_lock.rs` + `yarn_lock.rs` + parity catalog files + `docs/reference/sbom-format-mapping.md` + integration test. Commit message: `impl(159): pnpm/yarn npm-alias syntax support in dep-graph edges (closes #493)`. Follow milestone-157/158's HEREDOC + `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>` convention.

- [ ] T035 Commit `docs(159)`: `CHANGELOG.md` + `specs/159-pnpm-yarn-alias/checklists/requirements.md` (updated with impl notes).

- [ ] T036 Push to `fork` remote: `git push -u fork 159-pnpm-yarn-alias`. Then open PR against `kusari-oss/mikebom` `main` with title `impl(159): pnpm/yarn npm-alias syntax support (#493)` and body per the milestone-158 PR body template: summary, T028+T029 empirical table (10 dropped-edges → 10 correctly-resolved edges), test plan checklist, consumer jq recipe, wire-format cleanliness note.

**Checkpoint**: PR open. CI green. Ready for review + merge.

---

## Dependencies

**Story completion order**: US1 (P1) delivers the primary bug fix (aliased-canonical PURLs + edge rewriting). US2 (P2) requires US1's `extra_annotations` marker to project into wire-format annotations. US3 (P3) is a validation step run AFTER US1 + US2 land.

**Task ordering constraints**:

- **Setup (T001–T003)** → blocks nothing except SC-001/002/003 empirical + byte-identity in Polish.
- **Foundational (T004–T009)** → blocks ALL of US1 + US2 + US3.
- **US1 (T010–T017)** → produces the `PackageDbEntry.extra_annotations` marker that US2 projects.
- **US2 (T018–T026)** → requires US1's marker to be present (parity catalog registration + emission wire-up have no compile-time dependency on US1, but the SC-008 integration test at T026 assumes US1 has landed).
- **US3 (T027)** → requires US1 + US2 landed so goldens regenerate with the full end-state.
- **Polish (T028–T036)** → requires everything above landed.

**Parallel-execution opportunities**:

- T004 + T005 + T006 + T007 (four different function-add operations in the same file `alias_mapping.rs`, but the entire file is being authored in one shot so sequential is simpler).
- T014 + T015 + T016 + T017 (four independent unit tests in two different files: T014 + T016 in `pnpm_lock.rs`; T015 + T017 in `yarn_lock.rs`).
- T018 + T019 + T020 + T021 (four different parity-catalog files — mod.rs + cdx.rs + spdx2.rs + spdx3.rs; can be authored in parallel).
- T028 + T029 (two independent empirical scans on different test repos).

**Independent testing per US**:

- **US1**: T014–T017 unit tests + T028 (test-podman-desktop) empirical are all runnable without T018+ (US2's parity catalog rows aren't required for the primary alias-resolution + edge-rewriting behavior).
- **US2**: T026 integration test requires US1 landed (the `extra_annotations` marker must be present for the emission wire-up to project into wire-format annotations).
- **US3**: T027 requires US1 + US2 landed — the goldens reflect the full end-state.

## Implementation Strategy

**MVP scope**: US1 alone would deliver the primary bug fix (aliased-canonical PURLs + edge rewriting). But without US2 there's no audit-trail annotation. Ship US1 + US2 + US3 in ONE PR (matches milestone 158's shipping model). Total task count: 36 tasks.

**Post-merge follow-ups** (out of scope for milestone 159, per spec):

- Issue #494 (Go workspace-mode false edges) — next milestone.
- Issue #495 (Go transitive coverage) — next milestone.
- Issue #496 (Ruby built-in gems) — next milestone.
- Issue #498 (phantom empty-version edges) — the biggest remaining test-podman-desktop reachability blocker.
- **Package-lock (npm) alias syntax** — the `package-lock.json` format has a similar aliasing shape (`"depname": {"name": "aliased-name", ...}`). Out of scope for 159 but adjacent; likely a milestone 160 candidate.

## Format Validation

Every task above has:

- ✅ Checkbox `- [ ]`
- ✅ Task ID `T001`–`T036`
- ✅ `[P]` marker where parallelizable
- ✅ `[US1]` / `[US2]` / `[US3]` labels on the correct story-phase tasks; no story label on Setup / Foundational / Polish tasks.
- ✅ Exact file paths in every description (either an existing file to modify OR a new file to create).

39 tasks total (post-analyze remediation). 8 tasks parallelizable ([P] marker or in independent files per phase). 10 SC-001 through SC-011 verification steps embedded in Polish (T028a adds the SC-004 universal-presence check).

## Task counts per phase (post-analyze remediation 2026-07-04)

- Phase 1 (Setup): 3 tasks (T001–T003).
- Phase 2 (Foundational): 7 tasks (T004–T009 + T009a — closes analyze A1 by adding FR-007 byte-identity test).
- Phase 3 (US1, P1): 9 tasks (T010–T017 + T015a — closes analyze A2 by adding FR-012 multi-alias test).
- Phase 4 (US2, P2): 9 tasks (T018–T026 — T023/T024/T025 reworded per A5; no new tasks).
- Phase 5 (US3, P3): 1 task (T027).
- Phase 6 (Polish): 10 tasks (T028–T036 + T028a — closes analyze A3 by adding SC-004 universal-presence check).

**Total**: 39 tasks (net +3 from A1/A2/A3 additions).
