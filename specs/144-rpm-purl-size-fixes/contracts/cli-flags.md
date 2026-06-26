# Contract — new `mikebom sbom scan` CLI flags

Phase 1 output. Defines the operator-visible CLI surface introduced by milestone 144.

## `--rpm-distro <ID>` (new)

**Syntax**: `--rpm-distro <ID>` (long form only; no short form).

**Type**: optional `String`. When omitted, behavior is governed by the auto-detection ladder (`/etc/os-release` → per-RPM RPMTAG_VENDOR/PACKAGER → empty).

**Validation** (clap-side, BEFORE any scan begins):
- MUST be non-empty.
- MUST contain only characters acceptable in a purl-spec namespace segment after lowercasing (the value is lowercased by the `value_parser` closure before validation).
- Empty string → exit code != 0, error message: `error: invalid value '' for '--rpm-distro <ID>': must be non-empty`.

**Semantics**: When present, the value overrides ALL other distro-identity sources for the duration of the scan — including auto-detected `/etc/os-release` `ID=` AND per-RPM RPMTAG_VENDOR / RPMTAG_PACKAGER metadata. Authoritative across all standalone-`.rpm` files in the scan tree.

**Scope**: Applies ONLY to the standalone-`.rpm`-file reader (`scan_fs::package_db::rpm_file`). Does NOT affect:
- The rpmdb reader (`scan_fs::package_db::rpm`) — uses its own existing distro detection.
- Other ecosystem readers (dpkg, apk, alpm, etc.) — unrelated.

**`--help` text** (one-line description):

```
--rpm-distro <ID>     Override distro identifier for RPM PURL namespaces (e.g.,
                      --rpm-distro poky for Yocto). Overrides /etc/os-release
                      and per-RPM vendor tags. Default: auto-detect.
```

**Acceptance** (verifying scenarios from spec):

| Scenario | Input | Expected |
|---|---|---|
| US4-1 | `--rpm-distro poky` on dir with `.rpm` files | All emitted PURLs have namespace `poky` |
| US4-2 | `--rpm-distro poky` AND `etc/os-release` declares `ID=fedora` | All emitted PURLs use `poky` (CLI wins) |
| US4-3 | `--rpm-distro ""` | Clap rejects at parse time with non-empty error |
| US4-4 | `--rpm-distro poky` AND RPM has `RPMTAG_VENDOR=Fedora Project` | All emitted PURLs use `poky` (CLI wins over per-RPM) |

## `--max-rpm-bytes <BYTES>` (new)

**Syntax**: `--max-rpm-bytes <BYTES>` (long form only; no short form).

**Type**: optional `u64`. When omitted, the default of `512 * 1024 * 1024` (512 MiB) applies.

**Validation** (clap-side, BEFORE any scan begins):
- MUST parse as a `u64` integer (non-numeric → exit code != 0).
- MUST be `> 0` (zero → exit code != 0, error message: `error: invalid value '0' for '--max-rpm-bytes <BYTES>': must be > 0`).
- Negative inputs are rejected by `u64` parsing (no separate check needed).

**Semantics**: Sets the per-file cap (bytes) for standalone `.rpm` files. Files whose on-disk size exceeds the cap are skipped with a structured WARN log (`reason="size-cap-exceeded"`). Comparison is strict greater-than (`>`); a file exactly at the cap is included.

**Scope**: Applies ONLY to the standalone-`.rpm`-file reader. The `MAX_RPMDB_BYTES` constant in the rpmdb reader (`scan_fs::package_db::rpm`) is independently raised to 512 MiB at compile time and is NOT operator-overridable in this milestone (deferred per spec Out of Scope §2).

**`--help` text** (one-line description):

```
--max-rpm-bytes <BYTES>   Per-file size cap for standalone .rpm files. Files
                          exceeding the cap are skipped (with WARN). Useful
                          for Yocto debug RPMs (kernel-dbg, gcc-dbg).
                          Default: 536870912 (512 MiB).
```

**Acceptance** (verifying scenarios from spec):

| Scenario | Input | Expected |
|---|---|---|
| US2-1 | (default cap, 300 MB RPM) | RPM emitted; no WARN |
| US2-2 | (default cap, 600 MB RPM) | RPM skipped; WARN text without "malformed"; `reason="size-cap-exceeded"` present |
| US2-3 | `--max-rpm-bytes 1073741824` + 600 MB RPM | RPM emitted; no WARN |
| US3-1 | `--max-rpm-bytes 1073741824` + 700 MB RPM | RPM emitted |
| US3-2 | `--max-rpm-bytes 100000000` + 300 MB RPM | RPM skipped with size-cap WARN |
| US3-3 | `--max-rpm-bytes abc` | Clap rejects with parse error |
| US3 (zero) | `--max-rpm-bytes 0` | Clap rejects with > 0 error |

## Flag interaction

Both flags are independent and may be combined freely:

```bash
mikebom sbom scan --path tmp/deploy/rpm/ \
    --rpm-distro poky \
    --max-rpm-bytes 1073741824 \
    --format cyclonedx-json --output /tmp/yocto.cdx.json
```

Neither flag affects the other. The size check runs BEFORE the vendor-slug resolution (the cap-exceeded path doesn't even open the file with the RPM parser), so `--rpm-distro` is irrelevant for files that are skipped due to size.

## Out of scope for this contract

- `--max-rpmdb-bytes` — deferred per FR-008.
- `--rpm-distro-version` (analogue for `&distro=<vendor>-<version_id>` qualifier) — not requested; existing `distro_version` derivation from `/etc/os-release` `VERSION_ID=` covers the auto-detect case.
- Per-subtree config (e.g., `--rpm-distro path/prefix:value`) — explicitly rejected per spec Out of Scope §1.
