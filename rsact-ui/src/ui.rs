use crate::{
    el::El,
    event::{
        dev::DevToolsToggle, message::Message, BubbledData, Event,
        ExitEvent as _, UnhandledEvent,
    },
    layout::size::Size,
    page::{
        dev::DevTools,
        id::{PageId, SinglePage},
        Page,
    },
    render::{color::Color, draw_target::LayeringRenderer, Renderer},
    widget::{DrawResult, WidgetCtx, Wtf},
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use embedded_graphics::prelude::DrawTarget;
use rsact_reactive::prelude::*;

pub struct UI<W: WidgetCtx> {
    active_page: W::PageId,
    pages: BTreeMap<W::PageId, Page<W>>,
    viewport: Signal<Size>,
    on_exit: Option<Box<dyn Fn()>>,
    styler: Signal<W::Styler>,
    dev_tools: Signal<DevTools>,
    renderer: Signal<W::Renderer>,
}

impl<C, W> UI<W>
where
    C: Color,
    W: WidgetCtx<Renderer = LayeringRenderer<C>, Color = C>,
{
    pub fn draw(&mut self, target: &mut impl DrawTarget<Color = C>) -> bool {
        self.current_page().draw(target)
    }
}

impl<R, E, S> UI<Wtf<R, E, S, SinglePage>>
where
    R: Renderer + 'static,
    E: Event + 'static,
    S: PartialEq + Copy + 'static,
{
    pub fn single_page(
        root: impl IntoSignal<El<Wtf<R, E, S, SinglePage>>>,
        viewport: impl Into<Size> + Copy,
        styler: S,
    ) -> Self {
        Self::new(SinglePage, root, viewport, styler)
    }
}

impl<R, E, S, I> UI<Wtf<R, E, S, I>>
where
    R: Renderer + 'static,
    E: Event + 'static,
    S: PartialEq + Copy + 'static,
    I: PageId + 'static,
{
    pub fn new(
        page_id: I,
        start_page_root: impl IntoSignal<El<Wtf<R, E, S, I>>>,
        viewport: impl Into<Size> + Copy,
        styler: S,
    ) -> Self {
        let viewport = use_signal(viewport.into());
        let styler = use_signal(styler);
        let dev_tools = use_signal(DevTools { enabled: false, hovered: None });

        Self {
            active_page: page_id,
            viewport,
            pages: BTreeMap::new(),
            on_exit: None,
            styler,
            dev_tools,
            renderer: R::new(viewport.get()).into_signal(),
        }
        .with_page(page_id, start_page_root)
    }
}

impl<W: WidgetCtx> UI<W> {
    /// Add ExitEvent handler that eats exit event
    pub fn on_exit(mut self, on_exit: impl Fn() + 'static) -> Self {
        self.on_exit = Some(Box::new(on_exit));
        self
    }

    pub fn with_renderer_options(
        self,
        options: <W::Renderer as Renderer>::Options,
    ) -> Self {
        self.renderer.update(|renderer| renderer.set_options(options));
        self
    }

    pub fn current_page(&mut self) -> &mut Page<W> {
        self.pages.get_mut(&self.active_page).unwrap()
    }

    pub fn page(&mut self, id: W::PageId) -> &mut Page<W> {
        self.pages.get_mut(&id).unwrap()
    }

    pub fn add_page(
        &mut self,
        id: W::PageId,
        page_root: impl IntoSignal<El<W>>,
    ) {
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

    pub fn with_page(
        mut self,
        id: W::PageId,
        page_root: impl IntoSignal<El<W>>,
    ) -> Self {
        self.add_page(id, page_root);
        self
    }

    pub fn tick(
        &mut self,
        events: impl Iterator<Item = W::Event>,
    ) -> Vec<UnhandledEvent<W>> {
        self.current_page()
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
                } else if let UnhandledEvent::Bubbled(BubbledData::Message(
                    Message::GoTo(page),
                )) = unhandled
                {
                    self.active_page = page;
                    None
                } else {
                    Some(unhandled)
                }
            })
            .collect()
    }
}
