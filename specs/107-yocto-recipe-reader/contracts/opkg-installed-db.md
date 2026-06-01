# Contract: opkg installed-DB reader (US1, US3, US5, FR-001, FR-002, FR-006)

**New module**: `mikebom-cli/src/scan_fs/package_db/opkg.rs`

## Trigger

`<rootfs>/var/lib/opkg/status` is a regular file.

## Parsing

Delegates to the new shared `package_db/control_file.rs::parse_stanzas` helper (refactored out of `dpkg.rs`). Stanza-by-stanza iteration; each stanza becomes one `OpkgStanza` value (see `data-model.md`).

Format is byte-identical to dpkg's status file (see research R1). Multi-line `Description:` continuation lines (indented with a single space) are correctly merged by the shared parser.

## PURL derivation

```
pkg:opkg/<name>@<version>?arch=<arch>
```

Name and version are percent-encoded per the package-url spec via the existing `encode_purl_segment` helper. `arch` is passed verbatim into the qualifier — Yocto arch values like `cortexa7t2hf-neon-vfpv4` survive intact.

## Claim-path collection

After parsing, `opkg::collect_claimed_paths(rootfs, &mut claimed, &mut claimed_inodes)` reads each package's `/usr/lib/opkg/info/<pkg>.list` (newline-separated absolute paths) and inserts each path into the binary-walker's claim set — same pattern as `dpkg::collect_claimed_paths`. This prevents the binary walker from emitting duplicate `pkg:generic/<basename>` components for files already owned by an opkg package.

## Lifecycle-scope tagging (FR-005, FR-006)

Driven by the `ScanContext` value returned by `yocto/context.rs` for the same scan target:

- `ScanContext::Sysroot { .. }` or `ScanContext::AmbiguousSysroot { .. }` → tag every emitted entry with `LifecycleScope::Build`
- `ScanContext::Rootfs` → no lifecycle scope (runtime default)

Additionally, the **per-stanza FR-006 check** overrides the context-level tag for `nativesdk-*` packages:

- Stanza name starts with `nativesdk-` → ALWAYS `LifecycleScope::Build` (these are host-side build tools, never run on the target device)
- Stanza arch matches the host arch (typically `x86_64` on dev machines, never the target arch) → ALWAYS `LifecycleScope::Build`

## Annotations emitted (per component)

| Annotation | Value |
|---|---|
| `mikebom:source-files` | absolute path of `/var/lib/opkg/status` |
| `mikebom:source-mechanism` | `"opkg-installed"` |
| `mikebom:version-status` | `"missing"` if the stanza had no `Version:` field (rare) |
| `mikebom:feed-filename` | The `.ipk` filename from the stanza's `Filename:` field, if present (informational for feed-traceability) |

## Edge cases

- Stanza with empty `Package:` field → skip + `tracing::warn!`
- Stanza with missing `Architecture:` → fallback to `all`
- Multi-line `Description:` continuation lines → merged into single field by the shared parser
- Unknown fields (vendor-extension stanzas) → silently ignored
- `Status:` field with `not-installed` → still emitted (matches dpkg's reader behavior; the binary walker's claim-path check filters at a lower layer)

## Tests (per `tasks.md` later)

Per-module unit tests in `opkg.rs::tests`:
- `emits_basic_components` — happy path, multi-stanza
- `claims_files_from_info_dot_list` — file claim collection
- `applies_sysroot_lifecycle_scope` — `ScanContext::Sysroot` → all entries tagged Build
- `nativesdk_prefix_forces_build_scope` — FR-006 per-stanza override
- `missing_version_emits_status_annotation`
- `unknown_fields_silently_ignored`

Integration test at `mikebom-cli/tests/scan_opkg.rs`:
- End-to-end binary scan of the in-repo `opkg_basic/` fixture
- Assertion: emitted CDX contains expected `pkg:opkg/<name>@<version>?arch=<arch>` PURLs, license fields flow through, claimed paths are recorded
