# Quickstart: Verify cargo main-module emission

This 2-recipe verification covers the single-crate path (US1 AS#1) and the workspace + dogfood path (US1 AS#2 + AS#4 + SC-002). Both should pass after milestone 064 lands.

## Prerequisites

Build mikebom in the current branch:

```sh
cargo +stable build -p mikebom
```

## Recipe A — Single-crate scan (clap-rs)

```sh
git clone --depth 1 https://github.com/clap-rs/clap /tmp/clap-064
target/debug/mikebom sbom scan \
  --path /tmp/clap-064 \
  --format cyclonedx-json \
  --output /tmp/clap-064.cdx.json \
  --no-deep-hash

jq '.metadata.component | {bom_ref: ."bom-ref", type, name, version, purl}' \
  /tmp/clap-064.cdx.json
```

**Expect** (`<x.y.z>` matches the manifest version on `clap-rs/clap` `HEAD`):

```json
{
  "bom_ref": "pkg:cargo/clap@<x.y.z>",
  "type": "application",
  "name": "clap",
  "version": "<x.y.z>",
  "purl": "pkg:cargo/clap@<x.y.z>"
}
```

Verify the C40 supplementary tag:

```sh
jq '.metadata.component.properties[] | select(.name == "mikebom:component-role") | .value' \
  /tmp/clap-064.cdx.json
# → "main-module"
```

Verify the cargo main-module is **not** duplicated in `components[]`:

```sh
jq '[.components[] | select(.purl == .metadata.component.purl)] | length' \
  /tmp/clap-064.cdx.json
# → 0
```

## Recipe B — Dogfood mikebom workspace (SC-002)

```sh
target/debug/mikebom sbom scan \
  --path . \
  --format spdx-2.3-json \
  --output /tmp/mikebom-self.spdx.json \
  --no-deep-hash
```

Verify exactly four cargo main-modules emit:

```sh
jq '[.packages[]
     | select(.primaryPackagePurpose == "APPLICATION")
     | { name, purl: (.externalRefs[] | select(.referenceType == "purl") | .referenceLocator) }
    ] | sort_by(.name)' \
  /tmp/mikebom-self.spdx.json
```

**Expect**:

```json
[
  { "name": "mikebom",        "purl": "pkg:cargo/mikebom@0.1.0-alpha.11" },
  { "name": "mikebom-common", "purl": "pkg:cargo/mikebom-common@0.1.0-alpha.11" },
  { "name": "mikebom-ebpf",   "purl": "pkg:cargo/mikebom-ebpf@<its-version>" },
  { "name": "xtask",          "purl": "pkg:cargo/xtask@0.1.0-alpha.11" }
]
```

Verify `documentDescribes[]` covers all four:

```sh
jq '.documentDescribes | length' /tmp/mikebom-self.spdx.json
# → 4
```

Verify byte-stability across two consecutive scans:

```sh
target/debug/mikebom sbom scan --path . --format spdx-2.3-json \
  --output /tmp/mikebom-self-2.spdx.json --no-deep-hash
diff /tmp/mikebom-self.spdx.json /tmp/mikebom-self-2.spdx.json
# → no output (files are byte-identical)
```

## Recipe C — Same-PURL collision smoke (FR-001 + Q1)

Synthesize a duplicate-PURL scan and verify the dedup `tracing::warn!` fires:

```sh
mkdir -p /tmp/cargo-dedup-test/crates/foo /tmp/cargo-dedup-test/vendor/foo-1.2.3
cat > /tmp/cargo-dedup-test/Cargo.toml <<'EOF'
[workspace]
members = ["crates/foo"]
EOF
cat > /tmp/cargo-dedup-test/crates/foo/Cargo.toml <<'EOF'
[package]
name = "foo"
version = "1.2.3"
edition = "2021"
EOF
cat > /tmp/cargo-dedup-test/vendor/foo-1.2.3/Cargo.toml <<'EOF'
[package]
name = "foo"
version = "1.2.3"
edition = "2021"
EOF

RUST_LOG=mikebom=warn target/debug/mikebom sbom scan \
  --path /tmp/cargo-dedup-test \
  --format cyclonedx-json \
  --output /dev/null \
  --no-deep-hash \
  2>&1 | grep "deduped"
```

**Expect** a `tracing::warn!` of the form:

```text
cargo: deduped 1 same-PURL Cargo.toml file for pkg:cargo/foo@1.2.3 — kept /tmp/cargo-dedup-test/crates/foo/Cargo.toml, dropped: /tmp/cargo-dedup-test/vendor/foo-1.2.3/Cargo.toml
```

The SBOM contains exactly one `pkg:cargo/foo@1.2.3` component (no duplicates, no `mikebom:duplicate-purl` annotation — that signal is reserved for the divergent case in #125).

## When to run

- **Recipe A** during US1 implementation as the first acceptance check.
- **Recipe B** before declaring SC-002 satisfied (the dogfood test).
- **Recipe C** before declaring the Q1 dedup behavior verified.

All three recipes should also be exercised as integration tests in `tests/scan_cargo.rs` per the plan's task breakdown.
