use crate::{layout::LayoutKind, widget::prelude::*};
use core::marker::PhantomData;

// WS13.4 (Task 5.3): `Dir: Direction` was a compile-time-only tag — it only
// ever selected an `Axis` for `new()`'s size computation and was never read
// at runtime. Per the 7.2 slice (flex precedent: `Dir` type param -> runtime
// `Axis` ctor arg), it is dropped in favor of a plain `Axis` argument
// threaded through `row`/`col`, collapsing `Space<W, RowDir>`/
// `Space<W, ColDir>` into a single `Space<W>` monomorphization.
#[derive(Builder)]
#[builds(Space<W>)]
pub struct SpaceBuilder<W: WidgetCtx> {
    #[widget]
    layout: LayoutBuilder<W>,
    // Moved 1:1 by the derive into the retained `Space { layout, ctx }`:
    // `layout: LayoutData` alone doesn't use `W`, so `ctx: PhantomData<W>`
    // carries it (flex.rs/show.rs precedent) — `Space` already carried this
    // exact field pre-split (the row's "already the PhantomData precedent").
    #[widget]
    ctx: PhantomData<W>,
}

pub struct Space<W: WidgetCtx> {
    layout: LayoutData,
    // `W` is otherwise unused on the retained widget — kept only to satisfy
    // `Widget<W>`'s own `W` parameter, same as `flex.rs`/`show.rs`.
    ctx: PhantomData<W>,
}

impl<W: WidgetCtx> Space<W> {
    // pub fn row<L: Into<Length> + Clone + PartialEq + 'static>(
    //     length: impl AsMemo<L>,
    // ) -> Self {
    //     Self::new(length)
    // }

    pub fn row(length: impl Into<Length>) -> SpaceBuilder<W> {
        SpaceBuilder::new(Axis::X, length)
    }

    // pub fn col<L: Into<Length> + Clone + PartialEq + 'static>(
    //     length: impl AsMemo<L>,
    // ) -> Self {
    //     Self::new(length)
    // }

    pub fn col(length: impl Into<Length>) -> SpaceBuilder<W> {
        SpaceBuilder::new(Axis::Y, length)
    }
}

impl<W: WidgetCtx> SpaceBuilder<W> {
    // pub fn new<L: Into<Length> + Clone + PartialEq + 'static>(
    //     length: impl AsMemo<L>,
    // ) -> Self {
    //     let length = length.as_memo();
    //     let layout = Layout::shrink(LayoutKind::Edge).into_signal();

    //     layout.setter(length, move |length, layout| {
    //         layout.size =
    //             Dir::AXIS.canon(length.clone().into(), Length::fill());
    //     });

    //     Self { layout, ctx: PhantomData, dir: PhantomData }
    // }

    // TODO: Reactive length, MaybeReactive
    fn new(axis: Axis, length: impl Into<Length>) -> Self {
        let layout = LayoutBuilder::shrink(LayoutKind::Edge)
            .size(axis.canon(length.into(), Length::fill()));

        Self { layout, ctx: PhantomData }
    }
}

impl<W: WidgetCtx + 'static> Widget<W> for Space<W> {
    // NOTE: no `flags`/`debug_name` override on the retained widget — both
    // are read exactly once, pre-build, from `Build` (seeding `ElState` at
    // `state.rs:72`); post-build all consumption is via `ElState`, so an
    // override here would be dead duplication of `SpaceBuilder`'s derived
    // `Build::debug_name` ("Space" from `#[builds(Space<W>)]`). `Space` never
    // overrode `flags` either, so no `#[flags(...)]` attr is needed on
    // `SpaceBuilder`.
    fn render(&self, _ctx: RenderCtx<'_, W>) -> RenderResult {
        Ok(())
    }

    fn on_event(&mut self, ctx: EventCtx<'_, W>) -> EventResponse {
        ctx.ignore()
    }
}
