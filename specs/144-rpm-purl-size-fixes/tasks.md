---
description: "Task list for milestone 144 — RPM reader fixes (PURL namespace + size cap)"
---

# Tasks: milestone 144 — RPM reader fixes

**Input**: Design documents from `/specs/144-rpm-purl-size-fixes/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅, quickstart.md ✅

**Tests**: Spec mandates tests via SC-003 through SC-007 + SC-011 + SC-012 + SC-008 (pre-PR gate). Test tasks are included alongside implementation tasks (per the project's existing Rust convention of in-file `#[cfg(test)] mod tests`).

**Organization**: Tasks are grouped by user story. The foundational phase introduces shared types (`RpmReaderConfig`, `SkipReason`, `VendorSource::CliOverride`) and signature plumbing so each story's behavior change is isolated to a single function body.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Maps to user story from spec.md (US1, US2, US3, US4)
- Paths are absolute under `/Users/mlieberman/Projects/mikebom/`

## Path Conventions

All paths are absolute under repository root `/Users/mlieberman/Projects/mikebom/`. The single Cargo crate touched is `mikebom-cli/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify baseline before any code change.

- [X] T001 Confirm baseline pre-PR gate is green on a fresh checkout of branch `144-rpm-purl-size-fixes` — run `./scripts/pre-pr.sh` from repo root and verify exit code 0 (workspace clippy clean + all suites pass). If this fails, halt and investigate before proceeding (the milestone diffs need a clean baseline to attribute failures). **Result**: clippy clean; one local-environment-only failure in `sbomqs_parity::sbomqs_spdx_score_meets_or_beats_cdx_across_ecosystems` due to local `sbomqs` binary version drift (banner now on stdout instead of stderr); verified non-issue via PR #466 CI (all lanes green on identical commits). Not a milestone-144 regression. Proceed.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Introduce shared types and update function signatures so each user story can change a single function body in isolation.

**⚠️ CRITICAL**: All user stories depend on Phase 2 completion. Foundational changes preserve current behavior (no observable output change yet) — the per-story phases introduce the actual fixes.

- [X] T002 [P] Add `RpmReaderConfig` struct + `Default` impl to `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs` per `contracts/rpm-reader-api.md` §`RpmReaderConfig`. Add `pub const DEFAULT_RPM_FILE_BYTES: u64 = 512 * 1024 * 1024;` next to the existing `MAX_RPM_FILE_BYTES` const (T017 in US2 deletes the old const after swapping the reference). `Default` returns `cap_bytes = DEFAULT_RPM_FILE_BYTES, distro_override = None`. No behavior change yet (call sites still use `MAX_RPM_FILE_BYTES`).

- [X] T003 [P] Add `SkipReason` enum + `structured_reason()` + `warn_prefix()` methods to `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs` per `contracts/rpm-reader-api.md` §`SkipReason`. Variants: `StatFailed(std::io::Error)`, `TruncatedLead { size: u64 }`, `SizeCapExceeded { size: u64, cap: u64 }`, `ParseFailed { reason: &'static str, error: String }`. The `warn_prefix()` MUST return `"skipping oversized .rpm file"` ONLY for `SizeCapExceeded`; all other variants return `"skipping malformed .rpm file"`. No call sites yet — pure type addition.

- [X] T004 [P] Add `VendorSource::CliOverride` variant + update `as_str()` to return `"cli-override"` in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs` per `contracts/rpm-reader-api.md` §`VendorSource`. Pure additive enum change; existing match arms continue to compile.

- [X] T005 [P] Add the two new `ScanArgs` fields (`rpm_distro: Option<String>` and `max_rpm_bytes: Option<u64>`) with `value_parser` closures to `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/cli/scan_cmd.rs` per `contracts/cli-flags.md` §Validation. Add private helpers `parse_non_empty_lowercase_distro_id(s: &str) -> Result<String, String>` and `parse_nonzero_u64(s: &str) -> Result<u64, String>` next to the existing `parse_*` helpers in the same file. Use `#[arg(long, value_name = "ID", value_parser = ...)]` form (precedent: existing `--max-image-bytes` at line 311). No reader is using these fields yet; behavior unchanged.

- [X] T006 Change `resolve_rpm_vendor_slug` signature to 3-arg form `(cli_override, os_release_id, header_vendor)` in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs` per `contracts/rpm-reader-api.md` §`resolve_rpm_vendor_slug`. KEEP the existing precedence body for now (header → os_release → fallback "rpm") — US1 will reorder it. Update the single internal caller in `parse_rpm_file` (around line 301) to pass `None` as the first arg. Update the existing 4 unit tests at `rpm_file.rs:484-510` (or thereabouts) that exercise `resolve_rpm_vendor_slug` to use the new 3-arg form with `None` as the cli_override arg.

- [X] T007 Change `read` and `parse_rpm_file` signatures to accept `config: &RpmReaderConfig` per `contracts/rpm-reader-api.md` §`read` and §`parse_rpm_file` in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs`. KEEP the size check at line 226 using `MAX_RPM_FILE_BYTES` for now (US2 swaps to `config.cap_bytes`) and the vendor resolution using `None` for cli_override (US4 swaps to `config.distro_override.as_deref()`). Update the single production caller in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/mod.rs:1454` to pass `&RpmReaderConfig::default()`. **Also update every existing in-file test call site to `read(...)` in `rpm_file.rs#[cfg(test)] mod tests`** (verified-present at approximately lines 520, 552, 591, and any sibling synthetic-RPM tests) — append `&RpmReaderConfig::default()` as the third argument. Per /speckit-analyze finding I3, missing these breaks `cargo test --workspace` after Phase 2. Behavior unchanged.

- [X] T008 Add private helper `fn build_rpm_reader_config(scan_args: &ScanArgs) -> RpmReaderConfig` in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/mod.rs` near `read_all` (around line 1178). Maps `scan_args.max_rpm_bytes.unwrap_or(rpm_file::DEFAULT_RPM_FILE_BYTES)` to `cap_bytes`, and `scan_args.rpm_distro.clone()` to `distro_override`. Wire into `read_all` so the `rpm_file::read()` call site at line 1454 uses the built config. Add `&ScanArgs` parameter to `read_all` if not already present; update its existing callers accordingly (search for `read_all(` in `mikebom-cli/src/`).

- [X] T009 [P] Unit test in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs#[cfg(test)] mod tests`: `default_rpm_file_bytes_is_512_mib` — asserts `assert_eq!(DEFAULT_RPM_FILE_BYTES, 512 * 1024 * 1024)`. Guards against accidental revert per research §R7.

**Checkpoint**: Foundation ready. `cargo +stable build --workspace` MUST compile cleanly; `cargo +stable test --workspace` MUST pass with zero behavior change vs pre-milestone baseline. The plumbing is in place; the per-story changes follow.

---

## Phase 3: User Story 1 - RPM PURLs no longer contain literal `rpm` namespace (Priority: P1) 🎯 MVP

**Goal**: Replace the hardcoded `"rpm"` fallback with empty-namespace PURLs and reorder the precedence ladder per the spec's clarification (CLI > os-release > header > empty).

**Independent Test**: Scan a temp dir containing one `.rpm` file with no `etc/os-release`, assert the emitted PURL has shape `pkg:rpm/<name>@<ver>?arch=<arch>` (no namespace segment, no `pkg:rpm/rpm/` substring).

### Implementation for User Story 1

- [X] T011 [US1] Reorder the precedence body of `resolve_rpm_vendor_slug` in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs` to the new strict ladder per `contracts/rpm-reader-api.md` §`resolve_rpm_vendor_slug`: (1) `cli_override` non-empty → `VendorSource::CliOverride`; (2) `os_release_id` + `rpm_vendor_from_id` non-empty → `VendorSource::OsRelease`; (3) `header_vendor` prefix-match in `VENDOR_HEADER_MAP` → `VendorSource::Header`; (4) else `(String::new(), VendorSource::Fallback)`. **Key change**: the fallback returns empty `String::new()`, NOT `"rpm".to_string()`. Also delete or update the existing `assert_eq!(slug, "rpm")` at `rpm_file.rs:487` (the doc-comment example at lines 88-94 needs updating too — replace `("rpm".to_string(), VendorSource::Fallback)` with `(String::new(), VendorSource::Fallback)`).

- [X] T012 [US1] Update the PURL constructor branch in `parse_rpm_file` at `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs:335` per `data-model.md` §PURL constructor: conditionally omit the `/<namespace>/` segment when `vendor_slug.is_empty()`. Pattern:

  ```rust
  let purl_str = if vendor_slug.is_empty() {
      format!("pkg:rpm/{}@{}?arch={}{}{}", ...)  // no namespace
  } else {
      format!("pkg:rpm/{}/{}@{}?arch={}{}{}", ...) // existing with namespace
  };
  ```

  Verify `Purl::new(&purl_str).ok()?` still accepts the new shape (it must, per purl-spec — namespace is optional).

- [X] T013 [P] [US1] Unit test in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs#[cfg(test)] mod tests`: `resolve_rpm_vendor_slug_fallback_is_empty_not_rpm` — asserts `resolve_rpm_vendor_slug(None, None, None) == (String::new(), VendorSource::Fallback)`. Covers SC-002 at the helper level.

- [X] T014 [P] [US1] Unit test in the same `mod tests`: `resolve_rpm_vendor_slug_header_wins_when_no_cli_no_os_release` — asserts `resolve_rpm_vendor_slug(None, None, Some("Red Hat, Inc.")) == ("redhat".to_string(), VendorSource::Header)`. Preserves backward-compat where neither override is set.

- [X] T015 [P] [US1] Unit test in the same `mod tests`: `resolve_rpm_vendor_slug_os_release_overrides_header` — asserts `resolve_rpm_vendor_slug(None, Some("fedora"), Some("CentOS")) == ("fedora".to_string(), VendorSource::OsRelease)`. Covers SC-011 at the helper level.

- [X] T016 [P] [US1] Unit test in the same `mod tests`: `purl_omits_namespace_when_vendor_slug_empty` — constructs a tiny tempfile fixture or uses an existing minimal RPM fixture in `mikebom-cli/tests/fixtures/rpm-file/`, runs `parse_rpm_file` with `RpmReaderConfig::default()` and `os_release_id = None`, asserts the emitted `PackageDbEntry.purl.as_str()` starts with `pkg:rpm/<name>@` (no `/<ns>/`) and does NOT contain `pkg:rpm//`. Covers SC-002 + SC-003 at the integration level.

**Checkpoint**: After Phase 3, US1 is fully functional. `cargo +stable test --workspace` MUST pass; the new tests MUST fire. Manual smoke: scan any `.rpm`-containing directory and `grep -c 'pkg:rpm/rpm/'` the output → 0.

---

## Phase 4: User Story 2 - Well-formed Yocto debug RPMs no longer silently skipped (Priority: P1)

**Goal**: Raise the per-file size cap from 200 MB → 512 MB and decouple the size-cap WARN from the malformed WARN.

**Independent Test**: Construct a 200-byte synthetic file with the RPM lead magic bytes, run `parse_rpm_file` with `RpmReaderConfig { cap_bytes: 100, ... }`, assert it returns `None` and that the emitted WARN log contains `reason="size-cap-exceeded"` but NOT the substring `"malformed"`.

### Implementation for User Story 2

- [X] T017 [US2] Swap the size-check site in `parse_rpm_file` at `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs:226` from `if size > MAX_RPM_FILE_BYTES` to `if size > config.cap_bytes`. Inside the branch, replace the inline `tracing::warn!` call with code that constructs `SkipReason::SizeCapExceeded { size, cap: config.cap_bytes }` and emits the WARN using `reason.warn_prefix()` as the format string + `reason.structured_reason()` as the `reason=` field value. **Then delete the now-redundant `const MAX_RPM_FILE_BYTES: u64 = 200 * 1024 * 1024;` line at `rpm_file.rs:37`** — it is the only remaining reference site. (Moved from US1 to here per /speckit-analyze finding I2: deleting the const in US1 before this swap would break the build.)

- [X] T018 [US2] Apply the same `SkipReason` refactor to the other 3 WARN sites in `parse_rpm_file` (lines 207-213 for `StatFailed`, lines 217-223 for `TruncatedLead`, lines 238-246 for `ParseFailed`). Each site constructs the appropriate `SkipReason` variant and uses `warn_prefix()` + `structured_reason()`. The structured `reason=` field values MUST be unchanged from current (`stat-failed`, `truncated-lead`, and the `classify_rpm_error` output) per FR-006 invariant on log-grep tools.

- [X] T019 [US2] Raise `MAX_RPMDB_BYTES` in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm.rs:39` from `200 * 1024 * 1024` to `512 * 1024 * 1024` per FR-008. Const name unchanged.

- [X] T020 [P] [US2] Unit test in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs#[cfg(test)] mod tests`: `size_cap_exceeded_skips_file_without_malformed_in_warn` — uses `tempfile::NamedTempFile` to write the 4-byte RPM lead magic + ~200 bytes of zeros, constructs `RpmReaderConfig { cap_bytes: 100, distro_override: None }`, calls `parse_rpm_file(tempfile.path(), None, None, &config)`, asserts return is `None`. Additionally constructs the `SkipReason::SizeCapExceeded { size: 204, cap: 100 }` variant directly and asserts `reason.warn_prefix()` does NOT contain "malformed" AND `reason.structured_reason() == "size-cap-exceeded"`. Covers SC-007 + FR-006.

- [X] T021 [P] [US2] Unit test in the same `mod tests`: `size_cap_at_boundary_includes_file` — uses a tempfile sized exactly at `config.cap_bytes` (with magic bytes), asserts `parse_rpm_file` does NOT skip (i.e., proceeds to the parser). The strict `>` semantic is preserved from line 226 today. (The parser itself may then fail because the tempfile isn't a valid full RPM — the test only asserts the size check doesn't fire.)

- [X] T022 [P] [US2] Unit test in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm.rs#[cfg(test)] mod tests` (or co-located): `max_rpmdb_bytes_is_512_mib` — asserts `assert_eq!(MAX_RPMDB_BYTES, 512 * 1024 * 1024)`. Guards FR-008 against accidental revert per research §R7. Add the test module if it doesn't exist; follow the `#[cfg_attr(test, allow(clippy::unwrap_used))]` convention from elsewhere in the crate.

**Checkpoint**: After Phase 4, US2 is fully functional. Pre-PR gate (`./scripts/pre-pr.sh`) MUST pass. Manual smoke: a real Yocto `tmp/deploy/rpm/` scan now emits all 4587 RPMs instead of 4584.

---

## Phase 5: User Story 3 - Operator can override default RPM size cap (Priority: P2)

**Goal**: Wire the `--max-rpm-bytes <N>` clap flag end-to-end so operators can lift the 512 MB default per scan.

**Independent Test**: Run `mikebom sbom scan --path <tempdir-with-700MB-rpm> --max-rpm-bytes 1073741824` and confirm the 700 MB RPM is included as a component. Re-run with `--max-rpm-bytes 100000000` and confirm it's skipped with the size-cap WARN.

### Implementation for User Story 3

- [X] T023 [US3] Verify the T005-added `max_rpm_bytes` field on `ScanArgs` is flowing through `build_rpm_reader_config` (T008) — open `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/mod.rs` and confirm the helper does `cap_bytes: scan_args.max_rpm_bytes.unwrap_or(rpm_file::DEFAULT_RPM_FILE_BYTES)`. If the wiring isn't already present from T008, add it.

- [X] T024 [US3] Add `--help` text to the `max_rpm_bytes` clap attribute in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/cli/scan_cmd.rs` per `contracts/cli-flags.md` §`--max-rpm-bytes <BYTES>`. Use a `#[arg(long, value_name = "BYTES", value_parser = parse_nonzero_u64, help = "Per-file size cap for standalone .rpm files. Files exceeding the cap are skipped (with WARN). Useful for Yocto debug RPMs (kernel-dbg, gcc-dbg). Default: 536870912 (512 MiB).")]` form.

- [X] T025 [P] [US3] Clap-parser unit test in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/cli/scan_cmd.rs#[cfg(test)] mod tests` (use the existing `ScanArgsForTest` parser wrapper at line 3082): `max_rpm_bytes_accepts_valid_unsigned` — asserts `<ScanArgsForTest as Parser>::try_parse_from(["scan", "--path", "/tmp", "--max-rpm-bytes", "1073741824"])` succeeds and `parsed.inner.max_rpm_bytes == Some(1073741824)`.

- [X] T026 [P] [US3] Clap-parser unit test in the same `mod tests`: `max_rpm_bytes_rejects_zero` — asserts the same `try_parse_from` with `"--max-rpm-bytes", "0"` returns `Err(_)` with the error message containing `"must be > 0"`. Covers US3 acceptance scenario 3 (zero rejection at parse time).

- [X] T027 [P] [US3] Clap-parser unit test in the same `mod tests`: `max_rpm_bytes_rejects_non_numeric` — asserts `try_parse_from` with `"--max-rpm-bytes", "abc"` returns `Err(_)`. Covers US3 acceptance scenario 3.

**Checkpoint**: After Phase 5, US3 is fully functional. `mikebom sbom scan --help` now shows `--max-rpm-bytes`. The flag accepts/rejects values as specified.

---

## Phase 6: User Story 4 - Operator can override distro identity (Priority: P2)

**Goal**: Wire the `--rpm-distro <ID>` clap flag end-to-end so Yocto operators can encode their `DISTRO` value in PURLs.

**Independent Test**: Run `mikebom sbom scan --path <dir-with-rpm> --rpm-distro poky` and confirm all emitted PURLs have namespace `poky` (not the per-RPM RPMTAG_VENDOR value or auto-detected `/etc/os-release` ID).

### Implementation for User Story 4

- [X] T028 [US4] In `parse_rpm_file` at `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs` around line 301, change the call to `resolve_rpm_vendor_slug` from `resolve_rpm_vendor_slug(None, os_release_id, vendor_header.as_deref())` (T006/T007 placeholder) to `resolve_rpm_vendor_slug(config.distro_override.as_deref(), os_release_id, vendor_header.as_deref())`. This activates the CLI override path that T011 prepared in `resolve_rpm_vendor_slug`'s body.

- [X] T029 [US4] Verify the T005-added `rpm_distro` field is flowing through `build_rpm_reader_config` (T008) — open `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/mod.rs` and confirm the helper does `distro_override: scan_args.rpm_distro.clone()`. Lowercasing is already done by the clap `value_parser` (T005); no re-validation needed here.

- [X] T030 [US4] Add `--help` text to the `rpm_distro` clap attribute in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/cli/scan_cmd.rs` per `contracts/cli-flags.md` §`--rpm-distro <ID>`. Use the form `#[arg(long, value_name = "ID", value_parser = parse_non_empty_lowercase_distro_id, help = "Override distro identifier for RPM PURL namespaces (e.g., --rpm-distro poky for Yocto). Overrides /etc/os-release and per-RPM vendor tags. Default: auto-detect.")]`.

- [X] T031 [P] [US4] Unit test in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/scan_fs/package_db/rpm_file.rs#[cfg(test)] mod tests`: `resolve_rpm_vendor_slug_cli_overrides_everything` — asserts `resolve_rpm_vendor_slug(Some("poky"), Some("fedora"), Some("CentOS")) == ("poky".to_string(), VendorSource::CliOverride)`. Covers SC-012 at the helper level.

- [X] T032 [P] [US4] Clap-parser unit test in `/Users/mlieberman/Projects/mikebom/mikebom-cli/src/cli/scan_cmd.rs#[cfg(test)] mod tests`: `rpm_distro_accepts_lowercase_slug` — asserts `try_parse_from(["scan", "--path", "/tmp", "--rpm-distro", "poky"])` succeeds and `parsed.inner.rpm_distro == Some("poky".to_string())`.

- [X] T033 [P] [US4] Clap-parser unit test in the same `mod tests`: `rpm_distro_rejects_empty_string` — asserts `try_parse_from(["scan", "--path", "/tmp", "--rpm-distro", ""])` returns `Err(_)` with error message containing `"must be non-empty"`. Covers US4 acceptance scenario 3.

- [X] T034 [P] [US4] Clap-parser unit test in the same `mod tests`: `rpm_distro_lowercases_input` — asserts `try_parse_from(["scan", "--path", "/tmp", "--rpm-distro", "Poky"])` succeeds and `parsed.inner.rpm_distro == Some("poky".to_string())` (verifies the value_parser closure applies `.to_lowercase()`).

- [X] T035 [US4] New integration test file `/Users/mlieberman/Projects/mikebom/mikebom-cli/tests/rpm_file_yocto_regression.rs` per research §R9. **Build synthetic RPMs at runtime** via `rpm::PackageBuilder` — mirror the established pattern at `mikebom-cli/src/scan_fs/package_db/rpm_file.rs:524-577` (`fn parses_synthetic_rpm_file`); the repo has NO checked-in `.rpm` fixtures and the existing test convention is to build them in-test (verified via `find`-grep during /speckit-analyze finding I1). Use this shape:

  ```rust
  // Test fn 1: os-release overrides per-RPM RPMTAG_VENDOR (SC-011 end-to-end)
  #[test]
  fn rpm_file_os_release_overrides_per_rpm_vendor() {
      let dir = tempfile::tempdir().unwrap();
      std::fs::create_dir_all(dir.path().join("etc")).unwrap();
      std::fs::write(dir.path().join("etc/os-release"), "ID=fedora\n").unwrap();
      let rpm_path = dir.path().join("synth-1.0-1.x86_64.rpm");
      rpm::PackageBuilder::new("synth", "1.0", "MIT", "x86_64", "test")
          .release("1")
          .vendor("CentOS")  // per-RPM vendor — must be overridden by os-release
          .build().unwrap()
          .write_file(&rpm_path).unwrap();
      let cfg = RpmReaderConfig::default();
      let entries = read(dir.path(), None, &cfg);
      assert_eq!(entries.len(), 1);
      assert!(entries[0].purl.as_str().starts_with("pkg:rpm/fedora/"),
              "got {}", entries[0].purl.as_str());
  }

  // Test fn 2: --rpm-distro overrides os-release AND per-RPM RPMTAG_VENDOR (SC-012)
  #[test]
  fn rpm_file_cli_distro_overrides_everything() {
      let dir = tempfile::tempdir().unwrap();
      std::fs::create_dir_all(dir.path().join("etc")).unwrap();
      std::fs::write(dir.path().join("etc/os-release"), "ID=fedora\n").unwrap();
      let rpm_path = dir.path().join("synth-1.0-1.x86_64.rpm");
      rpm::PackageBuilder::new("synth", "1.0", "MIT", "x86_64", "test")
          .release("1")
          .vendor("CentOS")
          .build().unwrap()
          .write_file(&rpm_path).unwrap();
      let cfg = RpmReaderConfig { cap_bytes: DEFAULT_RPM_FILE_BYTES, distro_override: Some("poky".to_string()) };
      let entries = read(dir.path(), None, &cfg);
      assert_eq!(entries.len(), 1);
      assert!(entries[0].purl.as_str().starts_with("pkg:rpm/poky/"),
              "got {}", entries[0].purl.as_str());
  }
  ```

  Note that `RpmReaderConfig` is module-private in `rpm_file.rs`; to use it from an `mikebom-cli/tests/*.rs` integration test, either (a) promote the struct + the relevant `read` fn to `pub` accessibility via `pub use` from the crate root, or (b) write these as `#[cfg(test)] mod tests` in `rpm_file.rs` itself instead of as an out-of-source integration test. Recommend (b) for minimal blast radius; the integration-test file becomes unnecessary. Covers SC-011 + SC-012 end-to-end.

**Checkpoint**: After Phase 6, US4 is fully functional. All 4 user stories are complete and independently testable.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Validation, doc updates, golden-fixture audit, pre-PR gate.

- [X] T036 Audit golden test fixtures for any baked-in `pkg:rpm/rpm/` strings: run `grep -rln 'pkg:rpm/rpm/' /Users/mlieberman/Projects/mikebom/mikebom-cli/tests/fixtures/` from repo root. Per research §plan baseline check this returned 0 matches; re-confirm. If new matches appear (e.g., from a concurrent merge), refresh them per the milestone-specific update env-var (`MIKEBOM_UPDATE_CDX_GOLDENS=1` / `MIKEBOM_UPDATE_SPDX_GOLDENS=1` / `MIKEBOM_UPDATE_SPDX3_GOLDENS=1`) and commit the refresh separately from the fix per memory `feedback_prepr_gate_full_output.md`. Covers FR-009.

- [X] T037 Run the quickstart.md verification commands locally against an operator-supplied `.rpm` corpus (the repo ships zero `.rpm` fixtures per /speckit-analyze finding I1 — the existing test suite builds them at runtime). Suggested sources: (a) a Fedora / RHEL / Rocky container image extracted via `skopeo copy docker://fedora:latest dir:/tmp/fedora && umoci unpack --image /tmp/fedora:latest /tmp/fedora-rootfs`, then scan `/tmp/fedora-rootfs/var/lib/rpm/`; OR (b) a Yocto build output if accessible; OR (c) any single `.rpm` downloaded from a public mirror. Steps:
  1. Scan the corpus: `cargo run -p mikebom -- sbom scan --path <path-to-rpm-corpus> --format cyclonedx-json --output /tmp/check.cdx.json`
  2. Verify `jq -r '.components[].purl' /tmp/check.cdx.json | grep -c 'pkg:rpm/rpm/'` returns `0`.
  3. Re-run with `--rpm-distro poky` and verify `jq -r '.components[].purl' /tmp/check.cdx.json | head -1` shows `pkg:rpm/poky/...`.
  4. Re-run with `--rpm-distro ""` and verify clap exits with parse error.
  5. Re-run with `--max-rpm-bytes 0` and verify clap exits with parse error.

  If no `.rpm` corpus is reachable for steps 1–3, the helper-level + integration-level unit tests added by T013/T015/T016/T020/T031/T035 are the binding correctness signal. Document the skip in the PR description.

- [X] T038 Run mandatory pre-PR gate per Constitution Development Workflow + memory `feedback_prepr_gate_full_output.md`: `./scripts/pre-pr.sh` from repo root. Both steps MUST pass clean. If any test fails, scan the FULL output (do NOT grep on `^test result: FAILED` — that filter is known to drop multi-test suite summaries per the same memory). Covers SC-008. **Result**: clippy `--workspace --all-targets -- -D warnings` — clean (zero warnings, zero errors). Test suite: 100+ `test result: ok` lines + ONE pre-existing failure (`sbomqs_parity::sbomqs_spdx_score_meets_or_beats_cdx_across_ecosystems`) — same local-environment sbomqs-version-drift issue documented in T001; PR #466 CI history confirms it passes on identical commits. Milestone-144 changes pass clean; CI will validate sbomqs_parity behavior on a clean runner.

- [X] T039 [P] Update `mikebom-cli/src/scan_fs/package_db/rpm_file.rs` module doc-comment header (lines 1-17) to reflect the new vendor-slug priority: replace the existing "3. Hardcoded `"rpm"` fallback." line with "3. Per-RPM RPMTAG_VENDOR (line-94 ladder).\n//! 4. Empty namespace (the new fallback — emitted PURLs omit the namespace segment entirely)." and add a "0." entry for the CLI override above the existing "1." line. Also update the module doc-comment to mention milestone 144.

- [ ] T040 Commit the foundational + per-story changes as ONE focused commit (or split US1+US2 from US3+US4 if the diff is large — operator's call). Commit message format per project convention: `impl(144): RPM reader — drop literal 'rpm' namespace, raise size cap to 512 MB, add operator overrides`. Body should reference the spec branch + the two D-01/D-02 issues from the spec Origin section. Do NOT commit until T038 passes clean.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies. Verifies baseline.
- **Phase 2 (Foundational)**: Depends on Phase 1. BLOCKS all user stories.
- **Phase 3 (US1)**: Depends on Phase 2. Independent of US2/US3/US4.
- **Phase 4 (US2)**: Depends on Phase 2. Independent of US1/US3/US4.
- **Phase 5 (US3)**: Depends on Phase 2. Optionally builds on US2 (the cap default is set; flag lifts it).
- **Phase 6 (US4)**: Depends on Phase 2. Optionally builds on US1 (precedence ladder is in place; flag activates the CLI-override branch).
- **Phase 7 (Polish)**: Depends on US1+US2+US3+US4 being functionally complete.

### User Story Dependencies

- **US1 (P1, MVP)**: Standalone after Phase 2. Delivers the D-01 fix (no more `pkg:rpm/rpm/`).
- **US2 (P1)**: Standalone after Phase 2. Delivers the D-02 fix (Yocto debug RPMs included). **T017 also performs the `MAX_RPM_FILE_BYTES` const deletion** that was originally scoped to US1's T010 — moved here per /speckit-analyze finding I2 because deleting the const before swapping the reference would break the build.
- **US3 (P2)**: **Depends on US2's T017** (per /speckit-analyze finding F1). Reason: the `--max-rpm-bytes` flag flows the value into `RpmReaderConfig.cap_bytes`, but the size-check site at `rpm_file.rs:226` still references the static `MAX_RPM_FILE_BYTES` const until T017 swaps it to `config.cap_bytes`. US3 wired but not yet active without T017. Add operator knob for cap.
- **US4 (P2)**: Builds on US1's precedence-ladder change. The body of `resolve_rpm_vendor_slug` from T011 already includes the CLI-override branch; US4 only activates it via T028. Operationally independent for testing, but T028 is no-op without T011.

### Within Each User Story

- T011 / T012 / T017 / T028 (the body-edit tasks) cannot run in parallel within the same function. The [P] marker is reserved for test additions that touch only the `mod tests` section or for separate files.
- Test tasks ([P]-marked) within a story can run in parallel with each other and (in some cases) with the implementation task — though typical Rust workflow writes the impl first, then the tests, so they should not strictly precede the impl.

### Parallel Opportunities

Within Phase 2: T002, T003, T004, T005, T009 are all [P] (different files OR different additions in the same file's distinct sections).

Within US1: T013, T014, T015, T016 are all [P] (test functions in the same `mod tests`, no file-write conflict if added in one editor pass).

Within US2: T020, T021, T022 are all [P].

Within US3: T025, T026, T027 are all [P].

Within US4: T031, T032, T033, T034 are all [P].

Within Polish (Phase 7): T039 is [P]; T036, T037, T038 are sequential gate-style tasks.

---

## Parallel Example: Phase 2

```bash
# Foundational adds (different files / non-overlapping additions in same file):
Task T002: Add RpmReaderConfig struct + Default + DEFAULT_RPM_FILE_BYTES const → rpm_file.rs
Task T003: Add SkipReason enum + methods → rpm_file.rs (different impl block)
Task T004: Add VendorSource::CliOverride variant → rpm_file.rs (enum extension)
Task T005: Add ScanArgs fields + value_parser helpers → scan_cmd.rs (different file)
Task T009: Add default_rpm_file_bytes_is_512_mib unit test → rpm_file.rs#mod tests
```

After all five complete, T006/T007/T008 (signature changes) run sequentially because they touch overlapping function signatures + the dispatcher.

## Parallel Example: US1 (after T011, T012 land sequentially)

```bash
# Tests (independent assertions, same mod tests block, no impl conflict):
Task T013: resolve_rpm_vendor_slug_fallback_is_empty_not_rpm
Task T014: resolve_rpm_vendor_slug_header_wins_when_no_cli_no_os_release
Task T015: resolve_rpm_vendor_slug_os_release_overrides_header
Task T016: purl_omits_namespace_when_vendor_slug_empty
```

---

## Implementation Strategy

### MVP First (US1 only)

1. Complete Phase 1: T001 baseline check.
2. Complete Phase 2: T002–T009 foundational types + signatures.
3. Complete Phase 3: T011–T016 — the D-01 fix lands. (Note: T010 was moved into T017/US2 per /speckit-analyze finding I2; US1 is now T011–T016 only.)
4. **STOP and VALIDATE**: `./scripts/pre-pr.sh` clean. Manual smoke confirms `grep -c 'pkg:rpm/rpm/'` returns 0 on any RPM scan.
5. This alone is a shippable PR if US2 (size cap) needs to be separated for any reason. The const `MAX_RPM_FILE_BYTES` remains in the source tree, still referenced by `parse_rpm_file:226` until T017 — so clippy does NOT flag it as dead code in the US1-only shipping shape.

### Incremental (recommended for this milestone)

1. Phases 1 + 2 land together (foundational).
2. Phase 3 (US1) — D-01 fix.
3. Phase 4 (US2) — D-02 fix.
4. Phase 5 (US3) — operator size-cap override.
5. Phase 6 (US4) — operator distro override.
6. Phase 7 (Polish) — doc-comment + pre-PR gate + commit.

All in a single PR is the intended shape per the spec's framing of both D-01 and D-02 as one milestone.

### Single-developer Note

This milestone is too small to benefit from parallel-team execution. The [P] markers exist to signal "no cross-file write conflict" — useful for tooling that automates task execution (Aider, Cline, etc.) — but a human implementer will likely edit `rpm_file.rs` in one sitting and add all the unit tests in a second sitting.

---

## Notes

- Tests live in-file under `#[cfg(test)] mod tests` per the project's existing convention; the integration test (T035) is the only out-of-source test.
- The `#[cfg_attr(test, allow(clippy::unwrap_used))]` convention applies to any new `mod tests` block per Constitution Principle IV.
- The `cargo +stable test --workspace` invocation in T038 (not `cargo test -p mikebom`) is mandatory per `feedback_release_bump_prepr_slow.md` and Constitution Development Workflow §Pre-PR Verification.
- Memory `feedback_prepr_gate_full_output.md` is directly relevant: when verifying T038, scan the FULL output rather than greping on `^test result: FAILED` (the filter is known to drop multi-test-suite summaries).
- The milestone's commit message convention (T040) is `impl(144): <short desc>` matching recent precedent (milestone 143's `impl(143)` commits).
