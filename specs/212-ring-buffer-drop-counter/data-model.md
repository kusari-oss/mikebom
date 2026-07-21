# Data Model: Real ring-buffer-overflow counter

**Milestone**: 212
**Date**: 2026-07-20
**Status**: Phase 1

## Overview

m212 introduces **three new kernel-side maps** (per-CPU counter arrays) and **extends the semantics of an existing wire field** (`TraceIntegrity.kprobe_attach_failures[]`). No new persistent types. `TraceIntegrity` shape stays frozen per FR-003.

## E1 — `TraceIntegrity` (existing, frozen shape)

**Ownership**: `mikebom-common/src/attestation/integrity.rs`.
**Lifecycle**: constructed once per trace at emission time, serialized into the `BuildTracePredicate`'s `trace_integrity` field, emitted to the attestation JSON.

**Fields** (per current `mikebom-common/src/attestation/integrity.rs`, unchanged in this milestone):

| Field | Type | Post-m212 semantics |
|---|---|---|
| `ring_buffer_overflows` | u64 | **CHANGED**: now the actual sum of drops across all three per-CPU counter maps × all online CPUs. Was hardcoded `0`. |
| `events_dropped` | u64 | **UNCHANGED**: stays hardcoded `0` per Q2 (deferred to [waybill#618](https://github.com/kusari-oss/waybill/issues/618)). |
| `uprobe_attach_failures` | Vec\<String\> | **UNCHANGED**: existing kprobe-adjacent failure list. |
| `kprobe_attach_failures` | Vec\<String\> | **EXTENDED per Q3**: still carries real kprobe attach failure names; now ALSO carries counter-map attach failure names (e.g. `"file_event_drops"`) when a counter map fails to attach. Sorted lexicographically at emission time per R6. |
| `partial_captures` | Vec\<PartialCapture\> | **UNCHANGED**. |
| `bloom_filter_capacity` | u64 | **UNCHANGED**. |
| `bloom_filter_false_positive_rate` | f64 | **UNCHANGED**. |

**Wire shape**: identical to pre-m212 per FR-003. The two changes are SEMANTIC (a previously-lie counter now reflects reality; an existing free-form string array now carries more entries under certain conditions), not STRUCTURAL.

**Validation rules** (unchanged): standard serde derivation.

## E2 — Kernel-side per-CPU counter maps (NEW)

Three new `#[map] pub static <NAME>: PerCpuArray<u64>` declarations in `mikebom-ebpf/src/maps.rs`:

### `FILE_EVENT_DROPS`

- **Type**: `PerCpuArray<u64>` with `max_entries = 1` (single-element array; per-CPU is the dimension we care about, not per-index)
- **Written by**: 4 sites in `mikebom-ebpf/src/programs/file_ops.rs`:
  - `try_vfs_write` (line 45 area)
  - `try_vfs_read` (line 92 area)
  - `try_openat2` (line 147 area)
  - `try_do_filp_open` (line 230 area)
- **Read by**: userspace `mikebom-cli/src/trace/counters.rs::read_percpu_sum(bpf, "FILE_EVENT_DROPS")` at trace end.
- **Semantics**: incremented once per `FILE_EVENTS.reserve::<FileEvent>(0)` call that returns `None`.

### `NETWORK_EVENT_DROPS`

- **Type**: same as above.
- **Written by**: 3 sites:
  - `tcp_connect.rs::try_tcp_connect` (line 66 area) — 1 site.
  - `tls_openssl.rs::try_ssl_read` (line 103 area) — 1 site.
  - `tls_openssl.rs::try_ssl_write` (line 181 area) — 1 site.
- **Read by**: `read_percpu_sum(bpf, "NETWORK_EVENT_DROPS")`.
- **Semantics**: incremented once per `NETWORK_EVENTS.reserve::<NetworkEvent>(0)` call that returns `None`.

### `COMPILER_EXEC_DROPS`

- **Type**: same as above.
- **Written by**: 2 sites in `mikebom-ebpf/src/programs/compiler_exec.rs`:
  - `try_sched_process_exec` (line 224 area)
  - `try_sched_process_exit` (line 316 area)
- **Read by**: `read_percpu_sum(bpf, "COMPILER_EXEC_DROPS")`.
- **Semantics**: incremented once per `COMPILER_EXEC_EVENTS.reserve::<CompilerExecEvent>(0)` call that returns `None`.

### Sizing & memory footprint

Each `PerCpuArray<u64>` with `max_entries = 1` allocates 8 bytes per CPU × N CPUs. On a 128-core host: 128 × 8 = 1 KB per map, 3 KB total. Negligible kernel resource impact.

## E3 — `mikebom-cli/src/trace/counters.rs` (NEW module)

**Ownership**: `mikebom-cli/src/trace/counters.rs` (new file, ~50 LOC).
**Lifecycle**: called once per trace at trace-end, before `TraceIntegrity` construction.

**Public API**:

```rust
pub fn read_ring_buffer_drops(bpf: &mut aya::Ebpf) -> HashMap<&'static str, u64>;
```

Returns a per-map breakdown even though m212 only sums-then-uses-total. The per-map breakdown is preserved for future extensibility if US3 (dropped in Q1) is revived, without changing this function's signature.

**Internal helper**:

```rust
fn read_percpu_sum(bpf: &mut aya::Ebpf, name: &str) -> anyhow::Result<u64>;
```

Sums across all online CPUs via aya's `PerCpuArray::get(&0, 0)` which returns `Vec<u64>`.

## E4 — `TraceIntegrityBuilder` wiring changes

**Ownership**: `mikebom-cli/src/cli/scan.rs` (at the `TraceIntegrity` construction site, line 538 area per pre-m212 grep).

**Change**: replace `ring_buffer_overflows: 0` with:

```rust
let drops = counters::read_ring_buffer_drops(&mut handle.bpf);
let total_overflows: u64 = drops.values().sum();
// ...
TraceIntegrity {
    ring_buffer_overflows: total_overflows,
    // ... rest unchanged ...
    kprobe_attach_failures: {
        let mut all_failures = kprobe_failures;
        all_failures.extend(counter_attach_failures);
        all_failures.sort();
        all_failures.dedup();
        all_failures
    },
    // ...
}
```

Similar changes at every OTHER `TraceIntegrity { ring_buffer_overflows: 0, ... }` construction site (5 additional sites per pre-m212 grep) — EXCEPT test/synthetic fixture builders which stay unchanged.

## State transitions

None. Counter values monotonically increase during a trace + are read once at trace end. No state machine.

## Persistence

None. Counter maps are ephemeral eBPF kernel state; when the eBPF programs unload (mikebom-cli exits), the maps are freed. The aggregate lands only in the emitted attestation JSON.
