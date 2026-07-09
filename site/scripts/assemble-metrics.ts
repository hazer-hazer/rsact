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
