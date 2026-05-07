# Quickstart — milestone 078 SPDX 3.0.1 conformance pass

Five operator-facing recipes covering the post-fix SPDX 3 wire shape, validator usage, and the Java-library cross-check.

## Recipe 1 — Inspect the new SPDX 3 wire shape

```bash
mikebom sbom scan --path . --output out.spdx3.json
jq '.["@graph"][] | select(.type == "CreationInfo")' out.spdx3.json
# {
#   "@id": "_:creation-info",
#   "type": "CreationInfo",
#   "specVersion": "3.0.1",
#   "created": "...",
#   "createdBy": ["...://.../agent/mikebom-contributors"],   ← NEW: Organization
#   "createdUsing": ["...://.../tool/mikebom"]               ← NEW: Tool moved here
# }

jq '.["@graph"][] | select(.type == "Organization")' out.spdx3.json
# {
#   "type": "Organization",
#   "spdxId": "...://.../agent/mikebom-contributors",
#   "creationInfo": "_:creation-info",
#   "name": "mikebom contributors"
# }

jq '.["@graph"][] | select(.spdxId | endswith("/tool/mikebom"))' out.spdx3.json
# {
#   "type": "Tool",
#   "spdxId": "...://.../tool/mikebom",
#   "creationInfo": "_:creation-info",
#   "name": "mikebom-0.1.0-alpha.18"     ← unchanged from pre-fix
# }

# NEW in milestone 078: dataLicense now resolves (within @graph) to
# a typed simplelicensing_LicenseExpression element (concrete name
# verified via T001(c) audit against the SPDX 3 JSON-LD schema's
# simplelicensing_AnyLicenseInfo_derived enumeration).
jq '.["@graph"][] | select(.type == "simplelicensing_LicenseExpression")' out.spdx3.json
# {
#   "type": "simplelicensing_LicenseExpression",
#   "spdxId": "https://spdx.org/licenses/CC0-1.0",
#   "creationInfo": "_:creation-info",
#   "simplelicensing_licenseExpression": "CC0-1.0"
# }
```

The Tool element's content is byte-identical to pre-fix mikebom; only its referencing slot on CreationInfo moved. The new License element shape is byte-identical to mikebom's existing per-component license-expression elements.

## Recipe 2 — Run the JPEWdev validator locally

```bash
# One-time setup
bash scripts/install-spdx3-validate.sh
# → installed to .venv/spdx3-validate/bin/spdx3-validate
# → version 0.0.5 (pinned)

# Validate any mikebom-emitted SPDX 3 file
.venv/spdx3-validate/bin/spdx3-validate -j out.spdx3.json
# ✔ Loading out.spdx3.json
# ✔ Loading SPDX 3.0.1
# ✔ Validating schema for out.spdx3.json
# ✔ Checking SHACL for out.spdx3.json
# (zero output = clean validation, exit 0)
```

If you see any `Violation of type sh:ClassConstraintComponent:` lines, that's a regression — file an issue on the project with the validator's full stderr captured.

## Recipe 3 — Validate a freshly-emitted SBOM

```bash
mikebom sbom scan --path /opt/some-project --output /tmp/scan.spdx3.json
.venv/spdx3-validate/bin/spdx3-validate -j /tmp/scan.spdx3.json
echo $?
# 0  ← validator passed; SBOM conforms
```

Or use the helper-managed binary:

```bash
$(bash scripts/install-spdx3-validate.sh) -j /tmp/scan.spdx3.json
```

(The helper prints the binary path on stdout; using `$(...)` substitutes it inline.)

## Recipe 4 — Cross-check with the Java SPDX library

For operators who hit the original `Incompatible type for property … Core/createdBy: class … core.Agent` error: the post-fix output passes the same Java library's range checks. To verify:

```java
// The Java SPDX library snippet that previously threw on mikebom's output:
SpdxDocument doc = SpdxModelFactory.openSpdxDocument(
    "out.spdx3.json", SerializationFormat.JSON_LD);
// Pre-fix: throws "Incompatible type for property ... Core/createdBy: class ... core.Agent"
// Post-fix: returns a populated SpdxDocument; createdBy[0] is an Organization instance,
//           createdUsing[0] is a Tool instance.
```

The user-reported bug is fixed when the same operator's tool that previously threw now returns successfully. No change to the Java library version needed; mikebom's wire format is now what the library expects.

## Recipe 5 — Pre-PR gate behavior on a Mac that doesn't have Python configured

```bash
./scripts/pre-pr.sh
# >>> cargo +stable clippy --workspace --all-targets -- -D warnings
# >>> cargo +stable test --workspace
# (during cargo test, the spdx3_conformance.rs integration test runs:)
# WARN spdx3-validate not found at .venv/spdx3-validate/bin/spdx3-validate;
#      run scripts/install-spdx3-validate.sh and re-run cargo test.
#      Skipping conformance check — local dev mode (graceful skip).
# (test passes, gate proceeds)
# >>> all pre-PR checks passed.
```

The graceful-skip preserves the local-dev experience — developers without Python configured can still run pre-PR. CI runs the install step first and treats validator-absence as a hard failure (the env var `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1` is set by the workflow).

## What's NOT changed by this milestone

- **CDX 1.6 emission**: byte-identical to alpha.18.
- **SPDX 2.3 emission**: byte-identical to alpha.18.
- **CLI flag set**: no new flags. Existing flags from milestones 073–077 unchanged.
- **Operator-facing UX for SBOM scanning**: identical workflow; just the SPDX 3 output now passes external conformance validators.
