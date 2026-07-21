# Contract: Counter semantics for `ring_buffer_overflows`

**Milestone**: 212
**Date**: 2026-07-20
**Purpose**: Lock the semantic meaning of `TraceIntegrity.ring_buffer_overflows` post-m212 so operators and downstream tooling can rely on a stable interpretation.

## C-1: What `ring_buffer_overflows` counts

**Assertion**: `ring_buffer_overflows` is the **sum of `RingBuf::reserve() → None` occurrences across all three eBPF ring buffers × all online CPUs**, measured for the duration of a single trace.

**Includes**:
- `FILE_EVENTS.reserve::<FileEvent>(0) → None` at any of the 4 file_ops.rs sites
- `NETWORK_EVENTS.reserve::<NetworkEvent>(0) → None` at any of the 3 network-emitting sites (tcp_connect + 2× tls_openssl)
- `COMPILER_EXEC_EVENTS.reserve::<CompilerExecEvent>(0) → None` at either of the 2 compiler_exec.rs sites

**Excludes**:
- Events that reached the ring buffer but were dropped by userspace (see [waybill#618](https://github.com/kusari-oss/waybill/issues/618) `events_dropped` follow-up).
- Events dropped in-kernel by `should_trace()` filtering (this is by-design, not a drop).
- Events dropped in-kernel by content-hash bloom-filter dedup (this is by-design intra-trace dedup, not a drop).

## C-2: What "counter unavailable" looks like

**Assertion**: When any of the three per-CPU counter maps fails to load or attach (older kernel, unusual configuration), post-m212 mikebom:
1. Emits a WARN log line under 500 bytes containing the map name + reason.
2. Reports `ring_buffer_overflows: <sum of the maps that DID load>` (partial count; per FR-005).
3. Appends the failing map's short name (`"file_event_drops"`, `"network_event_drops"`, or `"compiler_exec_drops"`) to `TraceIntegrity.kprobe_attach_failures[]` per Clarification Q3.

**Consumer contract**: attestation consumers who trust the transparency signal MUST check `kprobe_attach_failures[]` for any entry matching `*_drops$` before treating `ring_buffer_overflows == 0` as "no drops occurred." If a `*_drops` entry appears, the reported value is a lower bound (only the maps that did load contribute), not a complete count.

## C-3: Ordering guarantee

**Assertion**: `TraceIntegrity.kprobe_attach_failures[]` is sorted lexicographically at emission time. Consumers can rely on stable ordering across runs on the same host.

**Rationale**: FR-003 byte-identity relies on deterministic ordering. Pre-m212 the array only carried real kprobe attach failures which happened to be attached in a stable order; post-m212 with counter-map names mixed in, explicit sorting locks the invariant.

## C-4: Threshold contract for m211 SC-001 fixture

**Assertion**: On the m211 SC-001 fixture (`mikebom-cli/tests/fixtures/compiler_pipeline/two_binaries_diverge`) built via the m211 `Dockerfile.ebpf-test` container harness on Colima Linux 6.8 aarch64, `ring_buffer_overflows` is expected to be `> 100`. The value is host-dependent (compose stack activity affects it) — the threshold is a lower bound to prove the counter is incrementing.

**Regression guard**: `scripts/ebpf-integration-test.sh` extends the m211 assertions with `jq -e '.predicate.trace_integrity.ring_buffer_overflows > 100'`. If this ever fails on a real-fixture trace, either the counter regressed or the drop-rate signature of the fixture changed materially (both warrant investigation).

## C-5: Hermetic-command floor

**Assertion**: `mikebom trace run --attestation-format mikebom-v1 -- true` on any supported host produces an attestation with `ring_buffer_overflows == 0` and `kprobe_attach_failures[]` NOT containing any `*_drops` entry.

**Regression guard**: unit test in `mikebom-cli/src/trace/loader.rs::tests` OR a lightweight integration test at `mikebom-cli/tests/trace_hermetic_zero_drops.rs`, gated behind the same `#[cfg(all(target_os = "linux", feature = "ebpf-tracing"))]` used by m211 integration tests.

**Failure modes if regressed**:
- Value goes non-zero → the counter is spuriously incrementing on a zero-syscall command → bug in the eBPF increment site (e.g. always fires regardless of reserve outcome).
- `kprobe_attach_failures[]` contains a `*_drops` entry → the counter map isn't loading on a fully-supported kernel → verifier regression or wrong map type.

## C-6: Type stability

**Assertion**: `TraceIntegrity.ring_buffer_overflows: u64` — type unchanged from pre-m212. Downstream consumers reading the field as u64 continue to work byte-identically. Serde output shape unchanged.

## C-7: Future extensibility

**Assertion**: The internal `HashMap<&'static str, u64>` returned by `read_ring_buffer_drops` preserves per-map counts even though m212 only exposes the sum. A future milestone that revives US3 (per-map wire-visible breakdown) can consume that HashMap directly without changes to the aggregation helper.
