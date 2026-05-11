# Feature Specification: Repo polish — quick-wins cleanup pass

**Feature Branch**: `093-repo-polish-quickwins`
**Created**: 2026-05-10
**Status**: Draft
**Input**: User description: "let's do your suggestion" — bundle the small repo-polish items as a single PR before tackling Homebrew tap + curl-pipe installer in a follow-up.

## Background

The mikebom repo is now at alpha.30 with a working cross-platform release pipeline (closing the milestone-090 build.rs gap shipped today). A casual survey of the repo against typical OSS-project-polish expectations surfaced five small gaps that together degrade the new-visitor experience:

1. **Broken git URL in README:238** — `git clone https://github.com/mlieberman85/mikebom.git` points at a stale personal-account location; the canonical repo is `kusari-sandbox/mikebom`. Anyone copy-pasting the install instructions hits a 404.
2. **No `SECURITY.md`** — important signal for a security tool. GitHub surfaces this in the "Security" tab and links it from the vulnerability disclosure page.
3. **No `CONTRIBUTING.md`** — onboarding new contributors. The CLAUDE.md mandatory pre-PR gate (`./scripts/pre-pr.sh`) and the speckit lifecycle (specify → plan → tasks → analyze → implement) aren't documented for external contributors.
4. **No issue / PR templates** — `.github/ISSUE_TEMPLATE/` and `.github/pull_request_template.md` funnel reports into structured shapes (bug report vs feature request vs question) and remind PR authors to confirm pre-PR-gate compliance.
5. **`cargo binstall` works but isn't documented** — the release artifact naming (`mikebom-vX.Y.Z-<arch>-<os>.tar.gz`) already matches `cargo binstall`'s auto-discovery convention. Adding one README line gives Rust-toolchain users a one-command install path without needing the multi-step `gh release download` dance.

This is a low-risk, documentation-and-metadata-only milestone — zero production code changes, zero golden churn, no Constitution-V concerns. Designed as one immediate cleanup PR (~1 hour of work) ahead of a separate, larger Homebrew tap + curl-pipe installer milestone.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — New user follows the README install instructions and they actually work (Priority: P1)

A new operator visits the mikebom GitHub page, scrolls to the Install section, and copy-pastes the recommended commands. Every URL and binary name resolves successfully; the install completes without manual fix-ups.

**Why this priority**: First-touch friction kills alpha adoption. A broken `git clone` URL on the canonical install path is the highest-blast-radius polish gap — 100% of visitors who try the source-build path hit it.

**Independent Test**: copy-paste each command block from README's Install section in a fresh shell. Confirm every URL resolves (no 404s) and every command completes successfully on at least one of: linux-x86_64, macOS aarch64.

**Acceptance Scenarios**:

1. **Given** a visitor on the README at the Install section, **When** they run `git clone https://github.com/kusari-sandbox/mikebom.git`, **Then** the clone succeeds. The README MUST NOT reference `mlieberman85/mikebom`.
2. **Given** a Rust-toolchain user, **When** they read the Install section, **Then** they see `cargo binstall mikebom` (or equivalent) documented as an alternative install path with a one-line callout.

---

### User Story 2 — Visitor reads SECURITY.md and knows how to report a vulnerability (Priority: P2)

A security researcher discovers a vulnerability in mikebom and looks for the project's disclosure process. They find a `SECURITY.md` in the repo root that names a contact channel, the response-time expectation, and the supported-versions policy. GitHub's "Security" tab automatically surfaces this file.

**Why this priority**: mikebom is itself a supply-chain security tool — operators expect it to demonstrate the security hygiene it advocates. Visible-and-credible vulnerability disclosure is table stakes for adoption by security-conscious teams.

**Independent Test**: open the repo's GitHub Security tab. Confirm the "Security policy" entry now points at `SECURITY.md` instead of being absent. Read the file end-to-end and confirm it answers: where to report, what to expect, which versions are in scope.

**Acceptance Scenarios**:

1. **Given** a researcher who found a vulnerability, **When** they search the repo for "SECURITY.md", **Then** they find a populated file naming a private disclosure channel (GitHub Security Advisory and/or email alias), a response-time expectation, and the supported-versions policy.
2. **Given** the GitHub Security tab, **When** the visitor lands on it, **Then** it shows a "Security policy" entry linking to the new `SECURITY.md` (GitHub auto-detects this file when present at repo root).

---

### User Story 3 — External contributor reads CONTRIBUTING.md and knows the workflow (Priority: P2)

A new contributor wants to submit a bug fix or feature PR. They find a `CONTRIBUTING.md` at the repo root that explains the speckit-driven workflow, the mandatory pre-PR gate (`./scripts/pre-pr.sh`), the constitution location, and the local development setup. They don't have to reverse-engineer the project's expectations from commit messages or CI configuration.

**Why this priority**: The project follows a non-obvious workflow (speckit specify → plan → tasks → analyze → implement) that external contributors won't know about. Without docs, the first PR-review cycle is spent explaining the pre-PR gate, the spec-first convention, the no-`.unwrap()`-in-production rule, etc.

**Independent Test**: read `CONTRIBUTING.md` end-to-end as someone unfamiliar with the project. Confirm it answers: how do I run the tests? what do I need to do before opening a PR? where is the constitution? how does the speckit lifecycle work? does the project accept drive-by contributions or require coordination first?

**Acceptance Scenarios**:

1. **Given** a potential contributor reads `CONTRIBUTING.md`, **When** they finish, **Then** they know: (a) the speckit lifecycle is the expected workflow for non-trivial changes, (b) `./scripts/pre-pr.sh` is the mandatory local gate, (c) `.specify/memory/constitution.md` is the source-of-truth for project principles, and (d) where to find the per-spec planning artifacts under `specs/`.
2. **Given** a contributor about to open a PR, **When** they re-read CONTRIBUTING.md's pre-PR section, **Then** the exact commands match what CI runs (clippy `--workspace --all-targets -D warnings` + `cargo test --workspace`).

---

### User Story 4 — Bug reporters and feature requesters file structured issues (Priority: P3)

A user finds an unexpected scan result and clicks "New issue". GitHub offers them a choice between "Bug report", "Feature request", and "Question / discussion" templates, each with a pre-filled structure that prompts for the information maintainers need. PR authors see a checklist that reminds them to confirm the pre-PR gate ran clean.

**Why this priority**: structured templates reduce the back-and-forth on every issue ("what OS?", "which version?", "what command did you run?"). Lower priority than US1–US3 because the project is alpha and issue volume is low.

**Independent Test**: click "New issue" in the repo's GitHub UI. Confirm the template chooser appears with at least 2 distinct templates. Open a fake bug-report draft and confirm it prompts for OS / version / reproduction steps. Open a fake PR draft and confirm the template appears with a pre-PR-gate checkbox.

**Acceptance Scenarios**:

1. **Given** a user on the "New issue" page, **When** they click the button, **Then** GitHub presents at least 2 templates: "Bug report" (prompts for: OS, mikebom version, command + output, expected vs actual) and "Feature request" (prompts for: use case, proposed solution, alternatives considered). Optional third template: "Question / discussion".
2. **Given** a contributor opening a PR, **When** the PR form loads, **Then** the body is pre-filled with a checklist that includes "I ran `./scripts/pre-pr.sh` clean locally" and "I followed the speckit lifecycle for non-trivial changes (or this is a trivial change)".

---

### User Story 5 — Rust-toolchain user installs mikebom with one command via cargo binstall (Priority: P3)

A user already running `cargo` runs `cargo binstall mikebom` and gets the latest pre-release tarball downloaded, verified against the SHA256SUMS, and installed to their cargo bin directory — no manual `gh release download` dance, no Homebrew dependency, no curl-pipe trust ceremony.

**Why this priority**: `cargo binstall` already works today (mikebom's release artifact naming matches its auto-discovery convention as of alpha.30); the gap is purely documentation. Lowest priority because it requires the user to already have `cargo binstall` installed, which is itself an extra step. But it's effectively free to surface.

**Independent Test**: on a host with `cargo binstall` installed, run `cargo binstall mikebom` (with any documented flags). Confirm the alpha.30 binary downloads, the SHA256 verifies, and `mikebom --version` reports `0.1.0-alpha.30`.

**Acceptance Scenarios**:

1. **Given** the README Install section, **When** a Rust-toolchain reader scans it, **Then** they see `cargo binstall` mentioned as an option, with the exact command needed and any required flags (e.g., `--pkg-url`, `--pkg-fmt`) to make auto-discovery work against the existing release-tarball naming.
2. **Given** a user runs the documented `cargo binstall mikebom` command, **When** it completes, **Then** the binary is installed in `~/.cargo/bin/mikebom` and runs successfully.

---

### Edge Cases

- **Repo gets transferred or renamed in the future**: the README install command should not embed an account name in a non-templated way. Fix: use the actual canonical repo location at time of writing (`kusari-sandbox/mikebom`); if it ever moves, a single search-and-replace updates everywhere.
- **`SECURITY.md` private-disclosure channel changes**: contact alias might rotate. Document the channel in `SECURITY.md` but keep the file short enough that updates are cheap.
- **GitHub auto-detection of `SECURITY.md` location**: the file MUST be at the repo root (or `.github/SECURITY.md`, or `docs/SECURITY.md`) for GitHub's Security-tab integration to surface it. Stick to repo root for maximum discoverability.
- **PR template length**: too long and contributors skip it; too short and it doesn't help. Target ~5 checklist items.
- **`cargo binstall` auto-discovery vs explicit `--pkg-url`**: if the release-tarball naming requires explicit flags, the README example MUST include them — silent failure ("cargo binstall mikebom" with no flags producing "no matching binary found") is worse than no mention at all.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: README MUST replace the stale `mlieberman85/mikebom.git` URL at line 238 with the canonical `kusari-sandbox/mikebom.git`. A repo-wide grep for `mlieberman85` MUST return zero hits in committed content (excluding historical git log entries).
- **FR-002**: A `SECURITY.md` MUST exist at the repo root. It MUST include: (a) a named private disclosure channel (GitHub Security Advisory link and/or email alias), (b) an explicit response-time expectation (e.g., "acknowledge within 7 days, fix or status update within 30"), (c) the supported-versions policy (e.g., "only the most recent alpha is supported"), (d) what NOT to do (e.g., "please don't open a public issue for vulnerabilities").
- **FR-003**: A `CONTRIBUTING.md` MUST exist at the repo root. It MUST explain: (a) the speckit lifecycle for non-trivial changes (link to existing speckit docs), (b) the mandatory pre-PR gate (`./scripts/pre-pr.sh`) with the exact commands, (c) the project constitution location (`.specify/memory/constitution.md`), (d) the local development setup (`cargo +stable build`), (e) the per-spec planning artifacts location (`specs/<NNN>-<short-name>/`).
- **FR-004**: A `.github/ISSUE_TEMPLATE/` directory MUST contain at least 2 issue templates: `bug_report.md` (or `.yml`) prompting for OS / mikebom version / reproduction steps / expected vs actual, and `feature_request.md` (or `.yml`) prompting for use case / proposed solution / alternatives. A third "Question / discussion" template is optional.
- **FR-005**: A `.github/pull_request_template.md` MUST exist. It MUST include a checklist with at minimum: (a) "I ran `./scripts/pre-pr.sh` clean locally", (b) "I followed the speckit lifecycle for non-trivial changes (or this is a trivial change)", (c) "I updated any goldens that needed regen".
- **FR-006**: The README's `## Install` section MUST add a `cargo binstall` install path with the exact command + any required flags so `cargo binstall mikebom` works against the existing release-tarball naming. If the naming requires updating release metadata in `Cargo.toml` to enable cargo-binstall's auto-discovery (a `[package.metadata.binstall]` block), that metadata MUST be added in this milestone.
- **FR-007**: This milestone MUST NOT touch production Rust code paths (`mikebom-cli/src/`, `mikebom-common/src/`, `xtask/src/`). Allowed file types: `*.md` at repo root or under `.github/`, `Cargo.toml` metadata-only additions (e.g., `[package.metadata.binstall]`), README content.
- **FR-008**: This milestone MUST NOT regenerate any byte-identity goldens. `git status mikebom-cli/tests/fixtures/golden/` MUST be empty after the polish PR's changes are applied.
- **FR-009**: This milestone MUST NOT add new Cargo dependencies. `Cargo.lock` MUST be unchanged (except for non-version-changing field migrations the cargo tool emits on touch, if any).
- **FR-010**: The mandatory pre-PR gate (`./scripts/pre-pr.sh`) MUST continue to pass post-changes — clippy zero warnings, all test suites `0 failed`.

### Key Entities

- **`README.md`**: the canonical landing-surface for new visitors. Touched in FR-001 (URL fix) and FR-006 (cargo-binstall path).
- **`SECURITY.md`** (NEW): the vulnerability disclosure surface. Touched in FR-002.
- **`CONTRIBUTING.md`** (NEW): the contributor onboarding surface. Touched in FR-003.
- **`.github/ISSUE_TEMPLATE/*.md`** (NEW): bug-report + feature-request templates. Touched in FR-004.
- **`.github/pull_request_template.md`** (NEW): PR checklist. Touched in FR-005.
- **`Cargo.toml`** (potentially touched): optional metadata-only addition for `cargo binstall` auto-discovery (no version bump, no dependency change). Touched in FR-006 only if needed.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of README install commands resolve and complete successfully on a fresh checkout. Specifically, the `git clone` URL at the new README:238 location returns the cloned repo (HTTP 200 or successful clone exit code), not a 404.
- **SC-002**: GitHub's Security tab surfaces a "Security policy" entry linking to `SECURITY.md` post-merge. (Validation: visit the rendered Security tab; confirm the entry appears without manual configuration — GitHub auto-detects files at the supported paths.)
- **SC-003**: A first-time contributor reading `CONTRIBUTING.md` end-to-end (~10 minutes) can answer four questions correctly without consulting other docs: (a) "What command do I run before opening a PR?", (b) "Where is the project constitution?", (c) "Where do speckit planning artifacts live?", (d) "Do I need to coordinate before opening a non-trivial PR?".
- **SC-004**: Clicking "New issue" in the GitHub UI presents a template chooser with at least 2 distinct templates (bug + feature). Each template's pre-filled body contains all the prompts named in FR-004.
- **SC-005**: On a host with `cargo binstall` installed, running the documented `cargo binstall mikebom` command (with any required flags) installs the alpha.30 binary in `~/.cargo/bin/` within 60 seconds, with SHA256 verification passing.
- **SC-006**: Pre-PR gate runs clean post-changes: `./scripts/pre-pr.sh` reports `>>> all pre-PR checks passed.` with zero clippy warnings and zero test failures across the workspace.
- **SC-007**: Diff scope audit: 100% of changed files match one of {`README.md`, `SECURITY.md`, `CONTRIBUTING.md`, `.github/**.md`, `.github/**.yml`, `Cargo.toml` metadata-only}. Zero changes under `mikebom-cli/src/`, `mikebom-common/src/`, `xtask/src/`, `mikebom-cli/tests/fixtures/golden/`, `Cargo.lock` (except metadata-touch propagation).

## Assumptions

- The canonical repo location is `kusari-sandbox/mikebom` (verified from `gh pr create` outputs earlier in the session, e.g., `https://github.com/kusari-sandbox/mikebom/pull/190`). No imminent transfer plans known.
- The private disclosure channel for `SECURITY.md` is GitHub Security Advisories (the most low-friction option for a small project; no email infrastructure required). A maintainer-named email alias can be added later if needed.
- The supported-versions policy is "only the most recent alpha is supported" (matches current pre-1.0 reality; tightens to a real SemVer policy at 1.0).
- `cargo binstall` auto-discovery works against mikebom's existing release-tarball naming (`mikebom-vX.Y.Z-<arch>-<os>.tar.gz`) with at most a `[package.metadata.binstall.overrides]` block in `Cargo.toml`. If discovery fails outright, the README documents the explicit `--pkg-url` + `--pkg-fmt` invocation as a workaround.
- The speckit lifecycle docs already exist in `.claude/skills/` (verified earlier in the session) and `CONTRIBUTING.md` can link to them rather than re-explain end-to-end.
- The constitution at `.specify/memory/constitution.md` is the canonical principles source (verified — used during milestone-092's Constitution Check). `CONTRIBUTING.md` links to it rather than excerpts.
- The repo is OK without a `CODE_OF_CONDUCT.md` for this milestone (deliberately deferred — adds maintenance footprint without proportional alpha-stage value). Can be added in a follow-up if community grows.
- This is a docs/metadata-only PR; no release version bump required. The next operator-facing change ships in a future milestone's release.

## Dependencies

- The repo's GitHub release-tarball naming MUST already match `cargo binstall` auto-discovery convention. Verified: alpha.30 artifacts use `mikebom-v0.1.0-alpha.30-aarch64-apple-darwin.tar.gz` etc. (per earlier `gh release view` output).
- The mandatory pre-PR gate script (`./scripts/pre-pr.sh`) is in place and known to work on this branch (used throughout milestone 092).
- `.github/workflows/` directory exists (CI runs `ci.yml`, `release.yml`, `realistic-projects.yml`); adding sibling `.github/ISSUE_TEMPLATE/` and `.github/pull_request_template.md` is purely additive.

## Out of Scope

- **Homebrew tap creation** (`kusari-sandbox/homebrew-tap` repo). Deferred to a separate follow-up milestone — requires a new repo, formula maintenance discipline, and an auto-bump step in `release.yml`. Out of scope here to keep the polish PR small.
- **`install.sh` curl-pipe installer** + hosting at a stable URL. Same rationale as above — a small additional milestone of its own.
- **Shell completions** (`mikebom completions <shell>` subcommand via `clap_complete`). Out of scope — adds a production code path which violates FR-007.
- **Man pages** via `clap_mangen`. Same rationale as completions.
- **crates.io publishing** of mikebom. Out of scope — requires Cargo.toml metadata polish, first-publish ceremony, and ongoing release-cycle integration; should be its own milestone.
- **Docker image** in release.yml. Out of scope — requires Dockerfile maintenance + image-naming policy + an additional release-yml job. Separate milestone.
- **deb / rpm packages** via cargo-deb / cargo-generate-rpm. Out of scope — same rationale as Docker.
- **`CODE_OF_CONDUCT.md`**. Deferred per the Assumptions section — alpha-stage maintenance footprint cost > current value. Revisit when community grows.
- **Renaming the canonical repo**, transferring ownership, or changing the GitHub org. Not happening in this milestone.
