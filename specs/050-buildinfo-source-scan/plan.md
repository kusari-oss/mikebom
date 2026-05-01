---
description: "Plan — milestone 050 BuildInfo-vs-go.sum scope hint for source-tree Go scans"
---

# Plan: Surface BuildInfo-vs-go.sum scope to source-tree Go users

**Branch**: `050-buildinfo-source-scan` | **Spec**: spec.md ✅
**Output**: 4-file tighter template (no research.md / data-model.md /
contracts/ / quickstart.md — pattern from 021/022/023/042/046/047/048/049).

## Phase 0: Recon (resolved inline; no `research.md`)

### R1. G3 already fires correctly on source-tree scans

**Finding**: `apply_go_linked_filter` in
`mikebom-cli/src/scan_fs/package_db/mod.rs:458-493` is rootfs-
agnostic. It folds over the entire `entries` vec and intersects
source-tier with analyzed-tier on `(name, version)`. When
`go_binary::read` finds at least one Go binary anywhere under
`--path`, G3 fires; when zero binaries are found, it no-ops.

**Verified empirically**: scanning `apigatewayv2/config` with the
built binary copied into the project directory produces 41
components (40 deps + 1 main); the same path without the binary
produces 63. Trace shows `G3 filter: dropped 25 go.sum entries
not confirmed by Go binary BuildInfo`.

**Decision**: no behavior change. The G3 implementation is
correct; we only need to surface the gap when it would benefit
users (binary-not-present case) and document the workflow.

### R2. Detection signal: `go_signals.main_modules.is_empty()` + `go_binary_entries.is_empty()`

**Existing primitives** in `mikebom-cli/src/scan_fs/package_db/mod.rs`:
- `go_signals.main_modules: HashSet<String>` — populated by
  `golang::read` when at least one `go.mod` `module` directive
  is parsed. Empty IFF no Go source-tree was found.
- `go_binary_entries: Vec<PackageDbEntry>` — populated by
  `go_binary::read`. Empty IFF no BuildInfo-readable Go binary
  was found in the rootfs.

**Decision**: emit the hint IFF
`!go_signals.main_modules.is_empty() && go_binary_entries.is_empty()
&& scan_mode == ScanMode::Path`.

The third condition gates the hint to `--path` scans. Container
scans (`--image`) shouldn't get the hint — the user has no
opportunity to run `go build` on the producer's behalf.

### R3. Hint message content

**Decision**: single `tracing::info` line, structured fields plus
a human-readable message. Match the existing G3/G4 trace style:

```rust
tracing::info!(
    go_modules = go_signals.main_modules.len(),
    go_sum_components = source_tier_count,
    "no Go binary found alongside go.mod — SBOM reflects the full \
     go.sum closure (build-tag alternatives + test scaffolding \
     included). Run `go build` and re-scan to filter to the \
     BuildInfo intersection (typically 30-40% smaller).",
);
```

`source_tier_count` is the count of `pkg:golang` entries with
`sbom_tier = "source"` in `out` immediately before the hint
emits. Computed via a single fold.

### R4. Doc surface

**Existing files**:
- `README.md` — has a Go section (verify exact location during impl)
- `docs/cli-reference.md` — `sbom scan --path` flag docs

**Decision**: append a paragraph to the README's Go section
naming the BuildInfo intersection workflow, with the concrete
63→41 number from `apigatewayv2/config`. Touch `cli-reference.md`
only if it has a `--path` flag-detail section that should mention
the binary-detection behavior.

## Implementation strategy

Single PR, one commit (the change is small enough that splitting
hint+docs adds churn).

### Commit 1 — `feat(050): scope hint when source-tree Go scan finds no binary`

**Touched files**:

- **`mikebom-cli/src/scan_fs/package_db/mod.rs`** (~30 LOC)
  - After the G3/G4/G5 chain (line ~706), before the `Ok(DbScanResult ...)`
    return, add a hint-emission block:
    ```rust
    if !go_signals.main_modules.is_empty()
        && go_binary_entries_count == 0
        && scan_mode == ScanMode::Path
    {
        let source_tier_count = out
            .iter()
            .filter(|e| e.purl.ecosystem() == "golang"
                && e.sbom_tier.as_deref() == Some("source"))
            .count();
        tracing::info!(
            go_modules = go_signals.main_modules.len(),
            go_sum_components = source_tier_count,
            "no Go binary found alongside go.mod — SBOM reflects \
             the full go.sum closure. Run `go build` and re-scan \
             to filter to the BuildInfo intersection.",
        );
    }
    ```
  - Capture `go_binary_entries_count = go_binary_entries.len()`
    BEFORE the `out.extend(go_binary_entries)` line at 629
    (after that, the count is moved into `out`).

- **`README.md`** (~10 LOC)
  - Append paragraph to the Go-ecosystem section. Concrete numbers
    from `apigatewayv2/config` (63 source-only / 41 with binary
    present), one-liner workflow.

- **`CHANGELOG.md`** (~6 LOC)
  - `[Unreleased]` → `### Added`: name the diagnostic, no behavior
    change, no flag.

- **`specs/050-buildinfo-source-scan/`** scaffolding (already
  present; gets bundled into the same commit via `git add -A` like
  other recent milestones).

**Verification**:
- `./scripts/pre-pr.sh` clean.
- New integration test in `mikebom-cli/tests/scan_go.rs`:
  synthetic rootfs with `go.mod` + `main.go` (no binary), scan,
  assert stderr contains the hint substring; same rootfs WITH a
  faked Go binary (or just verify the existing
  `scan_go_source_plus_binary_*` tests still pass and are silent
  on the hint).
- All 27 byte-identity goldens unchanged (no SBOM-output deltas).
- 11/11 holistic_parity unchanged.
- Real-world smoke: `mikebom sbom scan --path
  ~/Projects/iac/app-code/apigatewayv2/config` → stderr contains
  hint. Build binary, copy into dir, re-scan → no hint, 41
  components.

## Touched files

| File | LOC | Purpose |
|---|---|---|
| `mikebom-cli/src/scan_fs/package_db/mod.rs` | +25 | Hint emission |
| `README.md` | +10 | Document the BuildInfo workflow |
| `CHANGELOG.md` | +6 | `[Unreleased]` entry |
| `mikebom-cli/tests/scan_go.rs` | +30 | Hint integration test |
| `specs/050-buildinfo-source-scan/` | scaffolding | Spec + plan + tasks |

Total: ~70 LOC of Rust + 10 LOC docs + scaffolding.

## Risks

- **R1: ScanMode threading.** The hint emission needs to know
  whether this is a `--path` or `--image` scan. `ScanMode` is
  already passed to `read_all` per its existing signature
  (`pub fn read_all(... scan_mode: crate::scan_fs::ScanMode ...)`).
  Verify during impl that the value is `Path` for `--path` and
  `Image` for `--image`. If the existing dispatch maps them
  differently (e.g., `Image` is the scan-target post-extraction
  even for `--image` flows), gate on the original CLI invocation
  flag instead. Mitigation: read the existing `ScanMode` enum's
  variants and trace where each is set.

- **R2: Trace-line spam in CI.** Every CI Go-related test that
  scans a synthetic rootfs without a binary will emit the hint.
  Acceptable: it's a single-line `info` and CI logs aren't
  parsed for noise. If it materially degrades signal-to-noise,
  downgrade to `tracing::debug!`.

- **R3: README Go-section location.** Need to find the existing
  Go-ecosystem subsection during impl. If it doesn't exist as a
  distinct subsection, add a brief one rather than scatter.

## Out of scope

- **Auto-running `go build` on the user's behalf**: explicitly
  excluded per spec. Mikebom is read-only against the filesystem.
- **A `--build-first` flag**: same reason; would require a Go
  toolchain in the mikebom runtime, which we don't depend on.
- **Stdout warning**: keeping this tracing-only avoids breaking
  `--output -` pipelines.
- **cargo / gem / maven equivalents**: tracked as a separate
  future milestone (was task #124's "milestone 050" placeholder
  before this milestone took the 050 number; renumbered).
