# Realistic-project transitive-edge baselines (milestone 055 SC-003)

This file pins the `pkg:golang` transitive-edge counts that the
realistic-project CI gate (`.github/workflows/realistic-projects.yml`)
asserts as a regression backstop.

The thresholds intentionally sit BELOW the measured value with a
generous safety margin — the goal is to catch large regressions
(e.g., a refactor that drops the resolver's step 3 path) rather than
to track minor fluctuations in upstream go.sum churn.

## Methodology

For each entry below:

1. Clone the project at the pinned tag.
2. Build mikebom from `main` HEAD post-055.
3. Run `mikebom sbom scan --path <project> --format cyclonedx-json --output ... --no-deep-hash` (no `--offline`, no GOMODCACHE override — let the resolver's step 1 + step 3 supply edges).
4. Count `pkg:golang → pkg:golang` `dependsOn` edges via:

   ```bash
   jq '[.dependencies[] | select(.ref | startswith("pkg:golang/")) | (.dependsOn // [])[] | select(startswith("pkg:golang/"))] | length' <output>.cdx.json
   ```

5. Set the gate floor to ≈ 50–80 % of the measured value to absorb upstream-driven variance (a future maintenance bump of one of the project's deps may add or remove a handful of transitive modules).

## Baselines

### `knative-func @ knative-v1.22.0`

- **Measured (2026-05-02, mikebom @ post-055 main)**: TBD on first CI run — the gate floor is pre-set at 200 edges based on the milestone 055 spec SC-003 estimate; the first green CI run records the actual measured value here.
- **Floor in CI**: 200 edges
- **Platform**: ubuntu-latest (Go pre-installed). macos-latest also runs the gate; same floor (the measured value should be platform-independent because the resolver's output is content-determined, not perf-determined — only timing varies across platforms).
- **Notes**: knative/func has ~950 modules in its top-level `go.sum`. Not every module declares requires that resolve to other go.sum-set modules; the FR-003 intersection drops dangling targets. A measured value of ~300–500 is typical for projects of this size.

### Future entries

When milestone 109 expands the realistic-project matrix to per-ecosystem coverage (cargo / npm / maven / pip / gem), the cargo/maven entries will get analogous `expected_min_<ecosystem>_edges` fields here. The Go entry above is the template.

## Updating the floor

The floor MUST be updated alongside any tag bump for the project:

1. Bump `tag:` in `.github/workflows/realistic-projects.yml`.
2. Re-measure on a local clone of the new tag (steps 1–4 above).
3. Update `expected_min_pkg_golang_edges:` to ≈ 50–80 % of the new measured value.
4. Update this file's "Measured" line with the new value + date.
5. Land the workflow change + this file's update in a single PR so the floor and the test never drift.

This mirrors milestone 054's `expected_min_components` discipline.
