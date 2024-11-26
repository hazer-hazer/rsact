use crate::{
    el::El,
    event::{
        dev::DevToolsToggle,
        message::{UiMessage, UiQueue},
        Event, ExitEvent as _, NullEvent, UnhandledEvent,
    },
    layout::size::Size,
    page::{dev::DevTools, id::PageId, Page},
    render::{color::Color, draw_target::LayeringRenderer, Renderer},
    widget::{WidgetCtx, Wtf},
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use core::marker::PhantomData;
use embedded_graphics::prelude::DrawTarget;
use rsact_reactive::prelude::*;
use smallvec::SmallVec;

pub struct UiOptions {
    auto_focus: bool,
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
    page_history: SmallVec<[W::PageId; 1]>,
    pages: BTreeMap<W::PageId, Page<W>>,
    viewport: Memo<Size>,
    on_exit: Option<Box<dyn Fn()>>,
    styler: Memo<W::Styler>,
    dev_tools: Signal<DevTools>,
    renderer: Signal<W::Renderer>,
    message_queue: Option<UiQueue<W>>,
    options: UiOptions,
    has_pages: PhantomData<P>,
}

// LayeringRenderer is DrawTarget layering wrapper which is the only Renderer supported for now.
impl<C, W> UI<W, WithPages>
where
    C: Color,
    W: WidgetCtx<Renderer = LayeringRenderer<C>, Color = C>,
{
    pub fn draw(&mut self, target: &mut impl DrawTarget<Color = C>) -> bool {
        self.current_page().draw(target)
    }
}

impl<R, E, S, I> UI<Wtf<R, E, S, I>, NoPages>
where
    R: Renderer + 'static,
    E: Event + 'static,
    S: PartialEq + Copy + 'static,
    I: PageId + 'static,
{
    pub fn new<V: PartialEq + Into<Size> + Copy + 'static>(
        // TODO: Rewrite to `IntoMaybeReactive` + MaybeReactive viewport
        viewport: impl IntoMemo<V>,
        // TODO: `with_styler` optional. Note: Not easily implementable
        styler: S,
    ) -> Self {
        let viewport = viewport.memo().map(|&viewport| viewport.into());
        let dev_tools =
            create_signal(DevTools { enabled: false, hovered: None });

        Self {
            page_history: Default::default(),
            viewport,
            pages: BTreeMap::new(),
            on_exit: None,
            styler: create_memo(move |_| styler),
            dev_tools,
            // TODO: Reactive viewport in Renderer
            renderer: R::new(viewport.get()).signal(),
            message_queue: None,
            options: Default::default(),
            has_pages: PhantomData,
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
        W: WidgetCtx<Event = NullEvent>,
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
        options: impl Into<MaybeReactive<<W::Renderer as Renderer>::Options>>,
    ) -> Self {
        self.renderer.setter(options.into(), |renderer, options| {
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
            styler: self.styler,
            dev_tools: self.dev_tools,
            renderer: self.renderer,
            message_queue: self.message_queue,
            options: self.options,
            has_pages: PhantomData,
        };

        // Go to page if it is the first one
        if with_page.pages.len() == 1 {
            with_page.goto(id);
        }

        with_page
    }

    fn add_page(&mut self, id: W::PageId, page_root: impl Into<El<W>>) {
        assert!(self
            .pages
            .insert(
                id,
                Page::new(
                    page_root,
                    self.viewport,
                    self.styler,
                    self.dev_tools,
                    self.renderer
                )
            )
            .is_none())
    }
}

impl<W: WidgetCtx> UI<W, WithPages> {
    /// Get mutable reference to currently active [`Page`]. You likely don't need to get pages.
    pub fn current_page(&mut self) -> &mut Page<W> {
        self.pages.get_mut(&self.page_history.last().unwrap()).unwrap()
    }

    // TODO: Unused
    // pub fn page(&mut self, id: W::PageId) -> &mut Page<W> {
    //     self.pages.get_mut(&id).unwrap()
    // }

    /// Run some logic on page change
    fn on_page_change(&mut self) {
        self.current_page().force_redraw();

        if self.options.auto_focus {
            self.current_page().auto_focus();
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
        events: impl Iterator<Item = W::Event>,
    ) -> Vec<UnhandledEvent<W>> {
        let unhandled = self
            .current_page()
            .handle_events(events)
            .into_iter()
            .filter_map(|unhandled| {
                if let UnhandledEvent::Event(event) = unhandled {
                    if event.as_dev_tools_toggle() {
                        self.dev_tools.update(|dt| dt.enabled = !dt.enabled);
                        return None;
                    }

                    if let (Some(on_exit), true) =
                        (self.on_exit.as_ref(), event.as_exit())
                    {
                        on_exit();
                        return None;
                    }

                    Some(UnhandledEvent::Event(event))
                } else {
                    Some(unhandled)
                }
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
}
