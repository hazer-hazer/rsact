# rsact metrics store (WS0.3)

Local-first framework-metrics snapshots produced by the `metrics-probe` tool.

```sh
cargo run -p metrics-probe -- record        # snapshot HEAD -> snapshots/<rev>.json + regenerate index.html
cargo run -p metrics-probe -- diff <rev>     # compare the current working tree against a stored snapshot
cargo run -p metrics-probe -- diff <file>    # ...or against an explicit snapshot file
cargo run -p metrics-probe -- html           # regenerate the static viewer from snapshots/
```

## Layout

- `snapshots/<git-rev>.json` — one snapshot per commit, keyed by `git rev-parse HEAD`.
  A dirty working tree records as `<rev>-dirty.json` so it never overwrites the
  committed baseline for that rev.
- `index.html` — a self-contained static viewer (all snapshots inlined), openable
  straight from `file://`.

Both are **git-ignored**: snapshots are per-developer local history, and CI
archives its own. The tool CI runs is this same binary; CI merely stores the JSON
it emits and posts the `diff` output as a PR comment — it never replaces the local
tool.

## What is measured

**Layer 1 — framework metrics (host, always present).** Per scenario
(`reactive_only_16`, `ui_labels_5`, `ui_labels_10`): reactive node counts by kind
(via `current_runtime_profile()`), steady-state + peak heap bytes, build
allocations, and idle/change-frame allocations. When built with the
`rsact-ui/layout-counters` feature, layout visit/measure counts are included too.

**Layer 2 — target section sizes (`.text`/`.rodata`/`.bss`).** Schema is present
(`section_sizes`); populated when the thumb size probes are built. See the roadmap
(WS0.3) for status.
