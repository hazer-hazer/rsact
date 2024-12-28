use super::{layout::ContentLayout, Size};
use crate::widget::prelude::*;
use core::marker::PhantomData;
use embedded_graphics::pixelcolor::raw::BigEndian;
use rsact_icons::{EmptyIconSet, IconRaw, IconSet};
use rsact_reactive::maybe::{
    IntoMaybeReactive, IsInert, IsReactive, ReactivityMarker,
};

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

#[derive(Clone)]
pub enum IconValue<I: IconSet> {
    // Static icon of fixed size
    Fixed(IconRaw<BigEndian>),
    // Dynamically sized icon with dynamic icon kind
    Relative(Signal<FontSize>, MaybeSignal<I>),
}

pub struct Icon<W: WidgetCtx, I: IconSet, R: ReactivityMarker> {
    value: IconValue<I>,
    layout: Signal<Layout>,
    style: MemoChain<IconStyle<W::Color>>,
    is_reactive: PhantomData<R>,
}

impl<W: WidgetCtx> Icon<W, EmptyIconSet, IsInert> {
    pub fn fixed(icon: IconRaw<BigEndian>) -> Self {
        let layout = Layout::shrink(LayoutKind::Content(ContentLayout::fixed(
            Size::new_equal(icon.size),
        )))
        .signal();

        Self {
            value: IconValue::Fixed(icon),
            layout,
            style: IconStyle::base().memo_chain(),
            is_reactive: PhantomData,
        }
    }
}

impl<W: WidgetCtx, I: IconSet + 'static> Icon<W, I, IsReactive> {
    pub fn new(icon: impl IntoMaybeSignal<I>) -> Self {
        let icon = icon.maybe_signal();
        let size = FontSize::Relative(1.0).signal();
        let value = IconValue::Relative(size, icon);
        let layout = Layout::shrink(LayoutKind::Content(ContentLayout::Icon(
            size.memo(),
        )))
        .signal();

        Self {
            value,
            layout,
            style: IconStyle::base().memo_chain(),
            is_reactive: PhantomData,
        }
    }

    /// Inert icon kind setter
    pub fn set(&mut self, new_icon: I) {
        match &mut self.value {
            IconValue::Fixed(_) => {
                // TODO: Warn or panic?
            },
            IconValue::Relative(_, icon) => icon.set(new_icon),
        }
    }

    pub fn size<S: Into<FontSize> + Clone + PartialEq + 'static>(
        mut self,
        size_setter: impl IntoMaybeReactive<S>,
    ) -> Self {
        match &mut self.value {
            IconValue::Fixed(_) => {
                // TODO: Warn or panic?
            },
            IconValue::Relative(size, _) => {
                size.setter(size_setter.maybe_reactive(), |size, new_size| {
                    *size = new_size.clone().into();
                });
            },
        }
        self
    }
}

impl<W: WidgetCtx, I: IconSet + 'static, R: ReactivityMarker> Widget<W>
    for Icon<W, I, R>
where
    W::Styler: WidgetStylist<IconStyle<W::Color>>,
{
    fn meta(&self) -> MetaTree {
        MetaTree::childless(Meta::none)
    }

    fn on_mount(&mut self, ctx: MountCtx<W>) {
        ctx.accept_styles(self.style, ());
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn draw(
        &self,
        ctx: &mut crate::widget::DrawCtx<'_, W>,
    ) -> crate::widget::DrawResult {
        let viewport = ctx.viewport;
        let style = self.style.get();

        let icon_raw = match &self.value {
            &IconValue::Fixed(icon_raw) => icon_raw,
            IconValue::Relative(size, kind) => {
                with!(move |size, kind, viewport| kind
                    .size(size.resolve(*viewport)))
            },
        };

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
    ) -> EventResponse {
        let _ = ctx;

        ctx.ignore()
    }
}
