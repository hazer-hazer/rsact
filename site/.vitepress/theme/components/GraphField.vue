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
let last = 0
let spawnAcc = 0
let running = false

let accent = '#2ee6d6'
let edgeStyle = 'rgba(130,150,158,0.10)'
let nodeStyle = 'rgba(140,160,168,0.28)'

const MAX_SIG = 10
const SPAWN_EVERY = 0.7 // seconds between base spawns while under cap

const reduce = () =>
  typeof window !== 'undefined' &&
  window.matchMedia?.('(prefers-reduced-motion: reduce)').matches

function palette() {
  const dark = document.documentElement.classList.contains('dark')
  accent = dark ? '#2ee6d6' : '#0a8f83'
  edgeStyle = dark ? 'rgba(130,150,158,0.10)' : 'rgba(60,90,90,0.10)'
  nodeStyle = dark ? 'rgba(150,170,178,0.26)' : 'rgba(50,90,90,0.22)'
}

function rand(a: number, b: number) {
  return a + Math.random() * (b - a)
}

function layout() {
  if (!el.value) return
  W = window.innerWidth
  H = window.innerHeight
  dpr = Math.min(window.devicePixelRatio || 1, 2)
  el.value.width = Math.round(W * dpr)
  el.value.height = Math.round(H * dpr)
  ctx = el.value.getContext('2d')
  ctx?.setTransform(dpr, 0, 0, dpr, 0, 0)

  // Jittered grid → even but organic node spread; density scales with area.
  const count = Math.round(Math.min(46, Math.max(14, (W * H) / 46000)))
  const cols = Math.max(2, Math.round(Math.sqrt((count * W) / H)))
  const rows = Math.max(2, Math.ceil(count / cols))
  const cw = W / cols
  const ch = H / rows
  nodes = []
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      nodes.push({
        x: c * cw + rand(0.2, 0.8) * cw,
        y: r * ch + rand(0.2, 0.8) * ch,
        charge: 0,
      })
    }
  }

  // Edges: each node to its 2 nearest neighbours (deduped to ONE per pair), then
  // each connection gets a single FIXED direction — signals only ever flow that
  // way, so a wire is never bidirectional.
  const key = new Set<string>()
  edges = []
  out = nodes.map(() => [])
  const maxLen = Math.hypot(cw, ch) * 1.7
  nodes.forEach((n, i) => {
    const near = nodes
      .map((m, j) => ({ j, d: Math.hypot(m.x - n.x, m.y - n.y) }))
      .filter((o) => o.j !== i && o.d < maxLen)
      .sort((p, q) => p.d - q.d)
      .slice(0, 2)
    for (const { j } of near) {
      const k = i < j ? `${i}-${j}` : `${j}-${i}`
      if (key.has(k)) continue
      key.add(k)
      const [a, b] = Math.random() < 0.5 ? [i, j] : [j, i] // fix direction once
      edges.push({ a, b })
      out[a].push(b)
    }
  })
  sigs = []
}

function spawn(from?: number) {
  if (sigs.length >= MAX_SIG || !edges.length) return
  let a: number, b: number
  if (from !== undefined) {
    // propagate downstream only, along this node's OUTGOING edges
    const opts = out[from]
    if (!opts?.length) return
    a = from
    b = opts[(Math.random() * opts.length) | 0]
  } else {
    const e = edges[(Math.random() * edges.length) | 0]
    a = e.a // the edge's fixed direction — never reversed
    b = e.b
  }
  sigs.push({ a, b, t: 0, speed: rand(0.55, 1.15) })
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
      sigs.splice(i, 1)
      if (Math.random() < 0.62) spawn(s.b) // propagate onward (downstream only)
      continue
    }
    const hx = A.x + (B.x - A.x) * s.t
    const hy = A.y + (B.y - A.y) * s.t
    // trailing streak (the "shooting star")
    const tail = Math.max(0, s.t - 0.22)
    const tx = A.x + (B.x - A.x) * tail
    const ty = A.y + (B.y - A.y) * tail
    const g = ctx.createLinearGradient(tx, ty, hx, hy)
    g.addColorStop(0, 'transparent')
    g.addColorStop(1, accent)
    ctx.strokeStyle = g
    ctx.lineWidth = 1.6
    ctx.beginPath()
    ctx.moveTo(tx, ty)
    ctx.lineTo(hx, hy)
    ctx.stroke()
    // head
    ctx.fillStyle = accent
    ctx.shadowColor = accent
    ctx.shadowBlur = 6
    ctx.beginPath()
    ctx.arc(hx, hy, 1.7, 0, Math.PI * 2)
    ctx.fill()
    ctx.shadowBlur = 0
  }

  // nodes (dim; briefly bright when just charged)
  for (const n of nodes) {
    if (n.charge > 0) {
      ctx.fillStyle = accent
      ctx.globalAlpha = Math.min(1, n.charge) * 0.9
      ctx.beginPath()
      ctx.arc(n.x, n.y, 2.4, 0, Math.PI * 2)
      ctx.fill()
      ctx.globalAlpha = 1
      n.charge -= dt * 1.6
    } else {
      ctx.fillStyle = nodeStyle
      ctx.beginPath()
      ctx.arc(n.x, n.y, 1.5, 0, Math.PI * 2)
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
    ctx.arc(n.x, n.y, 1.5, 0, Math.PI * 2)
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
