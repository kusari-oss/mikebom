# Quickstart: Compiler-Pipeline eBPF Tracing (m210)

**Date**: 2026-07-19
**Purpose**: Shortest path from `cargo build` to seeing source-read-set annotations. Also serves as the manual QA script during implementation.

## Prerequisites

- Rust stable (workspace toolchain).
- **Linux kernel 4.19+** (`sched_process_exec` tracepoint availability).
- `CAP_BPF + CAP_PERFMON` OR root.
- `mikebom-cli` built WITH the eBPF feature: `cargo +stable build -p mikebom --release --features ebpf-tracing` (requires nightly + `bpf-linker` per m020 setup).

## Path 1 — Trace a simple two-file `gcc` build (US1 P1)

Create `hello.c` + `helper.c`:

```c
// hello.c
#include "helper.h"
int main(void) { return greet(); }
```

```c
// helper.c
int greet(void) { return 42; }
```

Trace the compile:

```sh
sudo mikebom trace run -- gcc hello.c helper.c -o hello
```

Verify the emitted SBOM's per-component annotation:

```sh
jq '.components[] | select(.name == "hello") | .properties[] | select(.name == "mikebom:source-read-set") | .value | fromjson' \
  /tmp/hello.cdx.json
```

**Expected**:

```json
{
  "invocation_ids": [1, 2],
  "read_set": [
    { "path": "/path/to/helper.c", "sha256": "<hex>", "kind": "file" },
    { "path": "/path/to/hello.c", "sha256": "<hex>", "kind": "file" }
  ]
}
```

## Path 2 — Verify byte-identity across two runs (US2 P2)

```sh
sudo mikebom trace run -- gcc hello.c helper.c -o hello \
    --output /tmp/hello-run1.cdx.json

sudo mikebom trace run -- gcc hello.c helper.c -o hello \
    --output /tmp/hello-run2.cdx.json

diff \
    <(jq '.components[].properties[]? | select(.name == "mikebom:source-read-set")' /tmp/hello-run1.cdx.json) \
    <(jq '.components[].properties[]? | select(.name == "mikebom:source-read-set")' /tmp/hello-run2.cdx.json)
```

**Expected**: zero diff. Byte-identical source-read-set annotations.

Then modify `helper.c` (add a comment); rebuild + retrace:

```sh
echo "// added" >> helper.c
sudo mikebom trace run -- gcc hello.c helper.c -o hello \
    --output /tmp/hello-run3.cdx.json

diff \
    <(jq '.components[].properties[]? | select(.name == "mikebom:source-read-set")' /tmp/hello-run1.cdx.json) \
    <(jq '.components[].properties[]? | select(.name == "mikebom:source-read-set")' /tmp/hello-run3.cdx.json)
```

**Expected**: `helper.c`'s sha256 differs between the two runs; `hello.c`'s sha256 unchanged.

## Path 3 — Verify exclusion invariant (SC-005)

Remove `helper.c` from the input tree AND modify `hello.c` to not depend on it:

```sh
rm helper.c
cat > hello.c <<EOF
int main(void) { return 42; }
EOF

sudo mikebom trace run -- gcc hello.c -o hello \
    --output /tmp/hello-run4.cdx.json

jq '.components[].properties[]? | select(.name == "mikebom:source-read-set") | .value | fromjson.read_set | map(.path)' \
  /tmp/hello-run4.cdx.json
```

**Expected**: `helper.c` NOT in the read_set. Only `hello.c`.

## Path 4 — Verify secrets filter (FR-016a)

Create a fixture that reads from a secret-adjacent path:

```sh
mkdir -p /tmp/fake-secrets
echo "sensitive" > /tmp/fake-secrets/token
cat > read_secret.sh <<'EOF'
#!/bin/sh
cat /var/run/secrets/token || true
gcc hello.c -o hello
EOF
chmod +x read_secret.sh

# NOTE: the trace-noise filter matches `/var/run/secrets/*` — a real secret path.
sudo mikebom trace run -- ./read_secret.sh \
    --output /tmp/hello-secret.cdx.json

jq '.metadata.properties[] | select(.name == "mikebom:secrets-read-filtered") | .value' \
  /tmp/hello-secret.cdx.json
```

**Expected**: value `"1"` (or higher — count of filtered reads). The `/var/run/secrets/token` path does NOT appear in any component's source-read-set.

Then re-run with the escape hatch:

```sh
sudo mikebom trace run --include-system-reads -- ./read_secret.sh \
    --output /tmp/hello-secret-audit.cdx.json

jq '.components[].properties[]? | select(.name == "mikebom:source-read-set") | .value | fromjson.read_set | map(select(.path | startswith("/var/run/secrets")))' \
  /tmp/hello-secret-audit.cdx.json
```

**Expected**: the `/var/run/secrets/token` path DOES appear (auditor mode).

## Path 5 — Rust workspace trace (US1 P1 end-to-end)

Clone the fixture project:

```sh
cd mikebom-cli/tests/fixtures/compiler_pipeline/two_binaries_diverge/
sudo mikebom trace run -- cargo build --release \
    --output /tmp/rust-workspace.cdx.json
```

Verify divergent attribution:

```sh
# safe-only binary — should only see libsafe sources
jq '.components[] | select(.name == "safe-only") | .properties[] | select(.name == "mikebom:source-read-set") | .value | fromjson.read_set | map(.path | test("libvuln")) | any' \
  /tmp/rust-workspace.cdx.json
```

**Expected**: `false` — no libvuln paths.

```sh
# vuln-included binary — should see BOTH libsafe + libvuln
jq '.components[] | select(.name == "vuln-included") | .properties[] | select(.name == "mikebom:source-read-set") | .value | fromjson.read_set | map(.path | test("libvuln")) | any' \
  /tmp/rust-workspace.cdx.json
```

**Expected**: `true` — libvuln paths present.

## Path 6 — Verify degraded completeness (SC-007)

Simulate a heavy-parallel build that overflows the ring buffer:

```sh
sudo mikebom trace run -- make -j64 -C /path/to/large-c-project \
    --output /tmp/degraded.cdx.json

jq '.metadata.properties[] | select(.name == "mikebom:compiler-pipeline-completeness") | .value | fromjson' \
  /tmp/degraded.cdx.json
```

**Expected**: if drops occurred, `{"state": "degraded", "dropped": N, "affected_component_count": M}`. Clean run: `{"state": "complete"}`.

## Non-goals surfaced in quickstart

- **Cache hits are marked but not traced**: sccache/ccache-served artifacts get `mikebom:read-set-source = "cache-hit"` + omit C130. Full cache-server tracing is a follow-up milestone.
- **No per-arch SBOMs**: cross-compilation attribution works (FR-012), but the trace emits ONE SBOM per build even when multiple architectures were produced.
- **No Java/.NET compilers**: `javac`, `roslyn`, `csc` deferred to a follow-up.
- **No interpreted-language module loaders**: Python `_PyImport_LoadDynamic`, Node V8, Ruby `rb_require_string` all deferred.
