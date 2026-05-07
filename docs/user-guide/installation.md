# Installation

mikebom has two modes with different runtime requirements.

| Mode | Subcommands | Needs |
|---|---|---|
| **Scanning** | `mikebom sbom scan`, `mikebom sbom verify`, `mikebom sbom enrich`, `mikebom sbom verify-binding`, `mikebom sbom trace-binding`, `mikebom policy init` | Any OS Rust runs on. No privilege. No eBPF. |
| **Tracing** | `mikebom trace capture`, `mikebom trace run` | Linux kernel ≥ 5.8, eBPF privilege (`--privileged` container, root, or CAP_BPF + CAP_PERFMON) |

If you only need the scanning surface, mikebom runs natively on macOS,
Windows (WSL2), or Linux. `trace` requires Linux with eBPF.

## Pre-built binaries (recommended)

Every mikebom release ships per-platform tarballs as GitHub Release assets.

Discover the latest release:

```bash
gh release list -R kusari-sandbox/mikebom --limit 1
```

Download the asset matching your platform:

```bash
TAG=$(gh release list -R kusari-sandbox/mikebom --limit 1 --json tagName --jq '.[0].tagName')
gh release download "$TAG" -R kusari-sandbox/mikebom -p "mikebom-${TAG}-*-$(uname -m)-*.tar.gz"
tar -xzf mikebom-*.tar.gz
sudo install -m 0755 mikebom /usr/local/bin/mikebom
mikebom --version
```

Or browse releases manually at
<https://github.com/kusari-sandbox/mikebom/releases> and pick the asset
that matches your platform.

## Build from source

Stable Rust, standard workspace build:

```bash
cargo build --release
```

The binary lands at `./target/release/mikebom`. Add it to `$PATH` or invoke
it directly.

```bash
./target/release/mikebom --help
```

For a system-wide install from source:

```bash
cargo install --path mikebom-cli
```

The workspace has three crates (`mikebom-cli`, `mikebom-common`,
`mikebom-ebpf`) plus an `xtask` crate. A single `cargo build --release`
from the repo root produces the CLI binary.

## Development container (Linux eBPF, macOS, Windows)

The tracing subcommands need a privileged Linux host. On macOS, Windows,
or when you don't want to build toolchain dependencies locally, use the
provided dev container — it ships a compatible kernel and the BPF
toolchain.

```bash
docker build -t mikebom-dev -f Dockerfile.dev .
docker run --rm --privileged \
  -v "$PWD:/mikebom-src:ro" \
  mikebom-dev \
  bash -c "cd /mikebom-src && cargo build --release"
```

`--privileged` is required: eBPF probe attachment uses capabilities that
rootless Docker and unprivileged containers don't expose.

## Lima VM (macOS)

For an interactive Linux shell on macOS without Docker, the repo ships a
[`lima.yaml`](../../lima.yaml) recipe. Provision with:

```bash
limactl start --name=mikebom lima.yaml
limactl shell mikebom
```

Inside the VM, `cargo build --release` and `trace`/`scan` subcommands work
as on any Linux host.

## Verify the install

```bash
mikebom --version
mikebom --help
mikebom sbom --help
mikebom trace --help
```

If `mikebom --help` shows the top-level `trace` / `sbom` / `attestation` /
`policy` nouns and the global flags (`--offline`, `--exclude-scope`,
`--include-legacy-rpmdb`), the install is ready. Move on to the
[quickstart](quickstart.md).

## What's next

- [Quickstart](quickstart.md) — operator onboarding recipes.
- [CLI reference](cli-reference.md) — every flag with type, default, and
  example.
- [Configuration](configuration.md) — global flags and environment variables.
