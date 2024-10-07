use crate::{
    declare_widget_style,
    style::{ColorStyle, Styler},
    widget::{prelude::*, Meta, MetaTree, SizedWidget},
};
use core::marker::PhantomData;
use embedded_graphics::{
    prelude::{Point, Primitive, Transform},
    primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder},
};
use layout::ContentLayout;

#[derive(Clone, Copy)]
pub enum ScrollableMode {
    Interactive,
    Tracker,
}

pub trait ScrollEvent {
    fn as_scroll(&self, axis: Axis) -> Option<i32>;
}

// TODO: Add feature for meaningful focus. When scrollable does not overflow, it
// is unfocusable and does not need to be allowed to be scrolled
#[derive(Clone, Copy, PartialEq)]
pub enum ScrollbarShow {
    Always,
    Never,
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

declare_widget_style! {
    ScrollableStyle (ScrollableState) {
        track_color: color,
        // thumb: container,
        thumb_color: color,
        container: container,
        scrollbar_width: u32,
        show: ScrollbarShow,
    }
}

impl<C: Color> ScrollableStyle<C> {
    pub fn base() -> Self {
        Self {
            track_color: ColorStyle::Unset,
            // thumb: BlockStyle::base(),
            thumb_color: ColorStyle::DefaultForeground,
            container: BlockStyle::base(),
            scrollbar_width: 5,
            show: ScrollbarShow::Auto,
        }
    }

    fn track_style(&self) -> PrimitiveStyle<C> {
        let style =
            PrimitiveStyleBuilder::new().stroke_width(self.scrollbar_width);

        if let Some(track_color) = self.track_color.get() {
            style.stroke_color(track_color)
        } else {
            style
        }
        .build()
    }

    fn thumb_style(&self) -> PrimitiveStyle<C> {
        let style =
            PrimitiveStyleBuilder::new().stroke_width(self.scrollbar_width);

        if let Some(thumb_color) = self.thumb_color.get() {
            style.stroke_color(thumb_color)
        } else {
            style
        }
        .build()
    }
}

pub struct Scrollable<W: WidgetCtx, Dir: Direction> {
    id: ElId,
    state: Signal<ScrollableState>,
    style: MemoChain<ScrollableStyle<W::Color>>,
    content: Signal<El<W>>,
    layout: Signal<Layout>,
    mode: ScrollableMode,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx> Scrollable<W, RowDir> {
    pub fn horizontal(content: impl IntoSignal<El<W>>) -> Self {
        Self::new(content)
    }
}

impl<W: WidgetCtx> Scrollable<W, ColDir> {
    pub fn vertical(content: impl IntoSignal<El<W>>) -> Self {
        Self::new(content)
    }
}

impl<W: WidgetCtx, Dir: Direction> Scrollable<W, Dir> {
    pub fn new(content: impl IntoSignal<El<W>>) -> Self {
        let content = content.into_signal();
        let state = use_signal(ScrollableState::none());

        let layout = Layout {
            kind: LayoutKind::Scrollable(ContentLayout {
                content_size: content.mapped(|content| {
                    content.layout().with(|layout| layout.content_size())
                }),
            }),
            size: Dir::AXIS.canon(
                Length::InfiniteWindow(Length::Shrink.try_into().unwrap()),
                Length::fill(),
            ),
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
            mode: ScrollableMode::Interactive,
            dir: PhantomData,
        }
    }

    pub fn tracker(mut self) -> Self {
        self.mode = ScrollableMode::Tracker;
        self
    }

    pub fn style(
        self,
        styler: impl Fn(
                ScrollableStyle<W::Color>,
                ScrollableState,
            ) -> ScrollableStyle<W::Color>
            + 'static,
    ) -> Self {
        let state = self.state;
        self.style.last(move |prev_style| styler(*prev_style, state.get()));
        self
    }

    fn max_offset(&self, ctx: &EventCtx<'_, W>) -> u32 {
        let content_length =
            ctx.layout.children().next().unwrap().area.size.main(Dir::AXIS);

        content_length.saturating_sub(ctx.layout.area.size.main(Dir::AXIS))
    }
}

impl<W, Dir> SizedWidget<W> for Scrollable<W, Dir>
where
    W::Event: ScrollEvent,
    W: WidgetCtx,
    Dir: Direction,
    W::Styler: Styler<ScrollableStyle<W::Color>, Class = ()>,
{
    fn width<L: Into<Length> + Copy + 'static>(
        self,
        width: impl MaybeSignal<L> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(width.maybe_signal(), |&width, layout| {
            layout.size.width =
                Length::InfiniteWindow(width.into().try_into().unwrap());
        });
        self
    }

    fn height<L: Into<Length> + Copy + 'static>(
        self,
        height: impl MaybeSignal<L> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(height.maybe_signal(), |&height, layout| {
            layout.size.height =
                Length::InfiniteWindow(height.into().try_into().unwrap());
        });
        self
    }
}

impl<W, Dir> Widget<W> for Scrollable<W, Dir>
where
    W::Event: ScrollEvent,
    W: WidgetCtx,
    Dir: Direction,
    W::Styler: Styler<ScrollableStyle<W::Color>, Class = ()>,
{
    fn meta(&self) -> crate::widget::MetaTree {
        MetaTree {
            data: Meta::none().into_memo(),
            children: self.content.mapped(|content| vec![content.meta()]),
        }
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);
        ctx.pass_to_child(self.content);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        let content = self.content;
        MemoTree {
            data: self.layout.into_memo(),
            children: content
                .mapped(|content| vec![content.build_layout_tree()]),
        }
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, W>,
    ) -> crate::widget::DrawResult {
        let layout = self.layout.get();
        let style = self.style.get();

        ctx.draw_focus_outline(self.id)?;

        ctx.renderer.block(Block::from_layout_style(
            ctx.layout.area,
            layout.block_model(),
            style.container,
        ))?;

        let child_layout = ctx.layout.children().next();
        let child_layout = child_layout.as_ref().unwrap();

        let mut content_length = child_layout.area.size.main(Dir::AXIS);
        let scrollable_length = ctx.layout.area.size.main(Dir::AXIS);

        let draw_scrollbar = match style.show {
            ScrollbarShow::Always => {
                // Note: Draw thumb of full length of scrollbar in Always
                // mode
                content_length = content_length.max(scrollable_length);
                true
            },
            ScrollbarShow::Never => false,
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
            ctx.renderer.line(track_line.into_styled(style.track_style()))?;

            let thumb_len = (scrollable_length as f32
                * (scrollable_length as f32 / content_length as f32))
                as u32;
            let thumb_len = thumb_len.max(1);
            let thumb_offset = ((scrollable_length as f32
                / content_length as f32)
                * offset as f32) as u32;

            let thumb_start =
                track_start + Dir::AXIS.canon::<Point>(thumb_offset as i32, 0);

            ctx.renderer.line(
                Line::new(
                    thumb_start,
                    thumb_start + Dir::AXIS.canon::<Point>(thumb_len as i32, 0),
                )
                .translate(scrollbar_translation)
                .into_styled(style.thumb_style()),
            )?;
        }

        self.content.with(|content| {
            ctx.renderer.clipped(ctx.layout.area, |renderer| {
                content.draw(&mut DrawCtx {
                    state: ctx.state,
                    renderer,
                    layout: &child_layout
                        .translate(Dir::AXIS.canon(-(offset as i32), 0)),
                    tree_style: ctx.tree_style,
                })
            })
        })
    }

    fn on_event(
        &mut self,
        ctx: &mut EventCtx<'_, W>,
    ) -> EventResponse<W> {
        let current_state = self.state.get();

        match self.mode {
            ScrollableMode::Interactive => {
                // FocusEvent can be treated as ScrollEvent thus handle it
                // before focus move
                if current_state.active && ctx.is_focused(self.id) {
                    if let Some(offset) = ctx.event.as_scroll(Dir::AXIS) {
                        let max_offset = self.max_offset(ctx);

                        let new_offset = (current_state.offset as i64
                            + offset as i64)
                            .clamp(0, max_offset as i64)
                            as u32;

                        if new_offset != current_state.offset {
                            self.state
                                .update(|state| state.offset = new_offset);
                        }

                        return W::capture();
                    }
                }

                ctx.handle_focusable(self.id, |pressed| {
                    let current_state = self.state.get();

                    if current_state.focus_pressed != pressed {
                        let toggle_active =
                            if !current_state.focus_pressed && pressed {
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

                        W::capture()
                    } else {
                        W::ignore()
                    }
                })
            },
            ScrollableMode::Tracker => {
                // If nothing was focused before passing event to children then
                // change of focus means moving focus to a widget inside
                // scrollable content
                let had_focused = ctx.pass.focused().is_some();

                let content_response = self
                    .content
                    .control_flow(|content| ctx.pass_to_child(content));

                if let (false, Some(focused)) =
                    (had_focused, ctx.pass.focused())
                {
                    let new_offset = focused
                        .absolute_position
                        .main(Dir::AXIS)
                        .saturating_sub(
                            ctx.layout.area.top_left.main(Dir::AXIS),
                        ) as u32;
                    let new_offset = new_offset.clamp(0, self.max_offset(ctx));

                    if current_state.offset != new_offset {
                        self.state.update(|state| {
                            state.offset = new_offset;
                        })
                    }
                }

                content_response
            },
        }
    }
}