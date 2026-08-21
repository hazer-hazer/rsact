//! Shared, `#[doc(hidden)]` test/bench scaffolding (WS0.7j). Both the
//! `metrics-probe` tool and the `layout` bench built the same headless N-label
//! page independently; a single builder here keeps them from drifting so their
//! numbers stay comparable. Not part of the public API.

pub mod tile_probe;

use crate::{
    el::ctx::{WidgetCtx, Wtf},
    page::Page,
    prelude::*,
    ui::{Frame, UI, WithPages},
};
use alloc::{format, string::String, vec::Vec};

/// The headless widget context: no-op renderer, unit page-id / stylist / event.
pub type NullWtf = Wtf<NullRenderer, (), (), ()>;

/// Build a headless page of `n` labels, each bound to a signal, and return the
/// built `UI` plus the label signals (so a caller can dirty one to force a
/// relayout). The active page's arena + layout tree are built before returning.
pub fn labels_page(n: usize) -> (UI<NullWtf, WithPages>, Vec<Signal<String>>) {
    let labels: Vec<Signal<String>> = (0..n)
        .map(|i| create_signal(format!("label {i}")))
        .collect();
    let init = labels.clone();
    let mut ui: UI<NullWtf, _> =
        UI::new((), Size::zero()).with_page((), move || {
            Flex::col(
                init.iter()
                    .map(|s| Label::new(*s).into_el())
                    .collect::<Vec<_>>(),
            )
            .into_el()
        });
    let _ = ui.current_page();
    (ui, labels)
}

/// Build a headless page of `n` buttons in a column, each wrapping a label
/// bound to a signal (button-heavy: `n` `ButtonBuilder -> Button` transforms,
/// WS13.2 Task 5). Returns the built `UI` plus the label signals, so a caller
/// can dirty one to force a relayout.
pub fn buttons_page(n: usize) -> (UI<NullWtf, WithPages>, Vec<Signal<String>>) {
    let labels: Vec<Signal<String>> = (0..n)
        .map(|i| create_signal(format!("button {i}")))
        .collect();
    let init = labels.clone();
    let mut ui: UI<NullWtf, _> =
        UI::new((), Size::zero()).with_page((), move || {
            Flex::col(
                init.iter()
                    .map(|s| Button::new(Label::new(*s)).into_el())
                    .collect::<Vec<_>>(),
            )
            .into_el()
        });
    let _ = ui.current_page();
    (ui, labels)
}

/// Build a headless page of `n` nested `Flex` containers (flex-heavy: `n + 1`
/// `FlexBuilder -> Flex` transforms — one outer column plus `n` inner rows,
/// each wrapping one label bound to a signal, WS13.2 Task 5). Returns the
/// built `UI` plus the label signals, so a caller can dirty one to force a
/// relayout.
pub fn nested_flex_page(
    n: usize,
) -> (UI<NullWtf, WithPages>, Vec<Signal<String>>) {
    let labels: Vec<Signal<String>> = (0..n)
        .map(|i| create_signal(format!("nested {i}")))
        .collect();
    let init = labels.clone();
    let mut ui: UI<NullWtf, _> =
        UI::new((), Size::zero()).with_page((), move || {
            Flex::col(
                init.iter()
                    .map(|s| Flex::row([Label::new(*s).into_el()]).into_el())
                    .collect::<Vec<_>>(),
            )
            .into_el()
        });
    let _ = ui.current_page();
    (ui, labels)
}

/// A [`Page`] bundled with the renderer it draws into (WS5.0b).
///
/// The renderer is owned by [`UI`] now, not by the page, so a page-level test —
/// which builds a bare `Page` without a `UI` — has to own one itself and lend it
/// to every render call. This wrapper keeps that plumbing in one place instead
/// of threading a `&mut renderer` argument through every assertion.
///
/// It `Deref`s to the page, so all non-rendering methods (`handle_events`,
/// `take_draw_calls`, `force_redraw`, …) work unchanged; the inherent
/// [`use_renderer`](Self::use_renderer) and [`clear`](Self::clear) shadow the
/// page's, supplying the renderer automatically (inherent methods win over
/// `Deref`).
pub struct TestPage<W: WidgetCtx> {
    pub page: Page<W>,
    pub renderer: W::Renderer,
}

impl<W: WidgetCtx> TestPage<W> {
    pub fn new(page: Page<W>, renderer: W::Renderer) -> Self {
        Self { page, renderer }
    }

    /// Poll the page's render gate, lending it the owned renderer.
    pub fn use_renderer(&mut self, f: impl FnOnce(&mut W::Renderer)) -> bool {
        self.page.use_renderer(&mut self.renderer, f)
    }

    /// WS6.4c: plan the frame, lending it the owned renderer.
    pub fn collect(&mut self) -> bool {
        self.page.collect(&mut self.renderer)
    }

    /// WS6.4c: paint one region, lending it the owned renderer.
    pub fn paint_region(&mut self, region: Rect) -> RenderResult {
        self.page.paint_region(&mut self.renderer, region)
    }
}

impl<W: WidgetCtx> core::ops::Deref for TestPage<W> {
    type Target = Page<W>;

    fn deref(&self) -> &Self::Target {
        &self.page
    }
}

impl<W: WidgetCtx> core::ops::DerefMut for TestPage<W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.page
    }
}

/// A [`UI`] bundled with the renderer it draws into (WS6.4d).
///
/// rsact no longer owns a renderer — the application lends one to each render
/// call — but a *test* is the application, and threading `&mut renderer` through
/// every assertion in a suite whose subject is something else entirely (layout
/// counts, damage rects, probe wake-ups) is noise. This owns the pair and
/// shadows the render entry points to supply it, exactly as [`TestPage`] does
/// one level down.
///
/// It `Deref`s to the `UI`, so navigation, events and page access work
/// unchanged. Production code should **not** grow an equivalent: the point of
/// the ownership split is that the application decides when a surface is
/// attached, and a bundle that hides the renderer hides that decision too.
pub struct TestUi<W: WidgetCtx> {
    pub ui: UI<W, WithPages>,
    pub renderer: W::Renderer,
}

impl<W: WidgetCtx> TestUi<W> {
    pub fn new(ui: UI<W, WithPages>, renderer: W::Renderer) -> Self {
        Self { ui, renderer }
    }

    /// Drive one complete frame — every planned region — and report whether
    /// anything was painted.
    ///
    /// The real path, not a shortcut around it: [`Frame`] is the only render API
    /// (WS6.4d), so a test that wants "one frame" runs its loop to completion.
    /// A test needing per-region control uses [`Self::frame`] instead.
    pub fn render(&mut self) -> bool {
        let Self { ui, renderer } = self;
        let mut frame = ui.start_frame(renderer);
        let mut painted = false;
        while frame.render(renderer).is_some() {
            painted = true;
        }
        painted
    }

    /// Begin a tiled frame, returning it **with** the renderer to paint into.
    ///
    /// Both, because [`Frame::render`] needs the renderer per region and a
    /// `Frame` borrowing all of `self` would put `self.renderer` out of reach.
    /// The two borrows are disjoint fields, which is exactly the shape a real
    /// caller has for free — there `ui` and `renderer` are separate locals.
    ///
    /// ```ignore
    /// let (mut frame, renderer) = ui.frame();
    /// while let Some(region) = frame.render(renderer) { … }
    /// ```
    pub fn frame(&mut self) -> (Frame<'_, W>, &mut W::Renderer) {
        let Self { ui, renderer } = self;
        let frame = ui.start_frame(renderer);
        (frame, renderer)
    }
}

impl<W: WidgetCtx> core::ops::Deref for TestUi<W> {
    type Target = UI<W, WithPages>;

    fn deref(&self) -> &Self::Target {
        &self.ui
    }
}

impl<W: WidgetCtx> core::ops::DerefMut for TestUi<W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ui
    }
}
