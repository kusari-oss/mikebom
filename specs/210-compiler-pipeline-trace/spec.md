# Feature Specification: Compiler-Pipeline eBPF Tracing (MVP: rustc + gcc/clang + go build)

**Feature Branch**: `210-compiler-pipeline-trace`
**Created**: 2026-07-19
**Status**: Draft
**Input**: User description: "compiler-pipeline eBPF tracing for rustc + gcc/clang + go build (MVP). Hook execve of a compiler whitelist; capture per-invocation read + write sets scoped to the compiler process subtree; build a parent-child compiler-invocation DAG in userspace. Extend the BuildTracePredicate with a compiler_pipeline section AND add a compiler-invocation v0.1 attestor entry to the witness-collection format. Emit per-component `mikebom:source-read-set` annotation as list of (path, sha256) tuples. Linux only; gated on the ebpf-tracing feature flag."

## Clarifications

### Session 2026-07-19

- Q: How does mikebom map a compiler invocation's write-set to a specific SBOM component? → A: **By output file path.** For each SBOM component that has a known file path (via milestone-133 file-tier evidence OR the component's `hashes[]`-anchoring file), the mapping is a set-intersection between the component's file path(s) and every compiler invocation's write-set. Component X gets attributed the read-set of every compiler invocation whose write-set contains X's file, PLUS the transitive-closure over the compiler-invocation DAG (ancestor invocations that wrote files consumed by descendant invocations that wrote X). Deterministic + zero heuristics.
- Q: How should mikebom handle secret-containing files in the read-set? → A: **Extend FR-016's hardcoded denylist with secret-adjacent paths.** Filtered by default; the existing `--include-system-reads` escape hatch bypasses for auditing. Denylist includes: `/var/run/secrets/*`, `/run/secrets/*`, `/run/keys/*`, `~/.ssh/*`, `~/.aws/*`, `~/.gnupg/*`, `~/.docker/config.json`, `~/.netrc`, `~/.kube/config`, plus any file whose path segment matches `*.pem`, `*.key`, `*.crt`, `*_rsa`, `*_ed25519` (heuristic key-file extension match). Same fail-closed instinct as the existing FR-016 filter.
- Q: What predicate URI + ownership model for the `compiler-invocation/v0.1` attestor entry? → A: **Mikebom-owned URI** `https://mikebom.dev/attestation/compiler-invocation/v0.1`. Documented in `docs/architecture/attestations.md`. No upstream coordination overhead; matches how the existing `build-trace/v1` predicate handled itself. Future migration to a `https://in-toto.io/attestation/compiler-invocation/*` URI is possible via URI alias without breaking downstream consumers.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Attribute a specific vulnerability to specific source files (Priority: P1)

A security engineer receives an advisory about a vulnerable function in an OSS library. Today mikebom's SBOM tells them "your build used version X of the library" — but not whether the vulnerable function was actually compiled into any output binary. With the compiler pipeline trace, the emitted SBOM carries per-component source-read-set metadata: for each binary artifact, the exact list of source files (path + content hash) that contributed to it. The engineer can then filter the advisory: "did the vulnerable file appear in the read-set of any of my binaries?" If yes → real exposure. If no → dead-code-elimination or intentional exclusion made this binary safe.

**Why this priority**: This is the atomic capability the whole feature exists for. Every downstream vulnerability-correlation and reachability-analysis tool wants this signal today; nobody has it. Without US1, the milestone delivers no user-visible value.

**Independent Test**: Build a fixture project containing (a) a binary that uses only file `safe.rs`, and (b) a binary that uses both `safe.rs` + `vuln.rs`. Assert the emitted SBOM's per-component annotations correctly attribute `vuln.rs` ONLY to binary (b). Then remove `vuln.rs` from the input tree, rebuild, and re-assert that no binary in the new SBOM has `vuln.rs` in its read-set (exclusion invariant).

**Acceptance Scenarios**:

1. **Given** a build that compiles `foo.c` + `bar.c` into `foo-server`, **When** the operator runs `mikebom trace run -- gcc foo.c bar.c -o foo-server`, **Then** the emitted SBOM's `foo-server` component carries a `mikebom:source-read-set` annotation listing both `foo.c` and `bar.c` with their content-hash values.
2. **Given** a Rust workspace with two binary targets that share some crates and diverge on others, **When** mikebom traces `cargo build --release`, **Then** each binary's SBOM component carries a distinct source-read-set reflecting only the crates actually compiled into that binary.
3. **Given** a source file NOT present in the input tree, **When** mikebom traces the build, **Then** the file is NOT in any output component's source-read-set.

---

### User Story 2 — Verify build reproducibility beyond byte-identity (Priority: P2)

A reproducibility engineer wants to confirm two builds are equivalent. Byte-identical output is one signal, but a stronger signal is source-identical: same read set → same binary. Byte-identity can succeed under adversarial conditions (e.g., a compromised toolchain that produces stable-looking output from different inputs); source-identity catches that. With this feature, the engineer compares source-read-set annotations across two builds and gets a diff of which source files changed between runs.

**Why this priority**: Amplifies the reproducibility posture mikebom already offers with deterministic emission. Not a blocker for US1 to be useful, but transforms the reproducibility story from "trust our determinism" to "here's the exact input set — compare yourself."

**Independent Test**: Run the same build twice against a fixture project; assert both runs' source-read-set annotations are byte-identical (identical paths, identical content hashes, identical ordering). Then modify one source file and rebuild; assert the delta appears in the source-read-set of exactly the binaries that consume that file.

**Acceptance Scenarios**:

1. **Given** two consecutive builds of the same source tree with no changes, **When** mikebom traces both and emits SBOMs, **Then** both SBOMs' source-read-set annotations are byte-identical.
2. **Given** a source file whose content changes between two builds, **When** mikebom traces both, **Then** the changed file's content-hash differs between the two SBOMs — and only in the binaries that consume it.

---

### User Story 3 — Downstream reachability tooling filters vulnerabilities by compiled source (Priority: P3)

A SBOM consumer builds a vulnerability-correlation pipeline. Today they get "your build used package X, which has CVE Y" — high false-positive rate because CVE Y may be in a code path never compiled into the operator's binaries. With the source-read-set signal, the pipeline can gate: "does binary Z have any file matching the CVE's affected-file pattern in its source-read-set?" This turns coarse-grained pkg-level VEX into fine-grained file-level VEX.

**Why this priority**: Ecosystem impact — enables an entire class of downstream tooling that doesn't exist today. Consumers (Trivy, Grype, Snyk, in-house pipelines) can start acting on file-level attribution once mikebom's emit format carries it. Not blocked on any other US, but no user pain if this ships in a follow-up.

**Independent Test**: A downstream tool parses a mikebom SBOM, reads each binary component's `mikebom:source-read-set`, and filters an advisory list by whether the advisory's known-affected files match. Assert the filter reduces false positives measurably on a fixture advisory set.

**Acceptance Scenarios**:

1. **Given** an advisory naming `libfoo/vuln.c` as the vulnerable file, **When** a downstream tool parses the SBOM emitted by mikebom for a build that imported `libfoo` but only compiled `libfoo/safe.c` into the output, **Then** the advisory can be marked "not reachable" for that binary.
2. **Given** the source-read-set annotation shape is stable across mikebom releases, **When** downstream tools depend on it, **Then** field name + value structure remain byte-consistent (interop contract).

---

### Edge Cases

- **Heavy-parallel builds** (make -j64, cargo -j32): the trace must capture events from every parallel child; ring-buffer overflow becomes a completeness gap and MUST be signaled per Principle X.
- **mmap-based file access**: some compilers (LLVM's linker, some Go tools) read files via mmap rather than read(). The trace MUST capture these via openat/openat2 hooks (mmap requires a prior open); read(2) tracing is not required.
- **Non-standard compiler locations**: compilers installed at `~/toolchain/rustc`, `/opt/rh/gcc-toolset-13/bin/gcc`, or via nix profiles. The whitelist MUST match by basename (comm-field) not absolute path.
- **Hermetic sandboxing**: build tools like Bazel run compilers inside a symlink forest. The captured paths reflect the sandbox view (symlink paths), which is what downstream tools expect for reproducibility.
- **Cross-compilation**: `rustc --target x86_64-unknown-linux-musl` on an aarch64 host still emits binaries; the trace attributes source files to the emitted (target-arch) binary, not the host-arch metadata.
- **Compiler cache hits (sccache, ccache, mold cache)**: when a cache serves an artifact, no compiler process runs → no read-set can be captured for that artifact. The emitted SBOM MUST mark cache-served components with a `mikebom:read-set-source = "cache-hit"` transparency annotation and OMIT the source-read-set annotation for those components (compiler-cache tracing is deferred to a follow-up milestone).
- **Distributed compilation** (distcc, icecc): the actual compile happens on a remote host outside the traced process tree. Falls under the same cache-hit convention: no read-set captured → transparency annotation.
- **Compiler that doesn't fork before exec** (some Go internal tools): parent-pid chaining still works via ppid tracking; no special handling required.
- **Trace attaches AFTER the compiler starts** (attach-to-existing-PID mode): early events are missed; MUST be signaled per Principle X with `mikebom:trace-attach-late` transparency annotation.
- **Compiler reads from stdin** (`gcc -x c -` piped input): the read-set for that invocation is empty on the FS side; the invocation MUST include a `mikebom:stdin-input` marker with the read-count of bytes rather than a `{path, sha256}` tuple.
- **Trace-noise files**: compilers routinely read `/etc/hosts`, `/proc/self/status`, `~/.cache/*`, `/dev/urandom`, temp files. Applying a hardcoded exclusion pattern (system + cache + temp paths) is required to keep the source-read-set signal dense; an operator-configurable escape hatch (`--include-system-reads`) MUST be available for auditing.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The trace pipeline MUST capture every `execve` of a process whose basename matches the compiler whitelist.
- **FR-002**: The compiler whitelist MUST include at minimum: `rustc`, `gcc`, `clang`, `g++`, `clang++`, `go` (when invoked with `build` or `tool compile` as subcommand), `ld`, `mold`, `cc1`, `cpp`, `as`.
- **FR-003**: For each captured compiler invocation, the trace MUST capture the read set — every file the invocation OR any of its descendant processes opened via `openat`/`openat2` for reading during the invocation's lifetime.
- **FR-004**: For each captured compiler invocation, the trace MUST capture the write set — every file the invocation OR any of its descendants wrote AND closed during the invocation's lifetime.
- **FR-005**: The trace MUST build a parent-child directed acyclic graph linking compiler invocations by (parent-pid, cgroup) ancestry, producing a compiler-invocation DAG whose root is the top-level build command and whose leaves are individual compiler/linker invocations.
- **FR-006**: For each SBOM component that carries a known output file path (via milestone-133 file-tier evidence OR the component's file-anchoring `hashes[]` entry), mikebom MUST match the component's file path(s) against every compiler invocation's write-set (per Clarifications Q1). Matched components MUST receive a per-component `mikebom:source-read-set` annotation carrying a deterministically-ordered list of `{path, sha256}` tuples for every file in that component's TRANSITIVE read-set — the union of the matching invocation's own read-set PLUS the read-sets of every ancestor invocation in the compiler-invocation DAG that wrote a file the descendant invocation consumed. Trace-noise paths (FR-016) are excluded from the emitted list.
- **FR-007**: Wall-clock overhead of the trace MUST NOT exceed 15 % on a moderately-sized real-world build (defined as: building mikebom-cli itself from a warm cargo cache, ~30 seconds baseline).
- **FR-008**: When the ring buffer overflows OR events are otherwise dropped, mikebom MUST emit a document-scope `mikebom:compiler-pipeline-completeness = "degraded"` transparency annotation naming the drop-count + the affected component-count, satisfying Principle X.
- **FR-009**: The compiler-invocation event MUST also appear as a `compiler-invocation/v0.1` attestor entry within the witness-collection attestation format (alongside the existing `material`/`command-run`/`product`/`network-trace` entries per feature 006), so downstream consumers of witness attestations can read it without needing mikebom-native format support. Per Clarifications Q3, the predicate URI is mikebom-owned: `https://mikebom.dev/attestation/compiler-invocation/v0.1`. Downstream consumers wanting to identify + parse this entry look up that URI. Future migration to a `https://in-toto.io/attestation/*` namespace is preserved via URI-alias support.
- **FR-010**: The native `BuildTracePredicate` MUST gain a `compiler_pipeline` section containing the compiler-invocation DAG + per-invocation read/write sets, positioned alongside the existing `network_trace` and `file_access` sections.
- **FR-011**: For byte-identical input builds, the emitted `mikebom:source-read-set` annotations MUST be byte-identical across runs (deterministic ordering, deterministic content-hash values, deterministic path formatting).
- **FR-012**: Cross-compilation MUST be supported — when the compiler emits artifacts for a different architecture than the host, the source-read-set MUST attribute to the emitted (target-arch) artifact, not to a synthetic host-arch metadata component.
- **FR-013**: The feature MUST be gated behind the existing `ebpf-tracing` Cargo feature flag from milestone 020 — default-off in the shipped binary, opt-in via `--features ebpf-tracing`, no CI regression on the default-lane build.
- **FR-014**: The feature MUST be Linux-only. On non-Linux hosts, the code path MUST be compile-time excluded (matching the existing `mikebom-ebpf` crate's Linux-only footprint per Principle VI).
- **FR-015**: Components served by a compiler cache (sccache, ccache, or equivalent) MUST carry a `mikebom:read-set-source = "cache-hit"` transparency annotation and MUST OMIT the source-read-set annotation for that component (compiler-cache tracing is deferred to a follow-up milestone).
- **FR-016**: The trace MUST filter out a hardcoded set of trace-noise paths from the emitted read-sets. The denylist covers three categories:
   - **System / kernel** paths: `/etc/*`, `/proc/*`, `/sys/*`, `/dev/*`
   - **User cache / ephemeral** paths: `~/.cache/*`, `~/.local/share/*`, `/tmp/*`, `/var/tmp/*`, `$TMPDIR/*`
   - **Secret-adjacent** paths (per Clarifications Q2): `/var/run/secrets/*`, `/run/secrets/*`, `/run/keys/*`, `~/.ssh/*`, `~/.aws/*`, `~/.gnupg/*`, `~/.docker/config.json`, `~/.netrc`, `~/.kube/config`, PLUS any file whose basename matches `*.pem`, `*.key`, `*.crt`, `*_rsa`, `*_ed25519` (heuristic key-file extension match)
   
   An operator-configurable CLI flag (`--include-system-reads`) MUST bypass ALL THREE filter categories for auditing purposes.
- **FR-016a**: When at least one read is filtered due to the secret-adjacent-paths denylist, the emitted SBOM MUST carry a document-scope `mikebom:secrets-read-filtered = "<count>"` transparency annotation naming the count of filtered reads. Signals to the operator that the build touched secret paths (auditable evidence) without leaking WHICH paths.
- **FR-017**: When the trace attaches to an ALREADY-RUNNING compiler process (attach-to-existing-PID mode), early events preceding attach are unavailable; mikebom MUST emit a `mikebom:trace-attach-late` transparency annotation on affected components.
- **FR-018**: A compiler that reads input from stdin instead of the filesystem MUST NOT crash the trace; the affected invocation's read-set MUST include a `mikebom:stdin-input` marker with the read-count of bytes rather than a `{path, sha256}` tuple.

### Key Entities *(include if feature involves data)*

- **Compiler invocation**: A single `execve` of a whitelisted compiler binary, uniquely identified by (pid, start-timestamp, cgroup-id). Carries: argv, envp subset, cwd, parent-invocation-id (if parent is also a compiler invocation), exit code, read set, write set, wall-clock duration.
- **Read set**: The deterministically-ordered list of `{path, sha256}` tuples for every file the invocation + its descendants opened for reading during the invocation's lifetime, filtered to exclude trace-noise paths.
- **Write set**: The deterministically-ordered list of file paths the invocation + its descendants wrote-and-closed during its lifetime. Content hashes are captured lazily at emit-time only for files that survive the trace window (i.e., final artifacts, not build intermediates that got deleted).
- **Compiler-invocation DAG**: A parent-child linkage graph. Root = the top-level build command's compiler invocation (usually `cargo`, `make`, `go build`). Leaves = individual `rustc` / `cc1` / `ld` invocations.
- **Source-read-set annotation**: Per-component SBOM annotation `mikebom:source-read-set` carrying the read-set of the invocation subtree that produced that component's write-set. Byte-deterministic across identical builds per FR-011.
- **Cache-hit transparency annotation**: Per-component `mikebom:read-set-source = "cache-hit"` marker emitted when the component was served from a compiler cache and no compiler-invocation captured its read-set (per FR-015).
- **Pipeline-completeness signal**: Document-scope `mikebom:compiler-pipeline-completeness` annotation set to `"complete"` on clean traces, `"degraded"` (with drop-count + affected-component-count) on ring-buffer-overflow scans, or `"partial"` on attach-late scans. Satisfies Principle X.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a fixture Rust workspace with two binary targets — one that imports `libsafe`, another that imports `libsafe` + `libvuln` — the emitted SBOM's per-component `mikebom:source-read-set` annotation for the first binary contains ONLY `libsafe`'s source files, and for the second contains BOTH `libsafe` and `libvuln`'s source files. Verified via automated diff.
- **SC-002**: On the mikebom self-build (cargo build against the workspace itself with `--features ebpf-tracing`), every emitted binary component has a non-empty `mikebom:source-read-set` containing at minimum the top-level `main.rs` / `lib.rs` files of the crate that produced it. Verified via unit test with reachability assertion.
- **SC-003**: The trace pipeline adds no more than 15 % wall-clock overhead on the mikebom self-build (warm-cargo-cache baseline: ~30 s → traced: ≤ 34.5 s), verified on a Linux CI runner with `MIKEBOM_PREPR_EBPF=1`.
- **SC-004**: Two consecutive traces of the same source tree produce byte-identical `mikebom:source-read-set` annotations across every emitted component. Any deviation is a determinism bug.
- **SC-005**: When a source file is removed from the input tree between two builds, no output binary in the SBOM of the second build has that file in its source-read-set. Attribution exclusivity is preserved.
- **SC-006**: A downstream tool parsing a mikebom SBOM (fixture consumer script) can reduce a synthetic 100-CVE advisory list to only those CVEs whose affected-file patterns intersect at least one binary's source-read-set. Verified via fixture assertion on a canned advisory set.
- **SC-007**: When ring-buffer overflow occurs on a synthetically-heavy build (10k+ syscalls/sec), mikebom emits the `mikebom:compiler-pipeline-completeness = "degraded"` annotation with a non-zero drop-count, satisfying Principle X's transparency requirement.

## Assumptions

- **Linux-only, feature-gated**: Per FR-013 + FR-014, the feature ships behind `--features ebpf-tracing` and is Linux-only. macOS + Windows fall back to no-op (matching pre-milestone-210 behavior — mikebom's trace pipeline is already Linux-only per m020).
- **Compiler whitelist scope**: The MVP whitelist (rustc, gcc, clang, g++, clang++, go, ld, mold, cc1, cpp, as) covers ≥ 90 % of real-world compiled-language builds. Extensions to Java (javac), .NET (csc, roslyn), C# etc. are deferred to a follow-up milestone.
- **Interpreted-language runtime module loaders are OUT of scope**: Python `_PyImport_LoadDynamic`, Node V8 module loader, Ruby `rb_require_string`, JVMTI — all deferred to a separate milestone. This milestone is COMPILED-language-only.
- **Package-manager tracing is OUT of scope**: `pip install`, `npm install`, `mvn dependency:resolve`, `cargo fetch`, `go mod download` — those download deps, they don't compile. Already partially covered by the existing network-trace layer.
- **Compiler-cache tracing is OUT of scope**: sccache, ccache, mold cache-hits get a transparency annotation per FR-015; full cache-server-query hooks are deferred to a follow-up.
- **Multi-arch cross-compilation is IN scope for attribution** (FR-012) but per-arch-SBOM output is NOT — the milestone emits ONE SBOM per traced build, even when the build produces multi-arch artifacts.
- **Ring-buffer sizing**: the MVP inherits the existing eBPF ring-buffer size from m020; overflow degrades gracefully with a transparency annotation (FR-008 + SC-007). Dynamic sizing / flow-control is a follow-up.
- **CI feature-flag coverage**: the existing `MIKEBOM_PREPR_EBPF=1` env-gated local pre-PR path AND the dedicated `lint-and-test-ebpf` CI job (per CLAUDE.md feature-flag section) already cover the on-side; this milestone doesn't need new CI infrastructure.
- **Trace-noise filter is hardcoded** (per FR-016) with an operator escape hatch. Configurable filter files are deferred to a follow-up if operator demand emerges.
- **stdin-input marker rather than content capture** (per FR-018): capturing stdin content in-kernel is complex and out of scope; marking the presence + read-count is the MVP shape.
- **No mikebom-common shape changes required beyond the additive `compiler_pipeline` section on `BuildTracePredicate`**. Pre-m210 attestation consumers ignore the unknown field per JSON forward-compatibility.
- **No new Cargo dependencies**: the existing `aya` + `aya-ebpf` + `aya-log` stack (m001–m020) covers the additional programs needed. No new user-space crates.
