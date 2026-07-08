# Contract: CLI flag surface for m173

**Feature**: 173-warm-go-cache
**Date**: 2026-07-08

Authoritative reference for the two new CLI flags. Deviations in flag parsing are grounds for review comment.

## `--warm-go-cache=<MODE>`

**Values**: `off` (default) | `per-workspace`

**Parse form**: `--warm-go-cache=per-workspace` (equals-required per FR-010).

**Rejected forms**:
- `--warm-go-cache per-workspace` (space-separated) — clap `require_equals = true` prevents this to avoid the next positional argument being consumed silently.
- `--warm-go-cache` (bare, no value) — clap `num_args = 1` rejects. No boolean-shorthand.
- `--warm-go-cache=on` / `--warm-go-cache=yes` — clap `value_enum` rejects unknown values with a "expected one of `off`, `per-workspace`" error message.

**Interaction with `--offline`**:
- If `--offline` AND `--warm-go-cache=per-workspace`: warmer skips all work; effective mode becomes `offline-inhibited`; one warn-level log emitted.
- If `--offline` AND `--warm-go-cache=off` (or unset): no warmer runs; effective mode is `off`; no conflict log.

## `--warm-go-cache-concurrency <N>`

**Values**: `0` (auto) | `1..=32`.

**Parse form**: `--warm-go-cache-concurrency 4` OR `--warm-go-cache-concurrency=4`. Both accepted (no `require_equals` — an integer can't be confused with a positional path).

**Runtime resolution**:
- `0` → `min(available_parallelism(), 8)`.
- `1..=32` → used as-is.
- `>32` → clamped to `32` with a `tracing::warn!` line naming the request + the clamp.
- Negative or non-integer values → clap parse error at flag-parse time.

**Interaction with `--warm-go-cache=off`**:
The concurrency flag has no effect when warming is off. Flag is silently ignored; no warn log fires (the operator may pass it in a shared config template that's occasionally reused with warming-off scans).

## Advisory-log detection contract

The FR-004 advisory log fires iff all four conditions hold:
1. Scan produced at least one Go component (FR-009).
2. `--offline` is not set.
3. `--warm-go-cache` was NOT explicitly set to any value (i.e., the default `off` was picked passively).
4. The emitted SBOM's `mikebom:go-transitive-fallback-count` value is a positive integer.

Detection of condition (3) uses `clap::ArgMatches::value_source("warm_go_cache") == Some(ValueSource::DefaultValue)`. Explicitly passing `--warm-go-cache=off` returns `ValueSource::CommandLine` and suppresses the advisory.

**Log line (stable substring)**:

```
mikebom:go-transitive-fallback-count > 0 detected. Prime the cache with --warm-go-cache=per-workspace or 'go mod download' per workspace before scanning.
```

Emitted at `tracing::info!` level exactly once per scan. Structured-log-mode emitters (JSON via `tracing`) MUST include the same substring in the message field so `grep -F` on the rendered form matches.
