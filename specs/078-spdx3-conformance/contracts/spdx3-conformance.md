# Contract — milestone 078 SPDX 3.0.1 conformance pass

The milestone's only contract.

## CLI surface

**No new flags.** The conformance pass is purely a wire-format change to existing SPDX 3 emission. Operators see no new flags on `mikebom sbom scan` or `mikebom trace run`.

## Library surface (`mikebom-cli` crate)

**No new public Rust API surface.** All changes are internal to `mikebom-cli/src/generate/spdx/v3_document.rs`. The graph-construction code adds two new graph entries (Organization, License) and one new field on the existing CreationInfo entry.

## Validator integration surface

### New shell helper script

```text
scripts/install-spdx3-validate.sh
```

- Creates `.venv/spdx3-validate/` if absent
- Installs `spdx3-validate==0.0.5` (pinned version per research §2)
- Prints the binary path to stdout
- Idempotent: re-running is a no-op if already installed at the pinned version

### New integration test

```text
mikebom-cli/tests/spdx3_conformance.rs
```

Test invocations:
1. **Per-fixture validation loop** — for each `*.spdx3.json` under `mikebom-cli/tests/fixtures/golden/spdx-3/`, shell out to the validator binary; assert exit code 0 + stderr contains zero `"Violation of type"` markers.
2. **Fresh-emission validation** — for at least 3 representative scan targets (one source-tier, one image-tier, one synthetic-build-tier per FR-003), invoke `mikebom sbom scan` to emit a fresh SPDX 3 document, then validate it. Assert same zero-violation outcome.

Behavior when the validator binary is absent:
- If `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1` env var is set (CI lane): test FAILS with a clear "validator not found at .venv/spdx3-validate/bin/spdx3-validate; run scripts/install-spdx3-validate.sh" message. CI workflow sets this env var unconditionally.
- Otherwise (local dev): test prints the same diagnostic to stderr but returns OK (graceful skip per research §5).

### CI workflow update

```text
.github/workflows/ci.yml
```

In the `Lint + test (linux-x86_64)` job, add a step before `cargo test`:

```yaml
- name: Install spdx3-validate
  run: bash scripts/install-spdx3-validate.sh
- name: Set MIKEBOM_REQUIRE_SPDX3_VALIDATOR
  run: echo "MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1" >> $GITHUB_ENV
```

The `Lint + test (macos-latest)` job intentionally does NOT install the validator (per the plan's project-structure decision — Linux is the authoritative gate; macOS Python toolchain variability is a known noise vector that we don't want gating PRs).

## Wire-format contract (per SPDX 3 model)

### CreationInfo expectations

```json
{
  "type": "CreationInfo",
  "@id": "_:creation-info",
  "specVersion": "3.0.1",
  "created": "<RFC 3339 timestamp>",

  // SHACL: range = Core/Agent, minCount = 1, nodeKind = IRI
  "createdBy": [
    "<IRI of an Agent-class element in @graph>"
  ],

  // NEW in milestone 078 (per FR-001 + clarification):
  // SHACL: range = Core/Tool (per SPDX 3 Core model)
  "createdUsing": [
    "<IRI of a Tool-class element in @graph>"
  ]
}
```

### Organization element (NEW)

Per SPDX 3 Core/Organization. Concrete Agent subclass.

```json
{
  "type": "Organization",
  "spdxId": "<doc_iri>/agent/mikebom-contributors",
  "creationInfo": "_:creation-info",
  "name": "mikebom contributors"
}
```

### Tool element (UNCHANGED)

The existing element's content is preserved verbatim. Only the slot referencing it on CreationInfo moves from `createdBy` to `createdUsing`.

### SpdxDocument.dataLicense expectations

```json
{
  "type": "SpdxDocument",
  ...
  // SHACL: range = SimpleLicensing/AnyLicenseInfo, maxCount = 1, nodeKind = IRI
  "dataLicense": "<IRI of an AnyLicenseInfo-class element in @graph>"
}
```

The IRI value MUST resolve (within @graph) to an element of an AnyLicenseInfo subclass. mikebom emits `simplelicensing_LicenseExpression` (verified during T001(c) audit against `mikebom-cli/tests/fixtures/schemas/spdx-3.0.1.json`'s `simplelicensing_AnyLicenseInfo_derived` enumeration).

### simplelicensing_LicenseExpression element (NEW)

```json
{
  "type": "simplelicensing_LicenseExpression",
  "spdxId": "https://spdx.org/licenses/CC0-1.0",
  "creationInfo": "_:creation-info",
  "simplelicensing_licenseExpression": "CC0-1.0"
}
```

**Verified property names**: `simplelicensing_LicenseExpression` is the concrete `AnyLicenseInfo` subclass enumerated by the SPDX 3 JSON-LD schema; `simplelicensing_licenseExpression` is its required property carrying the SPDX-listed license id (or expression). This shape is byte-identical to the per-component license-expression elements mikebom's existing emission already produces (`mikebom-cli/src/generate/spdx/v3_licenses.rs`), so the milestone-078 fix reuses the established pattern.

## Observable contract

### Pre-fix: validator output

```bash
$ spdx3-validate -j cargo.spdx3.json
ERROR: SHACL Validation failed for cargo.spdx3.json:
Violation of type sh:ClassConstraintComponent:
  Result path: Core/createdBy
  Message: Value does not have class ns1:Agent
Violation of type sh:ClassConstraintComponent:
  Result path: Core/dataLicense
  Message: Value does not have class SimpleLicensing/AnyLicenseInfo
$ echo $?
1
```

### Post-fix: validator output

```bash
$ spdx3-validate -j cargo.spdx3.json
✔ Loading cargo.spdx3.json
✔ Loading SPDX 3.0.1
✔ Validating schema for cargo.spdx3.json
✔ Checking SHACL for cargo.spdx3.json
$ echo $?
0
```

### Java SPDX library outcome (the user's original failure scenario)

The Java SPDX library throws "Incompatible type for property `Core/createdBy`: class `core.Agent`" when consuming pre-fix output. Post-fix, the same library processes the file without exception because `createdBy` resolves to an `Organization` (which extends Agent) and `createdUsing` is the slot the Java library expects for the Tool reference.

## Determinism contract (per FR-009, SC-007)

- Same scan inputs → byte-identical SPDX 3 output across re-runs.
- The new Organization element is deterministic per research §6.
- The new License element has a stable IRI (`https://spdx.org/licenses/CC0-1.0`) — same across all scans (it's an SPDX-listed-license, not scan-specific).
- The CreationInfo's `createdUsing` field references the same Tool IRI that already exists.

## Test contract

A new integration-test file `mikebom-cli/tests/spdx3_conformance.rs` MUST cover (per US1 + US2 + US3 acceptance scenarios):

| Test | Acceptance scenario | Validates |
|------|--------------------|-----------|
| `every_existing_golden_passes_validator` | US2 §1, SC-001 | FR-002 — loops over all 9 fixtures |
| `fresh_source_tier_emission_passes` | US2 §2, SC-002 | FR-003 — emits + validates |
| `fresh_image_tier_emission_passes` | US2 §2, SC-002 | FR-003 — emits + validates |
| `fresh_synthetic_build_tier_emission_passes` | US2 §2, SC-002 | FR-003 — synthetic ScanArtifacts + validates |
| `created_by_references_organization_post_fix` | US1 §3, SC-003 | FR-001 |
| `created_using_references_tool_post_fix` | US1 §3, SC-003 | FR-001 |
| `data_license_references_simplelicensing_license_post_fix` | (no spec acceptance scenario; covers Phase 0 §1 violation B — asserts `simplelicensing_LicenseExpression` per T001(c)) | FR-002 implicitly |
| `validator_absence_graceful_skip_local` | edge case | FR-005 graceful-skip |
| `validator_absence_hard_fail_ci` | edge case | research §5 strict-mode |
| `validator_pinned_version_check` | bump-policy | FR-008 |

## Performance contract

- Validator runs in <30s against the 9-fixture suite (per research §2 + JPEWdev tool's documented complexity).
- Integration test wall-time <60s end-to-end including pip install if not cached.
- Determinism preserved (FR-009): re-running the test against the same fixture set + same validator version produces identical results.
