use crate::{
    layout::length::LengthSize,
    render::primitives::{
        Primitive, arc::Arc, circle::Circle, ellipse::Ellipse, line::Line,
        polygon::Polygon, rounded_rect::RoundedRect, sector::Sector,
    },
    widget::prelude::*,
};
use alloc::collections::vec_deque::VecDeque;

// TODO: CanvasDrawable trait? - No, cause gives two different ways to implement one thing. CanvasDrawable would lead us to use dynamic dispatch and boxes that I avoided using DrawCommand enum.

// TODO: Replace with box dyn primitive?
// TODO: Maybe better add IntoDrawCommand for primitives
#[derive(Clone, PartialEq, Debug)]
pub enum DrawCommand<C: Color> {
    /// Actually useless for now as we clear the whole screen each frame :(
    Clear(C),
    ClearRect(Rect, C),
    Primitive(Primitive, DrawStyle<C>),
}

// TODO: Different color in DrawQueue mapped to renderer target color?

#[derive(Clone, Copy)]
pub struct DrawQueue<C: Color + 'static> {
    queue: Signal<VecDeque<DrawCommand<C>>>,
}

impl<C: Color> DrawQueue<C> {
    pub fn new() -> Self {
        Self { queue: create_signal(VecDeque::new()) }
    }

    pub fn draw_once(mut self, command: impl Into<DrawCommand<C>>) -> Self {
        // TODO: update_untracked?
        self.queue.update(|queue| queue.push_back(command.into()));
        self
    }

    pub fn clear(self, color: C) -> Self {
        self.draw_once(DrawCommand::Clear(color));
        self
    }

    pub fn clear_rect(self, rect: Rect, color: C) -> Self {
        self.draw_once(DrawCommand::ClearRect(rect, color));
        self
    }

    pub fn primitive(
        self,
        primitive: impl Into<Primitive>,
        style: DrawStyle<C>,
    ) -> Self {
        self.draw_once(DrawCommand::Primitive(primitive.into(), style));
        self
    }

    // pub fn arc(
    //     self,
    //     top_left: Point,
    //     diameter: u32,
    //     start: Angle,
    //     sweep: Angle,
    //     style: DrawStyle<C>,
    // ) -> Self {
    //     self.draw_once(DrawCommand::Arc {
    //         top_left,
    //         diameter,
    //         start,
    //         sweep,
    //         style,
    //     });
    //     self
    // }

    // pub fn circle(
    //     self,
    //     top_left: Point,
    //     diameter: u32,
    //     style: DrawStyle<C>,
    // ) -> Self {
    //     self.draw_once(DrawCommand::Circle { top_left, diameter, style });
    //     self
    // }

    // pub fn ellipse(self, bounding_box: Rect, style: DrawStyle<C>) -> Self {
    //     self.draw_once(DrawCommand::Ellipse { bounding_box, style });
    //     self
    // }

    // pub fn line(self, from: Point, to: Point, style: DrawStyle<C>) -> Self {
    //     self.draw_once(DrawCommand::Line { from, to, style });
    //     self
    // }

    // pub fn polygon(
    //     self,
    //     points: alloc::vec::Vec<Point>,
    //     style: DrawStyle<C>,
    // ) -> Self {
    //     self.draw_once(DrawCommand::Polygon { points, style });
    //     self
    // }

    // pub fn rect(self, rect: Rect, style: DrawStyle<C>) -> Self {
    //     self.draw_once(DrawCommand::Rect { rect, style });
    //     self
    // }

    // pub fn rounded_rect(
    //     self,
    //     rect: Rect,
    //     corners: CornerRadii,
    //     style: DrawStyle<C>,
    // ) -> Self {
    //     self.draw_once(DrawCommand::RoundedRect { rect, corners, style });
    //     self
    // }

    // pub fn sector(
    //     self,
    //     top_left: Point,
    //     diameter: u32,
    //     start: Angle,
    //     sweep: Angle,
    //     style: DrawStyle<C>,
    // ) -> Self {
    //     self.draw_once(DrawCommand::Sector {
    //         top_left,
    //         diameter,
    //         start,
    //         sweep,
    //         style,
    //     });
    //     self
    // }

    /// Delegate DrawQueue as settable signal for single command
    pub fn draw(mut self, commands: Memo<Vec<DrawCommand<C>>>) -> Self {
        self.queue.setter(commands, |queue, commands| {
            commands.iter().cloned().for_each(|command| {
                queue.push_back(command);
            });
        });
        self
    }

    fn pop(mut self) -> Option<DrawCommand<C>> {
        // TODO: Should notify on something popped?
        // No, because drawing is synchronous
        self.queue.update_untracked(|queue| queue.pop_front())
    }
}

pub struct Canvas<W: WidgetCtx> {
    queue: DrawQueue<W::Color>,
    layout: Layout,
}

impl<W: WidgetCtx> Canvas<W> {
    pub fn new(queue: DrawQueue<W::Color>) -> Self {
        Self {
            queue,
            layout: Layout::edge(LengthSize::new_equal(Length::fill())),
        }
    }
}

impl<W: WidgetCtx> SizedWidget<W> for Canvas<W> {}

impl<W: WidgetCtx> Widget<W> for Canvas<W> {
    fn meta(&self, _: ElId) -> MetaTree {
        MetaTree::none()
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, ctx: &mut RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self("Canvas", |ctx| {
            self.queue.queue.track();

            while let Some(command) = self.queue.pop() {
                match command {
                    DrawCommand::Clear(color) => {
                        let outer = ctx.layout.outer;
                        ctx.renderer().fill_solid(&outer, color)?;
                    },
                    DrawCommand::ClearRect(rect, color) => {
                        ctx.renderer().fill_solid(&rect, color)?;
                    },
                    DrawCommand::Primitive(primitive, style) => match primitive
                    {
                        Primitive::Arc(Arc {
                            top_left,
                            diameter,
                            start,
                            sweep,
                        }) => {
                            ctx.renderer().arc(
                                top_left, diameter, start, sweep, &style,
                            )?;
                        },
                        Primitive::Circle(Circle { top_left, diameter }) => {
                            ctx.renderer()
                                .circle(top_left, diameter, &style)?;
                        },
                        Primitive::Ellipse(Ellipse { top_left, size }) => {
                            ctx.renderer()
                                .ellipse(&Rect::new(top_left, size), &style)?;
                        },
                        Primitive::Line(Line { from, to }) => {
                            ctx.renderer().line(from, to, &style)?;
                        },
                        Primitive::Polygon(Polygon {
                            // TODO
                            translation,
                            vertices,
                        }) => {
                            ctx.renderer().polygon(&vertices, &style)?;
                        },
                        Primitive::Rect(rect) => {
                            ctx.renderer().rect(&rect, &style)?;
                        },
                        Primitive::RoundedRect(RoundedRect {
                            rect,
                            corners,
                        }) => {
                            ctx.renderer()
                                .rounded_rect(&rect, corners, &style)?;
                        },
                        Primitive::Sector(Sector {
                            top_left,
                            diameter,
                            start,
                            sweep,
                        }) => {
                            ctx.renderer().sector(
                                top_left, diameter, start, sweep, &style,
                            )?;
                        },
                    },
                }
            }

            Ok(())
        })
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        let _ = ctx;
        ctx.ignore()
    }
}
