import type { InjectionKey, Ref } from 'vue'

// The synchronized-crosshair column index (or null). MetricsDashboard provides
// it; MetricTable + TrendChart inject it to highlight the hovered column and
// draw the guide, so hovering one commit lights it up everywhere at once.
export const HOVER_KEY: InjectionKey<Ref<number | null>> = Symbol('rsact-metrics-hover')
