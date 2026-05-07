# Quickstart — milestone 082 maintainer-facing recipes

Five maintainer-facing recipes for keeping mikebom docs current going forward. After this milestone lands, contributors touching docs reference these recipes (linked from `contracts/docs-style.md`) to avoid re-litigating style choices or missing currency hooks.

## Recipe 1 — Adding a new CLI flag

When introducing a new flag on `mikebom sbom scan` / `mikebom trace run` / etc.:

1. Implement the flag in the appropriate `mikebom-cli/src/cli/<sub>.rs` file via clap derive.
2. Add an entry to `docs/user-guide/cli-reference.md` under the appropriate subcommand section:
   - Add a row to the **Quick reference** table at the top of the section.
   - Add a per-flag detail block below per the contracts/docs-style.md layout.
3. If the flag introduces a new operator-facing concept, link to (or create) a deep-dive `docs/reference/<topic>.md` with the full background. Add a "See also" cross-reference per FR-003.
4. Run `scripts/verify-docs-currency.sh` to confirm the new flag is documented; exit 0 confirms.
5. Style: omit milestone references in the operator-facing user-guide entry; retain `(milestone N)` parenthetical in any reference doc edits.

## Recipe 2 — Adding a new operator-visible environment variable

When mikebom gains a new env var operators or CI need to set:

1. Add the env var read in the relevant Rust source (`mikebom-cli/src/cli/scan_cmd.rs`, `binding/`, or wherever).
2. Add an entry to `docs/user-guide/configuration.md` under the "Environment variables" section. Each entry covers: name, purpose, accepted values, default, example use case.
3. If the env var is for CI lane behavior (e.g., `MIKEBOM_REQUIRE_SPDX3_VALIDATOR`), cross-reference from the relevant reference doc (e.g., `docs/reference/conformance-harness-guide.md`).

## Recipe 3 — Refreshing a quickstart recipe after operator-visible behavior changes

When a milestone changes operator-visible behavior in an existing quickstart recipe:

1. Re-run the recipe against the current `mikebom` binary (post-merge, pre-release-tag).
2. Update `docs/user-guide/quickstart.md` if the actual output differs from the documented snippet.
3. If the change is semantic (e.g., a new field in CDX output), call it out in the recipe text + cross-reference the relevant `docs/reference/<topic>.md`.
4. The quickstart's recipe count stays ≤10 — it's an onboarding doc, not an exhaustive reference.

## Recipe 4 — Adding a new reference doc

When introducing a new operator-facing concept that needs deep-dive documentation:

1. Create `docs/reference/<topic>.md` per the contract layout (overview → sections → "See also" at bottom).
2. Add the new doc to the cross-reference graph: add reciprocal "See also" entries in 2–4 neighboring reference docs.
3. Add the new doc to `docs/index.md`'s navigation under the appropriate track.
4. Verify SC-003-style ≤3-click reachability: starting from any existing reference doc, the operator can reach the new doc in ≤3 clicks.

## Recipe 5 — Deprecating a flag or feature

When a flag or feature is being deprecated (target removal in some future milestone):

1. Mark the flag in `docs/user-guide/cli-reference.md` with the deprecation block format from `contracts/docs-style.md`:
   ```markdown
   ### `--<deprecated-flag>`

   **Deprecated:** since milestone N. Replacement: `--<other-flag>`. Removal target: milestone M (if scheduled).

   [brief context]
   ```
2. Add a stderr deprecation warning in the implementation (per the milestone-052/part-3 + milestone-018 patterns).
3. The replacement flag MUST be documented + working before deprecation.
4. When the deprecation removal milestone lands, the flag entry is REMOVED from `cli-reference.md` (not just marked obsolete) and `git log` carries the history.

## Verification (the "is the docs current?" gate)

Before merging any PR that touches docs:
1. Run `bash scripts/verify-docs-currency.sh` — exit 0 confirms CLI reference is in sync with actual flag set.
2. Visually review the diff against the three style dimensions in `contracts/docs-style.md`.
3. Click any "See also" links in touched reference docs to confirm they resolve correctly.
4. The standard pre-PR gate (`./scripts/pre-pr.sh` — clippy + cargo test workspace) MUST stay clean — docs changes must NOT cause test regressions.

## When in doubt

- **Style question**: read `contracts/docs-style.md`. If still ambiguous, copy the convention from the most-recent in-scope reference doc (`docs/reference/sbom-types.md` from milestone 081 is the freshest exemplar).
- **Scope question** (should this go in user-guide or reference?): operator-facing → user-guide; deep-dive technical / wire-format / Principle V audit-record → reference; design rationale → architecture.
- **Currency question** (is this stale?): compare against `git log` for the source file the doc describes; if the source has been touched since the doc, the doc is a candidate for currency review.
