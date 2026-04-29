---
description: "Implementation plan — milestone 037 dpkg status.d/ reader"
status: plan
milestone: 037
---

# Plan: dpkg `status.d/` reader

## Architecture

Pure additive read-side extension. `dpkg.rs::read` gains a second
source (`status.d/` directory walk) alongside the existing
single-file `status` source. Both feed through the unchanged
`parse_stanza`/`split_stanzas` pipeline; the union of entries is
returned.

No new types. No new modules. No public-API change. No new deps.

## Reuse inventory

- **`parse_stanza`** (line 135) — handles a single stanza; reused
  unchanged for each `status.d/<pkg>` file.
- **`split_stanzas`** (line 116) — yields stanza strings from a
  multi-stanza text. For `status.d/` files we expect 1 stanza
  each, but reusing the splitter handles the (anomalous)
  multi-stanza case gracefully.
- **The Status-filter logic in `parse_stanza`** — same
  `install ok installed` requirement applies to both sources.
- **`std::fs::read_dir` + `read_to_string`** — same primitives
  used by `collect_claimed_paths`.

## Touched files

| File | Change | LOC |
|---|---|---|
| `mikebom-cli/src/scan_fs/package_db/dpkg.rs` | + DPKG_STATUS_D_DIR const, + read_status_d_dir helper, extend read(), + 5 inline tests | +180 |
| `mikebom-cli/tests/oci_registry_smoke.rs` | rename + update distroless assertion | +5 / -5 |
| `CHANGELOG.md` | unreleased entry | +5 |

Total ~190 LOC across 3 files. All in 1 PR.

## Phasing

Two atomic commits.

### Commit 1: `037/status-d-reader`
- Extend `dpkg.rs::read` to also walk `status.d/`.
- 5 new inline tests per FR-005.
- Update the distroless smoke test (rename + new assertion).
- All standard verification gates green.

### Commit 2: `037/changelog`
- CHANGELOG entry.

## Estimated effort

| Phase | Effort | Notes |
|---|---|---|
| Commit 1 | 3 hr | Synthetic fixtures + inline tests |
| Commit 2 | 0.5 hr | Text |
| Verification + PR | 0.5 hr | Tight loop |
| **Total** | **~4 hr** | Under the ½-day issue estimate. |

## Risks

- **R1: companion-file pattern.** `status.d/` directories contain
  `<pkg>` and `<pkg>.md5sums` (and sometimes `.conffiles`,
  `.triggers`, `.symbols` etc.). Pattern: skip any file with an
  extension (i.e. `path.extension().is_some()` → skip). Verified
  against distroless-static fixture: `base-files`, `media-types`,
  `netbase`, `tzdata` are extension-less; all companion files end
  in `.md5sums` only (in distroless). Safe.

- **R2: existing distroless smoke-test assertion now fails.** The
  asserting-zero-components test is the right guard for milestone
  031 anonymous-pull behaviour but becomes wrong once status.d/ is
  read. Mitigation: rename + flip the assertion in the same commit
  that lands the reader change. The commit's `./scripts/pre-pr.sh`
  passes; the smoke test stays gated behind
  `MIKEBOM_OCI_NETWORK_TESTS=1`, so default CI is unaffected.

- **R3: byte-identity goldens regen.** Existing 27-fixture goldens
  use either monolithic status or no dpkg metadata. The new code
  path runs only when `status.d/` exists. Verify via SC-003
  goldens-regen (zero diff expected).

## Constitution alignment

- **Principle I (zero C):** No new deps. ✓
- **Principle IV (no .unwrap() in production):** New helper uses
  `?` and `Option`/`Result` throughout. ✓
- **Principle VI (three-crate architecture):** Untouched. ✓

## What this milestone does NOT do

- Does not add per-file deep-hashing for status.d/ images.
- Does not change PURL grammar.
- Does not touch the apk reader (chainguard apko's hypothetical
  `installed.d/` variant — separate investigation if surfaced).
- Does not touch parity / generate / resolve.
