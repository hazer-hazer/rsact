# Metrics UI v3 (Phase A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge the site's per-group metric tables into one aligned grid with a sticky header/captions, clickable commit hashes, per-column change dimming, and a per-column net-effect arrow.

**Architecture:** One `<table>` in a bounded scroll region (`MetricsDashboard`); each group is a `<tbody>` section component (`MetricSection`, renamed from `MetricTable`). Change-detection/column helpers stay pure in `lib/` and are unit-tested; components stay thin. `TrendChart`'s hover tip is removed and folded into a hover-aware legend in the dashboard.

**Tech Stack:** VitePress 1.6.4, Vue 3.5 (`<script setup lang="ts">`), SCSS, Vitest + @vue/test-utils.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-09-metrics-ui-v3-design.md`. Design decisions there are binding.
- All work is under `site/`. Run tests from `site/` with `npm test` (`vitest run --passWithNoTests`); build with `npm run docs:build`. NEVER background a command — foreground with an extended timeout.
- TypeScript over JS, SCSS over CSS (maintainer standing preference).
- Pure logic lives in `site/.vitepress/theme/lib/*.ts` and is unit-tested; Vue components stay thin.
- `columnNet` and `boundaryFlags` live in `collapse.ts` (NOT `series.ts`) — `collapse.ts` already imports from `series.ts`; the reverse would be a circular import.
- Repo base URL constant must match `.vitepress/config.mts` socialLinks: `https://github.com/hazer-hazer/rsact`.
- Phase A only. Do NOT add `subject`/`pr` to the store, do NOT implement the commit-message tooltip (#4) or PR grouping (#7) — those are Phase B.
- Preserve all `Note:`/`TODO:` comments.

---

### Task 1: `lib/repo.ts` — GitHub URL helpers

**Files:**
- Create: `site/.vitepress/theme/lib/repo.ts`
- Test: `site/.vitepress/theme/lib/repo.test.ts`

**Interfaces:**
- Produces: `REPO_URL: string`, `commitUrl(sha: string): string`, `compareUrl(from: string, to: string): string`.

- [ ] **Step 1: Write the failing test** — `site/.vitepress/theme/lib/repo.test.ts`

```ts
import { describe, it, expect } from 'vitest'
import { REPO_URL, commitUrl, compareUrl } from './repo'

describe('repo urls', () => {
  it('REPO_URL matches the config socialLinks repo', () => {
    expect(REPO_URL).toBe('https://github.com/hazer-hazer/rsact')
  })
  it('commitUrl points at the commit page', () => {
    expect(commitUrl('abc123')).toBe('https://github.com/hazer-hazer/rsact/commit/abc123')
  })
  it('compareUrl uses the triple-dot range', () => {
    expect(compareUrl('par', 'last')).toBe('https://github.com/hazer-hazer/rsact/compare/par...last')
  })
})
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd site && npx vitest run .vitepress/theme/lib/repo.test.ts`
Expected: FAIL — cannot find module `./repo`.

- [ ] **Step 3: Implement** — `site/.vitepress/theme/lib/repo.ts`

```ts
// GitHub repo base. KEEP IN SYNC with .vitepress/config.mts socialLinks.
export const REPO_URL = 'https://github.com/hazer-hazer/rsact'

// Link to a single commit's page.
export function commitUrl(sha: string): string {
  return `${REPO_URL}/commit/${sha}`
}

// Link to the diff across a range. `from` is typically the parent of a collapsed
// run's first commit, so the compare shows every commit in the run.
export function compareUrl(from: string, to: string): string {
  return `${REPO_URL}/compare/${from}...${to}`
}
```

- [ ] **Step 4: Run it, verify it passes**

Run: `cd site && npx vitest run .vitepress/theme/lib/repo.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add site/.vitepress/theme/lib/repo.ts site/.vitepress/theme/lib/repo.test.ts
git commit -m "WS19.8: lib/repo.ts — GitHub commit/compare URL helpers"
```

---

### Task 2: `collapse.ts` — extract `boundaryFlags`

**Files:**
- Modify: `site/.vitepress/theme/lib/collapse.ts`
- Test: `site/.vitepress/theme/lib/collapse.test.ts` (add cases)

**Interfaces:**
- Consumes: `prevPresent` (from `series.ts`), `SeriesRow` type.
- Produces: `boundaryFlags(rows: SeriesRow[], n: number): boolean[]` — `flags[0] === true`; `flags[i]` (i>0) true iff some row's present value at `i` differs from its previous present value. `columnGroups` refactored to consume it (behavior unchanged).

- [ ] **Step 1: Write the failing test** — append to `site/.vitepress/theme/lib/collapse.test.ts`

```ts
import { boundaryFlags } from './collapse'

describe('boundaryFlags', () => {
  const rows = (values: (number | null)[]) => [{ key: 'k', label: 'k', values }]
  it('index 0 is always a boundary (baseline), unchanged runs are not', () => {
    expect(boundaryFlags(rows([10, 10, 10]), 3)).toEqual([true, false, false])
  })
  it('a real change flips the flag at that column', () => {
    expect(boundaryFlags(rows([10, 10, 20]), 3)).toEqual([true, false, true])
  })
  it('nulls never flag; a change across a gap flags at the reappearance', () => {
    expect(boundaryFlags(rows([10, null, 20]), 3)).toEqual([true, false, true])
    expect(boundaryFlags(rows([10, 10, null, 10]), 4)).toEqual([true, false, false, false])
  })
})
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd site && npx vitest run .vitepress/theme/lib/collapse.test.ts`
Expected: FAIL — `boundaryFlags` is not exported.

- [ ] **Step 3: Implement** — replace the `columnGroups` function in `site/.vitepress/theme/lib/collapse.ts` with:

```ts
// Per-commit boundary flags. flags[0] is always true (the baseline starts the
// first run). flags[i] (i>0) is true iff, for some row, its PRESENT value at i
// differs from that row's previous present value (gaps skipped). A null is never
// a boundary — it carries forward. Drives both column collapsing and the
// "changed column" dimming (#5).
export function boundaryFlags(rows: SeriesRow[], n: number): boolean[] {
  const flags: boolean[] = []
  for (let i = 0; i < n; i++) {
    flags.push(
      i === 0 ||
        rows.some((r) => {
          const v = r.values[i]
          if (v === null || v === undefined) return false
          const prev = prevPresent(r.values, i)
          return prev !== null && v !== prev
        }),
    )
  }
  return flags
}

// Maximal runs of commit indices between boundaries. Preserves a real change
// across a gap (e.g. [10, null, 20] splits at 20) while absorbing measurement
// gaps.
export function columnGroups(rows: SeriesRow[], n: number): number[][] {
  const flags = boundaryFlags(rows, n)
  const groups: number[][] = []
  let cur: number[] = []
  for (let i = 0; i < n; i++) {
    if (i > 0 && flags[i]) {
      groups.push(cur)
      cur = []
    }
    cur.push(i)
  }
  if (cur.length) groups.push(cur)
  return groups
}
```

- [ ] **Step 4: Run it, verify it passes** (both new and existing collapse tests)

Run: `cd site && npx vitest run .vitepress/theme/lib/collapse.test.ts`
Expected: PASS (existing 8 + 3 new).

- [ ] **Step 5: Commit**

```bash
git add site/.vitepress/theme/lib/collapse.ts site/.vitepress/theme/lib/collapse.test.ts
git commit -m "WS19.8: collapse.ts — extract boundaryFlags (DRY with columnGroups)"
```

---

### Task 3: `collapse.ts` — `columnNet` (per-column net effect)

**Files:**
- Modify: `site/.vitepress/theme/lib/collapse.ts`
- Test: `site/.vitepress/theme/lib/collapse.test.ts` (add cases)

**Interfaces:**
- Consumes: `trend`, `lowerIsBetter` (from `series.ts`), local `collapseValues`.
- Produces: `interface ColNet { up: number; down: number }`; `columnNet(rows: SeriesRow[], groups: number[][]): ColNet[]` — per collapsed column, counts of improved/regressed metrics vs the previous present column (domain-aware; column 0 neutral; gaps skipped).

- [ ] **Step 1: Write the failing test** — append to `site/.vitepress/theme/lib/collapse.test.ts`

```ts
import { columnNet, columnGroups } from './collapse'

describe('columnNet', () => {
  it('counts improvements (lower-is-better) and regressions per column; col 0 neutral', () => {
    // groups = one column per commit (no collapse)
    const groups = [[0], [1], [2]]
    const rows = [
      { key: 'a', label: 'a', values: [10, 8, 8] },   // improves at col1 (10->8)
      { key: 'b', label: 'b', values: [5, 9, 9] },    // regresses at col1 (5->9)
      { key: 'c', label: 'c', values: [1, 1, 2] },    // regresses at col2 (1->2)
    ]
    expect(columnNet(rows, groups)).toEqual([
      { up: 0, down: 0 }, // col0 baseline
      { up: 1, down: 1 }, // a up, b down
      { up: 0, down: 1 }, // c down
    ])
  })
  it('operates on collapsed values so it matches the displayed columns', () => {
    const rows = [{ key: 'a', label: 'a', values: [10, 10, 20] }]
    const groups = columnGroups(rows, 3) // [[0,1],[2]]
    expect(columnNet(rows, groups)).toEqual([{ up: 0, down: 0 }, { up: 1, down: 0 }])
  })
})
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd site && npx vitest run .vitepress/theme/lib/collapse.test.ts`
Expected: FAIL — `columnNet` is not exported.

- [ ] **Step 3: Implement** — in `site/.vitepress/theme/lib/collapse.ts`: extend the import from `./series` and append the function.

Change the top import line to:

```ts
import { prevPresent, trend, lowerIsBetter } from './series'
```

Append at the end of the file:

```ts
export interface ColNet { up: number; down: number }

// Per collapsed column: how many metrics improved vs regressed relative to their
// previous present column value (domain-aware via lowerIsBetter). Column 0 has no
// predecessor → all neutral. Feeds the "Δ overall" header row (#6). Collapses
// each row once, then scans columns — O(rows·cols).
export function columnNet(rows: SeriesRow[], groups: number[][]): ColNet[] {
  const nets: ColNet[] = groups.map(() => ({ up: 0, down: 0 }))
  for (const r of rows) {
    const collapsed = collapseValues(r.values, groups)
    const low = lowerIsBetter(r.key)
    for (let c = 0; c < groups.length; c++) {
      const t = trend(collapsed, c, low)
      if (t === 'up') nets[c].up++
      else if (t === 'down') nets[c].down++
    }
  }
  return nets
}
```

- [ ] **Step 4: Run it, verify it passes**

Run: `cd site && npx vitest run .vitepress/theme/lib/collapse.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add site/.vitepress/theme/lib/collapse.ts site/.vitepress/theme/lib/collapse.test.ts
git commit -m "WS19.8: collapse.ts — columnNet per-column net effect (domain-aware)"
```

---

### Task 4: `TrendChart.vue` — remove the internal tip (fold into legend, #1)

**Files:**
- Modify: `site/.vitepress/theme/components/TrendChart.vue`

**Interfaces:**
- Produces: `TrendChart` no longer renders a `.tip`, no longer accepts `snapshots`, no longer imports `revLabel`/`fmt`. Keeps crosshair (`hover`/`sharedHover`/guide) and the `interactive` write-gate. `onMove` still sets `sharedHover`.

- [ ] **Step 1: Edit the script** — in `site/.vitepress/theme/components/TrendChart.vue`:
  - Change the import `import { fmt, revLabel } from '../lib/series'` → delete it entirely (no longer used).
  - Remove the `snapshots?: Snapshot[]` prop from the `defineProps` type and its `snapshots: () => []` default; remove the now-unused `Snapshot` from the `types` import (keep `Series`).
  - Delete the entire `const tooltip = computed(...)` block.
  - Keep `hover`, `sharedHover`, `guideCol`, `onMove`, `onLeave`, `hoverX`, `lines`, `measuredW`, the ResizeObserver.

- [ ] **Step 2: Edit the template** — remove the tip block entirely:

```html
    <div v-if="interactive && tooltip" class="tip">
      <div class="tip-title">{{ tooltip.title }}</div>
      <div v-for="r in tooltip.rows" :key="r.label" class="tip-row">
        <span class="swatch" :style="{ background: r.color }"></span>{{ r.label }}:
        {{ r.v === null ? '–' : r.v }}
      </div>
    </div>
```

Delete those lines (the `<svg>…</svg>` stays). The root `<div class="trendchart">` now wraps only the `<svg>`.

- [ ] **Step 3: Edit the style** — remove the now-dead rules: `.tip`, `.tip-title`, `.tip-row`, and the `.swatch` rule (swatch was only used by the tip; the legend swatch lives in `MetricsDashboard`). Keep `.chart`, `.axis`, `.series-line`, `.guide`.

- [ ] **Step 4: Update the existing component test** — `site/.vitepress/theme/components/TrendChart.test.ts` still mounts with `series` + shared hover and asserts the guide line; it does not use `snapshots` or the tip, so it stays valid. Add one assertion that the tip is gone:

```ts
  it('renders no internal tooltip (folded into the dashboard legend)', () => {
    const w = mount(TrendChart, {
      props: { series: [{ label: 'x', values: [1, 2, 3], color: '#000' }], interactive: true },
    })
    expect(w.find('.tip').exists()).toBe(false)
  })
```

- [ ] **Step 5: Run tests + typecheck via build**

Run: `cd site && npx vitest run .vitepress/theme/components/TrendChart.test.ts && npm run docs:build`
Expected: PASS; build green (no unused-import / type errors).

- [ ] **Step 6: Commit**

```bash
git add site/.vitepress/theme/components/TrendChart.vue site/.vitepress/theme/components/TrendChart.test.ts
git commit -m "WS19.8: TrendChart — drop internal hover tip (folds into dashboard legend)"
```

---

### Task 5: `MetricSection.vue` — rename `MetricTable`, render a `<tbody>` section

**Files:**
- Rename: `site/.vitepress/theme/components/MetricTable.vue` → `site/.vitepress/theme/components/MetricSection.vue`
- Rename: `site/.vitepress/theme/components/MetricTable.test.ts` → `site/.vitepress/theme/components/MetricSection.test.ts`

**Interfaces:**
- Consumes: `TrendChart`, `trend`/`fmt`/`lowerIsBetter`/`deltaValues` (series), `collapseValues` (collapse), `colorFor` (colors), `HOVER_KEY` (hover).
- Produces: default component whose ROOT element is `<tbody>`. Props: `{ group: SeriesGroup; columns: Column[]; selected: Set<string>; delta: boolean; changed: boolean[] }` where `interface Column { label: string; title: string; href: string; group: number[] }`. Emits `toggle: [key: string]`. Renders a sticky caption row spanning `1 + columns.length`, then metric rows and (when selected) inline chart rows. Unchanged columns (`!changed[i]`) get a `dim` class.

- [ ] **Step 1: Do the rename**

```bash
git mv site/.vitepress/theme/components/MetricTable.vue site/.vitepress/theme/components/MetricSection.vue
git mv site/.vitepress/theme/components/MetricTable.test.ts site/.vitepress/theme/components/MetricSection.test.ts
```

- [ ] **Step 2: Write the failing test** — replace `site/.vitepress/theme/components/MetricSection.test.ts` with:

```ts
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import MetricSection from './MetricSection.vue'

const columns = [
  { label: 'aaaaaaaa', title: 'aaaaaaaa', href: '#a', group: [0] },
  { label: 'bbbbbbbb', title: 'bbbbbbbb', href: '#b', group: [1] },
  { label: 'cccccccc', title: 'cccccccc', href: '#c', group: [2] },
]
const group = {
  title: 'reactive_only_16',
  rows: [
    { key: 'reactive_only_16/signals', label: 'signals', values: [16, 16, 18] },
    { key: 'reactive_only_16/observers', label: 'observers', values: [17, 17, 17] },
  ],
}
const mountInTable = (props: Record<string, unknown>) =>
  mount(MetricSection, {
    props,
    // a <tbody> component must live inside a <table>
    attachTo: document.createElement('table'),
  })

describe('MetricSection', () => {
  const base = { group, columns, selected: new Set<string>(), delta: false, changed: [true, false, true] }
  it('renders a <tbody> root with a full-width caption row', () => {
    const w = mountInTable(base)
    expect(w.element.tagName).toBe('TBODY')
    const cap = w.find('tr.section-head th')
    expect(cap.exists()).toBe(true)
    expect(cap.attributes('colspan')).toBe(String(1 + columns.length))
    expect(cap.text()).toContain('reactive_only_16')
  })
  it('marks unchanged columns with the dim class', () => {
    const w = mountInTable(base)
    const firstRowCells = w.findAll('tr.metric')[0].findAll('td:not(.lbl)')
    expect(firstRowCells[0].classes()).not.toContain('dim') // changed[0] = true
    expect(firstRowCells[1].classes()).toContain('dim')     // changed[1] = false
  })
  it('shows an inline chart row only for selected metrics', () => {
    const w = mountInTable({ ...base, selected: new Set(['reactive_only_16/signals']) })
    expect(w.findAll('tr.chartrow').length).toBe(1)
  })
  it('emits toggle with the row key on click', async () => {
    const w = mountInTable(base)
    await w.findAll('tr.metric')[0].trigger('click')
    expect(w.emitted('toggle')?.[0]).toEqual(['reactive_only_16/signals'])
  })
})
```

- [ ] **Step 3: Run it, verify it fails**

Run: `cd site && npx vitest run .vitepress/theme/components/MetricSection.test.ts`
Expected: FAIL — component still has a `<table>` root / old props.

- [ ] **Step 4: Rewrite** `site/.vitepress/theme/components/MetricSection.vue`:

```vue
<script setup lang="ts">
import { computed, inject, ref } from 'vue'
import TrendChart from './TrendChart.vue'
import { trend, fmt, lowerIsBetter, deltaValues } from '../lib/series'
import { collapseValues } from '../lib/collapse'
import { colorFor } from '../lib/colors'
import { HOVER_KEY } from '../lib/hover'
import type { SeriesGroup } from '../lib/types'

// A "column" is a collapsed group of commit indices with a display label + link.
interface Column { label: string; title: string; href: string; group: number[] }

const props = defineProps<{
  group: SeriesGroup
  columns: Column[]
  selected: Set<string>
  delta: boolean
  changed: boolean[]
}>()
defineEmits<{ toggle: [key: string] }>()

const sharedHover = inject(HOVER_KEY, ref<number | null>(null))

const rows = computed(() =>
  props.group.rows.map((r) => {
    const collapsed = collapseValues(r.values, props.columns.map((c) => c.group))
    const shown = props.delta ? deltaValues(collapsed) : collapsed
    return {
      ...r,
      shown,
      cells: shown.map((v, i) => ({ v, mark: trend(collapsed, i, lowerIsBetter(r.key)) })),
    }
  }),
)
</script>

<template>
  <tbody>
    <tr class="section-head">
      <th class="section-h" :colspan="1 + columns.length">
        <span class="section-h-inner">{{ group.title }}</span>
      </th>
    </tr>
    <template v-for="row in rows" :key="row.key">
      <tr class="metric" :class="{ sel: selected.has(row.key) }" @click="$emit('toggle', row.key)">
        <td class="lbl">
          <span
            class="swatch"
            :style="{
              background: colorFor(row.key),
              visibility: selected.has(row.key) ? 'visible' : 'hidden',
            }"
          ></span>
          {{ row.label }}
        </td>
        <td
          v-for="(cell, i) in row.cells"
          :key="i"
          :class="{ hov: sharedHover === i, dim: !changed[i] }"
          @mouseenter="sharedHover = i"
          @mouseleave="sharedHover = null"
        >
          <span v-if="cell.v === null" class="muted">–</span>
          <template v-else
            >{{ delta && cell.v > 0 ? '+' : '' }}{{ fmt(cell.v) }}<span
              v-if="cell.mark"
              :class="cell.mark"
              >{{ cell.mark === 'up' ? ' ▲' : ' ▼' }}</span
            ></template
          >
        </td>
      </tr>
      <tr v-if="selected.has(row.key)" class="chartrow">
        <td class="lbl"></td>
        <td :colspan="columns.length">
          <TrendChart
            :series="[{ label: row.label, values: row.shown, color: colorFor(row.key) }]"
            :n="columns.length"
            :height="38"
            :show-dots="true"
          />
        </td>
      </tr>
    </template>
  </tbody>
</template>

<style scoped lang="scss">
// Sticky group caption: sticks just under the (also sticky) header. --head-h is
// set by MetricsDashboard from the measured thead height.
tr.section-head th.section-h {
  position: sticky; top: var(--head-h, 3.4rem); z-index: 2;
  text-align: left; font-weight: bold; background: var(--vp-c-bg);
  border-bottom: 1px solid var(--vp-c-divider); padding: 0.5rem 0.5rem 0.25rem;
}
// keep the caption text visible when the grid is scrolled horizontally
.section-h-inner { position: sticky; left: 0.5rem; }

td {
  border-bottom: 1px solid var(--vp-c-divider);
  border-right: 1px solid var(--vp-c-divider);
  padding: 0.15rem 0.5rem; text-align: right; white-space: nowrap;
  width: 5.5rem; overflow: hidden; text-overflow: ellipsis;
}
td.lbl {
  text-align: left; position: sticky; left: 0; z-index: 1;
  width: var(--metric-col-w, 13rem); min-width: var(--metric-col-w, 13rem);
  background: var(--vp-c-bg);
}
td.hov { background: var(--vp-c-bg-soft); }
td.dim { opacity: 0.4; }
tr.metric { cursor: pointer; }
tr.metric:hover td { background: var(--vp-c-bg-soft); }
tr.metric.sel td.lbl { font-weight: bold; }
tr.chartrow td { padding: 0.2rem 0; }
tr.chartrow td.lbl { background: var(--vp-c-bg); }
.muted { color: var(--vp-c-text-3); }
.up { color: #2e9e4f; }
.down { color: #d64545; }
.swatch {
  display: inline-block; width: 0.6rem; height: 0.6rem;
  border-radius: 2px; margin-right: 0.35rem; vertical-align: middle;
}
</style>
```

- [ ] **Step 5: Run it, verify it passes**

Run: `cd site && npx vitest run .vitepress/theme/components/MetricSection.test.ts`
Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add -A site/.vitepress/theme/components/
git commit -m "WS19.8: MetricSection — <tbody> section with sticky caption + dim + link-ready columns"
```

---

### Task 6: `MetricsDashboard.vue` — unified table, sticky thead, net-arrow row, hover legend

**Files:**
- Modify: `site/.vitepress/theme/components/MetricsDashboard.vue`
- Test: `site/.vitepress/theme/components/MetricsDashboard.test.ts` (update for the new structure)

**Interfaces:**
- Consumes: `MetricSection` (Task 5), `TrendChart`, `columnGroups`/`columnLabel`/`collapseValues`/`boundaryFlags`/`columnNet` (collapse), `buildSeries`/`isFlat`/`fmt` (series), `colorFor` (colors), `commitUrl`/`compareUrl` (repo), `HOVER_KEY`.
- Produces: renders ONE `<table class="grid">` inside `.grid-scroll`; `<thead>` with a commit-hash-link row + a "Δ overall" arrow row; `<tbody>` per group via `MetricSection`. Sets `--head-h` from the measured thead. Legend shows per-series value + column label on hover.

- [ ] **Step 1: Update the test** — replace `site/.vitepress/theme/components/MetricsDashboard.test.ts` with (fetch is stubbed; assert structure with injected `data`):

```ts
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import MetricsDashboard from './MetricsDashboard.vue'
import type { MetricsData } from '../lib/types'

vi.mock('vitepress', () => ({ withBase: (p: string) => p }))

const DATA: MetricsData = {
  index: {
    aaa: { date: 1_700_000_000, parent: 'par', branch: 'main' },
    bbb: { date: 1_700_100_000, parent: 'aaa', branch: 'main' },
  },
  snapshots: [
    { git_rev: 'aaaaaaaa11', git_dirty: false, scenarios: [
      { name: 's1', counts: { signals: 10, total: 10 }, heap_live_bytes: null, heap_peak_bytes: null, build_allocs: null, change_frame_allocs: null, layout: null } ] },
    { git_rev: 'bbbbbbbb22', git_dirty: false, scenarios: [
      { name: 's1', counts: { signals: 12, total: 12 }, heap_live_bytes: null, heap_peak_bytes: null, build_allocs: null, change_frame_allocs: null, layout: null } ] },
  ],
}

describe('MetricsDashboard', () => {
  beforeEach(() => {
    // @ts-expect-error test env
    global.ResizeObserver = class { observe() {} disconnect() {} }
  })
  it('renders a single grid table with a thead and per-group tbodies', async () => {
    const w = mount(MetricsDashboard, { props: { data: DATA } })
    await flushPromises()
    expect(w.findAll('table.grid').length).toBe(1)
    expect(w.findAll('table.grid > thead').length).toBe(1)
    expect(w.findAll('table.grid > tbody').length).toBeGreaterThanOrEqual(1)
  })
  it('commit header cells are links to GitHub', async () => {
    const w = mount(MetricsDashboard, { props: { data: DATA } })
    await flushPromises()
    const links = w.findAll('thead tr.cols th.col a')
    expect(links.length).toBe(2)
    expect(links[0].attributes('href')).toContain('github.com/hazer-hazer/rsact')
  })
  it('renders a "Δ overall" row with one cell per column', async () => {
    const w = mount(MetricsDashboard, { props: { data: DATA } })
    await flushPromises()
    const overall = w.find('thead tr.overall')
    expect(overall.exists()).toBe(true)
    expect(overall.findAll('th.col').length).toBe(2)
  })
})
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd site && npx vitest run .vitepress/theme/components/MetricsDashboard.test.ts`
Expected: FAIL — no `table.grid` yet.

- [ ] **Step 3: Rewrite the `<script setup>`** of `site/.vitepress/theme/components/MetricsDashboard.vue`. Keep the existing data-load / URL-hash / selection logic; add the new imports and computeds below. Full script:

```vue
<script setup lang="ts">
import { reactive, computed, ref, onMounted, onUnmounted, provide, watch } from 'vue'
import { withBase } from 'vitepress'
import MetricSection from './MetricSection.vue'
import TrendChart from './TrendChart.vue'
import { buildSeries, isFlat, fmt } from '../lib/series'
import { columnGroups, columnLabel, collapseValues, boundaryFlags, columnNet } from '../lib/collapse'
import { colorFor } from '../lib/colors'
import { commitUrl, compareUrl } from '../lib/repo'
import { HOVER_KEY } from '../lib/hover'
import { SAMPLE } from '../lib/sample'
import type { MetricsData, Snapshot, IndexMap, SeriesRow, Series } from '../lib/types'

const props = defineProps<{ data?: MetricsData }>()

const snapshots = ref<Snapshot[]>(props.data?.snapshots ?? [])
const index = ref<IndexMap>(props.data?.index ?? {})
const loading = ref(!props.data)

const selected = reactive(new Set<string>())
const collapse = ref(true)
const delta = ref(false)
const onlyChanged = ref(false)

// Synchronized crosshair column, provided to every table + chart, and read by
// the legend below.
const hover = ref<number | null>(null)
provide(HOVER_KEY, hover)

function parseHash() {
  if (typeof location === 'undefined') return
  const p = new URLSearchParams(location.hash.replace(/^#/, ''))
  collapse.value = p.get('collapse') !== '0'
  delta.value = p.get('delta') === '1'
  onlyChanged.value = p.get('changed') === '1'
  const sel = p.get('sel')
  selected.clear()
  if (sel) for (const k of sel.split('~').filter(Boolean)) selected.add(decodeURIComponent(k))
}
function writeHash() {
  if (typeof location === 'undefined') return
  const p = new URLSearchParams()
  if (!collapse.value) p.set('collapse', '0')
  if (delta.value) p.set('delta', '1')
  if (onlyChanged.value) p.set('changed', '1')
  if (selected.size) p.set('sel', [...selected].map(encodeURIComponent).join('~'))
  const hash = p.toString()
  history.replaceState(null, '', hash ? `#${hash}` : location.pathname + location.search)
}

// Measure the (2-row) sticky header so section captions can stick just below it.
const headEl = ref<HTMLElement | null>(null)
const gridEl = ref<HTMLElement | null>(null)
let ro: ResizeObserver | null = null
function measureHead() {
  if (headEl.value && gridEl.value) {
    gridEl.value.style.setProperty('--head-h', `${headEl.value.offsetHeight}px`)
  }
}

onMounted(async () => {
  parseHash()
  watch([selected, collapse, delta, onlyChanged], writeHash, { deep: true })
  if (typeof ResizeObserver !== 'undefined' && headEl.value) {
    ro = new ResizeObserver(measureHead)
    ro.observe(headEl.value)
  }
  if (!props.data) {
    try {
      const res = await fetch(withBase('/metrics/data.json'))
      if (!res.ok) throw new Error(String(res.status))
      const d = (await res.json()) as MetricsData
      if (Array.isArray(d.snapshots)) {
        snapshots.value = d.snapshots
        index.value = d.index ?? {}
      }
    } catch (e) {
      console.error('rsact metrics: failed to load /metrics/data.json', e)
      if (import.meta.env.DEV) {
        snapshots.value = SAMPLE.snapshots
        index.value = SAMPLE.index
      }
    } finally {
      loading.value = false
    }
  }
})
onUnmounted(() => { ro?.disconnect(); ro = null })

const groups = computed(() => buildSeries(snapshots.value))
const allRows = computed(() => groups.value.flatMap((g) => g.rows))

const colGroups = computed<number[][]>(() =>
  collapse.value
    ? columnGroups(allRows.value, snapshots.value.length)
    : snapshots.value.map((_, i) => [i]),
)

const iso = (secs?: number) => (secs ? new Date(secs * 1000).toISOString().slice(0, 10) : '')

// Columns carry a label, a hover title, the GitHub href, and their commit group.
const columns = computed(() =>
  colGroups.value.map((g) => {
    const label = columnLabel(snapshots.value, g)
    const first = snapshots.value[g[0]]
    const last = snapshots.value[g[g.length - 1]]
    const entry = index.value[first?.git_rev]
    const branch = entry?.branch
    const date = iso(entry?.date)
    const href =
      g.length === 1
        ? commitUrl(last.git_rev)
        : entry?.parent
          ? compareUrl(entry.parent, last.git_rev)
          : commitUrl(last.git_rev)
    const title = [label, branch, date].filter(Boolean).join(' · ')
    return { label, title, href, group: g }
  }),
)

// Per-column "changed" flags for dimming (#5). With collapse on every column is a
// boundary → nothing to dim.
const changed = computed<boolean[]>(() =>
  collapse.value
    ? colGroups.value.map(() => true)
    : boundaryFlags(allRows.value, snapshots.value.length),
)

// Per-column net effect for the "Δ overall" row (#6).
const nets = computed(() => columnNet(allRows.value, colGroups.value))

function collapsedRowFlat(r: SeriesRow): boolean {
  return isFlat(collapseValues(r.values, colGroups.value))
}
const shownGroups = computed(() =>
  onlyChanged.value
    ? groups.value
        .map((g) => ({ ...g, rows: g.rows.filter((r) => !collapsedRowFlat(r)) }))
        .filter((g) => g.rows.length)
    : groups.value,
)

const seriesByKey = computed(() => {
  const m = new Map<string, SeriesRow>()
  for (const g of groups.value) for (const r of g.rows) m.set(r.key, r)
  return m
})
function toggle(key: string) {
  if (selected.has(key)) selected.delete(key)
  else selected.add(key)
}
function selectAll() {
  for (const g of shownGroups.value) for (const r of g.rows) selected.add(r.key)
}
const selectedSeries = computed<Series[]>(() =>
  [...selected].map((key) => {
    const r = seriesByKey.value.get(key)
    const collapsed = collapseValues(r?.values ?? [], colGroups.value)
    return { label: r?.label ?? key, values: collapsed, color: colorFor(key) }
  }),
)

// Legend value at the hovered column (folds in the old chart tooltip, #1).
const hoverLabel = computed(() =>
  hover.value !== null && columns.value[hover.value] ? columns.value[hover.value].label : null,
)
function valAt(s: Series): string {
  if (hover.value === null) return ''
  const f = fmt(s.values[hover.value])
  return f === null ? '–' : f
}
</script>
```

- [ ] **Step 4: Rewrite the `<template>`** of `MetricsDashboard.vue`. Keep the intro/controls; replace `.wrap` contents:

```html
<template>
  <div class="metrics">
    <p v-if="loading" class="muted">Loading metrics…</p>
    <p v-else-if="!snapshots.length" class="muted">
      No metrics data available yet. Record a snapshot (<code>metrics-probe record</code>) or push a commit.
    </p>
    <template v-else>
      <p class="muted intro">
        Per-commit trend, oldest → newest. Click a metric row to chart it; charted rows overlay in the
        right panel, each normalized to its own max — hover any column to sync the crosshair everywhere.
        <span class="up">▲</span> improved, <span class="down">▼</span> regressed (domain-aware). Gaps
        mean the metric wasn't measured — never zero. Unchanged commits collapse to
        <code>a..b</code> columns; the <strong>Δ overall</strong> row sums each commit's net effect;
        bench medians are a ±noisy CI trend.
      </p>

      <div class="controls">
        <button @click="selectAll">select all</button>
        <button @click="selected.clear()">clear</button>
        <label><input type="checkbox" v-model="collapse" /> collapse unchanged</label>
        <label><input type="checkbox" v-model="delta" /> Δ from baseline</label>
        <label><input type="checkbox" v-model="onlyChanged" /> only changed</label>
        <span class="muted">{{ selected.size ? `${selected.size} charted` : 'no series selected' }}</span>
      </div>

      <div class="wrap">
        <div class="grid-scroll">
          <table class="grid" ref="gridEl">
            <thead ref="headEl">
              <tr class="cols">
                <th class="lbl">metric</th>
                <th
                  v-for="(c, i) in columns"
                  :key="c.label + i"
                  class="col"
                  :class="{ hov: hover === i, dim: !changed[i] }"
                  :title="c.title"
                  @mouseenter="hover = i"
                  @mouseleave="hover = null"
                >
                  <a :href="c.href" target="_blank" rel="noreferrer">{{ c.label }}</a>
                </th>
              </tr>
              <tr class="overall">
                <th class="lbl">Δ overall</th>
                <th
                  v-for="(net, i) in nets"
                  :key="i"
                  class="col"
                  :class="{ hov: hover === i, dim: !changed[i] }"
                  :title="`${net.up} improved, ${net.down} regressed`"
                  @mouseenter="hover = i"
                  @mouseleave="hover = null"
                >
                  <span v-if="net.up > net.down" class="up">▲</span>
                  <span v-else-if="net.down > net.up" class="down">▼</span>
                  <span v-else class="muted">–</span>
                </th>
              </tr>
            </thead>
            <MetricSection
              v-for="g in shownGroups"
              :key="g.title"
              :group="g"
              :columns="columns"
              :selected="selected"
              :delta="delta"
              :changed="changed"
              @toggle="toggle"
            />
          </table>
        </div>

        <div class="side">
          <h2>trend (selected)</h2>
          <TrendChart
            v-if="selected.size"
            :series="selectedSeries"
            :n="columns.length"
            :normalize="true"
            :interactive="true"
            :height="300"
            :pad-x="24"
            :pad-y="24"
          />
          <p v-else class="muted">select metric rows to overlay their trends</p>
          <div v-if="selected.size" class="legend">
            <p class="legend-head muted">
              {{ hoverLabel ? `at ${hoverLabel}:` : 'hover a column for values' }}
            </p>
            <div v-for="s in selectedSeries" :key="s.label" class="legend-item">
              <span class="swatch" :style="{ background: s.color }"></span>
              <span class="legend-label">{{ s.label }}</span>
              <span class="legend-val">{{ valAt(s) }}</span>
            </div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
```

- [ ] **Step 5: Rewrite the `<style scoped lang="scss">`** of `MetricsDashboard.vue`:

```scss
.metrics { margin-top: 1rem; --metric-col-w: 13rem; }
.muted { color: var(--vp-c-text-3); }
.intro { font-size: 13px; }
.up { color: #2e9e4f; }
.down { color: #d64545; }
.controls { margin: 0.6rem 0 1rem; display: flex; gap: 0.75rem; align-items: center; flex-wrap: wrap; }
button {
  font: inherit; cursor: pointer; border: 1px solid var(--vp-c-divider);
  border-radius: 4px; background: var(--vp-c-bg-soft); padding: 0.15rem 0.5rem;
}
label { font-size: 12px; display: inline-flex; gap: 0.25rem; align-items: center; cursor: pointer; }

.wrap { display: flex; gap: 1.5rem; align-items: flex-start; }

// Bounded scroll region: horizontal AND vertical scrolling happen HERE, so the
// sticky header/first-column/captions stick relative to this box (a plain
// overflow-x wrapper would also scroll vertically and break sticky-top).
.grid-scroll { flex: 1 1 auto; min-width: 0; max-height: 80vh; overflow: auto; }

table.grid {
  border-collapse: separate; border-spacing: 0; table-layout: fixed;
  font-family: var(--vp-font-family-mono); font-size: 12px;
}
// The WHOLE thead sticks vertically as one block (both header rows move
// together — more reliable than per-th top offsets); border-collapse:separate
// is required so sticky cells keep their borders.
thead { position: sticky; top: 0; z-index: 3; }
thead th {
  background: var(--vp-c-bg);
  border-bottom: 1px solid var(--vp-c-divider);
  border-right: 1px solid var(--vp-c-divider);
  padding: 0.2rem 0.5rem; text-align: right; white-space: nowrap;
  width: 5.5rem; overflow: hidden; text-overflow: ellipsis;
}
// first column also sticks horizontally (independent axis); z-index above the
// other header cells so the corner wins where the two sticky regions overlap.
thead th.lbl {
  position: sticky; left: 0; z-index: 4; text-align: left;
  width: var(--metric-col-w); min-width: var(--metric-col-w);
}
thead th.col a { color: var(--vp-c-brand-1); text-decoration: none; }
thead th.col a:hover { text-decoration: underline; }
th.hov { background: var(--vp-c-bg-soft); }
th.dim { opacity: 0.4; }

.side { flex: 0 0 380px; position: sticky; top: 5rem; }
h2 { font-size: 0.95rem; margin: 0 0 0.4rem; border: 0; padding: 0; }
@media (max-width: 900px) {
  .wrap { flex-direction: column; }
  .side { position: static; flex-basis: auto; width: 100%; }
  .grid-scroll { max-height: 70vh; }
}
.swatch { display: inline-block; width: 0.6rem; height: 0.6rem; border-radius: 2px; margin-right: 0.35rem; vertical-align: middle; }
.legend { margin-top: 0.5rem; font-size: 12px; }
.legend-head { margin: 0 0 0.25rem; }
.legend-item { display: flex; align-items: center; gap: 0.35rem; margin: 0.1rem 0; }
.legend-label { flex: 1 1 auto; }
.legend-val { font-family: var(--vp-font-family-mono); color: var(--vp-c-text-1); }
</style>
```

Note: `position: sticky` on `<thead>` keeps both header rows pinned as one block (evergreen-browser supported; VitePress targets those). Sticky visual behavior is NOT unit-testable — the component tests assert structure only, so the sticky header/captions/first-column must be verified in the manual check (Step 8). If a target browser ever fails to stick the `<thead>`, the fallback is per-`th` sticky with the second row offset by the measured row-1 height.

- [ ] **Step 6: Run it, verify it passes**

Run: `cd site && npx vitest run .vitepress/theme/components/MetricsDashboard.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 7: Full suite + build**

Run: `cd site && npm test && npm run docs:build`
Expected: all suites PASS; build green.

- [ ] **Step 8: Manual check (local store)**

Run: `cd site && npm run docs:dev`, open the Metrics page:
- one grid; scroll down → header + section captions stay pinned; scroll right → metric column + captions stay pinned.
- toggle "collapse unchanged" off → unchanged columns dim; the "Δ overall" arrows read green/red sensibly.
- click a metric → inline sparkline; select several → overlay + legend; hover a column → legend shows each series' value under the correct `a..b`/rev8 label (never `#index`).
- click a commit-hash header → opens the right GitHub commit/compare page.

- [ ] **Step 9: Commit**

```bash
git add site/.vitepress/theme/components/MetricsDashboard.vue site/.vitepress/theme/components/MetricsDashboard.test.ts
git commit -m "WS19.8: MetricsDashboard — unified grid, sticky thead + Δ-overall row, commit links, hover legend"
```

---

### Task 7: Theme registration + roadmap bookkeeping

**Files:**
- Check: `site/.vitepress/theme/index.ts` (component registration)
- Modify: `docs/plans/2026-07-05-rsact-evolution-roadmap.md`

- [ ] **Step 1: Fix component registration** — if `theme/index.ts` globally registers `MetricTable`/`MetricsDashboard`, update any `MetricTable` reference to the new name. (Dashboard imports `MetricSection` directly, so only a stale global registration or import would break.) Grep first:

Run: `cd site && grep -rn "MetricTable" .vitepress/`
Expected after fix: no matches except in git history.

- [ ] **Step 2: Build to confirm no dangling references**

Run: `cd site && npm run docs:build`
Expected: green.

- [ ] **Step 3: Roadmap note** — add a WS19.8 line under the WS19 section of `docs/plans/2026-07-05-rsact-evolution-roadmap.md`: Phase A done (unified grid, sticky header/captions, commit links, dim/brighten, Δ-overall row, legend fold) with commit hashes; Phase B (commit-message tooltip #4 + PR grouping #7 via build-time `subject`/`pr` enrichment) logged as follow-up.

- [ ] **Step 4: Commit**

```bash
git add site/.vitepress/theme/index.ts docs/plans/2026-07-05-rsact-evolution-roadmap.md
git commit -m "WS19.8: register MetricSection + roadmap Phase A done / Phase B logged"
```

---

## Self-Review notes (author)
- Spec coverage: #1 (Task 4 + 6 legend), #2 (Task 6 scroll region + Task 5 sticky caption), #3 (Tasks 5+6), #4-link (Tasks 1+6), #5 (Tasks 2+6), #6 (Tasks 3+6). Phase B (#4 msg, #7) intentionally excluded.
- Type consistency: `Column` gains `href` (Tasks 5 & 6 agree); `ColNet` used by Task 3 + Task 6; `changed: boolean[]` prop threaded dashboard→section.
- Deviation from spec: `columnNet`/`boundaryFlags` in `collapse.ts` not `series.ts` (circular-import avoidance) — noted in Global Constraints.
