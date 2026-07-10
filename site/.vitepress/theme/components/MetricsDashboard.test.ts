import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import MetricsDashboard from './MetricsDashboard.vue'
import type { MetricsData } from '../lib/types'

vi.mock('vitepress', () => ({ withBase: (p: string) => p }))

const DATA: MetricsData = {
  index: {
    aaaaaaaa11: { date: 1_700_000_000, parent: 'par', branch: 'main', pr: 5, subject: 'hello subj' },
    bbbbbbbb22: { date: 1_700_100_000, parent: 'aaaaaaaa11', branch: 'main', pr: 6 },
  },
  snapshots: [
    { git_rev: 'aaaaaaaa11', git_dirty: false, scenarios: [
      { name: 's1', counts: { signals: 10, total: 10 }, heap_live_bytes: null, heap_peak_bytes: null, build_allocs: null, change_frame_allocs: null, layout: null } ] },
    { git_rev: 'bbbbbbbb22', git_dirty: false, scenarios: [
      { name: 's1', counts: { signals: 12, total: 12 }, heap_live_bytes: null, heap_peak_bytes: null, build_allocs: null, change_frame_allocs: null, layout: null } ] },
  ],
}

const observed: unknown[] = []

describe('MetricsDashboard', () => {
  beforeEach(() => {
    observed.length = 0
    // @ts-expect-error test env
    global.ResizeObserver = class {
      observe(el: unknown) { observed.push(el) }
      disconnect() {}
    }
  })
  afterEach(() => {
    vi.restoreAllMocks()
  })
  it('attaches the head ResizeObserver once the thead appears on the async no-data path', async () => {
    global.fetch = vi.fn(async () => ({ ok: true, json: async () => DATA })) as unknown as typeof fetch
    const w = mount(MetricsDashboard)
    await flushPromises()
    expect(w.find('thead').exists()).toBe(true)
    expect(observed.length).toBeGreaterThan(0)
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
  it('renders a PR-grouped header row linking each column to its PR', async () => {
    const w = mount(MetricsDashboard, { props: { data: DATA } })
    await flushPromises()
    const prRow = w.find('thead tr.prgroups')
    expect(prRow.exists()).toBe(true)
    const links = prRow.findAll('th.col a')
    expect(links.length).toBe(2)
    expect(links[0].attributes('href')).toContain('/pull/5')
    expect(links[1].attributes('href')).toContain('/pull/6')
  })
  it('falls back to grouping by branch (as one spanning column) when pr is absent', async () => {
    const data: MetricsData = {
      index: {
        aaaaaaaa11: { date: 1_700_000_000, parent: 'par', branch: 'feature-y' },
        bbbbbbbb22: { date: 1_700_100_000, parent: 'aaaaaaaa11', branch: 'feature-y' },
      },
      snapshots: DATA.snapshots,
    }
    const w = mount(MetricsDashboard, { props: { data } })
    await flushPromises()
    const prRow = w.find('thead tr.prgroups')
    expect(prRow.exists()).toBe(true)
    const cols = prRow.findAll('th.col')
    expect(cols.length).toBe(1)
    expect(cols[0].attributes('colspan')).toBe('2')
    const link = cols[0].find('a')
    expect(link.text()).toBe('feature-y')
    expect(link.attributes('href')).toContain('/commits/feature-y')
  })
  it('omits the PR row when no column has a PR or branch to group by', async () => {
    const data: MetricsData = {
      index: {
        aaaaaaaa11: { date: 1_700_000_000, parent: 'par', branch: '' },
        bbbbbbbb22: { date: 1_700_100_000, parent: 'aaaaaaaa11', branch: '' },
      },
      snapshots: DATA.snapshots,
    }
    const w = mount(MetricsDashboard, { props: { data } })
    await flushPromises()
    expect(w.find('thead tr.prgroups').exists()).toBe(false)
  })
  it('includes the commit subject in the header title tooltip', async () => {
    const w = mount(MetricsDashboard, { props: { data: DATA } })
    await flushPromises()
    const th = w.find('thead tr.cols th.col')
    expect(th.attributes('title')).toContain('hello subj')
  })
})
