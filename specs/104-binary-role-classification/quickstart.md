# Quickstart: Binary Role Classification — Manual Reproduction

## Pre-fix behavior (baseline)

```bash
# Use the alpha.34 release binary (or main pre-104 build)
mikebom sbom scan --offline --path /bin --output bin.cdx.json
jq -r '.components[] | "\(.name)\t\(.type)"' bin.cdx.json | sort | uniq -c | sort -rn
```

Expected output (pre-fix): **every component reports `type: library`** — including executables like `ls`, `cat`, `cp`, and shared libraries like `libSystem.B.dylib`. The CDX `type` field is useless as a partition between executable and library.

## Post-fix behavior

```bash
# Same scan, but with the milestone-104 binary
mikebom sbom scan --offline --path /bin --output bin.cdx.json
jq -r '.components[] | "\(.name)\t\(.type)"' bin.cdx.json | sort | uniq -c | sort -rn
```

Expected output (post-fix): executables report `type: application`, dylibs report `type: library`. A consumer filtering for executables can run:

```bash
jq '.components[] | select(.type == "application") | .name' bin.cdx.json
```

and get the clean list of binaries — `ls`, `cat`, `cp`, `echo`, etc. — without any libraries leaking in.

## Cross-format verification

```bash
# Emit all three formats from the same scan
mikebom sbom scan --offline --path /bin \
  --format cyclonedx-json,spdx-2.3-json,spdx-3-json \
  --output cdx=bin.cdx.json --output spdx-2.3-json=bin.spdx.json --output spdx-3-json=bin.spdx3.json

# Confirm a known executable carries the role-equivalent value in all three formats
jq '.components[] | select(.name == "ls") | .type' bin.cdx.json
# expected: "application"

jq '.packages[] | select(.name == "ls") | .primaryPackagePurpose' bin.spdx.json
# expected: "APPLICATION"

jq '.["@graph"][] | select(.type == "software_Package" and .name == "ls") | .software_primaryPurpose' bin.spdx3.json
# expected: "application"

# Confirm a known shared library
jq '.components[] | select(.name | endswith(".dylib")) | .type' bin.cdx.json
# expected: every value is "library"

jq '.packages[] | select(.name | endswith(".dylib")) | .primaryPackagePurpose' bin.spdx.json
# expected: every value is "LIBRARY"
```

## Linux PIE executable verification (the ET_DYN ambiguity case)

```bash
# Pull a small alpine image which ships PIE executables in /bin
mikebom sbom scan --offline --image alpine:3 --output alpine.cdx.json

# Confirm a known PIE executable (most /bin entries on alpine are PIE)
jq '.components[] | select(.name == "busybox") | .type' alpine.cdx.json
# expected: "application"  (despite ELF e_type being ET_DYN — disambiguated by PT_INTERP presence)
```

## Auditing the `Other` bucket

```bash
# After scanning anywhere that includes Mach-O bundles or kernel extensions
jq '.components[] | select(.type == "library") | select(.properties[]?.name == "mikebom:binary-class")' bin.cdx.json
```

Components in the `Other` bucket still emit as CDX `type: library` (preserves historic default) but carry the `mikebom:binary-class` annotation for finer-grained inspection. To find them specifically, look for components that have a binary-class annotation but don't otherwise match the SharedLibrary heuristics (e.g., filename doesn't end in `.dylib` / `.so` / `.dll`). The full disambiguation is logged at scan time via `tracing::info!` per FR-004.

## Constitution conformance check

Per Constitution Principle V (standards-native first), confirm the SBOM emits NO `mikebom:binary-role` annotation:

```bash
jq '.components[] | select(.properties[]?.name == "mikebom:binary-role")' bin.cdx.json
# expected: empty output (no such annotation exists)
```

The role signal lives exclusively in the spec-native `type` field. Consumers reading the standards-native field get the complete answer without needing to parse any mikebom-prefixed namespace.

## Test fixture coverage

The milestone's integration tests build synthetic ELF, Mach-O, and PE binaries at test time covering all four `BinaryRole` variants:

```bash
cargo +stable test -p mikebom --test binary_role_parity
cargo +stable test -p mikebom --test binary_role_disambiguation
```

Both should report `ok` post-implementation.
