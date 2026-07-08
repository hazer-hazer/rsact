import { describe, it, expect } from 'vitest';
import { buildSeries, trend, prevPresent, fmt } from './series.js';
import { SAMPLE } from './sample.js';

describe('buildSeries', () => {
  const groups = buildSeries(SAMPLE.snapshots);
  const scen = groups.find((g) => g.title === 'ui_labels_10');
  const row = (label) => scen.rows.find((r) => r.label === label);

  it('aligns scenario values to snapshot order', () => {
    expect(row('nodes_total').values).toEqual([100, 80, 120, 120]);
  });

  it('leaves un-measured metrics as gaps (null), never zero', () => {
    // layout counters only exist from the 3rd commit on.
    expect(row('layout_visits').values).toEqual([null, null, 11, 9]);
  });

  it('extracts a bench group with a leading gap', () => {
    const bench = groups.find((g) => g.title.startsWith('bench'));
    expect(bench.rows[0].values).toEqual([null, 34, 33, 66]);
  });

  it('extracts sparse section sizes as their own group', () => {
    const size = groups.find((g) => g.title.startsWith('size:'));
    expect(size.rows.find((r) => r.label === '.text').values).toEqual([null, null, null, 65000]);
  });

  it('drops all-gap rows but keeps all-zero rows', () => {
    // idle_frame_allocs is absent from every sample scenario → all-null → dropped.
    expect(scen.rows.some((r) => r.label === 'idle_frame_allocs')).toBe(false);
    // effects is 0 everywhere → present (0 is a real value, not a gap) → kept.
    expect(row('effects').values).toEqual([0, 0, 0, 0]);
  });
});

describe('trend (domain-aware improvement)', () => {
  const nodes = [100, 80, 120, 120];
  it('marks a decrease as improved (green ▲) for lower-is-better', () => {
    expect(trend(nodes, 1)).toBe('up'); // 100 -> 80
  });
  it('marks an increase as regressed (red ▼)', () => {
    expect(trend(nodes, 2)).toBe('down'); // 80 -> 120
  });
  it('marks no change as neutral', () => {
    expect(trend(nodes, 3)).toBe(''); // 120 -> 120
  });

  const layout = [null, null, 11, 9];
  it('compares against the previous PRESENT value, skipping gaps', () => {
    expect(trend(layout, 3)).toBe('up'); // 11 -> 9
  });
  it('gives no marker to the first present value (prior all gaps)', () => {
    expect(trend(layout, 2)).toBe('');
  });

  const bench = [null, 34, 33, 66];
  it('treats a faster bench (smaller ns) as an improvement', () => {
    expect(trend(bench, 2)).toBe('up'); // 34 -> 33
    expect(trend(bench, 3)).toBe('down'); // 33 -> 66
  });
});

describe('prevPresent / fmt', () => {
  it('prevPresent skips nulls', () => {
    expect(prevPresent([null, null, 11, 9], 3)).toBe(11);
    expect(prevPresent([null, 5], 0)).toBe(null);
  });
  it('fmt renders integers with grouping and floats rounded, null passthrough', () => {
    expect(fmt(21000)).toBe((21000).toLocaleString());
    expect(fmt(33.7)).toBe('34');
    expect(fmt(null)).toBe(null);
  });
});
