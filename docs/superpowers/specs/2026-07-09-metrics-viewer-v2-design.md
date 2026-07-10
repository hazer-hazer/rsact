# Metrics viewer v2 + single-viewer consolidation — design

**Date:** 2026-07-09 · **Workstream:** WS19.7 (follow-up to WS19; roadmap `docs/plans/2026-07-05-rsact-evolution-roadmap.md`)
**Branch:** `ws19-metrics-v2`, stacked off `ws19-website` (PR #12 still open; rebase onto `master` after #12 merges).

## Goal

Make the site's metrics page the single, polished home for rsact metrics. Two phases:
- **A. Consolidation** — remove the standalone `metrics-probe` HTML viewer; metrics live only on the site; the site's dev mode gains the ability to chart the local git-ignored store so no capability is lost.
- **B. Enhancements** — six maintainer-requested UX fixes (one is a real bug) plus four adopted extras.

## Confirmed decisions (2026-07-09)
- Branch stacks off `ws19-website` (needs the site component to enhance).
- Commit-collapse (#3) is **default-on** with an "expand all" toggle.
- Recorded as **WS19.7** in the roadmap.
- All four extras in scope: synchronized crosshair, URL hash state, Δ-from-baseline, only-changed filter.

## Verified current state
- **Feature parity confirmed:** the site's `MetricsDashboard` (site/.vitepress/theme/components/) is a functional superset of the standalone `metrics-probe/viewer/` `App.vue` — same `buildSeries`, table-per-group, ▲/▼ domain-aware markers, gaps-not-zeros, click-to-expand inline charts, sidepanel overlay (per-series normalization) + hover tooltip, select-all/clear — plus runtime data-loading, loading/empty states, SSR safety.
- **The two viewers read DIFFERENT stores:** the site fetches the CI store (`metrics-data` branch, CI-runner numbers); the standalone `metrics-probe html` also renders the **local git-ignored `metrics/` store** (machine numbers, fed by the post-commit hook). Removing the standalone therefore drops the local store's *visual* view — preserved here via a dev-mode loader.
- **Removal surface (not just a folder delete):** `metrics-probe/viewer/`; `metrics-probe/src/html.rs`; `main.rs` (`mod html` :19, `html::regenerate` call in `cmd_record` :98, `html` subcommand :447, usage line :430, doc line :7); `scripts/ci-backfill.sh` (`metrics-probe -- html` :123 + comment :121); `scripts/ci-metrics.sh` (comment refresh). `record`/`diff`/`index`/`hook-install` + the local store + `metrics-data` writes all stay.
- **VitePress full-width is first-class:** `layout: page` frontmatter renders "a custom page with no applied styles" (the 992/752px prose caps are `doc`-layout-only).

## A. Consolidation

### A1. Remove the standalone viewer
Delete `metrics-probe/viewer/` and `metrics-probe/src/html.rs`; remove the four `main.rs` wirings (`mod html`, the `cmd_record` call, the subcommand, the usage/doc mentions); drop the `metrics-probe -- html` step from `ci-backfill.sh`; refresh `ci-metrics.sh` comments. Confirm `metrics-probe`'s existing tests don't reference `html` and stay green. `record` still writes `snapshots/<rev>.json` + merges `index.json`; it no longer emits `index.html`.

### A2. `metrics-data` becomes data-only
Prune the now-vestigial `index.html`, `README.md`, `.nojekyll` from the `metrics-data` branch (harmless post-cutover since Pages source = Actions). The store is `snapshots/` + `index.json` only. `metrics.yml`'s publish step is unchanged (it publishes whatever `metrics/` contains).

### A3. Shared assembly + dev-mode local-store loader
Factor the assembly core into one pure module `site/.vitepress/theme/lib/assemble.ts` — `assemble(index, snapshots[]): MetricsData` (sort by topo/date, the logic currently in `assemble-metrics.ts` and the deleted `html.rs`). Consumers:
- CI: `site/scripts/assemble-metrics.ts` (thin wrapper: read a `metrics-data` checkout → `assemble` → write `dist/metrics/data.json`).
- Dev: a **Vite dev-server plugin** in `.vitepress/config.mts` (`configureServer`) that intercepts `GET /metrics/data.json` (and the based path) during `docs:dev` only, reads the repo-root `metrics/` store (`../../metrics` relative to the config), runs `assemble`, and returns it. Empty/absent store → the bundled `SAMPLE` fixture. Prod (`docs:build`) is untouched — the static `dist/metrics/data.json` from CI is served. `MetricsDashboard`'s fetch path is unchanged: same URL, real local data in dev, CI data in prod.

## B. Enhancements

### B1. Fixed-layout table pillar (#1 sticky, #2 alignment bug, #4 uniform width)
`MetricTable` uses `table-layout: fixed`. The first column (`.lbl`) is a fixed, shared width (a CSS var reused by every table so they align vertically for cross-table eyeballing) with `position: sticky; left: 0` and a solid background; long metric names ellipsize with a `title` tooltip. Commit columns are equal width.
- **#2 bug fix:** today the inline chart row is `<td colspan="snapshots.length + 1">` (spans the metric column) and `xOf` anchors points at `i/(n-1)`, so points sit left of their commit cells. Fix: the chart row is an empty `<td>` (under the sticky column) + a `<td :colspan="ncols">` for the chart; the chart's x maps point `i` to the **center of cell i**: `x = (i + 0.5) / n * width`. Apply the same cell-centered convention to the sidepanel overlay so table and overlay agree. (`chart.ts` gains a cell-centered `xOf` variant; existing tests updated to the new geometry.)

### B2. Collapse unchanged commits (#3)
A pure, global pre-pass `collapse.ts` over topo-ordered snapshots. A commit is a **boundary** (starts a new column) iff, for some tracked metric, its *present* value differs from that metric's *previous present* value — i.e. the same gap-skipping `prevPresent` semantics that power the ▲/▼ markers. **A null is never a boundary**: it means "not measured here," not "changed," so it carries forward and is absorbed into the surrounding run. Maximal spans between boundaries collapse to one column headed `<first8>..<last8>` (single-commit columns keep the plain `revLabel`). Global (shared column axis across all tables) to preserve #4 alignment. Produces the reduced snapshot/column list that `buildSeries` and all charts consume. **Default-on**, with an "expand all" toggle in the controls (state also in the URL, B5).

Because "boundary" compares against the previous *present* value (not the immediate neighbour), a real change that straddles a gap is still preserved — `[10, null, 20]` splits at `20` rather than collapsing and hiding the 10→20 move. Within a collapsed run a metric's present values are therefore all equal by construction, so the column shows that single value (or a gap if the metric is absent throughout the run). Unit-tested: `[10,10,null,10]` fully collapses (gap absorbed, shows 10); `[10,null,20]` splits (real change across a gap preserved); a late-appearing metric (`[null,null,11,9]`) splits only at the 11→9 move; single-commit columns unaffected.

### B3. Stable per-metric colors (#5)
`colors.ts`: `colorFor(key: string): string` — a deterministic hash of the metric key into an expanded ~16-color qualitative palette. Replaces the `PALETTE[cursor++ % len]` cycle in `MetricsDashboard`. Same key → same color in the table swatch, inline chart, overlay line, and legend, everywhere and every session. Hash collisions only matter when two colliding metrics are co-selected (rare at 16 colors); the legend disambiguates. Unit-tested for determinism + spread.

### B4. Full-width page (#6)
`site/metrics/index.md` gets `layout: page` frontmatter. `MetricsDashboard` styled to fill the width with sensible padding and a max cap. No CSS override of internal VitePress classes.

### B5. Extras
- **Synchronized crosshair:** `MetricsDashboard` owns a `hoveredCol: Ref<number|null>` provided (`provide`/`inject`) to `MetricTable` + `TrendChart`. Hovering any commit cell or chart point sets it; every table highlights that column and every chart draws its guide at that column. Serves the cross-table correlation goal behind #4.
- **Δ-from-baseline toggle:** a control that renders each value as delta from the first visible (collapsed) column; a pure transform over the series consumed by tables (show `+N`/`−N`, keep the ▲/▼) and charts. Toggle state in the URL.
- **Only-changed filter:** a control that hides rows/groups flat across the visible range; a pure predicate over `SeriesRow.values`. Toggle state in the URL.
- **URL hash state:** encode selected metric keys + the three toggles (collapse, Δ, only-changed) in `location.hash`; restore on mount, update on change (`history.replaceState`, no navigation). Client-only (guarded like the fetch). Makes a view shareable/bookmarkable.

## Architecture / boundaries
Logic stays in pure, tested `lib/*.ts`; Vue components stay thin. New/changed pure modules: `assemble.ts` (new, shared), `collapse.ts` (new), `colors.ts` (new), `chart.ts` (cell-centered geometry), `series.ts` (Δ + only-changed helpers). Components: `MetricsDashboard.vue` (owns data-load, selection, hover, toggles, URL sync), `MetricTable.vue` (fixed/sticky layout, collapsed headers, crosshair, Δ display), `TrendChart.vue` (cell-centered points, crosshair guide). Each pure module is independently testable; the components remain assertion targets for mount tests.

## Testing
- Pure vitest: `collapse` (all-equal/any-change/gaps/single), `colors` (determinism + distinctness), `chart` cell-centered geometry (point i center = `(i+0.5)/n*width`), `series` Δ + only-changed.
- Component mount tests: sticky/fixed layout renders; collapsed `a..b` headers; crosshair sync (hover a cell → guides appear in charts); Δ toggle flips values; only-changed hides flat rows; URL round-trip (set selection → hash updates → remount restores).
- Rust: `metrics-probe` test suite green after `html` removal (verify no test references it).
- Manual/local: `docs:dev` charts the real local `metrics/` store; `docs:build` green; `metrics-probe record` still works without `html`.

## Bookkeeping
Add **WS19.7** to the roadmap (metrics viewer v2 + single-viewer consolidation), marked done with commit hashes. Update the WS19.3 note: the "html.rs viewer = local dev view" split is dissolved — one viewer (the site), which loads either store.

## Risks
- **Stacked branch:** if PR #12 changes in review, rebase `ws19-metrics-v2`. Accepted (avoids blocking on the human Pages cutover).
- **Collapse + gaps:** a gap (null) adjacent to a value must count as a change, or collapsing would hide a metric's appearance/disappearance. Covered by tests.
- **`metrics-data` prune:** removing files from an orphan data branch is a force-ish history edit on that branch; done via the normal publish mechanism (peaceiris with `keep_files:false` for the prune, or an explicit removal commit) — must not touch `snapshots/`/`index.json`. Detailed in the plan.
- **Cell-centered geometry change** alters existing `chart.ts` test expectations (endpoints → centers); tests updated deliberately, not deleted.
