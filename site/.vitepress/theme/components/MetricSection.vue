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
  groupStart: boolean[]
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
          :class="{ hov: sharedHover === i, dim: !changed[i], 'group-start': groupStart[i] }"
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
td.group-start { border-left: 2px solid var(--vp-c-text-3); }
tr.metric { cursor: pointer; }
tr.metric:hover td { background: var(--vp-c-bg-soft); }
tr.metric.sel td.lbl { font-weight: bold; }
// Zero horizontal padding so the inline chart SVG spans the data columns exactly — any inset would shift points off their column centers.
tr.chartrow td { padding: 0.2rem 0; }
tr.chartrow td.lbl { background: var(--vp-c-bg); }
.muted { color: var(--vp-c-text-3); }
.up { color: var(--vp-c-success-1); }
.down { color: var(--vp-c-danger-1); }
.swatch {
  display: inline-block; width: 0.6rem; height: 0.6rem;
  border-radius: 2px; margin-right: 0.35rem; vertical-align: middle;
}
</style>
