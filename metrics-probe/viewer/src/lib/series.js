// Domain logic: turn raw snapshots into per-metric "series" and decide the
// direction of "better". Pure and unit-tested — no Vue, no DOM.

// Every metric we record today is lower-is-better (fewer allocs / smaller flash
// / faster ns / fewer nodes). Listed as an explicit exception set so adding a
// future higher-is-better metric is a one-liner.
export const HIGHER_IS_BETTER = new Set([]);

export function lowerIsBetter(metricKey) {
  return !HIGHER_IS_BETTER.has(metricKey);
}

const num = (v) => (v === null || v === undefined ? null : v);

// [label, accessor] for the Layer-1 scenario metrics.
const SCENARIO_METRICS = [
  ['nodes_total', (s) => s.counts?.total],
  ['signals', (s) => s.counts?.signals],
  ['memos', (s) => s.counts?.memos],
  ['effects', (s) => s.counts?.effects],
  ['observers', (s) => s.counts?.observers],
  ['stored', (s) => s.counts?.stored],
  ['heap_live_bytes', (s) => s.heap_live_bytes],
  ['heap_peak_bytes', (s) => s.heap_peak_bytes],
  ['build_allocs', (s) => s.build_allocs],
  ['idle_frame_allocs', (s) => s.idle_frame_allocs],
  ['change_frame_allocs', (s) => s.change_frame_allocs],
  ['layout_visits', (s) => (s.layout ? s.layout.visits : null)],
  ['layout_measures', (s) => (s.layout ? s.layout.measures : null)],
];

// Group the ordered snapshots into display groups. Each row's `values` array is
// aligned to `snapshots` order; a missing measurement is `null` — a GAP, never
// 0 (the 0.7e/0.7f lesson: a metric added later must not read as a regression
// from zero). Rows that are all-gap are dropped.
export function buildSeries(snapshots) {
  const groups = [];

  const scenarioNames = [
    ...new Set(snapshots.flatMap((s) => s.scenarios.map((x) => x.name))),
  ];
  for (const name of scenarioNames) {
    const rows = [];
    for (const [label, fn] of SCENARIO_METRICS) {
      const values = snapshots.map((s) => {
        const sc = s.scenarios.find((x) => x.name === name);
        return sc ? num(fn(sc)) : null;
      });
      if (values.every((v) => v === null)) continue;
      rows.push({ key: `${name}/${label}`, label, values });
    }
    if (rows.length) groups.push({ title: name, rows });
  }

  const sizeKeys = [
    ...new Set(
      snapshots.flatMap((s) =>
        (s.section_sizes || []).map((x) => `${x.binary} / ${x.target}`),
      ),
    ),
  ];
  for (const keyName of sizeKeys) {
    const [binary, target] = keyName.split(' / ');
    const rows = [];
    for (const sec of ['text', 'rodata', 'bss']) {
      const values = snapshots.map((s) => {
        const e = (s.section_sizes || []).find(
          (x) => x.binary === binary && x.target === target,
        );
        return e ? num(e[sec]) : null;
      });
      if (values.every((v) => v === null)) continue;
      rows.push({ key: `size:${keyName}/.${sec}`, label: `.${sec}`, values });
    }
    if (rows.length) groups.push({ title: `size: ${keyName}`, rows });
  }

  const benchIds = [
    ...new Set(snapshots.flatMap((s) => (s.bench_medians || []).map((b) => b.id))),
  ].sort();
  if (benchIds.length) {
    const rows = [];
    for (const id of benchIds) {
      const values = snapshots.map((s) => {
        const e = (s.bench_medians || []).find((b) => b.id === id);
        return e ? e.median_ns : null;
      });
      const ci = snapshots.map((s) => {
        const e = (s.bench_medians || []).find((b) => b.id === id);
        return e ? e.ci_half_ns : null;
      });
      if (values.every((v) => v === null)) continue;
      rows.push({ key: `bench:${id}`, label: id, values, ci });
    }
    if (rows.length) {
      groups.push({
        title: 'bench medians (ns) — CI-runner trend, ±noise, informational',
        rows,
      });
    }
  }

  return groups;
}

// The last non-null value strictly before index i, or null.
export function prevPresent(values, i) {
  for (let j = i - 1; j >= 0; j--) if (values[j] !== null) return values[j];
  return null;
}

// Improvement marker for values[i] vs the previous PRESENT value (gaps skipped):
//   'up'   improved   (green ▲)
//   'down' regressed  (red ▼)
//   ''     unchanged / no prior
// Domain-aware: for a lower-is-better metric a DECREASE is the improvement, so
// e.g. fewer allocs or a faster bench reads as ▲ even though the number went down.
export function trend(values, i, lowIsBetter = true) {
  const cur = values[i];
  const prev = prevPresent(values, i);
  if (cur === null || prev === null || cur === prev) return '';
  const improved = lowIsBetter ? cur < prev : cur > prev;
  return improved ? 'up' : 'down';
}

export function fmt(v) {
  if (v === null || v === undefined) return null;
  return Number.isInteger(v) ? v.toLocaleString() : v.toFixed(0);
}

export function revLabel(snap) {
  return snap.git_rev.slice(0, 8) + (snap.git_dirty ? '*' : '');
}
