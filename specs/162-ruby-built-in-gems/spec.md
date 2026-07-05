# Feature Specification: Ruby built-in gem edges surfaced as SBOM components (fix + regression guard)

**Feature Branch**: `162-ruby-built-in-gems`
**Created**: 2026-07-04
**Status**: Draft
**Input**: User description: "496" (implement fix for [issue #496](https://github.com/kusari-oss/mikebom/issues/496))

## Motivation

Discovered during the milestone-157 Round-2 audit against `kusari-sandbox/test-rails`: when a `Gemfile.lock` GEM/specs entry declares a dep to a Ruby **built-in gem** (`bundler`, `bigdecimal`, `csv`, `logger`, `openssl`, `psych`, `rss`, `stringio`, etc. — the gems that ship with Ruby itself and are installed via a special mechanism, NOT via `gem install`), mikebom **silently drops the edge**. The dep-name is emitted as a target in the graph resolver's edge set, but no matching component exists in the SBOM's `components[]` array (built-in gems don't appear in `Gemfile.lock`'s GEM/specs section — they're pre-installed with Ruby). The graph resolver's dangling-target-drop pass at emission time removes the edge without any operator-visible signal.

Concrete reproduction from `test-rails`:

```text
GEM
  specs:
    bundler-audit (0.9.3)
      bundler (>= 1.2.0)     # ← edge target: `bundler` — Ruby built-in
      thor (~> 1.0)          # ← edge target: `thor` — normal gem, in GEM/specs

# mikebom emits:
jq '.dependencies[] | select(.ref == "pkg:gem/bundler-audit@0.9.3") | .dependsOn'
# → ["pkg:gem/thor@1.4.0"]
# → MISSING: pkg:gem/bundler@...
```

Empirical measurement 2026-07-03 on `test-rails` (250 total Gemfile.lock edges):

| shape | count | % |
|---|---|---|
| EXACT-MATCH | 248 | 99.60% |
| EMITTED-SUBSET (mikebom MISSING edges) | 1 | 0.40% |
| DIVERGE / EMITTED-SUPERSET / not-emitted | 0 | 0% |

Numerically low (0.40%) but the failure mode is **silent** and semantically load-bearing:

- A security-analysis tool consuming the SBOM would NOT see that `bundler-audit` depends on a specific `bundler` version.
- A supply-chain audit tool doing "what Ruby gems does this project depend on" would report incomplete data with no way to detect the incompleteness.
- Vulnerability scans against `bundler`-related CVEs (there have been several) would miss the affected components.

Consumer impact: Constitution Principle VIII (Completeness) failure — mikebom drops legitimate edges without signaling the drop.

## Distinction from #494 (milestone 160) and #495 (milestone 161)

- **Milestone 160** (#494): fixed **missing** transitive edges due to proxy-fetch degradation.
- **Milestone 161** (#495): fixed **wrong** edges due to workspace-root leakage into leaf modules.
- **Milestone 162** (this issue, #496): fixes **silently dropped** edges to Ruby built-in gems whose components aren't in the SBOM because they're pre-installed with the Ruby toolchain.

All three are complementary but non-overlapping.

## User Scenarios & Testing

### User Story 1 - SBOM consumer sees the `bundler-audit → bundler` edge (Priority: P1)

An SBOM consumer (Kusari Inspector, a vulnerability scanner, a supply-chain audit tool) loads mikebom's Ruby SBOM for a repo scanned with `--path <repo>` and finds that edges to Ruby built-in gems (`bundler`, `bigdecimal`, `csv`, `logger`, `openssl`, `psych`, `rss`, `stringio`, `json`, `uri`, `net-http`, `date`, `time`, etc.) are represented — either via emitted synthetic components with a special annotation OR via edge-source annotations naming the unresolved dep-name. The consumer can programmatically detect these built-in edges and decide how to handle them (e.g., trust the Ruby-toolchain-managed gem OR flag as needing separate CVE lookup).

**Why this priority**: This is the observed bug's user-visible symptom. Without this fix, mikebom's Ruby SBOMs silently drop 0.4% of edges. Numerically small but the SILENCE is the problem — Constitution Principle X (Transparency) violation. Any consumer downstream of the SBOM has no way to know that edges were dropped.

**Independent Test**: Scan `kusari-sandbox/test-rails` with mikebom. Assert:

- The 1 concrete missing edge from the milestone-157 audit (`bundler-audit@0.9.3 → bundler`) is EITHER present as a real `dependsOn` edge to an emitted synthetic `pkg:gem/bundler` component, OR emitted with a `mikebom:unresolved-dep-name` annotation on the source component naming the target.
- No new false-positive vulnerability-scan matches created (any synthetic component MUST carry a version-unknown signal so exact-version CVE lookups don't return spurious matches).
- 100% of previously-EXACT-MATCH edges (248 of 250 pre-162) remain EXACT-MATCH post-162 (no regression on the correctly-emitted edges).

**Acceptance Scenarios**:

1. **Given** `test-rails` scanned via `mikebom sbom scan --path test-rails --format cyclonedx-json`, **When** enumerating `bundler-audit@0.9.3`'s outgoing `dependsOn`, **Then** the list MUST include a reference to `bundler` — either as `pkg:gem/bundler` (synthetic component emitted per Option A) OR via a `mikebom:unresolved-dep-name = "bundler"` annotation on `bundler-audit@0.9.3` (Option B).

2. **Given** the same scan, **When** enumerating the SBOM's `components[]` array, **Then** any component with PURL matching `pkg:gem/<built-in>` (e.g., `pkg:gem/bundler`) MUST carry `mikebom:synthetic-built-in = "ruby"` annotation so vulnerability-scan consumers can distinguish real component evidence from synthetic-fill entries.

3. **Given** the same scan, **When** enumerating the 248 pre-162 EXACT-MATCH edges, **Then** ALL 248 remain EXACT-MATCH — no regression on correctly-emitted edges.

4. **Given** a repo with NO Ruby components (`test-podman`, `test-kubernetes`, any non-Ruby fixture), **When** mikebom scans, **Then** the emitted SBOM MUST be byte-identical to pre-162 (Ruby-scoped fix; SC-003 dual-side byte-identity guard).

---

### User Story 2 - Consumers can programmatically distinguish synthetic from real gem components (Priority: P2)

A compliance auditor loads a mikebom Ruby SBOM and wants to know: which components represent gems actually resolved from a Gemfile.lock (real evidence), vs which are synthetic placeholders for Ruby-toolchain built-in gems that mikebom inferred from dropped edges (synthetic fill). Programmatic detection is critical — vulnerability scanners typically map PURL → CVE lists, and a synthetic component with no version could false-positive OR silently omit real CVEs.

**Why this priority**: Constitution Principle IX (Accuracy) + Principle X (Transparency). Consumers must be able to trust that a component in the SBOM was actually observed via Gemfile.lock evidence — or, when it's a synthetic fill, that fact must be surfaced explicitly.

**Independent Test**: For every emitted SBOM containing at least one `pkg:gem/` component, assert:

- Every non-synthetic gem component (PURL from GEM/specs) has NO `mikebom:synthetic-built-in` annotation.
- Every synthetic gem component (PURL for a Ruby built-in) HAS `mikebom:synthetic-built-in = "ruby"` annotation.
- Zero synthetic components carry a `@version` PURL suffix (versionless PURL per data-model.md — vulnerability scanners doing exact-version match won't false-positive).

**Acceptance Scenarios**:

1. **Given** `test-rails` scanned, **When** enumerating `pkg:gem/bundler` (synthetic), **Then** it MUST carry `mikebom:synthetic-built-in = "ruby"` AND its PURL MUST NOT contain `@<version>` (versionless PURL).

2. **Given** the same scan, **When** enumerating `pkg:gem/thor@1.4.0` (real, from GEM/specs), **Then** it MUST NOT carry `mikebom:synthetic-built-in` annotation AND its PURL MUST contain the `@1.4.0` version segment.

3. **Given** a repo with no `Gemfile.lock` (a non-Ruby repo), **When** mikebom scans, **Then** neither synthetic components nor the annotation appear.

---

### User Story 3 - Non-Ruby scans byte-identical to pre-162 (Priority: P3)

Users scanning repos with NO Ruby components see byte-identical SBOM output vs. pre-162 milestones.

**Why this priority**: Regression guard. The synthetic-component emission + new annotation MUST be dormant when no Gemfile.lock is present. SC-003 dual-side byte-identity precedent (milestones 157–161).

**Independent Test**: Regenerate all non-Ruby milestone-090 goldens with the milestone-162 code. Diff against pre-162. Zero diff bytes on the 10 non-`gem` ecosystems × 3 formats = 30 goldens. The `gem` fixture goldens MAY change (new synthetic components + annotations expected there).

**Acceptance Scenarios**:

1. **Given** the milestone-090 npm fixture (no Ruby components), **When** mikebom scans, **Then** the emitted CDX diff vs. pre-162 is exactly ZERO bytes.

2. **Given** the milestone-090 `gem` fixture, **When** mikebom scans, **Then** the emitted CDX MUST contain the new `mikebom:synthetic-built-in` annotation on any synthetic built-in gem components AND the pre-162 goldens MAY change if the fixture's Gemfile.lock references Ruby built-in gems.

### Edge Cases

- **Repo with `Gemfile.lock` but no built-in gem references**: no synthetic components emitted; C113 annotation absent from every emitted gem component. Byte-identical to pre-162 emission for this file.

- **Same built-in gem name referenced by multiple sources** (e.g., 3 different gems declare `bundler` as a dep): emit ONE synthetic `pkg:gem/bundler` component with N incoming edges. Deduplication via the existing purl-based dedup at emission time.

- **Built-in gem name collision with a real gem from GEM/specs**: If `Gemfile.lock` HAS a `bundler` entry in GEM/specs (rare — real projects using non-built-in bundler), mikebom emits the real component with its version and does NOT emit a synthetic one. The real component takes precedence per FR-004.

- **New Ruby versions introduce new built-in gems**: The built-in gems list evolves across Ruby major versions (`csv` became built-in in Ruby 3.4; `net-http` and others are being extracted). mikebom's built-in gems list needs to be maintained per Ruby version. FR-006 documents this.

- **Ruby version detection**: mikebom does NOT probe the target's `ruby --version` — the built-in gems list is a static allowlist derived from Ruby-stdlib documentation. False-positive risk: mikebom emits a synthetic `pkg:gem/bundler` component when the target actually installed a non-built-in `bundler` via `gem install` but has no `Gemfile.lock` entry for it. Assumption §5 documents this trade-off.

- **Requirement string parsing** (`(>= 1.2.0)` in the example above): mikebom parses the requirement string per the existing Gemfile.lock parser and emits it as `mikebom:built-in-requirement = ">= 1.2.0"` per FR-005 so consumers can render "depends on bundler (>= 1.2.0, version resolution unavailable)."

## Requirements

### Functional Requirements

- **FR-001**: mikebom MUST maintain a static allowlist of Ruby built-in gems in `mikebom-cli/src/scan_fs/package_db/gem.rs` (or a sibling helper module). The list MUST include at minimum: `bundler`, `bigdecimal`, `csv`, `date`, `english`, `erb`, `fileutils`, `find`, `forwardable`, `getoptlong`, `io-console`, `ipaddr`, `irb`, `json`, `logger`, `mutex_m`, `net-http`, `open-uri`, `openssl`, `optparse`, `ostruct`, `pathname`, `pp`, `prime`, `psych`, `rdoc`, `resolv`, `rss`, `securerandom`, `set`, `singleton`, `stringio`, `strscan`, `tempfile`, `time`, `timeout`, `tmpdir`, `uri`, `weakref`, `yaml`. Sourced from Ruby 3.4's `Gem::default_gems` output (Ruby's own catalog of built-in gems).

- **FR-002**: When parsing `Gemfile.lock` at emission time, for each GEM/specs dep-name that does NOT match a GEM/specs entry (i.e., the graph resolver's dangling-target case), mikebom MUST check whether the dep-name matches the FR-001 built-in gems allowlist. If it does, mikebom MUST emit a synthetic `pkg:gem/<name>` component with `mikebom:synthetic-built-in = "ruby"` annotation AND `mikebom:built-in-requirement = "<original-requirement-string>"` annotation (per FR-005).

- **FR-003**: Synthetic built-in components MUST carry a **versionless PURL** (`pkg:gem/bundler` — no `@version` segment) per PURL spec's optional-version clause. Rationale: mikebom cannot know the actual installed version without probing `ruby --version`; emitting a fake version would create false-positive CVE matches at consumer time.

- **FR-004**: When the same gem name appears in BOTH the GEM/specs section AND as a dropped built-in edge target (rare — a real project using a non-built-in gem with the same name as a built-in), mikebom MUST emit the real component from GEM/specs (with its declared version) and MUST NOT emit a synthetic built-in component. Real evidence takes precedence.

- **FR-005**: mikebom MUST emit `mikebom:built-in-requirement = "<original-requirement-string>"` annotation on synthetic built-in components carrying the version constraint from the `Gemfile.lock` (e.g., `>= 1.2.0` from `bundler-audit`'s declaration). Consumers can render "depends on `bundler` (>= 1.2.0, version resolution unavailable)".

- **FR-006**: The FR-001 allowlist MUST be documented as static (not dynamically probed) with a maintenance note — new Ruby releases may introduce or extract built-in gems, so the list requires periodic review. The list is versioned in-source per the milestone number.

- **FR-007**: When a synthetic built-in component is emitted, the incoming edge from the source component (`bundler-audit@0.9.3 → pkg:gem/bundler`) MUST be preserved in the `dependencies[]` array — the whole point of the fix is that the edge stops being silently dropped.

- **FR-008**: mikebom MUST NOT emit synthetic components for gem names that are NOT in the FR-001 allowlist. Random dangling-target dep-names (a genuinely missing gem due to `Gemfile.lock` malformation) MUST continue to be dropped per existing behavior — this fix is scoped narrowly to the built-in gems case.

- **FR-009**: Standards-native precedence per Constitution Principle V. If either CDX 1.6 or SPDX 3.0.1 introduces an official "synthetic/inferred component" property, mikebom MUST prefer that property. As of 2026-07-04, no such standard property exists; the `mikebom:synthetic-built-in` prefix is used.

- **FR-010**: `mikebom:synthetic-built-in` and `mikebom:built-in-requirement` MUST be registered as new per-component parity-catalog rows (C113 + C114) with `Directionality::SymmetricEqual` — matching the milestone-158/159/160/161 pattern.

- **FR-011**: When mikebom emits synthetic built-in components, it MUST emit an info-level tracing log at scan-emission time: `"gem built-in synthetic components emitted"` with fields `count`, `built_in_names`. Grep-friendly for CI-log analysis per the milestone-157/158/159/160 observability convention.

### Key Entities

- **Built-in gems allowlist**: A static, in-source list of gem names sourced from Ruby-stdlib `Gem::default_gems`. Consulted at emission time. Immutable per compilation.

- **Synthetic built-in gem component**: A `pkg:gem/<name>` component with versionless PURL, `mikebom:synthetic-built-in = "ruby"` annotation, and `mikebom:built-in-requirement = "<requirement-string>"` annotation. Emitted iff the dep-name matches the allowlist AND was dropped by the graph resolver's dangling-target pass.

- **`mikebom:synthetic-built-in` (per-component)**: Component-scope annotation carrying `"ruby"` (single closed value in scope for this milestone). Future milestones may add other language-runtime built-in-gem systems (Python `stdlib` — though Python's stdlib is different mechanically).

- **`mikebom:built-in-requirement` (per-component)**: Component-scope annotation carrying the original Gemfile.lock requirement string (e.g., `>= 1.2.0`, `~> 1.0`). Bare-string value. Empty if the source declaration had no requirement clause.

## Success Criteria

### Measurable Outcomes

- **SC-001 (test-rails edge fix)**: After milestone 162 ships, running `mikebom sbom scan --path test-rails --format cyclonedx-json` and comparing against Gemfile.lock ground truth MUST show 100% edge-match (249 of 250 edges from the milestone-157 audit, plus the 1 previously-dropped `bundler-audit → bundler` edge). Pre-162 baseline: 248 of 250 = 99.20%. Target: 250 of 250 = 100%.

- **SC-002 (test-rails specific missing-edge fix)**: The 1 concrete missing edge from the milestone-157 audit (`bundler-audit@0.9.3 → bundler`) MUST be present in the emitted SBOM (either as a real edge to a synthetic `pkg:gem/bundler` component per Option A OR as a `mikebom:unresolved-dep-name` annotation on the source component per Option B). Both approaches surface the edge to consumers.

- **SC-003 (dual-side byte-identity guard, mirrors milestones 158/159/160/161)**: For every milestone-090 non-`gem` golden fixture (10 of 11 ecosystems: apk, bazel, cargo, cmake, deb, golang, maven, npm, pip, rpm), the emitted CDX / SPDX 2.3 / SPDX 3 SBOMs MUST be byte-identical to pre-162. The `gem` fixture is exempt — it will change to add `mikebom:synthetic-built-in` annotations if any built-in gems are referenced in its Gemfile.lock. Zero diff bytes on the 10 non-`gem` ecosystems × 3 formats = 30 goldens.

- **SC-004 (synthetic-component identifiability)**: 100% of emitted components carrying `mikebom:synthetic-built-in = "ruby"` MUST have a versionless PURL (no `@version` segment). 100% of emitted `pkg:gem/*` components NOT carrying the annotation MUST have a `@version` segment. This dual invariant is the load-bearing signal for consumer differentiation.

- **SC-005 (no false-positive CVE matches)**: Vulnerability-scanner tools (Grype, Trivy) processing a mikebom SBOM with synthetic built-in components MUST NOT report false-positive CVE matches on the synthetic components. Verifiable via running Grype against the emitted `test-rails` SBOM and confirming zero CVE matches on `pkg:gem/bundler` (versionless). Test is opportunistic per the milestone-083 external-tool pattern.

- **SC-006 (no regression on 248 EXACT-MATCH edges)**: The 248 pre-162 EXACT-MATCH edges MUST all remain EXACT-MATCH post-162. Zero regression on correctly-emitted edges.

- **SC-007 (non-Ruby repo byte-identity)**: Scanning any non-Ruby fixture (npm, cargo, apk, deb, etc.) MUST produce byte-identical output vs. pre-162.

- **SC-008 (pre-PR gate)**: `cargo +stable clippy --workspace --all-targets -- -D warnings` and `cargo +stable test --workspace --no-fail-fast` MUST both pass with zero errors before the PR is opened.

- **SC-009 (unit test coverage)**: The new synthetic-emission code paths MUST have at least 10 unit tests covering: (a) allowlist contains `bundler`; (b) allowlist does NOT contain `thor` (real gem); (c) synthetic component emission when Gemfile.lock references `bundler` as dep but not in GEM/specs; (d) synthetic component has versionless PURL; (e) synthetic component has `mikebom:synthetic-built-in = "ruby"` annotation; (f) synthetic component has `mikebom:built-in-requirement = <req>` annotation; (g) no synthetic emission when the same name IS in GEM/specs (FR-004 collision case); (h) synthetic component dedup — multiple sources referencing `bundler` yield ONE `pkg:gem/bundler` component; (i) edge from source to synthetic component preserved in `dependencies[]`; (j) unknown dangling-target (not in allowlist) still dropped per FR-008 (no synthetic emission).

- **SC-010 (integration test)**: A new integration test at `mikebom-cli/tests/ruby_built_in_gems.rs` MUST synthesize a Ruby project with a `Gemfile.lock` referencing `bundler-audit` (which itself declares `bundler` as a dep), scan it via the release binary, and assert (a) `pkg:gem/bundler-audit@0.9.3` emitted; (b) `pkg:gem/bundler` synthetic emitted with versionless PURL + both annotations; (c) `dependsOn` edge from bundler-audit to bundler present.

- **SC-011 (CHANGELOG entry)**: `CHANGELOG.md` MUST document the built-in-gem synthetic-component fix + FR-002/003/004 annotation vocabulary + the SC-001 empirical numbers + a consumer jq recipe for filtering synthetic components.

- **SC-012 (parity catalog registration)**: The 2 new annotations (C113 + C114 per-component) MUST have parity-catalog entries with `Directionality::SymmetricEqual`. Milestone-071 parity check MUST pass symmetrically across CDX / SPDX 2.3 / SPDX 3.

- **SC-013 (issue #496 closure)**: Issue #496 MUST reference this milestone (`closes #496` in the impl commit message) and the milestone MUST demonstrably resolve the reported symptom (silently dropped `bundler-audit → bundler` edge now visible).

## Assumptions

- **Ground truth = `Gemfile.lock` GEM/specs section**: The `Gemfile.lock`'s GEM/specs section is the authoritative source for what edges should be present in the SBOM. Ruby-toolchain-managed built-in gems are the specific edge case where GEM/specs doesn't include the source but the dep-name IS declared.

- **`test-rails` is the empirical benchmark**: SC-001/SC-002 numbers are pinned to this repo. The 1 specific missing edge (`bundler-audit@0.9.3 → bundler`) is the load-bearing verification.

- **Static allowlist is acceptable**: The FR-001 built-in gems list is derived from Ruby 3.4's `Gem::default_gems`. It will evolve — new Ruby releases may add or remove built-in gems. FR-006 documents this as a maintenance obligation. Alternative approaches (e.g., probing `ruby --version` on the scanned target) are out of scope per Assumption §4 below.

- **Versionless PURL is spec-compliant**: The PURL spec permits omitting the `@version` segment. Consumer vulnerability scanners typically do exact-version matching for CVE lookups; a versionless PURL will not false-positive on version-specific CVEs.

- **No `ruby --version` probe**: mikebom does NOT execute `ruby --version` to determine the actual installed Ruby version + inferred built-in gem versions. Rationale: (a) requires Ruby toolchain on scanning host (not always true); (b) probes the scanning host, not the target artifact (semantically wrong for image scans); (c) increases scan-time overhead. The versionless PURL is the accepted trade-off.

- **No new Cargo dependencies**: Following the milestone-157/158/159/160/161 precedent, this work uses existing crates only.

- **milestone-090 gem fixture may change**: If the fixture's Gemfile.lock references any built-in gems, its goldens will change. Verified at authoring time via inspection: the current `gem` fixture (`gem-source-project` in the fixture cache) likely does NOT reference built-in gems (it's a synthetic minimal fixture). SC-003 verifies zero diff on all fixtures EXCEPT `gem`; the `gem` fixture will be reviewed separately during Phase 5.

- **SC-001 target is achievable**: The 1-edge missing case is well-understood; the fix is structural (allowlist + synthetic emission). Unlike milestones 160 + 161, no empirical investigation loop is needed. This makes milestone 162 the simplest of the audit-round-2 issues.

## Out of Scope

- **The npm phantom empty-version edges fix (issue #498)** — separate milestone. npm scans are unaffected.

- **Python stdlib module handling** — Python's stdlib is imported directly (`import json`), not via a package manager, so there's no equivalent "dropped edge" symptom. Out of scope.

- **Probing `ruby --version`** — see Assumption §4. Not a fix path for this milestone.

- **Runtime Ruby version detection from artifacts** — a scanned image might contain Ruby with a known version, but extracting that reliably (Ruby vs JRuby vs TruffleRuby, multiple ruby installations, RVM/rbenv path resolution) is a separate engineering problem out of scope.

- **Cross-language `mikebom:synthetic-built-in` extension** — the annotation value vocab in this milestone is closed at `"ruby"`. Future milestones may extend to other language ecosystems if similar patterns are discovered (e.g., Node.js has `fs`, `path`, `crypto` built-in modules but they're not package-manager tracked; different mechanism entirely).

- **Version-range resolution against known Ruby stdlib versions** — the FR-005 `mikebom:built-in-requirement` annotation carries the version constraint as-declared. Cross-referencing against a table of "Ruby 3.4 ships bundler 2.5.x" is out of scope; consumers doing that lookup can build their own mapping.
