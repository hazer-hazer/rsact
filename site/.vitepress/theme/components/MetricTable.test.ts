import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import MetricTable from './MetricTable.vue'
import type { SeriesGroup } from '../lib/types'

const group: SeriesGroup = {
  title: 'ui_labels_10',
  rows: [
    { key: 'ui/nodes', label: 'nodes', values: [100, 80, 80] },
    { key: 'ui/heap', label: 'heap', values: [21000, 21000, 21000] },
  ],
}
const columns = [
  { label: 'aaaaaaaa', title: 'aaaaaaaa', group: [0] },
  { label: 'bbbbbbbb..cccccccc', title: 'run', group: [1, 2] },
]

describe('MetricTable', () => {
  it('renders collapsed columns + a header per column', () => {
    const w = mount(MetricTable, { props: { group, columns, selected: new Set(), delta: false } })
    const ths = w.findAll('thead th')
    expect(ths).toHaveLength(3) // metric + 2 columns
    expect(ths[2].text()).toBe('bbbbbbbb..cccccccc')
  })
  it('collapses values to one per column (100, then 80)', () => {
    const w = mount(MetricTable, { props: { group, columns, selected: new Set(), delta: false } })
    const firstRow = w.findAll('tbody tr.metric')[0]
    const cells = firstRow.findAll('td')
    expect(cells[1].text()).toContain('100')
    expect(cells[2].text()).toContain('80')
  })
  it('delta mode shows +/- from baseline', () => {
    const w = mount(MetricTable, { props: { group, columns, selected: new Set(), delta: true } })
    const cells = w.findAll('tbody tr.metric')[0].findAll('td')
    expect(cells[1].text()).toContain('0') // baseline
    expect(cells[2].text()).toContain('-20') // 80-100, fmt() uses toLocaleString() -> ASCII '-'
  })
  it('inline chart row has an empty leading cell (aligns to commit columns)', async () => {
    const w = mount(MetricTable, { props: { group, columns, selected: new Set(['ui/nodes']), delta: false } })
    const chartRow = w.find('tr.chartrow')
    expect(chartRow.exists()).toBe(true)
    expect(chartRow.findAll('td')[0].classes()).toContain('lbl')
    expect(chartRow.find('.trendchart').exists()).toBe(true)
  })
})
