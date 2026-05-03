# gem-source-project fixture (milestone 069)

Minimal Ruby gem project with a top-level `*.gemspec` exercising:

- Literal `s.name = "foo"` + `s.version = "1.0.0"` extraction
- `s.add_dependency "rake"` + `s.add_development_dependency "rspec"`
  for FR-007 direct-edge coverage

Used by integration tests in `tests/scan_gem.rs`.
