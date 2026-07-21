# Contract: eBPF verifier acceptance for the per-branch increment pattern

**Milestone**: 212
**Date**: 2026-07-20
**Purpose**: Lock the specific pattern used for kernel-side increment sites so reviewers can audit m212 changes AND future contributors know what pattern to follow when adding new counter maps.

## V-1: Canonical increment site pattern

Every one of the 9 sites in `mikebom-ebpf/src/programs/*.rs` that reserves a ring-buffer entry MUST use this pattern:

```rust
if let Some(mut buf) = <RING_BUFFER>.reserve::<EventType>(0) {
    // ... existing fill + submit logic ...
    let event = buf.as_mut_ptr();
    unsafe {
        (*event).field = value;
        // ...
    }
    buf.submit(0);
} else {
    increment_drop_counter(&<DROPS_MAP>);
}
```

Where `<RING_BUFFER>` is one of `FILE_EVENTS` / `NETWORK_EVENTS` / `COMPILER_EXEC_EVENTS`, and `<DROPS_MAP>` is the corresponding `FILE_EVENT_DROPS` / `NETWORK_EVENT_DROPS` / `COMPILER_EXEC_DROPS`.

## V-2: `increment_drop_counter` helper contract

Location: `mikebom-ebpf/src/helpers.rs`.

Signature:
```rust
#[inline(always)]
pub fn increment_drop_counter(map: &PerCpuArray<u64>) {
    if let Some(counter) = map.get_ptr_mut(0) {
        unsafe { *counter = (*counter).saturating_add(1); }
    }
}
```

**Verifier-friendliness properties**:
- **Bounded**: no loops. Straight-line code with one branch (`if let Some(...)`).
- **Bounded stack**: adds only the local `counter: *mut u64` (8 bytes) to the frame. Under budget.
- **Type-safe**: `PerCpuArray::get_ptr_mut(0)` returns `Option<*mut u64>`; the `unsafe` dereference is safe because `Some(...)` guarantees the pointer's validity (aya-ebpf contract).
- **`#[inline(always)]`**: guarantees zero function-call overhead at the eBPF-bytecode level; the helper becomes 3 inline instructions per call site (map lookup + load + store).
- **`saturating_add`**: eliminates the theoretical u64-overflow undefined-behavior concern in a single instruction (LLVM's `llvm.uadd.sat.i64` intrinsic).

## V-3: Verifier acceptance expectation

**Assertion**: The 9 sites × 1 helper add ≤ 30 verifier instructions per site (well under the 1M instruction budget). Total added: ~270 instructions across the trace-relevant programs.

**Reference precedent**: `mikebom-ebpf/src/maps.rs:41` declares `SCRATCH_BUF: PerCpuArray<[u8; 512]>` which is used by TLS programs; verifier accepts `PerCpuArray::get_ptr_mut(0)` + deref cleanly. Same pattern here for `PerCpuArray<u64>`.

**Regression guard**: the m211 container harness (`Dockerfile.ebpf-test`) rebuilds the eBPF bytecode via bpf-linker every run; a verifier rejection surfaces as a WARN at load time. Post-m212 the WARN pattern would name one of the counter maps (`FILE_EVENT_DROPS` / etc.), immediately identifying the regression.

## V-4: Verifier-hardening escape hatches

If the above pattern DOES trip a verifier rejection on some kernel we didn't test (Colima aarch64 6.8 is the target; edge kernels not exhaustively verified):

- **Fallback A**: convert the helper to inline the increment 9× at every site (no shared helper) to eliminate any hypothetical inlining edge case.
- **Fallback B**: replace `saturating_add(1)` with `wrapping_add(1)` — same behavior for the 600-year-to-overflow timescale, potentially simpler bytecode.
- **Fallback C**: split each counter map into per-program maps (`FILE_EVENTS_DROPS_FROM_VFS_WRITE`, `FILE_EVENTS_DROPS_FROM_VFS_READ`, etc.) so each site references a unique map — verifier state-tracking becomes even simpler at the cost of 9 maps vs 3.

None of these are expected to be needed on our target platform; documented as pre-known-good escape hatches.

## V-5: No new kernel API surface

**Assertion**: m212 uses ONLY the `PerCpuArray::get_ptr_mut()` API — the SAME API `SCRATCH_BUF` already uses at line 41 of maps.rs. No new BPF helpers introduced. No new kernel-version dependency.

**Kernel-version support**: PerCpuArray has been stable in Linux since 4.6 (2016); the `bpf_map_lookup_elem` helper it wraps has been stable since 3.19 (2015). Every kernel in m211's Q1 support matrix (Linux 6.5+) trivially supports it.
