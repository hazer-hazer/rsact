//! WS0.9d: read criterion median wall-clocks from `target/criterion`.
//!
//! metrics-probe does NOT run the benches — `scripts/ci-metrics.sh` (or a
//! developer) runs `cargo bench` first; this module reads whatever criterion
//! left at `target/criterion/<id>/new/estimates.json`. We take the **median**
//! (not the mean) because it is robust to the outlier spikes shared CI runners
//! produce. A missing dir or an unparseable file is skipped (logged), never an
//! abort — the same graceful policy the size probes use.

use crate::snapshot::BenchMedian;
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

const CRITERION_DIR: &str = "target/criterion";

// Minimal view of criterion's estimates.json — only the median block.
#[derive(Deserialize)]
struct Estimates {
    median: Estimate,
}
#[derive(Deserialize)]
struct Estimate {
    point_estimate: f64,
    confidence_interval: ConfidenceInterval,
}
#[derive(Deserialize)]
struct ConfidenceInterval {
    lower_bound: f64,
    upper_bound: f64,
}

/// Read every `target/criterion/<id>/new/estimates.json` (the latest run per
/// bench), returning medians sorted by id for deterministic snapshot output.
/// Empty (logged) when the criterion dir is absent — i.e. no benches were run.
pub fn read_all() -> Vec<BenchMedian> {
    let root = Path::new(CRITERION_DIR);
    if !root.is_dir() {
        eprintln!(
            "  benches: no {CRITERION_DIR} (run `cargo bench` first) — skipped"
        );
        return Vec::new();
    }
    let mut files = Vec::new();
    collect_new_estimates(root, &mut files);
    let mut out = Vec::new();
    for path in files {
        match read_one(&path, root) {
            Ok(bm) => out.push(bm),
            Err(e) => eprintln!("  benches: skipped {}: {e}", path.display()),
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    println!("  benches: {} criterion median(s)", out.len());
    out
}

/// Recursively collect every `.../new/estimates.json` under `dir` (criterion
/// nests grouped/parameterised benches to varying depth).
fn collect_new_estimates(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_new_estimates(&path, out);
        } else if is_new_estimates(&path) {
            out.push(path);
        }
    }
}

/// True for a file named `estimates.json` whose parent directory is `new`
/// (criterion's latest-run slot — as opposed to `base/` or a saved baseline).
fn is_new_estimates(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()) == Some("estimates.json")
        && path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str())
            == Some("new")
}

fn read_one(path: &Path, root: &Path) -> Result<BenchMedian, String> {
    // id = the path between `target/criterion/` and `/new/estimates.json`.
    let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
    let comps: Vec<_> = rel.components().collect();
    let id = comps[..comps.len().saturating_sub(2)]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");

    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let est: Estimates =
        serde_json::from_str(&data).map_err(|e| e.to_string())?;
    let ci = &est.median.confidence_interval;
    Ok(BenchMedian {
        id,
        median_ns: est.median.point_estimate,
        ci_half_ns: (ci.upper_bound - ci.lower_bound) / 2.0,
    })
}
