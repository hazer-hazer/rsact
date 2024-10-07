use crate::{
    declare_widget_style,
    event::EventResponse,
    font::FontSize,
    layout::{Layout, LayoutKind},
    render::{color::Color, Renderer},
    style::{ColorStyle, Styler},
    widget::{Meta, MetaTree, MountCtx, Widget, WidgetCtx},
};
use embedded_graphics::{
    iterator::raw::RawDataSlice,
    pixelcolor::raw::{BigEndian, RawU1},
    prelude::{Point, RawData},
    Pixel,
};
use rsact_reactive::{
    mapped,
    memo::{IntoMemo, MemoTree},
    memo_chain::IntoMemoChain,
    prelude::{use_signal, MemoChain},
    signal::{IntoSignal, ReadSignal, Signal, SignalMapper, SignalSetter},
};

pub struct IconRaw<'a> {
    data: RawDataSlice<'a, RawU1, BigEndian>,
}

#[derive(Clone, Copy)]
pub enum IconKind {
    Check,
}

impl IconKind {
    pub fn data(&self) -> IconRaw<'static> {
        todo!()
    }
}

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

pub struct Icon<W: WidgetCtx> {
    kind: Signal<IconKind>,
    size: Signal<FontSize>,
    real_size: Signal<u32>,
    layout: Signal<Layout>,
    style: MemoChain<IconStyle<W::Color>>,
}

impl<W: WidgetCtx> Icon<W> {
    pub fn new(kind: impl IntoSignal<IconKind> + 'static) -> Self {
        let real_size = use_signal(10);
        let layout = Layout::shrink(LayoutKind::Edge).into_signal();

        Self {
            kind: kind.into_signal(),
            size: use_signal(FontSize::Unset),
            real_size,
            layout,
            style: IconStyle::base().into_memo_chain(),
        }
    }
}

impl<W: WidgetCtx> Widget<W> for Icon<W>
where
    W::Styler: Styler<IconStyle<W::Color>, Class = ()>,
{
    fn meta(&self) -> MetaTree {
        MetaTree::childless(Meta::none())
    }

    fn on_mount(&mut self, ctx: MountCtx<W>) {
        ctx.accept_styles(self.style, ());

        let viewport = ctx.viewport;
        let size = self.size;

        self.real_size.set_from(mapped!(move |viewport, size| {
            size.resolve(*viewport)
        }))
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> rsact_reactive::prelude::MemoTree<Layout> {
        MemoTree::childless(self.layout.into_memo())
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, W>,
    ) -> crate::widget::DrawResult {
        let style = self.style.get();
        let icon = self.kind.get().data();
        let data_width = ctx.layout.area.size.width.max(8);

        ctx.renderer.translucent_pixel_iter(
            icon.data.into_iter().enumerate().map(|(index, color)| {
                let color = match color.into_inner() {
                    0 => style.background.get(),
                    1 => style.color.get(),
                    _ => None,
                }?;

                let x = index as u32 % data_width;
                let y = index as u32 / data_width;

                if x >= data_width {
                    None
                } else {
                    Some(Pixel(
                        ctx.layout.area.top_left
                            + Point::new(x as i32, y as i32),
                        color,
                    ))
                }
            }),
        )
    }

    fn on_event(
        &mut self,
        ctx: &mut crate::widget::EventCtx<'_, W>,
    ) -> EventResponse<W> {
        let _ = ctx;

        W::ignore()
    }
}
