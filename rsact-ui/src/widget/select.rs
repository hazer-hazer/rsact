use super::{container::Container, label::Label};
use crate::{
    declare_widget_style,
    layout::{LayoutKind, model::LayoutModelNode},
    widget::prelude::*,
};
use alloc::string::ToString;
use core::{cell::RefCell, fmt::Display, marker::PhantomData};
use itertools::Itertools as _;
use rsact_reactive::prelude::*;

#[derive(Clone, Copy)]
pub struct SelectState {
    pub pressed: bool,
    pub active: bool,
    pub selected: Option<usize>,
}

impl SelectState {
    pub fn initial(selected: Option<usize>) -> Self {
        Self { pressed: false, active: false, selected }
    }

    fn options_offset(
        &self,
        inner: Rect,
        children_layouts: &[LayoutModelNode<'_>],
    ) -> (Point, Option<usize>) {
        if let Some(selected) = self.selected {
            let selected_child_layout = children_layouts.get(selected).unwrap();

            let options_offset =
                inner.center_offset_of(selected_child_layout.inner);

            (options_offset, Some(selected))
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

impl<C: Color> SelectStyle<C> {
    pub fn base() -> Self {
        Self {
            container: BlockStyle::base(),
            selected: BlockStyle::base().border(
                BorderStyle::base().radius(5).color(C::default_foreground()),
            ),
            selected_text_color: ColorStyle::DefaultForeground,
            text_color: ColorStyle::DefaultForeground,
        }
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
            el: Container::new(Label::new(string.inert()).el())
                .padding(5u32)
                .el(),
        }
    }
}

impl<W: WidgetCtx, K: PartialEq> PartialEq for SelectOption<W, K> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

pub struct Select<W: WidgetCtx, K: PartialEq + 'static, Dir: Direction> {
    layout: Layout,
    state: Signal<SelectState>,
    style: Option<Box<dyn Fn(SelectStyle<W::Color>) -> SelectStyle<W::Color>>>,
    // TODO: Can we do fixed size?
    options: MaybeReactive<Vec<SelectOption<W, K>>>,
    dir: PhantomData<Dir>,
}

impl<W: WidgetCtx, K> Select<W, K, ColDir>
where
    K: PartialEq + Clone + Display + 'static,
{
    pub fn vertical(
        selected: impl IntoMaybeSignal<K>,
        options: impl SignalMapRefMaybeReactive<[K], Vec<SelectOption<W, K>>>
        + PartialEq,
    ) -> Self {
        Self::new(selected, options)
    }
}

impl<W: WidgetCtx, K> Select<W, K, RowDir>
where
    K: PartialEq + Clone + Display + 'static,
{
    pub fn horizontal(
        selected: impl IntoMaybeSignal<K>,
        options: impl SignalMapRefMaybeReactive<[K], Vec<SelectOption<W, K>>>,
    ) -> Self {
        Self::new(selected, options)
    }
}

impl<W: WidgetCtx, K, Dir> Select<W, K, Dir>
where
    K: PartialEq + Clone + Display + 'static,
    Dir: Direction,
{
    // TODO: Inert options?
    // TODO: MaybeReactive options
    pub fn new(
        selected: impl IntoMaybeSignal<K>,
        options: impl SignalMapRefMaybeReactive<[K], Vec<SelectOption<W, K>>>,
    ) -> Self {
        let options = options.map_ref_maybe_reactive(|options| {
            options
                .into_iter()
                .cloned()
                .map(|opt| SelectOption::new(opt))
                .collect::<Vec<_>>()
        });

        let mut selected = selected.maybe_signal();

        // TODO: This maybe-reactive optimization not working, as when selected is inert, it is then converted into a signal inside `selected.setter`, but this signal is unavailable outside, user still holds they Inert value and selected signal doesn't need to be tracked. So we either do runtime check like `if selected.is_inert() { ... }` or we just require selected to always be a signal. Or we can do two constructors: one for inert selected and one for reactive selected.
        // TODO: ... For this to work as expected we need `SelectState` to be Signal still its `selected` to be mapped as MaybeReactive. Select widget stylist expects full `SelectState` to be a signal.
        // TODO: ... What idea I like is just to remove `SignalSetter` implementation from `MaybeSignal` to avoid such problems and just dynamically check if `selected` is reactive or inert and create setter effect depending on that.
        // TODO: ... Wait wait wait. We receive selected and then make a setter for it, why not just put it inside the `SelectState`? It's not a problem to pass this when accepting styles as it's just a copy-type boolean.
        let state = SelectState::initial(with!(move |selected, options| {
            options.iter().position(|opt| &opt.key == selected)
        }))
        .signal();

        selected.setter(
            state.map(|state| state.selected).maybe_reactive(),
            move |selected, position| {
                if let Some(option) = position.and_then(|pos| {
                    options.with(|options| {
                        options.get(pos).map(|opt| opt.key.clone())
                    })
                }) {
                    *selected = option;
                }
            },
        );

        Self {
            layout: Layout::new(
                LayoutKind::Flex(
                    FlexLayout::base(
                        Dir::AXIS,
                        options.map(|options| {
                            options.iter().map(|opt| opt.el.layout()).collect()
                        }),
                    )
                    .block_model(BlockModel::zero().padding(1u32))
                    .gap(Dir::AXIS.canon(5, 0))
                    .align_main(Align::Center)
                    .align_cross(Align::Center),
                ),
                Dir::AXIS.canon(
                    Length::InfiniteWindow(Length::Shrink.try_into().unwrap()),
                    Length::Shrink,
                ),
            ),
            state,
            style: None,
            options,
            dir: PhantomData,
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
    //                 .with(|options| options.get(pos).map(|opt| opt.key.clone()))
    //             {
    //                 *value = opt
    //             }
    //         }
    //     });

    //     self
    // }
}

impl<W: WidgetCtx, K, Dir> BlockModelWidget<W> for Select<W, K, Dir>
where
    K: PartialEq + Display + 'static,
    Dir: Direction + 'static,
{
}

impl<W: WidgetCtx, K, Dir> SizedWidget<W> for Select<W, K, Dir>
where
    K: PartialEq + Clone + Display + 'static,
    Dir: Direction + 'static,
{
}

impl<W: WidgetCtx, K, Dir> FontSettingWidget<W> for Select<W, K, Dir>
where
    K: PartialEq + Clone + Display + 'static,
    Dir: Direction + 'static,
{
}

impl<W: WidgetCtx, K: PartialEq + 'static, Dir: Direction + 'static> Widget<W>
    for Select<W, K, Dir>
{
    fn debug_name(&self) -> &'static str {
        "Select"
    }

    fn build(&mut self, ctx: BuildCtx<W>) {
        let _ = ctx;
    }

    fn layout(&self) -> Layout {
        self.layout
    }

    #[track_caller]
    fn render(&self, mut ctx: RenderCtx<'_, W>) -> RenderResult {
        let children_layouts = ctx.layout.children().collect::<Vec<_>>();

        ctx.render_self("Select", |mut ctx| {
            let style = ctx.get_style(|t| t.select, self.style.as_deref());
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
                            Dir::AXIS.inverted(),
                            ctx.layout.inner.size.cross(Dir::AXIS),
                            Anchor::Center,
                        ),
                    BlockModel::zero().border_width(1),
                    style.selected,
                )
                .render(ctx.renderer())?;
            }

            // TODO: Review if focus outline visible
            ctx.render_focus_outline(ctx.id)
        })?;

        ctx.render_part("options", |mut ctx| {
            let state = self.state.get();
            let style = ctx.get_style(|t| t.select, self.style.as_deref());
            let (options_offset, _) =
                state.options_offset(ctx.layout.inner, &children_layouts);

            self.options.with(move |options| {
                ctx.clip_inner(|mut ctx| {
                    options
                        .iter()
                        .zip_eq(children_layouts.iter())
                        .enumerate()
                        .try_for_each(|(index, (option, option_layout))| {
                            // TODO: Need to thing how to properly handle select widget. Should options be real widgets or hidden inside Select just to render? Maybe we even don't need to have real Text widgets, instead storing only text and rendering it through renderer, but then we'll probably lose some text properties handling.
                            todo!()
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

        ctx.handle_focusable(|ctx, pressed| {
            // TODO: Generalize
            if state.pressed != pressed {
                let toggle_active = !state.pressed && pressed;

                self.state.update(|state| {
                    state.pressed = pressed;
                    if toggle_active {
                        state.active = !state.active;
                    }
                });

                ctx.capture()
            } else {
                ctx.ignore()
            }
        })
    }
}
