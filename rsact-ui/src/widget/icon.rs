use super::layout::ContentLayout;
use crate::{font::FontSize, widget::prelude::*};
use core::marker::PhantomData;
use rsact_reactive::prelude::*;
use rsact_tiny_icons::{EmptyIconSet, IconRaw, IconSet};

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
    Fixed(IconRaw),
    // Dynamically sized icon with dynamic icon kind
    Relative(Signal<FontSize>, MaybeReactive<I>),
}

#[derive(View)]
pub struct Icon<W: WidgetCtx, I: IconSet, R: ReactivityMarker> {
    value: IconValue<I>,
    layout: Layout,
    style: Option<Box<dyn Fn(IconStyle<W::Color>) -> IconStyle<W::Color>>>,
    is_reactive: PhantomData<R>,
    visible: MaybeReactive<bool>,
}

impl<W: WidgetCtx, I: IconSet, R: ReactivityMarker> Icon<W, I, R> {
    pub fn visible(mut self, visible: impl IntoMaybeReactive<bool>) -> Self {
        self.visible = visible.maybe_reactive();
        self
    }
}

impl<W: WidgetCtx> Icon<W, EmptyIconSet, IsInert> {
    pub fn inert(icon: IconRaw) -> Self {
        let layout = Layout::shrink(LayoutKind::Content(ContentLayout::fixed(
            Size::new_equal(icon.size),
        )));

        Self {
            value: IconValue::Fixed(icon),
            layout,
            style: None,
            is_reactive: PhantomData,
            visible: true.inert().maybe_reactive(),
        }
    }
}

impl<W: WidgetCtx, I: IconSet + 'static> Icon<W, I, IsReactive> {
    pub fn new(icon: impl IntoMaybeReactive<I>) -> Self {
        let icon = icon.maybe_reactive();
        // TODO: Here we need something like `SignalOnWrite` reactive type that
        // is unlike MaybeSignal turns into Signal on write instead of writing
        // to the owned value. Now size is always a signal, while in most cases
        // will be untouched, but making FontSize a MaybeSignal now will make it
        // always inert as MaybeSignal does not turn into reactive when updated,
        // it just updates owned value. (also as FontSize is a Copy-type this
        // may seem misleading). We need reactivity for layouts to react on size
        // change.
        let size = FontSize::Relative(1.0).signal();
        let value = IconValue::Relative(size, icon);

        let layout = Layout::shrink(LayoutKind::Content(ContentLayout::Icon(
            size.memo(),
        )));

        Self {
            value: IconValue::Relative(icon),
            layout,
            style: None,
            is_reactive: PhantomData,
            visible: true.inert().maybe_reactive(),
        }
    }

    // /// Inert icon kind setter
    // pub fn set(&mut self, new_icon: I) {
    //     match &mut self.value {
    //         IconValue::Fixed(_) => {
    //             // TODO: Warn or panic?
    //         },
    //         IconValue::Relative(_, icon) => icon.set(new_icon),
    //     }
    // }

    pub fn size<S: Into<FontSize> + Clone + PartialEq + 'static>(
        mut self,
        size_setter: impl IntoMaybeReactive<S>,
    ) -> Self {
        match &mut self.value {
            IconValue::Fixed(_) => {
                // TODO: Warn or panic?
                // Better only accept memos?
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
{
    fn debug_name(&self) -> &'static str {
        "Icon"
    }

    fn build(&mut self, ctx: BuildCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, ctx: &mut RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self(|ctx| {
            if !self.visible.get() {
                return Ok(());
            }

            let viewport = ctx.shared.viewport;
            let _style = ctx.get_style(|t| t.icon, self.style.as_deref());

            let _icon_raw = match &self.value {
                &IconValue::Fixed(icon_raw) => icon_raw,
                IconValue::Relative(size, kind) => {
                    with!(move |size, kind, viewport| kind
                        .size(size.resolve(*viewport)))
                },
            };

            #[cfg(feature = "embedded-graphics")]
            {
                let _eg_top_left: embedded_graphics::geometry::Point =
                    ctx.layout.inner.top_left.into();
                todo!()
                // let icon = rsact_icons::Icon::new(
                //     icon_raw,
                //     eg_top_left,
                //     style.background.get(),
                //     style.color.get(),
                // );
                // ctx.renderer.draw_iter(icon.iter()).ok().unwrap();
            }

            Ok(())
        })
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        let _ = ctx;

        ctx.ignore()
    }
}
