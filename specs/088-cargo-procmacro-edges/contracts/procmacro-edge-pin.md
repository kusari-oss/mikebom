# Contract — milestone 088 proc-macro outgoing-edge pin

The milestone's only contract. Documents the regression-test invariant + the audit-doc closure annotation + the issue-closure mechanic. No CLI surface, no Rust API surface — purely test + documentation.

## CLI surface

**No new operator-facing CLI flags.** This is a test-pinning + documentation milestone.

## Library surface (`mikebom-cli` crate)

**No new public Rust API.** No internal API changes either. The `transitive_parity_cargo.rs` integration test gains 4 entries in its `EXPECTED_REPRESENTATIVE_EDGES` constant; no production code paths touched.

## Regression-test invariant

For each cargo dep-tree audit run against `mikebom-cli/tests/fixtures/transitive_parity/cargo/` (clap-rs/clap @ v4.5.21), the emitted SPDX 2.3 document MUST contain a `DEPENDS_ON` relationship from `pkg:cargo/clap_derive@4.5.18` to EACH of:

- `pkg:cargo/heck@<v>`
- `pkg:cargo/proc-macro2@<v>`
- `pkg:cargo/quote@<v>`
- `pkg:cargo/syn@<v>`

(Versions per the workspace `Cargo.lock`; PURL-prefix matching survives fixture refreshes.)

In Rust:

```rust
const EXPECTED_REPRESENTATIVE_EDGES: &[(&str, &str)] = &[
    // ... existing 4 milestone-087 entries unchanged ...
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

This contract is enforced by VR-088-001 + VR-088-002 + VR-088-004.

## Audit-doc closure-annotation contract

`specs/083-transitive-correctness/research.md §8 — Ecosystem: cargo` MUST mark gap #2 closed by milestone 087, mirroring the gap #1 closure pattern. Both:

1. The gap body — strikethrough the original observation, append "**Closed by milestone 087**" with the root-cause one-liner.
2. The "Filed follow-up issues" footer — append ". **Closed by milestone 087.**" to the #173 row.

Both edits MUST cite milestone 087 as the closing milestone (not 088), since 087 is where the code change landed. Milestone 088 only adds the regression test.

This contract is enforced by VR-088-005 + VR-088-006.

## Issue-closure contract

The PR description MUST contain a `Closes #173` line (case-insensitive per GitHub linkage syntax). On merge to main, GitHub auto-closes issue #173.

The closure comment posted after merge SHOULD reference:
- PR #180 (milestone 087, root-cause fix)
- This milestone's PR (regression test pin + audit doc update)

This contract is enforced by VR-088-007 + VR-088-008.

## Per-format scope contract

| Format | Affected? | Verification |
|---|---|---|
| **CDX 1.6 cargo** | NO — no new components, no new edges, just a test pin | `cargo.cdx.json` golden byte-identical |
| **SPDX 2.3 cargo** | NO — same | `cargo.spdx.json` golden byte-identical |
| **SPDX 3 cargo** | NO — same | `cargo.spdx3.json` golden byte-identical |
| **Other ecosystems' goldens** | NO — no cargo-reader changes | All 27 goldens byte-identical |

This is a verify-only milestone. ZERO golden regenerations. If goldens regenerate, scope has crept and the PR must be narrowed.

## Test invocation contract

```bash
# Confirm the cargo regression test passes with the new representative edges:
cargo +stable test -p mikebom --test transitive_parity_cargo
# Expected: 4 tests pass, including transitive_edges_match_baseline
# (with the 4 new clap_derive → entries verified).

# Confirm zero goldens drift:
git status --short mikebom-cli/tests/fixtures/golden/
# Expected: empty output (no modified files).

# Standard pre-PR gate:
./scripts/pre-pr.sh
# Expected: zero clippy warnings, every test suite reports `0 failed`.
```

## Performance contract

- Emission wall-time: byte-identical to milestone-087 baseline (no code path changes).
- Test wall-time: existing parity test runs in <5s; adding 4 representative-edge entries is O(4) HashMap lookups in the post-extraction set, no measurable delta.
- No goldens regen wall-time (zero goldens regenerate).

## Backward-compatibility contract

- Operators of mikebom-emitted CDX/SPDX 2.3/SPDX 3 documents see ZERO behavior change post-088. This milestone makes the post-087 behavior REGRESSION-PROOF, not different.
- Pre-087 operators (alpha.25 and earlier) saw the wrong-version edge + zero clap_derive outgoing edges. Milestone 087 fixed both. Milestone 088 locks down the proc-macro half via test.
