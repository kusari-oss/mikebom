# Research — milestone 078 SPDX 3.0.1 conformance pass

Six implementation-level decisions to pin before Phase 1 design. The most important is **§1 — actual validator output against the existing fixtures** (run during research; results below). The fix list is smaller and more bounded than worst-case-feared.

## §1 — Validator-driven fix list (ground truth)

**Decision**: Two SHACL ClassConstraint violations to fix. Both are present in **all 9 existing SPDX 3 golden fixtures** (apk, cargo, deb, gem, golang, maven, npm, pip, rpm). No other violations surface.

### Violation A — `Core/createdBy` range mismatch (the user-reported bug)

```
Violation of type sh:ClassConstraintComponent:
  Severity: sh:Violation
  Source Shape: sh:class <https://spdx.org/rdf/3.0.1/terms/Core/Agent>
                sh:minCount 1, sh:nodeKind sh:IRI
                sh:path Core/createdBy
  Focus Node: _:creation-info
  Value Node: <…/tool/mikebom>
  Message: Value does not have class ns1:Agent
```

**Fix**: per the 2026-05-06 clarification, emit an `Organization` element with `name: "mikebom contributors"` and point `createdBy` at it. Move the existing `Tool` reference to a new `createdUsing` field.

### Violation B — `Core/dataLicense` range mismatch (NEW, not previously reported)

```
Violation of type sh:ClassConstraintComponent:
  Severity: sh:Violation
  Source Shape: sh:class <https://spdx.org/rdf/3.0.1/terms/SimpleLicensing/AnyLicenseInfo>
                sh:maxCount 1, sh:nodeKind sh:IRI
                sh:path Core/dataLicense
  Focus Node: <…/spdx3/doc-…>  (the SpdxDocument element)
  Value Node: <https://spdx.org/licenses/CC0-1.0>
  Message: Value does not have class
    <https://spdx.org/rdf/3.0.1/terms/SimpleLicensing/AnyLicenseInfo>
```

Today mikebom emits `dataLicense: "https://spdx.org/licenses/CC0-1.0"` as a bare license-URI string. SPDX 3 requires the value to be an IRI that resolves (within `@graph`) to an element typed as a subclass of `SimpleLicensing/AnyLicenseInfo` (likely `simplelicensing_License` for SPDX-listed licenses, or `simplelicensing_LicenseExpression` for the more-general expression case).

**Fix shape (recommended)**: emit a `simplelicensing_License` element with stable IRI `https://spdx.org/licenses/CC0-1.0` and `simplelicensing_simpleLicensingText: "CC0-1.0"` (or equivalent SPDX-license-expression syntax) into `@graph`. Point the SpdxDocument's `dataLicense` at that element's IRI. Implementation needs to verify the exact `simplelicensing_*` property names against the SPDX 3 model docs at implementation time — the SHACL constraint only specifies the abstract class; the concrete subclass + property-name details emerge from the model definition.

### What the validator does NOT flag (sanity-check — confirms our existing emission is conformant on these axes)

| Pattern | Status |
|---------|--------|
| `software_*` namespaced property names (`software_packageUrl`, `software_packageVersion`) | ✅ VALID |
| `type: "software_Package"` namespaced class names | ✅ VALID |
| `externalIdentifierType: "cpe23"` / `"packageUrl"` enum values | ✅ VALID |
| `_:creation-info` blank-node identifier | ✅ VALID |
| `@id` field for blank-node IDs + `spdxId` field for full-IRI element IDs | ✅ VALID |
| `@context: "https://spdx.org/rdf/3.0.1/spdx-context.jsonld"` | ✅ VALID |

Several of the suspicions raised in the spec's edge-case enumeration turn out to be unfounded — mikebom's existing emission is correct on those axes. The fix list is exactly the two SHACL violations above.

**Rationale**: Running the validator IS the milestone's design tool. Speculation about additional issues was pre-validator-execution caution; post-validator-execution we have ground truth and the scope is narrow. Plan + tasks should reflect the small actual scope.

**Alternatives considered**:
- Run a different validator (e.g., the Java SPDX library directly) — Rejected: JPEWdev `spdx3-validate` ran the SHACL shapes from the official SPDX 3 model, which is the authoritative source; running a second validator might surface validator-bugs without surfacing real spec violations the first one missed. Documented for future cross-checking only.
- Skip Phase 0 validator runs and let the implementer discover issues — Rejected: planning fixes blind would have over-scoped the milestone (e.g., if I'd assumed `software_*` naming needed changes, the task list would have been 3× larger and full of speculative fixes that the validator says aren't needed).

## §2 — JPEWdev `spdx3-validate` install + version pinning

**Decision**: Pin to PyPI version `0.0.5` (the version available at research time, 2026-05-06). Install via `pip install spdx3-validate==0.0.5` in a project-local virtualenv at `.venv/spdx3-validate/`. Provide a small `scripts/install-spdx3-validate.sh` helper that creates the venv if absent, installs the pinned version, and prints the binary path.

**Rationale**:
- **Pinned version**: per FR-008, validator updates that surface false positives should not break CI silently. A specific version pin makes upgrades deliberate PRs.
- **Project-local venv**: avoids polluting the developer's global Python; avoids `pip install --user` site-packages collisions across machines/CI runners. Also avoids `pipx` which isn't universally available.
- **Shell helper**: gives both CI and local devs a one-line install path. The integration test can call this helper if the binary is absent (or just point at the failure message).

**Alternatives considered**:
- `pipx install spdx3-validate` — Rejected: pipx isn't pre-installed on macOS or every Linux distro. The shell-helper-managed venv is more portable.
- Vendor the validator into the repo as a Python source tree — Rejected: significantly more maintenance burden; validator updates would be vendored manually.
- `uv` (modern Python package manager) — Considered but not yet ubiquitous enough on CI runners. Reach for it if pip-velocity becomes a CI bottleneck (unlikely at this milestone's scale).

The bump policy: any validator version update is a deliberate PR with proof the new version doesn't produce false positives against post-fix mikebom output. Same posture as Cargo dependency updates.

## §3 — Validator output shape + machine-readable parsing

**Decision**: Parse stderr text. Look for the literal string `"Violation of type"` to detect any failure (any non-zero match means the validator found at least one issue). Fail the integration test if any violations are present; assert the validator's exit code is 0 for full-pass.

**Rationale**: The validator's CLI surface (per `--help` + the demo run during §1 research) doesn't have a `--format json` option in version 0.0.5. Stderr text is the available output. Pattern-match for the violation marker is fragile but workable; if a future version adds `--format json` we promote to structured parsing.

**Alternatives considered**:
- Wait for `--format json` support and run a different validator in the meantime — Rejected: scope creep; existing stderr-text parsing is good enough for CI gating.
- Run the validator from Python rather than shell-out from Rust — Rejected: introduces Python-side test code that mikebom's existing test infrastructure doesn't support cleanly. Shell-out from Rust integration test is simpler.

The integration test's failure message captures the validator's full stderr verbatim so the developer sees the exact violation, not just "validation failed."

## §4 — `Organization` element IRI scheme

**Decision**: `<doc_iri>/agent/mikebom-contributors` (path-style under the document IRI, mirroring the existing `<doc_iri>/tool/mikebom` pattern). The IRI is document-scoped, not globally stable.

**Rationale**:
- **Mirrors existing pattern**: the `Tool` element today is at `<doc_iri>/tool/mikebom`. Putting the Organization at `<doc_iri>/agent/mikebom-contributors` is consistent and makes the wire-format read predictably.
- **Document-scoped is fine for v0**: the Organization IRI doesn't need to be globally identifiable across documents in the MVP. If downstream tools want to deduplicate "this is the same organization across multiple SBOMs", a future milestone can promote to a stable global IRI like `https://mikebom.kusari.dev/spdx3/agent/mikebom-contributors`.
- **Determinism**: the `<doc_iri>` portion is hash-derived from scan inputs (existing pattern), so the full IRI is deterministic per scan target.

**Alternatives considered**:
- Stable global IRI now (`https://mikebom.kusari.dev/spdx3/agent/mikebom-contributors`) — Rejected for MVP: introduces a new IRI namespace and the question of whether the Organization element is truly the same identity across documents. Defer to a future milestone if requested.
- Hash-suffixed IRI (`/agent/mikebom-contributors-<8-char-hash>`) — Rejected: the suffix adds determinism noise without value since the document IRI itself already deduplicates per scan.

## §5 — Test-side graceful-skip behavior + CI strictness

**Decision**: Two-tier behavior:

- **Local dev** (running `cargo test --workspace` without prior validator install): the `spdx3_conformance.rs` integration test prints a clear diagnostic — "spdx3-validate not found at .venv/spdx3-validate/bin/spdx3-validate; run scripts/install-spdx3-validate.sh and re-run cargo test" — and **passes** the test (returns OK). No false-fail for developers without Python.
- **CI** (when `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1` env var is set, which the CI workflow sets unconditionally): same test FAILS if the validator binary is absent. Asserts presence + zero-error validation against every fixture.

**Rationale**:
- Existing pre-PR gate semantics: developers should be able to run `./scripts/pre-pr.sh` without first installing a Python tool. The test's graceful-skip preserves that.
- CI strictness: the CI lane installs the validator unconditionally, so the test should hard-fail if for some reason the install fails silently. The env-var hook is the explicit "I want this to be required" signal that CI sets.
- Clear failure mode in CI: if the install step in `ci.yml` fails, the workflow's install step fails before the test runs, surfacing the issue at the install layer with a clear error message — not a confusing "test failed because tool isn't there" message.

**Alternatives considered**:
- Always-fail when validator absent (no graceful skip) — Rejected: hostile to local dev that doesn't have Python configured. Developers running just `cargo test -p mikebom` shouldn't need Python.
- Always-skip when validator absent (no env-var hook) — Rejected: would miss CI install failures; we'd have green CI runs that skipped the conformance check entirely.

## §6 — `Organization` element determinism contract

**Decision**: The `Organization` element's `spdxId` IRI is fully deterministic given the document IRI: `format!("{doc_iri}/agent/mikebom-contributors")`. No hashing, no random suffix. The `name` field is the literal string `"mikebom contributors"` (matching the CDX `metadata.tools[0].publisher` value verbatim).

**Rationale**:
- Document IRI is already deterministic (hash-derived from scan inputs at `v3_document.rs:107`+).
- Path-style suffix is constant.
- Publisher name is constant per release.
- Result: byte-identical Organization element across re-runs given fixed scan inputs. Same as the Tool element's existing determinism.

**Alternatives considered**:
- Hash-derived suffix on the Organization IRI (e.g., `/agent/mikebom-contributors-abc1234`) — Rejected: adds noise without value; the document IRI's hash already disambiguates per scan.
- Pull the publisher name from `cfg.mikebom_version` at emission time — Rejected: the publisher name and the version are different concepts; coupling them would break correctness if the publisher name ever needs to change.
