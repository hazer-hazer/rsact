pub mod id;

use crate::{
    el::El,
    event::{
        dev::DevElHover as _, Capture, EventPass, EventResponse, FocusEvent,
        Propagate, UnhandledEvent,
    },
    layout::{model_layout, size::Size, DevHoveredLayout, LayoutModel, Limits},
    render::{color::Color, Block, Border, Renderer},
    style::{
        block::{BlockStyle, BorderStyle},
        TreeStyle,
    },
    widget::{
        prelude::BlockModel, Behavior, DrawCtx, DrawResult, EventCtx, MetaTree,
        MountCtx, PageState, Widget as _, WidgetCtx,
    },
};
use alloc::vec::Vec;
use embedded_graphics::{
    mono_font::{ascii::FONT_8X13, MonoTextStyleBuilder},
    prelude::{DrawTarget, Point},
    primitives::Rectangle,
};
use embedded_text::{style::TextBoxStyleBuilder, TextBox};
use rsact_reactive::prelude::*;

pub struct PageStyle<C: Color> {
    // TODO: Use ColorStyle
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
pub struct DevToolsState {
    pub enabled: bool,
    pub hovered: Option<DevHoveredEl>,
}

#[derive(Clone, Copy)]
pub struct DevHoveredEl {
    pub layout: DevHoveredLayout,
}

impl DevHoveredEl {
    fn model<C: Color>(area: Rectangle, color: C) -> Block<C> {
        Block {
            border: Border::new(
                BlockStyle::base().border(BorderStyle::base().color(color)),
                BlockModel::zero().border_width(1),
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
        if let Some(padding) = self.layout.layout.kind.padding() {
            r.block(Self::model(area - padding, inner_color))?;
        }

        let area_text = format!(
            "{} {}x{}({}){}",
            self.layout.layout.kind,
            area.size.width,
            area.size.height,
            self.layout.size,
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

/// Tree of info about widget tree. Elements are mostly
struct PageTree {
    /// Page elements meta tree
    meta: MetaTree,

    /// Count of focusable elements
    focusable: Memo<usize>,

    /// Absolute tree index of focused element
    focused: Option<usize>,
}

pub struct Page<W: WidgetCtx> {
    root: El<W>,
    // ids: Memo<Vec<ElId>>,
    tree: PageTree,
    layout: Memo<LayoutModel>,
    state: PageState<W>,
    // TODO: Should be Memo?
    style: Signal<PageStyle<W::Color>>,
    renderer: W::Renderer,
    viewport: Memo<Size>,
    dev_tools: Signal<DevToolsState>,
}

impl<W: WidgetCtx> Page<W> {
    pub fn new(
        root: impl Into<El<W>>,
        viewport: Signal<Size>,
        styler: Signal<W::Styler>,
        dev_tools: Signal<DevToolsState>,
    ) -> Self {
        let mut root: El<W> = root.into();
        let state = PageState::new();
        let viewport = viewport.into_memo();

        // TODO: `on_mount` dependency on viewport can be removed for text,
        //  icon, etc. by adding special LayoutKind dependent on viewport and
        //  pass viewport to model_layout. In such a way, layout becomes much
        //  more straightforward and single-pass process.
        root.on_mount(MountCtx { viewport, styler: styler.into_memo() });

        let layout_tree = root.build_layout_tree();
        let layout = viewport.mapped(move |&viewport_size| {
            // println!("Relayout");
            // TODO: Possible optimization is to use previous memo result, pass
            // it to model_layout as tree and don't relayout parents if layouts
            // inside Fixed-sized container changed, returning previous result
            let layout = model_layout(
                layout_tree,
                Limits::only_max(viewport_size),
                viewport_size.into(),
                viewport,
            );

            // println!("{:#?}", layout.tree_root());

            layout
        });

        let meta = root.meta();
        let focusable = use_memo(move |_| {
            meta.flat_collect().iter().fold(0, |count, el| {
                el.with(|el| {
                    count + (el.behavior & Behavior::FOCUSABLE).bits() as usize
                })
            })
        });

        Self {
            root,
            layout,
            state,
            style: PageStyle::base().into_signal(),
            tree: PageTree { meta, focusable, focused: None },
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
        if self.tree.focusable.get() > 0 {
            self.handle_events([<W::Event as FocusEvent>::zero()].into_iter());
        }
    }

    fn find_hovered_el(&self, point: Point) -> Option<DevHoveredEl> {
        self.layout.with(|layout| {
            layout
                .tree_root()
                .dev_hover(point)
                .map(|layout| DevHoveredEl { layout })
        })
    }

    pub fn handle_events(
        &mut self,
        events: impl Iterator<Item = W::Event>,
    ) -> Vec<UnhandledEvent<W>> {
        events
            .filter_map(|event| {
                if self.dev_tools.get().enabled {
                    if let Some(point) = event.as_dev_el_hover() {
                        self.dev_tools.update(|dev_tools| {
                            dev_tools.hovered = self.find_hovered_el(point)
                        });
                        return None;
                    }
                }

                let layout = self.layout;

                let new_focus = event.as_focus_move().map(|offset| {
                    ((self.tree.focused.unwrap_or(0) as i64 + offset as i64)
                        as usize)
                        .clamp(0, self.tree.focusable.get())
                });

                let mut pass = EventPass::new(new_focus);

                let response = with!(|layout| {
                    self.root.on_event(&mut EventCtx {
                        event: &event,
                        page_state: &mut self.state,
                        layout: &layout.tree_root(),
                        pass: &mut pass,
                    })
                });

                match response {
                    EventResponse::Continue(propagate) => match propagate {
                        Propagate::Ignored => {
                            if let Some(focused) = pass.focused() {
                                debug_assert_eq!(pass.focus_search, None);

                                self.tree.focused = Some(new_focus.expect(
                                    "new_focus must be set in this case",
                                ));
                                self.state.focused = Some(focused.id);

                                None
                            } else {
                                // TODO: Should only bubbled events be returned?
                                Some(UnhandledEvent::Event(event))
                            }
                        },
                    },
                    EventResponse::Break(capture) => match capture {
                        Capture::Captured => None,
                        Capture::Bubble(data) => {
                            Some(UnhandledEvent::Bubbled(data))
                        },
                    },
                }
            })
            .collect()
    }

    pub fn draw(
        &mut self,
        target: &mut impl DrawTarget<Color = <W::Renderer as Renderer>::Color>,
    ) -> DrawResult {
        // FIXME: Performance?
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
                tree_style: TreeStyle::base(),
            })
        })?;

        if self.dev_tools.with(|dt| dt.enabled) {
            if let Some(hovered) = self.dev_tools.with(|dt| dt.hovered) {
                hovered.draw(&mut self.renderer, self.viewport.get())?;
            }
        }

        self.renderer.finish(target);

        Ok(result)
    }
}
