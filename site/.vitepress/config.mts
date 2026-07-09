import { defineConfig } from 'vitepress'

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
