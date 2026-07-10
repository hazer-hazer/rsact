# Metrics UI Phase B Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Enrich the metrics store with commit `subject` + `pr` (pure git, in `metrics-probe`), then use them for the commit-message tooltip (#4) and PR-grouped column headers (#7) on the site.

**Architecture:** Rust `metrics-probe` derives the fields from git during `record`/`index`; the site reads two new optional `IndexEntry` fields and renders a tooltip + a PR header row. Pure logic (Rust parsers, TS grouping) is unit-tested; components stay thin. Degrades gracefully when the fields are absent.

**Tech Stack:** Rust (metrics-probe, serde), VitePress + Vue 3 `<script setup lang="ts">` + SCSS + Vitest.

## Global Constraints
- Spec: `docs/superpowers/specs/2026-07-10-metrics-ui-phase-b-design.md` (binding).
- Rust changes in `metrics-probe/src/{index.rs,main.rs}`; verify with `cargo test -p metrics-probe` and `cargo build -p metrics-probe` (foreground; NEVER background — first build may take a minute, use an extended timeout).
- Site changes under `site/`; `node_modules` present (do NOT `npm install`); verify with `cd site && npx vitest run <file>` and `npm run docs:build`.
- **No new dependencies** (no regex crate — manual parsing; no npm additions — protects the lockfile).
- `IndexEntry` new fields use `#[serde(default, skip_serializing_if = ...)]` so existing entries serialize unchanged when the field is unknown.
- Pure logic is unit-tested; components thin. Preserve all `Note:`/`TODO:` comments.
- PR source is pure git (merge-parse + squash `(#N)`); NO `gh`/network. Commit message via the native `title` (no custom popover).
- Repo base is `lib/repo.ts` `REPO_URL` (already `https://github.com/hazer-hazer/rsact`).

---

### Task 1: `index.rs` — `IndexEntry` fields + pure PR parsers

**Files:** Modify `metrics-probe/src/index.rs` (struct + 2 fns + tests).

**Interfaces produced:**
- `IndexEntry` gains `pub subject: String` and `pub pr: Option<u32>`.
- `pub fn parse_merge_pr(subject: &str) -> Option<u32>` — `"Merge pull request #<N> from …"` → `Some(N)`.
- `pub fn parse_squash_pr(subject: &str) -> Option<u32>` — subject ending `"… (#<N>)"` → `Some(N)`.

- [ ] **Step 1: Write failing tests** — add to the `#[cfg(test)] mod tests` in `index.rs`:

```rust
#[test]
fn parse_merge_pr_reads_github_merge_subject() {
    assert_eq!(parse_merge_pr("Merge pull request #14 from hazer-hazer/ws19-metrics-v3"), Some(14));
    assert_eq!(parse_merge_pr("Merge pull request #7 from x/y"), Some(7));
    assert_eq!(parse_merge_pr("Merge branch 'main'"), None);
    assert_eq!(parse_merge_pr("WS19.8: normal commit"), None);
    assert_eq!(parse_merge_pr("Merge pull request #x from y"), None);
}

#[test]
fn parse_squash_pr_reads_trailing_pr_ref() {
    assert_eq!(parse_squash_pr("Fix the thing (#42)"), Some(42));
    assert_eq!(parse_squash_pr("Add feature (#3)"), Some(3));
    assert_eq!(parse_squash_pr("no pr here"), None);
    assert_eq!(parse_squash_pr("mentions (#5) mid-subject only"), None); // not trailing
    assert_eq!(parse_squash_pr("bad (#) ref"), None);
}

#[test]
fn index_entry_omits_empty_subject_and_none_pr() {
    let entry = IndexEntry { date: 5, parent: "p".into(), branch: "b".into(), ..Default::default() };
    let json = serde_json::to_string(&entry).unwrap();
    assert!(!json.contains("subject"), "empty subject must be skipped: {json}");
    assert!(!json.contains("\"pr\""), "None pr must be skipped: {json}");
    // Old-shape JSON (no subject/pr) still parses, defaulting the new fields.
    let back: IndexEntry = serde_json::from_str(r#"{"date":5,"parent":"p","branch":"b"}"#).unwrap();
    assert_eq!(back, entry);
}

#[test]
fn index_entry_roundtrips_with_subject_and_pr() {
    let entry = IndexEntry { date: 9, parent: "".into(), branch: "".into(), subject: "hi".into(), pr: Some(12) };
    let json = serde_json::to_string(&entry).unwrap();
    assert_eq!(serde_json::from_str::<IndexEntry>(&json).unwrap(), entry);
}
```

- [ ] **Step 2: Run, verify fail** — `cargo test -p metrics-probe parse_ ` and the two index_entry tests fail to compile (unknown fields / fns).

- [ ] **Step 3: Implement.** In `index.rs`, extend the struct (keep existing doc comment + `#[serde(default)]`):

```rust
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Debug)]
#[serde(default)]
pub struct IndexEntry {
    /// Committer date, unix seconds.
    pub date: u64,
    /// Parent (first-parent) full hash, or "" when unknown / a root commit.
    pub parent: String,
    /// `git name-rev` hint (e.g. `master`, `remotes/origin/ws3~2`), or "".
    pub branch: String,
    /// Commit subject (first line of the message), or "" when unknown.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub subject: String,
    /// Associated PR number (from a merge/squash commit), or None when unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr: Option<u32>,
}
```

Add the two pure parsers (near the bottom of the non-test code):

```rust
/// PR number from a GitHub merge-commit subject: `Merge pull request #N from …`.
pub fn parse_merge_pr(subject: &str) -> Option<u32> {
    let rest = subject.strip_prefix("Merge pull request #")?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// PR number from a squash-merge subject ending in `(#N)` (GitHub squash default).
pub fn parse_squash_pr(subject: &str) -> Option<u32> {
    let trimmed = subject.trim_end();
    let inner = trimmed.strip_suffix(')')?.rsplit_once("(#")?.1;
    if inner.is_empty() || !inner.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    inner.parse().ok()
}
```

Also update the existing test helper `fn e(date, parent)` if it constructs `IndexEntry { date, parent, branch }` literally — add `..Default::default()` so it still compiles:

```rust
fn e(date: u64, parent: &str) -> IndexEntry {
    IndexEntry { date, parent: parent.to_string(), branch: String::new(), ..Default::default() }
}
```

- [ ] **Step 4: Run, verify pass** — `cargo test -p metrics-probe` (all green, incl. the pre-existing index tests).

- [ ] **Step 5: Commit** — `git add metrics-probe/src/index.rs && git commit -m "WS19.8 Phase B: IndexEntry subject/pr fields + pure PR-subject parsers"`

---

### Task 2: `main.rs` — populate `subject` (record+index) and `pr` (index)

**Files:** Modify `metrics-probe/src/main.rs` (`git_index_entry`, `cmd_index`).

**Interfaces consumed:** `index::IndexEntry{subject,pr}`, `index::parse_merge_pr`, `index::parse_squash_pr`, existing `git_out`.

- [ ] **Step 1: Populate `subject` in `git_index_entry`.** Replace the final construction:

```rust
fn git_index_entry(rev: &str) -> index::IndexEntry {
    let date = git_out(&["show", "-s", "--format=%ct", rev])
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let parent =
        git_out(&["rev-parse", "--verify", "--quiet", &format!("{rev}^")])
            .unwrap_or_default();
    let branch = git_out(&["name-rev", "--name-only", rev])
        .filter(|s| s != "undefined")
        .unwrap_or_default();
    let subject = git_out(&["show", "-s", "--format=%s", rev]).unwrap_or_default();
    // pr is filled by cmd_index (needs merge history); record leaves it None.
    index::IndexEntry { date, parent, branch, subject, pr: None }
}
```

- [ ] **Step 2: Derive `pr` in `cmd_index`.** Add `use std::collections::HashMap;` (near the top `use` block; `std::collections` is already imported in index.rs but main.rs needs its own). Replace the body of `cmd_index`:

```rust
fn cmd_index() -> std::io::Result<()> {
    let path = Path::new(index::INDEX_PATH);
    let mut idx = index::load(path);

    // Pure-git PR map: for each `Merge pull request #N` merge commit M, the
    // commits it brought in are `git rev-list M^2 --not M^1`. No network.
    let mut pr_of: HashMap<String, u32> = HashMap::new();
    if let Some(out) = git_out(&["log", "--merges", "--format=%H %s"]) {
        for line in out.lines() {
            let (m, subject) = line.split_once(' ').unwrap_or((line, ""));
            if let Some(n) = index::parse_merge_pr(subject) {
                if let Some(revs) =
                    git_out(&["rev-list", &format!("{m}^2"), "--not", &format!("{m}^1")])
                {
                    for c in revs.lines() {
                        pr_of.insert(c.to_string(), n);
                    }
                }
                pr_of.insert(m.to_string(), n);
            }
        }
    }

    let mut resolved = 0;
    for rev in snapshot_revs(Path::new(SNAPSHOT_DIR)) {
        let mut entry = git_index_entry(&rev);
        if entry.date != 0 {
            // Exact merge-map ancestry wins; the squash-subject heuristic
            // (`index::resolve_pr`, in index.rs) only fills commits no merge covers.
            entry.pr = index::resolve_pr(pr_of.get(&rev).copied(), &entry.subject);
            index::merge_entry(&mut idx, &rev, entry);
            resolved += 1;
        }
    }
    index::save(&idx, path)?;
    println!("index: {} entries ({resolved} resolved from git)", idx.len());
    Ok(())
}
```

- [ ] **Step 3: Build + test** — `cargo build -p metrics-probe` (green, no warnings) and `cargo test -p metrics-probe` (green). Report any warning as a finding.

- [ ] **Step 4: Smoke (optional, local store is git-ignored).** If `metrics/snapshots/` exists, run `cargo run -q -p metrics-probe -- index` and confirm it prints the entry count and exits 0; spot-check that `metrics/index.json` now has `subject` on entries and `pr` on at least merged-PR commits. Do NOT commit the git-ignored `metrics/` store.

- [ ] **Step 5: Commit** — `git add metrics-probe/src/main.rs && git commit -m "WS19.8 Phase B: metrics-probe records commit subject + derives PR from merges"`

---

### Task 3: `lib/repo.ts` — PR + branch URL helpers

**Files:** Modify `site/.vitepress/theme/lib/repo.ts` + `repo.test.ts`.

**Interfaces produced:** `prUrl(pr: number): string`, `branchCommitsUrl(branch: string): string`.

- [ ] **Step 1: Failing tests** — append to `repo.test.ts`:

```ts
import { prUrl, branchCommitsUrl } from './repo'

describe('pr / branch urls', () => {
  it('prUrl points at the PR page', () => {
    expect(prUrl(14)).toBe('https://github.com/hazer-hazer/rsact/pull/14')
  })
  it('branchCommitsUrl points at the branch commits page', () => {
    expect(branchCommitsUrl('ws19-metrics-v4')).toBe(
      'https://github.com/hazer-hazer/rsact/commits/ws19-metrics-v4',
    )
  })
})
```

- [ ] **Step 2: Run, verify fail** — `cd site && npx vitest run .vitepress/theme/lib/repo.test.ts`.

- [ ] **Step 3: Implement** — append to `repo.ts`:

```ts
// Link to a pull request.
export function prUrl(pr: number): string {
  return `${REPO_URL}/pull/${pr}`
}

// Link to a branch's commit history (fallback when the PR number is unknown).
export function branchCommitsUrl(branch: string): string {
  return `${REPO_URL}/commits/${branch}`
}
```

- [ ] **Step 4: Run, verify pass** — same command (5 tests).

- [ ] **Step 5: Commit** — `git add site/.vitepress/theme/lib/repo.ts site/.vitepress/theme/lib/repo.test.ts && git commit -m "WS19.8 Phase B: repo.ts — prUrl + branchCommitsUrl"`

---

### Task 4: `lib/collapse.ts` — `prColumnGroups`

**Files:** Modify `site/.vitepress/theme/lib/collapse.ts` + `collapse.test.ts`.

**Interfaces produced:** `interface PrGroup { key: string | number | null; start: number; span: number }`; `prColumnGroups(keys: (string | number | null)[]): PrGroup[]` — merges **adjacent equal** keys into runs.

- [ ] **Step 1: Failing tests** — append to `collapse.test.ts`:

```ts
import { prColumnGroups } from './collapse'

describe('prColumnGroups', () => {
  it('merges adjacent equal keys into runs with start + span', () => {
    expect(prColumnGroups([12, 12, 12, 13])).toEqual([
      { key: 12, start: 0, span: 3 },
      { key: 13, start: 3, span: 1 },
    ])
  })
  it('null keys break runs and are not merged with values', () => {
    expect(prColumnGroups([12, null, null, 'ws3'])).toEqual([
      { key: 12, start: 0, span: 1 },
      { key: null, start: 1, span: 2 },
      { key: 'ws3', start: 3, span: 1 },
    ])
  })
  it('handles empty and single', () => {
    expect(prColumnGroups([])).toEqual([])
    expect(prColumnGroups(['a'])).toEqual([{ key: 'a', start: 0, span: 1 }])
  })
})
```

- [ ] **Step 2: Run, verify fail** — `cd site && npx vitest run .vitepress/theme/lib/collapse.test.ts`.

- [ ] **Step 3: Implement** — append to `collapse.ts`:

```ts
export interface PrGroup { key: string | number | null; start: number; span: number }

// Merge ADJACENT columns that share a grouping key (PR number, else branch,
// else null) into runs — the column spans for the PR header row (#7). null
// keys never merge into a value run (an ungrouped gap stays its own span).
export function prColumnGroups(keys: (string | number | null)[]): PrGroup[] {
  const out: PrGroup[] = []
  for (let i = 0; i < keys.length; i++) {
    const last = out[out.length - 1]
    if (last && last.key === keys[i]) last.span++
    else out.push({ key: keys[i], start: i, span: 1 })
  }
  return out
}
```

- [ ] **Step 4: Run, verify pass** — same command.

- [ ] **Step 5: Commit** — `git add site/.vitepress/theme/lib/collapse.ts site/.vitepress/theme/lib/collapse.test.ts && git commit -m "WS19.8 Phase B: collapse.ts — prColumnGroups (adjacent-run column grouping)"`

---

### Task 5: Dashboard tooltip + PR header row + separators

**Files:** Modify `site/.vitepress/theme/lib/types.ts`, `components/MetricsDashboard.vue`, `components/MetricSection.vue`, `components/MetricsDashboard.test.ts`, `components/MetricSection.test.ts`.

**Interfaces consumed:** `prUrl`/`branchCommitsUrl` (Task 3), `prColumnGroups`/`PrGroup` (Task 4), `IndexEntry.subject/pr` (Tasks 1–2 shape, mirrored in TS).

- [ ] **Step 1: `types.ts`** — add to `IndexEntry`:

```ts
export interface IndexEntry {
  date: number
  parent: string
  branch: string
  subject?: string
  pr?: number
}
```

- [ ] **Step 2: `MetricsDashboard.vue` script.**
  (a) Extend the repo import to add the two new helpers (the dashboard currently imports ONLY `columnHref` — do not re-add `commitUrl`/`compareUrl`, they're unused there and would fail the build): `import { columnHref, prUrl, branchCommitsUrl } from '../lib/repo'`.
  (b) Extend the collapse import to add `prColumnGroups`.
  (c) In the `columns` computed, append the commit subject to `title`. The current title is `const title = [label, branch, date].filter(Boolean).join(' · ')`. Change to include the (last commit's) subject:

```ts
    const subject = index.value[last?.git_rev]?.subject
    const title = [label, branch, date, subject].filter(Boolean).join(' · ')
```

  (d) Add PR-grouping computeds after `columns`:

```ts
// Per-column grouping key: PR number if known, else the branch hint, else null.
const perColumnKey = computed<(string | number | null)[]>(() =>
  colGroups.value.map((g) => {
    const last = snapshots.value[g[g.length - 1]]
    const entry = index.value[last?.git_rev]
    return entry?.pr ?? entry?.branch ?? null
  }),
)
const prGroups = computed(() => prColumnGroups(perColumnKey.value))
// Show the PR row only when at least one column is actually grouped/labeled.
const showPrRow = computed(() => prGroups.value.some((g) => g.key !== null))
// First column of each group (except column 0) starts a separator.
const groupStart = computed<boolean[]>(() => {
  const s = new Array(columns.value.length).fill(false)
  for (const g of prGroups.value) if (g.start > 0) s[g.start] = true
  return s
})
function groupLabel(g: { key: string | number | null }): string {
  return typeof g.key === 'number' ? `#${g.key}` : typeof g.key === 'string' ? g.key : ''
}
function groupHref(g: { key: string | number | null }): string | undefined {
  return typeof g.key === 'number' ? prUrl(g.key)
    : typeof g.key === 'string' ? branchCommitsUrl(g.key)
    : undefined
}
```

- [ ] **Step 3: `MetricsDashboard.vue` template.** Add the PR row as the FIRST `<thead>` row (above `tr.cols`), and add the `group-start` class to the existing `tr.cols` / `tr.overall` column cells. Insert before `<tr class="cols">`:

```html
              <tr v-if="showPrRow" class="prgroups">
                <th class="lbl"></th>
                <th
                  v-for="(g, gi) in prGroups"
                  :key="gi"
                  class="col"
                  :colspan="g.span"
                  :class="{ 'group-start': g.start > 0 }"
                >
                  <a v-if="groupHref(g)" :href="groupHref(g)" target="_blank" rel="noreferrer">{{ groupLabel(g) }}</a>
                </th>
              </tr>
```

In `tr.cols` and `tr.overall`, extend the `<th class="col" ...>` `:class` bindings to add `'group-start': groupStart[i]` (alongside the existing `hov`/`dim`). For example the cols row `<th>` becomes:

```html
                  :class="{ hov: hover === i, dim: !changed[i], 'group-start': groupStart[i] }"
```

and likewise the overall row `<th>`.

Then pass `:group-start="groupStart"` to `<MetricSection …>`.

- [ ] **Step 4: `MetricsDashboard.vue` style.** Add:

```scss
thead tr.prgroups th.col { text-align: left; font-size: 11px; }
thead tr.prgroups th.col a { color: var(--vp-c-brand-1); text-decoration: none; }
thead tr.prgroups th.col a:hover { text-decoration: underline; }
.group-start { border-left: 2px solid var(--vp-c-text-3); }
```

- [ ] **Step 5: `MetricSection.vue`.** Add `groupStart: boolean[]` to `defineProps`, and add the class to the metric-row value cells. The cell `:class` becomes:

```html
          :class="{ hov: sharedHover === i, dim: !changed[i], 'group-start': groupStart[i] }"
```

Add the CSS rule (mirrors the dashboard so the separator is one continuous line):

```scss
td.group-start { border-left: 2px solid var(--vp-c-text-3); }
```

- [ ] **Step 6: Tests.**
  - `MetricSection.test.ts`: the base props object gains `groupStart: [false, false, false]` (length = columns). Add an assertion: with `groupStart: [false, true, false]`, the 2nd value cell of a metric row has class `group-start`.
  - `MetricsDashboard.test.ts`: extend the `DATA` fixture's two index entries with `pr` so grouping is exercised — set `aaa.pr = 5, bbb.pr = 6` (two single-column groups) OR give both the same `pr` (one 2-span group). Use DIFFERENT prs to assert two `tr.prgroups th.col` links whose hrefs contain `/pull/5` and `/pull/6`. Add a test: with neither `pr` nor a usable `branch`, `tr.prgroups` is absent (set both entries' `branch` to '' and drop `pr`) — asserts `showPrRow` false. Add a test that a header `<a>` `title` contains the subject when the index entry has one (set `aaa.subject = 'hello subj'`; assert a `thead tr.cols th.col a` ancestor `th`'s `title` or the `<a>`'s parent title contains 'hello subj' — the title is on the `<a>`? No, title is currently on the `<a>` in Phase A? Check: Phase A put `:title="c.title"` on the `th.col`, and the `<a>` inside. Assert the `th.col`'s `title` attribute contains the subject.)

  Concretely, verify against the current MetricsDashboard.vue: the `:title="c.title"` binding is on the `<th class="col">`. Assert `w.find('thead tr.cols th.col').attributes('title')` contains the subject.

- [ ] **Step 7: Run + build** — `cd site && npx vitest run` (all green) and `npm run docs:build` (green).

- [ ] **Step 8: Manual (optional)** — `docs:dev`: branch grouping + separators render against the local store; hovering a commit header shows the subject in the native tooltip.

- [ ] **Step 9: Commit** — `git add site/.vitepress/theme/lib/types.ts site/.vitepress/theme/components/ && git commit -m "WS19.8 Phase B: commit-message tooltip + PR-grouped sticky header row + separators"`

---

### Task 6: Roadmap bookkeeping

**Files:** Modify `docs/plans/2026-07-05-rsact-evolution-roadmap.md`.

- [ ] **Step 1:** Under the WS19 section, mark WS19.8 #4 (commit-message tooltip) and #7 (PR grouping/link) done with the Phase B commit range; note WS19.8 is now fully complete (Phase A + B), and that existing store entries gain `subject`/`pr` on the next CI `metrics-probe index` rebuild. Match surrounding formatting; don't disturb other entries or `Note:`/`TODO:` markers.
- [ ] **Step 2: Commit** — `git add docs/plans/2026-07-05-rsact-evolution-roadmap.md && git commit -m "WS19.8 Phase B: roadmap — #4 + #7 done, WS19.8 complete"`

---

## Self-Review notes (author)
- Spec coverage: #4 (Tasks 1–2 subject + Task 5 title); #7 (Tasks 1–2 pr + Tasks 3–5 grouping/links/separators). Data enrichment offline/pure (Tasks 1–2). Graceful degradation (Task 5 `showPrRow` + branch fallback).
- Type consistency: `IndexEntry.subject/pr` shape matches Rust; `PrGroup` used in Task 4 + Task 5; `groupStart: boolean[]` threaded dashboard→section.
- No new deps (manual Rust parsing; no npm adds). New `IndexEntry` fields are `skip_serializing_if` → old store JSON unaffected.
