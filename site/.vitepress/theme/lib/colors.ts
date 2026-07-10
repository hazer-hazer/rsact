// A metric key maps to the SAME color everywhere and every session, so a reader
// learns "flash size is teal" once. Deterministic hash → a 16-color qualitative
// palette (Tableau-20 subset + extras). Collisions only matter when two colliding
// metrics are co-selected — rare, and the legend disambiguates.
export const PALETTE: readonly string[] = [
  '#4e79a7', '#f28e2c', '#e15759', '#76b7b2', '#59a14f', '#edc949',
  '#af7aa1', '#ff9da7', '#9c755f', '#bab0ab', '#1f77b4', '#ff7f0e',
  '#2ca02c', '#d62728', '#9467bd', '#8c564b',
]

// FNV-1a (32-bit) — stable across runs/machines, unlike Math.random or insertion order.
function hash(key: string): number {
  let h = 0x811c9dc5
  for (let i = 0; i < key.length; i++) {
    h ^= key.charCodeAt(i)
    h = Math.imul(h, 0x01000193)
  }
  return h >>> 0
}

export function colorFor(key: string): string {
  return PALETTE[hash(key) % PALETTE.length]
}
