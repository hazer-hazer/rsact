import { prevPresent } from './series'
import type { SeriesRow, Snapshot } from './types'

// Per-commit boundary flags. flags[0] is always true (the baseline starts the
// first run). flags[i] (i>0) is true iff, for some row, its PRESENT value at i
// differs from that row's previous present value (gaps skipped). A null is never
// a boundary — it carries forward. Drives both column collapsing and the
// "changed column" dimming (#5).
export function boundaryFlags(rows: SeriesRow[], n: number): boolean[] {
  const flags: boolean[] = []
  for (let i = 0; i < n; i++) {
    flags.push(
      i === 0 ||
        rows.some((r) => {
          const v = r.values[i]
          if (v === null || v === undefined) return false
          const prev = prevPresent(r.values, i)
          return prev !== null && v !== prev
        }),
    )
  }
  return flags
}

// Maximal runs of commit indices between boundaries. Preserves a real change
// across a gap (e.g. [10, null, 20] splits at 20) while absorbing measurement
// gaps.
export function columnGroups(rows: SeriesRow[], n: number): number[][] {
  const flags = boundaryFlags(rows, n)
  const groups: number[][] = []
  let cur: number[] = []
  for (let i = 0; i < n; i++) {
    if (i > 0 && flags[i]) {
      groups.push(cur)
      cur = []
    }
    cur.push(i)
  }
  if (cur.length) groups.push(cur)
  return groups
}

// One value per group: the group's first present value, else null (a metric
// absent throughout the run). Within a group present values are all equal by the
// boundary rule, so "first present" is the run's representative value.
export function collapseValues(
  values: (number | null)[],
  groups: number[][],
): (number | null)[] {
  return groups.map((g) => {
    for (const i of g) {
      const v = values[i]
      if (v !== null && v !== undefined) return v
    }
    return null
  })
}

const rev8 = (s: Snapshot) => s.git_rev.slice(0, 8) + (s.git_dirty ? '*' : '')

// Single-commit column → its 8-char rev; a collapsed run → `<first8>..<last8>`.
export function columnLabel(snapshots: Snapshot[], group: number[]): string {
  if (group.length === 1) return rev8(snapshots[group[0]])
  const first = snapshots[group[0]]
  const last = snapshots[group[group.length - 1]]
  return `${first.git_rev.slice(0, 8)}..${last.git_rev.slice(0, 8)}`
}
