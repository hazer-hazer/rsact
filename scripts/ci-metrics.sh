#!/usr/bin/env bash
# WS0.9b/0.9c/0.9d — record a metrics snapshot for HEAD over the accumulated
# history. Runnable locally; in CI the durable
# history lives on the orphan `metrics-data` branch and is pulled in first.
#
# Usage: scripts/ci-metrics.sh [--sizes] [--benches]
#   --sizes    build the thumb size-probes → Layer-2 .text/.rodata/.bss (minutes;
#              master-only in CI per 0.9c)
#   --benches  run the criterion bench groups (bounded time) then record their
#              medians → Layer WS0.9d wall-clock trend
#
# metrics-probe `record` writes metrics/snapshots/<rev>.json keyed by HEAD; the
# site assembles + charts the store.
set -euo pipefail

sizes=""
benches=""
for arg in "$@"; do
    case "$arg" in
        --sizes) sizes="--sizes" ;;
        --benches) benches="--benches" ;;
        *)
            echo "ci-metrics.sh: unknown arg '$arg' (want --sizes / --benches)" >&2
            exit 2
            ;;
    esac
done

mkdir -p metrics/snapshots

# Pull existing snapshots + the ordering index from the durable data branch so
# `record` APPENDS to the accumulated history and PRESERVES the backfilled index
# (order/pr for every commit) — otherwise it would publish a fresh single-entry
# index.json that clobbers the rich one (snapshots survive via keep_files, but
# index.json is one fixed path → overwritten). A fresh CI checkout has no local
# `metrics-data` ref, so fetch it explicitly (remote-only); harmless locally.
# Each pathspec is tolerated-if-absent (index.json won't exist on first run).
if git fetch --quiet origin metrics-data 2>/dev/null; then
    git archive FETCH_HEAD -- snapshots 2>/dev/null | tar -x -C metrics || true
    git archive FETCH_HEAD -- index.json 2>/dev/null | tar -x -C metrics || true
elif git rev-parse --verify --quiet metrics-data >/dev/null; then
    # local run: the data branch may already be a local ref (no remote to fetch).
    git archive metrics-data -- snapshots 2>/dev/null | tar -x -C metrics || true
    git archive metrics-data -- index.json 2>/dev/null | tar -x -C metrics || true
fi

# WS0.9d: run the criterion groups fresh so target/criterion holds only THIS
# commit's results (metrics-probe reads */new/estimates.json). Times are bounded
# — this is a noisy CI-runner trend, not a decision-grade A/B — and tunable here.
# Local criterion baselines remain the real A/B instrument.
if [ -n "$benches" ]; then
    rm -rf target/criterion
    crit_args="--warm-up-time 0.5 --measurement-time 1.0 --sample-size 10"
    cargo bench -q -p rsact-reactive --features std --bench reactivity -- $crit_args
    cargo bench -q -p rsact-ui --features "std,embedded-graphics" --bench layout -- $crit_args
fi

cargo run -q -p metrics-probe -- record ${sizes:+$sizes} ${benches:+$benches}
