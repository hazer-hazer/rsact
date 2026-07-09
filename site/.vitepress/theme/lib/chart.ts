// Pure SVG line-chart geometry. Gaps (null) become breaks in the line, never
// zero-valued points.
export interface Pt { i: number; v: number }
export interface DotPt { x: number; y: number; i: number; v: number }
export interface Poly { points: string }
export interface Shapes { polys: Poly[]; dots: DotPt[] }
export interface ShapeOpts {
  n: number; width: number; height: number; pad: number; max?: number; showDots?: boolean
}

export function segments(values: (number | null)[]): Pt[][] {
  const segs: Pt[][] = []
  let cur: Pt[] = []
  values.forEach((v, i) => {
    if (v === null || v === undefined) {
      if (cur.length) { segs.push(cur); cur = [] }
    } else {
      cur.push({ i, v })
    }
  })
  if (cur.length) segs.push(cur)
  return segs
}

export function seriesMax(values: (number | null)[]): number {
  let m = 0
  for (const v of values) if (v !== null && v !== undefined && v > m) m = v
  return m
}

// x for commit slot i of n, at the CENTER of cell i within [pad, width-pad].
// Cell-centered (not endpoint-anchored) so points sit dead-center under their
// equal-width table columns (fixed-layout table).
export function xOf(i: number, n: number, width: number, pad: number): number {
  return pad + (n <= 0 ? 0 : ((i + 0.5) / n) * (width - 2 * pad))
}
export function yOf(v: number, max: number, height: number, pad: number): number {
  return height - pad - (max <= 0 ? 0 : (v / max) * (height - 2 * pad))
}

// polys: point-strings for runs of >= 2 points; dots: isolated points (+ all
// points when showDots). `max` lets the caller normalize each series to its own
// scale when overlaying.
export function shapes(
  values: (number | null)[],
  { n, width, height, pad, max, showDots = false }: ShapeOpts,
): Shapes {
  const m = max ?? seriesMax(values)
  const segs = segments(values)
  const polys: Poly[] = []
  const dots: DotPt[] = []
  for (const seg of segs) {
    const pts: DotPt[] = seg.map((p) => ({
      x: xOf(p.i, n, width, pad),
      y: yOf(p.v, m, height, pad),
      i: p.i,
      v: p.v,
    }))
    if (pts.length >= 2) {
      polys.push({ points: pts.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(' ') })
    }
    if (showDots || pts.length === 1) dots.push(...pts)
  }
  return { polys, dots }
}
