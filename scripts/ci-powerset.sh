#!/usr/bin/env bash
# WS0.9a — per-crate feature powersets, runnable verbatim by a developer.
#
# A single `--all` powerset can't be green by design (rsact-reactive needs a
# downstream-chosen backend; rsact-ui needs a font provider), so each
# axis-owning crate is checked on its own. Rationale + the full (exhaustive)
# matrix live in docs/features.md. Requires cargo-hack (`cargo install cargo-hack`).
#
# reactive/render are small — checked exhaustively. rsact-ui's powerset is 1152
# combos (~30 min), so CI bounds it to `--depth 2` (all feature *pairs* — catches
# the interaction bugs; the exhaustive run is the docs/features.md command for
# local/nightly use). Override with UI_DEPTH= (empty) for exhaustive.
set -euo pipefail

UI_DEPTH="${UI_DEPTH-2}"

echo "== rsact-reactive (storage-backend axis) =="
cargo hack check --feature-powerset --no-dev-deps -p rsact-reactive \
    --mutually-exclusive-features std,single-thread,unsafe-single-thread \
    --at-least-one-of std,single-thread,unsafe-single-thread

echo "== rsact-render (+ libm/micromath math axis; std forced for its backend) =="
cargo hack check --feature-powerset --no-dev-deps -p rsact-render \
    --mutually-exclusive-features std,single-thread,unsafe-single-thread \
    --at-least-one-of std,single-thread,unsafe-single-thread \
    --mutually-exclusive-features libm,micromath \
    --at-least-one-of libm,micromath

echo "== rsact-ui (backend + math + required font provider; skip WIP tiny-icons) =="
cargo hack check --feature-powerset ${UI_DEPTH:+--depth "$UI_DEPTH"} \
    --no-dev-deps -p rsact-ui \
    --exclude-features tiny-icons \
    --mutually-exclusive-features std,single-thread,unsafe-single-thread \
    --at-least-one-of std,single-thread,unsafe-single-thread \
    --mutually-exclusive-features libm,micromath \
    --at-least-one-of libm,micromath \
    --at-least-one-of embedded-graphics,u8g2-fonts

echo "all powersets green"
