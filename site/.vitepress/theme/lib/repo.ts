import type { IndexMap, Snapshot } from './types'

// GitHub repo base. KEEP IN SYNC with .vitepress/config.mts socialLinks.
export const REPO_URL = 'https://github.com/hazer-hazer/rsact'

// Link to a single commit's page.
export function commitUrl(sha: string): string {
  return `${REPO_URL}/commit/${sha}`
}

// Link to the diff across a range. `from` is typically the parent of a collapsed
// run's first commit, so the compare shows every commit in the run.
export function compareUrl(from: string, to: string): string {
  return `${REPO_URL}/compare/${from}...${to}`
}

// Href for a column spanning commit indices `group` (into `snapshots`, oldest→newest):
// a single commit → its commit page; a collapsed run → the compare view from the
// FIRST commit's parent to the LAST commit (so the range shows every commit in the
// run), falling back to the last commit's page when the parent is unknown.
export function columnHref(group: number[], snapshots: Snapshot[], index: IndexMap): string {
  const first = snapshots[group[0]]
  const last = snapshots[group[group.length - 1]]
  if (group.length === 1) return commitUrl(last.git_rev)
  const parent = index[first?.git_rev]?.parent
  return parent ? compareUrl(parent, last.git_rev) : commitUrl(last.git_rev)
}

// Link to a pull request.
export function prUrl(pr: number): string {
  return `${REPO_URL}/pull/${pr}`
}

// Link to a branch's commit history (fallback when the PR number is unknown).
export function branchCommitsUrl(branch: string): string {
  return `${REPO_URL}/commits/${branch}`
}
