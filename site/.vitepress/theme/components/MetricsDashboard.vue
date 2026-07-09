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

// Registered synchronously (top-level, before any await) so it binds to this
// component's effect scope and is auto-disposed on unmount. `headEl` starts
// out null while `loading` is true (the <thead> is behind v-else), so a
// one-shot attach in onMounted would miss it; watching the ref re-attaches
// once the thead actually mounts after the fetch resolves.
watch(headEl, (el) => {
  ro?.disconnect()
  ro = null
  if (el && typeof ResizeObserver !== 'undefined') {
    ro = new ResizeObserver(measureHead)
    ro.observe(el)
  }
}, { immediate: true })

onMounted(async () => {
  parseHash()
  // Registered synchronously (before any await) so it binds to this component's
  // effect scope and is auto-disposed on unmount. It only depends on UI state,
  // not fetched data, so it doesn't need to wait for the fetch below.
  watch([selected, collapse, delta, onlyChanged], writeHash, { deep: true })
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

// The shared, collapsed column axis (identity when collapse is off).
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

// Optionally drop groups whose rows are all flat across the (collapsed) range.
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
// Overlay series use the collapsed values so the sidepanel matches the tables.
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

<style scoped lang="scss">
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
