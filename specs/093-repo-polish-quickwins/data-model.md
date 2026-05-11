# Data Model — milestone 093

Per-file structure for every deliverable. Zero schema-level changes (no JSON / no YAML data models). This is a documentation-only milestone; the "data model" here is just the section structure each Markdown file MUST adopt.

## File inventory

| File | State | Owner FRs |
|------|-------|-----------|
| `README.md` | MODIFIED (2 edits) | FR-001, FR-006 |
| `SECURITY.md` | NEW (repo root) | FR-002 |
| `CONTRIBUTING.md` | NEW (repo root) | FR-003 |
| `.github/ISSUE_TEMPLATE/bug_report.yml` | NEW | FR-004 |
| `.github/ISSUE_TEMPLATE/feature_request.yml` | NEW | FR-004 |
| `.github/ISSUE_TEMPLATE/config.yml` | NEW (optional) | FR-004 |
| `.github/pull_request_template.md` | NEW | FR-005 |
| `mikebom-cli/Cargo.toml` | MODIFIED (metadata-only, ~5 lines added) | FR-006 |

## `README.md` — modifications (NOT a full rewrite)

### Edit 1 — FR-001 URL fix (line ~238)

Search-replace `https://github.com/mlieberman85/mikebom.git` → `https://github.com/kusari-sandbox/mikebom.git`. Repo-wide grep for `mlieberman85` MUST return zero hits in committed content afterward.

### Edit 2 — FR-006 cargo-binstall addition (Install section)

Append a new subsection to `## Install` (after the existing tarball / source-build paths). Suggested heading: `### Via cargo binstall (Rust toolchain users)`. Body covers:

- One-liner explaining when this is useful (already have `cargo binstall`; want to skip the `gh release download` dance).
- The exact invocation: `cargo binstall --git https://github.com/kusari-sandbox/mikebom mikebom`.
- A note that bare `cargo binstall mikebom` will start working once mikebom is published to crates.io (future milestone).
- An acknowledgment that the install honors `--no-confirm` for CI use.

## `SECURITY.md` — section structure

Four sections, ~30-50 lines total:

1. `## Reporting a vulnerability` — names the private disclosure channel (GitHub Security Advisories link: `https://github.com/kusari-sandbox/mikebom/security/advisories/new`) plus an optional email alias (TBD or omitted if no maintainer alias exists). Explicitly tells researchers NOT to open a public issue.
2. `## What to expect` — response-time expectations. Per spec assumption: "acknowledge within 7 days, fix or status update within 30."
3. `## Supported versions` — per spec assumption: "Only the most recent alpha release is supported. mikebom is pre-1.0; SemVer guarantees do not apply yet."
4. `## Scope` — what counts as a vulnerability for this project (e.g., crashes during scan, SBOM emission that misrepresents the trace, etc.) vs what doesn't (e.g., upstream CVEs in dependencies — those go through normal advisories).

## `CONTRIBUTING.md` — section structure

Five sections, ~80-120 lines total:

1. `## Welcome` — one-paragraph framing. Pre-1.0 alpha; coordination encouraged before non-trivial PRs.
2. `## Workflow overview (the speckit lifecycle)` — explains specify → clarify → plan → tasks → analyze → implement. Links to `.claude/skills/speckit-*/SKILL.md` for full details. Notes that small drive-by fixes (typo corrections, doc tweaks) don't need the full lifecycle.
3. `## Local development setup` — Rust stable toolchain, `cargo +stable build`, eBPF caveats deferred to `docs/user-guide/installation.md`.
4. `## Pre-PR gate (MANDATORY)` — the exact commands from CLAUDE.md: `./scripts/pre-pr.sh` must exit clean (zero clippy warnings, all test suites `0 failed`). Includes the SPDX-3 validator opt-in (`MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1`) for spec-conformance-touching changes.
5. `## Project principles + where to find them` — points at `.specify/memory/constitution.md` as the source-of-truth. Lists the 12 principles by name with one-line gloss each, so contributors can audit their PR against the right ones.

## `.github/ISSUE_TEMPLATE/bug_report.yml` — form schema

YAML issue form. Required fields:

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| Bug title | `title` (auto-populated to PR title field) | yes | Format hint: "[BUG] short description" |
| Operating system | `dropdown` | yes | Options: `linux-x86_64`, `linux-aarch64`, `macOS aarch64`, `macOS x86_64 (Intel)`, `Windows`, `Other` |
| mikebom version | `input` | yes | Hint: "Output of `mikebom --version`" |
| Command + output | `textarea` | yes | Hint: "Paste the command you ran and the full output" |
| Expected vs actual | `textarea` | yes | Hint: "What did you expect to happen? What actually happened?" |
| Additional context | `textarea` | no | Hint: "Anything else? Screenshots, fixtures, related issues..." |

## `.github/ISSUE_TEMPLATE/feature_request.yml` — form schema

YAML issue form. Required fields:

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| Feature title | `title` | yes | Format hint: "[FEATURE] short description" |
| Use case | `textarea` | yes | Hint: "What are you trying to accomplish? Why?" |
| Proposed solution | `textarea` | yes | Hint: "How would you like mikebom to handle this?" |
| Alternatives considered | `textarea` | no | Hint: "What else have you tried or considered?" |
| Additional context | `textarea` | no | Hint: "Links, related work, anything else?" |

## `.github/ISSUE_TEMPLATE/config.yml` (optional) — chooser config

`blank_issues_enabled: false` so all reports go through one of the structured templates. Optional `contact_links` to point at `/discussions` or similar if those are enabled.

## `.github/pull_request_template.md` — checklist

Markdown body. Five checklist items per spec FR-005 + the spec's PR-template-length edge case:

```markdown
## Summary

<!-- 1-3 sentences explaining what this PR does and why. -->

## Test plan

<!-- Bulleted list. -->

## Pre-PR checklist

- [ ] I ran `./scripts/pre-pr.sh` and it exited clean (zero clippy warnings, all test suites `0 failed`).
- [ ] For non-trivial changes, I followed the speckit lifecycle (`specs/<NNN>-<short-name>/` exists with spec/plan/tasks).
- [ ] If I touched SBOM emission or output formats, I regenerated the affected byte-identity goldens.
- [ ] If I added a `mikebom:*` property, I audited each target format for an existing native construct first (Constitution Principle V).
- [ ] If this is a release-bump PR, I ran the SPDX-3 conformance validator via `MIKEBOM_REQUIRE_SPDX3_VALIDATOR=1 ./scripts/pre-pr.sh`.
```

## `mikebom-cli/Cargo.toml` — metadata-only addition

Append a `[package.metadata.binstall]` table per research.md §1's resolution:

```toml
[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/v{ version }/{ name }-v{ version }-{ target }{ archive-suffix }"
pkg-fmt = "tgz"
```

This is metadata-only (no `dependencies` change, no version bump, no feature flags). Cargo treats unknown `package.metadata.*` keys as opaque pass-through; this change cannot break the build.

## Compatibility

- **Render targets**: every new Markdown file renders correctly in (a) GitHub web UI, (b) the rust-docs Markdown parser (relevant for any doc-comment cross-references), (c) standard `cat`-style viewing.
- **Backward compatibility**: 100% additive. Anyone who previously installed mikebom by following the broken README:238 instruction was failing already; the fix is strict improvement.
- **Cargo.lock**: the metadata-only Cargo.toml change does NOT modify `Cargo.lock`. Diff scope audit (FR-009) should show `Cargo.lock` unchanged.

## No JSON / no YAML data model

This milestone introduces no JSON shapes, no SBOM emission changes, no API contracts. The only YAML files added are GitHub issue-form schemas, whose structure is GitHub's responsibility, not ours.
