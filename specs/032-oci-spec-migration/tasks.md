---
description: "Task list — milestone 032 oci-client → oci-spec migration"
---

# Tasks: oci-client → oci-spec migration

**Input**: Design documents from `/specs/032-oci-spec-migration/`
**Prerequisites**: spec.md (✅), plan.md (✅), checklists/requirements.md (✅), milestone 031 merged on main

**Tests**: 7+ inline ref-parsing tests + 3+ platform-resolver tests
+ 2+ bearer-token-parsing tests + 1 sha256-verify test +
preserved tarball-assembly test + preserved
`MIKEBOM_OCI_NETWORK_TESTS=1` smoke tests against alpine:3.19 +
distroless. The smoke tests are the behavior-parity gate.

**Organization**: Single user story (US1, P1). Three atomic commits.

## Path Conventions

- Touches `mikebom-cli/Cargo.toml` (swap oci-client → oci-spec).
- Touches `mikebom-cli/src/scan_fs/oci_pull.rs` (DELETED — promoted to directory).
- Adds `mikebom-cli/src/scan_fs/oci_pull/` directory with 5 submodules.
- Does NOT touch `mikebom-common/`, `mikebom-cli/src/cli/`,
  `mikebom-cli/src/parity/`, any `mikebom-cli/src/scan_fs/binary/`,
  any `mikebom-cli/src/scan_fs/package_db/`,
  `mikebom-cli/src/generate/`, `mikebom-cli/src/resolve/`,
  `mikebom-cli/tests/scan_binary.rs`, or
  `mikebom-cli/tests/oci_registry_smoke.rs` (the smoke test
  is preserved as-is — its passing IS the parity gate).

---

## Phase 1: Setup + baseline

- [X] T001 Recon done. Confirmed:
      - `reqwest 0.12 + rustls-tls (ring)` already in
        workspace `Cargo.toml:15` (no new HTTP/TLS deps needed).
      - `tokio 1 full` + `flate2`, `tar`, `tempfile`, `sha2`
        already in workspace.
      - oci-spec 0.9 maintained by youki-dev; pure-Rust + small
        clean dep graph (`serde`, `serde_json`, `thiserror`,
        `derive_builder`, `getset`, `strum`, `regex`,
        `const_format`).
      - Today's `oci_pull.rs` is 408 LOC; the migration roughly
        doubles total LOC but distributes across 5 submodules.
      - `no_c_dependencies_in_oci_registry_feature_tree` test
        already in place — locks in the no-aws-lc property
        across the dep swap.
      - PR #63 (milestone 031) merged on main (5fb9781).
- [ ] T002 Snapshot baseline:
      - `./scripts/pre-pr.sh 2>&1 | tee /tmp/baseline-032.txt | grep -cE '^test [a-z_:]+ \.\.\. ok' > /tmp/baseline-032-count.txt`
      - `cargo +stable test -p mikebom --features oci-registry 2>&1 | grep -cE '^test [a-z_:]+ \.\.\. ok' > /tmp/baseline-032-feature-count.txt`
      - Manual: `cargo run --features oci-registry -- sbom scan --image alpine:3.19 --output /tmp/baseline-alpine.json` — record component count for the parity gate.

---

## Phase 2: Commit 1 — `032/substrate`

**Goal**: Reorganize `oci_pull.rs` (single file) into `oci_pull/`
(directory) with 5 submodules. Add `reference.rs` + `platform.rs`
fresh. Tarball assembly + outer entry point preserved. oci-client
is STILL the substrate at this commit.

- [ ] T003 [US1] Delete
      `mikebom-cli/src/scan_fs/oci_pull.rs` and create
      `mikebom-cli/src/scan_fs/oci_pull/` directory.
- [ ] T004 [US1] `oci_pull/mod.rs` — public surface preserved
      from old file's top:
      - `pub fn pull_to_tarball(image_ref: &str) -> Result<TempDir>`
      - `pub enum ImageArgKind { Path, OciRef, Invalid }`
      - `pub fn detect_image_arg_kind(arg: &Path) -> ImageArgKind`
      - `pub fn host_oci_arch() -> Result<&'static str>`
      Module declarations: `mod reference; mod registry; mod platform; mod tarball;`
      `pull_to_tarball` body still calls oci-client at this
      commit (we'll swap in commit 2). Imports updated to
      reference the new submodules.
- [ ] T005 [US1] `oci_pull/tarball.rs` — extract
      `assemble_docker_save_tarball`, `decompress_layer`,
      `append_tarball_entry`, `sha256_hex`, `assert_layers_supported`
      verbatim from old file. Imports adjusted. The existing
      `assemble_docker_save_tarball_round_trips_via_extract` test
      moves with these helpers.
- [ ] T006 [US1] `oci_pull/reference.rs` — write fresh:
      ```rust
      pub struct ImageReference {
          pub registry: String,
          pub repository: String,
          pub tag: Option<String>,    // None when digest is set
          pub digest: Option<String>,  // None when tag is set
      }
      pub fn parse_reference(s: &str) -> Result<ImageReference>;
      ```
      Implements the 4 grammar rules (Spec §Clarifications):
      1. First path-segment is a registry hostname iff it contains
         `.` or `:` OR equals `localhost`. Otherwise the whole prefix
         is a repo path under `docker.io`.
      2. When the docker.io path has no `/`, prepend `library/`.
      3. Tag defaults to `latest` if neither tag nor digest present.
      4. Digest takes precedence over tag when both are present.
      8+ table-tests covering Spec Scenario 5 + invalid cases.
- [ ] T007 [US1] `oci_pull/platform.rs` — write fresh + extract
      from old file:
      ```rust
      pub fn resolve_manifest_list(
          entries: &[ImageIndexEntry],
          target_arch: &str,
      ) -> Result<String>;
      ```
      Returns the digest of the matching manifest. Errors when
      no entry matches `linux/<target_arch>`, listing the
      available platforms. 3+ tests covering happy path, no-match,
      multiple-matches-pick-first.
- [ ] T008 [US1] Verify dual-profile compile + tests:
      - `./scripts/pre-pr.sh` (default) clean.
      - `cargo +stable test -p mikebom --features oci-registry` —
        all prior tests pass + new ref/platform tests pass.
- [ ] T009 [US1] Commit:
      `refactor(032/substrate): split oci_pull into mod.rs + reference.rs + platform.rs + tarball.rs (oci-client still substrate)`.

---

## Phase 3: Commit 2 — `032/migration`

**Goal**: Write `registry.rs` (the thin HTTP client). Replace
oci-client integration in `mod.rs::pull_to_tarball` with calls
into the new substrate. oci-client crate is still in deps but
no longer used in production code.

- [ ] T010 [US1] Write `oci_pull/registry.rs`:
      - `pub struct RegistryClient { http: reqwest::Client }` —
        wraps a single reqwest Client for connection-pool reuse.
      - `pub async fn fetch_manifest(reference: &ImageReference) -> Result<ManifestOrIndex>`:
        - Build URL: `https://<registry>/v2/<repo>/manifests/<tag-or-digest>`.
        - `Accept` header lists 4 manifest media types (OCI v1 +
          v1 index + Docker v2 + v2 list).
        - On 401 with `Www-Authenticate: Bearer ...`: parse
          realm/service/scope, fetch token from realm, retry
          with `Authorization: Bearer <token>`.
        - On 200: dispatch on response `Content-Type`:
          - OCI manifest / Docker v2 manifest → ImageManifest
          - OCI index / Docker v2 list → ImageIndex
        - Return `ManifestOrIndex` enum.
      - `pub async fn fetch_blob(reference: &ImageReference,
        digest: &str) -> Result<Vec<u8>>`:
        - Build URL: `https://<registry>/v2/<repo>/blobs/<digest>`.
        - Same auth flow as fetch_manifest.
        - Verify SHA-256 of returned bytes matches `digest`.
        - Return verified bytes.
      - Helper: `parse_bearer_challenge(www_auth: &str) -> Result<BearerChallenge>` — small RFC 7235 parser for the `Bearer realm=...,service=...,scope=...` shape. ~30 LOC.
- [ ] T011 [US1] Replace `mod.rs::pull_to_tarball` body:
      ```rust
      pub async fn pull_to_tarball(image_ref: &str) -> Result<TempDir> {
          let reference = reference::parse_reference(image_ref)?;
          let client = registry::RegistryClient::new()?;
          let manifest = match client.fetch_manifest(&reference).await? {
              ManifestOrIndex::Manifest(m) => m,
              ManifestOrIndex::Index(idx) => {
                  let target_arch = host_oci_arch()?;
                  let digest = platform::resolve_manifest_list(&idx.manifests, target_arch)?;
                  let mut platform_ref = reference.clone();
                  platform_ref.digest = Some(digest);
                  platform_ref.tag = None;
                  match client.fetch_manifest(&platform_ref).await? {
                      ManifestOrIndex::Manifest(m) => m,
                      ManifestOrIndex::Index(_) => bail!("expected manifest, got nested index"),
                  }
              }
          };
          let config_bytes = client.fetch_blob(&reference, &manifest.config.digest).await?;
          let mut layer_bytes_with_media = Vec::new();
          for layer_desc in &manifest.layers {
              let bytes = client.fetch_blob(&reference, &layer_desc.digest).await?;
              layer_bytes_with_media.push((layer_desc.media_type.clone(), bytes));
          }
          tarball::assemble(image_ref, &config_bytes, &layer_bytes_with_media)
      }
      ```
- [ ] T012 [US1] Add inline tests in `registry.rs::tests`:
      - `parse_bearer_challenge_extracts_realm_service_scope` —
        Docker Hub's actual challenge format.
      - `parse_bearer_challenge_handles_quoted_values` — RFC 7235
        quoted-string variants.
      - `parse_bearer_challenge_rejects_non_bearer` — `Basic
        realm="x"` returns error.
      - sha256-verify test (already in tarball.rs::tests; verify
        the new pipeline still uses it).
- [ ] T013 [US1] Verify dual-profile compile + tests:
      - `./scripts/pre-pr.sh` (default) clean.
      - `cargo +stable test -p mikebom --features oci-registry` clean.
      - Manual smoke test: `cargo run --features oci-registry --
        sbom scan --image alpine:3.19 --output /tmp/032-alpine.json`
        — assert same component count as the T002 baseline.
- [ ] T014 [US1] Commit:
      `feat(032/migration): wire oci-spec + workspace reqwest as the registry substrate`.

---

## Phase 4: Commit 3 — `032/cleanup-oci-client`

**Goal**: Drop the `oci-client` dep entirely. The `oci-registry`
feature now points at oci-spec. Update spec/docs comments.

- [ ] T015 [US1] Edit `mikebom-cli/Cargo.toml`:
      - Remove `oci-client = { ... }` line from `[dependencies]`.
      - Update the `oci-registry` feature: change `["dep:oci-client"]`
        to `["dep:oci-spec"]`.
      - Add `oci-spec = { version = "0.9", default-features = false,
        features = ["distribution", "image"], optional = true }`.
      - Update the comment block above the feature declaration —
        replace oci-client references with oci-spec.
- [ ] T016 [US1] Verify the swap:
      - `cargo tree -p mikebom --features oci-registry | grep oci-client` empty.
      - `cargo tree -p mikebom --features oci-registry | grep aws-lc` empty.
      - `cargo +stable build -p mikebom --features oci-registry` clean.
      - `cargo +stable test -p mikebom --features oci-registry` clean.
      - `cargo +stable test -p mikebom --test no_c_dependencies` —
        2/2 pass (the `no_c_dependencies_in_oci_registry_feature_tree`
        test is the durability gate).
- [ ] T017 [US1] Update `mikebom-cli/src/scan_fs/oci_pull/mod.rs`
      module-doc-comment block:
      - Replace any `oci-client` mentions with `oci-spec + custom HTTP client`.
      - Cross-link to `specs/032-oci-spec-migration/spec.md`.
      - Keep the milestone-031 cross-link too (this is a layer
        change to the same feature).
- [ ] T018 [US1] Final smoke test:
      `MIKEBOM_OCI_NETWORK_TESTS=1 cargo +stable test -p mikebom
      --features oci-registry --test oci_registry_smoke` — both
      tests pass (alpine:3.19 + distroless). Same exit codes,
      same SBOM output, same component counts.
- [ ] T019 [US1] `./scripts/pre-pr.sh` (default) clean.
- [ ] T020 [US1] Commit:
      `refactor(032/cleanup-oci-client): drop oci-client dep + lock in oci-spec substrate`.

---

## Phase 5: Verification

- [ ] T021 SC-001 verification: pre-pr clean default; clippy
      clean feature-on; smoke tests pass.
- [ ] T022 SC-002 verification: `cargo tree -p mikebom --features
      oci-registry | grep -E "oci-client|aws-lc"` empty.
- [ ] T023 SC-003 verification: default `cargo tree -p mikebom`
      identical to pre-milestone (just confirms the substrate
      change is feature-gated).
- [ ] T024 SC-004 verification: alpine:3.19 smoke pull yields
      same component count as the T002 baseline. Distroless yields
      0 (matches T002 baseline).
- [ ] T025 SC-005 verification: `git diff main..HEAD --
      mikebom-cli/src/scan_fs/binary/ mikebom-cli/src/scan_fs/package_db/
      mikebom-cli/src/parity/ mikebom-common/` empty.
- [ ] T026 SC-007 verification: 27-golden regen
      (`MIKEBOM_UPDATE_*_GOLDENS=1`) zero diff.
- [ ] T027 SC-008 verification: `find
      mikebom-cli/src/scan_fs/oci_pull/ -name '*.rs' | xargs wc -l`
      ≤ 700 total.
- [ ] T028 Push branch; observe all 3 standard CI lanes
      (default-profile only) green (SC-006).
- [ ] T029 Author the PR description: 3-commit summary,
      durability rationale, behavior-parity attestation,
      cross-link to issue #65.

---

## Dependency graph

```text
T001-T002 (recon + baseline, recon done)
   │
   ↓
T003-T009 [Commit 1: substrate skeleton — split into submodules,
           oci-client still substrate]
   │
   ↓
T010-T014 [Commit 2: write registry.rs; replace oci-client calls
           with calls into the new substrate]
   │
   ↓
T015-T020 [Commit 3: drop oci-client dep entirely; lock in oci-spec]
   │
   ↓
T021-T029 (verification + PR)
```

## Estimated effort

| Phase | Effort | Notes |
|---|---|---|
| Phase 1 (baseline) | 5 min | T001 done; just snapshot |
| Phase 2 (substrate skeleton) | ½ day | reorganization + ref parser + platform resolver |
| Phase 3 (registry.rs + migration) | 1 day | HTTP client + bearer-token parsing is the new code |
| Phase 4 (cleanup + Cargo.toml + verify) | ½ day | dep removal + dual-profile verification + smoke test |
| Phase 5 (verify + PR) | a couple hours | dep audit + behavior parity + CI watch |
| **Total** | **~2 days** | comparable to milestone 031 itself. |
