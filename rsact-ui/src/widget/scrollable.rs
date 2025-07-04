use crate::{
    declare_widget_style,
    el::WithElId,
    render::{Renderable, primitives::line::Line},
    style::{ColorStyle, WidgetStylist},
    widget::{Meta, MetaTree, SizedWidget, prelude::*},
};
use core::marker::PhantomData;
use embedded_graphics::{
    prelude::{Point, Primitive, Transform},
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder},
};
use rsact_reactive::maybe::IntoMaybeReactive;

#[derive(Clone, Copy)]
pub enum ScrollableMode {
    Interactive,
    Tracker,
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
            thumb_color: ColorStyle::DefaultForeground,
            container: BlockStyle::base(),
            scrollbar_width: 5,
            show: ScrollbarShow::Auto,
        }
    }

    fn track_style(&self) -> PrimitiveStyle<C> {
        let style =
            PrimitiveStyleBuilder::new().stroke_width(self.scrollbar_width);

        (if let Some(track_color) = self.track_color.get() {
            style.stroke_color(track_color)
        } else {
            style
        })
        .build()
    }

    fn thumb_style(&self) -> PrimitiveStyle<C> {
        let style =
            PrimitiveStyleBuilder::new().stroke_width(self.scrollbar_width);

        (if let Some(thumb_color) = self.thumb_color.get() {
            style.stroke_color(thumb_color)
        } else {
            style
        })
        .build()
    }
}

pub struct Scrollable<W: WidgetCtx, Dir: Direction> {
    state: Signal<ScrollableState>,
    style: MemoChain<ScrollableStyle<W::Color>>,
    content: El<W>,
    layout: Signal<Layout>,
    mode: ScrollableMode,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx> Scrollable<W, RowDir> {
    pub fn horizontal(content: impl Widget<W> + 'static) -> Self {
        Self::new(content)
    }
}

impl<W: WidgetCtx> Scrollable<W, ColDir> {
    pub fn vertical(content: impl Widget<W> + 'static) -> Self {
        Self::new(content)
    }
}

impl<W: WidgetCtx, Dir: Direction> Scrollable<W, Dir> {
    pub fn new(content: impl Widget<W> + 'static) -> Self {
        let content = content.el();
        let state = create_signal(ScrollableState::none());

        let layout =
            Layout::scrollable::<Dir>(content.layout().memo()).signal();

        Self {
            content,
            state,
            style: ScrollableStyle::base().memo_chain(),
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
        styler: impl (Fn(
            ScrollableStyle<W::Color>,
            ScrollableState,
        ) -> ScrollableStyle<W::Color>)
        + 'static,
    ) -> Self {
        let state = self.state;
        self.style
            .last(move |prev_style| styler(*prev_style, state.get()))
            .unwrap();
        self
    }

    fn max_offset(&self, ctx: &EventCtx<'_, W>) -> u32 {
        let content_length =
            ctx.layout.children().next().unwrap().inner.size.main(Dir::AXIS);

        content_length.saturating_sub(ctx.layout.inner.size.main(Dir::AXIS))
    }
}

impl<W: WidgetCtx> SizedWidget<W> for Scrollable<W, RowDir>
where
    W::Styler: WidgetStylist<ScrollableStyle<W::Color>>,
{
    fn width<L: Into<Length> + PartialEq + Copy + 'static>(
        self,
        width: impl IntoMaybeReactive<L>,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(width.maybe_reactive(), |layout, &width| {
            layout.size.width =
                Length::InfiniteWindow(width.into().try_into().unwrap());
        });
        self
    }
}

impl<W> SizedWidget<W> for Scrollable<W, ColDir>
where
    W: WidgetCtx,
    W::Styler: WidgetStylist<ScrollableStyle<W::Color>>,
{
    fn height<L: Into<Length> + PartialEq + Copy + 'static>(
        self,
        height: impl IntoMaybeReactive<L> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(height.maybe_reactive(), |layout, &height| {
            layout.size.height =
                Length::InfiniteWindow(height.into().try_into().unwrap());
        });
        self
    }
}

impl<W: WidgetCtx, Dir: Direction + 'static> FontSettingWidget<W>
    for Scrollable<W, Dir>
where
    W::Styler: WidgetStylist<ScrollableStyle<W::Color>>,
{
}

impl<W, Dir> Widget<W> for Scrollable<W, Dir>
where
    W: WidgetCtx,
    Dir: Direction,
    W::Styler: WidgetStylist<ScrollableStyle<W::Color>>,
{
    fn meta(&self, id: ElId) -> crate::widget::MetaTree {
        let content_tree = self.content.meta(id);
        MetaTree {
            data: Meta::none.memo(),
            children: vec![content_tree].inert().memo(),
        }
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);
        ctx.pass_to_child(self.layout, &mut self.content);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn render(
        &self,
        ctx: &mut crate::widget::RenderCtx<'_, W>,
    ) -> crate::widget::RenderResult {
        // Note: Layouts can be untracked because relayout is full-redraw
        let child_layout = ctx.layout.children().next();
        let child_layout = child_layout.as_ref().unwrap();

        ctx.render_self(|ctx| {
            let style = self.style.get();

            Block::from_layout_style(
                ctx.layout.outer,
                self.layout.with(|layout| layout.block_model()),
                style.container,
            )
            .render(ctx.renderer())?;

            let mut content_length = child_layout.outer.size.main(Dir::AXIS);
            let scrollable_length = ctx.layout.inner.size.main(Dir::AXIS);

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
                    Axis::X => ctx.layout.inner.anchor_point(
                        embedded_graphics::geometry::AnchorPoint::BottomLeft,
                    ),
                    Axis::Y => ctx.layout.inner.anchor_point(
                        embedded_graphics::geometry::AnchorPoint::TopRight,
                    ),
                };

                let track_end = ctx
                    .layout
                    .inner
                    .bottom_right()
                    .unwrap_or(ctx.layout.inner.top_left);

                let scrollbar_translation =
                    Dir::AXIS.canon(0, -((style.scrollbar_width as i32) / 2));

                let track_line = Line::new(track_start, track_end)
                    .translate(scrollbar_translation);

                // Draw track
                track_line
                    .into_styled(style.track_style())
                    .render(ctx.renderer())?;

                let thumb_len = ((scrollable_length as f32)
                    * ((scrollable_length as f32) / (content_length as f32)))
                    as u32;
                let thumb_len = thumb_len.max(1);
                let thumb_offset =
                    (((scrollable_length as f32) / (content_length as f32))
                        * (offset as f32)) as u32;

                let thumb_start = track_start
                    + Dir::AXIS.canon::<Point>(thumb_offset as i32, 0);

                Line::new(
                    thumb_start,
                    thumb_start + Dir::AXIS.canon::<Point>(thumb_len as i32, 0),
                )
                .translate(scrollbar_translation)
                .into_styled(style.thumb_style())
                .render(ctx.renderer())?;
            }

            ctx.render_focus_outline(ctx.id)
        })?;

        ctx.render_part("scroll", |ctx| {
            let state = self.state.get();
            // // TODO: Should be clipping outer rect???!??!?
            ctx.clip_inner(|ctx| {
                ctx.for_child(
                    self.content.id(),
                    &child_layout
                        .translate(Dir::AXIS.canon(-(state.offset as i32), 0)),
                    |ctx| self.content.render(ctx),
                )
            })
        })
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        let current_state = self.state.get();

        match self.mode {
            ScrollableMode::Interactive => {
                // FocusEvent can be treated as ScrollEvent thus handle it
                // before focus move
                if current_state.active && ctx.is_focused() {
                    // TODO: Right scrolling handling
                    if let Some(offset) = ctx.event.interpret_as_rotation() {
                        let max_offset = self.max_offset(&ctx);

                        let new_offset = ((current_state.offset as i64)
                            + (offset as i64))
                            .clamp(0, max_offset as i64)
                            as u32;

                        if new_offset != current_state.offset {
                            self.state.update(|state| {
                                state.offset = new_offset;
                            });
                        }

                        return ctx.capture();
                    }
                }

                ctx.handle_focusable(|ctx, pressed| {
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

                        ctx.capture()
                    } else {
                        ctx.ignore()
                    }
                })
            },
            ScrollableMode::Tracker => {
                // If nothing was focused before passing event to children then
                // change of focus means moving focus to a widget inside
                // scrollable content

                let content_response = ctx.pass_to_child(&mut self.content);

                // TODO: Better need distinct `IsInteraction` event for such cases or define which events are considered an "interaction". For example, clicking on a button or focusing it is an interaction, but scrolling may be not, idk?
                // Now, I am checking if any child captured the event for tracking.
                if let EventResponse::Break(Capture::Captured(capture)) =
                    &content_response
                {
                    let new_offset = capture
                        .absolute_position
                        .main(Dir::AXIS)
                        .saturating_sub(
                            ctx.layout.inner.top_left.main(Dir::AXIS),
                        ) as u32;
                    let new_offset = new_offset.clamp(0, self.max_offset(&ctx));

                    if current_state.offset != new_offset {
                        self.state.update(|state| {
                            state.offset = new_offset;
                        });
                    }
                }

                content_response
            },
        }
    }
}
