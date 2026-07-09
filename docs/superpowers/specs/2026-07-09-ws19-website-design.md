# WS19 — Website (VitePress on GitHub Pages): v1 design

**Date:** 2026-07-09 · **Workstream:** WS19 (roadmap `docs/plans/2026-07-05-rsact-evolution-roadmap.md`)
**Scope this pass:** full v1 — items **19.1–19.5**. 19.6 (blog, custom domain, social cards, sitemap) is explicitly out of scope.

## 1. Goal & decisions (from the roadmap, all inline)

Ship a public site at `hazer-hazer.github.io/rsact/` that is **landing + docs skeleton + a live metrics section**, plus rustdoc under `/api/`. Decisions already recorded (2026-07-08):

- **VitePress** (docs+landing+blog hybrid; Vue components enable native metrics charts + future WASM demos). Node toolchain **isolated in `site/`**.
- **One assembled GitHub Actions Pages deployment** replaces the current `metrics-data`-branch Pages source.
- **VitePress owns all docs** — WS11.9's mdBook is dropped; the guide's destination becomes the site's docs section; rustdoc under `/api/`.
- **`base: '/rsact/'`** for now (custom domain later = one CNAME + base switch).

Session-level decisions (2026-07-09):
- **Full v1 (19.1–19.5)** in one PR on `ws19-website`.
- **Default VitePress theme + light brand** (no bespoke landing).
- **Land code + document the manual Pages cutover** — the Pages-source flip and merge are GitHub-side human steps; recorded like WS0.9b/c/e.
- Stack: **VitePress + Vue 3 + TypeScript + SCSS + Vitest** (user preference: TS over JS, SCSS over CSS).

## 2. Verified current state

- **Pages today** is served from the orphan **`metrics-data`** branch (`/` root). `metrics.yml` writes a snapshot there on every push via `peaceiris/actions-gh-pages` and publishes the dashboard. PR runs post one sticky `rsact-metrics` comment.
- **0.9e landed** and its dashboard was **rewritten as a Vue 3 SFC app** in `metrics-probe/viewer/` (PR #10, `2c61bfd`): `App.vue`, `components/MetricTable.vue`, `components/TrendChart.vue`, pure tested `lib/series.js` + `lib/chart.js`, dev fixture `lib/sample.js`. Data is injected as `<script type="application/json" id="metrics-data">{snapshots,index}</script>`. → **19.3 is a Vue→Vue port**, not a build.
- **Data shapes:** `index.json` = `{ rev: { date, parent, branch } }` (topologically orderable); each snapshot = `{ git_rev, host, scenarios:[{name, counts{…}, heap_*_bytes, *_allocs, layout}], … , section_sizes?, bench_medians? }`.
- Content sources: root `README.md` (the pitch, setup, target support), `docs/features.md` (feature matrix), `CLAUDE.md` (architecture).
- Node v22.17 / npm 10.9 present; npm registry reachable (VitePress 1.6.4, Vue 3.5.39). Local build is genuinely verifiable.
- No `site/` dir yet. `ci.yml` has no Node (must stay so). WS11.9 line already carries the mdBook→site amendment (`3157abd`); this pass only refines its destination pointer.

## 3. Directory layout (`site/`)

```
site/
  package.json               # vitepress, vue, sass, vitest, @vue/test-utils, jsdom, typescript
  package-lock.json          # committed
  .gitignore                 # node_modules/, .vitepress/cache/, .vitepress/dist/
  tsconfig.json              # for the SFCs/libs
  .vitepress/
    config.mts               # base:'/rsact/', dark/light, Shiki Rust, nav + sidebar
    theme/
      index.ts               # extend default theme; global-register <MetricsDashboard>; import custom.scss
      custom.scss            # brand color (light + dark)
      components/
        MetricsDashboard.vue  # NEW site wrapper: runtime fetch + dev fallback + selection/layout
        MetricTable.vue       # ported from viewer (lang="ts")
        TrendChart.vue        # ported from viewer (lang="ts")
      lib/
        series.ts             # ported + typed (buildSeries, trend, gaps)
        chart.ts              # ported + typed (SVG geometry)
        types.ts              # Snapshot / IndexEntry / Series / ChartGeometry interfaces
        sample.ts             # ported dev fixture
        series.test.ts        # ported vitest
        chart.test.ts         # ported vitest
  index.md                   # landing (home layout)
  docs/
    index.md                 # getting started
    features.md              # feature matrix (seeded from docs/features.md, links canonical)
    architecture.md          # architecture overview (condensed from CLAUDE.md)
  metrics/index.md           # hosts <MetricsDashboard/>
  roadmap.md                 # links repo markdown + interactive artifact
```

`metrics-probe/viewer/` is left untouched = the **local dev view** (the local git-ignored store). The site component = the **public CI view** (records the 19.3 split the roadmap asks for). Ported `series`/`chart` keep their tests → behavioral parity is guarded.

## 4. `site.yml` workflow (19.2 — the structural piece)

**Triggers**
- `push` to `master` on paths `site/**`, `docs/**`, `.github/workflows/site.yml` (content changes).
- `workflow_run` on workflow **"metrics"** `completed` (fresh snapshots landed on `metrics-data`).
- `pull_request` (build-only CI check — satisfies 19.1's "green as a CI check" **without adding Node to `ci.yml`**).
- `workflow_dispatch`.

**`build` job**
1. checkout; `actions/setup-node` (node 22, cache npm in `site/`).
2. `cd site && npm ci && npm run docs:build` → `site/.vitepress/dist`.
3. **Assemble metrics** into `dist/metrics/`: fetch `origin/metrics-data`, read `index.json` + `snapshots/*.json`, emit one `dist/metrics/data.json` = `{ snapshots:[…], index:{…} }` (the viewer's exact contract → component does ONE fetch). Also copy `index.json` + `snapshots/` for transparency. **Do NOT copy metrics-data's `index.html`** — it would clobber the VitePress metrics page's `dist/metrics/index.html`.
4. **rustdoc (19.5)**: `cargo doc --no-deps` per core crate with the documented feature set — `rsact-reactive` (`std`), `rsact-render` (`std,embedded-graphics`), `rsact-ui` (`std,embedded-graphics`); copy `target/doc` → `dist/api/` and write `dist/api/index.html` redirect → `rsact_ui/index.html`.
5. On non-PR: `actions/configure-pages` + `actions/upload-pages-artifact` (path `site/.vitepress/dist`). On PR: stop here (build validated, nothing uploaded/deployed).

**`deploy` job** — `if: github.event_name != 'pull_request'`, `needs: build`, `environment: github-pages`: `actions/deploy-pages`. Top-level `concurrency: { group: pages, cancel-in-progress: false }` (GitHub's own Pages template — never cancel an in-flight deploy). Permissions: `pages: write`, `id-token: write`, `contents: read`.

**Unchanged:** `metrics.yml` still writes to `metrics-data` (durable store, write path identical); `ci.yml` gains no Node; PR sticky comments unaffected. Metrics URLs move to `/rsact/metrics/`.

**Flagged tradeoff:** redeploying on *every* metrics `workflow_run` (all branches push metrics) can be frequent. `cancel-in-progress: false` serializes deploys safely; a comment in `site.yml` notes the maintainer can restrict to `head_branch == master` if runner-minute cost bites. This faithfully implements "after metrics records fresh snapshots" while surfacing the publish-race concern the roadmap flagged.

## 5. Metrics component data flow (19.3)

`MetricsDashboard.vue` mounts → `fetch(withBase('/metrics/data.json'))` → parses `{snapshots, index}` → renders through ported `series.ts`/`chart.ts` + `MetricTable`/`TrendChart`. On fetch failure (local `npm run dev`, no data present) it falls back to the bundled `sample.ts` fixture — mirrors the viewer's `main.js`. Data stays decoupled static files refreshed each deploy; no build-time injection into the VitePress bundle.

## 6. Content (19.4) — honesty rules

- **Landing** (`index.md`, home layout): hero "**you pay for what you wire**"; feature cards (fine-grained reactivity; `no_std` by default; pay-per-use by construction; multi-renderer embedded-graphics + tiny-skia; live performance transparency); a **real** `rsact-ui` code sample extracted from an existing example (verified, not invented); a "measured in CI every commit" note linking `/metrics/`.
- **Numbers policy:** the live dashboard *is* the numbers. The **LVGL/Slint comparison, on-device FPS, and "fits a Blue Pill" headline are marked placeholders pending WS17** — never estimates.
- **Docs skeleton:** `docs/index.md` getting-started (install, pick backends, minimal example — from README); `docs/features.md` feature-matrix summary seeded from `docs/features.md` (links canonical); `docs/architecture.md` overview condensed from CLAUDE.md. This is the skeleton WS11.9's full guide later lands on.
- **Roadmap page:** short workstream summary linking the repo markdown + the interactive artifact.

## 7. Testing & verification

- **Ported logic:** `series.test.ts` + `chart.test.ts` pass under `npm test` (vitest).
- **Build:** `npm run docs:build` green locally (the real 19.1 acceptance).
- **Render check:** `vitepress preview` + drive the metrics page against a fixture `data.json` copied into `dist/metrics/`, confirm charts render and gaps≠zeros.
- **Rust untouched:** no Rust code changes; existing test baseline unaffected.

**19.2 cutover runbook (documented human step — I cannot do these from here):**
1. Merge the `ws19-website` PR to `master`.
2. Repo **Settings → Pages → Source → GitHub Actions** (flips off the `metrics-data` branch source).
3. Confirm `site.yml` deploy succeeds; site live at `/rsact/` with `/rsact/metrics/` + `/rsact/api/`.
4. **Prove metrics-data + PR comments unaffected:** push a commit → `metrics.yml` records a new snapshot on `metrics-data` (durable store intact) → `site.yml` `workflow_run` redeploys with the new point; open a test PR → exactly one `rsact-metrics` sticky comment still appears.

## 8. Roadmap bookkeeping

- Mark 19.1–19.5 `[x]` with their commit hashes.
- Refine WS11.9's already-present amendment to point at the concrete `site/docs/` destination.
- Record the 19.2 verification boundary (code landed; cutover pending human steps) in the WS19 status.

## 9. Risks

- **Deploy frequency** (see §4) — mitigated by serialization + a documented tuning knob.
- **rustdoc multi-crate feature unification** — building per-crate sequentially into a shared `target/doc` avoids the "feature X not on crate Y" error a single multi-`-p` invocation would hit.
- **Sandbox cannot push/flip Pages/merge** — accepted; boundary documented per §7.
