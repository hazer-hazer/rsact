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
