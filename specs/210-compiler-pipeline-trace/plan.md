# Implementation Plan: Compiler-Pipeline eBPF Tracing (m210)

**Branch**: `210-compiler-pipeline-trace` | **Date**: 2026-07-19 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/210-compiler-pipeline-trace/spec.md`

## Summary

Extend the eBPF trace pipeline to capture per-compiler-invocation file read + write sets, build a parent-child compiler-invocation DAG in userspace, and emit per-component `mikebom:source-read-set` annotations attributing each output binary to the exact set of source files that contributed to it.

Technical approach: reuse the existing `mikebom-ebpf/src/programs/file_ops.rs` kprobes (`vfs_open`, `vfs_read`, `vfs_write`, `do_sys_openat2`) verbatim for read/write capture; add ONE new eBPF program at `mikebom-ebpf/src/programs/compiler_exec.rs` hooked to the `sched_process_exec` tracepoint that filters exec events by comm-field against a compiler whitelist and stamps a `is_compiler_descendant` flag on the per-PID tracking map so downstream file-op kprobes automatically scope their emissions. Build the compiler-invocation DAG + read/write set assembly in a new user-space aggregator at `mikebom-cli/src/trace/compiler_pipeline.rs`. Extend `mikebom-common::attestation::statement::BuildTracePredicate` with an ADDITIVE `compiler_pipeline: Option<CompilerPipelineData>` field (JSON-forward-compatible for pre-m210 consumers). Emit the per-component annotation via the existing per-format `extra_annotations` channel (matches m071 envelope shape).

Zero new Cargo dependencies. Linux-only. Gated behind the existing `ebpf-tracing` feature flag from milestone 020.

## Technical Context

**Language/Version**: Rust stable for user-space (workspace toolchain inherited from milestones 001–209); Rust nightly for the eBPF target via `aya-ebpf` (already required per m020). No new nightly features needed on top of what m020 pins.

**Primary Dependencies**: Existing only — `aya` (user-space eBPF loader; already in the dep graph behind the `ebpf-tracing` feature per m020), `aya-ebpf` (kernel-space; already in `mikebom-ebpf`), `aya-log` (logging; already), `sha2` (SHA-256 hashing at close-time; already pervasive), `serde`/`serde_json` (attestation serialization), `tracing`/`anyhow`/`thiserror` (error propagation + logs). **Zero new Cargo dependencies.**

**Storage**: N/A — compiler-invocation records + per-invocation read/write sets live in-process for the duration of a single trace. Emitted into the attestation JSON at trace-end. Matches every trace-mode milestone since 001.

**Testing**: `cargo +stable test --workspace --features ebpf-tracing` (feature-gated per FR-013). Unit tests for the DAG assembly + write-set-to-component mapping + trace-noise filter live in the mikebom-cli test suite (no CAP_BPF required — kernel side is mocked via constructed fixtures). Integration test for the full end-to-end trace runs under `MIKEBOM_PREPR_EBPF=1` locally + the dedicated `lint-and-test-ebpf` CI job per CLAUDE.md feature-flag section. Fixture project at `mikebom-cli/tests/fixtures/compiler_pipeline/two_binaries_diverge/` — two Rust binary targets sharing `libsafe` and diverging on `libvuln` per SC-001. Perf regression test at `mikebom-cli/tests/compiler_pipeline_perf.rs` — `#[ignore]`-gated per m094/m208 convention, asserts wall-clock overhead ≤ 15 % per FR-007/SC-003.

**Target Platform**: Linux only (per FR-014). Requires `CAP_BPF + CAP_PERFMON` (matches existing trace mode requirements). Non-Linux hosts: code path is `#[cfg(target_os = "linux")]`-gated (matches `mikebom-ebpf` crate footprint). Feature flag gates the whole surface out on default builds regardless of host OS.

**Project Type**: cli (extends the existing `mikebom-cli` + `mikebom-ebpf` + `mikebom-common` three-crate workspace per Principle VI; no new crate).

**Performance Goals**: Per SC-003, ≤ 15 % wall-clock overhead on the mikebom self-build (baseline ~30 s → traced ≤ 34.5 s). Achievable via: (a) in-kernel filtering by comm-field before ring-buffer emission (reject non-compiler-descendant events at zero cost), (b) trace-noise filter applied in-kernel where possible + userspace for the more complex glob patterns (secret-adjacent file extensions), (c) SHA-256 content hashes computed lazily at close-time in userspace (not in-kernel — BPF has strict complexity limits for crypto), (d) DAG assembly happens post-trace at emit time, not on every event.

**Constraints**: Byte-identical output on identical inputs (FR-011 → SC-004). Non-panic on any trace outcome (Principle III — ring-buffer overflow degrades gracefully per FR-008). Zero new Cargo deps. No CI regression on the default-lane build (feature-gate discipline per FR-013). Content-hash timing must handle files that get deleted between close + emit (edge case: build intermediates).

**Scale/Scope**: Typical build: ~500-5000 openat events + ~500-5000 read events + ~50-500 write events + ~10-100 exec events, over ~30 seconds. Ring buffer sized at existing m020 default (256 KB); heavy builds may overflow → degraded-completeness annotation per FR-008. Read-set cardinality per binary: typical ~50-500 source files; extreme (linux kernel builds) ~10k source files — the emitted annotation lists all of them per FR-006 no-truncation guarantee.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

**Principle I (Pure Rust, Zero C)** — PASS. All code is pure Rust. `mikebom-ebpf` is aya-based (no libbpf, no C). No new bindings. ✓

**Principle II (eBPF-Only Observation)** — **CORE MATCH**. This milestone is the HIGHEST-FIDELITY eBPF observation the trace pipeline will offer. No lockfile parsing, no manifest reading — every source-file attribution comes from a kernel-observed `openat` inside a whitelisted compiler process subtree. ✓

**Principle III (Fail Closed)** — PASS. Ring-buffer overflow → transparency annotation per FR-008 + SC-007 (not a fallback to lockfiles or heuristics). If `sched_process_exec` fails to attach, the trace fails-closed per existing m020 semantics. ✓

**Principle IV (Type-Driven Correctness)** — PASS. New event types `CompilerExecEvent`, `CompilerInvocation`, `CompilerPipelineData` use `#[repr(C)]` for kernel-userspace boundary crossing + strongly-typed `Purl`/`ContentHash` newtypes on the userspace side. No `.unwrap()` in production; `#[cfg(test)]` modules use the standard `#[cfg_attr(test, allow(clippy::unwrap_used))]` guard. ✓

**Principle V (Specification Compliance)** — PASS with Principle-V audit. The new `mikebom:source-read-set` + `mikebom:read-set-source` + `mikebom:secrets-read-filtered` + `mikebom:compiler-pipeline-completeness` + `mikebom:trace-attach-late` annotations are all justified per Principle V's parity-bridging clause: no standards-native carrier exists in CDX 1.6, SPDX 2.3, or SPDX 3.0.1 for "list of source files that contributed to this binary." Documented in `docs/reference/sbom-format-mapping.md` (new catalog rows C130–C134) with explicit "no native carrier" justification per Principle V. ✓

**Principle VI (Three-Crate Architecture)** — PASS. Extends `mikebom-ebpf` + `mikebom-common` + `mikebom-cli` only. No new crate. ✓

**Principle VII (Test Isolation)** — PASS. Unit tests + DAG-assembly + write-set-mapping tests run without root (in-memory event fixtures). eBPF integration test gated behind `MIKEBOM_PREPR_EBPF=1` + `CAP_BPF` — matches existing m020 discipline. ✓

**Principle VIII (Completeness)** — PASS. This milestone INCREASES completeness (more evidence captured). No loss of existing coverage. ✓

**Principle IX (Accuracy)** — PASS. This milestone DIRECTLY increases accuracy — source-to-binary attribution is the highest-fidelity attribution mikebom will emit. ✓

**Principle X (Transparency)** — PASS. 5 transparency signals for degraded / partial / masked cases: `compiler-pipeline-completeness` (ring-buffer overflow), `read-set-source = cache-hit` (compiler-cache hit), `trace-attach-late` (attach-after-start), `secrets-read-filtered` (secret-adjacent path filtered), `mikebom:stdin-input` (stdin-piped compiler). ✓

**Principle XI (Enrichment)** — N/A. No enrichment surface changes. ✓

**Principle XII (External Data Source Enrichment)** — N/A. No external data sources consulted. ✓

**Strict Boundaries** — all pass:
- #1 (No lockfile-based discovery): PASS — every source attribution is eBPF-observed.
- #2 (No MITM proxy): PASS — no network hooks changed.
- #3 (No C code): PASS.
- #4 (No `.unwrap()` in production): PASS.
- #5 (No file-tier duplicates in default mode): PASS — read-set annotation is per-component metadata, doesn't create new components.

**No violations.** Complexity Tracking table omitted.

## Project Structure

### Documentation (this feature)

```text
specs/210-compiler-pipeline-trace/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (attestor URI + annotation wire shapes)
├── checklists/
│   └── requirements.md  # From /speckit-specify (16/16 pass)
└── tasks.md             # Phase 2 output (via /speckit-tasks)
```

### Source Code (repository root)

Extension within the existing three-crate workspace (no new crate per Principle VI):

```text
mikebom-ebpf/
├── src/
│   ├── programs/
│   │   ├── compiler_exec.rs                  # NEW: sched_process_exec tracepoint;
│   │   │                                     #      filters by comm-field against
│   │   │                                     #      compiler whitelist; stamps
│   │   │                                     #      is_compiler_descendant on
│   │   │                                     #      per-PID tracking map
│   │   ├── file_ops.rs                       # EXTEND: existing vfs_open/vfs_read/
│   │   │                                     #      vfs_write kprobes gain a
│   │   │                                     #      compiler-descendant check that
│   │   │                                     #      tags events with
│   │   │                                     #      compiler_invocation_id
│   │   └── ... (existing programs unchanged)
│   ├── maps.rs                                # EXTEND: add COMPILER_INVOCATIONS
│   │                                          #      HashMap<pid, invocation_id> +
│   │                                          #      COMPILER_EXEC_EVENTS ring
│   │                                          #      buffer
│   └── ... (rest unchanged)

mikebom-common/
├── src/
│   ├── attestation/
│   │   ├── statement.rs                      # EXTEND: BuildTracePredicate gains
│   │   │                                     #      compiler_pipeline: Option<CompilerPipelineData>
│   │   │                                     #      (JSON-forward-compat additive)
│   │   ├── compiler_pipeline.rs              # NEW: CompilerPipelineData +
│   │   │                                     #      CompilerInvocation +
│   │   │                                     #      ReadSetEntry +
│   │   │                                     #      InvocationDagEdge types
│   │   └── witness.rs                        # EXTEND: emit compiler-invocation/v0.1
│   │                                         #      attestor entry in the witness
│   │                                         #      collection format
│   └── ... (rest unchanged)

mikebom-cli/
├── Cargo.toml                                # unchanged (zero new deps)
├── src/
│   ├── trace/
│   │   ├── mod.rs                            # EXTEND: register compiler_pipeline
│   │   │                                     #      module + wire aggregator
│   │   ├── compiler_pipeline.rs              # NEW: user-space aggregator —
│   │   │                                     #      builds DAG from exec events,
│   │   │                                     #      assembles read/write sets
│   │   │                                     #      per invocation, computes
│   │   │                                     #      SHA-256 content hashes at
│   │   │                                     #      close-time, applies trace-
│   │   │                                     #      noise + secrets filter,
│   │   │                                     #      maps write-set → SBOM
│   │   │                                     #      component via Q1 rule
│   │   ├── loader.rs                         # EXTEND: load + attach the new
│   │   │                                     #      sched_process_exec tracepoint
│   │   ├── processor.rs                      # EXTEND: dispatch new
│   │   │                                     #      COMPILER_EXEC event type to
│   │   │                                     #      compiler_pipeline module
│   │   └── ... (existing consumers unchanged)
│   ├── cli/
│   │   └── scan_cmd.rs                       # EXTEND: add
│   │                                         #      `--include-system-reads`
│   │                                         #      flag (bypasses FR-016
│   │                                         #      denylist)
│   ├── generate/
│   │   ├── cyclonedx/
│   │   │   └── metadata.rs                   # EXTEND: emit per-component
│   │   │                                     #      mikebom:source-read-set +
│   │   │                                     #      doc-scope
│   │   │                                     #      mikebom:compiler-pipeline-
│   │   │                                     #      completeness annotations
│   │   └── spdx/
│   │       ├── document.rs                   # EXTEND: same for SPDX 2.3
│   │       └── v3_document.rs                # EXTEND: same for SPDX 3
│   └── ... (rest unchanged)
└── tests/
    ├── compiler_pipeline_two_binaries.rs     # NEW: SC-001 acceptance
    ├── compiler_pipeline_reproducibility.rs  # NEW: SC-004 byte-identity
    ├── compiler_pipeline_exclusion.rs        # NEW: SC-005 file-removal
    ├── compiler_pipeline_secrets_filter.rs   # NEW: FR-016a secrets annotation
    ├── compiler_pipeline_perf.rs             # NEW: SC-003 perf regression (#[ignore])
    ├── compiler_pipeline_overflow.rs         # NEW: SC-007 degraded completeness
    └── fixtures/
        └── compiler_pipeline/
            ├── two_binaries_diverge/          # SC-001 fixture: two Rust binary
            │   │                              # targets, libsafe + libvuln
            │   ├── Cargo.toml
            │   ├── libsafe/src/lib.rs
            │   ├── libvuln/src/lib.rs
            │   └── binaries/
            │       ├── safe-only/src/main.rs
            │       └── vuln-included/src/main.rs
            ├── secrets_touch/                 # FR-016a fixture: build script
            │   │                              # reads ~/.aws/credentials
            │   └── ...
            └── stdin_input/                   # FR-018 fixture: `gcc -x c -`
                └── ...
```

**Structure Decision**: The three-crate architecture stays intact per Principle VI. `mikebom-ebpf` gains one new program file + one map-extension. `mikebom-common` gains one new attestation-types file + additive field on the existing predicate. `mikebom-cli` gains the user-space aggregator + CLI flag + per-format emitter augmentation + 6 integration tests + 3 fixture-project directories. Every existing consumer of `BuildTracePredicate` treats the new `compiler_pipeline` field as `Option<T>` and passes through unchanged when the trace ran without eBPF tracing enabled.

## Complexity Tracking

No constitution violations. Section deliberately empty — this milestone is the CANONICAL example of Principle II (eBPF-Only Observation) at its highest fidelity.
