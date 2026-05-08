# cargo transitive-parity fixture — clap-rs/clap @ v4.5.21

Manifest + lockfile only (per spec/083 FR-002 + Q1 clarification).

- **Source URL**: https://github.com/clap-rs/clap
- **Commit SHA**: 2920fb082c987acb72ed1d1f47991c4d157e380d
- **Tag**: v4.5.21
- **Packages**: 421 (cargo workspace + transitives)
- **Dep edges**: ~1006 (all `dependencies = [...]` array entries across `[[package]]` blocks)

## Reproducibility

```bash
git clone https://github.com/clap-rs/clap /tmp/clap-fixture
cd /tmp/clap-fixture
git checkout v4.5.21
# Cargo.toml + Cargo.lock match exactly what's vendored here.
```
