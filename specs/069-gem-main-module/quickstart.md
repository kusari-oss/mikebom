# Quickstart: Verify gem main-module emission

Three recipes covering top-level gemspec, non-literal version, and application-style skip.

## Prerequisites

```sh
cargo +stable build -p mikebom
```

## Recipe A — Top-level *.gemspec single-project

```sh
mkdir -p /tmp/gem-069
cat > /tmp/gem-069/foo.gemspec <<'EOF'
Gem::Specification.new do |s|
  s.name        = "foo"
  s.version     = "1.0.0"
  s.summary     = "demo gem"
  s.add_dependency "rake"
end
EOF

target/debug/mikebom sbom scan --path /tmp/gem-069 --format cyclonedx-json --output /tmp/gem.cdx.json --no-deep-hash

jq '.metadata.component | {bom_ref: ."bom-ref", type, name, version, purl}' /tmp/gem.cdx.json
```

**Expect**:

```json
{
  "bom_ref": "pkg:gem/foo@1.0.0",
  "type": "application",
  "name": "foo",
  "version": "1.0.0",
  "purl": "pkg:gem/foo@1.0.0"
}
```

## Recipe B — Non-literal version (constant ref)

```sh
mkdir -p /tmp/gem-const
cat > /tmp/gem-const/bar.gemspec <<'EOF'
Gem::Specification.new do |s|
  s.name    = "bar"
  s.version = Bar::VERSION
end
EOF
target/debug/mikebom sbom scan --path /tmp/gem-const --format cyclonedx-json --output /tmp/bar.cdx.json --no-deep-hash
jq '.metadata.component.purl' /tmp/bar.cdx.json
```

**Expect**: `"pkg:gem/bar@0.0.0-unknown"` (constant reference falls through to placeholder per FR-001 + A9).

## Recipe C — Application-style project (Gemfile only, no .gemspec)

```sh
mkdir -p /tmp/gem-app
cat > /tmp/gem-app/Gemfile <<'EOF'
source 'https://rubygems.org'
gem 'rake', '13.0.0'
EOF
cat > /tmp/gem-app/Gemfile.lock <<'EOF'
GEM
  remote: https://rubygems.org/
  specs:
    rake (13.0.0)

PLATFORMS
  ruby

DEPENDENCIES
  rake (= 13.0.0)
EOF

target/debug/mikebom sbom scan --path /tmp/gem-app --format spdx-2.3-json --output /tmp/gem-app.spdx.json --no-deep-hash

jq '[.packages[] | select(.primaryPackagePurpose == "APPLICATION")] | length' /tmp/gem-app.spdx.json
```

**Expect**: `0` (no main-module emitted per FR-002 — application-style project, no top-level `*.gemspec`).

The `rake` gem from `Gemfile.lock` is still emitted as a regular dep component (existing behavior).

## When to run

- **Recipe A** during US1 / SC-001 verification
- **Recipe B** for the non-literal-version edge case (FR-001 placeholder fallback)
- **Recipe C** for FR-002 / SC-002 (application-style project skip)

All three recipes should also be exercised as integration tests in `tests/scan_gem.rs`.
