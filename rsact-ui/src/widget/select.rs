use super::{
    container::Container,
    mono_text::{MonoText, MonoTextStyle},
};
use crate::{
    declare_widget_style,
    layout::LayoutKind,
    render::Renderable,
    style::{ColorStyle, WidgetStylist},
    widget::{prelude::*, Meta, MetaTree},
};
use alloc::string::ToString;
use core::{fmt::Display, marker::PhantomData};
use embedded_graphics::prelude::{Point, Transform};
use layout::{axis::Anchor, flex::flex_content_size, size::RectangleExt};
use rsact_reactive::{maybe::IntoMaybeReactive, memo_chain::IntoMemoChain};

pub trait SelectEvent {
    fn as_select(&self, axis: Axis) -> Option<i32>;
}

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
        W::Styler: WidgetStylist<MonoTextStyle<W::Color>>,
        K: Display,
    {
        let string = key.to_string();
        SelectOption {
            key,
            el: Container::new(MonoText::new_inert(string).el())
                .padding(5u32)
                .el(),
        }
    }

    pub fn widget(&self) -> &impl Widget<W> {
        &self.el
    }
}

impl<W: WidgetCtx, K: PartialEq> PartialEq for SelectOption<W, K> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

pub struct Select<W: WidgetCtx, K: PartialEq + 'static, Dir: Direction> {
    id: ElId,
    layout: Signal<Layout>,
    state: Signal<SelectState>,
    style: MemoChain<SelectStyle<W::Color>>,
    options: Memo<Vec<SelectOption<W, K>>>,
    dir: PhantomData<Dir>,
}

impl<W, K> Select<W, K, ColDir>
where
    W: WidgetCtx,
    W::Styler: WidgetStylist<MonoTextStyle<W::Color>>,
    K: PartialEq + Clone + Display + 'static,
{
    pub fn vertical(
        selected: impl Into<MaybeSignal<K>>,
        options: impl IntoMemo<Vec<K>>,
    ) -> Self {
        Self::new(selected, options)
    }
}

impl<W, K> Select<W, K, RowDir>
where
    W::Styler: WidgetStylist<MonoTextStyle<W::Color>>,
    W: WidgetCtx,
    K: PartialEq + Clone + Display + 'static,
{
    pub fn horizontal(
        selected: impl Into<MaybeSignal<K>>,
        options: impl IntoMemo<Vec<K>>,
    ) -> Self {
        Self::new(selected, options)
    }
}

impl<W, K, Dir> Select<W, K, Dir>
where
    K: PartialEq + Clone + Display + 'static,
    W: WidgetCtx,
    W::Styler: WidgetStylist<MonoTextStyle<W::Color>>,
    Dir: Direction,
{
    // TODO: MaybeReactive options
    pub fn new(
        selected: impl Into<MaybeSignal<K>>,
        options: impl IntoMemo<Vec<K>>,
    ) -> Self {
        let options: Memo<Vec<_>> = options.memo().map(|options| {
            options
                .into_iter()
                .cloned()
                .map(|opt| SelectOption::new(opt))
                .collect()
        });

        let mut selected: MaybeSignal<K> = selected.into();

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
            id: ElId::unique(),
            layout: Layout::shrink(LayoutKind::Flex(
                FlexLayout::base(
                    Dir::AXIS,
                    options.map(|options| {
                        flex_content_size(
                            Dir::AXIS,
                            options.iter().map(SelectOption::widget),
                        )
                    }),
                )
                .gap(Dir::AXIS.canon(5, 0))
                .align_main(Align::Center)
                .align_cross(Align::Center),
            ))
            .signal(),
            state,
            style: SelectStyle::base().memo_chain(),
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

impl<W, K, Dir> BlockModelWidget<W> for Select<W, K, Dir>
where
    W::Event: SelectEvent,
    W: WidgetCtx,
    K: PartialEq + Display + 'static,
    Dir: Direction,
    W::Styler: WidgetStylist<SelectStyle<W::Color>>,
{
}

impl<W, K, Dir> SizedWidget<W> for Select<W, K, Dir>
where
    W::Event: SelectEvent,
    W: WidgetCtx,
    K: PartialEq + Clone + Display + 'static,
    Dir: Direction,
    W::Styler: WidgetStylist<SelectStyle<W::Color>>,
{
}

impl<W: WidgetCtx, K: PartialEq + 'static, Dir: Direction> Widget<W>
    for Select<W, K, Dir>
where
    W::Event: SelectEvent,
    W::Styler: WidgetStylist<SelectStyle<W::Color>>,
{
    fn meta(&self) -> MetaTree {
        let id = self.id;
        MetaTree::childless(create_memo(move |_| Meta::focusable(id)))
    }

    fn on_mount(&mut self, ctx: crate::widget::MountCtx<W>) {
        ctx.accept_styles(self.style, self.state);
    }

    fn layout(&self) -> Signal<Layout> {
        self.layout
    }

    fn build_layout_tree(&self) -> rsact_reactive::prelude::MemoTree<Layout> {
        MemoTree {
            data: self.layout.memo(),
            children: self
                .options
                .map(|options| {
                    options
                        .iter()
                        .map(|opt| opt.el.build_layout_tree())
                        .collect()
                })
                .memo(),
        }
    }

    fn draw(&self, ctx: &mut DrawCtx<'_, W>) -> DrawResult {
        let style = self.style.get();
        let state = self.state.get();

        let children_layouts = ctx.layout.children().collect::<Vec<_>>();

        let options_offset = if let Some(selected) = state.selected {
            let selected_child_layout = children_layouts.get(selected).unwrap();

            let options_offset =
                ctx.layout.inner.center_offset_of(selected_child_layout.inner);

            Block::from_layout_style(
                selected_child_layout
                    .inner
                    .translate(options_offset)
                    .resized_axis(
                        Dir::AXIS.inverted(),
                        ctx.layout.inner.size.cross(Dir::AXIS),
                        Anchor::Center,
                    ),
                BlockModel::zero().border_width(1),
                style.selected,
            )
            .render(ctx.renderer)?;

            options_offset
        } else if let Some(first_option) = children_layouts.first() {
            ctx.layout.inner.center_offset_of(first_option.inner)
        } else {
            Point::zero()
        };

        // TODO: Review if focus outline visible
        ctx.draw_focus_outline(self.id)?;

        self.options.with(move |options| {
            ctx.renderer.clipped(ctx.layout.inner, |renderer| {
                options
                    .iter()
                    .zip(ctx.layout.children())
                    .enumerate()
                    .try_for_each(|(index, (option, option_layout))| {
                        let mut ctx = DrawCtx {
                            state: ctx.state,
                            renderer,
                            layout: &option_layout.translate(options_offset),
                            tree_style: ctx.tree_style.text_color(
                                if Some(index) == state.selected {
                                    style.selected_text_color
                                } else {
                                    style.text_color
                                }
                                .get(),
                            ),
                        };
                        option.widget().draw(&mut ctx)
                    })
            })
        })
    }

    fn on_event(&mut self, ctx: &mut EventCtx<'_, W>) -> EventResponse<W> {
        let state = self.state.get();

        if state.active && ctx.is_focused(self.id) {
            if let Some(mut offset) = ctx.event.as_select(Dir::AXIS) {
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
                        (current as i32 + offset).clamp(
                            0,
                            self.options
                                .with(|options| options.len().saturating_sub(1))
                                as i32,
                        ) as usize
                    });

                if current != new {
                    self.state.update(|state| state.selected = new);
                }

                return ctx.capture();
            }
        }

        ctx.handle_focusable(self.id, |ctx, pressed| {
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
