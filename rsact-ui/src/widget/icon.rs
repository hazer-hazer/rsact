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
    layout: LayoutBuilder<W>,
    #[widget]
    style: WidgetStyleFn<IconStyle<W::Color>>,
    #[widget]
    visible: MaybeReactive<bool>,
    is_reactive: PhantomData<R>,
}

// WS5.5 missed this one: the retained widget kept a `layout: LayoutData` copy
// after every other widget dropped theirs. It compiled nowhere to notice —
// `icon` is `#[cfg(feature = "tiny-icons")]` and `ci-powerset.sh` excludes that
// feature as WIP, so no CI job builds this module. `render` reads `ctx.layout`,
// never `self.layout`, so the copy was pure duplication of arena-owned state.
pub struct Icon<W: WidgetCtx, I: IconSet> {
    value: IconValue<I>,
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

        // ISSUE-2: the layout holds a plain `FontSize`, not the `Memo<FontSize>`
        // the layout pass used to read while measuring. Just the default,
        // written inline — the binding that keeps it up to date lives in
        // [`IconBuilder::size`], because that is where a size actually arrives.
        //
        // Unlike `Label`, whose text is supplied at construction and so binds in
        // `new`, an icon's size is supplied by a *setter*. Binding here would
        // mean every icon — including the overwhelmingly common one that never
        // calls `.size()` — paying for an effect to observe a value nobody
        // writes.
        let layout = LayoutBuilder::shrink(LayoutKind::Content(
            ContentLayout::icon(FontSize::Relative(1.0)),
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

    /// Set the icon's size.
    ///
    /// ISSUE-2: this writes **two** channels, and they are not redundant.
    /// Render resolves the size to pick which pre-rendered raster to draw
    /// (`kind.size(size.resolve(viewport))`), while layout needs it to measure —
    /// the same paint/geometry split as a `Label`'s text. The layout half goes
    /// through `LayoutBuilder::setter` like every other layout property, so a
    /// size change marks this element dirty instead of waking the page's layout
    /// probe with an empty dirty set (which relayouts and reflushes everything).
    ///
    /// An **inert** size costs nothing: both `setter` calls take their `Inert`
    /// arm and write at build. Only a reactive size creates a binding effect —
    /// and an icon that never calls this creates neither.
    pub fn size<S: Into<FontSize> + Clone + PartialEq + 'static>(
        mut self,
        size_setter: impl IntoMaybeReactive<S>,
    ) -> Self {
        let size_setter = size_setter.maybe_reactive();

        match &mut self.value {
            IconValue::Fixed(_) => {
                // TODO: Warn or panic?
                // Better only accept memos?
                return self;
            },
            IconValue::Relative(size, _) => {
                size.setter(size_setter.clone(), |size, new_size| {
                    *size = new_size.clone().into();
                });
            },
        }

        self.layout.setter(size_setter, |data, new_size| {
            data.set_icon_size(new_size.clone().into())
        });

        self
    }
}

// NOTE: these run in NO CI job — `icon` is `#[cfg(feature = "tiny-icons")]` and
// `ci-powerset.sh` excludes that feature as WIP (ISSUE-5). They were verified by
// hand with `--features "std,embedded-graphics,tiny-icons"` and will start
// running the moment that gap is closed. Written anyway: this module had zero
// coverage, which is how it accumulated three separate breakages in a day.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        el::{arena::ElArena, build::BuildCtx, view::View},
        font::FontSize,
        test_support::NullWtf,
    };
    use rsact_reactive::runtime::current_runtime_profile;

    /// Build an icon and report `(effects created, layout's icon size)`.
    fn build(
        make: impl FnOnce(
            IconBuilder<NullWtf, EmptyIconSet, IsReactive>,
        ) -> IconBuilder<NullWtf, EmptyIconSet, IsReactive>,
    ) -> (usize, Option<FontSize>) {
        let before = current_runtime_profile().effects;
        let mut root =
            make(Icon::<NullWtf, _>::new(EmptyIconSet.inert())).into_el();
        let arena = create_signal(ElArena::new());
        let id = BuildCtx::run(&mut root, arena);
        let created = current_runtime_profile().effects - before;
        (created, arena.with_untracked(|a| a.layout(id).unwrap().icon_size()))
    }

    /// ISSUE-2: the size binding lives on `size()`, not on `new()`. An icon that
    /// never sets a size must create nothing — binding in the constructor made
    /// every icon pay an effect to watch a value nobody writes.
    ///
    /// The reactive case is measured alongside for the usual reason: "creates 0
    /// effects" passes just as well when the instrument reads 0 for everything.
    ///
    /// **The reactive case costs TWO, and only one of them is this design's.**
    /// The layout binding is one. The other is the pre-existing relay in
    /// `IconValue::Relative`: because it holds a `Signal<FontSize>` rather than
    /// a `MaybeReactive<FontSize>`, `size()` must spend an effect copying the
    /// source into it for render to read — an effect whose entire job is
    /// mirroring one reactive value into another. That is exactly the cost
    /// `Icon::new`'s `SignalOnWrite` TODO predicts; collapsing the `Signal` to a
    /// `MaybeReactive` would make this 1, and inert `.size()` stays 0 either
    /// way. Locked at 2 so that collapse shows up here as a diff.
    #[test]
    fn only_a_reactive_size_costs_an_effect() {
        with_new_runtime(|_| {
            assert_eq!(
                build(|b| b),
                (0, Some(FontSize::Relative(1.0))),
                "an icon with no .size() must create no binding"
            );
        });
        with_new_runtime(|_| {
            assert_eq!(
                build(|b| b.size(FontSize::Fixed(20))),
                (0, Some(FontSize::Fixed(20))),
                "an INERT size writes the layout at build, no effect"
            );
        });
        with_new_runtime(|_| {
            let size = create_signal(FontSize::Fixed(20));
            assert_eq!(
                build(|b| b.size(size)),
                (2, Some(FontSize::Fixed(20))),
                "a reactive size costs bindings — if this read 0 the cases \
                 above would prove nothing"
            );
        });
    }

    /// …and that binding must reach the ARENA's dirty set, which is the whole
    /// point of ISSUE-2: waking the page's layout probe instead relayouts and
    /// reflushes the entire viewport.
    #[test]
    fn a_reactive_size_change_marks_the_arena_dirty() {
        with_new_runtime(|_| {
            let mut size = create_signal(FontSize::Fixed(20));
            let mut root = Icon::<NullWtf, _>::new(EmptyIconSet.inert())
                .size(size)
                .into_el();
            let arena = create_signal(ElArena::new());
            let id = BuildCtx::run(&mut root, arena);

            arena.clone().update_untracked(|a| {
                a.take_dirty();
            });

            size.set(FontSize::Fixed(32));

            assert_eq!(
                arena.with_untracked(|a| a.layout(id).unwrap().icon_size()),
                Some(FontSize::Fixed(32)),
            );
            assert!(
                arena.with_untracked(|a| a.is_layout_dirty()),
                "an icon-size change must mark the arena layout-dirty"
            );
        });
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
                    // `viewport` is a plain `Size` now, so it drops out of the
                    // `with!` — only `size` and `kind` are still reactive here.
                    with!(move |size, kind| kind.size(size.resolve(viewport)))
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
