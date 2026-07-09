# Metrics viewer v2 + single-viewer consolidation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the site's metrics page the single home for rsact metrics — remove the standalone `metrics-probe` HTML viewer (site dev-mode gains local-store charting so nothing is lost), then land 6 UX enhancements (incl. a chart-alignment bug fix) + 4 extras.

**Architecture:** Consolidation first (delete the Rust/viewer HTML path; one shared `assemble()` feeds both the CI `data.json` and a dev-server plugin that charts the local `metrics/` store). Then the enhancements, with all logic in pure, vitest-tested `lib/*.ts` and thin Vue components. Spec: `docs/superpowers/specs/2026-07-09-metrics-viewer-v2-design.md`.

**Tech Stack:** VitePress 1.6 + Vue 3 + TypeScript + SCSS + Vitest (site/); Rust (metrics-probe removal). Deps already installed in `site/`.

## Global Constraints

- Metrics live **only** on the site after this work; the standalone `metrics-probe html` viewer is removed. `record`/`diff`/`index`/`hook-install` + the local `metrics/` store + `metrics-data` writes all stay.
- **No capability lost:** `docs:dev` must chart the local git-ignored `metrics/` store (empty → sample fixture); `docs:build`/prod fetch path unchanged.
- TypeScript over JS, SCSS over CSS. Logic in pure `lib/*.ts`; components thin. `node_modules` ignored.
- **NEVER run commands in the background** (foreground only; deps installed so `npm test`/`docs:build` are fast; `cargo` may take minutes — extend the Bash timeout, don't background).
- **Collapse boundary rule:** a commit starts a new column iff some metric's *present* value differs from that metric's *previous present* value (gap-skipping `prevPresent` semantics). A null is **never** a boundary (carries forward). A real change across a gap (`[10,null,20]`) still splits.
- **Gaps ≠ zeros** everywhere (existing invariant).
- **Stable colors:** a metric key always maps to the same color (deterministic), everywhere.
- Branch `ws19-metrics-v2` (stacked off `ws19-website`). Commit trailer: `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`. Do NOT open the PR until after the final review (controller handles branch-finishing).

---

### Task 1: Remove the standalone viewer (Rust + scripts)

**Files:**
- Delete: `metrics-probe/viewer/` (whole dir), `metrics-probe/src/html.rs`
- Modify: `metrics-probe/src/main.rs` (remove `mod html`; the `html::regenerate` call in `cmd_record`; the `html` subcommand arm; the `html` usage + doc-comment lines)
- Modify: `scripts/ci-backfill.sh` (remove the `metrics-probe -- html` step + its comment)
- Modify: `scripts/ci-metrics.sh` (refresh the two comments that mention regenerating `index.html`)

**Interfaces:**
- Produces: a `metrics-probe` with no `html` subcommand; `record` writes only `snapshots/<rev>.json` + merges `index.json`.

- [ ] **Step 1: Confirm nothing else references `html`**

Run: `grep -rn "html\|regenerate\|viewer" metrics-probe/src metrics-probe/tests scripts 2>/dev/null`
Expected: matches only in `main.rs` (`mod html`, `html::regenerate`, the `html` subcommand + usage/doc lines), `html.rs` itself, `scripts/ci-backfill.sh`, and comments in `ci-metrics.sh`. No test references. If a test references `html`, STOP and report.

- [ ] **Step 2: Delete the viewer dir and html.rs**

```bash
git rm -r metrics-probe/viewer metrics-probe/src/html.rs
```

- [ ] **Step 3: Edit `metrics-probe/src/main.rs`** — apply these exact removals:

Remove the doc line (in the `//!` block near the top):
```rust
//! cargo run -p metrics-probe -- html           # regenerate metrics/index.html viewer
```

Remove the module declaration:
```rust
mod html;
```

In `cmd_record`, remove the regenerate call (the line is `    html::regenerate(&dir)?;` immediately before `    Ok(())`):
```rust
    html::regenerate(&dir)?;
```

Remove the subcommand arm:
```rust
        Some("html") => html::regenerate(Path::new(SNAPSHOT_DIR)),
```

In the `usage()` string, remove the `  metrics-probe html\n` fragment and the `html` mention so it reads (the surrounding lines stay):
```
"usage:\n  metrics-probe record [--sizes] [--benches]\n  metrics-probe diff [--sizes] [--benches] <rev|file>\n  metrics-probe index\n  metrics-probe hook-install\n\n  record     snapshot HEAD; also merges HEAD into metrics/index.json (ordering)\n  index      rebuild metrics/index.json for every snapshot rev from git history (WS0.9e backfill finalize)\n  --sizes    also build the thumb size-probes and record .text/.rodata/.bss (Layer 2, slower)\n  --benches  also read criterion medians from target/criterion (run `cargo bench` first; WS0.9d)"
```

- [ ] **Step 4: Edit `scripts/ci-backfill.sh`** — remove the viewer-regeneration step (around line 121-123):

Delete these lines:
```bash
# (backfill runs at fetch-depth 0), then regenerate the viewer with HEAD's tool.
```
and
```bash
cargo run -q -p metrics-probe -- html
```
(Keep the surrounding index-rebuild logic. If the comment line is part of a larger sentence, trim only the "then regenerate the viewer with HEAD's tool" clause.)

- [ ] **Step 5: Edit `scripts/ci-metrics.sh` comments** — replace the two mentions of regenerating the dashboard. Change the header comment `record a metrics snapshot for HEAD over the accumulated history and regenerate the dashboard.` to `record a metrics snapshot for HEAD over the accumulated history.` and change `regenerates metrics/index.html over every snapshot present — so pulling the history in first makes the dashboard cover the whole timeline.` to `writes metrics/snapshots/<rev>.json keyed by HEAD; the site assembles + charts the store.`

- [ ] **Step 6: Build + test metrics-probe**

Run: `cargo build -p metrics-probe 2>&1 | tail -5`
Expected: compiles (no `html`/unused-import errors).
Run: `cargo test -p metrics-probe --features layout-counters -- --test-threads=1 2>&1 | tail -8`
Expected: same pass count as baseline (no test referenced `html`), green.

- [ ] **Step 7: Confirm `record` still works without html**

Run: `cargo run -q -p metrics-probe -- record 2>&1 | tail -3 && ls metrics/ 2>/dev/null`
Expected: prints "recorded metrics/snapshots/<rev>.json (...)"; `metrics/` contains `snapshots/` (+ maybe `index.json`) but **no `index.html`**. (This writes to your local git-ignored store — fine.)

- [ ] **Step 8: Commit**

```bash
git add -A metrics-probe scripts/ci-backfill.sh scripts/ci-metrics.sh
git commit -m "WS19.7: remove the standalone metrics-probe HTML viewer

The site is now the single metrics home. Deletes metrics-probe/viewer/ +
src/html.rs and the html subcommand/record-hook; drops the viewer-regen step
from ci-backfill.sh; refreshes ci-metrics.sh comments. record/diff/index/
hook-install + the local store + metrics-data writes are unchanged.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Shared `assemble()` + dev-server local-store loader

**Files:**
- Create: `site/.vitepress/theme/lib/assemble.ts`
- Create: `site/.vitepress/theme/lib/assemble.test.ts`
- Modify: `site/scripts/assemble-metrics.ts` (use the shared fn)
- Modify: `site/.vitepress/config.mts` (add a dev-only Vite plugin)

**Interfaces:**
- Consumes: `types.ts` (`MetricsData`, `Snapshot`, `IndexMap`).
- Produces: `assemble(index: IndexMap, snapshots: Snapshot[]): MetricsData` (topo/date-sorted, pure). Dev server serves `GET <base>metrics/data.json` from the repo-root `metrics/` store.

- [ ] **Step 1: Write the failing test — `site/.vitepress/theme/lib/assemble.test.ts`**

```ts
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
```

- [ ] **Step 2: Run it — verify it fails**

Run: `npm test --prefix site -- assemble 2>&1 | tail -6`
Expected: FAIL — cannot resolve `./assemble`.

- [ ] **Step 3: Create `site/.vitepress/theme/lib/assemble.ts`**

```ts
import type { MetricsData, Snapshot, IndexMap } from './types'

// Turn a raw index + snapshot set into the {snapshots, index} the dashboard
// consumes, ordered oldest→newest. History order = index date, then recorded_at,
// then rev for stability (backfilled snapshots can share a wall-clock). Pure.
export function assemble(index: IndexMap, snapshots: Snapshot[]): MetricsData {
  const dateOf = (s: Snapshot) => index[s.git_rev]?.date ?? s.recorded_at ?? 0
  const sorted = [...snapshots].sort(
    (a, b) => dateOf(a) - dateOf(b) || a.git_rev.localeCompare(b.git_rev),
  )
  return { snapshots: sorted, index }
}

// Node-only: read a metrics-data-style directory (index.json + snapshots/*.json)
// and assemble it. Used by the CI script and the dev-server plugin. Kept here so
// there is ONE ordering implementation. Dynamically imports node:fs so importing
// this module in the browser bundle (for `assemble`) stays safe.
export async function assembleFromDir(dir: string): Promise<MetricsData> {
  const { readFileSync, readdirSync, existsSync } = await import('node:fs')
  const { join } = await import('node:path')
  const indexPath = join(dir, 'index.json')
  const index: IndexMap = existsSync(indexPath)
    ? (JSON.parse(readFileSync(indexPath, 'utf8')) as IndexMap)
    : {}
  const snapDir = join(dir, 'snapshots')
  const snapshots: Snapshot[] = existsSync(snapDir)
    ? readdirSync(snapDir)
        .filter((f) => f.endsWith('.json'))
        .map((f) => JSON.parse(readFileSync(join(snapDir, f), 'utf8')) as Snapshot)
    : []
  return assemble(index, snapshots)
}
```

- [ ] **Step 4: Run the test — verify it passes**

Run: `npm test --prefix site -- assemble 2>&1 | tail -6`
Expected: PASS (2 cases).

- [ ] **Step 5: Refactor `site/scripts/assemble-metrics.ts` to reuse `assemble`** — replace its body's sort with the shared fn. New file content:

```ts
// CI glue (run via tsx): read a checkout of the metrics-data branch and emit ONE
// history-ordered data.json (the {snapshots,index} contract) into the VitePress
// dist, plus copies of the raw sources for transparency.
import {
  readFileSync, readdirSync, writeFileSync, mkdirSync, copyFileSync, existsSync,
} from 'node:fs'
import { join } from 'node:path'
import { assemble } from '../.vitepress/theme/lib/assemble'
import type { Snapshot, IndexMap } from '../.vitepress/theme/lib/types'

const [srcDir, outDir] = process.argv.slice(2)
if (!srcDir || !outDir) {
  console.error('usage: assemble-metrics <metrics-data-dir> <out-dir>')
  process.exit(1)
}

const indexPath = join(srcDir, 'index.json')
const index: IndexMap = existsSync(indexPath)
  ? (JSON.parse(readFileSync(indexPath, 'utf8')) as IndexMap)
  : {}
const snapDir = join(srcDir, 'snapshots')
const snapFiles = existsSync(snapDir)
  ? readdirSync(snapDir).filter((f) => f.endsWith('.json'))
  : []
const snapshots: Snapshot[] = snapFiles.map(
  (f) => JSON.parse(readFileSync(join(snapDir, f), 'utf8')) as Snapshot,
)

const data = assemble(index, snapshots)
mkdirSync(outDir, { recursive: true })
writeFileSync(join(outDir, 'data.json'), JSON.stringify(data))

if (existsSync(indexPath)) copyFileSync(indexPath, join(outDir, 'index.json'))
if (snapFiles.length) {
  mkdirSync(join(outDir, 'snapshots'), { recursive: true })
  for (const f of snapFiles) copyFileSync(join(snapDir, f), join(outDir, 'snapshots', f))
}

console.log(`assembled ${data.snapshots.length} snapshots -> ${join(outDir, 'data.json')}`)
```

- [ ] **Step 6: Add the dev-server plugin to `site/.vitepress/config.mts`** — import at top and add a `vite.plugins` entry. Insert after the existing `import { defineConfig } from 'vitepress'`:

```ts
import type { Plugin } from 'vite'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

// Dev-only: serve /metrics/data.json from the repo-root local `metrics/` store
// (git-ignored, machine-local numbers) so `docs:dev` charts real local data.
// Production serves the static dist/metrics/data.json assembled by site.yml.
function localMetricsPlugin(): Plugin {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '../../..') // repo root from site/.vitepress/
  return {
    name: 'rsact-local-metrics',
    apply: 'serve',
    configureServer(server) {
      server.middlewares.use(async (req, res, next) => {
        if (!req.url || !/\/metrics\/data\.json(\?|$)/.test(req.url)) return next()
        try {
          const { assembleFromDir } = await import('./theme/lib/assemble')
          const data = await assembleFromDir(resolve(root, 'metrics'))
          if (!data.snapshots.length) {
            const { SAMPLE } = await import('./theme/lib/sample')
            res.setHeader('content-type', 'application/json')
            res.end(JSON.stringify(SAMPLE))
            return
          }
          res.setHeader('content-type', 'application/json')
          res.end(JSON.stringify(data))
        } catch (e) {
          console.error('local-metrics dev plugin failed', e)
          next()
        }
      })
    },
  }
}
```

Then add `vite: { plugins: [localMetricsPlugin()] },` as a top-level key of the `defineConfig({...})` object (e.g. immediately before `themeConfig`).

- [ ] **Step 7: Verify build still green + assembly script still works**

Run: `npm run docs:build --prefix site 2>&1 | tail -3`
Expected: exit 0.
Run: `cd /Users/as/dev/rsact-ws19 && rm -rf /tmp/mv2 && mkdir -p /tmp/mv2 && git archive origin/metrics-data | tar -x -C /tmp/mv2 && ./site/node_modules/.bin/tsx site/scripts/assemble-metrics.ts /tmp/mv2 /tmp/mv2out/metrics 2>&1 | tail -2`
Expected: `assembled N snapshots -> /tmp/mv2out/metrics/data.json` (N ≥ 1).

- [ ] **Step 8: Manually verify dev-mode local-store load**

Run (record a local snapshot if the store is empty, then start dev and curl the endpoint):
```bash
cd /Users/as/dev/rsact-ws19
cargo run -q -p metrics-probe -- record >/dev/null 2>&1 || true
( cd site && npm run docs:dev >/tmp/mv2dev.log 2>&1 & echo $! > /tmp/mv2dev.pid ; sleep 6 ; \
  curl -s http://localhost:5173/rsact/metrics/data.json | head -c 120 ; \
  kill "$(cat /tmp/mv2dev.pid)" 2>/dev/null )
```
Expected: JSON beginning `{"snapshots":[...` (local store data, or the sample fixture if the local store is empty). (Port may differ — check `/tmp/mv2dev.log` for the actual dev URL; adjust.)

- [ ] **Step 9: Commit**

```bash
git add site/.vitepress/theme/lib/assemble.ts site/.vitepress/theme/lib/assemble.test.ts \
  site/scripts/assemble-metrics.ts site/.vitepress/config.mts
git commit -m "WS19.7: shared assemble() + dev-server local-store loader

One ordering impl (assemble) feeds both the CI data.json script and a dev-only
Vite plugin that serves /metrics/data.json from the local git-ignored metrics/
store (empty → sample). Prod fetch path unchanged; docs:dev now charts local
numbers — the capability the removed standalone viewer provided.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: `collapse.ts` — global unchanged-commit collapse (TDD, pure)

**Files:**
- Create: `site/.vitepress/theme/lib/collapse.ts`
- Create: `site/.vitepress/theme/lib/collapse.test.ts`

**Interfaces:**
- Consumes: `series.ts` `prevPresent`; `types.ts` `SeriesRow`, `Snapshot`.
- Produces:
  - `columnGroups(rows: SeriesRow[], n: number): number[][]` — arrays of snapshot indices per collapsed column (identity `[[0],[1],…]` semantics come from the caller when disabled).
  - `collapseValues(values: (number|null)[], groups: number[][]): (number|null)[]` — one value per group (the group's first present value, else null).
  - `columnLabel(snapshots: Snapshot[], group: number[]): string` — single → 8-char rev (+`*` if dirty); run → `<first8>..<last8>`.

- [ ] **Step 1: Write the failing test — `collapse.test.ts`**

```ts
import { describe, it, expect } from 'vitest'
import { columnGroups, collapseValues, columnLabel } from './collapse'
import type { SeriesRow, Snapshot } from './types'

const row = (values: (number | null)[]): SeriesRow => ({ key: 'k', label: 'k', values })
const snaps = (revs: string[]): Snapshot[] =>
  revs.map((r) => ({ git_rev: r.repeat(8), git_dirty: false, recorded_at: 0, scenarios: [] }))

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
```

- [ ] **Step 2: Run it — verify it fails**

Run: `npm test --prefix site -- collapse 2>&1 | tail -6`
Expected: FAIL — cannot resolve `./collapse`.

- [ ] **Step 3: Create `site/.vitepress/theme/lib/collapse.ts`**

```ts
import { prevPresent } from './series'
import type { SeriesRow, Snapshot } from './types'

// A commit index i (i>0) is a boundary iff, for some row, its PRESENT value at i
// differs from that row's previous present value (gaps skipped). A null is never
// a boundary — it carries forward. This preserves a real change across a gap
// (e.g. [10, null, 20] splits at 20) while absorbing measurement gaps.
export function columnGroups(rows: SeriesRow[], n: number): number[][] {
  const groups: number[][] = []
  let cur: number[] = []
  for (let i = 0; i < n; i++) {
    const boundary =
      i > 0 &&
      rows.some((r) => {
        const v = r.values[i]
        if (v === null || v === undefined) return false
        const prev = prevPresent(r.values, i)
        return prev !== null && v !== prev
      })
    if (boundary) {
      groups.push(cur)
      cur = []
    }
    cur.push(i)
  }
  if (cur.length) groups.push(cur)
  return groups
}

// One value per group: the group's first present value, else null (a metric
// absent throughout the run). Within a group present values are all equal by the
// boundary rule, so "first present" is the run's representative value.
export function collapseValues(
  values: (number | null)[],
  groups: number[][],
): (number | null)[] {
  return groups.map((g) => {
    for (const i of g) {
      const v = values[i]
      if (v !== null && v !== undefined) return v
    }
    return null
  })
}

const rev8 = (s: Snapshot) => s.git_rev.slice(0, 8) + (s.git_dirty ? '*' : '')

// Single-commit column → its 8-char rev; a collapsed run → `<first8>..<last8>`.
export function columnLabel(snapshots: Snapshot[], group: number[]): string {
  if (group.length === 1) return rev8(snapshots[group[0]])
  const first = snapshots[group[0]]
  const last = snapshots[group[group.length - 1]]
  return `${first.git_rev.slice(0, 8)}..${last.git_rev.slice(0, 8)}`
}
```

- [ ] **Step 4: Run the test — verify it passes**

Run: `npm test --prefix site -- collapse 2>&1 | tail -6`
Expected: PASS (all cases).

- [ ] **Step 5: Commit**

```bash
git add site/.vitepress/theme/lib/collapse.ts site/.vitepress/theme/lib/collapse.test.ts
git commit -m "WS19.7: collapse.ts — global unchanged-commit collapse (prevPresent boundaries)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: `colors.ts` — stable per-metric colors (TDD, pure)

**Files:**
- Create: `site/.vitepress/theme/lib/colors.ts`
- Create: `site/.vitepress/theme/lib/colors.test.ts`

**Interfaces:**
- Produces: `colorFor(key: string): string` — deterministic hex from a 16-color palette; `PALETTE: readonly string[]`.

- [ ] **Step 1: Write the failing test — `colors.test.ts`**

```ts
import { describe, it, expect } from 'vitest'
import { colorFor, PALETTE } from './colors'

describe('colorFor', () => {
  it('is deterministic for a given key', () => {
    expect(colorFor('ui_labels_10/nodes_total')).toBe(colorFor('ui_labels_10/nodes_total'))
  })
  it('always returns a palette color', () => {
    for (const k of ['a', 'b', 'bench:reactivity/signal_read', 'size:ui/.text']) {
      expect(PALETTE).toContain(colorFor(k))
    }
  })
  it('spreads distinct keys across more than one color', () => {
    const keys = Array.from({ length: 20 }, (_, i) => `metric_${i}`)
    const used = new Set(keys.map(colorFor))
    expect(used.size).toBeGreaterThan(3)
  })
})
```

- [ ] **Step 2: Run it — verify it fails**

Run: `npm test --prefix site -- colors 2>&1 | tail -6`
Expected: FAIL — cannot resolve `./colors`.

- [ ] **Step 3: Create `site/.vitepress/theme/lib/colors.ts`**

```ts
// A metric key maps to the SAME color everywhere and every session, so a reader
// learns "flash size is teal" once. Deterministic hash → a 16-color qualitative
// palette (Tableau-20 subset + extras). Collisions only matter when two colliding
// metrics are co-selected — rare, and the legend disambiguates.
export const PALETTE: readonly string[] = [
  '#4e79a7', '#f28e2c', '#e15759', '#76b7b2', '#59a14f', '#edc949',
  '#af7aa1', '#ff9da7', '#9c755f', '#bab0ab', '#1f77b4', '#ff7f0e',
  '#2ca02c', '#d62728', '#9467bd', '#8c564b',
]

// FNV-1a (32-bit) — stable across runs/machines, unlike Math.random or insertion order.
function hash(key: string): number {
  let h = 0x811c9dc5
  for (let i = 0; i < key.length; i++) {
    h ^= key.charCodeAt(i)
    h = Math.imul(h, 0x01000193)
  }
  return h >>> 0
}

export function colorFor(key: string): string {
  return PALETTE[hash(key) % PALETTE.length]
}
```

- [ ] **Step 4: Run the test — verify it passes**

Run: `npm test --prefix site -- colors 2>&1 | tail -6`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add site/.vitepress/theme/lib/colors.ts site/.vitepress/theme/lib/colors.test.ts
git commit -m "WS19.7: colors.ts — deterministic stable per-metric colors (FNV-1a → palette)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: chart.ts cell-centered geometry + series.ts Δ/only-changed + hover key (TDD)

**Files:**
- Modify: `site/.vitepress/theme/lib/chart.ts` (add cell-centered `xOf`; update `shapes` to use it)
- Modify: `site/.vitepress/theme/lib/chart.test.ts` (update the geometry expectation)
- Modify: `site/.vitepress/theme/lib/series.ts` (add `deltaValues`, `isFlat`)
- Modify: `site/.vitepress/theme/lib/series.test.ts` (add cases)
- Create: `site/.vitepress/theme/lib/hover.ts` (crosshair injection key)

**Interfaces:**
- Produces:
  - `chart.ts`: `xOf(i, n, width, pad)` now returns the **center of cell i**: `pad + (width - 2*pad) * (i + 0.5) / n`. `shapes`/`seriesMax`/`segments`/`yOf` unchanged in signature.
  - `series.ts`: `deltaValues(values: (number|null)[]): (number|null)[]` (each present value minus the first present value; first present → 0; gaps stay null); `isFlat(values: (number|null)[]): boolean` (no present value differs from the previous present — i.e. never changes).
  - `hover.ts`: `HOVER_KEY: InjectionKey<Ref<number | null>>`.

- [ ] **Step 1: Update `chart.test.ts`** — replace the normalization test's expectation (endpoints → cell centers). Replace the `shapes` describe's third case with:

```ts
  it('centers point i in cell i: x = pad + (w-2pad)*(i+0.5)/n', () => {
    const { polys } = shapes([10, 20], { n: 2, width: 400, height: 100, pad: 10, max: 100 })
    const xs = polys[0].points.split(' ').map((p) => Number(p.split(',')[0]))
    // cell 0 center = 10 + 380*0.25 = 105; cell 1 center = 10 + 380*0.75 = 295
    expect(xs[0]).toBeCloseTo(105, 1)
    expect(xs[1]).toBeCloseTo(295, 1)
  })
  it('normalizes y to the provided max (top hugs the top pad)', () => {
    const { polys } = shapes([50, 100], { n: 2, width: 400, height: 100, pad: 10, max: 100 })
    const y1 = Number(polys[0].points.split(' ')[1].split(',')[1])
    expect(y1).toBeCloseTo(10, 1) // v=max → y at top pad
  })
```

- [ ] **Step 2: Run it — verify the geometry test fails**

Run: `npm test --prefix site -- chart 2>&1 | tail -8`
Expected: FAIL on the new "centers point i" case (current `xOf` uses `i/(n-1)` → x0=10, x1=390, not 105/295).

- [ ] **Step 3: Update `xOf` in `site/.vitepress/theme/lib/chart.ts`** — replace the function:

```ts
// x for commit slot i of n, at the CENTER of cell i within [pad, width-pad].
// Cell-centered (not endpoint-anchored) so points sit dead-center under their
// equal-width table columns (fixed-layout table).
export function xOf(i: number, n: number, width: number, pad: number): number {
  return pad + (n <= 0 ? 0 : ((i + 0.5) / n) * (width - 2 * pad))
}
```

- [ ] **Step 4: Run it — verify chart tests pass**

Run: `npm test --prefix site -- chart 2>&1 | tail -6`
Expected: PASS (segments/seriesMax/shapes incl. the new centering + normalization cases).

- [ ] **Step 5: Add `deltaValues` + `isFlat` to `series.ts`** — append after `revLabel`:

```ts
// Each present value minus the first present value (baseline). The first present
// value becomes 0; gaps stay null. For the Δ-from-baseline view.
export function deltaValues(values: (number | null)[]): (number | null)[] {
  const base = prevPresentOrFirst(values)
  if (base === null) return values.map(() => null)
  return values.map((v) => (v === null || v === undefined ? null : v - base))
}

// The first present value in the series (baseline), or null if all-gap.
function prevPresentOrFirst(values: (number | null)[]): number | null {
  for (const v of values) if (v !== null && v !== undefined) return v
  return null
}

// True if no present value ever differs from the previous present value — the
// series never actually changes (for the "only changed" filter).
export function isFlat(values: (number | null)[]): boolean {
  for (let i = 0; i < values.length; i++) {
    const v = values[i]
    if (v === null || v === undefined) continue
    const prev = prevPresent(values, i)
    if (prev !== null && v !== prev) return false
  }
  return true
}
```

- [ ] **Step 6: Add cases to `series.test.ts`** — append a describe:

```ts
import { deltaValues, isFlat } from './series'

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
})
```

- [ ] **Step 7: Create `site/.vitepress/theme/lib/hover.ts`**

```ts
import type { InjectionKey, Ref } from 'vue'

// The synchronized-crosshair column index (or null). MetricsDashboard provides
// it; MetricTable + TrendChart inject it to highlight the hovered column and
// draw the guide, so hovering one commit lights it up everywhere at once.
export const HOVER_KEY: InjectionKey<Ref<number | null>> = Symbol('rsact-metrics-hover')
```

- [ ] **Step 8: Run the full site test suite**

Run: `npm test --prefix site 2>&1 | tail -8`
Expected: PASS (all lib suites green, including updated chart + new series cases).

- [ ] **Step 9: Commit**

```bash
git add site/.vitepress/theme/lib/chart.ts site/.vitepress/theme/lib/chart.test.ts \
  site/.vitepress/theme/lib/series.ts site/.vitepress/theme/lib/series.test.ts \
  site/.vitepress/theme/lib/hover.ts
git commit -m "WS19.7: cell-centered chart geometry + Δ/only-changed series helpers + hover key

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: `TrendChart.vue` — cell-centered points already in; add crosshair guide

**Files:**
- Modify: `site/.vitepress/theme/components/TrendChart.vue`

**Interfaces:**
- Consumes: `chart.ts` (cell-centered `xOf` — already updated in Task 5), `hover.ts` `HOVER_KEY`.
- Produces: a chart that, when a shared hover column is set, draws its guide at that column (in addition to its own interactive hover).

- [ ] **Step 1: Inject the shared hover ref** — in `TrendChart.vue` `<script setup lang="ts">`, add to the imports and after the props:

Add import:
```ts
import { computed, ref, inject } from 'vue'
import { HOVER_KEY } from '../lib/hover'
```
(Replace the existing `import { computed, ref } from 'vue'` line.)

After the `hover` ref definition, add:
```ts
// Shared crosshair column (synchronized across all tables/charts), if provided.
const sharedHover = inject(HOVER_KEY, ref<number | null>(null))
// The guide shows either this chart's own hovered column or the shared one.
const guideCol = computed(() => hover.value ?? sharedHover.value)
```

- [ ] **Step 2: Update the guide line to use `guideCol`** — replace the `hoverX` computed:

```ts
const hoverX = computed(() =>
  guideCol.value === null || guideCol.value === undefined
    ? null
    : xOf(guideCol.value, n.value, props.width, props.pad),
)
```

And in `onMove`, also publish to the shared ref so hovering this chart lights up the tables:
```ts
function onMove(ev: MouseEvent) {
  if (!props.interactive || !n.value) return
  const rect = (ev.currentTarget as SVGSVGElement).getBoundingClientRect()
  const mx = (ev.clientX - rect.left) * (props.width / rect.width)
  const frac = n.value <= 1 ? 0 : (mx - props.pad) / (props.width - 2 * props.pad)
  hover.value = Math.max(0, Math.min(n.value - 1, Math.round(frac * (n.value - 1))))
  sharedHover.value = hover.value
}
```
And on `@mouseleave` clear both: change the template handler to `@mouseleave="hover = null; sharedHover = null"`.

- [ ] **Step 3: Mount test — `site/.vitepress/theme/components/TrendChart.test.ts`** (new):

```ts
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import { ref } from 'vue'
import TrendChart from './TrendChart.vue'
import { HOVER_KEY } from '../lib/hover'

describe('TrendChart', () => {
  it('draws a guide at the shared hover column when provided', () => {
    const hover = ref<number | null>(2)
    const w = mount(TrendChart, {
      props: { series: [{ label: 'x', values: [1, 2, 3, 4], color: '#000' }], height: 56 },
      global: { provide: { [HOVER_KEY as symbol]: hover } },
    })
    expect(w.find('line.guide').exists()).toBe(true)
  })
  it('no guide when shared hover is null and not interacting', () => {
    const hover = ref<number | null>(null)
    const w = mount(TrendChart, {
      props: { series: [{ label: 'x', values: [1, 2, 3], color: '#000' }] },
      global: { provide: { [HOVER_KEY as symbol]: hover } },
    })
    expect(w.find('line.guide').exists()).toBe(false)
  })
})
```

- [ ] **Step 4: Run tests + build**

Run: `npm test --prefix site -- TrendChart 2>&1 | tail -6`
Expected: PASS.
Run: `npm run docs:build --prefix site 2>&1 | tail -3`
Expected: exit 0.

- [ ] **Step 5: Commit**

```bash
git add site/.vitepress/theme/components/TrendChart.vue site/.vitepress/theme/components/TrendChart.test.ts
git commit -m "WS19.7: TrendChart cell-centered points + synchronized crosshair guide

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: `MetricTable.vue` — fixed/sticky layout, collapsed columns, crosshair, Δ

**Files:**
- Modify: `site/.vitepress/theme/components/MetricTable.vue`

**Interfaces:**
- Consumes: props now include the collapsed column model + display mode from the dashboard.
- Produces: a fixed-layout table with a sticky, uniform-width first column; commit columns from the collapsed model; inline chart aligned to the commit columns (empty first cell); crosshair highlight; Δ display when enabled.

- [ ] **Step 1: Replace `MetricTable.vue`** with the version below (props extended for collapsed columns + mode + hover; template uses `columns` not raw `snapshots`; inline chart row gets an empty leading cell). Full file:

```vue
<script setup lang="ts">
import { computed, inject, ref } from 'vue'
import TrendChart from './TrendChart.vue'
import { trend, fmt, lowerIsBetter, deltaValues } from '../lib/series'
import { collapseValues } from '../lib/collapse'
import { colorFor } from '../lib/colors'
import { HOVER_KEY } from '../lib/hover'
import type { SeriesGroup } from '../lib/types'

// A "column" is a collapsed group of commit indices with a display label.
interface Column { label: string; title: string; group: number[] }

const props = defineProps<{
  group: SeriesGroup
  columns: Column[]
  selected: Set<string>
  delta: boolean
}>()
defineEmits<{ toggle: [key: string] }>()

const sharedHover = inject(HOVER_KEY, ref<number | null>(null))

// Per row: collapse the full values to per-column values, optionally Δ, and
// precompute the ▲/▼ marker vs the previous present column value.
const rows = computed(() =>
  props.group.rows.map((r) => {
    const collapsed = collapseValues(r.values, props.columns.map((c) => c.group))
    const shown = props.delta ? deltaValues(collapsed) : collapsed
    return {
      ...r,
      shown,
      cells: shown.map((v, i) => ({ v, mark: trend(collapsed, i, lowerIsBetter(r.key)) })),
    }
  }),
)
</script>

<template>
  <table>
    <caption>{{ group.title }}</caption>
    <thead>
      <tr>
        <th class="lbl">metric</th>
        <th
          v-for="(c, i) in columns"
          :key="c.title"
          :title="c.title"
          :class="{ hov: sharedHover === i }"
          @mouseenter="sharedHover = i"
          @mouseleave="sharedHover = null"
        >
          {{ c.label }}
        </th>
      </tr>
    </thead>
    <tbody>
      <template v-for="row in rows" :key="row.key">
        <tr class="metric" :class="{ sel: selected.has(row.key) }" @click="$emit('toggle', row.key)">
          <td class="lbl">
            <span
              class="swatch"
              :style="{
                background: colorFor(row.key),
                visibility: selected.has(row.key) ? 'visible' : 'hidden',
              }"
            ></span>
            {{ row.label }}
          </td>
          <td
            v-for="(cell, i) in row.cells"
            :key="i"
            :class="{ hov: sharedHover === i }"
            @mouseenter="sharedHover = i"
            @mouseleave="sharedHover = null"
          >
            <span v-if="cell.v === null" class="muted">–</span>
            <template v-else
              >{{ delta && cell.v > 0 ? '+' : '' }}{{ fmt(cell.v) }}<span
                v-if="cell.mark"
                :class="cell.mark"
                >{{ cell.mark === 'up' ? ' ▲' : ' ▼' }}</span
              ></template
            >
          </td>
        </tr>
        <tr v-if="selected.has(row.key)" class="chartrow">
          <td class="lbl"></td>
          <td :colspan="columns.length">
            <TrendChart
              :series="[{ label: row.label, values: row.shown, color: colorFor(row.key) }]"
              :n="columns.length"
              :height="56"
              :pad="4"
              :show-dots="true"
            />
          </td>
        </tr>
      </template>
    </tbody>
  </table>
</template>

<style scoped lang="scss">
table { border-collapse: collapse; margin: 0.3rem 0 1.4rem; font-family: var(--vp-font-family-mono); font-size: 12px; table-layout: fixed; }
caption { text-align: left; font-weight: bold; margin-bottom: 0.3rem; }
th, td {
  border: 1px solid var(--vp-c-divider);
  padding: 0.15rem 0.5rem; text-align: right; white-space: nowrap;
  width: 5.5rem; overflow: hidden; text-overflow: ellipsis;
}
// Uniform, readable, STICKY first column so it stays visible when scrolled and
// lines up across every table (shared width var).
th.lbl, td.lbl {
  text-align: left; position: sticky; left: 0; z-index: 1;
  width: var(--metric-col-w, 13rem); min-width: var(--metric-col-w, 13rem);
  background: var(--vp-c-bg);
}
th.hov, td.hov { background: var(--vp-c-bg-soft); }
tr.metric { cursor: pointer; }
tr.metric:hover td { background: var(--vp-c-bg-soft); }
tr.metric.sel td.lbl { font-weight: bold; }
tr.chartrow td { padding: 0.2rem 0.4rem; }
tr.chartrow td.lbl { background: var(--vp-c-bg); }
.muted { color: var(--vp-c-text-3); }
.up { color: #2e9e4f; }
.down { color: #d64545; }
.swatch {
  display: inline-block; width: 0.6rem; height: 0.6rem;
  border-radius: 2px; margin-right: 0.35rem; vertical-align: middle;
}
</style>
```

Note: `TrendChart` now takes an explicit `n` prop (column count) instead of `snapshots`. Add `n?: number` to `TrendChart`'s props with default `0`, and change its `n` computed to `props.n || props.snapshots.length || Math.max(...)`. (Do this small `TrendChart` prop addition here so the inline chart aligns to the collapsed column count.)

- [ ] **Step 2: Add the `n` prop to `TrendChart.vue`** — in its `withDefaults(defineProps<{…}>(), {…})`, add `n?: number` to the type and `n: 0` to defaults; change the `n` computed to:

```ts
const n = computed(
  () => props.n || props.snapshots.length || Math.max(0, ...props.series.map((s) => s.values.length)),
)
```

- [ ] **Step 3: Mount test — `site/.vitepress/theme/components/MetricTable.test.ts`** (new):

```ts
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import MetricTable from './MetricTable.vue'
import type { SeriesGroup } from '../lib/types'

const group: SeriesGroup = {
  title: 'ui_labels_10',
  rows: [
    { key: 'ui/nodes', label: 'nodes', values: [100, 80, 80] },
    { key: 'ui/heap', label: 'heap', values: [21000, 21000, 21000] },
  ],
}
const columns = [
  { label: 'aaaaaaaa', title: 'aaaaaaaa', group: [0] },
  { label: 'bbbbbbbb..cccccccc', title: 'run', group: [1, 2] },
]

describe('MetricTable', () => {
  it('renders collapsed columns + a header per column', () => {
    const w = mount(MetricTable, { props: { group, columns, selected: new Set(), delta: false } })
    const ths = w.findAll('thead th')
    expect(ths).toHaveLength(3) // metric + 2 columns
    expect(ths[2].text()).toBe('bbbbbbbb..cccccccc')
  })
  it('collapses values to one per column (100, then 80)', () => {
    const w = mount(MetricTable, { props: { group, columns, selected: new Set(), delta: false } })
    const firstRow = w.findAll('tbody tr.metric')[0]
    const cells = firstRow.findAll('td')
    expect(cells[1].text()).toContain('100')
    expect(cells[2].text()).toContain('80')
  })
  it('delta mode shows +/− from baseline', () => {
    const w = mount(MetricTable, { props: { group, columns, selected: new Set(), delta: true } })
    const cells = w.findAll('tbody tr.metric')[0].findAll('td')
    expect(cells[1].text()).toContain('0')   // baseline
    expect(cells[2].text()).toContain('−20') // 80-100  (uses minus sign via fmt? see note)
  })
  it('inline chart row has an empty leading cell (aligns to commit columns)', async () => {
    const w = mount(MetricTable, { props: { group, columns, selected: new Set(['ui/nodes']), delta: false } })
    const chartRow = w.find('tr.chartrow')
    expect(chartRow.exists()).toBe(true)
    expect(chartRow.findAll('td')[0].classes()).toContain('lbl')
    expect(chartRow.find('.trendchart').exists()).toBe(true)
  })
})
```

Note on the delta test: `fmt` uses `toLocaleString()`, which renders negatives with an ASCII `-`, not `−`. Change the delta test assertion to `toContain('-20')` (ASCII) to match `fmt`, OR keep the display as `fmt` produces. Use whichever the implementation yields — verify against the actual rendered text and pin the test to it (do not invent a minus sign the code doesn't produce).

- [ ] **Step 4: Run tests + build**

Run: `npm test --prefix site -- MetricTable 2>&1 | tail -8`
Expected: PASS (adjust the delta assertion to the real `fmt` minus glyph as noted).
Run: `npm run docs:build --prefix site 2>&1 | tail -3`
Expected: exit 0.

- [ ] **Step 5: Commit**

```bash
git add site/.vitepress/theme/components/MetricTable.vue site/.vitepress/theme/components/TrendChart.vue \
  site/.vitepress/theme/components/MetricTable.test.ts
git commit -m "WS19.7: MetricTable fixed/sticky layout + collapsed columns + crosshair + Δ + stable colors

table-layout:fixed with a sticky uniform-width first column (shared --metric-col-w
so tables align); commit columns come from the collapsed model; inline chart row
has an empty leading cell so points align to commit columns; swatch/line use the
stable colorFor(); Δ mode shows deltas; column hover syncs via HOVER_KEY.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 8: `MetricsDashboard.vue` — orchestrate collapse, colors, Δ, only-changed, crosshair, URL state

**Files:**
- Modify: `site/.vitepress/theme/components/MetricsDashboard.vue`

**Interfaces:**
- Consumes: `collapse.ts` (`columnGroups`, `columnLabel`), `colors.ts` (`colorFor`), `series.ts` (`buildSeries`, `isFlat`), `hover.ts` (`HOVER_KEY`); child `MetricTable`/`TrendChart` props from Tasks 6-7.
- Produces: the full v2 dashboard: selection uses stable colors; global collapsed column axis (toggle); Δ toggle; only-changed toggle; provides the shared hover ref; selection + toggles round-trip through the URL hash.

- [ ] **Step 1: Replace `MetricsDashboard.vue`** with the version below (keeps the Task-4 data-load/SSR/`console.error` catch; replaces selection-color cycling with `colorFor`; adds collapse/Δ/only-changed/URL/crosshair). Full file:

```vue
<script setup lang="ts">
import { reactive, computed, ref, onMounted, provide, watch } from 'vue'
import { withBase } from 'vitepress'
import MetricTable from './MetricTable.vue'
import TrendChart from './TrendChart.vue'
import { buildSeries, isFlat } from '../lib/series'
import { columnGroups, columnLabel } from '../lib/collapse'
import { colorFor } from '../lib/colors'
import { HOVER_KEY } from '../lib/hover'
import { SAMPLE } from '../lib/sample'
import type { MetricsData, Snapshot, IndexMap, SeriesRow, Series } from '../lib/types'

const props = defineProps<{ data?: MetricsData }>()

const snapshots = ref<Snapshot[]>(props.data?.snapshots ?? [])
const index = ref<IndexMap>(props.data?.index ?? {})
const loading = ref(!props.data)

// UI state (also serialized to the URL hash).
const selected = reactive(new Set<string>())
const collapse = ref(true)
const delta = ref(false)
const onlyChanged = ref(false)

// Synchronized crosshair column, provided to every table + chart.
const hover = ref<number | null>(null)
provide(HOVER_KEY, hover)

function parseHash() {
  if (typeof location === 'undefined') return
  const p = new URLSearchParams(location.hash.replace(/^#/, ''))
  collapse.value = p.get('collapse') !== '0'
  delta.value = p.get('delta') === '1'
  onlyChanged.value = p.get('changed') === '1'
  const sel = p.get('sel')
  selected.clear()
  if (sel) for (const k of sel.split('~').filter(Boolean)) selected.add(decodeURIComponent(k))
}
function writeHash() {
  if (typeof location === 'undefined') return
  const p = new URLSearchParams()
  if (!collapse.value) p.set('collapse', '0')
  if (delta.value) p.set('delta', '1')
  if (onlyChanged.value) p.set('changed', '1')
  if (selected.size) p.set('sel', [...selected].map(encodeURIComponent).join('~'))
  const hash = p.toString()
  history.replaceState(null, '', hash ? `#${hash}` : location.pathname + location.search)
}

onMounted(async () => {
  parseHash()
  if (!props.data) {
    try {
      const res = await fetch(withBase('/metrics/data.json'))
      if (!res.ok) throw new Error(String(res.status))
      const d = (await res.json()) as MetricsData
      if (Array.isArray(d.snapshots)) {
        snapshots.value = d.snapshots
        index.value = d.index ?? {}
      }
    } catch (e) {
      console.error('rsact metrics: failed to load /metrics/data.json', e)
      if (import.meta.env.DEV) {
        snapshots.value = SAMPLE.snapshots
        index.value = SAMPLE.index
      }
    } finally {
      loading.value = false
    }
  }
  watch([selected, collapse, delta, onlyChanged], writeHash, { deep: true })
})

const groups = computed(() => buildSeries(snapshots.value))
const allRows = computed(() => groups.value.flatMap((g) => g.rows))

// The shared, collapsed column axis (identity when collapse is off).
const colGroups = computed<number[][]>(() =>
  collapse.value
    ? columnGroups(allRows.value, snapshots.value.length)
    : snapshots.value.map((_, i) => [i]),
)
const columns = computed(() =>
  colGroups.value.map((g) => {
    const label = columnLabel(snapshots.value, g)
    const branch = index.value[snapshots.value[g[0]]?.git_rev]?.branch
    return { label, title: branch ? `${label} (${branch})` : label, group: g }
  }),
)

// Optionally drop groups whose rows are all flat across the (collapsed) range.
function collapsedRowFlat(r: SeriesRow): boolean {
  const vals = colGroups.value.map((g) => {
    for (const i of g) { const v = r.values[i]; if (v !== null && v !== undefined) return v }
    return null
  })
  return isFlat(vals)
}
const shownGroups = computed(() =>
  onlyChanged.value
    ? groups.value
        .map((g) => ({ ...g, rows: g.rows.filter((r) => !collapsedRowFlat(r)) }))
        .filter((g) => g.rows.length)
    : groups.value,
)

const seriesByKey = computed(() => {
  const m = new Map<string, SeriesRow>()
  for (const g of groups.value) for (const r of g.rows) m.set(r.key, r)
  return m
})
function toggle(key: string) {
  if (selected.has(key)) selected.delete(key)
  else selected.add(key)
}
function selectAll() {
  for (const g of shownGroups.value) for (const r of g.rows) selected.add(r.key)
}
// Overlay series use the collapsed values so the sidepanel matches the tables.
const selectedSeries = computed<Series[]>(() =>
  [...selected].map((key) => {
    const r = seriesByKey.value.get(key)
    const collapsed = colGroups.value.map((g) => {
      for (const i of g) { const v = r?.values[i]; if (v !== null && v !== undefined) return v }
      return null
    })
    return { label: r?.label ?? key, values: collapsed, color: colorFor(key) }
  }),
)
</script>

<template>
  <div class="metrics">
    <p v-if="loading" class="muted">Loading metrics…</p>
    <p v-else-if="!snapshots.length" class="muted">
      No metrics data available yet. Record a snapshot (<code>metrics-probe record</code>) or push a commit.
    </p>
    <template v-else>
      <p class="muted intro">
        Per-commit trend, oldest → newest. Click a metric row to chart it; charted rows overlay in the
        right panel, each normalized to its own max — hover any column to sync the crosshair everywhere.
        <span class="up">▲</span> improved, <span class="down">▼</span> regressed (domain-aware). Gaps
        mean the metric wasn't measured — never zero. Unchanged commits collapse to
        <code>a..b</code> columns; bench medians are a ±noisy CI trend.
      </p>

      <div class="controls">
        <button @click="selectAll">select all</button>
        <button @click="selected.clear()">clear</button>
        <label><input type="checkbox" v-model="collapse" /> collapse unchanged</label>
        <label><input type="checkbox" v-model="delta" /> Δ from baseline</label>
        <label><input type="checkbox" v-model="onlyChanged" /> only changed</label>
        <span class="muted">{{ selected.size ? `${selected.size} charted` : 'no series selected' }}</span>
      </div>

      <div class="wrap">
        <div class="main">
          <MetricTable
            v-for="g in shownGroups"
            :key="g.title"
            :group="g"
            :columns="columns"
            :selected="selected"
            :delta="delta"
            @toggle="toggle"
          />
        </div>
        <div class="side">
          <h2>trend (selected)</h2>
          <TrendChart
            v-if="selected.size"
            :series="selectedSeries"
            :n="columns.length"
            :normalize="true"
            :interactive="true"
            :height="300"
          />
          <p v-else class="muted">select metric rows to overlay their trends</p>
          <div v-if="selected.size" class="legend">
            <div v-for="s in selectedSeries" :key="s.label" class="legend-item">
              <span class="swatch" :style="{ background: s.color }"></span>{{ s.label }}
            </div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped lang="scss">
.metrics { margin-top: 1rem; --metric-col-w: 13rem; }
.muted { color: var(--vp-c-text-3); }
.intro { font-size: 13px; }
.up { color: #2e9e4f; }
.down { color: #d64545; }
.controls { margin: 0.6rem 0 1rem; display: flex; gap: 0.75rem; align-items: center; flex-wrap: wrap; }
button {
  font: inherit; cursor: pointer; border: 1px solid var(--vp-c-divider);
  border-radius: 4px; background: var(--vp-c-bg-soft); padding: 0.15rem 0.5rem;
}
label { font-size: 12px; display: inline-flex; gap: 0.25rem; align-items: center; cursor: pointer; }
.wrap { display: flex; gap: 1.5rem; align-items: flex-start; }
.main { flex: 1 1 auto; min-width: 0; overflow-x: auto; }
.side { flex: 0 0 380px; position: sticky; top: 5rem; }
h2 { font-size: 0.95rem; margin: 0 0 0.4rem; border: 0; padding: 0; }
@media (max-width: 900px) {
  .wrap { flex-direction: column; }
  .side { position: static; flex-basis: auto; width: 100%; }
}
.swatch { display: inline-block; width: 0.6rem; height: 0.6rem; border-radius: 2px; margin-right: 0.35rem; vertical-align: middle; }
.legend { margin-top: 0.5rem; }
.legend-item { display: flex; align-items: center; margin: 0.1rem 0; }
</style>
```

- [ ] **Step 2: Update the mount test — `MetricsDashboard.test.ts`** (selection is now a `Set`, controls added). Replace the file:

```ts
import { describe, it, expect, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'

vi.mock('vitepress', () => ({ withBase: (p: string) => p }))

import MetricsDashboard from './MetricsDashboard.vue'
import { SAMPLE } from '../lib/sample'

describe('MetricsDashboard', () => {
  const factory = () => mount(MetricsDashboard, { props: { data: SAMPLE } })

  it('renders a table per group with domain-aware markers', () => {
    const w = factory()
    expect(w.findAll('table').length).toBeGreaterThanOrEqual(3)
    const html = w.html()
    expect(html).toContain('▲')
    expect(html).toContain('▼')
  })
  it('toggles a row to reveal an inline chart', async () => {
    const w = factory()
    expect(w.find('tr.chartrow').exists()).toBe(false)
    await w.find('tr.metric').trigger('click')
    await nextTick()
    expect(w.find('tr.chartrow').exists()).toBe(true)
  })
  it('collapse checkbox is on by default and reduces column count', async () => {
    const w = factory()
    const collapsedCols = w.findAll('table')[0].findAll('thead th').length
    await w.findAll('input[type=checkbox]')[0].setValue(false) // collapse off
    await nextTick()
    const expandedCols = w.findAll('table')[0].findAll('thead th').length
    expect(expandedCols).toBeGreaterThanOrEqual(collapsedCols)
  })
  it('empty state with no data', () => {
    const w = mount(MetricsDashboard, { props: { data: { snapshots: [], index: {} } } })
    expect(w.text()).toContain('No metrics data')
  })
})
```

- [ ] **Step 3: Run tests + build**

Run: `npm test --prefix site 2>&1 | tail -10`
Expected: PASS (all suites — lib + all three component tests).
Run: `npm run docs:build --prefix site 2>&1 | tail -3`
Expected: exit 0 (SSR-safe: `location`/`fetch` only touched in `onMounted`/handlers, all guarded).

- [ ] **Step 4: Commit**

```bash
git add site/.vitepress/theme/components/MetricsDashboard.vue site/.vitepress/theme/components/MetricsDashboard.test.ts
git commit -m "WS19.7: MetricsDashboard — collapse/Δ/only-changed toggles, stable colors, synced crosshair, URL state

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 9: Full-width metrics page (`layout: page`)

**Files:**
- Modify: `site/metrics/index.md`

- [ ] **Step 1: Replace `site/metrics/index.md`** (add `layout: page` frontmatter; update the stale "standalone viewer" note now that it's removed):

````md
---
layout: page
---

# Metrics

Per-commit performance and footprint, recorded in CI and charted live. Counts
(nodes, signals, allocations) are machine-independent; heap **bytes** and flash
sizes are CI-runner figures — compare trends within this store only.

<ClientOnly>
  <MetricsDashboard />
</ClientOnly>
````

- [ ] **Step 2: Build + verify the page renders full-width**

Run: `npm run docs:build --prefix site 2>&1 | tail -3 && test -f site/.vitepress/dist/metrics/index.html && echo OK`
Expected: exit 0; `OK`. (`layout: page` drops the prose max-width; the dashboard fills the width.)

- [ ] **Step 3: Commit**

```bash
git add site/metrics/index.md
git commit -m "WS19.7: metrics page uses layout:page for full-width; drop stale standalone-viewer note

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 10: Bookkeeping — roadmap WS19.7 + WS19.3 note + metrics-data prune note

**Files:**
- Modify: `docs/plans/2026-07-05-rsact-evolution-roadmap.md`

- [ ] **Step 1: Collect the commit hashes**

Run: `git log --oneline $(git merge-base master HEAD)..HEAD`
Expected: the WS19.7 commits (Tasks 1-9). Note them.

- [ ] **Step 2: Add a WS19.7 line under the WS19 section** in the roadmap — after the 19.6 item, insert:

```markdown
- [x] **19.7 Metrics viewer v2 + single-viewer consolidation (2026-07-09):** removed the standalone `metrics-probe html` viewer (deleted `metrics-probe/viewer/` + `src/html.rs` + the `html` subcommand/record-hook); the site is now the single metrics home, and its dev server charts the local git-ignored `metrics/` store via a Vite plugin sharing one `assemble()` with the CI `data.json`. Enhancements: fixed-layout sticky uniform-width first column; cell-centered inline-chart alignment (bug fix); global unchanged-commit collapse (`a..b` columns, prevPresent boundaries, nulls never split); stable per-metric colors; `layout:page` full-width; synchronized crosshair; Δ-from-baseline; only-changed filter; URL-hash state. All logic in pure vitest-tested `lib/*.ts`. **DONE — <hashes>.** (Branch `ws19-metrics-v2`, stacked on PR #12.)
```
Substitute `<hashes>` with the real short hashes from Step 1 (comma-separated or a range).

- [ ] **Step 3: Update the WS19.3 split note** — find the WS19.3 done-marker (added in the WS19 PR) that says the `html.rs` viewer stays the local dev view, and append: ` (WS19.7 dissolved this split: the standalone viewer is removed; the site is the one viewer and its dev server charts the local store.)`

- [ ] **Step 4: Note the metrics-data prune** — the vestigial `index.html`/`README.md`/`.nojekyll` on the `metrics-data` branch are now unused (the site never reads them). Leave them (harmless) OR prune via an explicit removal commit on that branch outside this PR. Record the decision in the WS19.7 line: append ` The metrics-data branch's stale index.html/README/.nojekyll are vestigial (harmless; optional manual prune).`

- [ ] **Step 5: Commit**

```bash
git add docs/plans/2026-07-05-rsact-evolution-roadmap.md
git commit -m "WS19.7: roadmap — record v2 + consolidation done (hashes); update WS19.3 split note

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 6: Final full verification**

Run: `cd /Users/as/dev/rsact-ws19/site && npm test 2>&1 | tail -6 && npm run docs:build 2>&1 | tail -3`
Expected: all tests pass; build exit 0.
Run: `cd /Users/as/dev/rsact-ws19 && cargo build -p metrics-probe 2>&1 | tail -2`
Expected: metrics-probe compiles (no html).

---

## Self-Review

**Spec coverage:** A1 remove viewer (T1); A2 metrics-data data-only (T1 + T10 note); A3 shared assemble + dev loader (T2); B1 fixed/sticky/uniform table + cell-centered bug fix (T5 geometry + T7 layout + T6/T7 chart cells); B2 collapse (T3 + wired T7/T8); B3 stable colors (T4 + wired T7/T8); B4 full-width (T9); B5 crosshair (T5 hover key + T6/T7/T8), Δ (T5 + T7/T8), only-changed (T5 isFlat + T8), URL state (T8). Bookkeeping (T10). All spec sections map to a task.

**Placeholder scan:** no TBD/TODO. Every code step has complete code or an exact edit. The one judgement note (delta test minus-glyph, T7 Step 3) explicitly says "pin the test to what `fmt` actually renders" — not a placeholder, a correctness instruction.

**Type consistency:** `MetricsData`/`Snapshot`/`IndexMap`/`SeriesRow`/`Series` reused from `types.ts`. `assemble(index, snapshots)` (T2) matches its callers (T2 script, T2 plugin). `columnGroups(rows, n)`/`collapseValues(values, groups)`/`columnLabel(snapshots, group)` (T3) match `MetricsDashboard`/`MetricTable` usage (T7/T8). `colorFor(key)` (T4) used in T7/T8. `HOVER_KEY: InjectionKey<Ref<number|null>>` (T5) provided in T8, injected in T6/T7. `xOf` cell-centered (T5) consumed by both charts. `MetricTable` props `{group, columns, selected: Set, delta}` (T7) match what `MetricsDashboard` passes (T8). `TrendChart` gains `n` prop (T7 Step 2) used by T7/T8. Selection is a `Set<string>` consistently across dashboard + table (changed from the old `Map`).

**Note on a cross-task edit:** Task 6 and Task 7 both touch `TrendChart.vue` (T6 adds crosshair, T7 adds the `n` prop). They commit separately; the T7 `n`-prop edit is additive to the T6 version. An implementer doing T7 sees the T6-modified file.
