<script setup lang="ts">
import { reactive, computed, ref, onMounted } from 'vue'
import { withBase } from 'vitepress'
import MetricTable from './MetricTable.vue'
import TrendChart from './TrendChart.vue'
import { buildSeries } from '../lib/series'
import { SAMPLE } from '../lib/sample'
import type { MetricsData, Snapshot, IndexMap, SeriesRow, Series } from '../lib/types'

// When `data` is provided (tests), render it directly. Otherwise fetch the
// assembled data.json on client mount; in dev (no data served) fall back to the
// bundled sample fixture — mirrors the standalone viewer's main.ts.
const props = defineProps<{ data?: MetricsData }>()

const snapshots = ref<Snapshot[]>(props.data?.snapshots ?? [])
const index = ref<IndexMap>(props.data?.index ?? {})
const loading = ref(!props.data)

onMounted(async () => {
  if (props.data) return
  try {
    const res = await fetch(withBase('/metrics/data.json'))
    if (!res.ok) throw new Error(String(res.status))
    const d = (await res.json()) as MetricsData
    if (Array.isArray(d.snapshots)) {
      snapshots.value = d.snapshots
      index.value = d.index ?? {}
    }
  } catch (e) {
    // Log so a broken live data.json is diagnosable in the console rather than
    // failing silently into the empty state. Dev also falls back to the fixture.
    console.error('rsact metrics: failed to load /metrics/data.json', e)
    if (import.meta.env.DEV) {
      snapshots.value = SAMPLE.snapshots
      index.value = SAMPLE.index
    }
  } finally {
    loading.value = false
  }
})

const PALETTE = [
  '#4e79a7', '#f28e2c', '#e15759', '#76b7b2', '#59a14f',
  '#edc949', '#af7aa1', '#ff9da7', '#9c755f', '#bab0ab',
]

const groups = computed(() => buildSeries(snapshots.value))
const seriesByKey = computed(() => {
  const m = new Map<string, SeriesRow>()
  for (const g of groups.value) for (const r of g.rows) m.set(r.key, r)
  return m
})

const selected = reactive(new Map<string, string>())
let cursor = 0
function toggle(key: string) {
  if (selected.has(key)) selected.delete(key)
  else selected.set(key, PALETTE[cursor++ % PALETTE.length])
}
function selectAll() {
  for (const g of groups.value)
    for (const r of g.rows)
      if (!selected.has(r.key)) selected.set(r.key, PALETTE[cursor++ % PALETTE.length])
}
const selectedSeries = computed<Series[]>(() =>
  [...selected.entries()].map(([key, color]) => ({
    label: seriesByKey.value.get(key)?.label ?? key,
    values: seriesByKey.value.get(key)?.values ?? [],
    color,
  })),
)
</script>

<template>
  <div class="metrics">
    <p v-if="loading" class="muted">Loading metrics…</p>
    <p v-else-if="!snapshots.length" class="muted">
      No metrics data available yet. Push a commit to populate the
      <code>metrics-data</code> store.
    </p>
    <template v-else>
      <p class="muted intro">
        Per-commit trend, oldest → newest. Click a metric row to chart it; charted rows overlay in
        the right panel, each normalized to its own max — hover for absolute values.
        <span class="up">▲</span> improved, <span class="down">▼</span> regressed (domain-aware:
        fewer/smaller/faster is better). Gaps mean the metric wasn't measured at that commit —
        never zero. Bench medians are a ±noisy CI-runner trend, informational.
      </p>

      <div class="controls">
        <button @click="selectAll">select all</button>
        <button @click="selected.clear()">clear</button>
        <span class="muted">{{
          selected.size ? `${selected.size} charted` : 'no series selected'
        }}</span>
      </div>

      <div class="wrap">
        <div class="main">
          <MetricTable
            v-for="g in groups"
            :key="g.title"
            :group="g"
            :snapshots="snapshots"
            :index="index"
            :selected="selected"
            @toggle="toggle"
          />
        </div>
        <div class="side">
          <h2>trend (selected)</h2>
          <TrendChart
            v-if="selected.size"
            :series="selectedSeries"
            :snapshots="snapshots"
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
.metrics { margin-top: 1rem; }
.muted { color: var(--vp-c-text-3); }
.intro { font-size: 13px; }
.up { color: #2e9e4f; }
.down { color: #d64545; }
.controls { margin: 0.6rem 0 1rem; }
button {
  font: inherit; margin-right: 0.4rem; cursor: pointer;
  border: 1px solid var(--vp-c-divider); border-radius: 4px;
  background: var(--vp-c-bg-soft); padding: 0.15rem 0.5rem;
}
.wrap { display: flex; gap: 1.5rem; align-items: flex-start; }
.main { flex: 1 1 auto; min-width: 0; overflow-x: auto; }
.side { flex: 0 0 380px; position: sticky; top: 5rem; }
h2 { font-size: 0.95rem; margin: 0 0 0.4rem; border: 0; padding: 0; }
@media (max-width: 900px) {
  .wrap { flex-direction: column; }
  .side { position: static; flex-basis: auto; width: 100%; }
}
.swatch {
  display: inline-block; width: 0.6rem; height: 0.6rem;
  border-radius: 2px; margin-right: 0.35rem; vertical-align: middle;
}
.legend { margin-top: 0.5rem; }
.legend-item { display: flex; align-items: center; margin: 0.1rem 0; }
</style>
