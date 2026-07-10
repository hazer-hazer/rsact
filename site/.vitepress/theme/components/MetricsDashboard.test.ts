import { describe, it, expect, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'

vi.mock('vitepress', () => ({ withBase: (p: string) => p }))

import MetricsDashboard from './MetricsDashboard.vue'
import { SAMPLE } from '../lib/sample'
import type { MetricsData } from '../lib/types'

// Two truly-flat adjacent commits (identical counts) followed by one that
// changes — collapse should merge the first pair into a single column.
const COLLAPSIBLE: MetricsData = {
  snapshots: [
    {
      git_rev: 'a'.repeat(40), git_dirty: false,
      scenarios: [{ name: 's', counts: { total: 100 }, heap_live_bytes: 1000, heap_peak_bytes: 1000, build_allocs: 10, change_frame_allocs: 5, layout: null }],
    },
    {
      git_rev: 'b'.repeat(40), git_dirty: false,
      scenarios: [{ name: 's', counts: { total: 100 }, heap_live_bytes: 1000, heap_peak_bytes: 1000, build_allocs: 10, change_frame_allocs: 5, layout: null }],
    },
    {
      git_rev: 'c'.repeat(40), git_dirty: false,
      scenarios: [{ name: 's', counts: { total: 200 }, heap_live_bytes: 1000, heap_peak_bytes: 1000, build_allocs: 10, change_frame_allocs: 5, layout: null }],
    },
  ],
  index: {},
}

describe('MetricsDashboard', () => {
  const factory = () => mount(MetricsDashboard, { props: { data: SAMPLE } })

  it('renders a table per group with domain-aware markers', () => {
    const w = factory()
    expect(w.findAll('table').length).toBeGreaterThanOrEqual(3)
    const html = w.html()
    expect(html).toContain('▲')
    expect(html).toContain('▼')
  })
  it('toggles a row to reveal an inline chart', async () => {
    const w = factory()
    expect(w.find('tr.chartrow').exists()).toBe(false)
    await w.find('tr.metric').trigger('click')
    await nextTick()
    expect(w.find('tr.chartrow').exists()).toBe(true)
  })
  it('collapse checkbox is on by default and reduces column count', async () => {
    const w = mount(MetricsDashboard, { props: { data: COLLAPSIBLE } })
    const collapsedCols = w.findAll('table')[0].findAll('thead th').length
    await w.findAll('input[type=checkbox]')[0].setValue(false) // collapse off
    await nextTick()
    const expandedCols = w.findAll('table')[0].findAll('thead th').length
    expect(collapsedCols).toBeLessThan(expandedCols)
  })
  it('empty state with no data', () => {
    const w = mount(MetricsDashboard, { props: { data: { snapshots: [], index: {} } } })
    expect(w.text()).toContain('No metrics data')
  })
})
