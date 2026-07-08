<script setup>
import { computed } from 'vue';
import TrendChart from './TrendChart.vue';
import { trend, fmt, revLabel, lowerIsBetter } from '../lib/series.js';

// One group (scenario / size-target / benches) as a value table. Columns are
// commits (already in history order); each cell shows the value plus a
// domain-aware ▲/▼ marker vs the previous present commit. Clicking a row toggles
// it in the parent's selection; a selected row expands an inline compact chart.
const props = defineProps({
  group: { type: Object, required: true },
  snapshots: { type: Array, required: true },
  index: { type: Object, default: () => ({}) },
  selected: { type: Map, required: true }, // key -> color
});
defineEmits(['toggle']);

const branchOf = (rev) => props.index[rev]?.branch || '';

// Precompute per-cell markers once per row (avoids recomputing trend() twice per cell).
const rows = computed(() =>
  props.group.rows.map((r) => ({
    ...r,
    cells: r.values.map((v, i) => ({
      v,
      mark: trend(r.values, i, lowerIsBetter(r.key)),
    })),
  })),
);
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
