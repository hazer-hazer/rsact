use crate::{
    declare_widget_style,
    event::{MouseButton, MouseEvent},
    widget::{Meta, MetaTree, SizedWidget, prelude::*},
};
use core::marker::PhantomData;
use rsact_reactive::prelude::*;

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
    /// Last cursor position when pointer-dragging, for delta calculation
    pub drag_pos: Option<i32>,
    // TODO: `is_scrolling` state when time source added. Reset it after
    // timeout
}

impl ScrollableState {
    pub fn none() -> Self {
        Self { offset: 0, focus_pressed: false, active: false, drag_pos: None }
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

    fn track_draw_style(&self) -> DrawStyle<C> {
        DrawStyle {
            fill: None,
            stroke: self.track_color.get(),
            stroke_width: self.scrollbar_width,
            stroke_alignment: StrokeAlignment::Center,
        }
    }

    fn thumb_draw_style(&self) -> DrawStyle<C> {
        DrawStyle {
            fill: None,
            stroke: self.thumb_color.get(),
            stroke_width: self.scrollbar_width,
            stroke_alignment: StrokeAlignment::Center,
        }
    }
}

pub struct Scrollable<W: WidgetCtx, Dir: Direction> {
    state: Signal<ScrollableState>,
    style: Option<
        Box<dyn Fn(ScrollableStyle<W::Color>) -> ScrollableStyle<W::Color>>,
    >,
    content: El<W>,
    layout: Layout,
    mode: ScrollableMode,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx> Scrollable<W, RowDir> {
    pub fn horizontal(content: impl Into<El<W>>) -> Self {
        Self::new(content)
    }
}

impl<W: WidgetCtx> Scrollable<W, ColDir> {
    pub fn vertical(content: impl Into<El<W>>) -> Self {
        Self::new(content)
    }
}

impl<W: WidgetCtx, Dir: Direction> Scrollable<W, Dir> {
    pub fn new(content: impl Into<El<W>>) -> Self {
        let content = content.into();
        let state = create_signal(ScrollableState::none());

        let layout = Layout::scrollable::<Dir>(content.layout());

        Self {
            content,
            state,
            style: None,
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
        mut self,
        styler: impl Fn(ScrollableStyle<W::Color>) -> ScrollableStyle<W::Color>
        + 'static,
    ) -> Self {
        self.style = Some(Box::new(styler));
        self
    }

    fn max_offset(&self, ctx: &EventCtx<'_, W>) -> u32 {
        let content_length =
            ctx.layout.children().next().unwrap().inner.size.main(Dir::AXIS);

        content_length.saturating_sub(ctx.layout.inner.size.main(Dir::AXIS))
    }
}

impl<W: WidgetCtx> SizedWidget<W> for Scrollable<W, RowDir> {
    fn width<L: Into<Length> + PartialEq + Copy + 'static>(
        self,
        width: impl IntoMaybeReactive<L>,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(width.maybe_reactive(), |layout, &width| {
            layout.size.set_width(Length::InfiniteWindow(
                width.into().try_into().unwrap(),
            ));
        });
        self
    }
}

impl<W: WidgetCtx> SizedWidget<W> for Scrollable<W, ColDir> {
    fn height<L: Into<Length> + PartialEq + Copy + 'static>(
        self,
        height: impl IntoMaybeReactive<L> + 'static,
    ) -> Self
    where
        Self: Sized + 'static,
    {
        self.layout().setter(height.maybe_reactive(), |layout, &height| {
            layout.size.set_height(Length::InfiniteWindow(
                height.into().try_into().unwrap(),
            ));
        });
        self
    }
}

impl<W: WidgetCtx, Dir: Direction + 'static> FontSettingWidget<W>
    for Scrollable<W, Dir>
{
}

impl<W: WidgetCtx, Dir: Direction> Widget<W> for Scrollable<W, Dir> {
    fn meta(&self, id: ElId) -> crate::widget::MetaTree {
        let content_tree = self.content.meta(id);
        MetaTree::new(Meta::none(), vec![content_tree].inert())
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    fn render(
        &self,
        mut ctx: crate::widget::RenderCtx<'_, W>,
    ) -> crate::widget::RenderResult {
        // Note: Layouts can be untracked because relayout is full-redraw
        let child_layout = ctx.layout.children().next();
        let child_layout = child_layout.as_ref().unwrap();

        ctx.render_self("Scrollable", |mut ctx| {
            let style = ctx.get_style(|t| t.scrollable, self.style.as_deref());

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
                let base = ctx.theme.with(|theme| theme.scrollable);
                let style =
                    self.style.as_ref().map(|f| f(base)).unwrap_or(base);

                let track_start = match Dir::AXIS {
                    Axis::X => {
                        ctx.layout.inner.anchor_point(AnchorPoint::BottomLeft)
                    },
                    Axis::Y => {
                        ctx.layout.inner.anchor_point(AnchorPoint::TopRight)
                    },
                };

                let track_end = ctx
                    .layout
                    .inner
                    .bottom_right()
                    .unwrap_or(ctx.layout.inner.top_left);

                let scrollbar_translation: Point =
                    Dir::AXIS.canon(0, -((style.scrollbar_width as i32) / 2));

                // Draw track
                ctx.renderer().line(
                    track_start + scrollbar_translation,
                    track_end + scrollbar_translation,
                    &style.track_draw_style(),
                )?;

                let thumb_len = ((scrollable_length as f32)
                    * ((scrollable_length as f32) / (content_length as f32)))
                    as u32;
                let thumb_len = thumb_len.max(1);
                let thumb_offset =
                    (((scrollable_length as f32) / (content_length as f32))
                        * (offset as f32)) as u32;

                let thumb_start = track_start
                    + Dir::AXIS.canon::<Point>(thumb_offset as i32, 0);

                ctx.renderer().line(
                    thumb_start + scrollbar_translation,
                    thumb_start
                        + Dir::AXIS.canon::<Point>(thumb_len as i32, 0)
                        + scrollbar_translation,
                    &style.thumb_draw_style(),
                )?;
            }

            ctx.render_focus_outline(ctx.id)
        })?;

        ctx.render_part("scroll", |mut ctx| {
            let state = self.state.get();
            // // TODO: Should be clipping outer rect???!??!?
            ctx.clip_inner(|mut ctx| {
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

                // TODO: Make this configurable.
                // Mouse drag: ButtonDown starts capture, MouseMove drags, ButtonUp releases
                match ctx.event {
                    Event::Mouse(MouseEvent::ButtonDown(
                        MouseButton::Left,
                        _,
                    )) => {
                        if let Some(pt) = ctx.cursor_pos() {
                            if ctx.layout.outer.contains(pt) {
                                let axis_pos = pt.main(Dir::AXIS);
                                ctx.capture_pointer();
                                self.state
                                    .update(|s| s.drag_pos = Some(axis_pos));
                                return ctx.capture();
                            }
                        }
                    },
                    Event::Mouse(MouseEvent::MouseMove(_)) => {
                        if current_state.drag_pos.is_some() {
                            if let Some(pt) = ctx.cursor_pos() {
                                let axis_pos = pt.main(Dir::AXIS);
                                let drag_pos = current_state.drag_pos.unwrap();
                                let delta = drag_pos - axis_pos;
                                let max_offset = self.max_offset(&ctx);

                                let new_offset = ((current_state.offset as i64)
                                    + (delta as i64))
                                    .clamp(0, max_offset as i64)
                                    as u32;

                                self.state.update(|s| {
                                    s.offset = new_offset;
                                    s.drag_pos = Some(axis_pos);
                                });

                                return ctx.capture();
                            }
                        }
                    },
                    Event::Mouse(MouseEvent::ButtonUp(
                        MouseButton::Left,
                        _,
                    )) => {
                        if current_state.drag_pos.is_some() {
                            ctx.release_pointer();
                            self.state.update(|s| s.drag_pos = None);
                            return ctx.capture();
                        }
                    },
                    _ => {},
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
