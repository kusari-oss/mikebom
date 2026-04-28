---
description: "Task list — milestone 034 authenticated OCI registry pulls"
---

# Tasks: Authenticated OCI registry pulls — Tighter Spec

**Input**: Design documents from `/specs/034-authenticated-registry-pulls/`
**Prerequisites**: spec.md (✅), plan.md (✅), checklists/requirements.md (✅)

**Tests**: ~10 inline tests in `auth.rs` + a mock-server integration test in `registry.rs` + a gated network smoke test in `tests/oci_registry_smoke.rs`. The existing 31-fixture byte-identity goldens are untouched (no SBOM-shape changes).

**Organization**: Single user story (US1, P1). Three atomic commits.

## Path Conventions

- Adds `mikebom-cli/src/scan_fs/oci_pull/auth.rs` (new module, ~400 LOC).
- Touches `mikebom-cli/src/scan_fs/oci_pull/{mod,registry}.rs` (additive).
- Touches `mikebom-cli/tests/oci_registry_smoke.rs` (additive — new gated test).
- Touches `docs/user-guide/cli-reference.md` (additive — new auth subsection).
- Touches `mikebom-cli/Cargo.toml` (comment update only on `oci-registry` feature).
- Touches `CHANGELOG.md` (one new entry).
- Does NOT touch any other module, CLI command, parity extractor, or test.

---

## Phase 1: Setup + baseline

- [X] T001 Recon done in this session (2026-04-26): registry.rs's auth seam is `fetch_with_bearer_retry` (line 107) calling `fetch_bearer_token` (line 166); both currently anonymous-only. 401-after-retry error message points users at issue #66 — that hint will be updated in commit 2.
- [ ] T002 Snapshot baseline: `./scripts/pre-pr.sh 2>&1 | tee /tmp/baseline-034.txt | grep -E '^test [a-z_:]+ \.\.\. ok' | sort -u > /tmp/baseline-034-tests.txt`. Confirm post-034 test list shows additions only.

---

## Phase 2: Commit 1 — `034/auth-module`

**Goal**: New `auth.rs` with config.json parsing, cred-helper subprocess, redacting Credential type. No call sites yet.

- [ ] T003 [US1] Create `mikebom-cli/src/scan_fs/oci_pull/auth.rs`. Header doc-comment names the milestone, links to the Docker cred-helper API doc, and notes the redaction discipline.
- [ ] T004 [US1] Add `Credential { username: String, secret: String }` with hand-written `Debug` (`Credential { username: "<redacted>", secret: "<redacted>" }`). Inline test asserts `format!("{:?}", c)` does not contain the secret.
- [ ] T005 [US1] Add `DockerConfig` serde struct: `auths: HashMap<String, AuthEntry>`, `creds_store: Option<String>` (rename `credsStore`), `cred_helpers: HashMap<String, String>` (rename `credHelpers`). All fields `#[serde(default)]`. `AuthEntry { auth: Option<String>, identitytoken: Option<String> }`.
- [ ] T006 [US1] Add `fn load_default_docker_config() -> Option<DockerConfig>`: prefer `$DOCKER_CONFIG/config.json`, fall back to `$HOME/.docker/config.json`. Missing/unreadable file → None (NOT error). Malformed JSON → None + `tracing::warn!` (no path content beyond the path itself).
- [ ] T007 [US1] Add `fn normalize_registry_key(s: &str) -> String`: strip `https://` / `http://` schemes, strip `/v1/` / `/v2/` paths, lowercase the host, treat `index.docker.io` and `docker.io` as the same. Inline tests for the 4 forms (`docker.io`, `index.docker.io`, `https://index.docker.io/v1/`, `https://index.docker.io/v2/`).
- [ ] T008 [US1] Add `fn resolve_credentials(cfg: &DockerConfig, registry: &str) -> Option<Credential>` implementing the precedence per FR-001:
  1. `credHelpers.<reg>` → run helper subprocess.
  2. `credsStore` → run helper subprocess (any registry).
  3. `auths.<reg>.auth` → base64-decode → split on `:` → username, secret.
  4. `auths.<reg>.identitytoken` → return `Credential { username: "<token>", secret: "" }` (Hub-style identity-token flow).
- [ ] T009 [US1] Add `fn run_credential_helper(helper: &str, registry: &str) -> Option<Credential>`: `Command::new("docker-credential-{helper}").args(["get"]).stdin(piped).stdout(piped).stderr(null).spawn()`, write registry hostname + newline to stdin, wait with 5s timeout. On non-zero exit OR stdout containing `"credentials not found"` → None. Otherwise parse `{Username, Secret, ServerURL}` JSON and return `Credential`.
- [ ] T010 [US1] Inline test for T008's auth-only flow: synthetic config.json with `auths.<reg>.auth = b64("user:pat")` → expect `Credential { username: "user", secret: "pat" }`.
- [ ] T011 [US1] Inline test for T008's identitytoken flow: synthetic config.json with `auths.<reg>.identitytoken = "tok"` → expect Credential populated.
- [ ] T012 [US1] Inline test for T008's empty-auths fall-through: `auths.<reg> = {}` + no credsStore → None.
- [ ] T013 [US1] [unix-only `#[cfg(unix)]`] Inline test for T009's helper subprocess: write a `docker-credential-mikebomtest` shell script to a tempdir, prepend tempdir to `PATH`, configure DockerConfig with `credHelpers.<reg> = "mikebomtest"`, assert resolve_credentials returns the script's hard-coded Credential.
- [ ] T014 [US1] [unix-only] Inline test for T009's "credentials not found" fall-through: shim script that prints "credentials not found" on stdout and exits 1 → resolve_credentials returns None (NOT error).
- [ ] T015 [US1] Edit `mikebom-cli/src/scan_fs/oci_pull/mod.rs` to add `mod auth;`.
- [ ] T016 [US1] Add `#[allow(dead_code)]` on the new pub(super) items (will be lifted in commit 2).
- [ ] T017 [US1] `./scripts/pre-pr.sh` clean.
- [ ] T018 [US1] Commit: `feat(034/auth-module): add Docker keychain credential resolver (config.json + cred helpers)`.

---

## Phase 3: Commit 2 — `034/wire-auth-into-registry`

**Goal**: `RegistryClient` resolves credentials at construction; `fetch_bearer_token` applies Basic auth on the realm GET when credentials are available.

- [ ] T019 [US1] Edit `RegistryClient::new` signature: take `reference: &ImageReference`, store `credentials: Option<Credential>` populated by calling `auth::load_default_docker_config()` + `auth::resolve_credentials(&cfg, &reference.registry)`.
- [ ] T020 [US1] Edit `mod.rs::pull_to_tarball`: pass `&reference` to `RegistryClient::new`.
- [ ] T021 [US1] Edit `fetch_bearer_token` to take `creds: Option<&Credential>` (or thread through `self.credentials`). When present, call `req.basic_auth(&c.username, Some(&c.secret))` on the realm GET request.
- [ ] T022 [US1] Update the 401-after-retry error message in `fetch_with_bearer_retry` per FR-005:
  - Credentials were used: "registry authentication failed for `<registry>` (401). Verify credentials in ~/.docker/config.json or your credential helper."
  - Credentials were absent: existing wording, but updated to point at milestone 034: "Authenticated registries: configure ~/.docker/config.json (auth/identitytoken) or a credential helper. See `mikebom sbom scan --help`."
- [ ] T023 [US1] Remove `#[allow(dead_code)]` from `auth.rs` items now that they're called.
- [ ] T024 [US1] Inline integration test in `registry.rs` (test module): use a hyper-tiny mock returning 401 + `WWW-Authenticate: Bearer realm="http://127.0.0.1:<port>/token",service="<reg>"`, expect mikebom's GET to the token endpoint with `Authorization: Basic <b64(user:pat)>`. Use `tokio::net::TcpListener` directly to avoid adding a mock-server crate.
- [ ] T025 [US1] Verify: `cargo +stable test -p mikebom --test oci_registry` (or whatever covers the inline registry tests) is green.
- [ ] T026 [US1] `./scripts/pre-pr.sh` clean.
- [ ] T027 [US1] Commit: `feat(034/wire-auth-into-registry): apply Docker-keychain credentials on the bearer-token realm fetch`.

---

## Phase 4: Commit 3 — `034/docs-and-smoke`

**Goal**: User-facing docs, CHANGELOG, smoke test, Cargo.toml feature comment refresh.

- [ ] T028 [US1] Edit `docs/user-guide/cli-reference.md`: add "Authenticating to private registries" subsection under `--image`, covering: Docker config.json layout (auth / identitytoken), credHelpers per-registry, credsStore registry-wide, ECR helper note (≥12h tokens — OK for one-shot scans), GHCR `read:packages` PAT scope reminder.
- [ ] T029 [US1] Edit `CHANGELOG.md`: add unreleased entry — "OCI registry image scan now supports private registries via the standard Docker keychain (~/.docker/config.json + credential helpers). Closes #66."
- [ ] T030 [US1] Edit `mikebom-cli/Cargo.toml`: update the `oci-registry` feature comment from "Anonymous public registries only…" to "Anonymous + Docker-keychain authenticated pulls (config.json + credential helpers)."
- [ ] T031 [US1] Edit `mikebom-cli/tests/oci_registry_smoke.rs`: add `#[ignore]` test gated on `MIKEBOM_OCI_NETWORK_TESTS=1` AND `MIKEBOM_OCI_AUTH_TESTS=1`. Pulls a private image whose ref is set via `MIKEBOM_OCI_AUTH_PRIVATE_IMAGE_REF` env var. Documented in PR description as user-side verification.
- [ ] T032 [US1] Verify: `MIKEBOM_OCI_NETWORK_TESTS=1 cargo +stable test -p mikebom --test oci_registry_smoke -- --ignored` works for the AUTHENTICATED smoke test if the environment is set up; existing anonymous smoke tests still pass.
- [ ] T033 [US1] Grep audit per SC-006: `rg 'tracing::|anyhow::|eprintln!' mikebom-cli/src/scan_fs/oci_pull/auth.rs` — manual review that none of the formatters interpolate `secret` / `password` / `token` field values. (Empty findings or only safe formatters.)
- [ ] T034 [US1] Verify: `cargo +stable test -p mikebom --test no_c_dependencies` — both regression tests still pass.
- [ ] T035 [US1] `./scripts/pre-pr.sh` clean.
- [ ] T036 [US1] Commit: `feat(034/docs-and-smoke): user-guide auth section + CHANGELOG + gated private-registry smoke test`.

---

## Phase 5: Verification + PR

- [ ] T037 SC-001: `./scripts/pre-pr.sh` clean.
- [ ] T038 SC-003: `git diff main..HEAD -- mikebom-cli/src/cli/ mikebom-cli/src/generate/` empty.
- [ ] T039 SC-004: `wc -l mikebom-cli/src/scan_fs/oci_pull/auth.rs` ≤ 500.
- [ ] T040 SC-005: `git diff main..HEAD -- mikebom-cli/Cargo.toml mikebom-common/Cargo.toml mikebom-ebpf/Cargo.toml Cargo.toml | grep -E '^\+[a-z][a-z0-9_-]+ = '` — should show ZERO new dep lines (only feature-comment changes).
- [ ] T041 SC-006: re-run grep audit from T033 + manual eyeball of auth.rs's tracing/anyhow surface.
- [ ] T042 27-golden regen: `MIKEBOM_UPDATE_*_GOLDENS=1 ./scripts/pre-pr.sh` — expected: ZERO diff (no SBOM-shape changes from this milestone).
- [ ] T043 Push branch; observe all 3 CI lanes green (SC-007).
- [ ] T044 Author the PR description: 3-commit summary, recon-context pointer to spec.md, scope reminder ("anonymous fallback preserved"), test instructions for the gated auth smoke test.

---

## Dependency graph

```text
T001 (recon, done) → T002 (baseline)
                       │
                       ↓
              T003-T018 [Commit 1: auth.rs + inline tests, dead-code allowed]
                       │
                       ↓
              T019-T027 [Commit 2: wire-up + mock-server test]
                       │
                       ↓
              T028-T036 [Commit 3: docs + smoke + Cargo.toml]
                       │
                       ↓
              T037-T044 (verify + PR)
```

## Estimated effort

| Phase | Effort | Notes |
|---|---|---|
| Phase 1 (baseline) | 5 min | T001 done; just snapshot |
| Phase 2 (auth module) | 6 hr | DockerConfig schema + subprocess shim test is the careful step |
| Phase 3 (wire-up) | 3 hr | Mock-server inline test |
| Phase 4 (docs + smoke) | 2 hr | Mostly text |
| Phase 5 (verify + PR) | 1 hr | Goldens + CI watch |
| **Total** | **~12 hr** | One+ focused day. |
