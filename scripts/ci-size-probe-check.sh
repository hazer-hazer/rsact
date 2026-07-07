#!/usr/bin/env bash
# WS0.9 hardening — compile-gate the Layer-2 `size-probe` binaries.
#
# The size probes are excluded from the workspace (they only build for thumb
# targets), so a plain `cargo build`/`check --workspace` never touches them, and
# `metrics-probe --sizes` treats a *failed* probe build the same as a missing
# toolchain — it logs "skipped" and carries on. That is exactly how the reactive
# probe's stale `runtime::observe()` reference (deleted in WS2) went unnoticed
# from WS2 until WS9a.1: the reactive `.text/.rodata/.bss` silently vanished
# from every snapshot while nothing failed.
#
# This turns probe bit-rot into a hard CI failure at PR time, while metrics.yml
# stays informational/non-gating. Release profile (opt-z + fat-LTO + panic=abort
# from size-probe/Cargo.toml) so it exercises the exact build metrics.yml runs.
# thumbv7m (Blue Pill floor) only — API drift is target-independent, so this
# catches bit-rot; thumbv6m is exercised by the master metrics run.
set -euo pipefail

cargo build \
    --manifest-path size-probe/Cargo.toml \
    --release \
    --target thumbv7m-none-eabi \
    --bin reactive \
    --bin ui \
    --target-dir target/size-probe
