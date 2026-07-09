import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import MetricsDashboard from './MetricsDashboard.vue'
import type { MetricsData } from '../lib/types'

vi.mock('vitepress', () => ({ withBase: (p: string) => p }))

const DATA: MetricsData = {
  index: {
    aaa: { date: 1_700_000_000, parent: 'par', branch: 'main' },
    bbb: { date: 1_700_100_000, parent: 'aaa', branch: 'main' },
  },
  snapshots: [
    { git_rev: 'aaaaaaaa11', git_dirty: false, scenarios: [
      { name: 's1', counts: { signals: 10, total: 10 }, heap_live_bytes: null, heap_peak_bytes: null, build_allocs: null, change_frame_allocs: null, layout: null } ] },
    { git_rev: 'bbbbbbbb22', git_dirty: false, scenarios: [
      { name: 's1', counts: { signals: 12, total: 12 }, heap_live_bytes: null, heap_peak_bytes: null, build_allocs: null, change_frame_allocs: null, layout: null } ] },
  ],
}

describe('MetricsDashboard', () => {
  beforeEach(() => {
    // @ts-expect-error test env
    global.ResizeObserver = class { observe() {} disconnect() {} }
  })
  it('renders a single grid table with a thead and per-group tbodies', async () => {
    const w = mount(MetricsDashboard, { props: { data: DATA } })
    await flushPromises()
    expect(w.findAll('table.grid').length).toBe(1)
    expect(w.findAll('table.grid > thead').length).toBe(1)
    expect(w.findAll('table.grid > tbody').length).toBeGreaterThanOrEqual(1)
  })
  it('commit header cells are links to GitHub', async () => {
    const w = mount(MetricsDashboard, { props: { data: DATA } })
    await flushPromises()
    const links = w.findAll('thead tr.cols th.col a')
    expect(links.length).toBe(2)
    expect(links[0].attributes('href')).toContain('github.com/hazer-hazer/rsact')
  })
  it('renders a "Δ overall" row with one cell per column', async () => {
    const w = mount(MetricsDashboard, { props: { data: DATA } })
    await flushPromises()
    const overall = w.find('thead tr.overall')
    expect(overall.exists()).toBe(true)
    expect(overall.findAll('th.col').length).toBe(2)
  })
})
