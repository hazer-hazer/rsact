<script setup>
import { computed, ref } from 'vue';
import { shapes, seriesMax, xOf } from '../lib/chart.js';
import { fmt, revLabel } from '../lib/series.js';

// One or more series drawn as line charts. When `normalize`, each series is
// scaled to its OWN 0..max so differently-scaled metrics (bytes vs counts vs ns)
// are all visible for spotting correlations; the hover tooltip always shows the
// real absolute values. A single non-normalized series therefore keeps a true
// absolute 0-based axis.
const props = defineProps({
  series: { type: Array, required: true }, // [{ label, values, color }]
  snapshots: { type: Array, default: () => [] },
  width: { type: Number, default: 380 },
  height: { type: Number, default: 300 },
  pad: { type: Number, default: 24 },
  normalize: { type: Boolean, default: false },
  showDots: { type: Boolean, default: false },
  interactive: { type: Boolean, default: false },
});

const n = computed(
  () => props.snapshots.length || Math.max(0, ...props.series.map((s) => s.values.length)),
);
const sharedMax = computed(() => Math.max(0, ...props.series.map((s) => seriesMax(s.values))));

const lines = computed(() =>
  props.series.map((s) => ({
    color: s.color,
    label: s.label,
    ...shapes(s.values, {
      n: n.value,
      width: props.width,
      height: props.height,
      pad: props.pad,
      max: props.normalize ? seriesMax(s.values) : sharedMax.value,
      showDots: props.showDots,
    }),
  })),
);

const hover = ref(null); // commit index under the cursor
function onMove(ev) {
  if (!props.interactive || !n.value) return;
  const rect = ev.currentTarget.getBoundingClientRect();
  const mx = (ev.clientX - rect.left) * (props.width / rect.width);
  const frac = n.value <= 1 ? 0 : (mx - props.pad) / (props.width - 2 * props.pad);
  hover.value = Math.max(0, Math.min(n.value - 1, Math.round(frac * (n.value - 1))));
}
const hoverX = computed(() =>
  hover.value === null ? null : xOf(hover.value, n.value, props.width, props.pad),
);
const tooltip = computed(() => {
  if (hover.value === null) return null;
  const s = props.snapshots[hover.value];
  return {
    title: s ? revLabel(s) : `#${hover.value}`,
    rows: props.series.map((se) => ({
      label: se.label,
      color: se.color,
      v: fmt(se.values[hover.value]),
    })),
  };
});
</script>

<template>
  <div class="trendchart">
    <svg
      :viewBox="`0 0 ${width} ${height}`"
      preserveAspectRatio="none"
      class="chart"
      @mousemove="onMove"
      @mouseleave="hover = null"
    >
      <line class="axis" :x1="pad" :y1="height - pad" :x2="width - pad" :y2="height - pad" />
      <template v-for="line in lines" :key="line.label">
        <polyline
          v-for="(poly, pi) in line.polys"
          :key="`p${pi}`"
          class="series-line"
          fill="none"
          :stroke="line.color"
          :points="poly.points"
        />
        <circle
          v-for="(d, di) in line.dots"
          :key="`d${di}`"
          :cx="d.x"
          :cy="d.y"
          r="1.6"
          :fill="line.color"
        />
      </template>
      <line v-if="hoverX !== null" class="guide" :x1="hoverX" :y1="pad" :x2="hoverX" :y2="height - pad" />
    </svg>
    <div v-if="interactive && tooltip" class="tip">
      <div class="tip-title">{{ tooltip.title }}</div>
      <div v-for="r in tooltip.rows" :key="r.label" class="tip-row">
        <span class="swatch" :style="{ background: r.color }"></span>{{ r.label }}:
        {{ r.v === null ? '–' : r.v }}
      </div>
    </div>
  </div>
</template>

<style scoped>
.chart {
  width: 100%;
  height: auto;
  display: block;
}
.axis {
  stroke: gray;
  stroke-width: 0.5;
}
.series-line {
  stroke-width: 1.5;
}
.guide {
  stroke: gray;
  stroke-width: 0.7;
  stroke-dasharray: 3 3;
}
.tip {
  font-size: 12px;
  margin-top: 0.4rem;
  line-height: 1.4;
}
.tip-title {
  font-weight: bold;
}
.tip-row {
  white-space: nowrap;
}
.swatch {
  display: inline-block;
  width: 0.6rem;
  height: 0.6rem;
  border-radius: 2px;
  margin-right: 0.35rem;
  vertical-align: middle;
}
</style>
