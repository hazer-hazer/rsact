use super::{container::Container, label::Label};
use crate::{
    declare_widget_style,
    layout::{LayoutKind, model::LayoutModelNode},
    widget::prelude::*,
};
use alloc::{rc::Rc, string::ToString};
use core::fmt::Display;
use rsact_reactive::prelude::*;

#[derive(Clone, Copy)]
pub struct SelectState {
    // Press state is global now (see `PageState`/`PointerState`); only the
    // widget-specific `active`/`selected` state is stored locally.
    pub active: bool,
    pub selected: Option<usize>,
}

impl SelectState {
    pub fn initial(selected: Option<usize>) -> Self {
        Self { active: false, selected }
    }

    fn options_offset(
        &self,
        inner: Rect,
        children_layouts: &[LayoutModelNode<'_>],
    ) -> (Point, Option<usize>) {
        // Prefer the selected option, but fall back to the first if the stored
        // index is stale: reactive `options` can shrink below `selected`
        // between an `on_event` clamp and the next render, so a bare
        // `.get(selected).unwrap()` would panic on the render path.
        if let Some((selected, layout)) = self.selected.and_then(|selected| {
            children_layouts
                .get(selected)
                .map(|layout| (selected, layout))
        }) {
            (inner.center_offset_of(layout.inner), Some(selected))
        } else if let Some(first_option) = children_layouts.first() {
            (inner.center_offset_of(first_option.inner), None)
        } else {
            (Point::zero(), None)
        }
    }
}

declare_widget_style! {
    SelectStyle (SelectState) {
        container: container,
        selected: container {
            selected_background_color: background_color,
            selected_border_color: border_color,
            selected_border_radius: border_radius,
        },
        selected_text_color: color {
            transparent_selected_text_color: transparent,
        },
        text_color: color {
            transparent_text_color: transparent,
        },
    }
}

pub struct SelectOption<W: WidgetCtx, K: PartialEq> {
    key: K,
    el: El<W>,
}

impl<W: WidgetCtx, K: PartialEq> SelectOption<W, K> {
    pub fn new(key: K) -> Self
    where
        K: Display,
    {
        let string = key.to_string();
        SelectOption {
            key,
            el: Container::new(Label::new(string.inert()).into_el())
                .padding(5u32)
                .into_el(),
        }
    }
}

impl<W: WidgetCtx, K: PartialEq> PartialEq for SelectOption<W, K> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

// WS13.4 (Task 5.13): mechanical split like Label/Edge/Bar/Slider/Knob —
// `layout`/`state`/`style`/`options` are ALL read by `render`/`on_event`
// post-build, so there is no build-only field to drop (a `size_of` `<`
// assertion would be false, not true, same as those widgets' fallback
// shape test). Unlike Button/Container/Flex/Scrollable, `Select` never
// registers any arena children via `ctx.set_single_child`/
// `ctx.set_children` in the first place — each `SelectOption::el` lives
// only inside the `options` `Rc` and is iterated manually in
// `render_part` (rendering it is itself `TODO(unimplemented)` below), so
// there is no `#[child]`/`#[children]` field on `SelectBuilder` either;
// `Build::build` here is a pure by-name field move with zero child
// wiring, same shape as Label/Edge/Bar.
//
// `Dir: Direction` is de-genericized into a runtime `axis: Axis` field,
// same 7.2 rule as Bar/Slider/Scrollable: `render` reads `Dir::AXIS`
// (`.inverted()`/cross-size) for the selected-option highlight box, not
// just `new()`'s one-shot layout construction, so it can't be dropped
// like `Space`'s ctor-only tag.
//
// `K: PartialEq` stays generic — it's `SelectOption`'s key type, with no
// canonical concrete choice, same call as Bar's undecided `V`.
//
// WS4.1/A18 (`docs/plans/2026-07-05-rsact-evolution-roadmap.md`'s A18
// backlog row, maintainer: "looks horrible"): the
// `options: Rc<MaybeReactive<Vec<SelectOption<W, K>>>>` storage and the
// `selected.setter(...)` reactive-effect wiring below (in `new()`) are
// UNTOUCHED by this split — both are constructed exactly as before,
// still inside `new()` on the "effect path" (not deferred into
// `Build::build`'s generated child-wiring), just now assembled into a
// `SelectBuilder` literal instead of a bare `Select`; the derive's
// by-name `#[widget]` move only relocates the already-built `Rc`
// handle/effect closures, it does not rebuild or rewire them. This is a
// pure identity-preserving mechanical split (assessment (a) of the
// 5.13 checklist row) — it does not touch, and does not attempt to fix,
// the A18 entanglement (options storage / selected-wiring awkwardness),
// which stays exactly as backlogged as before.
#[derive(Builder)]
#[builds(Select<W, K>)]
#[flags(focusable)]
pub struct SelectBuilder<W: WidgetCtx, K: PartialEq + 'static> {
    #[widget]
    layout: Layout,
    #[widget]
    state: Signal<SelectState>,
    #[widget]
    style: WidgetStyleFn<SelectStyle<W::Color>>,
    // TODO: Can we do fixed size?
    // WS4.1: `Rc` because `MaybeReactive<Vec<SelectOption>>` lost blanket `Copy`
    // (SelectOption holds an `El` — neither Copy nor Clone) yet is read by both
    // the `selected` setter effect and this widget. See `Select::new`.
    #[widget]
    options: Rc<MaybeReactive<Vec<SelectOption<W, K>>>>,
    #[widget]
    axis: Axis,
}

pub struct Select<W: WidgetCtx, K: PartialEq + 'static> {
    layout: Layout,
    state: Signal<SelectState>,
    style: WidgetStyleFn<SelectStyle<W::Color>>,
    options: Rc<MaybeReactive<Vec<SelectOption<W, K>>>>,
    axis: Axis,
}

impl<W: WidgetCtx, K> Select<W, K>
where
    K: PartialEq + Clone + Display + 'static,
{
    pub fn vertical(
        selected: impl IntoMaybeSignal<K>,
        options: impl SignalMapRefMaybeReactive<[K], Vec<SelectOption<W, K>>>
        + PartialEq,
    ) -> SelectBuilder<W, K> {
        Self::new(Axis::Y, selected, options)
    }

    pub fn horizontal(
        selected: impl IntoMaybeSignal<K>,
        options: impl SignalMapRefMaybeReactive<[K], Vec<SelectOption<W, K>>>,
    ) -> SelectBuilder<W, K> {
        Self::new(Axis::X, selected, options)
    }

    // TODO: Inert options?
    // TODO: MaybeReactive options
    pub fn new(
        axis: Axis,
        selected: impl IntoMaybeSignal<K>,
        options: impl SignalMapRefMaybeReactive<[K], Vec<SelectOption<W, K>>>,
    ) -> SelectBuilder<W, K> {
        let options = options.map_ref_maybe_reactive(|options| {
            options
                .into_iter()
                .cloned()
                .map(|opt| SelectOption::new(opt))
                .collect::<Vec<_>>()
        });
        // WS4.1: `MaybeReactive<Vec<SelectOption>>` is no longer `Copy` (its
        // `SelectOption`s hold `El`s — neither `Copy` nor `Clone`), yet the
        // options are read from two long-lived owners: the `selected` setter
        // effect and the widget itself (render/on_event). Share them via `Rc`.
        let options = Rc::new(options);

        let mut selected = selected.maybe_signal();

        // TODO: This maybe-reactive optimization not working, as when selected
        // is inert, it is then converted into a signal inside
        // `selected.setter`, but this signal is unavailable outside, user still
        // holds they Inert value and selected signal doesn't need to be
        // tracked. So we either do runtime check like `if selected.is_inert() {
        // ... }` or we just require selected to always be a signal. Or we can
        // do two constructors: one for inert selected and one for reactive
        // selected. TODO: ... For this to work as expected we need
        // `SelectState` to be Signal still its `selected` to be mapped as
        // MaybeReactive. Select widget stylist expects full `SelectState` to be
        // a signal. TODO: ... What idea I like is just to remove
        // `SignalSetter` implementation from `MaybeSignal` to avoid such
        // problems and just dynamically check if `selected` is reactive or
        // inert and create setter effect depending on that.
        // TODO: ... Wait wait wait. We receive selected and then make a setter
        // for it, why not just put it inside the `SelectState`? It's not a
        // problem to pass this when accepting styles as it's just a copy-type
        // boolean.
        let state = SelectState::initial(with!(|selected, options| {
            options.iter().position(|opt| &opt.key == selected)
        }))
        .signal();

        // WS5.1: option children come from the arena; the flex layout no
        // longer collects their layout handles.

        // WS4.5: only wire the `state.selected -> selected` feedback when
        // `selected` is a genuine reactive Signal. For an inert `selected` the
        // caller holds a plain value with no signal to push into, so `setter`
        // would promote it to an orphan Signal + build a Memo (state.map) + an
        // Effect that nothing observes (the pre-existing TODO above). Skipping
        // it saves 3 nodes per static Select with no observable behavior change.
        if selected.as_signal().is_some() {
            let setter_options = Rc::clone(&options);
            selected.setter(
                state.map(|state| state.selected).maybe_reactive(),
                move |selected, position| {
                    if let Some(option) = position.and_then(|pos| {
                        setter_options.with(|options| {
                            options.get(pos).map(|opt| opt.key.clone())
                        })
                    }) {
                        *selected = option;
                    }
                },
            );
        }

        SelectBuilder {
            layout: Layout::new(
                LayoutKind::Flex(
                    FlexLayout::base(axis)
                        .block_model(BlockModel::zero().padding(1u32))
                        .gap(axis.canon(5, 0))
                        .align_main(Align::Center)
                        .align_cross(Align::Center),
                ),
                axis.canon(
                    Length::InfiniteWindow(Length::Shrink.try_into().unwrap()),
                    Length::Shrink,
                ),
            ),
            state,
            style: None,
            options,
            axis,
        }
    }

    // fn option_position(&self, key: &K) -> Option<usize> {
    //     self.options
    //         .with(|options| options.iter().position(|opt| &opt.key == key))
    // }

    // TODO: Use lenses
    // pub fn use_value(
    //     self,
    //     value: impl WriteSignal<K> + ReadSignal<K> + 'static,
    // ) -> Self {
    //     value.with(|initial| {
    //         self.selected.set(self.option_position(initial));
    //     });

    //     let options = self.options;
    //     value.setter(self.selected, move |pos, value| {
    //         if let &Some(pos) = pos {
    //             if let Some(opt) = options
    //                 .with(|options| options.get(pos).map(|opt|
    // opt.key.clone()))             {
    //                 *value = opt
    //             }
    //         }
    //     });

    //     self
    // }
}

impl<W: WidgetCtx, K> LayoutWidget<W> for SelectBuilder<W, K>
where
    K: PartialEq + Display + 'static,
{
    fn layout_mut(&mut self) -> &mut Layout {
        &mut self.layout
    }
}

impl<W: WidgetCtx, K> BlockModelWidget<W> for SelectBuilder<W, K> where
    K: PartialEq + Display + 'static
{
}

impl<W: WidgetCtx, K> SizedWidget<W> for SelectBuilder<W, K> where
    K: PartialEq + Clone + Display + 'static
{
}

impl<W: WidgetCtx, K> FontSettingWidget<W> for SelectBuilder<W, K> where
    K: PartialEq + Clone + Display + 'static
{
}

impl<W: WidgetCtx, K: PartialEq + 'static> Widget<W> for Select<W, K> {
    // NOTE: no `debug_name`/`flags` override on the retained widget — both are
    // read exactly once, pre-build, from `Build` (seeding `ElState` at
    // `state.rs:72`); post-build all consumption is via `ElState`, so an
    // override here would be dead duplication of `SelectBuilder`'s derived
    // `Build::flags`/`Build::debug_name` ("Select" from
    // `#[builds(Select<W, K>)]`, `focusable` from `#[flags(focusable)]`).
    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        let children_layouts = ctx.layout.children().collect::<Vec<_>>();

        ctx.render_self(|mut ctx| {
            let style = ctx.get_style(self.style.as_deref());
            let state = self.state.get();

            if let (options_offset, Some(selected)) =
                state.options_offset(ctx.layout.inner, &children_layouts)
            {
                let selected_child_layout =
                    children_layouts.get(selected).unwrap();

                Block::from_layout_style(
                    selected_child_layout
                        .outer
                        .translate(options_offset)
                        .resized_axis(
                            self.axis.inverted(),
                            ctx.layout.inner.size.cross(self.axis),
                            Anchor::Center,
                        ),
                    BlockModel::zero().border_width(1),
                    style.selected,
                )
                .render(ctx.renderer)?;
            }

            // TODO: Review if focus outline visible
            ctx.render_focus_outline(ctx.id)
        })?;

        ctx.render_part("options", |mut ctx| {
            let state = self.state.get();
            let _style = ctx.get_style(self.style.as_deref());
            let (_options_offset, _) =
                state.options_offset(ctx.layout.inner, &children_layouts);

            self.options.with(move |options| {
                ctx.clip_inner(|_ctx| {
                    options
                        .iter()
                        .zip(children_layouts.iter())
                        .enumerate()
                        .try_for_each(|(_index, (_option, _option_layout))| {
                            // TODO: Need to thing how to properly handle select
                            // widget. Should options be real widgets or hidden
                            // inside Select just to render? Maybe we even don't
                            // need to have real Text widgets, instead storing
                            // only text and rendering it through renderer, but
                            // then we'll probably lose some text properties
                            // handling.
                            //
                            // TODO(unimplemented): render option text. Degrade
                            // to a no-op instead of `todo!()` so a Select with
                            // options does not abort the device on render.
                            Ok(())
                            // ctx.with_tree_style(
                            //     |tree_style| {
                            //         tree_style.text_color(
                            //             (if Some(index) == state.selected {
                            //                 style.selected_text_color
                            //             } else {
                            //                 style.text_color
                            //             })
                            //             .get(),
                            //         )
                            //     },
                            //     |mut ctx| {
                            //         let option = &option.el;
                            //         ctx.for_child(
                            //             option.id(),
                            //             &option_layout
                            //                 .translate(options_offset),
                            //             |ctx| option.render(ctx),
                            //         )
                            //     },
                            // )
                        })
                })
            })
        })
    }

    fn on_event(&mut self, mut ctx: EventCtx<'_, W>) -> EventResponse {
        let state = self.state.get();

        if state.active && ctx.is_focused() {
            // TODO: Right select interpretation
            if let Some(mut offset) = ctx.event.interpret_as_rotation() {
                let current = state.selected;

                let new = current
                    .or_else(|| {
                        if offset > 0 {
                            offset -= 1;
                            Some(0)
                        } else {
                            None
                        }
                    })
                    .map(|current| {
                        ((current as i32) + offset).clamp(
                            0,
                            self.options
                                .with(|options| options.len().saturating_sub(1))
                                as i32,
                        ) as usize
                    });

                if current != new {
                    self.state.update(|state| {
                        state.selected = new;
                    });
                }

                return ctx.capture();
            }
        }

        ctx.handle()?; // focus press claim (encoder), automatic
        ctx.handle_click(|ctx| {
            self.state.update(|state| state.active = !state.active);
            ctx.capture()
        })
    }
}
