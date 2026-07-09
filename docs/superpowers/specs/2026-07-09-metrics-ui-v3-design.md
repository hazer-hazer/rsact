# Metrics UI v3 — unified table + column interactions (Phase A) — design

**Date:** 2026-07-09 · **Workstream:** WS19.8 (follow-up to WS19.7; roadmap `docs/plans/2026-07-05-rsact-evolution-roadmap.md`)
**Branch:** `ws19-metrics-v3`, stacked off `ws19-metrics-v2` (PR #13, base `master`). Rebase onto `master` after #13 merges.

## Goal

Turn the site's per-group metric tables into one aligned, navigable grid with sticky context, per-column change cues, and clickable commits. Seven maintainer ideas, split into two phases by data availability. **Phase A (this spec)** ships everything achievable with the current store; **Phase B (deferred, recorded)** needs a build-time data enrichment.

## Confirmed decisions (2026-07-09)
- **Table structure:** one unified `<table>`; `MetricTable` becomes `MetricSection` (root `<tbody>`). Component granularity preserved.
- **Commit metadata (message + PR):** enrich the store at build time (Phase B). Not fetched live.
- **Scope:** Phase A now (#1, #2, #3, #5, #6, and #4's hash-link); Phase B later (#4 message tooltip, #7 PR grouping/link).
- Repo base URL constant matches `config.mts` socialLinks: `https://github.com/hazer-hazer/rsact`.

## Verified current state / data shape
- `Snapshot` has `git_rev` (full sha), `git_dirty`, `recorded_at`, `host`, `scenarios`, `section_sizes`, `bench_medians`. `IndexEntry` has `date`, `parent` (parent sha), `branch`. **No commit message, no PR number** — hence the phase split.
- Columns already collapse to a shared axis (`columnGroups`/`columnLabel`, WS19.7). Each column carries `group: number[]` (snapshot indices).
- Confirmed #1 bug: the overlay `TrendChart` is never passed `:snapshots`, and `hover.value` is a *column* index (post-collapse), not a snapshot index — so the tip title falls back to `#<index>`. Folding the tip into the legend (labeled by column) fixes it structurally.

## Phase A design

### A0. Backbone — unified table in a bounded scroll region
`MetricsDashboard` renders a single `<table class="grid">`. `MetricSection` (renamed from `MetricTable`) has a `<tbody>` root; a table legally contains many `<tbody>`s, so each group is one section component.

`overflow-x: auto` computes `overflow-y` to `auto` too (CSS spec), making the box a scroll container on **both** axes — which breaks vertical `position: sticky` against the window. So the grid gets its own **bounded scroll region**: `.grid-scroll { max-height: 80vh; overflow: auto }` (the data-grid pattern). All sticky offsets are then relative to that container, needing no navbar math:
- `thead { position: sticky; top: 0; z-index: 3 }` — commit columns always visible (#2).
- section caption rows: `position: sticky; top: var(--head-h)` where `--head-h` = the thead's measured height (a `ResizeObserver` on the thead sets the CSS var on `.grid`). Captions stack under the header instead of scrolling away (#2). Caption is a full-width row: `<tr class="section-head"><th :colspan="1 + columns.length">{title}</th></tr>` (#3).
- first column sticky-left carries over from WS19.7 (`th.lbl, td.lbl { position: sticky; left: 0 }`); the top-left header cell needs the highest z-index so it wins both axes.

`.grid-scroll` replaces the old `.main { overflow-x: auto }` wrapper. The sidepanel (`.side`, overlay chart + legend) stays a sibling flex column, unchanged.

### A1. Legend absorbs the hover tip (#1) + title fix
- `TrendChart` loses its internal tooltip: remove the `.tip` block, the `tooltip` computed, the `snapshots` prop, and the now-unused `revLabel` import. It keeps the crosshair (`hover`/`sharedHover`/guide) and, when `interactive`, still writes `sharedHover` on move.
- The legend (already in the dashboard, which holds `columns` + `selectedSeries`) becomes hover-aware:
  - a heading line: on `sharedHover != null` → `at {columns[sharedHover].label}:` (the column label — rev8 or `a..b`); otherwise a muted `hover a column for values`.
  - each legend row: swatch + label + (on hover) the series' **collapsed** value at that column via `fmt`; at rest, no trailing value (label only).
- This removes the double display and fixes the title (column label, never `#index`).

### A2. Sticky caption + header (#2)
Delivered by A0 (bounded scroll region + sticky thead + sticky caption rows). No separate mechanism.

### A3. Unified table (#3)
`MetricSection` renders a `<tbody>` containing: the sticky caption row, then per metric row `<tr class="metric">` (label cell + one cell per column) and, when selected, the inline `<tr class="chartrow">` (empty `.lbl` cell + `<td :colspan="columns.length">` with the `TrendChart`). The dashboard renders `<thead>` once (A4/A6) and the `<tbody v-for>` sections.

### A4. Clickable commit hash (#4, link half only)
- Header column cell content becomes an `<a>`:
  - single-commit column → `commitUrl(sha)` = `${REPO}/commit/${sha}`.
  - collapsed run `a..b` → `compareUrl(parentOfFirst, lastSha)` = `${REPO}/compare/${parentOfFirst}...${lastSha}` (parent comes from `index[firstSha].parent`; if absent, fall back to `commitUrl(lastSha)`).
- `title` attr = a free mini-tooltip `"{rev8} · {branch} · {yyyy-mm-dd}"` (branch/date from the index).
- New `lib/repo.ts`: `REPO_URL`, `commitUrl(sha)`, `compareUrl(from, to)`. Per-column href/sha/title computed in the dashboard's `columns` computed (it has `snapshots` + `index`).
- **Message-on-hover is Phase B** (needs stored `subject`).

### A5. Dim unchanged / brighten changed columns (#5)
- Extract `boundaryFlags(rows, n): boolean[]` from `collapse.ts` (the boundary predicate `columnGroups` already computes: index `i` is a boundary iff some row's present value at `i` differs from its previous present value; index 0 is a boundary). `columnGroups` is refactored to consume `boundaryFlags` (DRY, no behavior change).
- The dashboard computes a per-**column** `changed: boolean[]`:
  - collapse **off** → `changed = boundaryFlags(allRows, snapshots.length)` (one column per commit).
  - collapse **on** → all `true` (every collapsed column is a boundary by construction; nothing to dim).
- Cells and header hash of unchanged columns get `class="dim"` → `opacity: .45`. Only visibly affects the expanded (collapse-off) view, which is the point.

### A6. Per-column net arrow (#6)
- New pure `columnNet(rows, groups): { up: number; down: number }[]` in `series.ts`: for each column, over all rows, collapse values (`collapseValues`) and count `trend(collapsed, colIdx, lowerIsBetter(key))` results (`'up'`/`'down'`). Domain-aware. Column 0 has no previous present value → all neutral.
- A **second sticky header row** ("Δ overall" in the label cell) shows per column: green `▲` if `up > down`, red `▼` if `down > up`, neutral `–` if tie/none. `title` = `"{up} improved, {down} regressed"`. Lives in `<thead>` so it sticks with the commit-hash row.

## Architecture / boundaries
Logic stays in pure, tested `lib/*.ts`; components stay thin.
- New pure: `lib/repo.ts` (`commitUrl`/`compareUrl`), `collapse.ts` `boundaryFlags`, `series.ts` `columnNet`.
- Components: `MetricsDashboard.vue` (owns `<table>`, `<thead>` with hash-links + net-arrow row, scroll region + `--head-h` measure, hover-aware legend); `MetricSection.vue` (renamed from `MetricTable.vue`, `<tbody>` root, sticky caption, dim class, delta display); `TrendChart.vue` (drop tip/tooltip/snapshots).
- Rename `components/MetricTable.test.ts` → `components/MetricSection.test.ts`.

## Testing
- Pure vitest: `boundaryFlags` (all-equal / any-change / gaps / first-col true); `columnNet` (domain-aware up/down counts, ties→neutral, first column neutral, gaps skipped); `repo` (`commitUrl`/`compareUrl` formats + parent fallback).
- Component mount: one `<table>` with N `<tbody>`; sticky classes present (thead, caption, lbl); header cells are `<a>` with the correct href (single vs compare); the "Δ overall" row renders the expected arrow per column; `dim` class on unchanged columns only when collapse is off; legend shows a value + column label on `sharedHover` and none at rest; `TrendChart` renders no `.tip`.
- Manual/local: `docs:dev` against the local store — sticky header + captions while scrolling a tall grid; horizontal scroll keeps the metric column pinned; hash links open the right GitHub pages; dim/arrows read correctly with collapse on and off; `docs:build` green.

## Phase B (deferred — recorded, not built now)
- **#4 message tooltip + #7 PR grouping/link**, both via build-time enrichment:
  - Add commit `subject` (`git log -1 --format=%s`) — and optionally `pr` (via `gh` in CI) — to each `index.json` entry. `metrics-probe` (Rust) writes `subject` at record time; `scripts/ci-backfill.sh` fills it for historical commits still in the repo. `IndexEntry` gains `subject?: string` (and optionally `pr?: number`).
  - #4: header hash hover shows the `subject`.
  - #7: group columns by the `branch` already stored (≈ one PR per feature branch); render a PR/branch header row above the columns spanning them via `colspan`, with a thicker separator between groups; the label links to the PR (`${REPO}/pull/${pr}`) when `pr` is known, else the branch (`${REPO}/commits/${branch}`).

## Bookkeeping
Add **WS19.8** to the roadmap (metrics UI v3 — unified table + column interactions), Phase A marked done with commit hashes; Phase B logged as a follow-up.

## Risks
- **Bounded scroll region height (`80vh`)** is a heuristic; may need tuning for short viewports / mobile. Isolated to one CSS value.
- **Sticky caption offset** depends on the measured `--head-h`; the `ResizeObserver` must be set before first paint of a tall grid (charts/sections appear post-hydration, so this is safe, same as WS19.7's TrendChart width measure).
- **Multiple `<tbody>` + sticky first-row handoff** relies on modern-browser sticky behavior (VitePress targets evergreen). Acceptable.
- **Stacked branch:** rebase `ws19-metrics-v3` onto `master` after #13 merges (same pattern as v2→v3 here).
