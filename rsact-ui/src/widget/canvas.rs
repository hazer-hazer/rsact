use crate::{
    layout::length::LengthSize,
    render::primitives::{
        PrimitiveKind, arc::Arc, circle::Circle, ellipse::Ellipse, line::Line,
        polygon::Polygon, rounded_rect::RoundedRect, sector::Sector,
    },
    widget::prelude::*,
};
use alloc::collections::vec_deque::VecDeque;
use rsact_render::image::{
    DrawImage, Image, ImageOwned, storage::ImageStorage,
};

slotmap::new_key_type! {
    pub struct CanvasImageId;
}

#[derive(Clone, PartialEq, Debug)]
pub struct CanvasImage<'a, C: Color> {
    image: Image<'a, CanvasImageId, C>,
    position: Point,
}

impl<'a, C: Color> CanvasImage<'a, C> {
    pub fn new(
        image: impl Into<Image<'a, CanvasImageId, C>>,
        position: Point,
    ) -> Self {
        Self { image: image.into(), position }
    }

    pub const fn image(&self) -> &Image<'a, CanvasImageId, C> {
        &self.image
    }

    pub const fn position(&self) -> Point {
        self.position
    }

    pub fn translate_mut(&mut self, offset: Point) -> &mut Self {
        self.position += offset;
        self
    }
}

// TODO: CanvasDrawable trait? - No, cause gives two different ways to implement
// one thing. CanvasDrawable would lead us to use dynamic dispatch and boxes
// that I avoided using DrawCommand enum.

// TODO: Replace with box dyn primitive? - No, performance drawbacks, we know
// all values and don't plan to support custom ones. TODO: Maybe better add
// IntoDrawCommand for primitives
#[derive(Clone, PartialEq, Debug)]
pub enum DrawCommand<C: Color + 'static> {
    /// Actually useless for now as we clear the whole screen each frame :(
    Clear(C),
    ClearRect(Rect, C),
    Primitive(PrimitiveKind, DrawStyle<C>),
    // TODO: Image as resources with ID stored in some storage inside UI
    // context?
    Image(CanvasImage<'static, C>),
}

// TODO: Different color in DrawQueue mapped to renderer target color?

#[derive(Clone, Copy)]
pub struct DrawQueue<C: Color + 'static> {
    queue: Signal<VecDeque<DrawCommand<C>>>,
    // TODO: We don't need Signal here I think. Just a mutable stored value
    image_storage: Signal<ImageStorage<CanvasImageId, C>>,
}

impl<C: Color> DrawQueue<C> {
    pub fn new() -> Self {
        Self {
            queue: create_signal(VecDeque::new()),
            image_storage: ImageStorage::new().signal(),
        }
    }

    pub fn add_image(&mut self, image: ImageOwned<C>) -> CanvasImageId {
        self.image_storage.update_untracked(|storage| storage.add(image))
    }

    pub fn draw_once(
        &mut self,
        command: impl Into<DrawCommand<C>>,
    ) -> &mut Self {
        // TODO: update_untracked?
        self.queue.update(|queue| queue.push_back(command.into()));
        self
    }

    pub fn clear(&mut self, color: C) -> &mut Self {
        self.draw_once(DrawCommand::Clear(color));
        self
    }

    pub fn clear_rect(&mut self, rect: Rect, color: C) -> &mut Self {
        self.draw_once(DrawCommand::ClearRect(rect, color));
        self
    }

    pub fn primitive(
        &mut self,
        primitive: impl Into<PrimitiveKind>,
        style: DrawStyle<C>,
    ) -> &mut Self {
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

    pub fn draw(&mut self, commands: Memo<Vec<DrawCommand<C>>>) {
        self.queue.setter(commands, |queue, commands| {
            commands.iter().cloned().for_each(|command| {
                queue.push_back(command);
            });
        });
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

impl<W: WidgetCtx> LayoutWidget<W> for Canvas<W> {
    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }
}

impl<W: WidgetCtx> SizedWidget<W> for Canvas<W> {}

impl<W: WidgetCtx> Widget<W> for Canvas<W> {
    fn debug_name(&self) -> &'static str {
        "Canvas"
    }

    fn build(&mut self, ctx: BuildCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self(|ctx| {
            self.queue.queue.track();

            while let Some(command) = self.queue.pop() {
                match command {
                    DrawCommand::Clear(color) => {
                        let outer = ctx.layout.outer;
                        ctx.renderer.fill_solid(outer, color)?;
                    },
                    DrawCommand::ClearRect(rect, color) => {
                        ctx.renderer.fill_solid(rect, color)?;
                    },
                    DrawCommand::Primitive(primitive, style) => match primitive
                    {
                        PrimitiveKind::Arc(Arc {
                            top_left,
                            diameter,
                            start,
                            sweep,
                        }) => {
                            ctx.renderer.arc(
                                top_left, diameter, start, sweep, &style,
                            )?;
                        },
                        PrimitiveKind::Circle(Circle {
                            top_left,
                            diameter,
                        }) => {
                            ctx.renderer.circle(top_left, diameter, &style)?;
                        },
                        PrimitiveKind::Ellipse(Ellipse { top_left, size }) => {
                            ctx.renderer
                                .ellipse(Rect::new(top_left, size), &style)?;
                        },
                        PrimitiveKind::Line(Line { from, to }) => {
                            ctx.renderer.line(from, to, &style)?;
                        },
                        PrimitiveKind::Polygon(Polygon {
                            // TODO
                            translation: _,
                            vertices,
                        }) => {
                            ctx.renderer.polygon(&vertices, &style)?;
                        },
                        PrimitiveKind::Rect(rect) => {
                            ctx.renderer.rect(rect, &style)?;
                        },
                        PrimitiveKind::RoundedRect(RoundedRect {
                            rect,
                            corners,
                        }) => {
                            ctx.renderer.rounded_rect(rect, corners, &style)?;
                        },
                        PrimitiveKind::Sector(Sector {
                            top_left,
                            diameter,
                            start,
                            sweep,
                        }) => {
                            ctx.renderer.sector(
                                top_left, diameter, start, sweep, &style,
                            )?;
                        },
                    },
                    DrawCommand::Image(image) => match image.image() {
                        Image::Owned(image_owned) => {
                            ctx.renderer.image(DrawImage::new(
                                image_owned.as_ref(),
                                image.position(),
                            ))?;
                        },
                        &Image::Ref(image_ref) => {
                            ctx.renderer.image(DrawImage::new(
                                image_ref,
                                image.position(),
                            ))?;
                        },
                        &Image::Id(image_id) => {
                            self.queue.image_storage.with(|storage| {
                                ctx.renderer.image(DrawImage::new(
                                    storage.get(image_id).ok_or(())?,
                                    image.position(),
                                ))
                            })?;
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
