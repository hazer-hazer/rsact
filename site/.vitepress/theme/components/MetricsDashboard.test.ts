import { describe, it, expect, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'

// The component imports withBase from 'vitepress'; stub it so the module loads
// under vitest. Passing the `data` prop skips the fetch path entirely.
vi.mock('vitepress', () => ({ withBase: (p: string) => p }))

import MetricsDashboard from './MetricsDashboard.vue'
import { SAMPLE } from '../lib/sample'

describe('MetricsDashboard (mount)', () => {
  const factory = () => mount(MetricsDashboard, { props: { data: SAMPLE } })

  it('renders a table per group and domain-aware markers', () => {
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
    expect(w.findAll('.trendchart').length).toBeGreaterThanOrEqual(1)
  })

  it('shows the empty state with no data', () => {
    const w = mount(MetricsDashboard, { props: { data: { snapshots: [], index: {} } } })
    expect(w.text()).toContain('No metrics data')
    expect(w.findAll('table').length).toBe(0)
  })
})
