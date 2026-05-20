#[cfg(feature = "embedded-graphics")]
use crate::font::FontImport;
use crate::{
    el::El,
    event::{
        Event, UnhandledEvent,
        message::{UiMessage, UiQueue},
    },
    font::FontCtx,
    page::{Page, dev::DevTools, id::PageId},
    render::prelude::*,
    style::theme::Theme,
    widget::ctx::*,
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use core::{fmt::Debug, marker::PhantomData};
use log::info;
use rsact_reactive::prelude::*;
use tinyvec::TinyVec;

#[cfg(feature = "embedded-graphics")]
use embedded_graphics::prelude::DrawTarget;

pub struct UiOptions {
    auto_focus: bool,
    // TODO: Event interpretation
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

pub struct UI<W: WidgetCtx, P: HasPages> {
    page_history: TinyVec<[W::PageId; 1]>,
    pages: BTreeMap<W::PageId, Page<W>>,
    viewport: MaybeReactive<Size>,
    on_exit: Option<Box<dyn Fn()>>,
    // Note: Theme is Inert by design as we don't still support dynamic themes. But do be noted Inert make data 'static and only freed on reactive scope drop, so it is better to somehow run UI inside new reactive runtime, but this requires user signals to be created inside it.
    theme: Inert<Theme<W::Color>>,
    dev_tools: Signal<DevTools>,
    // TODO: Inert renderer. I don't think it is hardly needed to have reactive renderer options (this is the only reactive dependency).
    renderer: Signal<W::Renderer>,
    message_queue: Option<UiQueue<W>>,
    options: UiOptions,
    has_pages: PhantomData<P>,
    fonts: Signal<FontCtx>,
}

// LayeringRenderer is DrawTarget layering wrapper which is the only Renderer supported for now.
impl<W: WidgetCtx> UI<W, WithPages> {
    // TODO: Move `MapColor` mapping to separate drawing variant to avoid specifying generic for `C`
    #[cfg(feature = "embedded-graphics")]
    pub fn render(
        &mut self,
        target: &mut impl DrawTarget<Color = W::Color>,
    ) -> bool
    where
        W::Renderer: embedded_graphics::Drawable<Color = W::Color, Output = ()>,
    {
        self.current_page().render(target)
    }
}

#[cfg(not(feature = "embedded-graphics"))]
impl<R, I, E> UI<Wtf<R, I, E>, NoPages>
where
    R: Renderer + 'static,
    I: PageId + 'static,
    E: Debug + 'static,
{
    fn new(
        viewport: MaybeReactive<Size>,
        theme: Theme<<R as Renderer>::Color>,
        renderer: R,
    ) -> Self {
        let dev_tools =
            create_signal(DevTools { enabled: false, hovered: None });

        let fonts = create_signal(FontCtx::new());

        Self {
            page_history: Default::default(),
            viewport,
            pages: BTreeMap::new(),
            on_exit: None,
            theme: theme.inert(),
            dev_tools,
            // TODO: Reactive viewport in Renderer
            renderer: renderer.signal(),
            message_queue: None,
            options: Default::default(),
            has_pages: PhantomData,
            fonts,
        }
    }

    pub fn auto_focus(mut self) -> Self {
        self.options.auto_focus = true;
        self
    }
}

#[cfg(feature = "embedded-graphics")]
impl<R, I, E> UI<Wtf<R, I, E>, NoPages>
where
    R: Renderer
        + embedded_graphics::prelude::DrawTarget<
            Color = <R as Renderer>::Color,
            Error = (),
        > + 'static,
    <R as Renderer>::Color: embedded_graphics::prelude::PixelColor,
    I: PageId + 'static,
    E: Debug + 'static,
{
    pub(crate) fn new(
        viewport: MaybeReactive<Size>,
        theme: Theme<<R as Renderer>::Color>,
        renderer: R,
    ) -> Self {
        let dev_tools =
            create_signal(DevTools { enabled: false, hovered: None });

        let fonts = create_signal(FontCtx::new());

        Self {
            page_history: Default::default(),
            viewport,
            pages: BTreeMap::new(),
            on_exit: None,
            theme: theme.inert(),
            dev_tools,
            // TODO: Reactive viewport in Renderer
            renderer: renderer.signal(),
            message_queue: None,
            options: Default::default(),
            has_pages: PhantomData,
            fonts,
        }
    }

    pub fn auto_focus(mut self) -> Self {
        self.options.auto_focus = true;
        self
    }
}

impl<W: WidgetCtx, P: HasPages> UI<W, P> {
    /// Hinting method to avoid specifying generics but just set [`WidgetCtx::Event`] to [`NullEvent`]
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

    /// Set rendering options
    pub fn with_renderer_options(
        mut self,
        options: impl IntoMaybeReactive<<W::Renderer as Renderer>::Options>,
    ) -> Self {
        self.renderer.setter(options.maybe_reactive(), |renderer, options| {
            renderer.set_options(options.clone().into())
        });
        self
    }

    /// Set [`MessageQueue`] for UI, that will be used for animations and UI messages
    pub fn with_queue(mut self, queue: UiQueue<W>) -> Self {
        self.message_queue = Some(queue);
        self
    }

    // TODO: Type guard for SinglePage to disallow adding new pages.
    /// Adds page to the UI.
    /// The first added page becomes intro page
    pub fn with_page(
        mut self,
        id: W::PageId,
        page_root: impl Into<El<W>>,
    ) -> UI<W, WithPages> {
        self.add_page(id, page_root);

        let mut with_page = UI {
            page_history: self.page_history,
            pages: self.pages,
            viewport: self.viewport,
            on_exit: self.on_exit,
            theme: self.theme,
            dev_tools: self.dev_tools,
            renderer: self.renderer,
            message_queue: self.message_queue,
            options: self.options,
            has_pages: PhantomData,
            fonts: self.fonts,
        };

        // Go to page if it is the first one
        if with_page.pages.len() == 1 {
            with_page.goto(id);
        }

        with_page
    }

    fn add_page(&mut self, id: W::PageId, page_root: impl Into<El<W>>) {
        assert!(
            self.pages
                .insert(
                    id,
                    Page::new(
                        id,
                        page_root,
                        self.viewport,
                        self.theme,
                        self.dev_tools,
                        self.renderer,
                        self.fonts
                    )
                )
                .is_none()
        )
    }

    // Fonts //
    /// Adds font import into UI.
    #[cfg(feature = "embedded-graphics")]
    pub fn with_font(mut self, import: FontImport) -> Self {
        self.fonts.update(|fonts| fonts.insert(import));
        self
    }

    // TODO: Can we support reactive default?
    #[cfg(feature = "embedded-graphics")]
    pub fn with_default_font(mut self, import: FontImport) -> Self {
        self.fonts.update(|fonts| {
            fonts.set_default(import);
        });
        self
    }
}

impl<W: WidgetCtx> UI<W, WithPages> {
    /// Get mutable reference to currently active [`Page`]. You likely don't need to get pages.
    pub fn current_page(&mut self) -> &mut Page<W> {
        self.pages
            .get_mut(&self.page_history.last().unwrap())
            .expect("Page not found, likely you forget to add page to UI")
    }

    // TODO: Unused
    // pub fn page(&mut self, id: W::PageId) -> &mut Page<W> {
    //     self.pages.get_mut(&id).unwrap()
    // }

    /// Run some logic on page change
    fn on_page_change(&mut self) {
        info!("UI: Page changed to {:?}", self.page_history.last().unwrap());
        self.current_page().clear().force_redraw();

        if self.options.auto_focus {
            self.current_page().apply_auto_focus();
        }
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

        let now = (now - start_time) % u32::MAX as u128;

        self.tick_time(now as u32)
    }

    pub fn tick_time(&mut self, now_millis: u32) -> &mut Self {
        self.message_queue.as_mut().map(|queue| queue.tick(now_millis));

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
                    self.dev_tools.update(|dt| {
                        dt.enabled = match dt_event {
                            crate::event::DevToolsEvent::Activate => true,
                            crate::event::DevToolsEvent::Deactivate => false,
                            crate::event::DevToolsEvent::Toggle => !dt.enabled,
                        }
                    });
                    return None;
                }

                if let (Some(on_exit), Event::Exit) =
                    (self.on_exit.as_ref(), &event)
                {
                    on_exit();
                    return None;
                }

                Some(UnhandledEvent::Event(event))
            })
            .collect();

        // TODO: Dilemma: Should messages be processed before or after events?
        // I think after, because message can change page.

        while let Some(msg) = self.message_queue.map(|q| q.pop()).flatten() {
            match msg {
                UiMessage::GoTo(page_id) => self.goto(page_id),
                UiMessage::PreviousPage => {
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

    // pub fn draw_with_renderer(&mut self, f: impl FnOnce(&W::Renderer)) -> bool {
    //     self.current_page().use_renderer(f)
    // }
}
