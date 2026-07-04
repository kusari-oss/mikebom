# Annotation Wire Contracts: Milestone 160

**Date**: 2026-07-04
**Feature**: [spec.md](../spec.md) | **Plan**: [plan.md](../plan.md) | **Data Model**: [data-model.md](../data-model.md)

Per-format wire shapes for the 4 new annotations (C108/C109 per-component + C110/C111 document-scope). All 4 use raw string values (no envelope JSON), matching the milestone-158 (C104/C105) + milestone-159 (C106/C107) precedent.

## C108 — `mikebom:go-transitive-source` (per-component, universal per Q2)

### CycloneDX 1.6

```json
{
  "type": "library",
  "name": "cobra",
  "version": "v1.9.1",
  "purl": "pkg:golang/github.com/spf13/cobra@v1.9.1",
  "properties": [
    {"name": "mikebom:go-transitive-source", "value": "proxy-fetch"}
  ]
}
```

### SPDX 2.3

```json
{
  "SPDXID": "SPDXRef-Package-github.com-spf13-cobra-v1.9.1",
  "name": "cobra",
  "versionInfo": "v1.9.1",
  "annotations": [
    {
      "annotationDate": "2026-07-04T00:00:00Z",
      "annotationType": "OTHER",
      "annotator": "Tool: mikebom",
      "comment": "mikebom:go-transitive-source=proxy-fetch"
    }
  ]
}
```

### SPDX 3.0.1

```json
{
  "type": "Annotation",
  "spdxId": "...LicenseRef-mikebom-go-transitive-source-<sha>",
  "creationInfo": "_:CreationInfo-mikebom-scan",
  "annotationType": "other",
  "statement": "mikebom:go-transitive-source=proxy-fetch",
  "subject": "spdx:Package/cobra-v1.9.1"
}
```

**Value vocabulary** (closed, 5-code enum per Q1/E1):

| Value | Meaning |
|-------|---------|
| `go-mod-graph` | Step 1: `go mod graph` subprocess resolved this module. |
| `module-cache` | Step 2: `$GOMODCACHE` walk found this module. |
| `proxy-fetch` | Step 3: `$GOPROXY` HTTP fetch resolved this module. |
| `go-sum-fallback` | Step 5: milestone-091 go.sum-driven flat fallback. |
| `unresolved` | No step succeeded. C109 MUST accompany. |

**Emission universality (Q2)**: MUST appear on every Go component (`purl.starts_with("pkg:golang/")`), including main-module components.

## C109 — `mikebom:go-transitive-unresolved-reason` (per-component, conditional)

### CycloneDX 1.6

```json
{
  "properties": [
    {"name": "mikebom:go-transitive-source", "value": "unresolved"},
    {"name": "mikebom:go-transitive-unresolved-reason", "value": "proxy-fetch-not-found"}
  ]
}
```

### SPDX 2.3

```json
{
  "annotations": [
    {"comment": "mikebom:go-transitive-source=unresolved", ...},
    {"comment": "mikebom:go-transitive-unresolved-reason=proxy-fetch-not-found", ...}
  ]
}
```

### SPDX 3.0.1

```json
{
  "type": "Annotation",
  "statement": "mikebom:go-transitive-unresolved-reason=proxy-fetch-not-found",
  "subject": "spdx:Package/...",
  ...
}
```

**Value vocabulary** (closed 7-code enum per FR-003/E2):

| Value | Fetch-layer origin |
|-------|--------------------|
| `proxy-fetch-timeout` | `ErrorClass::{Timeout, Http5xx, Dns, Connection, Tls}` |
| `proxy-fetch-not-found` | `ErrorClass::{Http404, Http4xx-non-403}` |
| `proxy-fetch-forbidden` | `ErrorClass::Http4xx` with `403` in detail |
| `proxy-off-in-chain` | Proxy chain contained `off`; step 3 skipped |
| `goprivate-matched` | `GOPRIVATE` pattern matched; module intentionally not fetched |
| `module-cache-miss` | Step 2 cache lookup failed AND step 3 unavailable |
| `unknown-error` | `ErrorClass::{Parse, Other}` |

**Conditional emission**: MUST appear iff C108's value == `"unresolved"`. MUST NOT appear otherwise.

## C110 — `mikebom:go-transitive-coverage` (document-scope, universal-if-Go)

### CycloneDX 1.6

```json
{
  "metadata": {
    "properties": [
      {"name": "mikebom:go-transitive-coverage", "value": "partial"}
    ]
  }
}
```

### SPDX 2.3

```json
{
  "annotations": [
    {
      "annotationDate": "2026-07-04T00:00:00Z",
      "annotationType": "OTHER",
      "annotator": "Tool: mikebom",
      "comment": "mikebom:go-transitive-coverage=partial"
    }
  ]
}
```

Placed at document scope (not attached to any specific package's `annotations` array).

### SPDX 3.0.1

```json
{
  "type": "Annotation",
  "creationInfo": "_:CreationInfo-mikebom-scan",
  "annotationType": "other",
  "statement": "mikebom:go-transitive-coverage=partial",
  "subject": "spdx:SpdxDocument"
}
```

**Value vocabulary** (closed 3-code per FR-004/E3):

| Value | Semantic |
|-------|----------|
| `complete` | Every Go module in the scan had transitive requires resolved via steps 1–4 of the ladder. |
| `partial` | Ladder ran but ≥1 module ended `unresolved`. C111 MUST accompany. |
| `unknown` | Ladder couldn't run (offline / `off` in GOPROXY chain / `go mod graph` subprocess degraded). C111 MUST accompany. |

**Universality**: MUST appear iff the emitted SBOM contains ≥1 Go component. MUST NOT appear on Go-free scans (per SC-003 dual-side byte-identity guard).

## C111 — `mikebom:go-transitive-coverage-reason` (document-scope, conditional)

### CycloneDX 1.6

```json
{
  "metadata": {
    "properties": [
      {"name": "mikebom:go-transitive-coverage", "value": "partial"},
      {"name": "mikebom:go-transitive-coverage-reason",
       "value": "proxy-fetch-degraded: 45 of 300 modules unresolved"}
    ]
  }
}
```

### SPDX 2.3

```json
{
  "annotations": [
    {"comment": "mikebom:go-transitive-coverage=partial", ...},
    {"comment": "mikebom:go-transitive-coverage-reason=proxy-fetch-degraded: 45 of 300 modules unresolved", ...}
  ]
}
```

### SPDX 3.0.1

```json
{
  "type": "Annotation",
  "statement": "mikebom:go-transitive-coverage-reason=proxy-fetch-degraded: 45 of 300 modules unresolved",
  "subject": "spdx:SpdxDocument",
  ...
}
```

**Value grammar** (FR-005 + Q4 closed-but-extensible):

```text
reason        ::= entry (";" WS entry)*
entry         ::= code ":" WS detail
code          ::= "proxy-fetch-degraded"
                | "offline-mode"
                | "goproxy-off-in-chain"
                | "go-mod-graph-degraded"
                | "module-cache-empty-and-no-proxy"
detail        ::= <UTF-8 string, may include N-of-M counts or GOPROXY chain>
WS            ::= " "
```

**Conditional emission**: MUST appear iff C110's value ∈ `{"partial", "unknown"}`. MUST NOT appear when C110 == `"complete"`.

## Parity catalog integration

All 4 rows use the milestone-127 macro pattern at `mikebom-cli/src/parity/extractors/{cdx,spdx2,spdx3}.rs`:

```rust
// cdx.rs
cdx_anno!(c108_cdx, "mikebom:go-transitive-source",             component);
cdx_anno!(c109_cdx, "mikebom:go-transitive-unresolved-reason",  component);
cdx_anno!(c110_cdx, "mikebom:go-transitive-coverage",           document);
cdx_anno!(c111_cdx, "mikebom:go-transitive-coverage-reason",    document);

// spdx2.rs
spdx23_anno!(c108_spdx23, "mikebom:go-transitive-source",             component);
spdx23_anno!(c109_spdx23, "mikebom:go-transitive-unresolved-reason",  component);
spdx23_anno!(c110_spdx23, "mikebom:go-transitive-coverage",           document);
spdx23_anno!(c111_spdx23, "mikebom:go-transitive-coverage-reason",    document);

// spdx3.rs
spdx3_anno!(c108_spdx3, "mikebom:go-transitive-source",             component);
spdx3_anno!(c109_spdx3, "mikebom:go-transitive-unresolved-reason",  component);
spdx3_anno!(c110_spdx3, "mikebom:go-transitive-coverage",           document);
spdx3_anno!(c111_spdx3, "mikebom:go-transitive-coverage-reason",    document);
```

Registration in `mikebom-cli/src/parity/extractors/mod.rs` (adjacent to the C104/C105/C106/C107 block):

```rust
ParityExtractor { row_id: "C108", label: "mikebom:go-transitive-source",             cdx: c108_cdx, spdx23: c108_spdx23, spdx3: c108_spdx3, directional: Directionality::SymmetricEqual, order_sensitive: false },
ParityExtractor { row_id: "C109", label: "mikebom:go-transitive-unresolved-reason",  cdx: c109_cdx, spdx23: c109_spdx23, spdx3: c109_spdx3, directional: Directionality::SymmetricEqual, order_sensitive: false },
ParityExtractor { row_id: "C110", label: "mikebom:go-transitive-coverage",           cdx: c110_cdx, spdx23: c110_spdx23, spdx3: c110_spdx3, directional: Directionality::SymmetricEqual, order_sensitive: false },
ParityExtractor { row_id: "C111", label: "mikebom:go-transitive-coverage-reason",    cdx: c111_cdx, spdx23: c111_spdx23, spdx3: c111_spdx3, directional: Directionality::SymmetricEqual, order_sensitive: false },
```

## Consumer jq recipes

```bash
# Enumerate Go modules per resolution source (CDX)
jq '.components[]
    | select(.purl // "" | startswith("pkg:golang/"))
    | {purl: .purl,
       source: (.properties // [] | map(select(.name == "mikebom:go-transitive-source")) | .[0].value)}' \
    sbom.cdx.json

# Count Go modules per source
jq '[.components[]
     | select(.purl // "" | startswith("pkg:golang/"))
     | .properties // []
     | .[]
     | select(.name == "mikebom:go-transitive-source")
     | .value]
    | group_by(.)
    | map({(.[0]): length})
    | add' \
    sbom.cdx.json

# Check overall Go transitive coverage (doc-scope)
jq '.metadata.properties // []
    | map(select(.name == "mikebom:go-transitive-coverage"))
    | .[0].value' \
    sbom.cdx.json

# Fail if not "complete" (CI gate)
if [ "$(jq -r '.metadata.properties[] | select(.name == "mikebom:go-transitive-coverage") | .value' sbom.cdx.json)" != "complete" ]; then
    echo "SBOM has degraded Go transitive coverage; failing"
    exit 1
fi
```

## Byte-identity guarantee (SC-003)

For non-Go SBOMs (scans producing zero components with `purl` matching `pkg:golang/*`):

- C108/C109 MUST NOT appear on any component.
- C110/C111 MUST NOT appear at document scope.

This is the guard that keeps SC-003 achievable: 10 of 11 milestone-090 fixtures × 3 formats = 30 goldens remain byte-identical to pre-160.
