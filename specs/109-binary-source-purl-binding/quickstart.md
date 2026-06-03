# Quickstart — milestone 109 binary-source PURL binding

Once milestone 109 ships (`v0.1.0-alpha.45` or later), `mikebom sbom scan --fingerprints-corpus` on a cmake project root automatically attributes the binary scanner's fingerprint matches to the source-tier PURL declared in the project's `CMakeLists.txt`. Operators and consumers get ONE component per real library across the whole SBOM.

---

## Scenario 1 — Default operator flow

```bash
$ git clone https://github.com/kusari-sandbox/mikebom-cmake-demo
$ cd mikebom-cmake-demo
$ cmake -S . -B build -G Ninja
$ ninja -C build
$ mikebom sbom scan --path . --output sbom.cdx.json --fingerprints-corpus
```

Inspect the emitted SBOM:

```bash
$ jq '.components[] | select(.purl | contains("zlib")) | {purl, name, evidence_kind: ([.properties[]? | select(.name == "mikebom:evidence-kind") | .value])}' sbom.cdx.json
```

Pre-milestone-109 output (two non-joining components):

```json
{
  "purl": "pkg:github/madler/zlib@v1.3.1",
  "name": "zlib",
  "evidence_kind": ["cmake-fetchcontent-git"]
}
{
  "purl": "pkg:generic/zlib",
  "name": "zlib",
  "evidence_kind": ["symbol-fingerprint"]
}
```

Post-milestone-109 output (ONE component, both evidence kinds):

```json
{
  "purl": "pkg:github/madler/zlib@v1.3.1",
  "name": "zlib",
  "evidence_kind": ["cmake-fetchcontent-git", "symbol-fingerprint"]
}
```

---

## Scenario 2 — Consumer equality-joins source + binary SBOMs

A CI pipeline that emits a source-only SBOM at PR-merge time AND a binary SBOM at release-build time:

```bash
# At PR merge — scan source tree only
$ mikebom sbom scan --path src/ --output source.cdx.json --no-deep-hash

# At release build — scan the project root (source + build/)
$ mikebom sbom scan --path . --output binary.cdx.json --fingerprints-corpus --no-deep-hash

# Triage at vuln-scan time — equality-join by PURL
$ jq -r '.components[].purl' source.cdx.json | sort > source-purls.txt
$ jq -r '.components[].purl' binary.cdx.json | sort > binary-purls.txt

# What's declared but not in the binary? (legitimate "declared but not linked" signal)
$ comm -23 source-purls.txt binary-purls.txt

# What's in the binary but wasn't declared? (legitimate "transitive / vendored / static-linked-but-undeclared" signal)
$ comm -13 source-purls.txt binary-purls.txt
```

Post-milestone-109, neither diff shows phantom mismatches caused by PURL form drift. The diffs are genuine signals about what's declared-vs-shipped.

---

## Scenario 3 — Non-cmake / single-binary scans (no behavior change)

When the scan target doesn't contain a cmake build directory (operator scans only a binary, or operator scans a source tree without `_deps/`), the milestone-108 behavior is preserved exactly:

```bash
$ SCAN_DIR=$(mktemp -d)
$ cp /path/to/some-binary "$SCAN_DIR/"
$ mikebom sbom scan --path "$SCAN_DIR" --fingerprints-corpus --no-deep-hash
$ jq '.components[] | select(.evidence_kind == "symbol-fingerprint") | .purl' out.cdx.json
"pkg:generic/zlib"
```

The `pkg:generic/<library>` PURL is the milestone-108 fallback path; the consumer-side equality-join in Scenario 2 still works against a separately-scanned source SBOM if one exists.

---

## Scenario 4 — Opt-out via the existing flag

When `--fingerprints-corpus` is NOT set (the default), the binary matcher doesn't run at all — emitting BOTH:

- The cmake source-tier components from the source-tree reader.
- ZERO binary-tier fingerprint components.

The 33 byte-identity goldens pass byte-identically: no fingerprint annotations, no attribution, exactly the pre-milestone-108 emission shape. SC-003 contract.

---

## Inspecting the attribution mechanism

When attribution fires, the merged component carries both the source-tier `mikebom:source-mechanism` annotation AND the binary-tier `mikebom:fingerprint-corpus-sha`. Consumers can audit the attribution chain:

```bash
$ jq '.components[] | select(.purl == "pkg:github/madler/zlib@v1.3.1") | .properties' sbom.cdx.json
[
  { "name": "mikebom:source-mechanism",            "value": "cmake-fetchcontent-git" },
  { "name": "mikebom:evidence-kind",               "value": "cmake-fetchcontent-git" },
  { "name": "mikebom:evidence-kind",               "value": "symbol-fingerprint" },
  { "name": "mikebom:fingerprint-symbols-matched", "value": "10/10" },
  { "name": "mikebom:fingerprint-corpus-sha",      "value": "fff39c6ad22c" }
]
```

If you only see `mikebom:source-mechanism = cmake-fetchcontent-git` (no fingerprint annotations), the cmake declaration emitted but the binary scan didn't match — declared-but-not-linked.

If you only see `mikebom:fingerprint-corpus-sha` (no source-mechanism), the attribution didn't fire — either the operator wasn't scanning the project root (single-binary scan) or the cmake declaration name didn't match the fingerprint library name case-insensitively.

---

## What's NOT supported in milestone 109

- **`ExternalProject_Add` declarations**: out of scope per the Phase-2 clarification. Source-tier components still emit; no cross-attribution. Tracked as a follow-on milestone.
- **Bazel / Meson / hand-written Makefiles**: out of scope this milestone. Future observers plug into the same `BuildAttributionRegistry` per FR-012.
- **Cmake declarations with non-conventional names** (`zlib_static`, `ZLIB::ZLIB`, vendor-renamed targets): deliberately don't get attributed per the Phase-1 clarification. Silently aliasing would be worse than emitting both PURLs.
- **Version inference from symbols alone**: a separate research problem; deferred indefinitely. The source-tier PURL's version comes from the cmake declaration, not from the binary.

---

## Further reading

- [Spec](./spec.md) — full functional requirements + edge cases.
- [Plan](./plan.md) — architectural decisions + Constitution Check.
- [Data model](./data-model.md) — `CmakeBuildDirObservation` + `BuildAttributionRegistry` shapes.
- [Walker protocol](./contracts/walker-protocol.md) — discovery algorithm + budgets.
- [Attribution rules](./contracts/attribution-rules.md) — join key + per-record outcomes.
- [Annotation emission](./contracts/annotation-emission.md) — dedup pipeline interaction.
- [milestone-108 quickstart](../108-fingerprint-corpus/quickstart.md) — the external corpus + provenance recipe this milestone builds on.
- [`docs/reference/identifiers.md` §11](../../docs/reference/identifiers.md) — `mikebom:fingerprint-corpus-sha` consumer lookup.
