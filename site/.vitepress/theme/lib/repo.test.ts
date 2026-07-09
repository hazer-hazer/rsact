import { describe, it, expect } from 'vitest'
import { REPO_URL, commitUrl, compareUrl } from './repo'

describe('repo urls', () => {
  it('REPO_URL matches the config socialLinks repo', () => {
    expect(REPO_URL).toBe('https://github.com/hazer-hazer/rsact')
  })
  it('commitUrl points at the commit page', () => {
    expect(commitUrl('abc123')).toBe('https://github.com/hazer-hazer/rsact/commit/abc123')
  })
  it('compareUrl uses the triple-dot range', () => {
    expect(compareUrl('par', 'last')).toBe('https://github.com/hazer-hazer/rsact/compare/par...last')
  })
})
