//! WS6.4a: capture a real page's frame as a **tile schedule** and measure it.
//!
//! [`rsact_render::schedule`] owns the arithmetic; this module owns the driving.
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
//! # How a region pass is faked before 6.4c/6.4d exist
//!
//! A tile pass is a *second* pass over an already-painted frame, which today's
//! probe-gated render refuses: the probes went clean on the first pass (this is
//! precisely the conflict WS6.4c resolves by splitting collect from paint). The
//! harness gets its N passes with `Page::force_redraw`, whose flag WS6.4.0(iv)
//! turned into a value OR-ed into every part's gate — i.e. *paint everything,
//! regardless of probe state*, which is exactly the geometry-selected paint 6.4c
//! specifies. So the schedule this captures is the real thing minus the culling
//! 6.4b will add, which is the measurement wanted: today's cost, and the floor.
//!
//! One cost the harness deliberately keeps visible: forcing K passes pays
//! `Probe::poll`'s `clear_sources` + re-subscribe round trip K times. 6.4c calls
//! that out as the reason paint passes must be untracked; here it is accepted,
//! because a harness measures rather than optimises.
//!
//! [`ScheduleReport`]: rsact_render::schedule::ScheduleReport
//! [`tile_invariance`]: rsact_render::schedule::tile_invariance

use crate::{
    el::{arena::ElArena, ctx::Wtf, view::View},
    font::FontCtx,
    layout::model::LayoutModelNode,
    page::{Page, dev::DevTools},
    prelude::*,
    render::{
        record::{DrawOp, RecordingRenderer},
        schedule::{ScheduleLog, TilePass, TileSchedule},
    },
    test_support::TestPage,
};
use alloc::vec::Vec;
use rsact_reactive::{prelude::*, scope::new_scope};

/// The recording widget context: colour-agnostic op log, unit page-id / stylist /
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
                    viewport.maybe_reactive(),
                    ().inert(),
                    DevTools::default().signal(),
                    FontCtx::new().signal(),
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
        // Deliberately NOT forced: the point is which rects the probe-gated
        // render decided to repaint.
        self.page.use_renderer(|_| {});
        TileSchedule::from_regions(self.viewport(), self.page.damage_snapshot())
    }

    /// One forced frame, optionally announced as a region.
    fn frame(&mut self, region: Option<Rect>) -> Vec<DrawOp> {
        self.recorder.clear();
        self.page.force_redraw();

        if let Some(region) = region {
            // What a tiled frame does per region: `begin_region` says *where* we
            // are drawing (WS6.4.0(ii-3) — a no-op for a recorder, whose surface
            // already covers the frame), and the clip is what a future cull would
            // read (WS6.4.0(ii-1)). The recorder logs the clip as bookkeeping,
            // which `schedule` excludes from every count.
            // Unwrapped, not logged-and-continued: a region the renderer refused
            // to enter makes every number below meaningless, and this is a
            // harness, not the UI path WS1.8 governs.
            self.page
                .renderer
                .begin_region(region)
                .expect("recorder failed to begin a region");
            self.page.renderer.push_clip(region);
        }

        self.page.use_renderer(|_| {});

        if region.is_some() {
            self.page.renderer.pop_clip();
            self.page
                .renderer
                .end_region()
                .expect("recorder failed to end a region");
        }

        self.recorder.ops()
    }

    /// The traversal term: how many widget nodes a region schedule visits, as it
    /// stands and as WS6.4b's culling could make it.
    pub fn visits(&mut self, schedule: &TileSchedule) -> VisitReport {
        // `layout()` relayouts if needed and borrows the owned model
        // (WS6.4.0(iv)) — no clone of a recursive tree.
        let layout = self.page.layout();
        let root = layout.tree_root();

        let nodes = count_nodes(&root);
        VisitReport {
            nodes,
            regions: schedule.len(),
            visited: nodes * schedule.len(),
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
    /// obvious source (that is what `ClipPath::InnerRect` exists for). If this is
    /// non-zero, WS6.4b(i) cannot prune on `outer` alone — it must prune on the
    /// subtree's union extent, or only where a clip bounds the children.
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
