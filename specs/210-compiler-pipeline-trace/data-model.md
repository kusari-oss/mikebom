# Data Model: Compiler-Pipeline eBPF Tracing (m210)

**Date**: 2026-07-19
**Purpose**: Enumerate every new struct/enum + delta to existing types. Every entity has one entry with kernel-vs-userspace boundary + validation rules.

## E1: `CompilerExecEvent` (NEW — kernel-userspace boundary type)

**Location**: `mikebom-common/src/attestation/compiler_pipeline.rs` (userspace-visible; corresponding kernel side in `mikebom-ebpf/src/programs/compiler_exec.rs` as a `#[repr(C)]` struct with matching layout).

**Shape** (Rust newtype hidden behind `#[repr(C)]`):

```rust
#[repr(C)]
#[derive(Clone, Debug)]
pub struct CompilerExecEvent {
    pub invocation_id: u64,       // monotonically-increasing, set in userspace
    pub pid: u32,
    pub ppid: u32,
    pub cgroup_id: u64,
    pub start_ts_ns: u64,          // CLOCK_MONOTONIC nanoseconds
    pub comm: [u8; 16],            // process comm-field (kernel-limited)
    pub argv0_hint_len: u16,       // if the argv[0] path was capturable, its length
    pub argv0_hint: [u8; 128],    // truncated argv[0] path (bpf_probe_read_user)
}
```

**Validation rules**:
- `comm` MUST match one of the R2 whitelist entries (matched in-kernel before emit).
- `argv0_hint` is best-effort (may truncate); userspace verifies full path via `/proc/<pid>/exe` fallback.
- `invocation_id` is assigned in userspace at ring-buffer receive time (kernel doesn't know the monotonic counter).

## E2: `CompilerInvocation` (NEW — the userspace-canonical representation)

**Location**: `mikebom-common/src/attestation/compiler_pipeline.rs`.

**Shape**:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompilerInvocation {
    pub invocation_id: u64,
    pub compiler: CompilerFamily,     // Rustc | Gcc | Clang | Go | Ld | Mold | Cc1 | Cpp | As | Unknown
    pub pid: u32,
    pub ppid: u32,
    pub parent_invocation_id: Option<u64>,  // None for the root of the DAG
    pub cgroup_id: u64,
    pub start_timestamp: Timestamp,
    pub end_timestamp: Option<Timestamp>,   // set on exec-exit event
    pub argv_full_path: Option<PathBuf>,    // resolved via /proc/<pid>/exe
    pub argv: Vec<String>,                  // full argv (captured at exec, best-effort)
    pub cwd: Option<PathBuf>,               // /proc/<pid>/cwd at exec time
    pub exit_code: Option<i32>,             // set on exec-exit event
    pub read_set: Vec<ReadSetEntry>,        // populated by userspace aggregator
    pub write_set: Vec<WriteSetEntry>,      // populated by userspace aggregator
    pub events_dropped: u64,                // per-invocation ring-buffer overflow counter
}
```

**Validation rules**:
- `parent_invocation_id.is_none()` iff `ppid` is not in `COMPILER_INVOCATIONS` (root of the DAG).
- `read_set` is sorted lex by path before emission (R8).
- `write_set` is sorted lex by path before emission.

## E3: `CompilerFamily` enum (NEW)

**Location**: `mikebom-common/src/attestation/compiler_pipeline.rs`.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilerFamily {
    Rustc,
    Gcc,
    Clang,      // clang, clang++
    Gpp,        // g++
    Go,         // go build, go tool compile
    Ld,
    Mold,
    Cc1,
    Cpp,
    As,
    Unknown,    // matched the comm-field prefilter but argv[0] didn't map cleanly
}
```

## E4: `ReadSetEntry` (NEW)

**Location**: `mikebom-common/src/attestation/compiler_pipeline.rs`.

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReadSetEntry {
    pub path: PathBuf,                   // absolute path as seen by the compiler
    pub sha256: ContentHash,             // hashed at close-time via existing hasher.rs
    pub kind: ReadKind,                  // File | StdinInput (per FR-018)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadKind {
    File,
    StdinInput { bytes_read: u64 },      // FR-018 — no path, just byte count
}
```

**Validation rules**:
- `path` MUST be absolute (kernel emits absolute paths via `bpf_d_path`).
- `sha256` MUST be a valid 64-hex ContentHash.
- Entries with `kind == StdinInput` have a synthetic `path = "<stdin>"` for emission consistency.

## E5: `WriteSetEntry` (NEW)

**Location**: `mikebom-common/src/attestation/compiler_pipeline.rs`.

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WriteSetEntry {
    pub path: PathBuf,
    pub sha256: Option<ContentHash>,     // None if the file was deleted before close-time hash
    pub survived_trace_window: bool,      // false for build intermediates that got deleted
}
```

**Validation rules**:
- `path` MUST be absolute.
- `sha256` populated only when the file existed at trace-end AND was hashed successfully.
- `survived_trace_window == false` implies the file was a build intermediate; still recorded for DAG assembly.

## E6: `CompilerPipelineData` (NEW — the root record)

**Location**: `mikebom-common/src/attestation/compiler_pipeline.rs`.

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompilerPipelineData {
    pub invocations: Vec<CompilerInvocation>,     // sorted by (start_timestamp, pid) for determinism
    pub dag_edges: Vec<InvocationDagEdge>,        // explicit edge list for DAG reconstruction
    pub completeness: CompletenessState,          // Complete | Degraded { dropped: u64 } | Partial { reason: AttachLate }
    pub secrets_read_filtered: u64,               // FR-016a counter
    pub include_system_reads_flag: bool,          // did the operator pass --include-system-reads?
    pub filter_categories_applied: Vec<FilterCategory>,  // System | UserCache | Ephemeral | SecretsAdjacent
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InvocationDagEdge {
    pub parent_invocation_id: u64,
    pub child_invocation_id: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CompletenessState {
    Complete,
    Degraded { dropped: u64, affected_component_count: usize },
    Partial { reason: PartialReason },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialReason {
    AttachLate,       // FR-017 — trace attached after some compilers had started
}
```

## E7: `BuildTracePredicate` extension (EXTENDED)

**Location**: `mikebom-common/src/attestation/statement.rs`.

**Delta**: Add ONE new borrowed field to the existing struct:

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildTracePredicate {
    pub metadata: TraceMetadata,
    pub network_trace: NetworkTrace,
    pub file_access: FileAccess,
    pub trace_integrity: TraceIntegrity,
    /// Milestone 210: compiler-pipeline data captured via
    /// sched_process_exec + descendant file-op tracing. `None` when
    /// the trace ran without `--features ebpf-tracing` OR the host
    /// wasn't Linux. When `Some(_)`, downstream emitters populate
    /// per-component `mikebom:source-read-set` annotations per FR-006.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler_pipeline: Option<CompilerPipelineData>,
}
```

**Backwards-compatibility**: every existing `BuildTracePredicate` construction site (5 in tests + 1 in production `trace_cmd.rs`) is updated to explicitly set `compiler_pipeline: None`. Pre-m210 attestation consumers deserialize `compiler_pipeline` as `None` — matches the m208 defensive-default pattern that preserved scan-mode golden byte-identity.

## E8: New eBPF maps (EXTENDED)

**Location**: `mikebom-ebpf/src/maps.rs`.

```rust
/// Milestone 210: PID → compiler_invocation_id map. Populated in-kernel
/// by the sched_process_exec tracepoint when a whitelisted compiler
/// starts + propagated to children via sched_process_fork. Consumed
/// by every file-op kprobe to stamp events with the invocation ID
/// (or drop them entirely if the current PID isn't a compiler
/// descendant).
#[map]
static COMPILER_INVOCATIONS: HashMap<u32, u64> = HashMap::with_max_entries(4096, 0);

/// Milestone 210: ring buffer for compiler-exec events. Separate
/// from the existing FILE_OPS ring buffer so overflow accounting
/// per FR-008 is per-event-type.
#[map]
static COMPILER_EXEC_EVENTS: RingBuf = RingBuf::with_byte_size(256 * 1024, 0);
```

## E9: `mikebom:source-read-set` annotation (NEW — per-component)

Per catalog row **C130** (new; documented in `docs/reference/sbom-format-mapping.md`).

**Wire shape** (JSON, byte-identical envelope across CDX / SPDX 2.3 / SPDX 3):

```json
{
  "invocation_ids": [<u64>, ...],
  "read_set": [
    { "path": "<abs-path>", "sha256": "<64-hex>", "kind": "file" },
    { "path": "<stdin>", "kind": { "stdin_input": { "bytes_read": <u64> } } },
    ...
  ],
  "read_set_source": "<see C131>"
}
```

**Emission carriers**:
- CDX 1.6: `components[].properties[]` with `name = "mikebom:source-read-set"`, `value = <json-envelope-string>`.
- SPDX 2.3: `packages[].annotations[]` entry with `annotator = "Tool: mikebom"`, `annotationType = "OTHER"`, `comment = <json-envelope-string>`.
- SPDX 3.0.1: `Annotation` element with `subject = <package-element-IRI>`, `statement = <json-envelope-string>`.

**Presence rules**: emitted when `ScanArtifacts.attestation.predicate.compiler_pipeline.is_some()` AND the component's file-path intersects at least one compiler-invocation write-set (per Q1 mapping rule). Never emitted for pre-m210 attestations (backward-compat).

## E10: `mikebom:read-set-source` annotation (NEW — per-component)

Per catalog row **C131**.

**Wire shape**: string enum:
- `"traced"` — read-set captured by an eBPF compiler-invocation observation.
- `"cache-hit"` — component served from a compiler cache (sccache/ccache); read-set omitted per FR-015.
- `"trace-attach-late"` — mikebom attached after this component's compiler started; read-set may be partial.
- `"unknown"` — component not mapped to any compiler-invocation (rare — usually a build-tool artifact not from the whitelisted family).

**Presence rules**: emitted on every component that would otherwise be a candidate for C130 (source-read-set). When value is `"traced"`, the C130 annotation is present. When `"cache-hit"` or `"unknown"`, C130 is OMITTED per FR-015. When `"trace-attach-late"`, C130 is present but marked with a `partial: true` sub-field.

## E11: `mikebom:compiler-pipeline-completeness` (NEW — doc-scope)

Per catalog row **C132**.

**Wire shape**:
- `"complete"` — no ring-buffer overflow + no attach-late.
- `"degraded"` — object with `state: "degraded"`, `dropped: <u64>`, `affected_component_count: <usize>`.
- `"partial"` — object with `state: "partial"`, `reason: "attach_late"`.

**Emission carriers**: same doc-scope shape as C125 from m208 (CDX `metadata.properties[]`, SPDX 2.3 `annotations[]`, SPDX 3 `Annotation` on document IRI).

## E12: `mikebom:secrets-read-filtered` (NEW — doc-scope)

Per catalog row **C133**.

**Wire shape**: string containing the u64 count of secret-adjacent reads filtered out during this trace (`"0"` when no filter triggered — but this annotation is only emitted when the count is > 0 per FR-016a).

**Presence rules**: emitted at document scope IFF `CompilerPipelineData.secrets_read_filtered > 0`.

## E13: `mikebom:trace-attach-late` (NEW — per-component)

Per catalog row **C134**.

**Wire shape**: literal string `"true"` on components whose compiler-invocation started BEFORE mikebom attached (FR-017).

**Presence rules**: emitted per-component when the invocation's `attach_state == LateAttach`.

## E14: `compiler-invocation/v0.1` witness attestor entry (NEW)

Per Clarifications Q3.

**Predicate URI**: `https://mikebom.dev/attestation/compiler-invocation/v0.1`.

**Wire shape** (embedded inside the witness `attestation-collection/v0.1` predicate's `attestations[]` array):

```json
{
  "type": "https://mikebom.dev/attestation/compiler-invocation/v0.1",
  "attestation": {
    "invocations": [<CompilerInvocation, ...>],
    "dag_edges": [<InvocationDagEdge, ...>],
    "completeness": <CompletenessState>
  },
  "subjects": [...]
}
```

**Emission**: added by `mikebom-common::attestation::witness::build_witness_collection` when `compiler_pipeline.is_some()`.

## Cross-reference to `docs/reference/sbom-format-mapping.md`

Catalog rows to register (numerically sorted per m071 convention, appended after existing rows):

| Row | Annotation | Directionality | Scope |
|---|---|---|---|
| C130 | `mikebom:source-read-set` | `SymmetricEqual` | Per-component |
| C131 | `mikebom:read-set-source` | `SymmetricEqual` | Per-component |
| C132 | `mikebom:compiler-pipeline-completeness` | `SymmetricEqual` | Document |
| C133 | `mikebom:secrets-read-filtered` | `SymmetricEqual` | Document |
| C134 | `mikebom:trace-attach-late` | `SymmetricEqual` | Per-component |

Each row gets a Rationale line per Principle V naming why no native carrier exists.
