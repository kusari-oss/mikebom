# Quickstart — Authoring a `--supplement-cdx` file + reading the resulting SBOM

**Feature**: 119-supplement-cdx
**Audience**: an operator authoring a supplement file; a CI maintainer integrating `--supplement-cdx` into a scan pipeline; a consumer reading an SBOM that incorporated a supplement; a contributor extending the supplement merge semantics.

## The TL;DR

`mikebom sbom scan --supplement-cdx <PATH>` accepts a hand-authored CDX 1.6 JSON document listing components/services/dependencies the scanner cannot observe (SaaS deps, vendored libraries, license overrides). mikebom merges with scanner output via a hard/soft split:
- **Scanner wins** on bytes-derived facts (hashes, cpe, purl, version, binary fingerprint)
- **Developer wins** on metadata (licenses, supplier, copyright, name, description, externalReferences)

Critical safety property: the developer cannot suppress scanner detection of bytes-evident content. A supplement asserting "no openssl" still produces an SBOM containing openssl.

## Five-minute walkthrough — operator with a SaaS dep + vendored library

You ship a Rust project that depends on Stripe (SaaS) and `liberror` (vendored from upstream at `third_party/liberror/`, no Cargo.toml). The scanner sees only your Cargo project + its transitive deps; it doesn't know about Stripe or `liberror`. You author `supplement.cdx.json`:

```json
{
  "bomFormat": "CycloneDX",
  "specVersion": "1.6",
  "components": [
    {
      "type": "library",
      "bom-ref": "liberror-1.2.3",
      "purl": "pkg:generic/liberror@1.2.3",
      "name": "liberror",
      "version": "1.2.3",
      "supplier": { "name": "Acme Open Source Foundation" },
      "licenses": [ { "license": { "id": "MIT" } } ],
      "copyright": "© 2026 Acme",
      "description": "Error-handling library vendored from upstream"
    }
  ],
  "services": [
    {
      "bom-ref": "stripe-saas",
      "name": "Stripe",
      "provider": { "name": "Stripe, Inc." },
      "endpoints": [ "https://api.stripe.com" ],
      "description": "Payment processing"
    }
  ],
  "dependencies": [
    {
      "ref": "pkg:cargo/my-app@1.0.0",
      "dependsOn": [ "liberror-1.2.3", "stripe-saas" ]
    }
  ]
}
```

CI runs:

```bash
mikebom sbom scan --path . --supplement-cdx supplement.cdx.json --output sbom.cdx.json
```

The emitted SBOM contains:
- Every scanner-discovered component (your Cargo project + its transitive deps)
- The `pkg:generic/liberror@1.2.3` component carrying the declared license, supplier, copyright + `mikebom:source-tier = "declared"` annotation
- The Stripe service entry under `services[]` (a section scanner never populates)
- A new dependency edge from `my-app` → `liberror` AND `my-app` → `stripe-saas`
- A document-scope `mikebom:supplement-cdx` annotation on `metadata.properties[]` recording the supplement file's path + sha256

Operators' stderr also shows:

```text
INFO mikebom::cli::scan_cmd: scan complete components=42 relationships=58 ...
```

(The existing summary; this feature doesn't add stderr noise for supplement use.)

## Five-minute walkthrough — operator overriding a license

The scanner discovers `pkg:cargo/opaque-lib@1.0.0` but emits `licenses[] = []` because the upstream Cargo.toml carries no license field. You KNOW the library is `Apache-2.0` (from its upstream docs). You author:

```json
{
  "bomFormat": "CycloneDX",
  "specVersion": "1.6",
  "components": [
    {
      "purl": "pkg:cargo/opaque-lib@1.0.0",
      "licenses": [ { "license": { "id": "Apache-2.0" } } ]
    }
  ]
}
```

Scan with `--supplement-cdx`. The emitted SBOM's `opaque-lib` component:
- Carries `licenses[0].license.id = "Apache-2.0"` (developer wins on metadata)
- Carries a `mikebom:assertion-conflict` annotation recording: `{ field: "licenses", scanner_value: [], supplement_value: [...], winner: "supplement", justification: "developer-metadata-override" }`
- The scanner's empty value is preserved verbatim inside the annotation for audit

A vulnerability scanner downstream now reads `Apache-2.0` and acts accordingly. A compliance auditor reads the `mikebom:assertion-conflict` annotation and can verify the operator-asserted license against the scanner's (empty) finding.

## Five-minute walkthrough — what happens when developer tries to suppress detection

The scanner detects `pkg:generic/openssl@3.0.10` via symbol fingerprints (milestone-099). A developer mistakenly believes the build doesn't statically link openssl and tries:

```json
{
  "components": [
    { "purl": "pkg:generic/openssl@3.0.10", "_mikebom_assert_absent": true }
  ]
}
```

The custom `_mikebom_assert_absent` field is silently DROPPED at parse time (the parser doesn't accept it). The supplement entry is treated as a normal declaration, which collides with the scanner's existing detection.

Result:
- The emitted SBOM STILL CONTAINS `pkg:generic/openssl@3.0.10` (FR-015 — scanner detection is preserved).
- The component carries a `mikebom:assertion-conflict` annotation if any field differed between scanner and supplement.
- The developer learns from the still-present openssl component that their build IS statically linking openssl.

This is the safety property in action — there is NO supplement input that suppresses bytes-evident scanner detection.

## Five-minute walkthrough — consumer reading the supplemented SBOM

Downstream consumer (security scanner, compliance auditor, SBOM-quality dashboard) reads the emitted SBOM and wants to distinguish scanner-observed components from developer-declared ones. One grep over `components[].properties[]`:

```bash
jq '.components[] | select(.properties[]?.name == "mikebom:source-tier" and .properties[]?.value == "declared") | .purl' sbom.cdx.json
```

Lists every supplement-declared component PURL. The consumer assigns different trust weights — declared components carry operator authority, scanner-observed components carry scanner-bytes authority.

To enumerate every disagreement:

```bash
jq '.components[] | select(.properties[]?.name == "mikebom:assertion-conflict") | {purl, conflicts: [.properties[] | select(.name == "mikebom:assertion-conflict") | .value | fromjson]}' sbom.cdx.json
```

Outputs: per-component PURL + a list of every conflicted field with each side's value + the winner + the justification. Auditable.

To verify which supplement file fed the merge:

```bash
jq '.metadata.properties[] | select(.name == "mikebom:supplement-cdx") | .value' sbom.cdx.json
```

Outputs: `"supplement.cdx.json@sha256:5e884898da28..."`. The consumer recomputes the sha256 of the operator's source-control supplement file; mismatch indicates tampering between scan time and audit time.

## Contributor walkthrough — extending the merge semantics

You want to add a new field to `DEVELOPER_AUTHORITATIVE_FIELDS` (e.g., `pedigree.notes` as operator-authoritative provenance text).

### Step 1 — Update the field-set membership

Edit `mikebom-cli/src/supplement/conflict.rs`'s `DEVELOPER_AUTHORITATIVE_FIELDS` constant:

```rust
pub(crate) const DEVELOPER_AUTHORITATIVE_FIELDS: &[&str] = &[
    "licenses",
    "concluded_licenses",
    "supplier",
    "copyright",
    "name",
    "description",
    "externalReferences",
    "pedigree_notes",  // new
];
```

Add the corresponding `ConflictField::PedigreeNotes` enum variant in `conflict.rs`.

### Step 2 — Update the parser's honored-fields list

Edit `mikebom-cli/src/supplement/parser.rs`'s parsing logic to extract the new field from `components[].pedigree.notes` if present.

### Step 3 — Add an integration test

In `mikebom-cli/tests/supplement_cdx_integration.rs`, add a test verifying that a supplement declaring `pedigree.notes` overrides any scanner-side value AND emits the `mikebom:assertion-conflict` annotation when they differ.

### Step 4 — Update docs

Add `pedigree_notes` to `contracts/merge-pipeline.md`'s "DEVELOPER_AUTHORITATIVE_FIELDS" table. Update the supplement-format.md "Honored optional fields" table to include the new field.

The pattern is: one field-set constant edit + one parser edit + one test + one docs edit. Adding fields is intentionally low-friction.

## Negative test runbook

Verify the gate's safety property and fail-closed behavior:

### Test 1 — malformed JSON

```bash
echo "{ this is not JSON" > /tmp/bad.json
mikebom sbom scan --path . --supplement-cdx /tmp/bad.json
# Expected: exit code 1, error message naming /tmp/bad.json + JSON parse error
# Expected: NO SBOM file created
```

### Test 2 — missing supplement file

```bash
mikebom sbom scan --path . --supplement-cdx /tmp/nonexistent.json
# Expected: exit code 1, error naming /tmp/nonexistent.json + I/O error
# Expected: NO SBOM file created
```

### Test 3 — schema-invalid supplement

```bash
echo '{ "components": [ { "no-purl-field": "oops" } ] }' > /tmp/invalid.json
mikebom sbom scan --path . --supplement-cdx /tmp/invalid.json
# Expected: exit code 1, error like "supplement.cdx.json: components[0] missing required key `purl`"
# Expected: NO SBOM file created
```

### Test 4 — duplicate PURL in supplement

```bash
cat > /tmp/dup.json <<EOF
{ "bomFormat": "CycloneDX", "specVersion": "1.6",
  "components": [
    { "purl": "pkg:generic/foo@1.0.0" },
    { "purl": "pkg:generic/foo@1.0.0" }
  ]
}
EOF
mikebom sbom scan --path . --supplement-cdx /tmp/dup.json
# Expected: exit code 1, error like "supplement.cdx.json: duplicate PURL pkg:generic/foo@1.0.0"
# Expected: NO SBOM file created
```

### Test 5 — dangling dependsOn reference

```bash
cat > /tmp/dangling.json <<EOF
{ "bomFormat": "CycloneDX", "specVersion": "1.6",
  "dependencies": [ { "ref": "nowhere", "dependsOn": ["also-nowhere"] } ]
}
EOF
mikebom sbom scan --path . --supplement-cdx /tmp/dangling.json
# Expected: exit code 1, error like "supplement.cdx.json: dangling dependsOn reference also-nowhere"
# Expected: NO SBOM file created
```

All five tests are codified in `mikebom-cli/tests/supplement_cdx_integration.rs` per plan.md § Source Code.

## When NOT to use `--supplement-cdx`

- **Scanner correctly discovers everything you care about**: no need for supplement; scanner output is enough.
- **You want to suppress a detection**: the feature CANNOT do this (FR-015). If you believe a scanner detection is wrong, file an issue with the false-positive details rather than trying to suppress via supplement.
- **You want a lockfile-style import**: the supplement is for OPERATOR-ASSERTED data the scanner can't observe. If you want to import a lockfile's data into the SBOM, that's the scanner's existing lockfile-reader path (already in place for Cargo.lock, package-lock.json, etc.).
- **You want to override the scan target's identity**: use `--scan-as`; supplement's `metadata.component` is IGNORED per FR-014.

## Related docs

- [`spec.md`](./spec.md) — the user-visible contract
- [`research.md`](./research.md) — 8 implementation decisions
- [`data-model.md`](./data-model.md) — `Supplement` + `ConflictRecord` + `MergeOutcome` + extra_annotations channel
- [`contracts/supplement-format.md`](./contracts/supplement-format.md) — the CDX 1.6 subset mikebom accepts
- [`contracts/merge-pipeline.md`](./contracts/merge-pipeline.md) — the merge step's pre/post invariants + conflict-resolution algorithm
- [`contracts/annotation-shape.md`](./contracts/annotation-shape.md) — the 3 new annotation keys' value shapes
- Issue #326 — the motivating prior-art research issue
- Milestone 105 (`specs/105-source-mechanism-dedup-pipeline/`) — the dedup pipeline the merge runs after
- Milestone 110 (`specs/110-self-identity-resolver/`) — the `--scan-as` resolver that owns scan-target identity (FR-014)
- Milestone 113 (`specs/113-exclude-path-flag/`) — the document-scope envelope annotation precedent the supplement-cdx annotation mirrors
- `docs/reference/sbom-format-mapping.md` — the canonical home for the C65/C66/C67 Principle V audit citations
