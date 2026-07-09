import { describe, it, expect } from 'vitest'
import { colorFor, PALETTE } from './colors'

describe('colorFor', () => {
  it('is deterministic for a given key', () => {
    expect(colorFor('ui_labels_10/nodes_total')).toBe(colorFor('ui_labels_10/nodes_total'))
  })
  it('always returns a palette color', () => {
    for (const k of ['a', 'b', 'bench:reactivity/signal_read', 'size:ui/.text']) {
      expect(PALETTE).toContain(colorFor(k))
    }
  })
  it('spreads distinct keys across more than one color', () => {
    const keys = Array.from({ length: 20 }, (_, i) => `metric_${i}`)
    const used = new Set(keys.map(colorFor))
    expect(used.size).toBeGreaterThan(3)
  })
})
