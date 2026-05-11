# Contract — Repo surface deliverables

Behavioral / content contracts for every new or modified file. Each contract names: (a) what the file MUST contain, (b) how to verify (a) without executing code.

## Contract 1 — README install commands resolve (FR-001 / SC-001)

**File**: `README.md`

**Pre-093 state**: line 238 contains `git clone https://github.com/mlieberman85/mikebom.git` — 404 against the canonical repo.

**Post-093 state**: line 238 contains `git clone https://github.com/kusari-sandbox/mikebom.git`. Repo-wide grep for `mlieberman85` returns zero hits in committed content (`*.md`, `*.toml`, `*.yml`, etc. — excluding git log history).

**Verification (no code execution required)**:
```bash
grep -rn "mlieberman85" --include='*.md' --include='*.toml' --include='*.yml' .
# Expected: no output, exit 1.
```

## Contract 2 — README documents cargo binstall (FR-006 / SC-005)

**File**: `README.md`'s `## Install` section.

**Post-093 state**: a new subsection (heading approx `### Via cargo binstall (Rust toolchain users)`) appears AFTER the existing tarball-download path and BEFORE `## Supported ecosystems`. The subsection contains:

- A one-line preamble explaining when this is useful.
- A fenced bash block with the exact invocation: `cargo binstall --git https://github.com/kusari-sandbox/mikebom mikebom`
- A note that bare `cargo binstall mikebom` works once mikebom publishes to crates.io.

**Verification**: visual inspection of the rendered README. Open the PR's "Files changed" view and confirm the new section appears with all three elements.

## Contract 3 — Cargo.toml binstall metadata block exists (FR-006)

**File**: `mikebom-cli/Cargo.toml`

**Post-093 state**: a `[package.metadata.binstall]` table appears, with at minimum:

```toml
[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/v{ version }/{ name }-v{ version }-{ target }{ archive-suffix }"
pkg-fmt = "tgz"
```

**Verification**:
```bash
grep -A2 "\[package.metadata.binstall\]" mikebom-cli/Cargo.toml
# Expected: shows the table + pkg-url + pkg-fmt lines.
cargo +stable build -p mikebom 2>&1 | tail -3
# Expected: successful build (cargo treats unknown package.metadata.* as opaque; no parse error).
```

## Contract 4 — SECURITY.md presence + structure (FR-002 / SC-002)

**File**: `SECURITY.md` at repo root.

**Post-093 state**: file exists; renders cleanly on GitHub; contains four section headings:

- `## Reporting a vulnerability`
- `## What to expect`
- `## Supported versions`
- `## Scope`

Required content per section per data-model.md §`SECURITY.md`.

**Verification**:
```bash
test -f SECURITY.md && grep -E "^## (Reporting a vulnerability|What to expect|Supported versions|Scope)" SECURITY.md | wc -l
# Expected: 4 matches.
```

GitHub Security-tab integration verification (post-merge): visit `https://github.com/kusari-sandbox/mikebom/security/policy` and confirm it renders the SECURITY.md file (not a "no security policy" empty state).

## Contract 5 — CONTRIBUTING.md presence + structure (FR-003 / SC-003)

**File**: `CONTRIBUTING.md` at repo root.

**Post-093 state**: file exists; contains five section headings (per data-model.md §`CONTRIBUTING.md`):

- `## Welcome`
- `## Workflow overview (the speckit lifecycle)`
- `## Local development setup`
- `## Pre-PR gate (MANDATORY)`
- `## Project principles + where to find them`

Required content callouts:

- The `## Pre-PR gate (MANDATORY)` section MUST name `./scripts/pre-pr.sh` and document the exact env-var opt-in for the SPDX-3 validator (`MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1`).
- The `## Project principles` section MUST link to `.specify/memory/constitution.md` and enumerate the 12 principles by name.
- The `## Workflow overview` section MUST link to the per-skill files under `.claude/skills/speckit-*/SKILL.md` (or equivalent).

**Verification**:
```bash
test -f CONTRIBUTING.md \
  && grep -c '^## ' CONTRIBUTING.md \
  && grep -q "pre-pr.sh" CONTRIBUTING.md \
  && grep -q "constitution.md" CONTRIBUTING.md \
  && grep -q "MIKEBOM_REQUIRE_SPDX3_VALIDATOR" CONTRIBUTING.md
# Expected: file exists; >= 5 H2 headings; all four greps succeed.
```

## Contract 6 — Issue templates as YAML forms (FR-004 / SC-004)

**Files**: `.github/ISSUE_TEMPLATE/bug_report.yml` + `.github/ISSUE_TEMPLATE/feature_request.yml` (optional: `.github/ISSUE_TEMPLATE/config.yml`).

**Post-093 state for `bug_report.yml`**: GitHub issue-form schema with required fields per data-model.md §`bug_report.yml`:

- `name`, `description`, `title`, `labels` at the top level.
- A `body:` array containing at least one `markdown` greeting, then required fields: OS dropdown, mikebom version input, command + output textarea, expected vs actual textarea, optional additional context textarea.

**Post-093 state for `feature_request.yml`**: similar schema with required use-case + proposed-solution textareas, optional alternatives + additional context.

**Verification**:
```bash
test -f .github/ISSUE_TEMPLATE/bug_report.yml \
  && test -f .github/ISSUE_TEMPLATE/feature_request.yml
# Expected: both files exist.

# Validate YAML parses:
python3 -c "import yaml; yaml.safe_load(open('.github/ISSUE_TEMPLATE/bug_report.yml'))"
python3 -c "import yaml; yaml.safe_load(open('.github/ISSUE_TEMPLATE/feature_request.yml'))"
# Expected: both succeed with no output.
```

GitHub UI verification (post-merge): visit `https://github.com/kusari-sandbox/mikebom/issues/new/choose` and confirm both templates appear in the chooser.

## Contract 7 — PR template content (FR-005 / SC-004)

**File**: `.github/pull_request_template.md`

**Post-093 state**: file contains a Markdown body with a `## Pre-PR checklist` section containing at minimum these three checkbox items (additional items per data-model.md §`pull_request_template.md` allowed):

- "I ran `./scripts/pre-pr.sh` and it exited clean"
- "For non-trivial changes, I followed the speckit lifecycle"
- "If I touched SBOM emission or output formats, I regenerated the affected byte-identity goldens"

**Verification**:
```bash
test -f .github/pull_request_template.md \
  && grep -c '^- \[ \]' .github/pull_request_template.md
# Expected: file exists; >= 3 checklist items.

grep -q "pre-pr.sh" .github/pull_request_template.md \
  && grep -q "speckit" .github/pull_request_template.md \
  && grep -q "goldens" .github/pull_request_template.md
# Expected: all three succeed.
```

GitHub UI verification (post-merge): open a draft PR; confirm the template body appears pre-filled.

## Contract 8 — Diff scope guard (FR-007 / FR-008 / FR-009 / SC-007)

**Verification**:

```bash
# Allowed file paths only:
git diff --name-only main \
  | grep -vE '^(README\.md|SECURITY\.md|CONTRIBUTING\.md|\.github/.+\.(md|yml)|mikebom-cli/Cargo\.toml)$' \
  | grep -v '^$' \
  | wc -l
# Expected: 0 (no unauthorized file paths).

# No source-tree changes:
git diff --name-only main \
  | grep -E '^(mikebom-cli/src|mikebom-common/src|xtask/src)/' \
  | wc -l
# Expected: 0.

# No golden regen:
git diff --name-only main \
  | grep -E '^mikebom-cli/tests/fixtures/golden/' \
  | wc -l
# Expected: 0.

# No Cargo.lock churn:
git diff --name-only main \
  | grep -E '^Cargo\.lock$' \
  | wc -l
# Expected: 0.
```

## Contract 9 — Pre-PR gate clean (FR-010 / SC-006)

**Verification**:
```bash
./scripts/pre-pr.sh
# Expected: prints `>>> all pre-PR checks passed.`; exit code 0.
```

Both clippy (`--workspace --all-targets -- -D warnings`) and cargo test (`--workspace`) MUST report zero failures.
