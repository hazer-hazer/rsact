<script setup lang="ts">
import { computed, ref, inject, onMounted, onUnmounted } from 'vue'
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
    n?: number
    width?: number
    height?: number
    // padX defaults to 0 so inline charts align exactly with their table
    // columns (see chart.ts xOf); freestanding charts pass a small inset.
    padX?: number
    padY?: number
    normalize?: boolean
    showDots?: boolean
    interactive?: boolean
  }>(),
  {
    snapshots: () => [],
    n: 0,
    width: 380,
    height: 300,
    padX: 0,
    padY: 6,
    normalize: false,
    showDots: false,
    interactive: false,
  },
)

const n = computed(
  () => props.n || props.snapshots.length || Math.max(0, ...props.series.map((s) => s.values.length)),
)
const sharedMax = computed(() => Math.max(0, ...props.series.map((s) => seriesMax(s.values))))

// The viewBox width tracks the SVG's ACTUAL rendered pixel width so the
// coordinate space maps ~1:1 to screen (preserveAspectRatio="none" would
// otherwise stretch a fixed viewBox to the container and squash filled dots
// into flat ellipses on wide/scrolled tables). `props.width` is only the
// SSR / pre-measure fallback. Measured client-side; both chart usages appear
// only after a selection, so hydration never renders a stale width.
const svgEl = ref<SVGSVGElement | null>(null)
const measuredW = ref(props.width)
let ro: ResizeObserver | null = null
onMounted(() => {
  const el = svgEl.value
  if (!el || typeof ResizeObserver === 'undefined') return
  ro = new ResizeObserver(() => { measuredW.value = el.clientWidth || props.width })
  ro.observe(el)
})
onUnmounted(() => { ro?.disconnect(); ro = null })

const lines = computed(() =>
  props.series.map((s) => ({
    color: s.color,
    label: s.label,
    ...shapes(s.values, {
      n: n.value,
      width: measuredW.value,
      height: props.height,
      padX: props.padX,
      padY: props.padY,
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
  const mx = (ev.clientX - rect.left) * (measuredW.value / rect.width)
  const frac = (mx - props.padX) / (measuredW.value - 2 * props.padX)
  hover.value = Math.max(0, Math.min(n.value - 1, Math.floor(frac * n.value)))
  sharedHover.value = hover.value
}
// Mirrors onMove's interactive gate: onMove only WRITES sharedHover when
// interactive, so the clear must match — otherwise a non-interactive inline
// chart (e.g. MetricTable's per-row chart) would stomp the shared crosshair
// set by the interactive overlay chart on every mouseleave.
function onLeave() {
  hover.value = null
  if (props.interactive) sharedHover.value = null
}
const hoverX = computed(() =>
  guideCol.value === null || guideCol.value === undefined
    ? null
    : xOf(guideCol.value, n.value, measuredW.value, props.padX),
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
      ref="svgEl"
      :viewBox="`0 0 ${measuredW} ${height}`"
      preserveAspectRatio="none"
      class="chart"
      :style="{ height: `${height}px` }"
      @mousemove="onMove"
      @mouseleave="onLeave"
    >
      <line class="axis" :x1="padX" :y1="height - padY" :x2="measuredW - padX" :y2="height - padY" />
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
          r="1.4"
          :fill="line.color"
        />
      </template>
      <line
        v-if="hoverX !== null"
        class="guide"
        :x1="hoverX"
        :y1="padY"
        :x2="hoverX"
        :y2="height - padY"
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
// height is set inline (fixed px) so a wide colspan doesn't blow the chart up
// tall; preserveAspectRatio="none" stretches the viewBox to fill both axes.
.chart { width: 100%; display: block; }
// non-scaling-stroke keeps lines a constant thin SCREEN width despite the
// non-uniform viewBox stretch — otherwise wide charts render fat strokes.
.axis, .series-line, .guide { vector-effect: non-scaling-stroke; }
.axis { stroke: var(--vp-c-divider); stroke-width: 1; }
.series-line { stroke-width: 1.25; }
.guide { stroke: var(--vp-c-text-3); stroke-width: 1; stroke-dasharray: 3 3; }
.tip { font-size: 12px; margin-top: 0.4rem; line-height: 1.4; }
.tip-title { font-weight: bold; }
.tip-row { white-space: nowrap; }
.swatch {
  display: inline-block; width: 0.6rem; height: 0.6rem;
  border-radius: 2px; margin-right: 0.35rem; vertical-align: middle;
}
</style>
