import { describe, it, expect } from 'vitest'
import { assemble } from './assemble'
import type { Snapshot, IndexMap } from './types'

const snap = (rev: string, recorded_at: number): Snapshot => ({
  git_rev: rev, git_dirty: false, recorded_at,
  scenarios: [], section_sizes: [], bench_medians: [],
})

describe('assemble', () => {
  it('orders by index date, then recorded_at, then rev', () => {
    const snaps = [snap('c', 300), snap('a', 100), snap('b', 200)]
    const index: IndexMap = {
      a: { date: 100, parent: '', branch: 'master' },
      b: { date: 200, parent: 'a', branch: 'master' },
      c: { date: 300, parent: 'b', branch: 'master' },
    }
    const out = assemble(index, snaps)
    expect(out.snapshots.map((s) => s.git_rev)).toEqual(['a', 'b', 'c'])
    expect(out.index).toBe(index)
  })
  it('falls back to recorded_at when the index lacks a rev', () => {
    const out = assemble({}, [snap('y', 200), snap('x', 100)])
    expect(out.snapshots.map((s) => s.git_rev)).toEqual(['x', 'y'])
  })
})
