# Research — milestone 144 (RPM reader fixes)

Phase 0 output. Resolves all unknowns flagged in `plan.md`'s Technical Context.

## R1 — How to parameterize the size cap without breaking callers?

**Decision**: Introduce a `RpmReaderConfig` struct in `rpm_file.rs` that holds (a) the byte cap and (b) the optional CLI distro override. Thread it through `read()` and `parse_rpm_file()` as a borrow. Tests construct a small-cap config to exercise the cap path with a tiny fixture.

**Rationale**:
- The existing reader signature is `read(rootfs: &Path, distro_version: Option<&str>) -> Vec<PackageDbEntry>` (`rpm_file.rs:121`). Adding two more positional args (`max_bytes: u64`, `distro_override: Option<&str>`) inflates the surface and makes future additions painful.
- A config struct matches the established pattern in milestone 114 (`WalkConfig` for `safe_walk` at `mikebom-cli/src/scan_fs/walk.rs`) and milestone 113's `ExclusionSet`.
- Tests can construct `RpmReaderConfig { cap_bytes: 100, distro_override: None }` and write a 200-byte tempfile with the RPM lead magic — the size check at the current `if size > MAX_RPM_FILE_BYTES` (line 226) becomes `if size > config.cap_bytes` and fires before the parser is invoked, no real 500 MB fixture needed.

**Alternatives considered**:
- **Function-arg passing** (`fn read(rootfs, distro_version, max_bytes, distro_override)`) — rejected: arity creep, harder to default.
- **Module-level `OnceLock<RpmReaderConfig>`** set by the CLI at startup — rejected: thread-state coupling, breaks parallel test isolation, harder to override per-test.
- **Const-based runtime override via `std::env::var("MIKEBOM_RPM_CAP")`** — rejected: env-var-based config is a footgun (silent at help time, not discoverable).

## R2 — How do CLI flags propagate from clap into the reader?

**Decision**: Add two fields to the existing `ScanArgs` struct at `mikebom-cli/src/cli/scan_cmd.rs:65` — `rpm_distro: Option<String>` (clap `#[arg(long, value_name = "ID")]`) and `max_rpm_bytes: Option<u64>` (clap `#[arg(long, value_name = "BYTES")]`). The dispatcher `read_all` in `package_db/mod.rs:1178` already takes per-reader config (e.g., `include_dev: bool` for rpm); add a single `rpm_reader_config: &RpmReaderConfig` parameter alongside.

**Rationale**:
- `ScanArgs` is already the established home for per-reader knobs (`exclude_path`, `file_inventory_mode`, etc.). New fields follow the exact existing pattern.
- `Option<String>` + clap derive yields `--rpm-distro <ID>` with required value; `Option<u64>` yields `--max-rpm-bytes <BYTES>` parsing as integer. Non-numeric values are rejected at clap parse time (FR-005 last sentence).
- Empty-string and zero validation use clap `value_parser!` with closure (precedent: existing `--max-image-bytes` at line 311 uses `default_value_t = 100 * 1024 * 1024` and could carry a `value_parser` for non-zero; adding one for our new flags is a 3-line change per flag).
- The `read_all` dispatcher already has 4+ per-reader knobs (`deb_namespace`, `alpm_namespace`, `include_dev`, `distro_version`); adding a struct grouping the RPM-file-specific knobs reduces drift.

**Alternatives considered**:
- **Module-level CLI state struct** (one giant config struct for all readers) — rejected: increases blast radius of any single reader's change, slows future per-reader feature work.
- **Separate flag-validation function** outside clap — rejected: duplicates work clap already does.

## R3 — Where do `rpm_file::read` callers live, and what else needs threading?

**Decision**: Exactly ONE caller — `package_db/mod.rs:1454`. Thread `RpmReaderConfig` from `read_all` through to that single line. No other crates / modules invoke `rpm_file::read` directly.

**Rationale**:
- Verified via `grep -rn 'rpm_file::read\b' mikebom-cli/src/` — single match at the dispatcher.
- The dispatcher already builds `distro_version`, etc., inside the function; adding `RpmReaderConfig` construction near the top of `read_all` (built from the `ScanArgs` fields) keeps the wiring localized.

**Alternatives considered**: None viable — single-caller chain is the natural shape.

## R4 — How do we test the size cap with a tiny fixture?

**Decision**: Use `tempfile::NamedTempFile`, write the 4-byte RPM lead magic (`[0xED, 0xAB, 0xEE, 0xDB]`) plus a few hundred bytes of zero padding to exceed `MIN_RPM_FILE_BYTES` (96), then construct `RpmReaderConfig { cap_bytes: 100, distro_override: None }`. Assert `parse_rpm_file()` returns `None` and that the WARN line text does NOT contain "malformed" and DOES contain `reason="size-cap-exceeded"`.

**Rationale**:
- The cap check at line 226 runs BEFORE `rpm::Package::open()`. We don't need a parseable RPM — just one that passes the `MIN_RPM_FILE_BYTES + lead-magic` precondition and exceeds the configured cap.
- `tempfile = "*"` is already a dev-dep across the workspace (verified via `grep tempfile mikebom-cli/Cargo.toml`).
- For the WARN-text assertion, use `tracing-subscriber` test fixture (already in use in `rpm.rs` tests per pattern; check `cargo expand` or `grep -n 'with_default_collector' mikebom-cli/`). Alternative: split the WARN-emission into a small helper that returns the warn-text-tuple as a `Result<(), SkipReason>` and unit-test the enum variant. The helper-split is cleaner; the subscriber-capture is more direct.

**Decision sub-point** (warn-capture vs reason-enum): use a `SkipReason` enum return type for `parse_rpm_file`'s internal size check, with a `to_warn_text(&self) -> &'static str` method. The public `parse_rpm_file` still returns `Option<PackageDbEntry>` for back-compat; the enum is an internal helper. Tests check the enum variant directly (no log capture). The WARN is emitted inside `parse_rpm_file` from the enum.

**Alternatives considered**:
- **Real fixture (300 MB Yocto debug RPM)** — rejected: bloats the test-fixtures repo, slows CI, requires GitHub LFS or a sibling-repo fetch.
- **Mock the filesystem with `mockall`** — rejected: not currently in the dep graph, overkill for a 200-byte tempfile.

## R5 — Does the local `percent_encode_purl_segment` handle empty input correctly?

**Decision**: Don't rely on the encoder for the empty-namespace case. Instead, branch in the PURL constructor at `rpm_file.rs:335`: when `vendor_slug.is_empty()`, emit `pkg:rpm/{name}@{ver}?arch=...` (no namespace segment, no leading slash before name); when non-empty, emit the current `pkg:rpm/{vendor}/{name}@{ver}?arch=...`.

**Rationale**:
- Per purl-spec, a missing namespace must be elided entirely, not represented as an empty path segment (`pkg:rpm//name@ver` is INVALID — two consecutive slashes after the type are not allowed).
- The `mikebom_common::Purl::new()` validator at the end of the chain would reject `pkg:rpm//acl-dbg@...` anyway, but it's cleaner to construct the right string the first time.
- Format-string branch is 4 lines; cleaner than threading "is_empty" semantics into the encoder.

**Alternatives considered**:
- **Pass empty `vendor_slug` through the encoder** — rejected: produces invalid PURL string before validator catches it; wastes a validator call.
- **Have the encoder return `Option<String>`** — rejected: API change for one caller.

## R6 — Where exactly does `--rpm-distro` validation belong (clap parser or post-parse)?

**Decision**: Clap-side validation via `value_parser!` closure that rejects empty string. Zero-byte / non-numeric rejection for `--max-rpm-bytes` is automatic via `Option<u64>`'s built-in parser; add a separate `value_parser!` closure that additionally rejects zero.

**Rationale**:
- Clap-side rejection happens BEFORE any scan begins (FR-003 + FR-005 acceptance scenarios explicitly require this).
- Single closure per flag, ~5 lines each, no new modules.

**Sample code shape** (for reference, not normative):

```rust
// In ScanArgs:
#[arg(long, value_name = "ID", value_parser = parse_non_empty_str)]
pub rpm_distro: Option<String>,

#[arg(long, value_name = "BYTES", value_parser = parse_nonzero_u64)]
pub max_rpm_bytes: Option<u64>,

fn parse_non_empty_str(s: &str) -> Result<String, String> {
    if s.is_empty() { Err("must be non-empty".into()) } else { Ok(s.to_string()) }
}
fn parse_nonzero_u64(s: &str) -> Result<u64, String> {
    let v: u64 = s.parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    if v == 0 { Err("must be > 0".into()) } else { Ok(v) }
}
```

**Alternatives considered**:
- **Defer validation to scan-start** — rejected: violates FR-003/FR-005 explicit clap-time rejection, and operators waste cycles on a doomed scan.

## R7 — Does FR-008 (raise `MAX_RPMDB_BYTES`) need a separate test?

**Decision**: Yes, one small unit test in `rpm.rs`-adjacent location asserting the new const value (`512 * 1024 * 1024`). The const itself is the contract; no behavior-test needed because no observable behavior changes unless someone scans an installed-system rpmdb > 200 MB, which is hard to fixture.

**Rationale**:
- A `#[test] fn rpmdb_size_cap_is_512mb_for_yocto_compat() { assert_eq!(MAX_RPMDB_BYTES, 512 * 1024 * 1024); }` guards against future accidental reverts.
- More elaborate testing is YAGNI per spec Out of Scope §2 ("Operator override of MAX_RPMDB_BYTES").

**Alternatives considered**:
- **No test** — rejected: silent revert risk during future refactors.
- **End-to-end test with synthetic 300 MB rpmdb** — rejected: out of scope per spec; fixture burden too high.

## R8 — Should the `RpmReaderConfig.distro_override` field accept the `--rpm-distro` value as-is or apply any transformation?

**Decision**: Pass through verbatim, lowercased. No translation table. The existing `VENDOR_HEADER_MAP` at `rpm_file.rs:49` translates header strings ("Red Hat" → "redhat", "Fedora Project" → "fedora"); the CLI override is the operator's explicit choice and is treated as the canonical slug directly.

**Rationale**:
- Spec Assumption 2: "The PURL `ID=` value goes in verbatim (lowercased) without translation." Same rule applies to `--rpm-distro`.
- Avoids the surprise where `--rpm-distro "Fedora Project"` would be translated to `fedora`. The operator who wants `fedora` should pass `fedora`.
- Lowercase normalization is necessary because purl-spec namespaces are lowercase-canonical. Clap's `value_parser` does the lowercasing in the same closure that rejects empty strings.

**Alternatives considered**:
- **Apply VENDOR_HEADER_MAP** — rejected: operator surprise.
- **No lowercase normalization** — rejected: produces `pkg:rpm/Fedora/name@ver` which is non-canonical (purl-spec §lower-case-rules).

## R9 — Test strategy for the `/etc/os-release > per-RPM RPMTAG_VENDOR` override (SC-011)?

**Decision**: Use the existing `rpm_file_yocto_regression.rs` integration test (new file). Construct a tempdir with:
1. A `etc/os-release` file containing `ID=fedora`.
2. A real fixture `.rpm` whose header carries `RPMTAG_VENDOR=Some Vendor`. Use the milestone-004 vendor-aware fixture under `mikebom-cli/tests/fixtures/rpm-file/` (verified-present `centos.rpm` fixture has `RPMTAG_VENDOR="CentOS"`).

Run `rpm_file::read(&tmpdir.path(), config_with_no_override)` and assert the emitted PURL has namespace `fedora` (NOT `centos`).

**Rationale**:
- Integration test exercises the real `os_release.rs` helper, the real `rpm` crate parse path, and the real precedence ladder change — no mocks.
- Reuses existing fixtures; no new RPM-construction tooling required.

**Alternatives considered**:
- **Pure unit test on `resolve_rpm_vendor_slug` only** — kept as ADDITIONAL coverage (already covers the precedence ladder at the helper level); the integration test adds end-to-end confidence.

## R10 — How are the FR-006 WARN-text assertions kept stable against `tracing` field-ordering churn?

**Decision**: Use `tracing-test = "0.2"` (NOT currently in workspace) ... actually, **rejected**. Instead use the internal `SkipReason` enum approach from R4 + a unit test that builds the WARN message string from the enum + asserts on the string (free function, no subscriber needed).

**Rationale**:
- Adding a new crate (`tracing-test`) for one test invocation is over-investment per Constitution principle "No new Cargo dependencies" guidance.
- The `SkipReason` enum + its `Display` impl gives us a pure-Rust testable surface that maps 1-to-1 with the WARN text. Test asserts on `format!("{}", reason)` containing/not-containing specific substrings.
- Decouples WARN emission from WARN message construction — useful invariant beyond this milestone.

**Alternatives considered**:
- **`tracing-subscriber::fmt::TestWriter`** — works but adds boilerplate per test and pulls in `tracing-subscriber` test features; the enum approach is simpler.
- **`tracing-test` crate** — new dep, rejected.
- **No assertion on WARN text** — rejected: FR-006 explicitly requires the wording change be observable.

---

## Summary of decisions feeding Phase 1

- `RpmReaderConfig` struct with `cap_bytes: u64` + `distro_override: Option<String>`.
- Two new `ScanArgs` fields with clap `value_parser` closures.
- Single new `SkipReason` enum (internal to `rpm_file.rs`) for testable WARN-text generation.
- One new integration test file: `mikebom-cli/tests/rpm_file_yocto_regression.rs`.
- Zero new Cargo dependencies.
- The `os_release.rs` helper is reused as-is (no API change needed; its dual-path fallback handles all the edge cases in spec §Edge Cases).
