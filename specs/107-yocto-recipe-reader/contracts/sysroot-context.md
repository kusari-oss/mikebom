# Contract: Sysroot-vs-rootfs detection (US3, FR-005, FR-005a)

**New module**: `mikebom-cli/src/scan_fs/package_db/yocto/context.rs`

## Trigger

Called once per `mikebom sbom scan` invocation, by the opkg reader's `read()` entry point, with the scan-target rootfs path. Returns a `ScanContext` value consumed by both opkg and yocto/manifest readers to decide lifecycle-scope tagging.

## Two-signal heuristic (per 2026-06-01 Q3 clarification, Option B)

### Primary signal — Yocto SDK env-script presence

A file matching the glob `environment-setup-*` exists in EITHER:
1. The scan-target dir itself, OR
2. The scan-target's immediate parent dir.

Rationale: Yocto's SDK installer writes `environment-setup-<TARGET-SYS>` scripts alongside the sysroot. Detection at the parent dir level catches cases where the operator scans the sysroot subdir directly (e.g. `cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi/`) rather than the SDK installer's root dir.

### Secondary signal — filesystem-shape

ALL of:
1. `<scan-target>/usr/include/` directory exists
2. `<scan-target>/etc/init.d/` directory does NOT exist

Catches sysroots that have been moved away from their original SDK-installer parent dir (no longer have an adjacent env-script).

### Combination logic

| Primary | Secondary | Result |
|---|---|---|
| ✅ Fires | ✅ Fires | `ScanContext::Sysroot { primary: true, secondary: true }` |
| ✅ Fires | ❌ Doesn't fire | `ScanContext::AmbiguousSysroot { reason: "env-script present but filesystem shape suggests rootfs (init.d found or include missing)" }` — primary wins, but record the conflict |
| ❌ Doesn't fire | ✅ Fires | `ScanContext::Sysroot { primary: false, secondary: true }` |
| ❌ Doesn't fire | ❌ Doesn't fire | `ScanContext::Rootfs` |

## Diagnostic emission

When `ScanContext::AmbiguousSysroot { .. }` is returned, the scan's `ScanDiagnostics` collector records a `mikebom:scan-ambiguity` entry that surfaces in the emitted SBOM's `metadata.properties[]`. The annotation value is the `reason` string from the enum variant.

## Lifecycle-scope downstream behavior

The opkg / yocto-manifest readers consume `ScanContext` as follows:

- `Sysroot { .. }` OR `AmbiguousSysroot { .. }` → tag every emitted entry with `LifecycleScope::Build`
- `Rootfs` → no scope tag

Plus the per-stanza FR-006 override: `nativesdk-*` packages and host-arch packages ALWAYS carry `LifecycleScope::Build` regardless of the context-level result.

## API surface (Rust)

```rust
pub(super) enum ScanContext {
    Sysroot { primary_signal: bool, secondary_signal: bool },
    Rootfs,
    AmbiguousSysroot { reason: String },
}

pub(super) fn detect_scan_context(rootfs: &Path) -> ScanContext { ... }
```

`detect_scan_context` is pure (no side effects, no logging — the diagnostic emission happens at the caller layer where the `ScanDiagnostics` collector is in scope).

## Edge cases

- Multiple `environment-setup-*` scripts (multi-target SDK) → still fires the primary signal; treated as one detection. No multi-arch handling at the context layer.
- Symlinked sysroot (`/opt/foo/sysroots/<arch>` is a symlink) → `std::fs::canonicalize` resolved before signal evaluation
- `/usr/include/` exists but is empty → primary signal still considered satisfied (we don't recursively check contents)
- Scan target IS the env-script's parent dir (operator scans `/opt/poky/5.0/` rather than `/opt/poky/5.0/sysroots/<arch>/`) → primary signal fires; opkg reader walks down into `sysroots/*/var/lib/opkg/status` and tags the entries

## Tests

Per-module unit tests in `yocto/context.rs::tests`:
- `env_script_in_scan_target_fires_primary` (tempdir with `environment-setup-foo` in scan root)
- `env_script_in_parent_dir_fires_primary` (tempdir parent has env-script, scan target is child)
- `secondary_signal_fires_on_include_without_init_d`
- `rootfs_when_neither_signal_fires`
- `ambiguous_when_primary_fires_but_init_d_present`

Integration test at `mikebom-cli/tests/scan_yocto_sysroot.rs`:
- End-to-end scan of `yocto_sysroot/` fixture (synthetic sysroot with env-script + minimal opkg DB)
- Assertion: emitted CDX components carry `scope: "excluded"` (via milestone-052 lifecycle-scope mapping); SBOM metadata contains no `scan-ambiguity` annotation (clean sysroot, both signals fire)
