//! Layout-pass work counters (WS0.5), compiled only under the
//! `layout-counters` feature.
//!
//! Two global counters expose how much work a layout pass does:
//!
//! - **visits** — one per [`super::model::model_layout`] entry. Because
//!   `model_flex` re-enters `model_layout` for each child across its several
//!   sizing/placement passes, this naturally counts the multi-pass re-visits a
//!   fluid flex does (the reason a single leaf change costs ~visits ≫ node
//!   count today, pre-WS5 incremental layout).
//! - **measures** — one per text-measurement call the layout makes
//!   (`ContentLayout::content_sizing` / `height_for_width`).
//!
//! They are process-global (a layout pass is single-threaded on the reactive
//! thread), so a measurement resets them, runs one pass, and reads the deltas.
//! `metrics-probe` feeds these into the 0.3 snapshot; a rsact-ui baseline test
//! locks them. `portable_atomic` keeps them buildable on every target even
//! though the feature is host/dev-only.

use portable_atomic::{AtomicU64, Ordering::Relaxed};

static VISITS: AtomicU64 = AtomicU64::new(0);
static MEASURES: AtomicU64 = AtomicU64::new(0);

#[inline]
pub(crate) fn count_visit() {
    VISITS.fetch_add(1, Relaxed);
}

#[inline]
pub(crate) fn count_measure() {
    MEASURES.fetch_add(1, Relaxed);
}

/// Reset both counters to zero (call before the pass you want to measure).
pub fn reset() {
    VISITS.store(0, Relaxed);
    MEASURES.store(0, Relaxed);
}

/// Current `(visits, measures)` reading.
pub fn snapshot() -> (u64, u64) {
    (VISITS.load(Relaxed), MEASURES.load(Relaxed))
}
