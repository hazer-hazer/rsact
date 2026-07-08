# rsact metrics dashboard (Vue)

The GitHub-Pages / `file://` metrics dashboard (WS0.9e), as a Vue 3 single-file
app. Replaces the old hand-written JS that lived inside a Rust string in
`metrics-probe/src/html.rs`.

## How it fits together

```
src/**  ──(npm run build)──▶  dist/index.html  ──(include_str!)──▶  metrics-probe binary
                              (one self-contained     │
                               file, __DATA__          │  metrics-probe html:
                               placeholder)            └─ replaces __DATA__ with the
                                                          snapshot+index JSON → writes
                                                          metrics/index.html (Pages)
```

- `vite build` + `vite-plugin-singlefile` inline **all** JS/CSS into one
  `dist/index.html` — no external requests, so it opens from `file://` and is safe
  to publish to Pages (the project's long-standing "self-contained viewer" rule).
- The shell (`index.html`) holds `<script type="application/json" id="metrics-data">__DATA__</script>`.
  A `application/json` block is data, not JS, so the build leaves the `__DATA__`
  token byte-for-byte.
- `metrics-probe` bakes `dist/index.html` in with `include_str!` and, at generation
  time, replaces `__DATA__` with `{"snapshots":[…],"index":{…}}` (commits already
  topologically ordered). **`metrics-probe` needs no Node at runtime** — CI, the
  push `record` job, and the backfill job all just inject data into the baked-in file.

## Changing the dashboard

```sh
cd metrics-probe/viewer
npm ci            # first time (or after dependency changes)
npm run dev       # hot-reload dev server, renders src/lib/sample.js fixture data
npm test          # vitest: pure logic (series/chart) + a mounted-App smoke test
npm run build     # → dist/index.html
```

Then **commit `dist/index.html`** and rebuild the Rust crate (`cargo build -p metrics-probe`)
— `include_str!` embeds `dist/index.html` at compile time, so the committed build
artifact is what ships. Forgetting to rebuild the viewer just means the dashboard
lags the source; forgetting to commit `dist/` breaks `cargo build` for others.

## Layout

| Path | Role |
|------|------|
| `index.html` | Vite entry shell + the `#metrics-data` injection point |
| `src/main.js` | reads `#metrics-data` (falls back to the sample fixture in `dev`), mounts `App` |
| `src/App.vue` | state (selection, colours) + layout (tables + sidepanel) |
| `src/components/MetricTable.vue` | one group's value table: ▲/▼ markers, row-select, inline chart |
| `src/components/TrendChart.vue` | SVG line chart (per-series normalization, hover tooltip) |
| `src/lib/series.js` | `buildSeries`, domain-aware `trend`, gap handling — **pure, tested** |
| `src/lib/chart.js` | SVG geometry (segments/scale/shapes) — **pure, tested** |
| `src/lib/sample.js` | dev-only fixture (never shipped) |
| `dist/index.html` | built single-file app (**committed**; `include_str!`'d by metrics-probe) |

## Notes

- `node_modules/` is git-ignored; `dist/index.html` is **not** (it must be committed).
- Improvement direction is domain-aware (`src/lib/series.js`): every current metric is
  lower-is-better, so fewer allocs / smaller `.text` / faster ns render as ▲. Add a
  higher-is-better metric to `HIGHER_IS_BETTER` there.
- Absent metrics are **gaps (null), never zeros** — a metric added in a later commit must
  not look like a regression from 0 (the 0.7e/0.7f lesson).
