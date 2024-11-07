use embedded_graphics::primitives::{PrimitiveStyle, Rectangle, Styled};

use crate::{
    render::{
        primitives::{
            arc::Arc, circle::Circle, ellipse::Ellipse, line::Line,
            polygon::Polygon, rounded_rect::RoundedRect, sector::Sector,
        },
        Renderable,
    },
    widget::prelude::*,
};

// TODO: Replace with box dyn primitive or something else?
pub enum DrawCommand<C: Color> {
    Arc(Styled<Arc, PrimitiveStyle<C>>),
    Circle(Styled<Circle, PrimitiveStyle<C>>),
    Ellipse(Styled<Ellipse, PrimitiveStyle<C>>),
    Line(Styled<Line, PrimitiveStyle<C>>),
    Polygon(Styled<Polygon, PrimitiveStyle<C>>),
    Rect(Styled<Rectangle, PrimitiveStyle<C>>),
    RoundedRect(Styled<RoundedRect, PrimitiveStyle<C>>),
    Sector(Styled<Sector, PrimitiveStyle<C>>),
}

#[derive(Clone, Copy)]
pub struct DrawQueue<C: Color + 'static> {
    queue: Signal<Vec<DrawCommand<C>>>,
}

impl<C: Color> DrawQueue<C> {
    pub fn new() -> Self {
        Self { queue: create_signal(Vec::new()) }
    }

    pub fn draw(self, command: DrawCommand<C>) -> Self {
        // TODO: update_untracked?
        self.queue.update(|queue| queue.push(command));
        self
    }

    pub fn arc(self, arc: Styled<Arc, PrimitiveStyle<C>>) -> Self {
        self.draw(DrawCommand::Arc(arc));
        self
    }

    pub fn circle(self, circle: Styled<Circle, PrimitiveStyle<C>>) -> Self {
        self.draw(DrawCommand::Circle(circle));
        self
    }

    pub fn ellipse(self, ellipse: Styled<Ellipse, PrimitiveStyle<C>>) -> Self {
        self.draw(DrawCommand::Ellipse(ellipse));
        self
    }

    pub fn line(self, line: Styled<Line, PrimitiveStyle<C>>) -> Self {
        self.draw(DrawCommand::Line(line));
        self
    }

    pub fn polygon(self, polygon: Styled<Polygon, PrimitiveStyle<C>>) -> Self {
        self.draw(DrawCommand::Polygon(polygon));
        self
    }

    pub fn rect(self, rect: Styled<Rectangle, PrimitiveStyle<C>>) -> Self {
        self.draw(DrawCommand::Rect(rect));
        self
    }

    pub fn rounded_rect(
        self,
        round_rect: Styled<RoundedRect, PrimitiveStyle<C>>,
    ) -> Self {
        self.draw(DrawCommand::RoundedRect(round_rect));
        self
    }

    pub fn sector(self, sector: Styled<Sector, PrimitiveStyle<C>>) -> Self {
        self.draw(DrawCommand::Sector(sector));
        self
    }

    fn pop(self) -> Option<DrawCommand<C>> {
        // TODO: Should notify on something popped?
        // No, because drawing is synchronous
        self.queue.update_untracked(|queue| queue.pop())
    }
}

pub struct Canvas<W: WidgetCtx> {
    queue: DrawQueue<W::Color>,
    layout: Signal<Layout>,
}

impl<W: WidgetCtx> Canvas<W> {
    pub fn new(queue: DrawQueue<W::Color>) -> Self {
        Self {
            queue,
            layout: Layout {
                kind: LayoutKind::Edge,
                size: Size::new_equal(Length::fill()),
            }
            .into_signal(),
        }
    }
}

impl<W: WidgetCtx> Widget<W> for Canvas<W> {
    fn meta(&self) -> MetaTree {
        MetaTree::childless(Meta::none)
    }

    fn on_mount(&mut self, ctx: super::MountCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        MemoTree::childless(self.layout)
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        while let Some(command) = self.queue.pop() {
            match command {
                DrawCommand::Arc(arc) => arc.render(ctx.renderer)?,
                DrawCommand::Circle(circle) => circle.render(ctx.renderer)?,
                DrawCommand::Ellipse(ellipse) => {
                    ellipse.render(ctx.renderer)?
                },
                DrawCommand::Line(line) => line.render(ctx.renderer)?,
                DrawCommand::Polygon(polygon) => {
                    polygon.render(ctx.renderer)?
                },
                DrawCommand::Rect(rect) => rect.render(ctx.renderer)?,
                DrawCommand::RoundedRect(rounded_rect) => {
                    rounded_rect.render(ctx.renderer)?
                },
                DrawCommand::Sector(sector) => sector.render(ctx.renderer)?,
            }
        }

        Ok(())
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse<W> {
        let _ = ctx;
        ctx.ignore()
    }
}
