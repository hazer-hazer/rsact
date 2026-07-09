<script setup lang="ts">
import { reactive, computed, ref, onMounted, provide, watch } from 'vue'
import { withBase } from 'vitepress'
import MetricTable from './MetricTable.vue'
import TrendChart from './TrendChart.vue'
import { buildSeries, isFlat } from '../lib/series'
import { columnGroups, columnLabel, collapseValues } from '../lib/collapse'
import { colorFor } from '../lib/colors'
import { HOVER_KEY } from '../lib/hover'
import { SAMPLE } from '../lib/sample'
import type { MetricsData, Snapshot, IndexMap, SeriesRow, Series } from '../lib/types'

const props = defineProps<{ data?: MetricsData }>()

const snapshots = ref<Snapshot[]>(props.data?.snapshots ?? [])
const index = ref<IndexMap>(props.data?.index ?? {})
const loading = ref(!props.data)

// UI state (also serialized to the URL hash).
const selected = reactive(new Set<string>())
const collapse = ref(true)
const delta = ref(false)
const onlyChanged = ref(false)

// Synchronized crosshair column, provided to every table + chart.
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

const groups = computed(() => buildSeries(snapshots.value))
const allRows = computed(() => groups.value.flatMap((g) => g.rows))

// The shared, collapsed column axis (identity when collapse is off).
const colGroups = computed<number[][]>(() =>
  collapse.value
    ? columnGroups(allRows.value, snapshots.value.length)
    : snapshots.value.map((_, i) => [i]),
)
const columns = computed(() =>
  colGroups.value.map((g) => {
    const label = columnLabel(snapshots.value, g)
    const branch = index.value[snapshots.value[g[0]]?.git_rev]?.branch
    return { label, title: branch ? `${label} (${branch})` : label, group: g }
  }),
)

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
        <code>a..b</code> columns; bench medians are a ±noisy CI trend.
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
        <div class="main">
          <MetricTable
            v-for="g in shownGroups"
            :key="g.title"
            :group="g"
            :columns="columns"
            :selected="selected"
            :delta="delta"
            @toggle="toggle"
          />
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
          />
          <p v-else class="muted">select metric rows to overlay their trends</p>
          <div v-if="selected.size" class="legend">
            <div v-for="s in selectedSeries" :key="s.label" class="legend-item">
              <span class="swatch" :style="{ background: s.color }"></span>{{ s.label }}
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
.main { flex: 1 1 auto; min-width: 0; overflow-x: auto; }
.side { flex: 0 0 380px; position: sticky; top: 5rem; }
h2 { font-size: 0.95rem; margin: 0 0 0.4rem; border: 0; padding: 0; }
@media (max-width: 900px) {
  .wrap { flex-direction: column; }
  .side { position: static; flex-basis: auto; width: 100%; }
}
.swatch { display: inline-block; width: 0.6rem; height: 0.6rem; border-radius: 2px; margin-right: 0.35rem; vertical-align: middle; }
.legend { margin-top: 0.5rem; }
.legend-item { display: flex; align-items: center; margin: 0.1rem 0; }
</style>
