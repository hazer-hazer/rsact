import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { viteSingleFile } from 'vite-plugin-singlefile'

// The dashboard must be ONE self-contained file that opens from file:// with no
// external requests (it is published to GitHub Pages AND opened locally by
// `metrics-probe html`). `viteSingleFile` inlines every JS/CSS asset into
// dist/index.html; metrics-probe then `include_str!`s that file and replaces the
// `__DATA__` placeholder with the snapshot/index JSON at generation time.
export default defineConfig({
  plugins: [vue(), viteSingleFile()],
  // Relative base so the (already-inlined) output never depends on a host root.
  base: './',
  build: {
    target: 'es2020',
    // Keep the single file readable-ish and deterministic for review/diffing.
    minify: false,
    cssCodeSplit: false,
    assetsInlineLimit: 100000000,
    rollupOptions: { output: { inlineDynamicImports: true } },
  },
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.js'],
  },
})
