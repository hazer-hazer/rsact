export interface Counts {
  stored?: number
  signals?: number
  effects?: number
  memos?: number
  computed?: number
  observers?: number
  subscribers?: number
  subscribers_bindings?: number
  sources?: number
  sources_bindings?: number
  total?: number
}

export interface LayoutCounts {
  visits: number
  measures: number
}

export interface Scenario {
  name: string
  counts: Counts
  heap_live_bytes: number | null
  heap_peak_bytes: number | null
  build_allocs: number | null
  idle_frame_allocs?: number | null
  change_frame_allocs: number | null
  layout: LayoutCounts | null
}

export interface SectionSize {
  target: string
  binary: string
  text: number
  rodata: number
  bss: number
}

export interface BenchMedian {
  id: string
  median_ns: number
  ci_half_ns: number
}

export interface Snapshot {
  git_rev: string
  git_dirty: boolean
  recorded_at?: number
  host?: string
  scenarios: Scenario[]
  section_sizes?: SectionSize[]
  bench_medians?: BenchMedian[]
}

export interface IndexEntry {
  date: number
  parent: string
  branch: string
}
export type IndexMap = Record<string, IndexEntry>

export interface MetricsData {
  snapshots: Snapshot[]
  index: IndexMap
}

export interface SeriesRow {
  key: string
  label: string
  values: (number | null)[]
  ci?: (number | null)[]
}
export interface SeriesGroup {
  title: string
  rows: SeriesRow[]
}
export interface Series {
  label: string
  values: (number | null)[]
  color?: string
}
