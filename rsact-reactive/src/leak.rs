//! Leak-attribution diagnostics (WS3.0a).
//!
//! The 0.3 metrics *detect* leaks — node counts move when something is not
//! disposed. This module *attributes* them: take a [`leak_snapshot`] of the
//! live node-set before a page/subtree build, then after that build has been
//! disposed ask [`leak_report`] which nodes survived. Every survivor is a node
//! created since the snapshot that nothing disposed — reported with the
//! `file:line` that created it (under `debug-info`, via the `Location`
//! breadcrumb already recorded in each value's debug info).
//!
//! This is pure plumbing over existing state: the storage slotmap's live keys
//! plus the per-value debug info. No new tracking is added to the hot path.

use crate::{
    runtime::with_current_runtime,
    storage::{ValueId, ValueKindTag},
};
use alloc::vec::Vec;
use core::fmt::Display;

/// The set of live reactive-node ids captured at the moment [`leak_snapshot`]
/// ran. Diff a later runtime state against it with [`leak_report`].
#[derive(Clone, Debug)]
pub struct LeakSnapshot {
    live: Vec<ValueId>,
}

/// Snapshot every currently-live reactive node so a later [`leak_report`] can
/// tell which nodes were created (and not disposed) since.
#[must_use]
pub fn leak_snapshot() -> LeakSnapshot {
    with_current_runtime(|rt| LeakSnapshot {
        live: rt.storage.values.borrow().keys().collect(),
    })
}

/// One node that was created after a [`LeakSnapshot`] and is still alive.
#[derive(Clone)]
pub struct LeakedNode {
    pub id: ValueId,
    pub kind: ValueKindTag,
    /// Creation site + name/type of the survivor. Only available under
    /// `debug-info`; without it, only `id`/`kind` identify the leak.
    #[cfg(feature = "debug-info")]
    pub debug: crate::storage::ValueDebugInfo,
}

/// Nodes alive now that were not alive at snapshot time. If the build they
/// belong to has been disposed, every entry here is a leak.
#[derive(Clone)]
pub struct LeakReport {
    pub survivors: Vec<LeakedNode>,
}

impl LeakReport {
    /// No node survived the disposal — the clean result.
    pub fn is_empty(&self) -> bool {
        self.survivors.is_empty()
    }

    /// Number of surviving (leaked) nodes.
    pub fn len(&self) -> usize {
        self.survivors.len()
    }
}

/// Diff the live node-set against `snapshot`: every node alive now that was not
/// alive at snapshot time is reported as a survivor.
#[must_use]
pub fn leak_report(snapshot: &LeakSnapshot) -> LeakReport {
    with_current_runtime(|rt| {
        let values = rt.storage.values.borrow();
        // A survivor is any node alive now that was not alive at snapshot time.
        // Linear membership against the snapshot: this is a debug diagnostic,
        // not a hot path, so correctness (only `PartialEq` on `ValueId`) beats
        // the constant factor.
        let survivors = values
            .iter()
            .filter(|(id, _)| !snapshot.live.contains(id))
            .map(|(id, value)| LeakedNode {
                id,
                kind: value.kind.tag(),
                #[cfg(feature = "debug-info")]
                debug: value.debug,
            })
            .collect();
        LeakReport { survivors }
    })
}

impl Display for LeakReport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.survivors.is_empty() {
            return write!(f, "No leaked reactive nodes.");
        }
        writeln!(f, "{} leaked reactive node(s):", self.survivors.len())?;
        for node in &self.survivors {
            #[cfg(feature = "debug-info")]
            writeln!(f, "  - {:?} {}", node.kind, node.debug)?;
            #[cfg(not(feature = "debug-info"))]
            writeln!(
                f,
                "  - {:?} {:?} (enable `debug-info` for creation site)",
                node.kind, node.id
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{leak_report, leak_snapshot};
    use crate::{
        prelude::create_signal, runtime::with_new_runtime, scope::new_scope,
        storage::ValueKindTag,
    };

    /// A signal created (and never disposed) after a snapshot must show up as a
    /// survivor, tagged with its kind.
    #[test]
    fn leak_report_flags_undisposed_value() {
        with_new_runtime(|_| {
            let snap = leak_snapshot();
            let _leaked = create_signal(0i32); // no scope, never disposed
            let report = leak_report(&snap);
            assert_eq!(report.len(), 1, "expected exactly one survivor");
            assert_eq!(report.survivors[0].kind, ValueKindTag::Signal);
        });
    }

    /// Under `debug-info`, each survivor carries the `file:line` that created
    /// it — the attribution 3.0a exists to provide.
    #[cfg(feature = "debug-info")]
    #[test]
    fn leak_report_attributes_creation_site() {
        with_new_runtime(|_| {
            let snap = leak_snapshot();
            let _leaked = create_signal(0i32);
            let report = leak_report(&snap);
            assert_eq!(report.len(), 1);
            let site = report.survivors[0].debug.created_at;
            assert!(
                site.file().ends_with("leak.rs"),
                "survivor creation site not attributed to caller: {site}"
            );
        });
    }

    /// Values created inside a scope that is then dropped leave no survivors.
    #[test]
    fn leak_report_clean_after_scope_drop() {
        with_new_runtime(|_| {
            let snap = leak_snapshot();
            {
                let _scope = new_scope();
                let _a = create_signal(0i32);
                let _b = create_signal(1u8);
            }
            let report = leak_report(&snap);
            assert!(report.is_empty(), "clean scope leaked: {report}");
        });
    }
}
