# Quickstart: Milestone 160 (Go transitive-edge coverage)

**Date**: 2026-07-04
**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

Contributor onboarding for milestone 160. Assumes a working mikebom dev environment (per top-level `CLAUDE.md`).

## 1. Prerequisites

- Rust stable toolchain (workspace-managed).
- The `go` binary on `$PATH` (for T014–T016 empirical investigation AND for the SC-001 audit test).
- Milestone-090 fixture cache populated: `MIKEBOM_FIXTURES_DIR=~/.cache/mikebom/fixtures/<pinned-sha>/` (auto-populated on first test-run).

Verify:

```bash
go version                                    # expect: go1.21+
cargo +stable --version                       # expect: cargo 1.75+
ls -la "$MIKEBOM_FIXTURES_DIR"/transitive_parity/golang/  # expect: test-podman/ subdir OR fixture-cache init happens on first test
```

## 2. Investigation loop (FR-006 root causes)

The core of milestone 160 is FR-006's empirical investigation. Iterate on this loop:

```bash
# 1. Baseline: scan test-podman with current mikebom, capture the emitted CDX
cargo build --release --bin mikebom
./target/release/mikebom sbom scan \
    --path "$MIKEBOM_FIXTURES_DIR/transitive_parity/golang/test-podman" \
    --format cyclonedx-json \
    --output cyclonedx-json=/tmp/mikebom-test-podman.cdx.json

# 2. Compute ground truth via go mod graph
(cd "$MIKEBOM_FIXTURES_DIR/transitive_parity/golang/test-podman" && go mod graph) \
    > /tmp/go-mod-graph.txt

# 3. Diff: which edges are missing?
python3 scripts/audit/diff_transitive_edges.py \
    --sbom /tmp/mikebom-test-podman.cdx.json \
    --gomodgraph /tmp/go-mod-graph.txt \
    --show-missing 20

# 4. Hypothesize the root cause (FR-006a/b/c). Instrument the parser via
#    RUST_LOG=debug + tracing::debug! insertions in proxy_fetch.rs.
RUST_LOG=mikebom_cli::scan_fs::package_db::golang=debug \
    ./target/release/mikebom sbom scan --path ... 2>&1 \
    | grep 'go transitive edges' | head -20

# 5. Land the fix. Re-run steps 1-3. Verify improvement.
```

The 5 SC-002 spot-check edges to fix:

```text
github.com/containernetworking/plugins@v1.9.1 → alexflint/go-filemutex@v1.3.0
github.com/containernetworking/plugins@v1.9.1 → buger/jsonparser@v1.1.1
github.com/containernetworking/plugins@v1.9.1 → Microsoft/hcsshim@v0.13.0     (maybe legitimately platform-filtered)
github.com/containernetworking/plugins@v1.9.1 → coreos/go-iptables@v0.8.0     (maybe legitimately platform-filtered)
github.com/containernetworking/plugins@v1.9.1 → containerd/cgroups/v3@v3.0.3  (maybe legitimately indirect)
```

The two cross-platform ones (`alexflint/go-filemutex`, `buger/jsonparser`) MUST appear post-fix.

## 3. Per-component annotation implementation

Emission code goes in `mikebom-cli/src/scan_fs/package_db/golang/legacy.rs::read()`, per R2. Anchor at the existing `extra_annotations` insertion loop near `legacy.rs:1723`:

```rust
// Milestone 160: per-component transitive-source annotation
if let Some(entry) = module_graph.entry(&module_id) {
    pkg_entry.extra_annotations.insert(
        "mikebom:go-transitive-source".to_string(),
        serde_json::Value::String(entry.source.as_wire_str().to_string()),
    );
    if entry.source == ResolutionStep::None {
        // FR-003: emit unresolved-reason when source is None
        if let Some(reason_class) = unresolved_reasons.get(&module_id) {
            pkg_entry.extra_annotations.insert(
                "mikebom:go-transitive-unresolved-reason".to_string(),
                serde_json::Value::String(reason_class.as_wire_str().to_string()),
            );
        }
    }
}
```

The `unresolved_reasons: HashMap<ModuleId, UnresolvedReasonClass>` is populated during ladder step execution — proxy_fetch failures are classified via `UnresolvedReasonClass::from(&step_error)`.

## 4. Document-scope annotation implementation

Emission goes in `mikebom-cli/src/cli/scan_cmd.rs` near line 2585 (the existing `go_graph_completeness` emission block). Add a sibling block:

```rust
// Milestone 160: doc-scope go-transitive-coverage annotation
if go_component_count > 0 {
    if let Some(coverage) = &diagnostics.go_transitive_coverage {
        add_document_property(
            "mikebom:go-transitive-coverage",
            coverage.value_wire_str(),
        );
        if let Some(reason) = coverage.reason() {
            add_document_property(
                "mikebom:go-transitive-coverage-reason",
                reason,
            );
        }
    }
}
```

`compute_coverage(&summary, &ctx)` from `graph_resolver.rs` is invoked by `GraphResolver::resolve()` and stored into `ScanDiagnostics.go_transitive_coverage`.

## 5. Parity catalog registration

Update 4 files in one atomic commit:

- `mikebom-cli/src/parity/extractors/cdx.rs` — add C108/C109/C110/C111 `cdx_anno!` invocations.
- `mikebom-cli/src/parity/extractors/spdx2.rs` — add C108/C109/C110/C111 `spdx23_anno!` invocations.
- `mikebom-cli/src/parity/extractors/spdx3.rs` — add C108/C109/C110/C111 `spdx3_anno!` invocations.
- `mikebom-cli/src/parity/extractors/mod.rs` — add 4 `ParityExtractor` registration entries.

Exact syntax per `contracts/annotations.md` §Parity catalog integration.

## 6. Golden regeneration

The milestone-090 `golang` fixture SBOM changes; the other 10 ecosystems stay byte-identical (SC-003).

```bash
# Regenerate ONLY the golang fixture goldens
MIKEBOM_UPDATE_GOLDENS=golang cargo +stable test --workspace --no-fail-fast

# Verify SC-003: diff every non-golang fixture golden vs pre-160 HEAD
for eco in apk bazel cargo cmake deb gem maven npm pip rpm; do
    for fmt in cdx.json spdx.json spdx3.jsonld; do
        git diff HEAD -- "mikebom-cli/tests/fixtures/goldens/$eco/scan.$fmt" \
            | wc -l
    done
done
# Expect: 0 diff bytes on all 30 files.
```

## 7. Test the fix

```bash
# Full pre-PR gate
./scripts/pre-pr.sh

# SC-001 audit (gated behind env var to avoid running on every CI)
MIKEBOM_TRANSITIVE_COVERAGE_AUDIT=1 \
    cargo +stable test --workspace --no-fail-fast \
    --test go_transitive_coverage_audit

# Expected: PASS with edge-coverage ratio >= 0.90
```

## 8. Debugging: tracing recipes

```bash
# See per-module ladder-step attribution
RUST_LOG=mikebom_cli::scan_fs::package_db::golang::graph_resolver=info \
    ./target/release/mikebom sbom scan --path <fixture> 2>&1 \
    | grep 'go transitive edges'

# See per-fetch failure classification
RUST_LOG=mikebom_cli::scan_fs::package_db::golang::proxy_fetch=debug \
    ./target/release/mikebom sbom scan --path <fixture> 2>&1 \
    | grep 'proxy_fetch'
```

## 9. Common pitfalls

- **Forgetting to add `go_mod_graph_degraded` field to `LadderSummary`**: T014-T016 debugging depends on this signal to differentiate step-1-failed from step-1-succeeded-with-partial-output.
- **Emitting C110 on Go-free scans**: violates SC-003. Guard emission with `if go_component_count > 0`.
- **Emitting C109 without C108 == `unresolved`**: violates C109's conditional contract. Guard with `if entry.source == ResolutionStep::None`.
- **Not exercising the go.sum fallback in offline mode**: FR-006c says the offline early-skip applies to STEP 3 (proxy-fetch) only, NOT step 5 (go.sum fallback). Verify the ladder still descends through step 5 in offline mode.

## 10. Verify SC-002 spot-checks

Post-fix, the 2 cross-platform missing edges from `containernetworking/plugins@v1.9.1` MUST appear in the emitted CDX:

```bash
jq '.dependencies[]
    | select(.ref | contains("containernetworking-plugins-v1.9.1"))
    | .dependsOn[]' \
    /tmp/mikebom-test-podman.cdx.json \
    | grep -E 'alexflint-go-filemutex|buger-jsonparser'
# Expected: 2 lines (or 2 URN-shaped refs pointing to those PURLs)
```
