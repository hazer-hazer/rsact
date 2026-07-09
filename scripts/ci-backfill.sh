#!/usr/bin/env bash
# WS0.9e — historical backfill / gap-fill.
#
# For each first-parent commit in <range> that lacks a snapshot on the durable
# `metrics-data` branch, check it out in a throwaway git worktree and run THAT
# commit's OWN `metrics-probe -- record` (the tool-birth hard boundary: a commit
# can only be measured by instruments that live inside it). The snapshots are
# accumulated into ./metrics/snapshots; the ordering index is then rebuilt
# with HEAD's tool. Idempotent by construction — commits that
# already have a snapshot are skipped — so the same job also repairs any future
# CI-missed push.
#
# Usage: scripts/ci-backfill.sh [<range>]
#   <range>   git rev-list range (default TOOL_BIRTH^..HEAD, i.e. tool-birth
#             through HEAD, inclusive). Commits older than TOOL_BIRTH are always
#             skipped regardless of <range> — metrics-probe won't build there.
#
# Env:
#   DRY_RUN=1  print the per-commit decisions (build? which flags? skip why?) and
#              the summary, WITHOUT pulling metrics-data, building, or publishing.
#              For verifying orchestration/idempotency locally (laptop heap bytes
#              are not comparable to the CI store, so never publish from local).
#
# Publishing is the workflow's job (peaceiris over ./metrics); this script only
# populates ./metrics. Must run on the CI runner class for real snapshots.

set -uo pipefail   # deliberately NOT -e: each commit is best-effort (old trees
                   # may fail to build) and must never abort the whole batch.

TOOL_BIRTH=257f587    # 0.3b "WS0.3b: metrics-probe" — the tool is born here.
BENCH_BIRTH=f42c154   # 0.9d — `--benches` flag/schema born here.
SIZES_EVERY=5         # sparse `--sizes`: every Nth commit of the tool-birth chain.
CRIT_ARGS="--warm-up-time 0.5 --measurement-time 1.0 --sample-size 10"

DRY_RUN="${DRY_RUN:-}"
range="${1:-${TOOL_BIRTH}^..HEAD}"

root="$(git rev-parse --show-toplevel)"
cd "$root"
mkdir -p metrics/snapshots

if [ -z "$DRY_RUN" ] && git rev-parse --verify --quiet metrics-data >/dev/null; then
    echo "== pulling existing history from metrics-data =="
    git archive metrics-data -- snapshots 2>/dev/null | tar -x -C metrics || true
    git archive metrics-data -- index.json 2>/dev/null | tar -x -C metrics || true
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"; git worktree prune 2>/dev/null || true' EXIT
size_list="$tmp/size_revs"
: > "$size_list"

# Deterministic sparse-sizes subset: every Nth commit of the FULL tool-birth
# chain (stable across runs and independent of <range>).
i=0
while IFS= read -r c; do
    if [ $(( i % SIZES_EVERY )) -eq 0 ]; then echo "$c" >> "$size_list"; fi
    i=$(( i + 1 ))
done < <(git rev-list --first-parent "${TOOL_BIRTH}^..HEAD" 2>/dev/null)

have=0 old=0 built=0 failed=0 wt="$tmp/wt"

# Process substitution (not a pipe) so the counters survive into this shell.
while IFS= read -r rev; do
    # Clamp: never touch commits older than tool-birth.
    if ! git merge-base --is-ancestor "$TOOL_BIRTH" "$rev" 2>/dev/null; then
        old=$(( old + 1 )); continue
    fi
    # Idempotent skip: snapshot already present.
    if [ -f "metrics/snapshots/$rev.json" ]; then
        have=$(( have + 1 )); continue
    fi

    # Decide flags for this commit.
    flags=""
    if git merge-base --is-ancestor "$BENCH_BIRTH" "$rev" 2>/dev/null; then
        flags="--benches"
    fi
    if grep -qxF "$rev" "$size_list"; then flags="$flags --sizes"; fi

    if [ -n "$DRY_RUN" ]; then
        echo "WOULD build $rev   flags=[${flags:-layer1-only}]"
        built=$(( built + 1 )); continue
    fi

    echo ">> backfilling $rev  flags=[${flags:-layer1-only}]"
    rm -rf "$wt"
    if ! git worktree add --quiet --detach "$wt" "$rev" 2>/dev/null; then
        echo "   worktree add failed; skipping"; failed=$(( failed + 1 )); continue
    fi
    (
        cd "$wt" || exit 1
        case "$flags" in
            *--benches*)
                rm -rf target/criterion
                cargo bench -q -p rsact-reactive --features std --bench reactivity -- $CRIT_ARGS || true
                cargo bench -q -p rsact-ui --features "std,embedded-graphics" --bench layout -- $CRIT_ARGS || true
                ;;
        esac
        cargo run -q -p metrics-probe -- record $flags
    )
    if [ -f "$wt/metrics/snapshots/$rev.json" ]; then
        cp "$wt/metrics/snapshots/$rev.json" "metrics/snapshots/$rev.json"
        built=$(( built + 1 )); echo "   recorded"
    else
        echo "   probe produced no snapshot (build/run failed); skipping"
        failed=$(( failed + 1 ))
    fi
    git worktree remove --force "$wt" 2>/dev/null || rm -rf "$wt"
done < <(git rev-list --first-parent "$range" 2>/dev/null)

echo
echo "== backfill decisions: build=$built  already-had=$have  pre-tool-skipped=$old  failed=$failed =="

if [ -n "$DRY_RUN" ]; then
    echo "(DRY_RUN: no builds, no pull, no publish)"
    exit 0
fi

# Rebuild the ordering index for EVERY snapshot rev from full git history
# (backfill runs at fetch-depth 0).
cargo run -q -p metrics-probe -- index

count=$(ls metrics/snapshots/*.json 2>/dev/null | wc -l | tr -d ' ')
echo "== backfill complete: $count snapshots present =="
