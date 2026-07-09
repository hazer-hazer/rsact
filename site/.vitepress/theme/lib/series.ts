import type { Scenario, Snapshot, SeriesGroup, SeriesRow } from './types'

// Every metric recorded today is lower-is-better; the exception set makes adding
// a future higher-is-better metric a one-liner.
export const HIGHER_IS_BETTER = new Set<string>([])
export function lowerIsBetter(metricKey: string): boolean {
  return !HIGHER_IS_BETTER.has(metricKey)
}

const num = (v: number | null | undefined): number | null =>
  v === null || v === undefined ? null : v

const SCENARIO_METRICS: [string, (s: Scenario) => number | null | undefined][] = [
  ['nodes_total', (s) => s.counts?.total],
  ['signals', (s) => s.counts?.signals],
  ['memos', (s) => s.counts?.memos],
  ['effects', (s) => s.counts?.effects],
  ['observers', (s) => s.counts?.observers],
  ['stored', (s) => s.counts?.stored],
  ['heap_live_bytes', (s) => s.heap_live_bytes],
  ['heap_peak_bytes', (s) => s.heap_peak_bytes],
  ['build_allocs', (s) => s.build_allocs],
  ['idle_frame_allocs', (s) => s.idle_frame_allocs],
  ['change_frame_allocs', (s) => s.change_frame_allocs],
  ['layout_visits', (s) => (s.layout ? s.layout.visits : null)],
  ['layout_measures', (s) => (s.layout ? s.layout.measures : null)],
]

// Group ordered snapshots into display groups. Each row's `values` aligns to
// `snapshots` order; a missing measurement is null — a GAP, never 0. All-gap
// rows are dropped.
export function buildSeries(snapshots: Snapshot[]): SeriesGroup[] {
  const groups: SeriesGroup[] = []

  const scenarioNames = [...new Set(snapshots.flatMap((s) => s.scenarios.map((x) => x.name)))]
  for (const name of scenarioNames) {
    const rows: SeriesRow[] = []
    for (const [label, fn] of SCENARIO_METRICS) {
      const values = snapshots.map((s) => {
        const sc = s.scenarios.find((x) => x.name === name)
        return sc ? num(fn(sc)) : null
      })
      if (values.every((v) => v === null)) continue
      rows.push({ key: `${name}/${label}`, label, values })
    }
    if (rows.length) groups.push({ title: name, rows })
  }

  const sizeKeys = [
    ...new Set(snapshots.flatMap((s) => (s.section_sizes || []).map((x) => `${x.binary} / ${x.target}`))),
  ]
  for (const keyName of sizeKeys) {
    const [binary, target] = keyName.split(' / ')
    const rows: SeriesRow[] = []
    for (const sec of ['text', 'rodata', 'bss'] as const) {
      const values = snapshots.map((s) => {
        const e = (s.section_sizes || []).find((x) => x.binary === binary && x.target === target)
        return e ? num(e[sec]) : null
      })
      if (values.every((v) => v === null)) continue
      rows.push({ key: `size:${keyName}/.${sec}`, label: `.${sec}`, values })
    }
    if (rows.length) groups.push({ title: `size: ${keyName}`, rows })
  }

  const benchIds = [
    ...new Set(snapshots.flatMap((s) => (s.bench_medians || []).map((b) => b.id))),
  ].sort()
  if (benchIds.length) {
    const rows: SeriesRow[] = []
    for (const id of benchIds) {
      const values = snapshots.map((s) => {
        const e = (s.bench_medians || []).find((b) => b.id === id)
        return e ? e.median_ns : null
      })
      const ci = snapshots.map((s) => {
        const e = (s.bench_medians || []).find((b) => b.id === id)
        return e ? e.ci_half_ns : null
      })
      if (values.every((v) => v === null)) continue
      rows.push({ key: `bench:${id}`, label: id, values, ci })
    }
    if (rows.length) {
      groups.push({ title: 'bench medians (ns) — CI-runner trend, ±noise, informational', rows })
    }
  }

  return groups
}

// The last non-null value strictly before index i, or null.
export function prevPresent(values: (number | null)[], i: number): number | null {
  for (let j = i - 1; j >= 0; j--) if (values[j] !== null) return values[j]
  return null
}

// Domain-aware improvement marker vs the previous PRESENT value (gaps skipped).
export function trend(
  values: (number | null)[], i: number, lowIsBetter = true,
): '' | 'up' | 'down' {
  const cur = values[i]
  const prev = prevPresent(values, i)
  if (cur === null || prev === null || cur === prev) return ''
  const improved = lowIsBetter ? cur < prev : cur > prev
  return improved ? 'up' : 'down'
}

export function fmt(v: number | null | undefined): string | null {
  if (v === null || v === undefined) return null
  return Number.isInteger(v) ? v.toLocaleString() : v.toFixed(0)
}

export function revLabel(snap: Snapshot): string {
  return snap.git_rev.slice(0, 8) + (snap.git_dirty ? '*' : '')
}
