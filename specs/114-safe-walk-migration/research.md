# Phase 0 Research — Shared `safe_walk` Helper Migration

**Feature**: 114-safe-walk-migration
**Date**: 2026-06-12

Resolves the remaining design decisions left by the spec + clarifications. Each decision is presented with rationale and the alternatives that were considered and rejected.

---

## R1. Helper module location

**Decision**: `mikebom-cli/src/scan_fs/walk.rs` (top-level under `scan_fs/`, not nested under `package_db/`).

**Rationale**:
- The helper is consumed by BOTH `package_db/*` ecosystem walkers (cargo, maven, gem, …) AND the `binary/*` walkers (`binary/discover.rs`, `binary/source_binding/cmake_observer.rs`). Hosting it under `package_db/` would force a sibling-up `super::super::package_db::walk` import path from the binary side — confusing visual signal that the binary side "borrows from" the package-db side when both are equal-rank consumers.
- The existing `project_roots.rs` lives inside `package_db/` only because it was first introduced for pip + npm; binary walkers post-date it. The migration is the chance to fix the layering.
- Issue #108's suggested location: `scan_fs/walk.rs` — aligns with the codebase's existing pattern of one-file-per-shared-concern at `scan_fs/` top level (e.g., `walker.rs`, the deep-hash whole-FS file enumerator).

**Alternatives considered**:
- `scan_fs/package_db/walk.rs`: keeps proximity to the consumers but violates the consumer-rank symmetry above.
- `scan_fs/walk/mod.rs` (sub-directory): premature — the helper is one file, no sub-modules expected.

---

## R2. Public API shape

**Decision**: One pure function `safe_walk` and one config struct `WalkConfig`, declared in `mikebom-cli/src/scan_fs/walk.rs`:

```text
pub(crate) struct WalkConfig<'a> {
    pub max_depth: usize,
    pub should_skip: &'a dyn Fn(&Path, &Path) -> bool,
    pub exclude_set: &'a super::package_db::exclude_path::ExclusionSet,
}

pub(crate) fn safe_walk<F: FnMut(&Path)>(
    rootfs: &Path,
    cfg: &WalkConfig,
    mut visit: F,
);
```

The `visit` callback fires once per path the descent encounters — files AND directories (Q1 clarification). The `should_skip` predicate runs at the descent decision for each child directory and receives `(candidate, rootfs)` (Q2 clarification). The `exclude_set` field stays as a separate config field rather than folding into `should_skip` because (a) the helper centralizes the exclusion-debug-log emission inside its own loop (today only `project_roots.rs` logs it; centralizing fixes the milestone-113 invariant that EVERY walker should log skip events), and (b) the exclusion check is cheap-fast-path-skip-when-empty — the helper can short-circuit on `is_empty()` before invoking the closure.

**Rationale**:
- Q1 + Q2 already pinned the callback + predicate shapes. The remaining choice — whether `exclude_set` is a separate field or folded into `should_skip` — is decided in favor of "separate field" because it lets the helper centralize the exclusion-debug-log emission. If we folded it in, every per-walker `should_skip` closure would have to emit its own `tracing::debug!`, regressing observability.
- `pub(crate)` not `pub` because this is binary-internal; per Constitution Principle VI, only `parity`/`binding`/`identifiers` are exposed at lib root.

**Alternatives considered**:
- Fold `exclude_set` into `should_skip`: regresses logging consistency.
- Two top-level functions (`safe_walk_for_files`, `safe_walk_for_dirs`): doubles the surface for one branch's worth of work — the caller's callback already discriminates.
- Builder pattern for `WalkConfig`: over-engineered for three fields.

---

## R3. Skip-event observability

**Decision**: The helper emits `tracing::debug!` exactly once at each skip decision, with a per-skip-cause label (`"reason = built-in"` vs `"reason = exclude-path"`). Uniform across every walker post-migration.

**Rationale**:
- Per the milestone-113 invariant ("operators with `RUST_LOG=debug` should see WHY a directory got skipped"), centralizing the log into the helper closes the asymmetry where today only `project_roots.rs` logs skip events but the other 12 walkers don't.
- Labeling the cause lets operators correlate `exclude-path: skipping directory matched by user-supplied entry` (the milestone-113 log line) vs `safe_walk: skipping directory matched by built-in skip predicate` (the new line) in their stderr. The line shape stays close to milestone-113's existing line so operators reading old runbooks don't have to relearn it.

**Alternatives considered**:
- No debug logging in the helper, leave it to callers: keeps the regression where 12 walkers are silent. Rejected.
- Move logging behind a config flag: callers always want it on. Rejected.

---

## R4. Migration ordering (per-PR shape)

**Decision**: Five PRs in sequence:

- **PR 1** (foundational): Introduce `scan_fs/walk.rs` with the helper + 12 unit tests. Port `package_db/project_roots.rs` to a thin wrapper that re-exports the same `WalkConfig` public type and forwards `walk_for_project_roots` to `safe_walk`. No per-ecosystem walker changes; the existing pip/npm/gradle/nuget/yocto closures keep working through the thin wrapper. ONE PR, byte-identical, low blast radius.
- **PR 2** (cargo + gem + rpm_file): Port the three smallest hand-rolled walkers — each has 1–3 internal recursion sites. Per-PR byte-identity check via the existing golden suite for each affected ecosystem.
- **PR 3** (maven + golang + go_binary): Port the three Go-or-Maven walkers. These have more callsites and tests; biggest PR by line count.
- **PR 4** (nuget + yocto + binary/discover + binary/source_binding/cmake_observer): Port the four remaining one-off walkers.
- **PR 5** (audit + docs): Delete `project_roots.rs::walk_inner` (now dead — the thin wrapper from PR 1 delegates to `safe_walk` directly). Add the `Filesystem walking` section to `docs/design-notes.md`. Add the comment block in `scan_fs/walk.rs` enumerating the two known exceptions. Add the audit-grep documentation. CHANGELOG entry under `### Changed`.

**Rationale**:
- Five PRs balance reviewability (each ≤500 lines) against PR-overhead. Could be done in one big PR but reviewers would struggle.
- Byte-identity gate runs on every PR independently — if PR 3 breaks a golden, PR 4 doesn't ship until PR 3's fix is in.
- PR 1 + PR 5 are the "bookend" PRs (foundational + cleanup); PRs 2-4 are individually deliverable in any order if needed (no inter-PR dependency beyond PR 1).

**Alternatives considered**:
- One big PR: harder to review.
- Per-walker PR (12 PRs): too granular; CI cost.

---

## R5. Known-exception identification audit

**Decision**: Two walkers identified as documented known exceptions. The audit process: for each hand-rolled walker, check whether its descent semantics fit the `(rootfs, cfg, visit) → ()` shape with `WalkConfig`'s three fields. If yes → migrate. If no → document why and leave hand-rolled.

**Audit results**:

| Walker | Fits shape? | Decision |
|---|---|---|
| `package_db/project_roots.rs::walk_inner` | ✓ (this IS the shape) | Migrate (PR 1 source of truth). |
| `package_db/cargo.rs::walk_for_cargo_manifests` | ✓ | Migrate (PR 2). |
| `package_db/cargo.rs::walk_for_cargo_lockfiles` | ✓ | Migrate (PR 2). |
| `package_db/gem.rs::walk_for_top_level_gemspecs` | ✓ | Migrate (PR 2). |
| `package_db/gem.rs::walk_for_gemfile_locks` | ✓ | Migrate (PR 2). |
| `package_db/gem.rs::walk_for_gemspecs` | ✓ | Migrate (PR 2). |
| `package_db/rpm_file.rs::walk_dir` | ✓ | Migrate (PR 2). |
| `package_db/maven.rs::walk_for_maven` | ✓ (multi-extension via callback) | Migrate (PR 3). |
| `package_db/maven.rs::walk_for_top_level_poms` | ✓ | Migrate (PR 3). |
| `package_db/golang/legacy.rs::walk_for_go_roots` | ✓ (has milestone-113 testdata/_-prefix skips in the predicate) | Migrate (PR 3). |
| `package_db/go_binary.rs::walk_for_binaries` | ✓ (mutable-ref capture via closure) | Migrate (PR 3). |
| `package_db/nuget/mod.rs::walk_inner` | ✓ | Migrate (PR 4). |
| `package_db/yocto/recipe.rs::walk` | ✓ (emit-inside-walk via callback) | Migrate (PR 4). |
| `binary/discover.rs::walk_dir` | ✓ | Migrate (PR 4). |
| `binary/source_binding/cmake_observer.rs::walk_for_cmake_build_dirs` | ✓ | Migrate (PR 4). |
| `scan_fs/walker.rs::walk` (whole-FS file walker) | ✗ — no skip list, no depth bound, has size cap | Documented known exception. |
| `package_db/npm/walk.rs::walk_node_modules` | ✗ — `@scope` one-level recursion + `in_npm_internals` state propagation | Documented known exception. |

**Rationale**:
- The structural fit-check is mechanical: does this walker's semantics decompose into `(rootfs, max_depth, should_skip, visit)` without losing semantics? For 15 of 17, yes. For the two exceptions, no — and forcing the fit would either bloat the helper API (size_cap, hash-inside-walk, parent-name-aware predicate, per-descent state propagation) or distort the per-walker semantics.

**Alternatives considered**:
- Stretching the helper API to cover the deep-hash walker: rejected; the helper stays minimal per FR-002's "every walker carries the same invariants" — the deep-hash walker has DIFFERENT invariants (no depth bound, no skip list).
- A parallel `safe_walk_files` helper with size cap: defers; if a future walker needs that shape, add it then.

---

## R6. Audit-grep pattern

**Decision**: The documented audit pattern is `grep -rEn 'fn walk[_(]' mikebom-cli/src/scan_fs/`. The pattern intentionally catches every `fn walk_*` and `fn walk(` declaration regardless of visibility prefix (`pub`, `pub(crate)`) or indentation. The audit is paired with an explicit known-non-walker list documented in the comment block at the top of `scan_fs/walk.rs`. Acceptable matches post-migration:

**Filesystem ecosystem-discovery walkers (the shared helper + 2 documented exceptions)**:
- `mikebom-cli/src/scan_fs/walk.rs` — the helper file itself. Contains `pub(crate) fn safe_walk` and any internal helpers.
- `mikebom-cli/src/scan_fs/walker.rs` — documented known exception (deep-hash whole-FS file enumerator). Contains `pub fn walk_and_hash` and `fn walk`.
- `mikebom-cli/src/scan_fs/package_db/npm/walk.rs` — documented known exception (@scope-aware npm walker). Contains `fn walk_node_modules`.

**Non-filesystem-walker false positives — also acceptable** (these match the regex but are NOT filesystem walkers; documented in the helper module's comment block so reviewers can recognize them):
- `mikebom-cli/src/scan_fs/package_db/maven.rs` — `fn walk_m2_jars` (iterates a precomputed `Vec<PathBuf>` returned by `MavenRepoCache::discover`; no `read_dir` recursion) and `pub(crate) fn walk_jar_maven_meta` (walks JAR archive content, not the filesystem).
- `mikebom-cli/src/scan_fs/package_db/rpmdb_sqlite/schema.rs` — `fn walk_schema_page` (SQLite B-tree page walker, not filesystem).
- Test functions named `fn walks_*` (e.g., `walks_symlink_loop_without_hanging`) — these are tests OF walkers, not walkers themselves; live inside `#[cfg(test)] mod tests` and are typically indented but the regex catches them too.
- `mikebom-cli/src/scan_fs/package_db/maven.rs` test functions `fn walk_jar_*`, `fn walk_fat_jar_*`, `fn walk_rootfs_poms_*` — tests of the maven JAR walker.

Any match OUTSIDE the union of the two lists above is a regression: a contributor introduced a new hand-rolled walker bypassing the shared helper. Reviewer action: reject the PR or push back to either migrate the new walker to `safe_walk` OR document a new entry in the helper module's comment block.

**Rationale**:
- The initial design of `^fn walk` was too restrictive: it missed `pub(crate) fn safe_walk` and `pub fn walk_and_hash` (both prefixed) AND it failed to filter the un-prefixed `fn walk_m2_jars` (a non-filesystem walker). The broader `fn walk[_(]` pattern catches every walker-named function, paired with the explicit non-walker false-positive list, gives a procedurally-checkable result.
- The known-non-walker list lives in the comment block at the top of `scan_fs/walk.rs` (T007) so it's discoverable from the same place the audit is documented. Reviewers can `cat scan_fs/walk.rs | head -50` and see both the exception list and the audit pattern.

**Alternatives considered**:
- Rename `maven.rs::walk_m2_jars` → `iterate_m2_jars` to eliminate it from the audit's catch: viable but invasive (rename + every callsite). Deferred; current approach documents-rather-than-renames.
- Sophisticated regex like `^\s*(pub(\(\w+\))?\s+)?fn walk_` that excludes test functions: unreadable; harder to remember; equivalent net result.
- `clippy` lint: clippy doesn't have a hand-rolled-`read_dir`-detection lint and writing a custom one is over-engineering.
- CI hook check: out of scope per spec Assumptions; future follow-up.

---

## R7. Byte-identity test strategy

**Decision**: Each per-walker port PR runs `cargo +stable test --workspace` and verifies that:

1. Every existing per-walker test passes byte-identically (no test was modified to accept different output).
2. The 33 committed CDX / SPDX 2.3 / SPDX 3 byte-identity goldens pass without `MIKEBOM_UPDATE_*_GOLDENS` invocation.
3. The realistic-projects CI job (which scans real-world fixtures like knative/func) emits identical CDX output pre- and post-port.

These three gates run as the standard pre-PR `./scripts/pre-pr.sh` check. SC-002's "no goldens regen required" claim is enforced procedurally.

**Rationale**:
- The byte-identity guarantee is the principal risk lever for this refactor (FR-009, SC-002, US3). Per-PR testing means a bad port can't compound across PRs.
- No new test infrastructure required — the existing harness already enforces byte-identity in the `pre-pr.sh` gate.

**Alternatives considered**:
- A new "walker-output-equivalence" test that captures the exact path-set each walker produces pre-port and compares post-port: more thorough but redundant with the existing golden suite (which is downstream of every walker).

---

## R8. Performance verification

**Decision**: SC-004 (≤105% scan time) is verified informally — the helper preserves the same per-directory cost profile as the existing per-walker hand-rolled loops (one `canonicalize` + one `HashSet::insert` + one `read_dir` per directory + one `should_skip` invocation + one `is_empty()`/`matches` invocation per child). No measurable change is expected. A spot-check via the existing realistic-projects CI job suffices.

**Rationale**:
- The refactor doesn't introduce new allocations, new system calls, or new per-directory work. SC-004's 5% headroom is comfortable given zero structural change.
- A formal benchmark would be over-engineering for an invariant-preserving refactor.

**Alternatives considered**:
- New criterion benchmark: out of scope; FR-011's "no new Cargo deps" forbids criterion anyway, and the existing `realistic-projects.yml` workflow already measures scan time on the knative/func corpus.

---

## R9. Walkers with state-carrying closures (the cargo/gem/etc test callsites)

**Decision**: Closures that capture mutable references through the descent (`go_binary::walk_for_binaries` captures `&mut out: Vec<PackageDbEntry>` + `&mut seen_purls: HashSet<String>` + …) work cleanly with `FnMut` callbacks. The caller writes:

```text
let mut out = Vec::new();
let mut seen_purls = HashSet::new();
safe_walk(rootfs, &cfg, |path| {
    if path.is_file() && /* binary-shaped */ {
        if let Some(entry) = extract_buildinfo(path, &mut seen_purls) {
            out.push(entry);
        }
    }
});
```

The closure's `&mut` captures naturally bind to the outer `let mut`. No per-walker signature rewrite beyond replacing the hand-rolled recursion with the `safe_walk` call.

**Rationale**:
- Rust's closure borrowck handles this case cleanly via `FnMut`. The cargo-equivalent pattern (already in `project_roots.rs` consumers like `pip/mod.rs:285`) is the precedent.
- The complication for `go_binary` is that its current recursion takes `claimed_paths: &HashSet<PathBuf>` + `#[cfg(unix)] claimed_inodes: &HashSet<(u64, u64)>` as IMMUTABLE refs — those are even easier (no borrowck friction at all).

**Alternatives considered**:
- Threading state via a new `WalkContext` field: over-engineered; closures handle it.

---

## R10. CHANGELOG + docs touchpoints

**Decision**:
- `CHANGELOG.md` `[Unreleased]` → `### Changed` entry: "Internal cleanup: every ecosystem-reader filesystem walker migrated to a shared `safe_walk` helper. No user-visible behavior change. (Issue #108.)"
- `docs/design-notes.md` new section `## Filesystem walking pattern`: documents the API, the audit grep, the known-exception list, and a one-paragraph example of how a new ecosystem reader uses it.

**Rationale**:
- Issue #108's acceptance criteria explicitly require both touchpoints.
- The CHANGELOG entry is `### Changed` (not `### Removed` or `### Added`) because the change is invisible at the operator-visible surface — the audit-pattern + design-notes section is the only externally-discoverable signal.

**Alternatives considered**:
- Skip the CHANGELOG entry: issue #108's acceptance criteria require it.
- Generate the design-notes section automatically from the helper module's doc-comment: over-engineered; one-time write is fine.
