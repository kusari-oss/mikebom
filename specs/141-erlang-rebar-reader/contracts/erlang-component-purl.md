# Contract — Erlang/OTP component PURL shapes

Authoritative output-shape contract for components emitted by the
milestone-141 Erlang reader. Test fixtures in
`mikebom-cli/tests/erlang_*.rs` MUST assert exact PURL strings
matching the shapes below.

## 1. Hex package — default registry (Hex.pm)

**Input** (rebar.lock):
```erlang
{<<"cowboy">>, {pkg, <<"cowboy">>, <<"2.10.0">>, <<"abc123...">>}, 0}.
```

**Output**:
- `purl`: `pkg:hex/cowboy@2.10.0`
- `hashes`: `[{"alg": "SHA-256", "content": "abc123..."}]` (when 4th tuple element non-empty)
- `properties[]`:
  - `mikebom:source-type = "erlang-hex"`
  - `mikebom:evidence-kind = "rebar-lock"`

## 2. Hex package — private organization

**Input** (rebar.lock):
```erlang
{<<"internal_lib">>, {pkg, <<"internal_lib">>, <<"2.0.0">>, <<"def456...">>, #{repo => <<"hexpm:acme">>}}, 0}.
```

**Output**:
- `purl`: `pkg:hex/acme/internal_lib@2.0.0?repository_url=https://repo.hex.pm`
- `hashes`: `[{"alg": "SHA-256", "content": "def456..."}]`
- `properties[]`:
  - `mikebom:source-type = "erlang-hex"`
  - `mikebom:evidence-kind = "rebar-lock"`

**Variant**: bare-org form (no `hexpm:` prefix) `#{repo => <<"acme">>}` →
emits identical PURL. Both forms accepted per rebar3 documented flexibility.

## 3. Git source

**Input** (rebar.lock):
```erlang
{<<"my_fork">>, {git, "https://github.com/foo/my-fork.git", {ref, "eb39649a76b87e8451baf75d10ce82ca3a3d5601"}}, 0}.
```

**Output**:
- `purl`: `pkg:generic/my_fork@eb39649a76b87e8451baf75d10ce82ca3a3d5601?vcs_url=git+https://github.com/foo/my-fork.git`
- `hashes`: `[]`
- `properties[]`:
  - `mikebom:source-type = "erlang-git"`
  - `mikebom:evidence-kind = "rebar-lock"`
  - `mikebom:vcs-declared-ref = "ref"`

**Variant** (declared via tag):
```erlang
{<<"my_fork">>, {git, "https://github.com/foo/my-fork.git", {tag, "v1.2.3"}}, 0}.
```
→ `purl`: `pkg:generic/my_fork@v1.2.3?vcs_url=git+https://github.com/foo/my-fork.git`
+ `mikebom:vcs-declared-ref = "tag"`

**Variant** (declared via branch):
```erlang
{<<"my_fork">>, {git, "https://github.com/foo/my-fork.git", {branch, "main"}}, 0}.
```
→ `purl`: `pkg:generic/my_fork@main?vcs_url=git+https://github.com/foo/my-fork.git`
+ `mikebom:vcs-declared-ref = "branch"`

## 4. OTP runtime placeholder (Q1 fallback)

**Input** (`my_app.app.src` declares `applications: [kernel, stdlib, cowboy]`
where only `cowboy` appears in `rebar.lock`):

**Output for `kernel`** (allowlist member):
- `purl`: `pkg:generic/kernel@unspecified`
- `hashes`: `[]`
- `properties[]`:
  - `mikebom:source-type = "erlang-otp-runtime"`
  - `mikebom:evidence-kind = "app-src"`
  - `mikebom:otp-stdlib = "true"`

**Output for a non-allowlisted OTP atom** (e.g., custom OTP app `my_custom_otp`):
- `purl`: `pkg:generic/my_custom_otp@unspecified`
- `hashes`: `[]`
- `properties[]`:
  - `mikebom:source-type = "erlang-otp-runtime"`
  - `mikebom:evidence-kind = "app-src"`
- (no `mikebom:otp-stdlib` annotation — allowlist informational only)

## 5. Main-module from `*.app.src`

**Input** (`apps/my_app/src/my_app.app.src`):
```erlang
{application, my_app, [
    {vsn, "1.2.3"},
    {applications, [kernel, stdlib, cowboy]},
    {included_applications, [config_app]},
    {optional_applications, [telemetry]},
    {description, "My OTP application"}
]}.
```

**Output (main-module)**:
- `purl`: `pkg:hex/my_app@1.2.3`
- `hashes`: `[]`
- `properties[]`:
  - `mikebom:source-type = "erlang-main-module"`
  - `mikebom:component-role = "main-module"`
  - `mikebom:sbom-tier = "source"`
  - `mikebom:evidence-kind = "app-src"`
- `depends[]`: bom-refs targeting `kernel`, `stdlib`, `cowboy`, `config_app`, `telemetry`
  (each resolved per §3.5 of data-model.md; OTP atoms target the
  `pkg:generic/*` placeholders from §4 above; `cowboy` targets the
  `rebar.lock`-derived `pkg:hex/cowboy@<version>`)

**Edge-target annotations** (per Q3) on each dep component:
- `kernel`, `stdlib`, `cowboy` → carry `mikebom:erlang-app-dep-kind = "required"`
- `config_app` → carries `mikebom:erlang-app-dep-kind = "included"`
- `telemetry` → carries `mikebom:erlang-app-dep-kind = "optional"`

## 6. Main-module version fallback

**Input** (`my_app.app.src` without `{vsn, "..."}`):
```erlang
{application, my_app, [
    {applications, [kernel]},
    {description, "App without vsn"}
]}.
```

**Output (main-module)**:
- `purl`: `pkg:hex/my_app@0.0.0-unknown`
- (other fields per §5)

## 7. Main-module application-name fallback

**Input** (malformed `*.app.src` where `{application, <atom>, ...}` outer
tuple can't be parsed — but the file is at `apps/orphaned_app/src/orphaned_app.app.src`):

**Output (main-module)**:
- `purl`: `pkg:hex/orphaned_app@0.0.0-unknown`
  (app-name from parent-directory basename `orphaned_app/src/` →
  parent of `src/` is `orphaned_app/`)
- `properties[]`:
  - `mikebom:component-role = "main-module"`
  - `mikebom:sbom-tier = "source"`
  - `mikebom:evidence-kind = "app-src"`

## 8. Design-tier dep from `rebar.config` (no rebar.lock present)

**Input** (`rebar.config` only):
```erlang
{deps, [
    {cowboy, "~> 2.10"},
    {jiffy, {pkg, jiffy, "~> 1.1"}}
]}.
{profiles, [
    {test, [{deps, [{meck, "~> 0.9"}]}]}
]}.
```

**Output**:

`cowboy`:
- `purl`: `pkg:hex/cowboy@~>%202.10` (constraint URL-encoded)
- `properties[]`:
  - `mikebom:sbom-tier = "design"`
  - `mikebom:requirement-range = "~> 2.10"`
  - `mikebom:evidence-kind = "rebar-config"`
  - `mikebom:source-type = "erlang-hex"`

`jiffy`: identical shape, constraint `~> 1.1`.

`meck` (test-profile-scoped):
- `purl`: `pkg:hex/meck@~>%200.9`
- `properties[]`:
  - `mikebom:sbom-tier = "design"`
  - `mikebom:requirement-range = "~> 0.9"`
  - `mikebom:evidence-kind = "rebar-config"`
  - `mikebom:source-type = "erlang-hex"`
  - `mikebom:lifecycle-scope = "development"` (per FR-008)

## 9. Legacy Hex shape (pre-rebar3-3.7)

**Input** (rebar.lock with flat shape):
```erlang
{<<"lager">>, <<"3.9.2">>, 1}.
```

**Output**:
- `purl`: `pkg:hex/lager@3.9.2`
- `hashes`: `[]` (no inner SHA-256 in legacy shape)
- `properties[]`:
  - `mikebom:source-type = "erlang-hex"`
  - `mikebom:evidence-kind = "rebar-lock"`

## 10. Cross-format byte-equivalence

For every emission in §1–§9, the same `purl` value MUST appear in:
- CycloneDX 1.6 output's `components[].purl`
- SPDX 2.3 output's `packages[].externalRefs[].referenceLocator` (where `referenceType == "purl"`)
- SPDX 3.0.1 output's `@graph[].software_packageUrl`

Per the milestone-013 format-parity-enforcement work, the
`parity-check` subcommand verifies this invariant for milestone 141
test fixtures.

## 11. SBOM-format property name mapping

| Field | CycloneDX 1.6 | SPDX 2.3 | SPDX 3.0.1 |
|---|---|---|---|
| `mikebom:source-type` | `properties[].name = "mikebom:source-type"` | `annotations[]` with comment envelope | document-scope `Annotation` with envelope |
| `mikebom:evidence-kind` | `properties[]` | `annotations[]` | `Annotation` |
| `mikebom:sbom-tier` | `properties[]` | `annotations[]` | `Annotation` |
| `mikebom:requirement-range` | `properties[]` | `annotations[]` | `Annotation` |
| `mikebom:lifecycle-scope` | NATIVE: `components[].scope` | NATIVE: `relationships[].relationshipType = "DEV_DEPENDENCY_OF"` etc. | NATIVE: `LifecycleScopeType` |
| `mikebom:component-role` | `properties[]` | `annotations[]` | `Annotation` |
| `mikebom:erlang-app-dep-kind` | `properties[]` | `annotations[]` | `Annotation` |
| `mikebom:otp-stdlib` | `properties[]` | `annotations[]` | `Annotation` |
| `mikebom:vcs-declared-ref` | `properties[]` | `annotations[]` | `Annotation` |

Per Constitution Principle V, `mikebom:lifecycle-scope` flows through
the milestone-052 native-field path (CDX `scope` / SPDX 2.3
`DEV_DEPENDENCY_OF` / SPDX 3 `LifecycleScopeType`). The other
`mikebom:*` properties remain as standalone annotations because no
spec-native carrier exists for the semantic per research §R6.
