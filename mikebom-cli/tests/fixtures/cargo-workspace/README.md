# cargo-workspace fixture (milestone 064)

Two-member cargo workspace exercising:

- `[workspace.package].version = "0.5.0"` inheritance via
  `version.workspace = true` in each member's `Cargo.toml`
  (US1 AS#3 / FR-001 / Assumption A2).
- Multi-main-module emission: each member crate becomes one
  `pkg:cargo/<name>@0.5.0` main-module component (US1 AS#2 /
  FR-002).
- Path-dep edges between workspace members (`b → a`) per FR-011.
- `documentDescribes` lists both `a` and `b` SPDXIDs, sorted
  alphabetically (US3 AS#2 / FR-008).

Used by `tests/scan_cargo_workspace.rs` integration tests.
