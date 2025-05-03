use alloc::collections::vec_deque::VecDeque;
use embedded_graphics::{
    prelude::DrawTarget,
    primitives::{PrimitiveStyle, Rectangle, Styled},
};

use crate::{
    render::{
        Renderable,
        primitives::{
            arc::Arc, circle::Circle, ellipse::Ellipse, line::Line,
            polygon::Polygon, rounded_rect::RoundedRect, sector::Sector,
        },
    },
    widget::prelude::*,
};

// pub trait CanvasDrawable<C: Color> {
//     fn draw_on(self, queue: DrawQueue<C>);
// }

// macro_rules! impl_canvas_drawable_prim {
//     ($($ty: ty: $method: ident),* $(,)?) => {
//         impl<C: Color> CanvasDrawable<C> for Styled<Arc, PrimitiveStyle<C>> {
//             fn draw_on(self, queue: DrawQueue<C>) {
//                 queue.arc(self);
//             }
//         }
//     };
// }

macro_rules! impl_into_draw_command_primitive {
    ($($prim: ident),* $(,)?) => {
        $(impl<C: Color> Into<DrawCommand<C>>
            for Styled<$prim, PrimitiveStyle<C>>
        {
            fn into(self) -> DrawCommand<C> {
                DrawCommand::$prim(self)
            }
        })*
    };
}

impl_into_draw_command_primitive!(
    Arc,
    Circle,
    Ellipse,
    Line,
    Polygon,
    Rectangle,
    RoundedRect,
    Sector
);

// TODO: CanvasDrawable trait? - No, cause gives two different ways to implement one thing: `Into<DrawCommand>` and `CanvasDrawable`. CanvasDrawable would lead us to use dynamic dispatch and boxes that I avoided using DrawCommand enum.

// TODO: Replace with box dyn primitive?
#[derive(Clone, PartialEq, Debug)]
pub enum DrawCommand<C: Color> {
    /// Actually useless for now as we clear the whole screen each frame :(
    Clear(C),
    ClearRect(Rectangle, C),
    Arc(Styled<Arc, PrimitiveStyle<C>>),
    Circle(Styled<Circle, PrimitiveStyle<C>>),
    Ellipse(Styled<Ellipse, PrimitiveStyle<C>>),
    Line(Styled<Line, PrimitiveStyle<C>>),
    Polygon(Styled<Polygon, PrimitiveStyle<C>>),
    Rectangle(Styled<Rectangle, PrimitiveStyle<C>>),
    RoundedRect(Styled<RoundedRect, PrimitiveStyle<C>>),
    Sector(Styled<Sector, PrimitiveStyle<C>>),
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

    pub fn clear_rect(self, rect: Rectangle, color: C) -> Self {
        self.draw_once(DrawCommand::ClearRect(rect, color));
        self
    }

    pub fn arc(self, arc: Styled<Arc, PrimitiveStyle<C>>) -> Self {
        self.draw_once(DrawCommand::Arc(arc));
        self
    }

    pub fn circle(self, circle: Styled<Circle, PrimitiveStyle<C>>) -> Self {
        self.draw_once(DrawCommand::Circle(circle));
        self
    }

    pub fn ellipse(self, ellipse: Styled<Ellipse, PrimitiveStyle<C>>) -> Self {
        self.draw_once(DrawCommand::Ellipse(ellipse));
        self
    }

    pub fn line(self, line: Styled<Line, PrimitiveStyle<C>>) -> Self {
        self.draw_once(DrawCommand::Line(line));
        self
    }

    pub fn polygon(self, polygon: Styled<Polygon, PrimitiveStyle<C>>) -> Self {
        self.draw_once(DrawCommand::Polygon(polygon));
        self
    }

    pub fn rect(self, rect: Styled<Rectangle, PrimitiveStyle<C>>) -> Self {
        self.draw_once(DrawCommand::Rectangle(rect));
        self
    }

    pub fn rounded_rect(
        self,
        round_rect: Styled<RoundedRect, PrimitiveStyle<C>>,
    ) -> Self {
        self.draw_once(DrawCommand::RoundedRect(round_rect));
        self
    }

    pub fn sector(self, sector: Styled<Sector, PrimitiveStyle<C>>) -> Self {
        self.draw_once(DrawCommand::Sector(sector));
        self
    }

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
        Self { queue, layout: Layout::edge(Size::new_equal(Length::fill())) }
    }
}

impl<W: WidgetCtx> SizedWidget<W> for Canvas<W> {}

impl<W: WidgetCtx> Widget<W> for Canvas<W> {
    fn meta(&self) -> MetaTree {
        MetaTree::childless(Meta::none)
    }

    fn on_mount(&mut self, ctx: super::MountCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> &Layout {
        &self.layout
    }

    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }

    fn render(&self, ctx: RenderCtx<W>) -> Computed<()> {
        self.queue.queue.track();

        // TODO: Right DrawResult error
        while let Some(command) = self.queue.pop() {
            match command {
                DrawCommand::Clear(color) => {
                    ctx.renderer.clear(color).ok().unwrap()
                },
                DrawCommand::ClearRect(rect, color) => {
                    ctx.renderer.fill_solid(&rect, color).ok().unwrap()
                },
                DrawCommand::Arc(arc) => arc.render(ctx.renderer)?,
                DrawCommand::Circle(circle) => circle.render(ctx.renderer)?,
                DrawCommand::Ellipse(ellipse) => {
                    ellipse.render(ctx.renderer)?
                },
                DrawCommand::Line(line) => line.render(ctx.renderer)?,
                DrawCommand::Polygon(polygon) => {
                    polygon.render(ctx.renderer)?
                },
                DrawCommand::Rectangle(rect) => rect.render(ctx.renderer)?,
                DrawCommand::RoundedRect(rounded_rect) => {
                    rounded_rect.render(ctx.renderer)?
                },
                DrawCommand::Sector(sector) => sector.render(ctx.renderer)?,
            }
        }

        Ok(())
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse {
        let _ = ctx;
        ctx.ignore()
    }
}
