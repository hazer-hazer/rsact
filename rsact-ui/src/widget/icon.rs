use super::{layout::ContentLayout, Limits, Size};
use crate::widget::prelude::*;
use rsact_icons::IconSet;

declare_widget_style! {
    IconStyle () {
        background: color,
        color: color,
    }
}

impl<C: Color> IconStyle<C> {
    pub fn base() -> Self {
        Self {
            background: ColorStyle::Unset,
            color: ColorStyle::DefaultForeground,
        }
    }
}

pub struct Icon<W: WidgetCtx, I: IconSet> {
    pub icon: Signal<I>,
    size: Signal<FontSize>,
    real_size: Signal<u32>,
    layout: Signal<Layout>,
    style: MemoChain<IconStyle<W::Color>>,
}

impl<W: WidgetCtx, I: IconSet + 'static> Icon<W, I> {
    pub fn new(icon: impl IntoSignal<I> + 'static) -> Self {
        let real_size = create_signal(10);
        let layout = Layout::shrink(LayoutKind::Content(ContentLayout::new(
            real_size.mapped(|size| Limits::exact(Size::new_equal(*size))),
        )))
        .into_signal();

        Self {
            icon: icon.into_signal(),
            size: create_signal(FontSize::Unset),
            real_size,
            layout,
            style: IconStyle::base().into_memo_chain(),
        }
    }

    pub fn size(self, size: impl Into<FontSize>) -> Self {
        self.size.set(size.into());
        self
    }

    // pub fn size<S: Into<FontSize> + PartialEq + Copy + 'static>(
    //     self,
    //     size: impl AsMemo<S>,
    // ) -> Self {
    //     self.size.set_from(size.as_memo().mapped(|&size| size.into()));
    //     self
    // }
}

impl<W: WidgetCtx, I: IconSet + 'static> Widget<W> for Icon<W, I>
where
    W::Styler: Styler<IconStyle<W::Color>, Class = ()>,
{
    fn meta(&self) -> MetaTree {
        MetaTree::childless(Meta::none)
    }

    fn on_mount(&mut self, ctx: MountCtx<W>) {
        ctx.accept_styles(self.style, ());

        let viewport = ctx.viewport;
        let size = self.size;

        // self.real_size.set_from(mapped!(move |viewport, size| {
        //     size.resolve(*viewport)
        // }))

        // TODO: Computed setter
        // This is not reactive!
        self.real_size.set(size.get().resolve(viewport.get()));
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> rsact_reactive::prelude::MemoTree<Layout> {
        MemoTree::childless(self.layout.as_memo())
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, W>,
    ) -> crate::widget::DrawResult {
        let style = self.style.get();

        let icon = self.icon;
        let real_size = self.real_size;
        let icon_raw = with!(|icon, real_size| icon.size(*real_size));

        let icon = rsact_icons::Icon::new(
            icon_raw,
            ctx.layout.inner.top_left,
            style.background.get(),
            style.color.get(),
        );

        ctx.renderer.translucent_pixel_iter(icon.iter())
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse<W> {
        let _ = ctx;

        ctx.ignore()
    }
}
