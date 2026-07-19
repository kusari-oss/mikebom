# Research: Compiler-Pipeline eBPF Tracing (m210)

**Date**: 2026-07-19
**Phase**: 0 (pre-design)
**Purpose**: Resolve technical unknowns before Phase 1 design. Every open decision the plan surfaces gets a Decision / Rationale / Alternatives entry.

---

## R1: execve capture mechanism — tracepoint vs kprobe

**Decision**: `sched_process_exec` tracepoint attached via `#[tracepoint]` in aya-ebpf.

**Rationale**:
- **Stability**: `sched_process_exec` is a stable kernel tracepoint (unchanged ABI across kernels 4.19+); `execve` kprobes vary by kernel version (`__x64_sys_execve` on some, `do_execve` on others). Tracepoint = no per-kernel-version code path.
- **Timing**: fires AFTER exec succeeds — at the point the new binary is in place. Perfect for capturing the compiler's identity (comm-field, argv are in `bprm`).
- **Overhead**: tracepoints are slightly cheaper than kprobes (attached at compile time of the kernel, no dynamic patching).
- **Aya support**: `aya_ebpf::macros::tracepoint` + `TracePointContext` are stable + well-documented; matches the existing `mikebom-ebpf` pattern.

**Alternatives considered**:
- **kprobe on `execve`** (or `do_execve`): rejected — kernel-version-fragile.
- **`sched_process_fork` + `sched_process_exec`** (both): overkill — fork doesn't tell us the comm; exec is when the new binary settles. Fork tracking is handled by our per-PID map anyway.
- **`raw_tracepoint/sched_process_exec`**: slightly faster but harder to write (raw arg tuples); the small perf gain doesn't justify the complexity.

---

## R2: Compiler whitelist match strategy — comm-field vs full path

**Decision**: Match against the process's `comm` field (16-byte kernel identifier) in-kernel; verify against the full `argv[0]` path in userspace for high-fidelity attribution.

**Rationale**:
- **Kernel constraint**: BPF programs have limited string-manipulation primitives + the `comm` field is fixed 16-byte; matching against full path requires a userspace string-compare or a `bpf_d_path` helper call (which has kernel-version dependencies).
- **Two-stage match**: kernel-side comm-field prefilter drops 99 %+ of non-compiler execs at zero user-space cost. Userspace second-pass verifies the FULL argv path (handles `~/toolchain/rustc` cases per Edge Cases).
- **Whitelist scope**: `rustc`, `gcc`, `clang`, `g++`, `clang++`, `go`, `ld`, `mold`, `cc1`, `cpp`, `as` — all fit in 16 bytes. Longer variants like `x86_64-linux-gnu-gcc-13` DON'T — they truncate to `x86_64-linux-gn` which we treat as a match candidate + verify in userspace.

**Alternatives considered**:
- **Full-path match in-kernel via `bpf_d_path`**: rejected — kernel 5.9+ dependency, complex verifier discipline; two-stage match is simpler.
- **Match on argv[0] via `bprm` (kernel-side)**: rejected — argv is user-pointer at exec time; reading requires `bpf_probe_read_user`, verifier-heavy, has per-kernel struct-layout issues.
- **User-space-only match (no in-kernel filter)**: rejected — every process's exec event lands in userspace, blowing the perf budget on high-fork builds.

---

## R3: PID ancestry tracking — how to scope file-op events to a compiler's descendant tree

**Decision**: Reuse the existing `mikebom-cli/src/trace/pid_tracker.rs` infrastructure from m020; extend `mikebom-ebpf/src/maps.rs` with a new `COMPILER_INVOCATIONS: HashMap<pid_t, invocation_id>` map keyed on the compiler-invocation root PID. On every `sched_process_fork`, propagate the parent's `invocation_id` to the child (in-kernel). The existing file-op kprobes get a new prelude: check if `pid_of_current_process()` is a key in `COMPILER_INVOCATIONS`; if yes, stamp the file-op event with the invocation_id before ring-buffer emit.

**Rationale**:
- **In-kernel propagation via `sched_process_fork` tracepoint**: fork events already fire; adding one map lookup per fork is negligible cost.
- **Zero user-space overhead for out-of-scope events**: file-op events that don't have a compiler-invocation parent don't reach the ring buffer.
- **Reuses existing pid_tracker**: user-space assembles the (pid, invocation_id) → parent-invocation-id linkage into the DAG.
- **Handles late-attached tracers correctly**: if we attach mid-build, existing compiler PIDs aren't in `COMPILER_INVOCATIONS`; their subsequent forks won't be tracked either, which is the desired "attach-late = missing early events" semantic per FR-017.

**Alternatives considered**:
- **User-space ancestry reconstruction from `sched_process_fork` events alone**: works but every fork event has to reach userspace; heavy overhead on parallel builds.
- **cgroup-based scoping** (attach BPF to a specific cgroup): elegant on Linux 4.10+; rejected for MVP because it requires the operator to place the build in a specific cgroup — friction. Follow-up milestone can add opt-in.
- **PID-namespace-based scoping**: same friction as cgroup; deferred.

---

## R4: Content-hash timing — when is `sha256(file)` computed

**Decision**: SHA-256 computed **in userspace** at file-close time via the existing `mikebom-cli/src/trace/hasher.rs` infrastructure. In-kernel hashing is out — BPF has strict complexity limits + no crypto primitives.

**Rationale**:
- **Existing hasher**: `hasher.rs` already computes SHA-256 for file-access events at write-close boundary; we reuse it verbatim for read-close events.
- **Race-window bounds**: hashing at close-time captures the file content the compiler ACTUALLY consumed (post-open, pre-close). A file modified DURING open→close is a well-known TOCTOU class beyond our scope; documented in FR-011's "byte-identical for byte-identical inputs" caveat.
- **Deleted intermediates**: some files (build scratch, temp object files) get deleted immediately after close. We hash at close-time before the deletion → correct.
- **Perf**: hashing happens on the userspace side asynchronously; doesn't block the trace's critical path.

**Alternatives considered**:
- **In-kernel hashing via `bpf_probe_read` + a rolling SHA state**: rejected — verifier complexity + no SHA-256 primitive in BPF stdlib.
- **Hash at emit-time (trace end)**: rejected — deleted intermediates would lose their content.
- **Hash at open-time**: race-prone (a write between open + our hash would corrupt the hash-vs-consumed-content invariant).
- **Skip hashes; emit paths only**: rejected — FR-006 explicitly requires `{path, sha256}` tuples for downstream reachability tooling.

---

## R5: Trace-noise filter application layer — in-kernel or userspace

**Decision**: **Prefix-match denylist in-kernel** for the system + cache + tmp categories (`/etc/`, `/proc/`, `/sys/`, `/dev/`, `/tmp/`, `/var/tmp/`, `~/.cache/`, `~/.local/share/`); **glob + basename-heuristic denylist in userspace** for the secrets category (`/var/run/secrets/*`, `~/.ssh/*`, etc. + `*.pem`, `*.key`, `*.crt`, `*_rsa`, `*_ed25519`).

**Rationale**:
- **Kernel-side prefix match**: cheap + effective — kernel prefix-compare against a small `const` array eliminates 80 %+ of noise events at zero user-space cost. Aya-ebpf supports this via `bpf_probe_read_kernel_str` + inline compare.
- **User-side glob + heuristic**: too complex for BPF (variable-length glob compare, per-user-home path expansion). Perf hit is small because kernel-side already dropped the loud paths.
- **FR-016a counter**: incrementing a per-scan counter for filtered secret-adjacent paths is trivial in userspace; the counter feeds the `mikebom:secrets-read-filtered` doc-scope annotation.
- **`--include-system-reads` bypass**: sets a config flag that userspace consults; kernel filter still runs but userspace re-includes filtered events by matching the raw ring-buffer stream (kernel doesn't know about the CLI flag).

**Alternatives considered**:
- **All filtering in userspace**: every file-op event reaches userspace → perf hit on high-syscall builds.
- **All filtering in kernel**: complex glob matching in BPF is impractical + `$HOME` expansion isn't kernel-visible.
- **Post-emit filter (userspace filters the final annotation payload)**: works but wastes ring-buffer capacity on noise.

---

## R6: BuildTracePredicate backward compatibility

**Decision**: Add `compiler_pipeline: Option<CompilerPipelineData>` as a new `#[serde(skip_serializing_if = "Option::is_none")]` field on `BuildTracePredicate`. Pre-m210 attestation consumers deserialize it as `None`; the field simply doesn't appear in the emitted JSON when the trace was run without eBPF tracing (or non-Linux hosts).

**Rationale**:
- **JSON forward-compat**: `serde_json` ignores unknown fields by default; existing consumers deserialize successfully.
- **`Option<T>` + skip-if-none**: keeps the emitted JSON identical to pre-m210 output when the field is absent, preserving byte-identity of existing golden fixtures (SC-001 equivalent for other milestones' regression tests).
- **Explicit pass-through**: every existing `BuildTracePredicate` construction site (there are ~5 in tests + 1 in production) is updated to set `compiler_pipeline: None` unless the trace has compiler-pipeline data. This is the m208 defensive-default pattern.

**Alternatives considered**:
- **Nested inside `file_access`**: rejected — muddies the schema; `compiler_pipeline` is a peer of `network_trace` + `file_access`, not a subordinate.
- **New separate predicate URI (`build-trace-v2`)**: rejected — v1 stays backward-compat; v2 bump would break every existing consumer.
- **Optional-by-cargo-feature (only compile the field when `ebpf-tracing` is on)**: rejected — the field NEEDS to deserialize on non-eBPF-tracing consumers (SBOM emitters running on a pre-recorded attestation from an eBPF host).

---

## R7: Ring-buffer overflow detection specifically for the new event type

**Decision**: Extend the existing per-map ring-buffer overflow counter (from m020's `TraceIntegrity.events_dropped`) with a new `compiler_events_dropped: u64` field on the `CompilerPipelineData` root. When the new `COMPILER_EXEC_EVENTS` ring buffer overflows, increment this specific counter; when any file-op event that would have been stamped with a `compiler_invocation_id` is dropped, increment it too.

**Rationale**:
- **Separate counter**: consumers care about "was my source-read-set complete?" specifically; a mixed overall-events-dropped counter can't distinguish "network events dropped" (irrelevant to source-read-set) from "compiler file-op events dropped" (directly affects source-read-set).
- **FR-008 mapping**: when `compiler_events_dropped > 0`, emit `mikebom:compiler-pipeline-completeness = "degraded"` with the drop-count in the value. When `== 0` AND `attach_late == false`, emit `= "complete"`. When `attach_late == true`, emit `= "partial"`.
- **Per-invocation dropped-count aggregation**: userspace aggregator tracks per-invocation-id drop counts too; components whose invocation had drops get an additional per-component annotation.

**Alternatives considered**:
- **Reuse `TraceIntegrity.events_dropped`**: coarse; can't distinguish network vs. compiler drops.
- **Rely on ring-buffer's built-in `bpf_ringbuf_reserve` failure**: aya-ebpf reports these; we track them explicitly.

---

## R8: Deterministic ordering + hash-stability rules for `source-read-set` emission

**Decision**: **Read-set entries sorted lexicographically by path (byte-order, not locale-aware)**. Ties (same path — impossible in practice, but defensive) broken by sha256 ordering. **DAG traversal ordering deterministic** — parent invocations ordered by (start_timestamp_ns, pid) tuple.

**Rationale**:
- **Byte-order sort**: reproducible across locale settings + across mikebom versions.
- **No ties in practice**: the read-set is a set (uniqueness by path); ties would indicate a bug elsewhere.
- **DAG ordering matters for the write-set-to-component matching**: a component's ancestor-invocation chain is deterministic iff the DAG walk order is deterministic.

**Alternatives considered**:
- **Sort by content-hash first, then path**: less useful for humans reading the annotation; rejected.
- **Insertion-order preservation**: not deterministic (depends on ring-buffer read order which varies across runs).

---

## R9: Async-in-sync trace event dispatch (relates to spec FR-013 mention)

**Decision**: No async-in-sync problem — the trace pipeline is fully async via tokio (matches existing m001–m020 pipeline). The new `compiler_pipeline.rs` aggregator is `async fn`, consumes ring-buffer events via the existing `RingBuf::poll` interface, and dispatches per-event work via `tokio::spawn` where useful (SHA-256 hashing is CPU-bound + gets offloaded to a `spawn_blocking` pool per existing hasher.rs convention).

**Rationale**: preserves the existing pipeline shape.

**Alternatives considered**: none warranted; this decision is compatible with the existing pipeline.

---

## R10: Handling non-Linux hosts + `--features ebpf-tracing` OFF

**Decision**: The `compiler_pipeline: Option<CompilerPipelineData>` field is present in the compiled binary UNCONDITIONALLY (needed for deserialization on any host reading a pre-recorded attestation). The `#[cfg(feature = "ebpf-tracing", target_os = "linux")]`-gated code that POPULATES the field lives in `mikebom-ebpf` + gated modules of `mikebom-cli/src/trace/compiler_pipeline.rs`. On non-Linux OR default-features builds, the field is always `None`.

**Rationale**:
- **Deserialize everywhere, populate only on Linux+feature**: matches the existing pattern for every eBPF-derived attestation field since m020.
- **Non-Linux SBOM emitters can consume attestations from Linux hosts**: important for cross-platform CI pipelines that generate on Linux + verify on macOS.

**Alternatives considered**:
- **Feature-gate the field itself**: rejected — breaks non-Linux consumption of Linux-produced attestations.
