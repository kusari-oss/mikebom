---
description: "Replace `oci-client = \"0.12\"` with `oci-spec = \"0.9\"` (pure-Rust types-only crate) + a thin custom HTTP client built on the workspace's existing `reqwest 0.12 + rustls-tls (ring)`. Removes the version-pin trap that locked us out of future oci-client security updates."
status: spec
milestone: 032
---

# Spec: oci-client → oci-spec migration

## Background

PR #63 (milestone 031) shipped direct OCI registry image scanning
behind a `oci-registry` Cargo feature. The implementation depends on
`oci-client = "0.12"`, pinned to that specific version because
`oci-client = "0.16"` (latest) transitively bumps `reqwest` to 0.13 →
`rustls` 0.23+ → which uses **aws-lc-rs** as the default crypto
provider, dragging in `aws-lc-sys` (a `*-sys` crate wrapping the
AWS-LC C library). Constitution Principle I (Pure Rust, Zero C)
forbids this.

The pin works today (audit-test-locked) but goes stale: `oci-client
0.12.x` won't get security backports. Issue #65 tracks the planned
migration to a substrate that doesn't have this fragility — and this
spec executes #65.

The replacement strategy: split the layers cleanly.
- **`oci-spec`** (youki-dev, pure-Rust types-only) provides the
  data model (manifest, descriptor, image config, manifest list).
  Tiny stable dep graph: `serde / serde_json / thiserror /
  derive_builder / getset / strum / regex / const_format`. Already
  in mikebom's tree (or close).
- **Workspace `reqwest 0.12 + rustls-tls (ring)`** provides the
  HTTP transport — already pinned + audited from milestone 010
  era; no new TLS surface.
- **A new ~400-LOC `oci_pull/registry.rs` module** brings these
  together: manifest fetch, blob fetch, bearer-token retry flow
  for Docker Hub, sha256 digest verification.

This is the **same shape as milestone 029** (cargo-auditable used
`flate2 + serde_json` directly rather than the `auditable-extract`
wrapper crate) and **milestone 011** (SPDX 3 emission written
directly rather than via a hypothetical SPDX-3 crate). The pattern:
when a wrapper crate's dep graph drifts in ways we don't control,
work with the format directly using primitives we already trust.

## User story (US1, P1)

**As a contributor maintaining mikebom long-term**, I want the OCI
registry image-scan code to NOT be locked to a specific old
version of an upstream crate that we can't easily track forward,
so that future security advisories on the OCI registry path can be
addressed by updating mikebom's own code rather than waiting on
backports that won't come.

**Why P1**: Constitution-Principle-I durability. The current pin
is borrowed time. Best paid back deliberately, not under pressure.
And paying back BEFORE 031.x (auth) means the substantial auth
code lands on the durable substrate, not on `oci-client = "0.12"`
that we'd then have to migrate it through.

### Independent test

After implementation:
- `cargo tree -p mikebom --features oci-registry` shows zero
  `oci-client*` and zero `aws-lc*` entries.
- `mikebom-cli/tests/no_c_dependencies.rs::no_c_dependencies_in_oci_registry_feature_tree`
  continues passing (the regression test added in milestone 031
  is the durability guardrail).
- `MIKEBOM_OCI_NETWORK_TESTS=1 cargo test -p mikebom
  --features oci-registry --test oci_registry_smoke` passes — same
  binary-output as the oci-client implementation: 15 components
  for `alpine:3.19`, 0 components for `gcr.io/distroless/static-debian12:latest`.
- All inline tests in `oci_pull` continue to pass.
- Default profile: `cargo tree -p mikebom` is unchanged from
  pre-milestone (no new deps in default graph).

## Acceptance scenarios

**Scenario 1: Output parity vs. oci-client**
```
Given: any anonymous-public-registry image (e.g. alpine:3.19,
       gcr.io/distroless/static-debian12:latest, ghcr.io/some/public:tag)
When:  mikebom scans it under both pre-migration (oci-client) and
       post-migration (oci-spec + custom client) builds
Then:  the emitted CDX SBOMs are byte-identical modulo the
       generation timestamp and serial number. Same components,
       same PURLs, same dep edges.
```

**Scenario 2: Docker Hub bearer-token flow**
```
Given: an image ref pointing at Docker Hub (e.g. alpine:3.19,
       library/alpine:3.19, docker.io/library/alpine:3.19)
When:  the new registry client requests the manifest
Then:  the registry returns 401 with WWW-Authenticate: Bearer
       realm="https://auth.docker.io/token",service="registry.docker.io",scope="repository:library/alpine:pull".
       The client fetches a token from the realm, retries with
       Authorization: Bearer <token>, and proceeds with the pull.
       NO credentials are sent (anonymous flow); the token is just
       a registry-required handshake.
```

**Scenario 3: Direct anonymous (no token flow)**
```
Given: an image ref pointing at a registry that allows direct
       anonymous fetch (e.g. gcr.io/distroless/static-debian12:latest)
When:  the new registry client requests the manifest
Then:  the registry returns 200 immediately. NO token-fetch round
       trip. Pull proceeds.
```

**Scenario 4: Manifest list (multi-arch) resolution**
```
Given: a multi-arch image (e.g. alpine:3.19 ships linux/amd64,
       linux/arm64, linux/arm/v6, linux/arm/v7, linux/386, linux/ppc64le,
       linux/s390x)
When:  pull_to_tarball runs on a host where host_oci_arch() returns "arm64"
Then:  the linux/arm64 manifest is selected. Pull proceeds against
       that platform-specific manifest.
```

**Scenario 5: Reference parsing edge cases**
```
Given: various ref shapes:
       - bare `alpine` → `docker.io/library/alpine:latest`
       - `alpine:3.19` → `docker.io/library/alpine:3.19`
       - `library/alpine:3.19` → `docker.io/library/alpine:3.19`
       - `docker.io/library/alpine:3.19` → as-is
       - `gcr.io/foo/bar:tag` → as-is
       - `localhost:5000/foo/bar:tag` → registry=localhost:5000
       - `ghcr.io/foo/bar@sha256:0123...` → digest-form
When:  the new ref parser runs
Then:  each parses to the same registry/repo/tag/digest tuple as
       oci-client's `Reference::try_from(...)` would have.
```

**Scenario 6: Layer digest verification on the wire**
```
Given: a layer descriptor declaring `sha256:abc...`
When:  the registry returns bytes whose actual sha256 hash != abc
       (corruption, MITM, registry bug)
Then:  the pull aborts with a clear "layer digest mismatch" error.
       NO partial / unverified data is written to the tarball.
```

## Edge cases

- **HTTP redirects**: registries commonly redirect to a CDN
  (e.g., `production.cloudflare.docker.com`). reqwest 0.12 follows
  redirects by default; verify the bearer token follows redirects
  correctly (some clients don't propagate Authorization across
  hosts — that's intentional for security but breaks pulls if the
  CDN URL is HTTPS-redirected). reqwest's default behavior preserves
  Authorization across same-origin redirects but strips on
  cross-origin; manifest fetches typically stay same-origin so
  this should work.

- **Manifest list with no `linux/<host>` entry**: same error
  message as today's implementation — list available platforms.

- **Registries that DO require auth even for "public"** (some
  GitLab Container Registries, internal mirrors): the bearer-token
  flow we implement for Docker Hub generalizes — fetch the token
  from the realm in `WWW-Authenticate`, retry. **Anonymous-only
  scope still applies**: if the realm is reachable without
  credentials, this works. If the realm itself rejects, return the
  same "auth not yet supported in milestone 031" error.

- **Streaming vs. buffered**: layer blobs can be 100s of MB. We
  buffer them fully in memory (matches today's behavior with
  oci-client). Streaming-to-disk is a performance optimization
  deferred to layer-caching milestone (#68).

- **Connection reuse**: reqwest's `Client` reuses connections
  internally. Construct one Client per `pull_to_tarball` invocation
  (not per blob fetch) to amortize TLS handshake cost across
  manifest + N layer fetches.

- **Manifest media-type negotiation**: send
  `Accept: application/vnd.oci.image.manifest.v1+json,
  application/vnd.oci.image.index.v1+json,
  application/vnd.docker.distribution.manifest.v2+json,
  application/vnd.docker.distribution.manifest.list.v2+json`.
  Inspect the response's `Content-Type` to dispatch to the right
  oci-spec deserializer.

- **Error mapping**: every `?` propagates with `anyhow::Context`
  naming the failure point (parse / manifest / token-fetch /
  layer-fetch / digest-mismatch / decompress / tarball-write).

## Functional requirements

- **FR-001**: `mikebom-cli/Cargo.toml` removes `oci-client` from
  `[dependencies]` and the `oci-registry` feature. Adds
  `oci-spec = { version = "0.9", default-features = false,
  features = ["distribution", "image"], optional = true }` in its
  place. The `oci-registry` feature now declares
  `oci-registry = ["dep:oci-spec"]`.

- **FR-002**: `mikebom-cli/src/scan_fs/oci_pull.rs` (single file
  today) becomes `mikebom-cli/src/scan_fs/oci_pull/` (directory)
  with submodules:
  - `mod.rs` — public `pull_to_tarball` + `ImageArgKind` +
    `detect_image_arg_kind` + `host_oci_arch` (unchanged surface).
  - `reference.rs` — image-ref parser (`parse_reference(&str) ->
    Result<ImageReference>` returning a struct with registry, repo,
    tag, digest fields).
  - `registry.rs` — async HTTP client: manifest fetch,
    bearer-token retry, blob fetch, digest verification.
  - `platform.rs` — manifest-list → platform-specific manifest
    selection (mirrors today's custom platform_resolver closure).
  - `tarball.rs` — docker-save-format assembly (preserved from
    today's implementation; minimal/no changes).

- **FR-003**: `reference.rs::parse_reference` accepts the same ref
  shapes today's `oci_client::Reference::try_from` accepts. Tests
  cover all 7 shapes from Scenario 5.

- **FR-004**: `registry.rs` uses workspace `reqwest::Client`
  (rustls-tls / ring). Manifest endpoint:
  `GET /v2/<repo>/manifests/<reference>`. Blob endpoint:
  `GET /v2/<repo>/blobs/<digest>`. Bearer-token flow on 401
  with `Www-Authenticate: Bearer realm=...,service=...,scope=...`:
  fetch the token URL with the parsed realm/service/scope query
  params, parse `{"token": "..."}` JSON, retry the original
  request with `Authorization: Bearer <token>`.

- **FR-005**: `registry.rs::fetch_blob` verifies the SHA-256 of
  the received bytes against the descriptor's declared digest
  before returning. Mismatch → error with both digests in the
  message.

- **FR-006**: `platform.rs::resolve_manifest_list` walks an
  `oci_spec::image::ImageIndex` (or Docker manifest-list
  equivalent) and selects the entry matching `host_oci_arch()`'s
  output + `os: "linux"`. Same behavior as today's custom resolver
  closure. No matching platform → error listing available
  platforms.

- **FR-007**: `tarball.rs::assemble_docker_save_tarball` is the
  existing implementation, preserved (mostly). Imports of
  oci-client types replaced with equivalents from oci-spec or
  small newtypes defined locally.

- **FR-008**: Inline tests in each new submodule:
  - `reference.rs::tests` — 7 ref-parsing cases (Scenario 5)
    + invalid inputs.
  - `registry.rs::tests` — bearer-token-realm parsing helper +
    sha256-verify pass/fail. Network calls themselves NOT covered
    (covered by the smoke test).
  - `platform.rs::tests` — synthetic manifest-list selection
    + missing-platform error.
  - `tarball.rs::tests` — preserved
    `assemble_docker_save_tarball_round_trips_via_extract` test
    + `assert_layers_supported_rejects_zstd`.

- **FR-009**: `mikebom-cli/tests/oci_registry_smoke.rs` — same
  two existing tests (alpine:3.19 / distroless), unchanged. They
  exercise the full new stack end-to-end when
  `MIKEBOM_OCI_NETWORK_TESTS=1`.

- **FR-010**: `mikebom-cli/tests/no_c_dependencies.rs` — the
  feature-on tree test continues to pass. No other regression
  test adjustments needed (the blacklist already includes
  `aws-lc-sys`; oci-spec doesn't pull it).

- **FR-011**: `docs/user-guide/cli-reference.md` — no doc changes
  required (the `--image <tar-or-ref>` flag's behavior is
  unchanged externally).

- **FR-012**: Per-commit `./scripts/pre-pr.sh` clean BOTH
  default + feature-on profiles. Three atomic commits
  (substrate / migration / cleanup).

## Success criteria

- **SC-001**: `./scripts/pre-pr.sh` clean default; clippy clean
  feature-on; `cargo test --features oci-registry` clean (smoke
  test silently skips without env var).

- **SC-002**: `cargo tree -p mikebom --features oci-registry |
  grep -E "oci-client|aws-lc"` produces zero matches. Both crates
  are gone from the dep graph.

- **SC-003**: Default `cargo tree -p mikebom` (no features)
  unchanged from pre-milestone.

- **SC-004**: Smoke test against `alpine:3.19` (host-arch) yields
  the same component count + same PURLs as the pre-migration
  baseline. Smoke test against
  `gcr.io/distroless/static-debian12:latest` yields zero
  components (matches pre-migration).

- **SC-005**: `git diff main..HEAD --
  mikebom-cli/src/scan_fs/binary/ mikebom-cli/src/scan_fs/package_db/
  mikebom-cli/src/parity/ mikebom-common/`
  empty. The migration touches only the oci_pull module
  + Cargo.toml + the tests-dir for verification.

- **SC-006**: All 3 standard CI lanes (Linux default + Linux ebpf
  + macOS) green on the default profile.

- **SC-007**: 27-golden regen produces zero diff.

- **SC-008**: New module total LOC ≤ 700 (production + tests
  combined), distributed as ~400 production + ~300 test surface.
  Today's `oci_pull.rs` is 408 LOC total; this milestone roughly
  doubles it but spreads across submodules for readability.

## Clarifications

- **Reference parsing rules** (FR-003 acceptance):
  - The first path segment is a registry hostname iff it contains
    `.` or `:` OR equals `localhost`. Otherwise the whole prefix
    is a repository path under `docker.io`.
  - When the path under `docker.io` has no `/`, prepend `library/`
    (Docker Hub's official-images convention).
  - Tag defaults to `latest` if neither tag nor digest is present.
  - Digest takes precedence over tag when both are present (rare
    but valid).

- **Bearer-token flow scope**: anonymous-only. The token request
  to the realm is itself unauthenticated. If the realm rejects
  (e.g., it requires credentials for that scope), the pull fails
  with the same "auth not yet supported" error today's
  implementation produces — auth handling is milestone 031.x's
  problem. This milestone preserves anonymous-only.

- **No new transitive deps beyond oci-spec's**: the spec lists
  oci-spec's full transitive dep set (in plan.md); none are
  C-bound. The `no_c_dependencies` regression test is the
  enforcement.

- **Behavior parity is the primary success criterion**: the new
  implementation MUST produce the same SBOM output for the same
  inputs as the old. Any divergence is a bug, not a feature
  improvement.

## Out of scope

- **Auth** — milestone 031.x (#66).
- **`--image-platform` flag** — milestone 031.y (#67).
- **Layer caching** — milestone 031.z (#68).
- **Streaming layer extraction** — deferred indefinitely.
- **OCI signature verification** — separate concern; out of
  scope.
- **Push** — mikebom is read-only.
- **The `host_oci_arch` mapping** is unchanged (keeps the same
  six host arches mapped). 031.y will plumb a `--image-platform`
  override on top.
- **The `ImageArgKind` / `detect_image_arg_kind` surface is
  unchanged** — same trichotomy as milestone 031.
