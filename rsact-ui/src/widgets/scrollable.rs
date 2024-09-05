use crate::{
    style::{Styler, WidgetStyle},
    widget::{prelude::*, BoxModelWidget, SizedWidget},
};
use alloc::vec::Vec;
use core::marker::PhantomData;
use embedded_graphics::{
    prelude::{Point, Primitive, Transform},
    primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder},
};

pub trait ScrollEvent {
    fn as_scroll(&self, axis: Axis) -> Option<i32>;
}

#[derive(Clone, Copy, PartialEq)]
pub enum ScrollbarShow {
    Always,
    // TODO: Show on scroll + add transitions when animations added
    // OnScroll,
    Auto,
}

#[derive(Clone, Copy)]
pub struct ScrollableState {
    // offset: Size,
    // TODO: Maybe offset should be i32, so we can make smooth animations such
    // as IOS does
    pub offset: u32,
    pub focus_pressed: bool,
    pub active: bool,
    // TODO: `is_scrolling` state when time source added. Reset it after
    // timeout
}

impl ScrollableState {
    pub fn none() -> Self {
        Self { offset: 0, focus_pressed: false, active: false }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct ScrollableStyle<C: Color> {
    pub track_color: Option<C>,
    pub thumb_color: Option<C>,
    pub thumb_radius: Option<Radius>,
    pub scrollbar_width: u32,
    pub container: BoxStyle<C>,
    pub show: ScrollbarShow,
}

impl<C: Color> WidgetStyle for ScrollableStyle<C> {
    type Color = C;
    type Inputs = ScrollableState;
}

impl<C: Color> ScrollableStyle<C> {
    pub fn base() -> Self {
        Self {
            track_color: None,
            thumb_color: Some(C::default_foreground()),
            thumb_radius: None,
            container: BoxStyle::base(),
            scrollbar_width: 5,
            show: ScrollbarShow::Auto,
        }
    }

    pub fn container(mut self, container: BoxStyle<C>) -> Self {
        self.container = container;
        self
    }

    pub fn show(mut self, show: ScrollbarShow) -> Self {
        self.show = show;
        self
    }

    pub fn scrollbar_width(mut self, scrollbar_width: u32) -> Self {
        self.scrollbar_width = scrollbar_width;
        self
    }

    pub fn thumb_color(mut self, thumb_color: Option<C>) -> Self {
        self.thumb_color = thumb_color;
        self
    }

    pub fn track_color(mut self, track_color: Option<C>) -> Self {
        self.track_color = track_color;
        self
    }

    pub fn thumb_radius(
        mut self,
        thumb_radius: Option<impl Into<Radius>>,
    ) -> Self {
        self.thumb_radius = thumb_radius.map(Into::into);
        self
    }

    fn track_style(&self) -> PrimitiveStyle<C> {
        let style =
            PrimitiveStyleBuilder::new().stroke_width(self.scrollbar_width);

        if let Some(track_color) = self.track_color {
            style.stroke_color(track_color)
        } else {
            style
        }
        .build()
    }

    fn thumb_style(&self) -> PrimitiveStyle<C> {
        let style =
            PrimitiveStyleBuilder::new().stroke_width(self.scrollbar_width);

        if let Some(thumb_color) = self.thumb_color {
            style.stroke_color(thumb_color)
        } else {
            style
        }
        .build()
    }
}

pub struct Scrollable<C: WidgetCtx, Dir: Direction> {
    id: ElId,
    state: Signal<ScrollableState>,
    style: MemoChain<ScrollableStyle<C::Color>>,
    content: Signal<El<C>>,
    layout: Signal<Layout>,
    dir: PhantomData<Dir>,
}

impl<C: WidgetCtx> Scrollable<C, RowDir> {
    pub fn horizontal(content: impl IntoSignal<El<C>>) -> Self {
        Self::new(content)
    }
}

impl<C: WidgetCtx> Scrollable<C, ColDir> {
    pub fn vertical(content: impl IntoSignal<El<C>>) -> Self {
        Self::new(content)
    }
}

impl<C: WidgetCtx, Dir: Direction> Scrollable<C, Dir> {
    pub fn new(content: impl IntoSignal<El<C>>) -> Self {
        let content = content.into_signal();
        let state = use_signal(ScrollableState::none());

        let layout = Layout {
            kind: LayoutKind::Scrollable,
            size: Dir::AXIS.canon(
                Length::InfiniteWindow(Length::fill().try_into().unwrap()),
                Length::fill(),
            ),
            box_model: BoxModel::zero().border_width(2),
            content_size: content.mapped(|content| {
                content.layout().with(|layout| layout.content_size.get())
            }),
        }
        .into_signal();

        let content_layout: Signal<Layout> =
            content.with(|content| content.layout());
        let content_layout_length =
            content_layout.with(|layout| layout.size.main(Dir::AXIS));

        if content_layout_length.is_grow() {
            panic!(
                "Don't use growing Length (Div/fill) for content {}!",
                Dir::AXIS.length_name()
            );

            // warn!("Don't use growing Length (Div/fill) for content {}.
            // Resetting it to Shrink!", Dir::AXIS.length_name());

            // content_layout.update_untracked(|layout| {
            //     *layout.size.main_mut(Dir::AXIS) = Length::Shrink
            // })
        }

        Self {
            id: ElId::unique(),
            content,
            state,
            style: use_memo_chain(|_| ScrollableStyle::base()),
            layout,
            dir: PhantomData,
        }
    }

    pub fn style(
        self,
        styler: impl Fn(
                ScrollableStyle<C::Color>,
                ScrollableState,
            ) -> ScrollableStyle<C::Color>
            + 'static,
    ) -> Self {
        let state = self.state;
        self.style.last(move |prev_style| styler(*prev_style, state.get()));
        self
    }
}

impl<C, Dir> SizedWidget<C> for Scrollable<C, Dir>
where
    C::Event: ScrollEvent,
    C: WidgetCtx,
    Dir: Direction,
    C::Styler: Styler<ScrollableStyle<C::Color>, Class = ()>,
{
}

impl<C, Dir> BoxModelWidget<C> for Scrollable<C, Dir>
where
    C::Event: ScrollEvent,
    C: WidgetCtx,
    Dir: Direction,
    C::Styler: Styler<ScrollableStyle<C::Color>, Class = ()>,
{
}

impl<C, Dir> Widget<C> for Scrollable<C, Dir>
where
    C::Event: ScrollEvent,
    C: WidgetCtx,
    Dir: Direction,
    C::Styler: Styler<ScrollableStyle<C::Color>, Class = ()>,
{
    fn children_ids(&self) -> Memo<Vec<ElId>> {
        let id = self.id;
        use_memo(move |_| vec![id])
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<C>) {
        ctx.accept_styles(self.style, self.state);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        let content = self.content;
        MemoTree {
            data: self.layout.into_memo(),
            children: use_memo(move |_| {
                content.with(|content| vec![content.build_layout_tree()])
            }),
        }
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, C>,
    ) -> crate::widget::DrawResult {
        let layout = self.layout.get();
        let style = self.style.get();

        ctx.draw_focus_outline(self.id)?;

        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.area,
            layout.box_model,
            style.container,
        ))?;

        let child_layout = ctx.layout.children().next();
        let child_layout = child_layout.as_ref().unwrap();

        self.content.with(|content| {
            let mut content_length = child_layout.area.size.main(Dir::AXIS);
            let scrollable_length = ctx.layout.area.size.main(Dir::AXIS);

            let draw_scrollbar = match style.show {
                ScrollbarShow::Always => {
                    // Note: Draw thumb of full length of scrollbar in Always
                    // mode
                    content_length = content_length.max(scrollable_length);
                    true
                },
                ScrollbarShow::Auto => content_length > scrollable_length,
            };

            let state = self.state.get();
            let offset = state.offset;

            if draw_scrollbar {
                let style = self.style.get();

                let track_start = match Dir::AXIS {
                    Axis::X => ctx.layout.area.anchor_point(
                        embedded_graphics::geometry::AnchorPoint::BottomLeft,
                    ),
                    Axis::Y => ctx.layout.area.anchor_point(
                        embedded_graphics::geometry::AnchorPoint::TopRight,
                    ),
                };
                let track_end = ctx
                    .layout
                    .area
                    .bottom_right()
                    .unwrap_or(ctx.layout.area.top_left);

                let scrollbar_translation =
                    Dir::AXIS.canon(0, -(style.scrollbar_width as i32 / 2));

                let track_line = Line::new(track_start, track_end)
                    .translate(scrollbar_translation);

                // Draw track
                ctx.renderer
                    .line(track_line.into_styled(style.track_style()))?;

                let thumb_len = (scrollable_length as f32
                    * (scrollable_length as f32 / content_length as f32))
                    as u32;
                let thumb_len = thumb_len.max(1);
                let thumb_offset = ((scrollable_length as f32
                    / content_length as f32)
                    * offset as f32) as u32;

                let thumb_start = track_start
                    + Dir::AXIS.canon::<Point>(thumb_offset as i32, 0);

                ctx.renderer.line(
                    Line::new(
                        thumb_start,
                        thumb_start
                            + Dir::AXIS.canon::<Point>(thumb_len as i32, 0),
                    )
                    .translate(scrollbar_translation)
                    .into_styled(style.thumb_style()),
                )?;
            }

            ctx.renderer.clipped(ctx.layout.area, |renderer| {
                content.draw(&mut DrawCtx {
                    state: ctx.state,
                    renderer,
                    layout: &child_layout
                        .translate(Dir::AXIS.canon(-(offset as i32), 0)),
                })
            })
        })
    }

    fn on_event(
        &mut self,
        ctx: &mut EventCtx<'_, C>,
    ) -> EventResponse<C::Event> {
        let current_state = self.state.get();

        // FocusEvent can be treated as ScrollEvent thus handle it before
        // focus move
        if current_state.active && ctx.is_focused(self.id) {
            if let Some(offset) = ctx.event.as_scroll(Dir::AXIS) {
                let current_state = self.state.get();

                let content_length = ctx
                    .layout
                    .children()
                    .next()
                    .unwrap()
                    .area
                    .size
                    .main(Dir::AXIS);

                let max_offset = content_length
                    .saturating_sub(ctx.layout.area.size.main(Dir::AXIS));

                let new_offset = (current_state.offset as i64 + offset as i64)
                    .clamp(0, max_offset as i64)
                    as u32;

                if new_offset != current_state.offset {
                    self.state.update(|state| state.offset = new_offset);
                }

                return Capture::Captured.into();
            }
        }

        ctx.handle_focusable(self.id, |pressed| {
            let current_state = self.state.get();

            if current_state.focus_pressed != pressed {
                let toggle_active = if !current_state.focus_pressed && pressed {
                    true
                } else {
                    false
                };

                self.state.update(|state| {
                    state.focus_pressed = pressed;
                    if toggle_active {
                        state.active = !state.active;
                    }
                });

                Capture::Captured.into()
            } else {
                Propagate::Ignored.into()
            }
        })
    }
}
