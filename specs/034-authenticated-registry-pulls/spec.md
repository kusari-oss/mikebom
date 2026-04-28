---
description: "Authenticated OCI registry pulls — Docker keychain (config.json + cred helpers) + bearer-token flow with credentials"
status: spec
milestone: 034
closes: "#66"
---

# Spec: Authenticated OCI registry pulls (031.x)

## Background

Milestone 031 (#63) shipped anonymous-only OCI image scanning. Milestone 032
(#65) replaced the `oci-client` substrate with a thin custom HTTP client over
`oci-spec` types + workspace `reqwest`. Milestone 033 (#70) flipped
`oci-registry` on by default.

`mikebom-cli/src/scan_fs/oci_pull/registry.rs` today has a single auth seam:
`fetch_with_bearer_retry` issues an unauthenticated GET, on 401 parses
`WWW-Authenticate: Bearer realm/service/scope`, fetches a token from the realm
**unauthenticated**, and retries with `Authorization: Bearer <token>`. The 401
error path emits "Authenticated registries are not yet supported in milestone
031 (tracked as 031.x — see issue #66)".

Real-world container scanning is dominated by private registries (private
GHCR packages, private Hub repos, ECR, internal/self-hosted registries).
Anonymous-only is a meaningful but minority use case. Closing this gap brings
mikebom to feature-parity with `syft <ref>` / `trivy image <ref>` for the most
common adoption path.

## User story (US1, P1)

**As an SBOM consumer scanning a container image stored in a private
registry**, I want `mikebom sbom scan --image <private-ref>` to authenticate
using the same `~/.docker/config.json` (and credential helpers) that
`docker pull` already uses, so that I can scan private images without
embedding credentials in mikebom-specific config.

**Why P1**: this is the highest-value follow-on to milestone 031. Users with
private registries currently have to `docker pull && docker save` as a
two-step workaround; that loses the "single-command image scan" UX that 031
established for public images.

### Independent test

After implementation, the following work end-to-end (CI-gated by
`MIKEBOM_OCI_NETWORK_TESTS=1` + `MIKEBOM_OCI_AUTH_TESTS=1` for the auth path):

- `mikebom sbom scan --image ghcr.io/<private-org>/<private-image>:latest`
  succeeds when `~/.docker/config.json` contains a valid PAT for `ghcr.io`.
- `mikebom sbom scan --image <hub-priv>/<repo>:tag` succeeds when
  `~/.docker/config.json` declares either a direct `auth` field or
  `credsStore: desktop` (or a per-registry `credHelpers` entry).
- `RUST_LOG=debug mikebom sbom scan --image <ref>` does NOT leak the secret
  in any log line.
- `mikebom sbom scan --image <ref>` without auth (no config.json + public
  image) continues to work exactly as today.

## Acceptance scenarios

**Scenario 1: Direct `auth` field in `~/.docker/config.json`**
```
Given: ~/.docker/config.json contains
       { "auths": { "ghcr.io": { "auth": "<base64(user:pat)>" } } }
When:  mikebom sbom scan --image ghcr.io/private/repo:tag
Then:  the bearer-token fetch to ghcr.io's realm sends Basic auth using the
       decoded user:pat; the resulting bearer token authorizes the manifest
       fetch; scan completes; SBOM contains components.
```

**Scenario 2: `credHelpers` per-registry override**
```
Given: ~/.docker/config.json contains
       { "credHelpers": { "<reg>": "<helper>" } }
       and `docker-credential-<helper>` exists on PATH and returns
       {"Username": "...", "Secret": "...", "ServerURL": "<reg>"} on stdin
       "<reg>".
When:  mikebom sbom scan --image <reg>/...
Then:  mikebom invokes the helper subprocess, parses Username+Secret, uses
       them as Basic auth on the bearer-token realm fetch; scan completes.
```

**Scenario 3: `credsStore` registry-wide helper**
```
Given: ~/.docker/config.json contains
       { "auths": { "<reg>": {} }, "credsStore": "<helper>" }
       (the empty auths entry is the standard "credentials live in the helper"
       marker that `docker login` writes when credsStore is configured.)
When:  mikebom sbom scan --image <reg>/...
Then:  mikebom invokes `docker-credential-<helper> get` with stdin "<reg>";
       same outcome as Scenario 2.
```

**Scenario 4: No credentials → graceful anonymous fallback**
```
Given: no ~/.docker/config.json (or no entry for the target registry)
When:  mikebom sbom scan --image <public-ref>
Then:  scan behaves identically to milestone 031 — anonymous bearer-token
       handshake. No auth error.
```

**Scenario 5: Wrong credentials → clear error, no secret leak**
```
Given: ~/.docker/config.json contains an invalid PAT for <reg>
When:  mikebom sbom scan --image <reg>/private/...
Then:  mikebom reports "registry authentication failed for <reg> (401)".
       The PAT is NOT included in stdout, stderr, or any log line at any
       --verbose / RUST_LOG level.
```

## Edge cases

- **`identitytoken` field**: some registries (notably Azure ACR and some Hub
  flows) use `identitytoken` instead of `auth`. When present, send it as
  `Authorization: Bearer <identitytoken>` directly to the realm token
  endpoint (or as the bearer for the manifest GET, depending on registry).
  For 034, treat `identitytoken` as a synonym for "Basic-auth via
  username=`<token>`, password=empty, then bearer-token retry"; this is what
  `oras` does and matches Hub's documented flow.
- **Helper exit code != 0**: cred helper failure → fall through to anonymous
  (don't hard-fail; the user might still have public access). Log a single
  `tracing::warn!` line that names the helper but NOT its stderr (helper
  stderr can leak partial creds in some implementations).
- **Helper stdin contract**: per the
  [Docker cred-helper API](https://github.com/docker/docker-credential-helpers),
  stdin is the registry hostname (no scheme, no path). For `docker.io` we
  send `https://index.docker.io/v1/` (the legacy realm Docker uses for Hub).
- **registry name normalization**: config.json keys can be `https://...`,
  `http://...`, bare hostname, or with a path. Normalize to bare hostname
  for matching, and treat `index.docker.io` / `https://index.docker.io/v1/`
  / `docker.io` as equivalent.
- **Secret material in `Debug` impl**: the `Credential` struct must NOT
  derive `Debug` for its secret field (or must redact it). Inline tests
  assert that `format!("{:?}", credential)` does not contain the secret.
- **Empty `auth` field**: some `docker login` flows write `{"auth": ""}` as
  a marker. Treat empty `auth` the same as a missing `auths` entry → fall
  through to credsStore / anonymous.
- **ECR**: the `docker-credential-ecr-login` helper handles AWS SigV4 and
  STS; mikebom invokes it the same way as any other helper. Native AWS SDK
  integration is explicitly out of scope (deferred).

## Functional requirements

- **FR-001**: `mikebom-cli/src/scan_fs/oci_pull/auth.rs` is a new module
  behind the `oci-registry` Cargo feature, exporting (crate-private):
  - `pub(super) struct DockerConfig` — parsed `~/.docker/config.json`.
  - `pub(super) struct Credential { username: String, secret: String }`
    where `Debug` is hand-implemented to redact the secret.
  - `pub(super) fn load_default_docker_config() -> Option<DockerConfig>`
    reading `$HOME/.docker/config.json` (or `$DOCKER_CONFIG/config.json` if
    set). Missing/unreadable file → returns `None`, NOT an error.
  - `pub(super) fn resolve_credentials(cfg: &DockerConfig, registry: &str)
    -> Option<Credential>` — implements the precedence:
    `credHelpers` (per-registry) > `credsStore` (registry-wide) >
    `auths.<reg>.auth` direct credentials > `auths.<reg>.identitytoken` >
    None.

- **FR-002**: `mikebom-cli/src/scan_fs/oci_pull/auth.rs::run_credential_helper`
  invokes `docker-credential-<helper>` as a subprocess with the registry
  hostname on stdin, parses `{Username, Secret, ServerURL}` JSON from
  stdout. On non-zero exit OR stdout containing "credentials not found"
  (helper convention for "no credentials cached"), returns `None`. No
  helper stderr is captured into mikebom's logs.

- **FR-003**: `mikebom-cli/src/scan_fs/oci_pull/registry.rs::RegistryClient`
  gains an optional `credentials: Option<Credential>` field, populated in
  `RegistryClient::new(reference)` by calling
  `auth::load_default_docker_config()` + `auth::resolve_credentials(&cfg,
  &reference.registry)`.

- **FR-004**: `registry.rs::fetch_bearer_token` adds `Basic <b64(user:secret)>`
  to the realm token-fetch request when credentials are available.
  Anonymous fetch is preserved when credentials are absent.

- **FR-005**: `registry.rs::fetch_with_bearer_retry` updates the 401-after-
  retry error message: when credentials WERE used and the retry still
  returned 401, emit "registry authentication failed for <registry> (401).
  Verify credentials in ~/.docker/config.json or your credential helper."
  When credentials were NOT used, preserve a hint pointing to milestone
  034's auth modes (config.json + cred helpers).

- **FR-006**: NO secrets appear in any `tracing::*` macro invocation, any
  `anyhow::Context` chain, any `eprintln!`, or any `Debug` formatting.
  Inline tests assert this for `Credential::Debug` and for the auth-failure
  error message.

- **FR-007**: `mikebom-cli/tests/no_c_dependencies.rs::no_c_dependencies_in_oci_registry_feature_tree`
  continues to pass — no new C-bound deps introduced. Uses only existing
  `base64`, `serde_json`, `reqwest`, `std::process` workspace surface.

- **FR-008**: All existing `oci_pull` tests pass unchanged. New inline tests
  in `auth.rs` cover: config.json parsing for all 4 cred sources, registry
  name normalization, `Credential::Debug` redaction, helper subprocess via
  a synthetic `docker-credential-mikebomtest` shim built for the test, and
  empty-auth fall-through.

## Success criteria

- **SC-001**: `./scripts/pre-pr.sh` clean (default lane).
- **SC-002**: `MIKEBOM_OCI_NETWORK_TESTS=1 MIKEBOM_OCI_AUTH_TESTS=1
  cargo +stable test -p mikebom --test oci_registry_smoke
  -- --include-ignored` passes when `~/.docker/config.json` is configured
  for the target registry. (Smoke test is `#[ignore]`'d in CI; documented in
  the PR description for manual verification.)
- **SC-003**: `git diff main..HEAD -- mikebom-cli/src/cli/ mikebom-cli/src/generate/` is empty —
  no CLI / generator changes.
- **SC-004**: `wc -l mikebom-cli/src/scan_fs/oci_pull/auth.rs` ≤ 500.
- **SC-005**: Adding the `034` work introduces zero new top-level deps in
  `mikebom-cli/Cargo.toml`. (`base64`, `serde_json`, `reqwest`, and
  `std::process::Command` already cover everything.)
- **SC-006**: Grep audit: `rg 'tracing::|anyhow::|eprintln!' mikebom-cli/src/scan_fs/oci_pull/auth.rs`
  shows zero formatters that interpolate `secret` / `password` / `token`
  field values. (Mechanical guard against accidental leaks; FR-006 is the
  semantic guarantee.)
- **SC-007**: All 3 CI lanes green on the milestone PR.

## Clarifications

- **Why bearer-token + credentials (not Basic-auth-direct)**: All major
  registries (Hub, GHCR, GCR, ECR, ACR, Quay) use bearer tokens for the
  manifest/blob endpoints; credentials authorize the *token* fetch, not the
  manifest fetch. Direct Basic auth on manifest GETs works only for
  self-hosted Distribution registries with `Basic` realms — niche enough
  that we defer to a follow-on if it ever comes up.
- **Why the existing `base64` dep is enough**: `base64::engine::general_purpose::STANDARD`
  decodes `auth` fields; `URL_SAFE` is not needed (Docker uses standard
  base64 with padding). No new crate.
- **Why no `dirs` crate**: `std::env::var("HOME")` covers Linux/macOS;
  `DOCKER_CONFIG` env override (already standard) handles the Windows /
  alternative-config case. Adding `dirs` for one path lookup is overkill.
- **Helper-subprocess timeout**: 5 seconds. Cred helpers normally complete
  in <100ms; 5s is generous enough for ECR's STS round-trip on a slow
  network without making mikebom hang indefinitely.

## Out of scope

- Push (mikebom is read-only).
- Registry mirror configs (`registries.conf` / `mirrors`).
- OAuth refresh-token flows (separate from the bearer-token-with-creds path
  this milestone covers).
- Native AWS SDK integration for ECR (the standard cred helper covers it).
- `--registry-auth user:pass@host` CLI flag (deferred — start with
  config.json since that's where users' creds already live).
- 031.y `--image-platform` (#67) and 031.z layer caching (#68) remain
  separate follow-ons.
