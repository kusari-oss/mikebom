# Contract — milestone 082 docs style conventions

The milestone's only contract. Future milestones touching docs reference this file rather than re-litigating style choices.

## Three style dimensions

Per the 2026-05-07 Q1 clarification (aggressive normalization across all 22 in-scope files), three style conventions are pinned and applied uniformly. Picked during Phase 0 §2 of milestone 082.

### Dimension 1 — milestone-reference handling

| File category | Convention | Example |
|---|---|---|
| `docs/user-guide/*.md` + `README.md` | **Omitted.** Replace with milestone-agnostic prose. | "The `--sbom-type` flag accepts ..." (NOT "milestone 081 added `--sbom-type`") |
| `docs/reference/*.md` + `docs/architecture/*.md` + `docs/index.md` + `docs/design-notes.md` + `docs/ecosystems.md` | **Parenthetical `(milestone N)` form.** | "The cross-tier binding (milestone 072) bridges source ↔ build ↔ image SBOMs." |

**Forbidden forms** (must be normalized):
- Inline `[milestone N]` — too visually heavy
- Comma-separated `milestone N, ...` without parens — context-ambiguous
- Dash form `milestone-N` — looks like a typo
- Bare `M072` or `m072` — internal-only shorthand

**Source of truth for full milestone history**: `git log` + the per-milestone spec dirs under `specs/`. Operators don't need milestone numbers; maintainers can derive them.

### Dimension 2 — code-block fence convention

**Rule**: every fenced block MUST have a language tag from the allowed set.

| Language tag | Use case |
|---|---|
| `bash` | Shell command examples; `mikebom <noun> <verb>` invocations |
| `json` | JSON literal snippets; SBOM content excerpts; jq output |
| `text` | Plain output; expected stdout; error messages; ASCII diagrams |
| `rust` | Rust code snippets (rare in user-facing; common in reference/architecture for pseudo-types) |
| `yaml` | CI workflow examples; YAML config snippets |
| `toml` | `Cargo.toml` snippets; TOML config examples |
| `markdown` | Nested markdown examples (e.g., showing template syntax) |

**Forbidden**: bare ` ``` ` (no language tag). Phase 0 audit found 4 untagged blocks across 3 files; all get tagged.

### Dimension 3 — inter-doc link convention

**Rule**: relative paths from the file's own directory; NO leading `./`.

| From | To | Correct | Forbidden |
|---|---|---|---|
| `docs/index.md` | `docs/user-guide/quickstart.md` | `[Quickstart](user-guide/quickstart.md)` | `[Quickstart](./user-guide/quickstart.md)` |
| `docs/reference/identifiers.md` | `docs/user-guide/cli-reference.md` | `[CLI ref](../user-guide/cli-reference.md)` | `[CLI ref](./../user-guide/cli-reference.md)` |
| `docs/user-guide/quickstart.md` | `docs/reference/sbom-types.md` | `[SBOM types](../reference/sbom-types.md)` | `[SBOM types](./../reference/sbom-types.md)` |
| Any | External resource (CISA, SPDX spec, etc.) | `[CISA SBOM Types](https://www.cisa.gov/...)` (absolute URL) | (no relative substitution allowed) |

## CLI reference layout (FR-002)

`docs/user-guide/cli-reference.md` is the canonical operator reference. Per-subcommand sections, each with:

```markdown
## `mikebom <subcommand>`

One-paragraph summary of what the subcommand does.

### Quick reference

| Flag | Type | Default | Description |
|---|---|---|---|
| `--<flag-1>` | <type> | <default> | <one-line description> |
| `--<flag-2>` | <type> | <default> | <one-line description> |
| ... | ... | ... | ... |

### `--<flag-1> <VALUE>`

One-paragraph description of the flag.

```bash
mikebom <subcommand> --<flag-1> <example-value>
```

See also: [Deep-dive doc](../reference/<topic>.md).

### `--<deprecated-flag>`

**Deprecated:** since milestone N. Replacement: `--<other-flag>`. Removal target: milestone M (if scheduled).

[brief context]
```

**Subcommand coverage** (alpha.22 set):
- `mikebom sbom scan`
- `mikebom sbom verify`
- `mikebom sbom enrich`
- `mikebom trace run`
- `mikebom trace capture` *(experimental — banner at section top)*
- `mikebom policy init`
- `mikebom verify-binding`
- `mikebom trace-binding`

## "See also" cross-reference convention (FR-003)

Every `docs/reference/*.md` doc ends with:

```markdown
## See also

- [Doc Title](relative-path.md) — one-line context (≤80 chars).
- [Doc Title](relative-path.md) — one-line context.
```

**Constraints**:
- 2–5 bullets per doc (below 2 = orphan; above 5 = link soup).
- Bullet text ≤80 chars including the title and dash.
- Links are nearest-neighbor reference docs ONLY (don't link out to user-guide or architecture).

**Per-doc graph plan** (research §4):
| Doc | "See also" targets |
|---|---|
| `identifiers.md` | cross-tier-binding, sbom-types, sbom-format-mapping, conformance-harness-guide |
| `sbom-types.md` (existing) | identifiers, sbom-format-mapping, cross-tier-binding |
| `cross-tier-binding.md` | identifiers, sbom-types, conformance-harness-guide |
| `sbom-format-mapping.md` | identifiers, sbom-types, conformance-harness-guide |
| `conformance-harness-guide.md` | sbom-format-mapping, identifiers, sbom-types |

## Verification

- **`scripts/verify-docs-currency.sh`** — diffs `mikebom <sub> --help` flag set against `docs/user-guide/cli-reference.md`. Exit 0 = in sync; exit 1 = missing flags listed.
- **Style conformance** — manual review during PR + future milestone PRs reference this contract.
- **Cross-reference graph** — manual click-through during T-task verification + SC-003 ≤3-click reachability check.

## Future-milestone conformance

When milestone-083+ touches docs:
1. Apply the three style dimensions above to any newly-edited file.
2. When adding a new CLI flag: extend `cli-reference.md` per the per-subcommand layout; run `scripts/verify-docs-currency.sh`.
3. When adding a new reference doc: add it to the cross-reference graph (its own "See also" + add reciprocal entries from neighboring docs).
4. When adding a new operator-visible env var: extend `configuration.md`.
5. When adding a deprecation: mark the flag/feature in `cli-reference.md` per the deprecation block format above.
