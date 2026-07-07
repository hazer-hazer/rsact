#!/usr/bin/env bash
# WS0.9b — record a metrics snapshot for HEAD over the accumulated history and
# regenerate the dashboard. Runnable locally; in CI the durable history lives on
# the orphan `metrics-data` branch and is pulled in first.
#
# Usage: scripts/ci-metrics.sh [--sizes]
#
# metrics-probe `record` writes metrics/snapshots/<rev>.json (keyed by HEAD) and
# regenerates metrics/index.html over every snapshot present — so pulling the
# history in first makes the dashboard cover the whole timeline.
set -euo pipefail

sizes="${1:-}"
mkdir -p metrics/snapshots

# Pull existing snapshots from the data branch if it exists (CI). Harmless with
# no such branch (local / first run / bootstrap).
if git rev-parse --verify --quiet metrics-data >/dev/null; then
    git archive metrics-data -- snapshots 2>/dev/null | tar -x -C metrics || true
fi

cargo run -q -p metrics-probe -- record ${sizes:+$sizes}
