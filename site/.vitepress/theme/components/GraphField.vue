<script setup lang="ts">
// Home-page background: a sparse dependency graph with reactivity "signals"
// that fire from node to node along the edges — like shooting stars tracing the
// wires. Subtle, brand-cyan, dark/light-aware, pauses when hidden, and honors
// prefers-reduced-motion (renders a still graph, no motion). Canvas so the
// firing order is genuinely random rather than a looping SVG.
import { onMounted, onBeforeUnmount, ref } from 'vue'

const el = ref<HTMLCanvasElement | null>(null)

interface Node { x: number; y: number; charge: number }
interface Edge { a: number; b: number }
interface Sig { a: number; b: number; t: number; speed: number }

let ctx: CanvasRenderingContext2D | null = null
let raf = 0
let W = 0, H = 0, dpr = 1
let nodes: Node[] = []
let edges: Edge[] = []
let out: number[][] = [] // DIRECTED outgoing neighbours per node
let sigs: Sig[] = []
let busy = new Set<string>() // directed edges currently carrying a signal
let last = 0
let spawnAcc = 0
let running = false

let signal = '#c9a24b' // signals are ENIG-gold "pads"
let signalRGB = '201,162,75' // signal's r,g,b — for `rgba(...,a)` gradient stops
let edgeStyle = 'rgba(130,150,158,0.10)'
let nodeStyle = 'rgba(140,160,168,0.28)'

const MAX_SIG = 50
const SPAWN_EVERY = 0.5 // seconds between base spawns while under cap

const reduce = () =>
  typeof window !== 'undefined' &&
  window.matchMedia?.('(prefers-reduced-motion: reduce)').matches

// "#rrggbb" → "r,g,b" so gradient stops can fade alpha WITHIN the hue. Fading to
// the `transparent` keyword instead lerps rgb toward black (transparent black),
// which greys the glow over the dark background (canvas interpolates in sRGB).
function hexToRgb(hex: string): string {
  const n = parseInt(hex.slice(1), 16)
  return `${(n >> 16) & 255},${(n >> 8) & 255},${n & 255}`
}

function palette() {
  const dark = document.documentElement.classList.contains('dark')
  signal = dark ? '#c9a24b' : '#c9a24b' // ENIG gold; deeper on light for contrast
  signalRGB = hexToRgb(signal)
  edgeStyle = dark ? 'rgba(130,150,158,0.20)' : 'rgba(60,90,90,0.10)'
  nodeStyle = dark ? 'rgba(150,170,178,0.3)' : 'rgba(50,90,90,0.22)'
}

function rand(a: number, b: number) {
  return a + Math.random() * (b - a)
}

// A directed edge is uniquely a→b (one connection per pair, fixed direction).
const edgeKey = (a: number, b: number) => `${a}-${b}`

function layout() {
  if (!el.value) return
  W = window.innerWidth
  H = window.innerHeight
  dpr = Math.min(window.devicePixelRatio || 1, 2)
  el.value.width = Math.round(W * dpr)
  el.value.height = Math.round(H * dpr)
  ctx = el.value.getContext('2d')
  ctx?.setTransform(dpr, 0, 0, dpr, 0, 0)

  // Layered DAG — a reactivity graph: "source" signals on the left fan out to
  // dependent nodes to the right. Every edge points DOWNSTREAM (left→right), so
  // signals propagate one way, branching and occasionally merging (a memo with
  // several sources). Wiring to the nearest node in the next layer keeps edge
  // crossings low → an organic tree, not a street grid.
  const layers = Math.min(10, Math.max(4, Math.round(W / 120)))
  const bandW = W / layers
  nodes = []
  const byLayer: number[][] = []
  for (let L = 0; L < layers; L++) {
    const cnt = Math.min(8, Math.max(2, Math.round((H / 170) * rand(0.7, 1.15))))
    const idxs: number[] = []
    for (let k = 0; k < cnt; k++) {
      idxs.push(nodes.length)
      nodes.push({
        x: L * bandW + rand(0.28, 0.72) * bandW,
        y: ((k + rand(0.2, 0.8)) / cnt) * H,
        charge: 0,
      })
    }
    byLayer.push(idxs)
  }

  edges = []
  out = nodes.map(() => [])
  const key = new Set<string>()
  const incoming = new Set<number>()
  const link = (a: number, b: number) => {
    const kk = edgeKey(a, b)
    if (key.has(kk)) return
    key.add(kk)
    edges.push({ a, b })
    out[a].push(b)
    incoming.add(b)
  }
  // the `n` nodes in `pool` closest in y to `from`
  const nearestBy = (from: number, pool: number[], n: number) =>
    [...pool]
      .sort(
        (p, q) =>
          Math.abs(nodes[p].y - nodes[from].y) - Math.abs(nodes[q].y - nodes[from].y),
      )
      .slice(0, n)
  for (let L = 0; L < layers - 1; L++) {
    const next = byLayer[L + 1]
    // each node fans out to its 1–2 nearest downstream nodes
    for (const a of byLayer[L]) {
      const fan = Math.random() < 0.45 ? 2 : 1
      for (const b of nearestBy(a, next, fan)) link(a, b)
    }
    // no orphans: every next-layer node gets at least one upstream source
    for (const b of next) {
      if (!incoming.has(b)) link(nearestBy(b, byLayer[L], 1)[0], b)
    }
  }
  sigs = []
  busy.clear()
}

function spawn(from?: number) {
  if (sigs.length >= MAX_SIG || !edges.length) return
  let a: number, b: number
  if (from !== undefined) {
    // propagate downstream only, along this node's FREE outgoing edges
    const opts = out[from]?.filter((n) => !busy.has(edgeKey(from, n)))
    if (!opts || !opts.length) return
    a = from
    b = opts[(Math.random() * opts.length) | 0]
  } else {
    // a random edge that isn't already carrying a signal (a few tries)
    let e: Edge | undefined
    for (let tries = 0; tries < 5; tries++) {
      const c = edges[(Math.random() * edges.length) | 0]
      if (!busy.has(edgeKey(c.a, c.b))) { e = c; break }
    }
    if (!e) return
    a = e.a // the edge's fixed direction — never reversed
    b = e.b
  }
  busy.add(edgeKey(a, b)) // occupy the edge until this signal finishes
  // const speed = rand(0.55, 1.15)
  const speed = 1.0
  sigs.push({ a, b, t: 0, speed })
}

function frame(ts: number) {
  if (!running || !ctx) return
  const dt = Math.min(0.05, (ts - last) / 1000 || 0)
  last = ts
  ctx.clearRect(0, 0, W, H)

  // edges
  ctx.strokeStyle = edgeStyle
  ctx.lineWidth = 1
  ctx.beginPath()
  for (const e of edges) {
    ctx.moveTo(nodes[e.a].x, nodes[e.a].y)
    ctx.lineTo(nodes[e.b].x, nodes[e.b].y)
  }
  ctx.stroke()

  // base spawn cadence
  spawnAcc += dt
  if (spawnAcc >= SPAWN_EVERY) { spawnAcc = 0; spawn() }

  // signals
  for (let i = sigs.length - 1; i >= 0; i--) {
    const s = sigs[i]
    s.t += s.speed * dt
    const A = nodes[s.a]
    const B = nodes[s.b]
    if (s.t >= 1) {
      B.charge = 1 // light the destination node
      busy.delete(edgeKey(s.a, s.b)) // free the edge for reuse
      sigs.splice(i, 1)
      if (Math.random() < 0.62) spawn(s.b) // propagate onward (downstream only)
      continue
    }
    const hx = A.x + (B.x - A.x) * s.t
    const hy = A.y + (B.y - A.y) * s.t
    // Comet, oriented along travel (A→B): a teardrop glow — rounded head,
    // tapering to a point behind. u = forward unit vector, p = perpendicular.
    const dx = B.x - A.x, dy = B.y - A.y
    const len = Math.hypot(dx, dy) || 1
    const ux = dx / len, uy = dy / len
    const px = -uy, py = ux
    const headW = 3.4 // head half-width
    // tail grows as the signal pulls away from A, capped — never overshoots A.
    const tailLen = Math.min(30, len * s.t)
    const bx = hx - ux * tailLen, by = hy - uy * tailLen // tail tip
    // fade WITHIN the hue (rgba alpha→0), not to `transparent` (=black) — else
    // the tail greys as it fades over the dark background.
    const grad = ctx.createLinearGradient(bx, by, hx, hy)
    grad.addColorStop(0, `rgba(${signalRGB},0)`)
    grad.addColorStop(1, `rgba(${signalRGB},0.6)`)
    ctx.fillStyle = grad
    ctx.shadowColor = signal
    ctx.shadowBlur = 8 // soft edges → a glow, not a hard arrowhead
    ctx.beginPath()
    ctx.moveTo(bx, by) // tail point
    ctx.quadraticCurveTo(hx + px * headW * 1.3, hy + py * headW * 1.3, hx + ux * headW, hy + uy * headW) // belly → nose
    ctx.quadraticCurveTo(hx - px * headW * 1.3, hy - py * headW * 1.3, bx, by) // back to tail
    ctx.closePath()
    ctx.fill()
    ctx.shadowBlur = 0
    // hot core at the head
    ctx.fillStyle = signal
    ctx.beginPath()
    ctx.arc(hx, hy, 2, 0, Math.PI * 2)
    ctx.fill()
  }

  // nodes (dim; briefly bright when just charged)
  for (const n of nodes) {
    if (n.charge > 0) {
      ctx.fillStyle = signal
      ctx.globalAlpha = Math.min(1, n.charge) * 0.9
      ctx.beginPath()
      ctx.arc(n.x, n.y, 4, 0, Math.PI * 2)
      ctx.fill()
      ctx.globalAlpha = 1
      n.charge -= dt * 1.6
    } else {
      ctx.fillStyle = nodeStyle
      ctx.beginPath()
      ctx.arc(n.x, n.y, 3.2, 0, Math.PI * 2)
      ctx.fill()
    }
  }

  raf = requestAnimationFrame(frame)
}

function drawStill() {
  // reduced-motion: one static frame — the graph, no signals.
  if (!ctx) return
  ctx.clearRect(0, 0, W, H)
  ctx.strokeStyle = edgeStyle
  ctx.lineWidth = 1
  ctx.beginPath()
  for (const e of edges) {
    ctx.moveTo(nodes[e.a].x, nodes[e.a].y)
    ctx.lineTo(nodes[e.b].x, nodes[e.b].y)
  }
  ctx.stroke()
  ctx.fillStyle = nodeStyle
  for (const n of nodes) {
    ctx.beginPath()
    ctx.arc(n.x, n.y, 3.2, 0, Math.PI * 2)
    ctx.fill()
  }
}

function start() {
  if (reduce()) { drawStill(); return }
  if (running) return
  running = true
  last = performance.now()
  raf = requestAnimationFrame(frame)
}
function stop() {
  running = false
  cancelAnimationFrame(raf)
}

function onVisibility() {
  if (document.hidden) stop()
  else start()
}
let resizeTimer = 0
function onResize() {
  clearTimeout(resizeTimer)
  resizeTimer = window.setTimeout(() => { layout(); if (reduce()) drawStill() }, 150)
}

let themeObs: MutationObserver | null = null

onMounted(() => {
  palette()
  layout()
  start()
  window.addEventListener('resize', onResize)
  document.addEventListener('visibilitychange', onVisibility)
  // repaint palette on VitePress light/dark toggle
  themeObs = new MutationObserver(() => { palette(); if (reduce()) drawStill() })
  themeObs.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
})

onBeforeUnmount(() => {
  stop()
  window.removeEventListener('resize', onResize)
  document.removeEventListener('visibilitychange', onVisibility)
  themeObs?.disconnect()
  clearTimeout(resizeTimer)
})
</script>

<template>
  <canvas ref="el" class="graph-field" aria-hidden="true" />
</template>

<style scoped>
.graph-field {
  position: fixed;
  inset: 0;
  width: 100vw;
  height: 100vh;
  z-index: -1;
  pointer-events: none;
  opacity: 0.6;
}
</style>