use crate::{
    el::{
        ElId, RedrawReason, WidgetFlags,
        arena::{ArenaEls, ElArena},
        ctx::{PageState, WidgetCtx},
    },
    font::{Font, FontCtx, FontProps, ResolvedFontProps},
    layout::model::LayoutModelNode,
    page::PageStyle,
    render::prelude::*,
    style::{
        Style, StylePseudoClass, StyleSelector, TreeStyle, stylist::Stylist,
    },
};
use alloc::vec::Vec;
use core::{
    cell::{Cell, RefCell},
    marker::PhantomData,
};
use log::debug;
use rsact_reactive::{prelude::*, signal::marker::ReadOnly};
// Not in `render::prelude` — `Renderer::image` is the only method that needs
// it, so it is imported here for the drawing-seam impl below.
use rsact_render::image::DrawImage;
use tinyvec::TinyVec;

pub struct CtxReady;
pub struct CtxUnready;

// TODO: Make RenderCtx a delegate to renderer so u can do
// `Primitive::(...).render(ctx)`? Maybe later, and surely not .render(ctx), at
// least .render(ctx.renderer), otherwise it breaks encapsulation of the crates.

/// The screen area a widget actually affects: its layout rect grown by however
/// far it paints **outside** that rect (WS6.4c(G), ISSUE-3).
///
/// Four consumers must agree on this and none may compute it themselves — the
/// per-node geometry gate, the damage rect, the traversal prune, and (later) the
/// seam's bounds `debug_assert`. Routing them all through one function is what
/// makes `Widget::ext_draw` a one-line change when outlines, box shadows and
/// tooltips start reporting a non-zero outset, instead of a hunt through the
/// render path for every place that said `layout.outer`.
///
/// `ext` is `Padding::zero()` at every call site today, so this is currently the
/// identity on `layout.outer` — deliberately: the mechanism is declared now and
/// the computation is postponed (maintainer, 2026-08-05).
///
/// WS5.3's `layout_changed_set` and WS6.1's `layout_repaint_roots` derive damage
/// from `outer` too and join this list the moment `ext` stops being zero.
pub fn paint_bounds(layout: &LayoutModelNode<'_>, ext: Padding) -> Rect {
    layout.outer.outset(ext)
}

/// What this pass over the widget tree is *for* (WS6.4c).
///
/// A frame is one of two shapes. On a **full framebuffer** it is a single
/// [`Fused`] pass: the surface survives between frames, so skipping a clean
/// widget leaves last frame's pixels in place and probe-gating is not merely
/// allowed but optimal. Under **tiling** (6.4d) it is one [`Collect`] pass
/// followed by K [`Paint`] passes, because a tile is *scratch with no history* —
/// skipping a clean widget flushes a tile with holes, so paint must be selected
/// by geometry rather than by dirtiness.
///
/// That is why the split exists at all: `render_part`'s probe currently does two
/// jobs — *detect change* and *authorize paint* — and a second pass over an
/// already-painted frame finds every probe clean and paints nothing. Separating
/// the roles dissolves it.
///
/// [`Fused`]: RenderMode::Fused
/// [`Collect`]: RenderMode::Collect
/// [`Paint`]: RenderMode::Paint
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderMode {
    /// Full-frame: tracked, probe-gated, **and** painting, pushing damage for
    /// the flush. Today's behaviour and still the default — see the type docs
    /// for why probe-gating is right when the surface has history.
    Fused,
    /// Plan the frame: tracked and probe-gated exactly like [`Fused`], but the
    /// drawing is discarded and only the damage rects survive.
    ///
    /// The bodies must genuinely run: dirtiness is only knowable by running
    /// them (the tracked reads are interleaved with the draw calls), and the run
    /// is also what re-tracks dynamic dependencies — an `is_dirty()` peek would
    /// freeze the recorded source set and silently break conditional reads.
    /// Probes are marked clean *here*, not after the last paint, so a write that
    /// lands mid-frame re-plans on the next frame instead of being swallowed.
    ///
    /// [`Fused`]: RenderMode::Fused
    Collect,
    /// Paint one region: **untracked, no probe**, everything intersecting the
    /// region repaints.
    ///
    /// Untracked so that `run_probe`'s `clear_sources` + re-subscribe +
    /// `mark_clean` round-trip is paid once per frame regardless of how many
    /// regions the frame is cut into; polling K times would be K× graph churn.
    /// No damage is pushed either — under tiling the region *is* the flush unit,
    /// and paint-derived damage feeding the plan would make every painted region
    /// re-damage itself (6.4d(4): the two damage channels stay separate).
    Paint,
}

impl RenderMode {
    /// Does this pass discard its drawing?
    ///
    /// The check lives at the drawing seam ([`RenderCtx`]'s [`Renderer`] impl),
    /// so it short-circuits *before* a primitive rasterizes and no backend has
    /// to know the mode exists.
    pub fn is_muted(self) -> bool {
        matches!(self, Self::Collect)
    }

    /// Does this pass consult (and clean) part probes?
    ///
    /// False for [`Paint`], which is selected by geometry alone.
    ///
    /// [`Paint`]: RenderMode::Paint
    pub fn is_probe_gated(self) -> bool {
        matches!(self, Self::Fused | Self::Collect)
    }

    /// Does this pass contribute to the damage set?
    ///
    /// [`Fused`] records it for the flush and [`Collect`] records it as the
    /// *plan*; [`Paint`] records nothing, because under tiling the region is
    /// already the flush unit and paint-derived damage feeding the plan would
    /// make every painted region re-damage itself — the two channels have to
    /// stay separate (6.4d(4)).
    ///
    /// Spelled out rather than reusing [`is_probe_gated`] (which is the same
    /// set today): they answer different questions and will diverge the moment
    /// one of them grows a mode.
    ///
    /// [`Fused`]: RenderMode::Fused
    /// [`Collect`]: RenderMode::Collect
    /// [`Paint`]: RenderMode::Paint
    /// [`is_probe_gated`]: RenderMode::is_probe_gated
    pub fn records_damage(self) -> bool {
        matches!(self, Self::Fused | Self::Collect)
    }
}

/// The life of one region's background fill, which `Page::paint_region` defers
/// rather than paints.
///
/// Deferring is what lets it be *skipped*: a themed page fills a region-sized
/// rect up to three times — this background, a part's `clear_outer`, and a
/// widget's own opaque block — and every one past the first writes pixels the
/// next overwrites. Handing the fill to the walk as a pending one lets the first
/// op that provably covers the region take its place.
///
/// A state rather than an `Option` plus a bool: `Painted` and `Cancelled` are
/// both "no longer pending" but only one of them means a fill happened, and the
/// tile harness needs to tell them apart — an op log cannot, a root part's
/// `clear_outer` over the region having the same rect and colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RegionBackground<C> {
    /// Not wanted: the page declares no background colour.
    #[default]
    None,
    /// Wanted, not yet painted, and still cancellable.
    Pending(Rect, C),
    /// Painted, because nothing covered the region in time.
    Painted,
    /// Skipped, because some op wrote every pixel of the region. The win.
    Cancelled,
}

impl<C> RegionBackground<C> {
    pub fn pending(&self) -> Option<(Rect, C)>
    where
        C: Copy,
    {
        match self {
            Self::Pending(region, color) => Some((*region, *color)),
            _ => None,
        }
    }
}

pub struct RenderShared<'a, W: WidgetCtx> {
    /// What this pass is for (WS6.4c). Carried down the walk as a plain value,
    /// like `force_redraw`.
    pub mode: RenderMode,
    pub page_state: &'a PageState<W>,
    pub page_style: Signal<PageStyle<W::Color>, ReadOnly>,
    /// Both plain values, not reactive handles: rsact targets fixed displays
    /// and has no runtime font work, so `Page` holds them as constants (see
    /// `Page::fonts`). A borrow rides `Copy` `RenderShared` fine, alongside
    /// `page_state`/`stylist`/`damage`.
    pub viewport: Size,
    pub fonts: &'a FontCtx,
    pub stylist: &'a W::Stylist,
    /// Page-level "repaint everything this frame" flag (e.g. after a layout
    /// change that reached the root, or an explicit `Page::force_redraw`).
    ///
    /// WS6.4.0(iv): a plain `bool` carried down the walk, not a `Signal<bool>`
    /// that every part's probe subscribed to. It was never read as a value here —
    /// only `track()`ed — so its whole job was invalidation broadcast: setting it
    /// created and walked one subscriber edge PER PART. As an argument it is one
    /// boolean OR per part, no edges at all, and the page probe is woken by the
    /// same flag feeding its poll `force` (see `Page::use_renderer`).
    pub force_redraw: bool,
    /// WS6.2: the page's damage accumulator. `render_part` pushes the absolute
    /// outer rect of each **redraw root** (a part that repainted without a
    /// parent already clearing its area) here; the page flushes only these rects
    /// via `finish_frame_regions`. A shared `&RefCell` so it rides `Copy`
    /// `RenderShared` and every sibling in the walk appends to the one list.
    pub damage: &'a RefCell<Vec<Rect>>,
    /// This region's deferred background — see [`RegionBackground`].
    ///
    /// A shared `&Cell` for the same reason [`Self::damage`] is a shared
    /// `&RefCell`: `RenderShared` is `Copy` and is copied into every child ctx,
    /// so a plain field would be settled per part rather than per region — the
    /// background would be painted once for every drawing part. `Cell` rather
    /// than `RefCell` because the state is `Copy + Default` (`Color: Copy`),
    /// which keeps a borrow counter and its panic arm off the hottest path in
    /// the walk.
    pub region_background: &'a Cell<RegionBackground<W::Color>>,
    /// WS6.4c(E): how many nodes this pass actually processed, i.e. survived the
    /// traversal prune.
    ///
    /// The traversal term is the one cost an op log cannot see — a transparent
    /// `Flex` emits nothing yet is visited, styled and recursed through — so
    /// 6.4a could only ever *model* it. Counting it here makes the prune's
    /// effect measurable instead of argued, and gives 6.6's dirty-list walk a
    /// number to beat from the other end. One `Cell` increment per processed
    /// node, against the hundreds of cycles a node visit already costs.
    pub visits: &'a Cell<usize>,
}

impl<'a, W: WidgetCtx> Clone for RenderShared<'a, W> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, W: WidgetCtx> Copy for RenderShared<'a, W> {}

pub struct RenderVisual<W: WidgetCtx> {
    pub tree_style: TreeStyle<W::Color>,
    pub font_props: FontProps,
}

impl<W: WidgetCtx> Clone for RenderVisual<W> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<W: WidgetCtx> Copy for RenderVisual<W> {}

#[derive(Clone, Copy)]
pub struct RenderFrame {
    parent_dirty: bool,
    nesting_level: usize,
    call: usize,
}

impl RenderFrame {
    pub fn root(call: usize) -> Self {
        Self { parent_dirty: false, nesting_level: 0, call }
    }
}

pub struct RenderCtx<'a, W: WidgetCtx, S = CtxUnready> {
    pub id: ElId,
    debug_name: &'a str,
    dirten: &'a mut bool,
    needs_redraw: Option<RedrawReason>,
    /// This widget's declared behaviour (WS6.4c(F) reads `CLIPS_SELF` here).
    flags: WidgetFlags,
    /// How far outside `layout.outer` this widget paints (WS6.4c(G)). Zero for
    /// every widget today; see [`Widget::ext_draw`].
    ///
    /// [`Widget::ext_draw`]: crate::widget::Widget::ext_draw
    ext: Padding,
    hovered: bool,
    pressed: bool,
    /// This element's render probes, pre-extracted from its `ElState` for the
    /// duration of the render (the widget is only `&self`-borrowed, so the
    /// probes are `mem::take`-n out and written back in `render_subtree_body`).
    /// `render_part` looks up / lazily creates the probe for a part key here.
    part_probes: &'a mut TinyVec<[(&'static str, Probe); 2]>,

    /// **Private on purpose** (WS6.4c(A), maintainer-confirmed): the ctx *is*
    /// the renderer widgets draw on — see the [`Renderer`] impl below. A widget
    /// that could reach this field could bypass the render mode, and then the
    /// mode is advisory rather than enforced. Only this module touches it.
    renderer: &'a mut W::Renderer,
    pub layout: &'a LayoutModelNode<'a>,
    /// Inheritable visual properties (tree_style, font_props).
    pub visual: RenderVisual<W>,
    /// Per-element rendering state (dirty flags, nesting, call counter).
    frame: RenderFrame,
    /// Shared page-level context (signals, page state, stylist).
    pub shared: RenderShared<'a, W>,
    _marker: PhantomData<S>,
}

impl<'a, W: WidgetCtx> RenderCtx<'a, W, CtxReady> {
    #[must_use]
    pub fn render_font(
        &mut self,
        font: Font,
        content: &str,
        props: ResolvedFontProps,
        bounds: Rect,
        color: W::Color,
    ) -> RenderResult {
        // WS6.4c(A): the font stack draws through `self` — the drawing seam —
        // not through the raw renderer, so text obeys the render mode like every
        // other primitive. `FontHandler::draw` is generic over the *renderer*
        // for exactly this reason. The borrow is copied out first so the call
        // can borrow `self` mutably (`&FontCtx` is `Copy`).
        //
        // This used to be a `try_with` on a `Signal<FontCtx>` with a
        // log-and-degrade arm for "the font provider was disposed". A plain
        // borrow cannot be disposed, so the defensive path is gone rather than
        // merely unreachable — one fewer never-taken branch on the text path.
        let fonts = self.shared.fonts;
        fonts.render(font, content, props, bounds, color, self)
    }

    // TODO: Call automatically based on behavior
    #[must_use]
    pub fn render_focus_outline(&mut self, id: ElId) -> RenderResult {
        if self.shared.page_state.is_focused(id) {
            Block::from_layout_style(
                self.layout.outer,
                BlockStyle::base().outline(
                    OutlineStyle::base()
                        .width(1)
                        .color(<W::Color as Color>::accents()[1]),
                ),
            )
            // Through the seam (`self`), not the raw renderer: this is a draw
            // call like any other and must obey the render mode. It is also the
            // one that paints OUTSIDE `layout.outer` — offset 0 + width 1 +
            // `StrokeAlignment::Outside` puts it 1 px beyond on all four sides
            // — so it is the first customer for `ext_draw` (ISSUE-3).
            .render(self)
        } else {
            Ok(())
        }
    }

    /// Create a sub-context with a modified `tree_style`.
    #[must_use]
    pub fn with_tree_style<R>(
        &mut self,
        tree_style: impl FnOnce(TreeStyle<W::Color>) -> TreeStyle<W::Color>,
        f: impl FnOnce(RenderCtx<'_, W, CtxReady>) -> R,
    ) -> R {
        f(RenderCtx {
            id: self.id,
            debug_name: self.debug_name,
            dirten: self.dirten,
            needs_redraw: self.needs_redraw,
            flags: self.flags,
            ext: self.ext,
            hovered: self.hovered,
            pressed: self.pressed,
            part_probes: self.part_probes,
            renderer: self.renderer,
            layout: self.layout,
            visual: RenderVisual {
                tree_style: tree_style(self.visual.tree_style),
                font_props: self.visual.font_props,
            },
            frame: self.frame,
            shared: self.shared,
            _marker: PhantomData,
        })
    }

    /// Returns the current style pseudo-class based on hover / focus state.
    ///
    /// `hovered` is pre-extracted into `frame.hovered` by [`RenderPass`]
    /// before the arena borrow was released, so this method needs no arena
    /// access.
    pub fn pseudoclass(&self) -> StylePseudoClass {
        debug!(
            "State for pseudoclass: hovered={} pressed={} focused={}",
            self.hovered,
            self.pressed,
            self.shared.page_state.is_focused(self.id)
        );
        StylePseudoClass::default()
            .hovered(self.hovered)
            .pressed(self.pressed)
            .focused(self.shared.page_state.is_focused(self.id))
    }

    pub fn get_style<S: Style>(
        &self,
        style: Option<&dyn Fn(S, &StyleSelector) -> S>,
    ) -> S
    where
        W::Stylist: Stylist<S>,
    {
        let pseudoclass = self.pseudoclass();
        let selector = StyleSelector { pseudoclass };
        let base = self.shared.stylist.style(&S::base(), &selector);
        if let Some(style_fn) = style {
            style_fn(base, &selector)
        } else {
            base
        }
    }
}

/// **The drawing seam** (WS6.4c(A)): a ready [`RenderCtx`] *is* the renderer a
/// widget draws on, forwarding every primitive to the private `renderer` field.
///
/// Widgets used to receive `pub renderer: &mut W::Renderer` and draw straight
/// onto the backend, which left rsact with **nowhere to intercept drawing**.
/// That is why 6.4.0(ii-4) reached for a no-op `Renderer` *type* — unreachable,
/// since `Widget::render` is a non-generic trait method behind `dyn Widget<W>`
/// so `W::Renderer` cannot be substituted — and why a backend-side `set_muted`
/// was proposed and rejected: it would make every implementor carry logic only
/// rsact's frame planner needs. The missing piece was never a capability, it was
/// an **encapsulation boundary**. With the field private and this impl in place,
/// one forwarding layer owns every draw call a widget can make.
///
/// What that buys beyond 6.4c's collect pass:
///
/// - **`RenderMode`**: `Collect` runs bodies for their tracked reads and
///   discards the drawing — one branch here, short-circuiting *before* the
///   primitive rasterizes, with no backend involvement at all.
/// - **The bounds `debug_assert`** for 6.4a's "a primitive must not write
///   outside its declared bounds" invariant (ISSUE-3), which has no other home.
/// - **6.4d's per-region translation**, if it is ever wanted above the backend.
///
/// Cost is flat: primitives already take `&mut impl Renderer`, so they simply
/// instantiate against `RenderCtx<W>` instead of `W::Renderer` — the same count,
/// plus these inlinable forwarders. No `dyn`, no second widget tree.
///
/// [`Policy`] is forwarded rather than restated, so the frame planner sees the
/// real renderer's region bound through the seam.
///
/// [`Policy`]: Renderer::Policy
impl<'a, W: WidgetCtx> RenderCtx<'a, W, CtxReady> {
    /// The area a styled rect writes every pixel of, or `None` for no solid
    /// area.
    ///
    /// A fill covers the rect and nothing else has to be reasoned about: with no
    /// effective stroke the fill *is* the whole rect
    /// ([`crate::render::scan::rect`] uses `(rect, rect)` then), and with one the
    /// stroke bands cover exactly what the inset fill does not — under every
    /// [`StrokeAlignment`], since `Inside` insets the fill by the width it then
    /// strokes and the other two only grow outward.
    ///
    /// [`StrokeAlignment`]: rsact_render::style::StrokeAlignment
    fn solid_area(rect: Rect, style: &DrawStyle<W::Color>) -> Option<Rect> {
        style.fill.is_some().then_some(rect)
    }

    /// Same, for a rounded rect: the rect **minus its corner cut-outs**.
    ///
    /// Reported as coverage of `rect` only when `region` misses all four corner
    /// boxes, because inside a corner box the shape is an ellipse arc and the
    /// pixels outside it are not written. This is why tiling helps rather than
    /// hurts here: a 5 px radius on a 240x240 container spoils only the two
    /// bands that touch a corner, and the other eight are covered exactly.
    fn solid_area_rounded(
        rect: Rect,
        corners: CornerRadii,
        style: &DrawStyle<W::Color>,
        region: Rect,
    ) -> Option<Rect> {
        style.fill.is_some().then_some(rect).filter(|rect| {
            let c = corners.clamp_for(rect.size);
            let (x, y) = (rect.top_left.x, rect.top_left.y);
            let (w, h) = (rect.size.width as i32, rect.size.height as i32);
            let corner_boxes = [
                Rect::new(Point::new(x, y), c.top_left),
                Rect::new(
                    Point::new(x + w - c.top_right.width as i32, y),
                    c.top_right,
                ),
                Rect::new(
                    Point::new(
                        x + w - c.bottom_right.width as i32,
                        y + h - c.bottom_right.height as i32,
                    ),
                    c.bottom_right,
                ),
                Rect::new(
                    Point::new(x, y + h - c.bottom_left.height as i32),
                    c.bottom_left,
                ),
            ];
            !corner_boxes.iter().any(|box_| box_.intersects(&region))
        })
    }

    /// The region a deferred background is waiting on, for the two predicates
    /// that need it. `Rect::zero()` when nothing is pending, which no corner box
    /// intersects — so the rounded predicate degrades to "covered", and the
    /// settle it feeds is a no-op anyway.
    fn pending_region(&self) -> Rect {
        self.shared
            .region_background
            .get()
            .pending()
            .map_or(Rect::zero(), |(region, _)| region)
    }
}

impl<'a, W: WidgetCtx> Renderer for RenderCtx<'a, W, CtxReady> {
    type Color = W::Color;

    type Policy = <W::Renderer as Renderer>::Policy;

    fn size(&self) -> Size {
        self.renderer.size()
    }

    // A widget has no business opening a region — that is the frame driver's
    // call (6.4d) — but forwarding keeps this a faithful proxy instead of one
    // that silently swallows the call through the trait default.
    fn begin_region(&mut self, region: Rect) -> RenderResult {
        self.renderer.begin_region(region)
    }

    fn end_region(&mut self) -> RenderResult {
        self.renderer.end_region()
    }

    fn push_clip(&mut self, area: Rect) {
        self.renderer.push_clip(area)
    }

    fn pop_clip(&mut self) {
        self.renderer.pop_clip()
    }

    fn clip_bounds(&self) -> Option<Rect> {
        // Collect draws nowhere, so it is confined to nothing. Reporting `None`
        // — the trait's "not confined / not reported" — disables 6.4b's
        // per-node geometry gate and 6.4c's subtree prune for free, which is
        // exactly what "the collect pass must visit every node" requires, with
        // no special case anywhere in the walk. It is also simply true.
        if self.shared.mode.is_muted() {
            return None;
        }
        self.renderer.clip_bounds()
    }

    fn fill_solid(&mut self, rect: Rect, color: Self::Color) -> RenderResult {
        // WS6.4c: the mute. Collect runs bodies for their tracked reads
        // and discards the drawing, short-circuiting BEFORE the primitive
        // rasterizes — no backend knows the mode exists.
        if self.shared.mode.is_muted() {
            return Ok(());
        }
        self.settle_region_background(Some(rect));
        self.renderer.fill_solid(rect, color)
    }

    fn pixel(&mut self, point: Point, color: Self::Color) -> RenderResult {
        // WS6.4c: the mute. Collect runs bodies for their tracked reads
        // and discards the drawing, short-circuiting BEFORE the primitive
        // rasterizes — no backend knows the mode exists.
        if self.shared.mode.is_muted() {
            return Ok(());
        }
        self.settle_region_background(None);
        self.renderer.pixel(point, color)
    }

    fn line(
        &mut self,
        from: Point,
        to: Point,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // WS6.4c: the mute. Collect runs bodies for their tracked reads
        // and discards the drawing, short-circuiting BEFORE the primitive
        // rasterizes — no backend knows the mode exists.
        if self.shared.mode.is_muted() {
            return Ok(());
        }
        self.settle_region_background(None);
        self.renderer.line(from, to, style)
    }

    fn rect(
        &mut self,
        rect: Rect,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // WS6.4c: the mute. Collect runs bodies for their tracked reads
        // and discards the drawing, short-circuiting BEFORE the primitive
        // rasterizes — no backend knows the mode exists.
        if self.shared.mode.is_muted() {
            return Ok(());
        }
        self.settle_region_background(Self::solid_area(rect, style));
        self.renderer.rect(rect, style)
    }

    fn rounded_rect(
        &mut self,
        rect: Rect,
        corners: CornerRadii,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // WS6.4c: the mute. Collect runs bodies for their tracked reads
        // and discards the drawing, short-circuiting BEFORE the primitive
        // rasterizes — no backend knows the mode exists.
        if self.shared.mode.is_muted() {
            return Ok(());
        }
        let region = self.pending_region();
        self.settle_region_background(Self::solid_area_rounded(
            rect, corners, style, region,
        ));
        self.renderer.rounded_rect(rect, corners, style)
    }

    fn circle(
        &mut self,
        top_left: Point,
        diameter: u32,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // WS6.4c: the mute. Collect runs bodies for their tracked reads
        // and discards the drawing, short-circuiting BEFORE the primitive
        // rasterizes — no backend knows the mode exists.
        if self.shared.mode.is_muted() {
            return Ok(());
        }
        self.settle_region_background(None);
        self.renderer.circle(top_left, diameter, style)
    }

    fn arc(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // WS6.4c: the mute. Collect runs bodies for their tracked reads
        // and discards the drawing, short-circuiting BEFORE the primitive
        // rasterizes — no backend knows the mode exists.
        if self.shared.mode.is_muted() {
            return Ok(());
        }
        self.settle_region_background(None);
        self.renderer.arc(top_left, diameter, start, sweep, style)
    }

    fn ellipse(
        &mut self,
        bounding_box: Rect,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // WS6.4c: the mute. Collect runs bodies for their tracked reads
        // and discards the drawing, short-circuiting BEFORE the primitive
        // rasterizes — no backend knows the mode exists.
        if self.shared.mode.is_muted() {
            return Ok(());
        }
        self.settle_region_background(None);
        self.renderer.ellipse(bounding_box, style)
    }

    fn sector(
        &mut self,
        top_left: Point,
        diameter: u32,
        start: Angle,
        sweep: Angle,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // WS6.4c: the mute. Collect runs bodies for their tracked reads
        // and discards the drawing, short-circuiting BEFORE the primitive
        // rasterizes — no backend knows the mode exists.
        if self.shared.mode.is_muted() {
            return Ok(());
        }
        self.settle_region_background(None);
        self.renderer
            .sector(top_left, diameter, start, sweep, style)
    }

    fn polygon(
        &mut self,
        points: &[Point],
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // WS6.4c: the mute. Collect runs bodies for their tracked reads
        // and discards the drawing, short-circuiting BEFORE the primitive
        // rasterizes — no backend knows the mode exists.
        if self.shared.mode.is_muted() {
            return Ok(());
        }
        self.settle_region_background(None);
        self.renderer.polygon(points, style)
    }

    fn path(
        &mut self,
        path: &Path,
        style: &DrawStyle<Self::Color>,
    ) -> RenderResult {
        // WS6.4c: the mute. Collect runs bodies for their tracked reads
        // and discards the drawing, short-circuiting BEFORE the primitive
        // rasterizes — no backend knows the mode exists.
        if self.shared.mode.is_muted() {
            return Ok(());
        }
        self.settle_region_background(None);
        self.renderer.path(path, style)
    }

    fn image<'i>(&mut self, image: DrawImage<'i, Self::Color>) -> RenderResult {
        // WS6.4c: the mute. Collect runs bodies for their tracked reads
        // and discards the drawing, short-circuiting BEFORE the primitive
        // rasterizes — no backend knows the mode exists.
        if self.shared.mode.is_muted() {
            return Ok(());
        }
        self.settle_region_background(None);
        self.renderer.image(image)
    }
}

// CtxUnready //
impl<'a, W: WidgetCtx> RenderCtx<'a, W, CtxUnready> {
    // The part key is a `&'static str` (e.g. "self", "thumb", "options"): stable
    // per widget-source, combined with `self.id` for per-element identity, and
    // — crucially — allocation-free on the render hot path. It used to be a
    // `Display + Hash + Copy` generic, which let `render_self` build the key with
    // `format!` on every frame (WS1.7). The identity/ownership redesign is WS2.
    pub fn render_part(
        &mut self,
        hash_source: &'static str,
        f: impl FnOnce(RenderCtx<'_, W, CtxReady>) -> RenderResult,
    ) -> RenderResult {
        // WS6.4b: the GEOMETRY gate, ahead of the reactive one. A part whose area
        // cannot reach the renderer's clip cannot affect the output, so nothing
        // below needs to happen — no clear, no paint, no damage, not even a probe.
        //
        // Placed in `render_part` rather than per widget on purpose (AGENTS.md):
        // every drawing widget already funnels through here, and this is also
        // where `layout.outer` is known to be ABSOLUTE.
        //
        // Two consequences that make this sound rather than merely fast:
        //
        // - **The predicate is the renderer's own.** `clip_bounds()` never reports
        //   narrower than what the backend clips to, and a widget must not draw
        //   outside its declared bounds, so "outer misses the clip" ⇒ "every write
        //   would have been filtered anyway". `None` (a renderer that does not
        //   report, e.g. `NullRenderer`) disables the cull entirely.
        // - **A culled part is skipped, not resolved.** Its probe is left unpolled
        //   and therefore still dirty, so it repaints whenever it next comes into
        //   view — WS6.4c's "an unpainted probe stays dirty" property. The page
        //   does not spin on it either: the walk no longer reads that probe, so the
        //   page probe's `clear_sources` drops the edge and the next frame is idle.
        //   What brings it back is the geometry channel (a scroll/resize is a
        //   layout change ⇒ repaint roots or a blanket redraw), never a stale
        //   reactive edge.
        if let Some(clip) = self.renderer.clip_bounds()
            && !paint_bounds(self.layout, self.ext).intersects(&clip)
        {
            return Ok(());
        }

        // Imperative force-dirty flag that triggers redraw even if no reactive
        // dependency changed in the probe.
        // WS6.4.0(iv): `force_redraw` is OR-ed in here instead of being `track()`ed
        // inside the probe below. Same semantics — every part repaints — but as a
        // value carried down the walk rather than a subscription per part.
        let redraw = self.frame.parent_dirty
            || self.needs_redraw.is_some()
            || self.shared.force_redraw;

        // WS6.2: is this part the *root* of a repainted region? It is if no
        // parent already cleared its area (`!parent_dirty`) — then its own
        // `clear_outer` runs and it + its subtree repaint into `layout.outer`,
        // making that rect the exact damage. A part that repaints only because
        // its parent did (`parent_dirty`) is already inside the parent's damage
        // rect, so recording it would just add a redundant sub-rect.
        let is_redraw_root = !self.frame.parent_dirty;

        // WS6.4c: a **Paint** pass is selected by geometry alone — no probe is
        // consulted and none is cleaned. Two reasons, both forced rather than
        // convenient:
        //
        // - A tile is scratch with no history, so "skip the clean widgets" would
        //   flush a tile with holes. Everything the gate above admitted must
        //   repaint.
        // - Polling here would re-run `clear_sources` + re-subscribe +
        //   `mark_clean` once per region, i.e. K× the graph churn for a frame
        //   cut into K regions. The single tracked run belongs to Collect.
        //
        // No damage is recorded either: under tiling the region IS the flush
        // unit, and paint-derived damage feeding the plan would make every
        // painted region re-damage itself (6.4d(4)).
        if !self.shared.mode.is_probe_gated() {
            let result = self.run_body(hash_source, f);
            self.frame.parent_dirty = true;
            *self.dirten = true;
            return result;
        }

        // Look up (or lazily create) this element's probe for `hash_source`.
        // Linear scan with CONTENT comparison: `&'static str` pointer identity
        // is not guaranteed equal across codegen units, so keys must be
        // compared by value, never by pointer. The set is tiny (a widget's
        // part names are finite in its source), so this beats a hash. `Probe`
        // is `Copy`, so the borrow of `part_probes` ends right here — before
        // the `poll` closure below reborrows `self`.
        // TODO: `debug_assert` that one key is not polled twice per frame (a
        // widget-author bug); needs a per-frame "seen" marker to detect.
        let probe = match self
            .part_probes
            .iter()
            .find(|(key, _)| *key == hash_source)
        {
            Some(&(_, probe)) => probe,
            None => {
                // Create untracked so the probe is owned by no observer/scope:
                // its sole owner is this `ElState`, and it is disposed only via
                // `dispose_probes` (`remove_subtree` / page drop) — no cascade
                // can double-dispose it (WS2.3).
                let probe = untrack(create_probe);
                self.part_probes.push((hash_source, probe));
                probe
            },
        };

        let result = probe.poll(redraw, || self.run_body(hash_source, f));

        if result.is_some() {
            // Record the damage rect for the flush (WS6.2) — but only for a
            // redraw root, whose `outer` covers everything that repainted below
            // it. `outer` is absolute (the `LayoutModelNode` walk accumulated
            // parent offsets), which is exactly what `finish_frame_regions`
            // wants.
            if is_redraw_root {
                self.shared
                    .damage
                    .borrow_mut()
                    .push(paint_bounds(self.layout, self.ext));
            }
            self.frame.parent_dirty = true;
            *self.dirten = true;
        }

        result.unwrap_or(RenderResult::Ok(()))
    }

    /// Clear this part's rect (unless a parent already did) and run the
    /// widget's body against a ready ctx.
    ///
    /// Extracted from [`render_part`] in WS6.4c because both the probe-gated
    /// path and the geometry-selected Paint path need exactly this, and a copy
    /// in each is how the two drift apart.
    ///
    /// [`render_part`]: Self::render_part
    fn run_body(
        &mut self,
        hash_source: &'static str,
        f: impl FnOnce(RenderCtx<'_, W, CtxReady>) -> RenderResult,
    ) -> RenderResult {
        debug!(
            "{:indent$}Render {} [#{:?}] (mode={:?}, parent_dirty={}, needs_redraw={:?})",
            "",
            hash_source,
            self.id,
            self.shared.mode,
            self.frame.parent_dirty,
            self.needs_redraw,
            indent = self.frame.nesting_level
        );

        // Clear the element rect unless the parent already did so.
        //
        // Inside the body (vs outside, as the old code had it) so the clear is
        // always paired with an actual redraw — never a clear-without-redraw or
        // a redraw-without-clear.
        if !self.frame.parent_dirty {
            self.clear_outer()?;
        }

        // WS6.4c(F): a widget that declares `CLIPS_SELF` confines its OWN
        // drawing to its inner rect — `Canvas`, whose closure is user code the
        // framework did not write. Pushed AFTER the clear for the same reason
        // `CLIPS_CHILDREN` wraps only the children loop: the clear fills
        // `layout.outer`, and trimming it to `inner` would leave the padding
        // ring stale.
        let clips_self = self.flags.clips_self_set();
        if clips_self {
            self.renderer.push_clip(self.layout.inner);
        }

        let result = f(RenderCtx {
            id: self.id,
            debug_name: self.debug_name,
            dirten: self.dirten,
            needs_redraw: self.needs_redraw,
            flags: self.flags,
            ext: self.ext,
            hovered: self.hovered,
            pressed: self.pressed,
            part_probes: self.part_probes,
            renderer: self.renderer,
            layout: self.layout,
            visual: self.visual,
            shared: self.shared,
            // Children inside this closure see parent_dirty=true because we
            // just cleared/drew into this element's area above.
            frame: RenderFrame {
                parent_dirty: true,
                nesting_level: self.frame.nesting_level + 1,
                call: self.frame.call + 1,
            },
            _marker: PhantomData,
        });

        // Popped on the error path too, or an `Err` would leave every later
        // sibling clipped to this widget's rect.
        if clips_self {
            self.renderer.pop_clip();
        }

        result
    }

    #[must_use]
    pub fn render_self(
        &mut self,
        f: impl FnOnce(RenderCtx<'_, W, CtxReady>) -> RenderResult,
    ) -> RenderResult {
        // "self" is the whole-widget part key. It is combined with this
        // element's `id` inside `render_part`, so a plain `&'static str` is
        // already unique per element — no per-frame `format!` (WS1.7).
        self.render_part("self", f)
    }
}

impl<'a, W: WidgetCtx, S> RenderCtx<'a, W, S> {
    /// Resolve the deferred region background, given what the draw call about
    /// to run will cover.
    ///
    /// `covers` is the area this primitive writes **every pixel of** — `None`
    /// when it writes no solid area at all (a line, a glyph, an arc). If it
    /// contains the region, the background is redundant and dropped; otherwise
    /// it is painted first, and this is the last chance to do so in the right
    /// order.
    ///
    /// Called once per draw call and cheap after the first: the slot is `None`
    /// for the rest of the region, so this is a `Cell::get` and a branch.
    ///
    /// **The clip is part of the predicate.** A primitive's rect only covers
    /// what the clip lets through, so a narrowed clip (a `CLIPS_SELF` widget, a
    /// scrollable) can never cover the region however large the rect is. `None`
    /// from `clip_bounds` — a renderer that does not report — denies coverage,
    /// which is the safe direction.
    ///
    /// That clip test is a **precondition, not a live path**: `clear_outer`
    /// settles the slot before any widget draws, and a part's `outer` is bounded
    /// by the surface, so "the rect covers the region but the clip does not"
    /// cannot arise through the walk today. Verified by deleting it — no test
    /// fails, and `a_region_larger_than_the_surface_still_gets_its_background`
    /// says so where a reader will look. It stays because the predicate is
    /// otherwise unsound the moment anything settles the slot from inside a
    /// pushed clip (`render_subtree_body`'s `LayoutChange` fill is one line away
    /// from being that caller).
    fn settle_region_background(&mut self, covers: Option<Rect>) {
        let Some((region, bg)) = self.shared.region_background.get().pending()
        else {
            return;
        };
        let unclipped = self
            .renderer
            .clip_bounds()
            .is_some_and(|clip| clip.contains_rect(&region));
        let covered =
            unclipped && covers.is_some_and(|c| c.contains_rect(&region));

        if covered {
            self.shared
                .region_background
                .set(RegionBackground::Cancelled);
        } else {
            self.shared.region_background.set(RegionBackground::Painted);
            // Straight at the renderer: going through `self` would re-enter
            // this method, and the mute has already been checked by the caller.
            let _ = self.renderer.fill_solid(region, bg);
        }
    }

    fn clear_outer(&mut self) -> RenderResult {
        // TODO: Feature-gated or debug-redraw flag
        // Debug redraws, works good only for colors with alpha. But we can use
        // some bright background too TODO: Actually, this should happen
        // after draw [ ] better when render_pass added, or do it right
        // now as a separate call.

        // self.renderer.rect(
        //     self.layout.outer,
        //     &DrawStyle::default().fill(
        //         W::Color::accents()[(self.frame.nesting_level
        //             + self.frame.call)
        //             % ACCENT_COUNT],
        //     ),
        //     // .stroke(W::Color::accents()[4])
        //     // .stroke_width(1),
        // )

        // WS6.4c: the clear is a draw call like any other and must obey the
        // mode. It cannot go through the seam — `clear_outer` is available in
        // both ctx states and `Renderer` is implemented only for `CtxReady` —
        // so the mute is checked here explicitly. A Collect pass that cleared
        // would paint the page background over a frame it is only planning.
        if self.shared.mode.is_muted() {
            return Ok(());
        }

        // A draw call like any other, so it settles the deferred region
        // background first — and may cancel it: a root part's `outer` covers the
        // whole region on a full-frame pass, which is the common case and the
        // one where the two fills were pure duplication.
        let outer = self.layout.outer;
        self.settle_region_background(Some(outer));

        self.shared
            .page_style
            .try_with(|style| {
                if let Some(bg) = style.background_color {
                    self.renderer.fill_solid(outer, bg).map_err(|_| ())
                } else {
                    Ok(())
                }
            })
            .unwrap_or_else(|| {
                log::error!("clear skipped: page style signal was disposed");
                RenderResult::Ok(())
            })
    }
}

pub(crate) struct RenderPass<'a, W: WidgetCtx> {
    arena: &'a mut ElArena<W>,
    renderer: &'a mut W::Renderer,
    shared: RenderShared<'a, W>,
}

impl<'a, W: WidgetCtx> RenderPass<'a, W> {
    pub fn new(
        arena: &'a mut ElArena<W>,
        renderer: &'a mut W::Renderer,
        shared: RenderShared<'a, W>,
    ) -> Self {
        Self { arena, renderer, shared }
    }

    pub fn render(
        &mut self,
        layout: &LayoutModelNode<'_>,
        visual: RenderVisual<W>,
        frame: RenderFrame,
    ) -> RenderResult {
        render_subtree(
            &mut self.arena.els,
            self.renderer,
            self.shared,
            layout,
            visual,
            frame,
        )
    }
}

fn render_subtree<W: WidgetCtx>(
    els: &mut ArenaEls<W>,
    renderer: &mut W::Renderer,
    shared: RenderShared<'_, W>,
    layout: &LayoutModelNode<'_>,
    visual: RenderVisual<W>,
    frame: RenderFrame,
) -> RenderResult {
    debug!("{:indent$}->", "", indent = frame.nesting_level);

    // WS5.1: dispatch by the layout node's own id — the layout tree is the
    // source of truth for identity (transparent roots are flattened by
    // `model_layout`, so `layout.id()` is always a real widget).
    let id = layout.id();

    // WS6.4c(G): how far outside its rect this widget paints. Looked up here
    // rather than inside the body because the PRUNE below needs it, and a prune
    // that used a smaller rect than the paint is the unsound direction.
    let ext = els
        .expect(id)
        .and_then(|data| data.stage.built())
        .map(|widget| widget.ext_draw())
        .unwrap_or_else(Padding::zero);

    // WS6.4c(E): **the traversal prune** — the cost 6.4b left on the table.
    //
    // 6.4b's gate stops a node from PAINTING when its area misses the clip, but
    // the walk still visits every node, styles it and recurses: at 10 regions
    // that is ×10.00 the node visits of a single pass, against a floor of
    // ×1.19–2.85 (6.4a's `VisitReport`). Stopping the descent is the only thing
    // that closes the gap.
    //
    // Soundness rests on one invariant, which 6.4c(F) is what makes true:
    // **everything a subtree draws is inside `outer ∩ enclosing clips`**. A
    // child may only escape its parent's rect if that parent clips its children
    // (`CLIPS_CHILDREN`), in which case the escape is invisible anyway. The
    // `debug_assert` in the children loop is what keeps a future layout honest;
    // 6.4a's `escaping` counter is the same check from the measurement side.
    //
    // `clip_bounds()` already reports the composed clip — region ∩ every
    // enclosing widget clip (nested clips compose since PR #36) — so the prune
    // costs one rect test and **zero bytes per node**. This is why 6.4b's costed
    // `subtree_fits`-vs-union-`Rect` storage decision was struck: it existed only
    // to tolerate escapes that a widget-declared clip now prevents.
    //
    // `Collect` reports `None` here and so never prunes, which is exactly the
    // requirement that its bodies all run.
    if let Some(clip) = renderer.clip_bounds()
        && !paint_bounds(layout, ext).intersects(&clip)
    {
        debug!(
            "{:indent$}<- pruned (outside {clip})",
            "",
            indent = frame.nesting_level
        );
        return Ok(());
    }

    // Counted AFTER the prune, so this equals 6.4a's `VisitReport::cullable`
    // model rather than "times we looked at a node" — the number the floor is
    // expressed in.
    shared.visits.set(shared.visits.get() + 1);

    // Build the per-element frame.
    let child_frame = RenderFrame {
        parent_dirty: frame.parent_dirty,
        nesting_level: frame.nesting_level,
        call: frame.call,
    };

    // WS6.4c(F): the clip is applied inside `render_subtree_body`, around the
    // CHILDREN loop only — see the note there for why it cannot wrap the body.
    render_subtree_body(els, renderer, shared, layout, visual, child_frame, ext)
}

fn render_subtree_body<W: WidgetCtx>(
    els: &mut ArenaEls<W>,
    renderer: &mut W::Renderer,
    shared: RenderShared<'_, W>,
    layout: &LayoutModelNode<'_>,
    visual: RenderVisual<W>,
    frame: RenderFrame,
    ext: Padding,
) -> RenderResult {
    // WS5.1: this widget's id is the layout node's id (see `render_subtree`).
    let id = layout.id();

    // Pre-extract the mutable per-element render state so the widget can be
    // rendered through a *shared* borrow of its `ElData` (`Widget::render`
    // takes `&self`): `needs_redraw` is taken as before, and the element's
    // `part_probes` are `mem::take`-n into a local for `render_part` to look up
    // / grow. Both are written back after the render + children recursion
    // release their borrows on `els` (see the end of this fn). The probes are a
    // tiny inline `TinyVec`, so this move is cheap.
    let (needs_redraw, mut part_probes) = els
        .expect_mut(id)
        .map(|data| {
            (
                data.state.take_needs_redraw(),
                core::mem::take(&mut data.state.part_probes),
            )
        })
        .unwrap_or_default();

    let Some(data) = els.expect(id) else { return Ok(()) };

    debug!(
        "{:indent$}Check `{}` [{:?}]",
        "",
        data.state.debug_name,
        id,
        indent = frame.nesting_level
    );

    let flags = data.state.flags;
    let mut dirten = false;
    let ctx = RenderCtx {
        id,
        debug_name: data.state.debug_name,
        dirten: &mut dirten,
        needs_redraw,
        flags,
        ext,
        hovered: data.state.hovered(),
        pressed: data.state.pressed(),
        part_probes: &mut part_probes,
        renderer,
        layout,
        visual,
        frame,
        shared,
        _marker: PhantomData::<CtxUnready>,
    };
    match data.stage.built() {
        Some(widget) => widget.render(ctx)?,
        None => return Ok(()),
    }

    // WS6.1: a TRANSPARENT widget (no-op render — Flex / most containers) that
    // is a targeted repaint root (`LayoutChange`, from `layout_repaint_roots`)
    // never called `render_part`, so it neither cleared its area nor recorded
    // damage — yet it IS the stable ancestor whose clear must erase its moved
    // children's OLD positions. Do both here, then propagate `parent_dirty` so
    // the subtree redraws over the cleared area. A widget that actually drew
    // (`dirten`) already handled this in `render_part`, so skip it — no double
    // clear. Scoped to `LayoutChange` (only set under `incremental-layout`), so
    // the default blanket path is unaffected.
    if !dirten
        && !frame.parent_dirty
        && matches!(needs_redraw, Some(RedrawReason::LayoutChange))
    {
        // WS6.4c: this one writes to the renderer directly (a transparent
        // widget never reaches `render_part`, so there is no ctx to draw
        // through), which means the mode has to be honoured by hand — a Collect
        // pass must not paint, and a Paint pass must not feed the plan channel.
        if !shared.mode.is_muted()
            && let Some(bg) = shared.page_style.with(|s| s.background_color)
        {
            renderer.fill_solid(layout.outer, bg)?;
        }
        if shared.mode.records_damage() {
            shared.damage.borrow_mut().push(paint_bounds(layout, ext));
        }
        dirten = true;
    }

    let children_frame = RenderFrame {
        parent_dirty: dirten || frame.parent_dirty,
        nesting_level: frame.nesting_level + 1,
        ..frame
    };

    // WS5.1: dispatch to children by identity. `model_layout` walks the arena's
    // `effective_children` (transparent nodes like `Dynamic` already flattened)
    // and tags each `LayoutModel` child with its `ElId`, so we render each child
    // widget directly by that id — no positional zip against the raw arena child
    // list, and no separate transparent-layout branch (a transparent wrapper is
    // simply absent from the layout tree; its real child sits in its place, with
    // its own layout).
    // WS6.4c(F): a widget that declares `CLIPS_CHILDREN` confines its SUBTREE —
    // and only its subtree — to its inner rect.
    //
    // Around the children loop, never around the body above. The dead
    // `ClipPath::InnerRect` arm this replaces wrapped the whole body, which
    // would have clipped the widget's own paint too: `Scrollable`, the one
    // widget that must set this flag, draws its `Block` on `layout.outer`
    // (`scrollable.rs`), so a body-wide inner clip erases its own background and
    // border while `clear_outer`'s fill of `outer` is trimmed to `inner`,
    // leaving stale pixels in the padding ring. Latent until now only because
    // nothing ever set `clip_path`.
    //
    // This is what makes containment STRUCTURAL, and therefore what makes the
    // traversal prune sound with no per-node storage: everything a subtree draws
    // is inside `outer ∩ enclosing clips`, and `Renderer::clip_bounds()` already
    // reports that composed rect (nested clips compose since PR #36).
    //
    // `pop_clip` runs on the error path too — an `Err` escaping with the clip
    // still pushed would clip every later sibling to this subtree's rect.
    let clips_children = flags.clips_children_set();
    if clips_children {
        renderer.push_clip(layout.inner);
    }

    let children = (|| -> RenderResult {
        for child_layout in layout.children() {
            // WS6.4c(E): the prune's premise, checked where it can be violated.
            //
            // Pruning a subtree on its root's rect is sound only if the subtree
            // stays inside it — or if this widget clips, which makes any escape
            // invisible. A layout that breaks both silently produces a plausible
            // image today and a cracked tile under 6.4d, so it is worth a debug
            // check rather than a comment. 6.4a's `VisitReport::escaping` is the
            // same predicate from the measurement side.
            debug_assert!(
                clips_children
                    || layout.outer.union(&child_layout.outer) == layout.outer,
                "[BUG] child {:?} ({}) escapes its parent {:?} ({}) which does \
                 not declare CLIPS_CHILDREN — the traversal prune would skip a \
                 subtree that still draws",
                child_layout.id(),
                child_layout.outer,
                layout.id(),
                layout.outer,
            );

            let child_font_props =
                child_layout.font_props().unwrap_or(visual.font_props);
            let child_visual = RenderVisual {
                font_props: child_font_props,
                tree_style: visual.tree_style,
            };

            render_subtree(
                els,
                renderer,
                shared,
                &child_layout,
                child_visual,
                children_frame,
            )?;
        }
        Ok(())
    })();

    if clips_children {
        renderer.pop_clip();
    }
    children?;

    // // TODO: Remove/hide debug only
    // if needs_redraw.is_some() {
    //     renderer.arc(
    //         layout.outer.top_left,
    //         5,
    //         Angle::ZERO,
    //         Angle::FULL_CIRCLE,
    //         &DrawStyle::default().fill(W::Color::accents()[1]),
    //     )?;
    // }

    // Write the (possibly grown) probe set back into the element. `els` is
    // free again now that `data`'s shared borrow and the children recursion's
    // `&mut` borrows have ended. If the element was removed mid-render the
    // probes are simply dropped (their disposal is handled by `remove_subtree`).
    if let Some(data) = els.expect_mut(id) {
        data.state.part_probes = part_probes;
    }

    debug!("{:indent$}<-", "", indent = frame.nesting_level);

    Ok(())
}
