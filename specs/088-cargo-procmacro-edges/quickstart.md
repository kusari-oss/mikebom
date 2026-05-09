# Quickstart — milestone 088 maintainer recipes

Three maintainer-facing recipes for verifying the post-087 proc-macro outgoing-edge behavior, locking it down via regression test, and closing GitHub issue #173.

## Recipe 1 — Verify the post-087 behavior holds

```bash
cargo +stable build --release -p mikebom

target/release/mikebom --offline sbom scan \
    --path mikebom-cli/tests/fixtures/transitive_parity/cargo \
    --format spdx-2.3-json \
    --output /tmp/check-088.spdx.json \
    --no-deep-hash

# Find clap_derive's outgoing edges:
jq -r '
  ([.packages[] | {(.SPDXID): (.externalRefs[]? | select(.referenceType == "purl") | .referenceLocator)}] | add) as $purl |
  [.relationships[]
   | select(.relationshipType == "DEPENDS_ON")
   | {from: $purl[.spdxElementId], to: $purl[.relatedSpdxElement]}
   | select(.from != null and .to != null)
   | select(.from == "pkg:cargo/clap_derive@4.5.18")
   | .to
  ] | sort
' /tmp/check-088.spdx.json
```

Expected output (4 entries, sorted):
```json
[
  "pkg:cargo/heck@0.5.0",
  "pkg:cargo/proc-macro2@1.0.86",
  "pkg:cargo/quote@1.0.36",
  "pkg:cargo/syn@2.0.70"
]
```

If fewer than 4, the post-087 behavior has regressed and the regression test below will catch it. If different versions, the workspace lockfile has been updated — the PURL-prefix matcher in the regression test still works.

## Recipe 2 — Add the regression-test representative edges

Edit `mikebom-cli/tests/transitive_parity_cargo.rs::EXPECTED_REPRESENTATIVE_EDGES` and append 4 entries after the existing milestone-087 baseline:

```rust
const EXPECTED_REPRESENTATIVE_EDGES: &[(&str, &str)] = &[
    // ... existing 4 entries from milestone 087 unchanged ...
    ("pkg:cargo/clap", "pkg:cargo/automod"),
    ("pkg:cargo/anstream", "pkg:cargo/anstyle"),
    ("pkg:cargo/terminal_size", "pkg:cargo/rustix"),
    ("pkg:cargo/clap", "pkg:cargo/clap_builder"),
    // Milestone 088 additions — proc-macro outgoing-edge pin (closes #173):
    ("pkg:cargo/clap_derive", "pkg:cargo/heck"),
    ("pkg:cargo/clap_derive", "pkg:cargo/proc-macro2"),
    ("pkg:cargo/clap_derive", "pkg:cargo/quote"),
    ("pkg:cargo/clap_derive", "pkg:cargo/syn"),
];
```

Update the surrounding doc-comment to reflect the post-088 invariant:

```rust
/// ...
/// Closed by milestone 087:
/// - The `clap@4.5.21 → clap_builder@4.5.9` wrong-version edge is gone.
///
/// Closed by milestone 088 (locked-down by milestone 087's fix):
/// - clap_derive emits its 4 declared outgoing edges (heck, proc-macro2,
///   quote, syn) per Cargo.lock. Pinned below as the post-088 invariant.
const EXPECTED_REPRESENTATIVE_EDGES: &[(&str, &str)] = &[ ... ];
```

Then run the test:

```bash
cargo +stable test -p mikebom --test transitive_parity_cargo
# Expected: 4 tests pass.
```

## Recipe 3 — Update the milestone-083 audit research doc

Edit `specs/083-transitive-correctness/research.md §8 — Ecosystem: cargo`. Two changes:

**Change 1**: Mark gap #2 closed.

Find:
```markdown
2. **clap_derive emits zero outgoing edges**: mikebom emits no DEPENDS_ON entries from `clap_derive`, despite Cargo.lock showing it depends on `proc-macro2`, `quote`, `syn`. Trivy + syft both emit those edges. Suggests the cargo reader skips proc-macro crate dep extraction entirely for procedural-macro-typed Cargo.toml entries. (Issue #173 — open.)
```

Replace with:
```markdown
2. ~~**clap_derive emits zero outgoing edges**: mikebom emits no DEPENDS_ON entries from `clap_derive`, despite Cargo.lock showing it depends on `heck`, `proc-macro2`, `quote`, `syn`.~~ **Closed by milestone 087** (issue #173). Same root cause as gap #1: `parse_lockfile`'s `pkg.source.is_none()` skip dropped workspace MEMBERS from the component set, so `clap_derive@4.5.18` (a workspace-member proc-macro crate) had no PURL to attach outgoing edges to. With the skip removed, mikebom now emits 4 outgoing `DEPENDS_ON` edges from clap_derive matching the lockfile + matching trivy + syft. Pinned in `transitive_parity_cargo.rs::EXPECTED_REPRESENTATIVE_EDGES`.
```

**Change 2**: Annotate the "Filed follow-up issues" footer.

Find:
```markdown
- **#173** — cargo: proc-macro crates emit zero outgoing edges (clap_derive case)
```

Replace with:
```markdown
- **#173** — cargo: proc-macro crates emit zero outgoing edges (clap_derive case). **Closed by milestone 087.**
```

Mirrors the existing `#172 — ... **Closed by milestone 087.**` annotation pattern.

## Recipe 4 — Pre-PR gate + PR open

```bash
./scripts/pre-pr.sh
# Expected: zero clippy warnings, every test suite reports `0 failed`.
# Expected: zero goldens regenerate. Confirm with:
git status --short mikebom-cli/tests/fixtures/golden/
# Expected: empty output.

# Open the PR with `Closes #173` in the body:
gh pr create --base main --title "fix(088): pin cargo proc-macro outgoing edges (closes #173)" \
    --body "Closes #173. ..."
```

After merge, GitHub auto-closes issue #173. Optional follow-up: post a closure comment on #173 linking to PR #180 (milestone 087 root-cause fix) + this milestone's PR.

## When in doubt

- **Recipe 1's jq query returns fewer than 4 entries**: the post-087 behavior has regressed. Investigate what changed in `mikebom-cli/src/scan_fs/package_db/cargo.rs::parse_lockfile` or `mikebom-cli/src/scan_fs/mod.rs` since alpha.26.
- **Recipe 2's test fails on `transitive_edges_match_baseline`**: the edge-count drifted. Either (a) the post-087 behavior regressed (investigate per above), or (b) a future fixture refresh changed the lockfile and the count needs re-baselining per quickstart Recipe 3 of `specs/087-fix-cargo-workspace-version/`.
- **Goldens regenerate when running pre-PR gate**: scope creep. This milestone is verify-only — if the cargo goldens regenerate, the test changes are touching production code. Narrow the diff back to test-only.
