# Feature Specification: Helm chart scanning (Chart.yaml + Chart.lock + charts/*.tgz + templates/*.yaml)

**Feature Branch**: `188-helm-chart-scanning`
**Created**: 2026-07-13
**Status**: Draft
**Input**: User description from #455: "Helm is the dominant Kubernetes package manager. Helm charts are themselves bundles of dependencies (chart deps + image refs in templated manifests) — and no SBOM scanner handles charts well today. Two layers: (a) chart-level — parse Chart.yaml + Chart.lock + charts/*.tgz dependencies, each becomes `pkg:helm/<repo>/<chart>@<version>`; (b) template-level — extract container image refs from `image: <ref>` lines in `templates/*.yaml`, each becomes `pkg:oci/...` or `pkg:docker/...`. CLI: `mikebom sbom scan --helm-chart <path-or-tarball>`. Template extraction needs YAML-with-Go-template tolerance — `{{ .Values.image.tag }}` placeholders emit with `mikebom:image-ref-unresolved = true` annotation. Optional rendering pass shells out to `helm template` gated behind `--helm-render`."

## Clarifications

### Session 2026-07-13

- Q: What does `--helm-chart <path>` do vs `--path <path>` when a `Chart.yaml` is present? → A: `--path` auto-detects Chart.yaml AND emits helm components alongside other package-DB output (matches npm / cargo / ipk auto-detect convention). `--helm-chart` is the TARBALL-UNLOCK flag ONLY — it extracts a `.tgz` chart archive to a tempdir before scan; behavior against a directory input is identical to `--path`. Other package-DB readers (npm, cargo, etc.) run in parallel when their manifests are also present inside the chart tree — composability is preserved.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Chart-level dependency enumeration (Priority: P1)

An operator running mikebom against a Kubernetes chart directory (`ingress-nginx/`, `cert-manager/`, or a locally-authored chart) wants mikebom to emit one SBOM component per chart dependency declared in the chart's metadata. This lets Kubernetes-focused SBOM consumers (Kusari-style compliance workflows, cluster-audit tools, security scanners) understand which upstream charts contribute to a bundled deployment without manually decoding chart YAML.

**Why this priority**: This is the foundational data-fidelity layer. Without chart-level component enumeration, mikebom's Helm output would be indistinguishable from a generic YAML scan. Chart dependencies are the primary supply-chain-relevant metadata in a Helm chart — every chart dep is a potential vulnerability entry point. Existing SBOM tools (syft, trivy) either skip Helm entirely or produce trivially-poor output; delivering this correctly is the differentiator called out in the issue rationale.

**Independent Test**: Point mikebom at any Helm chart directory containing a `Chart.yaml` with at least one `dependencies[]` entry. Invoke `mikebom sbom scan --helm-chart <chart-dir> --format cyclonedx-json --output out.cdx.json`. Verify (a) the emitted CDX document contains a root component for the chart itself with PURL `pkg:helm/<repo-or-alias>/<chart>@<version>`, (b) one `pkg:helm/<dep-repo>/<dep-name>@<dep-version>` component per `dependencies[]` entry, (c) each chart-dep component carries `mikebom:evidence-kind = "helm-chart-yaml"` or `helm-chart-lock` naming the source file.

**Acceptance Scenarios**:

1. **Given** a chart directory containing `Chart.yaml` with 3 `dependencies[]` entries + no `Chart.lock`, **When** mikebom scans it, **Then** the emitted SBOM MUST contain 4 helm components (the chart itself + 3 deps), each with a valid `pkg:helm/...` PURL and `mikebom:evidence-kind = "helm-chart-yaml"`.
2. **Given** a chart directory containing both `Chart.yaml` (declared deps) AND `Chart.lock` (locked versions), **When** mikebom scans it, **Then** dep versions MUST come from `Chart.lock` (the authoritative resolution), analogous to package-lock.json precedence over package.json in the npm reader. Every locked dep MUST carry `mikebom:evidence-kind = "helm-chart-lock"`.
3. **Given** a chart directory with `charts/<subchart>.tgz` packaged dependencies alongside `Chart.yaml`, **When** mikebom scans it, **Then** each `.tgz` subchart MUST be recognized as a chart dep AND its own `Chart.yaml` (from inside the tarball) MUST be parsed to recursively emit ITS chart deps. Depth limit MUST match existing scan-tree conventions (m114 depth cap = 12).
4. **Given** a chart whose `Chart.yaml` declares a dep with a repository URL (`repository: https://charts.bitnami.com/bitnami`), **When** mikebom emits the dep component, **Then** the PURL's `<repo>` segment MUST be the URL-encoded repository host (`pkg:helm/charts.bitnami.com/nginx@13.0.0`) OR the repository alias if declared via `dependencies[].alias`. If neither is available, MUST fall back to `pkg:generic/<chart>@<version>` with a WARN log naming the ambiguity.
5. **Given** a chart tarball at `<path>/mychart-1.0.0.tgz` (rather than a directory), **When** mikebom scans it via `mikebom sbom scan --helm-chart <path>/mychart-1.0.0.tgz`, **Then** mikebom MUST extract the tarball to a tempdir, parse its `Chart.yaml`, and emit identical output as if the operator had scanned the extracted directory. Byte-identity for the chart-tier components.

---

### User Story 2 — Template-level image-reference extraction (Priority: P1)

An operator wants mikebom to enumerate every container image referenced from a Helm chart's `templates/*.yaml`, including handling of Go-template placeholders (`{{ .Values.image.repository }}:{{ .Values.image.tag }}`) which are the DOMINANT shape in the wild. Rendered-image extraction (via `helm template`) is deferred to US3 as opt-in; US2 delivers the always-on default that scans templates as-is.

**Why this priority**: Chart-level components (US1) tell you which charts are involved, but the ACTUAL VULNERABILITY SURFACE for a Kubernetes deployment is the container images running inside pods. Every Helm-based deployment ultimately produces N container images per resource. Emitting these — even in unresolved-placeholder form — gives operators the "which images MIGHT be pulled" list that today requires running `helm template` manually. This delivers immediate operator value even in the offline / no-helm-binary case.

**Independent Test**: Point mikebom at a chart directory containing `templates/deployment.yaml` with at least one `image: <ref>` line. Verify (a) each container image reference surfaces as a component with a valid `pkg:oci/...` or `pkg:docker/...` PURL, (b) unresolved templated refs (containing `{{ ... }}`) carry `mikebom:image-ref-unresolved = true` as a property, (c) resolved refs (no `{{ ... }}`) do NOT carry that property. Zero image refs surfaced as components → the chart is templated with 100% placeholders (still a valid outcome per the transparency contract).

**Acceptance Scenarios**:

1. **Given** a template file `templates/deployment.yaml` containing `image: nginx:1.27.0` and `image: busybox:latest`, **When** mikebom scans the chart, **Then** 2 image-ref components MUST be emitted with PURLs `pkg:docker/nginx@1.27.0` and `pkg:docker/busybox@latest` (or `pkg:oci/library/nginx@1.27.0` if a registry qualifier is inferable). Neither MUST carry `mikebom:image-ref-unresolved`.
2. **Given** a template with `image: "{{ .Values.image.repository }}:{{ .Values.image.tag }}"`, **When** mikebom scans it, **Then** exactly ONE image-ref component MUST be emitted with a placeholder PURL (`pkg:generic/{{Values.image.repository}}@{{Values.image.tag}}` or an equivalent quotable form) AND `mikebom:image-ref-unresolved = true` as a property. Downstream consumers can filter these out.
3. **Given** a template with a mixed ref like `image: "registry.example.com/{{ .Values.image.name }}:v1.2.3"`, **When** mikebom scans it, **Then** the emitted component MUST carry `mikebom:image-ref-unresolved = true` (because part of the string is unresolved) but the resolved fragments (registry host + version tag) MUST be preserved in the raw-value annotation for observability.
4. **Given** a chart with 5 template files, each containing image refs, **When** mikebom scans it, **Then** each unique image ref (post-normalization) MUST emit ONE component; duplicates (same image referenced from multiple templates) MUST be collapsed to a single component with `evidence.occurrences[]` listing all source template paths.
5. **Given** a template file that is not valid YAML due to unresolved Go-template blocks breaking the parse (`{{ if .Values.enabled }}` opening a block that closes several YAML documents later), **When** mikebom scans it, **Then** mikebom MUST fall back to line-based regex extraction of `image:` lines (not YAML parsing) AND emit a WARN log naming the template file and the parse-failure reason. Zero silent drops.

---

### User Story 3 — Rendered-image extraction via `--helm-render` (Priority: P2)

An operator with the `helm` CLI installed wants mikebom to shell out to `helm template <chart-dir>` to produce fully-rendered manifests before extracting image refs. This resolves every `{{ .Values.image.tag }}` placeholder to a concrete image string, dramatically improving fidelity for real-world charts that ship with template-heavy defaults.

**Why this priority**: Delivers the highest-fidelity image-ref extraction, but requires the `helm` binary to be present on the scan host AND requires the chart to be complete (all deps present + resolvable at render time). US1 + US2 already deliver useful output without this dependency; US3 is the "if you can afford helm, upgrade fidelity" path. Deferred to P2 so mikebom's core value doesn't hinge on an external binary.

**Independent Test**: Point mikebom at a chart directory with a helm binary on `$PATH`. Invoke `mikebom sbom scan --helm-chart <chart-dir> --helm-render --format cyclonedx-json --output out.cdx.json`. Verify (a) mikebom shells out to `helm template <chart-dir>` (verifiable via strace/dtruss), (b) the emitted image-ref components have zero `mikebom:image-ref-unresolved = true` properties (all placeholders resolved), (c) if the `helm` binary is absent, mikebom emits a WARN log naming the missing binary AND falls back to US2's unrendered extraction (no scan failure).

**Acceptance Scenarios**:

1. **Given** a chart directory + `helm` binary on `$PATH` + `--helm-render` flag, **When** mikebom scans, **Then** every emitted image-ref component MUST NOT carry `mikebom:image-ref-unresolved = true` (all templates rendered) AND the shell-out MUST complete within a bounded time (default 60s per FR-018 timeout).
2. **Given** the same setup BUT the `helm template` shell-out fails (exits non-zero, e.g., chart deps missing), **When** mikebom scans, **Then** mikebom MUST emit a WARN log naming the exit code + first 20 lines of stderr AND fall back to US2's unrendered extraction. The final SBOM MUST STILL be produced.
3. **Given** `--helm-render` is passed BUT the `helm` binary is not on `$PATH`, **When** mikebom scans, **Then** mikebom MUST emit a WARN log naming the missing binary + suggesting installation AND fall back to US2's unrendered extraction. The scan MUST NOT fail.
4. **Given** `--helm-render` is NOT passed (default), **When** mikebom scans a chart, **Then** mikebom MUST NOT invoke the `helm` binary AND MUST use US2's unrendered extraction path. Zero external-binary calls in the default flow.

---

### Edge Cases

- **`Chart.yaml` missing** — the scan target is not a Helm chart. mikebom MUST NOT emit any helm components; the scan proceeds through the standard multi-reader pipeline as if `--helm-chart` were not passed. WARN log optional. Applies to both `--path` auto-detection and explicit `--helm-chart <path>` invocations where the path doesn't contain `Chart.yaml`.
- **Auto-detection via `--path`** — mikebom's `--path <dir>` scan MUST auto-detect Helm charts when `<dir>/Chart.yaml` exists (matches npm/cargo auto-detection). Helm components MUST be emitted alongside any other package-DB reader that ALSO matches the same tree (e.g., a chart containing `package.json` will emit BOTH helm components AND npm components — composability preserved). The `--helm-chart <path>` flag is the tarball-unlock convenience for `.tgz` inputs; directory-input behavior is identical to `--path`.
- **Chart tarball at the root** — when `--helm-chart <path>.tgz` is invoked, mikebom extracts to a tempdir + scans the extracted contents. The tempdir is cleaned up after emission.
- **Recursive chart deps** — `charts/<subchart>.tgz` may itself contain `charts/<subsubchart>.tgz`. mikebom MUST recursively parse up to the m114 depth cap (12); deeper trees emit a WARN log and are skipped.
- **`Chart.lock` conflicts with `Chart.yaml`** — if `Chart.lock` lists a version that differs from `Chart.yaml`'s dependency `version`, `Chart.lock` wins (analogous to package-lock.json semantics) with a `mikebom:helm-lock-authoritative = "true"` annotation.
- **CRDs under `crds/*.yaml`** — mikebom MUST also scan CRD YAML files for image refs (some CRDs reference operator images). Same `image:` regex-and/or-YAML-walk extraction as `templates/*.yaml`.
- **Non-`image:` container refs** — Kubernetes CRDs may nest images under paths like `spec.image`, `containers[].image`, `initContainers[].image`, `containerImage:`, or vendor-specific keys. mikebom's DEFAULT scanner uses a permissive regex matching `^(\s*)image:\s*['"]?([^'"\s]+)['"]?\s*(#.*)?$` to catch the standard shape. Non-standard paths are not exhaustively enumerated in m188; a `mikebom:image-extraction-completeness = "partial"` annotation flags this.
- **Tagged vs digested image refs** — `nginx:1.27.0` → `pkg:docker/nginx@1.27.0`; `nginx@sha256:abc123...` → `pkg:oci/nginx@sha256:abc123...`. Both are valid PURL shapes. Tagged refs use `pkg:docker/`; digested refs use `pkg:oci/`. Ambiguous or unqualified refs default to `pkg:docker/library/<name>@<version>`.
- **Chart signature verification** — Helm supports GPG-signed charts (via `provenance.tgz` files). m188 does NOT verify signatures; signature verification is deferred to a follow-up milestone.
- **Repository alias vs URL** — `Chart.yaml`'s `dependencies[].repository` can be either a URL (`https://charts.bitnami.com/bitnami`) OR an alias (`@bitnami`). PURL `<repo>` segment is the URL's HOST-and-path if URL, or the alias name if `@`-prefixed. Neither format is URL-encoded further beyond PURL segment encoding.
- **Sub-chart values overrides** — parent charts can override subchart values via `values.yaml`. Since m188 is default-unrendered, subchart-values overrides are NOT considered — image refs from subchart `templates/*.yaml` are extracted from the subchart's OWN templates, not the parent's overrides. `--helm-render` resolves overrides correctly (that's the point of shelling out).
- **`.tgz` chart file discovery beyond the top level** — the walker discovers `.tgz` files ONLY under the `charts/` subdirectory (Helm convention). Loose `.tgz` files elsewhere in the scan tree are NOT treated as chart deps.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: mikebom MUST auto-detect a directory as a Helm chart when it contains a `Chart.yaml` file at its root during a `--path <dir>` scan (mirroring npm / cargo / ipk auto-detection). Helm components MUST be emitted ALONGSIDE any other package-DB reader output (npm, cargo, etc.) when their manifests are ALSO present in the tree — composability is preserved. An explicit `--helm-chart <path>` flag MUST also be accepted; its role is TARBALL UNLOCK — when `<path>` ends in `.tgz`, mikebom extracts to a tempdir before running the same scan pipeline. When `<path>` is a directory, `--helm-chart <path>` behaves identically to `--path <path>` (convenience alias — no exclusive helm-only mode).
- **FR-002**: mikebom MUST parse `Chart.yaml` and emit one CHART component (representing the chart itself) with PURL `pkg:helm/<repo-or-alias>/<name>@<version>` derived from the `name` + `version` fields. The `repository` field in the source `Chart.yaml` at scan-root is typically empty (only dep entries carry `repository`); when empty, the chart's `<repo>` segment is a synthetic `local` marker.
- **FR-003**: For each entry in `Chart.yaml`'s `dependencies[]` array, mikebom MUST emit a chart-dep component with PURL `pkg:helm/<repo-or-alias>/<name>@<version>`. `<repo>` MUST be derived from `dependencies[].repository` (URL host, or `@`-prefixed alias). Every chart-dep component MUST carry `mikebom:evidence-kind = "helm-chart-yaml"`.
- **FR-004**: When `Chart.lock` exists alongside `Chart.yaml`, mikebom MUST prefer version resolutions from `Chart.lock` (analogous to package-lock.json > package.json). Each locked dep MUST carry `mikebom:evidence-kind = "helm-chart-lock"`. Version conflicts between the two files MUST resolve in favor of the lock with a `mikebom:helm-lock-authoritative = "true"` annotation.
- **FR-005**: When `charts/<subchart>.tgz` files exist alongside `Chart.yaml`, mikebom MUST enumerate each `.tgz` as a packaged subchart dep AND recursively parse its inner `Chart.yaml` to emit ITS chart-deps. Recursion depth capped at m114 walker's 12-level limit.
- **FR-006**: mikebom MUST scan `templates/*.yaml` files for container image references. Extraction MUST use a permissive line-based regex matching `image:` field-and-value shape — NOT YAML tree walking. This choice is intentional: Go-template blocks (`{{ .Values.image.tag }}`) break YAML parsers roughly 80% of the time on real-world Helm templates, and the line-based approach handles both YAML-valid and YAML-broken files uniformly. Files that read as invalid UTF-8 or empty content are skipped with a WARN log per Constitution Principle III (no silent drops).
- **FR-007**: Every extracted image reference MUST produce a component with a valid PURL: `pkg:docker/<name>@<tag>` for tagged refs, `pkg:oci/<name>@sha256:<digest>` for digested refs, `pkg:generic/<placeholder>` for refs containing `{{ ... }}` blocks. Docker Hub's `library/` prefix MUST be preserved when applicable (per PURL spec).
- **FR-008**: Image-ref components with unresolved template placeholders MUST carry `mikebom:image-ref-unresolved = "true"` as a property. The raw pre-resolution value MUST be preserved in an `mikebom:image-ref-raw = "<raw-string>"` annotation for downstream inspection.
- **FR-009**: Duplicate image references (same image referenced from multiple templates) MUST collapse to a single component with `evidence.occurrences[]` listing all source template paths (matches CDX 1.6 evidence.occurrences convention).
- **FR-010**: mikebom MUST also scan `crds/*.yaml` files for image references (some CRDs reference operator images that aren't in `templates/`). Same extraction logic as templates.
- **FR-011**: When operator passes `--helm-render` AND `helm` binary is on `$PATH`, mikebom MUST shell out to `helm template <chart-dir>` and extract image refs from the RENDERED output rather than the raw templates. Shell-out MUST have a bounded timeout (default 60 seconds; env-var override).
- **FR-012**: When `--helm-render` is passed BUT `helm` binary is absent OR the shell-out fails, mikebom MUST fall back to unrendered extraction (US2 path) with a WARN log naming the failure reason. The scan MUST NOT abort.
- **FR-013**: mikebom MUST NOT invoke the `helm` binary when `--helm-render` is not passed. Zero external-binary calls in the default flow (matches Constitution I intent for the CLI-tool-invocation surface).
- **FR-014**: Chart-tarball input (`.tgz`) MUST be extracted to a tempdir; scan of the extracted contents MUST produce identical output as scanning the equivalent extracted directory. Tempdir MUST be cleaned up after emission.
- **FR-015**: mikebom MUST emit an operator-facing `mikebom:image-extraction-completeness = "partial"` OR `"full"` document-scope annotation. `"partial"` when unrendered extraction was used (US2, US3 fallback); `"full"` when `--helm-render` succeeded end-to-end. This is the transparency signal so consumers know whether image refs might be incomplete.
- **FR-016**: For scans that do NOT contain a `Chart.yaml` AND do NOT pass `--helm-chart`, mikebom MUST NOT emit any helm-related components. Default-scan byte-identity is preserved.
- **FR-017**: The `--helm-chart <path>` flag MUST be a documented CLI-help entry naming its narrow role (tarball unlock; directory-input behavior identical to `--path`). When passed with a non-existent path, mikebom MUST exit non-zero with an actionable error. When passed with a `.tgz` file that does NOT contain a `Chart.yaml` at the tarball root, mikebom MUST exit non-zero. When passed with a directory that does NOT contain `Chart.yaml`, mikebom MUST NOT exit non-zero (matches `--path` semantics for non-helm targets — the scan just doesn't emit helm components; other readers still run).
- **FR-018**: The `--helm-render` shell-out MUST NOT emit sensitive values (kubeconfig paths, template secrets) into mikebom's logs. Only the exit code + first 20 lines of stderr are logged on failure per FR-012.
- **FR-019**: mikebom MUST NOT introduce any new production Cargo dependency. Existing `serde_yaml` (workspace, m180+ pnpm/dart/etc. path) + `tar` (workspace, ipk-file precedent) + `flate2` (workspace) cover all needs. Regex extraction uses the existing `regex` workspace dep.

### Key Entities

- **Chart** — the top-level Helm package identified by `Chart.yaml`. Fields: `name`, `version`, `type` (`application` or `library`), `dependencies[]`, `keywords`, `home`, `maintainers`. Emitted as ONE PURL: `pkg:helm/<repo-or-alias>/<name>@<version>`.
- **Chart dependency** — entry in `Chart.yaml`'s `dependencies[]` array OR `Chart.lock`'s locked-resolution list. Fields: `name`, `version`, `repository`, `alias` (optional), `condition` (optional). Emitted as ONE PURL per entry.
- **Packaged subchart** — a `.tgz` file under `charts/`. Its inner `Chart.yaml` is parsed to enumerate the subchart's own deps recursively.
- **Image reference** — a container image string extracted from `templates/*.yaml` or `crds/*.yaml`. Fields: `raw` (the exact string), `resolved` (post-normalization PURL), `is_placeholder` (whether it contains `{{ ... }}`), `source_paths` (list of template files referencing it).
- **Chart tarball** — a `.tgz` file passed via `--helm-chart <path>.tgz`. Extracted to tempdir; treated as a chart directory downstream.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a stock ingress-nginx or cert-manager Helm chart (both well-known open-source charts with `Chart.yaml` + `Chart.lock` + a handful of templates), mikebom MUST emit at least 1 chart component (the chart itself) + 1 component per chart-dep in `Chart.yaml` + 1 component per unique image ref in templates. Zero chart components is a clear regression.
- **SC-002**: For a chart whose templates use 100% templated image refs (`{{ .Values.image }}`), mikebom's US2 default extraction MUST emit 1 image-ref component per unique templated string with `mikebom:image-ref-unresolved = "true"`. Downstream consumers can filter these out via that property.
- **SC-003**: For the same chart under `--helm-render`, when `helm` is available, ≥95% of image-ref components MUST be resolved (no `mikebom:image-ref-unresolved` property). The remaining 5% covers edge cases (Custom Resource images generated from unbound helm inputs).
- **SC-004**: `Chart.lock` version resolution MUST be authoritative over `Chart.yaml` when both are present. Verified via a fixture where `Chart.yaml` declares `version: 1.0.0` and `Chart.lock` locks to `1.0.5` — emitted PURL MUST be `@1.0.5`.
- **SC-005**: For scans that do NOT touch Helm (no `Chart.yaml`, no `--helm-chart` flag), the emitted SBOM MUST be byte-identical to pre-m188 output for the same input. Zero drift on existing golden fixtures (FR-016 gate).
- **SC-006**: `--helm-render` timeout MUST honor the FR-011 default of 60 seconds AND MUST honor an env-var override (`MIKEBOM_HELM_RENDER_TIMEOUT_SECS`) for edge cases with slow-rendering charts. Verified via an integration test that stubs a slow `helm` binary.
- **SC-007**: Zero new production Cargo dependencies added — `cargo tree --workspace | wc -l` MUST be identical pre- vs post-m188. FR-019 gate.
- **SC-008**: `--helm-chart <path>` where `<path>` doesn't exist or has no `Chart.yaml` MUST exit non-zero with an actionable error message naming the missing file. FR-017 gate.

## Assumptions

- The scan target's `Chart.yaml` conforms to Helm 3.x's chart schema. Helm 2.x charts (deprecated 2020) are NOT explicitly supported but the fields we read (`name`, `version`, `dependencies[]`) exist in both versions, so they usually work fine.
- Image-ref extraction uses regex `^(\s*)image:\s*['"]?([^'"\s]+)['"]?\s*(#.*)?$` as the DEFAULT extraction shape. This catches the standard k8s pod spec convention. Non-standard image fields (nested under CRD-specific keys) are not exhaustively supported; the `mikebom:image-extraction-completeness = "partial"` document-scope annotation flags this.
- Chart-tier components carry `sbom_tier = "design"` (per milestone 122's design-tier gating pattern) — helm charts are declarative deployment manifests, not build products, so declared components are design-time. Image-ref components carry `sbom_tier = "design"` too when unrendered (US2) and `"analyzed"` when rendered via `--helm-render` (US3).
- Chart tarball extraction (`.tgz`) reuses the existing `tar` + `flate2` workspace dependencies (same as m187's ipk reader).
- The `helm` binary invocation (`--helm-render`, US3) is an OPT-IN escape hatch. The default `--path` / `--helm-chart` scan does NOT require helm on the host, matching Constitution Principle I's intent to minimize external tool dependencies.
- The `pkg:helm/` PURL type is defined in the PURL spec (https://github.com/package-url/purl-spec/blob/master/PURL-TYPES.rst#helm). The `<namespace>` segment is the repository host or alias; the `<name>` is the chart name; the `<version>` is the semver.
- Chart-dep repository URL handling: `https://` URLs' scheme is stripped for the PURL `<namespace>` (per PURL convention); `@`-prefixed aliases are used verbatim.
- Signature verification of GPG-signed chart tarballs (`.tgz` + `.tgz.prov`) is DEFERRED to a follow-up milestone.
- Design-tier gating (per m122) means `sbom_tier = "design"` is emitted at the component level; the top-level document is untagged (matches existing convention).

## Constitution Alignment

- **Principle I (Pure Rust, Zero C)**: FR-019 + SC-007 verified. Zero new deps. `helm` binary shell-out is opt-in per FR-013 (matches m053's `git describe` precedent for the analogous concept).
- **Principle III (Fail Closed)**: FR-012 + FR-017 name specific failure modes with actionable errors. `--helm-render` failure falls back to unrendered extraction rather than aborting, but the transparency signal (FR-015 `mikebom:image-extraction-completeness = "partial"`) surfaces the degradation.
- **Principle IV (Type-Driven Correctness)**: chart-dep entries + image refs are captured as compile-time-typed structs (`ChartDep`, `ImageRef`) before emission. No stringly-typed dispatch.
- **Principle V (Specification Compliance + Native-first)**: `pkg:helm/...` + `pkg:oci/...` + `pkg:docker/...` are PURL-spec-native. Two new `mikebom:*` annotations (`mikebom:image-ref-unresolved`, `mikebom:image-extraction-completeness`) are needed because CDX / SPDX 2.3 / SPDX 3 have no native construct for "this image reference contains an unresolved template placeholder" or "the image-extraction pass ran in reduced-fidelity mode." Both documented in `docs/reference/sbom-format-mapping.md` per Principle V process.
- **Principle VI (Three-Crate Architecture)**: all changes confined to `mikebom-cli/src/scan_fs/package_db/` + `mikebom-cli/src/cli/`. Zero mikebom-common / mikebom-ebpf / xtask touches.
- **Principle VII (Test Isolation)**: unit tests colocated in `helm.rs`; integration tests via a new `mikebom-cli/tests/helm_reader.rs` with synthetic fixtures fabricated at test time (matches m187's `ipk_yocto_reader_fixes.rs` pattern).
- **Principle VIII (Completeness)**: chart-dep enumeration + image-ref extraction close the "no SBOM scanner handles Helm well" gap called out in the issue rationale. On typical open-source charts (ingress-nginx, cert-manager) mikebom emits 5-50x more helm-relevant components than the current pass-through-generic-YAML behavior.
- **Principle IX (Accuracy)**: templated image refs are surfaced with a clear `mikebom:image-ref-unresolved = "true"` marker so consumers don't accept placeholder PURLs as concrete container image identities.
- **Principle X (Transparency)**: FR-015 document-scope annotation surfaces whether extraction was full or partial. `mikebom:image-ref-raw` preserves the pre-resolution raw string per Constitution Principle X for auditability.

## Deferred to Future Milestones

- **Rendered-and-cluster-scoped extraction** — `helm install --dry-run` (as opposed to `helm template`) uses cluster-fetched CRDs to render post-install manifests. Requires kubeconfig context; more expensive; opt-in beyond `--helm-render`. Deferred.
- **Chart signature verification** — `.tgz.prov` GPG-signed provenance files. Deferred; separate crypto milestone.
- **Sub-chart values override rendering** — mikebom's default unrendered extraction does not apply parent-values overrides to subchart templates. `--helm-render` handles this correctly. Deferred to any follow-up that wants to skip helm-binary dependency.
- **Kustomize integration** — many Helm-adjacent workflows use Kustomize overlays on top of chart output. Not addressed here.
- **`kubectl apply -f` YAML** — raw Kubernetes YAML manifests (not Helm-templated) are covered by the existing scan_fs walker; Helm-specific extraction here doesn't overlap.
- **Non-standard image-field keys** — CRDs may reference images at paths like `spec.image`, `spec.template.image`, `containerImage:`, `initContainerImage:`, etc. m188 uses a permissive `image:` regex; broader CRD-aware extraction is deferred.
- **Chart repository index scraping** — Helm repositories serve an `index.yaml` catalog. mikebom does not fetch or consult `index.yaml`; only chart-local metadata is used. Deferred.
- **HelmFile / Chart-of-Charts orchestration files** — `helmfile.yaml` or umbrella-chart-of-charts patterns. Deferred.
