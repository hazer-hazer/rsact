import { describe, it, expect } from 'vitest'
import { segments, seriesMax, shapes } from './chart'

describe('segments', () => {
  it('splits on nulls so gaps become breaks, not zero points', () => {
    const segs = segments([null, null, 11, 9])
    expect(segs.length).toBe(1)
    expect(segs[0]).toEqual([{ i: 2, v: 11 }, { i: 3, v: 9 }])
  })
  it('produces multiple runs around an interior gap', () => {
    expect(segments([1, null, 3]).map((s) => s.length)).toEqual([1, 1])
  })
})

describe('seriesMax', () => {
  it('ignores nulls', () => {
    expect(seriesMax([null, 5, null, 12, 3])).toBe(12)
  })
})

describe('shapes', () => {
  const opts = { n: 4, width: 400, height: 100, pad: 10 }
  it('emits a polyline for a run of >= 2 points', () => {
    const { polys } = shapes([10, 20, 30, 40], opts)
    expect(polys.length).toBe(1)
    expect(polys[0].points.split(' ').length).toBe(4)
  })
  it('emits an isolated point as a dot, not a polyline', () => {
    const { polys, dots } = shapes([5, null, 9], opts)
    expect(polys.length).toBe(0)
    expect(dots.length).toBe(2)
  })
  it('normalizes to the provided max (top hugs the top pad)', () => {
    const { polys } = shapes([50, 100], { ...opts, n: 2, max: 100 })
    const [, second] = polys[0].points.split(' ')
    expect(Number(second.split(',')[1])).toBeCloseTo(opts.pad, 1)
  })
})
