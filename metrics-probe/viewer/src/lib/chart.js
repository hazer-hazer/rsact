// Pure SVG line-chart geometry. No DOM, no Vue. Gaps (null) become breaks in
// the line, never zero-valued points.

// Split a values[] into runs of consecutive non-null points {i, v}.
export function segments(values) {
  const segs = [];
  let cur = [];
  values.forEach((v, i) => {
    if (v === null || v === undefined) {
      if (cur.length) {
        segs.push(cur);
        cur = [];
      }
    } else {
      cur.push({ i, v });
    }
  });
  if (cur.length) segs.push(cur);
  return segs;
}

export function seriesMax(values) {
  let m = 0;
  for (const v of values) if (v !== null && v !== undefined && v > m) m = v;
  return m;
}

// x for commit slot i of n, within [pad, width-pad].
export function xOf(i, n, width, pad) {
  return pad + (n <= 1 ? 0 : (i / (n - 1)) * (width - 2 * pad));
}
// y for value v against axis max, within [pad, height-pad] (0 at the bottom).
export function yOf(v, max, height, pad) {
  return height - pad - (max <= 0 ? 0 : (v / max) * (height - 2 * pad));
}

// Turn a series into drawable pieces:
//   polys: [{ points }]      polyline point-strings for runs of >= 2 points
//   dots:  [{ x, y, i, v }]  isolated points (single-point runs) + all points when showDots
// `max` lets the caller normalize each series to its own scale when overlaying.
export function shapes(values, { n, width, height, pad, max, showDots = false }) {
  const m = max ?? seriesMax(values);
  const segs = segments(values);
  const polys = [];
  const dots = [];
  for (const seg of segs) {
    const pts = seg.map((p) => ({
      x: xOf(p.i, n, width, pad),
      y: yOf(p.v, m, height, pad),
      i: p.i,
      v: p.v,
    }));
    if (pts.length >= 2) {
      polys.push({ points: pts.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(' ') });
    }
    if (showDots || pts.length === 1) dots.push(...pts);
  }
  return { polys, dots };
}
