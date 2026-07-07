//! A global allocator that tracks allocation *churn* (count + bytes requested,
//! so freed-immediately allocations still show up — what matters for heap
//! fragmentation on an MCU) and *live* bytes (currently-outstanding, plus its
//! peak). Shared measurement primitive so `rsact-reactive`'s allocation bench
//! and the `metrics-probe` tool count identically (they used to have separate
//! copies that could drift and make their numbers incomparable — WS0.7j).
//!
//! Install it in a binary/bench with `#[global_allocator] static A: Tracking =
//! Tracking;`. std-only and `#[doc(hidden)]`: a measurement utility, not part of
//! the public reactive API.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

pub struct Tracking;

fn record_alloc(size: usize) {
    ALLOCS.fetch_add(1, Relaxed);
    BYTES.fetch_add(size, Relaxed);
    let live = LIVE.fetch_add(size, Relaxed) + size;
    // Monotonically raise the peak. Racy across threads, but the probe/bench run
    // the measured code single-threaded so this is exact in practice.
    PEAK.fetch_max(live, Relaxed);
}

unsafe impl GlobalAlloc for Tracking {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        record_alloc(l.size());
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        record_alloc(l.size());
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        // Count a realloc as one allocation event; charge the byte counters
        // only the growth, but track the true live delta.
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(new.saturating_sub(l.size()), Relaxed);
        if new >= l.size() {
            let grow = new - l.size();
            let live = LIVE.fetch_add(grow, Relaxed) + grow;
            PEAK.fetch_max(live, Relaxed);
        } else {
            LIVE.fetch_sub(l.size() - new, Relaxed);
        }
        unsafe { System.realloc(p, l, new) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Relaxed);
        unsafe { System.dealloc(p, l) }
    }
}

/// A point-in-time reading of the cumulative churn counters.
#[derive(Clone, Copy)]
pub struct Counters {
    pub allocs: usize,
    pub bytes: usize,
}

pub fn read() -> Counters {
    Counters { allocs: ALLOCS.load(Relaxed), bytes: BYTES.load(Relaxed) }
}

pub fn live() -> usize {
    LIVE.load(Relaxed)
}

/// Reset the peak watermark to the current live bytes, so a following
/// measurement window reports its own peak.
pub fn reset_peak() {
    PEAK.store(LIVE.load(Relaxed), Relaxed);
}

pub fn peak() -> usize {
    PEAK.load(Relaxed)
}
