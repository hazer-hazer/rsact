use super::layout::ContentLayout;
use crate::{font::FontSize, widget::prelude::*};
use core::marker::PhantomData;
use rsact_reactive::prelude::*;
use rsact_tiny_icons::{EmptyIconSet, IconRaw, IconSet};

declare_widget_style! {
    IconStyle () {
        background: color {
            transparent_background: transparent
        },
        color: color {
            transparent_color: transparent
        },
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

// WS13.4 (Task 5.15): `R: ReactivityMarker` is a compile-time-only tag that
// selects which constructor (`inert`/`new`) built the value, gating which
// builder methods are available (`size` only on the `IsReactive` path) — it
// is never matched on at runtime (`render` dispatches on the `IconValue`
// enum itself, not on `R`). Per the 7.2 slice (flex.rs `Dir`->`Axis`,
// space.rs `Dir` drop precedent), it is therefore build-only: `IconBuilder`
// keeps it to gate its own API surface, the retained `Icon` drops it
// entirely (a `PhantomData<R>` is a ZST, so — like `label.rs`/`space.rs` —
// this is not a `size_of` win; the win is not carrying a meaningless type
// parameter on the retained widget).
#[derive(Builder)]
#[builds(Icon<W, I>)]
pub struct IconBuilder<W: WidgetCtx, I: IconSet, R: ReactivityMarker> {
    #[widget]
    value: IconValue<I>,
    #[widget]
    layout: LayoutBuilder<W>,
    #[widget]
    style: WidgetStyleFn<IconStyle<W::Color>>,
    #[widget]
    visible: MaybeReactive<bool>,
    is_reactive: PhantomData<R>,
}

pub struct Icon<W: WidgetCtx, I: IconSet> {
    value: IconValue<I>,
    layout: LayoutData,
    style: WidgetStyleFn<IconStyle<W::Color>>,
    visible: MaybeReactive<bool>,
}

impl<W: WidgetCtx, I: IconSet, R: ReactivityMarker> IconBuilder<W, I, R> {
    pub fn visible(mut self, visible: impl IntoMaybeReactive<bool>) -> Self {
        self.visible = visible.maybe_reactive();
        self
    }
}

impl<W: WidgetCtx + 'static> Icon<W, EmptyIconSet> {
    pub fn inert(icon: IconRaw) -> IconBuilder<W, EmptyIconSet, IsInert> {
        let layout = LayoutBuilder::shrink(LayoutKind::Content(
            ContentLayout::fixed(Size::new_equal(icon.size)),
        ));

        IconBuilder {
            value: IconValue::Fixed(icon),
            layout,
            style: None,
            is_reactive: PhantomData,
            visible: true.inert().maybe_reactive(),
        }
    }
}

impl<W: WidgetCtx + 'static, I: IconSet + 'static> Icon<W, I> {
    pub fn new(
        icon: impl IntoMaybeReactive<I>,
    ) -> IconBuilder<W, I, IsReactive> {
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

        let layout = LayoutBuilder::shrink(LayoutKind::Content(
            ContentLayout::Icon(size.memo()),
        ));

        IconBuilder {
            value,
            layout,
            style: None,
            is_reactive: PhantomData,
            visible: true.inert().maybe_reactive(),
        }
    }
}

impl<W: WidgetCtx + 'static, I: IconSet + 'static>
    IconBuilder<W, I, IsReactive>
{
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

impl<W: WidgetCtx + 'static, I: IconSet + 'static> Widget<W> for Icon<W, I> {
    // NOTE: no `flags`/`debug_name` override on the retained widget — both are
    // read exactly once, pre-build, from `Build` (seeding `ElState`); a
    // retained override would be dead duplication (M7). `Build::debug_name`
    // on `IconBuilder` returns "Icon".
    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        ctx.render_self(|ctx| {
            if !self.visible.get() {
                return Ok(());
            }

            let viewport = ctx.shared.viewport;
            let _style = ctx.get_style(self.style.as_deref());

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
                // TODO(unimplemented): draw the icon via rsact-icons. Until the
                // draw path is wired up, degrade to not drawing rather than
                // `todo!()` — which would abort the device on every frame an
                // icon is on screen.
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
