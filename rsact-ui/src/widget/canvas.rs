use crate::{
    render::{Arc, Line, Rect},
    widget::prelude::*,
};

pub enum DrawCommand<C: Color> {
    Line(Line<C>),
    Rect(Rect<C>),
    Arc(Arc<C>),
    Block(Block<C>),
}

#[derive(Clone, Copy)]
pub struct DrawQueue<C: Color + 'static> {
    queue: Signal<Vec<DrawCommand<C>>>,
}

impl<C: Color> DrawQueue<C> {
    pub fn new() -> Self {
        Self { queue: use_signal(Vec::new()) }
    }

    pub fn draw(self, command: DrawCommand<C>) -> Self {
        // TODO: update_untracked?
        self.queue.update(|queue| queue.push(command));
        self
    }

    pub fn line(self, line: Line<C>) -> Self {
        self.draw(DrawCommand::Line(line));
        self
    }

    pub fn rect(self, rect: Rect<C>) -> Self {
        self.draw(DrawCommand::Rect(rect));
        self
    }

    pub fn arc(self, arc: Arc<C>) -> Self {
        self.draw(DrawCommand::Arc(arc));
        self
    }

    pub fn block(self, block: Block<C>) -> Self {
        self.draw(DrawCommand::Block(block));
        self
    }

    fn pop(self) -> Option<DrawCommand<C>> {
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
        MetaTree::childless(Meta::none())
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
                DrawCommand::Line(line) => ctx.renderer.line(line)?,
                DrawCommand::Rect(rect) => ctx.renderer.rect(rect)?,
                DrawCommand::Arc(arc) => ctx.renderer.arc(arc)?,
                DrawCommand::Block(block) => ctx.renderer.block(block)?,
            }
        }

        Ok(())
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse<W> {
        let _ = ctx;
        ctx.ignore()
    }
}
