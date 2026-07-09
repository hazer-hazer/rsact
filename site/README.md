# rsact website (`site/`)

VitePress (Vue 3 + TypeScript + SCSS) site for rsact — landing, docs, live
metrics, and rustdoc `/api/`. The Node toolchain is contained entirely in this
directory; the Rust workspace never depends on it.

## Develop

```sh
cd site
npm ci
npm run docs:dev     # hot-reload; the metrics dashboard renders the sample fixture
npm test             # vitest: metrics viewer logic + a mounted-component smoke test
npm run docs:build   # production build → .vitepress/dist
```

## How it deploys

`.github/workflows/site.yml` assembles ONE GitHub Pages artifact:

1. `npm run docs:build` → `.vitepress/dist`
2. metrics: fetch the `metrics-data` branch → `scripts/assemble-metrics.ts`
   writes `dist/metrics/data.json` (one history-ordered `{snapshots,index}` the
   `<MetricsDashboard>` component fetches at runtime)
3. rustdoc: `cargo doc` for the core crates → `dist/api/`
4. `actions/upload-pages-artifact` + `actions/deploy-pages`

Triggers: master content pushes, metrics-workflow completion (fresh snapshots),
manual dispatch. PRs build-only (the CI check). `metrics.yml` still WRITES the
`metrics-data` branch (durable store) — it just stopped being the Pages source.

## One-time cutover (human, post-merge)

1. Merge the WS19 PR to `master`.
2. Repo **Settings → Pages → Source → GitHub Actions** (turns off the
   `metrics-data` branch source).
3. Confirm the `site` workflow deploys; site live at `/rsact/`, with
   `/rsact/metrics/` and `/rsact/api/`. Also **click the "API" nav item** (not
   just the direct URL) and confirm it loads the rustdoc in a real page/new tab
   — not an in-app 404 (rustdoc is a static dir, not a VitePress route).
4. Prove the data path is intact: push a commit → `metrics.yml` records a new
   snapshot on `metrics-data` and `site.yml` redeploys with the new point; open
   a test PR → exactly one `rsact-metrics` sticky comment appears.
