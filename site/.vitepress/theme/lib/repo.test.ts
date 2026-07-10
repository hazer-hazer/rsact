import { describe, it, expect } from 'vitest'
import { REPO_URL, commitUrl, compareUrl, columnHref } from './repo'
import type { IndexMap, Snapshot } from './types'

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

describe('columnHref', () => {
  const snap = (git_rev: string): Snapshot => ({ git_rev, git_dirty: false, scenarios: [] })
  const snapshots: Snapshot[] = [snap('first-sha'), snap('last-sha')]

  it('single-commit group links to the commit page', () => {
    const index: IndexMap = {}
    expect(columnHref([0], snapshots, index)).toBe(commitUrl(snapshots[0].git_rev))
  })

  it('collapsed run links to the compare view from the first commit\'s parent', () => {
    const index: IndexMap = {
      'first-sha': { date: 0, parent: 'par', branch: 'main' },
    }
    expect(columnHref([0, 1], snapshots, index)).toBe(compareUrl('par', snapshots[1].git_rev))
  })

  it('collapsed run falls back to the last commit\'s page when the parent is unknown', () => {
    const index: IndexMap = {}
    expect(columnHref([0, 1], snapshots, index)).toBe(commitUrl(snapshots[1].git_rev))
  })
})
