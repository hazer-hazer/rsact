use crate::{
    el::{El, View, arena::ElArena, ctx::*},
    event::{
        Event, UnhandledEvent,
        message::{UiMessage, UiQueue},
    },
    font::{FontCtx, FontImport},
    page::{Page, dev::DevTools, id::PageId},
    render::prelude::*,
    style::stylist::InternalStylist,
};
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{fmt::Debug, marker::PhantomData};
use log::info;
use rsact_reactive::prelude::*;
use rsact_reactive::scope::new_scope;
use tinyvec::TinyVec;

/// WS6.4d: one tiled frame in progress — a cursor over the regions to paint.
///
/// Obtained from [`UI::start_frame`], which is where the frame is planned and
/// where the docs for the whole design live. Holding one borrows the `UI`
/// mutably for the frame's duration, which is the point: it makes mutating the
/// tree between two regions of the same frame a compile error rather than a
/// documented contract.
///
/// Regions are handed out in the display's own scan order. Dropping the handle
/// early is allowed and safe — the regions not yet handed out are **deferred**
/// into the next frame rather than dropped, so an abandoned frame costs latency
/// and never leaves a stale rectangle on the screen.
pub struct Frame<'a, W: WidgetCtx, P: FramePolicy> {
    ui: &'a mut UI<W, WithPages>,
    cursor: usize,
    policy: PhantomData<P>,
}

impl<W: WidgetCtx, P: FramePolicy> Frame<'_, W, P> {
    /// The next region to paint, or `None` when the frame is done.
    ///
    /// A `&mut self` cursor rather than an `Iterator`: an iterator would borrow
    /// the frame for the whole loop, and [`Self::render`] needs `&mut self`
    /// inside it.
    pub fn next_region(&mut self) -> Option<Rect> {
        let region = self.ui.frame_regions.get(self.cursor).copied();
        if region.is_some() {
            self.cursor += 1;
        }
        region
    }

    /// How many regions this frame was planned into. Constant for the frame.
    pub fn regions(&self) -> usize {
        self.ui.frame_regions.len()
    }

    /// Paint `region`.
    ///
    /// Untracked and probe-free: **everything intersecting `region` repaints**,
    /// changed or not, because the surface arrives holding whatever the last
    /// region left in it. That is the tile contract, and it is why region shape
    /// (rather than region count) is what the planner optimises.
    pub fn render(&mut self, region: Rect) -> RenderResult {
        let (page, renderer) = self.ui.current_page_and_renderer();
        page.paint_region(renderer, region)
    }

    /// The renderer, for the backend's own inherent API between regions —
    /// attaching and detaching tile buffers, submitting a GPU pass.
    ///
    /// rsact never sees a surface (roadmap 6.4.0, "surface ownership"), so this
    /// is the seam where the caller's buffers meet their renderer.
    pub fn renderer(&mut self) -> &mut W::Renderer {
        &mut self.ui.renderer
    }

    /// Flush `region` to `target` — the convenience path for backends that hand
    /// rsact a `DrawTarget` (the simulator, the host tests, any generic
    /// embedded-graphics driver) rather than owning their transport.
    ///
    /// A real tile pipeline does not call this: it takes the buffer through
    /// [`Self::renderer`] and ships it itself, which is what keeps the IO — and
    /// every `.await` — on the caller's side.
    pub fn flush<T: RenderTarget>(&mut self, target: &mut T, region: Rect)
    where
        W::Renderer: FinishRender<T::Color>,
    {
        self.ui.renderer.finish_frame_regions(target, &[region]);
    }
}

impl<W: WidgetCtx, P: FramePolicy> Drop for Frame<'_, W, P> {
    fn drop(&mut self) {
        let cursor = self.cursor;
        let planned = &self.ui.frame_regions;
        if cursor < planned.len() {
            // Not a warning: deferring is the designed outcome, not an error
            // (roadmap 6.7 — "defer + union-coalesce, never abort"). A page
            // change or an early `break` lands here legitimately.
            log::debug!(
                "frame dropped with {} of {} region(s) unpainted; deferring \
                 them to the next frame",
                planned.len() - cursor,
                planned.len()
            );
            let unpainted = planned[cursor..].to_vec();
            self.ui.deferred_regions.extend(unpainted);
        }
    }
}

pub struct UiOptions {
    auto_focus: bool,
    // TODO: Event interpretation logic settings
}

impl Default for UiOptions {
    fn default() -> Self {
        Self { auto_focus: false }
    }
}

pub trait HasPages {}
pub struct NoPages;
impl HasPages for NoPages {}
pub struct WithPages;
impl HasPages for WithPages {}

pub trait PageInitFn<W: WidgetCtx> {
    fn init_page(&self) -> El<W>;
}

impl<W: WidgetCtx, F, T> PageInitFn<W> for F
where
    F: Fn() -> T,
    T: View<W>,
{
    fn init_page(&self) -> El<W> {
        (self)().into_el()
    }
}

pub struct UI<W: WidgetCtx, P: HasPages> {
    page_history: TinyVec<[W::PageId; 1]>,
    // 9a.2: sorted `Vec` keyed by `PageId` instead of a `BTreeMap` — N is tiny
    // (1–5 pages), so binary search over a flat vec drops the BTreeMap
    // monomorphization/allocation for no practical lookup cost. Kept sorted by
    // id so `binary_search_by` is valid.
    pages: Vec<(W::PageId, Box<dyn PageInitFn<W>>)>,
    /// Currently active page. Lazily (re)built from the corresponding
    /// [`PageInitFn`] on navigation. Only the active page is kept built; each
    /// page owns its own arena, so dropping it (on navigation) frees its tree.
    active_page: Option<Page<W>>,
    /// The viewport, a plain `Size` — see the TODO on [`Self::new`]: rsact
    /// targets fixed displays and has no windowing, so this is a constant taken
    /// from the renderer at construction. It was a `MaybeReactive<Size>` that
    /// was always built as `Inert`, i.e. a reactive wrapper around a constant.
    viewport: Size,
    on_exit: Option<Box<dyn Fn()>>,
    // TODO: Get rid of Inert wrapper, it is at most RefCell
    stylist: Inert<W::Stylist>,
    dev_tools: Signal<DevTools>,
    /// The renderer. WS5.0b: a plain single-owner field.
    ///
    /// It was a `Signal<W::Renderer>` copied into every page — not for
    /// reactivity (it never had a subscriber; every access went through
    /// `update_untracked`) but because `Inert` is read-only and `Signal` was the
    /// only `Copy` handle yielding `&mut`. Pages now borrow it for the duration
    /// of a render call (see [`Self::current_page_and_renderer`]), which drops a
    /// reactive node, removes the last `update_untracked` on the render path,
    /// and keeps the renderer's size out of any move — relevant once WS6.4d's
    /// `TiledOutput<C, const MAX>` holds its buffer inline.
    renderer: W::Renderer,
    message_queue: Option<UiQueue<W>>,
    options: UiOptions,
    has_pages: PhantomData<P>,
    /// The font context. `Rc`, not a `Signal`: it is set only through the
    /// consuming builder methods below and there is no runtime font work, so
    /// the reactivity bought nothing and cost a node plus — once layout stopped
    /// tracking it (ISSUE-2) — a binding effect per page.
    ///
    /// `Rc` rather than a per-page copy because `FixedFontCollection` holds a
    /// nested `BTreeMap` of glyph data; duplicating that per page is not the
    /// "1–3 element Vec" the `FontCtx` comment suggests. `Rc` rather than
    /// lending `&FontCtx` into every `Page` method (the WS5.0b renderer
    /// pattern) because that threads a parameter through six methods and their
    /// tests to save one refcount — the renderer needs `&mut`, which forces the
    /// borrow; fonts are read-only after build, which does not.
    fonts: Rc<FontCtx>,
    /// WS6.4d: this frame's plan — the regions [`Frame`] hands out.
    ///
    /// A `UI` field rather than a `Frame` one so the allocation survives across
    /// frames: after the first few frames it never grows again, which is the
    /// no-per-frame-allocation property `rsact_render::region` was shaped for.
    frame_regions: Vec<Rect>,
    /// WS6.4d: damage a previous frame planned but never painted.
    ///
    /// Dropping a [`Frame`] before its regions run out defers them here instead
    /// of losing them — an unpainted region is a stale rectangle on the display,
    /// and nothing would damage it again until whatever is underneath happens to
    /// change. This is 6.7's frame-coherence rule ("defer + union-coalesce,
    /// never abort") at its smallest useful size: the deferred set is folded
    /// into the next frame's damage and re-planned, so it merges rather than
    /// accumulates and cannot grow without bound.
    deferred_regions: Vec<Rect>,
}

impl<R, I, S, E> UI<Wtf<R, I, S, E>, NoPages>
where
    R: Renderer + 'static,
    I: PageId + 'static,
    // WS4.1: `Wtf<..>: WidgetCtx` now requires the stylist be `Clone` (inline
    // `Inert` storage; the UI clones it into each page).
    S: InternalStylist<R::Color> + Clone + 'static,
    E: Debug + 'static,
{
    // TODO: For now I made viewport inert, but it is possible for the viewport
    // to change (e.g. window resize, etc). But as now we targeting embedded
    // devices with fixed displays and don't support any windowing, I hold it.
    pub fn new(stylist: S, renderer: R) -> Self {
        let viewport = renderer.size();

        let dev_tools =
            create_signal(DevTools { enabled: false, hovered: None });

        let fonts = Rc::new(FontCtx::new());

        Self {
            page_history: Default::default(),
            viewport,
            pages: Vec::new(),
            active_page: None,
            on_exit: None,
            stylist: stylist.inert(),
            dev_tools,
            renderer,
            message_queue: None,
            options: Default::default(),
            has_pages: PhantomData,
            fonts,
            frame_regions: Vec::new(),
            deferred_regions: Vec::new(),
        }
    }

    pub fn auto_focus(mut self) -> Self {
        self.options.auto_focus = true;
        self
    }
}

impl<W: WidgetCtx, P: HasPages> UI<W, P> {
    /// Hinting method to avoid specifying generics but just set
    /// [`WidgetCtx::Event`] to [`NullEvent`]
    pub fn no_events(self) -> Self
    where
        W: WidgetCtx<CustomEvent = ()>,
    {
        self
    }

    /// Add ExitEvent handler that eats exit event
    pub fn on_exit(mut self, on_exit: impl Fn() + 'static) -> Self {
        self.on_exit = Some(Box::new(on_exit));
        self
    }

    /// Set [`MessageQueue`] for UI, that will be used for animations and UI
    /// messages
    pub fn with_queue(mut self, queue: UiQueue<W>) -> Self {
        self.message_queue = Some(queue);
        self
    }

    // TODO: Can do with_single_page and avoid storing page function.
    // TODO: Type guard for SinglePage to disallow adding new pages.
    /// Adds page to the UI.
    /// The first added page becomes intro page
    pub fn with_page(
        mut self,
        id: W::PageId,
        page_root: impl PageInitFn<W> + 'static,
    ) -> UI<W, WithPages> {
        self.add_page(id, page_root);

        let mut with_page = UI {
            page_history: self.page_history,
            pages: self.pages,
            active_page: self.active_page,
            viewport: self.viewport,
            on_exit: self.on_exit,
            stylist: self.stylist,
            dev_tools: self.dev_tools,
            renderer: self.renderer,
            message_queue: self.message_queue,
            options: self.options,
            has_pages: PhantomData,
            fonts: self.fonts,
            frame_regions: self.frame_regions,
            deferred_regions: self.deferred_regions,
        };

        // Go to page if it is the first one
        if with_page.pages.len() == 1 {
            with_page.goto(id);
        }

        with_page
    }

    fn add_page(
        &mut self,
        id: W::PageId,
        page_fn: impl PageInitFn<W> + 'static,
    ) {
        match self.pages.binary_search_by(|(k, _)| k.cmp(&id)) {
            Ok(_) => panic!("Page with this id was already added"),
            Err(i) => self.pages.insert(i, (id, Box::new(page_fn))),
        }
    }

    // Fonts //
    //
    // ORDERING NOW MATTERS, and is enforced rather than documented. Both are
    // consuming *builder* methods, and `with_page` builds the first page
    // immediately (it calls `goto`) — so a font added after it would have to
    // retroactively re-measure a page that is already laid out.
    //
    // While `fonts` was a `Signal` the page shared the handle, so the ordering
    // was merely invisible, not correct: the page's text had still been
    // measured with the old fonts and nothing marked it dirty. Now the page
    // holds an `Rc` clone, which makes `Rc::get_mut` a precise test for "has
    // anyone built against these fonts yet" — so a late call is *rejected and
    // reported* instead of half-applied.
    //
    // Supporting it properly means a `mark_full` binding at `Page::new`, i.e.
    // making fonts dynamic again; see `Page::fonts`.

    /// Adds font import into UI. Call **before** [`Self::with_page`].
    pub fn with_font(mut self, import: FontImport) -> Self {
        if let Some(fonts) = self.fonts_before_build() {
            fonts.insert(import);
        }
        self
    }

    // TODO: Can we support reactive default?
    /// Sets the default font. Call **before** [`Self::with_page`].
    pub fn with_default_font(mut self, import: FontImport) -> Self {
        if let Some(fonts) = self.fonts_before_build() {
            fonts.set_default(import);
        }
        self
    }

    /// `&mut FontCtx` iff no page holds a clone of it yet. `Rc::get_mut`
    /// answers exactly that question — it succeeds only at refcount 1 — so this
    /// cannot report "fine" for a page that has already measured its text.
    fn fonts_before_build(&mut self) -> Option<&mut FontCtx> {
        let already_built = Rc::get_mut(&mut self.fonts).is_none();
        if already_built {
            log::warn!(
                "font set after a page was built — IGNORED. The page is \
                 already laid out with the previous fonts and nothing marks it \
                 dirty. Call `.with_font(..)`/`.with_default_font(..)` before \
                 `.with_page(..)`, which builds a page immediately."
            );
        }
        Rc::get_mut(&mut self.fonts)
    }
}

/// One-shot render (WS3.4, D7): build a [`UI`], lay it out, render a single
/// frame to `target`, then drop everything — reactive graph included — so the
/// heap returns to baseline. For static, e-paper-class displays that draw once
/// and never update: no `UI`, no signals, no arena kept alive afterward.
///
/// `build` must construct the whole `UI` *inside* the call so that every
/// reactive node it creates (the UI's own signals and every page node) is owned
/// by the one-shot scope and disposed when this returns. Returns whether the
/// frame drew (always `true` for the first frame).
///
/// ```no_run
/// # use rsact_ui::prelude::*;
/// # use rsact_ui::ui::{UI, render_once};
/// # fn sketch<D: RenderTarget<Color = NullColor>>(display: &mut D) {
/// // e-paper: compose the screen, flush it once, reclaim all the RAM.
/// let drew = render_once(
///     || {
///         UI::new((), NullRenderer::default())
///             .no_events()
///             .with_page((), || Label::new("Hello e-paper".inert()).into_el())
///     },
///     display,
/// );
/// # let _ = drew;
/// # }
/// ```
pub fn render_once<W, T>(
    build: impl FnOnce() -> UI<W, WithPages>,
    target: &mut T,
) -> bool
where
    W: WidgetCtx,
    T: RenderTarget,
    W::Renderer: FinishRender<T::Color>,
{
    // Everything `build` and the first render create lands in this scope.
    let scope = new_scope();
    let mut ui = build();
    let drew = ui.render(target);
    // Drop the UI first (its page's Drop disposes the arena + probes), then the
    // scope (disposes the UI's own signals and any page build-time nodes) —
    // leaving the runtime as it was before the call.
    drop(ui);
    drop(scope);
    drew
}

impl<W: WidgetCtx> UI<W, WithPages> {
    pub fn render<T: RenderTarget>(&mut self, target: &mut T) -> bool
    where
        W::Renderer: FinishRender<T::Color>,
    {
        let (page, renderer) = self.current_page_and_renderer();
        page.render(renderer, target)
    }

    /// WS6.4d: begin a **tiled frame** — plan it once, then paint and ship one
    /// region at a time.
    ///
    /// ```text
    /// let mut frame = ui.start_frame::<Tiles<240, 24>>();
    /// while let Some(region) = frame.next_region() {
    ///     frame.render(region)?;          // paint into the renderer's surface
    ///     let tile = frame.renderer().detach();
    ///     spi.write(tile).await;          // the IO is yours, always
    /// }
    /// ```
    ///
    /// Three properties this shape buys, and each is a constraint rather than a
    /// convenience:
    ///
    /// - **The IO is the caller's.** Nothing here awaits, blocks or owns a
    ///   transport; the loop above is the app's and every `.await` in it belongs
    ///   to the app. That is what lets one synchronous core serve blocking SPI,
    ///   polled DMA, Embassy and an RTIC ISR alike (roadmap 6.7).
    /// - **`tick()` mid-frame is a compile error.** The returned handle borrows
    ///   `&mut self` for the whole frame, so nothing can mutate the tree between
    ///   two regions of the same frame — where "region 3 paints a widget region
    ///   1 painted differently" is a tear no test would reliably catch.
    /// - **A surface too small for the policy does not compile.** The `const`
    ///   block below is WS6.4.0(iii)'s capacity proof: the policy's largest
    ///   region, converted to storage units by the renderer's own packing, must
    ///   fit [`Renderer::SURFACE_UNITS`]. Violating it is a
    ///   post-monomorphization error naming the concrete renderer and policy.
    ///
    /// The frame is **planned here, once**: one probe-gated [`Page::collect`]
    /// walk decides what changed, and the damage it records (plus anything a
    /// previous frame deferred) is planned into regions. Painting is then
    /// untracked and geometry-selected, so no probe can go clean halfway through
    /// a frame and leave the rest of the screen unpainted — the conflict that
    /// made tiling and probe-gated damage look mutually exclusive before WS6.4c
    /// separated "what changed" from "what does it look like there".
    ///
    /// [`Page::collect`]: crate::page::Page::collect
    /// [`Renderer::SURFACE_UNITS`]: rsact_render::renderer::Renderer::SURFACE_UNITS
    pub fn start_frame<P: FramePolicy>(&mut self) -> Frame<'_, W, P> {
        // WS6.4.0(iii). An inline `const` block, so this is evaluated at
        // monomorphization and the error names the instantiation:
        // `UI::<Wtf<EgTileRenderer<Rgb565, [u16; 5760]>, …>>::start_frame::<Tiles<240, 25>>`.
        const {
            assert_policy_fits::<P>(
                <W::Renderer as Renderer>::SURFACE_UNITS,
                <W::Renderer as Renderer>::SURFACE_PIXELS_PER_UNIT,
            )
        }

        let viewport = Rect::new(Point::zero(), self.viewport);

        // Plan the frame: one tracked, probe-gated walk that paints nothing.
        let (page, renderer) = self.current_page_and_renderer();
        page.collect(renderer);

        // Fold this frame's damage into whatever a previous frame deferred, and
        // plan the union. `active_page` and `deferred_regions` are disjoint
        // fields, which is what lets both be borrowed here.
        let deferred = &mut self.deferred_regions;
        if let Some(page) = self.active_page.as_ref() {
            page.with_damage(|rects| deferred.extend_from_slice(rects));
        }

        // `take` rather than a fresh `Vec`: the plan buffer is reused frame to
        // frame, and `plan_regions_into` clears it.
        let mut planned = core::mem::take(&mut self.frame_regions);
        plan_regions_into(
            &self.deferred_regions,
            viewport,
            &P::limits(
                self.viewport,
                <W::Renderer as Renderer>::SURFACE_PIXELS_PER_UNIT,
            ),
            &mut planned,
        );
        self.frame_regions = planned;
        self.deferred_regions.clear();

        Frame { ui: self, cursor: 0, policy: PhantomData }
    }

    /// Poll the current page's render gate **without** flushing to a display —
    /// the headless equivalent of [`Self::render`], used by the benches, the
    /// metrics probe and the size probe to drive a frame.
    ///
    /// WS5.0b: every one of those callers previously wrote
    /// `ui.current_page().use_renderer(…)`; with the renderer owned by `UI`
    /// they would each have to split the page/renderer borrow by hand, so the
    /// split lives here once instead.
    pub fn use_renderer(&mut self, f: impl FnOnce(&mut W::Renderer)) -> bool {
        let (page, renderer) = self.current_page_and_renderer();
        page.use_renderer(renderer, f)
    }

    /// The id of the page on top of the navigation history.
    fn current_page_id(&self) -> W::PageId {
        *self
            .page_history
            .last()
            .expect("Page history is empty, likely you forgot to add a page")
    }

    /// Build a fresh [`Page`] from its registered [`PageInitFn`].
    /// Each page gets its own arena so navigating away (dropping the page)
    /// frees its element tree.
    fn load_page(&self, id: W::PageId) -> Page<W> {
        let idx = self
            .pages
            .binary_search_by(|(k, _)| k.cmp(&id))
            .expect("Page not found, likely you forgot to add page to UI");
        let page_fn = &self.pages[idx].1;

        // The arena is created OUTSIDE the page scope: it keeps its explicit
        // WS2 disposal in `Page::drop`, so it must not also be scope-owned.
        let arena = create_signal(ElArena::new()).name("Page arena");

        // WS3.1 (G11): page-created = page-owned. Build the whole page — the
        // user's widgets (`init_page`) AND `Page::new`'s per-page nodes — with a
        // fresh scope current, and hand the still-alive handle to `Page::new`,
        // which `leave`s it (restoring the previous current scope so later work
        // isn't captured) and takes ownership. Dropping the page (goto
        // navigation frees the old page) disposes everything the page built,
        // killing the navigation leak and the disposed-arena delayed panic (a
        // `Dynamic` build effect no longer outlives its arena). Signals meant to
        // outlive a page must be created outside the `PageInitFn` — the contract.
        let scope = new_scope();
        Page::new(
            id,
            page_fn.init_page(),
            arena,
            self.viewport,
            // WS4.1: stylist is inline now (not a Copy node handle) — clone the
            // per-app config into the page (all stylists are Clone/Copy).
            self.stylist.clone(),
            self.dev_tools,
            // An `Rc` clone — a refcount bump, not a copy of the glyph data.
            // It is also what makes `Rc::get_mut` in `fonts_before_build` a
            // sound "has anything been built yet?" test.
            Rc::clone(&self.fonts),
            scope,
        )
    }

    /// Get mutable reference to currently active [`Page`]. You likely don't
    /// need to get pages.
    ///
    /// Lazily (re)builds the current page if it isn't the one already loaded.
    /// Assigning the freshly built page drops the previous one, disposing its
    /// arena.
    pub fn current_page(&mut self) -> &mut Page<W> {
        self.current_page_and_renderer().0
    }

    /// The current page **and** the renderer, as two disjoint mutable borrows.
    ///
    /// WS5.0b: rendering needs both at once, and `current_page` alone borrows
    /// all of `self`. Splitting the borrow here (rather than at each call site)
    /// keeps the lazy page-build in one place; borrowck accepts it because the
    /// two are distinct fields.
    pub fn current_page_and_renderer(
        &mut self,
    ) -> (&mut Page<W>, &mut W::Renderer) {
        let current_id = self.current_page_id();

        let needs_load = self
            .active_page
            .as_ref()
            .map_or(true, |page| page.id() != current_id);

        if needs_load {
            let page = self.load_page(current_id);
            self.active_page = Some(page);
        }

        (
            self.active_page
                .as_mut()
                .expect("Active page must be initialized"),
            &mut self.renderer,
        )
    }

    // TODO: Unused
    // pub fn page(&mut self, id: W::PageId) -> &mut Page<W> {
    //     self.pages.get_mut(&id).unwrap()
    // }

    /// Run some logic on page change.
    /// Building/loading of the now-current page happens lazily inside
    /// [`Self::current_page`], which is invoked here.
    fn on_page_change(&mut self) {
        info!("UI: Page changed to {:?}", self.current_page_id());
        let (page, renderer) = self.current_page_and_renderer();
        // WS6.4.0(iv): `clear` only. The `.force_redraw()` that used to follow it
        // was residue from stored pages, and BOTH of the things it did are now
        // redundant here, because `current_page_and_renderer` above BUILDS a
        // fresh `Page` on a change:
        //
        //   - its invalidation broadcast: every probe in a fresh page is newborn
        //     and therefore dirty, so the whole tree renders on the first poll
        //     regardless. (When pages persisted, their probes came back CLEAN
        //     from the previous visit and the broadcast was load-bearing.)
        //   - its whole-viewport flush: `Page::new`'s first-build relayout is
        //     always blanket, so it sets `full_flush` itself.
        //
        // `clear` is still needed, for the framebuffer rather than the flush: the
        // framebuffer is one `UI`-owned value outliving every page, so without it
        // the previous page's pixels stay under anything the new page's widgets
        // do not paint — and a `Flex` root paints nothing at all.
        //
        // `page_change_flushes_the_whole_viewport` (in this module's tests) pins
        // the observable end of this, which had NO coverage before.
        page.clear(renderer);

        // TODO
        // if self.options.auto_focus {
        //     self.current_page().apply_auto_focus();
        // }
    }

    // TODO: Should be public?
    // TODO: Browser-like history with preserved next pages and overwrites
    pub fn previous_page(&mut self) -> bool {
        if self.page_history.len() > 1 {
            self.page_history.pop();
            self.on_page_change();
            true
        } else {
            false
        }
    }

    // TODO: Should be public? We have MessageQueue
    pub fn goto(&mut self, page_id: W::PageId) {
        // Bound the back-stack so repeated forward navigation on a long-running
        // device can't grow it without limit. Drop the oldest entry past the
        // cap (deep-enough back history for any realistic UI).
        const MAX_PAGE_HISTORY: usize = 32;
        if self.page_history.len() >= MAX_PAGE_HISTORY {
            self.page_history.remove(0);
        }
        self.page_history.push(page_id);
        self.on_page_change();
    }

    /// Helper that's utilizing [`std::time::SystemTime`] for time ticks
    #[cfg(feature = "std")]
    pub fn tick_time_std(&mut self) -> &mut Self {
        // Use static start time to avoid time wrapping soon.
        thread_local! {
            static START_TIME: std::cell::LazyCell<u128> =
                std::cell::LazyCell::new(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis()
                });
        }

        let start_time = START_TIME.with(|start_time| **start_time);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        // Wrap into the full u32 range (2^32 values). `% u32::MAX` is one short
        // of that, so the clock wrapped a tick early and never produced
        // `u32::MAX` itself — a subtle drift the anim wrap-handling relies on.
        let now = (now - start_time) % (u32::MAX as u128 + 1);

        self.tick_time(now as u32)
    }

    pub fn tick_time(&mut self, now_millis: u32) -> &mut Self {
        self.message_queue
            .as_mut()
            .map(|queue| queue.tick(now_millis));

        self
    }

    pub fn tick(
        &mut self,
        events: impl Iterator<Item = Event<W::CustomEvent>>,
    ) -> Vec<UnhandledEvent<W>> {
        let unhandled = self
            .current_page()
            .handle_events(events)
            .into_iter()
            .filter_map(|unhandled| {
                let UnhandledEvent::Event(event) = unhandled;

                if let Event::DevTools(dt_event) = &event {
                    let dev_tools_state_changed = self.dev_tools.update(|dt| {
                        info!("DevTools event: {:?}", dt_event);
                        let was_enabled = dt.enabled;
                        dt.enabled = match dt_event {
                            crate::event::DevToolsEvent::Activate => true,
                            crate::event::DevToolsEvent::Deactivate => false,
                            crate::event::DevToolsEvent::Toggle => !dt.enabled,
                        };
                        was_enabled != dt.enabled
                    });

                    if dev_tools_state_changed {
                        info!("DevTools state changed, forcing redraw");
                        self.current_page().force_redraw();
                    }

                    return None;
                }

                if let (Some(on_exit), Event::Exit) =
                    (self.on_exit.as_ref(), &event)
                {
                    info!("Exit event received, calling on_exit handler and exiting");
                    on_exit();
                    return None;
                }

                info!("Unhandled event: {:?}", event);

                Some(UnhandledEvent::Event(event))
            })
            .collect();

        // TODO: Dilemma: Should messages be processed before or after events?
        // I think after, because message can change page.

        while let Some(msg) = self.message_queue.map(|q| q.pop()).flatten() {
            match msg {
                UiMessage::GoTo(page_id) => {
                    info!("UI message: Go to page {:?}", page_id);
                    self.goto(page_id)
                },
                UiMessage::PreviousPage => {
                    info!("UI message: Go to previous page");
                    self.previous_page();
                },
            }
        }

        unhandled
    }

    // pub fn draw_buffer(
    //     &mut self,
    //     f: impl Fn(&[<<W as WidgetCtx>::Color as PackedColor>::Storage]),
    // ) -> bool {
    //     self.current_page().draw_buffer(f)
    // }

    // pub fn draw_with_renderer(&mut self, f: impl FnOnce(&W::Renderer)) ->
    // bool {     self.current_page().use_renderer(f)
    // }
}

#[cfg(test)]
mod tests {
    use super::{UI, render_once};
    use crate::prelude::*;
    use rsact_reactive::{
        leak::{leak_report, leak_snapshot},
        runtime::with_new_runtime,
    };

    /// WS6.4.0(iv): navigating to another page must flush the WHOLE viewport.
    ///
    /// There was **no page-navigation coverage at all** before this, which is why
    /// dropping `force_redraw()` from `on_page_change` had to be argued from
    /// first principles. The argument, now pinned here:
    ///
    /// - the broadcast half of `force_redraw()` was residue from stored pages.
    ///   `current_page_and_renderer` BUILDS a fresh `Page` on a change, so every
    ///   probe is newborn and therefore dirty — the tree renders regardless.
    /// - the whole-viewport FLUSH is not residue. The framebuffer is one
    ///   `UI`-owned value outliving every page, and a `Flex` root's `render` is a
    ///   no-op, so container padding and inter-child gaps are painted by no widget
    ///   and would keep showing the previous page. `Page::clear` therefore
    ///   declares the repaint it performs, and this asserts that it does.
    #[test]
    fn page_change_flushes_the_whole_viewport() {
        use rsact_render::{record::RecordingRenderer, renderer::NullColor};

        with_new_runtime(|_| {
            let viewport = Size::new_equal(64);
            let full = Rect::new(Point::zero(), viewport);
            type RecWtf =
                crate::el::ctx::Wtf<RecordingRenderer<NullColor>, u8, (), ()>;

            let mut ui: UI<RecWtf, _> =
                UI::new((), RecordingRenderer::<NullColor>::new(viewport))
                    .with_page(0u8, || Label::new("a".inert()).into_el())
                    .with_page(1u8, || Label::new("b".inert()).into_el());

            // Settle page 0: the first frame is a full invalidate, so render
            // until the damage set stops covering everything.
            for _ in 0..6 {
                ui.use_renderer(|_| {});
            }
            assert!(
                ui.current_page().damage_snapshot() != vec![full],
                "page 0 must have settled to something other than a full flush"
            );

            ui.goto(1u8);
            ui.use_renderer(|_| {});

            let d = ui.current_page().damage_snapshot();
            assert_eq!(
                d,
                vec![full],
                "a page change must flush the whole viewport, or the previous \
                 page shows through wherever the new one paints nothing"
            );
        });
    }

    /// WS6.4d: the tiled frame driver must reconstruct the frame the
    /// full-framebuffer path would have painted.
    ///
    /// This is the *end-to-end* version of what WS6.4a's harness checks on
    /// synthetic schedules: it goes through the real public entry point
    /// ([`UI::start_frame`] → `next_region` → `render`), on a real `UI` with a
    /// real page, and the schedule is the planner's own — not one the test
    /// invented. `tile_invariance` is the assertion: every op a region is
    /// obliged to draw appears in that region's log, and no region draws
    /// geometry the full frame never produced.
    #[test]
    fn a_tiled_frame_reconstructs_the_full_frame() {
        use rsact_render::{
            record::RecordingRenderer,
            region::Tiles,
            renderer::NullColor,
            schedule::{ScheduleLog, TilePass, tile_invariance},
        };

        with_new_runtime(|_| {
            let viewport = Size::new_equal(64);
            type RecWtf =
                crate::el::ctx::Wtf<RecordingRenderer<NullColor>, u8, (), ()>;

            let renderer = RecordingRenderer::<NullColor>::new(viewport);
            let recorder = renderer.clone();
            let mut ui: UI<RecWtf, _> =
                UI::new((), renderer).with_page(0u8, || {
                    Flex::col(vec![
                        Label::new("alpha".inert()).into_el(),
                        Checkbox::new(true).into_el(),
                        Label::new("omega".inert()).into_el(),
                    ])
                    .fill()
                    .gap(4u32)
                    .into_el()
                });

            // Settle: the first frames are full invalidates while reactive
            // state stabilises (same warm-up the WS6.9 goldens use).
            for _ in 0..6 {
                ui.use_renderer(|_| {});
            }

            // The reference: one forced full-viewport frame.
            recorder.clear();
            ui.current_page().force_redraw();
            ui.use_renderer(|_| {});
            let full = recorder.ops();
            assert!(!full.is_empty(), "the reference frame drew nothing");

            // The same frame, tiled. 64x16 is four bands on this viewport, so
            // the plan is the degenerate strip case — which is exactly the one
            // where every widget is cut by some boundary.
            ui.current_page().force_redraw();
            let mut passes = Vec::new();
            {
                let mut frame = ui.start_frame::<Tiles<64, 16>>();
                assert!(
                    frame.regions() > 1,
                    "a forced full redraw on a 64x64 viewport must plan more \
                     than one 64x16 region, got {}",
                    frame.regions()
                );
                while let Some(region) = frame.next_region() {
                    recorder.clear();
                    frame.render(region).expect("paint_region failed");
                    passes.push(TilePass { tile: region, ops: recorder.ops() });
                }
            }

            let violations = tile_invariance(&ScheduleLog { full, passes });
            assert!(
                violations.is_empty(),
                "{} violation(s), first: {}",
                violations.len(),
                violations[0]
            );
        });
    }

    /// WS6.4d: dropping a frame with regions left must **defer** them, not lose
    /// them.
    ///
    /// An unpainted region is a stale rectangle on the display, and nothing
    /// would damage it again until whatever is underneath happens to change —
    /// so it would sit there, wrong, indefinitely. The next frame must therefore
    /// re-plan it even though nothing in the tree changed in between (roadmap
    /// 6.7: "defer + union-coalesce, never abort").
    #[test]
    fn an_abandoned_frame_defers_its_regions() {
        use rsact_render::{
            record::RecordingRenderer, region::Tiles, renderer::NullColor,
        };

        with_new_runtime(|_| {
            let viewport = Size::new_equal(64);
            type RecWtf =
                crate::el::ctx::Wtf<RecordingRenderer<NullColor>, u8, (), ()>;

            let mut ui: UI<RecWtf, _> =
                UI::new((), RecordingRenderer::<NullColor>::new(viewport))
                    .with_page(0u8, || Label::new("abandon".inert()).into_el());

            for _ in 0..6 {
                ui.use_renderer(|_| {});
            }

            ui.current_page().force_redraw();
            let planned = {
                let mut frame = ui.start_frame::<Tiles<64, 16>>();
                let planned = frame.regions();
                assert!(planned >= 2, "need a multi-region frame to abandon");
                let first = frame.next_region().unwrap();
                frame.render(first).expect("paint_region failed");
                planned
                // dropped here with `planned - 1` regions unpainted
            };

            // Nothing changed in between, so anything this frame plans came
            // from the deferral.
            let frame = ui.start_frame::<Tiles<64, 16>>();
            assert_eq!(
                frame.regions(),
                planned - 1,
                "the abandoned regions were lost — the screen would keep \
                 showing whatever was there"
            );
        });
    }

    /// The other half of that contract: a frame with nothing to do plans
    /// nothing, so the driver's loop body never runs and no region is
    /// transferred.
    ///
    /// Worth pinning separately because it is what makes the deferral test
    /// above meaningful — if a settled page planned regions anyway, that test
    /// would pass for the wrong reason.
    #[test]
    fn a_settled_page_plans_no_regions() {
        use rsact_render::{
            record::RecordingRenderer, region::Tiles, renderer::NullColor,
        };

        with_new_runtime(|_| {
            let viewport = Size::new_equal(64);
            type RecWtf =
                crate::el::ctx::Wtf<RecordingRenderer<NullColor>, u8, (), ()>;

            let mut ui: UI<RecWtf, _> =
                UI::new((), RecordingRenderer::<NullColor>::new(viewport))
                    .with_page(0u8, || Label::new("settled".inert()).into_el());

            for _ in 0..6 {
                ui.use_renderer(|_| {});
            }
            // Drain any frame the warm-up left planned.
            while ui.start_frame::<Tiles<64, 16>>().regions() > 0 {
                let mut frame = ui.start_frame::<Tiles<64, 16>>();
                while let Some(region) = frame.next_region() {
                    frame.render(region).expect("paint_region failed");
                }
            }

            let frame = ui.start_frame::<Tiles<64, 16>>();
            assert_eq!(
                frame.regions(),
                0,
                "an idle frame planned regions; every one of them is a wasted \
                 tree walk and a wasted transfer"
            );
        });
    }

    /// WS3.4: `render_once` builds, lays out and renders a single frame, then
    /// drops the whole UI + reactive graph — the runtime node population must
    /// return to exactly what it was before the call (nothing kept alive for a
    /// display that never updates again).
    #[test]
    fn render_once_returns_heap_to_baseline() {
        with_new_runtime(|_| {
            let snap = leak_snapshot();

            // Explicit colour: `NullRenderer` is generic since WS6.4.0(ii-4),
            // and a bare `default()` in a `&mut _` argument position has nothing
            // to infer `C` from.
            let mut target = NullRenderer::<NullColor>::default();
            let drew = render_once(
                || {
                    UI::new((), NullRenderer::default())
                        .no_events()
                        .with_page((), || Label::new("x".inert()).into_el())
                },
                &mut target,
            );

            assert!(drew, "render_once must draw the first frame");

            let report = leak_report(&snap);
            assert!(
                report.is_empty(),
                "render_once leaked {} node(s): {report}",
                report.len()
            );
        });
    }
}
