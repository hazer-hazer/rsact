import { describe, it, expect } from 'vitest'
import { boundaryFlags, columnGroups, collapseValues, columnLabel, columnNet } from './collapse'
import type { SeriesRow, Snapshot } from './types'

const row = (values: (number | null)[]): SeriesRow => ({ key: 'k', label: 'k', values })
const snaps = (revs: string[]): Snapshot[] =>
  revs.map((r) => ({ git_rev: r.repeat(8), git_dirty: false, recorded_at: 0, scenarios: [] }))

describe('boundaryFlags', () => {
  const rows = (values: (number | null)[]) => [{ key: 'k', label: 'k', values }]
  it('index 0 is always a boundary (baseline), unchanged runs are not', () => {
    expect(boundaryFlags(rows([10, 10, 10]), 3)).toEqual([true, false, false])
  })
  it('a real change flips the flag at that column', () => {
    expect(boundaryFlags(rows([10, 10, 20]), 3)).toEqual([true, false, true])
  })
  it('nulls never flag; a change across a gap flags at the reappearance', () => {
    expect(boundaryFlags(rows([10, null, 20]), 3)).toEqual([true, false, true])
    expect(boundaryFlags(rows([10, 10, null, 10]), 4)).toEqual([true, false, false, false])
  })
})

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

describe('columnNet', () => {
  it('counts improvements (lower-is-better) and regressions per column; col 0 neutral', () => {
    // groups = one column per commit (no collapse)
    const groups = [[0], [1], [2]]
    const rows = [
      { key: 'a', label: 'a', values: [10, 8, 8] },   // improves at col1 (10->8)
      { key: 'b', label: 'b', values: [5, 9, 9] },    // regresses at col1 (5->9)
      { key: 'c', label: 'c', values: [1, 1, 2] },    // regresses at col2 (1->2)
    ]
    expect(columnNet(rows, groups)).toEqual([
      { up: 0, down: 0 }, // col0 baseline
      { up: 1, down: 1 }, // a up, b down
      { up: 0, down: 1 }, // c down
    ])
  })
  it('operates on collapsed values so it matches the displayed columns', () => {
    const rows = [{ key: 'a', label: 'a', values: [20, 10, 10] }]
    const groups = columnGroups(rows, 3) // [[0,1],[2]]
    expect(columnNet(rows, groups)).toEqual([{ up: 0, down: 0 }, { up: 1, down: 0 }])
  })
})
