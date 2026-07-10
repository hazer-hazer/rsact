import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import MetricSection from './MetricSection.vue'

const columns = [
  { label: 'aaaaaaaa', title: 'aaaaaaaa', href: '#a', group: [0] },
  { label: 'bbbbbbbb', title: 'bbbbbbbb', href: '#b', group: [1] },
  { label: 'cccccccc', title: 'cccccccc', href: '#c', group: [2] },
]
const group = {
  title: 'reactive_only_16',
  rows: [
    { key: 'reactive_only_16/signals', label: 'signals', values: [16, 16, 18] },
    { key: 'reactive_only_16/observers', label: 'observers', values: [17, 17, 17] },
  ],
}
const mountInTable = (props: Record<string, unknown>) =>
  mount(MetricSection, {
    props,
    // a <tbody> component must live inside a <table>
    attachTo: document.createElement('table'),
  })

describe('MetricSection', () => {
  const base = {
    group,
    columns,
    selected: new Set<string>(),
    delta: false,
    changed: [true, false, true],
    groupStart: [false, false, false],
  }
  it('renders a <tbody> root with a full-width caption row', () => {
    const w = mountInTable(base)
    expect(w.element.tagName).toBe('TBODY')
    const cap = w.find('tr.section-head th')
    expect(cap.exists()).toBe(true)
    expect(cap.attributes('colspan')).toBe(String(1 + columns.length))
    expect(cap.text()).toContain('reactive_only_16')
  })
  it('marks unchanged columns with the dim class', () => {
    const w = mountInTable(base)
    const firstRowCells = w.findAll('tr.metric')[0].findAll('td:not(.lbl)')
    expect(firstRowCells[0].classes()).not.toContain('dim') // changed[0] = true
    expect(firstRowCells[1].classes()).toContain('dim')     // changed[1] = false
  })
  it('marks the group-start column with a separator class', () => {
    const w = mountInTable({ ...base, groupStart: [false, true, false] })
    const firstRowCells = w.findAll('tr.metric')[0].findAll('td:not(.lbl)')
    expect(firstRowCells[0].classes()).not.toContain('group-start')
    expect(firstRowCells[1].classes()).toContain('group-start')
    expect(firstRowCells[2].classes()).not.toContain('group-start')
  })
  it('shows an inline chart row only for selected metrics', () => {
    const w = mountInTable({ ...base, selected: new Set(['reactive_only_16/signals']) })
    expect(w.findAll('tr.chartrow').length).toBe(1)
  })
  it('emits toggle with the row key on click', async () => {
    const w = mountInTable(base)
    await w.findAll('tr.metric')[0].trigger('click')
    expect(w.emitted('toggle')?.[0]).toEqual(['reactive_only_16/signals'])
  })
})
