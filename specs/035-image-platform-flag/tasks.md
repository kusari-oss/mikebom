---
description: "Task list — milestone 035 --image-platform flag"
---

# Tasks: `--image-platform` flag

**Input**: spec.md (✅), plan.md (✅), checklists/requirements.md (✅).

**Tests**: ≥6 inline tests for `parse_platform_string`, ≥3 new
resolver tests covering variant matching, ≥1 gated network smoke
test for cross-arch pull.

**Organization**: Single user story (US1, P1). Two atomic commits.

## Path conventions

- Touches `mikebom-cli/src/cli/scan_cmd.rs` (additive only).
- Touches `mikebom-cli/src/scan_fs/oci_pull/{platform,mod}.rs`.
- Touches `mikebom-cli/tests/oci_registry_smoke.rs` (additive).
- Touches `docs/user-guide/cli-reference.md` and `CHANGELOG.md`.
- Does NOT touch parity/, generate/, resolve/, attestation/, or any
  other CLI command.

---

## Phase 1: Setup + baseline

- [X] T001 Recon done in this session (2026-04-28): `host_oci_arch`
      lives at `mod.rs:204-220`; `resolve_manifest_list_to_linux`
      at `platform.rs:18-42` already takes `target_arch: &str` —
      generalizing to `target_variant: Option<&str>` is mechanical.
      `pull_to_tarball` is the one external entry point that needs
      a new parameter.
- [ ] T002 Snapshot baseline: `./scripts/pre-pr.sh 2>&1 | tee
      /tmp/baseline-035.txt | grep -E '^test [a-z_:]+ \.\.\. ok' |
      sort -u > /tmp/baseline-035-tests.txt`. Confirm post-035 test
      list shows additions only.

---

## Phase 2: Commit 1 — `035/flag-and-resolver`

**Goal**: `--image-platform` is plumbed end-to-end and the resolver
honors variant. All existing tests pass; new tests cover the
additions.

- [ ] T003 [US1] Edit `platform.rs`: add `pub(super) struct ParsedPlatform { os: String, architecture: String, variant: Option<String> }` and `pub(super) fn parse_platform_string(s: &str) -> Result<ParsedPlatform>`. Validates: 2 or 3 `/`-separated non-empty fields; os == "linux".
- [ ] T004 [US1] Add inline tests for `parse_platform_string`: linux/amd64, linux/arm64, linux/arm/v7, linux/arm64/v8, error on linux/, error on /amd64, error on linux/amd64/v7/extra, error on windows/amd64.
- [ ] T005 [US1] Edit `platform.rs::ManifestListEntry`: add `pub variant: Option<String>`.
- [ ] T006 [US1] Edit `platform.rs::resolve_manifest_list_to_linux`: change signature to take `target_variant: Option<&str>`. Update the `find` predicate. Update existing callers (the unit tests already in this module — they all pass `None`).
- [ ] T007 [US1] Add inline tests covering variant: (a) user variant=None matches entry variant=None and entry variant=Some; (b) user variant=Some("v7") matches only entry variant=Some("v7"); (c) error message includes variant in available-platforms list.
- [ ] T008 [US1] Edit `mod.rs::pull_to_tarball` signature to take `image_platform: Option<&str>`. When `Some`, parse via FR-002 and use parsed arch+variant. When `None`, use `host_oci_arch()` + `None` variant.
- [ ] T009 [US1] Edit `mod.rs`'s manifest-list mapping: populate `variant: plat.variant().clone()` on each `ManifestListEntry`.
- [ ] T010 [US1] Edit `scan_cmd.rs::ScanArgs`: add `pub image_platform: Option<String>` with `#[arg(long, requires = "image", value_name = "linux/ARCH[/VARIANT]")]`. Doc-comment names supported shapes.
- [ ] T011 [US1] Edit `scan_cmd.rs` (the `--image` dispatch site): when `args.image_platform.is_some()` AND the kind is `ImageArgKind::Path` (tarball, not OCI ref), bail with "the --image-platform flag only applies to registry image references, not pre-extracted tarballs."
- [ ] T012 [US1] Edit `scan_cmd.rs` (the OciRef branch): pass `args.image_platform.as_deref()` into `pull_to_tarball`.
- [ ] T013 [US1] `cargo +stable test -p mikebom --bin mikebom scan_fs::oci_pull` green.
- [ ] T014 [US1] `./scripts/pre-pr.sh` clean.
- [ ] T015 [US1] Commit: `feat(035/flag-and-resolver): add --image-platform CLI flag with variant-aware multi-arch resolver`.

---

## Phase 3: Commit 2 — `035/docs-and-smoke`

**Goal**: Docs, CHANGELOG, gated smoke test.

- [ ] T016 [US1] Edit `docs/user-guide/cli-reference.md`: add a `--image-platform` row to the `mikebom sbom scan` flag table; update the `--image` row to drop the "deferred to 031.y" hint.
- [ ] T017 [US1] Edit `CHANGELOG.md`: unreleased entry — `--image-platform` flag for cross-arch image scans (closes #67).
- [ ] T018 [US1] Edit `mikebom-cli/tests/oci_registry_smoke.rs`: add `pulls_alpine_with_image_platform_override` test (gated on `MIKEBOM_OCI_NETWORK_TESTS=1`). Pulls `alpine:3.19 --image-platform <non-host-arch>` and asserts `arch=` qualifier reflects the requested arch.
- [ ] T019 [US1] `./scripts/pre-pr.sh` clean.
- [ ] T020 [US1] Commit: `feat(035/docs-and-smoke): document --image-platform + gated cross-arch smoke test`.

---

## Phase 4: PR + verification

- [ ] T021 SC-001: `./scripts/pre-pr.sh` clean.
- [ ] T022 SC-002: `git diff main..HEAD -- mikebom-cli/src/parity/ mikebom-cli/src/generate/ mikebom-cli/src/resolve/` empty.
- [ ] T023 SC-003: `MIKEBOM_UPDATE_*_GOLDENS=1 ./scripts/pre-pr.sh` produces zero diff.
- [ ] T024 SC-005: `wc -l mikebom-cli/src/scan_fs/oci_pull/platform.rs` ≤ 250.
- [ ] T025 Push branch; observe all 3 CI lanes green (SC-004).
- [ ] T026 Open PR closing #67.

---

## Dependency graph

```text
T001 (done) → T002 (baseline)
                │
                ↓
       T003-T015 [Commit 1]
                │
                ↓
       T016-T020 [Commit 2]
                │
                ↓
       T021-T026 (verify + PR)
```
