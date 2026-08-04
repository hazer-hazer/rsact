#!/usr/bin/env bash
# WS0.9a — the test suites CI runs, runnable verbatim by a developer.
#
# Tests run serially (--test-threads=1): the reactive runtime is a thread-local
# and the metrics-probe scenarios share a thread-local runtime. rsact-ui needs
# `--lib` (its examples lack required-features) AND a font provider feature.
set -euo pipefail

echo "== rsact-reactive =="
# The 2 known-fails are owned acceptance tests (static_wrapper → WS4,
# observe_recreates_disposed_child_observer → WS2). Skip them so the job is
# green-by-baseline and red on any NEW failure; WS2/WS4 drop the skips.
cargo test -p rsact-reactive --features std --lib -- --test-threads=1 \
    --skip static_wrapper \
    --skip observe_recreates_disposed_child_observer

echo "== rsact-ui (lib) =="
cargo test -p rsact-ui --lib --features "std,embedded-graphics" -- \
    --test-threads=1

echo "== rsact-ui (lib, incremental-layout) =="
# WS6.4.0(iv): the tests behind `incremental-layout` — WS5.2's incremental
# relayout and WS6.1's targeted invalidation — were running NOWHERE in CI
# (ci-test.sh omitted the feature; the powerset only *compiles* it). That is 9
# tests, and they are the ones that catch mis-ordering the relayout against the
# render gate: both halves of that ordering were verified to fail here and pass
# on default features, so without this job such a regression ships green.
cargo test -p rsact-ui --lib \
    --features "std,embedded-graphics,incremental-layout" -- --test-threads=1

echo "== rsact-render =="
cargo test -p rsact-render --features "std,embedded-graphics,tiny-skia" -- \
    --test-threads=1

echo "== metrics-probe (layout-counters) =="
cargo test -p metrics-probe --features layout-counters -- --test-threads=1

echo "all test suites green"
