---
description: "Implementation plan — milestone 035 --image-platform flag"
status: plan
milestone: 035
---

# Plan: `--image-platform` flag

## Architecture

Pure additive flag-threading. clap arg → `Option<String>` →
`pull_to_tarball` → `parse_platform_string` → resolver. No new
modules; no new types beyond a small `ParsedPlatform` struct local
to `platform.rs`.

The resolver's existing `target_arch: &str` parameter generalizes to
`(target_arch, target_variant: Option<&str>)` — a 1-LOC signature
change with a 3-LOC match update. `ManifestListEntry` gains a
`variant: Option<String>` field; existing call sites in `mod.rs`
populate it from `oci_spec::Platform::variant()`.

## Reuse inventory

- **`oci_spec::image::Platform::variant()`** — accessor returning
  `&Option<String>`. Already in scope (used elsewhere in mod.rs's
  manifest-list mapping).
- **`host_oci_arch()`** in `mod.rs` — already maps Rust ARCH to OCI
  arch names. Becomes the fallback when `--image-platform` is unset.
- **clap `requires = "image"`** — enforces the flag-needs-image
  invariant at clap-validation time, no runtime check needed for
  that case.
- **Existing `resolve_manifest_list_to_linux` tests** — minor
  refactor to add the `target_variant` parameter; existing assertions
  pass `None`.

## Touched files

| File | Change | LOC |
|---|---|---|
| `mikebom-cli/src/cli/scan_cmd.rs` | + `image_platform: Option<String>` ScanArgs field; + tarball+flag rejection branch | +25 |
| `mikebom-cli/src/scan_fs/oci_pull/platform.rs` | + `ParsedPlatform` + `parse_platform_string` + variant in `ManifestListEntry` + variant in resolver match; tests | +130 |
| `mikebom-cli/src/scan_fs/oci_pull/mod.rs` | thread `image_platform` arg through `pull_to_tarball`; populate variant in mapping | +20 |
| `mikebom-cli/tests/oci_registry_smoke.rs` | + gated cross-arch smoke test | +50 |
| `docs/user-guide/cli-reference.md` | + `--image-platform` flag row + section update | +25 |
| `CHANGELOG.md` | unreleased entry | +5 |

Total: ~255 LOC across 6 files.

## Phasing

Two atomic commits.

### Commit 1: `035/flag-and-resolver`
- ScanArgs: add `image_platform`, the tarball-rejection branch.
- platform.rs: `parse_platform_string` + tests; `ManifestListEntry`
  gets variant; resolver gets `target_variant` parameter.
- mod.rs: thread the arg, populate variant.
- Inline tests cover ≥6 parse cases + ≥3 new resolver cases.
- All existing tests pass with minimal mechanical updates (passing
  `None` to the new parameter).

### Commit 2: `035/docs-and-smoke`
- `--image-platform` row in cli-reference.md, plus a one-paragraph
  cross-arch note under the `--image` section.
- CHANGELOG entry.
- Gated `pulls_alpine_with_image_platform_override` smoke test.

## Estimated effort

| Phase | Effort | Notes |
|---|---|---|
| Commit 1 | 4 hr | Resolver refactor is mechanical; parse + variant matching + clap arg |
| Commit 2 | 1 hr | Docs + smoke test |
| PR + verification | 30 min | CI is fast (<5 min) |
| **Total** | **~5.5 hr** | Well under the 1-day issue estimate. |

## Risks

- **R1: oci_spec variant accessor shape.** `Platform::variant()`
  returns `&Option<String>` per oci-spec's getset macros. Cloning
  is free. If the API differs from expectation, fall back to a
  serde Value extraction. Verified by a quick `cargo doc -p oci-spec`
  spelunk at commit-1 start.
- **R2: Multi-arch test fixture.** alpine:3.19 has linux/amd64,
  linux/386, linux/arm64, linux/arm/v6, linux/arm/v7, linux/ppc64le,
  linux/s390x — confirmed via `docker manifest inspect alpine:3.19`.
  Pick the non-host arch in the smoke test (`amd64` if running on
  arm host, `arm64` otherwise).
- **R3: clap `requires = "image"` interaction with `conflicts_with =
  "path"`.** Clap should resolve correctly: `--image-platform`
  requires `--image`, which already conflicts with `--path`.
  Manual `mikebom sbom scan --path . --image-platform linux/amd64`
  invocation should produce a clean clap error.

## Constitution alignment

- **Principle I (zero C in deps):** No new deps. ✓
- **Principle IV (no `.unwrap()` in production):** The
  `parse_platform_string` parser uses `Result` everywhere; no panics. ✓
- **Principle VI (three-crate architecture):** Untouched. ✓
- **Per-commit verification:** Each commit's `./scripts/pre-pr.sh`
  passes.

## What this milestone does NOT do

- Does not change ANY scan output beyond the platform selected.
- Does not introduce a `--platform` alias.
- Does not implement multi-platform fan-out.
- Does not relax the linux-only constraint.
