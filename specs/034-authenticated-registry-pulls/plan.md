---
description: "Implementation plan — milestone 034 authenticated OCI registry pulls"
status: plan
milestone: 034
---

# Plan: Authenticated OCI registry pulls

## Architecture

A new `auth.rs` sibling to `registry.rs` resolves credentials for a target
registry, returning `Option<Credential>`. `RegistryClient::new()` calls into
it once per scan. `fetch_bearer_token` then either sends a Basic-auth header
on the realm GET (credentials present) or sends it unauthenticated
(anonymous, today's behavior).

No public API change. No new top-level dep. Lives entirely behind the
existing `oci-registry` Cargo feature.

```
┌──────────────┐
│ mod.rs       │ pull_to_tarball()
└──────┬───────┘
       │ creates
       ▼
┌──────────────┐    auth.rs::load_default_docker_config()
│ RegistryClient◄───┐ auth.rs::resolve_credentials()
└──────┬───────┘    │   → cred-helper subprocess
       │            │   → DockerConfig.auths[reg].auth (b64-decoded)
       │            │   → DockerConfig.auths[reg].identitytoken
       │ uses
       ▼
┌──────────────────────────┐
│ fetch_with_bearer_retry  │
│   GET → 401              │
│   parse Bearer challenge │
│   ┌───────────────────┐  │
│   │ fetch_bearer_token│ ◄── credentials → Basic auth on realm GET
│   └───────────────────┘  │
│   retry GET with Bearer  │
└──────────────────────────┘
```

## Reuse inventory

These existing items handle the work; no new infrastructure required:

- **`base64::engine::general_purpose::STANDARD`** — already a direct dep
  (Cargo.toml line 95). Decodes the `auth` field's `<base64(user:pass)>`.
- **`reqwest::RequestBuilder::basic_auth(user, Some(secret))`** — built-in
  on the workspace's `reqwest 0.12`. Sends `Authorization: Basic ...`.
- **`serde_json`** — workspace dep, parses config.json.
- **`std::process::Command`** — stdlib, drives the cred-helper subprocess.
  Stdin is the registry hostname; stdout is JSON with Username/Secret/ServerURL.
- **`std::env::var("HOME")` / `std::env::var("DOCKER_CONFIG")`** — stdlib
  path resolution. No `dirs` crate needed.
- **Existing `parse_bearer_challenge`, `fetch_bearer_token` shape in
  registry.rs** — surgical edit to add an optional Basic-auth header on
  the realm GET. ~5 LOC change.
- **Existing `oci_pull` test scaffolding + `MIKEBOM_OCI_NETWORK_TESTS=1`
  gate in `oci_registry_smoke.rs`** — extends with
  `MIKEBOM_OCI_AUTH_TESTS=1` for the auth path.

## Touched files

| File | Change | LOC |
|---|---|---|
| `mikebom-cli/src/scan_fs/oci_pull/auth.rs` | NEW — Docker config parser, cred-helper subprocess, Credential type | +400 |
| `mikebom-cli/src/scan_fs/oci_pull/mod.rs` | declare new module | +1 |
| `mikebom-cli/src/scan_fs/oci_pull/registry.rs` | thread Credential into fetch_bearer_token; update 401 message | +40 |
| `mikebom-cli/tests/oci_registry_smoke.rs` | gated auth smoke test | +50 |
| `docs/user-guide/cli-reference.md` | `--image` auth section | +30 |
| `CHANGELOG.md` | unreleased entry | +5 |
| `mikebom-cli/Cargo.toml` | comment update on `oci-registry` feature | +3 |

Total: ~530 LOC across 7 files. Bulk is `auth.rs` (with inline tests).

## Phasing

Three atomic commits; each `./scripts/pre-pr.sh`-clean.

### Commit 1: `034/auth-module`
- New `auth.rs` with: DockerConfig serde structs, registry name
  normalization, `load_default_docker_config`, `resolve_credentials`,
  `run_credential_helper` subprocess, `Credential` type with hand-written
  redacting `Debug`.
- Inline tests (10+): config.json variants (auth-only, identitytoken,
  credsStore, credHelpers, empty auths fallback), registry normalization
  (docker.io ↔ index.docker.io), Credential::Debug redaction, helper
  subprocess against a tempdir-deployed shim shell script.
- `#[allow(dead_code)]` on the public-to-supermodule items since they're
  not called yet — lifted in commit 2.
- mod.rs gains `mod auth;`.

### Commit 2: `034/wire-auth-into-registry`
- `RegistryClient::new()` accepts a `&ImageReference` and resolves
  credentials lazily.
- `fetch_bearer_token` takes `Option<&Credential>`, applies Basic auth
  when present.
- Update mod.rs's `pull_to_tarball` to pass the reference into
  `RegistryClient::new`.
- Update the 401-after-retry error message per FR-005.
- Remove `#[allow(dead_code)]` from `auth.rs`.
- Inline integration test: spin up a tiny reqwest mock that returns 401 +
  bearer challenge, then 200 on the retry; assert the realm-GET
  Authorization header decodes to the expected user:secret.

### Commit 3: `034/docs-and-smoke`
- `docs/user-guide/cli-reference.md` gains an "Authenticating to private
  registries" subsection under `--image`.
- `CHANGELOG.md` gets an unreleased entry.
- `mikebom-cli/Cargo.toml`'s `oci-registry` feature comment is updated
  ("anonymous + Docker-keychain auth").
- `tests/oci_registry_smoke.rs` gains a `#[ignore]`'d gated test that
  exercises a private GHCR pull when `MIKEBOM_OCI_AUTH_TESTS=1`.
- Final `./scripts/pre-pr.sh` audit + grep audit per SC-006.

## Estimated effort

| Phase | Effort | Notes |
|---|---|---|
| Commit 1 (auth module) | 6 hr | DockerConfig schema spelunking + subprocess shim test is the careful step |
| Commit 2 (wire-up) | 3 hr | Integration test with mock reqwest server |
| Commit 3 (docs + smoke) | 2 hr | Mostly text |
| Verification + PR | 1 hr | Goldens regen check + CI watch |
| **Total** | **~12 hr** | One+ focused day. |

## Risks

- **R1: Cred-helper protocol drift.** The Docker cred-helper API is
  documented at github.com/docker/docker-credential-helpers but isn't
  versioned formally. Mitigation: align with the
  [Docker source](https://github.com/docker/cli/blob/master/cli/config/credentials/native_store.go) —
  stdin = registry, stdout = JSON {Username, Secret, ServerURL}, exit
  non-zero or `"credentials not found"` on stdout = no creds. This shape
  has been stable since 2016.

- **R2: `~/.docker/config.json` schema drift.** Same mitigation: cite the
  Docker `cliconfig.ConfigFile` Go type as the canonical schema. Use serde
  `#[serde(default)]` everywhere so missing fields don't fail parsing.

- **R3: Test subprocess on Windows.** Cred helpers are platform-specific
  (`docker-credential-osxkeychain`, `docker-credential-wincred`). Inline
  test ships a shell-script shim, which won't run on Windows. Mitigation:
  `#[cfg(unix)]` the shim test; the parsing tests stay portable.

- **R4: Secret leak via `?`-operator + anyhow context.** `anyhow!("...
  {credential:?} ...")` would leak. Mitigation: hand-written Debug for
  `Credential` redacts the secret unconditionally; no `tracing::*!` or
  `anyhow::Context` may take a `Credential` by reference (lint
  enforced via grep audit per SC-006).

- **R5: GHCR's `package read` PAT scope.** GHCR requires `read:packages`
  scope on the PAT for private images. Mitigation: documented in
  cli-reference.md's auth section. Out-of-scope for code: that's a user
  config issue, not a mikebom bug.

- **R6: ECR token TTL (12h).** AWS ECR tokens expire. mikebom is a
  one-shot CLI — by the time the token expires, the scan has long since
  completed. No retry-on-401 logic needed beyond the existing single
  retry. Documented in the auth section's "ECR notes" callout.

## Constitution alignment

- **Principle I (zero C in deps):** No new top-level deps. `base64`,
  `serde_json`, `reqwest`, and stdlib `std::process` are all already in
  the tree. `no_c_dependencies_in_oci_registry_feature_tree` regression
  test still passes (verified at end of commit 3). ✓
- **Principle IV (no `.unwrap()` in production):** New `auth.rs` returns
  `Option`/`Result` everywhere. No panics. Inline tests use the standard
  `cfg_attr(test, allow(clippy::unwrap_used))` envelope. ✓
- **Principle VI (three-crate architecture):** Untouched. ✓
- **Per-commit verification (lessons from 018-022):** FR-008 enforced.

## What this milestone does NOT do

- Does not change CLI args, output flags, or any user-facing surface
  beyond "private images now scan when config.json is configured."
- Does not introduce a `--registry-auth` flag.
- Does not add OAuth refresh-token flows.
- Does not implement native AWS SDK / ECR client (cred helper handles it).
- Does not touch the milestone-031 anonymous-pull path's behavior — it
  remains the fallback when no credentials match.
