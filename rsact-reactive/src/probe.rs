//! Externally-polled reactions — the third reactive taxonomy alongside memos
//! and effects (WS2).
//!
//! - **memo** — a *lazy cached value*: recomputes on read, cuts propagation
//!   when its value is unchanged ([`crate::memo::Memo`]).
//! - **effect** — a *self-scheduling* side effect: the runtime queues and
//!   flushes it in topological order ([`crate::effect::Effect`]).
//! - **probe** — an *externally polled* reaction: it runs **only** when its
//!   owner calls [`Probe::poll`], and then only if a tracked dependency
//!   changed since the last poll (or the caller forces it). Nothing schedules
//!   a probe; the owner drives it (e.g. a render loop that redraws a widget
//!   part only when its reactive inputs changed).
//!
//! A [`Probe`] is a `Copy` handle whose identity *is* the handle (a
//! [`ValueId`] newtype, the same species as [`crate::signal::Signal`]) — there
//! is no registry, no keys, and no hashing. The owner stores the handle where
//! the reaction lives and disposes it with its owner. This crate stays
//! UI-vocabulary-free: it knows nothing about elements, parts, or pages — the
//! ownership map lives with the owner.

use crate::{runtime::with_current_runtime, storage::ValueId};
use core::panic::Location;

/// A `Copy` handle to an externally-polled reaction (see the [module
/// docs](self)). Identity is the handle itself; create one with
/// [`create_probe`] and drive it with [`Probe::poll`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Probe(pub(crate) ValueId);

/// Create a new [`Probe`], born dirty so its first [`poll`](Probe::poll) runs.
///
/// The probe is owned by the innermost active scope (like any reactive value),
/// so it is disposed when that scope drops. Its identity is the returned
/// handle — polling a disposed probe returns `None`.
#[track_caller]
pub fn create_probe() -> Probe {
    let caller = Location::caller();
    Probe(with_current_runtime(|rt| rt.create_probe(caller)))
}

impl Probe {
    /// Run `f` (with reactive reads tracked) iff a dependency changed since the
    /// last poll, or `force` is `true`. Returns `Some(f())` when `f` ran, or
    /// `None` if nothing changed (or the probe has been disposed).
    #[track_caller]
    pub fn poll<R>(&self, force: bool, f: impl FnOnce() -> R) -> Option<R> {
        let caller = Location::caller();
        with_current_runtime(|rt| rt.run_probe(self.0, force, caller, f))
    }

    /// The underlying [`ValueId`] — for owners that store probes in their own
    /// maps and need a stable key.
    pub fn id(&self) -> ValueId {
        self.0
    }

    /// Whether this probe is still alive (not disposed).
    pub fn is_alive(&self) -> bool {
        with_current_runtime(|rt| rt.is_alive(self.0))
    }

    /// Dispose the probe, detaching it from the dependency graph. `unsafe` for
    /// the same reason as every [`crate::ReactiveValue::dispose`]: calling it
    /// while a live edge still points at the node risks use-after-free. Prefer
    /// letting the owning scope dispose it.
    pub unsafe fn dispose(self) {
        with_current_runtime(|rt| unsafe { rt.dispose(self.0) });
    }
}

#[cfg(test)]
mod tests {
    use crate::{prelude::*, probe::create_probe, runtime::with_new_runtime};
    use alloc::rc::Rc;
    use core::cell::Cell;

    /// A probe runs its closure on the first poll (born dirty), skips polls
    /// where nothing changed, and re-runs when a tracked dependency changes.
    #[test]
    fn probe_runs_only_when_dep_changed() {
        with_new_runtime(|_| {
            let mut sig = create_signal(0u32);
            let runs = Rc::new(Cell::new(0u32));
            let probe = create_probe();

            let poll = |sig: Signal<u32>| {
                let r = runs.clone();
                probe.poll(false, move || {
                    r.set(r.get() + 1);
                    sig.get()
                })
            };

            // First poll: the probe is born Dirty, so it runs.
            assert_eq!(poll(sig), Some(0));
            assert_eq!(runs.get(), 1);

            // Nothing changed: the poll is a no-op.
            assert_eq!(poll(sig), None);
            assert_eq!(runs.get(), 1);

            // The tracked dependency changed: the probe re-runs.
            sig.set(5);
            assert_eq!(poll(sig), Some(5));
            assert_eq!(runs.get(), 2);
        });
    }

    /// Dependencies are re-tracked on every executed poll: a source that is no
    /// longer read (a conditionally-dropped dependency) is unsubscribed, so a
    /// later write to it does NOT re-run the probe. This is the `clear_sources`
    /// split — without it, a probe's sources accumulate append-only and a stale
    /// dependency spuriously re-runs it.
    #[test]
    fn probe_conditional_dep_unsubscribed() {
        with_new_runtime(|_| {
            let mut cond = create_signal(true);
            let mut a = create_signal(0u32);
            let mut b = create_signal(0u32);
            let runs = Rc::new(Cell::new(0u32));
            let probe = create_probe();

            let poll = |cond: Signal<bool>, a: Signal<u32>, b: Signal<u32>| {
                let r = runs.clone();
                probe.poll(false, move || {
                    r.set(r.get() + 1);
                    if cond.get() { a.get() } else { b.get() }
                })
            };

            // First poll tracks {cond, a}.
            assert_eq!(poll(cond, a, b), Some(0));
            assert_eq!(runs.get(), 1);

            // `a` is a live dependency: writing it re-runs.
            a.set(1);
            assert!(poll(cond, a, b).is_some());
            assert_eq!(runs.get(), 2);

            // Flip the condition: this poll re-runs and now tracks {cond, b};
            // `a` must be unsubscribed by the per-poll `clear_sources`.
            cond.set(false);
            assert!(poll(cond, a, b).is_some());
            assert_eq!(runs.get(), 3);

            // `a` is no longer a dependency: writing it must NOT re-run.
            a.set(2);
            assert_eq!(poll(cond, a, b), None);
            assert_eq!(runs.get(), 3);

            // `b` is the live dependency now: writing it re-runs.
            b.set(1);
            assert!(poll(cond, a, b).is_some());
            assert_eq!(runs.get(), 4);
        });
    }

    /// `force` runs the closure even when no dependency changed.
    #[test]
    fn probe_force_runs_without_change() {
        with_new_runtime(|_| {
            let runs = Rc::new(Cell::new(0u32));
            let probe = create_probe();
            let poll = |force: bool| {
                let r = runs.clone();
                probe.poll(force, move || r.set(r.get() + 1))
            };

            assert!(poll(false).is_some()); // born dirty
            assert_eq!(runs.get(), 1);
            assert!(poll(false).is_none()); // nothing changed
            assert_eq!(runs.get(), 1);
            assert!(poll(true).is_some()); // forced despite no change
            assert_eq!(runs.get(), 2);
        });
    }

    /// A disposed probe returns an honest `None` (even when forced) — identity
    /// is the handle, so there is no revive. A freshly created probe runs
    /// exactly once on its first poll.
    #[test]
    fn probe_disposed_returns_none_recreated_runs_once() {
        with_new_runtime(|_| {
            let runs = Rc::new(Cell::new(0u32));
            let probe = create_probe();

            let bump = |p: super::Probe, force: bool| {
                let r = runs.clone();
                p.poll(force, move || r.set(r.get() + 1))
            };

            assert!(bump(probe, false).is_some());
            assert_eq!(runs.get(), 1);

            unsafe { probe.dispose() };
            assert!(!probe.is_alive());
            // Even a forced poll of a dead handle is a no-op.
            assert!(bump(probe, true).is_none());
            assert_eq!(runs.get(), 1);

            // A fresh probe (a "recreated" one, e.g. on the next render) runs
            // exactly once.
            let probe2 = create_probe();
            assert!(bump(probe2, false).is_some());
            assert_eq!(runs.get(), 2);
            assert!(bump(probe2, false).is_none());
            assert_eq!(runs.get(), 2);
        });
    }

    /// A nested probe (created once, polled inside a parent probe — the
    /// arena-owned model) is NOT disposed and NOT re-run when only the parent's
    /// dependency changes: it re-runs iff its *own* dependency changed. This is
    /// the WS2 render-identity fix (G2 / invariants I1, I3).
    #[test]
    fn nested_probe_survives_parent_rerun() {
        with_new_runtime(|_| {
            let mut outer_sig = create_signal(0u32);
            let mut inner_sig = create_signal(0u32);
            let outer_runs = Rc::new(Cell::new(0u32));
            let inner_runs = Rc::new(Cell::new(0u32));

            // Both probes are created once and reused — the way an owner (e.g.
            // `ElState`) stores its part probes. `inner` is NOT owned by
            // `outer`; it is polled *inside* `outer`'s closure.
            let inner = create_probe();
            let outer = create_probe();

            let run = |outer_sig: Signal<u32>, inner_sig: Signal<u32>| {
                let (or, ir) = (outer_runs.clone(), inner_runs.clone());
                outer.poll(false, move || {
                    or.set(or.get() + 1);
                    outer_sig.get();
                    let ir2 = ir.clone();
                    inner.poll(false, move || {
                        ir2.set(ir2.get() + 1);
                        inner_sig.get();
                    });
                })
            };

            run(outer_sig, inner_sig);
            assert_eq!((outer_runs.get(), inner_runs.get()), (1, 1));

            // No change: neither runs.
            assert!(run(outer_sig, inner_sig).is_none());
            assert_eq!((outer_runs.get(), inner_runs.get()), (1, 1));

            // Only the *parent* dependency changed: the parent re-runs, the
            // child does NOT (its dependency is unchanged) and stays alive.
            outer_sig.set(1);
            assert!(run(outer_sig, inner_sig).is_some());
            assert_eq!((outer_runs.get(), inner_runs.get()), (2, 1));
            assert!(
                inner.is_alive(),
                "parent re-run disposed the nested probe"
            );

            // The child's own dependency changed: the child re-runs.
            inner_sig.set(1);
            assert!(run(outer_sig, inner_sig).is_some());
            assert_eq!(inner_runs.get(), 2);
        });
    }

    /// `clear_sources` (the per-poll edge detach) must NOT dispose values the
    /// probe owns — that ownership-disposal was the old `cleanup` bug that
    /// nuked nested probes on a parent re-run. Owned values survive re-runs and
    /// are disposed only when the probe itself is disposed.
    #[test]
    fn probe_rerun_keeps_owned_values_dispose_clears_them() {
        with_new_runtime(|_| {
            let trigger = create_trigger();
            let probe = create_probe();
            let owned = Rc::new(Cell::new(None::<Signal<u32>>));

            let run = || {
                let slot = owned.clone();
                probe.poll(false, move || {
                    trigger.track();
                    // Created while `probe` is the active observer → owned by it.
                    slot.set(Some(create_signal(0u32)));
                });
            };

            run();
            let first = owned.get().unwrap();
            assert!(first.is_alive());

            // Re-run detaches edges but keeps owned values alive.
            trigger.notify();
            run();
            assert!(first.is_alive(), "owned value disposed by probe re-run");

            // Disposing the probe disposes what it owns.
            unsafe { probe.dispose() };
            assert!(
                !first.is_alive(),
                "owned value not disposed with the probe"
            );
        });
    }
}
