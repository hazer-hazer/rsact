//! Regenerates a self-contained static HTML viewer over the snapshot dir. All
//! snapshots + the ordering index are embedded inline as JSON, so the page
//! opens straight from `file://` (no server, no external deps) — matching the
//! "static HTML viewer over the snapshot dir" contract.
//!
//! WS0.9e: commits are laid out in **topological order** (from `index.json`, not
//! `recorded_at` — backfilled snapshots share one wall-clock). The page is an
//! interactive trend viewer: a value table with domain-aware ▲/▼ improvement
//! markers, click-to-expand per-row charts, and a right sidepanel that overlays
//! selected series (each normalized to its own 0..max) with a hover tooltip.
//! Absent metrics render as **gaps, never zeros** (the 0.7e/0.7f lesson).

use crate::{index, snapshot::Snapshot};
use std::{collections::HashMap, fs, io, path::Path};

pub fn regenerate(dir: &Path) -> io::Result<()> {
    let mut snaps: Vec<Snapshot> = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                match Snapshot::load(&path) {
                    Ok(s) => snaps.push(s),
                    // Don't silently drop a snapshot the viewer can't parse —
                    // with serde(default) this should be rare, but if it happens
                    // the user needs to know history is incomplete.
                    Err(e) => {
                        eprintln!("  skipping {}: {e}", path.display())
                    },
                }
            }
        }
    }

    // Order commits topologically from the index (WS0.9e). Synthesize a
    // date-only entry (from recorded_at) for any snapshot rev the index doesn't
    // know yet, so ordering degrades gracefully to timestamp — never crashes.
    // This is in-memory only; the durable index.json is written by `record` /
    // `index`, not here.
    let mut idx = index::load(Path::new(index::INDEX_PATH));
    for s in &snaps {
        idx.entry(s.git_rev.clone()).or_insert_with(|| index::IndexEntry {
            date: s.recorded_at,
            parent: String::new(),
            branch: String::new(),
        });
    }
    let revs: Vec<String> = snaps.iter().map(|s| s.git_rev.clone()).collect();
    let order = index::topo_order(&idx, &revs);
    let pos: HashMap<&str, usize> =
        order.iter().enumerate().map(|(i, r)| (r.as_str(), i)).collect();
    snaps.sort_by_key(|s| *pos.get(s.git_rev.as_str()).unwrap_or(&usize::MAX));

    let snapshots_json = serde_json::to_string(&snaps).unwrap();
    let index_json = serde_json::to_string(&idx).unwrap();
    let out = dir.parent().unwrap_or(Path::new(".")).join("index.html");
    fs::write(&out, page(&snapshots_json, &index_json))?;
    println!("wrote {} ({} snapshots)", out.display(), snaps.len());
    Ok(())
}

/// The viewer template. `{ }` are used freely in the JS/CSS — data is injected
/// by placeholder replacement (not `format!`) so no brace-doubling is needed.
fn page(snapshots_json: &str, index_json: &str) -> String {
    TEMPLATE
        .replace("__SNAPSHOTS__", snapshots_json)
        .replace("__INDEX__", index_json)
}

const TEMPLATE: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>rsact metrics</title>
<style>
  :root { color-scheme: light dark; --line: gray; --muted: gray; --up: #2e9e4f; --down: #d64545; }
  body { font: 13px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; margin: 1.2rem; }
  h1 { font-size: 1.15rem; }
  p.muted { color: var(--muted); }
  .wrap { display: flex; gap: 1.5rem; align-items: flex-start; }
  .main { flex: 1 1 auto; min-width: 0; overflow-x: auto; }
  .side { flex: 0 0 380px; position: sticky; top: 1rem; }
  @media (max-width: 900px) { .wrap { flex-direction: column; } .side { position: static; flex-basis: auto; width: 100%; } }
  table { border-collapse: collapse; margin: .3rem 0 1.4rem; }
  caption { text-align: left; font-weight: bold; margin-bottom: .3rem; }
  th, td { border: 1px solid var(--line); padding: .15rem .5rem; text-align: right; white-space: nowrap; }
  th:first-child, td:first-child { text-align: left; }
  tr.metric { cursor: pointer; }
  tr.metric:hover td { background: rgba(127,127,127,.12); }
  tr.metric.sel td:first-child { font-weight: bold; }
  td.lbl .swatch { display: inline-block; width: .6rem; height: .6rem; border-radius: 2px; margin-right: .35rem; vertical-align: middle; }
  .up { color: var(--up); }
  .down { color: var(--down); }
  .muted { color: var(--muted); }
  th.dirty { opacity: .6; }
  tr.chartrow td { padding: .2rem .4rem; }
  .controls { margin: .4rem 0 1rem; }
  button { font: inherit; margin-right: .4rem; cursor: pointer; }
  .side h2 { font-size: .95rem; margin: 0 0 .4rem; }
  .legend { margin-top: .5rem; }
  .legend .item { display: flex; align-items: center; gap: .4rem; margin: .1rem 0; }
  .legend .swatch { width: .7rem; height: .7rem; border-radius: 2px; flex: 0 0 auto; }
  .tip { font-size: 12px; margin-top: .5rem; white-space: pre; min-height: 1.2rem; }
</style>
</head>
<body>
<h1>rsact framework metrics</h1>
<p class="muted">Per-commit trend. Commits left→right in history order (from index.json). Click a metric row to chart it; charted rows overlay in the right panel (each normalized to its own max — hover for absolute values). ▲ green = improved, ▼ red = regressed. Gaps = metric not measured at that commit (never zero). Bench medians are a ±noisy CI-runner trend — informational, never gating.</p>
<div class="controls">
  <button id="selall">select all</button>
  <button id="clear">clear</button>
  <span id="selcount" class="muted"></span>
</div>
<div class="wrap">
  <div class="main" id="main"></div>
  <div class="side">
    <h2>trend (selected)</h2>
    <svg id="big" width="380" height="300" role="img" aria-label="selected metric trends"></svg>
    <div id="legend" class="legend"></div>
    <div id="tip" class="tip muted">hover the chart for values</div>
  </div>
</div>
<script>
const SNAPSHOTS = __SNAPSHOTS__;
const INDEX = __INDEX__;
const SVGNS = "http://www.w3.org/2000/svg";
const PALETTE = ["#4e79a7","#f28e2c","#e15759","#76b7b2","#59a14f","#edc949","#af7aa1","#ff9da7","#9c755f","#bab0ab"];
// Every current metric is lower-is-better (fewer allocs / smaller flash /
// faster ns / fewer nodes). Encoded centrally so improvement is domain-aware,
// not numeric-direction.
const LOWER_IS_BETTER = true;
const N = SNAPSHOTS.length;

const main = document.getElementById("main");
const bigSvg = document.getElementById("big");
const legendEl = document.getElementById("legend");
const tipEl = document.getElementById("tip");

// Entry point lives at the END of the script: buildAndRender() closes over
// const state (selected / colorCursor / seriesByKey) declared further down, so
// it must run only after those initialize — calling it here would hit the
// temporal dead zone (ReferenceError).

function num(v) { return (v === null || v === undefined) ? null : v; }
function fmt(v) { return v === null ? null : (Number.isInteger(v) ? v.toLocaleString() : v.toFixed(0)); }
function revLabel(s) { return s.git_rev.slice(0, 8) + (s.git_dirty ? "*" : ""); }

// ---- assemble every metric as a series aligned to SNAPSHOTS order ----------
function buildSeries() {
  const groups = [];
  const scenMetrics = [
    ["nodes_total", s => s.counts.total],
    ["signals", s => s.counts.signals],
    ["memos", s => s.counts.memos],
    ["effects", s => s.counts.effects],
    ["observers", s => s.counts.observers],
    ["stored", s => s.counts.stored],
    ["heap_live_bytes", s => s.heap_live_bytes],
    ["heap_peak_bytes", s => s.heap_peak_bytes],
    ["build_allocs", s => s.build_allocs],
    ["idle_frame_allocs", s => s.idle_frame_allocs],
    ["change_frame_allocs", s => s.change_frame_allocs],
    ["layout_visits", s => s.layout ? s.layout.visits : null],
    ["layout_measures", s => s.layout ? s.layout.measures : null],
  ];
  const scenNames = [...new Set(SNAPSHOTS.flatMap(s => s.scenarios.map(x => x.name)))];
  for (const name of scenNames) {
    const rows = [];
    for (const [label, fn] of scenMetrics) {
      const values = SNAPSHOTS.map(s => {
        const sc = s.scenarios.find(x => x.name === name);
        return sc ? num(fn(sc)) : null;
      });
      if (values.every(v => v === null)) continue;
      rows.push({ key: name + "/" + label, label, values });
    }
    if (rows.length) groups.push({ title: name, rows });
  }

  const szKeys = [...new Set(SNAPSHOTS.flatMap(s => (s.section_sizes || []).map(x => x.binary + " / " + x.target)))];
  for (const key of szKeys) {
    const [binary, target] = key.split(" / ");
    const rows = [];
    for (const sec of ["text", "rodata", "bss"]) {
      const values = SNAPSHOTS.map(s => {
        const e = (s.section_sizes || []).find(x => x.binary === binary && x.target === target);
        return e ? num(e[sec]) : null;
      });
      if (values.every(v => v === null)) continue;
      rows.push({ key: "size:" + key + "/." + sec, label: "." + sec, values });
    }
    if (rows.length) groups.push({ title: "size: " + key, rows });
  }

  const benchIds = [...new Set(SNAPSHOTS.flatMap(s => (s.bench_medians || []).map(b => b.id)))].sort();
  if (benchIds.length) {
    const rows = [];
    for (const id of benchIds) {
      const values = SNAPSHOTS.map(s => {
        const e = (s.bench_medians || []).find(b => b.id === id);
        return e ? e.median_ns : null;
      });
      const ci = SNAPSHOTS.map(s => {
        const e = (s.bench_medians || []).find(b => b.id === id);
        return e ? e.ci_half_ns : null;
      });
      if (values.every(v => v === null)) continue;
      rows.push({ key: "bench:" + id, label: id, values, ci });
    }
    if (rows.length) groups.push({ title: "bench medians (ns) — CI-runner trend, ±noise, informational", rows });
  }
  return groups;
}

// selection: key -> {series, color}
const selected = new Map();
let colorCursor = 0;
const seriesByKey = new Map();

function buildAndRender() {
  const groups = buildSeries();
  for (const g of groups) for (const r of g.rows) seriesByKey.set(r.key, r);
  renderTables(groups);
  document.getElementById("selall").onclick = () => { for (const k of seriesByKey.keys()) select(k, true); syncAll(); };
  document.getElementById("clear").onclick = () => { selected.clear(); syncAll(); };
  redrawBig();
  updateCount();
}

function prevPresent(values, i) {
  for (let j = i - 1; j >= 0; j--) if (values[j] !== null) return values[j];
  return null;
}
function arrowHtml(values, i) {
  const cur = values[i], prev = prevPresent(values, i);
  if (cur === null || prev === null || cur === prev) return "";
  const improved = LOWER_IS_BETTER ? (cur < prev) : (cur > prev);
  return improved ? ' <span class="up" title="improved">▲</span>'
                  : ' <span class="down" title="regressed">▼</span>';
}

function renderTables(groups) {
  for (const g of groups) {
    const table = document.createElement("table");
    const cap = document.createElement("caption");
    cap.textContent = g.title;
    table.appendChild(cap);
    const head = document.createElement("tr");
    let h = "<th>metric</th>";
    for (const s of SNAPSHOTS) {
      const br = (INDEX[s.git_rev] && INDEX[s.git_rev].branch) || "";
      h += `<th class="${s.git_dirty ? "dirty" : ""}" title="${s.git_rev}${br ? " (" + br + ")" : ""}">${revLabel(s)}</th>`;
    }
    head.innerHTML = h;
    table.appendChild(head);
    for (const r of g.rows) {
      const tr = document.createElement("tr");
      tr.className = "metric";
      tr.dataset.key = r.key;
      let cells = `<td class="lbl"><span class="swatch" style="visibility:hidden"></span>${r.label}</td>`;
      r.values.forEach((v, i) => {
        cells += v === null
          ? '<td><span class="muted">–</span></td>'
          : `<td>${fmt(v)}${arrowHtml(r.values, i)}</td>`;
      });
      tr.innerHTML = cells;
      tr.onclick = () => { toggle(r.key); };
      table.appendChild(tr);
    }
    main.appendChild(table);
  }
}

function toggle(key) {
  if (selected.has(key)) selected.delete(key);
  else select(key, true);
  syncAll();
}
function select(key, on) {
  if (on && !selected.has(key)) {
    selected.set(key, { series: seriesByKey.get(key), color: PALETTE[colorCursor++ % PALETTE.length] });
  }
}

// Reflect selection into: row highlight + swatch, inline compact chart, sidepanel.
function syncAll() {
  for (const tr of main.querySelectorAll("tr.metric")) {
    const key = tr.dataset.key;
    const sel = selected.get(key);
    tr.classList.toggle("sel", !!sel);
    const sw = tr.querySelector(".swatch");
    if (sel) { sw.style.visibility = "visible"; sw.style.background = sel.color; }
    else { sw.style.visibility = "hidden"; }
    // inline compact chart row directly under the metric row
    const next = tr.nextElementSibling;
    const hasChart = next && next.classList.contains("chartrow");
    if (sel && !hasChart) {
      const cr = document.createElement("tr");
      cr.className = "chartrow";
      const td = document.createElement("td");
      td.colSpan = N + 1;
      const svg = document.createElementNS(SVGNS, "svg");
      svg.setAttribute("width", "100%");
      svg.setAttribute("height", "48");
      svg.setAttribute("viewBox", "0 0 600 48");
      svg.setAttribute("preserveAspectRatio", "none");
      drawLines(svg, [{ values: sel.series.values, color: sel.color }], 600, 48, 2, true);
      td.appendChild(svg);
      cr.appendChild(td);
      tr.after(cr);
    } else if (!sel && hasChart) {
      next.remove();
    } else if (sel && hasChart) {
      // color may have changed; nothing to do (color stable per selection)
    }
  }
  redrawBig();
  updateCount();
}

function updateCount() {
  document.getElementById("selcount").textContent =
    selected.size ? `${selected.size} series charted` : "no series selected";
}

// ---- line drawing ----------------------------------------------------------
// segments: split a values[] into runs of consecutive non-null points so gaps
// become breaks in the line (never zeros).
function segments(values) {
  const segs = [];
  let cur = [];
  values.forEach((v, i) => {
    if (v === null) { if (cur.length) { segs.push(cur); cur = []; } }
    else cur.push({ i, v });
  });
  if (cur.length) segs.push(cur);
  return segs;
}
function seriesMax(values) {
  let m = 0;
  for (const v of values) if (v !== null && v > m) m = v;
  return m;
}
// Draw one or more series, each normalized to its own 0..max (WS0.9e choice).
function drawLines(svg, list, W, H, pad, dots) {
  while (svg.firstChild) svg.removeChild(svg.firstChild);
  const xOf = i => pad + (N <= 1 ? 0 : (i / (N - 1)) * (W - 2 * pad));
  for (const { values, color } of list) {
    const max = seriesMax(values);
    const yOf = v => H - pad - (max <= 0 ? 0 : (v / max) * (H - 2 * pad));
    for (const seg of segments(values)) {
      if (seg.length >= 2) {
        const pl = document.createElementNS(SVGNS, "polyline");
        pl.setAttribute("fill", "none");
        pl.setAttribute("stroke", color);
        pl.setAttribute("stroke-width", "1.5");
        pl.setAttribute("points", seg.map(p => `${xOf(p.i).toFixed(1)},${yOf(p.v).toFixed(1)}`).join(" "));
        svg.appendChild(pl);
      }
      if (dots || seg.length === 1) {
        for (const p of seg) {
          const c = document.createElementNS(SVGNS, "circle");
          c.setAttribute("cx", xOf(p.i).toFixed(1));
          c.setAttribute("cy", yOf(p.v).toFixed(1));
          c.setAttribute("r", "1.6");
          c.setAttribute("fill", color);
          svg.appendChild(c);
        }
      }
    }
  }
}

// ---- sidepanel big chart + hover -------------------------------------------
const BW = 380, BH = 300, BPAD = 24;
function redrawBig() {
  while (bigSvg.firstChild) bigSvg.removeChild(bigSvg.firstChild);
  legendEl.innerHTML = "";
  // axes baseline
  const base = document.createElementNS(SVGNS, "line");
  base.setAttribute("x1", BPAD); base.setAttribute("y1", BH - BPAD);
  base.setAttribute("x2", BW - BPAD); base.setAttribute("y2", BH - BPAD);
  base.setAttribute("stroke", "gray"); base.setAttribute("stroke-width", ".5");
  bigSvg.appendChild(base);
  if (!selected.size) { tipEl.textContent = "hover the chart for values"; return; }
  const list = [...selected.values()].map(x => ({ values: x.series.values, color: x.color }));
  drawLines(bigSvg, list, BW, BH, BPAD, false);
  for (const { series, color } of selected.values()) {
    const item = document.createElement("div");
    item.className = "item";
    item.innerHTML = `<span class="swatch" style="background:${color}"></span><span>${series.label}</span>`;
    legendEl.appendChild(item);
  }
  tipEl.textContent = "hover the chart for values";
}

bigSvg.addEventListener("mousemove", ev => {
  if (!selected.size || N === 0) return;
  const rect = bigSvg.getBoundingClientRect();
  const mx = (ev.clientX - rect.left) * (BW / rect.width);
  const frac = N <= 1 ? 0 : (mx - BPAD) / (BW - 2 * BPAD);
  let i = Math.round(frac * (N - 1));
  i = Math.max(0, Math.min(N - 1, i));
  redrawBig();
  const x = BPAD + (N <= 1 ? 0 : (i / (N - 1)) * (BW - 2 * BPAD));
  const guide = document.createElementNS(SVGNS, "line");
  guide.setAttribute("x1", x); guide.setAttribute("y1", BPAD);
  guide.setAttribute("x2", x); guide.setAttribute("y2", BH - BPAD);
  guide.setAttribute("stroke", "gray"); guide.setAttribute("stroke-dasharray", "3 3"); guide.setAttribute("stroke-width", ".7");
  bigSvg.appendChild(guide);
  const s = SNAPSHOTS[i];
  let lines = [revLabel(s)];
  for (const { series, color } of selected.values()) {
    const v = series.values[i];
    lines.push(`${series.label}: ${v === null ? "–" : fmt(v)}`);
  }
  tipEl.textContent = lines.join("\n");
});

// ---- entry point: run only now that every const above is initialized -------
if (!N) {
  main.innerHTML = "<p>No snapshots yet. Run <code>cargo run -p metrics-probe -- record</code>.</p>";
} else {
  buildAndRender();
}
</script>
</body>
</html>
"##;
