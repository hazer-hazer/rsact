//! `metrics/index.json` — the commit **ordering index** (WS0.9e).
//!
//! Snapshots are keyed by commit hash, and hashes don't self-order. Backfilled
//! snapshots additionally all share ~one `recorded_at` (the backfill wall-clock),
//! so the viewer cannot order commits by timestamp. This index records, per rev,
//! the committer date + parent hash + a branch hint, so the viewer can lay
//! commits on a correct x-axis **without running git** (browsers can't).
//!
//! Maintenance is incremental so it works under shallow CI checkouts: each
//! `record` merges only *its own* rev's entry (all three fields are resolvable
//! for HEAD even at `fetch-depth: 1`); the full-history backfill fills entries
//! for every commit it touches.

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::Path,
};

pub const INDEX_PATH: &str = "metrics/index.json";

/// One commit's ordering metadata. Empty strings mean "unknown" (e.g. a
/// force-pushed-away parent, or a rev git can no longer name) — the viewer
/// degrades to date ordering for those.
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

/// rev (full hash) → entry. `BTreeMap` for deterministic serialization.
pub type Index = BTreeMap<String, IndexEntry>;

/// Load the index, or an **empty** index when the file is absent (the normal
/// first-run / bootstrap case) or unparseable (logged). Never an error — a
/// missing order index just means "fall back to date ordering".
pub fn load(path: &Path) -> Index {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            eprintln!("  index: skipping unparseable {}: {e}", path.display());
            Index::new()
        }),
        Err(_) => Index::new(), // absent → empty (bootstrap / first run)
    }
}

/// Write the index as pretty JSON, creating the parent directory.
pub fn save(index: &Index, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(index).unwrap())
}

/// Insert or overwrite one rev's entry (incremental maintenance).
pub fn merge_entry(index: &mut Index, rev: &str, entry: IndexEntry) {
    index.insert(rev.to_string(), entry);
}

/// Order `revs` **oldest → newest** by commit history: primarily by committer
/// `date` (stored per-rev in `index` — the real commit time, even for
/// backfilled snapshots), tie-broken by ancestor topology (how many of `revs`
/// are ancestors, walking `parent` links and transitively skipping commits
/// absent from `revs`) then the rev string.
///
/// Date is primary — not topology — because parent links are **not always
/// available**: a shallow CI checkout (`fetch-depth: 1`) makes HEAD a parentless
/// boundary commit, so `record` stores `parent == ""`. Committer date is always
/// real, so it keeps push-recorded and backfilled commits correctly interleaved;
/// topology only refines genuinely equal timestamps (e.g. same-second commits in
/// a backfill). Revs missing from `index` sort as date 0 — the caller should
/// synthesize a minimal entry (date = the snapshot's `recorded_at`) first.
// Note: retained (with tests) though currently unused in the build — the site
// orders the metrics store by date (assemble.ts); this parent-chain topological
// order is kept for a potential future TS port / index-ordering use.
#[allow(dead_code)]
pub fn topo_order(index: &Index, revs: &[String]) -> Vec<String> {
    let revset: HashSet<&str> = revs.iter().map(String::as_str).collect();
    let mut ordered = revs.to_vec();
    ordered.sort_by(|a, b| {
        let (da, db) = (
            index.get(a).map_or(0, |e| e.date),
            index.get(b).map_or(0, |e| e.date),
        );
        let (ca, cb) = (
            ancestor_count(index, &revset, a),
            ancestor_count(index, &revset, b),
        );
        da.cmp(&db).then(ca.cmp(&cb)).then_with(|| a.cmp(b))
    });
    ordered
}

/// How many members of `revset` are strict ancestors of `rev`, walking
/// first-parent links through `index` and **transitively skipping** commits
/// absent from `revset`. Cycle-guarded so a corrupt index can't hang.
// Note: retained (with tests) though currently unused in the build — see
// topo_order above; kept for a potential future TS port / index-ordering use.
#[allow(dead_code)]
fn ancestor_count(index: &Index, revset: &HashSet<&str>, rev: &str) -> usize {
    let mut count = 0;
    let mut seen: HashSet<&str> = HashSet::new();
    let mut cur = index.get(rev).map(|e| e.parent.as_str()).unwrap_or("");
    while !cur.is_empty() && seen.insert(cur) {
        if revset.contains(cur) {
            count += 1;
        }
        cur = index.get(cur).map(|e| e.parent.as_str()).unwrap_or("");
    }
    count
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn e(date: u64, parent: &str) -> IndexEntry {
        IndexEntry { date, parent: parent.to_string(), branch: String::new(), ..Default::default() }
    }

    #[test]
    fn topo_order_linear_chain_orders_oldest_first() {
        // a <- b <- c ; input deliberately shuffled.
        let mut idx = Index::new();
        idx.insert("a".into(), e(10, ""));
        idx.insert("b".into(), e(20, "a"));
        idx.insert("c".into(), e(30, "b"));
        let revs = vec!["c".to_string(), "a".to_string(), "b".to_string()];
        assert_eq!(topo_order(&idx, &revs), vec!["a", "b", "c"]);
    }

    #[test]
    fn topo_order_skips_commits_absent_from_the_revset() {
        // Chain a <- b <- c, but only a and c have snapshots. c's nearest
        // snapshot-ancestor is a (through the un-snapshotted b), so c has 1
        // ancestor in the set → ordered after a.
        let mut idx = Index::new();
        idx.insert("a".into(), e(10, ""));
        idx.insert("b".into(), e(20, "a"));
        idx.insert("c".into(), e(30, "b"));
        let revs = vec!["c".to_string(), "a".to_string()];
        assert_eq!(topo_order(&idx, &revs), vec!["a", "c"]);
    }

    #[test]
    fn topo_order_mixed_parent_availability_orders_by_date() {
        // Real CI shape: `root` + `a` are backfilled (fetch-depth 0 → real
        // parents), while `b` was recorded by a shallow push (fetch-depth 1 →
        // git can't see its parent, so parent==""). Ancestor-count-primary would
        // sort the newest commit `b` (count 0) BEFORE `a` (count 1); committer
        // date must win so history stays oldest→newest: root, a, b.
        let mut idx = Index::new();
        idx.insert("root".into(), e(50, ""));
        idx.insert("a".into(), e(100, "root"));
        idx.insert("b".into(), e(200, "")); // shallow push: parent unknown
        let revs = vec!["b".to_string(), "a".to_string(), "root".to_string()];
        assert_eq!(topo_order(&idx, &revs), vec!["root", "a", "b"]);
    }

    #[test]
    fn topo_order_roots_fall_back_to_date() {
        // Two unrelated roots (no parent) → tie on ancestor count → date order.
        let mut idx = Index::new();
        idx.insert("late".into(), e(200, ""));
        idx.insert("early".into(), e(100, ""));
        let revs = vec!["late".to_string(), "early".to_string()];
        assert_eq!(topo_order(&idx, &revs), vec!["early", "late"]);
    }

    #[test]
    fn topo_order_is_cycle_safe() {
        // Pathological self/loop parent must not hang.
        let mut idx = Index::new();
        idx.insert("x".into(), e(10, "y"));
        idx.insert("y".into(), e(20, "x"));
        let revs = vec!["x".to_string(), "y".to_string()];
        let out = topo_order(&idx, &revs);
        assert_eq!(out.len(), 2); // terminates, both present
    }

    #[test]
    fn merge_entry_inserts_and_overwrites() {
        let mut idx = Index::new();
        merge_entry(&mut idx, "r", e(1, ""));
        merge_entry(&mut idx, "r", e(2, "p"));
        assert_eq!(idx.len(), 1);
        assert_eq!(idx["r"], e(2, "p"));
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let idx = load(Path::new("/nonexistent/does/not/exist.json"));
        assert!(idx.is_empty());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = std::env::temp_dir()
            .join(format!("mp-index-test-{}", std::process::id()));
        let path = dir.join("index.json");
        let mut idx = Index::new();
        idx.insert("aa".into(), e(5, ""));
        idx.insert("bb".into(), e(6, "aa"));
        save(&idx, &path).expect("save");
        let back = load(&path);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(back, idx);
    }

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
}
