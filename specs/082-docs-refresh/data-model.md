# Data Model — milestone 082 Documentation refresh and audit

The milestone introduces ZERO new Rust types — it's a docs-only refresh. The "data model" here is the documentation-process entities used by the audit deliverable + the cross-reference graph.

## Entities

### `AuditedDoc` — one row per in-scope file in research.md §1

```text
AuditedDoc {
    path:                   String,    // e.g., "docs/user-guide/cli-reference.md"
    last_touched_milestone: String,    // e.g., "046 (PR #86)" or "very early (PR #17)"
    classification:         enum {     // per the rubric in research §1
        Current,             //   touched at milestone 072+; no obvious post-072 stale claims
        PartiallyStale,      //   touched at milestone 020–071; missing recent operator surface
        MateriallyStale,     //   touched pre-milestone 020 OR demonstrably obsolete
    },
    currency_gap:           String,    // free-text: dominant gap finding (≤200 chars)
    fix_scope:              enum {     // per spec FR + tasks.md task assignments
        SmallStyleOnly,      //   only Q1 style pass needed; no content edits
        SmallInline,         //   small inline content additions ≤30 lines
        SmallAddSeeAlso,     //   add "See also" cross-reference section per FR-003
        MajorRefresh,        //   substantial content rebuild (cli-reference, quickstart, configuration, installation)
        Q2SpotCheck,         //   architecture doc; 5–10 testable claims verified per Q2
    },
}
```

The 22 in-scope files yield 22 `AuditedDoc` rows in research.md §1's table.

### `SpotCheckClaim` — for architecture doc audit (research.md §6)

```text
SpotCheckClaim {
    doc_path:                String,    // architecture doc path
    claim_id:                u8,        // 1..=10 within the doc
    claim_text:              String,    // the testable behavioral claim
    verification_target:     String,    // source code path or test target to verify against
    status:                  enum {     // post-fix per SC-008
        Verified,            //   claim is accurate; no doc edit
        FixedInline,         //   ≤30-line doc edit applied
        FollowUpFiledIssueN, //   GitHub issue # filed; doc retains stale claim with note
    },
}
```

Per Q2: 5–10 claims per architecture doc; ~9 architecture docs × ~7 average = ~50–60 claims total. Verified during T-task execution; status recorded back into research.md §6's per-doc tables.

### `CrossReferenceLink` — for the "See also" graph (FR-003 + SC-003)

```text
CrossReferenceLink {
    from_path:    String,    // e.g., "docs/reference/identifiers.md"
    to_path:      String,    // e.g., "docs/reference/sbom-types.md"
    context_text: String,    // one-line context on the bullet (≤80 chars)
}
```

Per research §4: 5 reference docs × 2–4 outgoing links each ≈ 12–20 links total. SC-003 verification = graph traversal: starting from any reference doc, ≤3 clicks reach any other reference doc.

## Validation rules

- **VR-082-001**: Every in-scope file (per the spec's in-scope list) MUST appear exactly once in research.md §1's audit table.
- **VR-082-002**: For each `AuditedDoc` with `fix_scope = MajorRefresh`, the corresponding tasks.md task MUST cite the FR(s) the refresh closes.
- **VR-082-003**: For each `SpotCheckClaim` per architecture doc, post-fix status MUST be one of {Verified, FixedInline, FollowUpFiledIssueN}. ≥90% per-claim accuracy aggregated across the corpus.
- **VR-082-004**: The cross-reference graph MUST be connected: from any reference doc, ≤3 clicks reach any other reference doc.
- **VR-082-005**: Every fenced code block in any in-scope file MUST have a language tag from the allowed set (per research §2 code-block convention).
- **VR-082-006**: Every inter-doc link in any in-scope file MUST use the no-leading-`./` relative-path convention (per research §2 link convention).
- **VR-082-007**: Every milestone reference in `user-guide/*.md` + `README.md` MUST be replaced with milestone-agnostic prose (per research §2 milestone-reference convention).
- **VR-082-008**: Every milestone reference in `reference/*.md` + `architecture/*.md` + `design-notes.md` + `ecosystems.md` + `index.md` MUST be normalized to parenthetical `(milestone N)` form (per research §2).

## Backward compatibility

- **No code changes**: production Rust code, test code, and byte-identity goldens are untouched.
- **No CI workflow changes**: the new `scripts/verify-docs-currency.sh` is an operator-invoked script; no CI integration in this milestone.
- **Milestone-080/081 audit-record entries** in `docs/reference/sbom-format-mapping.md` Section I are preserved verbatim (durable Principle V audit trail).
- **Milestone-072 cross-tier-binding fixtures** under `docs/reference/binding-fixtures/` are out of scope per the spec; not touched.
- **External link stability**: existing absolute URLs (CISA, CycloneDX, SPDX spec links) preserved; never substituted with relative.
