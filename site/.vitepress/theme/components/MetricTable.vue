<script setup lang="ts">
import { computed } from 'vue'
import TrendChart from './TrendChart.vue'
import { trend, fmt, revLabel, lowerIsBetter } from '../lib/series'
import type { SeriesGroup, Snapshot, IndexMap } from '../lib/types'

// One group (scenario / size-target / benches) as a value table. Columns are
// commits (history order); each cell shows the value plus a domain-aware ▲/▼
// marker vs the previous present commit. Clicking a row toggles it in the
// parent's selection; a selected row expands an inline compact chart.
const props = defineProps<{
  group: SeriesGroup
  snapshots: Snapshot[]
  index?: IndexMap
  selected: Map<string, string>
}>()
defineEmits<{ toggle: [key: string] }>()

const branchOf = (rev: string) => props.index?.[rev]?.branch || ''

const rows = computed(() =>
  props.group.rows.map((r) => ({
    ...r,
    cells: r.values.map((v, i) => ({ v, mark: trend(r.values, i, lowerIsBetter(r.key)) })),
  })),
)
</script>

<template>
  <table>
    <caption>{{ group.title }}</caption>
    <thead>
      <tr>
        <th class="lbl">metric</th>
        <th
          v-for="s in snapshots"
          :key="s.git_rev"
          :class="{ dirty: s.git_dirty }"
          :title="s.git_rev + (branchOf(s.git_rev) ? ` (${branchOf(s.git_rev)})` : '')"
        >
          {{ revLabel(s) }}
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
                background: selected.get(row.key) || 'transparent',
                visibility: selected.has(row.key) ? 'visible' : 'hidden',
              }"
            ></span>
            {{ row.label }}
          </td>
          <td v-for="(cell, i) in row.cells" :key="i">
            <span v-if="cell.v === null" class="muted">–</span>
            <template v-else
              >{{ fmt(cell.v) }}<span v-if="cell.mark" :class="cell.mark">{{
                cell.mark === 'up' ? ' ▲' : ' ▼'
              }}</span></template
            >
          </td>
        </tr>
        <tr v-if="selected.has(row.key)" class="chartrow">
          <td :colspan="snapshots.length + 1">
            <TrendChart
              :series="[{ label: row.label, values: row.values, color: selected.get(row.key) }]"
              :snapshots="snapshots"
              :height="56"
              :pad="6"
              :show-dots="true"
            />
          </td>
        </tr>
      </template>
    </tbody>
  </table>
</template>

<style scoped lang="scss">
table { border-collapse: collapse; margin: 0.3rem 0 1.4rem; font-family: var(--vp-font-family-mono); font-size: 12px; }
caption { text-align: left; font-weight: bold; margin-bottom: 0.3rem; }
th, td {
  border: 1px solid var(--vp-c-divider);
  padding: 0.15rem 0.5rem; text-align: right; white-space: nowrap;
}
th.lbl, td.lbl { text-align: left; }
th.dirty { opacity: 0.6; }
tr.metric { cursor: pointer; }
tr.metric:hover td { background: var(--vp-c-bg-soft); }
tr.metric.sel td.lbl { font-weight: bold; }
tr.chartrow td { padding: 0.2rem 0.4rem; }
.muted { color: var(--vp-c-text-3); }
.up { color: #2e9e4f; }
.down { color: #d64545; }
.swatch {
  display: inline-block; width: 0.6rem; height: 0.6rem;
  border-radius: 2px; margin-right: 0.35rem; vertical-align: middle;
}
</style>
