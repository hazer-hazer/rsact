//! The on-disk snapshot schema. One JSON file per commit under
//! `metrics/snapshots/`, keyed by `git rev-parse HEAD`. Layer 1 (framework
//! metrics: node counts, heap, allocs/frame, layout counters) is host-measured
//! and always present; Layer 2 (`.text/.rodata/.bss` per target) is optional
//! and only populated when the size probes are built.

use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

pub const SNAPSHOT_DIR: &str = "metrics/snapshots";

// `#[serde(default)]` on every schema struct is deliberate: it makes snapshots
// forward/backward compatible, so adding a field later (as WS0.3a's `observers`
// did) leaves older snapshots deserializable — a missing field takes its type
// default instead of failing the whole load and silently dropping the file.

/// Reactive-runtime node population, by kind, from `current_runtime_profile()`.
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeCounts {
    pub stored: usize,
    pub signals: usize,
    pub effects: usize,
    pub memos: usize,
    pub computed: usize,
    pub observers: usize,
    pub subscribers: usize,
    pub subscribers_bindings: usize,
    pub sources: usize,
    pub sources_bindings: usize,
    /// Sum of the node kinds (stored+signals+effects+memos+computed+observers).
    pub total: usize,
    /// WS4.6: value-SlotMap backing capacity — the peak node-slot high-water
    /// mark (it never shrinks on dispose), i.e. permanent node-slot RAM.
    pub values_capacity: usize,
    /// WS4.6: retained-but-unused slots (`values_capacity` − `total`): freed on
    /// dispose, reusable by future inserts, still costing RAM.
    pub values_vacant: usize,
}

/// Layout-pass work counters (WS0.5), populated when built with the
/// `rsact-ui/layout-counters` feature.
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutCounters {
    pub visits: u64,
    pub measures: u64,
}

/// One measured scenario.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Scenario {
    pub name: String,
    pub counts: NodeCounts,
    /// Steady-state heap retained by the built scenario (live bytes delta).
    pub heap_live_bytes: usize,
    /// Peak live bytes reached while building.
    pub heap_peak_bytes: usize,
    pub build_allocs: usize,
    pub build_bytes: usize,
    /// `None` if the frame could not be measured (e.g. render gate panicked).
    pub idle_frame_allocs: Option<usize>,
    pub change_frame_allocs: Option<usize>,
    pub layout: Option<LayoutCounters>,
}

/// `.text/.rodata/.bss` for one measured binary on one target (Layer 2).
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SectionSizes {
    pub target: String,
    pub binary: String,
    pub text: u64,
    pub rodata: u64,
    pub bss: u64,
}

/// One criterion benchmark's median wall-clock (WS0.9d). Recorded on the CI
/// runner only — a *self-consistent* trend, NOT a gating number: shared GitHub
/// runners carry ±10–30% noise, so this is charted with wide error bars and no
/// alert thresholds. Local criterion baselines stay the decision-grade A/B
/// instrument.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BenchMedian {
    /// Criterion bench id — its `target/criterion/<id>` path, `/`-joined
    /// (e.g. `primitives/signal_read_get`, `layout/layout_only`).
    pub id: String,
    /// Median estimate, nanoseconds.
    pub median_ns: f64,
    /// Half-width of criterion's confidence interval on the median, ns — the
    /// ± error bar the viewer draws (runner-noise framing).
    pub ci_half_ns: f64,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub git_rev: String,
    pub git_dirty: bool,
    /// Unix seconds when recorded (for display/ordering only).
    pub recorded_at: u64,
    /// Host target the Layer-1 metrics were measured on.
    pub host: String,
    pub scenarios: Vec<Scenario>,
    /// Layer-2 target section sizes, empty when the size probes weren't built.
    pub section_sizes: Vec<SectionSizes>,
    /// Criterion bench medians (WS0.9d), empty unless recorded with `--benches`
    /// (CI-runner-only trend; see [`BenchMedian`]).
    pub bench_medians: Vec<BenchMedian>,
}

impl Snapshot {
    /// File name for this snapshot: `<rev>.json`, or `<rev>-dirty.json` for a
    /// dirty tree (so a work-in-progress reading never overwrites the committed
    /// baseline for the same rev).
    pub fn file_name(&self) -> String {
        if self.git_dirty {
            format!("{}-dirty.json", self.git_rev)
        } else {
            format!("{}.json", self.git_rev)
        }
    }

    pub fn save(&self, dir: &Path) -> io::Result<std::path::PathBuf> {
        fs::create_dir_all(dir)?;
        let path = dir.join(self.file_name());
        fs::write(&path, serde_json::to_string_pretty(self).unwrap())?;
        Ok(path)
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        let text = fs::read_to_string(path)?;
        serde_json::from_str(&text)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // WS0.7f: a snapshot from an older schema (missing fields that were added
    // later — here: no `section_sizes`, no `observers`, no `layout`) must still
    // deserialize, taking type defaults, so `diff <old-rev>` and the HTML
    // regeneration never silently drop history.
    #[test]
    fn old_snapshot_missing_additive_fields_still_loads() {
        let legacy = r#"{
            "git_rev": "deadbeef",
            "git_dirty": false,
            "recorded_at": 1,
            "host": "x-y",
            "scenarios": [
                { "name": "reactive_only_16",
                  "counts": { "signals": 16 },
                  "heap_live_bytes": 100,
                  "idle_frame_allocs": 0 }
            ]
        }"#;
        let snap: Snapshot = serde_json::from_str(legacy)
            .expect("legacy snapshot must deserialize via serde(default)");
        assert_eq!(snap.git_rev, "deadbeef");
        assert!(snap.section_sizes.is_empty()); // field absent → default
        assert!(snap.bench_medians.is_empty()); // WS0.9d field absent → default
        let s = &snap.scenarios[0];
        assert_eq!(s.counts.signals, 16);
        assert_eq!(s.counts.observers, 0); // added later → default
        assert_eq!(s.idle_frame_allocs, Some(0));
        assert!(s.change_frame_allocs.is_none()); // absent → None
        assert!(s.layout.is_none());
    }
}
