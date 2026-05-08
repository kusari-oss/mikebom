# Data Model — milestone 083 Transitive dep correctness audit

The milestone introduces ZERO new production Rust types — it's a test-infrastructure milestone. The "data model" here is the test-harness types in `mikebom-cli/tests/transitive_parity_common.rs`.

## Test-harness types (NEW — `mikebom-cli/tests/transitive_parity_common.rs`)

### `Edge` newtype

```rust
/// A directed dependency edge between two PURL-identified components.
/// Cross-tool comparison key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    pub from: String,    // PURL: e.g., "pkg:cargo/serde@1.0.193"
    pub to: String,      // PURL: e.g., "pkg:cargo/serde_derive@1.0.193"
}
```

PURL-normalized: each tool may emit different SPDX-IDs / bom-refs, but PURLs are the lowest-common-denominator identity for cross-tool comparison.

### `EdgeDiff` set-theoretic comparison

```rust
#[derive(Debug, Clone)]
pub struct EdgeDiff {
    pub agreement: HashSet<Edge>,         // Edges all 3 SBOM tools emit
    pub mikebom_only: HashSet<Edge>,      // Edges only mikebom emits
    pub trivy_only: HashSet<Edge>,        // Edges only trivy emits
    pub syft_only: HashSet<Edge>,         // Edges only syft emits
    pub mikebom_trivy: HashSet<Edge>,     // mikebom + trivy but not syft
    pub mikebom_syft: HashSet<Edge>,      // mikebom + syft but not trivy
    pub trivy_syft: HashSet<Edge>,        // trivy + syft but not mikebom
}

impl EdgeDiff {
    pub fn is_unanimous(&self) -> bool {
        self.mikebom_only.is_empty()
            && self.trivy_only.is_empty()
            && self.syft_only.is_empty()
            && self.mikebom_trivy.is_empty()
            && self.mikebom_syft.is_empty()
            && self.trivy_syft.is_empty()
    }

    /// When the 3 SBOM tools disagree, the source-format tiebreaker resolves
    /// each disputed edge.
    pub fn requires_tiebreaker(&self) -> bool {
        !self.is_unanimous()
    }
}
```

### `AuditRow` per-ecosystem audit findings

```rust
#[derive(Debug)]
pub struct AuditRow {
    pub ecosystem: &'static str,                      // "go" | "cargo" | "npm" | ...
    pub fixture_path: PathBuf,
    pub fixture_source_url: &'static str,             // Where the manifest was extracted from
    pub fixture_commit_sha: &'static str,             // 40-char hex
    pub mikebom_count: usize,
    pub trivy_count: usize,
    pub syft_count: usize,
    pub source_truth_count: Option<usize>,            // Only populated when tiebreaker invoked
    pub diff: EdgeDiff,
    pub classification: AuditClassification,
    pub follow_up_issue: Option<u32>,                 // GitHub issue # when gap surfaced
}

#[derive(Debug, Copy, Clone)]
pub enum AuditClassification {
    MatchesExpected,                                  // Unanimous agreement among 3 tools
    MinorDifferences,                                 // <5% per-edge divergence; documented in regression test
    GapSurfaced,                                      // Material divergence; follow-up issue filed
    NotApplicable,                                    // Tool support N/A (e.g., syft for some OS package cases)
}
```

### `TransitiveParityFixture` per-ecosystem fixture descriptor

```rust
#[derive(Debug)]
pub struct TransitiveParityFixture {
    pub ecosystem: &'static str,
    pub fixture_path: &'static str,                   // "tests/fixtures/transitive_parity/<ecosystem>/"
    pub expected_edge_count: usize,                   // Pinned at alpha.23 baseline
    pub expected_representative_edges: &'static [(&'static str, &'static str)],
    /// Required external tools per Q2 + research §4. Test gracefully skips when missing
    /// unless `MIKEBOM_REQUIRE_TRANSITIVE_PARITY=1` is set.
    pub required_tools: &'static [&'static str],
}
```

## Validation rules

- **VR-083-001**: Every per-ecosystem regression test (`transitive_parity_<ecosystem>.rs`) MUST pin `EXPECTED_EDGE_COUNT` + `EXPECTED_REPRESENTATIVE_EDGES` at alpha.23 baseline. The expected edge count is established by running mikebom against the fixture during T-task execution + recording the count.
- **VR-083-002**: Each `AuditRow` MUST have a `classification` from the 4-variant enum. `GapSurfaced` rows MUST have `follow_up_issue: Some(N)`.
- **VR-083-003**: When the `EdgeDiff` is non-unanimous, the test MUST invoke the source-format tiebreaker (per Q2) and record `source_truth_count`. When unanimous, tiebreaker is skipped and `source_truth_count: None`.
- **VR-083-004**: PURL normalization for cross-tool comparison: lowercase package types (`pkg:cargo/...` not `pkg:Cargo/...`); strip qualifiers when comparing if a tool doesn't emit them; alphabetical-sort `Vec<Edge>` for deterministic test output.
- **VR-083-005**: All milestone-082 byte-identity goldens (CDX 1.6 + SPDX 2.3 + SPDX 3) stay byte-identical pre/post merge. Audit doesn't change emission shape.
- **VR-083-006**: Pre-PR gate stays clean: clippy zero warnings; cargo test workspace `0 failed`; `cdx_regression`/`spdx_regression`/`spdx3_regression` pass without `MIKEBOM_UPDATE_*_GOLDENS` env vars.

## Backward compatibility

- **No production code changes**: `mikebom-cli/src/` untouched. Per FR-010, fixes for any gap surfaced ship as separate per-ecosystem follow-up milestones.
- **No new Cargo dependencies**: `toml` + `serde_json` already in workspace deps.
- **No CI workflow changes beyond tool installs**: trivy + syft installs added to the linux-x86_64 lane; macOS lane unchanged.
- **Existing test infrastructure preserved**: graceful-skip pattern matches milestone-078's; new env var `MIKEBOM_REQUIRE_TRANSITIVE_PARITY` parallels milestone-078's `MIKEBOM_REQUIRE_SPDX3_VALIDATOR`.
- **No goldens regenerated**: this is a test-infrastructure milestone; emission code paths are observed, not modified.
