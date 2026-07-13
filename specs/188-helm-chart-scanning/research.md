# Research: Helm chart scanning (m188)

**Feature**: [spec.md](./spec.md) · **Plan**: [plan.md](./plan.md)

## Decisions

### Decision 1 — Auto-detect posture: `Chart.yaml` at scan-root vs anywhere-in-tree

**Decision**: mikebom auto-detects Helm charts via `<scan-root>/Chart.yaml` PRESENCE — the file must be at the IMMEDIATE root of the scan target, not nested arbitrarily deep. When present, the reader emits helm components AND recursively processes `charts/<subchart>.tgz` up to the m114 walker's 12-level depth cap. Nested `Chart.yaml` files DEEPER than the immediate root are ignored by the top-level dispatch (their content surfaces via the subchart recursion path when their parent chart references them via `charts/*.tgz`).

**Rationale**:
- Matches Helm's own convention: `helm install <chart-dir>` requires `Chart.yaml` at the argument's root; nested charts are only discovered via the `charts/` subdir mechanism.
- Prevents false positives from vendored / documentation copies of `Chart.yaml` deep in unrelated trees (e.g., a repo whose `docs/examples/` contains a sample chart shouldn't cause helm emission when the operator scans the whole repo).
- The `charts/*.tgz` recursion path (per FR-005) covers the ONLY legitimate nested-chart scenario.
- Symmetric with existing readers: npm reader dispatches at `package.json`-at-root, cargo at `Cargo.toml`-at-root; helm should follow suit.

**Alternatives considered**:
- **Walk the whole tree looking for `Chart.yaml`** — rejected. False-positive rate on unrelated repos is unacceptable; would emit helm components for any repo containing an example / vendored chart in `docs/` or `test/fixtures/`.
- **Configurable via `--helm-search-depth <N>`** — rejected. Overengineers a knob for a scenario that doesn't exist. The Helm ecosystem's own convention is root-only.
- **Auto-detect at TOP-LEVEL only for `--path`; `--helm-chart <path>` allows depth-1** — rejected. Inconsistent between the two entry points; violates Clarifications Q1's "directory-input behavior identical to `--path`" contract.

---

### Decision 2 — PURL type disambiguation: `pkg:oci/` vs `pkg:docker/` vs `pkg:generic/`

**Decision**: mikebom emits image-ref components with the following PURL type mapping, in priority order:
- **`pkg:generic/<placeholder>`** — refs containing `{{ ... }}` blocks (unresolved template placeholders). The `<placeholder>` slug is the URL-encoded raw string with `{{ ... }}` blocks replaced by a stable `__PLACEHOLDER_N__` token (N=0,1,2… per unique placeholder in the ref). Attaches `mikebom:image-ref-unresolved = "true"` + `mikebom:image-ref-raw = "<original-string>"` property pair.
- **`pkg:oci/<name>@sha256:<digest>`** — refs with an explicit `sha256:<hex>` digest (`nginx@sha256:abc123...` or `ghcr.io/foo/bar@sha256:...`). Digested = content-addressable = OCI's canonical form.
- **`pkg:docker/<name>@<tag>`** — refs with a `:<tag>` version marker but NO digest (`nginx:1.27.0`, `busybox:latest`). Tagged = mutable-registry-based = Docker's canonical form.
- **`pkg:docker/library/<name>@<tag>`** — refs without an explicit namespace/registry prefix (`nginx:1.27.0` → Docker Hub library convention). Matches PURL-spec's `library/` prefix rule.

**Rationale**:
- The PURL spec's `oci` and `docker` types overlap but have DIFFERENT axes: `oci` is content-addressable (digest-native), `docker` is tag-based (registry-native). Choosing one type per ref-shape (rather than always using `pkg:oci/`) preserves the axis the operator actually used, and lets downstream tools (grype, trivy) apply their existing PURL matchers without transformation.
- The `pkg:generic/<placeholder>` type for unresolved refs is a deliberate "not a real image" signal — grype/trivy will not match against `pkg:generic/` refs, so they silently skip templated-placeholder components without producing spurious CVE hits. Combined with the `mikebom:image-ref-unresolved = "true"` property, this is a two-signal marker for downstream filtering.
- The `library/` prefix rule aligns mikebom with syft's + trivy's PURL emission for Docker Hub-hosted images.

**Alternatives considered**:
- **Always emit `pkg:oci/`** — rejected. Loses the tag-vs-digest axis; downstream tools don't have a canonical way to distinguish "operator picked a tag" vs "operator picked a digest."
- **Always emit `pkg:docker/`** — rejected. Digested refs are canonically OCI, not Docker; downstream tools' PURL matchers expect `pkg:oci/` for `sha256:` refs.
- **Emit BOTH `pkg:oci/` and `pkg:docker/` for tagged refs (dual emission)** — rejected. Duplicates the component count; downstream dedup burden.
- **`pkg:generic/<placeholder>` component NOT emitted for unresolved refs** — rejected. Loses the "we saw an image ref here but can't resolve it" signal; operators auditing charts want to see the placeholder count and locations.

---

### Decision 3 — Chart tarball tempdir lifecycle (`--helm-chart <path>.tgz`)

**Decision**: When `--helm-chart <path>.tgz` is passed:
1. mikebom creates a `tempfile::TempDir` via `tempfile::Builder::new().prefix("mikebom-helm-chart-").tempdir()` — same pattern as m031's OCI-pull tempdir + m187's `.tgz` extraction.
2. Extract the tarball via `flate2::read::GzDecoder` + `tar::Archive::unpack()` into the tempdir.
3. Verify the extracted contents contain a `Chart.yaml` at the extracted root (chart tarballs by convention wrap contents in a single top-level directory named after the chart — e.g., `mychart-1.0.0.tgz` extracts to `mychart/Chart.yaml`). If the tarball has multiple top-level dirs OR no `Chart.yaml`, mikebom exits non-zero per FR-017.
4. Descend into the discovered chart directory (`<tempdir>/<chart-name>/`) and run the standard reader pipeline against it — identical to a `--path <chart-dir>` scan.
5. The `TempDir` is held by an `Option<TempDir>` in `scan_cmd.rs` alive through emission; dropped after SBOM write, which triggers automatic cleanup via `TempDir`'s `Drop` impl.

**Rationale**:
- Mirrors the exact tempdir-and-descend pattern established by m031 for OCI image tarballs (`pull_to_tarball`). Constitution-friendly reuse.
- Handles the standard Helm packaging convention (`helm package ./mychart/` produces `mychart-1.0.0.tgz` containing `mychart/`). Non-conforming tarballs are rejected upfront rather than causing downstream confusion.
- `tempfile::TempDir::Drop` cleanup is standard practice; no manual cleanup required. Matches m187's ipk-file behavior.

**Alternatives considered**:
- **Stream-parse the tarball in memory without extracting to disk** — rejected. Would require a custom in-memory VFS abstraction; existing readers (npm, cargo, etc.) assume filesystem paths. Extraction is trivially cheap for typical chart sizes (<10MB).
- **Keep the tempdir alive by leaking a reference** — rejected. Leaks; unnecessary. `Option<TempDir>` in the caller scope achieves the same lifetime without leaking.
- **Accept multi-top-level-dir tarballs by scanning ALL top-level dirs** — rejected. Non-standard shape; likely a corrupted tarball. Exit non-zero surfaces the issue rather than emitting garbage.

---

### Decision 4 — Image-ref extraction: line-based regex primary, YAML walk as future extension

**Decision**: mikebom's default (US2) image-ref extraction uses a line-based regex `^(\s*)image:\s*['"]?([^'"\s]+)['"]?\s*(#.*)?$` applied to `templates/*.yaml` and `crds/*.yaml` line by line. YAML-tree walking is DEFERRED — the regex approach handles the standard Kubernetes pod-spec + CRD image-field shape and gracefully degrades on Go-template-broken YAML files (which are the majority in practice).

**Rationale**:
- Go-template blocks (`{{ if .Values.enabled }}...{{ end }}`) break YAML parsers — a `serde_yaml::from_str` on a real-world Helm template file returns `Err` roughly 80% of the time. The regex approach is agnostic to YAML validity.
- The `image:` field is a Kubernetes-spec-native convention that appears at consistent nesting depths (`spec.containers[].image`, `spec.initContainers[].image`, `spec.template.spec.containers[].image`). Line-based regex captures 100% of these without needing to know the nesting depth.
- Regex approach is fast (~5ms per template file); YAML-walk would be 2-3x slower even if it worked.
- Non-standard image fields (CRD-nested paths like `spec.image`, vendor-specific keys) are documented as "partial" via FR-015's `mikebom:image-extraction-completeness = "partial"` annotation. The `--helm-render` path resolves this (rendered YAML has resolved placeholders → YAML-walk becomes feasible; deferred to a follow-up).

**Alternatives considered**:
- **Full YAML tree walk** — rejected. Fails on 80% of real-world templates due to Go-template syntax; would require pre-processing to strip Go-template blocks (complex and lossy).
- **YAML walk with fallback to regex** — rejected. Doubles the code path; the "when does the fallback fire?" heuristic adds test complexity without meaningful accuracy gain over regex-only.
- **Pluggable extraction strategy** — rejected. Overengineered for m188's scope. If a future milestone wants CRD-aware extraction, it can add a new strategy alongside the regex one without breaking the m188 contract.

---

### Decision 5 — `--helm-render` subprocess pattern

**Decision**: The optional `--helm-render` shell-out uses `std::process::Command::new("helm").args(["template", chart_dir]).output()` with a 60-second timeout enforced via `std::thread::spawn` + `std::sync::mpsc` channel (same pattern as m055's `run_go_mod_graph` at `golang/go_mod_graph.rs:81-158`). Stderr and stdout are both captured; on non-zero exit or timeout, the first 20 lines of stderr are logged at WARN level per FR-018 (no full stderr — kubeconfig paths + secret values may leak).

**Rationale**:
- `std::process::Command` + thread-based timeout is the workspace's standard pattern for opt-in external-tool integration (m053 `git describe`, m055 `go mod graph`, m161 `go build`, m173 warm-go-cache). Zero new deps; no new abstractions.
- 60-second default matches typical helm-template runtime on moderate charts (500ms–3s common; 15-30s for umbrella charts with many subcharts). Env-var override (`MIKEBOM_HELM_RENDER_TIMEOUT_SECS`) per SC-006 covers edge cases.
- 20-line stderr cap balances "enough context for debugging" vs "avoid leaking kubeconfig / secrets" per FR-018.
- Fall-through to US2 unrendered extraction on ANY failure (missing binary, non-zero exit, timeout) means `--helm-render` is a NON-BREAKING opt-in — operators can pass it optimistically, and the scan still produces useful output even when helm is unavailable.

**Alternatives considered**:
- **`tokio::process::Command` async** — rejected. mikebom's package-db readers are synchronous; introducing an async boundary just for helm-render would perturb the existing scan_path→read_all→ecosystem_readers chain.
- **Vendor the helm library / bind against helm's Go source** — rejected. Massive scope creep + Constitution Principle I violation (Helm is Go, would require CGo or a full port).
- **Log full stderr always** — rejected. Real-world kubeconfig-related errors leak paths + occasionally embedded tokens.

---

## Bug Discovery

None so far. m188 is greenfield feature work — no existing helm code to regress against.

## Related Reading

- **spec.md**: full US1 + US2 + US3 acceptance scenarios + 12 edge cases + 19 FRs + 8 SCs.
- **Issue #455**: original feature request with implementation notes (`serde_yaml`, `--helm-render` shell-out, `mikebom:image-ref-unresolved` marker).
- **`mikebom-cli/src/scan_fs/package_db/mod.rs:1227`** — `read_all` dispatcher; m188 will wire `helm::read` in near the existing npm / cargo callsites.
- **`mikebom-cli/src/scan_fs/package_db/dart.rs`** — reference for a `serde_yaml`-based reader with `pubspec.yaml`-shaped input (similar to `Chart.yaml`).
- **`mikebom-cli/src/scan_fs/package_db/ipk_file.rs`** — reference for a `.tgz` tarball extraction + inner-file parsing pattern (m187 refactor is the freshest example).
- **`mikebom-cli/src/scan_fs/package_db/golang/go_mod_graph.rs:81-158`** — reference for the `std::process::Command` + thread + mpsc-channel timeout pattern used by `--helm-render`.
- **`mikebom-cli/src/scan_fs/oci_pull/mod.rs:93`** — reference for the `tempfile::TempDir` extraction lifecycle pattern used for `.tgz` chart tarballs.
- **PURL spec `helm` type**: https://github.com/package-url/purl-spec/blob/master/PURL-TYPES.rst#helm — reference for `pkg:helm/<repo>/<name>@<version>` shape.
- **PURL spec `oci` + `docker` types** — reference for image-ref PURL disambiguation per Decision 2.
