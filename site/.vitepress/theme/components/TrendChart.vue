<script setup lang="ts">
import { computed, ref, inject } from 'vue'
import { shapes, seriesMax, xOf } from '../lib/chart'
import { fmt, revLabel } from '../lib/series'
import { HOVER_KEY } from '../lib/hover'
import type { Series, Snapshot } from '../lib/types'

// One or more series as line charts. When `normalize`, each series scales to its
// OWN 0..max so differently-scaled metrics (bytes vs counts vs ns) are all
// visible; the hover tooltip always shows real absolute values.
const props = withDefaults(
  defineProps<{
    series: Series[]
    snapshots?: Snapshot[]
    width?: number
    height?: number
    pad?: number
    normalize?: boolean
    showDots?: boolean
    interactive?: boolean
  }>(),
  {
    snapshots: () => [],
    width: 380,
    height: 300,
    pad: 24,
    normalize: false,
    showDots: false,
    interactive: false,
  },
)

const n = computed(
  () => props.snapshots.length || Math.max(0, ...props.series.map((s) => s.values.length)),
)
const sharedMax = computed(() => Math.max(0, ...props.series.map((s) => seriesMax(s.values))))

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
)

const hover = ref<number | null>(null)
// Shared crosshair column (synchronized across all tables/charts), if provided.
const sharedHover = inject(HOVER_KEY, ref<number | null>(null))
// The guide shows either this chart's own hovered column or the shared one.
const guideCol = computed(() => hover.value ?? sharedHover.value)
function onMove(ev: MouseEvent) {
  if (!props.interactive || !n.value) return
  const rect = (ev.currentTarget as SVGSVGElement).getBoundingClientRect()
  const mx = (ev.clientX - rect.left) * (props.width / rect.width)
  const frac = n.value <= 1 ? 0 : (mx - props.pad) / (props.width - 2 * props.pad)
  hover.value = Math.max(0, Math.min(n.value - 1, Math.round(frac * (n.value - 1))))
  sharedHover.value = hover.value
}
const hoverX = computed(() =>
  guideCol.value === null || guideCol.value === undefined
    ? null
    : xOf(guideCol.value, n.value, props.width, props.pad),
)
const tooltip = computed(() => {
  if (hover.value === null) return null
  const s = props.snapshots[hover.value]
  return {
    title: s ? revLabel(s) : `#${hover.value}`,
    rows: props.series.map((se) => ({
      label: se.label,
      color: se.color,
      v: fmt(se.values[hover.value as number]),
    })),
  }
})
</script>

<template>
  <div class="trendchart">
    <svg
      :viewBox="`0 0 ${width} ${height}`"
      preserveAspectRatio="none"
      class="chart"
      @mousemove="onMove"
      @mouseleave="hover = null; sharedHover = null"
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
      <line
        v-if="hoverX !== null"
        class="guide"
        :x1="hoverX"
        :y1="pad"
        :x2="hoverX"
        :y2="height - pad"
      />
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

<style scoped lang="scss">
.chart { width: 100%; height: auto; display: block; }
.axis { stroke: var(--vp-c-divider); stroke-width: 0.5; }
.series-line { stroke-width: 1.5; }
.guide { stroke: var(--vp-c-text-3); stroke-width: 0.7; stroke-dasharray: 3 3; }
.tip { font-size: 12px; margin-top: 0.4rem; line-height: 1.4; }
.tip-title { font-weight: bold; }
.tip-row { white-space: nowrap; }
.swatch {
  display: inline-block; width: 0.6rem; height: 0.6rem;
  border-radius: 2px; margin-right: 0.35rem; vertical-align: middle;
}
</style>
