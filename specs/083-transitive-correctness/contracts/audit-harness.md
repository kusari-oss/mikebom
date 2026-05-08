# Contract — milestone 083 audit harness

The milestone's only contract. Documents the audit-harness invocation patterns + edge-extraction algorithms + audit-row format.

## CLI surface

**No new operator-facing CLI flags.** This is a test-infrastructure milestone. The audit harness lives in `mikebom-cli/tests/transitive_parity_common.rs` and is invoked via `cargo test`, not via `mikebom <subcommand>`.

## Library surface (`mikebom-cli` crate)

**No new public Rust API.** All new types are test-only (in `tests/` directory):
- `tests::transitive_parity_common::Edge`
- `tests::transitive_parity_common::EdgeDiff`
- `tests::transitive_parity_common::AuditRow`
- `tests::transitive_parity_common::AuditClassification`
- `tests::transitive_parity_common::TransitiveParityFixture`

Public surface of `mikebom-cli` crate unchanged.

## Per-tool invocation contracts

### `run_mikebom(fixture_path)`

```bash
cargo run -p mikebom-cli -- sbom scan \
    --path <fixture> \
    --format spdx-3-json \
    --output - \
    --offline
```

Output: SPDX 3 JSON-LD on stdout; harness extracts `software_dependsOn[]` per `software_Package` element, normalizes to `Edge` tuples by PURL.

### `run_trivy(fixture_path)`

```bash
trivy fs --format spdx-json --output - <fixture>
```

Output: SPDX 2.3 JSON; harness extracts `relationships[]` filtered to `relationshipType: "DEPENDS_ON"`, resolves SPDX-IDs to PURLs via `packages[]` lookup, normalizes to `Edge` tuples.

Trivy supports SPDX 2.3 only as of 0.69.3; SPDX 3 output not available. The audit accepts the format mismatch (PURLs are the comparison key, not the wire format).

### `run_syft(fixture_path)`

```bash
syft <fixture> -o spdx-json
```

Output: SPDX 2.3 JSON; harness extracts edges per the same algorithm as trivy.

### `run_source_format_direct(fixture_path, ecosystem)` (tiebreaker per Q2)

Invoked **only when `EdgeDiff::requires_tiebreaker() == true`** (the 3 SBOM tools disagree). Per-ecosystem dispatch from research §4:

| Ecosystem | Tiebreaker invocation |
|---|---|
| Go | `go mod graph` (subprocess) when `go` on PATH |
| Cargo | `toml::from_str(Cargo.lock).package` Rust-native parse; iterate `dependencies = [...]` |
| npm | `serde_json::from_reader(package-lock.json).packages` Rust-native parse |
| Maven | `mvn dependency:tree -DoutputType=text` (subprocess) |
| pip-poetry | `toml::from_str(poetry.lock).package` Rust-native parse |
| pip-pipfile | `serde_json::from_reader(Pipfile.lock)` Rust-native parse |
| gem | Custom parser for `Gemfile.lock` GEM block |
| dpkg | `dpkg-query --show -f='${Package} ${Depends}\n'` (Linux-only subprocess) |
| rpm | `rpm -q --requires --all` (Linux-only subprocess) |
| apk | `apk info -R` (Linux-only subprocess) |

Output: `Vec<Edge>` from the source format directly.

## Edge extraction algorithm

```rust
fn extract_edges_spdx_2_3(sbom: &SpdxDocument) -> Vec<Edge> {
    sbom.relationships.iter()
        .filter(|r| r.relationship_type == "DEPENDS_ON")
        .filter_map(|r| {
            let from = sbom.packages.iter().find(|p| p.spdx_id == r.spdx_element_id)?;
            let to = sbom.packages.iter().find(|p| p.spdx_id == r.related_spdx_element)?;
            Some(Edge {
                from: from.external_refs.iter().find_map(|e| e.purl())?,
                to: to.external_refs.iter().find_map(|e| e.purl())?,
            })
        })
        .collect()
}

fn extract_edges_spdx_3(sbom: &Spdx3Document) -> Vec<Edge> {
    sbom.graph.iter()
        .filter_map(|element| {
            if element.r#type != "software_Package" { return None; }
            let from_purl = element.external_identifier.iter().find_map(|e| e.package_url())?;
            let edges = element.software_depends_on.iter().filter_map(|target_iri| {
                let target = sbom.graph.iter().find(|e| e.spdx_id == *target_iri)?;
                let to_purl = target.external_identifier.iter().find_map(|e| e.package_url())?;
                Some(Edge { from: from_purl.clone(), to: to_purl })
            });
            Some(edges)
        })
        .flatten()
        .collect()
}
```

PURL normalization (per VR-083-004):
- Lowercase package type (`pkg:cargo/` not `pkg:Cargo/`)
- Strip qualifiers (`?repository_url=...`) when comparing if any tool omits them
- Sort `Vec<Edge>` alphabetically by `(from, to)` for deterministic output

## Audit-row JSON shape

Per research §7. Captured per-ecosystem in `research.md` under `### Ecosystem: <name>`. Test code emits the same shape via `Debug` derive on `AuditRow` so test failures include the structured findings inline.

## Test contract

Each `transitive_parity_<ecosystem>.rs` MUST cover:

| Test | Validates |
|---|---|
| `transitive_edges_match_baseline` | FR-007 — edge count matches `EXPECTED_EDGE_COUNT` from alpha.23 baseline; representative edges match `EXPECTED_REPRESENTATIVE_EDGES` |
| `cross_tool_parity_check` | FR-001 + Q2 — runs all 3 SBOM tools + tiebreaker; emits `AuditRow` to test stdout |
| `graceful_skip_when_tools_absent` | research §5 — skip when external tools missing AND env var unset |

The 11 per-ecosystem files (Go, Cargo, npm, Maven, pip-poetry, pip-pipfile, pip-plain, gem, dpkg, rpm, apk) follow the same structure.

## CI workflow contract

`.github/workflows/ci.yml` Linux job extends with three new steps before `cargo test`:

```yaml
- name: Install trivy
  run: |
    sudo apt-get install -y wget
    wget -qO - https://aquasecurity.github.io/trivy-repo/deb/public.key | sudo apt-key add -
    echo deb https://aquasecurity.github.io/trivy-repo/deb $(lsb_release -sc) main | sudo tee -a /etc/apt/sources.list.d/trivy.list
    sudo apt-get update
    sudo apt-get install -y trivy=0.69.3
- name: Install syft
  run: curl -sSfL https://raw.githubusercontent.com/anchore/syft/main/install.sh | sh -s -- -b /usr/local/bin v1.27.0
- name: Set MIKEBOM_REQUIRE_TRANSITIVE_PARITY
  run: echo "MIKEBOM_REQUIRE_TRANSITIVE_PARITY=1" >> $GITHUB_ENV
```

macOS lane intentionally skips (per milestone-078 macOS exclusion + the dpkg/rpm/apk Linux-only constraint).

## Performance contract

- Per-ecosystem test wall-time <10s (mikebom + trivy + syft invocations × ~3s each).
- Audit harness wall-time across all 11 tests <60s end-to-end.
- Test-binary compile cost negligible (~150 LOC shared harness + 11 thin per-ecosystem test files).
