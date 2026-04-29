# Phase 0 Research: Minimal-Image Per-File Evidence

**Feature**: 038-minimal-image-deep-hash
**Status**: Phase 0 complete

The spec carries no `[NEEDS CLARIFICATION]` markers, so this
document captures the design decisions made during planning and
flags one open recon item that resolves at implementation time
(US2: chainguard apko apk variant).

---

## R1 — Companion-file location in the per-package layout

**Question**: Where does the per-package metadata layout
(distroless / chainguard / Bazel-built) place the file-listing
companion that mirrors `info/<pkg>.list` and `info/<pkg>.md5sums`?

**Decision**: `var/lib/dpkg/status.d/<pkg>.md5sums` carries
`<32-hex-md5>  <relative-path>` lines, the same shape as
`info/<pkg>.md5sums` in the legacy layout. There is **no**
companion `<pkg>.list` file — distroless builds intentionally
strip those because the path list is derivable from the
`.md5sums` file's second column.

**Rationale**: Bazel-rules-distroless documents this layout and
the `gcr.io/distroless/static-debian12:latest` image observed
during milestone 037 smoke testing matches it. syft and trivy
both implement against the same shape.

**Implementation consequence**: the existing
`file_hashes.rs::read_info_file(rootfs, pkg, arch, "list")` returns
`None` for status.d/-only images. We need TWO changes:

1. Extend `read_info_file` / `read_info_file_bytes` to also try
   `var/lib/dpkg/status.d/<pkg>.<ext>` after the legacy info/
   paths. This handles the `.md5sums` lookup naturally (the
   status.d/-layout file IS at that path).
2. In `hash_package_files`, when the `.list` lookup fails, fall
   back to deriving the path list from the second column of
   `.md5sums`. The existing per-file SHA-256 loop is otherwise
   unchanged.

**Alternatives considered**:

- *Construct a full `<pkg>.list` synthetically and store it in
  memory*: would require a new function. Rejected — direct fall-
  back inside `hash_package_files` is simpler.
- *Skip per-file evidence entirely for status.d/ packages*:
  rejected — closes the milestone-038 P1 user story negatively.

---

## R2 — chainguard apko apk-DB layout

**Question**: Does chainguard apko emit a non-standard apk
metadata layout (analogous to dpkg's `status.d/`), or does it use
the standard `/lib/apk/db/installed`?

**Decision**: **Defer to implementation-phase recon (US2 P2).**
Per the spec's US2 contract, the milestone determines this
empirically by extracting a chainguard image and inspecting its
`/lib/apk/db/` layout. The spec is structured to accommodate
either outcome:

- **If apko uses standard apk DB layout**: the existing apk
  reader already covers it. US2 becomes a smoke-test verification
  + docs entry. Zero production code.
- **If apko uses a variant layout**: implement the variant
  reader following the same shape as the dpkg status.d/ work.
  Estimated ≤ 100 LOC.

**Rationale**: ten minutes of recon at implementation time
resolves a question that would take longer to answer
authoritatively from existing documentation alone. Pre-recon
speculation about apko's layout would risk over-scoping the spec.

**Implementation guard**: the recon step is the FIRST task in
the implementation tasks for US2 (T01x in tasks.md). Its outcome
gates whether subsequent US2 tasks become "smoke test" or "code
+ smoke test."

---

## R3 — Right seam in `file_hashes.rs`

**Question**: Should the legacy / status.d/ fork happen at
`read_info_file` (so `read_info_file` becomes layout-aware), or
at `hash_package_files` (so the caller decides which source to
consult)?

**Decision**: **At `read_info_file`** (and the bytes variant).
Make the helper layout-aware: try `info/<pkg>.<ext>` →
`info/<pkg>:<arch>.<ext>` → `status.d/<pkg>.<ext>`.

In addition, `hash_package_files` gains a fallback: when
`read_info_file(.., "list")` returns `None`, try to synthesize the
path list from `read_info_file(.., "md5sums")`'s second column.

**Rationale**: keeps the source-discovery logic centralized in
the two `read_info_file*` helpers (one place to extend in the
future), and keeps `hash_package_files` focused on the hashing
loop. The synthesize-list-from-md5sums step is a small surgical
addition, not a parallel code path.

**Alternatives considered**:

- *Pass an explicit `MetadataLayout` enum through every helper*:
  rejected — too invasive for the size of the change. The
  layout determination is entirely path-existence based and
  doesn't need to be threaded through the API.
- *Two-pass scan: detect layout once at `dpkg.rs::read`, then
  pass the layout marker to file_hashes.rs*: rejected — adds
  cross-module coupling for no observable benefit. Path-existence
  checks are cheap on local filesystem.

---

## R4 — `--no-deep-hash` fast path under status.d/

**Question**: Does the existing `hash_md5sums_only` fast path
work unchanged for status.d/ images once `read_info_file_bytes`
gains the status.d/ fallback?

**Decision**: **Yes.** `hash_md5sums_only` calls
`read_info_file_bytes(rootfs, pkg, arch, "md5sums")` and SHA-256s
the raw bytes. Once `read_info_file_bytes` is extended to find
`status.d/<pkg>.md5sums`, `hash_md5sums_only` returns the right
hash with no further changes.

**Rationale**: the fast-path's contract is "hash whatever the
.md5sums file is" — it doesn't care which directory it came from.
Spec FR-003 (the fast-path produces empty per-file evidence on
status.d/ images, matching full-fat behavior) is satisfied
because `hash_md5sums_only` doesn't populate occurrences at all
under any layout.

---

## R5 — Mixed-layout images (dedup precedence)

**Question**: For a pathological image with both
`info/<pkg>.list` AND `status.d/<pkg>.md5sums` for the same
package, which source provides the per-file evidence?

**Decision**: `info/<pkg>.list` wins because it appears first in
the `read_info_file` lookup chain.

**Rationale**: the legacy layout is the more complete source
(both `.list` and `.md5sums` available); status.d/ is the
fallback. This is consistent with milestone 037's dpkg-source
dedup, where status.d/ wins for *metadata stanza collisions*
because mixed-layout images in the wild have status.d/ as the
authoritative source — but for *file-list collisions* (which
require both layouts to be partially present), the more complete
data wins.

**Alternatives considered**:

- *Always prefer status.d/ for per-file evidence to match
  milestone-037 dedup*: rejected — the goal is "best evidence
  available", and the .list file's path-list is more reliable
  than synthesizing from .md5sums when both are present.

---

## Open at implementation time

- **Q (US2)**: Chainguard apko apk-DB layout → resolved by
  inspection in tasks T01x.

No items remain that block plan execution.
