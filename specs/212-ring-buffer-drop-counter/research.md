# Research: Real ring-buffer-overflow counter

**Milestone**: 212
**Date**: 2026-07-20
**Status**: Phase 0 complete

## R1 — Counter map type: `PerCpuArray<u64>` vs global `Array<u64>` vs atomic global

**Question**: Which eBPF map type minimizes overhead + avoids cross-CPU contention while remaining aya-ebpf-friendly?

**Decision**: `PerCpuArray<u64>` with 1 element per map. Increment via `if let Some(counter) = MAP.get_ptr_mut(0) { *counter = (*counter).saturating_add(1); }` in the eBPF program. Userspace-side, read via `PerCpuArray::get(&0)` (returns `Option<Vec<u64>>` in aya 0.13 — one entry per online CPU) and sum.

**Rationale**:
- **Per-CPU eliminates atomic contention**. On a 128-core host with heavy do_filp_open activity, a global counter would need `__sync_fetch_and_add` (BPF atomic add) which serializes across CPUs. Per-CPU per-element eliminates the atomic entirely; each CPU has its own slot updated with a plain non-atomic increment.
- **Established pattern in this codebase**: `SCRATCH_BUF: PerCpuArray<[u8; 512]>` at `mikebom-ebpf/src/maps.rs:41` is the existing precedent. Reusing the same map type keeps the code base consistent and confirms aya-ebpf 0.1.1 + bpf-linker produce verifier-friendly bytecode for this pattern.
- **Aya 0.13 API is stable**: `aya::maps::PerCpuArray::<MapData, u64>::try_from(bpf.map_mut(...))?.get(&0, 0)?` returns a `PerCpuValues<u64>` iterable over online CPUs. Documented, tested, no experimental flags needed.
- **saturating_add avoids overflow undefined behavior**. u64 max is ~1.8×10^19 — reaching it would require ~600 years at max observed event rate — but `saturating_add` is a one-instruction change in the eBPF backend and eliminates the theoretical concern.

**Alternatives considered**:
- **Alt A: `Array<u64>` (global) with `__sync_fetch_and_add`**. REJECTED — atomic contention across CPUs; per-event overhead higher than the FR-007 < 50 ns budget on multi-core hosts under load.
- **Alt B: `HashMap<u32, u64>` keyed by CPU number**. REJECTED — same effect as PerCpuArray but more overhead (hash lookup vs. direct offset), less idiomatic in aya-ebpf.
- **Alt C: Ring buffer with drop-event records**. REJECTED — recursion risk (a ring-buffer overflow while emitting an overflow event is a loop), and drops would need to be aggregated userspace-side anyway. Counter is strictly simpler.

## R2 — Increment site pattern: else-branch of `if let Some(mut buf) = <RINGBUF>.reserve()`

**Question**: What does the increment code look like at each of the 9 emission sites?

**Decision**: A one-line macro invocation at the tail of each `if let Some(mut buf) = <RINGBUF>.reserve() { ... }` block, in a new `else` branch. Example for `try_do_filp_open`:

```rust
if let Some(mut buf) = FILE_EVENTS.reserve::<FileEvent>(0) {
    // ... existing fill + submit ...
} else {
    increment_drop_counter(&FILE_EVENT_DROPS);
}
```

Where `increment_drop_counter` is a `#[inline(always)]` helper in `mikebom-ebpf/src/helpers.rs`:

```rust
#[inline(always)]
pub fn increment_drop_counter(map: &PerCpuArray<u64>) {
    if let Some(counter) = map.get_ptr_mut(0) {
        unsafe { *counter = (*counter).saturating_add(1); }
    }
}
```

**Rationale**:
- **One-line macro-like site keeps the diff small + reviewable**. Each of the 9 sites gets a 3-line `else { ... }` addition (2 LOC per site + closing brace).
- **`#[inline(always)]` guarantees zero function-call overhead** at the eBPF-bytecode level; the helper becomes a straight-line 3-instruction sequence per site.
- **`unsafe` block is minimal**: only the raw-pointer deref. The pointer's validity is guaranteed by `get_ptr_mut(0)` returning `Some(...)` — aya-ebpf's contract.

**Alternatives considered**:
- **Alt A: Duplicate the increment 9× inline (no helper)**. REJECTED — DRY violation; harder to audit whether all sites use identical semantics.
- **Alt B: Wrap `reserve()` in a helper that handles both branches**. REJECTED — the fill logic between reserve() + submit() is different at every site, so wrapping requires a callback-style API that fights the borrow checker.

## R3 — Userspace aggregation: sync vs async, error handling

**Question**: How does userspace aggregate per-CPU counter values at trace end?

**Decision**: Synchronous, one-shot at trace end. New helper in `mikebom-cli/src/trace/counters.rs`:

```rust
pub fn read_ring_buffer_drops(bpf: &mut aya::Ebpf) -> HashMap<&'static str, u64> {
    const MAP_NAMES: &[(&str, &str)] = &[
        ("file_event_drops",     "FILE_EVENT_DROPS"),
        ("network_event_drops",  "NETWORK_EVENT_DROPS"),
        ("compiler_exec_drops",  "COMPILER_EXEC_DROPS"),
    ];
    let mut totals = HashMap::new();
    for (short, map_name) in MAP_NAMES {
        let total = read_percpu_sum(bpf, map_name).unwrap_or(0);
        totals.insert(*short, total);
    }
    totals
}

fn read_percpu_sum(bpf: &mut aya::Ebpf, name: &str) -> anyhow::Result<u64> {
    let map = bpf.map_mut(name).ok_or_else(|| anyhow::anyhow!("map {name} not found"))?;
    let per_cpu: aya::maps::PerCpuArray<_, u64> = aya::maps::PerCpuArray::try_from(map)?;
    let values = per_cpu.get(&0, 0)?;  // Vec<u64>, one per CPU
    Ok(values.iter().sum())
}
```

The `TraceIntegrity` builder in `mikebom-cli/src/cli/scan.rs` calls `read_ring_buffer_drops` after the settling drain deadline expires and sums across all three maps into `ring_buffer_overflows`. On per-map failure (e.g. `PerCpuArray::try_from` returns an error because the map wasn't loaded), the userspace-side error handling adds the failing map name to `kprobe_attach_failures[]` per Q3 and uses `0` for that map's contribution.

**Rationale**:
- **Synchronous one-shot** avoids async complexity; the aggregation happens once, at a well-defined point (post-drain-settling, pre-attestation-emission), and completes in ~milliseconds even on high-core-count hosts.
- **`unwrap_or(0)` fallback per map** matches FR-005 semantics — degraded observability, not fatal.
- **`&'static str` map names** for the return value are compile-time constants matching the kernel-side map declarations, eliminating typo risk.
- **HashMap return** even though m212 only sums-then-throws-away lets a future milestone (US3 was rejected in Q1 but might come back) surface the per-map breakdown without changing this function's signature.

**Alternatives considered**:
- **Alt A: Async drain of each map on its own tokio task**. REJECTED — the maps are read-only at this point (trace-end); no benefit from concurrency.
- **Alt B: Store `Arc<Mutex<HashMap>>` accumulating throughout the trace**. REJECTED — per-CPU counters already accumulate kernel-side; userspace just reads the final value once.

## R4 — Wire-shape byte-identity strategy

**Question**: How do we verify `TraceIntegrity`'s serialized JSON matches the pre-fix shape byte-identically post-m212, per FR-003?

**Decision**: Extend the existing `trace_integrity_serde_round_trip` unit test at `mikebom-common/src/attestation/integrity.rs:30` with three additional cases:
1. All-zero case (default `TraceIntegrity`): serializes to a known-good byte pattern.
2. Populated-counter case (`ring_buffer_overflows: 12345`, `kprobe_attach_failures: vec!["file_event_drops"]`): asserts field ordering + serialization matches pre-fix serde output.
3. Round-trip case: serialize + deserialize → equal to input struct.

Reference JSON blobs stored inline as `&'static str` literals in the test; comparison via `serde_json::to_value(...)` (JSON-value equality, not string byte equality — allows serde-derived field ordering to drift across compiler versions without failing the test spuriously).

**Rationale**:
- **`serde_json::to_value` equality is the operator-facing contract**: consumers parsing the JSON care about field-name+value pairs, not literal byte positions.
- **Reference blobs inline in the test** eliminate cross-file test dependencies — the test is self-contained.
- **Test lives in `mikebom-common`** because that's where `TraceIntegrity` is defined; the test file is already committed with a round-trip test to extend.

**Alternatives considered**:
- **Alt A: Byte-for-byte string comparison of serialized JSON**. REJECTED — brittle across serde version bumps; adds no protection over `to_value`-equality.
- **Alt B: Committed golden file** (`tests/fixtures/trace_integrity_golden.json`). REJECTED — separate file to maintain; adds test-fixture churn on every serde-related change.

## R5 — Attach-failure surfacing implementation

**Question**: How does the loader detect + surface a counter-map attach failure per FR-005 + Q3?

**Decision**: In `mikebom-cli/src/trace/loader.rs`, immediately after the eBPF program object is loaded, verify each counter map is present by attempting an aya `PerCpuArray::try_from(bpf.map_mut(...))`. On failure, log a single WARN line (≤500 bytes) via the same pattern m211 uses for kprobe attach failures, AND capture the failing map name into a `Vec<String>` that the trace-end code path appends to `TraceIntegrity.kprobe_attach_failures`.

Concretely, extend the existing `attach_kprobe` sibling with a companion `verify_counter_map`:

```rust
fn verify_counter_map(bpf: &mut aya::Ebpf, name: &str) -> Result<(), String> {
    match bpf.map_mut(name) {
        Some(map) => match aya::maps::PerCpuArray::<_, u64>::try_from(map) {
            Ok(_) => Ok(()),
            Err(e) => {
                let short = extract_short_reason(&e.to_string());
                warn!("counter map {name} not usable on this kernel: {short}. \
                       ring_buffer_overflows will report 0.");
                Err(name.to_string())
            }
        },
        None => Err(name.to_string()),
    }
}
```

The `Err(name)` returns propagate up to the trace-loop state; at trace end, they're pushed onto `TraceIntegrity.kprobe_attach_failures[]` alongside real kprobe failures.

**Rationale**:
- **Reuses the m211 WARN-rate-limiting pattern** — same helper style as `attach_kprobe`'s error path.
- **Reuses `kprobe_attach_failures[]`** per Q3 (Vec<String> field, free-form names, no wire-shape addition).
- **Naming convention `<map_shortname>` matches the JSON key** in the `read_ring_buffer_drops` HashMap return (e.g. `"file_event_drops"`), so a failure surface + a data-side lookup share the same identifier.

**Alternatives considered**:
- **Alt A: Fail hard on counter-map attach failure**. REJECTED — mikebom's fail-closed principle applies to observation completeness (missing signal = flag it), not to hard-abort on optional-instrumentation loss.
- **Alt B: Introduce a new `TraceIntegrity.counter_attach_failures[]` array**. REJECTED per Q3 (wire-shape addition; FR-003 blocks).

## R6 — Ordering of kprobe_attach_failures[] entries

**Question**: When counter-map names get mixed in with real kprobe-name failures, does ordering matter?

**Decision**: Sort `kprobe_attach_failures[]` lexicographically at final emission. Both real kprobe attach failures AND counter map attach failures land in the same array, sorted. Duplicates removed via `dedup_by`.

**Rationale**:
- **Consistent ordering is critical for byte-identity comparisons** (FR-003) — if the two groups can appear in either order, byte-diff regressions become flaky.
- **Lexicographic sort is trivially deterministic** across runs, hosts, kernel versions.
- **The existing behavior** doesn't currently sort — but currently the array only contains kprobe attach failures, so ordering is stable-by-accident (matches attach order). Post-m212 with counter names mixed in, explicit sorting locks the ordering.

**Alternatives considered**:
- **Alt A: Insertion order (chronological)**. REJECTED — order depends on attach sequence which may drift between mikebom versions.
- **Alt B: Group by type (kprobes first, then counter maps)**. REJECTED — adds an implicit grouping consumers might not know about.

## R7 — SC-001 verification approach

**Question**: How do we prove SC-001 (`> 100` drops on the m211 SC-001 fixture) is reliably reproducible?

**Decision**: Use the exact container-harness invocation from m211's `quickstart.md`, extended with the new jq assertion. The fixture (`mikebom-cli/tests/fixtures/compiler_pipeline/two_binaries_diverge`) is proven per #614 investigation to saturate the ring buffer (12K+ events, cargo dominates, rustc events dropped). Threshold `> 100` gives an order-of-magnitude margin below the actual observed ~30K drops the buffer overflow was estimated to hide.

**Rationale**:
- **Reuses proven test infrastructure** — no new fixture needed.
- **Threshold `> 100`** avoids flakiness at the low end (SC-002's `== 0` case handles hermetic-command coverage separately).

**Alternatives considered**:
- **Alt A: Assert exact count**. REJECTED — the count is host-dependent (compose stack load affects it).
- **Alt B: Assert `> 1000`**. REJECTED — makes the test brittle to reasonable variations. `> 100` is a much safer lower bound that still proves the counter is incrementing.

## R8 — Constitution Principle X (Transparency) alignment

**Question**: Does m212 fully satisfy Principle X's transparency mandate, or are there residual gaps?

**Decision**: m212 fully addresses transparency for the `ring_buffer_overflows` signal. Residual gaps deferred to follow-ups:
- **`events_dropped` still hardcoded to 0** — [waybill#618](https://github.com/kusari-oss/waybill/issues/618) (Q2 deferral).
- **Per-map breakdown not exposed** — no filed issue yet (Q1 dropped US3 entirely; can be re-scoped later if operator demand surfaces).
- **`bloom_filter_capacity` + `bloom_filter_false_positive_rate`** — already accurate on `TraceIntegrity`, no change needed.
- **`partial_captures[]`** — semi-populated today; separate concern from ring buffer drops.

**Rationale**:
- Principle X is a MEANINGFUL FLOOR, not a "solve all observability at once" mandate. m212 removes the biggest lie (`ring_buffer_overflows` was always 0 while events dropped in the thousands); the remaining fields are either accurate today or filed as follow-ups.

**Alternatives considered**:
- **Alt A: Bundle events_dropped + bloom-filter fields into m212**. REJECTED per Q2; scope creep.
