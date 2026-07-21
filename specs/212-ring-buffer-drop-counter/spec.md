# Feature Specification: Real ring-buffer-overflow counter for eBPF trace-mode observability

**Feature Branch**: `212-ring-buffer-drop-counter`
**Created**: 2026-07-20
**Status**: Draft
**Input**: User description: "615" (GitHub issue #615)

## Clarifications

### Session 2026-07-20

- Q: US3 scope — does the per-map breakdown add a new wire field, or stay summary-only? → A: Drop US3 entirely. Retain aggregate `ring_buffer_overflows` only. FR-003 byte-identity stays strict. File US3 as a follow-up milestone if operators demand per-map attribution later.
- Q: Is `events_dropped` in scope for m212, or a separate follow-up? → A: Defer to follow-up. m212 ships only the kernel-side `ring_buffer_overflows` counter. `events_dropped` requires a different mechanism (userspace post-trace ringbuf backlog probe) that's cleaner in its own milestone. Filed as [kusari-oss/waybill#618](https://github.com/kusari-oss/waybill/issues/618) (repo was renamed from `mikebom` → `waybill`; redirect works transparently for the local git remote).
- Q: How does m212 signal "counter unavailable" vs "counter says zero"? → A: Reuse the existing `TraceIntegrity.kprobe_attach_failures[]` array. When a counter map fails to attach, add its name to that array. Zero new wire fields; consumers check the existing array to disambiguate.
- Q: On partial counter-map attach failure (some maps attach, others don't), does `ring_buffer_overflows` report `0` or a partial sum? → A: Partial sum. `read_percpu_sum` returns `0` for maps that failed to attach (via `unwrap_or(0)`); the successful maps contribute their real counts. The failing map's name still lands in `kprobe_attach_failures[]` per Q3 so consumers can tell the aggregate is a floor, not a total. Post-analysis-report Finding I1 refinement.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Operator sees real drop counts in the attestation (Priority: P1)

An operator runs `mikebom trace run -- <build-command>` on a busy Linux host. When the trace completes, they open the emitted attestation JSON and read `predicate.trace_integrity.ring_buffer_overflows` + `predicate.trace_integrity.events_dropped` to gauge trace fidelity. Today those two fields are hardcoded to zero everywhere — regardless of whether the trace actually captured every event or silently dropped 80% of them. Post-fix, both fields carry real u64 counts derived from kernel-side per-CPU counter maps that increment whenever an eBPF program's `RingBuf::reserve()` returns `None`. The operator can then decide whether to trust the trace or re-run with different tuning.

**Why this priority**: The observability prerequisite for every other trace-mode improvement. Issue #614's investigation showed a real drop-rate problem (~80% of rustc events lost when cargo's fingerprint spam fills the buffer) — but the misleading "overflows: 0" in the attestation hid it. Every future trace-mode fix depends on this signal being trustworthy so operators can distinguish "the fix worked" from "we're still silently dropping events." Without it, we're flying blind.

**Independent Test**: Run `mikebom trace run --attestation-format mikebom-v1 --attestation-output /tmp/out.json -- <any-heavy-command-that-fires-many-syscalls>` on a Linux host with `CAP_BPF`. Parse `/tmp/out.json`. Assert `.predicate.trace_integrity.ring_buffer_overflows >= 0` AND that the value increases when the traced workload is intentionally heavier (verify via a follow-up trace with a longer/busier command).

**Acceptance Scenarios**:

1. **Given** a hermetic short-lived traced command (e.g. `echo hello`), **When** the trace completes, **Then** the emitted attestation's `.predicate.trace_integrity.ring_buffer_overflows` = `0` and `.events_dropped` = `0` — the fields exist AND reflect the truth that no drops occurred.
2. **Given** a heavy traced command that intentionally saturates the ring buffer (e.g. the milestone-210 SC-001 `cargo build --release` fixture that saturated the buffer with cargo fingerprint spam per issue #614), **When** the trace completes, **Then** the emitted attestation's `.predicate.trace_integrity.ring_buffer_overflows > 0` — reflecting the drops that actually occurred.
3. **Given** the fix is applied, **When** `mikebom sbom scan` is used (no trace mode, no eBPF), **Then** the emitted CDX/SPDX SBOMs are byte-identical to pre-fix output — this feature only touches the trace-mode emission path.

---

### User Story 2 — Regression harness fails when overflows go unnoticed (Priority: P2)

A CI harness or human reviewer runs a trace against the m211 SC-001 fixture in the container harness. Post-fix, the container test harness assertions include a check that `ring_buffer_overflows` is populated with a real value (not always zero). If a future refactor accidentally re-hardcodes the counter or breaks the eBPF-to-userspace propagation, the container harness surfaces it via a failing assertion — instead of silently returning to the pre-fix state.

**Why this priority**: Guards against re-regression. The hardcoded-to-zero pattern that persisted in the codebase pre-fix (at 6 emission sites) shows the bug is easy to reintroduce; a specific regression test catches it.

**Independent Test**: Extend `scripts/ebpf-integration-test.sh` to include a jq assertion. Assert that `.predicate.trace_integrity.ring_buffer_overflows` is a `number` type (not always `0`; not `null`; not `undefined`). Runs as part of every container-harness invocation post-m211.

**Acceptance Scenarios**:

1. **Given** the container harness runs the m211 SC-001 fixture trace, **When** the assertion runs, **Then** `.predicate.trace_integrity.ring_buffer_overflows` is a JSON `number` type AND appears in the attestation output (not missing, not `null`).
2. **Given** a hypothetical regression where someone re-hardcodes the counter to `0` at a new emission site, **When** the trace runs against a workload known to exceed buffer capacity, **Then** the assertion fails because `ring_buffer_overflows == 0` for a workload that empirically drops events.

---

*(US3 — per-map drop attribution — was dropped per Clarification Q1. Aggregate `ring_buffer_overflows` is enough for m212; per-map breakdown would require a wire-field addition that conflicts with FR-003. Filed as a follow-up if operators demand per-buffer attribution later.)*

### Edge Cases

- **What happens on a hermetic short-lived command that fires zero events?** All counter maps stay at zero; the attestation reports `0`. Exercises the "count is accurate at the low end" case.
- **What happens when the container has multiple CPUs and events fire concurrently across them?** The per-CPU counter map's userspace-side aggregation sums across CPUs; the reported value is the total across all CPUs.
- **What happens if the counter map itself fails to attach (older kernel, unusual configuration)?** The trace continues without a fatal error; the attestation reports `0` (matches pre-fix behavior); a WARN log surfaces the counter-attach failure; AND the counter map's name is added to `TraceIntegrity.kprobe_attach_failures[]` per Clarification Q3. Consumers who trust the transparency signal check that array before relying on the reported `0` value.
- **What happens on a scan-mode invocation (no trace mode, no eBPF)?** The `trace_integrity` fields either stay at `0` (per pre-fix behavior for scan-mode) or the whole `trace_integrity` object is absent — either is acceptable as long as the wire shape matches the pre-fix scan-mode shape byte-identically.
- **What happens under counter-map overflow (u64 counter wrapping)?** Theoretical; u64 max is ~1.8×10^19. A trace would have to run for centuries at the highest observed event rate to overflow. Not a practical concern.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: mikebom's eBPF programs MUST increment a per-CPU counter whenever `RingBuf::reserve()` returns `None`, distinct per ring buffer (`FILE_EVENTS`, `NETWORK_EVENTS`, `COMPILER_EXEC_EVENTS`).
- **FR-002**: mikebom's userspace loader MUST read the per-CPU counter maps at trace end, sum across all CPUs, and populate `TraceIntegrity.ring_buffer_overflows` with the aggregate.
- **FR-003**: The emission of `TraceIntegrity` fields MUST match the pre-fix wire shape byte-identically EXCEPT for the (previously zero) `ring_buffer_overflows` and `events_dropped` values. Field names, types, ordering, and optional-field emission gates stay unchanged.
- **FR-004**: mikebom MUST NOT hardcode `ring_buffer_overflows: 0` anywhere in code paths that emit a real trace attestation. The value MUST derive from the kernel-side counter aggregate at every emission site (except pre-existing test / synthetic fixture builders where the value is intentionally scaffolded).
- **FR-005**: When one or more counter maps fail to attach (unsupported kernel), mikebom MUST (a) log a single WARN line under 500 bytes per failing map summarizing the failure, (b) continue emitting the trace with a **partial sum** of `ring_buffer_overflows` — the failing counter contributes `0` while any counter maps that DID successfully attach contribute their real counts (per Clarification Q4), AND (c) append the failed counter map's name (e.g. `"file_event_drops"`, `"network_event_drops"`, `"compiler_exec_drops"`) to the existing `TraceIntegrity.kprobe_attach_failures[]` array so downstream attestation consumers can distinguish "counter says zero drops" (trust the trace) from "counter unavailable on this kernel" (aggregate is a floor, not a total) per Clarification Q3. Reusing the existing field means zero wire-shape additions.
- **FR-006**: `events_dropped` stays hardcoded to `0` in m212 per Clarification Q2. Populating it requires a separate userspace-side post-trace ringbuf backlog probe (distinct mechanism from the kernel-side reserve-failure counter this milestone introduces); scoped to a follow-up milestone tracked at [waybill#618](https://github.com/kusari-oss/waybill/issues/618). Consumers of the emitted attestation continue to see `events_dropped: 0` post-m212 pending that follow-up.
- **FR-007**: The kernel-side per-CPU counter maps MUST NOT introduce ANY overhead on the reserve-success hot path (the `if let Some(...) = reserve()` branch that dominates in the trace's expected common case). The counter increment lives strictly in the `else` branch and fires only on drops. On the drop path, the counter increment is a straight-line per-CPU-array lookup + single `saturating_add` — inherently O(1) and lock-free thanks to per-CPU semantics (see research R1 + contracts/ebpf-verifier-notes.md V-2). Verification: the m211 SC-001 container harness runs pre-m212 vs post-m212 → assert the traced `cargo build` wall-clock time does not regress by more than 10% (pre-pr.sh + container harness both surface material perf regressions via existing test timing).
- **FR-008**: Scan-mode invocations (no trace, no eBPF) MUST continue emitting SBOMs byte-identically to pre-fix output. This feature only touches trace-mode paths.
- **FR-009**: The regression harness (`scripts/ebpf-integration-test.sh`) MUST include a jq assertion that verifies `.predicate.trace_integrity.ring_buffer_overflows` is a JSON `number` type (not always `0`; not `null`; not `undefined`).

### Key Entities

- **`TraceIntegrity`** (existing type in `mikebom-common/src/attestation/integrity.rs` or equivalent): the wire record carrying trace-fidelity signals. Fields include `ring_buffer_overflows: u64`, `events_dropped: u64`, `uprobe_attach_failures: Vec<String>`, `kprobe_attach_failures: Vec<String>`, `partial_captures: Vec<...>`, `bloom_filter_capacity: u64`, `bloom_filter_false_positive_rate: f64`. Wire shape unchanged per FR-003; only the `ring_buffer_overflows` + `events_dropped` values now reflect reality.
- **Per-CPU counter maps** (new, kernel-side): three new `PerCpuArray<u64>` maps in `mikebom-ebpf/src/maps.rs`, one per ring buffer (`FILE_EVENT_DROPS`, `NETWORK_EVENT_DROPS`, `COMPILER_EXEC_EVENT_DROPS`). Length 1 (single-element array). Incremented in the `else` branch of every `if let Some(mut buf) = <RINGBUF>.reserve()` site in `mikebom-ebpf/src/programs/*.rs` (9 sites total per grep: file_ops.rs × 4, compiler_exec.rs × 2, tcp_connect.rs × 1, tls_openssl.rs × 2).
- **Userspace aggregation** (new, userspace-side): a helper function that reads the per-CPU counter maps via aya's `PerCpuArray::get()` at trace end, sums across all CPUs, returns a per-map dictionary. Consumed by the `TraceIntegrity` builder in `mikebom-cli/src/trace/aggregator.rs` (or equivalent).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the m211 SC-001 fixture inside the Colima aarch64 6.8 container harness, `.predicate.trace_integrity.ring_buffer_overflows` in the emitted attestation is `> 100` — reflecting the observed drop volume from issue #614 (which showed 80%+ of events being silently dropped). Confirmation that the counter is actually incrementing.
- **SC-002**: On a hermetic short-lived traced command (`mikebom trace run -- true`), the counter is REACHABLE — `.predicate.trace_integrity.kprobe_attach_failures` does NOT contain any `*_drops` entry (all three counter maps loaded). The value of `ring_buffer_overflows` itself is host-dependent (post-m211 `filter_by_pid=false` when no `--target-pid` is set means the trace captures every process's syscalls on the whole host, not just the child's — an idle host might report `0`, a busy CI/dev host with compose stack activity will report non-zero). The invariant this SC actually guards is "counter is loaded and readable" — falsifying reachability is a real regression; falsifying "= 0" would be a spec bug we can't test around without adding `--target-pid`. Adjusted post-implementation per T011 finding.
- **SC-003**: On Ubuntu 22.04/24.04 amd64 CI (Linux 6.5+), the container harness assertion (`ring_buffer_overflows` is a number type, populated correctly) passes on every CI run. Regression guard against re-hardcoding.
- **SC-004**: A regression test in `mikebom-cli/src/trace/aggregator.rs::tests` OR an equivalent location MUST verify the wire shape of `TraceIntegrity` — deserializes byte-identically to a pre-fix reference JSON blob EXCEPT for the two counter fields. Guards against accidental wire-shape drift.
- **SC-005**: A `cargo +stable clippy --workspace --all-targets -- -D warnings` invocation passes clean post-fix. No new lints; no new unsafe blocks; matches the existing eBPF-adjacent code standards.
- **SC-006**: A unit test in `mikebom-cli/src/trace/aggregator.rs::tests` OR `loader.rs::tests` simulates the counter-map attach failure path (injectable via a test-only feature flag or by constructing a `LoaderError::MapNotFound(...)` directly). Asserts the resulting `TraceIntegrity` has `ring_buffer_overflows == 0` AND `kprobe_attach_failures` contains the counter map's name. Guards the Clarification-Q3 disambiguation contract against regression.

## Assumptions

- The three new `PerCpuArray<u64>` maps (one per existing ring buffer) fit within eBPF's map-count budget (kernel default: up to 1000+ maps per program on Linux 6.5+; mikebom currently uses ~15). No kernel resource impact.
- Reading the counter maps at trace end via aya's `PerCpuArray::get()` API is supported on aya-ebpf 0.1.1 and aya 0.13. Verified during phase 0 research; if not supported, fallback to `bpf_map_lookup_elem` via raw aya `MapData` handles.
- The eBPF verifier accepts the new `else`-branch atomic increment pattern without additional hardening. Increment-only atomic ops on a per-CPU u64 are the simplest possible pattern the verifier handles trivially.
- Wire-shape compatibility: `TraceIntegrity` fields are `u64` today; no type change. Consumers parsing `ring_buffer_overflows: 0` will see `ring_buffer_overflows: <real-non-zero-value>` post-fix. This is a semantic change (the value now reflects reality) but not a schema change.
- No new Cargo dependencies. The `aya` + `aya-ebpf` crates already expose per-CPU array support via `PerCpuArray`.
- Prerequisite for #616 (kernel-side trace-noise filter). This spec is the observability layer that will make #616's fix VERIFIABLE — without it we can't distinguish "filter works" from "some other bug still drops events".
- Older kernels (Ubuntu 20.04 LTS 5.15, etc.) — per the m211 Clarification-Q1 support matrix, best-effort. If per-CPU array reads fail on older kernels, the FR-005 fallback (WARN + report `0`) kicks in.
