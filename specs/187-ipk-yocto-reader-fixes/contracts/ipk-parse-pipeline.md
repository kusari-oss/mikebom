# Contract: ipk parse pipeline (m187)

**Feature**: [../spec.md](../spec.md) · **Plan**: [../plan.md](../plan.md) · **Data model**: [../data-model.md](../data-model.md)

## Pipeline shape

The `parse_ipk_file` entry point runs a 3-branch format-detection pipeline. Each branch has defined success + failure outcomes; the caller (`read()` in `ipk_file.rs`) dispatches on the outcome to either emit the component OR invoke the filename-fallback path.

```
                    read ipk bytes from disk
                             │
                             ▼
                first 8 bytes == `!<arch>\n`?
                    ╱                     ╲
                 yes                       no
                  │                         │
        [Branch 1: ar-format]     [Branch 2: gzip(tar) legacy]
                  │                         │
                  ▼                         ▼
        parse_ar_archive()          existing gzip(tar) parser
                  │                         │
      ┌───────────┼─────────────┐           │
      │           │             │           │
      ▼           ▼             ▼           ▼
   Ok(members) Err(ArMalformed) (n/a)    Ok(...)   Err(OuterMalformed)
      │           │                        │           │
      │           │                        │           ▼
      │           │                        │    Err(LegacyGzipTarFallbackFailed)
      │           │                        │
      ▼           ▼                        ▼
scan for control.tar[.gz] +          build_entry_from_control()
   data.tar[.gz]                          │
      │                                   ▼
      ├── found → extract_control_file()  emit component
      │           (existing helper)       (source-mechanism = "ipk-file")
      │           │
      │           ▼
      │    build_entry_from_control()
      │    (source-mechanism = "ipk-file-archive-extraction")
      │           │
      │           ▼
      │    emit component
      │
      └── missing → Err(ControlMissing)
                             │
                             ▼
                    ALL Err(_) outcomes bubble up
                             │
                             ▼
                    [Branch 3: filename fallback]
                             │
                             ▼
                filename_fallback_entry()
                             │
                             ├── parent_dir_arch_match hits?
                             │       │
                             │       yes → source-mechanism = "ipk-file-filename-fallback"
                             │               arch-source = "parent-directory"
                             │       │
                             │       no  → source-mechanism = "ipk-file-filename-fallback"
                             │               arch-source = "filename-heuristic"
                             │
                             ▼
                emit component (via WARN salvage log)
```

## Per-branch contracts

### Branch 1 — ar-format parser (US1 primary path)

**Trigger**: File bytes start with `!<arch>\n` (BSD ar magic).

**Function**: `parse_ar_archive(bytes: &[u8]) -> Result<Vec<ArMember>, ArError>` (new, private helper in `ipk_file.rs`).

**Success**: Returns `Vec<ArMember>` where each entry is `{ name: String, data: Vec<u8> }`. Members appear in archive order.

**Failure**: Returns `ArError`:
- `TruncatedHeader` — the 60-byte header couldn't be read (archive body ended mid-header).
- `NonAsciiSizeField` — the header's size field contained non-decimal bytes.
- `SizeOverrunsBody` — the size field claims more bytes than remain in the archive.

**Downstream** (on success): The caller scans the returned member list for:
1. `control.tar.gz` OR `control.tar` (uncompressed) — REQUIRED; missing → `Err(ControlMissing)`.
2. `data.tar.gz` OR `data.tar` — OPTIONAL; used for the file-list collection but not gating.
3. `debian-binary` — OPTIONAL; content diagnostic only (WARN if value ≠ `2.0\n`).

On found `control.tar[.gz]`, the caller invokes the existing `extract_control_file` helper (unchanged), then `build_entry_from_control` (unchanged except for the `source-mechanism` value).

### Branch 2 — pre-2015 `gzip(tar)` legacy path

**Trigger**: File bytes do NOT start with `!<arch>\n` (falls to this branch by elimination).

**Function**: Existing `parse_ipk_file` code below line 320 (unchanged). Opens the file as `GzDecoder → tar::Archive`, walks entries, extracts `control.tar.gz` + `data.tar.gz`.

**Success**: Emits component with `mikebom:source-mechanism = "ipk-file"` (existing value; byte-identical to pre-m187).

**Failure**: Returns `Err(LegacyGzipTarFallbackFailed(reason))` (renamed from pre-m187's `OuterMalformed`). Every existing malformed-gzip-tar reason string is preserved verbatim in the new variant's payload.

### Branch 3 — filename fallback (US2)

**Trigger**: Both Branch 1 AND Branch 2 returned `Err(_)`.

**Function**: `filename_fallback_entry(path, distro_tag)` (existing signature, updated internals per data-model.md §6).

**Steps**:
1. Extract parent-dir name via `path.parent().and_then(|p| p.file_name())`.
2. Call `parse_ipk_filename(filename, parent_dir_name)`.
3. `parse_ipk_filename` internally decides:
   - Try `parent_dir_arch_match` (Decision 3) — if `Some(prefix)`, use parent-dir as arch, split prefix on first `_` for name/version, set `arch_source = ParentDirectory`.
   - Else fall back to existing rsplit heuristic, set `arch_source = FilenameHeuristic`.
4. Build PURL via existing `build_opkg_purl`.
5. Emit `mikebom:source-mechanism = "ipk-file-filename-fallback"` + `mikebom:arch-source = <arch_source>` on the component.
6. Return `Some(entry)` OR `None` if filename doesn't fit either heuristic.

**Caller emits** a `tracing::warn!` on the salvage path per FR-006 / FR-007 — message includes the specific structural reason (e.g., "ar-format: truncated header" or "legacy gzip-tar: missing control.tar.gz") NOT the pre-m187 generic "legacy ar-format" language.

## Fall-through matrix

| File shape | Branch 1 | Branch 2 | Branch 3 | Emitted `source-mechanism` | Emitted `arch-source` |
|---|---|---|---|---|---|
| Well-formed ar-format (Yocto) | Ok(members) → success | (not reached) | (not reached) | `ipk-file-archive-extraction` | `control-file` |
| Well-formed pre-2015 gzip-tar | (branch 1 skipped — no ar magic) | Ok(...) → success | (not reached) | `ipk-file` | `control-file` |
| Malformed ar body, filename ends `_qemux86_64` in `qemux86_64/` dir | Err(ArMalformed) | (branch 2 skipped — ar magic detected) | filename-fallback via parent-dir | `ipk-file-filename-fallback` | `parent-directory` |
| Malformed ar body, filename `foo_1.0_all` in loose dir | Err(ArMalformed) | (branch 2 skipped — ar magic detected) | filename-fallback via rsplit | `ipk-file-filename-fallback` | `filename-heuristic` |
| Malformed gzip-tar body | (branch 1 skipped) | Err(LegacyGzipTarFallbackFailed) | filename-fallback (per parent-dir rules) | `ipk-file-filename-fallback` | `parent-directory` OR `filename-heuristic` |
| Neither ar nor gzip-tar (unknown format) | (branch 1 skipped — no ar magic) | Err(OuterMalformed OR similar) | filename-fallback | `ipk-file-filename-fallback` | `parent-directory` OR `filename-heuristic` |

## Byte-identity invariant

**FR-014 / SC-005**: For well-formed pre-2015 gzip-tar ipks, the emitted `PackageDbEntry` MUST be byte-identical to pre-m187. Specifically:
- `mikebom:source-mechanism = "ipk-file"` (existing value, unchanged).
- No `mikebom:arch-source` property emitted on the gzip-tar success path (property is only relevant for filename-fallback).
- All other fields (name, version, arch, purl, depends, licenses, extra_annotations) unchanged.

Verified via T031-equivalent golden-regen zero-drift check in Phase 6.

## Test surface

Data-model.md §9 enumerates 10 unit tests + 9 integration tests. Each per-branch outcome in the matrix above has ≥1 dedicated test.
