// CI glue (run via tsx): read a checkout of the metrics-data branch and emit ONE
// history-ordered data.json (the viewer's {snapshots,index} contract) into the
// VitePress dist, plus copies of the raw sources for transparency. `import type`
// is erased at runtime, so this needs no compiled types present.
import {
  readFileSync, readdirSync, writeFileSync, mkdirSync, copyFileSync, existsSync,
} from 'node:fs'
import { join } from 'node:path'
import type { MetricsData, Snapshot, IndexMap } from '../.vitepress/theme/lib/types'

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

// History order: by index date asc, then recorded_at, then rev for stability.
const dateOf = (s: Snapshot) => index[s.git_rev]?.date ?? s.recorded_at ?? 0
snapshots.sort((a, b) => dateOf(a) - dateOf(b) || a.git_rev.localeCompare(b.git_rev))

const data: MetricsData = { snapshots, index }
mkdirSync(outDir, { recursive: true })
writeFileSync(join(outDir, 'data.json'), JSON.stringify(data))

// Raw copies (transparency; not required by the component).
if (existsSync(indexPath)) copyFileSync(indexPath, join(outDir, 'index.json'))
if (snapFiles.length) {
  mkdirSync(join(outDir, 'snapshots'), { recursive: true })
  for (const f of snapFiles) copyFileSync(join(snapDir, f), join(outDir, 'snapshots', f))
}

console.log(`assembled ${snapshots.length} snapshots -> ${join(outDir, 'data.json')}`)
