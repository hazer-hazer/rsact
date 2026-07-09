import type { MetricsData } from './types'

// Dev-only fixture so `vitest` and `vite dev` render something. Exercises the
// tricky paths: a gap, an improvement, a regression, sparse benches and sizes.
const rev = (c: string): string => c.repeat(40)

export const SAMPLE: MetricsData = {
  snapshots: [
    {
      git_rev: rev('1'), git_dirty: false, recorded_at: 100, host: 'x86_64-linux',
      scenarios: [{ name: 'ui_labels_10', counts: { total: 100, signals: 60, memos: 20, effects: 0, observers: 20, stored: 0 }, heap_live_bytes: 21000, heap_peak_bytes: 21000, build_allocs: 97, change_frame_allocs: 12, layout: null }],
      section_sizes: [], bench_medians: [],
    },
    {
      git_rev: rev('2'), git_dirty: false, recorded_at: 200, host: 'x86_64-linux',
      scenarios: [{ name: 'ui_labels_10', counts: { total: 80, signals: 50, memos: 16, effects: 0, observers: 14, stored: 0 }, heap_live_bytes: 18000, heap_peak_bytes: 18000, build_allocs: 80, change_frame_allocs: 12, layout: null }],
      section_sizes: [], bench_medians: [{ id: 'reactivity/signal_read', median_ns: 34, ci_half_ns: 1 }],
    },
    {
      git_rev: rev('3'), git_dirty: false, recorded_at: 300, host: 'x86_64-linux',
      scenarios: [{ name: 'ui_labels_10', counts: { total: 120, signals: 70, memos: 24, effects: 0, observers: 26, stored: 0 }, heap_live_bytes: 23000, heap_peak_bytes: 23000, build_allocs: 110, change_frame_allocs: 14, layout: { visits: 11, measures: 40 } }],
      section_sizes: [], bench_medians: [{ id: 'reactivity/signal_read', median_ns: 33, ci_half_ns: 1 }],
    },
    {
      git_rev: rev('4'), git_dirty: false, recorded_at: 400, host: 'x86_64-linux',
      scenarios: [{ name: 'ui_labels_10', counts: { total: 120, signals: 70, memos: 24, effects: 0, observers: 26, stored: 0 }, heap_live_bytes: 22000, heap_peak_bytes: 22000, build_allocs: 108, change_frame_allocs: 14, layout: { visits: 9, measures: 36 } }],
      section_sizes: [{ target: 'thumbv7m-none-eabi', binary: 'ui', text: 65000, rodata: 3800, bss: 1000 }],
      bench_medians: [{ id: 'reactivity/signal_read', median_ns: 66, ci_half_ns: 3 }],
    },
  ],
  index: {
    [rev('1')]: { date: 100, parent: '', branch: 'master' },
    [rev('2')]: { date: 200, parent: rev('1'), branch: 'master' },
    [rev('3')]: { date: 300, parent: rev('2'), branch: 'feature-x' },
    [rev('4')]: { date: 400, parent: rev('3'), branch: 'master' },
  },
}
