import { defineConfig } from 'vitepress'
import type { Plugin } from 'vite'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

// Dev-only: serve /metrics/data.json from the repo-root local `metrics/` store
// (git-ignored, machine-local numbers) so `docs:dev` charts real local data.
// Production serves the static dist/metrics/data.json assembled by site.yml.
function localMetricsPlugin(): Plugin {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..') // repo root from site/.vitepress/
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

// The site is served from a project subpath for now; custom domain later flips
// base to '/'. Rust code blocks highlight via VitePress's bundled Shiki.
export default defineConfig({
  title: 'rsact',
  description: 'Reactive Rust GUI framework for embedded systems — you pay for what you wire.',
  base: '/rsact/',
  cleanUrls: true,
  lastUpdated: true,
  // /api/ is rustdoc (static, not a VitePress route); the metrics data.json is
  // fetched at runtime. Don't fail the build on those non-page links.
  ignoreDeadLinks: [/^\/api/, /\/metrics\/data\.json$/],
  vite: { plugins: [localMetricsPlugin()] },
  themeConfig: {
    nav: [
      { text: 'Docs', link: '/docs/' },
      { text: 'Metrics', link: '/metrics/' },
      // rustdoc is a static dir added to the Pages artifact by site.yml, NOT a
      // VitePress route. `target` makes VitePress render a plain <a> (its router
      // skips click-interception on anchors with a target), so this does a real
      // browser navigation to /rsact/api/ instead of an in-app SPA 404.
      { text: 'API', link: '/api/', target: '_blank', rel: 'noreferrer' },
      { text: 'Roadmap', link: '/roadmap' },
    ],
    sidebar: {
      '/docs/': [
        {
          text: 'Guide',
          items: [
            { text: 'Getting started', link: '/docs/' },
            { text: 'Feature matrix', link: '/docs/features' },
            { text: 'Architecture', link: '/docs/architecture' },
          ],
        },
      ],
    },
    socialLinks: [{ icon: 'github', link: 'https://github.com/hazer-hazer/rsact' }],
    search: { provider: 'local' },
  },
})
