# Metrics UI Phase B — commit-message tooltip (#4) + PR grouping (#7) — design

**Date:** 2026-07-10 · **Workstream:** WS19.8 Phase B (completes the WS19.8 metrics ideas; Phase A merged to `master` via PR #14)
**Branch:** `ws19-metrics-v4`, off `master` (Phase A UI is already in `master`; no stacking).

## Goal
Finish the two WS19.8 ideas that Phase A deferred because the data wasn't in the store: **#4** show a commit's message on hovering its column header, and **#7** group the commit columns by PR with a linked header. Both are unlocked by enriching the `metrics/index.json` store with two git-derived fields.

## Confirmed decisions
- **PR source:** pure git (parse merge commits + squash `(#N)` subjects) inside `metrics-probe index`. No `gh`/token/network.
- **Commit message display:** extend the header hash's existing native `title` tooltip (no custom popover).
- **Backward-compatible + graceful:** the live CI store won't carry the new fields until the metrics workflow re-runs `index`; the UI degrades cleanly until then.

## Data layer — `metrics-probe` (Rust)
`index::IndexEntry` gains two fields, both `#[serde(default, skip_serializing_if = ...)]` so existing entries serialize byte-identically when the field is unknown (minimal `metrics-data` diff; struct already has `#[serde(default)]`):
```rust
pub subject: String,   // commit subject (first line); "" = unknown
pub pr: Option<u32>,   // associated PR number; None = unknown
```
- **`subject`** — populated in `git_index_entry(rev)` via `git_out(&["show","-s","--format=%s",rev])`. Runs for both `record` (HEAD, shallow-safe) and `index` (all revs). `record` leaves `pr = None` (a feature-branch commit's PR isn't a merge yet at record time).
- **`pr`** — derived only in `cmd_index()` (full history in the backfill). Pure-git, two sources, unit-testable parsers in `index.rs`:
  - `parse_merge_pr(subject) -> Option<u32>`: `"Merge pull request #N from …"` → `N`. For each such merge commit `M`, `git rev-list M^2 --not M^1` enumerates exactly the commits that PR brought in → each maps to `N` (plus `M` itself).
  - `parse_squash_pr(subject) -> Option<u32>`: a subject ending `"… (#N)"` → `N` (GitHub squash-merge default).
  - `resolve_pr(ancestry, subject) -> Option<u32>`: `ancestry.or_else(|| parse_squash_pr(subject))` — exact merge-map ancestry wins; the squash `(#N)` heuristic only fills commits no merge covers.
  - Per rev: `entry.pr = index::resolve_pr(pr_map.get(rev).copied(), &entry.subject)`.
  - No regex crate — manual byte/char parsing (no new dependency, no lockfile churn).
- **No CI-script changes**: it's all inside `metrics-probe`. `record` fills `subject` immediately; `pr` fills on the next `metrics-probe index` (already run by `scripts/ci-backfill.sh`).

## Site layer
- `types.ts` `IndexEntry`: add `subject?: string; pr?: number`.
- `lib/repo.ts`: add `prUrl(pr)` = `${REPO_URL}/pull/${pr}` and `branchCommitsUrl(branch)` = `${REPO_URL}/commits/${branch}`.
- **#4 tooltip:** the dashboard `columns` computed appends the (collapsed run's last) commit `subject` to the header hash `title` → `rev8 · branch · date · subject`. `subject` absent → title unchanged (Phase A behavior).
- **#7 PR grouping:** new pure `prColumnGroups(keys)` in `lib/collapse.ts` — `keys` is a per-column grouping key (`pr` number if known, else `branch` string, else `null`); it merges **adjacent equal** keys into `{ key, start, span }` runs. The dashboard:
  - derives `perColumnKey[]` from each column's representative index entry (`pr ?? branch ?? null`),
  - renders a **third sticky `<thead>` row** *above* the commit-hash row: one `<th :colspan="span">` per group, labeled `#N` → `prUrl(N)` when `pr` known, else the branch name → `branchCommitsUrl(branch)`, else blank,
  - marks each group's first column (except column 0) in a `groupStart: boolean[]`, threaded to the header rows **and** `MetricSection`, which apply a `2px` left border (`.group-start`) across every row — the "thicker separator".
  - The `ResizeObserver` already measures the whole `<thead>`, so `--head-h` (sticky-caption offset) absorbs the new row automatically.
- **Graceful degradation:** render the PR row only when at least one group has a non-null key. `branch` is already populated in today's store, so grouping + separators work immediately; `#N` links appear after the next `index` rebuild. Empty store → no PR row.

## Architecture / boundaries
Pure, tested logic; thin components. New pure: `index.rs` `parse_merge_pr`/`parse_squash_pr` (Rust), `lib/repo.ts` `prUrl`/`branchCommitsUrl`, `lib/collapse.ts` `prColumnGroups` (TS). Touched components: `MetricsDashboard.vue` (columns title + PR header row + perColumnKey/groupStart), `MetricSection.vue` (accept `groupStart`, apply `.group-start`). Data: `metrics-probe/src/index.rs` (fields + parsers), `metrics-probe/src/main.rs` (`git_index_entry` subject, `cmd_index` PR map).

## Testing
- **Rust** (`metrics-probe`, `cargo test -p metrics-probe`): `parse_merge_pr` (match / non-merge / malformed), `parse_squash_pr` (trailing `(#N)` / none / mid-subject not matched), `IndexEntry` serde roundtrip incl. skip-serializing when empty/None (old JSON still parses; new JSON omits absent fields).
- **Site vitest**: `prColumnGroups` (adjacent-equal runs, null breaks, single, all-null); `repo` (`prUrl`/`branchCommitsUrl` formats); component (PR row renders groups w/ correct `colspan` + `#N`/branch links; `.group-start` on group boundaries not col 0; header `title` includes subject; PR row omitted when every key is null; branch-fallback when `pr` absent).
- **Manual**: `docs:dev` against the local store — branch grouping + separators render now; after a local `metrics-probe index` run the `#N` links + subjects appear.

## Bookkeeping
Roadmap: mark WS19.8 #4 + #7 done → **WS19.8 fully complete** (Phase A + B). Note the store repopulates `subject`/`pr` on the next CI `index` rebuild.

## Risks
- **name-rev branch hint vs. merge-branch name**: PR grouping uses `pr` (exact, ancestry-derived) as the primary key; `branch` is only the fallback label/grouping, so a fuzzy `name-rev` hint degrades gracefully (groups by whatever hint is stored, links to `/commits/<hint>`), never mis-links a PR.
- **Rebase-merged PRs** (no merge commit, no `(#N)`): `pr` stays `None` → branch fallback. Acceptable; this repo uses merge commits.
- **Lag**: existing store entries gain the fields only after the next `index` rebuild — by design; UI degrades gracefully. Flagged for the maintainer.
