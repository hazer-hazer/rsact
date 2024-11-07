use crate::{
    el::El,
    event::{
        dev::DevToolsToggle,
        message::{Message, MessageQueue},
        BubbledData, Event, ExitEvent as _, UnhandledEvent,
    },
    layout::size::Size,
    page::{
        dev::DevTools,
        id::{PageId, SinglePage},
        Page,
    },
    render::{color::Color, draw_target::LayeringRenderer, Renderer},
    widget::{Widget, WidgetCtx, Wtf},
};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use embedded_graphics::prelude::DrawTarget;
use rsact_reactive::prelude::*;

pub struct UI<W: WidgetCtx> {
    page_history: Vec<W::PageId>,
    pages: BTreeMap<W::PageId, Page<W>>,
    viewport: Signal<Size>,
    on_exit: Option<Box<dyn Fn()>>,
    styler: Signal<W::Styler>,
    dev_tools: Signal<DevTools>,
    renderer: Signal<W::Renderer>,
    message_queue: Option<MessageQueue<W>>,
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
    // TODO: S is not checked for being styler with specific color
    S: PartialEq + Copy + 'static,
{
    pub fn single_page(
        root: impl Into<El<Wtf<R, E, S, SinglePage>>>,
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
        start_page_root: impl Into<El<Wtf<R, E, S, I>>>,
        viewport: impl Into<Size> + Copy,
        styler: S,
    ) -> Self {
        let viewport = create_signal(viewport.into());
        let styler = create_signal(styler);
        let dev_tools =
            create_signal(DevTools { enabled: false, hovered: None });

        Self {
            page_history: vec![page_id],
            viewport,
            pages: BTreeMap::new(),
            on_exit: None,
            styler,
            dev_tools,
            renderer: R::new(viewport.get()).into_signal(),
            message_queue: None,
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
        self.pages.get_mut(&self.page_history.last().unwrap()).unwrap()
    }

    pub fn page(&mut self, id: W::PageId) -> &mut Page<W> {
        self.pages.get_mut(&id).unwrap()
    }

    pub fn add_page(&mut self, id: W::PageId, page_root: impl Into<El<W>>) {
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

    pub fn with_queue(mut self, queue: MessageQueue<W>) -> Self {
        self.message_queue = Some(queue);
        self
    }

    pub fn with_page(
        mut self,
        id: W::PageId,
        page_root: impl Into<El<W>>,
    ) -> Self {
        self.add_page(id, page_root);
        self
    }

    fn on_page_change(&mut self) {
        self.current_page().force_redraw();
    }

    pub fn previous_page(&mut self) -> bool {
        if self.page_history.len() > 1 {
            self.page_history.pop();
            self.on_page_change();
            true
        } else {
            false
        }
    }

    pub fn goto(&mut self, page_id: W::PageId) {
        self.page_history.push(page_id);
        self.on_page_change();
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
                Message::GoTo(page_id) => self.goto(page_id),
                Message::PreviousPage => {
                    self.previous_page();
                },
            }
        }

        unhandled
    }
}
