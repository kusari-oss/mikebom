# Research — milestone 093 repo polish

Phase 0 investigation. Three decision points; all resolved without further clarification.

## §1 — cargo-binstall discovery + the crates.io gap

**Findings**:

`cargo binstall <name>` resolves `<name>` via the crates.io index. mikebom is NOT yet published to crates.io (per README's "Status: pre-1.0 alpha. ... no crates.io release yet"), so plain `cargo binstall mikebom` fails at lookup. The Git source workaround is supported via `--git`:

```
cargo binstall --git https://github.com/kusari-sandbox/mikebom mikebom
```

Default `pkg-url` autodiscovery joins:

- Release-path prefix: `{repo}/releases/download/{version}/` then `{repo}/releases/download/v{version}/`
- Filename patterns including `{name}-{target}-v{version}{archive-suffix}` and `{name}-v{version}-{target}{archive-suffix}` and others
- `target` = **full Rust triple** (e.g. `aarch64-apple-darwin`, not short form)
- `version` token = bare (`0.1.0-alpha.30`); the `v` prefix is literal in templates
- Default `pkg-fmt` = `tgz`
- Inside archive: searches `{name}-{target}-v{version}/`, …, down to root, expecting binary `{name}{binary-ext}`. No `bin/` subdir needed.

mikebom's tarball naming `mikebom-v0.1.0-alpha.30-aarch64-apple-darwin.tar.gz` matches the `{name}-v{version}-{target}{archive-suffix}` fallback in binstall's default search list, so discovery WILL find it. But pinning it explicitly avoids any future drift if binstall's defaults change.

**Decision**: ship both halves of the fix —

(a) Add a deterministic `[package.metadata.binstall]` block to `mikebom-cli/Cargo.toml` so the URL template is locked to the existing release-tarball naming:

```toml
[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/v{ version }/{ name }-v{ version }-{ target }{ archive-suffix }"
pkg-fmt = "tgz"
```

(b) In the README's Install section, document the `--git` invocation (the crates.io workaround) until mikebom is published. Phrase it as "optional alternative, useful for Rust-toolchain users":

```bash
# alternative install path (skip the gh-release download dance):
cargo binstall --git https://github.com/kusari-sandbox/mikebom mikebom
```

**Rationale**:
- The Cargo.toml block is metadata-only (FR-007 compliant), zero new deps (FR-009), zero golden churn (FR-008).
- Documenting `--git` instead of bare `cargo binstall mikebom` is honest about the current crates.io gap — silent failure ("not found in registry") is worse than no mention.
- When mikebom is eventually published to crates.io (separate future milestone per Out of Scope), the documented invocation simplifies to `cargo binstall mikebom` with no other changes needed — the metadata block keeps working unchanged.

**Alternatives considered**:
- **Skip the metadata block, rely on default discovery**: works today but vulnerable to future binstall default-template changes. Rejected as fragile.
- **Publish to crates.io as part of this milestone**: out of scope per spec (separate milestone). Crates.io publish is irreversible name-squat + ongoing release-cycle integration; needs its own constitution check and pre-PR ceremony.
- **Skip cargo-binstall entirely, defer to Homebrew tap milestone**: leaves Rust-toolchain users with no one-command option until that lands. Rejected because the metadata block is ~5 lines and the README addition is ~3 lines — extremely low cost for non-zero adoption value.

## §2 — SECURITY.md location

**Findings**: GitHub auto-detects `SECURITY.md` at three paths: `.github/SECURITY.md`, repo-root `SECURITY.md`, `docs/SECURITY.md`. Precedence (observed): `.github/` > root > `docs/`. The same precedence chain falls back to the org-level `.github` repo's defaults if the project's own version is absent.

**Decision**: place `SECURITY.md` at **repo root** (not `.github/SECURITY.md`).

**Rationale**:
- Maximum human discoverability — `SECURITY.md` at root is what visitors see when browsing the repo directly (alongside README, CONTRIBUTING, LICENSE).
- GitHub's Security-tab integration works equally well from any of the three paths; the precedence only matters if multiple exist, which we don't have.
- Mirrors the precedent of every other top-level community-health file in this milestone (CONTRIBUTING.md, README.md are already at root).

**Alternatives considered**:
- **`.github/SECURITY.md`**: technically takes precedence but obscures the file from anyone browsing without expanding `.github/`. Rejected for discoverability.
- **`docs/SECURITY.md`**: lowest precedence; also less conventional. Rejected.

## §3 — Issue template format

**Findings**: GitHub supports two formats — classic Markdown templates (`.github/ISSUE_TEMPLATE/*.md` with YAML front-matter) and YAML issue forms (`.github/ISSUE_TEMPLATE/*.yml`). YAML forms render as structured UI with required-field enforcement, dropdowns, checkboxes; classic Markdown renders the body as-is and contributors can delete sections by accident.

**Decision**: use **YAML issue forms** (`.github/ISSUE_TEMPLATE/bug_report.yml` + `.github/ISSUE_TEMPLATE/feature_request.yml`).

**Rationale**:
- Required-field enforcement guarantees the maintainer-needed inputs (OS, mikebom version, command + output, expected vs actual for bug reports) actually get filled in — Markdown templates contributors routinely strip section headings.
- Dropdowns let us pre-enumerate the supported platforms (linux-x86_64, linux-aarch64, macOS aarch64) so we get clean data, not free-form OS strings.
- No ongoing maintenance cost vs Markdown templates — write once, GitHub renders the form forever.
- The YAML schema is well-documented; portability concerns are minimal because if the repo ever migrates off GitHub the templates would need rewriting either way.

**Alternatives considered**:
- **Classic Markdown templates**: portable across platforms (GitLab, Gitea also support them) but the spec is GitHub-specific and the small-OSS portability win is theoretical. Rejected.
- **`config.yml` with external links only** (no templates, just a chooser pointing at e.g. discussions): doesn't capture bug-report structure. Rejected for FR-004 noncompliance.

## §4 — PR template format

**Findings**: GitHub supports a single `.github/pull_request_template.md` (no PR-form equivalent). Multiple templates are possible via query-string selection but require maintainer effort per category and offer marginal value for a small project.

**Decision**: single `.github/pull_request_template.md` with a Markdown checklist body. ~5 checklist items per spec FR-005.

**Rationale**:
- Single template = zero choice-friction for the contributor. Click "New PR", template populates.
- Checklist format (`- [ ]`) renders as interactive checkboxes in the GitHub UI — contributors can tick them as they verify each item.
- ~5 items hits the spec's edge-case target ("too long and contributors skip it; too short and it doesn't help").

**Alternatives considered**:
- **Multiple PR templates via query-string**: maintenance cost > value for an alpha project. Rejected.

## Coverage map

| Spec section | Resolution |
|--------------|------------|
| FR-001 (README URL fix) | trivial; covered in quickstart Recipe 1. No research needed. |
| FR-002 (SECURITY.md presence + content) | §2 → repo root; content shape covered in data-model + contracts. |
| FR-003 (CONTRIBUTING.md presence + content) | content shape covered in data-model + contracts. No research needed. |
| FR-004 (issue templates) | §3 → YAML forms; bug_report.yml + feature_request.yml. |
| FR-005 (PR template) | §4 → single Markdown checklist. |
| FR-006 (cargo binstall documented + auto-discoverable) | §1 → README mentions `--git` invocation; Cargo.toml metadata block locks discovery. |
| FR-007 (no production code) | reinforced by §1's decision to keep Cargo.toml change to metadata-only. |
| FR-008 (no golden regen) | implied — no source change emits goldens. |
| FR-009 (no new deps) | implied — no `dependencies` table modified. |
| FR-010 (pre-PR gate clean) | covered in quickstart Recipe 6. |
| Constitution V audit | no `mikebom:*` properties; trivially satisfied. |

All open spec questions resolved. Ready for Phase 1 (data-model + contracts + quickstart).
