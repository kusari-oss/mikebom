# Contract: Yocto image manifest reader (US2, FR-003)

**New module**: `mikebom-cli/src/scan_fs/package_db/yocto/manifest.rs`

## Trigger

Any file matching the glob `build/tmp/deploy/images/*/*.manifest` anywhere in the scan tree. The walker uses `walkdir` (workspace dep, already in use across multiple readers).

## Parsing

`std::str::Lines` over the file contents. Each non-empty, non-comment line is split on whitespace into three tokens: `<name> <arch> <version>`.

Per `research.md` R2, the format is stable since Yocto 2.0 (2015):

```text
busybox       core2_64    1.36.1-r0
libc6         core2_64    2.38-r0
openssl       core2_64    3.0.5-r0
nativesdk-cmake  x86_64    3.27.5
```

## PURL derivation

```
pkg:opkg/<name>@<version>?arch=<arch>
```

Same PURL form as the opkg-installed-DB reader (they identify the same packages from different sources). The dedup pipeline (milestone 105) collapses cross-source emissions on canonical PURL; the lower-precedence source (`yocto-image-manifest`) appears in the loser's `mikebom:also-detected-via` annotation.

## Lifecycle-scope tagging (FR-006)

Manifest captures what's intended for the device — default lifecycle scope is **runtime** (`None`). The per-line FR-006 check still applies:

- Line where `name` starts with `nativesdk-` → `LifecycleScope::Build`
- Line where `arch` matches the host arch (heuristic: `x86_64`, `i686`, `aarch64` on a dev machine — but the manifest itself doesn't carry host-machine info, so this falls back to the name-prefix check alone)

## Annotations emitted (per component)

| Annotation | Value |
|---|---|
| `mikebom:source-files` | absolute path of the `<image>.manifest` |
| `mikebom:source-mechanism` | `"yocto-image-manifest"` |
| `mikebom:image-name` | filename stem (`core-image-sato` for `core-image-sato.manifest`) — informational |

## Edge cases

- Empty line → skipped silently
- Line with wrong token count (not 3) → skip + `tracing::warn!` with file path + line number
- Line starting with `#` → skipped silently (defensive; Yocto doesn't emit comments today but tolerating them is cheap insurance)
- Multiple `*.manifest` files in the same `images/<machine>/` dir (an image variant and a `core-image-foo-dbg.manifest` debug image) → each scanned independently; their components share canonical PURLs so the dedup pipeline collapses

## Tests

Per-module unit tests in `yocto/manifest.rs::tests`:
- `emits_one_component_per_line`
- `nativesdk_lines_tagged_build`
- `wrong_token_count_warns_and_skips`
- `empty_lines_ignored`

Integration test at `mikebom-cli/tests/scan_yocto_manifest.rs`:
- End-to-end binary scan against `yocto_manifest_basic/` fixture
- Assertion: emitted CDX contains expected PURLs; nativesdk-prefixed lines emerge with `scope: "excluded"` (via the milestone-052 lifecycle-scope → CDX-scope path)
