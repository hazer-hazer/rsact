import { describe, it, expect } from 'vitest'
import { buildSeries, trend, prevPresent, fmt, deltaValues, isFlat } from './series'
import { SAMPLE } from './sample'

describe('buildSeries', () => {
  const groups = buildSeries(SAMPLE.snapshots)
  const scen = groups.find((g) => g.title === 'ui_labels_10')!
  const row = (label: string) => scen.rows.find((r) => r.label === label)!

  it('aligns scenario values to snapshot order', () => {
    expect(row('nodes_total').values).toEqual([100, 80, 120, 120])
  })
  it('leaves un-measured metrics as gaps (null), never zero', () => {
    expect(row('layout_visits').values).toEqual([null, null, 11, 9])
  })
  it('extracts a bench group with a leading gap', () => {
    const bench = groups.find((g) => g.title.startsWith('bench'))!
    expect(bench.rows[0].values).toEqual([null, 34, 33, 66])
  })
  it('extracts sparse section sizes as their own group', () => {
    const size = groups.find((g) => g.title.startsWith('size:'))!
    expect(size.rows.find((r) => r.label === '.text')!.values).toEqual([null, null, null, 65000])
  })
  it('drops all-gap rows but keeps all-zero rows', () => {
    expect(scen.rows.some((r) => r.label === 'idle_frame_allocs')).toBe(false)
    expect(row('effects').values).toEqual([0, 0, 0, 0])
  })
})

describe('trend (domain-aware improvement)', () => {
  const nodes = [100, 80, 120, 120]
  it('marks a decrease as improved (green) for lower-is-better', () => {
    expect(trend(nodes, 1)).toBe('up')
  })
  it('marks an increase as regressed', () => {
    expect(trend(nodes, 2)).toBe('down')
  })
  it('marks no change as neutral', () => {
    expect(trend(nodes, 3)).toBe('')
  })
  const layout = [null, null, 11, 9]
  it('compares against the previous PRESENT value, skipping gaps', () => {
    expect(trend(layout, 3)).toBe('up')
  })
  it('gives no marker to the first present value', () => {
    expect(trend(layout, 2)).toBe('')
  })
  const bench = [null, 34, 33, 66]
  it('treats a faster bench (smaller ns) as an improvement', () => {
    expect(trend(bench, 2)).toBe('up')
    expect(trend(bench, 3)).toBe('down')
  })
})

describe('prevPresent / fmt', () => {
  it('prevPresent skips nulls', () => {
    expect(prevPresent([null, null, 11, 9], 3)).toBe(11)
    expect(prevPresent([null, 5], 0)).toBe(null)
  })
  it('fmt renders integers grouped, floats rounded, null passthrough', () => {
    expect(fmt(21000)).toBe((21000).toLocaleString())
    expect(fmt(33.7)).toBe('34')
    expect(fmt(null)).toBe(null)
  })
})

describe('deltaValues / isFlat', () => {
  it('deltaValues subtracts the first present value; gaps stay null; first → 0', () => {
    expect(deltaValues([null, 10, 12, null, 9])).toEqual([null, 0, 2, null, -1])
  })
  it('isFlat: a never-changing series (with gaps) is flat', () => {
    expect(isFlat([10, 10, null, 10])).toBe(true)
    expect(isFlat([null, null, 5])).toBe(true) // single appearance, no change
  })
  it('isFlat: a real change (even across a gap) is not flat', () => {
    expect(isFlat([10, null, 20])).toBe(false)
  })
  it('deltaValues: an all-null input returns all-null (no baseline to subtract)', () => {
    expect(deltaValues([null, null])).toEqual([null, null])
  })
})
