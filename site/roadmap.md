# Roadmap

rsact is developed against a living plan of record — 21 workstreams (WS0–WS19)
plus decision gates — covering the reactivity engine, the UI pipeline, embedded
bring-up, performance transparency, and this website.

The authoritative document lives in the repository:
[`docs/plans/2026-07-05-rsact-evolution-roadmap.md`](https://github.com/hazer-hazer/rsact/blob/master/docs/plans/2026-07-05-rsact-evolution-roadmap.md).

Highlights already delivered include the reactivity engine's render-identity
redesign (Probe), a metrics/CI harness that records footprint per commit (see
the [metrics page](/metrics/)), and per-page reactive scope disposal.

The broad direction: decouple from `embedded-graphics` as a hard dependency
(one render target among several), eliminate panics in favour of logged errors,
and keep optimizing signal/memo change propagation — all measured, not asserted.
