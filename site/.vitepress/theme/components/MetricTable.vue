<script setup lang="ts">
import { computed, inject, ref } from 'vue'
import TrendChart from './TrendChart.vue'
import { trend, fmt, lowerIsBetter, deltaValues } from '../lib/series'
import { collapseValues } from '../lib/collapse'
import { colorFor } from '../lib/colors'
import { HOVER_KEY } from '../lib/hover'
import type { SeriesGroup } from '../lib/types'

// A "column" is a collapsed group of commit indices with a display label.
interface Column { label: string; title: string; group: number[] }

const props = defineProps<{
  group: SeriesGroup
  columns: Column[]
  selected: Set<string>
  delta: boolean
}>()
defineEmits<{ toggle: [key: string] }>()

const sharedHover = inject(HOVER_KEY, ref<number | null>(null))

// Per row: collapse the full values to per-column values, optionally Δ, and
// precompute the ▲/▼ marker vs the previous present column value.
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
  <table>
    <caption>{{ group.title }}</caption>
    <thead>
      <tr>
        <th class="lbl">metric</th>
        <th
          v-for="(c, i) in columns"
          :key="c.title"
          :title="c.title"
          :class="{ hov: sharedHover === i }"
          @mouseenter="sharedHover = i"
          @mouseleave="sharedHover = null"
        >
          {{ c.label }}
        </th>
      </tr>
    </thead>
    <tbody>
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
            :class="{ hov: sharedHover === i }"
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
  </table>
</template>

<style scoped lang="scss">
table { border-collapse: collapse; margin: 0.3rem 0 1.4rem; font-family: var(--vp-font-family-mono); font-size: 12px; table-layout: fixed; }
caption { text-align: left; font-weight: bold; margin-bottom: 0.3rem; }
th, td {
  border: 1px solid var(--vp-c-divider);
  padding: 0.15rem 0.5rem; text-align: right; white-space: nowrap;
  width: 5.5rem; overflow: hidden; text-overflow: ellipsis;
}
// Uniform, readable, STICKY first column so it stays visible when scrolled and
// lines up across every table (shared width var).
th.lbl, td.lbl {
  text-align: left; position: sticky; left: 0; z-index: 1;
  width: var(--metric-col-w, 13rem); min-width: var(--metric-col-w, 13rem);
  background: var(--vp-c-bg);
}
th.hov, td.hov { background: var(--vp-c-bg-soft); }
tr.metric { cursor: pointer; }
tr.metric:hover td { background: var(--vp-c-bg-soft); }
tr.metric.sel td.lbl { font-weight: bold; }
// Zero horizontal padding on the chart cell so the SVG spans the data columns
// exactly — any inset here would shift points off their column centers.
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
