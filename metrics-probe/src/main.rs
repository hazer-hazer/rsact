//! `metrics-probe` — local-first framework-metrics tool for rsact (WS0.3).
//!
//! ```text
//! cargo run -p metrics-probe -- record        # snapshot HEAD -> metrics/snapshots/<rev>.json
//! cargo run -p metrics-probe -- diff <rev>     # compare current tree vs a stored snapshot
//! cargo run -p metrics-probe -- diff <file>    # ...or vs an explicit snapshot file
//! cargo run -p metrics-probe -- html           # regenerate metrics/index.html viewer
//! ```
//!
//! The same binary is what CI runs; CI merely archives the JSON it emits and
//! posts the `diff` output as a PR comment — it never replaces this local tool.

// The churn/live tracking allocator is shared with rsact-reactive's allocation
// bench (WS0.7j) so both count identically. Re-exported as `alloc` so the rest
// of the crate keeps referring to `crate::alloc`.
pub(crate) use rsact_reactive::alloc_probe as alloc;

mod html;
mod scenarios;
mod sizes;
mod snapshot;

use snapshot::{SNAPSHOT_DIR, Scenario, Snapshot};
use std::{
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
/// (Layer 2) — slower, so it is opt-in via `--sizes`.
fn measure(with_sizes: bool) -> Snapshot {
    let scenarios = scenarios::run_all();
    let section_sizes =
        if with_sizes { sizes::measure_all() } else { Vec::new() };
    Snapshot {
        git_rev: git_rev(),
        git_dirty: git_dirty(),
        recorded_at: now_secs(),
        host: host_triple(),
        scenarios,
        section_sizes,
    }
}

fn cmd_record(with_sizes: bool) -> std::io::Result<()> {
    let snap = measure(with_sizes);
    let dir = PathBuf::from(SNAPSHOT_DIR);
    let path = snap.save(&dir)?;
    println!(
        "recorded {} ({} scenarios)",
        path.display(),
        snap.scenarios.len()
    );
    print_snapshot(&snap);
    html::regenerate(&dir)?;
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

fn cmd_diff(arg: &str, with_sizes: bool) -> std::io::Result<()> {
    let baseline = resolve_baseline(arg)?;
    let current = measure(with_sizes);
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
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  metrics-probe record [--sizes]\n  metrics-probe diff [--sizes] <rev|file>\n  metrics-probe html\n\n  --sizes  also build the thumb size-probes and record .text/.rodata/.bss (Layer 2, slower)"
    );
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let with_sizes = args.iter().any(|a| a == "--sizes");
    // First non-flag argument after the subcommand.
    let positional = args.iter().skip(1).find(|a| !a.starts_with("--"));
    let result = match args.first().map(String::as_str) {
        Some("record") => cmd_record(with_sizes),
        Some("diff") => match positional {
            Some(arg) => cmd_diff(arg, with_sizes),
            None => usage(),
        },
        Some("html") => html::regenerate(Path::new(SNAPSHOT_DIR)),
        _ => usage(),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
