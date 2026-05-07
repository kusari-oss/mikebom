# Research — milestone 082 Documentation refresh and audit

Six implementation-level findings, including the per-file audit deliverable that IS this milestone's central work-plan input.

## §1 — Per-file audit table (the central deliverable)

Every in-scope file classified per spec FR-001. Last-touched milestone derived from `git log --oneline -1 -- <path>`. Classification rubric: `current` = touched at milestone 072+ AND no obvious post-072 stale claims; `partially stale` = touched at milestone 020–071 AND likely OK on its dimension but missing recent operator surface; `materially stale` = touched pre-milestone 020 OR demonstrably describes obsolete behavior.

| Path | Last-touched milestone | Classification | Currency gap | Fix scope |
|---|---|---|---|---|
| `README.md` | 081 (PR #162) | current | minor: "what mikebom emits" table may need a milestone-080/081 mention | small inline + Q1 style pass |
| `docs/index.md` | very early (PR #17) | materially stale | no mentions of recent reference docs (sbom-types.md, conformance-harness-guide.md) | add new entries + Q1 style pass |
| `docs/design-notes.md` | 073 (PR #144) | partially stale | covers 073 but not 074–081 follow-ons | small additions + Q1 style pass |
| `docs/ecosystems.md` | 023/025 (PR #51) | partially stale | misses milestones 026–081 ecosystem additions; per the `--milestone N` reference scan, 458 lines of ecosystem detail likely have stale-but-not-broken claims | spot-check inline fixes + Q1 style pass; deeper rewrite filed as follow-up |
| `docs/architecture/overview.md` | very early (PR earliest) | materially stale | foundational architecture; describes pre-072 design; needs Q2 spot-check | Q2 spot-check (5–10 claims), inline fixes ≤30 lines per claim |
| `docs/architecture/scanning.md` | early (PR #16) | materially stale | describes pre-052 scope handling, pre-072 binding, pre-078 SPDX 3 conformance | Q2 spot-check |
| `docs/architecture/generation.md` | very early | materially stale | pre-077 root component handling, pre-080 metadata flags, pre-081 SBOM-type signaling | Q2 spot-check |
| `docs/architecture/attestations.md` | early (PR #17) | partially stale | pre-072 binding semantics; otherwise stable | Q2 spot-check |
| `docs/architecture/enrichment.md` | very early | partially stale | pre-Principle-XII codification; otherwise stable | Q2 spot-check |
| `docs/architecture/licenses.md` | early (PR #16) | partially stale | pre-078 SPDX 3 license expression handling | Q2 spot-check |
| `docs/architecture/purls-and-cpes.md` | small fix (a1390db) | partially stale | pre-073 identifier work; PURL emission at the four-tier identity model is not described | Q2 spot-check |
| `docs/architecture/resolution.md` | very early | partially stale | pre-052 scope semantics + pre-072 cross-tier resolution | Q2 spot-check |
| `docs/architecture/signing.md` | 006 (be94f1b) | partially stale | hasn't been touched since milestone 006 | Q2 spot-check |
| `docs/reference/identifiers.md` | 081 (PR #162) | current | nothing material; Q1 style pass only | small style-only edits |
| `docs/reference/sbom-types.md` | 081 (PR #162) | current | already has "See also" (milestone 081); Q1 style pass only | small style-only edits |
| `docs/reference/cross-tier-binding.md` | 072 (PR #142) | current | minor: cross-references to milestones 076 (subjects) + 080/081 (creators, sbom-type) would strengthen | small additions + "See also" + Q1 style pass |
| `docs/reference/sbom-format-mapping.md` | 081 (PR #162) | current | Section I audit-record entries from 080 + 081 are durable; preserve verbatim | "See also" added + Q1 style pass |
| `docs/reference/conformance-harness-guide.md` | 071 (PR #138) | current | minor: post-078 SPDX 3 conformance work could be cross-referenced | small additions + "See also" + Q1 style pass |
| `docs/user-guide/installation.md` | very early (PR #17) | materially stale | pre-alpha.6 install instructions; likely covers `cargo build` correctly but misses release-asset workflow | major refresh — replace install instructions with current GitHub release / Cargo release / Docker workflows |
| `docs/user-guide/quickstart.md` | very early (PR #17) | materially stale | recipes don't reflect milestones 020–081 surface (everything from milestone 020 onward is missing) | major refresh per FR-004 — add recipes for `--sbom-type`, `--metadata-file`, `--component-id`, `--root-name` |
| `docs/user-guide/cli-reference.md` | 046 (PR #86) | materially stale | **the dominant gap**: 11 missing flags from milestones 073–081 + missing per-subcommand structure for everything since alpha.6 | major refresh per FR-002 — comprehensive CLI documentation rebuild |
| `docs/user-guide/configuration.md` | 046 (PR #86) | materially stale | misses post-046 env vars (`MIKEBOM_REQUIRE_SPDX3_VALIDATOR`, `MIKEBOM_UPDATE_*_GOLDENS`, `MIKEBOM_PREPR_EBPF`, `MIKEBOM_NO_DEPRECATION_NOTICE`) | major refresh per FR-005 — comprehensive env var coverage |

**Summary metrics**:
- 22 in-scope files total
- **6 current** (28%): README.md, all 5 reference docs
- **5 partially stale** (23%): design-notes, ecosystems, 4 architecture docs touched in milestone 006–025 era
- **11 materially stale** (50%): index.md + 4 user-guide files + 6 architecture docs touched pre-milestone 020

**Critical-path files** (the milestone's biggest fix scope):
1. `docs/user-guide/cli-reference.md` — 11 missing flags + structural rebuild
2. `docs/user-guide/quickstart.md` — recipes refresh covering ~milestones 020–081
3. `docs/user-guide/configuration.md` — env var coverage
4. `docs/user-guide/installation.md` — modern install workflow

**Architecture-doc Q2 spot-check candidates** (5–10 testable claims per doc; expanded in §6 below).

## §2 — Style-convention decisions (the three dimensions per FR-007)

Three decisions, applied across all 22 in-scope files per the 2026-05-07 Q1 clarification.

### Milestone-reference handling

**Decision**: omitted in `user-guide/*.md` + `README.md`; retained as parenthetical `(milestone N)` in `reference/*.md` + `architecture/*.md` + `design-notes.md` + `ecosystems.md` + `index.md`.

**Rationale**: operators reading user-guide / README don't need milestone numbers — git log + release notes carry that history. Maintainers reading reference + architecture docs benefit from the milestone-N anchor for traceability when investigating "when did this behavior change?". Single source of truth for full milestone history is `git log` + the per-milestone spec dirs.

**Phase 0 audit** found 104 milestone references in the corpus. The decision causes:
- `user-guide/cli-reference.md` + `user-guide/quickstart.md` + `user-guide/installation.md` + `user-guide/configuration.md` + `README.md`: strip all `milestone N` references; replace flag-introduction context with prose ("the `--sbom-type` flag accepts ..." not "milestone 081 added `--sbom-type`").
- All other in-scope files: normalize to parenthetical `(milestone N)` form. Existing variants (inline `[milestone N]`, comma-sep "milestone N", dash-form "milestone-N") get rewritten.

### Code-block fence convention

**Decision**: language tag REQUIRED on every fenced block. Allowed tags from the existing corpus survey:
- `bash` — for shell command examples
- `json` — for JSON literal snippets
- `text` — for plain output (e.g., expected stdout, error messages)
- `rust` — for Rust code snippets (rare in user-facing docs; common in reference + architecture)
- `yaml` — for CI config, configuration file examples
- `toml` — for `Cargo.toml` snippets
- `markdown` — for nested markdown examples

**Rationale**: untagged blocks render without syntax highlighting in most viewers (GitHub web, IDE markdown previews, pandoc) and signal "this content is unfamiliar" to readers. The Phase 0 audit found 4 untagged blocks across `cli-reference.md` (3) and `architecture/overview.md` (1) and `README.md` (1). Aggressive normalization adds tags to all.

### Inter-doc link convention

**Decision**: relative paths from the file's own directory; NO leading `./`.

**Rationale**: GitHub renders relative links correctly without the `./` prefix; pandoc/static-site generators do too; the prefix is visual noise. Mixed conventions in the corpus: `[X](./other.md)` vs `[X](other.md)` are both functionally equivalent but visually inconsistent. Normalize to no-`./`.

**Examples**:
- From `docs/index.md` to `docs/user-guide/quickstart.md`: `[Quickstart](user-guide/quickstart.md)` (no `./`).
- From `docs/reference/identifiers.md` to `docs/user-guide/cli-reference.md`: `[CLI reference](../user-guide/cli-reference.md)`.
- From `docs/user-guide/quickstart.md` to `docs/reference/sbom-types.md`: `[SBOM types](../reference/sbom-types.md)`.
- External links: keep absolute URLs verbatim (`[CISA](https://www.cisa.gov/...)`); never substitute relative.

## §3 — CLI-reference document structure (FR-002)

**Decision**: per-subcommand sections with a flag-summary table at top of each section + per-flag detail blocks below.

**Layout sketch**:
```markdown
# CLI reference

## `mikebom sbom scan`

Scan a directory or container image and emit one or more SBOM formats.

### Quick reference

| Flag | Type | Default | Description |
|---|---|---|---|
| `--path <PATH>` | path | (required if no --image) | Directory to scan |
| `--image <IMAGE>` | image-ref | (required if no --path) | Container image to scan |
| `--format <FMT>` | enum (repeatable) | `cyclonedx-json` | Output format |
| ... | ... | ... | ... |
| `--sbom-type <TYPE>` | enum | (auto-detect) | Operator-asserted CISA SBOM Type |

### `--path <PATH>`

Directory to scan. Required when `--image` is not provided. ...

```bash
mikebom sbom scan --path . --output sbom.cdx.json
```

See also: [Scanning](../architecture/scanning.md) for design details.
```

**Subcommand coverage**: `mikebom sbom scan`, `mikebom trace run`, `mikebom sbom verify`, `mikebom sbom enrich`, `mikebom policy init`, `mikebom verify-binding`, `mikebom trace-binding`, `mikebom trace capture` (experimental flag).

**Rationale**: per-subcommand structure matches `--help` output operators see; quick-reference table at top serves operators who know the flag name; per-flag detail serves operators who need depth. Per FR-011, deprecated flags get a `**Deprecated:** since milestone N. Replacement: `--<other-flag>`.` line in their detail block.

## §4 — Cross-reference graph design (FR-003)

**Decision**: bottom-of-page section titled exactly "## See also" with bullet list of 2–5 nearest-neighbor reference docs.

**Format**:
```markdown
## See also

- [SBOM types](sbom-types.md) — CISA SBOM Type signaling and the `--sbom-type` flag.
- [Cross-tier binding](cross-tier-binding.md) — Source ↔ build ↔ image SBOM correlation.
- [Identifiers](identifiers.md) — The four-layer identity model for component identification.
```

**Per-reference-doc link plan** (the connected graph):
- `identifiers.md` → cross-tier-binding.md, sbom-types.md, sbom-format-mapping.md, conformance-harness-guide.md
- `sbom-types.md` (already exists) → identifiers.md, sbom-format-mapping.md, cross-tier-binding.md
- `cross-tier-binding.md` → identifiers.md, sbom-types.md, conformance-harness-guide.md
- `sbom-format-mapping.md` → identifiers.md, sbom-types.md, conformance-harness-guide.md
- `conformance-harness-guide.md` → sbom-format-mapping.md, identifiers.md, sbom-types.md

**SC-003 verification**: starting from any reference doc, ≤3 clicks reach any other reference doc. The above graph is fully connected; max distance = 2 (e.g., `identifiers.md → sbom-types.md → cross-tier-binding.md`).

**Rationale**: 5 reference docs × 2–4 links each = small surface; review-able by hand; serves operators navigating sideways through related concepts.

## §5 — Audit deliverable structure (research.md format)

**Decision**: per-file table format used for §1 above; per-architecture-doc spot-check appendix in §6 below.

**Rationale**: the table format is compact (~22 rows for the per-file audit), scannable for reviewers, and serves as the work-plan input for /speckit.tasks. The appendix format for architecture docs allows per-doc claim verification with status tracking.

## §6 — Architecture-doc spot-check claims (Q2 deliverable)

Per the 2026-05-07 Q2 clarification, each architecture doc gets 5–10 testable behavioral claims identified + verified against alpha.22 source. Below is the audit's scoping list (the actual verification happens during T-task execution).

### `docs/architecture/overview.md` — five-stage-pipeline claims
1. mikebom's pipeline has the documented stages (verify by tracing `mikebom-cli/src/cli/scan_cmd.rs::execute`).
2. Each stage's output type matches the doc (verify against the actual public types).
3. The "no fallback" claim (Principle III) — failures abort with non-zero exit (verify against `anyhow::Result` propagation).
4. The native-Rust claim (Principle I) — no C dependencies (verify via `cargo tree | grep -i "c-bindings"` returning empty).
5. The eBPF-discovery claim (Principle II) — only eBPF for discovery (verify against the boundary between `binding/identifiers/auto_detect.rs` and the eBPF trace path).

### `docs/architecture/scanning.md` — filesystem-walk + per-ecosystem package DBs
1. mikebom walks all of `--path` recursively (verify against `scan_fs/walker.rs`).
2. Each ecosystem has a per-package-DB module under `scan_fs/package_db/` (verify against directory listing).
3. mikebom resolves dep-scope per milestone 052 (verify against the `--exclude-scope` flag's behavior).
4. mikebom auto-detects `mikebom:sbom-tier` per milestone 047 (verify against per-component annotations in fresh emission).
5. eBPF trace path observes builds, not runtime (verify per CISA Runtime semantics + research §3 from milestone 081).

### `docs/architecture/generation.md` — per-format builders
1. CDX 1.6 emission goes through `generate/cyclonedx/builder.rs` (verify path exists).
2. SPDX 2.3 emission goes through `generate/spdx/document.rs` (verify path exists).
3. SPDX 3 emission goes through `generate/spdx/v3_document.rs` (verify path exists).
4. The three formats share the same `lifecycle_phases.rs` aggregation (per milestones 047 + 081).
5. Schema validation runs per format (verify against the existing `schema_validation` test targets).

### `docs/architecture/attestations.md` — in-toto witness-v0.1 emission
1. mikebom emits in-toto witness-v0.1 envelopes (verify against `mikebom_common::attestation`).
2. The cross-tier binding spec (milestone 072) adds binding metadata (verify per `verify-binding` / `trace-binding` subcommands).
3. Build-tier subjects per milestone 076 are documented (verify against `--subject-hash` flag's behavior).

### `docs/architecture/enrichment.md` — Principle XII external-source enrichment
1. mikebom NEVER introduces components from external sources (Strict Boundary 1; verify against the `discovery vs enrichment` invariant in `binding/`).
2. ClearlyDefined enrichment is opt-out via `--offline` (verify behavior).
3. `deps.dev` queries are enrichment-only (verify against the existing query path).

### `docs/architecture/licenses.md` — license-expression handling
1. SPDX-listed licenses use the canonical IRI (verify per milestone 078 emission).
2. License expressions are canonicalized via the `spdx` crate (verify dep usage).
3. SPDX 3 emits `simplelicensing_LicenseExpression` per milestone 078 (verify wire shape).

### `docs/architecture/purls-and-cpes.md` — PURL + CPE emission
1. Every component carries a PURL per Principle V (verify against per-format emission).
2. CPEs are emitted when content-shape detection succeeds (verify per ecosystem coverage).
3. The four-layer identity model (milestones 072–077) is the canonical identity surface (verify against `identifiers.md` reference).

### `docs/architecture/resolution.md` — dep-tree resolution
1. mikebom uses lockfiles for dependency-tree edges per Principle XII (verify behavior).
2. Pre-052 dep-scope handling is OBSOLETE; current behavior is `--exclude-scope`-based (verify against current flag).
3. Polyglot scans aggregate per-ecosystem (verify against `polyglot-monorepo` fixture).

### `docs/architecture/signing.md` — DSSE signing
1. mikebom emits DSSE-signed envelopes when signing is enabled (verify against `verify_dsse` test target).
2. Signing keys are operator-supplied; mikebom doesn't manage key material (verify against the signing CLI surface).

**Verification rubric** (per Q2): each claim's status post-fix is one of:
- `verified` — claim is accurate; no doc edit needed.
- `fixed inline` — claim was stale; ≤30-line doc edit applied.
- `filed as follow-up #N` — claim's accurate fix requires >30 lines; GitHub issue filed; doc retains the stale claim with a note.

≥90% per-claim accuracy post-fix per SC-008.

## §7 — `scripts/verify-docs-currency.sh` design (SC-001 verifier)

**Decision**: small bash script that diffs `mikebom <subcommand> --help` flag set against the doc's documented flag set. Exits 0 when in sync; exits 1 with the missing-flag list when out of sync.

**Implementation sketch** (~40 LOC):
```bash
#!/usr/bin/env bash
# verify-docs-currency.sh — SC-001 verifier for milestone 082
# Diffs `mikebom <sub> --help` flag set against docs/user-guide/cli-reference.md.

set -euo pipefail

CLI_REF="${REPO_ROOT:-.}/docs/user-guide/cli-reference.md"
MIKEBOM="${MIKEBOM_BIN:-cargo run --quiet --}"

extract_flags_from_help() {
  $MIKEBOM "$@" --help 2>&1 | grep -oE '^\s+--[a-z][a-z0-9-]*' | tr -d ' ' | sort -u
}

extract_flags_from_doc() {
  grep -oE -- '--[a-z][a-z0-9-]*' "$CLI_REF" | sort -u
}

ok=0
for sub in "sbom scan" "trace run" "sbom verify" "sbom enrich" "policy init"; do
  binary_flags=$(extract_flags_from_help $sub)
  doc_flags=$(extract_flags_from_doc)
  missing=$(comm -23 <(echo "$binary_flags") <(echo "$doc_flags") || true)
  if [ -n "$missing" ]; then
    echo "FLAGS MISSING from CLI reference for 'mikebom $sub':"
    echo "$missing"
    ok=1
  fi
done

if [ "$ok" -eq 0 ]; then
  echo "✓ CLI reference is current — every flag from --help is documented."
fi
exit $ok
```

**Rationale**: script-based verification beats manual review for currency; runs in <1s; can be invoked in CI optionally (out of scope for this milestone but easily added later). The script lives in `scripts/` alongside `pre-pr.sh` + `install-spdx3-validate.sh`; chmod +x.

**Alternatives considered**:
- Cargo test integration — overkill for a docs-currency check; would couple test runtime to docs.
- Manual review only — doesn't scale with future flag additions; SC-001 needs an objective verifier.
