# Quickstart — milestone 092 maintainer recipes

Five maintainer-facing recipes to reproduce the bug, apply the fix, regenerate the milestone-083 baseline, and confirm post-092 byte-stability for non-Maven fixtures.

## Recipe 1 — Reproduce the pre-092 buggy emission

```bash
cargo +stable build --release -p mikebom

target/release/mikebom --offline sbom scan \
    --path "$MIKEBOM_FIXTURES_DIR/transitive_parity/maven" \
    --format spdx-2.3-json \
    --output /tmp/pre-092.spdx.json \
    --no-deep-hash

# Look up the main-module's PURL:
jq -r '.packages[] | select(.name == "commons-lang3") | .externalRefs[] | select(.referenceType == "purl") | .referenceLocator' /tmp/pre-092.spdx.json
# Expected (BUG): pkg:maven/org.apache.commons/commons-lang3@64
```

## Recipe 2 — Apply the fix

Implementation lives in `mikebom-cli/src/scan_fs/package_db/maven.rs`. Three edits:

```rust
// Edit 1: PomXmlDocument struct (~line 546)
//
//     pub self_artifact_id: Option<String>,
//     pub self_version: Option<String>,  // NEW — milestone 092
//     pub modules: Vec<String>,

// Edit 2: parse_pom_xml constructor (~line 711)
//
//     doc.self_artifact_id = self_a.clone();
//     doc.self_version = self_v.clone();  // NEW — milestone 092

// Edit 3: build_maven_main_module_entry's raw_version chain (~line 3412)
//
//     let raw_version = doc
//         .self_coord
//         .as_ref()
//         .map(|c| c.2.clone())
//         .or_else(|| doc.self_version.clone())  // NEW — milestone 092
//         .or_else(|| doc.parent_coord.as_ref().map(|c| c.2.clone()))?;

// Edit 4: resolve_pom_property_value's "project.version" arm (~line 3351)
//
//     "project.version" => self_doc
//         .self_coord
//         .as_ref()
//         .map(|c| c.2.clone())
//         .or_else(|| self_doc.self_version.clone())  // NEW — milestone 092
//         .or_else(|| {
//             self_doc.parent_coord.as_ref().map(|c| c.2.clone())
//         }),

// Edit 5: resolve_maven_property's "project.version" arm (~line 738)
//
//     "project.version" => doc
//         .self_coord
//         .as_ref()
//         .map(|(_, _, v)| v.clone())
//         .or_else(|| doc.self_version.clone()),  // NEW — milestone 092
```

Update doc-comment on `self_artifact_id` (line 540-546) to also reference `self_version` for the parallel inheritance gap.

## Recipe 3 — Verify post-092 emission

```bash
cargo +stable build --release -p mikebom

target/release/mikebom --offline sbom scan \
    --path "$MIKEBOM_FIXTURES_DIR/transitive_parity/maven" \
    --format spdx-2.3-json \
    --output /tmp/post-092.spdx.json \
    --no-deep-hash

# Same query as Recipe 1:
jq -r '.packages[] | select(.name == "commons-lang3") | .externalRefs[] | select(.referenceType == "purl") | .referenceLocator' /tmp/post-092.spdx.json
# Expected (FIX): pkg:maven/org.apache.commons/commons-lang3@3.14.0
```

## Recipe 4 — Regenerate the milestone-083 transitive-parity baseline

```bash
# Step 1: run the existing transitive_parity_maven test post-fix.
cargo +stable test -p mikebom --test transitive_parity_maven
# If the milestone-083 expectation already pinned @3.14.0 (per FR-006),
# this should now PASS (was failing pre-092). If the expectation pinned
# @64 to "match the buggy emission", update it to @3.14.0.

# Step 2: confirm the EXPECTED_REPRESENTATIVE_EDGES match.
grep -n "commons-lang3" mikebom-cli/tests/transitive_parity_maven.rs
# Expected: at least one edge entry with @3.14.0.

# Step 3: re-run, confirm pass.
cargo +stable test -p mikebom --test transitive_parity_maven
# Expected: 4/4 tests pass.
```

## Recipe 5 — Confirm byte-stability for non-Maven goldens

```bash
cargo +stable test -p mikebom --test cdx_regression
cargo +stable test -p mikebom --test spdx_regression
cargo +stable test -p mikebom --test spdx3_regression
# Expected: 0 failures across all three. Maven goldens MAY change
# (correcting parent-version → project-version); non-Maven goldens
# MUST NOT change.

# Audit the diff scope:
git status --short mikebom-cli/tests/fixtures/golden/
# Expected: only maven golden files touched, if any.
```

If a Maven golden file shows a version flip (e.g., `@64` → `@3.14.0`), confirm:

```bash
git diff mikebom-cli/tests/fixtures/golden/ | grep -E "^[+-]" | head -20
# Expected: ONLY version-string flips on commons-lang3-style entries.
# NO PURL count changes, NO new components, NO new annotations.
```

## Recipe 6 — Final pre-PR gate

```bash
./scripts/pre-pr.sh
```

Expected: zero clippy warnings, every test suite reports `0 failed`. Standard CLAUDE.md mandatory gate.

## When in doubt

- **Post-092 emits correct version but a different fixture broke**: confirm the broken fixture has `<groupId>` AND `<version>` at project level. If yes, the issue is independent of milestone 092 (the `self_coord` path is unchanged).
- **Property substitution still emits parent's version**: check that Contract 3 (the `resolve_pom_property_value` edit) was applied. The main-module fix alone doesn't cover `${project.version}`-resolved dep edges.
- **Test failure in `transitive_parity_maven` pinning `@64`**: pre-092 the test may have been "XFAIL"-style, expecting the buggy value to lock in the regression. Update to the correct `@3.14.0`.
- **`mikebom-test-fixtures` cache stale**: the fixture sibling repo's build.rs cache fetch should already have the right pom.xml. If you've recently re-pulled, run `cargo clean` once and rebuild.
- **Edge count change in transitive_parity_maven**: not expected. The milestone-092 fix only changes the **version string** of the main-module emission; the dependency edges themselves are unaffected (their PURL targets are constructed independently via dep groupId/artifactId/version).
