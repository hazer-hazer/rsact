<script setup>
import { reactive, computed } from 'vue';
import MetricTable from './components/MetricTable.vue';
import TrendChart from './components/TrendChart.vue';
import { buildSeries } from './lib/series.js';

const props = defineProps({
  snapshots: { type: Array, default: () => [] },
  index: { type: Object, default: () => ({}) },
});

const PALETTE = [
  '#4e79a7', '#f28e2c', '#e15759', '#76b7b2', '#59a14f',
  '#edc949', '#af7aa1', '#ff9da7', '#9c755f', '#bab0ab',
];

const groups = computed(() => buildSeries(props.snapshots));
const seriesByKey = computed(() => {
  const m = new Map();
  for (const g of groups.value) for (const r of g.rows) m.set(r.key, r);
  return m;
});

// key -> color. `reactive` Map so the table swatches + sidepanel react to edits.
const selected = reactive(new Map());
let cursor = 0;
function toggle(key) {
  if (selected.has(key)) selected.delete(key);
  else selected.set(key, PALETTE[cursor++ % PALETTE.length]);
}
function selectAll() {
  for (const g of groups.value)
    for (const r of g.rows)
      if (!selected.has(r.key)) selected.set(r.key, PALETTE[cursor++ % PALETTE.length]);
}

const selectedSeries = computed(() =>
  [...selected.entries()].map(([key, color]) => ({
    label: seriesByKey.value.get(key)?.label ?? key,
    values: seriesByKey.value.get(key)?.values ?? [],
    color,
  })),
);
</script>

<template>
  <h1>rsact framework metrics</h1>
  <p class="muted">
    Per-commit trend, oldest → newest (history order from <code>index.json</code>). Click a
    metric row to chart it; charted rows overlay in the right panel, each normalized to its own
    max — hover for absolute values. <span class="up">▲</span> improved,
    <span class="down">▼</span> regressed (domain-aware: fewer/smaller/faster is better). Gaps
    mean the metric wasn't measured at that commit — never zero. Bench medians are a ±noisy
    CI-runner trend, informational.
  </p>

  <p v-if="!snapshots.length" class="muted">
    No snapshots yet. Run <code>cargo run -p metrics-probe -- record</code>.
  </p>

  <template v-else>
    <div class="controls">
      <button @click="selectAll">select all</button>
      <button @click="selected.clear()">clear</button>
      <span class="muted">{{ selected.size ? `${selected.size} charted` : 'no series selected' }}</span>
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
</template>

<style>
:root {
  color-scheme: light dark;
  --line: gray;
  --muted: gray;
  --up: #2e9e4f;
  --down: #d64545;
}
body {
  font: 13px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace;
  margin: 1.2rem;
}
h1 { font-size: 1.15rem; }
h2 { font-size: 0.95rem; margin: 0 0 0.4rem; }
.muted { color: var(--muted); }
.up { color: var(--up); }
.down { color: var(--down); }
code { font-size: 0.95em; }

.controls { margin: 0.4rem 0 1rem; }
button { font: inherit; margin-right: 0.4rem; cursor: pointer; }

.wrap { display: flex; gap: 1.5rem; align-items: flex-start; }
.main { flex: 1 1 auto; min-width: 0; overflow-x: auto; }
.side { flex: 0 0 380px; position: sticky; top: 1rem; }
@media (max-width: 900px) {
  .wrap { flex-direction: column; }
  .side { position: static; flex-basis: auto; width: 100%; }
}

table { border-collapse: collapse; margin: 0.3rem 0 1.4rem; }
caption { text-align: left; font-weight: bold; margin-bottom: 0.3rem; }
th, td {
  border: 1px solid var(--line);
  padding: 0.15rem 0.5rem;
  text-align: right;
  white-space: nowrap;
}
th.lbl, td.lbl { text-align: left; }
th.dirty { opacity: 0.6; }
tr.metric { cursor: pointer; }
tr.metric:hover td { background: rgba(127, 127, 127, 0.12); }
tr.metric.sel td.lbl { font-weight: bold; }
tr.chartrow td { padding: 0.2rem 0.4rem; }

.swatch {
  display: inline-block;
  width: 0.6rem;
  height: 0.6rem;
  border-radius: 2px;
  margin-right: 0.35rem;
  vertical-align: middle;
}
.legend { margin-top: 0.5rem; }
.legend-item { display: flex; align-items: center; margin: 0.1rem 0; }
</style>
