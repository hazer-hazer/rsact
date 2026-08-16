//! WS6.4a: capture a real page's frame as a **tile schedule** and measure it.
//!
//! [`rsact_render::test_support::schedule`] owns the arithmetic; this module owns the driving.
//! It builds a page against a [`RecordingRenderer`], captures the frame once
//! whole and once per region, and reports two independent cost terms:
//!
//! - **ops** ([`TileProbe::capture`]) — what the drawing code emits. Fed straight
//!   to [`ScheduleReport`] / [`tile_invariance`].
//! - **node visits** ([`TileProbe::visits`]) — the traversal term, which no op log
//!   can see: a transparent `Flex` emits nothing yet is still visited, styled and
//!   recursed through. Derived from the `LayoutModel`, so it needs no
//!   instrumentation in the render path at all.
//!
//! # How a region pass is driven (WS6.4c: for real, no longer simulated)
//!
//! A tile pass is a *second* pass over an already-painted frame, which the
//! probe-gated render used to refuse: the probes went clean on the first pass.
//! That is precisely the conflict 6.4c resolves by splitting **collect** from
//! **paint**, and until it landed this harness faked its way through with
//! `Page::force_redraw` — a flag OR-ed into every part's gate, i.e. "paint
//! everything regardless of probe state", which approximated geometry-selected
//! paint closely enough to measure.
//!
//! It no longer approximates anything. [`TileProbe::damage_after`] calls
//! [`Page::collect`] — the plan pass, tracked and probe-gated with its drawing
//! discarded — and each region goes through [`Page::paint_region`], the same
//! entry point 6.4d's frame driver will use: untracked, probe-free, selected by
//! geometry. The goldens did not move when the harness switched over, which is
//! the evidence that the simulation had been faithful.
//!
//! One cost the fake used to pay and the real API does not: forcing K passes
//! paid `Probe::poll`'s `clear_sources` + re-subscribe round trip K times. That
//! is exactly why 6.4c requires paint passes to be untracked, and it is now
//! paid once per frame by `collect` rather than once per region.
//!
//! [`Page::collect`]: crate::page::Page::collect
//! [`Page::paint_region`]: crate::page::Page::paint_region
//!
//! [`ScheduleReport`]: rsact_render::test_support::schedule::ScheduleReport
//! [`tile_invariance`]: rsact_render::test_support::schedule::tile_invariance

use crate::{
    el::{arena::ElArena, ctx::Wtf, view::View},
    font::FontCtx,
    layout::model::LayoutModelNode,
    page::{Page, dev::DevTools},
    prelude::*,
    render::{
        record::{DrawOp, RecordingRenderer},
        test_support::schedule::{ScheduleLog, TilePass, TileSchedule},
    },
    test_support::TestPage,
};
use alloc::vec::Vec;
use rsact_reactive::{prelude::*, scope::new_scope};

/// The recording widget context: color-agnostic op log, unit page-id / stylist /
/// event. Same shape the WS6.9 goldens use.
pub type RecWtf = Wtf<RecordingRenderer<NullColor>, (), (), ()>;

/// A built page wired to an op recorder, replayable region by region.
pub struct TileProbe {
    page: TestPage<RecWtf>,
    /// Shares the op log with the page's renderer (`Rc` inside).
    recorder: RecordingRenderer<NullColor>,
    viewport: Size,
}

impl TileProbe {
    /// Build `root` into a page of `viewport` size and settle its reactive
    /// render, so a later capture measures a steady state rather than start-up.
    pub fn new(viewport: Size, root: impl View<RecWtf>) -> Self {
        let renderer = RecordingRenderer::<NullColor>::new(viewport);
        let recorder = renderer.clone();
        let arena = create_signal(ElArena::new()).name("Page arena");
        let scope = new_scope();
        let mut probe = Self {
            page: TestPage::new(
                Page::new(
                    (),
                    root,
                    arena,
                    viewport,
                    ().inert(),
                    DevTools::default().signal(),
                    alloc::rc::Rc::new(FontCtx::new()),
                    scope,
                ),
                renderer,
            ),
            recorder,
            viewport,
        };

        // The first frames may run the render gate a few times while reactive
        // state stabilises; discard them (mirrors the WS6.9 golden harness).
        for _ in 0..4 {
            probe.page.use_renderer(|_| {});
        }

        probe
    }

    pub fn viewport(&self) -> Rect {
        Rect::new(Point::zero(), self.viewport)
    }

    /// The page, for interactions that reach the state under test (events, signal
    /// writes) before a capture.
    pub fn page(&mut self) -> &mut Page<RecWtf> {
        &mut self.page
    }

    /// Capture the same frame twice: once over the whole viewport, then once per
    /// region of `schedule`.
    pub fn capture(&mut self, schedule: &TileSchedule) -> ScheduleLog {
        let full = self.frame(None);
        let passes = schedule
            .tiles()
            .iter()
            .map(|&tile| TilePass { tile, ops: self.frame(Some(tile)) })
            .collect();
        ScheduleLog { full, passes }
    }

    /// Run `interact`, then one **ordinary** (unforced) frame, and return the
    /// damage that frame recorded as a region schedule.
    ///
    /// This is the schedule WS6.4d will actually be handed: rsact's own
    /// damage rects for a real change, not an invented partition. It is what
    /// makes the cost model's "interactive frames +0%" claim checkable — a
    /// forced-full schedule can only ever measure the cold-frame case.
    pub fn damage_after(
        &mut self,
        interact: impl FnOnce(&mut Page<RecWtf>),
    ) -> TileSchedule {
        interact(&mut self.page);
        self.recorder.clear();
        // Deliberately NOT forced: the point is which rects the render decided
        // to repaint.
        //
        // WS6.4c: this is now a `collect` — the plan pass — rather than a
        // painting frame. Same rects (both modes record damage identically),
        // and the goldens prove it, but it is what 6.4d is actually handed:
        // regions derived from a pass that painted nothing yet.
        self.page.collect();
        TileSchedule::from_regions(self.viewport(), self.page.damage_snapshot())
    }

    /// One frame: the whole viewport as a forced [`Fused`] pass, or one region
    /// as a real [`Paint`] pass.
    ///
    /// **WS6.4c: the region path is no longer a simulation.** It used to force a
    /// redraw and re-run the probe-gated pass under a hand-pushed clip, because
    /// a second pass over an already-painted frame otherwise finds every probe
    /// clean and draws nothing — `force_redraw` was the only way through before
    /// the collect/paint split existed. It now calls [`Page::paint_region`], the
    /// same entry point WS6.4d's frame driver will use: untracked, probe-free,
    /// selected by geometry, with `begin_region`/`push_clip` handled inside.
    ///
    /// The full-frame reference stays `Fused`, which is exactly the comparison
    /// tile-invariance wants: *does the union of the region passes reconstruct
    /// the frame the full-framebuffer path would have painted?*
    ///
    /// [`Fused`]: crate::el::render::RenderMode::Fused
    /// [`Paint`]: crate::el::render::RenderMode::Paint
    /// [`Page::paint_region`]: crate::page::Page::paint_region
    fn frame(&mut self, region: Option<Rect>) -> Vec<DrawOp> {
        self.recorder.clear();

        match region {
            Some(region) => {
                // Unwrapped, not logged-and-continued: a region the renderer
                // refused to enter makes every number below meaningless, and
                // this is a harness, not the UI path WS1.8 governs.
                self.page.paint_region(region).expect("paint_region failed");
            },
            None => {
                self.page.force_redraw();
                self.page.use_renderer(|_| {});
            },
        }

        self.recorder.ops()
    }

    /// The traversal term: how many widget nodes a region schedule visits — the
    /// modelled floor, and (WS6.4c(E)) what the walk **actually** does.
    pub fn visits(&mut self, schedule: &TileSchedule) -> VisitReport {
        // MEASURED first: paint each region for real and read the page's own
        // counter. `cullable` below is the model; this is the implementation,
        // and the two agreeing is the claim worth making.
        let measured = schedule
            .tiles()
            .iter()
            .map(|&tile| {
                self.page.paint_region(tile).expect("paint_region failed");
                self.page.nodes_visited()
            })
            .sum();

        // `layout()` relayouts if needed and borrows the owned model
        // (WS6.4.0(iv)) — no clone of a recursive tree.
        let layout = self.page.layout();
        let root = layout.tree_root();

        let nodes = count_nodes(&root);
        VisitReport {
            nodes,
            regions: schedule.len(),
            visited: nodes * schedule.len(),
            measured,
            cullable: schedule
                .tiles()
                .iter()
                .map(|&tile| count_reachable(&root, tile))
                .sum(),
            escaping: count_escaping(&root),
        }
    }
}

/// The per-object traversal cost of a schedule — the term the op log cannot see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisitReport {
    /// Nodes in the layout tree (transparent wrappers are already flattened out
    /// of it by `model_layout`, so this is the set the render walk dispatches to).
    pub nodes: usize,
    /// WS6.4c(E): nodes the walk **actually** processed across the schedule,
    /// counted by the page rather than modelled. Before the traversal prune this
    /// equalled `visited`; it should now equal `cullable`.
    pub measured: usize,
    pub regions: usize,
    /// Visits a region schedule performs today: the walk has no geometry test at
    /// all, so this is exactly `nodes * regions`.
    pub visited: usize,
    /// Visits left under WS6.4b(i)'s rule — prune a subtree whose `outer` misses
    /// the region. The ratio to [`Self::nodes`] is the per-object multiplier the
    /// WS6.4 cost model estimates at ×1.6–2.0.
    pub cullable: usize,
    /// Nodes whose `outer` is **not** inside their parent's `outer`.
    ///
    /// A load-bearing soundness number, not a curiosity: pruning a subtree on the
    /// parent's rect silently drops these children. Scrollable content is the
    /// obvious source. If this is non-zero, WS6.4b(i) cannot prune on `outer`
    /// alone — it must prune on the subtree's **union extent**.
    ///
    /// **RESOLVED 2026-08-06 (WS6.4c(F)) — read this number differently now.**
    /// When it was written, "prune under `outer` except where a clip bounds the
    /// children" was unavailable: `ElState::clip_path` was initialised to `None`
    /// and set nowhere, so the clip arm was unreachable and overflowing content
    /// was bounded only by the framebuffer viewport. Clipping is now declared
    /// behaviour (`WidgetFlags::CLIPS_CHILDREN`, set by `Scrollable`), the
    /// framework pushes it around the children loop, and `Renderer::clip_bounds`
    /// composes it — so containment is structural and the prune needs no
    /// per-node storage at all.
    ///
    /// Consequently this counter now **over-reports**: it compares layout rects,
    /// so a child that escapes a *clipping* parent still counts, even though it
    /// is provably invisible and prunable. It stays as-is deliberately — it is
    /// the regression that shows whether the geometry still overflows, which is
    /// what `ext_draw`/`paint_bounds` will need — but "escaping > 0" no longer
    /// implies "cannot prune on `outer`".
    pub escaping: usize,
}

impl VisitReport {
    /// `visited / nodes` — today's traversal multiplier.
    pub fn visited_multiplier(&self) -> f32 {
        ratio(self.visited, self.nodes)
    }

    /// `cullable / nodes` — the floor culling could reach.
    pub fn cullable_multiplier(&self) -> f32 {
        ratio(self.cullable, self.nodes)
    }
}

impl core::fmt::Display for VisitReport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(
            f,
            "{:<14}{:>9}{:>9}{:>9}{:>9}{:>9}",
            "visits", "nodes", "visited", "cullable", "xvisit", "xcull"
        )?;
        writeln!(
            f,
            "{:<14}{:>9}{:>9}{:>9}{:>9.2}{:>9.2}",
            "nodes",
            self.nodes,
            self.visited,
            self.cullable,
            self.visited_multiplier(),
            self.cullable_multiplier()
        )?;
        writeln!(
            f,
            "{:<14}{:>9}{:>9}{:>9}{:>9.2}{:>9.2}",
            "nodes(real)",
            self.nodes,
            self.measured,
            self.cullable,
            ratio(self.measured, self.nodes),
            self.cullable_multiplier()
        )?;
        writeln!(f, "escaping {}", self.escaping)
    }
}

fn ratio(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 { 0.0 } else { numerator as f32 / denominator as f32 }
}

fn count_nodes(node: &LayoutModelNode<'_>) -> usize {
    1 + node
        .children()
        .map(|child| count_nodes(&child))
        .sum::<usize>()
}

/// Nodes a WS6.4b subtree prune would still visit for `region`.
fn count_reachable(node: &LayoutModelNode<'_>, region: Rect) -> usize {
    if !node.outer.intersects(&region) {
        return 0;
    }
    1 + node
        .children()
        .map(|child| count_reachable(&child, region))
        .sum::<usize>()
}

/// Children that reach outside their parent's `outer` — see
/// [`VisitReport::escaping`].
fn count_escaping(node: &LayoutModelNode<'_>) -> usize {
    node.children()
        .map(|child| {
            // Containment without a new geometry method: a rect contains another
            // exactly when their union adds nothing.
            let escapes =
                usize::from(node.outer.union(&child.outer) != node.outer);
            escapes + count_escaping(&child)
        })
        .sum()
}
