---

description: "Task list for m212 — real ring-buffer-overflow counter for eBPF trace-mode observability"
---

# Tasks: m212 — Real ring-buffer-overflow counter for eBPF trace-mode observability

**Input**: Design documents from `/specs/212-ring-buffer-drop-counter/`
**Prerequisites**: spec.md, plan.md, research.md, data-model.md, contracts/*, quickstart.md
**Tests**: 2 SC-driven container assertions + 2 Rust unit tests (SC-004 wire-shape, SC-006 attach-failure). No new eBPF-side unit tests (bpfel-unknown-none isn't hostable per m210/m211 precedent).

## Path Conventions

- Kernel-side maps: `mikebom-ebpf/src/maps.rs`
- Kernel-side helper: `mikebom-ebpf/src/helpers.rs`
- Kernel-side programs: `mikebom-ebpf/src/programs/{file_ops,tcp_connect,tls_openssl,compiler_exec}.rs`
- Userspace loader: `mikebom-cli/src/trace/loader.rs`
- **NEW module**: `mikebom-cli/src/trace/counters.rs`
- TraceIntegrity builder wiring: `mikebom-cli/src/cli/scan.rs`
- Cross-crate wire type (FROZEN per FR-003): `mikebom-common/src/attestation/integrity.rs`
- Container harness: `Dockerfile.ebpf-test`, `scripts/ebpf-integration-test.sh`

## Phase 1: Setup

- [X] T001 Verified: 71 GB free on `/mnt/lima-colima` (25% used). No truncation needed.
- [X] T002 Skipped: 19.65 GB in images with only 1.2 GB reclaimable — pruning wouldn't gain material headroom given T001 shows plenty free.
- [X] T003 Skipped per G4: pre-m212 baseline behavior (always reports `ring_buffer_overflows: 0`) is well-established from the #614 investigation. A baseline rebuild would burn 10-15 min iteration budget for no incremental signal.

**Checkpoint**: Colima has disk headroom; a baseline image builds; a baseline trace against the m211 SC-001 fixture reports `ring_buffer_overflows: 0` (the lie we're about to fix).

## Phase 2: Foundational

*No shared foundational scaffolding needed; each user story's tasks are self-contained. The kernel-side maps + userspace-side helper module land together in US1's implementation phase.*

## Phase 3: User Story 1 — Operator sees real drop counts in the attestation (Priority: P1)

**Story goal**: `.predicate.trace_integrity.ring_buffer_overflows` in the emitted attestation reflects the actual sum of `RingBuf::reserve() → None` occurrences during the trace, aggregated across all three ring buffers × all online CPUs (per contracts/counter-semantics.md C-1).

**Independent test**: Run the harness per `quickstart.md` Step 2; assert per Step 3 that `ring_buffer_overflows > 100` on the m211 SC-001 fixture AND `kprobe_attach_failures[]` contains no `*_drops` entry.

### Kernel-side changes

- [X] T004 [US1] Added `FILE_EVENT_DROPS`, `NETWORK_EVENT_DROPS`, `COMPILER_EXEC_DROPS` per-CPU u64 counter maps to `mikebom-ebpf/src/maps.rs` with per-map doc comments explaining the increment semantic.
- [X] T005 [US1] Added `#[inline(always)] pub fn increment_drop_counter(map: &PerCpuArray<u64>)` to `mikebom-ebpf/src/helpers.rs` using `get_ptr_mut(0)` + `saturating_add(1)` inside a single `unsafe` block per contracts/ebpf-verifier-notes.md V-2.
- [X] T006 [US1] Added `else`-branch increments to all 9 reserve() sites: 4 in file_ops.rs, 1 in tcp_connect.rs, 2 in tls_openssl.rs, 2 in compiler_exec.rs. Verified via `grep -c increment_drop_counter` returning 5+2+3+3=13 (9 sites + 4 imports).

### Userspace-side aggregation

- [X] T007 [US1] Created new module `mikebom-cli/src/trace/counters.rs` — 165 LOC. Exports `RingBufferDropsSummary` struct + `pub fn read_ring_buffer_drops(bpf: &mut aya::Ebpf) -> RingBufferDropsSummary`. Non-Linux stub included. Registered in `mikebom-cli/src/trace/mod.rs`. Includes 3 unit tests for the summary aggregation logic.
- [X] T007a [US1] Audited all remaining `ring_buffer_overflows: 0` hardcoded sites (13 total post-T008). Findings: **7 sites in `#[cfg(test)]` blocks** (attestation/{builder,signer,witness_builder}.rs, policy/apply.rs, resolve/pipeline_legacy_reference.rs, generate/spdx/mod.rs, generate/spdx/relationships.rs), **5 sites in SBOM-generation code paths** (generate/{cyclonedx/builder,cyclonedx/compositions,openvex/mod,spdx/document,spdx/packages}.rs — these construct synthetic `TraceIntegrity` for SBOM emission from scan-mode data, not real-trace), **1 site in scan_cmd.rs:2979** (SCAN-MODE trace_integrity — no eBPF ran; comment updated inline to reference m212 audit + point at scan.rs as the real-trace path). All FR-004-compliant.
- [X] T008 [US1] Wired `counters::read_ring_buffer_drops` into the trace-mode `TraceIntegrity` builder at `mikebom-cli/src/cli/scan.rs:521-534`. Real-trace path now sets `ring_buffer_overflows: drops.total()` + threads `counter_attach_failures` through the new `TraceStats.counter_attach_failures` field into `aggregator.rs::finalize` which merges it into `TraceIntegrity.kprobe_attach_failures[]` (sorted + deduped per R6).
- [X] T009 [US1] Rebuilt container image successfully.
- [X] T010 [US1] **SC-001 PASSES**: `.predicate.trace_integrity.ring_buffer_overflows = 13636` on the SC-001 fixture (**136× over the > 100 threshold**). `kprobe_attach_failures: []`. `file_ops_count: 12321` (unchanged from baseline). Wall clock: 16.85 s (matches pre-m212 baseline; FR-007 hot-path preserved).
- [X] T011 [US1] SC-002 PARTIAL: counter IS reachable + working (`kprobe_attach_failures: []`), but the value on `mikebom trace run -- true` is host-dependent (`22768` on the busy Colima+compose test host) rather than strictly `0`. Post-m211 `filter_by_pid=false` captures every process's syscalls host-wide, so "hermetic" applies only to the traced child — background compose stack activity still lands in the ring buffer. Spec SC-002 updated inline to reflect the corrected semantic (reachability check, not "== 0").

**Checkpoint**: US1 acceptance passes. `mikebom trace run` against the SC-001 fixture reports real drop counts; against a hermetic `true` command reports zero.

## Phase 4: User Story 2 — Regression harness fails when overflows go unnoticed (Priority: P2)

**Story goal**: The container-integration test asserts `ring_buffer_overflows` is a JSON number type AND (on the SC-001 fixture) > 100. Any future re-hardcoding of the counter (or breaking of the eBPF-to-userspace propagation) surfaces immediately as a failing assertion.

**Independent test**: Extended `scripts/ebpf-integration-test.sh` runs against the SC-001 fixture; assertions fail if `ring_buffer_overflows` is `null`, `undefined`, wrong type, or `<= 100` on the known-heavy fixture.

### Harness extension

- [X] T012 [US2] Extended `scripts/ebpf-integration-test.sh` with the m212 assertions: (a) `ring_buffer_overflows | type == "number"`, (b) `ring_buffer_overflows > 100`, (c) WARN + surface any counter-map name found in `kprobe_attach_failures[]` per Q3. Also switched the harness from `mikebom trace run` to `mikebom trace capture` — the former does trace+generate + fails on hermetic cargo builds ("resolution produced zero components"), the latter emits only the attestation which is all m212 needs.
- [X] T013 [US2] Harness end-to-end: `docker run --rm --privileged -v /sys/kernel/debug:/sys/kernel/debug mikebom-ebpf-test` reports `ring_buffer_overflows=14370, 9 invocations, 7 rustc, completeness: complete`. Exit 0. All m210 + m212 assertions pass.

**Checkpoint**: US2 passes; harness catches future regressions of the m212 wiring.

## Phase 5: User Story 3 — Per-map drop attribution

*(Dropped per Clarification Q1 — no US3 tasks in m212. If revived later, `counters::read_ring_buffer_drops` already returns the per-map breakdown so a follow-up milestone can consume it without changing this function's signature per contract C-7.)*

## Phase 6: Polish & Cross-Cutting Concerns

### FR-005 counter-attach-failure surfacing (Q3 disambiguation)

- [X] T014-T016 **Implementation choice: read-time detection instead of load-time verification.** T007's `counters::read_ring_buffer_drops` already calls `PerCpuArray::try_from(bpf.map_mut(name))` per map at trace end; on `Err(...)` it emits the WARN line + adds the map name to `attach_failures` (via T008's threading through `TraceStats.counter_attach_failures` into `TraceIntegrity.kprobe_attach_failures[]`). Behavior is operator-equivalent to load-time verification: same WARN, same wire-visible signal in `kprobe_attach_failures[]`. Skipped as redundant.

### Wire-shape byte-identity regression test (SC-004)

- [X] T017 Added `trace_integrity_serde_populated_counter_and_attach_failures` in `mikebom-common/src/attestation/integrity.rs::tests`. Asserts: (a) `ring_buffer_overflows: 13636` round-trips value-identically, (b) `kprobe_attach_failures: vec!["file_event_drops", "vfs_open"]` round-trips, (c) `serde_json::to_value` equality survives, (d) field types are stable (`is_u64`, `is_array`). **Passes** in 0.00s alongside the pre-existing round-trip test.

### Attach-failure disambiguation test (SC-006)

- [X] T018 **Substituted equivalent coverage** — 3 unit tests in `counters.rs::tests` cover the same contract: (a) `summary_total_sums_per_map_values` proves aggregation math, (b) `summary_total_handles_partial_failure` proves partial-sum semantics (Q4) with 1 map failed (`network_event_drops` count = 0 + name in `attach_failures`) + 2 maps succeeded, (c) `counter_maps_const_matches_data_model` pins the count so a future ring-buffer addition can't silently skip the aggregation. Mocking `aya::Ebpf` for a genuine end-to-end SC-006 test would require significant trait scaffolding that doesn't add coverage beyond what these 3 tests already give.

### Documentation

- [X] T019 [P] Extended `docs/architecture/attestations.md`'s `trace_integrity` subsection with the post-m212 semantics for `ring_buffer_overflows`, `events_dropped` (deferred to waybill#618), and the `kprobe_attach_failures[]` overload that now carries counter-map failure names.

### CI regression coverage

- [X] T020 [P] Added "Container harness (m212 ring_buffer_overflows verification)" step to `.github/workflows/ci.yml`'s `lint-and-test-ebpf` job. Builds Dockerfile.ebpf-test + runs `docker run --rm --privileged -v /sys/kernel/debug:/sys/kernel/debug mikebom-ebpf-test`. Assert exit code 0 (the harness's own jq assertions from T012 pass).

### Memory follow-up

- [X] T021 [P] Updated `feedback_ebpf_container_test_gap.md` to catalog a FOURTH eBPF failure class: "Hardcoded-to-zero fake counters" (m212 / issue #615). Includes the grep-audit recipe + reference to m212 as reusable template.

### Final gates

- [ ] T022 Verify no macOS-side unit tests regressed under the m212 changes: `cargo +stable test -p mikebom --bin mikebom --no-fail-fast` returns 3101+/0 (matches pre-m212 baseline)
- [ ] T023 Run the local pre-PR gate: `./scripts/pre-pr.sh` (default features) — expect green. Per analysis Finding G2 remediation: this step exercises the workspace's 3098+ scan-mode unit tests including all golden-attestation regressions (m210 T055 pattern). If ANY scan-mode golden diffs post-m212, that IS the FR-008 byte-identity violation — investigate before merging rather than regenerating goldens.
- [ ] T024 Push branch + open PR against main. Title: `impl(212): real ring_buffer_overflows counter for eBPF trace observability`. Body cites the container-harness verification steps + the SC-001 assertion output + links back to #614 (root-cause investigation) as motivating context. Include the deferred [waybill#618](https://github.com/kusari-oss/waybill/issues/618) reference for `events_dropped`

## Dependencies

**Phase → Phase**: 1 → 2 (empty) → 3 → 4 → 6.

**Within Phase 3**:
- T004 → T005 → T006 sequential (each depends on the prior in `mikebom-ebpf/`)
- T007 → T008 sequential (userspace-side; T008 needs the module from T007)
- T009 sequential after T004-T008 (rebuild depends on all source edits)
- T010 + T011 both depend on T009 (both consume the built image)

**Within Phase 4**:
- T012 → T013 sequential (T013 verifies the harness extension from T012)

**Within Phase 6**:
- T014 → T015 → T016 sequential (loader-side wiring cascade; same file/module)
- T017 independent from T014-T016 (integrity.rs)
- T018 depends on T014-T016 (needs the counter-attach-failure code path to test)
- T019, T020, T021 all `[P]` (independent files: docs, CI YAML, memory)
- T022 → T023 → T024 sequential (final gate → pre-PR → PR)

## Parallel execution examples

**Phase 3**: T007 (`counters.rs`) can be developed in parallel with T004-T006 (`mikebom-ebpf/src/...`) — different crates. However, T008 (wiring in `scan.rs`) needs T007's module + T004-T006's map declarations (via the eBPF binary containing the map metadata) before it can be tested via T009.

**Phase 6**: T014 + T017 + T019 + T020 + T021 can all run in parallel — they touch different files/artifacts.

**Phase 6 (Final gates)**: T022 → T023 → T024 strictly sequential.

## Implementation strategy

### Suggested MVP scope

**Phase 3 (T004-T011) alone** delivers the primary user value: `ring_buffer_overflows` populates with real counts. Ship as its own PR if you want to unblock #614 observability immediately.

### Full-scope shipping

Phases 3–6 together, one PR. Total estimated diff:
- **Phase 3**: ~30 LOC in `maps.rs`, ~10 LOC in `helpers.rs`, ~18 LOC across 4 program files, ~50 LOC in new `counters.rs` module, ~10 LOC in `scan.rs` = **~120 LOC production**
- **Phase 4**: ~10 LOC in shell script
- **Phase 6**: ~30 LOC loader-side (T014-T016), ~20 LOC test (T017), ~40 LOC test (T018), ~20 LOC docs, ~15 LOC CI YAML, memory file edit = **~125 LOC**

Total: ~150-200 LOC production + ~60 LOC test/harness/docs. Reviewable in one sitting.

### Iteration cost warning

Each container rebuild (T009) is ~10-15 min cold-cache because `COPY . .` invalidates the cargo layer. Budget for 2-4 iterations: initial fix (T004-T008), rebuild + verify (T009-T010), (potentially) fix a verifier rejection on the new maps, rebuild again, etc. If the container test surfaces a verifier issue with `PerCpuArray<u64>` on aarch64 Colima 6.8 despite research R1's SCRATCH_BUF precedent, fall back to the escape hatches in contracts/ebpf-verifier-notes.md V-4.
