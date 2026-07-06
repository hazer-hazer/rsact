//! The on-disk snapshot schema. One JSON file per commit under
//! `metrics/snapshots/`, keyed by `git rev-parse HEAD`. Layer 1 (framework
//! metrics: node counts, heap, allocs/frame, layout counters) is host-measured
//! and always present; Layer 2 (`.text/.rodata/.bss` per target) is optional
//! and only populated when the size probes are built.

use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

pub const SNAPSHOT_DIR: &str = "metrics/snapshots";

/// Reactive-runtime node population, by kind, from `current_runtime_profile()`.
#[derive(Clone, Copy, Serialize, Deserialize)]
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
}

/// Layout-pass work counters (WS0.5), populated when built with the
/// `rsact-ui/layout-counters` feature.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct LayoutCounters {
    pub visits: u64,
    pub measures: u64,
}

/// One measured scenario.
#[derive(Clone, Serialize, Deserialize)]
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
#[derive(Clone, Serialize, Deserialize)]
pub struct SectionSizes {
    pub target: String,
    pub binary: String,
    pub text: u64,
    pub rodata: u64,
    pub bss: u64,
}

#[derive(Clone, Serialize, Deserialize)]
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
