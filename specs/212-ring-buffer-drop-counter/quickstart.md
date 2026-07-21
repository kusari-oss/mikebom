# Quickstart: end-to-end verification for m212

**Milestone**: 212
**Date**: 2026-07-20
**Purpose**: Concrete, copy-pasteable recipe to reproduce the m212 acceptance criteria from a clean check-out. Reviewers run this to satisfy themselves the fix works.

## Prerequisites

- macOS + Colima running an aarch64 Linux VM (`colima start` if not already running).
- Colima's docker storage has ≥ 30 GB free (`colima ssh -- df -h /mnt/lima-colima`). If low, truncate the busiest compose-stack container logs — same procedure as m211 quickstart Step 1 (`bash /tmp/free-colima.sh` re-pointed at the current top-1 offender per `sudo find /var/lib/docker/containers -maxdepth 2 -name "*.log" | ...`).
- The mikebom repo is checked out on branch `212-ring-buffer-drop-counter` at commit HEAD.
- The m211 `Dockerfile.ebpf-test` + `scripts/ebpf-integration-test.sh` are present (they are — landed in m211 commit 6ac7c43).

## Step 1: Build the container image

```
docker build -f Dockerfile.ebpf-test -t mikebom-ebpf-test .
```

First run: ~10–15 min. Subsequent rebuilds after m212 source edits: also ~10 min (`COPY . .` invalidates the cargo cache).

**Expected exit**: 0. Any error indicates the m212 changes broke the eBPF-side build (verifier rejected the new counter maps, or the userspace-side changes broke type-checking under `--features ebpf-tracing`).

## Step 2: Run the trace against the m211 SC-001 fixture

```
docker run --rm --privileged \
  -v /sys/kernel/debug:/sys/kernel/debug \
  -v /tmp:/host-out \
  --entrypoint bash mikebom-ebpf-test -c '
    cd /mikebom && \
    /mikebom/target/release/mikebom trace run \
      --attestation-format mikebom-v1 \
      --attestation-output /host-out/m212-verify.json \
      -- cargo build --release \
        --manifest-path /mikebom/mikebom-cli/tests/fixtures/compiler_pipeline/two_binaries_diverge/Cargo.toml \
        --target-dir /tmp/m212-verify-target
  ' 2>&1
```

## Step 3: Verify SC-001 — real drops reported

```
colima ssh -- sudo cat /tmp/m212-verify.json | jq '{
  ring_buffer_overflows: .predicate.trace_integrity.ring_buffer_overflows,
  kprobe_attach_failures: .predicate.trace_integrity.kprobe_attach_failures,
  events_dropped: .predicate.trace_integrity.events_dropped,
  file_ops_count: (.predicate.file_access.operations | length)
}'
```

**Expected**:
- `ring_buffer_overflows` is a **number > 100** — proves the counter incremented on the fixture's known-heavy drop pattern (per #614's observation of ~30K drops silently hidden pre-m212).
- `kprobe_attach_failures` is `[]` OR does NOT contain any entry matching `*_drops` — proves all three counter maps loaded cleanly on Colima aarch64 6.8.
- `events_dropped` is still `0` per Q2 (deferred to waybill#618).
- `file_ops_count` matches the pre-m212 range (~12K events) — proves the ring buffer + drain still work at pre-m212 throughput.

## Step 4: Verify SC-002 — zero drops on a hermetic command

```
docker run --rm --privileged \
  -v /sys/kernel/debug:/sys/kernel/debug \
  -v /tmp:/host-out \
  --entrypoint bash mikebom-ebpf-test -c '
    /mikebom/target/release/mikebom trace run \
      --attestation-format mikebom-v1 \
      --attestation-output /host-out/m212-hermetic.json \
      -- true
  ' 2>&1

colima ssh -- sudo cat /tmp/m212-hermetic.json | jq '.predicate.trace_integrity.ring_buffer_overflows'
```

**Expected**: `0`. A `true` invocation forks + exits immediately — zero file/network/exec activity, so no ring buffer pressure. The counter MUST correctly report zero to avoid false-positive drop signals.

## Step 5: Verify C-4 — wire-shape byte-identity of pre-m212 baseline vs post-m212

Compare a baseline attestation from m211 `6ac7c43` (a scan-mode SBOM with no trace involvement) against a post-m212 scan-mode SBOM. Assert deletion of `.predicate.trace_integrity` from both yields a byte-identical diff:

```
diff <(jq 'del(.predicate.trace_integrity)' /tmp/pre-m212-scan.cdx.json) \
     <(jq 'del(.predicate.trace_integrity)' /tmp/post-m212-scan.cdx.json)
```

**Expected**: empty diff. Any output = FR-003 byte-identity violation on scan-mode; investigate before merging.

## Step 6: Run the unit tests

```
cargo test -p mikebom --bin mikebom -- trace::loader::tests::warn_line
cargo test -p mikebom_common -- trace_integrity_serde_round_trip
cargo test -p mikebom_common -- trace_integrity_serde_populated_counter  # NEW post-m212
```

**Expected**: all pass. `trace_integrity_serde_round_trip` is the pre-existing test; the new `_populated_counter` extension asserts wire-shape byte-identity when the counter is non-zero AND when `kprobe_attach_failures[]` carries counter-map names.

## Step 7 (optional): Verify SC-006 attach-failure disambiguation

The counter-attach-failure case can be simulated via a test-only feature flag OR by constructing an `aya::EbpfError::MapNotFound(...)` directly in a unit test.

```
cargo test -p mikebom --bin mikebom -- trace::loader::tests::counter_attach_failure_surfaces_in_kprobe_attach_failures
```

**Expected**: test passes. Asserts that on synthetic attach-failure input:
- `TraceIntegrity.ring_buffer_overflows == 0` (partial value from any counter maps that DID succeed)
- `TraceIntegrity.kprobe_attach_failures` contains the failing counter map's name

## Cleanup

```
docker image prune -f
colima ssh -- sudo rm -f /tmp/m212-verify.json /tmp/m212-hermetic.json
```

## When something fails

- **Docker build fails with "no space left on device"**: `colima ssh -- df -h /mnt/lima-colima`; if compose logs have refilled, truncate via the m211 recipe.
- **Step 3 reports `ring_buffer_overflows: 0` unexpectedly**: check `kprobe_attach_failures[]` — if it contains a `*_drops` entry, the counter map didn't attach (see V-4 escape hatches in `contracts/ebpf-verifier-notes.md`). If it does NOT contain a `*_drops` entry, the counter is failing to increment; check the eBPF-side `else` branch code at the site that should be firing.
- **Step 4 reports `ring_buffer_overflows > 0` on `true`**: the counter is spuriously incrementing on a zero-activity command; suspect an eBPF-side bug where the counter fires regardless of reserve() outcome.
- **Step 5 diff is non-empty**: FR-003 wire-shape violation — inspect the diff for the field that drifted.
- **Unit tests fail**: run with `--nocapture` for stack traces.
