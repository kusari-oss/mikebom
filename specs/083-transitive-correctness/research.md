# Research — milestone 083 Transitive dep correctness audit per ecosystem

Six implementation-level decisions to pin before Phase 1 design + the per-ecosystem audit fixture-selection table that's the central deliverable.

## §1 — Tool version pinning

**Decision**: trivy `0.69.3` + syft `1.27.0` (the versions installed on the developer workstation at `/speckit.plan` time, 2026-05-07). Pin in research.md; CI installs the same versions; future re-audits document any version bumps.

**Rationale**: pinning prevents silent comparison-baseline drift. Edge counts change between trivy/syft releases (e.g., a trivy release that adds a new transitive resolver step would shift mikebom's "matches expected" classification to "minor differences" without any mikebom code change). Pinning makes the audit reproducible.

**Alternatives considered**:
- "Latest" rolling version — Rejected: edge counts shift silently; regression tests become flaky as upstream tools update.
- Multiple version-matrix testing — Rejected: scope creep; one pinned version per tool is sufficient for the audit's purpose.

## §2 — Per-ecosystem fixture selection

**Decision**: 11 vendored fixtures (pip splits into 3: poetry/pipfile/plain) under `mikebom-cli/tests/fixtures/transitive_parity/<ecosystem>/`, manifest+lockfile-only per the Q1 clarification. Each fixture's `README.md` cites the source URL + commit SHA the manifest was extracted from.

**Per-ecosystem fixture candidates** (final selection happens at fixture-extraction time during T-tasks; this list is a shortlist):

| Ecosystem | Candidate source repo | Why |
|---|---|---|
| **Go** | `kubernetes/cri-tools` (~300 deps in go.sum) | Real-world Kubernetes-ecosystem project; large transitive closure; well-pinned releases |
| **Cargo** | `clap-rs/clap` workspace (~80 deps) | Idiomatic Rust workspace project; non-trivial dep tree |
| **npm** | `expressjs/express` (~200 deps in package-lock.json v3) | Canonical npm reference; large transitive closure; lockfile-v3 (post-milestone-008 reader target) |
| **Maven** | `apache/commons-lang` | Real Apache project with parent POM chain |
| **pip-poetry** | `pypa/poetry` itself (poetry.lock) | Self-hosting; ~100 deps |
| **pip-pipfile** | A simple Pipfile.lock-using project (selection deferred) | Less common but encoded edges |
| **pip-plain** | A small `requirements.txt`-only project | Documents upstream limitation per FR-008; expected zero transitive edges |
| **gem** | `rubocop/rubocop` (Gemfile.lock; ~60 deps) | Real Ruby tooling project |
| **dpkg** | Debian 12 base container `/var/lib/dpkg/status` extract | Standard debian rootfs |
| **rpm** | Fedora 39 base container rpmdb extract | Standard fedora rootfs |
| **apk** | Alpine 3.20 base container `/lib/apk/db/installed` extract | Standard alpine rootfs |

**Rationale**: real-world coverage with bounded fixture size. Each fixture meets the ≥50 components / ≥100 edges threshold per FR-002.

**Alternatives considered**:
- Generated synthetic fixtures — Rejected per Q1 (less representative).
- Vendoring full source trees — Rejected per Q1 (unbounded repo growth).

## §3 — Audit harness implementation strategy

**Decision**: shared helper module at `mikebom-cli/tests/transitive_parity_common.rs` with the four invocation functions (run_mikebom / run_trivy / run_syft / run_source_format_direct) + the `diff_edge_sets` helper + the `assert_graceful_skip` env-var hook. Mirrors milestone-078's `spdx3_conformance.rs` graceful-skip pattern.

**Per-tool invocation pattern**:
```rust
fn run_trivy(fixture_path: &Path) -> Result<Vec<Edge>, AuditError> {
    let output = Command::new("trivy")
        .args(["fs", "--format", "spdx-json", fixture_path.to_str().unwrap()])
        .output()
        .context("invoking trivy")?;
    let sbom: SpdxDocument = serde_json::from_slice(&output.stdout)?;
    Ok(extract_edges_from_spdx_relationships(&sbom))
}
```

Symmetric for `run_syft` (uses `syft <path> -o spdx-json`) + `run_mikebom` (`cargo run -p mikebom-cli -- sbom scan --path <fixture> --format spdx-3-json --output -`).

**Edge extraction**: SPDX 2.3 `relationships[]` filtered to `relationshipType: "DEPENDS_ON"` → `(spdxElementId, relatedSpdxElement)` tuples. SPDX 3 `software_dependsOn` arrays → `(from_iri, to_iri)` tuples. PURL-normalized for cross-tool equality (since each tool may emit slightly different SPDX-IDs but the underlying PURLs match).

**Rationale**: minimum new infrastructure; reuses existing milestone-078 patterns; PURL-based comparison is the lowest-common-denominator across tools.

## §4 — Source-format direct-read tiebreaker dispatch (per Q2)

**Decision**: per-ecosystem dispatch in `run_source_format_direct(fixture_path, ecosystem)`:

| Ecosystem | Tiebreaker source | Implementation |
|---|---|---|
| Go | `go mod graph` if `go` on PATH | Subprocess; same as milestone-055 step 1 |
| Cargo | Parse `Cargo.lock` `dependencies = [...]` | TOML parser (`toml = "0.8"` already in deps); ~30 LOC |
| npm | Parse `package-lock.json` `packages[].dependencies` | `serde_json` parser; ~40 LOC |
| Maven | `mvn dependency:tree -DoutputType=text` if `mvn` on PATH | Subprocess |
| pip-poetry | Parse `poetry.lock` `[[package]]` blocks | TOML parser |
| pip-pipfile | Parse `Pipfile.lock` `default` + `develop` JSON | `serde_json` parser |
| gem | Parse `Gemfile.lock` GEM block | Custom parser (~30 LOC; gem lockfile is YAML-adjacent custom format) |
| dpkg | `dpkg-query --show -f='${Package} ${Depends}\n'` | Linux only; subprocess |
| rpm | `rpm -q --requires --all` | Linux only; subprocess |
| apk | `apk info -R` | Linux only; subprocess |

The tiebreaker is invoked **only when mikebom + trivy + syft disagree** on a specific edge. When unanimous agreement among the 3 SBOM tools, the tiebreaker is skipped (saves wall-time).

**Rationale**: the tiebreaker captures peer-tool bugs (e.g., trivy has known issues with certain Maven `<dependencyManagement>` cases); without it the audit only flags mikebom's deviations FROM trivy/syft consensus, missing cases where the peer tools are wrong.

## §5 — Graceful-skip + CI strict-mode pattern

**Decision**: env var `MIKEBOM_REQUIRE_TRANSITIVE_PARITY=1` set by CI lane; absent otherwise. When unset + a required tool is missing, the test prints `WARN tool not on PATH; skipping` + returns OK. When set + tool missing, the test fails with a clear diagnostic.

**Mirrors**: milestone-078's `MIKEBOM_REQUIRE_SPDX3_VALIDATOR` pattern verbatim. Same code shape; same operator UX.

**Per-test skip rules**:
- Tests requiring `trivy`: skip when trivy not on PATH (unless env var set).
- Tests requiring `syft`: same.
- Tests requiring `dpkg-query` / `rpm` / `apk`: skip on macOS unconditionally (these tools don't exist there); on Linux, follow the env-var rule.
- Tests requiring `mvn` (Maven tiebreaker): skip when mvn not on PATH.

**Rationale**: preserves developer-workstation experience (no forcing trivy/syft installs); CI lane enforces strict mode for regression detection.

## §6 — Indirect-vs-direct decision rubric (US3 / FR-004)

**Decision**: per-ecosystem decision matrix:

| Ecosystem | Source-format distinction | Mikebom current behavior | Audit decision |
|---|---|---|---|
| Go | `// indirect` marker in go.mod | All edges under root identically | **Defer** to follow-up issue. Mikebom's "all-edges-under-root" is operator-comprehensible; not a P1/P2 gap. |
| npm | `dependencies` vs `devDependencies` in package.json | Already mapped to milestone-052 lifecycle scope | **Verified — no new work**. Audit confirms milestone-052's mapping covers this case. |
| Cargo | `[dependencies]` vs `[dev-dependencies]` | Already mapped to milestone-052 lifecycle scope | **Verified — no new work**. Same as npm. |
| Other ecosystems | No native distinction | N/A | N/A |

**Rationale**: most distinctions are already covered by milestone-052's lifecycle-scope work. Go's `// indirect` is the one open question, but the gap is small enough to defer rather than gate this milestone on it.

**Alternatives considered**:
- Implement Go `// indirect` distinction in this milestone — Rejected: scope creep; out per FR-010.
- Document Go indirect as "deliberate divergence" with rationale — Considered; deferred to follow-up issue with the freedom to reverse if downstream tooling (e.g., dependency-track) gains a strong dependency on the distinction.

## §7 — Audit-row JSON-shape contract

**Decision**: each per-ecosystem entry in `research.md` follows the same structure (used by `data-model.md` + `contracts/audit-harness.md`):

```text
### Ecosystem: <name>

**Fixture**: tests/fixtures/transitive_parity/<ecosystem>/
**Source URL**: https://github.com/<org>/<repo>
**Commit SHA**: <40-char hex>
**Tool version**: trivy 0.69.3 / syft 1.27.0 / mikebom alpha.23

**Edge counts** (PURL-normalized):
- mikebom: N edges
- trivy: M edges
- syft: K edges
- source-format direct (when tiebreaker invoked): T edges

**Diff classification**: matches expected | minor differences | gap surfaced
**Tiebreaker resolution** (when invoked): mikebom correct | trivy correct | syft correct | source-format-says-X
**Indirect-vs-direct decision**: implement #N | document-as-divergence | deferred to #N | N/A

**Specific edge differences** (sample of up to 10 per category):
- Mikebom-only edges: ...
- Trivy-only edges: ...
- Syft-only edges: ...

**Follow-up disposition**: matches → no action | minor → tracked in regression test | gap → filed as #N
```

**Rationale**: consistent format makes the audit machine-readable AND human-readable. Operators reading any per-ecosystem entry know exactly what "the audit found" and what (if anything) is being done about it.
