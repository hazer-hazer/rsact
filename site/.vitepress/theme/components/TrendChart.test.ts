import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import { ref } from 'vue'
import TrendChart from './TrendChart.vue'
import { HOVER_KEY } from '../lib/hover'

describe('TrendChart', () => {
  it('draws a guide at the shared hover column when provided', () => {
    const hover = ref<number | null>(2)
    const w = mount(TrendChart, {
      props: { series: [{ label: 'x', values: [1, 2, 3, 4], color: '#000' }], height: 56 },
      global: { provide: { [HOVER_KEY as symbol]: hover } },
    })
    expect(w.find('line.guide').exists()).toBe(true)
  })
  it('no guide when shared hover is null and not interacting', () => {
    const hover = ref<number | null>(null)
    const w = mount(TrendChart, {
      props: { series: [{ label: 'x', values: [1, 2, 3], color: '#000' }] },
      global: { provide: { [HOVER_KEY as symbol]: hover } },
    })
    expect(w.find('line.guide').exists()).toBe(false)
  })
})
