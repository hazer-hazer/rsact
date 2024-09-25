use crate::{
    el::{El, ElId},
    event::{
        dev::{DevElHover, DevToolsToggle},
        Capture, Event, EventResponse, FocusEvent, Propagate,
    },
    layout::{model_layout, size::Size, DevHoveredLayout, LayoutModel, Limits},
    render::{
        color::Color, draw_target::LayeringRenderer, Block, Border, Renderer,
    },
    style::{
        block::{BorderStyle, BoxStyle},
        NullStyler,
    },
    widget::{
        prelude::BoxModel, DrawCtx, DrawResult, EventCtx, MountCtx, PageState,
        PhantomWidgetCtx, Widget, WidgetCtx,
    },
};
use alloc::{boxed::Box, vec::Vec};
use embedded_graphics::{
    mono_font::{
        ascii::{FONT_5X7, FONT_6X9, FONT_8X13, FONT_9X15},
        MonoTextStyle, MonoTextStyleBuilder,
    },
    prelude::{DrawTarget, Point},
    primitives::Rectangle,
};
use embedded_text::{style::TextBoxStyleBuilder, TextBox};
use rsact_core::prelude::*;

pub struct PageStyle<C: Color> {
    background_color: Option<C>,
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

#[derive(Clone, Copy)]
struct HoveredEl {
    layout: DevHoveredLayout,
}

impl HoveredEl {
    fn model<C: Color>(area: Rectangle, color: C) -> Block<C> {
        Block {
            border: Border::new(
                BoxStyle::base().border(BorderStyle::base().color(color)),
                BoxModel::zero().border_width(1),
            ),
            rect: area,
            background: None,
        }
    }

    fn draw<C: Color>(
        &self,
        r: &mut impl Renderer<Color = C>,
        viewport: Size,
    ) -> DrawResult {
        let area = self.layout.area;

        let [text_color, inner_color, padding_color, ..] = C::accents();

        r.block(Self::model(area, padding_color))?;
        if let Some(padding) = self.layout.kind.padding() {
            r.block(Self::model(area - padding, inner_color))?;
        }

        let area_text = format!(
            "{} {}x{}{}",
            self.layout.kind,
            area.size.width,
            area.size.height,
            if self.layout.children_count > 0 {
                format!(" [{}]", self.layout.children_count)
            } else {
                alloc::string::String::new()
            },
        );

        // Ignore error, TextBox sometimes fails
        r.mono_text(TextBox::with_textbox_style(
            &area_text,
            Rectangle::new(Point::zero(), viewport.into()),
            MonoTextStyleBuilder::new()
                .font(&FONT_8X13)
                .text_color(text_color)
                .background_color(C::default_background())
                .build(),
            TextBoxStyleBuilder::new()
                .alignment(embedded_text::alignment::HorizontalAlignment::Right)
                .vertical_alignment(
                    embedded_text::alignment::VerticalAlignment::Bottom,
                )
                .build(),
        ))
        .ok();

        Ok(())
    }
}

#[derive(Clone, Copy)]
struct DevToolsState {
    enabled: bool,
    hovered: Option<HoveredEl>,
}

pub struct Page<W: WidgetCtx> {
    root: El<W>,
    ids: Memo<Vec<ElId>>,
    layout: Memo<LayoutModel>,
    state: PageState<W>,
    // TODO: Should be Memo?
    style: Signal<PageStyle<W::Color>>,
    renderer: W::Renderer,
    viewport: Memo<Size>,
    dev_tools: Signal<DevToolsState>,
}

impl<W: WidgetCtx> Page<W> {
    fn new(
        root: impl Into<El<W>>,
        viewport: Signal<Size>,
        styler: Signal<W::Styler>,
        dev_tools: Signal<DevToolsState>,
    ) -> Self {
        let mut root = root.into();
        let state = PageState::new();
        let viewport = viewport.into_memo();

        root.on_mount(MountCtx {
            viewport: viewport.into_memo(),
            styler: styler.into_memo(),
        });

        let layout_tree = root.build_layout_tree();
        let layout = use_memo(move |_| {
            // println!("Relayout");
            model_layout(layout_tree, Limits::only_max(viewport.get()))
        });
        // TODO: Children ids should be paired with Behavior settings, child can
        // have an id but not be focusable for example
        let ids = root.children_ids();

        Self {
            root,
            layout,
            state,
            style: PageStyle::base().into_signal(),
            ids,
            // TODO: Signal viewport in Renderer
            renderer: W::Renderer::new(viewport.get()),
            viewport,
            dev_tools,
        }
    }

    // pub fn style(
    //     mut self,
    //     style: impl IntoSignal<PageStyle<C::Color>>,
    // ) -> Self {
    //     self.style = style.signal();
    //     self
    // }

    pub fn auto_focus(&mut self) {
        self.ids.with(|ids| {
            self.state.focused = ids.first().copied();
        })
    }

    fn find_hovered_el(&self, point: Point) -> Option<HoveredEl> {
        self.layout.with(|layout| {
            layout
                .tree_root()
                .dev_hover(point)
                .map(|layout| HoveredEl { layout })
        })
    }

    pub fn handle_events(
        &mut self,
        events: impl Iterator<Item = W::Event>,
    ) -> Vec<W::Event> {
        events
            .map(|event| {
                if self.dev_tools.get().enabled {
                    if let Some(point) = event.as_dev_el_hover() {
                        self.dev_tools.update(|dev_tools| {
                            dev_tools.hovered = self.find_hovered_el(point)
                        });
                        return None;
                    }
                }

                let response = self.layout.with(|layout| {
                    self.root.on_event(&mut EventCtx {
                        event: &event,
                        page_state: &mut self.state,
                        layout: &layout.tree_root(),
                    })
                });

                match response {
                    EventResponse::Continue(propagate) => match propagate {
                        Propagate::Ignored => Some(event),
                        Propagate::BubbleUp(_, event) => Some(event),
                    },
                    EventResponse::Break(capture) => match capture {
                        Capture::Captured => None,
                        Capture::Bubbled(el_id, event) => {
                            if let Some(offset) = event.as_focus_move() {
                                self.ids.with(|ids| {
                                    let position =
                                        ids.iter().position(|&id| id == el_id);

                                    if let Some(new) =
                                        position.and_then(|pos| {
                                            ids.get(
                                                (pos as i64 + offset as i64)
                                                    as usize,
                                            )
                                            .copied()
                                        })
                                    {
                                        self.state.focused.replace(new);
                                    }
                                });

                                None
                            } else {
                                Some(event)
                            }
                        },
                    },
                }
            })
            .filter_map(|event| event)
            .collect()
    }

    pub fn draw(
        &mut self,
        target: &mut impl DrawTarget<Color = <W::Renderer as Renderer>::Color>,
    ) -> DrawResult {
        self.style.with(|style| {
            if let Some(background_color) = style.background_color {
                Renderer::clear(&mut self.renderer, background_color)
            } else {
                Ok(())
            }
        })?;

        let result = self.layout.with(|layout| {
            self.root.draw(&mut DrawCtx {
                state: &self.state,
                renderer: &mut self.renderer,
                layout: &layout.tree_root(),
            })
        })?;

        if let Some(hovered) = self.dev_tools.with(|dt| dt.hovered) {
            hovered.draw(&mut self.renderer, self.viewport.get())?;
        }

        self.renderer.finish(target);

        Ok(result)

        // self.style.with(|style| {
        //     if let Some(focused) = self.state.focused {
        //         renderer.block(Block {
        //             border:
        // Border::zero().color(style.focus_outline.color).radius(style.
        // focus_outline.radius).width(1),             rect: ,
        //             background: todo!(),
        //         })
        //     }
        // });
    }
}

pub struct UI<R, E, S>
where
    R: Renderer + 'static,
    E: Event + 'static,
    S: PartialEq + Copy + 'static,
{
    active_page: usize,
    pages: Vec<Page<PhantomWidgetCtx<R, E, S>>>,
    viewport: Signal<Size>,
    on_exit: Option<Box<dyn Fn()>>,
    // TODO: Use `Option` instead of NullStyler to avoid useless allocation of
    // Default ThemeStyler. ThemeStyler should only be set when theme is set
    styler: Signal<S>,
    dev_tools: Signal<DevToolsState>,
}

impl<C, E, S> UI<LayeringRenderer<C>, E, S>
where
    E: Event + 'static,
    C: Color + 'static,
    S: PartialEq + Copy + 'static,
{
    pub fn draw(
        &mut self,
        target: &mut impl DrawTarget<Color = C>,
    ) -> DrawResult {
        self.pages[self.active_page].draw(target)
    }
}

// impl<R, E> UI<R, E, ThemeStyler<R::Color>>
// where
//     R: Renderer + 'static,
//     E: Event + 'static,
//     R::Color: ThemeColor,
// {
//     pub fn theme(mut self, theme: Theme<R::Color>) -> Self {
//         if let Some(styler) = self.styler.as_mut() {
//             styler.set_theme(theme);
//         } else {
//             self.styler.replace(ThemeStyler::new(theme));
//         }
//         self
//     }
// }

// impl<R, E> UI<R, E, NullStyler>
// where
//     R: Renderer + 'static,
//     E: Event + 'static,
// {
// }

impl<R, E, S> UI<R, E, S>
where
    R: Renderer + 'static,
    E: Event + 'static,
    S: PartialEq + Copy + 'static,
{
    pub fn new(
        root: impl Into<El<PhantomWidgetCtx<R, E, S>>>,
        viewport: impl Into<Size> + Copy,
        styler: S,
    ) -> Self {
        let viewport = use_signal(viewport.into());
        let styler = use_signal(styler);
        let dev_tools =
            use_signal(DevToolsState { enabled: false, hovered: None });

        Self {
            active_page: 0,
            viewport,
            pages: vec![Page::new(root, viewport, styler, dev_tools)],
            on_exit: None,
            styler,
            dev_tools,
        }
    }

    /// Add ExitEvent handler that eats exit event
    pub fn on_exit(mut self, on_exit: impl Fn() + 'static) -> Self {
        self.on_exit = Some(Box::new(on_exit));
        self
    }

    pub fn current_page(&mut self) -> &mut Page<PhantomWidgetCtx<R, E, S>> {
        &mut self.pages[self.active_page]
    }

    pub fn add_page(&mut self, root: impl Into<El<PhantomWidgetCtx<R, E, S>>>) {
        self.pages.push(Page::new(
            root,
            self.viewport,
            self.styler,
            self.dev_tools,
        ))
    }

    pub fn with_page(
        mut self,
        root: impl Into<El<PhantomWidgetCtx<R, E, S>>>,
    ) -> Self {
        self.add_page(root);
        self
    }

    pub fn tick(&mut self, events: impl Iterator<Item = E>) -> Vec<E> {
        let page = &mut self.pages[self.active_page];

        page.handle_events(events)
            .iter()
            .cloned()
            .filter_map(|e| {
                if e.as_dev_tools_toggle() {
                    self.dev_tools.update(|dt| dt.enabled = !dt.enabled);
                    return None;
                }

                if let (Some(on_exit), true) =
                    (self.on_exit.as_ref(), e.as_exit())
                {
                    on_exit();
                    return None;
                }

                Some(e)
            })
            .collect()
    }
}
