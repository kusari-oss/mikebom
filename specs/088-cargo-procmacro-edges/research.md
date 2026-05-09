# Research — milestone 088 verify-and-close cargo proc-macro outgoing dep edges

This file captures the implementation decisions for the verify-and-close work. Plan-level decisions (deferred from spec.md per the `/speckit.clarify` coverage report) are resolved here.

## §1 — How many representative edges to pin

**Decision**: pin **all 4** of `clap_derive@4.5.18`'s outgoing edges in `EXPECTED_REPRESENTATIVE_EDGES`:

- `pkg:cargo/clap_derive → pkg:cargo/heck`
- `pkg:cargo/clap_derive → pkg:cargo/proc-macro2`
- `pkg:cargo/clap_derive → pkg:cargo/quote`
- `pkg:cargo/clap_derive → pkg:cargo/syn`

**Rationale**: A future cargo-reader change that drops proc-macro outgoing edges might drop a SUBSET (e.g., a refactor that filters `proc-macro2` deps as "build-only" but keeps regular runtime deps). Pinning only one edge could miss subset regressions. Pinning all 4 is cheap (4 lines of code) and forces the regression to be visible regardless of which dep the change drops.

**Alternatives considered**:
- Pin only 1 edge (`heck`, the simplest non-proc-macro dep): cheap but misses subset regressions.
- Pin all 4 + add a count-only assertion (`assert_eq!(clap_derive_outgoing.len(), 4)`): more thorough but adds new test infrastructure for low marginal value.

The chosen approach reuses the existing `EXPECTED_REPRESENTATIVE_EDGES` mechanism (PURL-prefix matching that survives version bumps). No new test infrastructure needed.

## §2 — Whether to add a unit test in `cargo.rs`

**Decision**: **no unit test added**. The integration test in `transitive_parity_cargo.rs` is sufficient.

**Rationale**: The behavior under test (proc-macro workspace member → its declared deps emit as outgoing edges) is an end-to-end behavior that depends on the interaction of `parse_lockfile`, `package_to_entry`, the `name_to_purl` build loop, and the edge-emission loop in `scan_fs/mod.rs`. A unit test on any single function would not catch a regression in the wiring between them. The integration test catches that class of regression directly.

The milestone-087 unit tests (`package_to_entry_preserves_version_disambiguation` + `parse_lockfile_emits_workspace_root` + `parse_lockfile_emits_all_workspace_members`) already lock down the per-function behavior at the source layer. Layering a per-function test for proc-macro specifically would duplicate coverage without adding signal.

**Alternatives considered**:
- Add a `parse_lockfile_emits_proc_macro_outgoing_deps` unit test: redundant — `parse_lockfile_emits_all_workspace_members` already verifies that source=None entries (which all proc-macro workspace members are by definition) emit as `PackageDbEntry` with their full depends list.

## §3 — Audit research doc closure note phrasing

**Decision**: in `specs/083-transitive-correctness/research.md §8 — Ecosystem: cargo`, gap #2 currently reads:

> 2. **clap_derive emits zero outgoing edges**: mikebom emits no DEPENDS_ON entries from `clap_derive`, despite Cargo.lock showing it depends on `proc-macro2`, `quote`, `syn`. Trivy + syft both emit those edges. Suggests the cargo reader skips proc-macro crate dep extraction entirely for procedural-macro-typed Cargo.toml entries. (Issue #173 — open.)

Replace with:

> 2. ~~**clap_derive emits zero outgoing edges**: mikebom emits no DEPENDS_ON entries from `clap_derive`, despite Cargo.lock showing it depends on `heck`, `proc-macro2`, `quote`, `syn`.~~ **Closed by milestone 087** (issue #173). Same root cause as gap #1: `parse_lockfile`'s `pkg.source.is_none()` skip dropped workspace MEMBERS from the component set, so `clap_derive@4.5.18` (a workspace-member proc-macro crate) had no PURL to attach outgoing edges to. With the skip removed, mikebom now emits 4 outgoing `DEPENDS_ON` edges from clap_derive matching the lockfile + matching trivy + syft. Pinned in `transitive_parity_cargo.rs::EXPECTED_REPRESENTATIVE_EDGES`.

Rationale: the strikethrough preserves the original observation (audit history) while the post-hoc closure note attributes the fix correctly + identifies the test that locks down the behavior. Mirrors the gap #1 closure pattern already in place.

**Also update the "Filed follow-up issues" list** at the bottom of the cargo audit row from:

> - **#173** — cargo: proc-macro crates emit zero outgoing edges (clap_derive case)

to:

> - **#173** — cargo: proc-macro crates emit zero outgoing edges (clap_derive case). **Closed by milestone 087.**

Mirrors how #172 is annotated post-milestone-087.

## §4 — Whether to bump tool version in audit doc

**Decision**: do NOT bump `mikebom alpha.24` → `mikebom alpha.26` in the research-doc tool-version line.

**Rationale**: The tool-version line is the version that originally OBSERVED the audit, not the version that closed the gaps. Bumping it would mislead future readers into thinking the divergence numbers (319 / 85 / 721) were re-measured against alpha.26. Milestone 087's research doc edit already added "; post-milestone-087 numbers measured against alpha.26" as a parenthetical — this milestone leaves that wording intact.

## §5 — GitHub issue #173 closure mechanics

**Decision**: close issue #173 in the PR description via a `Closes #173` line. Add a closure comment after merge linking to:

- PR #180 (milestone 087 root-cause fix)
- This milestone's PR (regression test pin + audit doc update)
- The post-fix smoke-test output showing 4 emitted edges

**Rationale**: GitHub auto-closes the issue when a PR with `Closes #173` merges to main. The closure comment provides context for anyone arriving at the issue from a stale link (Slack, design docs, etc.).

## §6 — Edge representation in `EXPECTED_REPRESENTATIVE_EDGES`

**Decision**: use the same PURL-prefix format already in use (no `@version` suffix), so the test survives a future fixture refresh that bumps `clap_derive`'s pinned version:

```rust
("pkg:cargo/clap_derive", "pkg:cargo/heck"),
("pkg:cargo/clap_derive", "pkg:cargo/proc-macro2"),
("pkg:cargo/clap_derive", "pkg:cargo/quote"),
("pkg:cargo/clap_derive", "pkg:cargo/syn"),
```

Per the `strip_version` helper at `transitive_parity_cargo.rs:133-138`, the matcher strips the `@<version>` suffix from both endpoints before comparison. So as long as the workspace lockfile resolves clap_derive's deps to SOME version of heck/proc-macro2/quote/syn, the test passes.

**Rationale**: Identical convention to the 4 existing representative edges in milestone-087's baseline. Future fixture refreshes (e.g., bumping clap-rs/clap to v4.6.x) only break the test if the proc-macro outgoing-edge behavior actually regresses, not if version numbers shift.
