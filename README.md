# rsact metrics store (WS0.3)

Local-first framework-metrics snapshots produced by the `metrics-probe` tool.

```sh
cargo run -p metrics-probe -- record          # Layer-1 snapshot HEAD -> snapshots/<rev>.json + index.html
cargo run -p metrics-probe -- record --sizes  # ...also build the thumb size-probes + record Layer-2 sizes
cargo run -p metrics-probe -- diff [--sizes] <rev|file>   # compare current tree vs a stored snapshot
cargo run -p metrics-probe -- html            # regenerate the static viewer from snapshots/
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

## Automatic snapshots on commit (WS0.8, opt-in)

Enable the committed post-commit hook once per clone:

```sh
cargo run -p metrics-probe -- hook-install     # or: git config core.hooksPath .githooks
```

After that every commit records a **Layer-1** snapshot (no `--sizes`) for the new
commit, **in the background** — the commit returns instantly, output goes to
`metrics/hook.log`, and the hook **never blocks or fails the commit** (metrics
observe, they don't gate; the 0.4 regression test and CI deltas are the gates).
It skips during rebase/cherry-pick/merge and when the commit already has a
snapshot. Because snapshots are git-ignored, the hook only fills in *your local*
timeline — the durable shared record is still CI's archive.

## What is measured

**Layer 1 — framework metrics (host, always present).** Per scenario
(`reactive_only_16`, `ui_labels_5`, `ui_labels_10`): reactive node counts by kind
(via `current_runtime_profile()`), steady-state + peak heap bytes, build
allocations, and idle/change-frame allocations. When built with the
`rsact-ui/layout-counters` feature, layout visit/measure counts are included too.

*WS4.6 capacity metrics:* `values_capacity` and `values_vacant` record the value
`SlotMap`'s backing capacity and its retained-but-unused slack (`capacity −
total`). **Contract: capacity = peak.** `SlotMap::remove` frees a disposed
node's payload immediately, but the slot array — and the dense `SecondaryMap`s
keyed by `ValueId` (subscribers/sources/owned/mark_seen) — never shrink; capacity
grows to the peak live node count and stays there, so it is the runtime's
permanent node-slot RAM high-water. On embedded that determinism is usually
desirable, so these are recorded (informational), **not** asserted (SlotMap's
growth strategy is an implementation detail). In-place compaction would require a
full rebuild — deferred to WS9b.1's storage rework, which changes this layout.

**Layer 2 — target section sizes (`.text`/`.rodata`/`.bss`), opt-in via `--sizes`.**
Builds the `size-probe` crate (excluded from the workspace; a `cortex-m-rt` +
`embedded-alloc` + generic `memory.x` no_std binary that is *linked but never run*
— the numbers are the regression signal) for the floor targets (thumbv7m Blue
Pill + the thumbv6m baseline) with two binaries — `reactive` (pure engine) and
`ui` (a headless 10-label page) — at `opt-level="z"`, fat-LTO. Sections are read
from the ELF with the `object` crate. NOTE: `.bss` is dominated by the probe's
tiny fixed heap buffer + cortex-m-rt statics (framework RAM is heap-resident, not
`.bss`), so `.text`/`.rodata` are the meaningful flash signal. Add
`thumbv7em-none-eabihf` (Black Pill) to `TARGETS` in `sizes.rs` for its budgets.
