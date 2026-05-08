# Quickstart — milestone 083 maintainer recipes

Five maintainer-facing recipes for re-running the audit, picking new fixtures, bumping baselines, reading findings, and filing follow-up issues.

## Recipe 1 — Re-running the audit against the alpha.23 baseline

```bash
# Install required tools (skip if already installed)
brew install trivy syft   # or: apt-get install trivy / curl-install syft per CI workflow

# Run the audit (graceful-skip when tools absent — gates strictly under env var)
MIKEBOM_REQUIRE_TRANSITIVE_PARITY=1 cargo +stable test --test 'transitive_parity_*' 2>&1 | grep "test result"
```

Expected output: 11 test files × ~2-3 tests each = ~22-33 test results, all `0 failed`. Each per-ecosystem test reports its `AuditRow` to stdout.

## Recipe 2 — Picking a new fixture for an ecosystem

When upstream releases shift the existing fixture's baseline (e.g., kubernetes/cri-tools tags a new release with a substantially different go.sum), pick a new fixture:

1. Identify a real-world repo for the ecosystem with ≥50 components AND ≥100 edges in its lockfile.
2. `git clone <repo>` to a scratch dir; `git checkout <tag>` to pin the commit SHA.
3. Copy ONLY the manifest + lockfile to `mikebom-cli/tests/fixtures/transitive_parity/<ecosystem>/` (NOT the source tree per Q1):
   - Cargo: `Cargo.toml` + `Cargo.lock`
   - npm: `package.json` + `package-lock.json`
   - Go: `go.mod` + `go.sum`
   - Maven: `pom.xml` + parent POMs needed for `<parent>` resolution
   - pip-poetry: `pyproject.toml` + `poetry.lock`
   - pip-pipfile: `Pipfile` + `Pipfile.lock`
   - pip-plain: `requirements.txt`
   - gem: `Gemfile` + `Gemfile.lock`
   - dpkg/rpm/apk: extract from container image's installed database
4. Update the fixture's `README.md` with the source URL + commit SHA.
5. Re-run mikebom against the fixture; record the new edge count + 10-20 representative edges.
6. Update `EXPECTED_EDGE_COUNT` + `EXPECTED_REPRESENTATIVE_EDGES` in `mikebom-cli/tests/transitive_parity_<ecosystem>.rs`.
7. Update the per-ecosystem audit-record entry in `specs/083-transitive-correctness/research.md` (or in a new audit-row addendum if researching post-merge).

## Recipe 3 — Bumping the expected baseline after a deliberate edge-emission change

When a future milestone deliberately changes mikebom's per-ecosystem reader's edge-emission logic (e.g., adds a new transitive resolver step, fixes an edge-emission bug), update the baseline:

1. Run the test; observe the failure showing the actual-vs-expected edge-count mismatch.
2. Verify the change is INTENDED (not a regression): cross-check by running trivy + syft against the same fixture; confirm the new edge count is more correct OR matches peer-tool consensus better.
3. Update `EXPECTED_EDGE_COUNT` to the new value.
4. Update `EXPECTED_REPRESENTATIVE_EDGES` if the representative sample changed.
5. Document the bump in the milestone PR's commit message: `bump transitive_parity_<ecosystem> baseline from N to M (rationale)`.

## Recipe 4 — Reading the audit-record findings

Per-ecosystem rows live in `specs/083-transitive-correctness/research.md` under `### Ecosystem: <name>`. Each row has:

- **Fixture** — path under `mikebom-cli/tests/fixtures/transitive_parity/`.
- **Source URL + commit SHA** — where the manifest was extracted from. Re-clone if you want to re-run trivy/syft against the full source tree.
- **Tool versions** — pinned trivy + syft versions used for the audit baseline.
- **Edge counts** — mikebom / trivy / syft / source-format-direct (when tiebreaker invoked).
- **Diff classification** — `matches expected` (unanimous agreement) | `minor differences` (<5% per-edge divergence) | `gap surfaced` (material divergence; follow-up issue filed).
- **Tiebreaker resolution** (when invoked) — which tool was correct per the source-format direct read.
- **Indirect-vs-direct decision** — implement / document-as-divergence / deferred / N/A.
- **Follow-up disposition** — link to GitHub issue if `gap surfaced`.

To answer "what's mikebom's edge accuracy for ecosystem X?": find the per-ecosystem row, read the `diff classification` and `follow-up disposition` columns. Operators evaluating mikebom's per-ecosystem maturity for procurement use this directly.

## Recipe 5 — Filing a follow-up issue for a surfaced gap

When the audit finds a gap (mikebom emits edges trivy/syft/source-truth disagree with), file a per-ecosystem GitHub issue with:

```markdown
## Background

Milestone 083's transitive-edge audit found a gap in mikebom's <ecosystem> reader's edge emission against the alpha.23 baseline.

**Fixture**: tests/fixtures/transitive_parity/<ecosystem>/
**Source URL**: <repo>
**Commit SHA**: <sha>

## Per-tool edge counts

- mikebom: N edges
- trivy: M edges
- syft: K edges
- Source-format direct read (tiebreaker): T edges (correct)

## Specific missing edges

- `pkg:<ecosystem>/<from>@<ver>` → `pkg:<ecosystem>/<to>@<ver>`
- (10 representative edges)

## Specific extra (false-positive) edges

- (similar list)

## Suggested fix shape

[per-ecosystem reader change description]

## Scope estimate

Person-days: ~N

## Reproduction

\`\`\`bash
cargo +stable test --test transitive_parity_<ecosystem>
\`\`\`

## Related

- Milestone 083: `specs/083-transitive-correctness/`
- Audit research: research.md §<N>
```

The issue closes when a follow-up milestone ships the per-ecosystem fix; the regression test's `EXPECTED_EDGE_COUNT` bumps as part of that milestone.

## When in doubt

- **Tool not installed** (e.g., trivy/syft missing locally): tests skip with a clear diagnostic. Set `MIKEBOM_REQUIRE_TRANSITIVE_PARITY=1` to force-fail when tools missing (CI's strict mode).
- **OS-package tests on macOS**: skip unconditionally (dpkg/rpm/apk don't exist on macOS). Linux CI runs them.
- **Fixture went stale** (upstream released new tag): use Recipe 2 to refresh.
- **A test fails after a fresh `git pull`**: run Recipe 1 to verify; if the failure is a baseline-bump, follow Recipe 3.
