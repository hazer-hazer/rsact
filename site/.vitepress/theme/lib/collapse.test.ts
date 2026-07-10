import { describe, it, expect } from 'vitest'
import { columnGroups, collapseValues, columnLabel } from './collapse'
import type { SeriesRow, Snapshot } from './types'

const row = (values: (number | null)[]): SeriesRow => ({ key: 'k', label: 'k', values })
const snaps = (revs: string[]): Snapshot[] =>
  revs.map((r) => ({ git_rev: r.repeat(8), git_dirty: false, recorded_at: 0, scenarios: [] }))

describe('columnGroups', () => {
  it('collapses a fully-flat run into one column', () => {
    expect(columnGroups([row([10, 10, 10, 10])], 4)).toEqual([[0, 1, 2, 3]])
  })
  it('absorbs an interior gap (null is never a boundary)', () => {
    expect(columnGroups([row([10, 10, null, 10])], 4)).toEqual([[0, 1, 2, 3]])
  })
  it('splits on a real change that straddles a gap', () => {
    // 20 differs from the previous PRESENT value (10), so it is a boundary.
    expect(columnGroups([row([10, null, 20])], 3)).toEqual([[0, 1], [2]])
  })
  it('a late-appearing metric splits only at its real change', () => {
    // first present (11) has no prior present → not a boundary; 9 vs 11 → boundary.
    expect(columnGroups([row([null, null, 11, 9])], 4)).toEqual([[0, 1, 2], [3]])
  })
  it('boundary is global across all rows', () => {
    const rows = [row([1, 1, 1]), row([5, 5, 6])]
    expect(columnGroups(rows, 3)).toEqual([[0, 1], [2]])
  })
})

describe('collapseValues', () => {
  it('takes the group first-present value, gap if none present', () => {
    expect(collapseValues([10, 10, null, 10], [[0, 1, 2, 3]])).toEqual([10])
    expect(collapseValues([null, null, 11, 9], [[0, 1, 2], [3]])).toEqual([11, 9])
    expect(collapseValues([null, null], [[0, 1]])).toEqual([null])
  })
})

describe('columnLabel', () => {
  const s = snaps(['1', '2', '3'])
  it('single commit → 8-char rev', () => {
    expect(columnLabel(s, [1])).toBe('22222222')
  })
  it('run → first8..last8', () => {
    expect(columnLabel(s, [0, 2])).toBe('11111111..33333333')
  })
})
