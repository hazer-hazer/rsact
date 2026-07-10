//! `metrics-probe` — local-first framework-metrics tool for rsact (WS0.3).
//!
//! ```text
//! cargo run -p metrics-probe -- record        # snapshot HEAD -> metrics/snapshots/<rev>.json
//! cargo run -p metrics-probe -- diff <rev>     # compare current tree vs a stored snapshot
//! cargo run -p metrics-probe -- diff <file>    # ...or vs an explicit snapshot file
//! ```
//!
//! The same binary is what CI runs; CI merely archives the JSON it emits and
//! posts the `diff` output as a PR comment — it never replaces this local tool.

// The churn/live tracking allocator is shared with rsact-reactive's allocation
// bench (WS0.7j) so both count identically. Re-exported as `alloc` so the rest
// of the crate keeps referring to `crate::alloc`.
pub(crate) use rsact_reactive::alloc_probe as alloc;

mod benches;
mod index;
mod scenarios;
mod sizes;
mod snapshot;

use snapshot::{SNAPSHOT_DIR, Scenario, Snapshot};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[global_allocator]
static GLOBAL: alloc::Tracking = alloc::Tracking;

fn git_rev() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn git_dirty() -> bool {
    Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false)
}

fn host_triple() -> String {
    format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Build a snapshot of the current working tree (runs all scenarios). When
/// `with_sizes`, also builds the thumb size-probes and reads their sections
/// (Layer 2) — slower, so it is opt-in via `--sizes`. When `with_benches`, reads
/// criterion medians from `target/criterion` (WS0.9d) — the caller must have run
/// `cargo bench` first; opt-in via `--benches`.
fn measure(with_sizes: bool, with_benches: bool) -> Snapshot {
    let scenarios = scenarios::run_all();
    let section_sizes =
        if with_sizes { sizes::measure_all() } else { Vec::new() };
    let bench_medians =
        if with_benches { benches::read_all() } else { Vec::new() };
    Snapshot {
        git_rev: git_rev(),
        git_dirty: git_dirty(),
        recorded_at: now_secs(),
        host: host_triple(),
        scenarios,
        section_sizes,
        bench_medians,
    }
}

fn cmd_record(with_sizes: bool, with_benches: bool) -> std::io::Result<()> {
    let snap = measure(with_sizes, with_benches);
    let dir = PathBuf::from(SNAPSHOT_DIR);
    let path = snap.save(&dir)?;
    println!(
        "recorded {} ({} scenarios)",
        path.display(),
        snap.scenarios.len()
    );
    print_snapshot(&snap);
    update_index(&snap)?;
    Ok(())
}

/// Resolve a `diff` argument to a snapshot: a file path, a snapshot filename
/// stem, or any git revision (`HEAD~1`, a short hash, a branch/tag) — the last
/// is resolved to a full hash via `git rev-parse --verify`.
fn resolve_baseline(arg: &str) -> std::io::Result<Snapshot> {
    let as_path = Path::new(arg);
    if as_path.is_file() {
        return Snapshot::load(as_path);
    }
    if let Some(snap) = try_load_rev(arg)? {
        return Ok(snap);
    }
    // Fall back to resolving `arg` as a git revision, then look up its hash.
    if let Some(full) = git_rev_parse(arg) {
        if let Some(snap) = try_load_rev(&full)? {
            return Ok(snap);
        }
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "'{arg}' resolved to {full} but {SNAPSHOT_DIR}/{full}.json does not exist"
            ),
        ));
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!(
            "no snapshot for '{arg}' (tried a file path, {SNAPSHOT_DIR}/{arg}.json, and `git rev-parse`)"
        ),
    ))
}

/// Load `<rev>.json` or `<rev>-dirty.json` from the snapshot dir, if present.
fn try_load_rev(rev: &str) -> std::io::Result<Option<Snapshot>> {
    for name in [format!("{rev}.json"), format!("{rev}-dirty.json")] {
        let p = Path::new(SNAPSHOT_DIR).join(&name);
        if p.is_file() {
            return Snapshot::load(&p).map(Some);
        }
    }
    Ok(None)
}

/// Resolve a git revision spec to a full commit hash, or `None`.
fn git_rev_parse(rev: &str) -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", rev])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Run `git <args>` and return trimmed stdout on success, else `None`.
fn git_out(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Ordering metadata for `rev` from git (WS0.9e): committer date, first-parent
/// hash, and a `name-rev` branch hint. All resolvable for HEAD even at shallow
/// `fetch-depth: 1` — the parent *hash* lives in HEAD's own commit object. A
/// root commit (no parent), an un-nameable rev, or a commit git can't see
/// (shallow) yields "" / 0, which the viewer treats as "order by date".
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
    let subject =
        git_out(&["show", "-s", "--format=%s", rev]).unwrap_or_default();
    // pr is filled by cmd_index (needs merge history); record leaves it None.
    index::IndexEntry { date, parent, branch, subject, pr: None }
}

/// Merge one snapshot's rev into `metrics/index.json` (incremental, shallow-safe
/// — used by `record`).
fn update_index(snap: &Snapshot) -> std::io::Result<()> {
    let path = Path::new(index::INDEX_PATH);
    let mut idx = index::load(path);
    index::merge_entry(&mut idx, &snap.git_rev, git_index_entry(&snap.git_rev));
    index::save(&idx, path)
}

/// Full commit hashes that have a snapshot on disk (strips `.json` and the
/// `-dirty` suffix; de-duplicated).
fn snapshot_revs(dir: &Path) -> Vec<String> {
    let mut revs: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                return None;
            }
            let stem = p.file_stem()?.to_str()?;
            Some(stem.strip_suffix("-dirty").unwrap_or(stem).to_string())
        })
        .collect();
    revs.sort();
    revs.dedup();
    revs
}

/// `index` subcommand (WS0.9e backfill finalize): rebuild `metrics/index.json`
/// entries for **every** snapshot rev from full git history. Only overwrites a
/// rev's entry when git can resolve it (date != 0), so on a shallow checkout it
/// harmlessly leaves unresolvable revs to whatever they already had.
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
                if let Some(revs) = git_out(&[
                    "rev-list",
                    &format!("{m}^2"),
                    "--not",
                    &format!("{m}^1"),
                ]) {
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
            // Exact merge-map ancestry wins; squash-subject only fills commits
            // no merge covers (true squash-merges).
            entry.pr = index::resolve_pr(pr_of.get(&rev).copied(), &entry.subject);
            index::merge_entry(&mut idx, &rev, entry);
            resolved += 1;
        }
    }
    index::save(&idx, path)?;
    println!("index: {} entries ({resolved} resolved from git)", idx.len());
    Ok(())
}

fn cmd_diff(
    arg: &str,
    with_sizes: bool,
    with_benches: bool,
) -> std::io::Result<()> {
    let baseline = resolve_baseline(arg)?;
    let current = measure(with_sizes, with_benches);
    println!(
        "diff  baseline {}{}  ->  current {}{}\n",
        short(&baseline.git_rev),
        if baseline.git_dirty { "-dirty" } else { "" },
        short(&current.git_rev),
        if current.git_dirty { "-dirty" } else { "" },
    );
    for cur in &current.scenarios {
        let base = baseline.scenarios.iter().find(|s| s.name == cur.name);
        print_scenario_diff(cur, base);
    }
    for cur in &current.section_sizes {
        let base = baseline
            .section_sizes
            .iter()
            .find(|s| s.binary == cur.binary && s.target == cur.target);
        println!("  {}/{} (section sizes)", cur.binary, cur.target);
        irow(".text", base.map(|b| b.text as i64), cur.text as i64);
        irow(".rodata", base.map(|b| b.rodata as i64), cur.rodata as i64);
        irow(".bss", base.map(|b| b.bss as i64), cur.bss as i64);
        println!();
    }
    if !current.bench_medians.is_empty() {
        // Wall-clock is noisy (±runner); show the delta but frame it, never gate.
        println!("  criterion medians (ns) — informational, ±runner noise");
        for cur in &current.bench_medians {
            let base = baseline.bench_medians.iter().find(|b| b.id == cur.id);
            let (b, d) = match base {
                Some(b) => (
                    format!("{:.0}", b.median_ns),
                    delta_str((cur.median_ns - b.median_ns) as i64),
                ),
                None => ("-".to_string(), String::new()),
            };
            println!(
                "    {:<34} {b:>12} -> {:>12.0}  ±{:.0}  {d}",
                cur.id, cur.median_ns, cur.ci_half_ns
            );
        }
        println!();
    }
    Ok(())
}

fn short(rev: &str) -> String {
    rev.chars().take(9).collect()
}

fn irow(label: &str, base: Option<i64>, cur: i64) {
    let (b, d) = match base {
        Some(b) => (b.to_string(), delta_str(cur - b)),
        None => ("-".to_string(), String::new()),
    };
    println!("    {label:<22} {b:>10} -> {cur:>10}  {d}");
}

fn delta_str(d: i64) -> String {
    if d == 0 { "=".to_string() } else { format!("{d:+}") }
}

fn print_scenario_diff(cur: &Scenario, base: Option<&Scenario>) {
    println!("  {}", cur.name);
    irow(
        "nodes_total",
        base.map(|b| b.counts.total as i64),
        cur.counts.total as i64,
    );
    irow(
        "signals",
        base.map(|b| b.counts.signals as i64),
        cur.counts.signals as i64,
    );
    irow("memos", base.map(|b| b.counts.memos as i64), cur.counts.memos as i64);
    irow(
        "effects",
        base.map(|b| b.counts.effects as i64),
        cur.counts.effects as i64,
    );
    irow(
        "observers",
        base.map(|b| b.counts.observers as i64),
        cur.counts.observers as i64,
    );
    irow(
        "stored",
        base.map(|b| b.counts.stored as i64),
        cur.counts.stored as i64,
    );
    irow(
        "heap_live_bytes",
        base.map(|b| b.heap_live_bytes as i64),
        cur.heap_live_bytes as i64,
    );
    irow(
        "heap_peak_bytes",
        base.map(|b| b.heap_peak_bytes as i64),
        cur.heap_peak_bytes as i64,
    );
    irow(
        "build_allocs",
        base.map(|b| b.build_allocs as i64),
        cur.build_allocs as i64,
    );
    opt_irow(
        "idle_frame_allocs",
        base.and_then(|b| b.idle_frame_allocs.map(|v| v as i64)),
        cur.idle_frame_allocs.map(|v| v as i64),
    );
    opt_irow(
        "change_frame_allocs",
        base.and_then(|b| b.change_frame_allocs.map(|v| v as i64)),
        cur.change_frame_allocs.map(|v| v as i64),
    );
    if let Some(l) = cur.layout {
        irow(
            "layout_visits",
            base.and_then(|b| b.layout).map(|l| l.visits as i64),
            l.visits as i64,
        );
        irow(
            "layout_measures",
            base.and_then(|b| b.layout).map(|l| l.measures as i64),
            l.measures as i64,
        );
    }
    println!();
}

fn opt_irow(label: &str, base: Option<i64>, cur: Option<i64>) {
    match cur {
        Some(c) => irow(label, base, c),
        None => println!("    {label:<22} {:>10} -> {:>10}", "-", "n/a"),
    }
}

fn print_snapshot(snap: &Snapshot) {
    for s in &snap.scenarios {
        println!("  {}", s.name);
        println!(
            "    nodes={} (sig {}, memo {}, eff {}, obs {}, stored {})",
            s.counts.total,
            s.counts.signals,
            s.counts.memos,
            s.counts.effects,
            s.counts.observers,
            s.counts.stored,
        );
        println!(
            "    heap_live={}B peak={}B build_allocs={} idle_frame={} change_frame={}",
            s.heap_live_bytes,
            s.heap_peak_bytes,
            s.build_allocs,
            s.idle_frame_allocs
                .map(|v| v.to_string())
                .unwrap_or("n/a".into()),
            s.change_frame_allocs
                .map(|v| v.to_string())
                .unwrap_or("n/a".into()),
        );
    }
    for sz in &snap.section_sizes {
        println!(
            "  size {}/{}: .text={} .rodata={} .bss={}",
            sz.binary, sz.target, sz.text, sz.rodata, sz.bss
        );
    }
    for bm in &snap.bench_medians {
        println!("  bench {}: {:.0} ns (±{:.0})", bm.id, bm.median_ns, bm.ci_half_ns);
    }
}

/// Point this clone's git hooks at `.githooks` (WS0.8), enabling the
/// post-commit metrics snapshot. One-time, per-clone; equivalent to
/// `git config core.hooksPath .githooks`.
fn cmd_hook_install() -> std::io::Result<()> {
    let status = Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .status()?;
    if status.success() {
        println!(
            "core.hooksPath -> .githooks; the post-commit metrics hook is now active."
        );
        Ok(())
    } else {
        Err(std::io::Error::other("`git config core.hooksPath` failed"))
    }
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  metrics-probe record [--sizes] [--benches]\n  metrics-probe diff [--sizes] [--benches] <rev|file>\n  metrics-probe index\n  metrics-probe hook-install\n\n  record     snapshot HEAD; also merges HEAD into metrics/index.json (ordering)\n  index      rebuild metrics/index.json for every snapshot rev from git history (WS0.9e backfill finalize)\n  --sizes    also build the thumb size-probes and record .text/.rodata/.bss (Layer 2, slower)\n  --benches  also read criterion medians from target/criterion (run `cargo bench` first; WS0.9d)"
    );
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let with_sizes = args.iter().any(|a| a == "--sizes");
    let with_benches = args.iter().any(|a| a == "--benches");
    // First non-flag argument after the subcommand.
    let positional = args.iter().skip(1).find(|a| !a.starts_with("--"));
    let result = match args.first().map(String::as_str) {
        Some("record") => cmd_record(with_sizes, with_benches),
        Some("diff") => match positional {
            Some(arg) => cmd_diff(arg, with_sizes, with_benches),
            None => usage(),
        },
        Some("index") => cmd_index(),
        Some("hook-install") => cmd_hook_install(),
        _ => usage(),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
