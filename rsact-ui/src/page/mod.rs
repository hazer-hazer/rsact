use crate::{
    el::{arena::ElArena, build::BuildCtx, *},
    event::{
        Capture, Event, EventResponse, MouseButton, MouseEvent, PressEvent,
        UnhandledEvent,
    },
    font::{Font, FontCtx, FontProps},
    layout::{
        LayoutCtx, Limits,
        model::{LayoutModel, PPLayoutModel, model_layout},
    },
    render::prelude::*,
    style::TreeStyle,
};
use alloc::{rc::Rc, vec::Vec};
use core::cell::{Cell, RefCell};
use dev::{DevHoveredEl, DevTools};
use log::{debug, error, info, warn};
use rsact_reactive::prelude::*;
use rsact_reactive::scope::ScopeHandle;

pub mod dev;
pub mod id;

pub struct PageStyle<C: Color> {
    // TODO: Use ColorStyle
    pub background_color: Option<C>,
}

impl<C: Color> PageStyle<C> {
    pub fn base() -> Self {
        // TODO: Base should be None and user should set theme/palette
        Self { background_color: Some(C::default_background()) }
    }

    pub fn background_color(mut self, background_color: C) -> Self {
        self.background_color = Some(background_color);
        self
    }
}

// TODO: As we now have element arena we can split functionality to add
// optimization structures. For example, not all drivers have focusing logic, so
// we can have it behind a generic or other abstraction to toggle, and pre-build
// focusable elements list only when it's needed. Same for hoverable, etc.
// Hoverable is kinda more interesting case because with element arena we can
// build spatial index like an R-Tree or just cache hit-tests which is also
// good.

pub struct Page<W: WidgetCtx> {
    id: W::PageId,
    root: ElId,
    needs_redraw: bool,
    arena: Signal<ElArena<W>>,
    /// The page's layout, owned inline — **not** a reactive value (WS6.4.0(iv)).
    ///
    /// It used to be a `Memo<LayoutModel>`, which made recomputation happen
    /// wherever the first pull of the frame landed. Owning it moves the decision
    /// to one explicit call, [`relayout_if_needed`](Self::relayout_if_needed),
    /// so a frame can hold a single consistent layout for all of its passes
    /// without cloning a recursive tree or holding a `RefCell` borrow across the
    /// frame (the latter would panic on a mid-frame pull — WS1.8).
    ///
    /// Being owned also unlocks something a memo cannot express: a future
    /// relayout can *splice this tree in place* instead of building a fresh one,
    /// since a memo's contract is to produce a value and `&mut` access to its
    /// contents would make change-detection meaningless.
    layout: LayoutModel,
    /// Gate for [`relayout_if_needed`](Self::relayout_if_needed): answers "did a
    /// tracked layout input change?" without storing the model in the graph.
    ///
    /// Tracks the reactive inputs the old memo tracked — `viewport` and `fonts`,
    /// both owned outside the page. The structure/prop channel is *not* tracked:
    /// it arrives through the poll's `force` flag, because the arena's own dirty
    /// set already records it (see `relayout_if_needed`). Owned by the page and
    /// disposed in `Drop`, like `render_probe`.
    layout_probe: Probe,
    state: PageState<W>,
    style: Signal<PageStyle<W::Color>>,
    // WS5.0b: the renderer is NOT stored here. It is a single-owner value living
    // in `UI` and handed to `render`/`use_renderer` as `&mut` for the duration of
    // the call. It was a `Signal` only because `Inert` is read-only and `Signal`
    // was the one `Copy` handle giving `&mut` access — it never had a subscriber
    // (every access was `update_untracked`), so it was pure graph freight.
    /// The page's fixed viewport. A plain `Size`, not a reactive handle:
    /// rsact targets fixed displays and there is no resize path (the standing
    /// TODO on `UI::new`), so it is a build-time constant. Making it reactive
    /// again means giving it a `mark_full` binding — see `fonts`.
    viewport: Size,
    stylist: Inert<W::Stylist>,
    dev_tools: Signal<DevTools>,
    /// "Repaint every part this frame." WS6.4.0(iv): a plain `bool`, carried into
    /// the walk through `RenderShared` and into the render probe's poll `force`.
    ///
    /// It was a `Signal<bool>`, and the cost was not the node — it was the
    /// broadcast. Every part's probe `track()`ed it (`el/render.rs`), so setting
    /// it created, maintained and walked one subscriber edge per widget in the
    /// tree. Nothing ever read its value reactively; the subscription *was* the
    /// mechanism. As a carried flag it is one boolean OR per part.
    ///
    /// Caveat worth keeping in mind: losing `track()` means losing the automatic
    /// wake, so every setter must sit on a path that reaches `use_renderer`. All
    /// of them are `&mut Page` methods called from the driver loop, which always
    /// renders after ticking. A future API letting user code request a redraw
    /// outside that loop would need to preserve the property deliberately.
    force_redraw: bool,
    /// WS6.2: "flush the whole viewport this frame". Drained in `use_renderer`
    /// to expand the damage set to the full viewport, because the page
    /// background outside the widget tree is not painted per-frame.
    ///
    /// WS6.4.0(iv): a plain `bool`. It was a `Signal<bool>` that never had a
    /// subscriber and never could usefully have one — every access was
    /// `set_untracked`/`get_untracked` — so it was a `Cell<bool>` paying for a
    /// graph node, plus a standing footgun (a stray `.get()` would have minted a
    /// subscription and spuriously re-dirtied the render probe). The only reason
    /// it needed to be a `Copy` handle was that the layout memo's closure wrote
    /// it; relayout is a `&mut self` call now, so a field suffices.
    full_flush: bool,
    render_calls: usize,
    /// The font context, shared with the `UI` — an `Rc`, not a `Signal`.
    ///
    /// It was reactive so a font added after the page was built could reach it.
    /// ISSUE-2's audit priced that: a graph node, plus — once layout stopped
    /// tracking it — a binding effect per page, paid by every UI for a
    /// capability nothing exercises. There is no runtime font work today
    /// (`UI::with_font` is a consuming *builder* method), so it is deliberately
    /// static, and `UI` now rejects a late font change outright rather than
    /// half-applying one.
    ///
    /// **What is deliberately not supported:** changing fonts after build. To
    /// restore it, make this dynamic again and give it a `mark_full` binding at
    /// `Page::new` — the same shape every other layout input uses. Forgetting
    /// the binding is what the tripwire in `relayout_if_needed` catches.
    fonts: Rc<FontCtx>,
    /// The page's render gate (WS2). One probe, polled once per frame in
    /// [`use_renderer`](Self::use_renderer); it re-runs the page render only
    /// when a tracked dependency changed or a redraw is forced. Owned by the
    /// page — disposed in `Drop` alongside the arena.
    render_probe: Probe,
    /// WS6.2: the damage accumulator. The render pass pushes the absolute outer
    /// rect of every part that actually repainted *and was not covered by a
    /// parent redraw* (a "redraw root") here; [`render`](Self::render) then
    /// flushes only those rects via `finish_frame_regions`. `RefCell` because
    /// the pass writes through a shared `RenderShared` borrow (which is `Copy`),
    /// while `render` reads it back afterwards. Cleared at the start of each
    /// pass, so an idle frame leaves it empty (flush nothing).
    damage: RefCell<Vec<Rect>>,
    /// WS6.4c(E): nodes processed by the last pass (see `RenderShared::visits`).
    nodes_visited: Cell<usize>,
    /// The page's reactive scope (WS3.1). Everything the page built —
    /// `init_page()`'s widgets (run before `Page::new` while this scope is
    /// current) and `Page::new`'s per-page nodes (`force_redraw`, the layout
    /// memo, `style`) — is owned by this scope, so dropping the page (goto
    /// navigation frees the old page) disposes it all. Declared last so it
    /// drops *after* the `Drop` body's explicit WS2 probe/arena disposal; nodes
    /// that body already disposed (`render_probe`, arena) are skipped by
    /// `drop_scope`'s `is_alive` guard. Always present: `Page::new` requires the
    /// caller to hand it the scope that was current during the build (G11 —
    /// page-created is page-owned, no unmanaged pages).
    scope: ScopeHandle,
}

/// Compute a page's [`LayoutModel`]. Extracted from what used to be the page's
/// layout `Memo` callback (WS6.4.0(iv)) so it can be called from `Page::new` and
/// from [`Page::relayout_if_needed`] alike — the same body, no longer wrapped in
/// a reactive node.
///
/// Reading `viewport`/`fonts` here TRACKS them, so call it inside the layout
/// probe's `poll` closure and nowhere else; that is what keeps a change to either
/// one able to re-run a relayout.
///
/// `prev` is the page's current model, used only by the `incremental-layout`
/// splice path. `None` on first build.
/// What [`compute_layout`] produced: the model, plus whether the caller owes a
/// blanket repaint.
///
/// WS6.4.0(iv): `compute_layout` used to *write* `force_redraw`/`full_flush`
/// itself, from inside what was then a memo callback. Reporting instead of
/// writing is what lets those two stop being reactive at all — a `Signal` was
/// only ever needed because a closure cannot hold `&mut`.
struct Relayout {
    layout: LayoutModel,
    blanket: bool,
}

/// Fired by [`Page::relayout_if_needed`]'s tripwire: a relayout happened that
/// nobody asked for through the dirty set, so it must have come from a tracked
/// read inside `compute_layout` — a layout input that skipped the one channel.
///
/// Consequences of leaving one in place: the empty dirty set makes
/// `compute_layout` skip the incremental path and take the blanket
/// `force_redraw`/`full_flush`, so *every* change of that input relayouts the
/// whole tree and reflushes the whole viewport. Correct pixels, worst-case cost,
/// no visible symptom — which is why this is an assert and not a comment.
///
/// The fix is always the same: give the input an `ElId` and route it through
/// `LayoutBuilder::setter`, so its binding effect calls `mark_dirty`. If the
/// input is genuinely page-wide (fonts, viewport), bind it to `mark_full`
/// instead.
const UNMARKED_LAYOUT_INPUT: &str = "relayout with an empty dirty set: some layout input changed reactively \
     without marking the arena. Route it through `LayoutBuilder::setter` \
     (per-element `mark_dirty`) or, if page-wide, `mark_full`.";

fn compute_layout<W: WidgetCtx>(
    id: W::PageId,
    arena: Signal<ElArena<W>>,
    root: ElId,
    viewport: Size,
    fonts: &FontCtx,
    prev: Option<&LayoutModel>,
) -> Relayout {
    info!("Relayout page {:?}", id);

    // `prev` is only consumed by the incremental path.
    #[cfg(not(feature = "incremental-layout"))]
    let _ = prev;

    // **This function performs NO tracked reads, and that is the invariant.**
    // `viewport` and `fonts` were the last two, held as reactive handles and
    // read here so the layout probe would subscribe to them. They are plain
    // values now (see `Page::fonts`), which — with text/icon-size/`show` moved
    // onto the dirty set by ISSUE-2 — leaves the layout pass with **zero**
    // subscriptions. The probe wrapping this call is therefore a pure tripwire:
    // it has no sources, so it can only ever fire if someone reintroduces a
    // tracked read, which is exactly what it exists to catch.

    // WS6.1: set true by the incremental path below when it does a
    // TARGETED repaint (marked specific stable ancestors), so the
    // blanket `force_redraw`/`full_flush` is skipped. Only exists under
    // the feature — the default path is always blanket (see `blanket`).
    #[cfg(feature = "incremental-layout")]
    let mut targeted = false;

    let layout = {
        // `take_dirty` mutates the arena, so `update_untracked` (needs
        // `&mut` on the Copy signal handle) rather than `with_untracked`.
        let mut arena = arena;
        arena.update_untracked(|arena| {
            let ctx = LayoutCtx {
                fonts,
                viewport,
                font_props: FontProps {
                    font: Some(Font::Auto),
                    font_size: None,
                    font_style: None,
                },
            };

            // Drain the dirty set every pass so marks are attributed to
            // the relayout that handles them (also stops it accumulating
            // under default features, which never read it).
            let _dirty = arena.take_dirty();

            // Incremental only for a non-empty, non-`full` dirty set with
            // a previous tree. A `full` mark (structure change / fonts),
            // an empty set (viewport change / first build), or no `prev`
            // ⇒ full recompute.
            #[cfg(feature = "incremental-layout")]
            if let Some(prev) = prev
                && !_dirty.is_empty()
                && !_dirty.is_full()
            {
                let new = crate::layout::model::relayout_incremental(
                    prev,
                    _dirty.nodes(),
                    fonts,
                    viewport,
                    arena,
                );

                // WS6.1: TARGETED invalidation. Instead of the blanket
                // `force_redraw`/`full_flush` below, repaint only the
                // nearest size-stable ancestor of each moved node (its
                // clear covers the old + new child positions — no
                // ghosting). Those ancestors' `outer` rects become the
                // damage the render pass records + flushes (WS6.2). If a
                // change reached the ROOT (`full`), no stable ancestor
                // exists ⇒ fall through to the blanket path.
                let roots =
                    crate::layout::model::layout_repaint_roots(prev, &new);
                if !roots.full {
                    for id in roots.roots {
                        arena.mark_needs_redraw(id);
                    }
                    targeted = true;
                }

                return new;
            }

            model_layout(
                &ctx,
                arena,
                root,
                Limits::only_max(viewport),
                viewport.into(),
            )
        })
    };

    // Whether the caller must do a BLANKET repaint + whole-viewport flush —
    // true unless the incremental path already did a TARGETED repaint above
    // (WS6.1). The default (non-incremental) build has no targeted path, so it
    // is always blanket: the historical behaviour. Covers first build,
    // fonts/viewport change, structure change, and any relayout that reached
    // the root.
    #[cfg(feature = "incremental-layout")]
    let blanket = !targeted;
    #[cfg(not(feature = "incremental-layout"))]
    let blanket = true;

    debug!("{}", PPLayoutModel::root(&layout));

    Relayout { layout, blanket }
}

impl<W: WidgetCtx> Drop for Page<W> {
    fn drop(&mut self) {
        // Dispose every probe the page owns before the arena signal itself
        // (WS2.3): each element's `part_probes` (walked via the arena), the page
        // `render_probe`, and the `layout_probe` (WS6.4.0(iv)). Goto navigation
        // drops the old page, so without this every navigation would leak probe
        // nodes.
        self.arena
            .update_untracked(|arena| arena.dispose_all_probes());
        unsafe {
            self.render_probe.dispose();
            self.layout_probe.dispose();
            self.arena.dispose();
        }
    }
}

impl<W: WidgetCtx> Page<W> {
    pub(crate) fn new(
        id: W::PageId,
        // TODO: Do we really need to accept Into<El> and expect it to always
        // be a El::New, or we can require root to be a Widget non-wrapped?
        // No, ElData can contain additional information raw widget does not
        // provide
        root: impl View<W>,
        arena: Signal<ElArena<W>>,
        viewport: Size,
        stylist: Inert<W::Stylist>,
        dev_tools: Signal<DevTools>,
        fonts: Rc<FontCtx>,
        // The page scope (WS3.1, G11). The caller creates it with `new_scope()`
        // *before* evaluating `root`/`init_page()` so it is current for the
        // whole build (the widget tree AND the per-page nodes below); `Page::new`
        // `leave`s it at the end and takes ownership, disposing it on drop.
        scope: ScopeHandle,
    ) -> Self {
        let mut root: El<W> = root.into_el();
        let state = PageState::new();

        let mut force_redraw = false;

        // WS6.4.0(iv): a plain flag, not a signal (see the field's doc). It is
        // still true that `force_redraw` must not be value-read to make the flush
        // decision — reading a notifying signal propagates pending dirtiness to
        // the render probe — which is why the flush decision has its own flag at
        // all rather than reusing `force_redraw`.
        let mut full_flush = false;

        let root = BuildCtx::run(&mut root, arena);

        // ISSUE-2 originally gave `fonts` and `viewport` a `mark_full` binding
        // here — they are the two layout inputs owned OUTSIDE the tree, so no
        // `ElId` can claim them, but they still belonged on the one channel with
        // the whole tree as their blast radius.
        //
        // Removed by maintainer decision the same day: neither is reactive any
        // more (both are plain values — see the fields), because there is no
        // runtime font work and no resize path, so the two effects bought a
        // capability nothing exercises and cost one node each per page. When
        // either becomes dynamic, the binding is what to restore — one
        // `create_effect` per input calling `arena.mark_full_relayout()`.

        // Untracked so the probe is owned by no observer/scope — the page owns
        // it and disposes it explicitly in `Drop` (WS2.3), like `render_probe`.
        let layout_probe = untrack(create_probe);

        // The initial layout.
        let compute =
            || compute_layout::<W>(id, arena, root, viewport, &fonts, None);

        // `force: true` — the probe has no sources to register any more (see
        // `compute_layout`), so the first layout must be requested outright
        // rather than relying on birth state. `None` for `prev` — nothing to
        // splice.
        let relayout = layout_probe.poll(true, compute).unwrap_or_else(|| {
            // Unreachable: a probe created three lines above cannot already be
            // disposed. Logged and degraded rather than unwrapped (WS1.8).
            error!("Page {id:?}: fresh layout probe was already disposed");
            compute()
        });
        let layout = relayout.layout;

        // First build is always blanket, so the initial frame repaints the whole
        // tree and flushes the whole viewport. Applied HERE rather than inside
        // the compute, which no longer writes redraw state (see `Relayout`).
        if relayout.blanket {
            force_redraw = true;
            full_flush = true;
        }

        let style = PageStyle::base().signal().name("Page style");

        // Untracked so the probe is owned by no observer/scope — the page owns
        // it and disposes it explicitly in `Drop` (WS2.3).
        let render_probe = untrack(create_probe);

        // The page tree is fully built; `leave` restores the previously-current
        // scope so later work (renders, events) is not captured here, while the
        // handle stays alive to dispose everything on page drop (WS3.1).
        scope.leave();

        Self {
            id,
            root,
            needs_redraw: true,
            arena,
            layout,
            layout_probe,
            state,
            style,
            // TODO: reactive viewport in Renderer? Windows can change size.
            viewport,
            stylist,
            dev_tools,
            force_redraw,
            full_flush,
            render_calls: 0,
            fonts,
            render_probe,
            damage: RefCell::new(Vec::new()),
            nodes_visited: Cell::new(0),
            scope,
        }
    }

    pub(crate) fn id(&self) -> W::PageId {
        self.id
    }

    /// The page's current layout, relayouting first if an input changed.
    ///
    /// `&mut self` is the point (WS6.4.0(iv)): it is what makes a relayout
    /// *during* a frame a compile error rather than a runtime hazard, since a
    /// frame holds `&mut Page` for its duration.
    /// Harness-only, and unusable by the production passes rather than merely
    /// discouraged: the returned reference borrows all of `*self` (it comes from a
    /// `&mut self` method), which conflicts with the `&mut self.state` the event
    /// passes need alongside it — so they call `relayout_if_needed()` and then
    /// borrow the `layout` FIELD, which is disjoint. Kept because it is the shape
    /// a `Frame` will want (WS6.4d), where holding `&mut Page` for the frame is
    /// the point. WS6.4a's `test_support::tile_probe` is the in-crate caller (it
    /// derives the traversal cost from the layout tree), which is why this is no
    /// longer `#[cfg(test)]`.
    pub(crate) fn layout(&mut self) -> &LayoutModel {
        self.relayout_if_needed();
        &self.layout
    }

    /// Bring `self.layout` up to date. Returns whether it actually recomputed.
    ///
    /// **Relayout is never an effect and never a memo** (WS6.4.0(iv)): it is this
    /// one call, made where `&mut Page` is held — the start of a render frame and
    /// before an event pass — and *never* while a frame is in flight. That last
    /// part is enforced rather than documented: a frame holds `&mut Page`, so no
    /// other `&mut self` method can run during it.
    ///
    /// It replaced a `Memo<LayoutModel>`, where recomputation happened wherever
    /// the frame's first pull landed. Two things follow from making it explicit:
    ///
    /// - the caller can order it *between* draining the root's redraw flag and
    ///   polling the render gate, feeding the result into that gate's `force`
    ///   (`use_renderer` documents why both sides matter). With the memo, a
    ///   targeted relayout (WS6.1
    ///   marks specific ancestors instead of forcing a blanket redraw) woke the
    ///   render probe only because reading the memo inside the render pass
    ///   subscribed the probe to it. Deleting the memo *without* this reordering
    ///   would leave a targeted layout change repainting *nothing*.
    /// - `force_redraw`/`full_flush` stop being written from inside a memo
    ///   callback, which is what lets them stop being reactive at all.
    ///
    /// # One dirty channel
    ///
    /// **Every layout input marks the arena's dirty set. That is the channel.**
    /// A property setter marks its own element (`bind_layout`'s effect —
    /// `mark_dirty`); a structure change or a font/viewport change marks the
    /// whole tree (`BuildCtx::set_children`/`set_single_child`, and the
    /// `Page::new` font binding — `mark_full`). The mark arrives here as the
    /// poll's `force` flag; nothing about layout is *tracked* by design.
    ///
    /// Those sites used to *also* fire a `relayout` Trigger for the memo to
    /// track; the mark and the request were the same fact recorded twice, so the
    /// Trigger is gone. The marks are kept on **both** feature paths — a default
    /// build never reads the dirty set to choose incremental-vs-full, but it does
    /// maintain and drain it — so the gate works with or without
    /// `incremental-layout`.
    ///
    /// # The probe is a tripwire, not a channel
    ///
    /// `compute_layout` still runs inside `layout_probe.poll`, so any tracked
    /// read it performs subscribes the probe. That is deliberately **not** a
    /// second way to request a relayout — it is how we *detect* a layout input
    /// that skipped the channel above.
    ///
    /// The distinction matters because the failure it catches is invisible
    /// otherwise: an unmarked input still produces correct pixels, it just
    /// recomputes and repaints the whole tree forever (an empty dirty set skips
    /// the incremental path in `compute_layout`, and no `mark_needs_redraw` means
    /// the blanket `force_redraw`/`full_flush`). That is exactly how ISSUE-2
    /// (`Label` text) survived unnoticed — along with `ContentLayout::Icon`'s
    /// size memo and `Layout::show`, which had the identical shape.
    ///
    /// So: `poll` runs when `marked || probe_dirty`. If it ran and nothing was
    /// marked, the probe fired on its own and some input is on the wrong
    /// channel — `debug_assert` in debug, `warn!` in release. Release stays
    /// *correct*: `poll` has already recomputed by the time we look, so the
    /// warning reports waste, not breakage.
    ///
    /// Since `fonts`/`viewport` stopped being reactive too, `compute_layout`
    /// has **no tracked reads at all**, so the probe holds no sources: the
    /// tripwire is now unfireable except by a genuine regression. That is the
    /// strongest form of it — before, "the probe is clean" merely meant fonts
    /// and the viewport had not changed.
    fn relayout_if_needed(&mut self) -> bool {
        let marked = self.arena.with_untracked(|arena| arena.is_layout_dirty());

        // `Probe` is `Copy`: taking it out first means `poll` does not borrow
        // `self`, so the closure can borrow `self.layout` as `prev` and the
        // assignment below can take `&mut self.layout` once the poll returns.
        let probe = self.layout_probe;
        let recomputed = probe.poll(marked, || {
            compute_layout::<W>(
                self.id,
                self.arena,
                self.root,
                self.viewport,
                &self.fonts,
                Some(&self.layout),
            )
        });

        match recomputed {
            Some(relayout) => {
                // The tripwire (see this method's docs). `poll` runs on
                // `marked || probe_dirty`; recomputing with nothing marked means
                // the probe fired alone. Debug: hard failure. Release: warn and
                // carry on — the recompute already happened, so the frame is
                // correct, just needlessly whole-tree.
                debug_assert!(marked, "{}", UNMARKED_LAYOUT_INPUT);
                if !marked {
                    warn!("{UNMARKED_LAYOUT_INPUT}");
                }

                self.layout = relayout.layout;
                // Applied outside the poll closure: `compute_layout` reports
                // rather than writes, so the redraw state is set here, by a
                // `&mut self` method, which is what lets `full_flush` be a plain
                // field (WS6.4.0(iv)).
                if relayout.blanket {
                    self.force_redraw();
                }
                true
            },
            None => false,
        }
    }

    pub(crate) fn force_redraw(&mut self) -> &mut Self {
        info!("Force redraw page {:?}", self.id);
        self.force_redraw = true;
        // WS6.2: a forced redraw flushes the whole viewport (see `full_flush`).
        self.full_flush = true;
        self
    }

    /// This frame's damage rects (WS6.2), for callers outside the `page` module —
    /// `damage` itself is private to it. WS6.4a's `test_support::tile_probe` turns
    /// this into a *region schedule*, which is how the harness measures a real
    /// interactive frame rather than an invented one; that is also why it is no
    /// longer `#[cfg(test)]`.
    pub(crate) fn damage_snapshot(&self) -> alloc::vec::Vec<Rect> {
        self.damage.borrow().clone()
    }

    /// This frame's damage rects, borrowed rather than cloned — what
    /// [`UI::start_frame`] plans from.
    ///
    /// The clone in [`Self::damage_snapshot`] is fine for a test harness and
    /// wrong for the render loop: it is an allocation on every frame, in the one
    /// path WS6.4/WS18 want allocation-free once warmed up.
    ///
    /// [`UI::start_frame`]: crate::ui::UI::start_frame
    pub fn with_damage<R>(&self, f: impl FnOnce(&[Rect]) -> R) -> R {
        f(&self.damage.borrow())
    }

    pub fn take_draw_calls(&mut self) -> usize {
        core::mem::replace(&mut self.render_calls, 0)
    }

    // TODO
    // pub fn style(
    //     mut self,
    //     style: impl IntoSignal<PageStyle<C::Color>>,
    // ) -> Self {
    //     self.style = style.signal();
    //     self
    // }

    /// WS5.0b: takes the renderer as a borrow (it is owned by `UI`, not the
    /// page).
    pub fn clear(&mut self, renderer: &mut W::Renderer) -> &mut Self {
        let viewport = self.viewport;
        self.style.with(|style| {
            // TODO: Will not work without background, must always have a
            // background
            if let Some(bg) = style.background_color {
                // NOTE (WS5.0b): behaviour preserved verbatim, including this
                // `.ok().unwrap()`. It is a panic site on a UI path and so
                // violates WS1.8 ("the UI must log and degrade, never panic") —
                // left as-is to keep this refactor behaviour-identical rather
                // than smuggling in a semantic change. Belongs to WSi.2's
                // unwrap burn-down.
                Renderer::fill_solid(
                    renderer,
                    Rect::new(Point::zero(), viewport),
                    bg,
                )
                .ok()
                .unwrap();

                // WS6.4.0(iv): whoever paints pixels declares them. This just
                // repainted the WHOLE viewport in the framebuffer, so the whole
                // viewport must reach the display; otherwise a damage-driven
                // flush sends only the widget rects and everything the widgets do
                // not cover keeps whatever the framebuffer held. That matters
                // because the framebuffer is one `UI`-owned value outliving every
                // page, and a `Flex` root's `render` is a literal no-op — so
                // container padding and inter-child gaps are painted by no widget.
                //
                // NOT load-bearing in production today, and the comment should
                // say so: `clear`'s only caller is `UI::on_page_change`, which
                // runs on a freshly built page whose first-build blanket already
                // sets this flag. It is here so `clear` is correct in isolation
                // for any future caller, and `clear_declares_a_full_viewport_flush`
                // pins it (verified: that test fails with `[]` damage without it).
                //
                // It must be this flag rather than a damage rect: `use_renderer`
                // clears the damage list at frame start, so a rect pushed before
                // the frame would be silently dropped.
                //
                // Inside the `if let` on purpose — with no background colour
                // nothing was painted, so nothing needs flushing.
                self.full_flush = true;
            }
        });
        self
    }

    // Focus //

    // /// Focus first focusable element in page
    // pub fn focus_first(&mut self) {
    //     self.focus_el(0);
    // }

    // /// Focus first focusable element in page if no element focused
    // pub fn apply_auto_focus(&mut self) {
    //     if self.state.with(|state| state.focused.is_none()) {
    //         self.focus_first();
    //     }
    // }

    // /// Find element to focus by offset from currently focused element index.
    // fn find_focus(&mut self, offset: i32) -> Option<(ElId, usize)> {
    //     let focusable_count = self.meta.focusable.with(Vec::len);

    //     let current_offset = self.state.with(|state| {
    //         state.focused.as_ref().map(|focused| focused.1).unwrap_or(0)
    //     });

    //     let new_focus_offset = (current_offset as i64 + offset as i64)
    //         .clamp(0, focusable_count as i64)
    //         as usize;

    //     let new_focus_id = self
    //         .meta
    //         .focusable
    //         .with(|focusable| focusable.get(new_focus_offset).copied());

    //     // Set new focus only in case there's a corresponding element by
    // index. Otherwise it means buggy meta collection     if let
    // Some(new_focus_id) = new_focus_id {         Some((new_focus_id,
    // new_focus_offset))     } else {
    //         None
    //     }
    // }

    // fn apply_focus(&mut self, new_focus: (ElId, usize)) {
    //     self.state.update(|state| state.focused = Some(new_focus))
    // }

    // /// Send focus event to tree. Sets new focus if event was captured
    // fn focus_el(&mut self, offset: i32) -> Option<UnhandledEvent<W>> {
    //     if let Some(new_focus) = self.find_focus(offset) {
    //         let response =
    //
    // self.send_event(Event::Focus(FocusEvent::Focus(new_focus.0)));

    //         if response.is_none() {
    //             self.apply_focus(new_focus);
    //         }

    //         response
    //     } else {
    //         None
    //     }
    // }

    /// For Dev tools
    fn find_el_under_cursor(&self, point: Point) -> Option<DevHoveredEl> {
        // WS6.4.0(iv): a plain read of the owned model, and deliberately NOT a
        // relayout. Pulling the old `Memo` from `&self` could recompute (a memo
        // is interior-mutable), so a dev-tools hover could trigger a relayout.
        // Dev tools are an add-on — they must never define rsact's logic — and
        // they are headed for a separate window/host anyway.
        self.layout
            .tree_root()
            .dev_hover(point)
            .map(|layout| DevHoveredEl { layout })
    }

    /// Apply global logic for unhandled events.
    /// Some events have different interpretations.
    /// For example `MoveEvent` can be treated as focus move, and if no element
    /// captured this event, we move the focus.
    fn on_unhandled_event(
        &mut self,
        unhandled: Event<W::CustomEvent>,
    ) -> Option<UnhandledEvent<W>> {
        // if let Some(focus_offset) = unhandled.interpret_as_focus_move() {
        //     // TODO: Focus event is eaten here, even if no element focused.
        // This might be incorrect     return
        // self.focus_el(focus_offset); }

        Some(UnhandledEvent::Event(unhandled))
    }

    #[must_use]
    fn send_specific_event(
        &mut self,
        event: &Event<W::CustomEvent>,
    ) -> EventResponse {
        // Note: Need to have special deferred reactive updates zone. Because if
        // some child node depends on value it's children set, then there will
        // be a BorrowRefMut error because children are borrowed mutably for
        // update on events. This happens for example if flex layout contains a
        // checkbox toggling this flex layout wrap.

        let defer_effects = defer_effects();

        // WS6.4.0(iv): event routing hit-tests against layout, so bring it up
        // to date first. This used to happen implicitly by pulling the layout
        // `Memo`; now it is an explicit, ordered call.
        self.relayout_if_needed();

        let layout = &self.layout;
        let res = self.arena.update_untracked(|arena| {
            EventPass::run(arena, event, &mut self.state, &layout.tree_root())
        });

        // TODO: notify root on event capture?
        //  - No, root is not used reactively, it is a signal only to be
        //    usable in reactive contexts. Need `StoredValue`

        defer_effects.run();

        res
    }

    /// Dispatch an event to a single widget (its layout node is resolved by
    /// walking the tree). Used for pointer capture: the capturing widget
    /// receives mouse events exclusively.
    fn send_event_to(
        &mut self,
        target: ElId,
        event: &Event<W::CustomEvent>,
    ) -> EventResponse {
        let defer_effects = defer_effects();

        // WS6.4.0(iv): see `send_event` — explicit relayout before hit-testing.
        self.relayout_if_needed();

        let layout = &self.layout;
        let res = self.arena.update_untracked(|arena| {
            EventPass::run_to(
                target,
                arena,
                event,
                &mut self.state,
                &layout.tree_root(),
            )
        });

        defer_effects.run();

        res
    }

    fn send_update_inner(
        id: ElId,
        update: Update,
        arena: &mut ElArena<W>,
    ) -> UpdateResult {
        let Some(el) = arena.expect_mut(id) else {
            return UpdateResult::none();
        };

        debug!("Send update {:?} to {}[{:?}]", update, el.state.debug_name, id);
        let result = match el.stage.built_mut() {
            Some(widget) => {
                widget.update(UpdateCtx { id, update, state: &mut el.state })
            },
            None => UpdateResult::none(),
        };

        if result.should_bubble()
            && let Some(bubble) = update.as_bubble()
            && let Some(parent) = arena.parents.get(id).copied()
        {
            Self::send_update_inner(parent, bubble, arena).merge(result)
        } else {
            result
        }
    }

    fn send_update(&mut self, id: ElId, update: Update) {
        let result = self.arena.update_untracked(|arena| {
            Self::send_update_inner(id, update, arena)
        });

        if result.is_redraw_requested() {
            self.needs_redraw = true;
        }
    }

    #[must_use]
    fn send_event(
        &mut self,
        event: Event<W::CustomEvent>,
    ) -> Option<UnhandledEvent<W>> {
        // WS3.3: drop any focus/pointer references to elements that have left
        // the arena since the last event (a `Dynamic` rebuild or list update
        // between frames), so the `captured_by` fast-path below — and the
        // normal focus/hover routing — never dispatch to a freed id (D2-F5).
        let arena = self.arena;
        arena.with(|arena| self.state.retain_existing(arena));

        // === Pointer capture ===
        // While a widget holds the pointer capture, every mouse event is
        // delivered to it exclusively — no hit-testing and no hover changes —
        // so it can keep tracking the cursor even when it leaves its own bounds
        // (drags, sliders, scrollbars). A `ButtonUp` ends the capture (and any
        // in-flight press). Non-mouse events fall through to normal handling.
        if let Some(captured_id) = self.state.pointer.captured_by
            && let Event::Mouse(mouse) = &event
        {
            if let MouseEvent::MouseMove(pt) = mouse {
                self.state.pointer.pos = Some(*pt);
            }

            let _ = self.send_event_to(captured_id, &event);

            if matches!(mouse, MouseEvent::ButtonUp(_, _)) {
                self.state.pointer.captured_by = None;

                // End the mouse press and reset the pressed visual.
                // Unconditional so a release outside the widget's bounds
                // (drag-off) still ends the press; a no-op for non-click
                // captures (e.g. a `Scrollable` drag leaves `pressed` `None`).
                if let Some(pressed) = self.state.pointer.pressed {
                    self.send_update(pressed, Update::PressChange(false));
                    self.state.pointer.pressed = None;
                }
            }

            return None;
        }

        if let Event::Mouse(MouseEvent::MouseMove(point)) = event {
            if self.dev_tools.with(|dt| dt.enabled) {
                let hovered_el = self.find_el_under_cursor(point);
                let dev_hovered_changed = self.dev_tools.update(|dt| {
                    let changed = dt.hovered != hovered_el;
                    dt.hovered = hovered_el;
                    changed
                });

                if dev_hovered_changed {
                    // TODO: Real rendering requires smarter dirty rectangles as
                    // dev tools are overlaying and have absolute position.
                    // Clearing whole screen is bad

                    self.force_redraw();
                }
                // TODO: Should dev tools capture mouse movement?
                // return None;
            }

            self.state.pointer.pos = Some(point);

            let old_hovered = self.state.pointer.hovered;
            self.state.pointer.hovered = None;
            let _ = self.send_specific_event(&event);

            // Dispatch Enter/Leave if hovered widget changed
            let new_hovered = self.state.pointer.hovered;
            if old_hovered != new_hovered {
                debug!("Hover change: {:?} -> {:?}", old_hovered, new_hovered);
                if let Some(left) = old_hovered {
                    self.send_update(left, Update::HoverChange(false));
                }
                if let Some(entered) = new_hovered {
                    self.send_update(entered, Update::HoverChange(true));
                }
            }

            // TODO: Should mouse event always be ignored?
            return None;
        }

        // Global, page-level event handling //
        // Press bookkeeping mirrors hover: the event pass claims the press
        // (mouse via `pointer.pressed`, encoder via `focus_pressed`); here we
        // turn that state change into the `pressed` pseudo-class. Clearing on
        // mouse release happens in the capture branch above; focus release is
        // handled below.
        let old_pressed = self.state.pointer.pressed;

        let response = self.send_specific_event(&event);

        match &event {
            // Mouse press just claimed during this pass (normally `None ->
            // Some`); notify the newly pressed widget.
            Event::Mouse(MouseEvent::ButtonDown(MouseButton::Left, _)) => {
                let new_pressed = self.state.pointer.pressed;
                if old_pressed != new_pressed {
                    if let Some(old) = old_pressed {
                        self.send_update(old, Update::PressChange(false));
                    }
                    if let Some(new) = new_pressed {
                        self.send_update(new, Update::PressChange(true));
                    }
                }
            },
            // Encoder/keyboard press claimed on the focused widget.
            Event::Press(PressEvent::Press) => {
                if self.state.focus_pressed
                    && let Some((id, _)) = self.state.focused
                {
                    self.send_update(id, Update::PressChange(true));
                }
            },
            // Encoder/keyboard release: the pass already fired `handle_click`
            // (it saw `focus_pressed == true`); now end the press.
            Event::Press(PressEvent::Release) => {
                if self.state.focus_pressed {
                    if let Some((id, _)) = self.state.focused {
                        self.send_update(id, Update::PressChange(false));
                    }
                    self.state.focus_pressed = false;
                }
            },
            _ => {},
        }

        match response {
            EventResponse::Continue(()) => {
                info!(
                    "Event {:?} was ignored, applying global handling",
                    event
                );
                self.on_unhandled_event(event)
            },
            EventResponse::Break(capture) => match capture {
                // TODO: Captured data may be useful for debugging, for example
                // we can point where on screen user clicked or something
                Capture::Captured(_capture) => {
                    info!(
                        "Event {:?} was captured, stopping propagation",
                        event
                    );
                    None
                },
            },
        }
    }

    pub fn handle_events(
        &mut self,
        events: impl Iterator<Item = Event<W::CustomEvent>>,
    ) -> Vec<UnhandledEvent<W>> {
        let unhandled =
            events.filter_map(|event| self.send_event(event)).collect();

        unhandled
    }

    // NOTE (WS6.4d): `render(renderer, target)` lived here and flushed the damage
    // and flushed the damage itself through `FinishRender`. Both halves moved
    // out: the renderer is the caller's, and so is the transport.
    // `use_renderer` runs the pass; `with_damage` says what it repainted, and
    // the caller streams that out however it likes.

    /// The render pass proper: walk the tree into `renderer`, then draw the
    /// dev-tools overlay. Split out of [`Self::use_renderer`] (WS5.0b) so the
    /// `?`s below have a function boundary to return to now that the body is
    /// no longer wrapped in a `Signal::update_untracked` closure.
    fn render_pass(
        &mut self,
        renderer: &mut W::Renderer,
        mode: RenderMode,
    ) -> RenderResult {
        // self.style
        //     .with(|style| {
        //         if let Some(background_color) =
        //             style.background_color
        //         {
        //             debug!(
        //                 "Clear page {:?} with color {:?}",
        //                 self.id, background_color
        //             );
        //             let viewport = self.viewport;
        //             Renderer::fill_solid(
        //                 renderer,
        //                 Rect::new(Point::zero(), viewport),
        //                 background_color,
        //             )
        //         } else {
        //             Ok(())
        //         }
        //     })
        //     .ok()
        //     .unwrap();

        let fonts = &self.fonts;
        // WS6.4.0(iv): the layout is an owned field, so it is a plain borrow —
        // no longer one of the reactive values `with!` unwraps. It is guaranteed
        // current because `use_renderer` calls `relayout_if_needed` before
        // polling the render probe, which is also what makes a targeted layout
        // change wake this pass at all.
        let layout = &self.layout;
        // WS4.1: borrow (don't move) — `Inert<W::Stylist>` is inline
        // now and no longer `Copy`; the `with!` below only reads it.
        let stylist = &self.stylist;

        with!(|stylist| {
            // Plain read since WS6.4.0(iv) — this used to be `.get()`, i.e. a
            // debug-only *subscription* of the render probe to `force_redraw`.
            debug!("Force redraw: {}", self.force_redraw);
            self.arena.update_untracked(|arena| {
                RenderPass::new(
                    arena,
                    renderer,
                    RenderShared {
                        mode,
                        page_state: &self.state,
                        page_style: self.style.read_only(),
                        viewport: self.viewport,
                        fonts,
                        stylist,
                        force_redraw: self.force_redraw,

                        damage: &self.damage,
                        visits: &self.nodes_visited,
                    },
                )
                .render(
                    &layout.tree_root(),
                    RenderVisual {
                        tree_style: TreeStyle::base(),
                        font_props: FontProps {
                            font: Some(Font::Auto),
                            font_size: None,
                            font_style: None,
                        },
                    },
                    RenderFrame::root(self.render_calls),
                )
            })
        })?;

        self.dev_tools.with(|dev_tools| {
            if dev_tools.enabled {
                if let Some(hovered) = &dev_tools.hovered {
                    return hovered.draw::<W>(renderer, fonts, self.viewport);
                }
            }
            Ok(())
        })
    }

    /// Poll the page's render probe and, if it ran, hand `renderer` to `f`.
    ///
    /// WS5.0b: `renderer` is a borrow for the duration of the call. It used to
    /// be a `Signal<W::Renderer>` field, unwrapped here with `update_untracked`
    /// — a reactive node with no subscribers, kept only because `Signal` was the
    /// one `Copy` handle that yielded `&mut`.
    pub fn use_renderer(
        &mut self,
        renderer: &mut W::Renderer,
        f: impl FnOnce(&mut W::Renderer),
    ) -> bool {
        let drawn = self.probe_gated_pass(renderer, RenderMode::Fused);

        // TODO: Can be put directly into the observe
        if drawn {
            f(renderer);
        }
        drawn
    }

    /// WS6.4c: **plan** the frame — one tracked, probe-gated walk whose drawing
    /// is discarded, leaving the damage rects in `self.damage`.
    ///
    /// This is the pass that answers "what changed"; [`paint_region`] then
    /// answers "what does the screen look like there". Same walk as
    /// [`use_renderer`], same probe polling, same cleaning — only the output is
    /// thrown away, at the drawing seam, before any primitive rasterizes.
    ///
    /// Returns whether the page was dirty at all. When it returns `false` the
    /// plan is empty and the frame can be skipped entirely.
    ///
    /// The bodies genuinely run, which is the point: dirtiness is only knowable
    /// by running them, and the run is what re-tracks dynamic dependencies.
    ///
    /// [`paint_region`]: Self::paint_region
    /// [`use_renderer`]: Self::use_renderer
    pub fn collect(&mut self, renderer: &mut W::Renderer) -> bool {
        self.probe_gated_pass(renderer, RenderMode::Collect)
    }

    /// WS6.4c: **paint** one region — untracked, no probes, everything
    /// intersecting `region` repaints.
    ///
    /// Call after [`collect`] has planned the frame, once per region. The walk
    /// is wrapped in `untrack` so a paint pass cannot subscribe anything: the
    /// page probe's `clear_sources` + re-subscribe + `mark_clean` round-trip is
    /// paid once per frame by `collect`, not once per region, and a bare re-run
    /// would otherwise donate every leaf's dependencies to whatever observer is
    /// ambient — destroying targeted invalidation (6.4c(2)).
    ///
    /// The damage set is deliberately left alone: the region already *is* the
    /// flush unit, and letting paint feed the plan channel would make every
    /// painted region re-damage itself (6.4d(4)).
    ///
    /// [`collect`]: Self::collect
    /// WS6.4c(E): nodes the last pass actually processed — the traversal term,
    /// which no op log can see. Reset at the start of every pass.
    pub fn nodes_visited(&self) -> usize {
        self.nodes_visited.get()
    }

    pub fn paint_region(
        &mut self,
        renderer: &mut W::Renderer,
        region: Rect,
    ) -> RenderResult {
        self.nodes_visited.set(0);
        renderer.begin_region(region)?;
        renderer.push_clip(region);

        // `untrack` is load-bearing, not hygiene — see the doc comment.
        let result = untrack(|| self.render_pass(renderer, RenderMode::Paint));

        renderer.pop_clip();
        renderer.end_region()?;

        result
    }

    /// The shared body of [`use_renderer`] and [`collect`]: everything a
    /// probe-gated pass does, parameterized by what the pass is *for*.
    ///
    /// [`use_renderer`]: Self::use_renderer
    /// [`collect`]: Self::collect
    fn probe_gated_pass(
        &mut self,
        renderer: &mut W::Renderer,
        mode: RenderMode,
    ) -> bool {
        debug_assert!(
            mode.is_probe_gated(),
            "[BUG] {mode:?} must not go through the probe-gated pass"
        );

        self.nodes_visited.set(0);

        // WS6.2: start a fresh damage set for this frame. Cleared here (not at
        // the end) so a pass that the probe SKIPS leaves it empty — an idle
        // frame flushes nothing — while `render` reads it back after the pass.
        self.damage.borrow_mut().clear();

        // (Removed the debug-only `("page_force_redraw", id)` observer here: it
        // only logged force-redraw changes and its `force_redraw` dependency is
        // already tracked by `render_probe` below — WS2 dropped it with the
        // observer registry rather than mint a debug-only probe for a log line.)

        // Drained BEFORE the relayout below — see the note there.
        let redraw_reason = self.arena.update_untracked(|arena| {
            arena
                .expect_mut(self.root)
                .unwrap()
                .state
                .take_needs_redraw()
        });

        // WS6.4.0(iv): relayout HERE — after the drain above, before the gate
        // below. Both sides of that sandwich are load-bearing.
        //
        // *Before the gate*: the layout used to be a `Memo` pulled from inside
        // `render_pass`, i.e. from inside the poll closure, so the render probe
        // *subscribed* to it and a relayout woke the pass as a side effect of that
        // subscription. With the layout owned, nothing subscribes — so a TARGETED
        // relayout (WS6.1 marks specific stable ancestors rather than forcing a
        // blanket redraw) would wake nothing and repaint NOTHING. OR-ing
        // `relayouted` into `needs_redraw` restores that wake explicitly.
        //
        // *After the drain*: a targeted relayout marks its repaint roots via
        // `arena.mark_needs_redraw`, and those marks must survive for the tree
        // walk to read. Draining afterwards consumes a mark placed on the root
        // element before the walk ever sees it — which is exactly what
        // `incremental_layout_change_damages_only_the_stable_parent` catches
        // (verified: swapping these two makes it fail with `[]` damage).
        let relayouted = self.relayout_if_needed();
        // `self.force_redraw` is OR-ed in because nothing subscribes to it any
        // more (WS6.4.0(iv)): the render probe used to `track()` it inside its own
        // poll closure. Setting the flag must still guarantee the poll runs, or a
        // `Page::force_redraw()` on an otherwise-idle frame would be swallowed.
        let needs_redraw = relayouted
            || redraw_reason.is_some()
            || self.needs_redraw
            || self.force_redraw;

        // Copy the probe handle out (it is `Copy`) so the poll closure can
        // borrow `self` mutably without aliasing `self.render_probe`.
        let render_probe = self.render_probe;
        let drawn = render_probe.poll(needs_redraw, || {
            info!(
                "Render page {:?} (call: {})",
                self.id,
                self.render_calls + 1
            );

            #[cfg(feature = "debug-info")]
            {
                rsact_reactive::debug::observer_debug_info().map(|di| {
                    info!("Rerender debug info: {di}");
                });
            }

            self.render_calls += 1;

            // WS5.0b: the pass body moved into `render_pass` (below). It
            // used to sit here inside `renderer.update_untracked(|renderer|
            // …)` — an untracked unwrap of a subscriber-less signal. A
            // method, rather than an inline closure, gives the `?`
            // operator inside it somewhere to return to.
            self
                .render_pass(renderer, mode)
                // A render error must not abort the device: log and
                // continue. A dropped frame is recoverable; a panic in the
                // render loop (which runs every frame) is not.
                .unwrap_or_else(|_| {
                    log::error!("page render failed; skipping this frame");
                });
        });

        // WS6.2 full-invalidate escape hatch: a `force_redraw` frame (page
        // enter / layout change — the layout memo sets it, and it is set on
        // first build) must flush the WHOLE viewport, not just the redraw-root
        // rects the pass recorded. The page background outside the root widget
        // is never painted per-frame (the page-clear is disabled above), so an
        // incremental flush would leave it stale on a fresh screen. Reading it
        // AFTER the pass captures the value the layout memo set while pulling
        // `self.layout`. (Until Stage 6.1 makes layout invalidation targeted,
        // any layout change is conservatively a full flush — never a
        // regression.)
        self.force_redraw = false;
        self.needs_redraw = false;

        // WS6.2 full-invalidate escape hatch: a full-redraw frame (page enter /
        // layout change — the layout memo sets `full_flush`; `force_redraw()`
        // sets it too) must flush the WHOLE viewport, not just the redraw-root
        // rects the pass recorded. The page background outside the root widget
        // is not painted per-frame (the page-clear is disabled above), so an
        // incremental flush would leave it stale on a fresh screen. Read via the
        // non-reactive `full_flush` (NOT `force_redraw`, whose value-read would
        // spuriously re-dirty the render probe — see `full_flush`'s doc). Until
        // WS6.1 makes layout invalidation targeted, any layout change is
        // conservatively a full flush — never a regression.
        let full_flush = core::mem::take(&mut self.full_flush);
        if full_flush {
            let viewport = self.viewport;
            let mut damage = self.damage.borrow_mut();
            damage.clear();
            damage.push(Rect::new(Point::zero(), viewport));
        }

        drawn.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::{Page, dev::DevTools};
    use crate::{
        el::{El, arena::ElArena, ctx::*, view::View},
        font::FontCtx,
        prelude::*,
        test_support::TestPage,
    };
    use alloc::{rc::Rc, string::String};
    use rsact_reactive::prelude::*;
    use rsact_reactive::scope::new_scope;

    type NullWtf = Wtf<NullRenderer, (), (), ()>;

    fn create_null_page(root: impl View<NullWtf>) -> TestPage<NullWtf> {
        create_null_page_sized(Size::new_equal(1), root)
    }

    /// Like [`create_null_page`] but with an explicit viewport. The default is
    /// `1x1`, which clamps every fixed-size child to a pixel — layout-geometry
    /// tests need room for the per-slot advance to be observable.
    fn create_null_page_sized(
        viewport: Size,
        root: impl View<NullWtf>,
    ) -> TestPage<NullWtf> {
        // Mirror `UI::load_page` (WS3.1): the arena is created outside the page
        // scope (it keeps its explicit WS2 disposal), then the page is built
        // with a fresh scope current so every build-time reactive node the page
        // creates is scope-owned and disposed when the page drops. `root` is a
        // closure here in the scope-sensitive tests, so its `into_el` (and any
        // `Dynamic` effects) run inside `Page::new` while the scope is current.
        let arena = create_signal(ElArena::new()).name("Page arena");

        let scope = new_scope();
        TestPage::new(
            Page::new(
                (),
                root,
                arena,
                viewport,
                ().inert(),
                DevTools::default().signal(),
                Rc::new(FontCtx::new()),
                scope,
            ),
            NullRenderer::default(),
        )
    }

    /// Rebuilding a subtree must not leak the old subtree in the arena. A
    /// `Dynamic` (factory closure) rebuilds its child on each signal change; the
    /// arena's live-node count must stay constant across many rebuilds (before
    /// the fix, each rebuild orphaned the old child subtree in `els`).
    #[test]
    fn arena_rebuild_does_not_leak_subtree() {
        use crate::widget::{container::Container, label::Label};
        use rsact_reactive::runtime::with_new_runtime;

        with_new_runtime(|_| {
            let mut rebuild = create_signal(0i32);
            // Rebuilds are driven by the Dynamic's build effect on each `set`,
            // so no render is needed (rendering the null theme would hit an
            // unrelated pre-existing ColorStyle panic).
            let page = create_null_page(move || {
                rebuild.get(); // track: re-run the factory on each change
                Container::new(Label::new("x".inert()).into_el())
            });

            let baseline = page.arena.with(|arena| arena.el_count());
            assert!(baseline > 0, "arena should have nodes after first build");

            for i in 1..=20 {
                rebuild.set(i);
            }

            let after = page.arena.with(|arena| arena.el_count());
            assert_eq!(
                after,
                baseline,
                "arena leaked {} node(s) across 20 rebuilds",
                after as i64 - baseline as i64
            );
        });
    }

    /// WS2.3: dropping a page must dispose every render probe it owns — the
    /// page's `render_probe` and every element's `part_probes`. Otherwise each
    /// page navigation (goto frees the old page) leaks probe nodes unbounded.
    /// Measured via the probe (`observers`) count returning to baseline across
    /// 100 create → render → drop cycles (a goto round-trip).
    #[test]
    fn page_drop_disposes_all_probes() {
        use rsact_reactive::runtime::{
            current_runtime_profile, with_new_runtime,
        };

        with_new_runtime(|_| {
            let baseline = current_runtime_profile().observers;

            for _ in 0..100 {
                let mut page =
                    create_null_page(Label::new("x".inert()).into_el());
                // Render so the element's `part_probes` and the page's
                // `render_probe` are actually created.
                page.use_renderer(|_| {});
                drop(page);
            }

            let after = current_runtime_profile().observers;
            assert_eq!(
                after,
                baseline,
                "leaked {} probe(s) across 100 page create/render/drop cycles",
                after as i64 - baseline as i64
            );
        });
    }

    /// WS2.3: when a subtree leaves the tree (`set_children`/`set_single_child`
    /// → `remove_subtree`), the removed element's `part_probes` must be
    /// disposed, not orphaned — otherwise every list reconciliation / tab
    /// switch leaks probe nodes. Rendered once so the child owns a probe, then
    /// its subtree is removed via the arena's public `set_children`; the removed
    /// child's probe must be gone.
    #[test]
    fn subtree_removal_disposes_part_probes() {
        use rsact_reactive::runtime::{
            current_runtime_profile, with_new_runtime,
        };

        with_new_runtime(|_| {
            // A `Dynamic` (closure) root renders its `Label` child cleanly in
            // the null theme (a bare `Container` would hit the pre-existing
            // ColorStyle render panic). The child is registered under the root
            // via `set_single_child` at build.
            let mut page =
                create_null_page(|| Label::new("x".inert()).into_el());

            // Render so the child Label owns its "self" probe.
            page.use_renderer(|_| {});
            let with_child = current_runtime_profile().observers;

            // Remove the container's children directly through the arena (what a
            // widget does on a list/child change). This runs `remove_subtree`
            // over the Label, which must dispose its probe.
            let root = page.root;
            page.arena.update_untracked(|arena| {
                arena.set_children(root, alloc::vec::Vec::new());
            });

            let after = current_runtime_profile().observers;
            assert!(
                after < with_child,
                "remove_subtree did not dispose the removed subtree's probe(s) \
                 (before={with_child}, after={after})"
            );
        });
    }

    /// D2-F1 disposed-arena delayed-panic repro (WS3.1). A page's `Dynamic`
    /// reads an app-level signal that OUTLIVES the page. After navigating away
    /// (dropping the page — goto frees the old page), firing the app signal
    /// must not re-run the page's build-time effects: they were built against
    /// the now-freed arena. Before WS3.1 the page owned no scope, so the
    /// `Dynamic` build effect survived the drop, re-ran on the write, and
    /// touched the disposed arena (panic / logged dead-handle churn) — and the
    /// build-time nodes leaked. After WS3.1 the page scope disposes them on
    /// drop, so the write is inert and nothing survives.
    #[test]
    fn disposed_page_effect_does_not_panic_on_app_signal() {
        use rsact_reactive::{
            leak::{leak_report, leak_snapshot},
            runtime::with_new_runtime,
        };

        with_new_runtime(|_| {
            // App-level signal, created OUTSIDE any page — it must survive
            // navigation and remain safe to write.
            let mut app_signal = create_signal(0i32);

            let snap = leak_snapshot();

            {
                // A page whose `Dynamic` root subscribes to the app signal.
                let mut page = create_null_page(move || {
                    app_signal.get(); // Dynamic factory tracks app_signal
                    Label::new("x".inert()).into_el()
                });
                // Build the Dynamic child so its build effect actually runs.
                page.use_renderer(|_| {});
                // Navigate away.
                drop(page);
            }

            // Firing the app signal after the page is gone must not panic and
            // must not resurrect any disposed effect.
            app_signal.set(1);
            app_signal.set(2);

            // Every node the page built must be disposed; only nodes present at
            // snapshot time (the app signal) remain.
            let report = leak_report(&snap);
            assert!(
                report.is_empty(),
                "page leaked {} build-time node(s) after drop: {report}",
                report.len()
            );
        });
    }

    /// Navigation round-trip leak test (WS3.1 acceptance): building and
    /// dropping pages — what `goto` does on every navigation — must return the
    /// runtime node population to baseline. Before the per-page scope, each
    /// visited page leaked all its build-time nodes, growing unbounded with
    /// navigation depth.
    #[test]
    fn page_navigation_round_trip_is_leak_free() {
        use rsact_reactive::runtime::{
            current_runtime_profile, with_new_runtime,
        };

        with_new_runtime(|_| {
            // One warm-up cycle absorbs any first-time lazy allocation so the
            // baseline reflects the steady state.
            {
                let mut page =
                    create_null_page(|| Label::new("x".inert()).into_el());
                page.use_renderer(|_| {});
            }
            let baseline = current_runtime_profile().total();

            for _ in 0..50 {
                let mut page =
                    create_null_page(|| Label::new("x".inert()).into_el());
                page.use_renderer(|_| {});
                drop(page);
            }

            let after = current_runtime_profile().total();
            assert_eq!(
                after,
                baseline,
                "navigation leaked {} node(s) over 50 round-trips",
                after as i64 - baseline as i64
            );
        });
    }

    /// Subtree disposal (WS3.2): rebuilding a `Dynamic` child must not leak the
    /// old subtree's *reactive* nodes (the WS2 `arena_rebuild_does_not_leak_subtree`
    /// test covers only arena `ElData`; this covers signals/memos/layouts). The
    /// factory runs inside the `Dynamic` layout effect, so each rebuild's
    /// widgets are owned by that effect and disposed by its `cleanup` on the
    /// next run. A nested reactive subtree (`Container` > `Checkbox`, which mints
    /// a signal + layout) exercises recursive owned-child disposal.
    #[test]
    fn dynamic_rebuild_does_not_leak_reactive_nodes() {
        use crate::widget::{checkbox::Checkbox, container::Container};
        use rsact_reactive::runtime::{
            current_runtime_profile, with_new_runtime,
        };

        with_new_runtime(|_| {
            let mut rebuild = create_signal(0i32);
            let _page = create_null_page(move || {
                rebuild.get(); // track: re-run the factory on each change
                Container::new(Checkbox::new(false).into_el()).into_el()
            });

            // The layout effect ran once at construction; measure the steady
            // state, then rebuild many times.
            let baseline = current_runtime_profile().total();

            for i in 1..=20 {
                rebuild.set(i);
            }

            let after = current_runtime_profile().total();
            assert_eq!(
                after,
                baseline,
                "dynamic rebuild leaked {} reactive node(s) over 20 rebuilds",
                after as i64 - baseline as i64
            );
        });
    }

    /// PageState pruning (WS3.3): once an element leaves the arena (subtree
    /// replace / `Dynamic` rebuild / navigation), focus and pointer state must
    /// stop referencing it so events are never routed to a freed id (D2-F5);
    /// an id still present is kept.
    #[test]
    fn pagestate_forgets_removed_element_ids() {
        use crate::widget::{container::Container, label::Label};
        use rsact_reactive::runtime::with_new_runtime;

        with_new_runtime(|_| {
            let mut page = create_null_page(|| {
                Container::new(Label::new("x".inert()).into_el()).into_el()
            });
            // Build the tree so the Dynamic root's child exists.
            page.use_renderer(|_| {});

            let root = page.root;
            let child = page
                .arena
                .with(|arena| {
                    arena.children(root).and_then(|c| c.first().copied())
                })
                .expect("root must have a child after build");

            // Point every PageState reference at the child.
            page.state.focused = Some((child, 0));
            page.state.focus_pressed = true;
            page.state.pointer.captured_by = Some(child);
            page.state.pointer.hovered = Some(child);
            page.state.pointer.pressed = Some(child);

            // Remove the child subtree from the arena.
            page.arena.update_untracked(|arena| {
                arena.set_children(root, alloc::vec::Vec::new());
            });

            // Prune: every reference to the now-freed child is dropped.
            let arena = page.arena;
            arena.with(|arena| page.state.retain_existing(arena));

            assert_eq!(page.state.focused, None, "focused not cleared");
            assert!(!page.state.focus_pressed, "focus_pressed not cleared");
            assert_eq!(
                page.state.pointer.captured_by, None,
                "captured_by not cleared"
            );
            assert_eq!(page.state.pointer.hovered, None, "hovered not cleared");
            assert_eq!(page.state.pointer.pressed, None, "pressed not cleared");

            // A reference to a still-present element (the root) is retained.
            page.state.focused = Some((root, 3));
            arena.with(|arena| page.state.retain_existing(arena));
            assert_eq!(
                page.state.focused,
                Some((root, 3)),
                "focus on a live element was wrongly cleared"
            );
        });
    }

    /// WS3.5: when the arena child list and the layout tree diverge in length
    /// (a rebuild/list update between frames), the event pass must degrade to
    /// the common prefix and log — never abort. Here we force a divergence by
    /// dropping an arena child without relaying out, then route an event; the
    /// checked zip iterates the common prefix and reaching the end of the test
    /// (no panic) is the assertion.
    #[test]
    fn arena_layout_divergence_degrades_without_panic() {
        use crate::event::{Event, MouseEvent};
        use crate::widget::{flex::Flex, label::Label};
        use rsact_reactive::runtime::with_new_runtime;

        with_new_runtime(|_| {
            let mut page = create_null_page(
                Flex::col(alloc::vec![
                    Label::new("a".inert()).into_el(),
                    Label::new("b".inert()).into_el(),
                    Label::new("c".inert()).into_el(),
                ])
                .into_el(),
            );

            // Build the layout tree (3 children) without rendering — the null
            // theme panics on a Label's unset text color, and the event pass is
            // what we want to exercise anyway.
            let pt = page.layout().tree_root().outer.center();
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::MouseMove(pt),
            )));

            let root = page.root;
            let kids: Vec<_> = page
                .arena
                .with(|a| a.children(root).map(|c| c.to_vec()))
                .unwrap_or_default();
            assert_eq!(kids.len(), 3, "flex root should have 3 children");

            // Desync: drop one arena child WITHOUT relayout, so the arena (2)
            // and the still-cached layout (3) diverge.
            page.arena.update_untracked(|a| {
                a.set_children(root, alloc::vec![kids[0], kids[1]])
            });

            // Route another event through the divergence. No panic ⇒ pass.
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::MouseMove(pt),
            )));
        });
    }

    // WS5.1 identity lock (render/layout side): a transparent `Dynamic` in the
    // middle of a flex must be flattened by `effective_children` into a real
    // layout slot — the flex lays out THREE children (a, b-via-Dynamic, c) at
    // distinct, increasing y, not a collapsed/zero middle. Locks the
    // arena-walk ↔ `LayoutModel` correspondence the render/event passes consume
    // (guards the positional-zip → ElId-dispatch refactor).
    #[test]
    fn dynamic_child_is_laid_out_as_a_real_flex_slot() {
        use crate::widget::{dynamic::dynamic, edge::Edge, flex::Flex};
        use rsact_reactive::runtime::with_new_runtime;

        // Fixed-size children (not zero-height null-font text) so the flex main
        // axis advances a definite amount per slot and the y-positions are
        // unambiguous even in the 1x1 null viewport.
        fn boxed() -> impl View<NullWtf> {
            Edge::<NullWtf>::new().width(10u32).height(10u32)
        }

        with_new_runtime(|_| {
            let mut page = create_null_page_sized(
                Size::new_equal(100),
                Flex::col(alloc::vec![
                    boxed().into_el(),
                    dynamic(|| boxed()).into_el(),
                    boxed().into_el(),
                ])
                .into_el(),
            );

            let ys = {
                let m = page.layout();
                let root = m.tree_root();
                assert_eq!(
                    root.children_len(),
                    3,
                    "flex must lay out 3 effective children through the Dynamic"
                );
                root.children()
                    .map(|c| c.outer.top_left.y)
                    .collect::<Vec<_>>()
            };
            assert!(
                ys[0] < ys[1] && ys[1] < ys[2],
                "the Dynamic's child must occupy a real middle slot, got y={ys:?}"
            );
        });
    }

    // WS5.1 identity lock (event side): a focused widget nested inside a
    // transparent `Dynamic` must still receive events. `EventPass::run` walks
    // the whole tree (`run_`), descending through the Dynamic's
    // transparent-layout branch to reach the checkbox; a click toggles it, which
    // redraws exactly that widget. A broken transparent traversal would never
    // dispatch to it, leaving the page quiescent (redraw 0). Guards the
    // positional-zip → ElId-dispatch refactor on the event pass.
    #[test]
    fn focused_event_reaches_child_through_dynamic() {
        use crate::event::{Event, PressEvent};
        use crate::widget::{checkbox::Checkbox, dynamic::dynamic, flex::Flex};
        use rsact_reactive::runtime::with_new_runtime;

        with_new_runtime(|_| {
            let mut page = create_null_page(
                Flex::col(alloc::vec![
                    Checkbox::new(false).into_el(),
                    dynamic(|| Checkbox::new(false)).into_el(),
                    Checkbox::new(false).into_el(),
                ])
                .into_el(),
            );

            // Settle layout/style, then confirm quiescent.
            for _ in 0..4 {
                page.use_renderer(|_| {});
            }
            page.take_draw_calls();
            page.use_renderer(|_| {});
            assert_eq!(
                page.take_draw_calls(),
                0,
                "page must be quiescent before the click"
            );

            // The middle flex child is the transparent Dynamic; its single arena
            // child is the checkbox we target.
            let root = page.root;
            let flex_kids = page
                .arena
                .with(|a| a.children(root).map(|c| c.to_vec()))
                .unwrap_or_default();
            assert_eq!(flex_kids.len(), 3, "flex should have 3 children");
            let dynamic_id = flex_kids[1];
            let cb_id = page
                .arena
                .with(|a| a.children(dynamic_id).map(|c| c.to_vec()))
                .unwrap_or_default();
            assert_eq!(cb_id.len(), 1, "Dynamic should wrap exactly one child");
            let cb_id = cb_id[0];

            // Focus the Dynamic-wrapped checkbox and click it (press + release).
            page.state.focused = Some((cb_id, 0));
            let _ = page.handle_events(
                [
                    Event::Press(PressEvent::Press),
                    Event::Press(PressEvent::Release),
                ]
                .into_iter(),
            );

            // Reaching it through the Dynamic => it toggled => it redrew.
            page.use_renderer(|_| {});
            assert_eq!(
                page.take_draw_calls(),
                1,
                "focused click must reach the checkbox through the Dynamic"
            );
        });
    }

    #[test]
    fn draw_on_demand() {
        let mut redraw_signal_data = String::new().signal();

        let mut page =
            create_null_page(Label::new(redraw_signal_data).into_el());

        assert_eq!(page.take_draw_calls(), 0);

        // First draw request without changes subscribes to reactive values
        // inside drawing context.
        page.use_renderer(|_| {});
        assert_eq!(page.take_draw_calls(), 1);

        // Nothing changed inside drawing context
        page.use_renderer(|_| {});
        assert_eq!(page.take_draw_calls(), 0);

        // Something's changed
        redraw_signal_data.update(|string| string.push_str("kek"));
        page.use_renderer(|_| {});
        assert_eq!(page.take_draw_calls(), 1);

        page.use_renderer(|_| {});
        assert_eq!(page.take_draw_calls(), 0);
        page.use_renderer(|_| {});
        assert_eq!(page.take_draw_calls(), 0);
    }

    // Regression: a `Checkbox` built from a plain `bool` (the "uncontrolled"
    // case) must still redraw when toggled by a click/press. Previously the
    // checked value was stored as a `MaybeSignal<bool>`, which is `Inert` for a
    // plain value: the `value.get()` in `render` didn't track and the
    // `value.update()` in `on_event` didn't notify, so the toggle changed the
    // internal value but never triggered a redraw.
    #[test]
    fn checkbox_redraws_on_toggle() {
        use crate::event::{Event, PressEvent};

        // Isolate in a fresh runtime: all null pages share page id `()`, so the
        // page-observer key `("render_page", ())` collides in the shared
        // `static_observers` and tests would pollute each other otherwise.
        with_new_runtime(|_| {
            let mut page = create_null_page(Checkbox::new(false).into_el());

            // Render until the page settles (the first passes warm up
            // layout/style), then confirm it is quiescent with nothing changing.
            for _ in 0..4 {
                page.use_renderer(|_| {});
            }
            page.take_draw_calls();
            page.use_renderer(|_| {});
            assert_eq!(
                page.take_draw_calls(),
                0,
                "page must be quiescent before the toggle"
            );

            // Focus the checkbox (it is the page root) and click it (press then
            // release) — the checkbox toggles its value on release.
            page.state.focused = Some((page.root, 0));
            let _ = page.handle_events(
                [
                    Event::Press(PressEvent::Press),
                    Event::Press(PressEvent::Release),
                ]
                .into_iter(),
            );

            // Toggling the checked value must redraw the checkbox. Before the
            // fix the inert `MaybeSignal<bool>` neither tracked the read in
            // `render` nor notified the write in `on_event`, so this stayed 0.
            page.use_renderer(|_| {});
            assert_eq!(
                page.take_draw_calls(),
                1,
                "toggling the checkbox must trigger a redraw"
            );
        });
    }

    // Reproduce the real user scenario: a full mouse click (press + release,
    // with the pointer hovering the widget). The checkbox toggles on release.
    #[test]
    fn checkbox_redraws_on_mouse_click() {
        use crate::event::{Event, MouseButton, MouseEvent};

        with_new_runtime(|_| {
            let mut page = create_null_page(Checkbox::new(false).into_el());

            // Reading the layout memo builds/lays out the tree; no rendering is
            // needed. (Rendering a `Label` in the null theme panics on the
            // unset text color — the same pre-existing limitation as
            // `draw_on_demand` — and the click logic under test runs entirely in
            // `handle_events`, independent of rendering.)
            let pt = page.layout().tree_root().outer.center();

            // Pointer hovers the checkbox, then settle any hover-driven redraw.
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::MouseMove(pt),
            )));
            page.use_renderer(|_| {});
            page.take_draw_calls();

            // Full left-button click (down then up) toggles the checkbox.
            let _ = page.handle_events(
                [
                    Event::Mouse(MouseEvent::ButtonDown(
                        MouseButton::Left,
                        Some(pt),
                    )),
                    Event::Mouse(MouseEvent::ButtonUp(
                        MouseButton::Left,
                        Some(pt),
                    )),
                ]
                .into_iter(),
            );

            page.use_renderer(|_| {});
            assert_eq!(
                page.take_draw_calls(),
                1,
                "a mouse click must trigger a redraw of the checkbox"
            );
        });
    }

    // The click action fires once, on the release edge, and only when the
    // release lands on the same widget that received the press (the globally
    // tracked `pointer.pressed`). This is the release-edge semantics that
    // replaced the per-widget press bool.
    #[test]
    fn button_click_fires_on_release_not_press() {
        use crate::event::{Event, MouseButton, MouseEvent};

        with_new_runtime(|_| {
            let mut clicks = create_signal(0u32);
            let mut page = create_null_page(
                Button::new(Label::new("x"))
                    .on_click(move || clicks.update(|c| *c += 1))
                    .into_el(),
            );

            // Reading the layout memo builds/lays out the tree; no rendering is
            // needed. (Rendering a `Label` in the null theme panics on the
            // unset text color — the same pre-existing limitation as
            // `draw_on_demand` — and the click logic under test runs entirely in
            // `handle_events`, independent of rendering.)
            let pt = page.layout().tree_root().outer.center();
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::MouseMove(pt),
            )));

            // Press alone must NOT fire the click.
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::ButtonDown(MouseButton::Left, Some(pt)),
            )));
            assert_eq!(clicks.get(), 0, "click must not fire on press-down");
            assert_eq!(
                page.state.pointer.pressed,
                Some(page.root),
                "press-down claims the global press target"
            );

            // Release on the same widget fires the click exactly once and
            // clears the global press state.
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::ButtonUp(MouseButton::Left, Some(pt)),
            )));
            assert_eq!(clicks.get(), 1, "click fires once on release");
            assert_eq!(
                page.state.pointer.pressed, None,
                "press is cleared after release"
            );
        });
    }

    // Press on the widget then release outside its bounds (drag-off) must NOT
    // fire the click, but the global press state is still cleared.
    #[test]
    fn button_click_cancelled_when_released_outside() {
        use crate::event::{Event, MouseButton, MouseEvent};

        with_new_runtime(|_| {
            let mut clicks = create_signal(0u32);
            let mut page = create_null_page(
                Button::new(Label::new("x"))
                    .on_click(move || clicks.update(|c| *c += 1))
                    .into_el(),
            );

            // Reading the layout memo builds/lays out the tree; no rendering is
            // needed. (Rendering a `Label` in the null theme panics on the
            // unset text color — the same pre-existing limitation as
            // `draw_on_demand` — and the click logic under test runs entirely in
            // `handle_events`, independent of rendering.)
            let pt = page.layout().tree_root().outer.center();
            let outside = Point::new(10_000, 10_000);

            let _ = page.handle_events(
                [
                    Event::Mouse(MouseEvent::MouseMove(pt)),
                    Event::Mouse(MouseEvent::ButtonDown(
                        MouseButton::Left,
                        Some(pt),
                    )),
                    Event::Mouse(MouseEvent::ButtonUp(
                        MouseButton::Left,
                        Some(outside),
                    )),
                ]
                .into_iter(),
            );

            assert_eq!(
                clicks.get(),
                0,
                "release outside bounds must not click"
            );
            assert_eq!(
                page.state.pointer.pressed, None,
                "press state is cleared on release even when released outside"
            );
        });
    }

    // Pointer capture: while a widget holds the pointer, mouse moves are routed
    // to it exclusively — hover is frozen and the cursor is tracked even far
    // outside the widget's bounds. Releasing ends the capture and resumes
    // normal hit-testing.
    #[test]
    fn capture_freezes_hover_and_tracks_cursor_outside() {
        use crate::event::{Event, MouseButton, MouseEvent};

        with_new_runtime(|_| {
            let mut page = create_null_page(Checkbox::new(false).into_el());
            let cb = page.root;

            let pt = page.layout().tree_root().outer.center();
            let outside = Point::new(10_000, 10_000);

            // Hover the checkbox.
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::MouseMove(pt),
            )));
            assert_eq!(
                page.state.pointer.hovered,
                Some(cb),
                "hovered before press"
            );

            // Press to capture the pointer, then move the cursor far outside.
            let _ = page.handle_events(
                [
                    Event::Mouse(MouseEvent::ButtonDown(
                        MouseButton::Left,
                        Some(pt),
                    )),
                    Event::Mouse(MouseEvent::MouseMove(outside)),
                ]
                .into_iter(),
            );

            assert_eq!(
                page.state.pointer.captured_by,
                Some(cb),
                "press captures the pointer"
            );
            // Capture freezes hover: moving outside does NOT clear it.
            assert_eq!(
                page.state.pointer.hovered,
                Some(cb),
                "hover is frozen while captured"
            );
            // The move was still processed — the captured widget follows the
            // cursor even outside its own bounds.
            assert_eq!(page.state.pointer.pos, Some(outside));

            // Releasing (even outside) ends the capture.
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::ButtonUp(MouseButton::Left, Some(outside)),
            )));
            assert_eq!(
                page.state.pointer.captured_by, None,
                "release ends capture"
            );

            // With capture ended, moves resume normal hit-testing (the cursor
            // is outside, so nothing is hovered).
            let _ = page.handle_events(core::iter::once(Event::Mouse(
                MouseEvent::MouseMove(outside),
            )));
            assert_eq!(
                page.state.pointer.hovered, None,
                "hover resumes after capture ends"
            );
        });
    }

    // Reproduce the widget_gallery structure: the checkbox lives inside a
    // `dynamic(...)` factory (built in a `create_effect`, held in a Signal,
    // inserted via `set_single_child`). This is the real-world embedding.
    #[test]
    fn dynamic_checkbox_redraws_on_toggle() {
        use crate::el::ElId;
        use crate::event::{Event, PressEvent};

        fn find_checkbox<W: WidgetCtx>(
            arena: &ElArena<W>,
            id: ElId,
        ) -> Option<ElId> {
            if arena.expect(id).map_or(false, |d| {
                d.state.flags.is_focusable() && d.state.flags.is_clickable()
            }) {
                return Some(id);
            }
            for &child in arena.children(id).unwrap_or(&[]) {
                if let Some(found) = find_checkbox(arena, child) {
                    return Some(found);
                }
            }
            None
        }

        with_new_runtime(|_| {
            // Factory reads an external signal (like widget_gallery's tab
            // signal), so the checkbox's own signals are created inside the
            // tracking effect.
            let tab = create_signal(0u8);
            let mut page = create_null_page(dynamic(move || {
                let _ = tab.get();
                Checkbox::new(false)
            }));

            for _ in 0..4 {
                page.use_renderer(|_| {});
            }
            page.take_draw_calls();
            page.use_renderer(|_| {});
            assert_eq!(
                page.take_draw_calls(),
                0,
                "page must be quiescent before the toggle"
            );

            let checkbox_id = page
                .arena
                .with(|arena| find_checkbox(arena, page.root))
                .expect("checkbox must be in the tree");
            page.state.focused = Some((checkbox_id, 0));
            let _ = page.handle_events(
                [
                    Event::Press(PressEvent::Press),
                    Event::Press(PressEvent::Release),
                ]
                .into_iter(),
            );

            page.use_renderer(|_| {});
            assert_eq!(
                page.take_draw_calls(),
                1,
                "toggling a checkbox inside dynamic() must trigger a redraw"
            );
        });
    }

    // A renderer that records primitive draw calls. NullRenderer is a no-op and
    // cannot reveal whether a primitive (e.g. the check-icon path) was actually
    // drawn.
    mod recording_renderer {
        use crate::prelude::*;
        use alloc::rc::Rc;
        use core::cell::Cell;

        #[derive(Clone, Default)]
        pub struct RecordingRenderer {
            pub paths: Rc<Cell<usize>>,
        }

        impl RenderTarget for RecordingRenderer {
            type Color = NullColor;
            fn draw(
                &mut self,
                _pixels: impl Iterator<
                    Item = crate::render::output::pixel::Pixel<Self::Color>,
                >,
            ) {
            }
        }

        impl Renderer for RecordingRenderer {
            type Color = NullColor;
            type Policy = rsact_render::region::Unbounded;
            fn size(&self) -> Size {
                Size::new_equal(64)
            }
            fn push_clip(&mut self, _area: Rect) {}
            fn pop_clip(&mut self) {}
            fn fill_solid(
                &mut self,
                _rect: Rect,
                _color: Self::Color,
            ) -> RenderResult {
                Ok(())
            }
            fn pixel(
                &mut self,
                _point: Point,
                _color: Self::Color,
            ) -> RenderResult {
                Ok(())
            }
            fn line(
                &mut self,
                _from: Point,
                _to: Point,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn rect(
                &mut self,
                _rect: Rect,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn rounded_rect(
                &mut self,
                _rect: Rect,
                _corners: CornerRadii,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn circle(
                &mut self,
                _top_left: Point,
                _diameter: u32,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn arc(
                &mut self,
                _top_left: Point,
                _diameter: u32,
                _start: Angle,
                _sweep: Angle,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn ellipse(
                &mut self,
                _bounding_box: Rect,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn sector(
                &mut self,
                _top_left: Point,
                _diameter: u32,
                _start: Angle,
                _sweep: Angle,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn polygon(
                &mut self,
                _points: &[Point],
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
            fn path(
                &mut self,
                _path: &Path,
                _style: &DrawStyle<Self::Color>,
            ) -> RenderResult {
                self.paths.set(self.paths.get() + 1);
                Ok(())
            }
            fn image<'a>(
                &mut self,
                _image: crate::render::image::DrawImage<'a, Self::Color>,
            ) -> RenderResult {
                Ok(())
            }
        }
    }

    // The exact user scenario, end-to-end: a checkbox that starts UNCHECKED,
    // then is toggled to checked via a click. The resulting redraw must
    // actually draw the check-icon path (the NullRenderer-based redraw tests
    // only prove the page re-renders, not that the icon is drawn).
    #[test]
    fn toggling_checkbox_to_checked_draws_icon() {
        use crate::event::{Event, PressEvent};
        use recording_renderer::RecordingRenderer;

        type RecWtf = Wtf<RecordingRenderer, (), (), ()>;

        with_new_runtime(|_| {
            let renderer = RecordingRenderer::default();
            let paths = renderer.paths.clone();
            let arena = create_signal(ElArena::new()).name("Page arena");
            let scope = new_scope();
            let mut page: TestPage<RecWtf> = TestPage::new(
                Page::new(
                    (),
                    Checkbox::<RecWtf>::new(false),
                    arena,
                    Size::new_equal(64),
                    ().inert(),
                    DevTools::default().signal(),
                    Rc::new(FontCtx::new()),
                    scope,
                ),
                renderer,
            );

            // Settle; value is false, so no icon is drawn yet.
            for _ in 0..4 {
                page.use_renderer(|_| {});
            }
            let before = paths.get();

            // Click (press + release) the focused checkbox -> toggle to checked.
            page.state.focused = Some((page.root, 0));
            let _ = page.handle_events(
                [
                    Event::Press(PressEvent::Press),
                    Event::Press(PressEvent::Release),
                ]
                .into_iter(),
            );

            page.use_renderer(|_| {});
            let after = paths.get();

            assert!(
                after > before,
                "toggling a checkbox to checked must draw the icon on the \
                 redraw (path calls before={before}, after={after})"
            );
        });
    }

    // Same as above but the checkbox is NESTED (inside `dynamic`, like
    // widget_gallery). Reproduces the real bug: the page re-renders on toggle,
    // but the nested checkbox's render observer is skipped so the icon is never
    // drawn.
    #[test]
    fn nested_checkbox_toggle_draws_icon() {
        use crate::el::ElId;
        use crate::event::{Event, PressEvent};
        use recording_renderer::RecordingRenderer;

        type RecWtf = Wtf<RecordingRenderer, (), (), ()>;

        fn find_checkbox(arena: &ElArena<RecWtf>, id: ElId) -> Option<ElId> {
            if arena.expect(id).map_or(false, |d| {
                d.state.flags.is_focusable() && d.state.flags.is_clickable()
            }) {
                return Some(id);
            }
            for &child in arena.children(id).unwrap_or(&[]) {
                if let Some(found) = find_checkbox(arena, child) {
                    return Some(found);
                }
            }
            None
        }

        with_new_runtime(|_| {
            let renderer = RecordingRenderer::default();
            let paths = renderer.paths.clone();
            let arena = create_signal(ElArena::new()).name("Page arena");
            let scope = new_scope();
            let mut page: TestPage<RecWtf> = TestPage::new(
                Page::new(
                    (),
                    dynamic(|| Checkbox::<RecWtf>::new(false)),
                    arena,
                    Size::new_equal(64),
                    ().inert(),
                    DevTools::default().signal(),
                    Rc::new(FontCtx::new()),
                    scope,
                ),
                renderer,
            );

            for _ in 0..4 {
                page.use_renderer(|_| {});
            }
            let before = paths.get();

            let checkbox_id = page
                .arena
                .with(|arena| find_checkbox(arena, page.root))
                .expect("checkbox must be in the tree");
            page.state.focused = Some((checkbox_id, 0));
            let _ = page.handle_events(
                [
                    Event::Press(PressEvent::Press),
                    Event::Press(PressEvent::Release),
                ]
                .into_iter(),
            );

            page.use_renderer(|_| {});
            let after = paths.get();

            assert!(
                after > before,
                "toggling a NESTED checkbox must draw the icon on the redraw \
                 (path calls before={before}, after={after})"
            );
        });
    }

    // WS6.9: page-render draw-op goldens. Build a page through the *reusable*
    // RecordingRenderer (rsact-render), render one full frame, and lock its
    // draw-op log against a blessed golden file under `tests/goldens/`. This
    // exercises the whole harness end-to-end and makes every later WS6 damage
    // change reviewable as a golden diff — a damage frame will be a strict
    // subset of this full-frame log.
    //
    // Deterministic by construction: fixed 64x64 viewport, the geometry-only
    // (colour-agnostic) op log, and a `Checkbox` (which resolves to the null
    // theme's *default* colours — a bare `Container` would hit the
    // `ColorStyle::expect` panic). Bless/update with `UPDATE_GOLDENS=1`.
    mod render_goldens {
        use super::*;
        use crate::render::{
            golden::assert_text_golden,
            record::{RecordingRenderer, format_ops},
        };
        use crate::widget::checkbox::Checkbox;
        use rsact_reactive::runtime::with_new_runtime;

        type RecWtf = Wtf<RecordingRenderer<NullColor>, (), (), ()>;

        /// Build a `RecWtf` page for `root`, settle its reactive render, run
        /// `interact` (input/state changes to reach the state under test), then
        /// force exactly one full frame and return its draw-op log as text.
        ///
        /// `force_redraw()` re-arms every part (WS6.4.0(iv): the flag is OR-ed
        /// into each part's gate, no longer tracked by its probe), so the captured
        /// frame is the complete paint — not a partial, probe-gated one — and
        /// independent of exactly which parts `interact` happened to dirty.
        fn full_frame_log_after(
            root: impl View<RecWtf>,
            interact: impl FnOnce(&mut Page<RecWtf>),
        ) -> String {
            let renderer =
                RecordingRenderer::<NullColor>::new(Size::new_equal(64));
            let recorder = renderer.clone(); // shares the op log via Rc
            let arena = create_signal(ElArena::new()).name("Page arena");
            let scope = new_scope();
            let mut page: TestPage<RecWtf> = TestPage::new(
                Page::new(
                    (),
                    root,
                    arena,
                    Size::new_equal(64),
                    ().inert(),
                    DevTools::default().signal(),
                    Rc::new(FontCtx::new()),
                    scope,
                ),
                renderer,
            );

            // Settle: the first frames may run the render observer a few times
            // as reactive state stabilises. We discard those and capture a
            // single forced frame below.
            for _ in 0..4 {
                page.use_renderer(|_| {});
            }

            interact(&mut page);

            recorder.clear();
            page.force_redraw();
            page.use_renderer(|_| {});

            format_ops(&recorder.ops())
        }

        /// No-interaction convenience wrapper.
        fn full_frame_log(root: impl View<RecWtf>) -> String {
            full_frame_log_after(root, |_| {})
        }

        #[test]
        fn checkbox_unchecked_page() {
            with_new_runtime(|_| {
                let log = full_frame_log(Checkbox::<RecWtf>::new(false));
                assert_text_golden(
                    env!("CARGO_MANIFEST_DIR"),
                    "checkbox_unchecked_64.txt",
                    &log,
                );
            });
        }

        /// The same checkbox after a click toggles it to checked — the golden
        /// must now include the check-icon `Path` the unchecked frame omits.
        /// This is the harness proving it distinguishes a real visual state
        /// change, not just that *something* drew.
        #[test]
        fn checkbox_checked_page() {
            use crate::event::{Event, PressEvent};

            with_new_runtime(|_| {
                let log = full_frame_log_after(
                    Checkbox::<RecWtf>::new(false),
                    |page| {
                        // Focus the checkbox, then press+release to toggle it.
                        page.state.focused = Some((page.root, 0));
                        let _ = page.handle_events(
                            [
                                Event::Press(PressEvent::Press),
                                Event::Press(PressEvent::Release),
                            ]
                            .into_iter(),
                        );
                    },
                );
                assert_text_golden(
                    env!("CARGO_MANIFEST_DIR"),
                    "checkbox_checked_64.txt",
                    &log,
                );
            });
        }
    }

    /// WS6.4b: the geometry cull in `render_part`. What the op-log measurements
    /// (`tests/tile_schedule.rs`) cannot show is the *behaviour* around a culled
    /// part — that it comes back, and that skipping it does not leave the page
    /// spinning. Both are properties of the reactive graph, so they are asserted
    /// here rather than counted there.
    /// WS6.4c: the collect/paint split — probes stop gating paint.
    ///
    /// Every test here fails on the pre-split design, and the reason is always
    /// the same one sentence: `render_part`'s probe used to both *detect change*
    /// and *authorize paint*, so a second pass over an already-painted frame
    /// found every probe clean and painted nothing. Under tiling that is not a
    /// missed optimization, it is a tile flushed with holes.
    mod collect_paint {
        use super::culling::{RecWtf, two_checkbox_page};
        use super::*;
        use crate::render::record::{DrawOp, RecordingRenderer};
        use rsact_reactive::runtime::with_new_runtime;

        const VIEWPORT: Rect =
            Rect::new(Point::zero(), Size { width: 64, height: 64 });

        /// Ops that actually paint. `Clip` carries no bounds, obliges nobody and
        /// is excluded everywhere in WS6 — pushing a region clip must not read
        /// as drawing.
        fn painted(recorder: &RecordingRenderer<NullColor>) -> usize {
            recorder
                .ops()
                .iter()
                .filter(|op| op.bounds().is_some())
                .count()
        }

        /// A settled page whose probes are all clean.
        fn settled(
            second: Signal<bool>,
        ) -> (TestPage<RecWtf>, RecordingRenderer<NullColor>) {
            let (mut page, recorder) = two_checkbox_page(second);
            page.force_redraw();
            page.use_renderer(|_| {});
            recorder.clear();
            (page, recorder)
        }

        #[test]
        fn collect_plans_the_frame_and_paints_nothing() {
            with_new_runtime(|_| {
                let mut second = create_signal(false);
                let (mut page, recorder) = settled(second);

                second.set(true);
                let dirty = page.collect();

                assert!(dirty, "a changed signal must make the plan non-empty");
                assert_eq!(
                    painted(&recorder),
                    0,
                    "collect must discard its drawing — the mute sits at the \
                     drawing seam, before any primitive rasterizes"
                );
                assert!(
                    !page.damage_snapshot().is_empty(),
                    "collect must leave the damage rects behind as the plan"
                );
            });
        }

        #[test]
        fn collect_cleans_probes_so_an_unchanged_frame_is_idle() {
            with_new_runtime(|_| {
                let mut second = create_signal(false);
                let (mut page, _recorder) = settled(second);

                second.set(true);
                assert!(page.collect(), "the change must be seen once");
                assert!(
                    !page.collect(),
                    "collect must CLEAN the probes it polled; leaving them dirty \
                     would re-plan the same rect every frame forever"
                );
            });
        }

        /// **The property the whole item exists for.** After `collect` has run
        /// and cleaned every probe, a paint pass must still paint — because it
        /// is selected by geometry, not by dirtiness.
        #[test]
        fn paint_repaints_what_the_now_clean_probes_would_have_skipped() {
            with_new_runtime(|_| {
                let mut second = create_signal(false);
                let (mut page, recorder) = settled(second);

                second.set(true);
                page.collect();
                assert_eq!(painted(&recorder), 0, "collect painted");

                recorder.clear();
                page.paint_region(VIEWPORT).unwrap();

                assert!(
                    painted(&recorder) > 0,
                    "paint must not be gated by the probes collect just \
                     cleaned — on a tile there are no previous pixels to keep"
                );
            });
        }

        /// A tile has no history, so a region repaints EVERYTHING inside it,
        /// clean widgets included — not just the one that changed.
        #[test]
        fn paint_repaints_every_part_in_the_region_not_only_the_dirty_one() {
            with_new_runtime(|_| {
                let mut second = create_signal(false);
                let (mut page, recorder) = settled(second);

                // Only the SECOND checkbox changes...
                second.set(true);
                page.collect();
                recorder.clear();

                // ...but a full-viewport region must repaint both, or the tile
                // would be flushed with a hole where the first one belongs.
                page.paint_region(VIEWPORT).unwrap();
                let ops = recorder.ops();
                let top = ops
                    .iter()
                    .filter_map(DrawOp::bounds)
                    .filter(|b| b.top_left.y < super::culling::CLIP_H as i32)
                    .count();
                assert!(
                    top > 0,
                    "the unchanged part inside the region must repaint too"
                );
            });
        }

        /// Paint is untracked and probe-free, so it neither consumes a pending
        /// change nor subscribes anything. Both halves matter: consuming would
        /// lose the repaint, subscribing would donate every leaf's dependencies
        /// to whatever observer is ambient and destroy targeted invalidation.
        #[test]
        fn paint_neither_consumes_dirtiness_nor_subscribes() {
            with_new_runtime(|_| {
                let mut second = create_signal(false);
                let (mut page, _recorder) = settled(second);

                second.set(true);
                // Paint BEFORE planning: the change must survive it.
                page.paint_region(VIEWPORT).unwrap();
                page.collect();
                assert!(
                    !page.damage_snapshot().is_empty(),
                    "a paint pass consumed the pending change — it must not \
                     poll (or clean) any probe, or the plan loses the very \
                     rect it exists to find"
                );

                // And after a planned frame, painting again leaves the page idle
                // rather than re-arming it.
                page.paint_region(VIEWPORT).unwrap();
                page.paint_region(VIEWPORT).unwrap();
                assert!(
                    !page.collect(),
                    "repeated paints left the page armed — painting is not an \
                     invalidation event"
                );
            });
        }

        /// Paint must not feed the plan channel: under tiling the region already
        /// IS the flush unit, so damage recorded while painting would make every
        /// painted region re-damage itself, frame after frame (6.4d(4)).
        #[test]
        fn paint_does_not_add_to_the_damage_plan() {
            with_new_runtime(|_| {
                let mut second = create_signal(false);
                let (mut page, _recorder) = settled(second);

                second.set(true);
                page.collect();
                let plan = page.damage_snapshot();
                assert!(!plan.is_empty());

                page.paint_region(VIEWPORT).unwrap();
                assert_eq!(
                    page.damage_snapshot(),
                    plan,
                    "painting must leave the plan exactly as collect left it"
                );
            });
        }
    }

    /// WS6.4c(F): clipping is widget behaviour the framework reads, not a call a
    /// widget makes inside its own `render`.
    /// WS5.5: box-model and border behaviour after the retained layout copy was
    /// deleted.
    ///
    /// **On what these do and do not prove.** The drift `button.rs` admitted —
    /// *"a padding/border bound to a signal after build drifts here (the
    /// retained snapshot isn't the live arena value)"* — is now impossible
    /// **structurally**: the widget has no layout copy to go stale, so no test
    /// can fail for it. A test asserting "reactive padding reaches the render"
    /// would have passed before this change too, because padding always flowed
    /// through the *arena's* live copy into the layout model; only
    /// `border_width` was ever read from the snapshot.
    ///
    /// So these pin the two things that ARE newly true: the box model still
    /// drives the render end-to-end after the deletion, and a border width can
    /// now be set through the style — the capability the move bought, and one a
    /// layout-side width could never have.
    mod reactive_box_model {
        use super::culling::RecWtf;
        use super::*;
        use crate::render::record::{DrawOp, RecordingRenderer};
        use rsact_reactive::runtime::with_new_runtime;

        #[test]
        fn a_reactive_padding_moves_what_is_painted() {
            with_new_runtime(|_| {
                let mut padding = create_signal(2u32);
                let renderer =
                    RecordingRenderer::<NullColor>::new(Size::new_equal(64));
                let recorder = renderer.clone();
                let arena =
                    create_signal(ElArena::<RecWtf>::new()).name("Page arena");
                let scope = new_scope();
                let mut page = TestPage::new(
                    Page::new(
                        (),
                        Container::<RecWtf>::new(
                            Checkbox::new(false).into_el(),
                        )
                        .padding(padding),
                        arena,
                        Size::new_equal(64),
                        ().inert(),
                        DevTools::default().signal(),
                        Rc::new(FontCtx::new()),
                        scope,
                    ),
                    renderer,
                );
                for _ in 0..4 {
                    page.use_renderer(|_| {});
                }

                // Where does the inner content sit at padding = 2?
                let inner_at = |recorder: &RecordingRenderer<NullColor>| {
                    recorder
                        .ops()
                        .iter()
                        .filter_map(DrawOp::bounds)
                        .map(|b| b.top_left.x)
                        .max()
                };

                recorder.clear();
                page.force_redraw();
                page.use_renderer(|_| {});
                let before = inner_at(&recorder);

                padding.set(10);
                recorder.clear();
                page.use_renderer(|_| {});
                let after = inner_at(&recorder);

                assert!(before.is_some() && after.is_some(), "nothing painted");
                assert_ne!(
                    before, after,
                    "the box model stopped reaching the render after the \
                     retained layout copy was deleted"
                );
            });
        }

        /// The capability the move bought: a border width that comes from the
        /// style, and can therefore differ per pseudo-class — which a
        /// layout-side width could never do, because layout must not consult
        /// the stylist (a hover-driven relayout would be thrash).
        ///
        /// Asserted at the style layer rather than on pixels because an
        /// inside-aligned stroke changes neither the primitive nor its bounds,
        /// so the op log cannot see it; the plumbing is what is new here.
        #[test]
        fn a_border_width_is_a_style_property_and_resolves_per_pseudoclass() {
            use crate::style::Style as _;
            use crate::widget::button::ButtonStyle;

            let base = ButtonStyle::<NullColor>::base();
            assert_eq!(
                base.container.border.width, 0,
                "the base style must not invent a border"
            );

            // The setter `declare_widget_style!` generates beside
            // `border_color`/`outline_width` (WS5.5).
            let hovered = ButtonStyle::<NullColor>::base().border_width(3);
            assert_eq!(hovered.container.border.width, 3);

            // And it is a plain value on the style, so a style fn is free to
            // return a different one per selector — the whole point.
            assert_ne!(
                base.container.border.width,
                hovered.container.border.width
            );
        }
    }

    mod widget_clipping {
        use super::*;
        use crate::{
            event::{Event, MouseButton, MouseEvent},
            render::record::{DrawOp, RecordingRenderer},
            widget::{checkbox::Checkbox, scrollable::Scrollable},
        };
        use rsact_reactive::runtime::with_new_runtime;

        type RecWtf = Wtf<RecordingRenderer<NullColor>, (), (), ()>;

        /// The scroll window: short enough that the content overflows it, and
        /// far enough from the bottom of the page that the overflow lands
        /// **inside the viewport**. That second half is what makes this test
        /// meaningful — content below the viewport was already culled against
        /// the framebuffer bounds by WS6.4b, so a scrollable that fills the page
        /// cannot tell you whether its own clip works.
        const WINDOW_H: u32 = 20;

        fn scrollable_page() -> (TestPage<RecWtf>, RecordingRenderer<NullColor>)
        {
            scrollable_page_watching(create_signal(false))
        }

        /// As above, but the **third** checkbox — the one below the scroll
        /// window, i.e. the clipped-away one — is bound to `watched`.
        fn scrollable_page_watching(
            watched: Signal<bool>,
        ) -> (TestPage<RecWtf>, RecordingRenderer<NullColor>) {
            let renderer =
                RecordingRenderer::<NullColor>::new(Size::new_equal(64));
            let recorder = renderer.clone();
            let arena = create_signal(ElArena::new()).name("Page arena");
            let scope = new_scope();
            let mut page = TestPage::new(
                Page::new(
                    (),
                    Scrollable::vertical(
                        Flex::col(
                            (0..4)
                                .map(|i| {
                                    if i == 2 {
                                        Checkbox::new(watched).into_el()
                                    } else {
                                        Checkbox::new(false).into_el()
                                    }
                                })
                                .collect::<Vec<_>>(),
                        )
                        .gap(2u32),
                    )
                    .height(WINDOW_H),
                    arena,
                    Size::new_equal(64),
                    ().inert(),
                    DevTools::default().signal(),
                    Rc::new(FontCtx::new()),
                    scope,
                ),
                renderer,
            );
            for _ in 0..4 {
                page.use_renderer(|_| {});
            }
            (page, recorder)
        }

        /// WS6.4c: **hit-testing respects the clip, like painting does.**
        ///
        /// Before clipping existed, paint and hit-testing agreed because both
        /// used `layout.outer`. Once `render_part` started honouring clips they
        /// diverged, and the divergence is the bad direction: a scrolled-away
        /// button is invisible and still clickable. The pointer must not be able
        /// to reach what the renderer refuses to draw.
        #[test]
        fn a_clipped_away_widget_does_not_claim_hover() {
            with_new_runtime(|_| {
                let (mut page, _recorder) = scrollable_page();

                // Inside the third checkbox's layout rect, below the 20 px
                // scroll window — i.e. exactly the pixels the clip removes.
                let over_clipped = Point::new(8, 40);
                let _ = page.handle_events(core::iter::once(Event::Mouse(
                    MouseEvent::MouseMove(over_clipped),
                )));

                assert_eq!(
                    page.state.pointer.hovered, None,
                    "a widget clipped out of view claimed hover — the pointer \
                     must not reach what the renderer refuses to draw"
                );
                // NOTE: this one is enforced by the traversal PRUNE (the
                // scrollable's own rect misses the cursor, so the subtree is
                // never walked), not by `hit_bounds`. It is the end-to-end
                // behaviour; `a_clipped_away_widget_cannot_be_clicked` is what
                // pins the hit rect itself.
            });
        }

        /// The test that isolates [`EventCtx::hit_bounds`] from the traversal
        /// prune.
        ///
        /// A `MouseMove` over clipped-away content never arrives, because the
        /// prune stops at the scrollable whose own rect misses the cursor — so
        /// the hover test above passes even with `hit_bounds` clip-blind, and
        /// proves the end-to-end behaviour rather than the mechanism. A **click**
        /// is deliberately not pruned (`ButtonUp` must reach a pressed widget
        /// wherever the cursor went), so it walks all the way to the clipped
        /// checkbox and only `hit_bounds` can stop it.
        ///
        /// This is also the bug a user would actually hit: toggling a checkbox
        /// they cannot see.
        #[test]
        fn a_clipped_away_widget_cannot_be_clicked() {
            with_new_runtime(|_| {
                let watched = create_signal(false);
                let (mut page, _recorder) = scrollable_page_watching(watched);

                // Inside the third checkbox's rect, below the 20 px window.
                let over_clipped = Point::new(8, 40);
                let _ = page.handle_events(
                    [
                        Event::Mouse(MouseEvent::ButtonDown(
                            MouseButton::Left,
                            Some(over_clipped),
                        )),
                        Event::Mouse(MouseEvent::ButtonUp(
                            MouseButton::Left,
                            Some(over_clipped),
                        )),
                    ]
                    .into_iter(),
                );

                assert_eq!(
                    page.state.pointer.pressed, None,
                    "a clipped-away widget accepted a press"
                );
                assert!(
                    !watched.get(),
                    "a checkbox the renderer refuses to draw was toggled by a \
                     click — hit-testing must honour the clip, or the user \
                     operates controls they cannot see"
                );
            });
        }

        #[test]
        fn a_clipping_parent_confines_its_children() {
            with_new_runtime(|_| {
                let (mut page, recorder) = scrollable_page();

                recorder.clear();
                page.force_redraw();
                page.use_renderer(|_| {});

                let escaped: Vec<Rect> = recorder
                    .ops()
                    .iter()
                    .filter_map(DrawOp::bounds)
                    .filter(|b| b.top_left.y >= WINDOW_H as i32)
                    .collect();

                assert!(
                    !recorder.ops().is_empty(),
                    "the visible part of the scrollable must still draw"
                );
                assert!(
                    escaped.is_empty(),
                    "content below the scroll window escaped its parent and \
                     drew into the page: {escaped:?}. `CLIPS_CHILDREN` is what \
                     confines it — and what lets the traversal prune skip it \
                     without per-node storage."
                );
            });
        }
    }

    mod culling {
        use super::*;
        use crate::{
            render::record::{DrawOp, RecordingRenderer},
            widget::checkbox::Checkbox,
        };
        use rsact_reactive::runtime::with_new_runtime;

        pub(super) type RecWtf = Wtf<RecordingRenderer<NullColor>, (), (), ()>;

        /// Two checkboxes stacked in a 64x64 page: the first at y≈0, the second
        /// below `CLIP_H`, so a clip of `0,0 64xCLIP_H` culls exactly the second.
        pub(super) const CLIP_H: u32 = 18;

        pub(super) fn two_checkbox_page(
            second: Signal<bool>,
        ) -> (TestPage<RecWtf>, RecordingRenderer<NullColor>) {
            let renderer =
                RecordingRenderer::<NullColor>::new(Size::new_equal(64));
            let recorder = renderer.clone();
            let arena = create_signal(ElArena::new()).name("Page arena");
            let scope = new_scope();
            let mut page = TestPage::new(
                Page::new(
                    (),
                    Flex::col(vec![
                        Checkbox::new(false).into_el(),
                        Checkbox::new(second).into_el(),
                    ])
                    .gap(4u32),
                    arena,
                    Size::new_equal(64),
                    ().inert(),
                    DevTools::default().signal(),
                    Rc::new(FontCtx::new()),
                    scope,
                ),
                renderer,
            );
            // Settle, so every part's probe exists before anything is culled.
            for _ in 0..4 {
                page.use_renderer(|_| {});
            }
            (page, recorder)
        }

        /// One forced frame under `clip`, returning its ops.
        fn frame(
            page: &mut TestPage<RecWtf>,
            recorder: &RecordingRenderer<NullColor>,
            clip: Option<Rect>,
        ) -> Vec<DrawOp> {
            recorder.clear();
            page.force_redraw();
            if let Some(clip) = clip {
                page.renderer.push_clip(clip);
            }
            page.use_renderer(|_| {});
            if clip.is_some() {
                page.renderer.pop_clip();
            }
            recorder.ops()
        }

        /// Ops that draw at or below `y` — i.e. outside the clip used here.
        fn below(ops: &[DrawOp], y: i32) -> usize {
            ops.iter()
                .filter_map(|op| op.bounds())
                .filter(|b| b.top_left.y >= y)
                .count()
        }

        #[test]
        fn a_part_outside_the_clip_is_not_drawn() {
            with_new_runtime(|_| {
                let second = create_signal(false);
                let (mut page, recorder) = two_checkbox_page(second);

                let whole = frame(&mut page, &recorder, None);
                assert!(
                    below(&whole, CLIP_H as i32) > 0,
                    "the second checkbox must be below the clip, or this test \
                     proves nothing"
                );

                let clipped = frame(
                    &mut page,
                    &recorder,
                    Some(Rect::new(Point::zero(), Size::new(64, CLIP_H))),
                );
                assert_eq!(
                    below(&clipped, CLIP_H as i32),
                    0,
                    "a part whose area cannot reach the clip must not draw"
                );
                assert!(
                    !clipped.is_empty(),
                    "the part INSIDE the clip must still draw"
                );
            });
        }

        /// The two reactive properties of a culled part, in one sequence:
        ///
        /// 1. it does not keep the page awake — the walk stops reading its probe,
        ///    so the page probe's `clear_sources` drops the edge and the next
        ///    frame is idle **even though the culled part is dirty**;
        /// 2. it is skipped, not resolved — when it comes back into view it
        ///    repaints, with the state it acquired while invisible.
        ///
        /// Together these are why "cull the paint, leave the probe dirty" is
        /// sound (WS6.4c's "an unpainted probe stays dirty"). Get (1) wrong and
        /// every frame re-walks the tree forever; get (2) wrong and scrolled-away
        /// content comes back stale.
        #[test]
        fn a_culled_part_stays_dirty_without_waking_the_page() {
            with_new_runtime(|_| {
                let mut second = create_signal(false);
                let (mut page, recorder) = two_checkbox_page(second);

                let clip = Rect::new(Point::zero(), Size::new(64, CLIP_H));
                let _ = frame(&mut page, &recorder, Some(clip));

                // The culled part's own state changes while it is invisible.
                page.take_draw_calls();
                second.set(true);

                // (1) An ORDINARY frame: nothing to do. The dirty-but-culled part
                // must not have kept the page's render gate armed.
                recorder.clear();
                page.renderer.push_clip(clip);
                page.use_renderer(|_| {});
                page.renderer.pop_clip();
                assert_eq!(
                    page.take_draw_calls(),
                    0,
                    "a dirty culled part kept the page re-rendering"
                );
                // Drawing ops only: `push_clip` above logs a `Clip`, which is
                // bookkeeping and paints nothing (same rule `schedule` uses).
                assert_eq!(
                    recorder
                        .ops()
                        .iter()
                        .filter(|op| op.bounds().is_some())
                        .count(),
                    0,
                    "an idle frame must paint nothing"
                );

                // (2) Back in view: it repaints, and shows the state it picked up
                // while culled — a checked box draws its icon `Path`, which the
                // unchecked frame does not have (same signal the WS6.9 goldens
                // use to tell the two states apart).
                let back = frame(&mut page, &recorder, None);
                assert!(
                    below(&back, CLIP_H as i32) > 0,
                    "the part must repaint once it is visible again"
                );
                assert!(
                    back.iter().any(|op| matches!(op, DrawOp::Path { .. })),
                    "it must repaint with the state it acquired while culled"
                );
            });
        }
    }

    // WS6.2: a paint-only change (checkbox toggle — same size, no relayout)
    // repaints only that widget, so the page's damage set is exactly its rect,
    // NOT the whole viewport. This is the "one change flushes only its region"
    // win: `render` passes this set to `finish_frame_regions`. An idle frame
    // (nothing changed) damages nothing.
    #[test]
    fn paint_change_damages_only_the_widget_not_the_viewport() {
        use crate::event::{Event, PressEvent};
        use crate::widget::checkbox::Checkbox;
        use rsact_reactive::runtime::with_new_runtime;

        with_new_runtime(|_| {
            let viewport = Size::new_equal(64);
            let mut page =
                create_null_page_sized(viewport, Checkbox::new(false));

            // Settle the initial render(s).
            for _ in 0..4 {
                page.use_renderer(|_| {});
            }

            // Idle frame: the probe skips, nothing repaints → no damage.
            page.use_renderer(|_| {});
            assert!(
                page.damage.borrow().is_empty(),
                "an idle frame must produce no damage, got {:?}",
                page.damage.borrow()
            );

            // Toggle the checkbox: focus it, then press+release.
            page.state.focused = Some((page.root, 0));
            let _ = page.handle_events(
                [
                    Event::Press(PressEvent::Press),
                    Event::Press(PressEvent::Release),
                ]
                .into_iter(),
            );

            // The pass that repaints the toggled checkbox.
            page.use_renderer(|_| {});

            let damage = page.damage.borrow();
            assert_eq!(
                damage.len(),
                1,
                "a single paint change is one damage rect, got {damage:?}"
            );
            // The checkbox is 16x16 at the origin (see the render goldens) — a
            // strict subset of the 64x64 viewport, so the flush touches 1/16th
            // of the screen instead of all of it.
            assert_eq!(
                damage[0],
                Rect::new(Point::zero(), Size::new_equal(16)),
                "damage must be exactly the checkbox rect"
            );
            assert!(
                damage[0].size.width < viewport.width
                    && damage[0].size.height < viewport.height,
                "damage {:?} must be strictly smaller than the {viewport:?} \
                 viewport",
                damage[0]
            );
        });
    }

    /// WS6.4.0(iv): `clear` repaints the whole viewport, so it declares the whole
    /// viewport.
    ///
    /// Nothing in production depends on this *today* — `clear` is only called
    /// from `UI::on_page_change`, on a freshly built page whose first-build
    /// blanket already sets `full_flush`. It is here because damage-driven
    /// flushing only composes if whoever writes a pixel records it, and a `clear`
    /// that painted the viewport while declaring nothing would be a trap for the
    /// next caller.
    #[test]
    fn clear_declares_a_full_viewport_flush() {
        use crate::widget::checkbox::Checkbox;
        use rsact_reactive::runtime::with_new_runtime;

        with_new_runtime(|_| {
            let viewport = Size::new_equal(64);
            let full = Rect::new(Point::zero(), viewport);
            let mut page =
                create_null_page_sized(viewport, Checkbox::new(false));

            // Drain the first-build blanket so the assertion below is about
            // `clear` and nothing else.
            for _ in 0..6 {
                page.use_renderer(|_| {});
            }
            assert_ne!(
                page.damage.borrow().clone(),
                vec![full],
                "page must settle to something other than a full flush first"
            );

            page.clear();
            page.use_renderer(|_| {});
            assert_eq!(
                page.damage.borrow().clone(),
                vec![full],
                "clear repainted the viewport, so it must flush the viewport"
            );
        });
    }

    // WS6.2 escape hatch: a full invalidate (first render / `force_redraw` /
    // layout change) flushes the WHOLE viewport, not just the redraw-root rects
    // — the page background outside the root widget is never painted per-frame,
    // so an incremental flush would leave it stale on a fresh screen.
    #[test]
    fn force_redraw_damages_the_whole_viewport() {
        use crate::widget::checkbox::Checkbox;
        use rsact_reactive::runtime::with_new_runtime;

        with_new_runtime(|_| {
            let viewport = Size::new_equal(64);
            let full = Rect::new(Point::zero(), viewport);
            let mut page =
                create_null_page_sized(viewport, Checkbox::new(false));

            // The very first render is a full invalidate (the layout memo sets
            // force_redraw on the first build).
            page.use_renderer(|_| {});
            {
                let d = page.damage.borrow();
                assert_eq!(d.len(), 1);
                assert_eq!(d[0], full, "first render must flush the viewport");
            }

            // Settle (force_redraw clears), then request an explicit full redraw.
            for _ in 0..4 {
                page.use_renderer(|_| {});
            }
            page.force_redraw();
            page.use_renderer(|_| {});
            {
                let d = page.damage.borrow();
                assert_eq!(d.len(), 1);
                assert_eq!(d[0], full, "force_redraw must flush the viewport");
            }
        });
    }

    /// WS6.1 (end-to-end): a TARGETED (incremental) layout change repaints only
    /// the nearest size-stable ancestor, so the damage is that ancestor's rect —
    /// NOT the whole viewport. This proves the blanket `force_redraw`/`full_flush`
    /// is skipped for incremental relayouts (a child resizes inside a fixed
    /// parent; the parent is the repaint root). Only meaningful under the feature
    /// (the default build has no targeted path — every layout change is blanket).
    #[cfg(feature = "incremental-layout")]
    #[test]
    fn incremental_layout_change_damages_only_the_stable_parent() {
        use crate::widget::{SizedWidget, flex::Flex};
        use rsact_reactive::runtime::with_new_runtime;

        with_new_runtime(|_| {
            let viewport = Size::new_equal(64);
            let mut height = create_signal(10u32);
            // A FIXED 40x40 parent (stays stable when the child grows) holding a
            // child whose height is a reactive signal.
            let child = Flex::<NullWtf>::col(["x"]).width(10u32).height(height);
            let root = Flex::col([child]).width(40u32).height(40u32);
            let mut page = create_null_page_sized(viewport, root);

            // Settle (the first frame is a full invalidate).
            for _ in 0..4 {
                page.use_renderer(|_| {});
            }

            // Grow the child: an incremental relayout (child resizes inside the
            // fixed parent → the parent is the stable repaint root).
            height.set(20);
            page.use_renderer(|_| {});

            let damage = page.damage.borrow();
            assert_eq!(
                damage.len(),
                1,
                "one repaint root (the stable 40x40 parent), got {damage:?}"
            );
            // The 40x40 fixed parent — NOT the 64x64 viewport.
            assert_eq!(
                damage[0].size,
                Size::new_equal(40),
                "damage {:?} must be the stable parent, not the viewport",
                damage[0]
            );
            assert!(
                damage[0].size.width < viewport.width,
                "targeted damage must be strictly smaller than the viewport"
            );
        });
    }

    // The `View` migration: `row!`/`col!` and `impl View<W>` APIs accept bare
    // widgets *and* leaf values (`&str`, `String`, `Option<View>`, existing
    // `El`) uniformly, without an explicit `.el()`. A bare `Button` in `row!`
    // did not compile under the old `Into<El>` path (no `From<Button> for El`);
    // it works now because every widget gets its own concrete `View` impl
    // (a one-liner next to its `Widget` impl), which coexists with the leaf
    // impls.
    #[test]
    fn view_accepts_bare_widgets_and_leaves() {
        fn build(v: impl View<NullWtf>) -> El<NullWtf> {
            v.into_el()
        }

        let _: El<NullWtf> = build(Flex::row((
            Button::new("bare button"), // bare widget, no `.el()`
            "string literal",           // &str leaf
            String::from("owned string"), // String leaf
            Container::new("nested str"), // container takes `impl View`
            Label::new("explicit").into_el(), // existing El still fine
            Some(Button::new("optional")), // Option<View>
        )));

        let _: El<NullWtf> = build(Flex::col((Button::new("a"), "b")));

        // And it composes as a real page root (`PageInitFn` is `View`-based).
        let _ = create_null_page(Flex::row((Button::new("root"), "title")));
    }

    // ViewSequence: Flex accepts a heterogeneous tuple of widget types, a
    // homogeneous widget array, a Vec of views, and the row!/col! macros,
    // auto-erasing each element via `View::into_el` (no manual `.el()`), with
    // static children stored inert (`MaybeSignal::new_inert`).
    #[test]
    fn flex_view_sequence() {
        // Heterogeneous tuple of different widget types + a `&str` leaf:
        let _ = create_null_page(Flex::row((
            Button::new("a"),
            "b",
            Container::new("c"),
        )));
        // Homogeneous array of widgets:
        let _ =
            create_null_page(Flex::col([Button::new("x"), Button::new("y")]));
        // Vec of views (here `&str`):
        let _ = create_null_page(Flex::row(alloc::vec!["a", "b", "c"]));
        // Macros still accept bare widgets + leaves:
        let _ = create_null_page(Flex::col((
            Container::new("x"),
            "y",
            Button::new("z"),
        )));
    }

    // WS13.2: Button is the first real builder/widget split — `ButtonBuilder`
    // carries the build-only `content` child, the retained `Button` drops it.
    #[test]
    fn button_split_drops_content_husk() {
        use crate::widget::button::{Button, ButtonBuilder};
        // The retained widget must not carry the build-only child husk, so it is
        // strictly smaller than its builder.
        assert!(
            core::mem::size_of::<Button<NullWtf>>()
                < core::mem::size_of::<ButtonBuilder<NullWtf>>(),
            "retained Button must be smaller than ButtonBuilder (dropped content husk)"
        );
        // And a button page still builds end-to-end (transform ran).
        let _ = create_null_page(Button::new("ok"));
    }

    // WS13.2 (Task 4): `#[derive(Builder)]` emits `View` (+ `SingleViewMarker` +
    // `Build`) for `ButtonBuilder` — this compile-drives that the derive path is
    // actually taken (the hand-written `impl View`/`Build` blocks are gone).
    #[test]
    fn derived_button_builder_is_a_view() {
        fn assert_view<W: crate::el::WidgetCtx, V: crate::el::View<W>>(_: &V) {}
        let b = crate::widget::button::Button::<NullWtf>::new("x");
        assert_view(&b);
    }

    // WS13.2: Flex is de-genericized (`Flex<W, Dir>` -> `Flex<W>` with a
    // runtime `axis: Axis` field) and split — `FlexBuilder` carries the
    // build-only `children` vec, the retained `Flex` drops it.
    #[test]
    fn flex_split_drops_children_and_phantom() {
        use crate::widget::flex::{Flex, FlexBuilder};
        // Retained Flex holds only its layout handle — no children Vec, no PhantomData.
        assert!(
            core::mem::size_of::<Flex<NullWtf>>()
                < core::mem::size_of::<FlexBuilder<NullWtf>>(),
            "retained Flex must be smaller than FlexBuilder (dropped children)"
        );
        // De-generic: `Flex<W>` takes no Dir param.
        let _ = create_null_page(Flex::<NullWtf>::row(("a", "b")));
        let _ = create_null_page(Flex::<NullWtf>::col([
            Button::new("x"),
            Button::new("y"),
        ]));
    }

    // WS13.4 (Task 5.1): `Show` is split — `ShowBuilder` carries the
    // build-only `el` child (and consumes `show: Memo<bool>` inline in
    // `Show::new`, storing neither on the builder), the retained `Show`
    // keeps only the `layout` handle it shares with `el` (plus the `ctx`
    // phantom, since `layout: Layout` alone doesn't use `W`).
    #[test]
    fn show_split_drops_el_husk() {
        use crate::widget::show::{Show, ShowBuilder};
        // Retained Show holds only its layout handle — no `el` child, no
        // `show: Memo<bool>`.
        assert!(
            core::mem::size_of::<Show<NullWtf>>()
                < core::mem::size_of::<ShowBuilder<NullWtf>>(),
            "retained Show must be smaller than ShowBuilder (dropped el child)"
        );
        let _ = create_null_page(Show::new(
            true.inert(),
            Button::new("ok").into_el(),
        ));
    }

    // WS13.4 (Task 5.2): `Label` is split, but unlike `Button`/`Flex`/`Show`
    // it has no build-only field to drop — `content`/`layout`/`style` are all
    // read by `render`, so `LabelBuilder` moves every field into the retained
    // `Label` unchanged (a `size_of` `<` assertion would be false, not true).
    // Per the row's fallback: lock that `LabelBuilder` exists as a real,
    // distinct type and that a page built from `Label::new(...).into_el()`
    // still builds and renders end-to-end through the derive-generated
    // `Build` path.
    #[test]
    fn label_split_builder_exists_and_page_renders() {
        use crate::widget::label::{Label, LabelBuilder};

        fn assert_is_label_builder<W: WidgetCtx>(_: &LabelBuilder<W>) {}
        let b = Label::<NullWtf>::new("x".inert());
        assert_is_label_builder(&b);

        let mut page = create_null_page(Label::new("hello".inert()).into_el());
        // Label's render falls back to the theme's default foreground on an
        // unset text color (no panic) — see the render body's Note: — so this
        // renders cleanly through the null theme, unlike a bare `Container`.
        page.use_renderer(|_| {});
    }

    // WS13.4 (Task 5.3): `Space` is split, but like `Label` it has no
    // build-only field to drop — `layout`/`ctx` are the same two fields on
    // both sides (`Dir: Direction` was de-genericized away per the 7.2 slice,
    // flex precedent: it was a compile-time-only tag selecting `Axis` in
    // `new()`, never read at runtime) — a `size_of` `<` assertion would be
    // false, not true, so this mirrors the label fallback shape test.
    #[test]
    fn space_split_builder_exists_and_page_renders() {
        use crate::widget::space::{Space, SpaceBuilder};

        fn assert_is_space_builder<W: WidgetCtx>(_: &SpaceBuilder<W>) {}
        let b = Space::<NullWtf>::row(10);
        assert_is_space_builder(&b);

        // Space renders nothing (a no-op `render`), so building the page
        // through the derive-generated `Build` path is the meaningful check.
        let _ = create_null_page(Space::col(10));
    }

    // WS13.4 (Task 5.4): `Edge` is split, but like `Label`/`Space` it has no
    // build-only field to drop — `layout`/`style` are both read by
    // `render`/`layout`, so `EdgeBuilder` moves both fields into the
    // retained `Edge` unchanged (a `size_of` `<` assertion would be false,
    // not true).
    #[test]
    fn edge_split_builder_exists_and_page_builds() {
        use crate::widget::edge::{Edge, EdgeBuilder};

        fn assert_is_edge_builder<W: WidgetCtx>(_: &EdgeBuilder<W>) {}
        let b = Edge::<NullWtf>::new();
        assert_is_edge_builder(&b);

        // Not rendering: `Edge`'s `container` style panics on the null
        // theme's unset background/border `ColorStyle`, the same
        // pre-existing limitation documented on `Container` elsewhere in
        // this file (e.g. `arena_rebuild_does_not_leak_subtree`'s "unrelated
        // pre-existing ColorStyle panic" note) — unrelated to this split, so
        // building (not rendering) the page is the meaningful check here.
        let _ = create_null_page(Edge::new());
    }

    // WS13.4 (Task 5.5): `Bar` is split, but like `Label`/`Space`/`Edge` it
    // has no build-only field to drop — `value`/`layout`/`style`/`axis` are
    // all read by `render`, so `BarBuilder` moves all four fields into the
    // retained `Bar` unchanged (a `size_of` `<` assertion would be false, not
    // true). `Dir: Direction` was de-genericized into a runtime `axis: Axis`
    // field (unlike `Space`, `render` reads it too, not just `new()` — see
    // bar.rs's WS13.4 comment); `V: RangeValue` is deliberately left generic
    // (deferred to 5.8, see the same comment).
    #[test]
    fn bar_split_builder_exists_and_page_builds() {
        use crate::{
            value::{RangeU8, RangeValue},
            widget::bar::{Bar, BarBuilder},
        };

        fn assert_is_bar_builder<W: WidgetCtx, V: RangeValue>(
            _: &BarBuilder<W, V>,
        ) {
        }
        let b = Bar::<NullWtf, RangeU8>::horizontal(
            RangeU8::new_full_range(0).inert(),
        );
        assert_is_bar_builder(&b);

        // Not rendering: `Bar`'s `container` style panics on the null
        // theme's unset background/border `ColorStyle`, the same
        // pre-existing limitation documented on `Edge`/`Container` elsewhere
        // in this file — unrelated to this split, so building (not
        // rendering) the page is the meaningful check here.
        let _ = create_null_page(Bar::<NullWtf, RangeU8>::vertical(
            RangeU8::new_full_range(0).inert(),
        ));
    }

    // WS13.4 (Task 5.6): `Checkbox` is split, but like `Label`/`Space`/
    // `Edge`/`Bar` it has no build-only field to drop — `layout`/`value`/
    // `style` are all read by `render`/`on_event`, so `CheckboxBuilder`
    // moves all three fields into the retained `Checkbox` unchanged (a
    // `size_of` `<` assertion would be false, not true). `value:
    // Signal<bool>` is the widget's JOB (WS4.5 audit: the checked state IS
    // what Checkbox is), so it stays a retained field rather than becoming
    // build-only — see checkbox.rs's WS13.4 comment.
    #[test]
    fn checkbox_split_builder_exists_and_page_renders() {
        use crate::widget::checkbox::{Checkbox, CheckboxBuilder};

        fn assert_is_checkbox_builder<W: WidgetCtx>(_: &CheckboxBuilder<W>) {}
        let b = Checkbox::<NullWtf>::new(false);
        assert_is_checkbox_builder(&b);

        // Unlike Edge/Bar/Container, Checkbox renders cleanly through the
        // null theme (exercised extensively by the toggle/click tests
        // above), so render (not just build) the page for the meaningful
        // check, mirroring Label's version of this test.
        let mut page = create_null_page(Checkbox::<NullWtf>::new(false));
        page.use_renderer(|_| {});
    }

    // WS13.4 (Task 5.7): `Container` is split like `Button` (single child) —
    // `content: El<W>` is build-only (consumed by `ctx.set_single_child` in
    // `Build::build`, never read again), so `ContainerBuilder` carries it as
    // `#[child(single)]` and the retained `Container` drops it; `layout`/
    // `style` stay retained `#[widget]` fields (both read by `render`).
    #[test]
    fn container_split_drops_content_husk() {
        use crate::widget::container::{Container, ContainerBuilder};
        // The retained widget must not carry the build-only child husk, so it
        // is strictly smaller than its builder.
        assert!(
            core::mem::size_of::<Container<NullWtf>>()
                < core::mem::size_of::<ContainerBuilder<NullWtf>>(),
            "retained Container must be smaller than ContainerBuilder \
             (dropped content husk)"
        );

        // Not rendering: `Container`'s `container` style panics on the null
        // theme's unset background/border `ColorStyle` (the same
        // pre-existing limitation noted on `Edge`/`Bar` elsewhere in this
        // file), so building (not rendering) the page is the meaningful
        // check here.
        let _ = create_null_page(Container::new("ok"));
    }

    // WS13.4 (Task 5.8): `Slider` is split, but like `Label`/`Space`/`Edge`/
    // `Bar`/`Checkbox` it has no build-only field to drop — `value`/`range`/
    // `step`/`state`/`layout`/`style`/`axis` are all read by `render`/
    // `on_event`, so `SliderBuilder` moves all seven fields into the
    // retained `Slider` unchanged (a `size_of` `<` assertion would be false,
    // not true). Unlike `Bar`, `Slider` never had a `V: RangeValue` generic
    // to begin with (its value/range/step are already concrete `f32`), so
    // there is no `V` decision to make here — only `Dir: Direction` was
    // de-genericized, into a runtime `axis: Axis` field (Bar precedent:
    // `render` reads it repeatedly, not just `new()`'s size computation) —
    // see slider.rs's WS13.4 comment.
    #[test]
    fn slider_split_builder_exists_and_page_renders() {
        use crate::widget::slider::{Slider, SliderBuilder};

        fn assert_is_slider_builder<W: WidgetCtx>(_: &SliderBuilder<W>) {}
        let b = Slider::<NullWtf>::horizontal(0.5, (0.0..=1.0).inert());
        assert_is_slider_builder(&b);

        // Unlike Edge/Bar/Container, Slider renders cleanly through the null
        // theme (its track/thumb draw styles resolve `ColorStyle::get()`,
        // never `.expect()`), so render (not just build) the page for the
        // meaningful check, mirroring Checkbox's/Label's version of this
        // test.
        let mut page = create_null_page(Slider::<NullWtf>::horizontal(
            0.5,
            (0.0..=1.0).inert(),
        ));
        page.use_renderer(|_| {});
    }

    // WS13.4 (Task 5.9): `Knob` is split like `Slider` — like `Label`/
    // `Space`/`Edge`/`Bar`/`Checkbox`/`Slider` it has no build-only field to
    // drop — `layout`/`value`/`state`/`style` are all read by `render`/
    // `on_event`, so `KnobBuilder` moves all four fields into the retained
    // `Knob` unchanged (a `size_of` `<` assertion would be false, not true).
    // Unlike `Slider`, `Knob<W, V: RangeValue>` DOES carry a genuine `V`
    // generic; applying Bar's decision rule to the same evidence Bar found
    // (only live `RangeValue` impl is the const-generic `RangeU8` family;
    // other impls are commented-out/TODO), `V` is deliberately left generic
    // here too, NOT de-genericized — same call as Bar, deferred to the same
    // WS7 remainder. `Knob` has no `Dir`/`Axis` generic at all, so there is
    // no Dir-side decision on this widget — see knob.rs's WS13.4 comment.
    #[test]
    fn knob_split_builder_exists_and_page_renders() {
        use crate::{
            value::{RangeU8, RangeValue},
            widget::knob::{Knob, KnobBuilder},
        };

        fn assert_is_knob_builder<W: WidgetCtx, V: RangeValue>(
            _: &KnobBuilder<W, V>,
        ) {
        }
        let b = Knob::<NullWtf, RangeU8>::new(create_signal(
            RangeU8::new_full_range(0),
        ));
        assert_is_knob_builder(&b);

        // Unlike Edge/Bar/Container, Knob renders cleanly through the null
        // theme (its sector draw style resolves `ColorStyle::get()`, never
        // `.expect()`), so render (not just build) the page for the
        // meaningful check.
        let mut page = create_null_page(Knob::<NullWtf, RangeU8>::new(
            create_signal(RangeU8::new_full_range(0)),
        ));
        page.use_renderer(|_| {});
    }

    // WS13.4 (Task 5.10): `Scrollable` is split like `Button`/`Container`
    // (single child) — `content: El<W>` is build-only (consumed by
    // `ctx.set_single_child` in `Build::build`, never read again), so
    // `ScrollableBuilder` carries it as `#[child(single)]` and the retained
    // `Scrollable` drops it; `state`/`style`/`layout`/`mode` stay retained
    // `#[widget]` fields (all read by `render`/`on_event`). `Dir: Direction`
    // is de-genericized like `Bar`/`Slider` into a runtime `axis: Axis`
    // field — both `render` and `on_event` read `Dir::AXIS` repeatedly (drag
    // projection, scrollbar geometry), not just `new()`'s one-shot
    // `Layout::scrollable` call — collapsing `Scrollable<W, RowDir>`/
    // `Scrollable<W, ColDir>` into a single `Scrollable<W>`. See
    // scrollable.rs's WS13.4 comment for how the compile-time
    // `SizedWidget<RowDir>::width`/`SizedWidget<ColDir>::height`
    // specialization becomes a single runtime `if self.axis == ...` branch.
    #[test]
    fn scrollable_split_drops_content_husk() {
        use crate::widget::scrollable::{Scrollable, ScrollableBuilder};
        // The retained widget must not carry the build-only child husk, so it
        // is strictly smaller than its builder.
        assert!(
            core::mem::size_of::<Scrollable<NullWtf>>()
                < core::mem::size_of::<ScrollableBuilder<NullWtf>>(),
            "retained Scrollable must be smaller than ScrollableBuilder \
             (dropped content husk)"
        );

        // De-generic: `Scrollable<W>` takes no Dir param; horizontal/vertical
        // both produce the same `ScrollableBuilder<W>`. Not rendering: like
        // Edge/Bar/Container, Scrollable's `container` style panics on the
        // null theme's unset background/border `ColorStyle`, so building
        // (not rendering) the page is the meaningful check here.
        let _ =
            create_null_page(Scrollable::<NullWtf>::vertical(Button::new("x")));
        let _ = create_null_page(Scrollable::<NullWtf>::horizontal(
            Button::new("y"),
        ));
    }

    // WS13.4 (Task 5.11): `Canvas` is split, but like `Label`/`Space`/
    // `Edge`/`Bar`/`Checkbox`/`Slider`/`Knob` it has no build-only field to
    // drop — `draw`/`layout` are both read by `render`/`layout`, so
    // `CanvasBuilder` moves both fields into the retained `Canvas` unchanged
    // (a `size_of` `<` assertion would be false, not true). The split is
    // purely mechanical: the WS1b.1 immediate-mode draw closure is moved by
    // name like any other `#[widget]` field, never invoked or inspected
    // during the build-time move, so the DrawQueue/draw-command semantics
    // documented in canvas.rs are untouched.
    #[test]
    fn canvas_split_builder_exists_and_page_renders() {
        use crate::widget::canvas::{Canvas, CanvasBuilder};

        fn assert_is_canvas_builder<W: WidgetCtx>(_: &CanvasBuilder<W>) {}
        let b = Canvas::<NullWtf>::new(|_renderer| Ok(()));
        assert_is_canvas_builder(&b);

        // Unlike Edge/Bar/Container, Canvas resolves no style at all (no
        // `declare_widget_style!`), so it renders cleanly through the null
        // theme; render (not just build) the page for the meaningful check.
        let mut page =
            create_null_page(Canvas::<NullWtf>::new(|_renderer| Ok(())));
        page.use_renderer(|_| {});
    }

    // WS13.4 (Task 5.13): `Select` is split, but like `Label`/`Space`/`Edge`/
    // `Bar`/`Checkbox`/`Slider`/`Knob`/`Canvas` it has no build-only field to
    // drop — `layout`/`state`/`style`/`options` are all read by
    // `render`/`on_event`, so `SelectBuilder` moves all four fields into the
    // retained `Select` unchanged (a `size_of` `<` assertion would be false,
    // not true). `Dir: Direction` was de-genericized like Bar/Slider/
    // Scrollable into a runtime `axis: Axis` field (`render` reads
    // `Dir::AXIS` for the selected-option highlight box, not just `new()`'s
    // size computation); `K: PartialEq` stays generic (no canonical key
    // type, same call as Bar's undecided `V`). This is a mechanical
    // rename-only split — the `options: Rc<MaybeReactive<Vec<SelectOption<W,
    // K>>>>` storage and the `selected.setter(...)` reactive-effect wiring
    // (both backlogged as awkward under A18) are untouched — see
    // select.rs's WS13.4 comment.
    #[test]
    fn select_split_builder_exists_and_page_builds() {
        use crate::widget::select::{Select, SelectBuilder};

        fn assert_is_select_builder<W: WidgetCtx, K: PartialEq + 'static>(
            _: &SelectBuilder<W, K>,
        ) {
        }
        let selected = create_signal(1u32);
        let b = Select::<NullWtf, u32>::vertical(
            selected,
            alloc::vec![1u32, 2, 3].inert(),
        );
        assert_is_select_builder(&b);

        // Not rendering: like Edge/Bar/Container/Scrollable, Select's
        // `selected`/`container` styles panic on the null theme's unset
        // background/border `ColorStyle`, so building (not rendering) the
        // page is the meaningful check here.
        let selected = create_signal(1u32);
        let _ = create_null_page(Select::<NullWtf, u32>::horizontal(
            selected,
            alloc::vec![1u32, 2, 3].inert(),
        ));
    }

    // WS13.4 (Task 5.15): `Icon` is split after its WS4.5 repair (icon.rs's
    // own commit). Like `Label`/`Space`/`Edge`/…, its only build-only field
    // is a ZST (`is_reactive: PhantomData<R>` — the `ReactivityMarker`
    // compile-time constructor tag: it only ever selected which builder
    // methods are available, `render` dispatches on the `IconValue` enum
    // itself, never on `R`), so a `size_of` `<` assertion would be false,
    // not true — the 7.2 slice drops `R` from the retained `Icon<W, I>`
    // (flex.rs `Dir`->`Axis`/space.rs `Dir`-drop precedent) rather than
    // shrinking it.
    #[cfg(feature = "tiny-icons")]
    #[test]
    fn icon_split_builder_exists_and_page_renders() {
        use crate::widget::icon::{Icon, IconBuilder};
        use rsact_tiny_icons::{EmptyIconSet, IconRaw};

        fn assert_is_icon_builder<W: WidgetCtx, R: ReactivityMarker>(
            _: &IconBuilder<W, EmptyIconSet, R>,
        ) {
        }
        let b = Icon::<NullWtf, EmptyIconSet>::new(EmptyIconSet.inert());
        assert_is_icon_builder(&b);

        // Not rendering through `new`: the reactive constructor's
        // `IconValue::Relative` render path calls `IconSet::size`, which
        // `EmptyIconSet` intentionally panics on ("Cannot use empty icon
        // set") — there is no other zero-dependency `IconSet` to exercise
        // here (the generated sets need SVGs this environment lacks, per
        // WS9a.2). `inert` builds the `Fixed` variant instead, which
        // `render` never calls `IconSet::size` on, so it is both a real
        // build+render check (the derive-generated `Build` path ran, the
        // retained `Icon` renders through the null theme) and panic-free.
        const ICON_DATA: &[u8] = &[0u8; 1];
        let mut page = create_null_page(
            Icon::<NullWtf, EmptyIconSet>::inert(IconRaw::new(ICON_DATA, 1))
                .into_el(),
        );
        page.use_renderer(|_| {});
    }

    // WS13.2 (Task 5): locks the exact `size_of` byte counts behind the
    // `<` assertions above (`button_split_drops_content_husk`,
    // `flex_split_drops_children_and_phantom`) — the concrete numbers fed to
    // the 13.3 measurement gate (struct-size axis). Measured on
    // `x86_64`/`aarch64` host (NullWtf: `Wtf<NullRenderer, (), (), ()>`); a
    // toolchain/target change that shifts padding is expected to move these,
    // in which case re-lock in the same commit rather than loosening to `<`.
    #[test]
    fn split_widget_sizes_recorded() {
        use crate::widget::{
            button::{Button, ButtonBuilder},
            flex::{Flex, FlexBuilder},
        };
        // WS5.1: these grew sharply from the pre-off-graph lock (Button 48,
        // ButtonBuilder 152, Flex 12, FlexBuilder 40). The retained widget and
        // its builder now embed an owned `LayoutData`/`LayoutBuilder` inline
        // instead of an 8-byte `Layout` (`ValueId`) handle into the reactive
        // graph — that is the off-graph trade.
        //
        // **WS5.5 paid that trade back, and then some.** The follow-up this
        // comment predicted ("widgets whose `render` never reads `self.layout`
        // could drop the retained copy") turned out to apply to ALL of them
        // once the border moved to style: the only thing render ever read from
        // the snapshot was `block_model().border_width`. With the copy gone the
        // retained widget keeps just its own state:
        //
        //   Button   144 -> 32   (-112 B per instance)
        //   Flex     112 ->  0   (a ZST — Flex held NOTHING but the copy)
        //
        // Builders keep their `LayoutBuilder` until build hands it to the arena.
        // That asymmetry is the point — build-time cost, no retained cost.
        //
        // ISSUE-2 shaved 8 B off every builder (272 -> 264, 160 -> 152): the
        // `LayoutData.show` field was an `Option<Memo<bool>>` — a reactive
        // handle stored so the layout pass could *read* it — and is now a plain
        // `bool` written through the layout-prop channel. A rare win: the
        // correctness fix is also the smaller representation, because storing a
        // graph handle to re-read later is strictly more machinery than storing
        // the value someone already computed.
        assert_eq!(core::mem::size_of::<Button<NullWtf>>(), 32);
        assert_eq!(core::mem::size_of::<ButtonBuilder<NullWtf>>(), 264);
        assert_eq!(core::mem::size_of::<Flex<NullWtf>>(), 0);
        assert_eq!(core::mem::size_of::<FlexBuilder<NullWtf>>(), 152);
    }

    // Regression (WS5.1, off-graph): a reactive source set through the
    // trait-default setter (`SizedWidget::width` -> `LayoutBuilder::setter`)
    // must, once the element is built, drive an off-graph relayout. The
    // recorded binding writes the *arena-owned* `LayoutData` and fires the page
    // `relayout` trigger on every source change. (Pre-WS5.1 this asserted a
    // reactive `Layout` handle on the builder; that handle is gone, so the test
    // now builds into an arena and observes the arena + trigger directly — this
    // is the coverage that closes the WS5.1 A1 reactive-relayout gap.)
    #[test]
    fn reactive_width_setter_persists_and_reacts() {
        use crate::{
            el::build::BuildCtx, layout::length::Length, widget::edge::Edge,
        };

        with_new_runtime(|_| {
            let mut w = create_signal(Length::fill());
            // `.width(w)` records a reactive binding on the builder's off-graph
            // `LayoutBuilder`; `BuildCtx::run` drains it through `bind_layout`.
            let mut root: El<NullWtf> =
                Edge::<NullWtf>::new().width(w).into_el();
            let arena = create_signal(ElArena::new());
            let root_id = BuildCtx::run(&mut root, arena);

            // The binding wrote the signal's initial value into the arena.
            assert_eq!(
                arena.with_untracked(|a| a
                    .layout(root_id)
                    .unwrap()
                    .size
                    .width()),
                Length::fill(),
            );

            // WS6.4.0(iv): the relayout request IS the arena's dirty mark — the
            // `relayout` Trigger this used to observe is gone, because every site
            // that fired it marked the arena in the same breath. Drain the
            // build-time marks so the assertion below is about THIS write only.
            arena.clone().update_untracked(|a| {
                a.take_dirty();
            });
            assert!(
                !arena.with_untracked(|a| a.is_layout_dirty()),
                "dirty set must start drained"
            );

            w.set(Length::Fixed(50));

            // The binding re-ran: arena `LayoutData` updated AND the layout was
            // marked dirty — which is what `relayout_if_needed` polls.
            assert_eq!(
                arena.with_untracked(|a| a
                    .layout(root_id)
                    .unwrap()
                    .size
                    .width()),
                Length::Fixed(50),
            );
            assert!(
                arena.with_untracked(|a| a.is_layout_dirty()),
                "a reactive layout-prop write must mark the arena layout-dirty \
                 — that mark is the relayout request"
            );
        });
    }

    // The layout pass performs NO tracked reads — the invariant that makes
    // `relayout_if_needed`'s tripwire meaningful rather than merely quiet.
    //
    // `compute_layout` runs inside the layout probe's `poll`, so anything it
    // reads reactively becomes a probe source and can request a relayout behind
    // the dirty set's back. ISSUE-2 moved the last three per-widget reads off
    // that path, and dropping the reactive `fonts`/`viewport` removed the final
    // two — so the probe should now hold no sources at all and a settled page
    // should never relayout again, no matter how often it is asked.
    //
    // Asserted through behaviour rather than by counting sources, because the
    // runtime's profile is global and cannot attribute a source to one probe.
    // A page whose only content is INERT has nothing that could legitimately
    // dirty the probe, so any recompute after the first is the bug.
    //
    // The reactive half is not decoration: "asked N times, relayouted zero" is
    // satisfied just as well by a `relayout_if_needed` that can never return
    // true. Driving a real text change through it is what proves the instrument
    // reads before the quiet half is allowed to mean anything.
    #[test]
    fn a_settled_page_never_relayouts_again() {
        use alloc::string::ToString;

        with_new_runtime(|_| {
            let mut page = create_null_page_sized(
                Size::new_equal(64),
                Label::<NullWtf>::new("static".inert()).into_el(),
            );

            // Drain the build-time marks by taking the first relayout.
            page.relayout_if_needed();

            for i in 0..4 {
                assert!(
                    !page.relayout_if_needed(),
                    "settled page relayouted on ask #{i} — the layout pass has \
                     a tracked read again, so the layout probe has sources and \
                     the tripwire's silence no longer means anything"
                );
            }
        });

        with_new_runtime(|_| {
            let mut caption = create_signal("before".to_string());
            let mut page = create_null_page_sized(
                Size::new_equal(64),
                Label::<NullWtf>::new(caption).into_el(),
            );
            page.relayout_if_needed();
            assert!(!page.relayout_if_needed(), "must settle first");

            caption.set("after".to_string());
            assert!(
                page.relayout_if_needed(),
                "a marked text change must relayout — without this the quiet \
                 half above proves nothing"
            );
            assert!(
                !page.relayout_if_needed(),
                "…and then settle again immediately"
            );
        });
    }

    // ISSUE-2. The same guarantee, for the property that did not have it: a
    // `Label`'s TEXT. It used to live in `ContentLayout::Text` as a
    // `MaybeReactive<String>` that the layout pass read while measuring, so a
    // text write woke the page's layout probe and left the dirty set empty —
    // which is exactly the condition `compute_layout` treats as "relayout
    // everything" and `Page::relayout_if_needed`'s tripwire now rejects.
    #[test]
    fn reactive_text_persists_and_marks_dirty() {
        use crate::el::build::BuildCtx;
        use alloc::string::ToString;

        with_new_runtime(|_| {
            let mut caption = create_signal("before".to_string());
            let mut root: El<NullWtf> =
                Label::<NullWtf>::new(caption).into_el();
            let arena = create_signal(ElArena::new());
            let root_id = BuildCtx::run(&mut root, arena);

            let text = |a: &ElArena<NullWtf>| {
                a.layout(root_id).unwrap().text().map(|t| t.to_string())
            };

            // The binding wrote the signal's initial value into the arena.
            assert_eq!(
                arena.with_untracked(text),
                Some("before".to_string()),
                "the layout's text must be written at build, not read lazily"
            );

            arena.clone().update_untracked(|a| {
                a.take_dirty();
            });

            caption.set("after".to_string());

            assert_eq!(
                arena.with_untracked(text),
                Some("after".to_string()),
                "the binding must rewrite the arena-owned text"
            );
            assert!(
                arena.with_untracked(|a| a.is_layout_dirty()),
                "a text change must mark the arena layout-dirty — that mark is \
                 what lets WS5.2 relayout only this label and WS6.1 repaint only \
                 its box, instead of the whole viewport (ISSUE-2)"
            );
        });
    }

    // ISSUE-2's other half, and the reason the fix is not simply "make text a
    // plain value": an INERT label must stay free. `LayoutBuilder::setter`'s
    // Inert arm writes the string into the owned `LayoutData` at build and
    // records no binding, so a static label creates no effect — the cost lands
    // only on labels that can actually change.
    //
    // Both cases are measured, deliberately. "Creates nothing" is the kind of
    // assertion that passes for the wrong reason — a miscounted or always-zero
    // profile satisfies it just as well as a correct one — so the reactive label
    // next to it is what proves the instrument reads.
    #[test]
    fn an_inert_label_creates_no_binding_effect() {
        use crate::el::build::BuildCtx;
        use alloc::string::ToString;
        use rsact_reactive::runtime::current_runtime_profile;

        fn build_label(
            content: impl SignalMapRefMaybeReactive<str, String>,
        ) -> usize {
            let before = current_runtime_profile().effects;
            let mut root: El<NullWtf> =
                Label::<NullWtf>::new(content).into_el();
            let arena = create_signal(ElArena::new());
            let root_id = BuildCtx::run(&mut root, arena);

            assert!(
                arena.with_untracked(|a| a
                    .layout(root_id)
                    .unwrap()
                    .text()
                    .is_some_and(|t| !t.is_empty())),
                "the text must reach the layout either way"
            );

            current_runtime_profile().effects - before
        }

        with_new_runtime(|_| {
            assert_eq!(
                build_label("static".inert()),
                0,
                "an inert label must not create a layout binding effect"
            );
        });
        with_new_runtime(|_| {
            let caption = create_signal("dynamic".to_string());
            assert_eq!(
                build_label(caption),
                1,
                "a reactive label creates exactly one — if this reads 0 the \
                 count above proves nothing"
            );
        });
    }

    // Same guarantee for `FontSettingWidget::font_size` on a `Label` (whose
    // Text layout owns `FontProps`): the binding writes the arena-owned
    // `LayoutData`'s font props and fires the page relayout trigger.
    #[test]
    fn reactive_font_size_setter_persists_and_reacts() {
        use crate::{el::build::BuildCtx, font::FontSize};

        with_new_runtime(|_| {
            let mut fs = create_signal(FontSize::Fixed(10));
            let mut root: El<NullWtf> =
                Label::<NullWtf>::new("x").font_size(fs).into_el();
            let arena = create_signal(ElArena::new());
            let root_id = BuildCtx::run(&mut root, arena);

            assert_eq!(
                arena.with_untracked(|a| a
                    .layout(root_id)
                    .unwrap()
                    .font_props()
                    .map(|fp| fp.font_size)),
                Some(Some(FontSize::Fixed(10))),
            );

            // WS6.4.0(iv): the relayout request IS the arena's dirty mark — the
            // `relayout` Trigger this used to observe is gone, because every site
            // that fired it marked the arena in the same breath. Drain the
            // build-time marks so the assertion below is about THIS write only.
            arena.clone().update_untracked(|a| {
                a.take_dirty();
            });
            assert!(
                !arena.with_untracked(|a| a.is_layout_dirty()),
                "dirty set must start drained"
            );

            fs.set(FontSize::Fixed(20));

            assert_eq!(
                arena.with_untracked(|a| a
                    .layout(root_id)
                    .unwrap()
                    .font_props()
                    .map(|fp| fp.font_size)),
                Some(Some(FontSize::Fixed(20))),
            );
            assert!(
                arena.with_untracked(|a| a.is_layout_dirty()),
                "a reactive layout-prop write must mark the arena layout-dirty \
                 — that mark is the relayout request"
            );
        });
    }
}
