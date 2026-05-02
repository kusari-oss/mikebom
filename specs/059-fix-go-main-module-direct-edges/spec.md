# Feature Specification: Fix Go main-module dependency graph topology — direct-only edges from root

**Feature Branch**: `059-fix-go-main-module-direct-edges`
**Created**: 2026-05-02
**Status**: Draft
**Input**: User description: graph correctness fix per #113 reviewer feedback ("the SBOM doesn't care it's indirect because if I have the right relationships it's `A → B → C` and so B is dependency of A and C is dependency of B"). Reverses milestone 053 FR-002's deliberate "include `// indirect` in main-module edges" choice. Closes the gap that motivated #113 by making the graph itself express direct-vs-indirect natively (no annotation needed). Accepts Trivy's trade-off: components reachable only via transitive paths are orphans when the milestone 055 resolver can't supply transitive edges (offline + empty cache + no proxy).

## Clarifications

### Session 2026-05-02

- Q: What does "direct" mean for the main-module's outgoing edges? → A: **Only requires NOT marked `// indirect`** in the workspace `go.mod`. The `// indirect` marker is Go's own canonical signal that the dependency was pulled in transitively by some other dep and surfaced into go.mod by `go mod tidy` purely for go.sum reproducibility. Treating those as direct edges from main-module is a topology lie.
- Q: What happens to components that the workspace `go.mod` records as `// indirect` AND that the milestone 055 resolver fails to supply transitive edges for (offline + empty cache + GOPROXY=off case)? → A: **They become orphans in the graph** — present in `components[]` (sourced from `go.sum`) but not reachable from main-module via any path. This matches Trivy's behavior. The trade-off is principled: a component reached via no edge in the graph is more honestly represented than one reached via a fake direct edge.
- Q: Should we keep the `mikebom:dependency-kind` annotation work from PR #117? → A: **No.** Closed without merging. Once the graph topology is correct, the annotation becomes redundant — consumers walk from main-module to find directs, traverse one more hop for indirects. Re-encoding graph topology as a property violates Constitution Principle V's "use native fields" discipline. Catalog row C43 is NOT introduced.
- Q: Is milestone 053's FR-002 being reversed? → A: **Yes, deliberately.** 053's FR-002 said "Direct edges cover every `require` in `go.mod`, after `replace`/`exclude` application, INCLUDING `// indirect` requires." This milestone reverts the "INCLUDING `// indirect`" portion. The 053 spec doc gets a follow-up cross-reference noting the supersession.
- Q: Should we emit a tracing breadcrumb when the graph is incomplete (orphans exist)? → A: **Yes.** End-of-scan `tracing::info` line of the form `Go graph: M of N go.sum components reachable from main-module via dependsOn edges. P orphans (no incoming edge — typical when --offline + empty cache + indirect-only requires).` Operator-friendly visibility.

## Investigation findings

The actual SBOM output for `tests/fixtures/go/simple-module/` post-053+054+055+056+057 (current main, alpha.10):

```
example.com/simple → davecgh/go-spew, mousetrap, mattn/go-isatty,
                     pmezard/go-difflib, sirupsen/logrus, spf13/cobra,
                     spf13/pflag, stretchr/testify, golang.org/x/sys,
                     gopkg.in/yaml.v3
```

That's all 10 go.sum modules. The workspace `go.mod` declares only 5 as direct (`mattn/go-isatty`, `sirupsen/logrus`, `spf13/cobra`, `stretchr/testify`, `gopkg.in/yaml.v3`); the other 5 are `// indirect`. Per milestone 053's deliberate choice, all 10 are emitted as direct edges from main-module. Consumers of the SBOM cannot tell from the graph alone that `davecgh/go-spew` is reached via testify, not directly.

Every transitive component in the same golden has `dependsOn: []`:

```
spf13/cobra → []
testify → []
sirupsen/logrus → []
```

That's because the milestone 055 resolver runs in `--offline` + scrubbed-`$GOMODCACHE` mode for the goldens, which lands at step 4 (no-edges fallthrough) for every module. With cache or network, those edges populate correctly.

Combining the over-eager root + under-eager transitive edges produces a graph where:
- Every component looks "direct" from main-module.
- No transitive structure is visible.
- "What is the project actually using directly?" is unanswerable from the graph.

Reviewer feedback on PR #117: an annotation that re-encodes graph topology is the wrong layer — fix the graph instead.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Direct deps reachable in 1 hop from the SBOM root (Priority: P1)

A consumer of mikebom's Go SBOM wants to answer "what does this project directly import?" by walking the dependency graph from the main-module component. Today the answer is "every go.sum module, including ones tagged `// indirect`" — which is wrong. After 059, the answer is "only the modules the workspace `go.mod` declares as non-`// indirect` requires."

**Why this priority**: This is the core graph-correctness fix. Without it, the dependency graph in mikebom's SBOMs lies about the project's direct-vs-transitive relationships, and downstream tools (vulnerability triage, license audit, attack-surface analysis) draw wrong conclusions. With it, the graph IS the answer — no annotation overlay needed.

**Independent Test**: Construct a Go workspace fixture whose `go.mod` declares 2 direct requires + 3 `// indirect` requires, with all 5 modules in `go.sum`. Scan; assert the main-module component's `dependsOn` (across CDX `dependencies[]`, SPDX 2.3 `relationships[type=DEPENDS_ON]`, SPDX 3 `relationship[type=dependsOn]`) contains EXACTLY the 2 direct paths and NONE of the 3 indirect paths.

**Acceptance Scenarios**:

1. **Given** a workspace with `require ( github.com/foo/a v1.0.0; github.com/foo/b v1.0.0 // indirect )`, **When** mikebom scans, **Then** the main-module component's `dependsOn` contains `pkg:golang/github.com/foo/a@v1.0.0` and does NOT contain `pkg:golang/github.com/foo/b@v1.0.0`.
2. **Given** the same workspace, **When** mikebom scans with `$GOMODCACHE` populated such that the 055 resolver supplies transitive edges, **Then** the `b` component appears in `components[]` AND has at least one incoming `dependsOn` from some other component (e.g., `a → b` if a's go.mod requires b). The graph is FULLY connected.
3. **Given** the same workspace, **When** mikebom scans with `--offline` + empty cache + `GOPROXY=off` (the "trivy-style worst case"), **Then** the `b` component appears in `components[]` but has NO incoming `dependsOn` edges (orphan). The end-of-scan tracing summary names the orphan count.
4. **Given** the existing `tests/fixtures/go/simple-module/` golden (5 direct + 5 indirect requires), **When** the goldens regenerate post-059, **Then** the main-module's `dependsOn` lists exactly the 5 direct paths (mattn/go-isatty, sirupsen/logrus, spf13/cobra, stretchr/testify, gopkg.in/yaml.v3) and the 5 indirect paths (davecgh/go-spew, mousetrap, pmezard/go-difflib, spf13/pflag, golang.org/x/sys) become orphans in the offline + empty-cache golden generation context.

### User Story 2 — Operational visibility when orphans exist (Priority: P2)

A maintainer running mikebom in CI wants to know whether the graph they emitted is fully connected or has orphan components, so they can decide whether to populate `$GOMODCACHE` (or remove `--offline`) to get better resolution. Today they have to walk the graph themselves to find orphans.

**Why this priority**: Orphans are an expected outcome in `--offline` + empty cache mode (the cost of Option A), but they're invisible without explicit reporting. P2 because it's operational quality-of-life, not correctness.

**Independent Test**: Run the existing `simple-module` scan with `--offline` + empty cache; verify a `tracing::info` line of the form `Go graph: M of N go.sum components reachable from main-module via dependsOn edges. P orphans (...)` appears at end of scan with M ≤ 5, N == 10, P ≥ 5.

**Acceptance Scenarios**:

1. **Given** a Go scan that produced N components and M reachable-from-root, **When** the scan completes, **Then** a `tracing::info` line at scan end names the counts.
2. **Given** a fully-connected graph (M == N), **When** the scan completes, **Then** the same line emits with `0 orphans` so the operational signal is consistent.

### Edge Cases

- **Workspace with no `// indirect` requires** (everything is explicit): main-module's `dependsOn` matches the full `require` list. Pre-059 and post-059 behavior identical for these workspaces.
- **Workspace with `replace foo v1.0.0 => bar v2.0.0`** where `foo` is a direct require (no `// indirect`): per existing 053 behavior, `apply_replace_and_exclude` rewrites the edge to point at `bar v2.0.0`. The replace target inherits the source's direct status. `bar` becomes a direct edge from main-module.
- **Workspace `go.mod` with the `require` directive missing entirely** (just `module X` declaration): main-module emits with empty `dependsOn`. Already handled by the existing code path; no change.
- **Multi-project rootfs scan**: each project's main-module classifier runs independently against its own `go.mod`. A module that's `// indirect` in project A and direct in project B becomes a direct edge in B's main-module subgraph and is reached transitively in A's.
- **`go.mod` with the `// indirect` marker on a line in the FIRST `require` block** (mixed-style with both direct and indirect in the same parenthesized group, valid per the go.mod spec): the parser already records `indirect: bool` per individual `GoModRequire`; classifier sees the right flag.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `build_main_module_entry` (`mikebom-cli/src/scan_fs/package_db/golang/legacy.rs:657`) MUST emit `depends` containing only the workspace `go.mod`'s requires whose `GoModRequire.indirect == false`, after `replace`/`exclude` application via the existing `apply_replace_and_exclude` helper. The pre-059 behavior of including `// indirect` requires is REMOVED.

- **FR-002**: This MILESTONE supersedes milestone 053 FR-002's "INCLUDING `// indirect` requires" clause. The 053 spec gets a follow-up note pointing at 059. No behavioral change to the 053 main-module construction beyond the `dependsOn` filter.

- **FR-003**: Components in `go.sum` that are referenced ONLY by `// indirect` requires AND that the milestone 055 resolver cannot supply transitive edges for (offline + empty cache + no proxy fetch) become orphans in the graph — present in `components[]` but with no incoming `dependsOn`. This is the deliberate Trivy-style trade-off; the orphan is more honestly represented than a fake direct edge.

- **FR-004**: At end of scan, mikebom MUST emit a `tracing::info` line summarizing graph reachability: `Go graph (project=<workspace-name>): M of N go.sum components reachable from main-module via dependsOn edges. P orphans.` where N is the total go.sum module count, M is the count reachable via 1+ inbound edge, P = N - M. Operational visibility for the orphan condition.

- **FR-005**: Existing 27 byte-identity goldens MUST be regenerated. Specifically, the simple-module fixture's main-module `dependsOn` shrinks from 10 entries to 5 (the 5 direct requires from its `go.mod`); the 5 indirect-only modules (davecgh/go-spew, mousetrap, pmezard/go-difflib, spf13/pflag, golang.org/x/sys) become orphans. The argo-style-no-cache fixture sees a similar shrink based on its `go.mod`'s direct/indirect split.

- **FR-006**: Existing milestone 053 unit tests that assert on the over-eager behavior MUST be updated:
  - `build_main_module_entry_three_requires_produces_three_depends`: still passes if all 3 are non-indirect (verify the test's setup).
  - `build_main_module_entry_includes_indirect_requires`: this test is INVERTED — rename to `build_main_module_entry_excludes_indirect_requires` and assert the indirect entries are NOT in `depends`.
  - Other 053 tests: audit for assumptions about main-module's edge count.

- **FR-007**: Realistic-project CI gate (`.github/workflows/realistic-projects.yml`'s knative-func entry) MUST continue to pass. The `expected_min_pkg_golang_edges: 200` floor (from PR #115) was set against the pre-059 over-eager edge count; post-059 the count drops because main-module's outgoing edges shrink. **Re-measure on the first green CI run** and adjust the floor if needed (likely lowering it to 100–150 for knative/func given the project's `// indirect` ratio). Update the floor + the baseline doc (`specs/055-go-transitive-edges/realistic-project-baseline.md`) in the same PR if a re-measure is needed.

- **FR-008**: PR #117's catalog row C43 + extractors + `mikebom:dependency-kind` emission MUST NOT be re-introduced. The closed PR is the canonical "wrong layer" precedent; future contributors finding this milestone should understand the graph IS the answer.

- **FR-009**: Pre-PR gate (`./scripts/pre-pr.sh`) MUST pass.

### Key Entities

- **Direct require**: a `GoModRequire` from the workspace `go.mod`'s `require` block whose `indirect == false`. Becomes one outgoing edge from main-module's `depends`.
- **Indirect require**: a `GoModRequire` whose `indirect == true` — Go's `// indirect` marker. After 059: NOT a direct edge from main-module. Reached transitively via the milestone 055 resolver, or orphaned if the resolver can't supply edges.
- **Orphan**: a `go.sum` component with no incoming `dependsOn` edges in the emitted SBOM. Acceptable in `--offline` + empty cache + indirect-only situations; reported via the FR-004 tracing summary.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For the `tests/fixtures/go/simple-module/` golden, the main-module component's `dependsOn` contains EXACTLY 5 entries (the 5 non-indirect requires from its `go.mod`). Asserted via the regenerated CDX/SPDX 2.3/SPDX 3 goldens being byte-identical to a re-measurement.
- **SC-002**: Existing 053 unit tests covering main-module edge construction pass post-update (`build_main_module_entry_excludes_indirect_requires` is the renamed inversion).
- **SC-003**: The realistic-projects CI knative-func gate passes post-059 (with adjusted `expected_min_pkg_golang_edges` floor if the re-measure indicates the pre-059 floor is now too high).
- **SC-004**: Pre-PR gate passes.
- **SC-005**: A new unit test `main_module_emits_only_non_indirect_requires` synthesizes a 2-direct + 3-indirect `go.mod` and asserts the entry's `depends` contains exactly the 2 direct paths.
- **SC-006**: The FR-004 tracing summary line appears at end of scan with the right counts (M, N, P). Verified via `tracing-test`-style log capture in a unit test, OR via manual smoke check on a populated cache.

## Assumptions

- **`GoModRequire.indirect` is reliably set by the existing `parse_go_mod`**: verified by milestone 049/053/055 unit tests covering the `// indirect` marker parse path.
- **Trivy's orphan trade-off is acceptable for our user base**: Trivy ships this exact behavior in production today. Mikebom's existing user base (per the issue tracker discussions) hasn't pushed back on Trivy-style graphs.
- **No new crate.** The change is a single-line filter in `build_main_module_entry`'s existing iterator chain plus an end-of-scan tracing line. No dependency churn.
- **Out of scope**: per-ecosystem application of the same pattern (npm / cargo / maven / pip / gem main-modules). Those don't have main-module components today (#104 follow-up). When they do, the same direct-only filter applies — but that's a 059-extension milestone.
- **Out of scope**: improvements to the milestone 055 resolver to reduce orphan frequency (e.g., on-disk cache for fetched `.mod` files, deps.dev fallback). Tracked separately if/when real users hit a wall.
