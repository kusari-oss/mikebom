# Research: CDX License Splitter — LicenseRef Escape Hatch

**Date**: 2026-07-17
**Purpose**: Resolve 4 mechanical unknowns before task decomposition.

## R1 — SPDX License List membership check API

**Investigation**:
- The `spdx = "0.10"` workspace crate provides `spdx::LicenseId::from_name(name: &str) -> Option<&'static LicenseId>` — direct static-table lookup, returns `Some(&LicenseId)` if `name` is on the SPDX License List (any of the ~600 canonical identifiers), `None` otherwise.
- `SpdxExpression::try_canonical` at `mikebom-common/src/types/license.rs:135` also validates, but it PARSES + NORMALIZES: `try_canonical("GPL-2.0")` succeeds AND returns the canonical short-form `"GPL-2.0"` normalized from the legacy long-form. That normalization is a side-effect we do NOT want in the CDX splitter — it would silently rewrite `license.id` values in emitted output, drifting goldens.
- The spdx crate also has `spdx::exception_id(name: &str)` for SPDX exception identifiers (e.g., `Classpath-exception-2.0` in a `WITH` clause).

**Decision**: Use `spdx::LicenseId::from_name(token).is_some()` for the CDX splitter's membership check. Non-normalizing, static-table lookup, zero side effects. Extend to `exception_id` check for completeness on `WITH`-clause operands per edge case in spec.md.

**Rationale**:
- Preserves the wire-format value verbatim when the token IS on the list (no `GPL-2.0-only` → `GPL-2.0` legacy-name drift).
- Direct table lookup is O(1) — no parse-tree construction.
- Symmetric with the spdx crate's own API (no wrapper leakage).

**Alternatives considered + rejected**:
- `SpdxExpression::try_canonical(token)` — rejected: legacy-name normalization side-effect would silently drift emitted `license.id` values, breaking existing golden byte-identity.
- Regex against a hand-maintained SPDX list — rejected: reinvents what the spdx crate already provides via `from_name`.
- Full expression parse via `spdx::Expression::parse(token)` — rejected: overkill for a single-token membership check; introduces parse-error handling for a case that's already been split into individual tokens by the m190 splitter.

**References**:
- `spdx` crate docs: `LicenseId::from_name`, `exception_id`.
- `mikebom-common/src/types/license.rs:135` — existing `SpdxExpression::try_canonical` (contrast).

## R2 — Sanitizer extraction location

**Investigation**:
- `sanitize_to_license_ref_idstring` at `mikebom-cli/src/scan_fs/package_db/rpm_file.rs:778` — currently `pub(crate)`, standalone, no RPM-specific state.
- Function signature: `pub(crate) fn sanitize_to_license_ref_idstring(s: &str) -> Option<String>`. Returns `None` if sanitization can't produce a valid `LicenseRef-*` suffix (empty after filtering, all-invalid-chars, etc.).
- Test coverage in `rpm_file.rs::tests` — several test functions verify the sanitizer's behavior standalone (idempotent, alphanumeric+`-`+`.` filtering, prefix wrapping).

**Decision**: Extract to `mikebom-common/src/types/license.rs` as `pub fn sanitize_license_operand_to_ref(s: &str) -> Option<String>` (renamed for clarity — the current name `sanitize_to_license_ref_idstring` is longer than needed and doesn't imply the "operand" input scope). Keep the function's behavior byte-identical: same input → same output. Move existing tests too.

**Rationale**:
- Natural home alongside `SpdxExpression` (both are SPDX-format-related utilities).
- Avoids cross-module leakage from the RPM reader (CDX builder shouldn't need to reach into `scan_fs/package_db/rpm_file.rs`).
- `mikebom-common` is already a workspace dep of `mikebom-cli`, so import path is clean: `use mikebom_common::types::license::sanitize_license_operand_to_ref;`.
- Existing SPDX 2.3 emitter path in `rpm_file.rs` becomes a thin caller of the shared function — one-line diff for the migration.

**Alternatives considered + rejected**:
- Keep in `rpm_file.rs`, change visibility to `pub(crate)` and import via full path: rejected — cross-module leakage from a reader-specific file into an emitter is architecturally ugly and hurts discoverability.
- New standalone module `mikebom-common/src/license_ref.rs`: rejected — over-modularization for a single ~20-LOC function; existing `types/license.rs` is the right home.
- Duplicate the function in `mikebom-cli/src/generate/cyclonedx/builder.rs`: rejected — violates FR-002 parity requirement at the source-of-truth level (two implementations WILL drift over time).

**References**:
- `mikebom-cli/src/scan_fs/package_db/rpm_file.rs:778` — extraction source.
- `mikebom-common/src/types/license.rs` — extraction target.

## R3 — Golden regen scope

**Investigation** (grep of existing goldens for compound-license or non-canonical operand patterns):

```bash
# Check every CDX golden for potentially-affected license entries
grep -rlE '"id":\s*"[a-z0-9._-]+"' mikebom-cli/tests/fixtures/golden/cyclonedx/ | wc -l
# → all cdx.json goldens have license entries; need to check if any use non-canonical operands
grep -rlE '"id":\s*"[a-zA-Z0-9._-]+-[0-9]+\.[0-9]' mikebom-cli/tests/fixtures/golden/cyclonedx/ | head
```

**Empirical claim**: 0 pre-existing CDX goldens contain non-canonical license IDs. The compound-splitter path (m190) fires only on ipk/opkg readers whose recipes reference non-SPDX operands; the ecosystem goldens in `fixtures/golden/cyclonedx/` (cargo, npm, pip, maven, gem) all carry canonical SPDX identifiers.

**Verified at implement time**: run `git diff --stat mikebom-cli/tests/fixtures/` post-fix; ONLY expected diff is the new `ipk/license_licenseref_splitter_m202/` fixture. Any drift on existing goldens signals unexpected reclassification — investigate before committing.

**Decision**: Golden regen scope: **empirically 0 files**. Re-verified at implement time per m199-m201 lesson. If non-zero, scope-drift disposition per m199 pattern.

**Public-corpus goldens** (`fixtures/public_corpus/rust-ripgrep/cdx.json`, etc.): these are real-world project SBOMs. Rust/Python/npm/Maven/Go/Postgres:16 all use canonical SPDX identifiers → expected 0 drift. If any drift, regen via `public-corpus.yml` workflow_dispatch per m196/m199/m200 pattern.

**Alternatives considered + rejected**:
- Preemptively regen all cargo/npm goldens: rejected — no expected drift; would bulk the PR unnecessarily.
- Create fixture separate from existing ipk-files/: rejected — new ipk/ subdirectory is topically related and matches existing convention.

**References**:
- Memory `feedback_verify_research_empirical_claims`.
- m199-m201 all confirmed 0 golden drift for narrowly-scoped fixes.

## R4 — Regression fixture format

**Investigation** (ipk fixture layout patterns):
- Existing `mikebom-cli/tests/fixtures/ipk-files/` contains checked-in synthetic `.ipk` binary files.
- Existing `mikebom-cli/tests/fixtures/golden_inputs/opkg_basic/` contains a synthetic opkg database directory.
- The m187 ar-format reader (per issue #543) handles both real ipks and synthetic ones with correct 8-byte magic + 60-byte member headers.

**Decision**: Create a synthetic `.ipk` file for the m202 fixture. Approach:
1. Generate the fixture via a small script (`scripts/gen-m202-fixture.sh` or a Rust build-script equivalent) that runs at fixture-creation time to produce the binary. Commit the resulting `.ipk` binary directly to `tests/fixtures/ipk/license_licenseref_splitter_m202/test-license-ref.ipk`.
2. Content: an ar-format archive containing:
   - `debian-binary` (2.0)
   - `control.tar.gz` with a `control` file: `Package: test\nVersion: 1.0\nDescription: test\nLicense: GPL-2.0-only & bzip2-1.0.4\nArchitecture: all\n`
   - `data.tar.gz` with a stub file (e.g., empty `usr/bin/test`)

**Rationale**:
- Committing the binary + generation script preserves reproducibility (reviewers can re-generate + diff) without requiring `ar` / `tar` toolchain at test-runtime.
- Matches the m187 / m190 / m192 fixture-authoring pattern (all under `fixtures/ipk-files/`).
- Alternative (dynamically generate at test time) would require `ar` in CI — a system dep we've deliberately avoided elsewhere.

**Alternatives considered + rejected**:
- Reuse an existing ipk-files/ synthetic that HAPPENS to have a compound license: rejected — none exist per grep; would require fixture-content modification which drifts unrelated tests.
- Test via a directly-parsed control file (bypassing the ipk archive path): rejected — misses the archive-extraction path activated by m187; less-load-bearing.

**References**:
- `mikebom-cli/tests/fixtures/ipk-files/` — existing pattern.
- m187 / m190 / m192 fixture-authoring precedent.

## Decision Summary

| Decision | Chosen | Alternative | Rationale |
|---|---|---|---|
| Membership check API | `spdx::LicenseId::from_name(token).is_some()` | `SpdxExpression::try_canonical` | No legacy-name normalization side-effect |
| Sanitizer extraction location | `mikebom-common/src/types/license.rs` as `pub fn sanitize_license_operand_to_ref` | Keep in `rpm_file.rs` with widened visibility | Natural home; avoids reader→emitter cross-module leakage |
| Golden regen scope | Empirically 0 files (verified at implement time) | Preemptive full-corpus regen | No compound-non-canonical operands in existing goldens |
| Fixture strategy | Checked-in `.ipk` binary + optional generation script | Test-time dynamic generation | Matches m187 pattern; no CI system-dep |
| New Cargo deps | Zero | (n/a) | Nothing needed |
