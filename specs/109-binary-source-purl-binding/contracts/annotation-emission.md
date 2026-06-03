# Contract: annotation-emission — dedup pipeline interaction

## Pre-condition

The matcher rewrite (per `attribution-rules.md`) has already changed `match.target_purl` from `pkg:generic/<library>` to the source-tier PURL. The downstream per-binary `entry::symbol_match_to_entry` builds a `PackageDbEntry` carrying that PURL.

## Per-format emission (unchanged from milestone 105 + 108)

The cmake source-tier component AND the rewritten binary-tier component both exist as `PackageDbEntry` instances when the resolver runs. The resolver-time dedup pipeline (milestone 105) merges them by PURL equality. Post-merge, the component carries:

### CycloneDX 1.6

- `bom-ref` / `purl` = the cmake-derived source-tier PURL (e.g., `pkg:github/madler/zlib@v1.3.1`).
- `properties[]` includes:
  - `mikebom:source-mechanism = "cmake-fetchcontent-git"` (or `cmake-fetchcontent-url`).
  - `mikebom:evidence-kind = "cmake-fetchcontent-git"` (from the source-tier entry).
  - `mikebom:evidence-kind = "symbol-fingerprint"` (from the binary-tier entry — additional row in the bag; existing milestone-105 multi-evidence pattern).
  - `mikebom:fingerprint-symbols-matched = "<count>/<total>"` (from the binary-tier entry).
  - `mikebom:fingerprint-corpus-sha = "<12-hex>"` or `"bundled"` (from the binary-tier entry).
- `evidence.identity[].methods[]` (CDX-native multi-method shape per C56) MAY include both the source-tier mechanism + the binary-tier `symbol-fingerprint` mechanism for parity with the property-bag.

### SPDX 2.3

- `Package.SPDXID` = SHA-derived ID per milestone 011's deterministic scheme.
- `Package.externalRefs[type:purl]` = the cmake-derived source-tier PURL.
- `Package.annotations[]` includes the parity-bridging annotations: `mikebom:source-mechanism`, `mikebom:fingerprint-symbols-matched`, `mikebom:fingerprint-corpus-sha` per their existing C55 / C58 contracts.

### SPDX 3.0.1

- `software_Package` element with `software_packageUrl` = the cmake-derived source-tier PURL.
- Graph-element `Annotation`s carry the same set of `mikebom:` properties as SPDX 2.3.

## Multi-record collision (FR-006) cases

When the binary-tier matcher's FR-013 multi-record collision fires AND attribution applies to one of the colliding records:

- Each colliding record produces a separate `SymbolFingerprintMatch`.
- The matcher rewrite applies independently per-match: if `match[0]` (zlib upstream) has an attribution AND `match[1]` (libressl variant) doesn't, then `match[0]` gets the source-tier PURL and `match[1]` stays at `pkg:generic/libressl?variant=...`.
- The two emitted components remain DISTINCT in the output (no over-merge); the `mikebom:also-detected-via` annotation on each lists the OTHER's library name per milestone 108's FR-013 contract.

## When attribution does NOT apply

The matcher's output is byte-identical to milestone 108 — `pkg:generic/<library>`, no `mikebom:source-mechanism` annotation (since the binary matcher itself doesn't tag source-mechanism for fingerprint matches; only the cmake-derived attribution path attaches it). The cmake source-tier component, if any exists, emits its source-tier annotations independently as it did pre-milestone-109.

This means a non-opt-in scan (`--fingerprints-corpus` off) produces ZERO new annotations on existing components; the cmake source-tier component is unchanged from alpha.44. SC-003 byte-identity holds.

## Parity-extractor implications

NO new catalog rows in `docs/reference/sbom-format-mapping.md`. The existing extractors handle the milestone-109 cases:

- C55 (`mikebom:source-mechanism`) — extractor unchanged; the attributed components just cause more `cmake-fetchcontent-*` values to appear in the per-component output.
- C58 (`mikebom:fingerprint-corpus-sha`) — extractor unchanged; attributed components retain the same corpus-sha as their unattributed milestone-108 counterparts.

The `every_catalog_row_has_an_extractor` invariant (`parity::extractors::tests`) is NOT affected.

## Non-opt-in regression guarantees (SC-003)

When `--fingerprints-corpus` is NOT set:

- The binary matcher's `scan_with_corpus(symbols, bundled, false, None, _)` is called with `None` for the attribution-registry parameter.
- `None` → no lookup → no rewrite → matcher output identical to milestone 108.
- The cmake reader's output is unchanged (the cmake reader is independent of `--fingerprints-corpus`).
- The 33 byte-identity goldens pass byte-identically.

## Single-binary-scan regression guarantees (SC-004)

When the scan target contains a binary but no source tree:

- `cmake_observer::observe(scan_root, &[])` returns an empty `Vec` (no cmake declarations to bind against).
- The `BuildAttributionRegistry` is built empty.
- `registry.lookup(library, binary_path)` returns `None` for every call.
- The matcher's behavior is identical to milestone 108: `pkg:generic/<library>` emitted with the corpus-sha annotation.
