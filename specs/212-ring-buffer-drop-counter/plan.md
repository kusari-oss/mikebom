# Implementation Plan: Real ring-buffer-overflow counter for eBPF trace-mode observability

**Branch**: `212-ring-buffer-drop-counter` | **Date**: 2026-07-20 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/212-ring-buffer-drop-counter/spec.md`

## Summary

Add three per-CPU `u64` counter maps to `mikebom-ebpf/src/maps.rs`, one per existing ring buffer (`FILE_EVENTS`, `NETWORK_EVENTS`, `COMPILER_EXEC_EVENTS`). Increment the appropriate counter in the `else` branch of every `if let Some(mut buf) = <RINGBUF>.reserve() {...}` site in the eBPF programs (9 sites total per grep). At trace end, userspace-side reads each counter map via aya's `PerCpuArray::get()`, sums across all online CPUs, populates `TraceIntegrity.ring_buffer_overflows` with the aggregate. On counter-map attach failure (older kernel, unusual config), the map's name gets appended to the existing `TraceIntegrity.kprobe_attach_failures[]` array per Clarification Q3 — reusing an existing field, no wire-shape addition. `events_dropped` stays hardcoded to `0` per Clarification Q2 (deferred to [waybill#618](https://github.com/kusari-oss/waybill/issues/618)). Container-harness assertion + Rust unit test guard against regression.

## Technical Context

**Language/Version**: Rust nightly (eBPF target via `aya-ebpf` 0.1.1) + Rust stable (workspace toolchain inherited from milestones 001–212; no new nightly features). No MSRV change.

**Primary Dependencies**: Existing only — `aya = 0.13` (userspace-side `PerCpuArray` handle reader), `aya-ebpf = 0.1.1` (kernel-side `PerCpuArray<u64>` writer — pattern proven at `mikebom-ebpf/src/maps.rs:41` where `SCRATCH_BUF: PerCpuArray<[u8; 512]>` already exists for TLS scratch), `serde`/`serde_json` (existing wire encoding for `TraceIntegrity`), `tracing` (WARN emission on FR-005 counter-attach failure), `anyhow`/`thiserror` (error propagation). **No new Cargo dependencies at any layer.**

**Storage**: N/A — counters are in-kernel per-CPU u64 slots; userspace reads them once at trace-end + drops the map handle. Nothing persists past a single trace invocation.

**Testing**:
- Rust unit tests in `mikebom-cli/src/trace/loader.rs::tests` (SC-006) — synthesize a counter-map attach failure via test-only feature flag; assert `TraceIntegrity.kprobe_attach_failures` contains the counter map's name AND `ring_buffer_overflows == 0`.
- Rust unit tests in `mikebom-common/src/attestation/integrity.rs::tests` (SC-004) — extend the existing `trace_integrity_serde_round_trip` test to cover the case where `ring_buffer_overflows > 0` alongside `kprobe_attach_failures[]` populated with counter names.
- Container integration test (SC-001 + SC-002) — reuse the m211 `Dockerfile.ebpf-test` harness. Extend `scripts/ebpf-integration-test.sh` with a jq assertion that `.predicate.trace_integrity.ring_buffer_overflows` is a JSON `number` type AND value > `0` on the m211 SC-001 fixture (per #614 evidence that the fixture already saturates the buffer).
- Manual smoke test on `mikebom trace run -- true` — asserts `ring_buffer_overflows: 0` on a zero-syscall command (SC-002 counterpart).

**Target Platform**: Linux only (eBPF is Linux-native). Per m211 Clarification Q1's support matrix: Colima aarch64 Linux 6.8 (dev) + Ubuntu 22.04/24.04 amd64 Linux 6.5+ (CI). Older kernels (5.15 LTS, 5.10) — best-effort per FR-005; counter map attach failure surfaces via `kprobe_attach_failures[]` per Clarification Q3.

**Project Type**: eBPF-instrumented CLI tool. Three-crate architecture (Constitution Principle VI) untouched — this milestone only touches:
- `mikebom-ebpf` (new maps + `else`-branch increments in existing programs)
- `mikebom-cli` (new aggregation helper + wiring into `TraceIntegrity` builder + FR-005 attach-failure handling)
- `mikebom-common` (no changes — `TraceIntegrity` shape is frozen per FR-003)

**Performance Goals**:
- FR-007: kernel-side counter increment overhead < 50 ns per dropped event; zero overhead when reserve succeeds (the increment lives in the `else` branch, not on the hot path).
- Userspace-side aggregation at trace end: one-time O(num_cpus × num_maps) — for 3 maps on a 128-core host that's ~384 map lookups total, well under 10 ms.

**Constraints**:
- **FR-003 byte-identity**: `TraceIntegrity`'s serialized JSON MUST match the pre-fix shape byte-identically EXCEPT for the (previously always-zero) `ring_buffer_overflows` value AND the (previously always-empty) `kprobe_attach_failures[]` array when a counter map fails to attach. Field names, types, ordering, `serde_json` output — all unchanged.
- **eBPF stack limit** (512 bytes per program frame): counter increment adds ~8 bytes to the stack frame (a `u64` local for the accumulator). Well within budget.
- **eBPF verifier**: `PerCpuArray::get_ptr_mut()` + atomic increment is a well-established pattern (SCRATCH_BUF precedent); no new verifier hardening expected. Per-CPU maps eliminate cross-CPU atomic contention entirely — each CPU has its own u64 slot.
- **Constitution Principle II** (eBPF-Only Observation): no LD_PRELOAD, no ptrace. The fix stays in-kernel.
- **Constitution Principle IV** (Type-Driven Correctness): the counter is a plain `u64`, wire-serialized via existing `TraceIntegrity.ring_buffer_overflows: u64` field. Type doesn't change.

**Scale/Scope**:
- Three new `PerCpuArray<u64>` map declarations in `mikebom-ebpf/src/maps.rs` (~30 LOC).
- Nine `else`-branch increments across `mikebom-ebpf/src/programs/*.rs` (~2 LOC per site, 18 LOC total).
- One userspace aggregation helper in `mikebom-cli/src/trace/loader.rs` or a new `mikebom-cli/src/trace/counters.rs` module (~50 LOC).
- Wiring into the `TraceIntegrity` builder at existing emission sites (~10 LOC).
- FR-005 attach-failure handling in `mikebom-cli/src/trace/loader.rs` (~30 LOC).
- One unit test in `integrity.rs::tests` (~20 LOC extending the existing round-trip test) + one unit test in `loader.rs::tests` for SC-006 (~40 LOC).
- One harness assertion extension in `scripts/ebpf-integration-test.sh` (~10 LOC).
- Total estimated diff: **~150-200 LOC production, ~60 LOC test/harness, 0 LOC docs**.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Pure Rust, Zero C**: ✅ No C. Rust + eBPF bytecode (aya-ebpf compiled via bpf-linker; no C toolchain touched).
- **II. eBPF-Only Observation**: ✅ CANONICAL principle — the counter maps are pure kernel-side observation. No new observation surface introduced; just measuring an existing one.
- **III. Fail Closed**: ✅ On counter-map attach failure, the trace continues (matches pre-fix behavior) BUT the failure is surfaced via `kprobe_attach_failures[]` (Q3) so downstream consumers see the degraded state. Operators can decide whether to trust the trace.
- **IV. Type-Driven Correctness**: ✅ `TraceIntegrity.ring_buffer_overflows: u64` field type unchanged. New maps use the existing `PerCpuArray<u64>` type. All values flow through typed serde interfaces.
- **V. Specification Compliance**: ✅ `TraceIntegrity` JSON shape is the wire contract; FR-003 locks it byte-identically. No new fields; existing field semantics extended (kprobe_attach_failures[] now carries counter-map names too, per Q3).
- **VI. Three-Crate Architecture**: ✅ Only `mikebom-ebpf` + `mikebom-cli` change structurally. `mikebom-common` `TraceIntegrity` shape is FROZEN. Cross-crate boundary preserved.
- **VII. Test Isolation**: ✅ Container harness runs in `--privileged` Docker isolated from host. Unit tests run in-process with feature-flag-injected failure simulation (no real kernel attachment needed).
- **VIII. Completeness**: ✅ Adds coverage (real counter values), removes fake ones (0 → real). No feature dropped.
- **IX. Accuracy**: ✅ Fixes an accuracy bug — pre-fix `ring_buffer_overflows: 0` was factually false when drops occurred. Post-fix reports reality.
- **X. Transparency**: ✅ CANONICAL principle — this milestone IS transparency: exposing a signal previously hidden. Q3's attach-failure disambiguation is a further transparency step.
- **XI. Enrichment**: N/A — no enrichment layer.
- **XII. External Data Source Enrichment**: N/A — no external data sources.

**Strict Boundaries**: no violations. eBPF stays in `mikebom-ebpf`. `TraceIntegrity` shape (only cross-crate shared type touched) stays frozen per FR-003.

**Verdict**: ✅ Constitution check passes; no violations to justify. Proceed to Phase 0.

## Project Structure

### Documentation (this feature)

```text
specs/212-ring-buffer-drop-counter/
├── plan.md              # This file (/speckit.plan command output)
├── research.md          # Phase 0 output — PerCpuArray API choice + increment pattern + attach-failure surfacing strategy + wire-shape byte-identity strategy
├── data-model.md        # Phase 1 output — new PerCpuArray<u64> maps + kprobe_attach_failures[] semantic extension
├── quickstart.md        # Phase 1 output — end-to-end verification recipe (container harness + jq assertions + smoke test on hermetic short command)
├── contracts/
│   ├── counter-semantics.md   # Wire contract for what ring_buffer_overflows means post-m212
│   └── ebpf-verifier-notes.md # Verifier acceptance contract for the per-branch atomic increment pattern
└── tasks.md             # Phase 2 output (/speckit.tasks command — NOT created by /speckit.plan)
```

### Source Code (repository root)

```text
mikebom-ebpf/
└── src/
    ├── maps.rs                          # 3 new #[map] static PerCpuArray<u64> declarations
    └── programs/
        ├── file_ops.rs                  # 4 else-branch increments on FILE_EVENT_DROPS
        ├── tcp_connect.rs               # 1 else-branch increment on NETWORK_EVENT_DROPS
        ├── tls_openssl.rs               # 2 else-branch increments on NETWORK_EVENT_DROPS
        └── compiler_exec.rs             # 2 else-branch increments on COMPILER_EXEC_DROPS

mikebom-cli/
└── src/
    └── trace/
        ├── loader.rs                    # Counter map attach + FR-005 failure surfacing (add counter name to kprobe_attach_failures)
        └── counters.rs                  # NEW module — userspace aggregation helper
                                          # ~50 LOC. Public: fn read_ring_buffer_drops(bpf: &mut aya::Ebpf) -> HashMap<&'static str, u64>

mikebom-cli/src/cli/
└── scan.rs                              # Wiring: at trace-end, call counters::read_ring_buffer_drops, sum into TraceIntegrity.ring_buffer_overflows

mikebom-common/src/attestation/
└── integrity.rs                         # FROZEN per FR-003 — only extend the existing round-trip test to cover counter-populated + attach-failure cases

scripts/
└── ebpf-integration-test.sh             # New jq assertion: ring_buffer_overflows is a number + > 0 on SC-001 fixture (per SC-001)
```

**Structure Decision**: single-milestone edit primarily in `mikebom-ebpf` + `mikebom-cli/src/trace/`. One new module (`mikebom-cli/src/trace/counters.rs`) — keeps the aggregation logic testable in isolation. `mikebom-common` untouched. No new crates, no new modules elsewhere.

## Complexity Tracking

> No Constitution violations. Complexity tracking section unused.
