# Quickstart — milestone 093 maintainer recipes

Six maintainer-facing recipes to apply the polish edits and verify each contract.

## Recipe 1 — Fix the broken README git URL (FR-001 / SC-001)

```bash
# Find the offending line:
grep -n "mlieberman85/mikebom" README.md
# Expected (pre-093): line ~238 prints the wrong URL.

# Apply the fix (one-shot):
sed -i.bak \
  's|https://github.com/mlieberman85/mikebom.git|https://github.com/kusari-sandbox/mikebom.git|g' \
  README.md
rm README.md.bak

# Verify:
grep -rn "mlieberman85" --include='*.md' --include='*.toml' --include='*.yml' .
# Expected: no output (exit code 1 from grep is OK — means no matches).
```

## Recipe 2 — Add cargo binstall to README + Cargo.toml (FR-006 / SC-005)

Step 1 — append to `mikebom-cli/Cargo.toml`:

```toml
[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/v{ version }/{ name }-v{ version }-{ target }{ archive-suffix }"
pkg-fmt = "tgz"
```

Step 2 — add a new subsection to the README's `## Install` section (right after the existing tarball-download block, before "Or build from source"):

```markdown
### Via cargo binstall (Rust toolchain users)

If you already have [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall)
installed, you can skip the `gh release download` dance:

```bash
cargo binstall --git https://github.com/kusari-sandbox/mikebom mikebom
```

Bare `cargo binstall mikebom` will work once mikebom is published to crates.io
(planned for a future milestone). Today the `--git` form pulls the release
tarball from GitHub Releases per the same naming pattern the [Cargo.toml
binstall metadata](mikebom-cli/Cargo.toml) pins.
```

Step 3 — verify the metadata table:

```bash
cargo +stable build -p mikebom 2>&1 | tail -3
# Expected: clean build (cargo treats package.metadata.* as opaque).
```

## Recipe 3 — Create SECURITY.md (FR-002 / SC-002)

```bash
cat > SECURITY.md <<'EOF'
# Security policy

## Reporting a vulnerability

Please **do not open a public GitHub issue** for vulnerabilities. Report
them privately through GitHub Security Advisories:

  https://github.com/kusari-sandbox/mikebom/security/advisories/new

This routes the report to the maintainers without exposing it publicly.

## What to expect

- **Acknowledgment** within 7 days of report.
- **Triage decision** (accepted / declined / needs-more-info) within
  14 days.
- **Fix or detailed status update** within 30 days.

Coordinated disclosure timelines can be negotiated case-by-case if a
fix requires longer (e.g., a deep refactor or upstream dependency
update).

## Supported versions

mikebom is pre-1.0 alpha. Only the **most recent alpha release** is
supported for security fixes. SemVer guarantees do not apply until 1.0.

## Scope

In scope:

- Crashes, hangs, or memory-safety issues during scan / verify / trace.
- SBOM emission that misrepresents the underlying observed evidence
  (e.g., a component reported as present that wasn't observed in the
  trace, or vice versa — this contradicts Constitution Principles VIII
  + IX).
- Attestation verification failures that incorrectly accept malformed
  or revoked signatures.
- Privilege-escalation or container-escape via the eBPF tracer.

Out of scope:

- Vulnerabilities in mikebom's upstream Rust crate dependencies — those
  go through normal CVE channels and are tracked by `cargo audit` / the
  release-blocking Kusari Inspector CI check.
- Bugs that produce wrong SBOM output without a security implication
  (those should be filed as regular GitHub issues).
EOF
```

Verify:
```bash
grep -c '^## ' SECURITY.md
# Expected: 4
```

## Recipe 4 — Create CONTRIBUTING.md (FR-003 / SC-003)

See `data-model.md §CONTRIBUTING.md` for full content shape. Skeleton:

```bash
cat > CONTRIBUTING.md <<'EOF'
# Contributing to mikebom

Thanks for your interest in contributing! mikebom is pre-1.0 alpha; we
encourage coordination on non-trivial changes before you open a PR.

## Workflow overview (the speckit lifecycle)

For non-trivial changes (new features, behavior changes, large
refactors), mikebom uses the spec-kit lifecycle:

1. `/speckit.specify` — write the feature spec
2. `/speckit.clarify` (optional) — resolve open questions
3. `/speckit.plan` — produce research.md, data-model.md, contracts/
4. `/speckit.tasks` — break work into checklist tasks
5. `/speckit.analyze` (optional) — cross-check spec ↔ plan ↔ tasks
6. `/speckit.implement` — execute the task list

Full skill references live under `.claude/skills/speckit-*/SKILL.md`.
Small drive-by fixes (typos, doc tweaks, single-line bug fixes) skip
the lifecycle — just open a PR.

## Local development setup

```bash
git clone https://github.com/kusari-sandbox/mikebom.git
cd mikebom
cargo +stable build --release
```

The `sbom`, `policy`, `attestation`, and related subcommands build
under the **stable** toolchain. The eBPF-based `trace` subcommands
additionally need nightly — see
[`docs/user-guide/installation.md`](docs/user-guide/installation.md).

## Pre-PR gate (MANDATORY)

Before opening any PR, **both** of these MUST exit clean:

```bash
./scripts/pre-pr.sh
```

This runs `cargo +stable clippy --workspace --all-targets -- -D warnings`
(zero warnings) AND `cargo +stable test --workspace` (every suite
`0 failed`). Both must pass before CI will accept the PR.

For SBOM-spec-touching changes, also set the SPDX-3 conformance gate:

```bash
MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1 ./scripts/pre-pr.sh
```

This requires the JPEWdev `spdx3-validate` package pinned in
`.venv/spdx3-validate/`.

## Project principles + where to find them

The canonical source-of-truth is [`.specify/memory/constitution.md`](.specify/memory/constitution.md).
Twelve principles to be aware of:

- **I. Pure Rust, Zero C** — no FFI, no `libbpf` bindings.
- **II. eBPF-Only Observation** — eBPF tracing is the trust-rooted
  discovery path; external sources only enrich what was observed.
- **III. Fail Closed** — never gap-fill with heuristics when the trace
  loses data.
- **IV. Type-Driven Correctness** — newtype wrappers for PURL, hashes,
  license expressions; no `.unwrap()` in production code.
- **V. Specification Compliance** — CycloneDX 1.6 + SPDX 2.3 +
  SPDX 3.x conformance; **standards-native fields take precedence
  over `mikebom:*` properties**.
- **VI. Three-Crate Architecture** — `mikebom-ebpf` (no_std),
  `mikebom-common` (shared structs), `mikebom-cli` (user-space).
- **VII. Test Isolation** — privilege-dependent tests gated behind
  CAP_BPF; unprivileged unit tests run on every CI.
- **VIII. Completeness** — minimize false negatives.
- **IX. Accuracy** — minimize false positives; flag low-confidence
  matches.
- **X. Transparency** — surface limitations via spec-native fields.
- **XI. Enrichment** — license, VEX, supplier metadata when available.
- **XII. External Data Source Enrichment** — lockfiles / registries
  enrich; eBPF trace remains authoritative.

If your change touches any principle, link to it from your PR description
and explain how the change preserves the principle.

## Per-spec planning artifacts

Each milestone lives at `specs/<NNN>-<short-name>/`:

```
specs/092-fix-maven-version-extract/
├── spec.md           # WHAT and WHY
├── plan.md           # HOW (architecture, gates)
├── research.md       # Phase 0 decisions
├── data-model.md     # Phase 1 data shapes
├── contracts/        # Phase 1 behavioral contracts
├── quickstart.md     # Maintainer recipes
├── tasks.md          # Phase 2 checklist
└── checklists/
    └── requirements.md
```

Open one for any non-trivial change before writing code.
EOF
```

Verify:
```bash
grep -c '^## ' CONTRIBUTING.md       # >= 5
grep -q 'pre-pr.sh' CONTRIBUTING.md
grep -q 'constitution.md' CONTRIBUTING.md
grep -q 'MIKEBOM_REQUIRE_SPDX3_VALIDATOR' CONTRIBUTING.md
```

## Recipe 5 — Create issue + PR templates (FR-004, FR-005 / SC-004)

```bash
mkdir -p .github/ISSUE_TEMPLATE
```

Then write the three template files (`.github/ISSUE_TEMPLATE/bug_report.yml`,
`.github/ISSUE_TEMPLATE/feature_request.yml`, `.github/pull_request_template.md`)
per the shape defined in `data-model.md` and Contracts 6 + 7. See those
documents for exact YAML and Markdown bodies.

Verify:

```bash
test -f .github/ISSUE_TEMPLATE/bug_report.yml \
  && test -f .github/ISSUE_TEMPLATE/feature_request.yml \
  && test -f .github/pull_request_template.md

# YAML parses:
python3 -c "import yaml; yaml.safe_load(open('.github/ISSUE_TEMPLATE/bug_report.yml'))"
python3 -c "import yaml; yaml.safe_load(open('.github/ISSUE_TEMPLATE/feature_request.yml'))"

# PR-template checklist count:
grep -c '^- \[ \]' .github/pull_request_template.md
# Expected: >= 3.
```

## Recipe 6 — Final pre-PR gate (FR-010 / SC-006)

```bash
./scripts/pre-pr.sh
# Expected: prints `>>> all pre-PR checks passed.`
# All test targets report `N passed; 0 failed`.
# Zero clippy warnings.
```

## Recipe 7 — Diff scope audit (FR-007/FR-008/FR-009 / SC-007)

```bash
git diff --name-only main | sort

# Expected output (exact set, only):
# .github/ISSUE_TEMPLATE/bug_report.yml
# .github/ISSUE_TEMPLATE/feature_request.yml
# .github/pull_request_template.md
# CONTRIBUTING.md
# README.md
# SECURITY.md
# mikebom-cli/Cargo.toml

# Confirm NO source-code, golden, or Cargo.lock files appear:
git diff --name-only main | grep -E '^(mikebom-cli/src|mikebom-common/src|xtask/src|mikebom-cli/tests/fixtures/golden|Cargo\.lock)' \
  && echo "SCOPE-CREEP DETECTED" || echo "Scope clean."
```

## When in doubt

- **`SECURITY.md` doesn't surface on GitHub's Security tab post-merge**: confirm the file is at repo root (not `docs/`). GitHub picks it up automatically; no settings page interaction needed.
- **Issue templates don't appear in the chooser**: the templates must be in `.github/ISSUE_TEMPLATE/` (uppercase `ISSUE_TEMPLATE`). YAML files must parse cleanly — a malformed YAML will silently fall through and not show.
- **`cargo binstall --git ... mikebom` errors with "no matching binary found"**: the `[package.metadata.binstall]` block's `pkg-url` template doesn't match the actual release-tarball naming. Double-check the four template variables (`{ repo }`, `{ version }`, `{ name }`, `{ target }`, `{ archive-suffix }`) against an actual GitHub Releases URL.
- **Pre-PR gate fails clippy**: shouldn't happen — no Rust source touched. If it does, audit `git diff` for accidental source-tree changes.
- **PR-template checkbox renders as plain text not interactive**: ensure the Markdown uses `- [ ]` (with the space inside the brackets). `- []` won't render as a checkbox.
