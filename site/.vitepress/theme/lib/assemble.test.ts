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

  it('orders by index.order first, ahead of commit date', () => {
    // 'c' has the EARLIEST date but the LAST order → order wins (this is the
    // grouped-mainline case: a PR merged late can carry commits dated early).
    const snaps = [snap('c', 100), snap('a', 300), snap('b', 200)]
    const index: IndexMap = {
      a: { date: 300, parent: '', branch: '', order: 0 },
      b: { date: 200, parent: '', branch: '', order: 1 },
      c: { date: 100, parent: '', branch: '', order: 2 },
    }
    expect(assemble(index, snaps).snapshots.map((s) => s.git_rev)).toEqual(['a', 'b', 'c'])
  })

  it('sorts order-less snapshots after ordered ones, by date', () => {
    // A freshly `record`ed HEAD (no order yet) is the newest commit → it must
    // sort AFTER the ordered history, not interleave by its raw date value.
    const snaps = [snap('newHead', 999), snap('x', 500)]
    const index: IndexMap = { x: { date: 500, parent: '', branch: '', order: 5 } }
    expect(assemble(index, snaps).snapshots.map((s) => s.git_rev)).toEqual(['x', 'newHead'])
  })
})
