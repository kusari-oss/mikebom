# Milestone 110 — pickup notes for the next session

**Status as of**: 2026-06-08 (alpha.46 shipped — PRs #313–#318 + release #320)
**Where to read next**: this doc → [`spec.md`](./spec.md) → [`plan.md`](./plan.md) → [`tasks.md`](./tasks.md)

This is the handoff doc for resuming milestone-110 work. It captures (a) what's shipped today, (b) what's deliberately deferred from the spec, and (c) recommended next slices with concrete pickup pointers.

## What shipped in alpha.46

Six PRs landed building toward the v2 corpus capability:

| PR | Title | Surface |
|---|---|---|
| #313 | Foundational types + numeric confidence annotation | `CorpusRecordV2`, `IndicatorKind`, `IndicatorSpec`, `Confidence`, `FusedConfidence`, `MatchResult`, `SelfIdentity` (stub), `CorpusSource`, `CorpusSourceId`, `CorpusError` — all in `mikebom-cli/src/scan_fs/binary/fingerprints/`. Public JSON Schema at `docs/reference/corpus-record-v2.schema.json` |
| #314 | C59 parity row | `mikebom:fingerprint-confidence` catalog entry in `docs/reference/sbom-format-mapping.md` + parity-extractor entries |
| #315 | Matcher + "max + bump" fusion algorithm | `matcher::match_binary`, `fuse_confidence`, per-indicator matchers (symbol-set / rodata-literal / exact-hash) |
| #316 | v2 loader | `loader::load_v2_records_from_cache` with record-level schema_version peek dispatch; coexists with v1 in a single cache |
| #317 | Production wiring | `binary/mod.rs` calls `match_binary` after v1 path; `by_library.entry().or_insert_with(...)` gate preserves milestone-108 byte-identity; new `v2_bridge.rs` (rodata extraction + BinaryArtifact builder) + `entry::v2_match_to_entry` |
| #318 | E2E integration test | `tests/fingerprints_v2_e2e.rs` — 3 tests proving v2 records emit versioned PURLs against a real binary via fixture cache |

The v2 pipeline is **end-to-end live in production scans** as of alpha.46. Third-party corpus authors can target the public JSON Schema today and have mikebom load, fuse, and emit versioned PURLs.

## What's deliberately deferred (still in the spec, not yet implemented)

Cross-reference with [`spec.md`](./spec.md)'s user stories and FRs:

### Phase 5 — pluggable corpus sources + auth + fallback (US2, P1)

Today, the v2 loader reads from the SAME single milestone-108 cache directory the v1 loader uses. The spec defines a richer model — multiple configured corpus sources, each optionally authenticated, with per-source sigstore allowed-issuers, graceful fallback across sources.

**Not implemented yet**:
- FR-006 — multiple source URLs configurable via env var + config file + CLI flag
- FR-007 — bearer-token auth via per-source `credential_env` env-var lookup
- FR-008 — per-archive cosign signature verification with per-source allowed-issuers (research R6)
- FR-010 — multi-source partial-fail-degrades-gracefully orchestration
- FR-012a — 24-hour cache TTL with `last_used.touch` sidecar
- New CLI flags: `--fingerprints-source URL[=ENV_VAR]` (repeatable), `--fingerprints-source-no-default`
- The hermetic HTTP fixture server scaffolding from [`contracts/fetch-protocol-v2.md`](./contracts/fetch-protocol-v2.md)

**Effort estimate**: ~13 tasks per the original `tasks.md` (T038–T050). Multi-day work. The matcher + emission paths are unchanged from this slice's perspective — Phase 5 just expands where records can come from.

**Recommended next slice (Phase 5-Slim)**: implement the multi-source URL configuration + the merged-corpus loading, defer auth + per-source allowed-issuers for a follow-on. That delivers the "operators can configure additional public sources" UX without standing up the auth gateway story.

### Phase 6 — multi-indicator confidence fusion polish (US4, P2)

The matcher today returns a single MatchResult per matching record with an empty `also_detected_via`. The spec defines richer cross-referencing for collision cases (e.g., a binary matching BOTH BoringSSL and OpenSSL indicators).

**Not implemented yet**:
- FR-014 — when two records match the same binary at non-suppressed confidence, both components emit with `mikebom:also-detected-via` cross-referencing
- FR-015 — self-identity suppression via the resolver ladder (`--scan-as` override → cmake `project()` → cargo `[package].name` → npm name → PEP 621 → git remote). Currently `SelfIdentity::matches_record` is a stub returning false.

**Effort estimate**: ~14 tasks per the original `tasks.md` (T051–T064). The collision-handling code path is in `matcher.rs::match_binary` (already structured to accept multi-record results, just doesn't populate `also_detected_via`); the self-identity ladder is a new `self_identity.rs::resolve_identity_for_root()` function plus reuse of milestone-064/066/068/073 readers.

**Recommended next slice (Phase 6-Slim)**: implement the self-identity ladder + `SelfIdentity::matches_record`. Skip the collision-handling polish; it's a small downstream change on top of the resolver.

### Phase 7 — polish + cross-cutting

Most of Phase 7 is already done (parity row C59 in #314, docs in spec.md). Remaining:
- A `mikebom:fingerprint-source-id` annotation for v2-derived components (replaces the v1 `mikebom:fingerprint-corpus-sha` whose value space is v1-specific) — needs a new C-row (C60 or later) in the parity catalog. Currently the v2 path emits no source-attribution annotation; consumers can derive the source from the PURL.
- Bug fix in the numeric-vs-native confidence mismatch: today the CDX-native `evidence.identity[0].confidence` is hardcoded to 0.85 in the source-binding code path while the new `mikebom:fingerprint-confidence` annotation carries the matcher's actual fused value. Both should agree.

## How to pick up the next session

```bash
# 1. Read the deferred FRs in the spec
sed -n '/FR-006/,/FR-012/p' spec.md

# 2. Read the research items relevant to Phase 5 / Phase 6
sed -n '/^## R4/,/^## R7/p' research.md  # source config + sigstore + R7 test infra
sed -n '/^## R8/p' research.md          # self-identity ladder design

# 3. Reference the contracts dir for the protocol shape
cat contracts/fetch-protocol-v2.md
cat contracts/cli-flags.md

# 4. The original tasks.md has T015–T074 as the full task breakdown.
#    Currently completed: T001–T015, T021–T022 (Phase 1+2+3 MVP) +
#    the Phase 4 slices delivered via the 6 PRs above.
#    Remaining: T023–T037 (US1 fixture-binary work — partially done via the
#    e2e test in #318), T038–T050 (US2 Phase 5), T051–T064 (US4 Phase 6),
#    T065–T074 (Phase 7).
grep '^- \[ \]' tasks.md | wc -l   # remaining task count
```

## Recommended order if resuming

1. **Phase 5-Slim** — multi-source URL configuration. Biggest user-facing value-add: lets operators consume v2 corpora from sources other than the milestone-108 sibling repo. Auth + cosign-per-source can defer.
2. **Phase 6-Slim — self-identity ladder**. Smaller, self-contained. Fixes the design-doc §7.1 footgun where mikebom would identify a project's own source tree as containing itself.
3. **Phase 7 polish**. Pick up the source-id annotation + the confidence-value mismatch fix.

Each slice is a separate PR. The cadence from alpha.46 worked well — 5–6 PRs of pure-additive code + 1 release PR.

## Pointers to durable artifacts

- Public spec: [`spec.md`](./spec.md) — FR list, success criteria, edge cases, assumptions
- Design rationale: `/Users/mlieberman/Projects/mikebom-design-notes/corpus-v2-symbols-to-purls.md` (private gist `6d2bde7965e67ffa3123d0a5d23ae034`)
- Kusari deployment companion: `/Users/mlieberman/Projects/mikebom-design-notes/corpus-v2-kusari-deployment.md` (when/if Kusari ships a private corpus)
- Public JSON Schema: `docs/reference/corpus-record-v2.schema.json`
- Format-mapping catalog: `docs/reference/sbom-format-mapping.md` (search for C59)
