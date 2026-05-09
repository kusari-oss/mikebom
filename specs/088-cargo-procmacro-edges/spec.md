# Feature Specification: Verify-and-close cargo proc-macro outgoing dep edges (closes #173)

**Feature Branch**: `088-cargo-procmacro-edges`
**Created**: 2026-05-08
**Status**: Draft
**Input**: User description: "173"

## Background

Issue #173 was filed during milestone 083's transitive-parity audit alongside #172. Both surfaced gaps in the cargo reader against the clap-rs/clap @ v4.5.21 audit fixture:

- **#172** — workspace-member version mismatch: `clap@4.5.21 → clap_builder@4.5.9` (wrong version) instead of `→ 4.5.21`. **Closed by milestone 087.**
- **#173** — proc-macro crates emit zero outgoing edges: `clap_derive@4.5.18` (a workspace-member proc-macro crate) had `dependencies = ["heck", "proc-macro2", "quote", "syn"]` in `Cargo.lock`, but mikebom emitted **zero** outgoing `DEPENDS_ON` edges from it. trivy + syft both correctly emitted those edges.

Post-milestone-087 verification reveals that #173 is **also closed** by the same fix: the root cause for both was `parse_lockfile`'s over-zealous `pkg.source.is_none()` skip, which dropped workspace MEMBERS (not just the workspace root) from the component set. Without `clap_derive@4.5.18` in `db_entries`, no PURL existed to attach its outgoing depends list to. Removing the skip in milestone 087 caused workspace members to be emitted, and their depends lists (correctly version-preserved per the depends-parser fix) now resolve to in-scope component PURLs.

This milestone is therefore a **verify-and-close** track, not a substantive code change. Scope: regression-test pinning of the now-correct outgoing edges + audit-row update + issue closure note.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Verify clap_derive emits 4 outgoing dep edges (Priority: P1)

A maintainer running mikebom against the clap-rs/clap audit fixture sees `clap_derive@4.5.18 → heck@0.5.0`, `→ proc-macro2@<v>`, `→ quote@<v>`, `→ syn@<v>` in the emitted SBOM, matching the lockfile + matching trivy + syft.

**Why this priority**: This is the only user-visible behavior change being claimed by this milestone. Without a regression test, future cargo-reader changes could silently regress the proc-macro outgoing-edge behavior again. Pinning the 4 edges by name in the milestone-083 transitive-parity test catches that regression class.

**Independent Test**: Run `cargo +stable test -p mikebom --test transitive_parity_cargo`. Assert that the 4 `clap_derive →` edges are present in `EXPECTED_REPRESENTATIVE_EDGES` and the test passes.

**Acceptance Scenarios**:

1. **Given** the clap-rs/clap audit fixture is unchanged from milestone 087 baseline, **When** the maintainer scans it via `mikebom sbom scan --format spdx-2.3-json`, **Then** the emitted document contains 4 `DEPENDS_ON` relationships from `pkg:cargo/clap_derive@4.5.18` to `heck`, `proc-macro2`, `quote`, `syn` PURLs (any version-compatible match — workspace lockfile resolves them to specific versions).
2. **Given** the milestone-083 regression test (`transitive_parity_cargo.rs`) is updated to encode the proc-macro invariant, **When** a future cargo-reader change accidentally drops proc-macro outgoing edges, **Then** the regression test fails with a "missing representative edge: pkg:cargo/clap_derive → pkg:cargo/<dep>" assertion.

---

### User Story 2 - Audit research doc reflects gap #2 closure (Priority: P2)

A maintainer reading `specs/083-transitive-correctness/research.md §8 — Ecosystem: cargo` finds gap #2 (proc-macro zero-outgoing-edges) marked as closed by milestone 087, with a one-line rationale ("same root cause as #172 — workspace-member skip removal made the source PURL available for outgoing-edge attachment").

**Why this priority**: The audit doc is the canonical record of what's-fixed-vs-open per ecosystem. Leaving gap #2 marked as open (or marked closed by the wrong milestone) creates confusion for future ecosystem-reader work. Lower than P1 because it's documentation, not user-observable behavior.

**Independent Test**: Read `specs/083-transitive-correctness/research.md`; confirm the cargo audit row shows zero open gaps for mikebom-side, with both #172 and #173 marked "Closed by milestone 087" (087 for #172, this milestone for #173 per attribution clarity).

**Acceptance Scenarios**:

1. **Given** the research.md cargo audit row pre-088, **When** the maintainer reads §8 — Ecosystem: cargo, **Then** gap #2 is marked closed (current state: gap #2 is marked open, "issue #173 — open").
2. **Given** the GitHub issue tracker, **When** the maintainer views issue #173, **Then** the issue is closed with a comment explaining the closure (linked to milestone 087's PR #180 and this milestone's PR).

---

### Edge Cases

- **Same-name proc-macro versions in the lockfile**: clap_derive is a single-version workspace member in this fixture, but the regression test should still resolve correctly if a future fixture refresh introduces multi-version. Mitigated by milestone-087's dual-key insert.
- **proc-macro crate with zero deps**: A future fixture might include a leaf proc-macro crate with no deps (e.g., a re-export macro). The fix shouldn't synthesize phantom edges — the depends list comes from the lockfile, which is empty for such crates.
- **Workspace root that IS a proc-macro crate**: Unusual but possible. Same emission path; no special-casing needed.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The transitive-parity regression test (`mikebom-cli/tests/transitive_parity_cargo.rs`) MUST pin at least one `pkg:cargo/clap_derive → pkg:cargo/<dep>` representative edge in `EXPECTED_REPRESENTATIVE_EDGES` so that future cargo-reader changes that regress the proc-macro outgoing-edge behavior fail loudly.
- **FR-002**: The milestone-083 audit research doc (`specs/083-transitive-correctness/research.md §8 — Ecosystem: cargo`) MUST mark gap #2 (proc-macro zero-outgoing-edges) as closed by milestone 087, mirroring the gap #1 closure already in place.
- **FR-003**: GitHub issue #173 MUST be closed with a closure comment that references milestone 087 (root-cause fix) and this milestone (regression test + audit-row update).
- **FR-004**: No code changes to `mikebom-cli/src/scan_fs/package_db/cargo.rs` or `mikebom-cli/src/scan_fs/mod.rs`. The behavior is already correct post-087; this milestone is verification + documentation only.
- **FR-005**: The pre-PR gate (`./scripts/pre-pr.sh`) MUST pass cleanly with the regression-test addition (zero clippy warnings, every test suite reports `0 failed`).
- **FR-006**: No goldens regenerate. The cargo regression test fixture (`tests/fixtures/cargo/lockfile-v3`) doesn't include a proc-macro crate, so the byte-identity goldens for cargo are unaffected. The audit fixture (`tests/fixtures/transitive_parity/cargo`) drives `transitive_parity_cargo.rs` which is a count + representative-edge test, not a byte-identity golden.

### Key Entities

- **Audit Fixture**: `mikebom-cli/tests/fixtures/transitive_parity/cargo/` — clap-rs/clap @ v4.5.21 (commit `2920fb082c987acb72ed1d1f47991c4d157e380d`). Manifest + lockfile only.
- **Proc-Macro Workspace Member**: `clap_derive@4.5.18` — workspace member with `[lib] proc-macro = true`, depends on `heck@0.5.0`, `proc-macro2@<v>`, `quote@<v>`, `syn@<v>`.
- **Regression Test**: `mikebom-cli/tests/transitive_parity_cargo.rs::transitive_edges_match_baseline` — baseline test that pins edge count + representative edges for the cargo audit fixture.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Operators of cargo SBOMs containing proc-macro workspace members see correct outgoing-edge graphs — `clap_derive` (or any proc-macro crate) emits its declared `Cargo.lock` deps as `DEPENDS_ON` relationships, not zero edges.
- **SC-002**: 100% of representative `clap_derive →` edges declared in `Cargo.lock` (4 edges: heck, proc-macro2, quote, syn) appear in mikebom's emitted SBOM for the audit fixture.
- **SC-003**: Future cargo-reader changes that regress proc-macro outgoing-edge emission fail the milestone-083 regression test within ≤5 seconds of running it (the test's existing wall-time bound).
- **SC-004**: GitHub issue #173 is closed within the same PR as the regression-test addition.
- **SC-005**: Zero new dependencies added to `Cargo.toml` and zero non-cargo goldens regenerate.

## Assumptions

- The audit fixture is unchanged from the milestone-083/087 baseline. If the clap-rs/clap fixture is bumped to a newer release in a future milestone, the representative edges may need re-derivation but the behavior under test (proc-macro outgoing edges emit non-zero) holds.
- Gap #2's closure is fully attributable to milestone 087's skip-removal — no additional code path was at fault. Verified by post-087 SBOM inspection: `clap_derive@4.5.18` emits exactly 4 outgoing `DEPENDS_ON` edges matching the lockfile.
- This milestone does NOT investigate proc-macro source-tree resolution (e.g., `[lib] proc-macro = true` parsing). The lockfile-driven dep extraction Just Works once the source PURL is in the component set.
- The pre-PR gate's existing 50+ test suites continue to pass without modification — this milestone only ADDS one or more representative edges to the existing parity test.

## Dependencies

- Depends on milestone 087 (PR #180) being merged into main (✅ done, commit `eebcae1`).
- Depends on alpha.26 release tag being pushed (✅ done, tag `v0.1.0-alpha.26` pushed; release.yml in flight).
- No new external dependencies (no Cargo.toml changes, no CI workflow changes).
