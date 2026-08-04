import type { MetricsData, Snapshot, IndexMap } from './types'

// Turn a raw index + snapshot set into the {snapshots, index} the dashboard
// consumes, ordered oldest→newest. Primary key is index `order` — the grouped
// mainline sequence (a PR's commits contiguous, so the header can span a whole
// PR) computed by `metrics-probe index`. Commits without an order (a shallow
// `record` since the last index pass) sort AFTER all ordered ones — they're the
// newest, so date-ordering them at the tail is correct. Ties fall back to
// committer date, then recorded_at, then rev for stability. Pure.
export function assemble(index: IndexMap, snapshots: Snapshot[]): MetricsData {
  const orderOf = (s: Snapshot) => index[s.git_rev]?.order ?? Infinity
  const dateOf = (s: Snapshot) => index[s.git_rev]?.date ?? s.recorded_at ?? 0
  const sorted = [...snapshots].sort(
    (a, b) =>
      orderOf(a) - orderOf(b) || dateOf(a) - dateOf(b) || a.git_rev.localeCompare(b.git_rev),
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
