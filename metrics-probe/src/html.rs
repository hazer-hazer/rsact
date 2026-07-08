//! Regenerates the self-contained metrics dashboard over the snapshot dir. The
//! viewer is a Vue 3 app that lives in `metrics-probe/viewer/` and is built once
//! (`npm run build` → `viewer/dist/index.html`, a single file with everything
//! inlined). That built file is baked into this binary via `include_str!`; at
//! generation time we inject the snapshot + ordering JSON into its
//! `<script type="application/json" id="metrics-data">` block. The result stays
//! self-contained (opens from `file://`, no external requests) and is published
//! to GitHub Pages.
//!
//! WS0.9e: commits are laid out in **topological order** (from `index.json`, not
//! `recorded_at` — backfilled snapshots share one wall-clock). The viewer shows a
//! value table with domain-aware ▲/▼ improvement markers, click-to-expand per-row
//! charts, and a sidepanel overlaying selected series (each normalized to its own
//! max). Absent metrics render as **gaps, never zeros** (the 0.7e/0.7f lesson).
//!
//! To change the dashboard, edit `metrics-probe/viewer/src/**`, run
//! `npm run build` there, commit `viewer/dist/index.html`, and rebuild this crate.

use crate::{index, snapshot::Snapshot};
use std::{collections::HashMap, fs, io, path::Path};

pub fn regenerate(dir: &Path) -> io::Result<()> {
    let mut snaps: Vec<Snapshot> = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                match Snapshot::load(&path) {
                    Ok(s) => snaps.push(s),
                    // Don't silently drop a snapshot the viewer can't parse —
                    // with serde(default) this should be rare, but if it happens
                    // the user needs to know history is incomplete.
                    Err(e) => {
                        eprintln!("  skipping {}: {e}", path.display())
                    },
                }
            }
        }
    }

    // Order commits topologically from the index (WS0.9e). Synthesize a
    // date-only entry (from recorded_at) for any snapshot rev the index doesn't
    // know yet, so ordering degrades gracefully to timestamp — never crashes.
    // This is in-memory only; the durable index.json is written by `record` /
    // `index`, not here.
    let mut idx = index::load(Path::new(index::INDEX_PATH));
    for s in &snaps {
        idx.entry(s.git_rev.clone()).or_insert_with(|| index::IndexEntry {
            date: s.recorded_at,
            parent: String::new(),
            branch: String::new(),
        });
    }
    let revs: Vec<String> = snaps.iter().map(|s| s.git_rev.clone()).collect();
    let order = index::topo_order(&idx, &revs);
    let pos: HashMap<&str, usize> =
        order.iter().enumerate().map(|(i, r)| (r.as_str(), i)).collect();
    snaps.sort_by_key(|s| *pos.get(s.git_rev.as_str()).unwrap_or(&usize::MAX));

    let snapshots_json = serde_json::to_string(&snaps).unwrap();
    let index_json = serde_json::to_string(&idx).unwrap();
    let out = dir.parent().unwrap_or(Path::new(".")).join("index.html");
    fs::write(&out, page(&snapshots_json, &index_json))?;
    println!("wrote {} ({} snapshots)", out.display(), snaps.len());
    Ok(())
}

/// The pre-built Vue dashboard (single self-contained file). Rebuild with:
///   (cd metrics-probe/viewer && npm ci && npm run build)
/// then rebuild this crate. See `metrics-probe/viewer/README.md`.
const VIEWER: &str = include_str!("../viewer/dist/index.html");

/// Inject the ordered snapshot + index JSON into the dashboard's
/// `<script type="application/json" id="metrics-data">` block. The `__DATA__`
/// placeholder appears exactly once in the built file (guarded in
/// `viewer/index.html`); the app reads and `JSON.parse`s the block on load.
///
/// The metric data is numbers, git hashes, scenario/bench ids and `name-rev`
/// hints — none can contain the `</script>` sequence that would close the block
/// early, so raw JSON injection is safe here.
fn page(snapshots_json: &str, index_json: &str) -> String {
    let data = format!(r#"{{"snapshots":{snapshots_json},"index":{index_json}}}"#);
    debug_assert_eq!(
        VIEWER.matches("__DATA__").count(),
        1,
        "viewer/dist/index.html must contain the __DATA__ placeholder exactly once — rebuild the viewer"
    );
    VIEWER.replace("__DATA__", &data)
}
