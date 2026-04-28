---
description: "Implementation plan — milestone 032 oci-client → oci-spec migration"
status: plan
milestone: 032
---

# Plan: oci-client → oci-spec migration

## Architecture

Pure substitution at the dep layer. External surface (the public
`pull_to_tarball` fn + `--image <ref>` CLI dispatch + smoke test
contract) is preserved exactly. Only the implementation under
`oci_pull/` swaps its substrate.

```
                       ┌──────────────────────────────┐
                       │ Same external behavior:      │
   --image alpine:3.19 │ - pull_to_tarball(ref)       │
                       │ - ImageArgKind detect        │
                       │ - host_oci_arch              │
                       │ - same SBOM output           │
                       └─────────────┬────────────────┘
                                     │
   BEFORE (031):                     │   AFTER (032):
   ┌─────────────────────┐           │   ┌────────────────────┐
   │ oci-client = "0.12" │           │   │ oci-spec = "0.9"   │
   │  - reference parser │           │   │  - manifest types  │
   │  - HTTP client      │           │   │  - descriptor types│
   │  - bearer-token     │           │   │  - image config    │
   │  - blob fetch       │           │   └─────────┬──────────┘
   │  - digest verify    │           │             │
   │  - manifest-list    │           │             │ + custom thin layer:
   │  - platform select  │           │             ▼
   │  (~all of it)       │           │   ┌────────────────────┐
   └─────────────────────┘           │   │ scan_fs/oci_pull/  │
                                     │   │  mod.rs            │
                                     │   │  reference.rs ←NEW │
                                     │   │  registry.rs  ←NEW │
                                     │   │  platform.rs  ←NEW │
                                     │   │  tarball.rs   ←kept│
                                     │   └────────────────────┘
                                                 │
                                                 ▼
                                       reqwest 0.12 + rustls-tls
                                       (already in workspace deps)
```

## Reuse inventory

These existing items handle the work; this milestone consumes them:

- **`reqwest = "0.12"`** with `default-features = false, features =
  ["json", "rustls-tls"]` in workspace `Cargo.toml:15`. Already
  pinned to ring-based rustls. **No HTTP/TLS dep changes.**
- **`tokio = "1" full`** workspace dep — async runtime ready.
- **`flate2`, `tar`, `tempfile`, `sha2`, `serde / serde_json`,
  `anyhow`, `tracing`** — all already in workspace.
- **Existing `tarball.rs` logic** (today's `oci_pull.rs::assemble_docker_save_tarball`
  + `decompress_layer` + `append_tarball_entry` + `sha256_hex` +
  `assert_layers_supported`) — preserved with minimal type-import
  swaps.
- **Existing tests in `oci_pull.rs::tests`** — preserved across
  the split, redistributed to per-submodule `tests` blocks.
- **The `no_c_dependencies` regression test** (extended in
  milestone 031) — already audits the feature-on tree with
  `aws-lc-sys` blacklisted. Acts as the durability guardrail.

## Crate choice: oci-spec 0.9

Selected: **`oci-spec = "0.9"`** maintained by youki-dev
(https://github.com/youki-dev/oci-spec-rs).

Rationale:
- **Pure-Rust + clean dep graph** (`serde`, `serde_json`,
  `thiserror`, `derive_builder`, `getset`, `strum`,
  `strum_macros`, `regex`, `const_format`). All already present
  in mikebom's tree or close. Verified at recon time.
- **Types-only** — no HTTP, no auth, no TLS. Zero surface for
  upstream transitive-dep drift to pull in C deps. THIS is the
  property that makes oci-spec the right substitute for the
  oci-client wrapper that locked us out.
- **Feature-gated by spec area** (`distribution`, `image`,
  `runtime`). We need `distribution` (manifest types) +
  `image` (image-config + descriptor types). Skip `runtime`.
- Maintained by the youki container-runtime project — same Rust
  container ecosystem that wrote oci-client, so the type
  fidelity is high.

Rejected:
- `oci-client = "0.16+"` — the version-pin trap we're escaping.
- Roll types entirely from scratch — O(weeks) of tedious
  serde wiring; oci-spec already does this correctly.

## Touched files

| File | Change | LOC |
|---|---|---|
| `mikebom-cli/Cargo.toml` | swap oci-client → oci-spec; oci-registry feature points at oci-spec | +/- 5 |
| `mikebom-cli/src/scan_fs/oci_pull.rs` | DELETED — promoted to a directory | -408 |
| `mikebom-cli/src/scan_fs/oci_pull/mod.rs` | NEW — public surface (mostly preserved from old file's top) | +90 |
| `mikebom-cli/src/scan_fs/oci_pull/reference.rs` | NEW — image-ref parser + 7+ tests | +180 |
| `mikebom-cli/src/scan_fs/oci_pull/registry.rs` | NEW — HTTP client (manifest fetch, bearer-token retry, blob fetch + digest verify) | +250 |
| `mikebom-cli/src/scan_fs/oci_pull/platform.rs` | NEW — manifest-list resolver + tests | +90 |
| `mikebom-cli/src/scan_fs/oci_pull/tarball.rs` | NEW — moved from old file's bottom (~unchanged) | +200 |
| `mikebom-cli/src/scan_fs/mod.rs` | gated `pub mod oci_pull;` declaration unchanged (still points at the directory) | 0 |
| `mikebom-cli/src/cli/scan_cmd.rs` | unchanged | 0 |
| `mikebom-cli/tests/oci_registry_smoke.rs` | unchanged (preserves the contract) | 0 |
| `mikebom-cli/tests/no_c_dependencies.rs` | unchanged (already audits feature-on with aws-lc-sys blacklisted) | 0 |

Net: ~+400 LOC of new code, but distributed across submodules for
readability. Old single-file `oci_pull.rs` (408 LOC) goes away.

## Phasing

Three atomic commits.

### Commit 1: `032/substrate` — new oci_pull/ skeleton + reference.rs + platform.rs
- Promote `oci_pull.rs` (single file) to `oci_pull/` (directory).
- `mod.rs` retains today's public surface (`pull_to_tarball`,
  `ImageArgKind`, `detect_image_arg_kind`, `host_oci_arch`).
  pull_to_tarball still calls into the old oci-client at this
  point — substrate is being introduced, not yet swapped in.
- `tarball.rs` extracted from old file's bottom (assembly helpers,
  unchanged behavior).
- `reference.rs` written from scratch with 7+ ref-parsing tests
  (Scenario 5 of spec.md).
- `platform.rs` extracted with manifest-list selection logic (today
  inlined in pull_to_tarball; lifted out for testability) + tests.
- Verify: workspace builds + tests both ways. `oci-client` is
  STILL in deps at this commit; we haven't dropped it yet.

### Commit 2: `032/migration` — replace oci-client integration with the new substrate + add registry.rs
- Write `registry.rs`: thin HTTP client with manifest fetch,
  bearer-token retry, blob fetch, digest verification. Uses
  workspace `reqwest::Client`. Async-native (matches pull_to_tarball).
- Update `mod.rs::pull_to_tarball` to use registry.rs / reference.rs
  / platform.rs / tarball.rs in place of oci-client.
- Verify: workspace builds + tests both ways. `oci-client` is
  imported but no longer referenced in production code.
- Smoke test against `alpine:3.19` end-to-end — assert same
  component count as pre-migration baseline.

### Commit 3: `032/cleanup-oci-client` — drop the oci-client dep + update spec/docs
- `mikebom-cli/Cargo.toml`: remove `oci-client` from `[dependencies]`,
  remove from `oci-registry` feature's dep list, replace with
  `oci-spec` declaration.
- Verify `cargo tree -p mikebom --features oci-registry | grep
  oci-client` is empty.
- Verify the `no_c_dependencies_in_oci_registry_feature_tree` test
  still passes (i.e., oci-spec doesn't pull anything bad).
- Update `mikebom-cli/src/scan_fs/oci_pull/mod.rs` doc-comment
  block referencing oci-client → reference oci-spec instead.
- Final smoke-test verification.

Per FR-012, each commit's `./scripts/pre-pr.sh` is clean BOTH
default + feature-on profiles.

## Estimated effort

| Phase | Effort | Notes |
|---|---|---|
| Phase 1 (recon + baseline) | done | T001 done in scoping |
| Phase 2 (substrate skeleton) | ½ day | mostly mechanical reorganization + ref parser + platform resolver |
| Phase 3 (registry.rs + migration) | 1 day | HTTP client + bearer-token flow is the meaty part |
| Phase 4 (cleanup + Cargo.toml + verify) | ½ day | dep removal + dual-profile verification + smoke test |
| **Total** | **~2 days** | comparable to milestone 031 itself. |

## Risks

- **R1: Reference-parsing edge cases.** Image-ref grammar is
  surprisingly subtle (registry-vs-repo disambiguation; `library/`
  prefix on Docker Hub bare names; digest-vs-tag precedence).
  Mitigation: comprehensive table-test against the 7 shapes in
  Scenario 5 + comparison-test against today's
  `oci_client::Reference` parser. If any case diverges, fix the
  divergence in commit 1 before commit 2 lands.

- **R2: Bearer-token realm parsing.** The
  `Www-Authenticate: Bearer realm=...,service=...,scope=...`
  header is comma-separated key="value" pairs. Quoting rules are
  per RFC 7235 (escaped double-quotes, etc.). Mitigation: small
  parser + table-test for the 3-4 real-world shapes (Docker Hub,
  GHCR public, generic). Any other shape returns a clear
  "couldn't parse Www-Authenticate" error.

- **R3: reqwest's redirect-Authorization behavior.** When a CDN
  redirect happens (Docker Hub → cloudflare), reqwest may strip
  the Authorization header on cross-origin redirect (security
  default). For anonymous bearer tokens this might still work
  (the token is sent only to the original registry; the CDN URL
  may or may not need it depending on CDN config). Mitigation:
  smoke-test reproduction first. If redirects break the pull,
  set reqwest's redirect policy to follow-and-preserve-auth (with
  documented caveat) or use the redirect-aware bearer flow that
  re-auths against the CDN.

- **R4: oci-spec API drift between 0.9 and a future version.**
  We're pinning to 0.9 today. oci-spec is types-only so future
  bumps should be additive but worth flagging. Mitigation:
  the `no_c_dependencies` regression test catches transitive-dep
  drift; a behavior-parity smoke test catches schema drift.

- **R5: Manifest media-type variability.** Some registries return
  Docker v2 manifests instead of OCI v1 (different field names in
  some edge cases). Mitigation: oci-spec covers both. Inspect the
  response `Content-Type` to dispatch.

## Constitution alignment

- **Principle I (Pure Rust, Zero C):** verified at recon — oci-spec
  has zero `*-sys` deps; the existing
  `no_c_dependencies_in_oci_registry_feature_tree` test enforces
  it at PR time.
- **Principle IV (no `.unwrap()` in production):** new code
  uses `?` throughout, returns `anyhow::Result`.
- **Principle VI (Three-Crate Architecture):** untouched; oci-spec
  lives in `mikebom-cli`'s feature-gated section like oci-client did.
- **Principle VIII (Completeness):** behavior-parity contract
  (FR-001 acceptance scenarios) ensures no SBOM-output regression.
- **Per-commit verification (lessons from 016-031):** FR-012
  enforced.
- **Recon-first:** every claim grounded in file:line evidence
  from the milestone-031 code we're swapping AND from oci-spec's
  Cargo.toml.

## What this milestone does NOT do

- Does not change the public CLI surface.
- Does not change SBOM output for any input.
- Does not add auth (031.x / #66).
- Does not add `--image-platform` flag (031.y / #67).
- Does not add layer caching (031.z / #68).
- Does not improve error messages beyond what's needed for the
  swap (UX work is separate).
- Does not introduce new features. This is purely a substrate
  swap.
