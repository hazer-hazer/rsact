use super::prelude::*;

/// Widget dependent on boolean memo.
/// When memo is false widget:
/// - Has zero layout
/// - Isn't drawn
/// - Ignores events
pub struct Show<W: WidgetCtx> {
    show: Memo<bool>,
    el: El<W>,
}

impl<W: WidgetCtx> Show<W> {
    pub fn new(show: impl IntoMemo<bool>, el: El<W>) -> Self {
        Self { show: show.memo(), el }
    }
}

impl<W: WidgetCtx> Widget<W> for Show<W> {
    fn meta(&self) -> MetaTree {
        self.el.meta()
    }

    fn on_mount(&mut self, ctx: MountCtx<W>) {
        self.el.on_mount(ctx);
    }

    // Note: This method is used to modify the layout of the element, not to
    // model it, so we don't need to depend on `show` condition. User able to
    // change the layout of the element, but the layout of `Show` is zero in
    // `build_layout_tree` if `show` is false
    fn layout(&self) -> Signal<Layout> {
        self.el.layout()
    }

    fn build_layout_tree(&self) -> MemoTree<Layout> {
        let el_layout_tree = self.el.build_layout_tree();

        MemoTree {
            data: self.show.map(move |&show| {
                if show {
                    el_layout_tree.data.get_cloned()
                } else {
                    Layout::zero()
                }
            }),
            children: self.show.map(move |&show| {
                if show {
                    el_layout_tree.children.get_cloned()
                } else {
                    vec![]
                }
            }),
        }
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        if self.show.get() {
            self.el.draw(ctx)
        } else {
            Ok(())
        }
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse<W> {
        if self.show.get() {
            self.el.on_event(ctx)
        } else {
            ctx.ignore()
        }
    }
}
