use crate::{
    el::{El, arena::ElArena, ctx::*},
    event::{
        Event, UnhandledEvent,
        message::{UiMessage, UiQueue},
    },
    font::{FontCtx, FontImport},
    page::{Page, dev::DevTools, id::PageId},
    render::prelude::*,
    style::{stylist::InternalStylist, theme::Theme},
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use core::{fmt::Debug, marker::PhantomData};
use log::info;
use rsact_reactive::prelude::*;
use tinyvec::TinyVec;

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
    T: Into<El<W>>,
{
    fn init_page(&self) -> El<W> {
        (self)().into()
    }
}

pub struct UI<W: WidgetCtx, P: HasPages> {
    page_history: TinyVec<[W::PageId; 1]>,
    pages: BTreeMap<W::PageId, Box<dyn PageInitFn<W>>>,
    /// Currently active page. Lazily (re)built from the corresponding
    /// [`PageInitFn`] on navigation. Only the active page is kept built; each
    /// page owns its own arena, so dropping it (on navigation) frees its tree.
    active_page: Option<Page<W>>,
    viewport: MaybeReactive<Size>,
    on_exit: Option<Box<dyn Fn()>>,
    // TODO: Get rid of Inert wrapper, it is at most RefCell
    stylist: Inert<W::Stylist>,
    dev_tools: Signal<DevTools>,
    // TODO: Inert renderer. I don't think it is hardly needed to have reactive
    // renderer options (this is the only reactive dependency). The problem
    // is that Inert is a readonly value, while we need a mutable reference to
    // the renderer
    renderer: Signal<W::Renderer>,
    message_queue: Option<UiQueue<W>>,
    options: UiOptions,
    has_pages: PhantomData<P>,
    fonts: Signal<FontCtx>,
}

impl<R, I, S, E> UI<Wtf<R, I, S, E>, NoPages>
where
    R: Renderer + 'static,
    I: PageId + 'static,
    S: InternalStylist<R::Color> + 'static,
    E: Debug + 'static,
{
    // TODO: For now I made viewport inert, but it is possible for the viewport
    // to change (e.g. window resize, etc). But as now we targeting embedded
    // devices with fixed displays and don't support any windowing, I hold it.
    pub fn new(stylist: S, renderer: R) -> Self {
        let viewport = renderer.size().inert().maybe_reactive();

        let dev_tools =
            create_signal(DevTools { enabled: false, hovered: None });

        let fonts = create_signal(FontCtx::new());

        Self {
            page_history: Default::default(),
            viewport,
            pages: BTreeMap::new(),
            active_page: None,
            on_exit: None,
            stylist: stylist.inert(),
            dev_tools,
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
        assert!(self.pages.insert(id, Box::new(page_fn)).is_none())
    }

    // Fonts //
    /// Adds font import into UI.
    pub fn with_font(mut self, import: FontImport) -> Self {
        self.fonts.update(|fonts| fonts.insert(import));
        self
    }

    // TODO: Can we support reactive default?
    pub fn with_default_font(mut self, import: FontImport) -> Self {
        self.fonts.update(|fonts| {
            fonts.set_default(import);
        });
        self
    }
}

impl<W: WidgetCtx> UI<W, WithPages> {
    pub fn render<T: RenderTarget>(&mut self, target: &mut T) -> bool
    where
        W::Renderer: FinishRender<T::Color>,
    {
        self.current_page().render(target)
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
        let page_fn = self
            .pages
            .get(&id)
            .expect("Page not found, likely you forgot to add page to UI");

        let arena = create_signal(ElArena::new()).name("Page arena");

        Page::new(
            id,
            page_fn.init_page(),
            arena,
            self.viewport,
            self.stylist,
            self.dev_tools,
            self.renderer,
            self.fonts,
        )
    }

    /// Get mutable reference to currently active [`Page`]. You likely don't
    /// need to get pages.
    ///
    /// Lazily (re)builds the current page if it isn't the one already loaded.
    /// Assigning the freshly built page drops the previous one, disposing its
    /// arena.
    pub fn current_page(&mut self) -> &mut Page<W> {
        let current_id = self.current_page_id();

        let needs_load = self
            .active_page
            .as_ref()
            .map_or(true, |page| page.id() != current_id);

        if needs_load {
            let page = self.load_page(current_id);
            self.active_page = Some(page);
        }

        self.active_page.as_mut().expect("Active page must be initialized")
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
        self.current_page().clear().force_redraw();

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
